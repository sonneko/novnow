//! GUI の合成の根（`eframe::App`）。§9 の共通シェル・§14.4 のプロジェクト選択・§8 の保存をつなぐ。
//!
//! 描画とキー操作はここと `ui::shell`・`ui::widgets`、状態の規則は egui に依存しない
//! `ui::workbench`・`ui::save_machine`・`ui::settings`・`ui::palette` に置く（テストはそちら）。
//!
//! 規則 10: eframe の `persistence` feature は使わない（ウィンドウの位置も覚えない）。書くのは
//! プロジェクトの中（本文・`.novnow/ui.json`）と、デスクトップのパス履歴（`RecentFile`）だけ。

use std::path::PathBuf;

use egui::{
    Align, Align2, Color32, Context, FontFamily, FontId, Frame, Id, Key, Layout, Margin, Modifiers,
    Pos2, Rect, RichText, Sense, Stroke, Vec2,
};

use crate::core::features::Project;
use crate::ui::i18n::{self, Locale};
use crate::ui::palette;
use crate::ui::save_machine::{Indicator, State};
use crate::ui::settings::{LanguageChoice, PaneSide, ThemeChoice, UiSettings};
use crate::ui::shell::{self, Command, ImeState};
use crate::ui::theme::{self, Mode};
use crate::ui::widgets::{context_view, reference_pane, section_editor};
use crate::ui::workbench::Workbench;

#[cfg(not(target_arch = "wasm32"))]
pub use native::run;

/// 開いたプロジェクトに対する I/O（デスクトップはファイル、Web は IndexedDB）。
pub trait Host {
    /// ファイルを書く（プロジェクトの中）。失敗は表示用の文言。
    fn write(&mut self, path: &str, text: &str) -> Result<(), String>;
    /// 外部変更の監視 1 回分（§8.5）: 変わったファイルと中身（消えたら `None`）。
    fn poll(&mut self) -> Vec<(String, Option<String>)>;
    /// プロジェクトの名前（ヘッダ・ウィンドウ題名）。
    fn name(&self) -> String;
}

/// フォーカス中のペイン（§9.2.3。外周 2px の `accent` 枠で示す）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Context,
    Editor,
    Reference,
}

/// 画面の上に重ねるもの（同時に 1 つ）。
#[derive(Debug, Clone, PartialEq)]
enum Overlay {
    None,
    Palette { input: String, selected: usize },
    Jump { input: String, missed: bool },
    Help,
    SaveError,
    CloseGuard,
}

/// 開いているプロジェクト。
pub struct Session {
    host: Box<dyn Host>,
    bench: Workbench,
    settings_dirty: bool,
    last_poll: f64,
    context_cache: context_view::Cache,
    editor_cache: section_editor::Cache,
    reference_cache: reference_pane::Cache,
    diag_count: Option<(u64, String, usize)>,
    notice: Option<(String, f64)>,
}

impl Session {
    /// 読み込んだプロジェクトから作る。
    pub fn new(host: Box<dyn Host>, project: Project, settings: &UiSettings) -> Self {
        let bench = Workbench::new(
            project,
            settings.last_scene.as_deref(),
            settings.autosave_ms,
        );
        Self {
            host,
            bench,
            settings_dirty: false,
            last_poll: 0.0,
            context_cache: context_view::Cache::default(),
            editor_cache: section_editor::Cache::default(),
            reference_cache: reference_pane::Cache::default(),
            diag_count: None,
            notice: None,
        }
    }
}

/// 起動前の画面（§14.4 のプロジェクト選択）。
pub trait Picker {
    /// 描く。プロジェクトを開いたら `Some`。
    fn show(&mut self, ctx: &Context, locale: Locale, mode: Mode) -> Option<(Session, UiSettings)>;
    /// `Ctrl+O`。
    fn open_folder(&mut self);
}

enum Stage {
    Picker(Box<dyn Picker>),
    Open(Box<Session>),
}

/// novnow の GUI。
pub struct NovnowApp {
    stage: Stage,
    settings: UiSettings,
    env_locale: Locale,
    ime: ImeState,
    overlay: Overlay,
    focus: Pane,
    applied: Option<(Mode, u32)>,
    font_warning: bool,
    allow_close: bool,
    mac: bool,
}

impl NovnowApp {
    /// 起動。`picker` はプロジェクトを開く前の画面、`opened` は起動時に開けたプロジェクト。
    pub fn new(
        ctx: &Context,
        env_locale: Locale,
        picker: Box<dyn Picker>,
        opened: Option<(Session, UiSettings)>,
    ) -> Self {
        let (fonts, fallback) = theme::fonts();
        ctx.set_fonts(fonts);
        let (stage, settings) = match opened {
            Some((session, settings)) => (Stage::Open(Box::new(session)), settings),
            None => (Stage::Picker(picker), UiSettings::default()),
        };
        Self {
            stage,
            settings,
            env_locale,
            ime: ImeState::default(),
            overlay: Overlay::None,
            focus: Pane::Editor,
            applied: None,
            font_warning: fallback && !cfg!(target_arch = "wasm32"),
            allow_close: false,
            mac: ctx.os() == egui::os::OperatingSystem::Mac,
        }
    }

    /// 開いているプロジェクトの編集状態（テスト用の読み口）。
    pub fn workbench(&self) -> Option<&Workbench> {
        match &self.stage {
            Stage::Open(session) => Some(&session.bench),
            Stage::Picker(_) => None,
        }
    }

    /// 何か重ねて出しているか（パレット・ヘルプ・モーダル）。
    pub fn overlay_open(&self) -> bool {
        self.overlay != Overlay::None
    }

    /// `ui.json` の中身（今の値）。
    pub fn settings(&self) -> &UiSettings {
        &self.settings
    }

    /// IME の変換中か（§9.8.5）。
    pub fn composing(&self) -> bool {
        self.ime.composing()
    }

    /// 今の言語（§12.4: `ui.json` → `NOVNOW_LANG` → OS → ja）。
    pub fn locale(&self) -> Locale {
        match self.settings.language {
            LanguageChoice::Ja => Locale::Ja,
            LanguageChoice::En => Locale::En,
            LanguageChoice::System => self.env_locale,
        }
    }

    fn mode(&self, ctx: &Context, system_dark: Option<bool>) -> Mode {
        let _ = ctx;
        Mode::resolve(self.settings.theme, system_dark)
    }

    /// 1 フレーム（`eframe::App::update` から呼ぶ）。`system_dark` は OS の配色（分からなければ `None`）。
    pub fn frame(&mut self, ctx: &Context, system_dark: Option<bool>) {
        let mode = self.mode(ctx, system_dark);
        self.apply_style(ctx, mode);
        let now = ctx.input(|input| input.time);
        let locale = self.locale();
        // §9.8.5 (3): IME → 変換中なら Key を捨てる → ショートカット
        let text_focused = ctx.wants_keyboard_input();
        let commands = ctx
            .input_mut(|input| shell::take_commands(input, &mut self.ime, self.mac, text_focused));
        let focus_lost = ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::WindowFocused(false)))
        });
        for command in commands {
            self.run(ctx, command, now);
        }
        match &mut self.stage {
            Stage::Picker(picker) => {
                if let Some((session, settings)) = picker.show(ctx, locale, mode) {
                    self.settings = settings;
                    self.stage = Stage::Open(Box::new(session));
                    self.overlay = Overlay::None;
                    self.focus = Pane::Editor;
                    ctx.request_repaint();
                }
            }
            Stage::Open(session) => {
                if self.font_warning {
                    // §11.5 SHOULD: 起動エラーにせず、警告を出して同梱フォントで続ける
                    self.font_warning = false;
                    session.notice =
                        Some((i18n::text(locale, "status.font_fallback").to_owned(), now));
                }
                self.workbench_frame(ctx, mode, locale, now, focus_lost);
            }
        }
        self.close_guard(ctx, now);
        // 次の再描画（保存のタイマ・外部変更の監視 1s・OS の配色の追従 1s、§11.4）
        ctx.request_repaint_after(std::time::Duration::from_millis(1000));
    }

    fn apply_style(&mut self, ctx: &Context, mode: Mode) {
        let scale_key = (self.settings.font_scale * 100.0).round() as u32;
        if self.applied == Some((mode, scale_key)) {
            return;
        }
        ctx.set_visuals(theme::visuals(mode));
        let scale = self.settings.font_scale;
        ctx.style_mut(|style| {
            use egui::TextStyle;
            let base = |text: &TextStyle| match text {
                TextStyle::Small => 10.0,
                TextStyle::Body | TextStyle::Button | TextStyle::Monospace => 13.0,
                TextStyle::Heading => 18.0,
                TextStyle::Name(_) => 13.0,
            };
            for (text, font) in style.text_styles.iter_mut() {
                font.size = base(text) * scale;
            }
            // 切替アニメーションはしない（§11.4 MUST）
            style.animation_time = 0.0;
        });
        self.applied = Some((mode, scale_key));
    }

    fn run(&mut self, ctx: &Context, command: Command, now: f64) {
        let locale = self.locale();
        match command {
            Command::ToggleLanguage => {
                self.settings.language = match locale {
                    Locale::Ja => LanguageChoice::En,
                    Locale::En => LanguageChoice::Ja,
                };
                self.settings_changed();
            }
            Command::CycleTheme => {
                self.settings.theme = self.settings.theme.next();
                self.settings_changed();
            }
            Command::FontUp | Command::FontDown => {
                self.settings.step_font(command == Command::FontUp);
                self.settings_changed();
            }
            Command::FontReset => {
                self.settings.font_scale = 1.0;
                self.settings_changed();
            }
            Command::Help => {
                self.overlay = if self.overlay == Overlay::Help {
                    Overlay::None
                } else {
                    Overlay::Help
                };
            }
            Command::Escape => {
                if self.overlay != Overlay::None {
                    self.overlay = Overlay::None;
                }
                self.focus = Pane::Editor;
                if let Stage::Open(session) = &self.stage {
                    ctx.memory_mut(|memory| {
                        memory.request_focus(section_editor::editor_id(&session.bench))
                    });
                }
            }
            Command::Open => {
                if let Stage::Picker(picker) = &mut self.stage {
                    picker.open_folder();
                }
                // 開いている間の `Ctrl+O`（別のプロジェクトへ）は M8 のパレットから（B-117）
            }
            _ => {}
        }
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        let bench = &mut session.bench;
        match command {
            Command::Palette => {
                self.overlay = Overlay::Palette {
                    input: String::new(),
                    selected: 0,
                };
            }
            Command::Jump => {
                self.overlay = Overlay::Jump {
                    input: String::new(),
                    missed: false,
                };
            }
            Command::SaveNow => {
                bench.commit();
                bench.edited(now);
                bench.save.request_now();
            }
            Command::Reload => {
                // F5: 監視を今すぐ 1 回回す（外部変更の読み直し、§8.5）
                session.last_poll = f64::NEG_INFINITY;
            }
            Command::FocusContext => self.focus = Pane::Context,
            Command::FocusEditor => {
                self.focus = Pane::Editor;
                ctx.memory_mut(|memory| memory.request_focus(section_editor::editor_id(bench)));
            }
            Command::FocusReference => {
                self.focus = Pane::Reference;
                self.settings.reference_visible = true;
            }
            Command::ToggleReference => {
                self.settings.reference_visible = !self.settings.reference_visible;
            }
            Command::SwapReference => {
                self.settings.reference_pane_side = match self.settings.reference_pane_side {
                    PaneSide::Right => PaneSide::Left,
                    PaneSide::Left => PaneSide::Right,
                };
                session.settings_dirty = true;
            }
            Command::Nav(nav) => {
                if bench.go(nav) {
                    session.settings_dirty = true;
                    ctx.memory_mut(|memory| memory.request_focus(section_editor::editor_id(bench)));
                }
            }
            Command::ConfirmNext => {
                if bench.confirm_next(now) {
                    session.settings_dirty = true;
                    ctx.memory_mut(|memory| memory.request_focus(section_editor::editor_id(bench)));
                }
            }
            Command::NewSection => {
                if bench.new_section(now) {
                    session.settings_dirty = true;
                    ctx.memory_mut(|memory| memory.request_focus(section_editor::editor_id(bench)));
                }
            }
            Command::MainRatio(grow) => {
                self.settings.step_ratio(true, grow);
                session.settings_dirty = true;
            }
            Command::ContextRatio(grow) => {
                self.settings.step_ratio(false, grow);
                session.settings_dirty = true;
            }
            _ => {}
        }
    }

    fn settings_changed(&mut self) {
        if let Stage::Open(session) = &mut self.stage {
            session.settings_dirty = true;
        }
    }

    // ============================================================ 執筆画面（§9.1）

    fn workbench_frame(
        &mut self,
        ctx: &Context,
        mode: Mode,
        locale: Locale,
        now: f64,
        focus_lost: bool,
    ) {
        let colors = theme::palette(mode);
        let scale = self.settings.font_scale;
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        // 画面離脱で即時保存（§8.2）
        if focus_lost {
            session.bench.commit();
            session.bench.save.request_now();
        }
        // ① ヘッダ（28px・ラベル無し）
        egui::TopBottomPanel::top("header")
            .exact_height(28.0)
            .frame(
                Frame::none()
                    .fill(colors.bg_raised)
                    .inner_margin(Margin::symmetric(10.0, 4.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // ワークスペースインジケータ（M7 は write のみ。鉛筆の図形）
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Label,
                            i18n::text(locale, "header.workspace.write"),
                        )
                    });
                    let painter = ui.painter_at(rect.expand(2.0));
                    let center = rect.center();
                    painter.line_segment(
                        [center + Vec2::new(-5.0, 5.0), center + Vec2::new(5.0, -5.0)],
                        Stroke::new(1.5_f32, colors.accent),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(session.host.name())
                            .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                            .color(colors.fg_faint),
                    );
                    ui.add_space(8.0);
                    if let Some(crumb) = session.bench.breadcrumb() {
                        ui.label(
                            RichText::new(crumb)
                                .font(FontId::new(12.0 * scale, FontFamily::Monospace))
                                .color(colors.fg_muted),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let theme_button = ui
                            .add(egui::Button::new(theme_symbol(self.settings.theme)).frame(false))
                            .on_hover_text(i18n::text(locale, "header.theme"));
                        theme_button.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                i18n::text(locale, "header.theme"),
                            )
                        });
                        if theme_button.clicked() {
                            self.settings.theme = self.settings.theme.next();
                            session.settings_dirty = true;
                        }
                        // §12.5: 2 文字の記号ボタン（あ / A）。文字ラベルは置かない
                        let symbol = match locale {
                            Locale::Ja => "あ",
                            Locale::En => "A",
                        };
                        let language = ui.add(egui::Button::new(symbol).frame(false));
                        language.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                i18n::text(locale, "header.language"),
                            )
                        });
                        if language.clicked() {
                            self.settings.language = match locale {
                                Locale::Ja => LanguageChoice::En,
                                Locale::En => LanguageChoice::Ja,
                            };
                            session.settings_dirty = true;
                        }
                    });
                });
            });
        // ④ ステータスバー（22px）
        let indicator = session.bench.save.indicator(now);
        let mut open_error = false;
        egui::TopBottomPanel::bottom("status")
            .exact_height(22.0)
            .frame(
                Frame::none()
                    .fill(colors.bg_raised)
                    .inner_margin(Margin::symmetric(10.0, 2.0)),
            )
            .show(ctx, |ui| {
                open_error = status_bar(ui, session, locale, mode, scale, indicator, now);
            });
        if open_error {
            self.overlay = Overlay::SaveError;
        }
        // ③ 参照ペイン（既定 38%、最小 320px / 最大 560px）。幅 < 900px では畳む（§9.1）
        let width = ctx.screen_rect().width();
        let show_reference =
            self.settings.reference_visible && (width >= 900.0 || self.focus == Pane::Reference);
        if show_reference {
            if let Some(next) = session.reference_cache.update(&session.bench, now) {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                    (next - now).max(0.0),
                ));
            }
            let panel = match self.settings.reference_pane_side {
                PaneSide::Right => egui::SidePanel::right("reference"),
                PaneSide::Left => egui::SidePanel::left("reference"),
            };
            let default_width = (width * (1.0 - self.settings.main_ratio)).clamp(320.0, 560.0);
            let response = panel
                .resizable(false)
                .exact_width(default_width)
                .frame(
                    Frame::none()
                        .fill(colors.bg_base)
                        .inner_margin(Margin::symmetric(14.0, 4.0)),
                )
                .show(ctx, |ui| {
                    reference_pane::show(ui, &session.reference_cache, locale, mode, scale);
                });
            if self.focus == Pane::Reference {
                focus_ring(ctx, response.response.rect, colors.accent);
            }
        }
        // ② メインカラム: 文脈ビュー（55%）/ 節エディタ（45%）
        let mut clicked = None;
        let mut changed = false;
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(colors.bg_base)
                    .inner_margin(Margin::symmetric(16.0, 6.0)),
            )
            .show(ctx, |ui| {
                if session.bench.current().is_none() {
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "editor.no_sections"))
                            .color(colors.fg_muted),
                    );
                    return;
                }
                let total = ui.available_height();
                let context_height = (total * self.settings.context_ratio).max(80.0);
                let digits = session.bench.project().digits();
                let scene = session
                    .bench
                    .current()
                    .map(|current| current.id.display(digits))
                    .unwrap_or_default();
                let (section_chars, _, _) = session.bench.counts();
                let target = session.bench.current().and_then(|current| {
                    crate::core::features::reference::target_length(
                        session.bench.project(),
                        current.id,
                    )
                });
                let placeholder = context_view::Placeholder {
                    scene: &scene,
                    chars: section_chars,
                    target,
                    indicator,
                    accessible: i18n::text(locale, "editor.editing"),
                };
                let context =
                    ui.allocate_ui(Vec2::new(ui.available_width(), context_height), |ui| {
                        ui.set_min_height(context_height);
                        context_view::show(
                            ui,
                            &session.bench,
                            &mut session.context_cache,
                            mode,
                            scale,
                            self.settings.context_sections,
                            &placeholder,
                        )
                    });
                clicked = context.inner;
                if self.focus == Pane::Context {
                    focus_ring(ctx, context.response.rect, colors.accent);
                }
                ui.add_space(6.0);
                let editor = ui.allocate_ui(ui.available_size(), |ui| {
                    section_editor::show(
                        ui,
                        &mut session.bench,
                        &mut session.editor_cache,
                        mode,
                        scale,
                    )
                });
                changed = editor.inner;
                if self.focus == Pane::Editor {
                    focus_ring(ctx, editor.response.rect, colors.accent);
                }
            });
        if changed {
            session.bench.edited(now);
        }
        if let Some(index) = clicked {
            if session.bench.go_to(index) {
                session.settings_dirty = true;
                ctx.memory_mut(|memory| {
                    memory.request_focus(section_editor::editor_id(&session.bench))
                });
            }
        }
        // 保存（§8.2）
        save_if_due(session, &mut self.settings, now);
        if let Some(at) = session.bench.save.next_wakeup(now) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((at - now).max(0.0)));
        }
        // 外部変更の監視（1,000ms、§8.5）
        if now - session.last_poll >= 1.0 {
            session.last_poll = now;
            for (path, text) in session.host.poll() {
                if session.bench.external_change(&path, text)
                    == crate::ui::workbench::Reaction::Reloaded
                {
                    let message = i18n::format(locale, "diag.I052", &[&path]);
                    session.notice = Some((message, now));
                }
            }
        }
        self.overlays(ctx, mode, locale, now);
    }

    // ============================================================ 重ねるもの

    fn overlays(&mut self, ctx: &Context, mode: Mode, locale: Locale, now: f64) {
        let colors = theme::palette(mode);
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        let mut close = false;
        match &mut self.overlay {
            Overlay::None => {}
            Overlay::Palette { input, selected } => {
                let (mode_kind, query) = palette::split(input);
                let items = palette_items(&session.bench, locale, mode_kind, query);
                let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
                let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                if down && *selected + 1 < items.len() {
                    *selected += 1;
                }
                if up {
                    *selected = selected.saturating_sub(1);
                }
                let mut chosen = None;
                popup(ctx, "palette", colors, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(input)
                            .hint_text(i18n::text(locale, "palette.placeholder"))
                            .desired_width(520.0),
                    );
                    edit.request_focus();
                    if edit.changed() {
                        *selected = 0;
                    }
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for (index, item) in items.iter().enumerate() {
                                let text =
                                    RichText::new(&item.label).color(if index == *selected {
                                        colors.accent
                                    } else {
                                        colors.fg_primary
                                    });
                                if ui.selectable_label(index == *selected, text).clicked() {
                                    chosen = Some(index);
                                }
                            }
                        });
                });
                if enter {
                    chosen = Some(*selected);
                }
                if let Some(index) = chosen {
                    if let Some(item) = items.get(index) {
                        match &item.action {
                            PaletteAction::Section(at) => {
                                session.bench.go_to(*at);
                                session.settings_dirty = true;
                            }
                            PaletteAction::Command(command) => {
                                let command = *command;
                                self.overlay = Overlay::None;
                                self.run(ctx, command, now);
                                return;
                            }
                        }
                    }
                    close = true;
                }
            }
            Overlay::Jump { input, missed } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                popup(ctx, "jump", colors, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(input)
                            .hint_text("@01-04-02-03")
                            .desired_width(240.0),
                    );
                    edit.request_focus();
                    if *missed {
                        ui.label(
                            RichText::new(i18n::text(locale, "jump.not_found")).color(colors.error),
                        );
                    }
                });
                if enter {
                    if session.bench.jump(input) {
                        session.settings_dirty = true;
                        close = true;
                    } else {
                        *missed = true;
                    }
                }
            }
            Overlay::Help => {
                popup(ctx, "help", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "help.title")).color(colors.fg_faint),
                    );
                    ui.add_space(6.0);
                    egui::Grid::new("help-keys")
                        .num_columns(2)
                        .spacing([16.0, 4.0])
                        .show(ui, |ui| {
                            for (keys, description) in HELP_KEYS {
                                ui.label(
                                    RichText::new(*keys)
                                        .font(FontId::new(12.0, FontFamily::Monospace))
                                        .color(colors.fg_muted),
                                );
                                ui.label(i18n::text(locale, description));
                                ui.end_row();
                            }
                        });
                });
            }
            Overlay::SaveError => {
                let message = session.bench.save.error().unwrap_or_default().to_owned();
                popup(ctx, "save-error", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "status.save_failed")).color(colors.error),
                    );
                    ui.label(i18n::text(locale, "status.dont_close"));
                    ui.add_space(4.0);
                    ui.label(RichText::new(&message).color(colors.fg_muted));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(i18n::text(locale, "modal.retry")).clicked() {
                            session.bench.save.request_now();
                            close = true;
                        }
                        if ui.button(i18n::text(locale, "modal.close")).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Overlay::CloseGuard => {
                popup(ctx, "close-guard", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "status.save_failed")).color(colors.error),
                    );
                    ui.label(i18n::text(locale, "status.dont_close"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(i18n::text(locale, "modal.discard_and_close"))
                            .clicked()
                        {
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            close = true;
                        }
                        if ui.button(i18n::text(locale, "modal.retry")).clicked() {
                            session.bench.save.request_now();
                            close = true;
                        }
                        if ui.button(i18n::text(locale, "modal.cancel")).clicked() {
                            close = true;
                        }
                    });
                });
            }
        }
        if close {
            self.overlay = Overlay::None;
            ctx.memory_mut(|memory| {
                memory.request_focus(section_editor::editor_id(&session.bench))
            });
        }
    }

    /// 終了ガード（§8.4）: 未保存なら止めて保存し、保存が終わったら閉じる。失敗中は 3 択。
    fn close_guard(&mut self, ctx: &Context, now: f64) {
        let requested = ctx.input(|input| input.viewport().close_requested());
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        if self.allow_close {
            return;
        }
        if requested {
            session.bench.commit();
            session.bench.save.request_now();
            save_if_due(session, &mut self.settings, now);
            if session.bench.save.guards_close() || session.bench.unsaved().next().is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if session.bench.save.state() == State::Failed {
                    self.overlay = Overlay::CloseGuard;
                } else {
                    // 保存が済んだら閉じる（次のフレームで）
                    ctx.request_repaint();
                    ctx.data_mut(|data| data.insert_temp(Id::new("close-after-save"), true));
                }
                return;
            }
            flush_settings(session, &mut self.settings);
        }
        let pending = ctx.data(|data| {
            data.get_temp::<bool>(Id::new("close-after-save"))
                .unwrap_or(false)
        });
        if pending && !session.bench.save.guards_close() && session.bench.unsaved().next().is_none()
        {
            flush_settings(session, &mut self.settings);
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// 保存の時刻なら書く（§8.2）。`ui.json` の変更も一緒に書く。
fn save_if_due(session: &mut Session, settings: &mut UiSettings, now: f64) {
    if session.bench.save.due(now) {
        let writes = session.bench.take_writes();
        session.bench.save.begin(now);
        let mut written = Vec::new();
        let mut failures = Vec::new();
        for (path, text) in writes {
            match session.host.write(&path, &text) {
                Ok(()) => written.push(path),
                Err(message) => failures.push((path, message)),
            }
        }
        session.bench.write_finished(now, &written, &failures);
        if failures.is_empty() {
            flush_settings(session, settings);
        }
    } else if session.settings_dirty && session.bench.save.state() == State::Clean {
        flush_settings(session, settings);
    }
}

/// `ui.json` を書く（プロジェクトの中、§4.4.1・§11.4）。
fn flush_settings(session: &mut Session, settings: &mut UiSettings) {
    if !session.settings_dirty {
        return;
    }
    settings.last_scene = session.bench.last_scene();
    if session
        .host
        .write(crate::ui::settings::UI_JSON, &settings.to_json())
        .is_ok()
    {
        session.settings_dirty = false;
    }
}

/// ステータスバー（§9.6.1）。保存インジケータが押されたら `true`。
fn status_bar(
    ui: &mut egui::Ui,
    session: &mut Session,
    locale: Locale,
    mode: Mode,
    scale: f32,
    indicator: Indicator,
    now: f64,
) -> bool {
    let colors = theme::palette(mode);
    let font = FontId::new(11.0 * scale, FontFamily::Proportional);
    let mono = FontId::new(11.0 * scale, FontFamily::Monospace);
    let mut clicked = false;
    ui.horizontal_centered(|ui| {
        let dot = || {
            RichText::new("  ·  ")
                .font(font.clone())
                .color(colors.fg_faint)
        };
        if let Some(crumb) = session.bench.breadcrumb() {
            ui.label(
                RichText::new(crumb)
                    .font(mono.clone())
                    .color(colors.fg_muted),
            );
            ui.label(dot());
        }
        let (section, episode, project) = session.bench.counts();
        let unit = i18n::text(locale, "status.chars_unit");
        ui.label(
            RichText::new(format!(
                "{}{unit} / {}{unit} / {}{unit}",
                context_view::group(section),
                context_view::group(episode),
                context_view::group(project)
            ))
            .font(mono.clone())
            .color(colors.fg_muted),
        );
        ui.label(dot());
        let page = session
            .bench
            .project()
            .settings()
            .writing
            .manuscript_page_chars
            .max(1) as f64;
        ui.label(
            RichText::new(format!(
                "{:.1}{}",
                episode as f64 / page,
                i18n::text(locale, "status.pages_unit")
            ))
            .font(mono.clone())
            .color(colors.fg_muted),
        );
        // 診断件数（E + W、今のファイル。0 件なら出さない）
        let count = diagnostic_count(session);
        if count > 0 {
            ui.label(dot());
            ui.label(
                RichText::new(format!("⚑{count}"))
                    .font(mono.clone())
                    .color(colors.warn),
            );
        }
        if session.bench.conflicts().next().is_some() {
            ui.label(dot());
            ui.label(RichText::new("⇄").font(mono.clone()).color(colors.error))
                .on_hover_text(i18n::text(locale, "status.conflict"));
        }
        if let Some((message, at)) = &session.notice {
            if now - at < 4.0 {
                ui.label(dot());
                ui.label(
                    RichText::new(message)
                        .font(font.clone())
                        .color(colors.fg_faint),
                );
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::click());
            let painter = ui.painter_at(rect.expand(2.0));
            let center = rect.center();
            match indicator {
                Indicator::Nothing => {}
                Indicator::Unsaved => {
                    painter.circle_filled(center, 4.0, colors.unsaved);
                }
                Indicator::Spinner => {
                    let angle = (now * 6.0) as f32;
                    let points: Vec<Pos2> = (0..12)
                        .map(|step| {
                            let t = angle + step as f32 * 0.4;
                            center + Vec2::new(t.cos(), t.sin()) * 5.0
                        })
                        .collect();
                    painter.add(egui::Shape::line(
                        points,
                        Stroke::new(1.5_f32, colors.fg_muted),
                    ));
                    ui.ctx().request_repaint();
                }
                Indicator::Failed => {
                    let top = center + Vec2::new(0.0, -6.0);
                    let left = center + Vec2::new(-6.0, 5.0);
                    let right = center + Vec2::new(6.0, 5.0);
                    painter.add(egui::Shape::convex_polygon(
                        vec![top, right, left],
                        colors.error,
                        Stroke::NONE,
                    ));
                    painter.text(
                        center + Vec2::new(0.0, 1.0),
                        Align2::CENTER_CENTER,
                        "!",
                        FontId::new(9.0, FontFamily::Proportional),
                        colors.bg_base,
                    );
                }
            }
            let tooltip = match indicator {
                Indicator::Unsaved => Some("status.unsaved"),
                Indicator::Spinner => Some("status.saving"),
                Indicator::Failed => Some("status.save_failed"),
                Indicator::Nothing => None,
            };
            let response = match tooltip {
                Some(key) => {
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, i18n::text(locale, key))
                    });
                    response.on_hover_text(i18n::text(locale, key))
                }
                None => response,
            };
            if response.clicked() && indicator == Indicator::Failed {
                clicked = true;
            }
            // 「閉じないで」（錠前の開いた形）。Dirty / Saving / SaveFailed のときだけ（§8.4）
            if session.bench.save.guards_close() {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Label,
                        i18n::text(locale, "status.dont_close"),
                    )
                });
                let painter = ui.painter_at(rect.expand(2.0));
                let color = if indicator == Indicator::Failed {
                    colors.error
                } else {
                    colors.fg_muted
                };
                let body = Rect::from_center_size(
                    rect.center() + Vec2::new(0.0, 2.5),
                    Vec2::new(9.0, 6.0),
                );
                painter.rect_stroke(body, 1.0, Stroke::new(1.2_f32, color));
                // 開いた掛け金（左だけ下りている）
                let top = body.min.y - 4.0;
                painter.line_segment(
                    [
                        Pos2::new(body.min.x + 2.0, body.min.y),
                        Pos2::new(body.min.x + 2.0, top),
                    ],
                    Stroke::new(1.2_f32, color),
                );
                painter.line_segment(
                    [
                        Pos2::new(body.min.x + 2.0, top),
                        Pos2::new(body.max.x - 2.0, top),
                    ],
                    Stroke::new(1.2_f32, color),
                );
                painter.line_segment(
                    [
                        Pos2::new(body.max.x - 2.0, top),
                        Pos2::new(body.max.x - 2.0, top + 2.0),
                    ],
                    Stroke::new(1.2_f32, color),
                );
                if indicator == Indicator::Failed {
                    ui.label(
                        RichText::new(i18n::text(locale, "status.dont_close"))
                            .font(font.clone())
                            .color(colors.error),
                    );
                }
            }
        });
    });
    clicked
}

fn diagnostic_count(session: &mut Session) -> usize {
    let Some(current) = session.bench.current() else {
        return 0;
    };
    let key_path = current.path.clone();
    if let Some((generation, path, count)) = &session.diag_count {
        if *generation == session.bench.generation() && path == &key_path {
            return *count;
        }
    }
    let count = session
        .bench
        .project()
        .document(&key_path)
        .map(|document| {
            crate::core::diagnostics::document(document, session.bench.project())
                .iter()
                .filter(|found| {
                    matches!(
                        found.severity(),
                        crate::core::diagnostics::Severity::Error
                            | crate::core::diagnostics::Severity::Warning
                    )
                })
                .count()
        })
        .unwrap_or(0);
    session.diag_count = Some((session.bench.generation(), key_path, count));
    count
}

fn focus_ring(ctx: &Context, rect: Rect, color: Color32) {
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("focus-ring"),
    ))
    .rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(2.0_f32, color));
}

fn theme_symbol(choice: ThemeChoice) -> &'static str {
    match choice {
        ThemeChoice::System => "◐",
        ThemeChoice::Dark => "☾",
        ThemeChoice::Light => "☀",
    }
}

/// 画面上部の中央に出す小窓（パレット・ジャンプ・ヘルプ・モーダル）。背景を暗くする。
fn popup(ctx: &Context, id: &str, colors: theme::Palette, add: impl FnOnce(&mut egui::Ui)) {
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, Id::new("dim")))
        .rect_filled(screen, 0.0, Color32::from_black_alpha(96));
    egui::Area::new(Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 72.0))
        .show(ctx, |ui| {
            Frame::none()
                .fill(colors.bg_raised)
                .stroke(Stroke::new(1.0_f32, colors.border))
                .inner_margin(Margin::same(12.0))
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    add(ui);
                });
        });
}

/// パレットの候補 1 つ。
struct PaletteItem {
    key: String,
    label: String,
    action: PaletteAction,
}

enum PaletteAction {
    Section(usize),
    Command(Command),
}

/// パレットのコマンド（`>`）。キーは英語の固定名（§9.7.1 の例 `>theme`）。
const PALETTE_COMMANDS: &[(&str, &str, Command)] = &[
    ("theme", "command.theme", Command::CycleTheme),
    ("language", "command.language", Command::ToggleLanguage),
    ("save", "command.save", Command::SaveNow),
    ("reload", "command.reload", Command::Reload),
    ("new-section", "command.new_section", Command::NewSection),
    ("jump", "command.jump", Command::Jump),
    ("reference", "command.reference", Command::ToggleReference),
    ("help", "command.help", Command::Help),
];

fn palette_items(
    bench: &Workbench,
    locale: Locale,
    mode: palette::Mode,
    query: &str,
) -> Vec<PaletteItem> {
    match mode {
        palette::Mode::Command => {
            let all: Vec<PaletteItem> = PALETTE_COMMANDS
                .iter()
                .map(|(name, key, command)| PaletteItem {
                    key: (*name).to_owned(),
                    label: format!(">{name}  {}", i18n::text(locale, key)),
                    action: PaletteAction::Command(*command),
                })
                .collect();
            let ranked = palette::rank(query, &all, |item| (item.key.as_str(), item.key.as_str()));
            ranked
                .into_iter()
                .map(|item| PaletteItem {
                    key: item.key.clone(),
                    label: item.label.clone(),
                    action: match item.action {
                        PaletteAction::Command(command) => PaletteAction::Command(command),
                        PaletteAction::Section(at) => PaletteAction::Section(at),
                    },
                })
                .collect()
        }
        palette::Mode::Help => vec![PaletteItem {
            key: "help".into(),
            label: format!("?  {}", i18n::text(locale, "command.help")),
            action: PaletteAction::Command(Command::Help),
        }],
        palette::Mode::Fuzzy | palette::Mode::Scene => {
            let digits = bench.project().digits();
            let all: Vec<PaletteItem> = bench
                .sections()
                .iter()
                .enumerate()
                .map(|(index, section)| {
                    let id = section.id.display(digits);
                    PaletteItem {
                        label: format!("@{id}  {}", section.scene_title),
                        key: id,
                        action: PaletteAction::Section(index),
                    }
                })
                .collect();
            let ranked =
                palette::rank(query, &all, |item| (item.key.as_str(), item.label.as_str()));
            ranked
                .into_iter()
                .map(|item| PaletteItem {
                    key: item.key.clone(),
                    label: item.label.clone(),
                    action: match item.action {
                        PaletteAction::Section(at) => PaletteAction::Section(at),
                        PaletteAction::Command(command) => PaletteAction::Command(command),
                    },
                })
                .collect()
        }
        // 辞書・資料の検索は M8（§10.5・§16.3、`PLANS.md` §9.2 B-117）
        palette::Mode::Dictionary | palette::Mode::Material => Vec::new(),
    }
}

/// キーボードヘルプ（§9.8 のうち M7 で動くもの）。左はキー、右はカタログのキー。
const HELP_KEYS: &[(&str, &str)] = &[
    ("Ctrl+P", "help.palette"),
    ("Ctrl+G", "help.jump"),
    ("Ctrl+S", "help.save"),
    ("Alt+↑ / Alt+↓", "help.section"),
    ("Alt+← / Alt+→", "help.scene"),
    ("Ctrl+Alt+← / →", "help.file"),
    ("Ctrl+Enter", "help.confirm_next"),
    ("Ctrl+;", "help.new_section"),
    ("Ctrl+1 / 2 / 3", "help.focus"),
    ("Ctrl+\\", "help.reference"),
    ("Ctrl+Shift+\\", "help.swap"),
    ("Ctrl+Alt+↑ / ↓", "help.context_ratio"),
    ("Ctrl+Alt+Shift+← / →", "help.main_ratio"),
    ("Ctrl+Plus / Minus / 0", "help.font"),
    ("Ctrl+Alt+L", "help.language"),
    ("Ctrl+Alt+T", "help.theme"),
    ("F5", "help.reload"),
    ("Esc", "help.escape"),
];

// ============================================================ デスクトップ

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use egui::{Context, FontFamily, FontId, Frame, Key, Margin, Modifiers, RichText, Vec2};

    use super::{Host, NovnowApp, Picker, Session};
    use crate::adapters::block_on;
    use crate::adapters::clock_system::SystemClock;
    use crate::adapters::dialog_rfd::RfdDialog;
    use crate::adapters::env_system::SystemEnv;
    use crate::adapters::project_load;
    use crate::adapters::recent_file::RecentFile;
    use crate::adapters::storage_fs::{FsGate, FsStorage};
    use crate::cli::{self, Cli, Terminal};
    use crate::core::model::project::ProjectError;
    use crate::core::policy::external_change::{ChangeKind, Snapshot};
    use crate::core::policy::ProjectPath;
    use crate::ports::dialog::Dialog;
    use crate::ui::i18n::{self, Locale};
    use crate::ui::settings::{UiSettings, UI_JSON};
    use crate::ui::theme::{self, Mode};

    /// GUI を起動する（`main.rs` の `Outcome::Gui`）。`project` は起動前の検査を通ったディレクトリ。
    pub fn run(project: Option<PathBuf>) -> Result<(), String> {
        let env = SystemEnv;
        let env_locale = Locale::detect(None, &env);
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("novnow")
                .with_inner_size([1280.0, 800.0])
                .with_min_inner_size([640.0, 400.0]),
            follow_system_theme: true,
            // 規則 10: ウィンドウの位置・大きさを覚えない（persistence feature も使わない）
            persist_window: false,
            ..Default::default()
        };
        eframe::run_native(
            "novnow",
            options,
            Box::new(move |cc| {
                let picker = Box::new(NativePicker::new(env_locale));
                let opened = project.and_then(|root| open_project(&root).ok());
                Box::new(App(NovnowApp::new(
                    &cc.egui_ctx,
                    env_locale,
                    picker,
                    opened,
                )))
            }),
        )
        .map_err(|error| error.to_string())
    }

    struct App(NovnowApp);

    impl eframe::App for App {
        fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
            let system_dark = frame
                .info()
                .system_theme
                .map(|theme| theme == eframe::Theme::Dark);
            self.0.frame(ctx, system_dark);
        }
    }

    /// デスクトップのプロジェクト（`FsStorage` と外部変更の監視）。
    struct FsHost {
        storage: FsStorage,
        snapshot: Snapshot,
    }

    impl Host for FsHost {
        fn write(&mut self, path: &str, text: &str) -> Result<(), String> {
            let path = ProjectPath::new(path).map_err(|error| format!("{error:?}"))?;
            let stamp = self
                .storage
                .write_bytes(&path, text.as_bytes())
                .map_err(|error| format!("{error:?}"))?;
            self.snapshot.acknowledge(path, stamp);
            Ok(())
        }

        fn poll(&mut self) -> Vec<(String, Option<String>)> {
            let Ok(changes) = block_on(project_load::poll(&self.storage, &mut self.snapshot))
            else {
                return Vec::new();
            };
            changes
                .into_iter()
                .filter(|change| project_load::is_document(&change.path))
                .map(|change| {
                    let text = match change.kind {
                        ChangeKind::Removed => None,
                        _ => self
                            .storage
                            .read_bytes(&change.path)
                            .ok()
                            .and_then(|bytes| String::from_utf8(bytes).ok())
                            .map(|text| {
                                crate::core::text::lexer::normalize_on_load(&text).into_owned()
                            }),
                    };
                    (change.path.as_str().to_owned(), text)
                })
                .collect()
        }

        fn name(&self) -> String {
            self.storage
                .root()
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        }
    }

    /// ディレクトリを開く。開けなければ §14.2.2 と同じ表示の文字列。
    fn open_project(root: &Path) -> Result<(Session, UiSettings), String> {
        let gate = Arc::new(FsGate::new(&SystemEnv));
        let storage = FsStorage::open(gate.clone(), root).map_err(|error| format!("{error:?}"))?;
        let loaded =
            block_on(project_load::load(&storage)).map_err(|error| format!("{error:?}"))?;
        if let Err(error) = &loaded.validation {
            return Err(format!("{error:?}"));
        }
        let settings = ProjectPath::new(UI_JSON)
            .ok()
            .and_then(|path| storage.read_bytes(&path).ok())
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(|text| UiSettings::parse(&text))
            .unwrap_or_default();
        let snapshot = Snapshot::from_entries(storage.list_files().unwrap_or_default());
        // パス履歴（§13.2。失敗しても開くのは止めない）
        let _ = RecentFile::new(gate).push(storage.root(), &SystemClock);
        let host = FsHost { storage, snapshot };
        let session = Session::new(Box::new(host), loaded.project, &settings);
        Ok((session, settings))
    }

    /// 起動前の検査（`cli::open::precheck`）と同じ表示を文字列で受け取る。
    fn precheck_report(dir: &Path, locale: Locale) -> Result<PathBuf, (String, bool)> {
        let env = SystemEnv;
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let terminal = Terminal {
            stdin: false,
            stdout: false,
            stderr: false,
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let incomplete = {
            let gate = Arc::new(FsGate::new(&env));
            FsStorage::open(gate, dir)
                .ok()
                .and_then(|storage| block_on(project_load::listing(&storage)).ok())
                .map(|listing| {
                    matches!(
                        crate::core::model::project::validate(&listing),
                        Err(ProjectError::Incomplete { .. })
                    )
                })
                .unwrap_or(false)
        };
        let mut cli = Cli::new(&env, &mut out, &mut err, cwd, terminal);
        cli.set_locale(locale);
        let result = cli::open::precheck(&mut cli, dir);
        drop(cli);
        result.map_err(|_| (String::from_utf8_lossy(&err).into_owned(), incomplete))
    }

    /// `novnow new` と同じ雛形を作る（§14.4「ここに新規作成する」・「新規作成」）。
    fn create_here(dir: &Path, locale: Locale) -> Result<(), String> {
        let env = SystemEnv;
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let terminal = Terminal {
            stdin: false,
            stdout: false,
            stderr: false,
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut cli = Cli::new(&env, &mut out, &mut err, cwd, terminal);
        cli.set_locale(locale);
        let exit = cli::new::run(&mut cli, dir);
        drop(cli);
        if exit == cli::Exit::Ok {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&err).into_owned())
        }
    }

    /// §14.4 のプロジェクト選択（デスクトップ）。
    struct NativePicker {
        recent: Vec<(String, bool)>,
        selected: usize,
        problem: Option<(PathBuf, String, bool)>,
        want_folder: bool,
        want_new: bool,
        locale: Locale,
    }

    impl NativePicker {
        fn new(locale: Locale) -> Self {
            let gate = Arc::new(FsGate::new(&SystemEnv));
            let recent = RecentFile::new(gate)
                .load()
                .items
                .into_iter()
                .map(|item| {
                    let exists = Path::new(&item.path).is_dir();
                    (item.path, exists)
                })
                .collect();
            Self {
                recent,
                selected: 0,
                problem: None,
                want_folder: false,
                want_new: false,
                locale,
            }
        }

        fn try_open(&mut self, dir: &Path) -> Option<(Session, UiSettings)> {
            match precheck_report(dir, self.locale) {
                Ok(root) => match open_project(&root) {
                    Ok(opened) => Some(opened),
                    Err(message) => {
                        self.problem = Some((root, message, false));
                        None
                    }
                },
                Err((report, incomplete)) => {
                    self.problem = Some((dir.to_path_buf(), report, incomplete));
                    None
                }
            }
        }
    }

    impl Picker for NativePicker {
        fn open_folder(&mut self) {
            self.want_folder = true;
        }

        fn show(
            &mut self,
            ctx: &Context,
            locale: Locale,
            mode: Mode,
        ) -> Option<(Session, UiSettings)> {
            self.locale = locale;
            let colors = theme::palette(mode);
            let mut opened = None;
            if self.want_folder || self.want_new {
                let new = self.want_new;
                self.want_folder = false;
                self.want_new = false;
                if let Some(dir) = block_on(RfdDialog.pick_folder()) {
                    if new {
                        match create_here(&dir, locale) {
                            Ok(()) => opened = self.try_open(&dir),
                            Err(report) => self.problem = Some((dir, report, false)),
                        }
                    } else {
                        opened = self.try_open(&dir);
                    }
                }
            }
            if opened.is_some() {
                return opened;
            }
            // ↑/↓ + Enter（§14.4）
            let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
            let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
            let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
            if down && self.selected + 1 < self.recent.len() {
                self.selected += 1;
            }
            if up {
                self.selected = self.selected.saturating_sub(1);
            }
            let mut choose: Option<PathBuf> = None;
            if enter {
                if let Some((path, true)) = self.recent.get(self.selected) {
                    choose = Some(PathBuf::from(path));
                }
            }
            let mut create_at: Option<PathBuf> = None;
            egui::CentralPanel::default()
                .frame(
                    Frame::none()
                        .fill(colors.bg_base)
                        .inner_margin(Margin::same(32.0)),
                )
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.set_max_width(620.0);
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new(i18n::text(locale, "project.pick.title"))
                                .font(FontId::new(18.0, FontFamily::Proportional))
                                .color(colors.fg_primary),
                        );
                        ui.add_space(16.0);
                        if self.recent.is_empty() {
                            ui.label(
                                RichText::new(i18n::text(locale, "project.pick.empty"))
                                    .color(colors.fg_faint),
                            );
                        }
                        for (index, (path, exists)) in self.recent.iter().enumerate() {
                            let name = Path::new(path)
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| path.clone());
                            let shown = if *exists {
                                format!("{name}    {path}")
                            } else {
                                format!("{}    {path}", i18n::text(locale, "project.pick.missing"))
                            };
                            let color = if !exists {
                                colors.fg_faint
                            } else if index == self.selected {
                                colors.accent
                            } else {
                                colors.fg_primary
                            };
                            let response = ui.selectable_label(
                                index == self.selected,
                                RichText::new(shown).color(color),
                            );
                            if response.clicked() && *exists {
                                choose = Some(PathBuf::from(path));
                            }
                        }
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.add_space((ui.available_width() - 320.0).max(0.0) / 2.0);
                            if ui
                                .button(format!("📂 {}", i18n::text(locale, "project.pick.open")))
                                .clicked()
                            {
                                self.want_folder = true;
                            }
                            ui.add_space(12.0);
                            if ui
                                .button(format!("＋ {}", i18n::text(locale, "project.pick.new")))
                                .clicked()
                            {
                                self.want_new = true;
                            }
                        });
                        if let Some((dir, report, incomplete)) = &self.problem {
                            ui.add_space(20.0);
                            Frame::none()
                                .fill(colors.bg_inset)
                                .inner_margin(Margin::same(12.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    for line in report.lines() {
                                        if let Some(start) = line.find("https://") {
                                            let url = line[start..].trim();
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(&line[..start])
                                                        .color(colors.fg_muted),
                                                );
                                                ui.hyperlink(url);
                                            });
                                        } else {
                                            ui.label(RichText::new(line).color(colors.fg_muted));
                                        }
                                    }
                                    if *incomplete {
                                        ui.add_space(8.0);
                                        // 勝手に作らない（明示操作が必要、§14.4）
                                        if ui
                                            .button(i18n::text(locale, "project.pick.create_here"))
                                            .clicked()
                                        {
                                            create_at = Some(dir.clone());
                                        }
                                    }
                                });
                        }
                    });
                });
            if let Some(dir) = create_at {
                match create_here(&dir, locale) {
                    Ok(()) => {
                        self.problem = None;
                        return self.try_open(&dir);
                    }
                    Err(report) => self.problem = Some((dir, report, false)),
                }
            }
            if self.want_folder || self.want_new {
                ctx.request_repaint();
            }
            let _ = Vec2::ZERO;
            choose.and_then(|dir| self.try_open(&dir))
        }
    }
}

/// メモリの `Host`（テストと、ファイルを持たない環境の確認用）。中身は `Rc` で共有するので、
/// `Session` に渡した後も外から読める。
#[derive(Debug, Default, Clone)]
pub struct MemoryHost {
    /// 共有の中身。
    pub files: std::rc::Rc<std::cell::RefCell<MemoryFiles>>,
}

/// [`MemoryHost`] の中身。
#[derive(Debug, Default)]
pub struct MemoryFiles {
    /// 書かれたファイル（パス → 中身）。
    pub written: std::collections::BTreeMap<String, String>,
    /// 書き込みの回数。
    pub writes: usize,
    /// 書き込みを失敗させる。
    pub fail: bool,
    /// 次の監視で外部変更として返すもの。
    pub changes: Vec<(String, Option<String>)>,
}

impl Host for MemoryHost {
    fn write(&mut self, path: &str, text: &str) -> Result<(), String> {
        let mut files = self.files.borrow_mut();
        if files.fail {
            return Err("disk full".into());
        }
        files.writes += 1;
        files.written.insert(path.to_owned(), text.to_owned());
        Ok(())
    }

    fn poll(&mut self) -> Vec<(String, Option<String>)> {
        std::mem::take(&mut self.files.borrow_mut().changes)
    }

    fn name(&self) -> String {
        "memory".into()
    }
}

/// 起動時に何もしない `Picker`（テスト・Web の仮置き）。
#[derive(Debug, Default)]
pub struct NoPicker;

impl Picker for NoPicker {
    fn show(&mut self, ctx: &Context, locale: Locale, mode: Mode) -> Option<(Session, UiSettings)> {
        let colors = theme::palette(mode);
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(colors.bg_base)
                    .inner_margin(Margin::same(32.0)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "project.pick.title"))
                            .color(colors.fg_primary),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "project.pick.empty"))
                            .color(colors.fg_faint),
                    );
                });
            });
        None
    }

    fn open_folder(&mut self) {}
}

/// `PathBuf` を使う公開 API の型を固定する（`main.rs` から呼ぶ）。
pub type StartPath = Option<PathBuf>;

// ============================================================ Web

/// Web の入口（trunk の `index.html` の `the_canvas_id`）。
#[cfg(target_arch = "wasm32")]
pub mod web {
    use egui::{Context, FontFamily, FontId, Frame, Margin, RichText};

    use super::{NovnowApp, Picker, Session};
    use crate::adapters::env_system::SystemEnv;
    use crate::ui::i18n::{self, Locale};
    use crate::ui::settings::UiSettings;
    use crate::ui::theme::{self, Mode};

    /// GUI を起動する（`main.rs` の wasm の `main`）。
    pub fn start() {
        console_error_panic_hook::set_once();
        let runner = eframe::WebRunner::new();
        wasm_bindgen_futures::spawn_local(async move {
            let locale = Locale::detect(None, &SystemEnv);
            let started = runner
                .start(
                    "the_canvas_id",
                    eframe::WebOptions::default(),
                    Box::new(move |cc| {
                        Box::new(App(NovnowApp::new(
                            &cc.egui_ctx,
                            locale,
                            Box::new(WebPicker),
                            None,
                        )))
                    }),
                )
                .await;
            if let Err(error) = started {
                web_sys::console::error_1(&error);
            }
        });
    }

    struct App(NovnowApp);

    impl eframe::App for App {
        fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
            let system_dark = frame
                .info()
                .system_theme
                .map(|theme| theme == eframe::Theme::Dark);
            self.0.frame(ctx, system_dark);
        }
    }

    /// Web のプロジェクト選択（§13.4）。IndexedDB への接続（`adapters::storage_idb`）は M5 で作ってあるが、
    /// 画面からの接続は M7 では入れていない（`PLANS.md` §9.2 B-118）。
    struct WebPicker;

    impl Picker for WebPicker {
        fn show(
            &mut self,
            ctx: &Context,
            locale: Locale,
            mode: Mode,
        ) -> Option<(Session, UiSettings)> {
            let colors = theme::palette(mode);
            egui::CentralPanel::default()
                .frame(
                    Frame::none()
                        .fill(colors.bg_base)
                        .inner_margin(Margin::same(32.0)),
                )
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new(i18n::text(locale, "project.pick.title"))
                                .font(FontId::new(18.0, FontFamily::Proportional))
                                .color(colors.fg_primary),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(i18n::text(locale, "project.pick.web_pending"))
                                .color(colors.fg_muted),
                        );
                    });
                });
            None
        }

        fn open_folder(&mut self) {}
    }
}
