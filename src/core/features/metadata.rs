use crate::core::{
    features::Project,
    id::SceneId,
    syntax::{plot_episode, plot_outline},
    text::lexer::FieldKey,
};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Record {
    pub id: SceneId,
    pub depth: usize,
    pub path: String,
    pub range: Range<usize>,
    pub fields: BTreeMap<String, String>,
}
fn key_name(key: FieldKey) -> &'static str {
    crate::core::text::lexer::FIELD_KEYS
        .iter()
        .find(|(k, _, _, _)| *k == key)
        .map_or("", |(_, s, _, _)| *s)
}
/// プロジェクト全体のメタデータ（§9.5）を、**変わったプロットだけ**作り直して返す（B13-5）。
///
/// 毎回全ファイルをパースし直すと、費用がプロジェクトの大きさに比例して伸びる
/// （`REF:docs/plans.md#perf-b13-5`）。中身は [`Project::plot_records`] のキャッシュ。
pub fn records(project: &Project) -> Arc<Vec<Record>> {
    project.plot_records()
}

/// [`records`] の中身。キャッシュが空のときだけ走る。
pub(crate) fn build_records(project: &Project) -> Vec<Record> {
    let mut out = Vec::new();
    for doc in project.documents() {
        match doc.format {
            crate::core::model::document::DocumentFormat::PlotOutline => {
                let tree = plot_outline::parse_with(&doc.text, &project.outline_options(&doc.path));
                for block in &tree.blocks {
                    let Some(id) = block.id else { continue };
                    let fields = block
                        .fields
                        .iter()
                        .filter_map(|n| tree.field(*n))
                        .map(|f| (key_name(f.key).to_owned(), f.full_value()))
                        .collect();
                    let starts = crate::core::features::project::line_starts_of(&doc.text);
                    out.push(Record {
                        id,
                        depth: block.depth,
                        path: doc.path.clone(),
                        range: starts.get(block.header).copied().unwrap_or(0)
                            ..starts
                                .get(block.span.end)
                                .copied()
                                .unwrap_or(doc.text.len()),
                        fields,
                    });
                }
            }
            crate::core::model::document::DocumentFormat::PlotEpisode => {
                let tree = plot_episode::parse_with(&doc.text, &project.contents_options());
                let starts = crate::core::features::project::line_starts_of(&doc.text);
                for scene in &tree.scenes {
                    let Some(id) = scene.id else { continue };
                    let fields = scene
                        .fields
                        .iter()
                        .filter_map(|n| tree.field(*n))
                        .map(|f| (key_name(f.key).to_owned(), f.full_value()))
                        .collect();
                    out.push(Record {
                        id,
                        depth: 3,
                        path: doc.path.clone(),
                        range: starts.get(scene.header).copied().unwrap_or(0)
                            ..starts
                                .get(scene.span.end)
                                .copied()
                                .unwrap_or(doc.text.len()),
                        fields,
                    });
                    for section in &scene.sections {
                        let fields = section
                            .fields
                            .iter()
                            .filter_map(|n| tree.field(*n))
                            .map(|f| (key_name(f.key).to_owned(), f.full_value()))
                            .collect();
                        out.push(Record {
                            id: SceneId {
                                section: section.number,
                                ..id
                            },
                            depth: 4,
                            path: doc.path.clone(),
                            range: starts.get(section.span.start).copied().unwrap_or(0)
                                ..starts
                                    .get(section.span.end)
                                    .copied()
                                    .unwrap_or(doc.text.len()),
                            fields,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out.sort_by_key(|r| (r.id, r.depth));
    out
}
pub fn inherited(rows: &[Record], id: SceneId, key: &str) -> Option<String> {
    rows.iter()
        .filter(|r| {
            r.id.chapter == id.chapter
                && (r.depth < 2 || r.id.episode == id.episode)
                && (r.depth < 3 || r.id.scene == id.scene)
                && (r.depth < 4 || r.id.section == id.section)
        })
        .filter_map(|r| r.fields.get(key).map(|v| (r.depth, v)))
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, v)| v.clone())
}
pub fn chapter_blurb(project: &Project, chapter: u16) -> String {
    records(project)
        .iter()
        .find(|r| r.id.chapter == chapter && r.depth == 2 && r.id.episode == 0)
        .and_then(|r| r.fields.get("blurb"))
        .cloned()
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq)]
pub struct When {
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub clock: u16,
    pub flashback: bool,
}
/// i18n: format-vocabulary
pub fn when(value: &str, previous: Option<f64>) -> Option<When> {
    let flashback = FLASHBACK.iter().any(|word| value.contains(word));
    if value.trim().starts_with('?') {
        return Some(When {
            start: None,
            end: None,
            clock: 0,
            flashback,
        });
    }
    fn number(s: &str) -> Option<f64> {
        let part: String = s
            .trim()
            .chars()
            .take_while(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))
            .collect();
        part.parse::<f64>().ok().filter(|n| n.is_finite())
    }
    let (left, right) = value
        .split_once("..")
        .map_or((value, None), |(l, r)| (l, Some(r)));
    let n = number(left)?;
    let relative = left.trim_start().starts_with(['+', '-']);
    let start = if relative { previous? + n } else { n };
    let clock = left
        .split_whitespace()
        .filter_map(|part| part.split_once(':'))
        .find_map(|(h, m)| {
            let h: u16 = h.parse().ok()?;
            let m: u16 = m.parse().ok()?;
            (h < 24 && m < 60).then_some(h * 60 + m)
        })
        .unwrap_or(0);
    Some(When {
        start: Some(start),
        end: right.and_then(number),
        clock,
        flashback,
    })
}
#[derive(Debug, Clone)]
pub struct Timeline {
    pub record: Record,
    pub time: When,
}
pub fn timeline(rows: &[Record]) -> Vec<Timeline> {
    let mut previous = BTreeMap::new();
    let mut result = Vec::new();
    for row in rows {
        if let Some(mut time) = row
            .fields
            .get("when")
            .and_then(|value| when(value, previous.get(&row.depth).copied()))
        {
            if let Some(start) = time.start {
                previous.insert(row.depth, start);
            }
            time.flashback |= row
                .fields
                .get("note")
                .is_some_and(|note| is_flashback(note));
            result.push(Timeline {
                record: row.clone(),
                time,
            });
        }
    }
    result
}
/// i18n: format-vocabulary
fn is_flashback(text: &str) -> bool {
    FLASHBACK.iter().any(|w| text.contains(w))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub speaker: String,
    pub mode: Option<String>,
    pub target: String,
    pub names: Vec<String>,
    pub line: usize,
}
pub fn addresses(text: &str) -> Vec<Address> {
    let mut speaker = "";
    let mut mode = None;
    let mut out = Vec::new();
    for (line, raw) in text.lines().enumerate() {
        if let Some(name) = raw.strip_prefix("# ") {
            speaker = name.trim();
            mode = None;
        }
        if let Some(name) = raw.strip_prefix("### ") {
            mode = name.split_whitespace().next().map(str::to_owned);
        }
        if raw.starts_with("## ") {
            mode = None;
        }
        let crate::core::text::lexer::LineKind::Field(field) =
            crate::core::text::lexer::classify(raw, crate::core::text::lexer::TextFormat::Markdown)
        else {
            continue;
        };
        if field.key != FieldKey::Address {
            continue;
        }
        let value = crate::core::text::lexer::strip_line_comment(field.value)
            .split(" #")
            .next()
            .unwrap_or(field.value);
        let Some((target, names)) = value
            .split_once("=>")
            .or_else(|| value.split_once('→'))
            .or_else(|| value.split_once(':'))
        else {
            continue;
        };
        let target = target.trim().trim_start_matches('\'');
        out.push(Address {
            speaker: speaker.to_owned(),
            mode: mode.clone(),
            target: target.to_owned(),
            names: names
                .split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect(),
            line,
        });
    }
    out
}
/// 安全なプロジェクト内の相対パスだけを返す。中身の解釈はしない（§5.3.6・§5.5.3）。
pub fn asset_path(value: &str, icon: bool) -> Option<String> {
    let value = value.strip_prefix("./").unwrap_or(value);
    let path = crate::core::policy::ProjectPath::new(value).ok()?;
    if icon && !path.as_str().starts_with("assets/") {
        return None;
    }
    Some(path.as_str().to_owned())
}

/// i18n: format-vocabulary
const MODES_HEADING_JA: &str = "モード";
/// i18n: format-vocabulary
const SENSE_SIGHT_JA: &str = "視覚";
/// i18n: format-vocabulary
const SENSE_HEARING_JA: &str = "聴覚";
/// i18n: format-vocabulary
const SENSE_SMELL_JA: &str = "嗅覚";
/// i18n: format-vocabulary
const SENSE_TASTE_JA: &str = "味覚";
/// i18n: format-vocabulary
const SENSE_TOUCH_JA: &str = "触覚";

/// i18n: format-vocabulary
const FLASHBACK: &[&str] = &["回想", "過去", "flashback"];

/// 住所の欄を 1 つだけ差し替える（基底モードの欄が無ければ挿入する）。他の欄には触れない。
pub fn set_address(text: &str, speaker: &str, target: &str, names: &str) -> Option<String> {
    set_address_with_mode(text, speaker, None, target, names)
}

/// モード別にも対応した `address:` の差し替え・挿入・削除（§10.6.1）。
/// `names` が空（空白のみ）なら該当する `address:` 行を削除する。
pub fn set_address_with_mode(
    text: &str,
    speaker: &str,
    mode: Option<&str>,
    target: &str,
    names: &str,
) -> Option<String> {
    if names.contains(['\n', '\r']) {
        return None;
    }
    let trimmed = names.trim();
    let found = addresses(text)
        .into_iter()
        .find(|a| a.speaker == speaker && a.target == target && a.mode.as_deref() == mode);
    let starts = crate::core::features::project::line_starts_of(text);
    if trimmed.is_empty() {
        let Some(address) = found else {
            let heading = format!("# {speaker}");
            return text
                .lines()
                .any(|raw| raw.trim() == heading)
                .then(|| text.to_owned());
        };
        let start = *starts.get(address.line)?;
        let end = starts.get(address.line + 1).copied().unwrap_or(text.len());
        return Some(format!("{}{}", &text[..start], &text[end..]));
    }
    let field = format!("address: {target} => {trimmed}");
    if let Some(address) = found {
        let start = *starts.get(address.line)?;
        let end = starts.get(address.line + 1).copied().unwrap_or(text.len());
        let original = &text[start..end];
        let comment = crate::core::text::lexer::find_line_comment(original)
            .map(|at| format!(" {}", original[at..].trim_end()))
            .unwrap_or_default();
        return Some(format!(
            "{}{}{}\n{}",
            &text[..start],
            field,
            comment,
            &text[end..]
        ));
    }
    let heading = format!("# {speaker}");
    let lines: Vec<&str> = text.lines().collect();
    let speaker_line = lines.iter().position(|raw| raw.trim() == heading)?;
    let speaker_end_line = lines
        .iter()
        .enumerate()
        .skip(speaker_line + 1)
        .find_map(|(i, raw)| raw.starts_with("# ").then_some(i))
        .unwrap_or(lines.len());
    let byte_at = |line_idx: usize| starts.get(line_idx).copied().unwrap_or(text.len());

    match mode {
        None => {
            let mut insert_line = speaker_end_line;
            for (i, raw) in lines
                .iter()
                .enumerate()
                .take(speaker_end_line)
                .skip(speaker_line + 1)
            {
                if raw.starts_with('#') {
                    insert_line = i;
                    break;
                }
            }
            let insert = byte_at(insert_line);
            let separator = if insert > 0 && !text[..insert].ends_with('\n') {
                "\n"
            } else {
                ""
            };
            Some(format!(
                "{}{separator}{field}\n{}",
                &text[..insert],
                &text[insert..]
            ))
        }
        Some(mode_name) => {
            let mode_line = lines
                .iter()
                .enumerate()
                .take(speaker_end_line)
                .skip(speaker_line + 1)
                .find_map(|(i, raw)| {
                    raw.strip_prefix("### ")
                        .and_then(|rest| rest.split_whitespace().next())
                        .filter(|&id| id == mode_name)
                        .map(|_| i)
                });
            if let Some(m_line) = mode_line {
                let m_end = lines
                    .iter()
                    .enumerate()
                    .take(speaker_end_line)
                    .skip(m_line + 1)
                    .find_map(|(i, raw)| raw.starts_with('#').then_some(i))
                    .unwrap_or(speaker_end_line);
                let insert = byte_at(m_end);
                let separator = if insert > 0 && !text[..insert].ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                return Some(format!(
                    "{}{separator}{field}\n{}",
                    &text[..insert],
                    &text[insert..]
                ));
            }
            let modes_sec = lines
                .iter()
                .enumerate()
                .take(speaker_end_line)
                .skip(speaker_line + 1)
                .find_map(|(i, raw)| {
                    raw.strip_prefix("## ")
                        .map(str::trim)
                        .filter(|&s| s == "modes" || s == MODES_HEADING_JA)
                        .map(|_| i)
                });
            if let Some(sec_line) = modes_sec {
                let sec_end = lines
                    .iter()
                    .enumerate()
                    .take(speaker_end_line)
                    .skip(sec_line + 1)
                    .find_map(|(i, raw)| raw.starts_with("## ").then_some(i))
                    .unwrap_or(speaker_end_line);
                let insert = byte_at(sec_end);
                let separator = if insert > 0 && !text[..insert].ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                Some(format!(
                    "{}{separator}### {mode_name}\n{field}\n{}",
                    &text[..insert],
                    &text[insert..]
                ))
            } else {
                let insert = byte_at(speaker_end_line);
                let separator = if insert > 0 && !text[..insert].ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                Some(format!(
                    "{}{separator}## modes\n### {mode_name}\n{field}\n{}",
                    &text[..insert],
                    &text[insert..]
                ))
            }
        }
    }
}

/// 既存の呼称リストに新しい呼称候補 `surface` を 1 件追加する（§10.6.1）。
pub fn add_address_candidate(
    text: &str,
    speaker: &str,
    mode: Option<&str>,
    target: &str,
    surface: &str,
) -> Option<String> {
    let surface = surface.trim();
    if surface.is_empty() {
        return None;
    }
    let existing = addresses(text)
        .into_iter()
        .find(|a| a.speaker == speaker && a.target == target && a.mode.as_deref() == mode);
    let merged = match existing {
        Some(mut addr) => {
            if !addr.names.iter().any(|n| n == surface) {
                addr.names.push(surface.to_owned());
            }
            addr.names.join(", ")
        }
        None => surface.to_owned(),
    };
    set_address_with_mode(text, speaker, mode, target, &merged)
}

/// W084（呼称違反）の 1 箇所（§10.6.1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressViolation {
    pub speaker: String,
    pub mode: Option<String>,
    pub target: String,
    pub path: String,
    pub byte: usize,
    pub scene: Option<SceneId>,
    pub line_text: String,
}

/// プロジェクト全体の W084（呼称違反）箇所をペア情報付きで列挙する（§10.6.1）。
pub fn address_violations(project: &Project) -> Vec<AddressViolation> {
    use crate::core::model::document::DocumentFormat;
    use crate::core::resolve::role_language;
    use crate::core::syntax::contents;

    let char_doc = project.text("characters.md");
    let addr_list = addresses(char_doc);
    let characters = project.characters();
    let mut out = Vec::new();

    for doc in project
        .documents()
        .filter(|d| d.format == DocumentFormat::Contents)
    {
        let tree = contents::parse(&doc.text);
        let mut previous = String::new();
        let mut offset = 0usize;
        for (line_idx, line) in tree.lines.iter().enumerate() {
            match line {
                contents::Line::Text(text) => {
                    let spans = role_language::dialogue_spans(text.body);
                    if spans.is_empty() && !text.body.trim().is_empty() {
                        previous.clear();
                    }
                    if !spans.is_empty() {
                        let speaker = text.speaker.as_ref().map_or(previous.as_str(), |s| s.name);
                        let name = speaker.to_owned();
                        let mode = text.speaker.as_ref().and_then(|s| s.modes.first().copied());
                        let own: Vec<_> = addr_list
                            .iter()
                            .filter(|a| {
                                a.speaker == name && (a.mode.is_none() || a.mode.as_deref() == mode)
                            })
                            .collect();
                        if !own.is_empty() {
                            let body: String =
                                spans.iter().map(|span| &text.body[span.clone()]).collect();
                            for target in &characters.characters {
                                if target.name == name {
                                    continue;
                                }
                                let choice = own
                                    .iter()
                                    .filter(|a| a.target == target.name || a.target == "*")
                                    .max_by_key(|a| (a.target != "*", a.mode.is_some()));
                                let Some(allowed) = choice else { continue };
                                if body.contains(target.name)
                                    && !allowed.names.iter().any(|n| body.contains(n))
                                {
                                    out.push(AddressViolation {
                                        speaker: name.clone(),
                                        mode: mode.map(str::to_owned),
                                        target: target.name.to_owned(),
                                        path: doc.path.clone(),
                                        byte: offset,
                                        scene: project.scene_at(doc, line_idx),
                                        line_text: text.body.trim().to_owned(),
                                    });
                                }
                            }
                        }
                        previous = name;
                    }
                }
                contents::Line::Scene(_) | contents::Line::Break(_) => {
                    previous.clear();
                }
                _ => {}
            }
            offset += line.raw().len() + 1;
        }
    }
    out
}

/// 本文の台詞から抽出した呼称候補（`Ctrl+Shift+A`、§10.6.1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressCandidate {
    pub speaker: String,
    pub mode: Option<String>,
    pub target: String,
    pub surface: String,
    pub count: usize,
    pub sample_path: String,
    pub sample_byte: usize,
    pub sample_scene: Option<SceneId>,
}

/// i18n: format-vocabulary
const HONORIFIC_SUFFIXES: &[&str] = &[
    "ちゃん",
    "お姉ちゃん",
    "お兄ちゃん",
    "姉さん",
    "兄さん",
    "先生",
    "先輩",
    "後輩",
    "さん",
    "くん",
    "さま",
    "殿",
    "様",
    "君",
    "氏",
    "姉",
    "兄",
    "嬢",
    "坊",
];

/// 本文の台詞を走査して、未登録の呼称候補を抽出する（自動では書き込まない、§10.6.1 MUST）。
pub fn extract_address_candidates(project: &Project) -> Vec<AddressCandidate> {
    use crate::core::model::document::DocumentFormat;
    use crate::core::resolve::role_language;
    use crate::core::syntax::contents;

    let char_doc = project.text("characters.md");
    let existing = addresses(char_doc);
    let characters = project.characters();
    let known_names: Vec<String> = characters
        .characters
        .iter()
        .map(|c| c.name.to_owned())
        .collect();
    if known_names.len() < 2 {
        return Vec::new();
    }

    // 各キャラの照合語幹（フルネーム・姓・名）。他キャラと重複しない 2 文字以上の部分語幹も含める。
    let mut stems_by_target: Vec<(String, Vec<String>)> = Vec::new();
    let mut part_counts: BTreeMap<String, usize> = BTreeMap::new();
    for name in &known_names {
        for part in name
            .split([' ', '　', '・'])
            .filter(|s| s.chars().count() >= 2)
        {
            *part_counts.entry(part.to_owned()).or_default() += 1;
        }
    }
    for name in &known_names {
        let mut stems = vec![name.clone()];
        for part in name.split([' ', '　', '・']) {
            if part != name
                && part.chars().count() >= 2
                && part_counts.get(part).copied().unwrap_or(0) == 1
            {
                stems.push(part.to_owned());
            }
        }
        stems.sort_by_key(|s| std::cmp::Reverse(s.len()));
        stems.dedup();
        stems_by_target.push((name.clone(), stems));
    }

    type Key = (String, Option<String>, String, String);
    let mut counts: BTreeMap<Key, (usize, String, usize, Option<SceneId>)> = BTreeMap::new();

    for doc in project
        .documents()
        .filter(|d| d.format == DocumentFormat::Contents)
    {
        let tree = contents::parse(&doc.text);
        let mut previous = String::new();
        let mut previous_mode: Option<String> = None;
        let mut offset = 0usize;
        for (line_idx, line) in tree.lines.iter().enumerate() {
            match line {
                contents::Line::Text(text) => {
                    let spans = role_language::dialogue_spans(text.body);
                    if spans.is_empty() && !text.body.trim().is_empty() {
                        previous.clear();
                        previous_mode = None;
                    }
                    if !spans.is_empty() {
                        let (speaker, mode) = match &text.speaker {
                            Some(sp) => (
                                sp.name.to_owned(),
                                sp.modes.first().map(|m| (*m).to_owned()),
                            ),
                            None => (previous.clone(), previous_mode.clone()),
                        };
                        if !speaker.is_empty() && known_names.iter().any(|n| n == &speaker) {
                            for span in &spans {
                                let utter = &text.body[span.clone()];
                                for (target, stems) in &stems_by_target {
                                    if target == &speaker {
                                        continue;
                                    }
                                    for stem in stems {
                                        let mut search_from = 0usize;
                                        while let Some(rel) =
                                            utter[search_from..].find(stem.as_str())
                                        {
                                            let start = search_from + rel;
                                            let after = start + stem.len();
                                            let rest = &utter[after..];
                                            let suffix = HONORIFIC_SUFFIXES
                                                .iter()
                                                .copied()
                                                .filter(|suf| rest.starts_with(suf))
                                                .max_by_key(|suf| suf.len())
                                                .unwrap_or("");
                                            let surface = format!("{stem}{suffix}");
                                            let already = existing.iter().any(|a| {
                                                a.speaker == speaker
                                                    && a.target == *target
                                                    && (a.mode == mode || a.mode.is_none())
                                                    && a.names.iter().any(|n| n == &surface)
                                            });
                                            if !already {
                                                let key = (
                                                    speaker.clone(),
                                                    mode.clone(),
                                                    target.clone(),
                                                    surface,
                                                );
                                                let entry =
                                                    counts.entry(key).or_insert_with(|| {
                                                        (
                                                            0,
                                                            doc.path.clone(),
                                                            offset,
                                                            project.scene_at(doc, line_idx),
                                                        )
                                                    });
                                                entry.0 += 1;
                                            }
                                            search_from = after + suffix.len();
                                            if search_from <= start {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        previous = speaker;
                        previous_mode = mode;
                    }
                }
                contents::Line::Scene(_) | contents::Line::Break(_) => {
                    previous.clear();
                    previous_mode = None;
                }
                _ => {}
            }
            offset += line.raw().len() + 1;
        }
    }

    counts
        .into_iter()
        .map(
            |(
                (speaker, mode, target, surface),
                (count, sample_path, sample_byte, sample_scene),
            )| {
                AddressCandidate {
                    speaker,
                    mode,
                    target,
                    surface,
                    count,
                    sample_path,
                    sample_byte,
                    sample_scene,
                }
            },
        )
        .collect()
}

/// タイムライン表示の 1 行（上位からの継承・回想・W080 矛盾を含む、§10.3.2）。
#[derive(Debug, Clone)]
pub struct TimelineViewEntry {
    pub record: Record,
    pub time: When,
    pub display_when: String,
    pub inherited: bool,
    /// 執筆順で前にある確定時刻ブロックより巻き戻っている場合、その比較元 ID（W080）。
    pub rewind_from: Option<SceneId>,
}

/// 継承・回想・W080 警告を含めたタイムライン表示データを構築する（§10.3.2・§5.3.5）。
/// プロジェクトに明示的な `when:` が 1 件も無ければ空を返す。
pub fn timeline_view(rows: &[Record]) -> Vec<TimelineViewEntry> {
    if !rows.iter().any(|r| r.fields.contains_key("when")) {
        return Vec::new();
    }
    let mut prev_by_depth: BTreeMap<usize, f64> = BTreeMap::new();
    let mut explicit_resolved: Vec<Option<(When, String)>> = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(raw) = row.fields.get("when") {
            if let Some(mut parsed) = when(raw, prev_by_depth.get(&row.depth).copied()) {
                if let Some(start) = parsed.start {
                    prev_by_depth.insert(row.depth, start);
                }
                parsed.flashback |= row
                    .fields
                    .get("note")
                    .is_some_and(|note| is_flashback(note));
                explicit_resolved.push(Some((parsed, raw.trim().to_owned())));
            } else {
                explicit_resolved.push(None);
            }
        } else {
            explicit_resolved.push(None);
        }
    }

    let mut entries = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        if let Some((time, display_when)) = explicit_resolved[idx].clone() {
            entries.push(TimelineViewEntry {
                record: row.clone(),
                time,
                display_when,
                inherited: false,
                rewind_from: None,
            });
            continue;
        }
        // 上位ブロック（章 → 話 → 場面）から最も深い明示 when を継承する
        let ancestor = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.depth < row.depth
                    && r.id.chapter == row.id.chapter
                    && (r.depth < 2 || r.id.episode == row.id.episode)
                    && (r.depth < 3 || r.id.scene == row.id.scene)
            })
            .filter_map(|(anc_idx, r)| {
                explicit_resolved[anc_idx]
                    .as_ref()
                    .map(|(t, disp)| (r.depth, t.clone(), disp.clone()))
            })
            .max_by_key(|(depth, _, _)| *depth);
        if let Some((_, mut time, display_when)) = ancestor {
            time.flashback |= row
                .fields
                .get("note")
                .is_some_and(|note| is_flashback(note));
            entries.push(TimelineViewEntry {
                record: row.clone(),
                time,
                display_when,
                inherited: true,
                rewind_from: None,
            });
        }
    }

    // 執筆順（rows は SceneId, depth 昇順）に対する巻き戻り（W080）を検出する
    let mut latest_by_depth: BTreeMap<usize, (f64, u16, SceneId)> = BTreeMap::new();
    for entry in &mut entries {
        if entry.inherited {
            continue;
        }
        if let Some(start) = entry.time.start {
            if !entry.time.flashback {
                if let Some(&(prev_start, prev_clock, prev_id)) =
                    latest_by_depth.get(&entry.record.depth)
                {
                    if (start, entry.time.clock) < (prev_start, prev_clock) {
                        entry.rewind_from = Some(prev_id);
                    }
                }
                let update = latest_by_depth
                    .get(&entry.record.depth)
                    .is_none_or(|&(ps, pc, _)| (start, entry.time.clock) >= (ps, pc));
                if update {
                    latest_by_depth.insert(
                        entry.record.depth,
                        (start, entry.time.clock, entry.record.id),
                    );
                }
            }
        }
    }

    // 縦軸: ordinal_key (start) 昇順 → clock 昇順 → SceneId 昇順。`?` (start = None) は末尾の順序未定帯。
    entries.sort_by(|a, b| {
        match (a.time.start, b.time.start) {
            (Some(sa), Some(sb)) => sa.total_cmp(&sb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| a.time.clock.cmp(&b.time.clock))
        .then_with(|| a.record.id.cmp(&b.record.id))
        .then_with(|| a.record.depth.cmp(&b.record.depth))
    });
    entries
}

/// 集計の範囲（章 / 話 / プロジェクト全体、§10.9.3・§10.9.5(b)）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MetricsScope {
    #[default]
    All,
    Chapter(u16),
    Episode(u16, u16),
}

impl MetricsScope {
    pub fn matches(self, id: SceneId) -> bool {
        match self {
            Self::All => id.chapter > 0,
            Self::Chapter(ch) => id.chapter == ch,
            Self::Episode(ch, ep) => id.chapter == ch && id.episode == ep,
        }
    }
}

/// プロジェクト内に存在する章番号一覧（ストック章 00 を除く昇順）。
pub fn chapters_in_project(project: &Project) -> Vec<u16> {
    let mut set = std::collections::BTreeSet::new();
    for r in records(project).iter() {
        if r.id.chapter > 0 {
            set.insert(r.id.chapter);
        }
    }
    for s in project.sections().iter() {
        if s.id.chapter > 0 {
            set.insert(s.id.chapter);
        }
    }
    set.into_iter().collect()
}

/// プロジェクト内に存在する `(章, 話)` 一覧（ストック章 00 を除く昇順）。
pub fn episodes_in_project(project: &Project) -> Vec<(u16, u16)> {
    let mut set = std::collections::BTreeSet::new();
    for r in records(project).iter() {
        if r.id.chapter > 0 && r.id.episode > 0 {
            set.insert((r.id.chapter, r.id.episode));
        }
    }
    for s in project.sections().iter() {
        if s.id.chapter > 0 && s.id.episode > 0 {
            set.insert((s.id.chapter, s.id.episode));
        }
    }
    set.into_iter().collect()
}

/// 五感の種別（§5.4.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenseKind {
    Sight = 0,
    Hearing = 1,
    Smell = 2,
    Taste = 3,
    Touch = 4,
}

impl SenseKind {
    pub const ALL: [Self; 5] = [
        Self::Sight,
        Self::Hearing,
        Self::Smell,
        Self::Taste,
        Self::Touch,
    ];

    pub fn parse_token(token: &str) -> Option<Self> {
        match token.trim() {
            t if t == SENSE_SIGHT_JA || t == "sight" => Some(Self::Sight),
            t if t == SENSE_HEARING_JA || t == "hearing" => Some(Self::Hearing),
            t if t == SENSE_SMELL_JA || t == "smell" => Some(Self::Smell),
            t if t == SENSE_TASTE_JA || t == "taste" => Some(Self::Taste),
            t if t == SENSE_TOUCH_JA || t == "touch" => Some(Self::Touch),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenseSectionHit {
    pub id: SceneId,
    pub path: String,
    pub byte: usize,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualStreak {
    pub start: SceneId,
    pub end: SceneId,
    pub count: usize,
    pub path: String,
    pub byte: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SensesSummary {
    pub total_sections: usize,
    pub declared_sections: usize,
    pub undeclared_sections: usize,
    pub counts: [usize; 5],
    pub sections_by_sense: [Vec<SenseSectionHit>; 5],
    pub visual_streaks: Vec<VisualStreak>,
}

/// `senses:` の宣言のみを集計し、視覚のみ 5 節以上連続（W083）の区間を列挙する（§10.9.3）。
pub fn senses_summary(project: &Project, scope: MetricsScope) -> SensesSummary {
    let rows = records(project);
    let mut out = SensesSummary::default();

    // 対象範囲のすべての節 ID（プロットの節 ∪ 本文の節）を順序通りに集める。
    let mut section_ids = std::collections::BTreeSet::new();
    for r in rows
        .iter()
        .filter(|r| r.depth == 4 && r.id.chapter > 0 && r.id.episode > 0 && scope.matches(r.id))
    {
        section_ids.insert(r.id);
    }
    for s in project
        .sections()
        .iter()
        .filter(|s| s.id.chapter > 0 && s.id.episode > 0 && scope.matches(s.id))
    {
        section_ids.insert(s.id);
    }

    let mut run_start: Option<(SceneId, String, usize)> = None;
    let mut run_end: Option<SceneId> = None;
    let mut run_count = 0usize;

    let flush_streak = |out: &mut SensesSummary,
                        start: &mut Option<(SceneId, String, usize)>,
                        end: &mut Option<SceneId>,
                        count: &mut usize| {
        if *count >= 5 {
            if let (Some((s_id, path, byte)), Some(e_id)) = (start.take(), end.take()) {
                out.visual_streaks.push(VisualStreak {
                    start: s_id,
                    end: e_id,
                    count: *count,
                    path,
                    byte,
                });
            }
        }
        *start = None;
        *end = None;
        *count = 0;
    };

    for sec_id in section_ids {
        out.total_sections += 1;
        // 節自身の senses:（第 1 節で見出し直下に書かれた場合は親場面 depth == 3 の senses:）
        let source_rec = rows
            .iter()
            .find(|r| r.depth == 4 && r.id == sec_id && r.fields.contains_key("senses"))
            .or_else(|| {
                (sec_id.section == 1).then(|| {
                    rows.iter().find(|r| {
                        r.depth == 3
                            && r.id.chapter == sec_id.chapter
                            && r.id.episode == sec_id.episode
                            && r.id.scene == sec_id.scene
                            && r.fields.contains_key("senses")
                    })
                })?
            });
        let Some((rec, raw)) = source_rec.and_then(|r| {
            r.fields
                .get("senses")
                .filter(|s| !s.trim().is_empty())
                .map(|s| (r, s.clone()))
        }) else {
            out.undeclared_sections += 1;
            continue;
        };
        let mut seen = [false; 5];
        for tok in raw.split([',', '、', ' ', '　']).filter(|s| !s.is_empty()) {
            if let Some(kind) = SenseKind::parse_token(tok) {
                seen[kind as usize] = true;
            }
        }
        if !seen.iter().any(|&b| b) {
            out.undeclared_sections += 1;
            continue;
        }
        out.declared_sections += 1;
        for kind in SenseKind::ALL {
            let idx = kind as usize;
            if seen[idx] {
                out.counts[idx] += 1;
                out.sections_by_sense[idx].push(SenseSectionHit {
                    id: sec_id,
                    path: rec.path.clone(),
                    byte: rec.range.start,
                    raw: raw.clone(),
                });
            }
        }
        let sight_only = seen[SenseKind::Sight as usize]
            && !seen[SenseKind::Hearing as usize]
            && !seen[SenseKind::Smell as usize]
            && !seen[SenseKind::Taste as usize]
            && !seen[SenseKind::Touch as usize];
        if sight_only {
            if run_start.is_none() {
                run_start = Some((sec_id, rec.path.clone(), rec.range.start));
            }
            run_end = Some(sec_id);
            run_count += 1;
        } else {
            flush_streak(&mut out, &mut run_start, &mut run_end, &mut run_count);
        }
    }
    flush_streak(&mut out, &mut run_start, &mut run_end, &mut run_count);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IllustrationSource {
    Plot,
    Contents,
}
pub struct Illustration {
    pub path: String,
    pub at: usize,
    pub id: Option<SceneId>,
    pub memo: String,
    pub source: IllustrationSource,
}
pub fn illustrations(project: &Project) -> Vec<Illustration> {
    let mut out = records(project)
        .iter()
        .cloned()
        .filter_map(|row| {
            row.fields
                .get("illustration")
                .filter(|v| !v.trim().is_empty())
                .map(|memo| Illustration {
                    path: row.path.clone(),
                    at: row.range.start,
                    id: Some(row.id),
                    memo: memo.clone(),
                    source: IllustrationSource::Plot,
                })
        })
        .collect::<Vec<_>>();
    for doc in project
        .documents()
        .filter(|d| d.format == crate::core::model::document::DocumentFormat::Contents)
    {
        let mut at = 0;
        let mut id = None;
        for raw in doc.text.split_inclusive('\n') {
            if let Some(header) = raw
                .strip_prefix('@')
                .and_then(|s| s.split_whitespace().next())
            {
                id = crate::core::id::PartialSceneId::parse(header)
                    .ok()
                    .map(|id| id.expand_low());
            }
            if let Some(memo) = crate::core::annotations::illustration(raw.trim_end()) {
                out.push(Illustration {
                    path: doc.path.clone(),
                    at,
                    id,
                    memo: memo.to_owned(),
                    source: IllustrationSource::Contents,
                });
            }
            at += raw.len();
        }
    }
    out.sort_by(|a, b| (&a.path, a.at).cmp(&(&b.path, b.at)));
    out
}
