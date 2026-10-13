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

/// WCAG 2.x のコントラスト比（§9.9）。
fn contrast(a: egui::Color32, b: egui::Color32) -> f64 {
    let luminance = |c: egui::Color32| {
        let channel = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    };
    let (hi, lo) = {
        let (x, y) = (luminance(a), luminance(b));
        if x >= y {
            (x, y)
        } else {
            (y, x)
        }
    };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn every_token_meets_the_contrast_minimums() {
    // §9.9: 本文 4.5:1、12px 以上の補助テキストと図形 3:1。§11.1 の注記（B-121）:
    // - 文字の色（fg.* と §11.2 のトークン）は本文と同じ 4.5:1。fg.faint は 10〜11px の見出しにも使うので 4.5:1。
    // - 本文の大きさで描く注記（comment / illustration の `//`・`! `、section.break の `;`）は補助で 3:1。
    // - state.* はアイコンとステータスバーの 12px の数字なので 3:1。border は飾り（対象外）。
    use novnow::core::features::semantic_tokens::TokenType;
    use novnow::ui::theme::{palette, token_style, Mode};
    for mode in [Mode::Dark, Mode::Light] {
        let colors = palette(mode);
        // 節エディタ（枠なしの TextEdit）と文脈ビューは bg.base、ヘッダ・ステータスバー・モーダルは bg.raised
        for background in [colors.bg_base, colors.bg_raised] {
            for (name, fg) in [
                ("fg.primary", colors.fg_primary),
                ("fg.muted", colors.fg_muted),
                ("fg.faint", colors.fg_faint),
                ("accent", colors.accent),
            ] {
                let ratio = contrast(fg, background);
                assert!(ratio >= 4.5, "{mode:?} {name} {ratio:.2}");
            }
            for (name, fg) in [
                ("state.unsaved", colors.unsaved),
                ("state.error", colors.error),
                ("state.warn", colors.warn),
                ("state.ok", colors.ok),
            ] {
                let ratio = contrast(fg, background);
                assert!(ratio >= 3.0, "{mode:?} {name} {ratio:.2}");
            }
        }
        for kind in TokenType::ALL {
            let minimum = match kind {
                TokenType::Comment | TokenType::Illustration | TokenType::SectionBreak => 3.0,
                _ => 4.5,
            };
            let ratio = contrast(token_style(mode, kind).color, colors.bg_base);
            assert!(ratio >= minimum, "{mode:?} {kind:?} {ratio:.2}");
        }
        // 入力欄（パレット・検索）は bg.inset に fg.primary
        assert!(contrast(colors.fg_primary, colors.bg_inset) >= 4.5);
    }
}

#[test]
fn theme_colors_are_the_spec_tables() {
    // §11.1 / §11.2 の表が正（色を変えるときは仕様書から）。表に無い 4 種（B-115）は近い役割の色なので比べない
    use novnow::core::features::semantic_tokens::TokenType;
    use novnow::ui::theme::{palette, token_style, Mode};
    let spec = include_str!("../SPECIFICATIONS.md");
    let row = |name: &str| -> (egui::Color32, egui::Color32) {
        let prefix = format!("| `{name}`");
        let line = spec
            .lines()
            .find(|line| {
                line.starts_with(&prefix)
                    && line[prefix.len()..].starts_with([' ', '（'])
                    && line.contains("| `#")
            })
            .unwrap_or_else(|| panic!("仕様書の表に {name} の行が無い"));
        let colors: Vec<egui::Color32> = line
            .split('|')
            .filter_map(|cell| cell.trim().strip_prefix("`#")?.strip_suffix('`'))
            .map(|hex| {
                let rgb = u32::from_str_radix(hex, 16).expect("hex");
                egui::Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
            })
            .collect();
        assert_eq!(colors.len(), 2, "{line}");
        (colors[0], colors[1])
    };
    let (dark, light) = (palette(Mode::Dark), palette(Mode::Light));
    for (name, d, l) in [
        ("bg.base", dark.bg_base, light.bg_base),
        ("bg.raised", dark.bg_raised, light.bg_raised),
        ("bg.inset", dark.bg_inset, light.bg_inset),
        ("bg.selection", dark.bg_selection, light.bg_selection),
        ("fg.primary", dark.fg_primary, light.fg_primary),
        ("fg.muted", dark.fg_muted, light.fg_muted),
        ("fg.faint", dark.fg_faint, light.fg_faint),
        ("border", dark.border, light.border),
        ("accent", dark.accent, light.accent),
        ("state.unsaved", dark.unsaved, light.unsaved),
        ("state.error", dark.error, light.error),
        ("state.warn", dark.warn, light.warn),
        ("state.ok", dark.ok, light.ok),
    ] {
        assert_eq!(row(name), (d, l), "§11.1 {name}");
    }
    for kind in TokenType::ALL {
        if matches!(
            kind,
            TokenType::RefAddress
                | TokenType::Citation
                | TokenType::Illustration
                | TokenType::RubyBase
        ) {
            continue;
        }
        let want = row(kind.as_str());
        let got = (
            token_style(Mode::Dark, kind).color,
            token_style(Mode::Light, kind).color,
        );
        assert_eq!(got, want, "§11.2 {}", kind.as_str());
    }
}

#[test]
fn no_pane_is_labelled_files_editor_or_information() {
    // §19.1 / §9.2.2: ペイン見出し「ファイル」「エディタ」「情報」は撤廃。文言の一部（「最近開いたファイル」など）は
    // 見出しではないので、文言**そのもの**がこの語であるものを探す。
    for (locale, catalog, banned) in [
        (
            Locale::Ja,
            include_str!("../src/ui/i18n/ja.toml"),
            ["ファイル", "エディタ", "情報"],
        ),
        (
            Locale::En,
            include_str!("../src/ui/i18n/en.toml"),
            ["Files", "Editor", "Info"],
        ),
    ] {
        let table: toml::Table = catalog.parse().expect("catalog");
        let mut stack = vec![("".to_owned(), toml::Value::Table(table))];
        while let Some((path, value)) = stack.pop() {
            match value {
                toml::Value::Table(table) => {
                    for (key, child) in table {
                        stack.push((format!("{path}.{key}"), child));
                    }
                }
                toml::Value::String(text) => {
                    let trimmed = text.trim().trim_end_matches([':', '：']);
                    assert!(
                        !banned.contains(&trimmed),
                        "{locale:?} {path} がペイン名「{text}」"
                    );
                }
                _ => {}
            }
        }
    }
    // 実際に描いた画面にも、この語だけの文字列が無い（ステータスバー・文脈ビュー・参照ペイン）
    for dark in [true, false] {
        let (mut harness, _) = Harness::start_with(world(), dark);
        let output = harness.idle(0.2);
        let text = painted_text(&output);
        for line in text.lines() {
            let trimmed = line.trim();
            assert!(
                !["ファイル", "エディタ", "情報", "Files", "Editor", "Info"].contains(&trimmed),
                "画面にペイン名「{line}」"
            );
        }
    }
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
    dark: bool,
}

impl Harness {
    fn new() -> Self {
        Self::start().0
    }

    /// 最初のフレームの出力も返す（フォントのテクスチャは最初のフレームにだけ全体が来る）。
    fn start() -> (Self, egui::FullOutput) {
        Self::start_with(world(), true)
    }

    fn start_with(
        project: novnow::core::features::Project,
        dark: bool,
    ) -> (Self, egui::FullOutput) {
        let ctx = egui::Context::default();
        let host = MemoryHost::default();
        let settings = UiSettings::default();
        let session = Session::new(Box::new(host.clone()), project, &settings);
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
        let dark = self.dark;
        self.ctx.run(input, |ctx| app.frame(ctx, Some(dark)))
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

// ============================================================ 目視用のスクリーンショット（手動）

/// egui の出力をソフトウェアで描いて RGBA を書き出す（`cargo test --features ui --test gui
/// screenshot -- --ignored`）。作業環境に画面が無いので、配置と配色を目で確かめるための道具。
/// 書き出し先は環境変数 `NOVNOW_SHOT`（無ければ何もしない）。PNG への変換は呼び出し側。
#[test]
#[ignore]
fn screenshot() {
    let Some(target) = std::env::var_os("NOVNOW_SHOT") else {
        return;
    };
    // 2 節目に辞書に無い参照（E001 の波線）を入れ、`NOVNOW_SHOT_LIGHT` があれば明るい配色で描く
    let text = world()
        .text(CONTENTS_PATH)
        .replace("図面の精巧な", "'未登録の図面の精巧な");
    let light = std::env::var_os("NOVNOW_SHOT_LIGHT").is_some();
    let (mut harness, first) = Harness::start_with(world_with(CONTENTS_PATH, &text), !light);
    let mut textures: std::collections::HashMap<
        egui::TextureId,
        (usize, usize, Vec<egui::Color32>),
    > = std::collections::HashMap::new();
    let absorb = |output: &egui::FullOutput,
                  textures: &mut std::collections::HashMap<
        egui::TextureId,
        (usize, usize, Vec<egui::Color32>),
    >| {
        for (id, delta) in &output.textures_delta.set {
            let (w, h, pixels): (usize, usize, Vec<egui::Color32>) = match &delta.image {
                egui::ImageData::Color(image) => {
                    (image.size[0], image.size[1], image.pixels.clone())
                }
                egui::ImageData::Font(image) => (
                    image.size[0],
                    image.size[1],
                    image.srgba_pixels(None).collect(),
                ),
            };
            match delta.pos {
                None => {
                    textures.insert(*id, (w, h, pixels));
                }
                Some([x0, y0]) => {
                    let entry = textures
                        .get_mut(id)
                        .expect("partial update of a known texture");
                    for y in 0..h {
                        for x in 0..w {
                            entry.2[(y0 + y) * entry.0 + x0 + x] = pixels[y * w + x];
                        }
                    }
                }
            }
        }
    };
    // `NOVNOW_SHOT_SCENE`: edit（既定）/ completion / palette / find / help
    let scene = std::env::var("NOVNOW_SHOT_SCENE").unwrap_or_default();
    let mut steps: Vec<Vec<Event>> = vec![
        vec![key(Key::Num2, Modifiers::CTRL)],
        vec![Event::Text("ここで一文を書き足す。".into())],
        vec![],
        vec![],
    ];
    match scene.as_str() {
        "completion" => steps.push(vec![Event::Text("'".into())]),
        "palette" => steps.push(vec![key(Key::P, Modifiers::CTRL)]),
        "find" => steps.push(vec![key(Key::H, Modifiers::CTRL)]),
        "help" => steps.push(vec![key(Key::Escape, Modifiers::NONE)]),
        _ => {}
    }
    if scene == "help" {
        // 入力欄の外で `?`
        steps.push(vec![Event::Text("?".into())]);
    }
    absorb(&first, &mut textures);
    let mut last = None;
    for events in steps {
        let output = harness.frame(events);
        absorb(&output, &mut textures);
        last = Some(output);
    }
    for _ in 0..4 {
        let output = harness.frame(Vec::new());
        absorb(&output, &mut textures);
        last = Some(output);
    }
    let output = last.expect("frame");
    let primitives = harness
        .ctx
        .tessellate(output.shapes.clone(), output.pixels_per_point);
    let (width, height) = (1280usize, 800usize);
    let mut frame = vec![[0f32; 4]; width * height];
    for clipped in primitives {
        let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else {
            continue;
        };
        let Some((tw, th, texels)) = textures.get(&mesh.texture_id) else {
            continue;
        };
        let clip = clipped.clip_rect;
        for triangle in mesh.indices.chunks(3) {
            let [a, b, c] = [
                mesh.vertices[triangle[0] as usize],
                mesh.vertices[triangle[1] as usize],
                mesh.vertices[triangle[2] as usize],
            ];
            let min_x = a
                .pos
                .x
                .min(b.pos.x)
                .min(c.pos.x)
                .max(clip.min.x)
                .max(0.0)
                .floor() as i32;
            let max_x = a
                .pos
                .x
                .max(b.pos.x)
                .max(c.pos.x)
                .min(clip.max.x)
                .min(width as f32)
                .ceil() as i32;
            let min_y = a
                .pos
                .y
                .min(b.pos.y)
                .min(c.pos.y)
                .max(clip.min.y)
                .max(0.0)
                .floor() as i32;
            let max_y = a
                .pos
                .y
                .max(b.pos.y)
                .max(c.pos.y)
                .min(clip.max.y)
                .min(height as f32)
                .ceil() as i32;
            let edge =
                |p: Pos2, q: Pos2, r: Pos2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
            let area = edge(a.pos, b.pos, c.pos);
            if area.abs() < 1e-6 {
                continue;
            }
            for y in min_y..max_y {
                for x in min_x..max_x {
                    let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = edge(b.pos, c.pos, p) / area;
                    let w1 = edge(c.pos, a.pos, p) / area;
                    let w2 = edge(a.pos, b.pos, p) / area;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let u = w0 * a.uv.x + w1 * b.uv.x + w2 * c.uv.x;
                    let v = w0 * a.uv.y + w1 * b.uv.y + w2 * c.uv.y;
                    let tx = ((u * *tw as f32) as usize).min(tw - 1);
                    let ty = ((v * *th as f32) as usize).min(th - 1);
                    let texel = texels[ty * tw + tx].to_array();
                    let mut src = [0f32; 4];
                    for (channel, value) in src.iter_mut().enumerate() {
                        let vertex = w0 * a.color.to_array()[channel] as f32
                            + w1 * b.color.to_array()[channel] as f32
                            + w2 * c.color.to_array()[channel] as f32;
                        *value = vertex / 255.0 * texel[channel] as f32 / 255.0;
                    }
                    let dst = &mut frame[y as usize * width + x as usize];
                    for channel in 0..4 {
                        dst[channel] = src[channel] + dst[channel] * (1.0 - src[3]);
                    }
                }
            }
        }
    }
    let mut bytes = Vec::with_capacity(width * height * 4);
    for pixel in frame {
        for channel in pixel {
            bytes.push((channel.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    std::fs::write(target, bytes).expect("write");
}

// ============================================================ 検索・置換・補完（§9.7.3・§9.8）

#[test]
fn find_steps_through_matches_and_wraps() {
    use novnow::ui::assist;
    let text = "あいあいう";
    let matches = assist::find_all(text, "あい");
    assert_eq!(matches, vec![0..6, 6..12]);
    assert_eq!(assist::step(&matches, 0..0, true), Some(0..6));
    assert_eq!(assist::step(&matches, 0..6, true), Some(6..12));
    assert_eq!(assist::step(&matches, 6..12, true), Some(0..6), "端で回る");
    assert_eq!(assist::step(&matches, 6..12, false), Some(0..6));
    assert_eq!(assist::step(&matches, 0..6, false), Some(6..12));
    assert!(assist::find_all(text, "").is_empty());
}

#[test]
fn project_search_lists_prose_hits_with_line_numbers() {
    let project = world_with("contents/01-05.txt", "@01-05-01 屋上\n'リリアが笑う。\n");
    let hits = novnow::ui::assist::search(&project, "リリア");
    assert!(hits.len() >= 3, "{hits:?}");
    assert!(hits.iter().all(|hit| hit.path.starts_with("contents/")));
    let last = hits.last().expect("hit");
    assert_eq!((last.path.as_str(), last.line), ("contents/01-05.txt", 2));
    assert_eq!(last.text, "'リリアが笑う。");
}

#[test]
fn the_modes_popup_lists_every_mode_and_marks_unavailable_ones() {
    use novnow::core::features::hover;
    let project = world();
    let document = project.document(CONTENTS_PATH).expect("doc");
    let at = document.text.find("'アルス").expect("ref") + 1;
    let found = hover::at(document, at, &project).expect("hover");
    let lines = novnow::ui::assist::info_lines(&found, true).expect("character");
    assert_eq!(lines[0], "アルス（あるす）");
    assert!(
        lines.iter().any(|line| line.contains("A 探究")),
        "{lines:?}"
    );
    // B 補習は @02 から（この節 @01-04-02 では使えない）
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with('×') && line.contains("B 補習")),
        "{lines:?}"
    );
    assert!(
        novnow::ui::assist::info_lines(&found, false).is_none(),
        "用語ではない"
    );
}

#[test]
fn the_workbench_reveals_a_byte_and_deletes_or_replaces_sections() {
    let mut bench = Workbench::new(world(), None, 400);
    let text = bench.project().text(CONTENTS_PATH).to_owned();
    let at = text.find("図面の精巧な").expect("match");
    let offset = bench.reveal(CONTENTS_PATH, at).expect("inside a section");
    assert_eq!(bench.current_id(), Some(id(1, 4, 2, 2)));
    assert_eq!(&bench.buffer()[offset..offset + "図面".len()], "図面");
    // 見出しの行は節の外: 直後の節へ移るが選択はしない
    assert_eq!(bench.reveal(CONTENTS_PATH, 0), None);
    assert_eq!(bench.current_id(), Some(id(1, 4, 2, 1)));
    // 削除: 次の節へ
    assert!(bench.delete_section(1.0));
    assert_eq!(bench.sections().len(), 2);
    assert!(bench.buffer().starts_with("「旧校舎"));
    // すべて置換
    let replaced = bench
        .project()
        .text(CONTENTS_PATH)
        .replace("リリア", "リリィ");
    assert!(bench.replace_current_file(replaced, 2.0));
    assert!(!bench.project().text(CONTENTS_PATH).contains("リリア"));
    assert!(bench.unsaved().count() == 1);
}

#[test]
fn ctrl_f_and_enter_jump_to_the_next_match_in_another_section() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::F, Modifiers::CTRL)]);
    assert!(harness.app.overlay_open());
    harness.frame(vec![Event::Text("精巧".into())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.idle(0.1);
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 4, 2, 2)),
        "一致のある 2 節目へ"
    );
    // F3 は端で回って同じ一致に戻る（一致は 1 つ）
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    harness.frame(vec![key(Key::F3, Modifiers::NONE)]);
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 4, 2, 2))
    );
}

#[test]
fn ctrl_shift_semicolon_asks_before_deleting_the_section() {
    let mut harness = Harness::new();
    let ctrl_shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    harness.frame(vec![key(Key::Semicolon, ctrl_shift)]);
    assert!(harness.app.overlay_open(), "確認を出す");
    assert_eq!(
        harness.app.workbench().map(|bench| bench.sections().len()),
        Some(3)
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(!harness.app.overlay_open());
    assert_eq!(
        harness.app.workbench().map(|bench| bench.sections().len()),
        Some(2)
    );
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get(CONTENTS_PATH)
        .cloned();
    assert!(!written.expect("saved").contains("呆れたような声"));
}

#[test]
fn typing_a_quote_opens_completion_and_enter_inserts_the_candidate() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    // キャレットは節の末尾（`Ctrl+Home` は §9.8.2 の「ファイル先頭の節」なので使わない）
    harness.frame(vec![Event::Text("'".into())]);
    harness.frame(Vec::new());
    assert!(
        harness.app.overlay_open(),
        "`'` の直後に補完を出す（§9.7.3）"
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert!(!harness.app.overlay_open());
    let buffer = harness
        .app
        .workbench()
        .map(|bench| bench.buffer().to_owned())
        .expect("open");
    assert!(
        buffer.ends_with("引き寄せた。'アルス") || buffer.ends_with("引き寄せた。'リリア"),
        "候補を挿入する（Enter で改行は入らない）: {buffer:?}"
    );
}

#[test]
fn quick_fix_and_register_write_to_the_dictionary_files() {
    use novnow::core::features::code_action;
    use novnow::core::resolve::dictionary::EntryKind;
    let text = world()
        .text(CONTENTS_PATH)
        .replace("呆れたような声", "'新語な声");
    let mut bench = Workbench::new(world_with(CONTENTS_PATH, &text), None, 400);
    let (document, start) = bench.spliced_document().expect("open");
    let at = document.text.find("'新語").expect("ref") - start + 1;
    let actions = code_action::at(&document, start + at..start + at, bench.project());
    let register = actions
        .iter()
        .find(|action| action.kind == code_action::ActionKind::RegisterWord)
        .expect("E001 の登録");
    assert!(bench.apply_edit(&register.edit, 1.0));
    let words = bench.project().text("words.md").to_owned();
    assert!(
        words.contains(&format!("# {}\ndefinition: \n", register.arg)),
        "{:?} / {words}",
        register.arg
    );
    assert!(register.arg.starts_with("新語"), "{:?}", register.arg);
    assert!(bench.unsaved().any(|path| path == "words.md"));
    assert_eq!(bench.current_id(), Some(id(1, 4, 2, 1)), "同じ節にいる");
    // Ctrl+T / Ctrl+Shift+T と同じ編集
    let edit = code_action::register_edit(bench.project(), EntryKind::Character, "ミナ");
    assert!(bench.apply_edit(&edit, 2.0));
    assert!(bench
        .project()
        .text("characters.md")
        .ends_with("\n\n# ミナ\n"));
    let writes = bench.take_writes();
    let paths: Vec<&str> = writes.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, vec!["characters.md", "words.md"]);
}

#[test]
fn the_diff_modal_keeps_mine_or_takes_theirs() {
    let mut bench = Workbench::new(world(), None, 400);
    bench.buffer_mut().push_str("自分の編集");
    bench.edited(0.0);
    let theirs = "@01-04-02 図書室の攻防\n外の版。\n".to_owned();
    assert_eq!(
        bench.external_change(CONTENTS_PATH, Some(theirs.clone())),
        Reaction::Conflict
    );
    bench.keep_mine(CONTENTS_PATH, 1.0);
    assert_eq!(bench.conflicts().count(), 0);
    assert!(bench.save.due(1.0), "自分の版を書き出す");
    assert!(bench.take_writes()[0].1.contains("自分の編集"));
    // もう一度衝突させて、今度は外部の版を読む
    bench.buffer_mut().push_str("さらに");
    bench.edited(2.0);
    assert_eq!(
        bench.external_change(CONTENTS_PATH, Some(theirs.clone())),
        Reaction::Conflict
    );
    bench.take_external(CONTENTS_PATH, Some(theirs.clone()));
    assert_eq!(bench.project().text(CONTENTS_PATH), theirs);
    assert_eq!(bench.buffer(), "外の版。");
    assert_eq!(bench.unsaved().count(), 0);
}

// ============================================================ エクスポート（§17.2、M9）

/// パレットから `>export` を開く。
fn open_export(harness: &mut Harness) {
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text(">export".into())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(harness.app.overlay_open(), "エクスポートのモーダル");
    // 範囲の欄にフォーカスが移るのを待つ
    harness.frame(Vec::new());
}

fn exported(project: &novnow::core::features::Project, range: Option<&str>) -> String {
    use novnow::core::export;
    let range = range.map(|text| export::parse_range(text).expect(text));
    let options = export::Options::from_settings(&project.settings().export);
    export::run(project, range.as_ref(), &options)
}

#[test]
fn the_export_modal_saves_into_exports_with_the_default_name() {
    let mut harness = Harness::new();
    open_export(&mut harness);
    let output = harness.frame(Vec::new());
    let painted = painted_text(&output);
    assert!(painted.contains("3 節・"), "件数と字数: {painted}");
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(!harness.app.overlay_open(), "書いたら閉じる");
    let name = format!("exports/memory-all-{}.txt", novnow::ui::app::MEMORY_STAMP);
    let written = harness.host.files.borrow().written.get(&name).cloned();
    assert_eq!(written, Some(exported(&world(), None)), "{name}");
    let painted = painted_text(&harness.frame(Vec::new()));
    assert!(
        painted.contains(&format!("エクスポートしました: {name}")),
        "{painted}"
    );
}

#[test]
fn the_export_modal_narrows_by_range_and_state() {
    // 章 @01 の state を外し、2 節目だけ fixed にする
    let all = "@01 失われた刻印\nsummary: 導入章。\n;\n";
    let plot = "@01-04-02 図書室の攻防\n咎める。\n;\n明かす。\nstate: fixed\n;\n心変わり。\n";
    let make = || {
        let mut project = world_with("plots/all.txt", all);
        project.insert(novnow::core::model::document::Document::new(
            "plots/01-04.txt",
            plot,
        ));
        project
    };
    let project = make();
    let (mut harness, _) = Harness::start_with(make(), true);
    open_export(&mut harness);
    // Tab で review 以上 → fixed だけ
    harness.frame(vec![key(Key::Tab, Modifiers::NONE)]);
    harness.frame(vec![key(Key::Tab, Modifiers::NONE)]);
    let painted = painted_text(&harness.frame(Vec::new()));
    assert!(painted.contains("1 節・"), "{painted}");
    // 範囲が読めなければ書かない
    harness.frame(vec![Event::Text("@01-x".into())]);
    let painted = painted_text(&harness.frame(Vec::new()));
    assert!(painted.contains("範囲として読めません"), "{painted}");
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(harness.app.overlay_open());
    assert!(harness.host.files.borrow().written.is_empty());
    // 範囲を直して書く（全部消してから）
    for _ in 0.."@01-x".chars().count() {
        harness.frame(vec![key(Key::Backspace, Modifiers::NONE)]);
    }
    harness.frame(vec![Event::Text("01-04-02".into())]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(!harness.app.overlay_open());
    let name = format!(
        "exports/memory-01-04-02-{}.txt",
        novnow::ui::app::MEMORY_STAMP
    );
    let written = harness.host.files.borrow().written.get(&name).cloned();
    let want = {
        use novnow::core::export;
        let range = export::parse_range("@01-04-02").expect("range");
        let options = export::Options {
            state: Some(export::State::Fixed),
            ..export::Options::from_settings(&project.settings().export)
        };
        export::run(&project, Some(&range), &options)
    };
    assert_eq!(written.as_deref(), Some(want.as_str()));
    assert!(want.starts_with("「旧校舎の地下遺跡に"), "{want}");
}

#[test]
fn save_as_goes_through_the_dialog_and_copy_fills_the_clipboard() {
    let mut harness = Harness::new();
    // 名前を付けて保存（Shift+Enter）: ダイアログが選んだ 1 か所にだけ書く
    harness.host.files.borrow_mut().chosen = Some("/elsewhere/公開用.md".into());
    open_export(&mut harness);
    harness.frame(vec![key(Key::Enter, Modifiers::SHIFT)]);
    assert!(!harness.app.overlay_open());
    {
        let files = harness.host.files.borrow();
        assert_eq!(
            files.exported.get("/elsewhere/公開用.md"),
            Some(&exported(&world(), None))
        );
        assert!(files.written.is_empty(), "プロジェクトには書かない");
    }
    // 取り消したら何もせずモーダルのまま
    open_export(&mut harness);
    harness.frame(vec![key(Key::Enter, Modifiers::SHIFT)]);
    assert!(harness.app.overlay_open());
    // .txt / .md でない名前は E060 で書かない
    harness.host.files.borrow_mut().chosen = Some("/elsewhere/a.zip".into());
    harness.frame(vec![key(Key::Enter, Modifiers::SHIFT)]);
    // 新しく出たモーダルは最初のフレームでは大きさを測るだけで描かれない（egui の Area）
    harness.frame(Vec::new());
    let painted = painted_text(&harness.frame(Vec::new()));
    assert!(
        painted.contains("エクスポートできませんでした: プロジェクトの外には書き込めません"),
        "{painted}"
    );
    assert_eq!(harness.host.files.borrow().exported.len(), 1);
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    // コピー（Ctrl+Shift+C）
    open_export(&mut harness);
    // 実機の egui-winit は Windows / Linux の Ctrl で ctrl と command の両方を立てる
    let ctrl_shift = Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT;
    let output = harness.frame(vec![key(Key::C, ctrl_shift)]);
    assert_eq!(output.platform_output.copied_text, exported(&world(), None));
    assert!(!harness.app.overlay_open());
}
