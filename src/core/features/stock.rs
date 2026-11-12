//! ストックの話への明示的な配属と逆操作。入出力より前に、変える本文をすべて計算する（§10.3.3）。
use crate::core::{
    features::{FileEdit, Project, TextEdit, WorkspaceEdit},
    id::{PartialSceneId, SceneId},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub edit: WorkspaceEdit,
    pub renames: Vec<(String, String)>,
    pub target_chapter: u16,
    pub target_episode: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StockError {
    ZeroTarget,
    SameLocation,
    SourceMissing(String),
    DestinationExists(String),
    EpisodeLimit,
}

impl StockError {
    pub fn as_static_str(&self) -> &'static str {
        match self {
            Self::ZeroTarget => "a nonzero episode and chapter are required",
            Self::SameLocation => "source and destination are identical",
            Self::SourceMissing(_) => "stock episode does not exist",
            Self::DestinationExists(_) => "assignment destination exists",
            Self::EpisodeLimit => "episode limit",
        }
    }
}

/// ストック章（`A = 00`）の 1 話ぶんの一覧行情報（§10.3.3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockItem {
    pub episode: u16,
    pub title: String,
    pub summary: String,
    pub chars: usize,
    pub updated_at: String,
}

/// 章 `chapter`（`0` を含む）の次の空き話番号（末尾 + 1、無ければ 1）を返す。
pub fn next_episode_in_chapter(project: &Project, chapter: u16) -> Option<u16> {
    crate::core::features::plots::episodes(project, chapter)
        .into_iter()
        .filter(|&ep| ep > 0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .filter(|&n| n <= 999)
}

/// ストック章（`00`）にある話の一覧（§10.3.3）。
pub fn list(project: &Project) -> Vec<StockItem> {
    let episodes = crate::core::features::plots::episodes(project, 0);
    let outline = crate::core::features::plots::outline_blocks(project, Some(0));
    let prose = crate::core::features::sections::all(project);
    let mut out = Vec::new();
    for ep in episodes {
        if ep == 0 {
            continue;
        }
        let block = outline
            .iter()
            .find(|b| b.id.chapter == 0 && b.id.episode == ep);
        let title = block.map(|b| b.title.clone()).unwrap_or_default();
        let mut summary = block.map(|b| b.summary.clone()).unwrap_or_default();
        if summary.is_empty() {
            summary = crate::core::features::plots::episode_rows(project, 0, ep)
                .into_iter()
                .find(|r| !r.summary.is_empty())
                .map(|r| r.summary)
                .unwrap_or_default();
        }
        let mut chars = 0usize;
        for sec in prose.iter() {
            if sec.id.chapter == 0 && sec.id.episode == ep {
                let text = crate::core::features::sections::text_of(project.text(&sec.path), sec);
                chars += crate::core::features::sections::count_chars(&text);
            }
        }
        let updated_at = block
            .and_then(|_| project.document(&project.chapter_plot_path(0)))
            .or_else(|| project.document(&project.contents_path(0, ep)))
            .or_else(|| project.document(&project.episode_plot_path(0, ep)))
            .and_then(|doc| {
                doc.text.lines().find_map(|l| {
                    let trimmed = l.trim();
                    trimmed
                        .strip_prefix("// updated_at:")
                        .or_else(|| trimmed.strip_prefix("updated_at:"))
                        .or_else(|| trimmed.strip_prefix("updated:"))
                        .map(|v| v.trim().to_owned())
                })
            })
            .unwrap_or_default();
        out.push(StockItem {
            episode: ep,
            title,
            summary,
            chars,
            updated_at,
        });
    }
    out
}

/// ストックの話 `00-episode` を章 `chapter` の末尾話番号へ配属する（既存互換ラッパー）。
pub fn assign(project: &Project, episode: u16, chapter: u16) -> Result<Assignment, &'static str> {
    if episode == 0 || chapter == 0 {
        return Err("a nonzero episode and chapter are required");
    }
    let old = project.contents_path(0, episode);
    if project.document(&old).is_none() {
        return Err("stock episode does not exist");
    }
    let next = next_episode_in_chapter(project, chapter).ok_or("episode limit")?;
    move_episode(project, 0, episode, chapter, next).map_err(|e| e.as_static_str())
}

/// 任意の話 `(from_chapter, from_episode)` を `(to_chapter, to_episode)` へ原子的に移動する（§10.3.3）。
/// - `from_chapter == 0 && to_chapter >= 1`: ストックから正式な章・話への配属。
/// - `from_chapter >= 1 && to_chapter == 0`: 正式な話をストック章へ戻す逆操作（`Ctrl+Shift+Enter`）。
pub fn move_episode(
    project: &Project,
    from_chapter: u16,
    from_episode: u16,
    to_chapter: u16,
    to_episode: u16,
) -> Result<Assignment, StockError> {
    if from_episode == 0 || to_episode == 0 {
        return Err(StockError::ZeroTarget);
    }
    if from_episode > 999 || to_episode > 999 || from_chapter > 999 || to_chapter > 999 {
        return Err(StockError::EpisodeLimit);
    }
    if from_chapter == to_chapter && from_episode == to_episode {
        return Err(StockError::SameLocation);
    }
    let from_contents = project.contents_path(from_chapter, from_episode);
    let from_plot = project.episode_plot_path(from_chapter, from_episode);
    if project.document(&from_contents).is_none() && project.document(&from_plot).is_none() {
        return Err(StockError::SourceMissing(from_contents));
    }
    let to_contents = project.contents_path(to_chapter, to_episode);
    let to_plot = project.episode_plot_path(to_chapter, to_episode);
    if project.document(&to_contents).is_some() {
        return Err(StockError::DestinationExists(to_contents));
    }
    if project.document(&to_plot).is_some() {
        return Err(StockError::DestinationExists(to_plot));
    }
    let existing_dest_block =
        crate::core::features::plots::outline_blocks(project, Some(to_chapter))
            .into_iter()
            .any(|b| b.id.chapter == to_chapter && b.id.episode == to_episode);
    if existing_dest_block {
        let shown = crate::core::features::plots::short_id(
            SceneId {
                chapter: to_chapter,
                episode: to_episode,
                scene: 0,
                section: 0,
            },
            project.digits(),
        );
        return Err(StockError::DestinationExists(format!("@{shown}")));
    }

    let mut renames = Vec::new();
    for (from, to) in [(from_contents, to_contents), (from_plot, to_plot)] {
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
            if full.chapter != from_chapter || full.episode != from_episode {
                continue;
            }
            full.chapter = to_chapter;
            full.episode = to_episode;
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
        target_chapter: to_chapter,
        target_episode: to_episode,
    })
}

/// 正式な話 `(chapter, episode)` をストック章（`00`）の次の空き番号へ戻す（`Ctrl+Shift+Enter`、§10.3.3）。
pub fn return_to_stock(
    project: &Project,
    chapter: u16,
    episode: u16,
) -> Result<Assignment, StockError> {
    if chapter == 0 || episode == 0 {
        return Err(StockError::ZeroTarget);
    }
    let next = next_episode_in_chapter(project, 0).ok_or(StockError::EpisodeLimit)?;
    move_episode(project, chapter, episode, 0, next)
}
