//! Explicit stock-episode assignment. Compute all text changes before any I/O.
use crate::core::{
    features::{FileEdit, Project, TextEdit, WorkspaceEdit},
    id::PartialSceneId,
};
pub struct Assignment {
    pub edit: WorkspaceEdit,
    pub renames: Vec<(String, String)>,
}
pub fn assign(project: &Project, episode: u16, chapter: u16) -> Result<Assignment, &'static str> {
    if episode == 0 || chapter == 0 {
        return Err("a nonzero episode and chapter are required");
    }
    let old = project.contents_path(0, episode);
    if project.document(&old).is_none() {
        return Err("stock episode does not exist");
    }
    let next = crate::core::features::plots::episodes(project, chapter)
        .into_iter()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .filter(|n| *n <= 999)
        .ok_or("episode limit")?;
    let mut renames = Vec::new();
    for (from, to) in [
        (old, project.contents_path(chapter, next)),
        (
            project.episode_plot_path(0, episode),
            project.episode_plot_path(chapter, next),
        ),
    ] {
        if project.document(&to).is_some() {
            return Err("assignment destination exists");
        }
        if project.document(&from).is_some() {
            renames.push((from, to));
        }
    }
    let re = regex::Regex::new(r"@[0-9]+(?:-[0-9]+){1,3}").unwrap();
    let mut changes = Vec::new();
    for doc in project.documents() {
        let mut edits = Vec::new();
        for found in re.find_iter(&doc.text) {
            let line_start = doc.text[..found.start()].rfind('\n').map_or(0, |at| at + 1);
            let line_end = doc.text[found.start()..]
                .find('\n')
                .map_or(doc.text.len(), |at| found.start() + at);
            let line = &doc.text[line_start..line_end];
            let local = found.start() - line_start;
            if line.trim_start().starts_with("# ")
                || crate::core::text::lexer::find_line_comment(line).is_some_and(|at| at <= local)
                || crate::core::text::lexer::find_inline_comment(line)
                    .is_some_and(|range| range.contains(&local))
                || doc.text[..found.start()]
                    .chars()
                    .rev()
                    .take_while(|ch| *ch == '\\')
                    .count()
                    % 2
                    == 1
            {
                continue;
            }
            let Ok(id) = PartialSceneId::parse(&found.as_str()[1..]) else {
                continue;
            };
            let mut full = id.expand_low();
            if full.chapter != 0 || full.episode != episode {
                continue;
            }
            full.chapter = chapter;
            full.episode = next;
            let text = full
                .display(project.digits())
                .split('-')
                .take(id.depth())
                .collect::<Vec<_>>()
                .join("-");
            edits.push(TextEdit {
                range: found.range(),
                new_text: format!("@{text}"),
            });
        }
        if !edits.is_empty() {
            changes.push(FileEdit {
                path: doc.path.clone(),
                edits,
            });
        }
    }
    Ok(Assignment {
        edit: WorkspaceEdit { changes },
        renames,
    })
}
