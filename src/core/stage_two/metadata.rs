use crate::core::{
    features::Project,
    id::SceneId,
    syntax::{plot_episode, plot_outline},
    text::lexer::FieldKey,
};
use std::collections::BTreeMap;
use std::ops::Range;

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
pub fn records(project: &Project) -> Vec<Record> {
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
/// Return only safe, project-relative references. No content interpretation.
pub fn asset_path(value: &str, icon: bool) -> Option<String> {
    let value = value.strip_prefix("./").unwrap_or(value);
    let path = crate::core::policy::ProjectPath::new(value).ok()?;
    if icon && !path.as_str().starts_with("assets/") {
        return None;
    }
    Some(path.as_str().to_owned())
}

/// i18n: format-vocabulary
const FLASHBACK: &[&str] = &["回想", "過去", "flashback"];

/// Replace one address field (or insert a base-mode field) without touching other fields.
pub fn set_address(text: &str, speaker: &str, target: &str, names: &str) -> Option<String> {
    if names.contains(['\n', '\r']) {
        return None;
    }
    let found = addresses(text)
        .into_iter()
        .find(|a| a.speaker == speaker && a.target == target && a.mode.is_none());
    let starts = crate::core::features::project::line_starts_of(text);
    let field = format!("address: {target} => {names}");
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
    let mut matched = false;
    let mut insert = text.len();
    for (line, raw) in text.lines().enumerate() {
        if raw.trim() == heading {
            matched = true;
            continue;
        }
        if matched && raw.starts_with('#') {
            insert = starts[line];
            break;
        }
    }
    if !matched {
        return None;
    }
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
        .into_iter()
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
