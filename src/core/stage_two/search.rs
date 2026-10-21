//! Bounded search engine with an injected elapsed-time function.
use crate::core::features::{FileEdit, Project, TextEdit, WorkspaceEdit};
use std::ops::Range;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub regex: bool,
    pub case_sensitive: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub path: String,
    pub range: Range<usize>,
    pub excerpt: String,
    pub matched: String,
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
        .size_limit(2 * 1024 * 1024)
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
            && (d.path.ends_with(".txt") || d.path.ends_with(".md"))
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
                let suffix: String = line[found.end()..].chars().take(20).collect();
                out.hits.push(Hit {
                    path: doc.path.clone(),
                    range: start + found.start()..start + found.end(),
                    matched: found.as_str().to_owned(),
                    excerpt: format!("{prefix}**{}**{suffix}", found.as_str()),
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
            re.replace(old, replacement).into_owned()
        } else {
            replacement.to_owned()
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
    let body = format!(
        "# {title}\n\n{}",
        results
            .hits
            .iter()
            .map(|h| format!(
                "- {} @{}..{}\n  {}\n",
                h.path, h.range.start, h.range.end, h.excerpt
            ))
            .collect::<String>()
    );
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

/// i18n: format-vocabulary
const SEARCH_CATEGORY: &str = "検索結果";
