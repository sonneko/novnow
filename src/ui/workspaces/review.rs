//! `review` — 点検（§10.9。v1 ① は**診断と進捗の 2 タブ**。文体・統計・挿絵・資料のタブは ②）。
//!
//! - タブは `Ctrl+Shift+→` / `Ctrl+Shift+←`（review の中だけ。B-126）。
//! - 診断: プロジェクト全体の診断（コード / 重大度 / `パス:行:列` / 作家の言葉での「どこ」/ 文言。
//!   §10.9.2・第 2.11 版）。重大度の絞り込みと並べ替え、`Enter`（または `Alt+↑/↓` で選んで
//!   `Enter`・ダブルクリック）で該当箇所へ。`Ctrl+Shift+E` でいつでもここへ来て一括で数え直す。
//! - 進捗: 章 → 話 → 場面 → 節の 4 段階ツリーに状態・文字数・目標比（`core::features::plots::progress`）。
//!
//! review には編集欄が無いので、`↑` / `↓` / `Enter` も素のキーで効く（テキストにフォーカスが無いとき）。

use egui::{Context, FontFamily, FontId, Frame, Key, Margin, Modifiers, RichText, Ui};

use crate::core::diagnostics::{self, Severity};
use crate::core::features::place;
use crate::core::features::plots::{self, ProgressRow};
use crate::core::id::SceneId;
use crate::core::text::position::LineTable;
use crate::ui::i18n::{self, Locale};
use crate::ui::theme;
use crate::ui::widgets::context_view;
use crate::ui::workbench::{Nav, Workbench};

use super::{list_row, state_color, Request, Row, View};

/// タブ（§10.9.1 の 1・2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// 診断（§10.9.2）。
    #[default]
    Diagnostics,
    /// 進捗。
    Progress,
}

/// 診断の並べ替え。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// パス → 位置 → コード（`novnow check` と同じ）。
    #[default]
    Position,
    /// 重大度（E → W → I）→ パス → 位置。
    Severity,
    /// コード → パス → 位置。
    Code,
}

/// 診断の 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRow {
    /// 文書のパス。
    pub path: String,
    /// 1 始まりの（行, 列）。位置を持たない診断は `None`。
    pub position: Option<(usize, usize)>,
    /// 範囲の先頭の byte（ジャンプに使う）。
    pub byte: Option<usize>,
    /// コード（`E004`）。
    pub code: String,
    /// 重大度。
    pub severity: Severity,
    /// 作家の言葉での「どこ」（`本文 @01-04-02-03` など）。
    pub place: Option<String>,
    /// 文言。
    pub message: String,
}

/// 飛び先（`NovnowApp` が開く）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// 本文・プロット・アイデアなどの文書の byte 位置。
    Document(String, Option<usize>),
    /// 構造 ID（進捗ツリーの行）。
    Scene(SceneId),
}

/// `review` の状態（切替で破棄しない、§10.0）。
#[derive(Debug)]
pub struct State {
    /// 今のタブ。
    pub tab: Tab,
    /// 選んでいる行（タブごと）。
    selected: [usize; 2],
    /// 重大度の絞り込み（E / W / I）。
    pub shown: [bool; 3],
    /// 並べ替え。
    pub sort: Sort,
    diagnostics: Option<(u64, Locale, Vec<DiagnosticRow>)>,
    progress: Option<(u64, Vec<ProgressRow>)>,
    reveal: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::Diagnostics,
            selected: [0, 0],
            shown: [true, true, true],
            sort: Sort::Position,
            diagnostics: None,
            progress: None,
            reveal: false,
        }
    }
}

impl State {
    /// プロジェクト全体の診断（文書の世代か言語が変わったときだけ数え直す）。
    pub fn diagnostics(&mut self, bench: &Workbench, locale: Locale) -> &[DiagnosticRow] {
        let fresh = matches!(&self.diagnostics, Some((generation, at, _)) if *generation == bench.generation() && *at == locale);
        if !fresh {
            self.diagnostics = Some((bench.generation(), locale, collect(bench, locale)));
        }
        self.diagnostics
            .as_ref()
            .map_or(&[], |(_, _, rows)| rows.as_slice())
    }

    /// 最後に数えた診断（場所順。テスト用の読み口）。
    pub fn cached(&self) -> Option<&[DiagnosticRow]> {
        self.diagnostics
            .as_ref()
            .map(|(_, _, rows)| rows.as_slice())
    }

    /// 数え直す（`Ctrl+Shift+E`）。
    pub fn rerun(&mut self) {
        self.diagnostics = None;
        self.progress = None;
        self.tab = Tab::Diagnostics;
        self.reveal = true;
    }

    /// 絞り込んで並べた診断。
    pub fn visible(&mut self, bench: &Workbench, locale: Locale) -> Vec<DiagnosticRow> {
        let shown = self.shown;
        let sort = self.sort;
        let mut rows: Vec<DiagnosticRow> = self
            .diagnostics(bench, locale)
            .iter()
            .filter(|row| shown[severity_index(row.severity)])
            .cloned()
            .collect();
        match sort {
            Sort::Position => {}
            Sort::Severity => rows.sort_by_key(|row| severity_index(row.severity)),
            Sort::Code => rows.sort_by(|a, b| a.code.cmp(&b.code)),
        }
        rows
    }

    /// 進捗ツリー。
    pub fn progress(&mut self, bench: &Workbench) -> &[ProgressRow] {
        let fresh =
            matches!(&self.progress, Some((generation, _)) if *generation == bench.generation());
        if !fresh {
            self.progress = Some((bench.generation(), plots::progress(bench.project())));
        }
        self.progress
            .as_ref()
            .map_or(&[], |(_, rows)| rows.as_slice())
    }

    /// タブを切り替える（`Ctrl+Shift+→` / `←`。2 つなので行き来する）。
    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            Tab::Diagnostics => Tab::Progress,
            Tab::Progress => Tab::Diagnostics,
        };
        self.reveal = true;
    }

    fn count(&mut self, bench: &Workbench, locale: Locale) -> usize {
        match self.tab {
            Tab::Diagnostics => self.visible(bench, locale).len(),
            Tab::Progress => self.progress(bench).len(),
        }
    }

    /// 選んでいる行（タブの中の位置）。
    pub fn selected(&self) -> usize {
        self.selected[self.tab_index()]
    }

    fn tab_index(&self) -> usize {
        match self.tab {
            Tab::Diagnostics => 0,
            Tab::Progress => 1,
        }
    }

    /// 前 / 次の行（`Alt+↑/↓`・`↑/↓`・`Ctrl+Home/End`）。
    pub fn nav(&mut self, bench: &Workbench, locale: Locale, nav: Nav) -> bool {
        let count = self.count(bench, locale);
        let at = self.selected().min(count.saturating_sub(1));
        let target = match nav {
            Nav::PrevSection | Nav::PrevScene => at.checked_sub(1),
            Nav::NextSection | Nav::NextScene => (at + 1 < count).then_some(at + 1),
            Nav::FileStart => (count > 0).then_some(0),
            Nav::FileEnd => count.checked_sub(1),
            Nav::PrevFile | Nav::NextFile => None,
        };
        match target {
            Some(found) => {
                let index = self.tab_index();
                self.selected[index] = found;
                self.reveal = true;
                true
            }
            None => false,
        }
    }

    /// 選んでいる行の飛び先。
    pub fn target(&mut self, bench: &Workbench, locale: Locale) -> Option<Target> {
        let at = self.selected();
        match self.tab {
            Tab::Diagnostics => {
                let row = self.visible(bench, locale).get(at)?.clone();
                Some(Target::Document(row.path, row.byte))
            }
            Tab::Progress => self
                .progress(bench)
                .get(at)
                .map(|row| Target::Scene(row.id)),
        }
    }
}

fn severity_index(severity: Severity) -> usize {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}

/// 全文書の診断（`novnow check` と同じもの・同じ並び）。
fn collect(bench: &Workbench, locale: Locale) -> Vec<DiagnosticRow> {
    let project = bench.project();
    let mut documents: Vec<_> = project.documents().collect();
    documents.sort_by(|a, b| a.path.cmp(&b.path));
    let mut out = Vec::new();
    for document in documents {
        let text = document.text.as_str();
        let table = LineTable::new(text);
        let mut rows: Vec<DiagnosticRow> = diagnostics::document(document, project)
            .into_iter()
            .map(|found| {
                let byte = found.range.as_ref().map(|range| range.start);
                let position = byte.map(|start| {
                    let at = table.char_position_of_byte(text, start);
                    (at.line + 1, at.character + 1)
                });
                DiagnosticRow {
                    path: document.path.clone(),
                    position,
                    byte,
                    code: found.code.as_str().to_owned(),
                    severity: found.severity(),
                    place: place::at(document, byte, project)
                        .map(|found| i18n::place(locale, &found)),
                    message: i18n::diagnostic(locale, &found),
                }
            })
            .collect();
        rows.sort_by(|a, b| {
            a.position
                .cmp(&b.position)
                .then_with(|| a.code.cmp(&b.code))
        });
        out.extend(rows);
    }
    out
}

/// 1 フレーム。飛び先が決まったら返す。
pub fn show(
    ctx: &Context,
    state: &mut State,
    bench: &mut Workbench,
    view: &View,
) -> (Option<Request>, Option<Target>) {
    let colors = theme::palette(view.mode);
    let locale = view.locale;
    // 編集欄が無いので、テキストにフォーカスが無ければ素の矢印と Enter も受ける
    let typing = ctx.memory(|memory| memory.focused().is_some());
    let mut jump = None;
    let mut request = None;
    if !typing {
        let (down, up, enter) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if down {
            state.nav(bench, locale, Nav::NextSection);
        }
        if up {
            state.nav(bench, locale, Nav::PrevSection);
        }
        if enter {
            jump = state.target(bench, locale);
        }
    }
    egui::CentralPanel::default()
        .frame(
            Frame::none()
                .fill(colors.bg_base)
                .inner_margin(Margin::symmetric(16.0, 8.0)),
        )
        .show(ctx, |ui| {
            if tabs(ui, state, view) {
                request = Some(Request::Shortcut(crate::ui::shell::Command::Review));
            }
            super::separator(ui, view.mode);
            let clicked = match state.tab {
                Tab::Diagnostics => diagnostics_tab(ui, state, bench, view),
                Tab::Progress => progress_tab(ui, state, bench, view),
            };
            if let Some((index, double)) = clicked {
                let tab = state.tab_index();
                state.selected[tab] = index;
                if double {
                    jump = state.target(bench, locale);
                }
            }
            state.reveal = false;
        });
    (request, jump)
}

fn tabs(ui: &mut Ui, state: &mut State, view: &View) -> bool {
    let colors = theme::palette(view.mode);
    let mut rerun = false;
    ui.horizontal(|ui| {
        for (tab, key) in [
            (Tab::Diagnostics, "review.tab_diagnostics"),
            (Tab::Progress, "review.tab_progress"),
        ] {
            let selected = state.tab == tab;
            let text = RichText::new(i18n::text(view.locale, key))
                .font(FontId::new(13.0 * view.scale, FontFamily::Proportional))
                .color(if selected {
                    colors.accent
                } else {
                    colors.fg_muted
                });
            if ui.selectable_label(selected, text).clicked() {
                state.tab = tab;
            }
        }
        if ui
            .button(i18n::text(view.locale, "review.rerun_button"))
            .clicked()
        {
            rerun = true;
        }
        ui.label(
            RichText::new(i18n::text(view.locale, "review.keys"))
                .font(FontId::new(11.0 * view.scale, FontFamily::Proportional))
                .color(colors.fg_faint),
        );
    });
    rerun
}

/// 診断タブ。クリックされた行（と、ダブルクリックか）を返す。
fn diagnostics_tab(
    ui: &mut Ui,
    state: &mut State,
    bench: &Workbench,
    view: &View,
) -> Option<(usize, bool)> {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let locale = view.locale;
    let all = state.diagnostics(bench, locale).to_vec();
    let counts = [Severity::Error, Severity::Warning, Severity::Info]
        .map(|wanted| all.iter().filter(|row| row.severity == wanted).count());
    ui.horizontal(|ui| {
        for (index, key) in ["review.errors", "review.warnings", "review.infos"]
            .iter()
            .enumerate()
        {
            let text = i18n::format(locale, key, &[&counts[index].to_string()]);
            let color = match index {
                0 => colors.error,
                1 => colors.warn,
                _ => colors.fg_muted,
            };
            let mut shown = state.shown[index];
            if ui
                .checkbox(
                    &mut shown,
                    RichText::new(text)
                        .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                        .color(color),
                )
                .changed()
            {
                state.shown[index] = shown;
            }
        }
        ui.add_space(12.0);
        for (sort, key) in [
            (Sort::Position, "review.sort_position"),
            (Sort::Severity, "review.sort_severity"),
            (Sort::Code, "review.sort_code"),
        ] {
            let selected = state.sort == sort;
            let text = RichText::new(i18n::text(locale, key))
                .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                .color(if selected {
                    colors.accent
                } else {
                    colors.fg_muted
                });
            if ui.selectable_label(selected, text).clicked() {
                state.sort = sort;
            }
        }
    });
    ui.add_space(4.0);
    let rows = state.visible(bench, locale);
    if rows.is_empty() {
        super::hint(
            ui,
            view.mode,
            scale,
            i18n::text(locale, "review.no_diagnostics"),
        );
        return None;
    }
    let selected = state.selected().min(rows.len() - 1);
    let reveal = state.reveal;
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_source("review-diagnostics")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, row) in rows.iter().enumerate() {
                let location = match row.position {
                    Some((line, column)) => format!("{}:{line}:{column}", row.path),
                    None => row.path.clone(),
                };
                let mut text = String::new();
                if let Some(place) = &row.place {
                    text.push_str(&format!("[{place}] "));
                }
                text.push_str(&row.message);
                text.push_str("  ");
                text.push_str(&location);
                let color = match row.severity {
                    Severity::Error => colors.error,
                    Severity::Warning => colors.warn,
                    Severity::Info => colors.fg_faint,
                };
                let response = list_row(
                    ui,
                    view.mode,
                    scale,
                    Row {
                        selected: index == selected,
                        bar: Some(color),
                        label: &row.code,
                        text: &text,
                        muted: false,
                    },
                );
                if index == selected && reveal {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.double_clicked() {
                    clicked = Some((index, true));
                } else if response.clicked() {
                    clicked = Some((index, false));
                }
            }
        });
    clicked
}

/// 進捗タブ。
fn progress_tab(
    ui: &mut Ui,
    state: &mut State,
    bench: &Workbench,
    view: &View,
) -> Option<(usize, bool)> {
    let scale = view.scale;
    let locale = view.locale;
    let rows = state.progress(bench).to_vec();
    if rows.is_empty() {
        super::hint(
            ui,
            view.mode,
            scale,
            i18n::text(locale, "review.no_progress"),
        );
        return None;
    }
    let digits = bench.project().digits();
    let selected = state.selected().min(rows.len() - 1);
    let reveal = state.reveal;
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_source("review-progress")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, row) in rows.iter().enumerate() {
                let amount = match row.target {
                    Some(target) if target > 0 => i18n::format(
                        locale,
                        "review.chars_of",
                        &[
                            &context_view::group(row.chars),
                            &context_view::group(target as usize),
                            &(row.chars * 100 / target as usize).to_string(),
                        ],
                    ),
                    _ => i18n::format(locale, "review.chars", &[&context_view::group(row.chars)]),
                };
                let mut text = String::new();
                if !row.title.is_empty() {
                    text.push_str(&row.title);
                    text.push_str("  ");
                }
                text.push_str(&amount);
                let label = format!(
                    "{}{}",
                    "  ".repeat(row.depth),
                    plots::short_id(row.id, digits)
                );
                let response = list_row(
                    ui,
                    view.mode,
                    scale,
                    Row {
                        selected: index == selected,
                        bar: Some(state_color(view.mode, Some(row.state))),
                        label: &label,
                        text: &text,
                        muted: !row.prose,
                    },
                );
                if index == selected && reveal {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.double_clicked() {
                    clicked = Some((index, true));
                } else if response.clicked() {
                    clicked = Some((index, false));
                }
            }
        });
    clicked
}
