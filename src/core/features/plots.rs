//! プロットのワークスペース（§10.2 `plot-episode`・§10.3 `plot-outline`）の行の並び。
//!
//! LSP には対応しない補助（`docs/history.md` B-127）。**文言は持たない**（§12.2）。
//!
//! - **話プロットの節リスト**（[`episode_rows`]）: `plots/AA-BB.cnov`（B 形式）を、場面見出しの行と
//!   節の行の並びにする。各行は「エディタで編集する行の範囲」を持つ。場面の行は見出しから最初の
//!   節の前まで（場面レベルのフィールド、§5.4.1 `scene_field`）、節の行は節の内容（`;` と末尾の空行を
//!   含まない）。状態は §17.2 のエクスポートと同じ実効の状態（B-122）。
//! - **上位のブロック**（[`outline_block`]）: 章プロット（`plots/AA.cnov`）の話のブロックや全体プロット
//!   （`plots/all.cnov`）の章のブロックの行（`plot-episode` の上段・`plot-outline` の中央）。
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
    /// この行のブロックに書かれた `when:`（作中日時、§5.3.5・§10.2）。無ければ `None`。
    pub when: Option<String>,
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
        let scene_when = (scene.header..head_end)
            .filter_map(|idx| plot.lines.get(idx))
            .find_map(|line| match line {
                plot_episode::Line::Field(f) if f.key == FieldKey::When => {
                    let v = f.value.trim();
                    (!v.is_empty()).then(|| v.to_owned())
                }
                _ => None,
            });
        out.push(EpisodeRow {
            id: scene_id,
            lines: head.clone(),
            summary: title,
            state: None,
            when: scene_when,
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
                when: None,
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
            let sec_when = section
                .span
                .clone()
                .filter_map(|idx| plot.lines.get(idx))
                .find_map(|line| match line {
                    plot_episode::Line::Field(f) if f.key == FieldKey::When => {
                        let v = f.value.trim();
                        (!v.is_empty()).then(|| v.to_owned())
                    }
                    _ => None,
                });
            out.push(EpisodeRow {
                id: section_id,
                lines: section.content.clone(),
                summary,
                state: None,
                when: sec_when,
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

/// 節 `id` に対応する話プロットのブロック 1 つ（§8.1.2 の一体削除の対応判定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionBlock {
    /// ブロックの行（末尾の空行を含む。区切り `;` を含まない）。
    pub span: Range<usize>,
    /// 内容の行（末尾の空行を含まない）。
    pub content: Range<usize>,
}

/// 節 `id` に対応する話プロットのブロックすべて（場面見出しの ID が壊れた場面は数えない）。
/// 一体削除（§8.1.2）は、結果が 1 件のときだけ一意とみなす。
pub fn section_blocks(project: &Project, id: SceneId) -> Vec<SectionBlock> {
    let Some(plot) = project.episode_plot(id.chapter, id.episode) else {
        return Vec::new();
    };
    let scene_of = SceneId { section: 0, ..id };
    let mut out = Vec::new();
    for scene in &plot.scenes {
        let Some(scene_id) = scene.id else {
            continue;
        };
        let of_scene = SceneId {
            section: 0,
            ..scene_id
        };
        if of_scene != scene_of {
            continue;
        }
        for section in &scene.sections {
            if section.number == id.section {
                out.push(SectionBlock {
                    span: section.span.clone(),
                    content: section.content.clone(),
                });
            }
        }
    }
    out
}

/// ブロックを消したファイル全体（ブロックの行と区切り `;` を消す。本文側の
/// `sections::remove` と同じ規則、§8.1.2）。
pub fn remove_block(text: &str, block: &SectionBlock) -> String {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let end = block.span.end.min(lines.len());
    let start = block.span.start.min(end);
    let is_break = |line: Option<&&str>| {
        line.is_some_and(|line| {
            matches!(
                lexer::classify(line, lexer::TextFormat::PlotEpisode),
                lexer::LineKind::SectionBreak
            )
        })
    };
    let (from, to) = if is_break(lines.get(end)) {
        (start, end + 1)
    } else if start > 0 && is_break(lines.get(start - 1)) {
        (start - 1, end)
    } else {
        (start, end)
    };
    // ファイル末尾の改行の後ろの空の「行」は残す
    let to = if to == lines.len() && text.ends_with('\n') && to > from {
        to - 1
    } else {
        to
    };
    lines.drain(from..to.max(from));
    lines.join("\n")
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

/// 話プロットの行 `id`（節の行。無ければ同じ場面の見出しの行）の最後に `line` を 1 行足した
/// ファイル全体（§16.4「Enter でプロットの `material:` に追記」・§10.2 の `Ctrl+M`）。
///
/// 節の行なら節の内容の最後（`;` と末尾の空行の前）、場面の行なら場面レベルのフィールドの最後に
/// 入る。見出しだけの場面（空の節の行）は見出しの直後なので場面レベルのフィールドになる。
/// 行が無ければ（話プロットが無い・場面が無い）`None`（UI が通知する、`docs/history.md` B-134）。
pub fn append_line(text: &str, rows: &[EpisodeRow], id: SceneId, line: &str) -> Option<String> {
    let row = rows.iter().find(|row| row.id == id).or_else(|| {
        rows.iter()
            .find(|row| row.is_scene() && row.id == SceneId { section: 0, ..id })
    })?;
    let mut lines: Vec<&str> = text.split('\n').collect();
    let at = row.lines.end.min(lines.len());
    lines.insert(at, line);
    Some(lines.join("\n"))
}

// ============================================================ 全体 / 章プロット（A 形式）

/// 全体 / 章プロットの 1 ブロック（§10.3 の中央・§10.2 の上段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineBlock {
    /// ブロックの ID（章なら `episode == 0`）。
    pub id: SceneId,
    /// 文書のパス（`plots/all.cnov` / `plots/01.cnov`）。
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
        None => ("plots/all.cnov".to_owned(), project.all_plot()),
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
    out.retain(|chapter| *chapter > 0 || project.settings().structure.allow_chapter_zero);
    out.sort_by_key(|chapter| (*chapter == 0, *chapter));
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

// ============================================================ 進捗（§10.9.1 の進捗タブ）

/// 進捗ツリーの 1 行（章 → 話 → 場面 → 節。§10.9.1、B-132）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressRow {
    /// 構造 ID（章の行は `episode == 0`、話の行は `scene == 0`、場面の行は `section == 0`）。
    pub id: SceneId,
    /// 階層（0 = 章 … 3 = 節）。
    pub depth: usize,
    /// タイトル（章・話は全体 / 章プロットのブロック、場面は見出し。節は最初の要点か空）。
    pub title: String,
    /// 本文の文字数（空白除外。上の階層は下の合計）。
    pub chars: usize,
    /// 目標字数（節は話プロットの `length:`、上の階層は下の目標の合計。どこにも無ければ `None`）。
    pub target: Option<u32>,
    /// 状態（節は実効の状態 B-122、上の階層は下の**最も遅い**状態）。
    pub state: State,
    /// 本文があるか（上の階層は下のどれかにあれば）。
    pub prose: bool,
}

/// 進捗ツリー（本文の節と話プロットの節の和。本文の無い計画だけの節も 0 字で出す）。
pub fn progress(project: &Project) -> Vec<ProgressRow> {
    use std::collections::{BTreeMap, BTreeSet};
    let prose = crate::core::features::sections::all(project);
    let mut chars: BTreeMap<SceneId, usize> = BTreeMap::new();
    let mut scene_titles: BTreeMap<SceneId, String> = BTreeMap::new();
    for section in prose.iter() {
        if section.id.chapter == 0 || section.id.episode == 0 {
            continue;
        }
        let text = crate::core::features::sections::text_of(project.text(&section.path), section);
        *chars.entry(section.id).or_default() +=
            crate::core::features::sections::count_chars(&text);
        scene_titles
            .entry(SceneId {
                section: 0,
                ..section.id
            })
            .or_insert_with(|| section.scene_title.clone());
    }
    let mut ids: BTreeSet<SceneId> = chars.keys().copied().collect();
    let mut point_titles: BTreeMap<SceneId, String> = BTreeMap::new();
    for (chapter, episode) in all_episodes(project) {
        if chapter == 0 || episode == 0 {
            continue;
        }
        for row in episode_rows(project, chapter, episode) {
            if row.is_scene() {
                if !row.summary.is_empty() {
                    scene_titles.entry(row.id).or_insert(row.summary);
                }
            } else {
                ids.insert(row.id);
                point_titles.insert(row.id, row.summary);
            }
        }
    }
    let list: Vec<SceneId> = ids.iter().copied().collect();
    let states = export::effective_states(project, &list);
    let mut chapter_titles: BTreeMap<u16, String> = BTreeMap::new();
    for block in outline_blocks(project, None) {
        chapter_titles.insert(block.id.chapter, block.title);
    }
    let mut episode_titles: BTreeMap<(u16, u16), String> = BTreeMap::new();
    for chapter in list.iter().map(|id| id.chapter).collect::<BTreeSet<_>>() {
        for block in outline_blocks(project, Some(chapter)) {
            episode_titles.insert((block.id.chapter, block.id.episode), block.title);
        }
    }
    // 節の行（ID の順）から上の階層を積み上げる
    let mut out: Vec<ProgressRow> = Vec::new();
    let mut open: [Option<usize>; 3] = [None, None, None];
    for (id, state) in list.iter().zip(states) {
        let keys = [
            SceneId {
                episode: 0,
                scene: 0,
                section: 0,
                ..*id
            },
            SceneId {
                scene: 0,
                section: 0,
                ..*id
            },
            SceneId { section: 0, ..*id },
        ];
        for depth in 0..3 {
            let current = open[depth].map(|at| out[at].id);
            if current != Some(keys[depth]) {
                let title = match depth {
                    0 => chapter_titles.get(&id.chapter).cloned(),
                    1 => episode_titles.get(&(id.chapter, id.episode)).cloned(),
                    _ => scene_titles.get(&keys[2]).cloned(),
                }
                .unwrap_or_default();
                out.push(ProgressRow {
                    id: keys[depth],
                    depth,
                    title,
                    chars: 0,
                    target: None,
                    state: State::Fixed,
                    prose: false,
                });
                open[depth] = Some(out.len() - 1);
                for deeper in open.iter_mut().skip(depth + 1) {
                    *deeper = None;
                }
            }
        }
        let count = chars.get(id).copied();
        let target = crate::core::features::reference::target_length(project, *id);
        out.push(ProgressRow {
            id: *id,
            depth: 3,
            title: point_titles.get(id).cloned().unwrap_or_default(),
            chars: count.unwrap_or(0),
            target,
            state,
            prose: count.is_some(),
        });
        for at in open.iter().flatten() {
            let parent = &mut out[*at];
            parent.chars += count.unwrap_or(0);
            if let Some(value) = target {
                parent.target = Some(parent.target.unwrap_or(0) + value);
            }
            parent.state = parent.state.min(state);
            parent.prose |= count.is_some();
        }
    }
    out
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

/// 同一場面内の 2 つの節 `id_a` と `id_b` について、本文側に非空の節が存在するか（§10.2 `Alt+Shift+↑/↓` 確認モーダル判定）。
pub fn has_prose_for_swap(project: &Project, id_a: SceneId, id_b: SceneId) -> bool {
    let path = project.contents_path(id_a.chapter, id_a.episode);
    let Some(doc) = project.document(&path) else {
        return false;
    };
    project
        .sections()
        .iter()
        .filter(|s| s.path == path && (s.id == id_a || s.id == id_b))
        .any(|s| {
            !crate::core::features::sections::text_of(&doc.text, s)
                .trim()
                .is_empty()
        })
}

fn line_span_bytes(text: &str, lines: &Range<usize>) -> Range<usize> {
    let starts = crate::core::features::project::line_starts_of(text);
    let from = starts.get(lines.start).copied().unwrap_or(text.len());
    let to = if lines.end == 0 {
        from
    } else if lines.end < starts.len() {
        starts[lines.end].saturating_sub(1)
    } else {
        text.len()
    };
    from..to.max(from)
}

/// 話プロット内の隣り合う 2 つの節（同じ場面内）を入れ替える `WorkspaceEdit` を作る（§10.2 `Alt+Shift+↑/↓`）。
/// `include_prose == true` なら、対応する本文ファイルの 2 節の中身も連動して入れ替える。
pub fn swap_episode_sections(
    project: &Project,
    id_a: SceneId,
    id_b: SceneId,
    include_prose: bool,
) -> Option<crate::core::features::WorkspaceEdit> {
    use crate::core::features::{FileEdit, TextEdit, WorkspaceEdit};
    if id_a.chapter != id_b.chapter
        || id_a.episode != id_b.episode
        || id_a.scene != id_b.scene
        || id_a.section == 0
        || id_b.section == 0
        || id_a == id_b
    {
        return None;
    }
    let (first_id, second_id) = if id_a < id_b {
        (id_a, id_b)
    } else {
        (id_b, id_a)
    };
    let plot_path = project.episode_plot_path(first_id.chapter, first_id.episode);
    let plot_doc = project.document(&plot_path)?;
    let blocks_first = section_blocks(project, first_id);
    let blocks_second = section_blocks(project, second_id);
    if blocks_first.len() != 1 || blocks_second.len() != 1 {
        return None;
    }
    let r1 = line_span_bytes(&plot_doc.text, &blocks_first[0].span);
    let r2 = line_span_bytes(&plot_doc.text, &blocks_second[0].span);
    if r1.end > r2.start {
        return None;
    }
    let t1 = plot_doc.text[r1.clone()].to_owned();
    let t2 = plot_doc.text[r2.clone()].to_owned();
    let mut changes = vec![FileEdit {
        path: plot_path,
        edits: vec![
            TextEdit {
                range: r1,
                new_text: t2,
            },
            TextEdit {
                range: r2,
                new_text: t1,
            },
        ],
    }];

    if include_prose {
        let prose_path = project.contents_path(first_id.chapter, first_id.episode);
        if let Some(prose_doc) = project.document(&prose_path) {
            let secs: Vec<_> = project
                .sections()
                .iter()
                .filter(|s| s.path == prose_path && (s.id == first_id || s.id == second_id))
                .cloned()
                .collect();
            if secs.len() == 2 && secs[0].id == first_id && secs[1].id == second_id {
                let pr1 = line_span_bytes(&prose_doc.text, &secs[0].lines);
                let pr2 = line_span_bytes(&prose_doc.text, &secs[1].lines);
                if pr1.end <= pr2.start {
                    let pt1 = prose_doc.text[pr1.clone()].to_owned();
                    let pt2 = prose_doc.text[pr2.clone()].to_owned();
                    changes.push(FileEdit {
                        path: prose_path,
                        edits: vec![
                            TextEdit {
                                range: pr1,
                                new_text: pt2,
                            },
                            TextEdit {
                                range: pr2,
                                new_text: pt1,
                            },
                        ],
                    });
                }
            }
        }
    }

    Some(WorkspaceEdit { changes })
}

/// 現在の節/場面 `id` に対して有効な `space:`（節 → 場面 → 話 → 章の順で継承、`assets/...` も展開）を返す（§9.5.3・§9.8.3）。
pub fn space_lines(project: &Project, id: SceneId) -> Vec<String> {
    let records = crate::core::features::metadata::records(project);
    let candidates = [
        id,
        SceneId { section: 0, ..id },
        SceneId {
            scene: 0,
            section: 0,
            ..id
        },
        SceneId {
            episode: 0,
            scene: 0,
            section: 0,
            ..id
        },
    ];
    for wanted in candidates {
        if let Some(rec) = records.iter().find(|r| r.id == wanted) {
            if let Some(raw) = rec.fields.get("space").filter(|s| !s.trim().is_empty()) {
                let resolved = crate::core::features::metadata::asset_path(raw, false)
                    .and_then(|p| project.document(&p))
                    .map(|d| d.text.clone())
                    .unwrap_or_else(|| raw.clone());
                return resolved.lines().map(ToOwned::to_owned).collect();
            }
        }
    }
    Vec::new()
}

/// 構成ツリー（§10.3.1）の場面ノード。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineSceneNode {
    pub scene: u16,
    pub title: String,
    pub chars: usize,
}

/// 構成ツリー（§10.3.1）の話ノード。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineEpisodeNode {
    pub episode: u16,
    pub title: String,
    pub chars: usize,
    pub section_count: usize,
    pub scenes: Vec<OutlineSceneNode>,
}

/// 構成ツリー（§10.3.1）の章ノード（ストック章 `0` は常に末尾）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineChapterNode {
    pub chapter: u16,
    pub title: String,
    pub chars: usize,
    pub episode_count: usize,
    pub foldable: bool,
    pub episodes: Vec<OutlineEpisodeNode>,
}

/// 章 → 話 → 場面の 3 階層構成ツリーと文字数・下位数を集計する（§10.3.1）。
/// ストック章（`chapter == 0`）は番号順にかかわらず常に最後尾に置く。
pub fn outline_tree(project: &Project) -> Vec<OutlineChapterNode> {
    use std::collections::BTreeMap;
    let progress_rows = progress(project);
    let titles_all = outline_blocks(project, None);
    let mut chapter_list = chapters(project);
    chapter_list.sort_by_key(|&ch| (ch == 0, ch));
    chapter_list.dedup();

    let mut out = Vec::new();
    for chapter in chapter_list {
        let title = titles_all
            .iter()
            .find(|b| b.id.chapter == chapter && b.id.episode == 0)
            .map(|b| b.title.clone())
            .unwrap_or_default();
        let ch_blocks = outline_blocks(project, Some(chapter));
        let ep_numbers = episodes(project, chapter);

        let mut ep_nodes = Vec::new();
        let mut ch_chars = 0usize;
        let mut regular_episode_count = 0usize;

        for ep in ep_numbers {
            let ep_title = ch_blocks
                .iter()
                .find(|b| b.id.chapter == chapter && b.id.episode == ep)
                .map(|b| b.title.clone())
                .unwrap_or_default();
            let mut ep_chars = 0usize;
            let mut sec_count = 0usize;
            let mut scene_map: BTreeMap<u16, (String, usize)> = BTreeMap::new();

            if ep > 0 {
                for row in episode_rows(project, chapter, ep) {
                    if row.is_scene() && row.id.scene > 0 {
                        scene_map
                            .entry(row.id.scene)
                            .or_insert_with(|| (row.summary, 0));
                    }
                }
            }
            for prow in progress_rows
                .iter()
                .filter(|r| r.id.chapter == chapter && r.id.episode == ep)
            {
                if prow.depth == 2 && prow.id.scene > 0 {
                    let entry = scene_map
                        .entry(prow.id.scene)
                        .or_insert_with(|| (prow.title.clone(), 0));
                    if entry.0.is_empty() && !prow.title.is_empty() {
                        entry.0 = prow.title.clone();
                    }
                } else if prow.depth == 3 && prow.id.section > 0 {
                    ep_chars += prow.chars;
                    sec_count += 1;
                    let entry = scene_map
                        .entry(prow.id.scene)
                        .or_insert_with(|| (String::new(), 0));
                    entry.1 += prow.chars;
                }
            }
            if ep == 0 {
                let syn_path = project.contents_path(chapter, 0);
                if let Some(doc) = project.document(&syn_path) {
                    ep_chars = crate::core::features::sections::count_chars(&doc.text);
                }
            } else {
                regular_episode_count += 1;
            }
            ch_chars += ep_chars;
            let scenes = scene_map
                .into_iter()
                .map(|(sc, (sc_title, sc_chars))| OutlineSceneNode {
                    scene: sc,
                    title: sc_title,
                    chars: sc_chars,
                })
                .collect();
            ep_nodes.push(OutlineEpisodeNode {
                episode: ep,
                title: ep_title,
                chars: ep_chars,
                section_count: sec_count,
                scenes,
            });
        }

        let foldable = !ep_nodes.is_empty();
        out.push(OutlineChapterNode {
            chapter,
            title,
            chars: ch_chars,
            episode_count: regular_episode_count,
            foldable,
            episodes: ep_nodes,
        });
    }
    out
}
