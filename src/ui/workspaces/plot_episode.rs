//! `plot-episode` — 話プロット（§10.2）。
//!
//! ```text
//! ┌──────────────────────────────┬────────────────────┐
//! │ 上: 章プロットの該当話ブロック │ 参照ペイン          │
//! │    （読み取り専用）           │  本文の有無・配役と  │
//! ├──────────────────────────────┤  モード・用語・資料  │
//! │ 中: 節リスト（1 行 1 節。左端 │  ・診断             │
//! │    3px の縦線が状態）         │                    │
//! ├──────────────────────────────┤                    │
//! │ 下: 選んだ行のエディタ         │                    │
//! └──────────────────────────────┴────────────────────┘
//! ```
//!
//! - 上段は**常に親（章）プロット**（指示 11 をレイアウトで保証する。§19.5・§19.11）。
//! - 節リストの行は場面見出し（場面レベルのフィールドを編集する）と節（節の内容を編集する）。
//!   `Alt+↑/↓` で前 / 次の行、`Alt+←/→` で前 / 次の場面、`Ctrl+Alt+←/→` で前 / 次の話。
//! - 下段は `Workbench` の Slice（B-126）。書き戻しと保存は節エディタと同じ規則（§8.1・§8.2）。
//! - 右端の台詞比率バーと `when:` の表示は ②（`PLANS.md` §5.9）。

use std::ops::Range;

use egui::{Align, Context, FontFamily, FontId, Frame, Margin, RichText, Ui, Vec2};

use crate::core::export::State as PlotState;
use crate::core::features::plots::{self, EpisodeRow, OutlineBlock};
use crate::core::features::project::line_starts_of;
use crate::core::features::reference::{self, Reference};
use crate::core::features::sections;
use crate::core::id::SceneId;
use crate::ui::i18n;
use crate::ui::settings::PaneSide;
use crate::ui::theme;
use crate::ui::widgets::context_view;
use crate::ui::widgets::reference_pane::{self, Chosen};
use crate::ui::widgets::slice_editor;
use crate::ui::widgets::syntax_text::{self, Highlight, Metrics};
use crate::ui::workbench::{Nav, Workbench};

use super::{list_row, state_color, Request, Row, View};

/// 上段の中身（章プロットの話ブロック・その文字列・色分け）。
type Upper = (OutlineBlock, String, Highlight);

/// 参照ペインの中身（世代・節・中身・本文の文字数）。
type Pane = (u64, SceneId, Reference, Option<usize>);

/// `plot-episode` の状態（切替で破棄しない、§10.0）。
#[derive(Debug, Default)]
pub struct State {
    /// 見ている話（章, 話）。まだ一度も開いていなければ `None`。
    episode: Option<(u16, u16)>,
    /// 選んでいる行の ID（場面見出しの行は `section == 0`）。
    selected: Option<SceneId>,
    /// 節リスト（文書の世代・話が同じあいだは作り直さない）。
    rows: Option<(u64, (u16, u16), Vec<EpisodeRow>)>,
    /// 上段（章プロットの話ブロック）。
    block: Option<(u64, (u16, u16), Option<Upper>)>,
    /// 参照ペインの中身。
    reference: Option<Pane>,
    editor: slice_editor::Cache,
    /// 次のフレームで選んだ行までスクロールし、エディタにフォーカスを移す。
    reveal: bool,
    /// List highlight moved without opening/committing its text slice.
    preview: bool,
    /// Scroll the list once after a keyboard-only selection change.
    scroll_list: bool,
}

impl State {
    /// 見ている話。
    pub fn episode(&self) -> Option<(u16, u16)> {
        self.episode
    }

    /// 選んでいる行の ID。
    pub fn selected(&self) -> Option<SceneId> {
        self.selected
    }

    /// `id` の節（無ければ同じ場面の見出し）に着地する（初めて開くとき・`Ctrl+B`）。
    pub fn land(&mut self, bench: &Workbench, id: SceneId) {
        self.episode = Some((id.chapter, id.episode));
        let rows = self.rows(bench);
        self.selected = plots::row_near(rows, id)
            .and_then(|index| rows.get(index))
            .map(|row| row.id)
            .or(Some(id));
        self.preview = false;
        self.reveal = true;
    }

    /// 話 `chapter`-`episode` の最初の行に着地する（`plot-outline` から降りたとき）。
    pub fn land_episode(&mut self, bench: &Workbench, chapter: u16, episode: u16) {
        self.episode = Some((chapter, episode));
        let rows = self.rows(bench);
        self.selected = rows.first().map(|row| row.id);
        self.preview = false;
        self.reveal = true;
    }

    /// 節リスト（必要なら作り直す）。
    pub fn rows(&mut self, bench: &Workbench) -> &[EpisodeRow] {
        let Some(episode) = self.episode else {
            return &[];
        };
        let fresh = matches!(&self.rows, Some((generation, at, _)) if *generation == bench.generation() && *at == episode);
        if !fresh {
            let rows = plots::episode_rows(bench.project(), episode.0, episode.1);
            self.rows = Some((bench.generation(), episode, rows));
        }
        self.rows
            .as_ref()
            .map_or(&[], |(_, _, rows)| rows.as_slice())
    }

    /// 選んでいる行の位置（選んだ ID が無くなっていれば近い行）。
    fn selected_index(&mut self, bench: &Workbench) -> Option<usize> {
        let selected = self.selected;
        let rows = self.rows(bench);
        let wanted = selected?;
        plots::row_near(rows, wanted)
    }

    /// 選んでいる行（複製）。
    pub fn current_row(&mut self, bench: &Workbench) -> Option<EpisodeRow> {
        let index = self.selected_index(bench)?;
        self.rows(bench).get(index).cloned()
    }

    /// 話プロットのパス。
    fn path(&self, bench: &Workbench) -> Option<String> {
        let (chapter, episode) = self.episode?;
        Some(bench.project().episode_plot_path(chapter, episode))
    }

    /// 選んだ行をエディタ（Slice）に開く。行が無ければ Slice を閉じる。
    pub fn sync(&mut self, bench: &mut Workbench) {
        if self.preview {
            return;
        }
        let Some(path) = self.path(bench) else {
            bench.close_slice();
            return;
        };
        match self.current_row(bench) {
            Some(row) => {
                let digits = bench.project().digits();
                let tag = format!("plot-episode:{path}:{}", row.id.display(digits));
                bench.open_slice(&path, row.lines.clone(), &tag);
                self.selected = Some(row.id);
            }
            None => bench.close_slice(),
        }
    }

    /// Highlight the adjacent plot row without committing/changing the Slice editor.
    pub fn preview_row(&mut self, bench: &Workbench, forward: bool) -> bool {
        let at = self.selected_index(bench);
        let rows = self.rows(bench);
        let next = if forward {
            at.map_or(Some(0), |index| {
                (index + 1 < rows.len()).then_some(index + 1)
            })
        } else {
            at.and_then(|index| index.checked_sub(1))
        };
        let Some(id) = next.and_then(|index| rows.get(index)).map(|row| row.id) else {
            return false;
        };
        self.selected = Some(id);
        self.preview = true;
        self.scroll_list = true;
        self.reveal = false;
        true
    }

    /// 構造移動（`Alt+↑/↓/←/→`・`Ctrl+Alt+←/→`・`Ctrl+Home/End`）。動けなければ `false`。
    pub fn nav(&mut self, bench: &mut Workbench, nav: Nav) -> bool {
        self.preview = false;
        bench.commit();
        let index = self.selected_index(bench);
        let rows = self.rows(bench).to_vec();
        let target = match nav {
            Nav::PrevSection => index.and_then(|at| at.checked_sub(1)),
            Nav::NextSection => match index {
                Some(at) => (at + 1 < rows.len()).then_some(at + 1),
                None => (!rows.is_empty()).then_some(0),
            },
            Nav::PrevScene => index.and_then(|at| {
                // 今の場面の見出し → そのさらに前の場面の見出し
                let head = (0..=at).rev().find(|found| rows[*found].is_scene())?;
                if head < at {
                    Some(head)
                } else {
                    (0..head).rev().find(|found| rows[*found].is_scene())
                }
            }),
            Nav::NextScene => {
                let from = index.map_or(0, |at| at + 1);
                (from..rows.len()).find(|found| rows[*found].is_scene())
            }
            Nav::FileStart => (!rows.is_empty()).then_some(0),
            Nav::FileEnd => rows.len().checked_sub(1),
            Nav::PrevFile | Nav::NextFile => {
                return self.step_episode(bench, nav == Nav::NextFile);
            }
        };
        match target {
            Some(found) if Some(found) != index => {
                self.selected = Some(rows[found].id);
                self.reveal = true;
                bench.save.request_now();
                true
            }
            _ => false,
        }
    }

    /// 前 / 次の話へ（話の並びは `plots::all_episodes`）。
    fn step_episode(&mut self, bench: &mut Workbench, forward: bool) -> bool {
        let Some(current) = self.episode else {
            return false;
        };
        let all = plots::all_episodes(bench.project());
        let next = if forward {
            all.iter().find(|found| **found > current)
        } else {
            all.iter().rev().find(|found| **found < current)
        };
        match next.copied() {
            Some((chapter, episode)) => {
                self.land_episode(bench, chapter, episode);
                bench.save.request_now();
                true
            }
            None => false,
        }
    }

    /// 節を挿入して選ぶ（§10.2 `Ctrl+Shift+N`）。話プロットが無ければ見出しを書いて作る。
    pub fn insert(&mut self, bench: &mut Workbench, scene: SceneId, now: f64) -> bool {
        let Some(path) = self.path(bench) else {
            return false;
        };
        self.preview = false;
        bench.commit();
        let index = self.selected_index(bench);
        let rows = self.rows(bench).to_vec();
        let digits = bench.project().digits();
        let (chapter, episode) = self.episode.unwrap_or((scene.chapter, scene.episode));
        let fallback = SceneId {
            chapter,
            episode,
            scene: if scene.chapter == chapter && scene.episode == episode {
                scene.scene.max(1)
            } else {
                1
            },
            section: 0,
        };
        let text = bench.project().text(&path).to_owned();
        let (updated, at) = plots::insert_section(&text, &rows, index, fallback, digits);
        bench.replace_document(&path, updated, now);
        let rows = self.rows(bench);
        let found = rows
            .iter()
            .find(|row| !row.is_scene() && row.lines.start == at)
            .or_else(|| rows.iter().rev().find(|row| !row.is_scene()))
            .map(|row| row.id);
        self.selected = found;
        self.reveal = true;
        true
    }

    /// パンくず（`01›04›02›03`）。
    pub fn breadcrumb(&self, bench: &Workbench) -> Option<String> {
        let digits = bench.project().digits();
        let id = self.selected.or_else(|| {
            self.episode.map(|(chapter, episode)| SceneId {
                chapter,
                episode,
                scene: 0,
                section: 0,
            })
        })?;
        let text = id.display(digits);
        let parts: Vec<&str> = text.split('-').collect();
        // 0 の階層（場面の行・話だけ）は出さない
        let depth = if id.section > 0 {
            4
        } else if id.scene > 0 {
            3
        } else {
            2
        };
        Some(parts[..depth.min(parts.len())].join("›"))
    }

    fn block(&mut self, bench: &Workbench) -> Option<&Upper> {
        let episode = self.episode?;
        let fresh = matches!(&self.block, Some((generation, at, _)) if *generation == bench.generation() && *at == episode);
        if !fresh {
            let found =
                plots::outline_block(bench.project(), episode.0, episode.1).and_then(|block| {
                    let document = bench.project().document(&block.path)?;
                    let starts = line_starts_of(&document.text);
                    let range =
                        context_view::byte_range(&starts, document.text.len(), &block.lines);
                    let text = document.text.get(range.clone())?.to_owned();
                    let highlight = Highlight::of_range(document, range, bench.project());
                    Some((block, text, highlight))
                });
            self.block = Some((bench.generation(), episode, found));
        }
        self.block.as_ref().and_then(|(_, _, found)| found.as_ref())
    }

    fn reference(&mut self, bench: &Workbench) -> Option<&Pane> {
        let row = self.current_row(bench)?;
        let fresh = matches!(&self.reference, Some((generation, id, _, _)) if *generation == bench.generation() && *id == row.id);
        if !fresh {
            let path = self.path(bench)?;
            let document = bench.project().document(&path)?;
            let mut content = reference::at(document, row.lines.clone(), row.id, bench.project());
            // 1. プロットはエディタに出ている
            content.plot.clear();
            let prose = prose_chars(bench, row.id);
            self.reference = Some((bench.generation(), row.id, content, prose));
        }
        self.reference.as_ref()
    }
}

/// この節（場面の行なら場面）の本文の文字数（空白除外）。本文が無ければ `None`。
fn prose_chars(bench: &Workbench, id: SceneId) -> Option<usize> {
    let path = bench.project().contents_path(id.chapter, id.episode);
    let document = bench.project().document(&path)?;
    let mut found = false;
    let mut total = 0;
    for section in bench.sections() {
        let same = section.path == path
            && section.id.scene == id.scene
            && (id.section == 0 || section.id.section == id.section);
        if same {
            found = true;
            total += sections::count_chars(&sections::text_of(&document.text, section));
        }
    }
    found.then_some(total)
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
                egui::ScrollArea::vertical()
                    .id_source("episode-reference")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some((_, _, content, prose)) = state.reference(bench) {
                            reference_pane::heading(
                                ui,
                                i18n::text(locale, "pane.prose"),
                                view.mode,
                                scale,
                            );
                            let text = match prose {
                                Some(chars) => i18n::format(
                                    locale,
                                    "episode.prose_chars",
                                    &[&context_view::group(*chars)],
                                ),
                                None => i18n::text(locale, "episode.no_prose").to_owned(),
                            };
                            ui.label(
                                RichText::new(text)
                                    .font(FontId::new(13.0 * scale, FontFamily::Proportional))
                                    .color(colors.fg_primary),
                            );
                            let content = content.clone();
                            match reference_pane::sections(ui, &content, locale, view.mode, scale) {
                                Some(Chosen::Material(id)) => {
                                    request = Some(Request::AppendMaterial(id));
                                }
                                Some(Chosen::OpenMaterial(id)) => {
                                    request = Some(Request::OpenMaterial(id));
                                }
                                Some(Chosen::Register { kind, name }) => {
                                    request = Some(Request::Register { kind, name });
                                }
                                None => {}
                            }
                        }
                    });
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
            // 上段: 章プロットの話ブロック（読み取り専用）
            let top = (total * 0.24).max(60.0);
            ui.allocate_ui(Vec2::new(ui.available_width(), top), |ui| {
                ui.set_min_height(top);
                upper(ui, state, bench, view);
            });
            super::separator(ui, view.mode);
            // 中段: 節リスト
            let middle = (total * 0.34).max(80.0);
            ui.allocate_ui(Vec2::new(ui.available_width(), middle), |ui| {
                ui.set_min_height(middle);
                if let Some(found) = list(ui, state, bench, view) {
                    request = Some(found);
                }
            });
            super::separator(ui, view.mode);
            // 下段: エディタ
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
                super::hint(ui, view.mode, scale, i18n::text(locale, "episode.empty"));
            }
            state.reveal = false;
        });
    request
}

/// 上段: 章プロットの該当話ブロック。
fn upper(ui: &mut Ui, state: &mut State, bench: &Workbench, view: &View) {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let digits = bench.project().digits();
    let episode = state.episode;
    egui::ScrollArea::vertical()
        .id_source("episode-upper")
        .auto_shrink([false, false])
        .show(ui, |ui| match state.block(bench) {
            Some((_, text, highlight)) => {
                let metrics = Metrics {
                    size: 13.0 * scale,
                    line_height: 1.6,
                };
                let job =
                    syntax_text::job(text, highlight, view.mode, metrics, ui.available_width());
                ui.label(job);
            }
            None => {
                let (chapter, number) = episode.unwrap_or((0, 0));
                let id = SceneId {
                    chapter,
                    episode: number,
                    scene: 0,
                    section: 0,
                };
                let path = bench.project().chapter_plot_path(chapter);
                let shown = id.display(digits);
                let shown = shown.split('-').take(2).collect::<Vec<_>>().join("-");
                ui.label(
                    RichText::new(i18n::format(
                        view.locale,
                        "episode.no_block",
                        &[&path, &shown],
                    ))
                    .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                    .color(colors.fg_faint),
                );
            }
        });
}

/// 中段: 節リスト。ダブルクリックで下の段（write）へ。
fn list(ui: &mut Ui, state: &mut State, bench: &Workbench, view: &View) -> Option<Request> {
    let digits = bench.project().digits();
    let selected = state.selected_index(bench);
    let reveal = state.reveal || std::mem::take(&mut state.scroll_list);
    let rows = state.rows(bench).to_vec();
    let mut request = None;
    let mut clicked = None;
    if rows.is_empty() {
        let path = state.path(bench).unwrap_or_default();
        super::hint(
            ui,
            view.mode,
            view.scale,
            &i18n::format(view.locale, "episode.no_plot", &[&path]),
        );
        if ui
            .button(i18n::text(view.locale, "episode.new_button"))
            .clicked()
        {
            return Some(Request::Shortcut(crate::ui::shell::Command::InsertSection));
        }
        return None;
    }
    egui::ScrollArea::vertical()
        .id_source("episode-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, row) in rows.iter().enumerate() {
                let is_selected = Some(index) == selected;
                let response = if row.is_scene() {
                    let id = row.id.display(digits);
                    let id: Vec<&str> = id.split('-').take(3).collect();
                    let text = format!("@{} {}", id.join("-"), row.summary);
                    list_row(
                        ui,
                        view.mode,
                        view.scale,
                        Row {
                            selected: is_selected,
                            text: &text,
                            muted: true,
                            ..Row::default()
                        },
                    )
                } else {
                    let number = format!("{:02}", row.id.section);
                    list_row(
                        ui,
                        view.mode,
                        view.scale,
                        Row {
                            selected: is_selected,
                            bar: Some(state_color(view.mode, row.state.or(Some(PlotState::Draft)))),
                            label: &number,
                            text: &row.summary,
                            muted: false,
                        },
                    )
                };
                if is_selected && reveal {
                    response.scroll_to_me(Some(Align::Center));
                }
                if response.double_clicked() {
                    request = Some(Request::Down);
                }
                if response.clicked() {
                    clicked = Some(row.id);
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

/// 選んでいる行の範囲（テスト用の読み口）。
pub fn selected_lines(state: &mut State, bench: &Workbench) -> Option<Range<usize>> {
    state.current_row(bench).map(|row| row.lines)
}
