//! `plot-outline` — 全体 / 章プロット（§10.3。v1 ① は単一ビュー: タブ・折りたたみは ②）。
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
//! - 下位へ降りても**親が常に見える**（章のときは全体プロットの章のブロックを上段に出す。§19.11）。
//! - 右は伏線（§10.3 の既定のタブ）。`seed` / `payoff` を伏線 ID で突き合わせ、未回収を上に並べる。

use egui::{Align, Context, FontFamily, FontId, Frame, Margin, RichText, Ui, Vec2};

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

/// `plot-outline` の状態（切替で破棄しない、§10.0）。
#[derive(Debug, Default)]
pub struct State {
    /// 見ているレベル。まだ一度も開いていなければ `None`。
    level: Option<Level>,
    /// 選んでいるブロックの ID。
    selected: Option<SceneId>,
    blocks: Option<(u64, Level, Vec<OutlineBlock>)>,
    chapters: Option<(u64, Vec<(u16, String)>)>,
    foreshadows: Option<(u64, Vec<Foreshadow>)>,
    parent: Option<(u64, Level, Option<Parent>)>,
    editor: slice_editor::Cache,
    reveal: bool,
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
            }
            self.blocks = Some((bench.generation(), level, blocks));
        }
        self.blocks
            .as_ref()
            .map_or(&[], |(_, _, blocks)| blocks.as_slice())
    }

    fn chapters(&mut self, bench: &Workbench) -> &[(u16, String)] {
        let fresh =
            matches!(&self.chapters, Some((generation, _)) if *generation == bench.generation());
        if !fresh {
            let titles = plots::outline_blocks(bench.project(), None);
            let list = plots::chapters(bench.project())
                .into_iter()
                .map(|chapter| {
                    let title = titles
                        .iter()
                        .find(|block| block.id.chapter == chapter && block.id.episode == 0)
                        .map(|block| block.title.clone())
                        .unwrap_or_default();
                    (chapter, title)
                })
                .collect();
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

    /// 文書のパス（全体 `plots/all.txt`、章 `plots/AA.txt`）。
    fn path(&self, bench: &Workbench) -> Option<String> {
        match self.level? {
            None => Some("plots/all.txt".to_owned()),
            Some(chapter) => Some(bench.project().chapter_plot_path(chapter)),
        }
    }

    /// 選んだブロックをエディタ（Slice）に開く。
    pub fn sync(&mut self, bench: &mut Workbench) {
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

    /// 構造移動（`Alt+↑/↓`・`Alt+←/→` = 前 / 次のブロック、`Ctrl+Alt+←/→` = 前 / 次のレベル、
    /// `Ctrl+Home/End`）。
    pub fn nav(&mut self, bench: &mut Workbench, nav: Nav) -> bool {
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
        levels.extend(
            self.chapters(bench)
                .iter()
                .map(|(chapter, _)| Some(*chapter)),
        );
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
    let mut request = None;
    // 左: レベル切替
    egui::SidePanel::left("outline-levels")
        .resizable(false)
        .exact_width(200.0 * scale.min(1.2))
        .frame(
            Frame::none()
                .fill(colors.bg_base)
                .inner_margin(Margin::symmetric(8.0, 8.0)),
        )
        .show(ctx, |ui| {
            let current = state.level.unwrap_or(None);
            let mut chosen: Option<Level> = None;
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
            for (chapter, title) in chapters {
                let label = plots::short_id(
                    SceneId {
                        chapter,
                        episode: 0,
                        scene: 0,
                        section: 0,
                    },
                    digits,
                );
                if list_row(
                    ui,
                    view.mode,
                    scale,
                    Row {
                        selected: current == Some(chapter),
                        label: &label,
                        text: &title,
                        ..Row::default()
                    },
                )
                .clicked()
                {
                    chosen = Some(Some(chapter));
                }
            }
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
        });
    // 右: 伏線
    if let Some((side, width)) = view.reference {
        let panel = match side {
            PaneSide::Right => egui::SidePanel::right("reference"),
            PaneSide::Left => egui::SidePanel::left("reference"),
        };
        panel
            .resizable(false)
            .exact_width(width)
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
                let output = slice_editor::show(ui, bench, &mut state.editor, view.mode, scale);
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
    let reveal = state.reveal;
    let blocks = state.blocks(bench).to_vec();
    if blocks.is_empty() {
        let path = state.path(bench).unwrap_or_default();
        super::hint(
            ui,
            view.mode,
            view.scale,
            &i18n::format(view.locale, "outline.no_blocks", &[&path]),
        );
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
