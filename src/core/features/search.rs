//! Bounded search engine with an injected elapsed-time function.
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
        // Match one line at a time: regex uses a linear-time automaton, never unbounded backtracking.
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
    let mut changes: Vec<FileEdit> = Vec::new();
    for hit in &results.hits {
        let old = project
            .text(&hit.path)
            .get(hit.range.clone())
            .ok_or("stale search")?;
        if old != hit.matched {
            return Err("stale search".to_owned());
        }
        let new_text = if query.regex {
            re.replace(old, &expand_escapes(replacement)).into_owned()
        } else {
            expand_escapes(replacement)
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
/// 置換の入力に書かれた `\n` を改行にする（§9.7.4「`\n` は改行」）。
///
/// 置換の入力欄は 1 行なので、改行は書けず `\n` と打つ決まりになっている。
fn expand_escapes(replacement: &str) -> String {
    replacement.replace("\\n", "\n")
}

/// Freeze a search as a material; no live dependency on the manuscript remains.
/// i18n: format-vocabulary
pub fn snapshot(
    project: &Project,
    query: &Query,
    results: &Results,
    stamp: &str,
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
    let path = format!("materials/docs/{id}.md");
    let title = query.text.chars().take(70).collect::<String>();
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
    index.items.push(Material {
        id: id.clone(),
        title,
        path: path.clone(),
        category: SEARCH_CATEGORY.to_owned(),
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
                            (
                                section.header.unwrap_or(section.lines.start),
                                section.id.display(digits),
                            )
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
