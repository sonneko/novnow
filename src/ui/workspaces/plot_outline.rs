//! `plot-outline` — 全体 / 章プロット（§10.3）。タイムラインなどのタブは `tabs.rs`、章 → 話の
//! 折りたたみは [`crate::core::features::folding`] が担う。
//!
//! ```text
//! ┌────────┬──────────────────────────────┬────────────────┐
//! │ 全体    │ 上: 親のブロック（章のときだけ）│ 伏線            │
//! │ 01 …   ├──────────────────────────────┤  未回収を上に    │
//! │ 02 …   │ 中: ブロックの一覧（1 段落ずつ）│                │
//! │        ├──────────────────────────────┤                │
//! │        │ 下: 選んだブロックのエディタ    │                │
//! └────────┴──────────────────────────────┴────────────────┘
//! ```
//!
//! - 左のレベル切替（全体 / 各章）。`Ctrl+B`（`Ctrl+Shift+B`）で上位へ、`Ctrl+Enter` で下位へ
//!   （章のブロック → その章、話のブロック → `plot-episode`）。`Ctrl+Alt+←/→` で前 / 次のレベル。
//! - 下位へ降りても**親が常に見える**（章のときは全体プロットの章のブロックを上段に出す。§10.3.1）。
//! - 右は伏線（§10.3 の既定のタブ）。`seed` / `payoff` を伏線 ID で突き合わせ、未回収を上に並べる。

use std::collections::BTreeSet;

use egui::{Align, Context, FontFamily, FontId, Frame, Margin, RichText, Ui, Vec2};

use crate::core::features::document_symbol::{self, SymbolKind};
use crate::core::features::folding;
use crate::core::features::plots::{self, Foreshadow, OutlineBlock};
use crate::core::features::project::line_starts_of;
use crate::core::id::SceneId;
use crate::ui::i18n;
use crate::ui::settings::PaneSide;
use crate::ui::theme;
use crate::ui::widgets::context_view;
use crate::ui::widgets::reference_pane;
use crate::ui::widgets::slice_editor;
use crate::ui::widgets::syntax_text::{self, Highlight, Metrics};
use crate::ui::workbench::{Nav, Workbench};

use super::{list_row, state_color, Request, Row, View};

/// レベル（全体 = `None`、章 = `Some(章)`）。
pub type Level = Option<u16>;

/// 親のブロックの中身（文字列・色分け）。
type Parent = (String, Highlight);

/// 左の構成ツリーの 1 章ぶん（§10.3.1: 章 → 話 → 場面の 3 階層と集計）。
type ChapterNode = plots::OutlineChapterNode;

/// `plot-outline` の状態（切替で破棄しない、§10.0）。
#[derive(Debug, Default)]
pub struct State {
    /// 見ているレベル。まだ一度も開いていなければ `None`。
    level: Option<Level>,
    /// 選んでいるブロックの ID。
    selected: Option<SceneId>,
    blocks: Option<(u64, Level, Vec<OutlineBlock>)>,
    chapters: Option<(u64, Vec<ChapterNode>)>,
    foreshadows: Option<(u64, Vec<Foreshadow>)>,
    parent: Option<(u64, Level, Option<Parent>)>,
    /// 構成ツリーで展開している単位（章 `@01`、話 `@01-04`）。
    expanded: BTreeSet<String>,
    editor: slice_editor::Cache,
    reveal: bool,
    /// 一覧の選択だけを動かし、そのブロックの本文は開かず確定もしない（§10.3）。
    preview: bool,
    /// キーだけで選択を動かした直後に、一覧を 1 回スクロールして見せる。
    scroll_list: bool,
}

/// `Ctrl+Enter` の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Down {
    /// 章のレベルへ降りた（同じワークスペースの中）。
    Chapter,
    /// 話のブロック → `plot-episode` へ降りる。
    Episode(u16, u16),
}

impl State {
    /// 見ているレベル。
    pub fn level(&self) -> Option<Level> {
        self.level
    }

    /// 選んでいるブロックの ID。
    pub fn selected(&self) -> Option<SceneId> {
        self.selected
    }

    /// 章のレベルの、話 `episode` のブロックに着地する（初めて開くとき・`plot-episode` から上へ）。
    /// `episode == 0` なら章の最初のブロック。
    pub fn land_chapter(&mut self, bench: &Workbench, chapter: u16, episode: u16) {
        self.level = Some(Some(chapter));
        let wanted = SceneId {
            chapter,
            episode,
            scene: 0,
            section: 0,
        };
        let blocks = self.blocks(bench);
        self.selected = blocks
            .iter()
            .find(|block| block.id == wanted)
            .or(blocks.first())
            .map(|block| block.id)
            .or(Some(wanted));
        self.preview = false;
        self.reveal = true;
    }

    /// 全体のレベルの、章 `chapter` のブロックに着地する。
    pub fn land_all(&mut self, bench: &Workbench, chapter: u16) {
        self.level = Some(None);
        let blocks = self.blocks(bench);
        self.selected = blocks
            .iter()
            .find(|block| block.id.chapter == chapter)
            .or(blocks.first())
            .map(|block| block.id);
        self.preview = false;
        self.reveal = true;
    }

    /// 今のレベルのブロック（必要なら作り直す）。
    pub fn blocks(&mut self, bench: &Workbench) -> &[OutlineBlock] {
        let Some(level) = self.level else {
            return &[];
        };
        let fresh = matches!(&self.blocks, Some((generation, at, _)) if *generation == bench.generation() && *at == level);
        if !fresh {
            let mut blocks = plots::outline_blocks(bench.project(), level);
            if let Some(chapter) = level {
                // 章プロットに書かれた他の章のブロック（書き間違い）は出さない
                blocks.retain(|block| block.id.chapter == chapter && block.id.episode > 0);
            } else {
                blocks.retain(|block| block.id.episode == 0);
                // ストック章（`00`）は番号順では先頭だが、表示上は常に最後に置く（§10.3.1）
                blocks.sort_by_key(|block| block.id.chapter == 0);
            }
            self.blocks = Some((bench.generation(), level, blocks));
        }
        self.blocks
            .as_ref()
            .map_or(&[], |(_, _, blocks)| blocks.as_slice())
    }

    fn chapter_key(chapter: u16) -> String {
        format!("@{chapter:02}")
    }

    fn episode_key(chapter: u16, episode: u16) -> String {
        format!("@{chapter:02}-{episode:02}")
    }

    /// 章 `chapter` の話一覧を展開しているか。
    pub fn is_expanded(&self, chapter: u16) -> bool {
        self.expanded.contains(&Self::chapter_key(chapter))
    }

    /// 話 `(chapter, episode)` の場面一覧を展開しているか。
    pub fn is_episode_expanded(&self, chapter: u16, episode: u16) -> bool {
        self.expanded.contains(&Self::episode_key(chapter, episode))
    }

    /// 展開している単位の鍵（`@NN` / `@NN-MM`。`.novnow/ui.json` の `folded.outline`、§4.4.1）。
    pub fn expanded_keys(&self) -> Vec<String> {
        self.expanded.iter().cloned().collect()
    }

    /// `.novnow/ui.json` の `folded.outline` から展開状態を復元する（§4.4.1・§10.3.1）。
    /// 読めない鍵は無視する（壊れていても既定値で動く、§4.4）。
    pub fn restore_expanded(&mut self, keys: &[String]) {
        self.expanded.clear();
        for raw in keys {
            let trimmed = raw.trim().trim_start_matches('@');
            let parts: Vec<&str> = trimmed.split('-').collect();
            match parts.as_slice() {
                [ch] => {
                    if let Ok(c) = ch.parse::<u16>() {
                        self.expanded.insert(Self::chapter_key(c));
                    }
                }
                [ch, ep, ..] => {
                    if let (Ok(c), Ok(e)) = (ch.parse::<u16>(), ep.parse::<u16>()) {
                        self.expanded.insert(Self::chapter_key(c));
                        if e > 0 {
                            self.expanded.insert(Self::episode_key(c, e));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// 1 つの章だけを畳む（`fold == true`）/ 展開する（`plot.outline_fold`、§10.3.1）。
    pub fn set_chapter_folded(&mut self, chapter: u16, fold: bool) {
        let key = Self::chapter_key(chapter);
        if fold {
            let prefix = format!("{key}-");
            self.expanded
                .retain(|k| k != &key && !k.starts_with(&prefix));
        } else {
            self.expanded.insert(key);
        }
    }

    /// 1 つの話の場面一覧だけを畳む / 展開する。
    pub fn set_episode_folded(&mut self, chapter: u16, episode: u16, fold: bool) {
        let key = Self::episode_key(chapter, episode);
        if fold {
            self.expanded.remove(&key);
        } else {
            self.expanded.insert(Self::chapter_key(chapter));
            self.expanded.insert(key);
        }
    }

    /// すべて畳む（`Ctrl+K Ctrl+0`）。
    pub fn fold_all(&mut self) {
        self.expanded.clear();
    }

    /// すべて展開する（`Ctrl+K Ctrl+J`）。
    pub fn unfold_all(&mut self, bench: &Workbench) {
        for node in self.chapters(bench).to_vec() {
            self.expanded.insert(Self::chapter_key(node.chapter));
            for ep in &node.episodes {
                if ep.episode > 0 && !ep.scenes.is_empty() {
                    self.expanded
                        .insert(Self::episode_key(node.chapter, ep.episode));
                }
            }
        }
    }

    /// 検索ヒット等で対象 `id` が見えるよう該当する章・話を自動展開する（§9.3.3・§9.7.4）。
    pub fn expand_for_scene(&mut self, id: SceneId) {
        self.expanded.insert(Self::chapter_key(id.chapter));
        if id.episode > 0 && id.scene > 0 {
            self.expanded
                .insert(Self::episode_key(id.chapter, id.episode));
        }
    }

    /// 今の単位を段階的に畳む（`fold == true`）/ 展開する（`false`、§9.3.3・§10.3.1）。
    pub fn set_folded(&mut self, bench: &Workbench, fold: bool) -> bool {
        let sel = self.selected;
        let target = self
            .level
            .flatten()
            .or_else(|| sel.map(|id| id.chapter))
            .or_else(|| self.chapters(bench).first().map(|node| node.chapter));
        let Some(chapter) = target else {
            return false;
        };
        if let Some(ep) = sel
            .filter(|id| id.chapter == chapter && id.episode > 0)
            .map(|id| id.episode)
        {
            let has_scenes = self
                .chapters(bench)
                .iter()
                .find(|c| c.chapter == chapter)
                .and_then(|c| c.episodes.iter().find(|e| e.episode == ep))
                .is_some_and(|e| !e.scenes.is_empty());
            if fold {
                if self.is_episode_expanded(chapter, ep) {
                    self.set_episode_folded(chapter, ep, true);
                    return true;
                }
            } else if self.is_expanded(chapter)
                && has_scenes
                && !self.is_episode_expanded(chapter, ep)
            {
                self.set_episode_folded(chapter, ep, false);
                return true;
            }
        }
        self.set_chapter_folded(chapter, fold);
        true
    }

    fn chapters(&mut self, bench: &Workbench) -> &[ChapterNode] {
        let fresh =
            matches!(&self.chapters, Some((generation, _)) if *generation == bench.generation());
        if !fresh {
            let project = bench.project();
            let all_folds = project
                .document("plots/all.cnov")
                .map(folding::ranges)
                .unwrap_or_default();
            let mut list = plots::outline_tree(project);
            for node in &mut list {
                let path = project.chapter_plot_path(node.chapter);
                let (ranges, symbols) = project
                    .document(&path)
                    .map(|doc| (folding::ranges(doc), document_symbol::all(doc)))
                    .unwrap_or_default();
                for ep in &mut node.episodes {
                    if ep.title.is_empty() && ep.episode > 0 {
                        if let Some(detail) = symbols
                            .iter()
                            .filter(|s| s.kind == SymbolKind::Episode)
                            .find_map(|s| {
                                let id = SceneId::parse(&s.name).ok()?;
                                (id.chapter == node.chapter && id.episode == ep.episode)
                                    .then(|| s.detail.clone())
                            })
                        {
                            ep.title = detail;
                        }
                    }
                }
                node.foldable = !node.episodes.is_empty()
                    || ranges.iter().any(|item| item.depth == 2)
                    || all_folds.iter().any(|item| item.depth == 1);
            }
            self.chapters = Some((bench.generation(), list));
        }
        self.chapters
            .as_ref()
            .map_or(&[], |(_, list)| list.as_slice())
    }

    /// 伏線（必要なら作り直す）。
    pub fn foreshadows(&mut self, bench: &Workbench) -> &[Foreshadow] {
        let fresh =
            matches!(&self.foreshadows, Some((generation, _)) if *generation == bench.generation());
        if !fresh {
            self.foreshadows = Some((bench.generation(), plots::foreshadows(bench.project())));
        }
        self.foreshadows
            .as_ref()
            .map_or(&[], |(_, list)| list.as_slice())
    }

    fn selected_index(&mut self, bench: &Workbench) -> Option<usize> {
        let selected = self.selected;
        let blocks = self.blocks(bench);
        if blocks.is_empty() {
            return None;
        }
        Some(
            selected
                .and_then(|id| blocks.iter().position(|block| block.id == id))
                .unwrap_or(0),
        )
    }

    /// 文書のパス（全体 `plots/all.cnov`、章 `plots/AA.cnov`）。
    fn path(&self, bench: &Workbench) -> Option<String> {
        match self.level? {
            None => Some("plots/all.cnov".to_owned()),
            Some(chapter) => Some(bench.project().chapter_plot_path(chapter)),
        }
    }

    /// 選んだブロックをエディタ（Slice）に開く。
    pub fn sync(&mut self, bench: &mut Workbench) {
        if self.preview {
            return;
        }
        let index = self.selected_index(bench);
        let found = index.and_then(|at| self.blocks(bench).get(at).cloned());
        match found {
            Some(block) => {
                let digits = bench.project().digits();
                let tag = format!(
                    "plot-outline:{}:{}",
                    block.path,
                    plots::short_id(block.id, digits)
                );
                bench.open_slice(&block.path, block.lines.clone(), &tag);
                self.selected = Some(block.id);
            }
            None => bench.close_slice(),
        }
    }

    /// エディタを確定も変更もせずに、隣のブロックへ選択だけ移す（§10.3）。
    pub fn preview_row(&mut self, bench: &Workbench, forward: bool) -> bool {
        let at = self.selected_index(bench);
        let blocks = self.blocks(bench);
        let next = if forward {
            at.map_or(Some(0), |index| {
                (index + 1 < blocks.len()).then_some(index + 1)
            })
        } else {
            at.and_then(|index| index.checked_sub(1))
        };
        let Some(id) = next
            .and_then(|index| blocks.get(index))
            .map(|block| block.id)
        else {
            return false;
        };
        self.selected = Some(id);
        self.preview = true;
        self.scroll_list = true;
        self.reveal = false;
        true
    }

    /// 構造移動（`Alt+↑/↓`・`Alt+←/→` = 前 / 次のブロック、`Ctrl+Alt+←/→` = 前 / 次のレベル、
    /// `Ctrl+Home/End`）。
    pub fn nav(&mut self, bench: &mut Workbench, nav: Nav) -> bool {
        self.preview = false;
        bench.commit();
        if matches!(nav, Nav::PrevFile | Nav::NextFile) {
            return self.step_level(bench, nav == Nav::NextFile);
        }
        let index = self.selected_index(bench);
        let count = self.blocks(bench).len();
        let target = match nav {
            Nav::PrevSection | Nav::PrevScene => index.and_then(|at| at.checked_sub(1)),
            Nav::NextSection | Nav::NextScene => index.filter(|at| at + 1 < count).map(|at| at + 1),
            Nav::FileStart => (count > 0).then_some(0),
            Nav::FileEnd => count.checked_sub(1),
            Nav::PrevFile | Nav::NextFile => None,
        };
        match target {
            Some(found) if Some(found) != index => {
                self.selected = self.blocks(bench).get(found).map(|block| block.id);
                self.reveal = true;
                bench.save.request_now();
                true
            }
            _ => false,
        }
    }

    /// 前 / 次のレベル（全体 → 01 → 02 …）。
    fn step_level(&mut self, bench: &mut Workbench, forward: bool) -> bool {
        let mut levels: Vec<Level> = vec![None];
        levels.extend(self.chapters(bench).iter().map(|node| Some(node.chapter)));
        let current = self.level.unwrap_or(None);
        let at = levels
            .iter()
            .position(|level| *level == current)
            .unwrap_or(0);
        let next = if forward {
            levels.get(at + 1)
        } else {
            at.checked_sub(1).and_then(|before| levels.get(before))
        };
        match next.copied() {
            Some(None) => {
                self.land_all(bench, current.unwrap_or(0));
                true
            }
            Some(Some(chapter)) => {
                self.land_chapter(bench, chapter, 0);
                true
            }
            None => false,
        }
    }

    /// 1 つ上へ（章 → 全体）。全体にいれば `false`（それより上は無い）。
    pub fn up(&mut self, bench: &mut Workbench) -> bool {
        bench.commit();
        match self.level {
            Some(Some(chapter)) => {
                self.land_all(bench, chapter);
                true
            }
            _ => false,
        }
    }

    /// 1 つ下へ（全体の章のブロック → その章、章の話のブロック → `plot-episode`）。
    pub fn down(&mut self, bench: &mut Workbench) -> Option<Down> {
        bench.commit();
        let index = self.selected_index(bench)?;
        let block = self.blocks(bench).get(index)?.clone();
        match self.level? {
            None => {
                self.land_chapter(bench, block.id.chapter, 0);
                Some(Down::Chapter)
            }
            Some(_) => Some(Down::Episode(block.id.chapter, block.id.episode)),
        }
    }

    /// ブロックを足して選ぶ（`Ctrl+Shift+N`、B-129）。文書が無ければ作る。
    pub fn insert(&mut self, bench: &mut Workbench, now: f64) -> bool {
        let (Some(level), Some(path)) = (self.level, self.path(bench)) else {
            return false;
        };
        self.preview = false;
        bench.commit();
        let index = self.selected_index(bench);
        let blocks = self.blocks(bench).to_vec();
        let id = plots::next_block_id(&blocks, level);
        let digits = bench.project().digits();
        let text = bench.project().text(&path).to_owned();
        let (updated, _) = plots::insert_block(&text, &blocks, index, id, digits);
        bench.replace_document(&path, updated, now);
        self.selected = Some(id);
        self.reveal = true;
        true
    }

    /// パンくず（全体 / `01` / `01›04`）。全体の文字はカタログから。
    pub fn breadcrumb(&self, bench: &Workbench, all: &str) -> Option<String> {
        let digits = bench.project().digits();
        match self.level? {
            None => Some(match self.selected {
                Some(id) => format!("{all}›{}", plots::short_id(id, digits)),
                None => all.to_owned(),
            }),
            Some(_) => self
                .selected
                .map(|id| plots::short_id(id, digits).replace('-', "›")),
        }
    }

    fn parent(&mut self, bench: &Workbench) -> Option<&Parent> {
        let level = self.level?;
        let fresh = matches!(&self.parent, Some((generation, at, _)) if *generation == bench.generation() && *at == level);
        if !fresh {
            let found = level.and_then(|chapter| {
                let block = plots::outline_blocks(bench.project(), None)
                    .into_iter()
                    .find(|block| block.id.chapter == chapter && block.id.episode == 0)?;
                let document = bench.project().document(&block.path)?;
                let starts = line_starts_of(&document.text);
                let range = context_view::byte_range(&starts, document.text.len(), &block.lines);
                let text = document.text.get(range.clone())?.to_owned();
                Some((text, Highlight::of_range(document, range, bench.project())))
            });
            self.parent = Some((bench.generation(), level, found));
        }
        self.parent
            .as_ref()
            .and_then(|(_, _, found)| found.as_ref())
    }
}

/// 1 フレーム。
pub fn show(
    ctx: &Context,
    state: &mut State,
    bench: &mut Workbench,
    view: &View,
) -> Option<Request> {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let locale = view.locale;
    state.sync(bench);
    if let Some(forward) = super::browse_keys(ctx, view) {
        state.preview_row(bench, forward);
    }
    let mut request = None;
    // 左: 構成ツリー（章 → 話 → 場面、§10.3.1）
    super::fixed_width(
        egui::SidePanel::left("outline-levels"),
        250.0 * scale.min(1.2),
        8.0,
    )
    .frame(
        Frame::none()
            .fill(colors.bg_base)
            .inner_margin(Margin::symmetric(8.0, 8.0)),
    )
    .show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .id_source("outline-tree-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let current = state.level.unwrap_or(None);
                let mut chosen: Option<Level> = None;
                let mut chosen_episode: Option<(u16, u16)> = None;
                let mut chosen_scene: Option<SceneId> = None;
                let all = i18n::text(locale, "outline.all");
                if list_row(
                    ui,
                    view.mode,
                    scale,
                    Row {
                        selected: current.is_none(),
                        text: all,
                        ..Row::default()
                    },
                )
                .clicked()
                {
                    chosen = Some(None);
                }
                let chapters = state.chapters(bench).to_vec();
                let digits = bench.project().digits();
                for node in chapters {
                    let chapter = node.chapter;
                    let expanded = state.is_expanded(chapter);
                    let arrow = if !node.foldable || node.episodes.is_empty() {
                        ""
                    } else if expanded {
                        "▾ "
                    } else {
                        "▸ "
                    };
                    let ch_label = format!(
                        "{arrow}{}",
                        plots::short_id(
                            SceneId {
                                chapter,
                                episode: 0,
                                scene: 0,
                                section: 0,
                            },
                            digits,
                        )
                    );
                    let ch_title = if chapter == 0 && node.title.is_empty() {
                        i18n::text(locale, "stage.outline_stock").to_owned()
                    } else {
                        node.title.clone()
                    };
                    let ch_stats = format!(
                        "{}  {}",
                        i18n::format(
                            locale,
                            "stage.stock_chars_unit",
                            &[&context_view::group(node.chars)],
                        ),
                        i18n::format(
                            locale,
                            "stage.outline_episodes_unit",
                            &[&node.episode_count.to_string()],
                        ),
                    );
                    let ch_text = if ch_title.is_empty() {
                        ch_stats
                    } else {
                        format!("{ch_title}  {ch_stats}")
                    };
                    let ch_resp = list_row(
                        ui,
                        view.mode,
                        scale,
                        Row {
                            selected: current == Some(chapter),
                            label: &ch_label,
                            text: &ch_text,
                            ..Row::default()
                        },
                    );
                    if ch_resp.clicked() {
                        if current == Some(chapter) && !node.episodes.is_empty() {
                            state.set_chapter_folded(chapter, expanded);
                        }
                        chosen = Some(Some(chapter));
                    }
                    if expanded {
                        for ep_node in &node.episodes {
                            let episode = ep_node.episode;
                            let ep_id = SceneId {
                                chapter,
                                episode,
                                scene: 0,
                                section: 0,
                            };
                            let ep_expanded =
                                episode > 0 && state.is_episode_expanded(chapter, episode);
                            let ep_arrow = if episode == 0 || ep_node.scenes.is_empty() {
                                "  "
                            } else if ep_expanded {
                                "▾ "
                            } else {
                                "▸ "
                            };
                            let ep_label = if episode == 0 {
                                format!(
                                    "    {}-{:0width$}",
                                    plots::short_id(
                                        SceneId {
                                            chapter,
                                            episode: 0,
                                            scene: 0,
                                            section: 0,
                                        },
                                        digits,
                                    ),
                                    0,
                                    width = usize::from(digits.episode),
                                )
                            } else {
                                format!("  {ep_arrow}{}", plots::short_id(ep_id, digits))
                            };
                            let base_ep_title = if episode == 0 && ep_node.title.is_empty() {
                                i18n::text(locale, "stage.outline_synopsis").to_owned()
                            } else {
                                ep_node.title.clone()
                            };
                            let ep_stats = if episode == 0 {
                                i18n::format(
                                    locale,
                                    "stage.stock_chars_unit",
                                    &[&context_view::group(ep_node.chars)],
                                )
                            } else {
                                format!(
                                    "{}  {}",
                                    i18n::format(
                                        locale,
                                        "stage.stock_chars_unit",
                                        &[&context_view::group(ep_node.chars)],
                                    ),
                                    i18n::format(
                                        locale,
                                        "stage.outline_sections_unit",
                                        &[&ep_node.section_count.to_string()],
                                    ),
                                )
                            };
                            let ep_text = if base_ep_title.is_empty() {
                                ep_stats
                            } else {
                                format!("{base_ep_title}  {ep_stats}")
                            };
                            let is_ep_selected =
                                current == Some(chapter) && state.selected == Some(ep_id);
                            let ep_resp = list_row(
                                ui,
                                view.mode,
                                scale,
                                Row {
                                    selected: is_ep_selected,
                                    label: &ep_label,
                                    text: &ep_text,
                                    muted: true,
                                    ..Row::default()
                                },
                            );
                            if ep_resp.double_clicked() && episode > 0 {
                                chosen_scene = Some(SceneId {
                                    chapter,
                                    episode,
                                    scene: 1,
                                    section: 0,
                                });
                            } else if ep_resp.clicked() && episode > 0 {
                                if is_ep_selected && !ep_node.scenes.is_empty() {
                                    state.set_episode_folded(chapter, episode, ep_expanded);
                                }
                                chosen_episode = Some((chapter, episode));
                            }
                            if ep_expanded {
                                for sc_node in &ep_node.scenes {
                                    let sc_id = SceneId {
                                        chapter,
                                        episode,
                                        scene: sc_node.scene,
                                        section: 0,
                                    };
                                    let sc_prefix = sc_id
                                        .display(digits)
                                        .split('-')
                                        .take(3)
                                        .collect::<Vec<_>>()
                                        .join("-");
                                    let sc_label = format!("      {sc_prefix}");
                                    let sc_chars = i18n::format(
                                        locale,
                                        "stage.stock_chars_unit",
                                        &[&context_view::group(sc_node.chars)],
                                    );
                                    let sc_text = if sc_node.title.is_empty() {
                                        sc_chars
                                    } else {
                                        format!("{}  {sc_chars}", sc_node.title)
                                    };
                                    if list_row(
                                        ui,
                                        view.mode,
                                        scale,
                                        Row {
                                            selected: false,
                                            label: &sc_label,
                                            text: &sc_text,
                                            muted: true,
                                            ..Row::default()
                                        },
                                    )
                                    .clicked()
                                    {
                                        chosen_scene = Some(sc_id);
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(sc_id) = chosen_scene {
                    request = Some(Request::OpenEpisodeScene(sc_id));
                } else if let Some((chapter, episode)) = chosen_episode {
                    bench.commit();
                    state.land_chapter(bench, chapter, episode);
                } else {
                    match chosen {
                        Some(None) => {
                            bench.commit();
                            state.land_all(bench, current.unwrap_or(0));
                        }
                        Some(Some(chapter)) => {
                            bench.commit();
                            state.land_chapter(bench, chapter, 0);
                        }
                        None => {}
                    }
                }
            });
    });
    // 右: 伏線
    if let Some((side, width)) = view.reference {
        let panel = match side {
            PaneSide::Right => egui::SidePanel::right("reference"),
            PaneSide::Left => egui::SidePanel::left("reference"),
        };
        super::fixed_width(panel, width, 14.0)
            .frame(
                Frame::none()
                    .fill(colors.bg_base)
                    .inner_margin(Margin::symmetric(14.0, 4.0)),
            )
            .show(ctx, |ui| {
                if let Some(found) = foreshadow_pane(ui, state, bench, view) {
                    request = Some(found);
                }
            });
    }
    egui::CentralPanel::default()
        .frame(
            Frame::none()
                .fill(colors.bg_base)
                .inner_margin(Margin::symmetric(16.0, 6.0)),
        )
        .show(ctx, |ui| {
            let total = ui.available_height();
            // 上: 親のブロック（章のときだけ）
            if state.level.is_some_and(|level| level.is_some()) {
                let top = (total * 0.18).max(48.0);
                ui.allocate_ui(Vec2::new(ui.available_width(), top), |ui| {
                    ui.set_min_height(top);
                    parent(ui, state, bench, view);
                });
                super::separator(ui, view.mode);
            }
            let middle = (ui.available_height() * 0.45).max(80.0);
            ui.allocate_ui(Vec2::new(ui.available_width(), middle), |ui| {
                ui.set_min_height(middle);
                if let Some(found) = block_list(ui, state, bench, view) {
                    request = Some(found);
                }
            });
            super::separator(ui, view.mode);
            if bench.slice().is_some() {
                if state.reveal {
                    let id = slice_editor::editor_id(bench);
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
                let output = slice_editor::show(ui, bench, &mut state.editor, view.look());
                if output.changed {
                    bench.slice_edited(view.now);
                }
            } else {
                super::hint(ui, view.mode, scale, i18n::text(locale, "outline.empty"));
            }
            state.reveal = false;
        });
    request
}

/// 上段: 親（全体プロットの章のブロック）。
fn parent(ui: &mut Ui, state: &mut State, bench: &Workbench, view: &View) {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    egui::ScrollArea::vertical()
        .id_source("outline-parent")
        .auto_shrink([false, false])
        .show(ui, |ui| match state.parent(bench) {
            Some((text, highlight)) => {
                let metrics = Metrics {
                    size: 13.0 * scale,
                    line_height: 1.6,
                };
                let job =
                    syntax_text::job(text, highlight, view.mode, metrics, ui.available_width());
                ui.label(job);
            }
            None => {
                ui.label(
                    RichText::new(i18n::text(view.locale, "outline.no_parent"))
                        .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                        .color(colors.fg_faint),
                );
            }
        });
}

/// 中段: ブロックの一覧（1 ブロック = 1 段落: 見出しの行と `summary:` の 1 行目）。ダブルクリックで下位へ。
fn block_list(ui: &mut Ui, state: &mut State, bench: &Workbench, view: &View) -> Option<Request> {
    let digits = bench.project().digits();
    let selected = state.selected_index(bench);
    let reveal = state.reveal || std::mem::take(&mut state.scroll_list);
    let blocks = state.blocks(bench).to_vec();
    let progress = plots::progress(bench.project());
    let lengths: Vec<_> = blocks
        .iter()
        .map(|b| {
            progress
                .iter()
                .find(|r| r.id == b.id)
                .map_or(0, |r| r.chars)
        })
        .collect();
    let scores = crate::core::features::metrics::t_scores(&lengths);
    if blocks.is_empty() {
        let path = state.path(bench).unwrap_or_default();
        super::hint(
            ui,
            view.mode,
            view.scale,
            &i18n::format(view.locale, "outline.no_blocks", &[&path]),
        );
        if super::action_button(ui, i18n::text(view.locale, "outline.new_button")).clicked() {
            return Some(Request::Shortcut(crate::ui::shell::Command::InsertSection));
        }
        return None;
    }
    let colors = theme::palette(view.mode);
    let mut request = None;
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_source("outline-blocks")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, block) in blocks.iter().enumerate() {
                let is_selected = Some(index) == selected;
                let id = plots::short_id(block.id, digits);
                let response = list_row(
                    ui,
                    view.mode,
                    view.scale,
                    Row {
                        selected: is_selected,
                        bar: block.state.map(|found| state_color(view.mode, Some(found))),
                        label: &id,
                        text: &block.title,
                        muted: false,
                    },
                );
                let t = scores[index];
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}  T {}",
                        lengths[index],
                        t.map_or_else(|| "—".to_owned(), |n| format!("{n:.0}"))
                    ));
                    if t.is_some_and(|n| !(35.0..=65.0).contains(&n)) {
                        ui.colored_label(egui::Color32::from_rgb(210, 145, 45), "⚠");
                    }
                });
                if !block.summary.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(40.0 * view.scale);
                        ui.label(
                            RichText::new(&block.summary)
                                .font(FontId::new(12.0 * view.scale, FontFamily::Proportional))
                                .color(colors.fg_muted),
                        );
                    });
                }
                ui.add_space(6.0);
                if is_selected && reveal {
                    response.scroll_to_me(Some(Align::Center));
                }
                if response.double_clicked() {
                    request = Some(Request::Down);
                }
                if response.clicked() {
                    clicked = Some(block.id);
                }
            }
        });
    if let Some(id) = clicked {
        state.selected = Some(id);
        state.preview = false;
        state.reveal = true;
    }
    request
}

/// 右: 伏線（未回収 → 二重回収 → その他）。
fn foreshadow_pane(
    ui: &mut Ui,
    state: &mut State,
    bench: &Workbench,
    view: &View,
) -> Option<Request> {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let locale = view.locale;
    let list = state.foreshadows(bench).to_vec();
    reference_pane::heading(
        ui,
        i18n::text(locale, "outline.foreshadow"),
        view.mode,
        scale,
    );
    if list.is_empty() {
        super::hint(
            ui,
            view.mode,
            scale,
            i18n::text(locale, "outline.no_foreshadow"),
        );
        return None;
    }
    let digits = bench.project().digits();
    let site = |found: &plots::ForeshadowSite| match found.block {
        Some(id) => format!("@{}", plots::short_id(id, digits)),
        None => found.path.clone(),
    };
    egui::ScrollArea::vertical()
        .id_source("outline-foreshadow")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for item in &list {
                let (status, color) = if item.open() {
                    (i18n::text(locale, "outline.open"), colors.warn)
                } else if item.doubled() {
                    (i18n::text(locale, "outline.doubled"), colors.error)
                } else if item.seeds.is_empty() {
                    (i18n::text(locale, "outline.no_seed"), colors.warn)
                } else {
                    (i18n::text(locale, "outline.closed"), colors.ok)
                };
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&item.tag)
                            .font(FontId::new(12.0 * scale, FontFamily::Monospace))
                            .color(colors.fg_primary),
                    );
                    ui.label(
                        RichText::new(status)
                            .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                            .color(color),
                    );
                });
                for seed in &item.seeds {
                    let mut text = format!("{} {}", i18n::text(locale, "outline.seed"), site(seed));
                    if !seed.note.is_empty() {
                        text.push_str("  ");
                        text.push_str(&seed.note);
                    }
                    if !seed.target.is_empty() {
                        text.push_str(" → ");
                        text.push_str(&seed.target);
                    }
                    ui.label(
                        RichText::new(text)
                            .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                            .color(colors.fg_muted),
                    );
                }
                for payoff in &item.payoffs {
                    let text = format!("{} {}", i18n::text(locale, "outline.payoff"), site(payoff));
                    ui.label(
                        RichText::new(text)
                            .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                            .color(colors.fg_muted),
                    );
                }
            }
        });
    None
}
