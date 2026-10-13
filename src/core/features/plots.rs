//! プロットのワークスペース（§10.2 `plot-episode`・§10.3 `plot-outline`）の行の並び。
//!
//! LSP には対応しない補助（`PLANS.md` §9.2 B-127）。**文言は持たない**（§12.2）。
//!
//! - **話プロットの節リスト**（[`episode_rows`]）: `plots/AA-BB.txt`（B 形式）を、場面見出しの行と
//!   節の行の並びにする。各行は「エディタで編集する行の範囲」を持つ。場面の行は見出しから最初の
//!   節の前まで（場面レベルのフィールド、§5.4.1 `scene_field`）、節の行は節の内容（`;` と末尾の空行を
//!   含まない）。状態は §17.2 のエクスポートと同じ実効の状態（B-122）。
//! - **上位のブロック**（[`outline_block`]）: 章プロット（`plots/AA.txt`）の話のブロックや全体プロット
//!   （`plots/all.txt`）の章のブロックの行（`plot-episode` の上段・`plot-outline` の中央）。
//! - **伏線**（[`foreshadows`]）: 全体・章・話プロットの `seed:` / `payoff:` を伏線 ID で突き合わせる
//!   （§10.3 の伏線タブ。未回収を上に）。

use std::collections::BTreeMap;
use std::ops::Range;

use crate::core::export::{self, State};
use crate::core::features::project::{chapter_of_path, episode_of_path};
use crate::core::features::Project;
use crate::core::id::{Digits, SceneId};
use crate::core::model::document::DocumentFormat;
use crate::core::syntax::{plot_episode, plot_outline};
use crate::core::text::lexer::{self, FieldKey};

// ============================================================ 話プロット（B 形式）

/// 話プロットの節リストの 1 行（§10.2 の中段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeRow {
    /// 構造 ID。場面見出しの行なら `section == 0`。
    pub id: SceneId,
    /// エディタで編集する行（ファイルの行 index）。空の節なら、その位置の空範囲。
    pub lines: Range<usize>,
    /// 1 行の要約（場面: 見出しのタイトル、節: 最初の内容行。無ければ空）。
    pub summary: String,
    /// 実効の状態（節の行だけ。B-122）。
    pub state: Option<State>,
}

impl EpisodeRow {
    /// 場面見出しの行か。
    pub fn is_scene(&self) -> bool {
        self.id.section == 0
    }
}

/// 話 `chapter`-`episode` の話プロットの節リスト（場面見出しの ID が壊れた場面は入れない）。
/// 話プロットが無ければ空。節が 1 つも無い場面（見出しだけ）には、見出しの直後に空の節の行を置く。
pub fn episode_rows(project: &Project, chapter: u16, episode: u16) -> Vec<EpisodeRow> {
    let Some(plot) = project.episode_plot(chapter, episode) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut ids = Vec::new();
    for scene in &plot.scenes {
        let Some(id) = scene.id else {
            continue;
        };
        let scene_id = SceneId { section: 0, ..id };
        let head_end = scene
            .sections
            .first()
            .map_or(scene.span.end, |section| section.span.start);
        let title = match plot.lines.get(scene.header) {
            Some(plot_episode::Line::Scene(header)) => header.title.trim().to_owned(),
            _ => String::new(),
        };
        let head = trim_blank_tail(&plot.lines, scene.header..head_end);
        out.push(EpisodeRow {
            id: scene_id,
            lines: head.clone(),
            summary: title,
            state: None,
        });
        if scene.sections.is_empty() {
            // 見出し（と場面レベルのフィールド）だけの場面: 節は 0 個（§5.4.1・B-35）。書き始める
            // 位置として、見出しの直後に空の節の行を 1 つ置く（書けば最初の節になる）
            let number = match plot.lines.get(scene.header) {
                Some(plot_episode::Line::Scene(header)) => header.section_start().unwrap_or(1),
                _ => 1,
            };
            let section_id = SceneId {
                section: number,
                ..scene_id
            };
            out.push(EpisodeRow {
                id: section_id,
                lines: head.end..head.end,
                summary: String::new(),
                state: None,
            });
            ids.push(section_id);
        }
        for section in &scene.sections {
            let section_id = SceneId {
                section: section.number,
                ..scene_id
            };
            let summary = section
                .points
                .iter()
                .filter_map(|line| plot.lines.get(*line))
                .map(|line| line.raw().trim())
                .find(|line| !line.is_empty())
                .unwrap_or_default()
                .to_owned();
            out.push(EpisodeRow {
                id: section_id,
                lines: section.content.clone(),
                summary,
                state: None,
            });
            ids.push(section_id);
        }
    }
    let mut states = export::effective_states(project, &ids).into_iter();
    for row in out.iter_mut().filter(|row| !row.is_scene()) {
        row.state = states.next();
    }
    out
}

/// 末尾の空行を除いた行範囲。
fn trim_blank_tail(lines: &[plot_episode::Line<'_>], range: Range<usize>) -> Range<usize> {
    let mut end = range.end.min(lines.len());
    while end > range.start && lines.get(end - 1).is_some_and(|line| line.is_blank()) {
        end -= 1;
    }
    range.start..end
}

/// 節リストの中で `id` に最も近い行（同じ ID → 同じ場面の見出し → 先頭）。
pub fn row_near(rows: &[EpisodeRow], id: SceneId) -> Option<usize> {
    rows.iter()
        .position(|row| row.id == id)
        .or_else(|| {
            rows.iter()
                .position(|row| row.is_scene() && row.id == SceneId { section: 0, ..id })
        })
        .or(if rows.is_empty() { None } else { Some(0) })
}

/// 話プロットに新しい（空の）節を足したファイル全体と、新しい節の位置の行（§10.2 `Ctrl+Shift+N`）。
///
/// 新しい節は**空**で、返す行は新しい節を閉じる `;` の行（エディタはその位置の空範囲を開き、書くと
/// `;` の前に行が入る）。
///
/// - `after` が節の行なら、その節の後ろに入れる。その節を閉じる `;` があればその次の行に `;` を
///   1 行足し、無ければ（場面の最後の節）`;` を 2 行足す（1 行目で閉じ、2 行目で新しい節を閉じる）。
/// - `after` が場面の行なら、場面の最初の節の前に入れる（見出しと場面レベルのフィールドの直後に
///   `;` を 1 行。最初の節が空の節になり、元の最初の節は 2 番目になる）。
/// - 節リストが空（ファイルが無い・場面が無い）なら、末尾に `scene` の場面見出し（`@AA-BB-CC`）を書く。
///   見出しの次の行が最初の節の位置。
pub fn insert_section(
    text: &str,
    rows: &[EpisodeRow],
    after: Option<usize>,
    scene: SceneId,
    digits: Digits,
) -> (String, usize) {
    let is_break = |line: &str| {
        matches!(
            lexer::classify(line, lexer::TextFormat::PlotEpisode),
            lexer::LineKind::SectionBreak
        )
    };
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let at = match after.and_then(|index| rows.get(index)) {
        Some(row) if !row.is_scene() => {
            let closing = (row.lines.end..lines.len())
                .find(|index| !lines[*index].trim().is_empty())
                .filter(|index| is_break(&lines[*index]));
            match closing {
                Some(index) => {
                    lines.insert(index + 1, ";".to_owned());
                    index + 1
                }
                None => {
                    let end = row.lines.end.min(lines.len());
                    lines.insert(end, ";".to_owned());
                    lines.insert(end + 1, ";".to_owned());
                    end + 1
                }
            }
        }
        Some(row) => {
            let end = row.lines.end.min(lines.len());
            lines.insert(end, ";".to_owned());
            end
        }
        None => {
            // 改行で終わる（または空の）ファイルの最後の空の「行」の位置に見出しを書く
            let header = format!(
                "@{}",
                SceneId {
                    section: 0,
                    ..scene
                }
                .display(digits)
            );
            let last_is_empty = lines.last().is_some_and(|line| line.is_empty());
            if last_is_empty {
                lines.pop();
            }
            if lines.last().is_some_and(|line| !line.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(header);
            let at = lines.len();
            lines.push(String::new());
            at
        }
    };
    (lines.join("\n"), at)
}

// ============================================================ 全体 / 章プロット（A 形式）

/// 全体 / 章プロットの 1 ブロック（§10.3 の中央・§10.2 の上段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineBlock {
    /// ブロックの ID（章なら `episode == 0`）。
    pub id: SceneId,
    /// 文書のパス（`plots/all.txt` / `plots/01.txt`）。
    pub path: String,
    /// 見出しからの行（`;` と末尾の空行を含まない）。エディタで編集する範囲。
    pub lines: Range<usize>,
    /// ブロックの終わり（`;` があればその次の行、無ければ次の見出しの行）。
    pub end: usize,
    /// 見出しのタイトル。
    pub title: String,
    /// `summary:` の 1 行目。
    pub summary: String,
    /// `state:`（書かれていなければ `None`）。
    pub state: Option<State>,
}

/// 全体プロット（`chapter == None`）または章プロット（`Some(章)`）のブロック（見出しの ID が
/// 壊れたブロックは入れない）。文書が無ければ空。
pub fn outline_blocks(project: &Project, chapter: Option<u16>) -> Vec<OutlineBlock> {
    let (path, outline) = match chapter {
        Some(number) => (
            project.chapter_plot_path(number),
            project.chapter_plot(number),
        ),
        None => ("plots/all.txt".to_owned(), project.all_plot()),
    };
    let Some(outline) = outline else {
        return Vec::new();
    };
    outline
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            let id = block.id?;
            let mut end = block.end.unwrap_or(block.span.end).min(outline.lines.len());
            while end > block.header + 1 && outline.lines[end - 1].is_blank() {
                end -= 1;
            }
            let first = |key: FieldKey| {
                outline
                    .fields_of(index, key)
                    .first()
                    .map(|field| field.value.trim().to_owned())
            };
            Some(OutlineBlock {
                id,
                path: path.clone(),
                lines: block.header..end,
                end: block.span.end,
                title: outline
                    .header(block.header)
                    .map(|header| header.title.trim().to_owned())
                    .unwrap_or_default(),
                summary: first(FieldKey::Summary).unwrap_or_default(),
                state: first(FieldKey::State).and_then(|value| State::parse(&value)),
            })
        })
        .collect()
}

/// 全体 / 章プロットに新しいブロックを足したファイル全体と、新しい見出しの行（`plot-outline` の
/// `Ctrl+Shift+N`、B-129）。
///
/// 見出しは `id`（全体なら `@03`、章なら `@01-05`）だけで、すぐ `;` で閉じる。`after` のブロックの
/// 後ろ（無ければファイルの末尾）に、空行を 1 行挟んで入れる。
pub fn insert_block(
    text: &str,
    blocks: &[OutlineBlock],
    after: Option<usize>,
    id: SceneId,
    digits: Digits,
) -> (String, usize) {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let header = format!("@{}", short_id(id, digits));
    let at = match after.and_then(|index| blocks.get(index)) {
        Some(block) => block.end.min(lines.len()),
        None => {
            // 改行で終わる（または空の）ファイルの最後の空の「行」の位置
            if lines.last().is_some_and(|line| line.is_empty()) {
                lines.len() - 1
            } else {
                lines.len()
            }
        }
    };
    let needs_gap = at > 0
        && lines
            .get(at - 1)
            .is_some_and(|line| !line.trim().is_empty());
    let mut inserted = Vec::new();
    if needs_gap {
        inserted.push(String::new());
    }
    let header_at = at + inserted.len();
    inserted.push(header);
    inserted.push(";".to_owned());
    let followed = lines.get(at).is_some_and(|line| !line.trim().is_empty());
    if followed {
        inserted.push(String::new());
    }
    lines.splice(at..at, inserted);
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    (out, header_at)
}

/// 次に足すブロックの ID（全体なら最大の章 + 1、章なら章の中の最大の話 + 1）。
pub fn next_block_id(blocks: &[OutlineBlock], chapter: Option<u16>) -> SceneId {
    match chapter {
        None => SceneId {
            chapter: blocks
                .iter()
                .map(|block| block.id.chapter)
                .max()
                .unwrap_or(0)
                + 1,
            episode: 0,
            scene: 0,
            section: 0,
        },
        Some(number) => SceneId {
            chapter: number,
            episode: blocks
                .iter()
                .filter(|block| block.id.chapter == number)
                .map(|block| block.id.episode)
                .max()
                .unwrap_or(0)
                + 1,
            scene: 0,
            section: 0,
        },
    }
}

/// 階層の深さだけの ID（`01`・`01-04`・`01-04-02`・`01-04-02-03`）。0 の階層から下は書かない。
pub fn short_id(id: SceneId, digits: Digits) -> String {
    let depth = if id.episode == 0 {
        1
    } else if id.scene == 0 {
        2
    } else if id.section == 0 {
        3
    } else {
        4
    };
    let full = SceneId {
        section: if depth == 4 { id.section } else { 0 },
        ..id
    }
    .display(digits);
    full.split('-').take(depth).collect::<Vec<_>>().join("-")
}

/// 章プロットの中の話 `chapter`-`episode` のブロック（§10.2 の上段）。
pub fn outline_block(project: &Project, chapter: u16, episode: u16) -> Option<OutlineBlock> {
    outline_blocks(project, Some(chapter))
        .into_iter()
        .find(|block| block.id.chapter == chapter && block.id.episode == episode)
}

/// プロジェクトにある章（全体プロットのブロック・章プロットのファイル・本文と話プロットのファイルの
/// どれかに現れる章。昇順）。`plot-outline` の左のレベル切替に使う。
pub fn chapters(project: &Project) -> Vec<u16> {
    let mut out: Vec<u16> = outline_blocks(project, None)
        .iter()
        .map(|block| block.id.chapter)
        .collect();
    for document in project.documents() {
        let found = match document.format {
            DocumentFormat::PlotOutline => chapter_of_path(&document.path),
            DocumentFormat::PlotEpisode | DocumentFormat::Contents => {
                episode_of_path(&document.path).map(|(chapter, _)| chapter)
            }
            _ => None,
        };
        if let Some(chapter) = found {
            out.push(chapter);
        }
    }
    out.retain(|chapter| *chapter > 0);
    out.sort_unstable();
    out.dedup();
    out
}

/// 章 `chapter` にある話（章プロットのブロック・本文と話プロットのファイル。昇順）。
pub fn episodes(project: &Project, chapter: u16) -> Vec<u16> {
    let mut out: Vec<u16> = outline_blocks(project, Some(chapter))
        .iter()
        .filter(|block| block.id.chapter == chapter)
        .map(|block| block.id.episode)
        .collect();
    for document in project.documents() {
        if matches!(
            document.format,
            DocumentFormat::PlotEpisode | DocumentFormat::Contents
        ) {
            if let Some((found, episode)) = episode_of_path(&document.path) {
                if found == chapter {
                    out.push(episode);
                }
            }
        }
    }
    out.retain(|episode| *episode > 0);
    out.sort_unstable();
    out.dedup();
    out
}

/// プロジェクトにある話（章 → 話の昇順。章プロットのブロック・本文と話プロットのファイル）。
/// `plot-episode` の「前 / 次の話」（`Ctrl+Alt+←` / `→`）に使う。
pub fn all_episodes(project: &Project) -> Vec<(u16, u16)> {
    chapters(project)
        .into_iter()
        .flat_map(|chapter| {
            episodes(project, chapter)
                .into_iter()
                .map(move |episode| (chapter, episode))
        })
        .collect()
}

// ============================================================ 伏線（§10.3 の伏線タブ）

/// 伏線 1 つ（伏線 ID ごと）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Foreshadow {
    /// 伏線 ID（`F01`）。
    pub tag: String,
    /// 設置（`seed:`）の場所と説明。
    pub seeds: Vec<ForeshadowSite>,
    /// 回収（`payoff:`）の場所。
    pub payoffs: Vec<ForeshadowSite>,
}

impl Foreshadow {
    /// 未回収か（設置があって回収が無い）。
    pub fn open(&self) -> bool {
        !self.seeds.is_empty() && self.payoffs.is_empty()
    }

    /// 二重回収か（回収が 2 つ以上）。
    pub fn doubled(&self) -> bool {
        self.payoffs.len() > 1
    }
}

/// `seed:` / `payoff:` の 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeshadowSite {
    /// 書かれた文書。
    pub path: String,
    /// 行 index。
    pub line: usize,
    /// 書かれたブロック・場面の ID（前文なら `None`）。
    pub block: Option<SceneId>,
    /// 説明（`seed` の値の `F01` と `->` の間）。
    pub note: String,
    /// `->` の後ろ（`@03-02`）。
    pub target: String,
}

/// 全体・章・話プロットの伏線（未回収 → 二重回収 → その他の順、同じ順位の中は伏線 ID の順）。
pub fn foreshadows(project: &Project) -> Vec<Foreshadow> {
    let mut by_tag: BTreeMap<String, Foreshadow> = BTreeMap::new();
    let mut push = |key: FieldKey, site: ForeshadowSite, tag: &str| {
        let entry = by_tag.entry(tag.to_owned()).or_insert_with(|| Foreshadow {
            tag: tag.to_owned(),
            seeds: Vec::new(),
            payoffs: Vec::new(),
        });
        if key == FieldKey::Seed {
            entry.seeds.push(site);
        } else {
            entry.payoffs.push(site);
        }
    };
    for document in project.documents() {
        match document.format {
            DocumentFormat::PlotOutline => {
                let outline = plot_outline::parse_with(
                    &document.text,
                    &project.outline_options(&document.path),
                );
                for (index, line) in outline.lines.iter().enumerate() {
                    let plot_outline::Line::Field(field) = line else {
                        continue;
                    };
                    if !matches!(field.key, FieldKey::Seed | FieldKey::Payoff) {
                        continue;
                    }
                    let Some(found) = field.foreshadow() else {
                        continue;
                    };
                    let block = outline
                        .blocks
                        .iter()
                        .find(|block| block.span.contains(&index))
                        .and_then(|block| block.id);
                    push(
                        field.key,
                        ForeshadowSite {
                            path: document.path.clone(),
                            line: index,
                            block,
                            note: found.note.trim().to_owned(),
                            target: found.target.trim().to_owned(),
                        },
                        found.tag,
                    );
                }
            }
            DocumentFormat::PlotEpisode => {
                let plot = plot_episode::parse_with(&document.text, &project.contents_options());
                for (index, line) in plot.lines.iter().enumerate() {
                    let plot_episode::Line::Field(field) = line else {
                        continue;
                    };
                    if !matches!(field.key, FieldKey::Seed | FieldKey::Payoff) {
                        continue;
                    }
                    let Some(found) = field.foreshadow() else {
                        continue;
                    };
                    let block = plot.scene_of(index).and_then(|scene| {
                        let id = scene.id?;
                        let section = plot.section_of(index).map_or(0, |section| section.number);
                        Some(SceneId { section, ..id })
                    });
                    push(
                        field.key,
                        ForeshadowSite {
                            path: document.path.clone(),
                            line: index,
                            block,
                            note: found.note.trim().to_owned(),
                            target: found.target.trim().to_owned(),
                        },
                        found.tag,
                    );
                }
            }
            _ => {}
        }
    }
    let mut out: Vec<Foreshadow> = by_tag.into_values().collect();
    out.sort_by_key(|found| (!found.open(), !found.doubled(), found.tag.clone()));
    out
}
