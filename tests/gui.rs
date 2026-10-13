//! GUI（M7、`ui` feature）のテスト。egui を**画面なし**で回す（`egui::Context::run`）。
//!
//! 状態の規則（保存ステートマシン・`ui.json`・パレットの採点・キー割当・IME の判定・節単位編集）は
//! egui の画面を使わずに確かめ、画面の MUST（文脈ビューに編集中の本文を描かない・変換中は
//! ショートカットが効かない・保存は 400ms の idle の後）は `NovnowApp` を 1 フレームずつ回して確かめる。

#![cfg(feature = "ui")]

mod world;

use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};

use novnow::core::id::SceneId;
use novnow::ui::app::{MemoryHost, NoPicker, NovnowApp, Session};
use novnow::ui::i18n::Locale;
use novnow::ui::palette;
use novnow::ui::save_machine::{Indicator, SaveMachine, State};
use novnow::ui::settings::{LanguageChoice, ThemeChoice, UiSettings};
use novnow::ui::shell::{self, Command, ImeState};
use novnow::ui::workbench::{Nav, Reaction, Workbench};

use world::{world, world_with, CONTENTS_PATH};

fn id(chapter: u16, episode: u16, scene: u16, section: u16) -> SceneId {
    SceneId {
        chapter,
        episode,
        scene,
        section,
    }
}

// ============================================================ 保存ステートマシン（§8.2）

#[test]
fn saving_waits_for_400ms_of_idle_and_restarts_the_timer_on_each_edit() {
    let mut machine = SaveMachine::default();
    assert_eq!(machine.state(), State::Clean);
    assert_eq!(machine.indicator(0.0), Indicator::Nothing);
    machine.edited(1.0);
    assert_eq!(machine.state(), State::Dirty);
    assert_eq!(machine.indicator(1.0), Indicator::Unsaved);
    assert!(!machine.due(1.3));
    machine.edited(1.3);
    assert!(!machine.due(1.6), "追加編集でタイマ再起動");
    assert!(machine.due(1.7));
    machine.begin(1.7);
    assert_eq!(
        machine.indicator(1.8),
        Indicator::Nothing,
        "300ms 未満はスピナーを出さない"
    );
    assert_eq!(machine.indicator(2.0), Indicator::Spinner);
    machine.succeeded();
    assert_eq!(machine.state(), State::Clean);
    assert!(!machine.guards_close());
}

#[test]
fn moving_or_ctrl_s_saves_at_once_and_edits_while_saving_stay_dirty() {
    let mut machine = SaveMachine::new(400);
    machine.request_now();
    assert!(!machine.due(0.0), "変更が無ければ何もしない");
    machine.edited(5.0);
    machine.request_now();
    assert!(machine.due(5.01), "即時（節移動・Ctrl+S）");
    machine.begin(5.01);
    machine.edited(5.02);
    machine.succeeded();
    assert_eq!(
        machine.state(),
        State::Dirty,
        "保存中の編集はまだ書かれていない"
    );
}

#[test]
fn failures_retry_with_exponential_backoff_up_to_8_seconds() {
    let mut machine = SaveMachine::new(400);
    machine.edited(0.0);
    let mut now = 1.0;
    let mut waits = Vec::new();
    for _ in 0..6 {
        assert!(machine.due(now));
        machine.begin(now);
        machine.failed(now, "disk full");
        let retry = machine.retry_at().expect("retry");
        waits.push(retry - now);
        assert!(!machine.due(retry - 0.01));
        now = retry;
    }
    assert_eq!(waits, vec![1.0, 2.0, 4.0, 8.0, 8.0, 8.0]);
    assert_eq!(machine.indicator(now), Indicator::Failed);
    assert!(machine.guards_close());
    assert_eq!(machine.error(), Some("disk full"));
    machine.begin(now);
    machine.succeeded();
    assert_eq!(machine.state(), State::Clean);
}

#[test]
fn the_idle_time_is_clamped_to_200_2000ms() {
    let mut fast = SaveMachine::new(10);
    fast.edited(0.0);
    assert!(!fast.due(0.19));
    assert!(fast.due(0.2));
    let mut slow = SaveMachine::new(60_000);
    slow.edited(0.0);
    assert!(slow.due(2.0));
}

// ============================================================ ui.json（§4.4.1）

#[test]
fn a_broken_or_missing_ui_json_gives_the_defaults() {
    for text in ["", "{", "[]", "null", "{\"theme\": 3}"] {
        let settings = UiSettings::parse(text);
        assert_eq!(settings.theme, ThemeChoice::System, "{text}");
        assert_eq!(settings.language, LanguageChoice::System);
        assert_eq!(settings.context_sections, 3);
        assert_eq!(settings.autosave_ms, 400);
    }
}

#[test]
fn ui_json_keeps_unknown_keys_and_clamps_values() {
    let text = r#"{"schema_version":1,"theme":"light","language":"en","font_scale":9,
        "pane_ratio":{"main":0.1,"context_view":0.5},"context_sections":12,"autosave_ms":50,
        "folded":{"outline":["@02"]},"future_key":true}"#;
    let mut settings = UiSettings::parse(text);
    assert_eq!(settings.theme, ThemeChoice::Light);
    assert_eq!(settings.language, LanguageChoice::En);
    assert_eq!(settings.font_scale, 1.6);
    assert_eq!(settings.main_ratio, 0.3);
    assert_eq!(settings.context_sections, 3, "範囲外は既定値");
    assert_eq!(settings.autosave_ms, 200);
    settings.theme = settings.theme.next();
    settings.last_scene = Some("01-04-02-03".into());
    let written = settings.to_json();
    let value: serde_json::Value = serde_json::from_str(&written).expect("json");
    assert_eq!(value["theme"], "system");
    assert_eq!(value["future_key"], true, "知らないキーを残す");
    assert_eq!(value["folded"]["outline"][0], "@02");
    assert_eq!(value["last_scene"], "01-04-02-03");
    assert!(written.ends_with('\n'));
    assert_eq!(UiSettings::parse(&written).theme, ThemeChoice::System);
}

#[test]
fn the_theme_cycles_system_dark_light_and_font_steps_are_bounded() {
    assert_eq!(ThemeChoice::System.next(), ThemeChoice::Dark);
    assert_eq!(ThemeChoice::Dark.next(), ThemeChoice::Light);
    assert_eq!(ThemeChoice::Light.next(), ThemeChoice::System);
    let mut settings = UiSettings::default();
    for _ in 0..20 {
        settings.step_font(true);
    }
    assert_eq!(settings.font_scale, 1.6);
    for _ in 0..20 {
        settings.step_font(false);
    }
    assert_eq!(settings.font_scale, 0.8);
    settings.step_ratio(true, true);
    assert_eq!(settings.main_ratio, 0.67);
}

// ============================================================ パレット（§9.7.1）

#[test]
fn the_first_character_chooses_the_palette_mode() {
    assert_eq!(palette::split(">theme"), (palette::Mode::Command, "theme"));
    assert_eq!(palette::split("@01-04"), (palette::Mode::Scene, "01-04"));
    assert_eq!(
        palette::split("'リリア"),
        (palette::Mode::Dictionary, "リリア")
    );
    assert_eq!(palette::split("M0137"), (palette::Mode::Material, "M0137"));
    assert_eq!(palette::split("#取材"), (palette::Mode::Material, "取材"));
    assert_eq!(palette::split("?"), (palette::Mode::Help, ""));
    assert_eq!(palette::split("Meiryo"), (palette::Mode::Fuzzy, "Meiryo"));
    assert_eq!(palette::split("0104"), (palette::Mode::Fuzzy, "0104"));
}

#[test]
fn fuzzy_scores_follow_the_fixed_rules() {
    // 先頭一致 +3、連続 +10
    assert_eq!(palette::score("ab", "abc"), Some(13));
    // 語境界 +6（`-` の直後）
    assert_eq!(palette::score("b", "a-b"), Some(6 - 1));
    // スキップ文字 -1
    assert_eq!(palette::score("ac", "abc"), Some(3 - 1));
    assert_eq!(palette::score("x", "abc"), None);
    // 区切りを読み飛ばす: `0104` は `01-04` に連続で当たる
    let compact = palette::score("0104", "@01-04-02").expect("hit");
    let scattered = palette::score("0104", "@01-02-04").expect("hit");
    assert!(compact > scattered, "{compact} > {scattered}");
}

#[test]
fn ranking_is_stable_by_key_and_limited_to_50() {
    let items: Vec<(String, String)> = (0..80)
        .rev()
        .map(|n| (format!("{n:03}"), "same".to_owned()))
        .collect();
    let ranked = palette::rank("s", &items, |item| (item.0.as_str(), item.1.as_str()));
    assert_eq!(ranked.len(), palette::LIMIT);
    assert_eq!(ranked[0].0, "000");
    assert_eq!(ranked[1].0, "001");
}

// ============================================================ キー割当・IME（§9.8）

#[test]
fn keys_map_to_the_v1_table() {
    let none = Modifiers::NONE;
    let alt = Modifiers::ALT;
    let ctrl = Modifiers::CTRL;
    let ctrl_alt = Modifiers {
        ctrl: true,
        alt: true,
        ..Modifiers::NONE
    };
    let ctrl_shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    let ctrl_alt_shift = Modifiers {
        ctrl: true,
        alt: true,
        shift: true,
        ..Modifiers::NONE
    };
    let of = |key, modifiers| shell::command_of(key, modifiers, false, true);
    assert_eq!(of(Key::P, ctrl), Some(Command::Palette));
    assert_eq!(of(Key::G, ctrl), Some(Command::Jump));
    assert_eq!(of(Key::S, ctrl), Some(Command::SaveNow));
    assert_eq!(of(Key::ArrowUp, alt), Some(Command::Nav(Nav::PrevSection)));
    assert_eq!(
        of(Key::ArrowDown, alt),
        Some(Command::Nav(Nav::NextSection))
    );
    assert_eq!(of(Key::ArrowLeft, alt), Some(Command::Nav(Nav::PrevScene)));
    assert_eq!(of(Key::ArrowRight, alt), Some(Command::Nav(Nav::NextScene)));
    assert_eq!(
        of(Key::ArrowLeft, ctrl_alt),
        Some(Command::Nav(Nav::PrevFile))
    );
    assert_eq!(
        of(Key::ArrowRight, ctrl_alt),
        Some(Command::Nav(Nav::NextFile))
    );
    assert_eq!(
        of(Key::ArrowUp, ctrl_alt),
        Some(Command::ContextRatio(true))
    );
    assert_eq!(
        of(Key::ArrowRight, ctrl_alt_shift),
        Some(Command::MainRatio(true))
    );
    assert_eq!(of(Key::Enter, ctrl), Some(Command::ConfirmNext));
    assert_eq!(of(Key::Enter, none), None, "Enter は改行（節エディタ）");
    assert_eq!(of(Key::Semicolon, ctrl), Some(Command::NewSection));
    assert_eq!(of(Key::L, ctrl_alt), Some(Command::ToggleLanguage));
    assert_eq!(of(Key::T, ctrl_alt), Some(Command::CycleTheme));
    assert_eq!(of(Key::Backslash, ctrl), Some(Command::ToggleReference));
    assert_eq!(of(Key::Backslash, ctrl_shift), Some(Command::SwapReference));
    assert_eq!(of(Key::Num2, ctrl), Some(Command::FocusEditor));
    assert_eq!(of(Key::Num0, ctrl), Some(Command::FontReset));
    // `?` は入力欄の外だけ
    assert_eq!(of(Key::Questionmark, none), None);
    assert_eq!(
        shell::command_of(Key::Questionmark, none, false, false),
        Some(Command::Help)
    );
    // macOS: 矢印以外の Alt 系は Ctrl+Alt（矢印はそのまま）
    assert_eq!(
        shell::command_of(Key::ArrowUp, alt, true, true),
        Some(Command::Nav(Nav::PrevSection))
    );
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn ime_composition_is_tracked_with_both_failsafes() {
    let mut ime = ImeState::default();
    ime.update_with(&[Event::CompositionStart], 1.0);
    assert!(ime.composing());
    ime.update_with(&[Event::CompositionUpdate("か".into())], 1.5);
    assert!(ime.composing());
    ime.update_with(&[Event::CompositionEnd("火".into())], 1.6);
    assert!(!ime.composing());
    // フォーカスを失ったら解除
    ime.update_with(&[Event::CompositionStart], 2.0);
    ime.update_with(&[Event::WindowFocused(false)], 2.1);
    assert!(!ime.composing());
    // 2 秒何も来なければ解除。打鍵が来ている間は張り付きではない
    ime.update_with(&[Event::CompositionStart], 3.0);
    ime.update_with(&[key(Key::Space, Modifiers::NONE)], 4.5);
    ime.update_with(&[], 6.0);
    assert!(
        ime.composing(),
        "4.5 秒に打鍵があったので 6.0 秒ではまだ変換中"
    );
    ime.update_with(&[], 6.6);
    assert!(!ime.composing());
}

#[test]
fn while_composing_every_key_event_is_dropped_before_shortcuts() {
    let mut ime = ImeState::default();
    let mut input = egui::InputState::default();
    input.events = vec![
        Event::CompositionStart,
        key(Key::Num1, Modifiers::ALT),
        key(Key::Enter, Modifiers::NONE),
        key(Key::Escape, Modifiers::NONE),
        Event::Text("あ".into()),
    ];
    let commands = shell::take_commands(&mut input, &mut ime, false, true);
    assert!(commands.is_empty(), "{commands:?}");
    assert!(input
        .events
        .iter()
        .all(|event| !matches!(event, Event::Key { .. })));
    assert!(
        input.events.contains(&Event::Text("あ".into())),
        "Text は通す"
    );
    // 変換が終われば効く（ショートカットのイベントは消費される）
    input.events = vec![
        Event::CompositionEnd("亜".into()),
        key(Key::S, Modifiers::CTRL),
    ];
    let commands = shell::take_commands(&mut input, &mut ime, false, true);
    assert_eq!(commands, vec![Command::SaveNow]);
    assert!(input
        .events
        .iter()
        .all(|event| !matches!(event, Event::Key { .. })));
}

// ============================================================ 節単位編集（§8.1）

#[test]
fn the_workbench_edits_one_section_and_writes_the_whole_file() {
    let mut bench = Workbench::new(world(), None, 400);
    assert_eq!(bench.current_id(), Some(id(1, 4, 2, 1)));
    bench.buffer_mut().push_str("\n追記した一文。");
    bench.edited(0.0);
    assert!(bench.save.due(0.5));
    let writes = bench.take_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, CONTENTS_PATH);
    assert!(
        writes[0]
            .1
            .contains("引き寄せた。\n追記した一文。\n;\n「旧校舎"),
        "{}",
        writes[0].1
    );
    bench.save.begin(0.5);
    bench.write_finished(0.5, &[CONTENTS_PATH.to_owned()], &[]);
    assert_eq!(bench.save.state(), State::Clean);
    assert_eq!(bench.unsaved().count(), 0);
}

#[test]
fn moving_between_sections_commits_and_requests_an_immediate_save() {
    let mut bench = Workbench::new(world(), None, 400);
    bench.buffer_mut().insert_str(0, "冒頭。\n");
    bench.edited(10.0);
    assert!(bench.go(Nav::NextSection));
    assert_eq!(bench.current_id(), Some(id(1, 4, 2, 2)));
    assert!(bench.save.due(10.01), "節移動で即時保存");
    assert!(bench
        .project()
        .text(CONTENTS_PATH)
        .contains("冒頭。\n「やっぱり"));
    assert!(bench.go(Nav::PrevSection));
    assert!(bench.buffer().starts_with("冒頭。\n「やっぱり"));
    assert!(!bench.go(Nav::PrevSection), "最初の節より前は無い");
    assert!(bench.jump("01-04-02-03"));
    assert_eq!(bench.current_id(), Some(id(1, 4, 2, 3)));
    assert!(!bench.jump("99"));
}

#[test]
fn ctrl_enter_at_the_end_of_a_file_makes_a_new_section() {
    let mut bench = Workbench::new(world(), Some("01-04-02-03"), 400);
    assert_eq!(bench.current_id(), Some(id(1, 4, 2, 3)));
    assert!(bench.confirm_next(1.0));
    assert_eq!(bench.current_id(), Some(id(1, 4, 2, 4)));
    assert_eq!(bench.buffer(), "");
    bench.buffer_mut().push_str("新しい節。");
    bench.edited(1.1);
    let writes = bench.take_writes();
    assert!(
        writes[0]
            .1
            .ends_with("// ここでモードが切り替わる\n;\n新しい節。\n"),
        "{:?}",
        writes[0].1
    );
    // Ctrl+; は今の節の直後
    let mut bench = Workbench::new(world(), None, 400);
    assert!(bench.new_section(2.0));
    assert_eq!(bench.current_id(), Some(id(1, 4, 2, 2)));
    assert_eq!(bench.sections().len(), 4);
}

#[test]
fn external_changes_reload_clean_files_and_conflict_with_dirty_ones() {
    let mut bench = Workbench::new(world(), None, 400);
    let changed = bench
        .project()
        .text(CONTENTS_PATH)
        .replace("呆れたような声", "冷たい声");
    assert_eq!(
        bench.external_change(CONTENTS_PATH, Some(changed.clone())),
        Reaction::Reloaded
    );
    assert!(bench.buffer().contains("冷たい声"), "編集中の節も読み直す");
    assert_eq!(
        bench.external_change(CONTENTS_PATH, Some(changed.clone())),
        Reaction::Unchanged
    );
    bench.buffer_mut().push_str("未保存");
    bench.edited(0.0);
    assert_eq!(
        bench.external_change(CONTENTS_PATH, Some("@01-04-02 x\n別物\n".into())),
        Reaction::Conflict
    );
    assert_eq!(bench.conflicts().count(), 1);
    assert!(bench.buffer().ends_with("未保存"), "自動反映しない");
}

#[test]
fn counts_are_section_episode_and_project_without_whitespace() {
    let project = world_with("contents/01-05.txt", "@01-05-01 屋上\nあい う\n");
    let mut bench = Workbench::new(project, None, 400);
    let (section, episode, total) = bench.counts();
    assert!(section > 0 && episode >= section && total > episode);
    bench.buffer_mut().push_str("かきく");
    let (after_section, after_episode, after_total) = bench.counts();
    assert_eq!(after_section, section + 3);
    assert_eq!(after_episode, episode + 3);
    assert_eq!(after_total, total + 3);
    assert_eq!(bench.breadcrumb().as_deref(), Some("01›04›02›01"));
}

// ============================================================ 画面（egui を画面なしで回す）

struct Harness {
    ctx: egui::Context,
    app: NovnowApp,
    host: MemoryHost,
    time: f64,
}

impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        let host = MemoryHost::default();
        let settings = UiSettings::default();
        let session = Session::new(Box::new(host.clone()), world(), &settings);
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
        };
        harness.frame(Vec::new());
        harness
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            time: Some(self.time),
            events,
            focused: true,
            ..Default::default()
        };
        let app = &mut self.app;
        self.ctx.run(input, |ctx| app.frame(ctx, Some(true)))
    }

    fn idle(&mut self, seconds: f64) -> egui::FullOutput {
        let mut last = self.frame(Vec::new());
        let end = self.time + seconds;
        while self.time < end {
            last = self.frame(Vec::new());
        }
        last
    }
}

/// フレームに描かれた文字列を全部集める。
fn painted_text(output: &egui::FullOutput) -> String {
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

#[test]
fn the_context_view_never_paints_the_section_being_edited() {
    let mut harness = Harness::new();
    let output = harness.idle(0.2);
    let text = painted_text(&output);
    // 編集中（1 節目）の本文は節エディタにだけ出る。文脈ビュー（2・3 節目）は描かれている
    assert!(
        text.contains("図面の精巧な書き込みに目を奪われた"),
        "{text}"
    );
    let first = "呆れたような声とともに";
    assert_eq!(
        text.matches(first).count(),
        1,
        "1 節目の本文は節エディタの 1 回だけ（文脈ビューはプレースホルダ）: {text}"
    );
    assert!(text.contains("01-04-02-01"), "プレースホルダの SceneId");
    // 次の節へ移ると、今度は 2 節目が文脈ビューから消える
    harness.frame(vec![key(Key::ArrowDown, Modifiers::ALT)]);
    let text = painted_text(&harness.idle(0.2));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 4, 2, 2))
    );
    assert_eq!(
        text.matches("図面の精巧な書き込みに目を奪われた").count(),
        1,
        "{text}"
    );
    assert!(text.contains(first), "1 節目は文脈ビューに戻る");
}

#[test]
fn shortcuts_do_nothing_while_composing() {
    let mut harness = Harness::new();
    harness.frame(vec![Event::CompositionStart]);
    assert!(harness.app.composing());
    harness.frame(vec![
        Event::CompositionUpdate("せつ".into()),
        key(Key::ArrowDown, Modifiers::ALT),
        key(Key::Enter, Modifiers::CTRL),
        key(Key::P, Modifiers::CTRL),
    ]);
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 4, 2, 1)),
        "節が動かない"
    );
    assert!(!harness.app.overlay_open(), "パレットが開かない");
    // モーダルを開いてから変換を始めると、Esc で閉じない
    harness.frame(vec![Event::CompositionEnd("節".into())]);
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    assert!(harness.app.overlay_open());
    harness.frame(vec![
        Event::CompositionStart,
        key(Key::Escape, Modifiers::NONE),
    ]);
    assert!(
        harness.app.overlay_open(),
        "変換中の Esc でモーダルが閉じない"
    );
    harness.frame(vec![Event::CompositionEnd(String::new())]);
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!harness.app.overlay_open());
}

#[test]
fn the_editor_asks_the_platform_for_ime_every_frame() {
    let mut harness = Harness::new();
    // Ctrl+2 で節エディタにフォーカス
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    let output = harness.frame(Vec::new());
    assert!(
        output.platform_output.ime.is_some(),
        "§9.8.5 (5): 本文編集にフォーカスがある間は IMEOutput を出す"
    );
}

#[test]
fn typing_is_saved_after_the_idle_time_and_theme_language_go_to_ui_json() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("追".into())]);
    assert_eq!(
        harness.host.files.borrow().writes,
        0,
        "打鍵ごとには書かない"
    );
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get(CONTENTS_PATH)
        .cloned();
    let written = written.expect("400ms の idle の後に本文を書く");
    assert!(written.contains("追"), "{written}");
    // テーマと言語（ui.json はプロジェクトの中）
    let ctrl_alt = Modifiers {
        ctrl: true,
        alt: true,
        ..Modifiers::NONE
    };
    harness.frame(vec![key(Key::T, ctrl_alt)]);
    harness.frame(vec![key(Key::L, ctrl_alt)]);
    harness.idle(0.1);
    assert_eq!(harness.app.settings().theme, ThemeChoice::Dark);
    assert_eq!(harness.app.locale(), Locale::En);
    let ui_json = harness
        .host
        .files
        .borrow()
        .written
        .get(".novnow/ui.json")
        .cloned()
        .expect("ui.json");
    let value: serde_json::Value = serde_json::from_str(&ui_json).expect("json");
    assert_eq!(value["theme"], "dark");
    assert_eq!(value["language"], "en");
    assert_eq!(value["last_scene"], "01-04-02-01");
}

#[test]
fn a_failed_save_shows_the_red_indicator_and_keeps_the_text() {
    let mut harness = Harness::new();
    harness.host.files.borrow_mut().fail = true;
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("失".into())]);
    let output = harness.idle(0.6);
    let bench = harness.app.workbench().expect("open");
    assert_eq!(bench.save.state(), State::Failed);
    assert!(bench.buffer().contains('失'), "編集はメモリに残る");
    assert!(painted_text(&output).contains("書き込みが終わるまで閉じないでください"));
    harness.host.files.borrow_mut().fail = false;
    harness.idle(1.2);
    assert_eq!(
        harness.app.workbench().map(|bench| bench.save.state()),
        Some(State::Clean),
        "再試行で保存できる"
    );
}
