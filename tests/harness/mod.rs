//! 画面のテスト（`tests/gui.rs`・`tests/workspaces.rs`）の共通部品: egui を**画面なし**で 1 フレームずつ
//! 回す `Harness`（`egui::Context::run`）と、描かれた文字列の収集。
//!
//! 使う側のクレートが `mod world;` も宣言すること（既定のプロジェクトは `tests/world`）。

#![allow(dead_code)]

use super::world::world;

use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};

use novnow::ui::app::{MemoryFiles, MemoryHost, NoPicker, NovnowApp, Session};
use novnow::ui::i18n::Locale;
use novnow::ui::settings::UiSettings;

pub struct Harness {
    pub ctx: egui::Context,
    pub app: NovnowApp,
    pub host: MemoryHost,
    pub time: f64,
    pub dark: bool,
}

impl Harness {
    pub fn new() -> Self {
        Self::start().0
    }

    /// 最初のフレームの出力も返す（フォントのテクスチャは最初のフレームにだけ全体が来る）。
    pub fn start() -> (Self, egui::FullOutput) {
        Self::start_with(world(), true)
    }

    pub fn start_with(
        project: novnow::core::features::Project,
        dark: bool,
    ) -> (Self, egui::FullOutput) {
        Self::open(project, dark, MemoryHost::default(), false)
    }

    /// エージェントの画面のテスト: `setup` で `MemoryHost`（LLM の台本・環境変数）を整えてから開き、
    /// デスクトップの `open_project` と同じく `Session::prepare_agents` を呼ぶ（欠けたプロンプトを書く）。
    pub fn start_agents(
        project: novnow::core::features::Project,
        setup: impl FnOnce(&mut MemoryFiles),
    ) -> (Self, egui::FullOutput) {
        let host = MemoryHost::default();
        setup(&mut host.files.borrow_mut());
        Self::open(project, true, host, true)
    }

    fn open(
        project: novnow::core::features::Project,
        dark: bool,
        host: MemoryHost,
        agents: bool,
    ) -> (Self, egui::FullOutput) {
        let ctx = egui::Context::default();
        let settings = UiSettings::default();
        let mut session = Session::new(Box::new(host.clone()), project, &settings);
        if agents {
            session.prepare_agents(Locale::Ja);
        }
        let app = NovnowApp::new(
            &ctx,
            Locale::Ja,
            Box::new(NoPicker),
            Some((session, settings)),
        );
        let mut harness = Self {
            ctx,
            app,
            host,
            time: 0.0,
            dark,
        };
        let first = harness.frame(Vec::new());
        (harness, first)
    }

    pub fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            time: Some(self.time),
            events,
            focused: true,
            ..Default::default()
        };
        let app = &mut self.app;
        let dark = self.dark;
        self.ctx.run(input, |ctx| app.frame(ctx, Some(dark)))
    }

    pub fn idle(&mut self, seconds: f64) -> egui::FullOutput {
        let mut last = self.frame(Vec::new());
        let end = self.time + seconds;
        while self.time < end {
            last = self.frame(Vec::new());
        }
        last
    }
}

/// フレームに描かれた文字列を全部集める。
pub fn painted_text(output: &egui::FullOutput) -> String {
    fn walk(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(text) => {
                out.push_str(text.galley.text());
                out.push('\n');
            }
            egui::Shape::Vec(shapes) => {
                for inner in shapes {
                    walk(inner, out);
                }
            }
            _ => {}
        }
    }
    let mut out = String::new();
    for clipped in &output.shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

/// キーを押すイベント。
pub fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
