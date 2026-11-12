//! `review` — 点検（§10.9）。このファイルは**診断と進捗の 2 タブ**を描く。文体・統計・挿絵・
//! 五感・提案のタブは `tabs.rs` が同じワークスペースに足す。
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

/// タブ（§10.9.1 の 1・2 + §15.6 スレッド）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// 診断（§10.9.2）。
    #[default]
    Diagnostics,
    /// 進捗。
    Progress,
    /// エージェント間通信のスレッド（§15.6）。
    Threads,
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
    selected: [usize; 3],
    /// 重大度の絞り込み（E / W / I）。
    pub shown: [bool; 3],
    /// 並べ替え。
    pub sort: Sort,
    /// 全診断コードの有効設定パネルを開いているか（§10.9.6）。
    pub settings_open: bool,
    /// 診断コード設定パネルの絞り込み文字列。
    pub settings_filter: String,
    diagnostics: Option<(u64, Locale, Vec<DiagnosticRow>)>,
    progress: Option<(u64, Vec<ProgressRow>)>,
    reveal: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::Diagnostics,
            selected: [0, 0, 0],
            shown: [true, true, true],
            sort: Sort::Position,
            settings_open: false,
            settings_filter: String::new(),
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

    /// タブを切り替える（`Ctrl+Shift+→` / `←`）。
    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            Tab::Diagnostics => Tab::Progress,
            Tab::Progress => Tab::Diagnostics,
            Tab::Threads => Tab::Diagnostics,
        };
        self.reveal = true;
    }

    /// 指定のサブタブに切り替える。
    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.reveal = true;
    }

    /// 指定の `thread_id` のスレッドを `Tab::Threads` で開く（§15.6）。
    pub fn select_thread(&mut self, bench: &Workbench, thread_id: &str) {
        self.tab = Tab::Threads;
        let board = crate::core::agents::files::Board::of(bench.project());
        if let Some(pos) = board.threads.iter().position(|t| t.id == thread_id) {
            self.selected[2] = pos;
        } else {
            self.selected[2] = 0;
        }
        self.reveal = true;
    }

    /// 現在の節 `scene` に関する診断（無ければ診断一覧の先頭）に着地する（`Ctrl+Shift+9`、§10.0）。
    pub fn land_scene(&mut self, bench: &Workbench, locale: Locale, scene: Option<SceneId>) {
        self.tab = Tab::Diagnostics;
        self.reveal = true;
        let Some(scene) = scene else {
            self.selected[0] = 0;
            return;
        };
        let matches_scene = |row: &DiagnosticRow| -> bool {
            let digits = bench.project().digits();
            let full_id = scene.display(digits);
            if row.place.as_deref().is_some_and(|p| p.contains(&full_id)) {
                return true;
            }
            let contents_path = bench.project().contents_path(scene.chapter, scene.episode);
            if row.path == contents_path {
                if let Some((line_1, _)) = row.position {
                    let line_0 = line_1.saturating_sub(1);
                    if bench.project().sections().iter().any(|s| {
                        s.path == contents_path && s.id == scene && s.lines.contains(&line_0)
                    }) {
                        return true;
                    }
                }
            }
            let plot_path = bench
                .project()
                .episode_plot_path(scene.chapter, scene.episode);
            if row.path == plot_path {
                if let Some((line_1, _)) = row.position {
                    let line_0 = line_1.saturating_sub(1);
                    let ep_rows =
                        plots::episode_rows(bench.project(), scene.chapter, scene.episode);
                    if ep_rows
                        .iter()
                        .any(|r| r.id == scene && r.lines.contains(&line_0))
                    {
                        return true;
                    }
                }
            }
            false
        };
        let visible = self.visible(bench, locale);
        if let Some(index) = visible.iter().position(&matches_scene) {
            self.selected[0] = index;
            return;
        }
        if self.shown != [true, true, true]
            && self.diagnostics(bench, locale).iter().any(&matches_scene)
        {
            self.shown = [true, true, true];
            let expanded = self.visible(bench, locale);
            if let Some(index) = expanded.iter().position(&matches_scene) {
                self.selected[0] = index;
                return;
            }
        }
        self.selected[0] = 0;
    }

    fn count(&mut self, bench: &Workbench, locale: Locale) -> usize {
        match self.tab {
            Tab::Diagnostics => self.visible(bench, locale).len(),
            Tab::Progress => self.progress(bench).len(),
            Tab::Threads => crate::core::agents::files::Board::of(bench.project())
                .threads
                .len(),
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
            Tab::Threads => 2,
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
            Tab::Threads => {
                let board = crate::core::agents::files::Board::of(bench.project());
                let thread = board.threads.get(at)?;
                let raw = thread.scope.trim().trim_start_matches('@');
                crate::core::id::PartialSceneId::parse(raw)
                    .ok()
                    .map(|p| Target::Scene(p.expand_low()))
            }
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
    // この画面にはテキストの入力欄が無い。見出しのボタンにフォーカスがあるときも、
    // 診断・進み具合の一覧から修飾なしの矢印を奪わない。
    let filter_id = egui::Id::new("review-settings-filter");
    let typing = view.overlay || (state.settings_open && ctx.memory(|m| m.has_focus(filter_id)));
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
            let (rerun, guide) = tabs(ui, state, view);
            if rerun {
                request = Some(Request::Shortcut(crate::ui::shell::Command::Review));
            }
            match guide {
                Some(Guide::Next) => {
                    state.nav(bench, locale, Nav::NextSection);
                }
                Some(Guide::Open) => {
                    jump = state.target(bench, locale);
                }
                None => {}
            }
            super::separator(ui, view.mode);
            let (clicked, sub_request) = match state.tab {
                Tab::Diagnostics => diagnostics_tab(ui, state, bench, view),
                Tab::Progress => (progress_tab(ui, state, bench, view), None),
                Tab::Threads => threads_tab(ui, state, bench, view),
            };
            if sub_request.is_some() {
                request = sub_request;
            }
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

/// キー案内のボタンから頼まれたこと（キーと同じ経路で実行する）。
enum Guide {
    Next,
    Open,
}

fn tabs(ui: &mut Ui, state: &mut State, view: &View) -> (bool, Option<Guide>) {
    let colors = theme::palette(view.mode);
    let mut rerun = false;
    let mut guide = None;
    ui.horizontal(|ui| {
        for (tab, key) in [
            (Tab::Diagnostics, "review.tab_diagnostics"),
            (Tab::Progress, "review.tab_progress"),
            (Tab::Threads, "review.tab_threads"),
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
    // キー案内（項目ごとにボタン。実行はキーと同じ経路）
    let items = [
        i18n::text(view.locale, "review.key_tab"),
        i18n::text(view.locale, "review.key_select"),
        i18n::text(view.locale, "review.key_open"),
        i18n::text(view.locale, "review.rerun_button"),
    ];
    match crate::ui::widgets::key_guide(
        ui,
        &items,
        FontId::new(11.0 * view.scale, FontFamily::Proportional),
        &colors,
    ) {
        Some(0) => {
            state.tab = match state.tab {
                Tab::Diagnostics => Tab::Progress,
                Tab::Progress => Tab::Threads,
                Tab::Threads => Tab::Diagnostics,
            };
        }
        Some(1) => guide = Some(Guide::Next),
        Some(2) => guide = Some(Guide::Open),
        Some(3) => rerun = true,
        _ => {}
    }
    (rerun, guide)
}

/// 診断タブ。クリックされた行（と、ダブルクリックか）と有効設定の要求を返す。
fn diagnostics_tab(
    ui: &mut Ui,
    state: &mut State,
    bench: &Workbench,
    view: &View,
) -> (Option<(usize, bool)>, Option<Request>) {
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
            if super::selectable_action(ui, selected, text).clicked() {
                state.sort = sort;
            }
        }
        ui.add_space(12.0);
        let settings_text = RichText::new(i18n::text(locale, "review.settings_toggle"))
            .font(FontId::new(12.0 * scale, FontFamily::Proportional))
            .color(if state.settings_open {
                colors.accent
            } else {
                colors.fg_muted
            });
        if super::selectable_action(ui, state.settings_open, settings_text).clicked() {
            state.settings_open = !state.settings_open;
        }
    });
    ui.add_space(4.0);
    let mut level_request = None;
    if state.settings_open {
        let overrides = &bench.project().settings().features.diagnostics;
        let query = state.settings_filter.trim().to_ascii_uppercase();
        Frame::none()
            .fill(colors.bg_raised)
            .inner_margin(Margin::symmetric(8.0, 6.0))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.settings_filter)
                        .id(egui::Id::new("review-settings-filter"))
                        .hint_text(i18n::text(locale, "review.settings_filter_hint"))
                        .desired_width(220.0),
                );
                egui::ScrollArea::vertical()
                    .id_source("review-catalog-levels")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for (code, _) in diagnostics::CATALOG {
                            let code_str = code.as_str();
                            if !query.is_empty() && !code_str.contains(&query) {
                                continue;
                            }
                            let current = diagnostics::level_for(*code, overrides);
                            ui.horizontal(|ui| {
                                ui.monospace(code_str);
                                for (value, key, lv) in [
                                    ("off", "review.level_off", diagnostics::DiagnosticLevel::Off),
                                    (
                                        "warn",
                                        "review.level_warn",
                                        diagnostics::DiagnosticLevel::Warn,
                                    ),
                                    (
                                        "error",
                                        "review.level_error",
                                        diagnostics::DiagnosticLevel::Error,
                                    ),
                                ] {
                                    let selected = current == Some(lv);
                                    let text = RichText::new(i18n::text(locale, key))
                                        .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                                        .color(if selected {
                                            colors.accent
                                        } else {
                                            colors.fg_muted
                                        });
                                    if super::selectable_action(ui, selected, text).clicked() {
                                        level_request = Some(Request::SetDiagnosticLevel {
                                            code: code_str.to_owned(),
                                            level: value.to_owned(),
                                        });
                                    }
                                }
                                if current.is_none() {
                                    ui.label(
                                        RichText::new(i18n::text(locale, "review.level_default"))
                                            .font(FontId::new(
                                                11.0 * scale,
                                                FontFamily::Proportional,
                                            ))
                                            .color(colors.fg_faint),
                                    );
                                }
                            });
                        }
                    });
            });
        ui.add_space(4.0);
    }
    let rows = state.visible(bench, locale);
    // 選択中コードの有効設定（§10.9.6。[features.diagnostics] へ保存する）と W084 ペア導線（§10.6.1）
    if let Some(row) = rows.get(state.selected()) {
        let overrides = &bench.project().settings().features.diagnostics;
        let current = diagnostics::CATALOG
            .iter()
            .find(|(code, _)| code.as_str() == row.code)
            .and_then(|(code, _)| diagnostics::level_for(*code, overrides));
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(i18n::format(locale, "review.level_for", &[&row.code]))
                    .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                    .color(colors.fg_muted),
            );
            for (value, key, level) in [
                ("off", "review.level_off", diagnostics::DiagnosticLevel::Off),
                (
                    "warn",
                    "review.level_warn",
                    diagnostics::DiagnosticLevel::Warn,
                ),
                (
                    "error",
                    "review.level_error",
                    diagnostics::DiagnosticLevel::Error,
                ),
            ] {
                let selected = current == Some(level);
                let text = RichText::new(i18n::text(locale, key))
                    .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                    .color(if selected {
                        colors.accent
                    } else {
                        colors.fg_muted
                    });
                if super::selectable_action(ui, selected, text).clicked() {
                    level_request = Some(Request::SetDiagnosticLevel {
                        code: row.code.clone(),
                        level: value.to_owned(),
                    });
                }
            }
            if current.is_none() {
                ui.label(
                    RichText::new(i18n::text(locale, "review.level_default"))
                        .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                        .color(colors.fg_muted),
                );
            }
            if row.code == "W084" {
                let matched = crate::core::features::metadata::address_violations(bench.project())
                    .into_iter()
                    .find(|v| v.path == row.path && Some(v.byte) == row.byte);
                if let Some(v) = matched {
                    ui.add_space(8.0);
                    let btn_label =
                        i18n::format(locale, "review.open_address_pair", &[&v.speaker, &v.target]);
                    if ui.button(btn_label).clicked() {
                        level_request = Some(Request::OpenAddressPair {
                            speaker: v.speaker,
                            target: v.target,
                            mode: v.mode,
                        });
                    }
                }
            }
        });
        ui.add_space(4.0);
    }
    if rows.is_empty() {
        super::hint(
            ui,
            view.mode,
            scale,
            i18n::text(locale, "review.no_diagnostics"),
        );
        return (None, level_request);
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
    (clicked, level_request)
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

/// エージェント間通信のスレッド一覧・本文・承認状態タブ（§15.6）。
fn threads_tab(
    ui: &mut Ui,
    state: &mut State,
    bench: &Workbench,
    view: &View,
) -> (Option<(usize, bool)>, Option<Request>) {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let locale = view.locale;
    let board = crate::core::agents::files::Board::of(bench.project());
    if board.threads.is_empty() {
        super::hint(
            ui,
            view.mode,
            scale,
            i18n::text(locale, "review.no_threads"),
        );
        return (None, None);
    }
    let selected = state.selected().min(board.threads.len() - 1);
    let reveal = state.reveal;
    let mut clicked = None;
    let mut req = None;

    egui::ScrollArea::vertical()
        .id_source("review-threads-list")
        .max_height(160.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, thread) in board.threads.iter().enumerate() {
                let has_pending = thread
                    .messages
                    .iter()
                    .any(|m| m.approved_by == crate::core::agents::files::PENDING);
                let bar_color = if has_pending {
                    colors.warn
                } else {
                    colors.fg_faint
                };
                let text = format!(
                    "{}  {}  [{}] ({})",
                    thread.title,
                    thread.scope,
                    thread.status,
                    thread.messages.len()
                );
                let response = list_row(
                    ui,
                    view.mode,
                    scale,
                    Row {
                        selected: index == selected,
                        bar: Some(bar_color),
                        label: &thread.id,
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

    super::separator(ui, view.mode);
    if let Some(thread) = board.threads.get(selected) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(i18n::format(
                locale,
                "review.thread_header",
                &[&thread.id, &thread.title, &thread.scope, &thread.status],
            ));
            if ui
                .button(i18n::text(locale, "review.open_in_agent"))
                .clicked()
            {
                req = Some(Request::OpenAgentThread(thread.id.clone()));
            }
        });
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_source("review-thread-messages")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for msg in &thread.messages {
                    let is_pending = msg.approved_by == crate::core::agents::files::PENDING;
                    let hdr = i18n::format(
                        locale,
                        "review.thread_message",
                        &[
                            &format!("{:03}", msg.number),
                            &msg.from,
                            &msg.to,
                            &msg.kind,
                            &msg.approved_by,
                        ],
                    );
                    ui.label(
                        RichText::new(hdr)
                            .font(FontId::new(12.0 * scale, FontFamily::Monospace))
                            .color(if is_pending {
                                colors.warn
                            } else {
                                colors.fg_muted
                            }),
                    );
                    if !msg.body.is_empty() {
                        ui.label(
                            RichText::new(&msg.body)
                                .font(FontId::new(13.0 * scale, FontFamily::Proportional))
                                .color(colors.fg_primary),
                        );
                    }
                    ui.add_space(6.0);
                }
            });
    }
    (clicked, req)
}
