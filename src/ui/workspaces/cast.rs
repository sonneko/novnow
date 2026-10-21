//! `cast` — 配役・モード・役割語（§10.6）。
//!
//! - 左: キャラ一覧（`reading` の順）。中央: `characters.md` のそのキャラの部分のエディタ（Slice）。
//! - 右: タブ（`Ctrl+Shift+→` / `←`。cast の中だけ、B-126）。
//!   - **モード**: モードタイムライン（横軸 = 章、縦軸 = モード、`range` を帯で）。帯をクリックして
//!     タイムラインにフォーカスを移し、`↑/↓` で帯を選び `Shift+←/→` で 1 章ずらす（ドラッグも同じ）。
//!     `range:` が書き換わる。
//!   - **役割語**: 役割語テスター。台詞を打つと、どのパターンが一致するか / しないかと、§6.5.1 の
//!     正規表現を出す。一致した部分は本文と同じ緑の下線。
//!   - **未一致**: `Ctrl+Shift+V` でプロジェクト全体の台詞を走査し、役割語が一致しなかった台詞を一覧
//!     （`Enter` / ダブルクリックで本文のその台詞へ）。
//! - 呼称タブ（§10.6.1）は ②。

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Context, FontFamily, FontId, Frame, Key, Margin, Modifiers, Pos2, Rect, RichText, Sense,
    Stroke, Ui, Vec2,
};

use crate::core::features::cast::{self, CastRow, TesterResult, Timeline, Unmatched};
use crate::ui::i18n;
use crate::ui::settings::PaneSide;
use crate::ui::theme;
use crate::ui::widgets::slice_editor;
use crate::ui::workbench::{Nav, Workbench};

use super::{list_row, Request, Row, View};

/// 右のタブ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// モードタイムライン（既定）。
    #[default]
    Modes,
    /// 役割語テスター。
    Tester,
    /// 役割語が一致しなかった台詞（`Ctrl+Shift+V`）。
    Unmatched,
}

/// 飛び先（未一致の台詞）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jump {
    /// 本文のパス。
    pub path: String,
    /// byte 位置。
    pub byte: usize,
}

/// `cast` の状態（切替で破棄しない、§10.0）。
#[derive(Debug, Default)]
pub struct State {
    /// 選んでいるキャラ（`characters.md` の H1 の順番）。
    selected: Option<usize>,
    /// 右のタブ。
    pub tab: Tab,
    /// タイムラインで選んでいる帯。
    band: usize,
    /// テスターの入力。
    pub utterance: String,
    rows: Option<(u64, Vec<CastRow>)>,
    timeline: Option<(u64, usize, Timeline)>,
    tester: Option<(u64, usize, String, TesterResult)>,
    unmatched: Option<(u64, Vec<Unmatched>)>,
    unmatched_selected: usize,
    editor: slice_editor::Cache,
    reveal: bool,
    /// Browse the cast list without replacing the open editor Slice.
    preview: bool,
    /// Scroll the list once after a keyboard-only selection change.
    scroll_list: bool,
    /// ドラッグ中の帯と、ずらした量（px）。
    drag: Option<(usize, f32)>,
    /// キーでタブを切り替えた直後（タブの中身にフォーカスを移す）。
    focus_tab: bool,
}

impl State {
    /// 選んでいるキャラ（H1 の順番）。
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// キャラ一覧（必要なら作り直す）。
    pub fn rows(&mut self, bench: &Workbench) -> &[CastRow] {
        let fresh =
            matches!(&self.rows, Some((generation, _)) if *generation == bench.generation());
        if !fresh {
            self.rows = Some((bench.generation(), cast::characters(bench.project())));
        }
        self.rows.as_ref().map_or(&[], |(_, rows)| rows.as_slice())
    }

    fn current(&mut self, bench: &Workbench) -> Option<CastRow> {
        let selected = self.selected;
        let rows = self.rows(bench);
        selected
            .and_then(|index| rows.iter().find(|row| row.index == index))
            .or(rows.first())
            .cloned()
    }

    /// H1 の順番 `index` のキャラを選ぶ（パレットの `'名前`、§9.7.1）。
    pub fn select(&mut self, index: usize) {
        self.selected = Some(index);
        self.band = 0;
        self.preview = false;
        self.reveal = true;
    }

    /// `characters.md` の行 `line` を含むキャラを選ぶ（点検からのジャンプ）。
    pub fn select_line(&mut self, bench: &Workbench, line: usize) {
        // 行を含むキャラ（H1 の前の行なら最初のキャラ）
        let found = self
            .rows(bench)
            .iter()
            .filter(|row| row.lines.start <= line)
            .max_by_key(|row| row.lines.start)
            .map(|row| row.index);
        if let Some(index) = found {
            self.selected = Some(index);
            self.band = 0;
            self.preview = false;
            self.reveal = true;
        }
    }

    /// 選んだキャラをエディタ（Slice）に開く。
    pub fn sync(&mut self, bench: &mut Workbench) {
        if self.preview {
            return;
        }
        match self.current(bench) {
            Some(row) => {
                let tag = format!("cast:{}:{}", row.index, row.name);
                bench.open_slice(cast::PATH, row.lines.clone(), &tag);
                self.selected = Some(row.index);
            }
            None => bench.close_slice(),
        }
    }

    /// Browse the adjacent character without committing the editor.
    pub fn preview_row(&mut self, bench: &Workbench, forward: bool) -> bool {
        let selected = self.selected;
        let rows = self.rows(bench);
        let at = selected.and_then(|index| rows.iter().position(|row| row.index == index));
        let next = if forward {
            at.map_or(Some(0), |index| {
                (index + 1 < rows.len()).then_some(index + 1)
            })
        } else {
            at.and_then(|index| index.checked_sub(1))
        };
        let Some(index) = next.and_then(|index| rows.get(index)).map(|row| row.index) else {
            return false;
        };
        self.selected = Some(index);
        self.preview = true;
        self.scroll_list = true;
        self.reveal = false;
        true
    }

    /// 前 / 次のキャラ（`Alt+↑/↓`）。
    pub fn nav(&mut self, bench: &mut Workbench, nav: Nav) -> bool {
        self.preview = false;
        bench.commit();
        let rows = self.rows(bench).to_vec();
        let at = self
            .selected
            .and_then(|index| rows.iter().position(|row| row.index == index));
        let target = match nav {
            Nav::PrevSection | Nav::PrevScene => at.and_then(|found| found.checked_sub(1)),
            Nav::NextSection | Nav::NextScene => match at {
                Some(found) => (found + 1 < rows.len()).then_some(found + 1),
                None => (!rows.is_empty()).then_some(0),
            },
            Nav::FileStart => (!rows.is_empty()).then_some(0),
            Nav::FileEnd => rows.len().checked_sub(1),
            Nav::PrevFile | Nav::NextFile => None,
        };
        match target {
            Some(found) if Some(found) != at => {
                self.selected = rows.get(found).map(|row| row.index);
                self.band = 0;
                self.reveal = true;
                bench.save.request_now();
                true
            }
            _ => false,
        }
    }

    /// 次のタブ（`Ctrl+Shift+→`）・前のタブ（`←`）。
    pub fn step_tab(&mut self, forward: bool) {
        let all = [Tab::Modes, Tab::Tester, Tab::Unmatched];
        let at = all.iter().position(|tab| *tab == self.tab).unwrap_or(0);
        let next = if forward {
            (at + 1) % all.len()
        } else {
            (at + all.len() - 1) % all.len()
        };
        self.tab = all[next];
        self.focus_tab = true;
    }

    /// `Ctrl+Shift+V`: 未一致の台詞を数え直してタブを開く。
    pub fn scan(&mut self) {
        self.unmatched = None;
        self.unmatched_selected = 0;
        self.tab = Tab::Unmatched;
    }

    /// タイムライン。
    pub fn timeline(&mut self, bench: &Workbench) -> Option<&Timeline> {
        let index = self.selected?;
        let fresh = matches!(&self.timeline, Some((generation, at, _)) if *generation == bench.generation() && *at == index);
        if !fresh {
            self.timeline = Some((
                bench.generation(),
                index,
                cast::timeline(bench.project(), index),
            ));
        }
        self.timeline.as_ref().map(|(_, _, found)| found)
    }

    /// テスターの結果。
    pub fn tester(&mut self, bench: &Workbench) -> Option<&TesterResult> {
        let index = self.selected?;
        let fresh = matches!(&self.tester, Some((generation, at, text, _)) if *generation == bench.generation() && *at == index && *text == self.utterance);
        if !fresh {
            let result = cast::test_utterance(bench.project(), index, &self.utterance);
            self.tester = Some((bench.generation(), index, self.utterance.clone(), result));
        }
        self.tester.as_ref().map(|(_, _, _, found)| found)
    }

    /// 未一致の台詞。
    pub fn unmatched(&mut self, bench: &Workbench) -> &[Unmatched] {
        let fresh =
            matches!(&self.unmatched, Some((generation, _)) if *generation == bench.generation());
        if !fresh {
            self.unmatched = Some((
                bench.generation(),
                cast::unmatched_dialogue(bench.project()),
            ));
        }
        self.unmatched
            .as_ref()
            .map_or(&[], |(_, list)| list.as_slice())
    }

    /// 選んでいる帯を `delta` 章ずらす（`range:` を書き換える）。
    pub fn shift_band(&mut self, bench: &mut Workbench, delta: i32, now: f64) -> bool {
        let Some(index) = self.selected else {
            return false;
        };
        bench.commit();
        let text = bench.project().text(cast::PATH).to_owned();
        match cast::shift_band(&text, index, self.band, delta) {
            Some(updated) => bench.replace_document(cast::PATH, updated, now),
            None => false,
        }
    }
}

/// 1 フレーム。未一致の台詞へ飛ぶなら返す。
pub fn show(
    ctx: &Context,
    state: &mut State,
    bench: &mut Workbench,
    view: &View,
) -> (Option<Request>, Option<Jump>) {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let locale = view.locale;
    state.sync(bench);
    // The right-hand timeline owns unmodified arrows while focused.
    let timeline_focus = state.tab == Tab::Modes
        && (state.focus_tab
            || ctx.memory(|memory| memory.has_focus(egui::Id::new("cast-timeline"))));
    if !timeline_focus {
        if let Some(forward) = super::browse_keys(ctx, view) {
            state.preview_row(bench, forward);
        }
    }
    let mut jump = None;
    let mut request = None;
    super::fixed_width(
        egui::SidePanel::left("cast-list"),
        200.0 * scale.min(1.2),
        8.0,
    )
    .frame(
        Frame::none()
            .fill(colors.bg_base)
            .inner_margin(Margin::symmetric(8.0, 8.0)),
    )
    .show(ctx, |ui| {
        let rows = state.rows(bench).to_vec();
        if rows.is_empty() {
            super::hint(ui, view.mode, scale, i18n::text(locale, "cast.none"));
            return;
        }
        let mut clicked = None;
        let scroll_list = std::mem::take(&mut state.scroll_list);
        egui::ScrollArea::vertical()
            .id_source("cast-list-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in &rows {
                    crate::ui::widgets::portrait::character(
                        ui,
                        bench.project(),
                        &row.name,
                        32.0 * scale,
                    );
                    let response = list_row(
                        ui,
                        view.mode,
                        scale,
                        Row {
                            selected: state.selected == Some(row.index),
                            text: &row.name,
                            label: "",
                            ..Row::default()
                        },
                    );
                    if scroll_list && state.selected == Some(row.index) {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if response.clicked() {
                        clicked = Some(row.index);
                    }
                }
            });
        if let Some(index) = clicked {
            state.selected = Some(index);
            state.band = 0;
            state.preview = false;
            state.reveal = true;
        }
    });
    if let Some((side, width)) = view.reference {
        let panel = match side {
            PaneSide::Right => egui::SidePanel::right("reference"),
            PaneSide::Left => egui::SidePanel::left("reference"),
        };
        super::fixed_width(panel, width, 14.0)
            .frame(
                Frame::none()
                    .fill(colors.bg_base)
                    .inner_margin(Margin::symmetric(14.0, 8.0)),
            )
            .show(ctx, |ui| {
                tabs(ui, state, view);
                super::separator(ui, view.mode);
                match state.tab {
                    Tab::Modes => modes_tab(ui, state, bench, view),
                    Tab::Tester => tester_tab(ui, state, bench, view),
                    Tab::Unmatched => {
                        if let Some(found) = unmatched_tab(ui, state, bench, view) {
                            jump = Some(found);
                        }
                    }
                }
            });
    }
    egui::CentralPanel::default()
        .frame(
            Frame::none()
                .fill(colors.bg_base)
                .inner_margin(Margin::symmetric(16.0, 8.0)),
        )
        .show(ctx, |ui| {
            if super::action_button(ui, i18n::text(locale, "cast.scan_button")).clicked() {
                request = Some(Request::Shortcut(crate::ui::shell::Command::Scan));
            }
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
                super::hint(ui, view.mode, scale, i18n::text(locale, "cast.empty"));
            }
            state.reveal = false;
        });
    (request, jump)
}

fn tabs(ui: &mut Ui, state: &mut State, view: &View) {
    let colors = theme::palette(view.mode);
    ui.horizontal(|ui| {
        for (tab, key) in [
            (Tab::Modes, "cast.tab_modes"),
            (Tab::Tester, "cast.tab_tester"),
            (Tab::Unmatched, "cast.tab_unmatched"),
        ] {
            let selected = state.tab == tab;
            let text = RichText::new(i18n::text(view.locale, key))
                .font(FontId::new(13.0 * view.scale, FontFamily::Proportional))
                .color(if selected {
                    colors.accent
                } else {
                    colors.fg_muted
                });
            if super::selectable_action(ui, selected, text).clicked() {
                state.tab = tab;
            }
        }
    });
}

/// モードタイムライン（横軸 = 章、縦軸 = モード）。
fn modes_tab(ui: &mut Ui, state: &mut State, bench: &mut Workbench, view: &View) {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let Some(timeline) = state.timeline(bench).cloned() else {
        super::hint(ui, view.mode, scale, i18n::text(view.locale, "cast.empty"));
        return;
    };
    if timeline.bands.is_empty() {
        super::hint(
            ui,
            view.mode,
            scale,
            i18n::text(view.locale, "cast.no_modes"),
        );
        return;
    }
    let focus_id = egui::Id::new("cast-timeline");
    if std::mem::take(&mut state.focus_tab) {
        ui.memory_mut(|memory| memory.request_focus(focus_id));
    }
    let focused = ui.memory(|memory| memory.has_focus(focus_id));
    // フォーカス中だけ: ↑/↓ で帯を選び、Shift+←/→ で 1 章ずらす（編集欄の選択と重ねない）
    if focused {
        let (up, down, left, right) = ui.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                input.consume_key(Modifiers::SHIFT, Key::ArrowLeft),
                input.consume_key(Modifiers::SHIFT, Key::ArrowRight),
            )
        });
        if up {
            state.band = state.band.saturating_sub(1);
        }
        if down && state.band + 1 < timeline.bands.len() {
            state.band += 1;
        }
        if left {
            state.shift_band(bench, -1, view.now);
        }
        if right {
            state.shift_band(bench, 1, view.now);
        }
    }
    let label_width = 96.0 * scale;
    let row_height = (22.0 * scale).max(24.0);
    let columns = timeline.chapters.len().max(1);
    let width = ui.available_width();
    let column = ((width - label_width) / columns as f32).max(18.0);
    let height = row_height * (timeline.bands.len() as f32 + 1.0) + 8.0;
    // キー操作を受けるウィジェットは、フォーカスの Id で作る（毎フレーム同じ Id でないとフォーカスが外れる）
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let response = ui.interact(rect, focus_id, Sense::click_and_drag());
    ui.memory_mut(|memory| {
        memory.interested_in_focus(focus_id);
        // 矢印は帯の操作に使う（egui の矢印でのフォーカス移動に取られないように）
        memory.set_focus_lock_filter(
            focus_id,
            egui::EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                ..Default::default()
            },
        );
    });
    if response.clicked() || response.drag_started() {
        ui.memory_mut(|memory| memory.request_focus(focus_id));
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            i18n::text(view.locale, "cast.timeline"),
        )
    });
    let painter = ui.painter_at(rect);
    let font = FontId::new(11.0 * scale, FontFamily::Proportional);
    let mono = FontId::new(10.0 * scale, FontFamily::Monospace);
    // 見出し（章）
    for (at, chapter) in timeline.chapters.iter().enumerate() {
        let x = rect.min.x + label_width + column * at as f32;
        painter.text(
            Pos2::new(x + column / 2.0, rect.min.y + row_height / 2.0),
            egui::Align2::CENTER_CENTER,
            format!("@{chapter:02}"),
            mono.clone(),
            colors.fg_faint,
        );
    }
    // どの帯の上でクリック・ドラッグしたか
    let hover = ui
        .ctx()
        .input(|input| input.pointer.hover_pos())
        .filter(|at| rect.contains(*at));
    if response.hovered()
        && hover.is_some_and(|at| {
            at.y >= rect.min.y + row_height
                && at.y < rect.min.y + row_height * (timeline.bands.len() as f32 + 1.0)
        })
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if let Some(at) = response.interact_pointer_pos() {
        let row = ((at.y - rect.min.y) / row_height).floor() as isize - 1;
        if row >= 0 && (row as usize) < timeline.bands.len() && response.drag_started() {
            state.band = row as usize;
            state.drag = Some((row as usize, 0.0));
        } else if row >= 0 && (row as usize) < timeline.bands.len() && response.clicked() {
            state.band = row as usize;
        }
    }
    if let Some((band, moved)) = state.drag.as_mut() {
        *moved += response.drag_delta().x;
        let _ = band;
    }
    if response.drag_stopped() {
        if let Some((band, moved)) = state.drag.take() {
            let delta = (moved / column).round() as i32;
            if delta != 0 {
                state.band = band;
                state.shift_band(bench, delta, view.now);
            }
        }
    }
    for (row, band) in timeline.bands.iter().enumerate() {
        let y = rect.min.y + row_height * (row as f32 + 1.0);
        if response.hovered() && hover.is_some_and(|at| at.y >= y && at.y < y + row_height) {
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(rect.min.x, y), Vec2::new(width, row_height)),
                2.0,
                colors.bg_selection.gamma_multiply(0.6),
            );
        }
        let selected = row == state.band;
        let label = if band.label.is_empty() {
            band.id.clone()
        } else {
            format!("{} {}", band.id, band.label)
        };
        let label_rect = painter.text(
            Pos2::new(rect.min.x, y + row_height / 2.0),
            egui::Align2::LEFT_CENTER,
            label,
            font.clone(),
            if selected {
                colors.accent
            } else {
                colors.fg_primary
            },
        );
        painter.line_segment(
            [
                label_rect.left_bottom() + Vec2::Y,
                label_rect.right_bottom() + Vec2::Y,
            ],
            Stroke::new(0.6_f32, colors.border),
        );
        let offset = match state.drag {
            Some((dragged, moved)) if dragged == row => moved,
            _ => 0.0,
        };
        for (at, covered) in band.covered.iter().enumerate() {
            if !covered {
                continue;
            }
            let x = rect.min.x + label_width + column * at as f32 + offset;
            let cell = Rect::from_min_size(
                Pos2::new(x + 1.0, y + 5.0 * scale),
                Vec2::new(column - 2.0, row_height - 10.0 * scale),
            );
            painter.rect_filled(
                cell,
                2.0,
                if selected {
                    colors.accent
                } else {
                    colors.bg_selection
                },
            );
        }
        if selected && focused {
            painter.rect_stroke(
                Rect::from_min_size(
                    Pos2::new(rect.min.x + label_width, y + 2.0),
                    Vec2::new(column * columns as f32, row_height - 4.0),
                ),
                2.0,
                Stroke::new(1.0_f32, colors.accent),
            );
        }
    }
    ui.add_space(6.0);
    if let Some(band) = timeline.bands.get(state.band) {
        let range = if band.range.is_empty() {
            i18n::text(view.locale, "cast.range_all").to_owned()
        } else {
            band.range.clone()
        };
        ui.label(
            RichText::new(format!("range: {range}"))
                .font(FontId::new(12.0 * scale, FontFamily::Monospace))
                .color(colors.fg_muted),
        );
    }
    ui.label(
        RichText::new(i18n::text(view.locale, "cast.timeline_keys"))
            .font(FontId::new(11.0 * scale, FontFamily::Proportional))
            .color(colors.fg_faint),
    );
}

/// 役割語テスター。
fn tester_tab(ui: &mut Ui, state: &mut State, bench: &Workbench, view: &View) {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let input_id = egui::Id::new("cast-tester-input");
    if std::mem::take(&mut state.focus_tab) {
        ui.memory_mut(|memory| memory.request_focus(input_id));
    }
    ui.add(
        egui::TextEdit::singleline(&mut state.utterance)
            .id(input_id)
            .hint_text(i18n::text(view.locale, "cast.tester_hint"))
            .desired_width(f32::INFINITY),
    );
    let Some(result) = state.tester(bench).cloned() else {
        return;
    };
    ui.add_space(6.0);
    // プレビュー: 一致した部分に緑の下線（§11.3）
    if !state.utterance.is_empty() {
        let mut job = LayoutJob::default();
        let text = state.utterance.clone();
        let base = TextFormat {
            font_id: FontId::new(15.0 * scale, FontFamily::Proportional),
            color: colors.fg_primary,
            ..Default::default()
        };
        let mut at = 0;
        for range in &result.underlines {
            if range.start > at {
                job.append(&text[at..range.start], 0.0, base.clone());
            }
            job.append(
                &text[range.clone()],
                0.0,
                TextFormat {
                    underline: Stroke::new(2.0_f32, colors.ok),
                    ..base.clone()
                },
            );
            at = range.end;
        }
        if at < text.len() {
            job.append(&text[at..], 0.0, base);
        }
        ui.label(job);
        ui.add_space(6.0);
    }
    if result.checks.is_empty() {
        super::hint(
            ui,
            view.mode,
            scale,
            i18n::text(view.locale, "cast.no_patterns"),
        );
        return;
    }
    egui::ScrollArea::vertical()
        .id_source("cast-tester")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for check in &result.checks {
                let (mark, color) = if state.utterance.is_empty() {
                    ("·", colors.fg_faint)
                } else if check.matched {
                    ("✓", colors.ok)
                } else {
                    ("×", colors.fg_faint)
                };
                let owner = match &check.mode {
                    Some(mode) => mode.clone(),
                    None => i18n::text(view.locale, "cast.common").to_owned(),
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(mark)
                            .font(FontId::new(13.0 * scale, FontFamily::Proportional))
                            .color(color),
                    );
                    ui.label(
                        RichText::new(format!("「{}」", check.source))
                            .font(FontId::new(13.0 * scale, FontFamily::Proportional))
                            .color(colors.fg_primary),
                    );
                    ui.label(
                        RichText::new(owner)
                            .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                            .color(colors.fg_faint),
                    );
                });
                ui.label(
                    RichText::new(&check.regex)
                        .font(FontId::new(11.0 * scale, FontFamily::Monospace))
                        .color(colors.fg_muted),
                );
                ui.add_space(4.0);
            }
        });
}

/// 未一致の台詞（`Ctrl+Shift+V`）。
fn unmatched_tab(ui: &mut Ui, state: &mut State, bench: &Workbench, view: &View) -> Option<Jump> {
    let scale = view.scale;
    let digits = bench.project().digits();
    let list = state.unmatched(bench).to_vec();
    if list.is_empty() {
        super::hint(
            ui,
            view.mode,
            scale,
            i18n::text(view.locale, "cast.all_matched"),
        );
        return None;
    }
    let mut jump = None;
    ui.label(
        RichText::new(i18n::format(
            view.locale,
            "cast.unmatched_count",
            &[&list.len().to_string()],
        ))
        .font(FontId::new(11.0 * scale, FontFamily::Proportional))
        .color(theme::palette(view.mode).fg_faint),
    );
    egui::ScrollArea::vertical()
        .id_source("cast-unmatched")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, item) in list.iter().enumerate() {
                let label = crate::core::features::plots::short_id(item.scene, digits);
                let text = format!("{}「{}」", item.speaker, item.text);
                let response = list_row(
                    ui,
                    view.mode,
                    scale,
                    Row {
                        selected: index == state.unmatched_selected,
                        label: &label,
                        text: &text,
                        ..Row::default()
                    },
                );
                if response.clicked() {
                    state.unmatched_selected = index;
                }
                if response.double_clicked() {
                    jump = Some(Jump {
                        path: item.path.clone(),
                        byte: item.byte,
                    });
                }
            }
        });
    jump
}
