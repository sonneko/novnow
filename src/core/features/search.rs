//! 上限付きの検索。経過時間は外から注入する関数で測る（§9.7.4）。
use crate::core::features::{FileEdit, Project, TextEdit, WorkspaceEdit};
use std::ops::Range;
/// 検索の条件（§9.7.4）。
///
/// **大小文字は既定で区別する**（仕様 §9.7.4 の表。`Alt+C` または画面のチェックで
/// 無視に切り替える）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub regex: bool,
    pub case_sensitive: bool,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            text: String::new(),
            regex: false,
            case_sensitive: true,
        }
    }
}

/// コンパイル後の正規表現の大きさの上限（§9.7.4「1MB。超えたら構文エラー扱い」）。
const SIZE_LIMIT: usize = 1024 * 1024;
/// 検索のヒット。抜粋は「前文・一致・後文」の構造で持ち、表示用の記法
/// （強調や `**…**`）はデータへ埋め込まない（画面表示と資料保存で組み立てが違うため）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub path: String,
    pub range: Range<usize>,
    /// 一致の前の文脈（同じ行の最大 20 字）。
    pub before: String,
    pub matched: String,
    /// 一致の後の文脈（同じ行の最大 20 字）。
    pub after: String,
}
#[derive(Debug, Clone, Default)]
pub struct Results {
    pub hits: Vec<Hit>,
    pub timed_out: bool,
}
pub fn run(
    project: &Project,
    query: &Query,
    mut elapsed_ms: impl FnMut() -> u64,
) -> Result<Results, String> {
    let mut out = Results::default();
    if query.text.is_empty() {
        return Ok(out);
    }
    let pattern = if query.regex {
        query.text.clone()
    } else {
        regex::escape(&query.text)
    };
    let re = regex::RegexBuilder::new(&pattern)
        .case_insensitive(!query.case_sensitive)
        .size_limit(SIZE_LIMIT)
        .build()
        .map_err(|e| {
            e.to_string()
                .lines()
                .last()
                .unwrap_or("invalid expression")
                .to_owned()
        })?;
    for doc in project.documents().filter(|d| {
        !d.path.starts_with(".novnow/")
            && !d.path.starts_with("exports/")
            && (d.path.ends_with(".cnov") || d.path.ends_with(".md"))
    }) {
        // 1 行ずつ照合する。正規表現は線形時間のオートマトンで動くので、
        // 際限のない後戻りは起きない（§9.7.4）。
        let mut start = 0;
        for line in doc.text.split_inclusive('\n') {
            if elapsed_ms() >= 1000 {
                out.timed_out = true;
                return Ok(out);
            }
            for found in re.find_iter(line) {
                if elapsed_ms() >= 1000 {
                    out.timed_out = true;
                    return Ok(out);
                }
                let prefix: String = line[..found.start()]
                    .chars()
                    .rev()
                    .take(20)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                let suffix: String = line[found.end()..]
                    .trim_end_matches(['\n', '\r'])
                    .chars()
                    .take(20)
                    .collect();
                out.hits.push(Hit {
                    path: doc.path.clone(),
                    range: start + found.start()..start + found.end(),
                    before: prefix,
                    matched: found.as_str().to_owned(),
                    after: suffix,
                });
            }
            start += line.len();
        }
    }
    Ok(out)
}
pub fn replacement(
    project: &Project,
    query: &Query,
    replacement: &str,
    results: &Results,
) -> Result<WorkspaceEdit, String> {
    let pattern = if query.regex {
        query.text.clone()
    } else {
        regex::escape(&query.text)
    };
    let re = regex::RegexBuilder::new(&pattern)
        .case_insensitive(!query.case_sensitive)
        .size_limit(SIZE_LIMIT)
        .build()
        .map_err(|e| e.to_string())?;
    let expanded = expand_escapes(replacement);
    let mut changes: Vec<FileEdit> = Vec::new();
    for hit in &results.hits {
        let doc_text = project.text(&hit.path);
        let old = doc_text.get(hit.range.clone()).ok_or("stale search")?;
        if old != hit.matched {
            return Err("stale search".to_owned());
        }
        let new_text = if query.regex {
            let line_start = doc_text[..hit.range.start]
                .rfind('\n')
                .map_or(0, |at| at + 1);
            let line_end = doc_text[hit.range.end..]
                .find('\n')
                .map_or(doc_text.len(), |at| hit.range.end + at + 1)
                .min(doc_text.len());
            let line = &doc_text[line_start..line_end];
            let rel_range = (hit.range.start - line_start)..(hit.range.end - line_start);
            if let Some(caps) = re
                .captures_iter(line)
                .find(|c| c.get(0).is_some_and(|m| m.range() == rel_range))
            {
                let mut dst = String::new();
                caps.expand(&expanded, &mut dst);
                dst
            } else {
                re.replace(old, expanded.as_str()).into_owned()
            }
        } else {
            expanded.clone()
        };
        if changes.last().is_none_or(|f| f.path != hit.path) {
            changes.push(FileEdit {
                path: hit.path.clone(),
                edits: Vec::new(),
            });
        }
        changes.last_mut().unwrap().edits.push(TextEdit {
            range: hit.range.clone(),
            new_text,
        });
    }
    Ok(WorkspaceEdit { changes })
}

/// 1 つの文書に対する一括置換（§9.7.4。`$1`・`${name}` キャプチャ参照と `\n` 改行を展開し、
/// 複数一致を後ろから置換して範囲ずれを防ぐ）。戻り値は `(一致件数, 置換後全文)`。
pub fn replace_in_text(
    text: &str,
    query: &str,
    replacement: &str,
    regex: bool,
    case_sensitive: bool,
) -> Result<(usize, String), String> {
    if query.is_empty() {
        return Ok((0, text.to_owned()));
    }
    let expanded = expand_escapes(replacement);
    if !regex {
        let hits: Vec<Range<usize>> = if case_sensitive {
            text.match_indices(query)
                .map(|(at, found)| at..at + found.len())
                .collect()
        } else {
            let re = regex::RegexBuilder::new(&regex::escape(query))
                .case_insensitive(true)
                .size_limit(SIZE_LIMIT)
                .build()
                .map_err(|e| e.to_string())?;
            re.find_iter(text).map(|m| m.range()).collect()
        };
        let mut out = text.to_owned();
        for range in hits.iter().rev() {
            out.replace_range(range.clone(), &expanded);
        }
        return Ok((hits.len(), out));
    }
    let re = regex::RegexBuilder::new(query)
        .case_insensitive(!case_sensitive)
        .size_limit(SIZE_LIMIT)
        .build()
        .map_err(|e| e.to_string())?;
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for caps in re.captures_iter(text) {
        let Some(matched) = caps.get(0) else {
            continue;
        };
        let mut dst = String::new();
        caps.expand(&expanded, &mut dst);
        edits.push((matched.range(), dst));
    }
    let count = edits.len();
    let mut out = text.to_owned();
    for (range, new_text) in edits.into_iter().rev() {
        out.replace_range(range, &new_text);
    }
    Ok((count, out))
}

/// 置換の入力に書かれた `\n` を改行にする（§9.7.4「`\n` は改行」）。
///
/// 置換の入力欄は 1 行なので、改行は書けず `\n` と打つ決まりになっている。
pub fn expand_escapes(replacement: &str) -> String {
    replacement.replace("\\n", "\n")
}

/// 検索結果から資料ファイル名用の slug を作る（§16.3.1 `S0042-<slug>.md`）。
pub fn slug_of(input: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in input.trim().chars() {
        let safe = ch.is_alphanumeric() || ('\u{3040}'..='\u{9fff}').contains(&ch) || ch == '_';
        if safe {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
        if slug.chars().count() >= 32 {
            break;
        }
    }
    slug.trim_matches('-').to_owned()
}

/// 検索の結果を資料として凍結する。元の本文への生きた依存は残さない（§16.3.1）。
pub fn snapshot(
    project: &Project,
    query: &Query,
    results: &Results,
    stamp: &str,
) -> Result<crate::core::features::materials_ops::Created, String> {
    snapshot_with(project, query, results, stamp, "", "")
}

/// タイトルと任意の `summary` を指定して検索結果を資料化する（§16.3.1）。
pub fn snapshot_with(
    project: &Project,
    query: &Query,
    results: &Results,
    stamp: &str,
    custom_title: &str,
    summary: &str,
) -> Result<crate::core::features::materials_ops::Created, String> {
    use crate::core::model::material::{Material, MaterialIndex, Source, PREFIX_SEARCH};
    let text = project.text("materials/index.json");
    let mut index: MaterialIndex = if text.trim().is_empty() {
        MaterialIndex {
            schema_version: 1,
            next_id: 1,
            ..Default::default()
        }
    } else {
        serde_json::from_str(text).map_err(|e| e.to_string())?
    };
    let id = index.issue(PREFIX_SEARCH).to_string();
    let raw_title = if custom_title.trim().is_empty() {
        &query.text
    } else {
        custom_title.trim()
    };
    let title = raw_title
        .replace('\n', " ")
        .chars()
        .take(70)
        .collect::<String>();
    let slug = slug_of(&title);
    let path = if slug.is_empty() {
        format!("materials/docs/{id}.md")
    } else {
        format!("materials/docs/{id}-{slug}.md")
    };
    let body = snapshot_body(project, query, results, stamp, &id, &title);
    let mut source = Source {
        kind: "search".to_owned(),
        query: Some(query.text.clone()),
        searched_at: Some(stamp.to_owned()),
        ..Default::default()
    };
    source
        .unknown
        .insert("regex".to_owned(), serde_json::json!(query.regex));
    source.unknown.insert(
        "case_sensitive".to_owned(),
        serde_json::json!(query.case_sensitive),
    );
    let trimmed_summary = summary.trim();
    index.items.push(Material {
        id: id.clone(),
        title,
        path: path.clone(),
        category: SEARCH_CATEGORY.to_owned(),
        summary: (!trimmed_summary.is_empty()).then(|| trimmed_summary.to_owned()),
        source,
        status: "approved".to_owned(),
        added_by: "human".to_owned(),
        saved_at: stamp.to_owned(),
        ..Default::default()
    });
    let index = serde_json::to_string_pretty(&index).map_err(|e| e.to_string())? + "\n";
    Ok(crate::core::features::materials_ops::Created {
        id,
        path,
        body,
        index,
    })
}

/// 保存済み検索資料に対する再検索の新旧差分（§16.3.1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub unchanged_count: usize,
    pub new_body: String,
    pub new_hits: usize,
}

fn extract_result_bullets(md: &str) -> Vec<String> {
    let after = md
        .split_once(SNAPSHOT_RESULTS_HEADING)
        .map_or("", |(_, rest)| rest);
    after
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- "))
        .map(ToOwned::to_owned)
        .collect()
}

/// 保存済みの検索資料 MD と再実行した検索結果を比較し、新規ヒット / 消失ヒットと更新後 MD を返す。
pub fn rerun_diff(
    project: &Project,
    old_md: &str,
    id: &str,
    title: &str,
    query: &Query,
    results: &Results,
    stamp: &str,
) -> SearchDiff {
    let effective_title = if title.trim().is_empty() {
        old_md
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("# "))
            .unwrap_or(&query.text)
    } else {
        title.trim()
    };
    let self_exact = format!("materials/docs/{id}.md");
    let self_prefix = format!("materials/docs/{id}-");
    let filtered_results = Results {
        hits: results
            .hits
            .iter()
            .filter(|h| h.path != self_exact && !h.path.starts_with(&self_prefix))
            .cloned()
            .collect(),
        timed_out: results.timed_out,
    };
    let new_body = snapshot_body(
        project,
        query,
        &filtered_results,
        stamp,
        id,
        effective_title,
    );
    let old_bullets = extract_result_bullets(old_md);
    let new_bullets = extract_result_bullets(&new_body);
    let mut old_counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for line in &old_bullets {
        *old_counts.entry(line.as_str()).or_default() += 1;
    }
    let mut new_counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for line in &new_bullets {
        *new_counts.entry(line.as_str()).or_default() += 1;
    }
    let mut added = Vec::new();
    let mut unchanged_count = 0usize;
    let mut rem_old = old_counts.clone();
    for line in &new_bullets {
        if let Some(c) = rem_old.get_mut(line.as_str()) {
            if *c > 0 {
                *c -= 1;
                unchanged_count += 1;
                continue;
            }
        }
        added.push(line.clone());
    }
    let mut removed = Vec::new();
    let mut rem_new = new_counts;
    for line in &old_bullets {
        if let Some(c) = rem_new.get_mut(line.as_str()) {
            if *c > 0 {
                *c -= 1;
                continue;
            }
        }
        removed.push(line.clone());
    }
    SearchDiff {
        added,
        removed,
        unchanged_count,
        new_body,
        new_hits: filtered_results.hits.len(),
    }
}

/// 再検索結果を人間が採用したときに `materials/index.json` の `searched_at` / `updated_at` を更新する。
pub fn apply_rerun(index_text: &str, id: &str, stamp: &str) -> Result<String, String> {
    use crate::core::model::material::MaterialIndex;
    let mut index: MaterialIndex = serde_json::from_str(index_text).map_err(|e| e.to_string())?;
    let found = index
        .items
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("material not found: {id}"))?;
    found.source.searched_at = Some(stamp.to_owned());
    found.updated_at = Some(stamp.to_owned());
    Ok(serde_json::to_string_pretty(&index).map_err(|e| e.to_string())? + "\n")
}

/// 保存上限を超えたときの末尾の注記（§16.3.1 の固定形の一部）。
/// i18n: format-vocabulary
const SNAPSHOT_LIMIT_NOTE: &str = "\n（500 件まで保存）\n";

/// ヒット一覧の見出し（§16.3.1 の固定形の一部）。
/// i18n: format-vocabulary
const SNAPSHOT_RESULTS_HEADING: &str = "## 結果";

/// 資料として保存する MD の固定形（§16.3.1）。画面の強調とは別に、ここでだけ
/// 一致を `**…**` で囲み、節 ID 付きの箇条書きにする。保存は先頭 500 件まで。
fn snapshot_body(
    project: &Project,
    query: &Query,
    results: &Results,
    stamp: &str,
    id: &str,
    title: &str,
) -> String {
    const SAVED_LIMIT: usize = 500;
    let total = results.hits.len();
    let digits = project.digits();
    // パス → （開始行，節 ID 表示）の列。本文以外（プロット・資料）のヒットには節が無く、パスで示す。
    let mut sections_of: std::collections::HashMap<String, Vec<(usize, String)>> =
        std::collections::HashMap::new();
    let mut lines = String::new();
    for hit in results.hits.iter().take(SAVED_LIMIT) {
        let at = sections_of.entry(hit.path.clone()).or_insert_with(|| {
            project
                .documents()
                .find(|doc| doc.path == hit.path)
                .map(|doc| {
                    crate::core::features::sections::of_document(doc, project)
                        .iter()
                        .map(|section| {
                            let start_line = if section.id.section <= 1 {
                                section.header.unwrap_or(section.lines.start)
                            } else {
                                section.lines.start
                            };
                            (start_line, section.id.display(digits))
                        })
                        .collect()
                })
                .unwrap_or_default()
        });
        let line =
            crate::core::features::project::line_of(project.text(&hit.path), hit.range.start);
        let place = at
            .iter()
            .rev()
            .find(|(from, _)| *from <= line)
            .map_or_else(|| hit.path.clone(), |(_, id)| format!("@{id}"));
        lines.push_str(&format!(
            "- {place} … {}**{}**{}\n",
            hit.before, hit.matched, hit.after
        ));
    }
    if total > SAVED_LIMIT {
        lines.push_str(SNAPSHOT_LIMIT_NOTE);
    }
    format!(
        "# {title}\n<!-- novnow:{id} -->\n\nquery: {}\nregex: {}\ncase_sensitive: {}\nsearched_at: {stamp}\nhits: {total}\n\n{SNAPSHOT_RESULTS_HEADING}\n\n{lines}",
        query.text, query.regex, query.case_sensitive
    )
}

/// i18n: format-vocabulary
const SEARCH_CATEGORY: &str = "検索結果";
