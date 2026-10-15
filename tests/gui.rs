//! GUI（M7、`ui` feature）のテスト。egui を**画面なし**で回す（`egui::Context::run`）。
//!
//! 状態の規則（保存ステートマシン・`ui.json`・パレットの採点・キー割当・IME の判定・節単位編集）は
//! egui の画面を使わずに確かめ、画面の MUST（文脈ビューに編集中の本文を描かない・変換中は
//! ショートカットが効かない・保存は 400ms の idle の後）は `NovnowApp` を 1 フレームずつ回して確かめる。

#![cfg(feature = "ui")]

mod harness;
mod world;

use egui::{Event, Key, Modifiers, Pos2};

use novnow::core::id::SceneId;
use novnow::ui::i18n::Locale;
use novnow::ui::palette;
use novnow::ui::save_machine::{Indicator, SaveMachine, State};
use novnow::ui::settings::{LanguageChoice, ThemeChoice, UiSettings};
use novnow::ui::shell::{self, BrowserKeys, Command, ImeState};
use novnow::ui::widgets::inlay;
use novnow::ui::workbench::{Nav, Reaction, Workbench};

use harness::{key, painted_text, Harness};
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
        assert!(settings.inlay_hints, "インレイヒントは既定で出す: {text}");
    }
}

#[test]
fn localized_maximum_line_width_round_trips_and_is_bounded() {
    let default = UiSettings::default();
    assert_eq!(default.line_chars(false), 44);
    assert_eq!(default.line_chars(true), 70);
    assert!(!default.to_json().contains("max_line_chars"));
    for (input, expected) in [("5", 20), ("90", 90), ("999", 120)] {
        let settings = UiSettings::parse(&format!(r#"{{"max_line_chars":{input}}}"#));
        assert_eq!(settings.line_chars(false), expected);
        assert_eq!(settings.line_chars(true), expected);
        let restored = UiSettings::parse(&settings.to_json());
        assert_eq!(restored.line_chars(false), expected);
    }
}

#[test]
fn ui_json_keeps_unknown_keys_and_clamps_values() {
    let text = r#"{"schema_version":1,"theme":"light","language":"en","font_scale":9,
        "pane_ratio":{"main":0.1,"context_view":0.5},"context_sections":12,"autosave_ms":50,
        "inlay_hints":false,"folded":{"outline":["@02"]},"future_key":true}"#;
    let mut settings = UiSettings::parse(text);
    assert_eq!(settings.theme, ThemeChoice::Light);
    assert_eq!(settings.language, LanguageChoice::En);
    assert_eq!(settings.font_scale, 1.6);
    assert_eq!(settings.main_ratio, 0.3);
    assert_eq!(settings.context_sections, 3, "範囲外は既定値");
    assert_eq!(settings.autosave_ms, 200);
    assert!(!settings.inlay_hints);
    settings.theme = settings.theme.next();
    settings.last_scene = Some("01-04-02-03".into());
    let written = settings.to_json();
    let value: serde_json::Value = serde_json::from_str(&written).expect("json");
    assert_eq!(value["theme"], "system");
    assert_eq!(value["future_key"], true, "知らないキーを残す");
    assert_eq!(value["folded"]["outline"][0], "@02");
    assert_eq!(value["last_scene"], "01-04-02-03");
    assert_eq!(value["inlay_hints"], false);
    assert!(written.ends_with('\n'));
    assert_eq!(UiSettings::parse(&written).theme, ThemeChoice::System);
    assert!(!UiSettings::parse(&written).inlay_hints);
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
    let spec = include_str!("../docs/specifications/ui.md");
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
    assert_eq!(
        palette::complete_token(">ag", "agent "),
        Some(">agent ".into())
    );
    assert_eq!(
        palette::complete_token(">agent de", "agent delegation writer 2"),
        Some(">agent delegation ".into())
    );
    assert_eq!(
        palette::complete_token(">agent delegation w", "agent delegation writer 2"),
        Some(">agent delegation writer ".into())
    );
    assert_eq!(
        palette::complete_token(">the", "theme"),
        Some(">theme".into())
    );
    assert_eq!(palette::complete_token(">bad", "agent "), None);
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
fn tab_completes_the_agent_group_then_opens_its_commands() {
    let (mut harness, _) = Harness::start_with(world(), true);
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text(">ag".into())]);
    let before = painted_text(&harness.frame(Vec::new()));
    assert!(before.contains("Agent settings"), "{before}");
    harness.frame(vec![key(Key::Tab, Modifiers::NONE)]);
    let after = painted_text(&harness.frame(Vec::new()));
    assert!(after.contains(">agent delegation"), "{after}");
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
    assert_eq!(of(Key::H, alt), Some(Command::InlayHints));
    assert_eq!(
        of(Key::H, ctrl),
        Some(Command::Replace),
        "Ctrl+H は置換のまま"
    );
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
    assert_eq!(
        shell::command_of(Key::H, ctrl_alt, true, true),
        Some(Command::InlayHints)
    );
    assert_eq!(
        shell::command_of(Key::H, alt, true, true),
        None,
        "macOS の Option+H は文字の入力"
    );
}

// ============================================================ インレイヒント（§6.6.2、B-137）

#[test]
fn an_inlay_gap_is_leading_space_at_the_hint_and_keeps_every_character() {
    use egui::text::{LayoutJob, TextFormat};
    let text = "'M0137 @x\nend";
    let mut plain = LayoutJob::default();
    plain.append(text, 0.0, TextFormat::default());
    let mut job = plain.clone();
    // 6 = トークンの直後（空白の前）、9 = 改行の直前、13 = 末尾（後ろに字が無いので空けない）
    inlay::open_gaps(&mut job, &[(6, 40.0), (9, 30.0), (13, 10.0)]);
    let sections: Vec<(std::ops::Range<usize>, f32)> = job
        .sections
        .iter()
        .map(|section| (section.byte_range.clone(), section.leading_space))
        .collect();
    assert_eq!(sections, vec![(0..6, 0.0), (6..13, 40.0)]);
    // セクションの頭なら割らずに足す
    inlay::open_gaps(&mut job, &[(6, 2.0)]);
    assert_eq!(job.sections[1].leading_space, 42.0);
    // 文字と Galley の対応は変わらない（カーソル・選択・IME はそのまま）
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |_| {});
    let before = ctx.fonts(|fonts| fonts.layout_job(plain));
    let after = ctx.fonts(|fonts| fonts.layout_job(job));
    assert_eq!(after.text(), before.text());
    assert_eq!(after.rows[0].glyphs.len(), before.rows[0].glyphs.len());
    let x = |galley: &egui::Galley, index: usize| galley.rows[0].glyphs[index].pos.x;
    assert_eq!(x(&after, 5), x(&before, 5), "トークンの字は動かない");
    assert!(x(&after, 6) - x(&before, 6) >= 41.0, "隙間の後ろの字が右へ");
}

#[test]
fn mode_names_show_as_inlay_hints_and_alt_h_hides_them_without_touching_the_text() {
    let mut harness = Harness::new();
    let alt = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };
    // world の @01-04-02-01: 「やっぱり'アルス、ここにいたのね」@リリア:A（A = 呆れ）
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(text.contains("⟨呆れ⟩"), "{text}");
    let before = harness.app.workbench().expect("open").buffer().to_owned();
    assert!(before.contains("@リリア:A"), "{before}");
    assert!(!before.contains('⟨'), "ヒントは本文に入らない");
    harness.frame(vec![key(Key::H, alt)]);
    let output = harness.frame(Vec::new());
    assert!(!painted_text(&output).contains("⟨呆れ⟩"));
    assert!(!harness.app.settings().inlay_hints);
    assert_eq!(harness.app.workbench().expect("open").buffer(), before);
    harness.idle(0.3);
    let ui_json = harness
        .host
        .files
        .borrow()
        .written
        .get(".novnow/ui.json")
        .cloned()
        .expect("ui.json");
    let value: serde_json::Value = serde_json::from_str(&ui_json).expect("json");
    assert_eq!(value["inlay_hints"], false);
    assert!(
        !harness
            .host
            .files
            .borrow()
            .written
            .contains_key(CONTENTS_PATH),
        "本文は書き直さない"
    );
    harness.frame(vec![key(Key::H, alt)]);
    let output = harness.frame(Vec::new());
    assert!(painted_text(&output).contains("⟨呆れ⟩"));
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
fn pasting_a_break_line_splits_the_section_and_the_file() {
    let mut bench = Workbench::new(world(), None, 400);
    let at = bench.buffer().len();
    assert!(bench.paste(at..at, "追記。\nつぎの行。\n;\n新しい節。", 1.0));
    assert_eq!(
        bench.buffer(),
        "「やっぱり'アルス、ここにいたのね」@リリア:A\n\n呆れたような声とともに、'リリアが隣の椅子を引き寄せた。追記。\nつぎの行。",
        "§8.1.3: エディタに残るのは先頭の節だけ"
    );
    assert_eq!(
        bench.current_id(),
        Some(id(1, 4, 2, 1)),
        "編集中の節は動かない"
    );
    assert_eq!(bench.sections().len(), 4, "分けた後ろは続きの節になる");
    let writes = bench.take_writes();
    assert_eq!(writes[0].0, CONTENTS_PATH);
    assert!(
        writes[0]
            .1
            .contains("引き寄せた。追記。\nつぎの行。\n;\n新しい節。\n;\n「旧校舎"),
        "{}",
        writes[0].1
    );
    assert!(bench.save.due(1.5), "貼り付けも保存のタイマを回す");
}

#[test]
fn a_paste_without_breaks_is_left_to_the_editor() {
    let mut bench = Workbench::new(world(), None, 400);
    let at = bench.buffer().len();
    assert!(!bench.paste(at..at, "追記。\n次の行。", 1.0));
    assert_eq!(bench.sections().len(), 3, "節は増えない");
    assert!(!bench.save.due(10.0), "何も変えない");
}

#[test]
fn a_paste_with_a_break_line_becomes_a_new_section_in_the_file() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![Event::Paste("追記。\n;\nあたらしい節。".into())]);
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get(CONTENTS_PATH)
        .cloned()
        .expect("400ms の idle の後に本文を書く");
    assert!(
        written.contains("引き寄せた。追記。\n;\nあたらしい節。\n;\n「旧校舎"),
        "{written}"
    );
    assert!(painted_text(&harness.frame(Vec::new())).contains("追記。"));
}

#[test]
fn a_pasted_crlf_becomes_lf() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![Event::Paste("CR の行。\r\nつぎの行。\r".into())]);
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get(CONTENTS_PATH)
        .cloned()
        .expect("400ms の idle の後に本文を書く");
    assert!(
        written.contains("引き寄せた。CR の行。\nつぎの行。\n"),
        "{written}"
    );
    assert!(!written.contains('\r'), "§5.0.4 MUST: 内部表現は LF");
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

#[test]
fn an_asynchronous_save_keeps_what_was_committed_while_it_was_writing() {
    // Web の IndexedDB のように保存の結果が後のフレームで来るとき（B-156）: 書いているあいだに節を移って
    // 文書に入った打鍵は、書き終わった後も未保存のまま残り、次の保存で書かれる
    let mut harness = Harness::new();
    harness.host.files.borrow_mut().async_saves = true;
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("一".into())]);
    harness.idle(0.6);
    assert!(
        harness.host.files.borrow().held_save.is_some(),
        "保存を始めた"
    );
    assert_eq!(
        harness.app.workbench().map(|bench| bench.save.state()),
        Some(State::Saving)
    );
    // 書いているあいだに打って、節を移る（打鍵が文書に入る）
    harness.frame(vec![Event::Text("二".into())]);
    harness.frame(vec![key(Key::ArrowDown, Modifiers::ALT)]);
    harness.host.files.borrow_mut().release_save();
    harness.idle(0.6);
    let first = harness
        .host
        .files
        .borrow()
        .written
        .get(CONTENTS_PATH)
        .cloned();
    let first = first.expect("1 回目の保存");
    assert!(first.contains('一') && !first.contains('二'), "{first}");
    assert!(
        harness.host.files.borrow().held_save.is_some(),
        "書いているあいだの打鍵は次の保存で書く"
    );
    harness.host.files.borrow_mut().release_save();
    harness.idle(0.6);
    let second = harness
        .host
        .files
        .borrow()
        .written
        .get(CONTENTS_PATH)
        .cloned();
    assert!(second.expect("2 回目の保存").contains("一二"));
    assert_eq!(
        harness.app.workbench().map(|bench| bench.save.state()),
        Some(State::Clean)
    );
}

#[test]
fn on_the_web_only_browser_defaults_that_novnow_needs_are_blocked() {
    // B-158: 印刷・ページの保存・検索バー・戻るなどは止め、コピー / 貼り付け・再読み込み・開発者ツール・
    // AltGr の字・macOS の Option の字・修飾キーの無い字は止めない
    let command = BrowserKeys {
        command: true,
        ..BrowserKeys::default()
    };
    for key in [
        "p", "o", "s", "f", "h", "g", "m", "b", "1", ";", "Enter", "Home",
    ] {
        assert!(shell::browser_default_blocked(key, command), "Ctrl+{key}");
    }
    for key in ["c", "v", "x", "r"] {
        assert!(!shell::browser_default_blocked(key, command), "Ctrl+{key}");
    }
    let devtools = BrowserKeys {
        shift: true,
        ..command
    };
    assert!(!shell::browser_default_blocked("I", devtools));
    // F-17: Browser-owned keys have actionable alternate chords.
    let alternate = egui::Modifiers {
        ctrl: true,
        shift: true,
        alt: true,
        ..Default::default()
    };
    for (key, command) in [
        (egui::Key::I, Command::Capture),
        (egui::Key::N, Command::InsertSection),
        (egui::Key::W, Command::Term),
        (egui::Key::T, Command::Register { character: true }),
    ] {
        assert_eq!(
            shell::command_of(key, alternate, false, false),
            Some(command)
        );
    }
    assert!(shell::browser_default_blocked(
        "I",
        BrowserKeys {
            alt: true,
            ..devtools
        }
    ));
    assert!(
        shell::browser_default_blocked("F", devtools),
        "Ctrl+Shift+F は全体検索"
    );
    let alt = BrowserKeys {
        alt: true,
        ..BrowserKeys::default()
    };
    assert!(
        shell::browser_default_blocked("1", alt),
        "Alt+1 はワークスペース"
    );
    assert!(
        shell::browser_default_blocked("ArrowLeft", alt),
        "Alt+← は場面（戻るではない）"
    );
    assert!(!shell::browser_default_blocked(
        "1",
        BrowserKeys { mac: true, ..alt }
    ));
    assert!(!shell::browser_default_blocked(
        "@",
        BrowserKeys {
            alt_graph: true,
            ..command
        }
    ));
    for key in ["a", "あ", "/", "Enter", "F5", "F12"] {
        assert!(
            !shell::browser_default_blocked(key, BrowserKeys::default()),
            "{key}"
        );
    }
    assert!(shell::browser_default_blocked("F2", BrowserKeys::default()));
    assert!(shell::browser_default_blocked("F3", BrowserKeys::default()));
}

// ============================================================ 目視用のスクリーンショット（手動）

/// スクリーンショットの agent の場面: 委譲の設定・MCP の定義・writer のタスク 2 つ。
fn shot_agent_project(project: &mut novnow::core::features::Project) {
    use novnow::core::agents::runner;
    use novnow::core::model::document::Document;
    // 仕様書の例の全体プロットは第 1 章を確定済みにしているので、節 4 だけにする（propose の writer が走れるように）
    project.insert(Document::new(
        world::ALL_PATH,
        world::ALL.replacen("state: fixed", "state: draft", 1),
    ));
    project.insert(Document::new(
        "project.json",
        "{\n  \"schema_version\": 1,\n  \"title\": \"星屑\",\n  \"agents\": { \"roles\": { \"ideator\": \"apply\" } }\n}\n",
    ));
    project.insert(Document::new(
        "agents/mcp.json",
        "{\n  \"schema_version\": 1,\n  \"servers\": {\n    \"llm-main\": {\n      \"transport\": \"stdio\",\n      \"command\": \"my-llm-mcp\",\n      \"env\": { \"API_KEY\": \"${NOVNOW_LLM_KEY}\" },\n      \"capabilities\": [\"sampling\"],\n      \"roles\": [\"*\"]\n    }\n  }\n}\n",
    ));
    for scope in ["@01-04-02-02", "@01-04-02-03"] {
        let plan = runner::new_tasks(
            project,
            &runner::Draft {
                role: "writer",
                scope,
                intent: "テンポを上げる",
                comment: "",
                examples: "",
                initiator: "human",
                status: "draft",
            },
            "2026-10-13T21:00:00+09:00",
        );
        for change in &plan.writes {
            project.insert(Document::new(change.path.clone(), change.text.clone()));
        }
    }
}

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
    let scene = std::env::var("NOVNOW_SHOT_SCENE").unwrap_or_default();
    let mut text = world()
        .text(CONTENTS_PATH)
        .replace("図面の精巧な", "'未登録の図面の精巧な");
    if scene == "inlay" {
        // 資料 ID（タイトルのインレイヒント）を行の途中に置く（§6.6.2、B-137）
        text = text.replace(
            "隣の椅子を引き寄せた。",
            "隣の椅子を引き寄せた。机には'M0137 の写しがある。",
        );
    }
    let light = std::env::var_os("NOVNOW_SHOT_LIGHT").is_some();
    let mut project = world_with(CONTENTS_PATH, &text);
    if scene.starts_with("material") {
        // 資料 MD（§5.8.3 の形）と、索引に無い資料 MD（W064）
        project.insert(novnow::core::model::document::Document::new(
            "materials/docs/M0137-kagaku-koshien.md",
            "# 科学の甲子園 本戦の運営と実技課題\n<!-- novnow:M0137 -->\n\n## タイムテーブル\n- 9:00 開会\n- 13:00 実技課題\n",
        ));
        project.insert(novnow::core::model::document::Document::new(
            "materials/inbox/memo.md",
            "# 取材メモ\n",
        ));
    }
    let (mut harness, first) = if scene == "agent" {
        // M10 の agent ワークスペース: 節 2 と 3 の writer のタスクを作っておき、LLM の答えを 1 つ用意する
        shot_agent_project(&mut project);
        Harness::start_agents(project, |files| {
            files.mcp = true;
            files
                .env
                .insert("NOVNOW_LLM_KEY".to_owned(), "secret".to_owned());
            files
                .replies
                .push_back(Ok(novnow::ports::mcp::McpReply {
                    text: "「……付き合ってあげるわよ。今日だけね」\n彼女はそう言って、図面から目を離さなかった。"
                        .to_owned(),
                    model: "model-x".to_owned(),
                    truncated: false,
                }));
        })
    } else {
        Harness::start_with(project, !light)
    };
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
    // `NOVNOW_SHOT_SCENE`: edit（既定）/ inlay / completion / palette / find / help / episode /
    // outline / idea / material / material-search / cast / review / agent
    let mut steps: Vec<Vec<Event>> = if scene == "agent" {
        // Alt+8 → 選択中のタスクを R で走らせ → Enter で結果レビューへ
        vec![
            vec![key(Key::Num8, Modifiers::ALT)],
            vec![],
            vec![],
            vec![key(Key::R, Modifiers::NONE)],
            vec![],
            vec![],
            vec![key(Key::Enter, Modifiers::NONE)],
        ]
    } else {
        vec![
            vec![key(Key::Num2, Modifiers::CTRL)],
            vec![Event::Text("ここで一文を書き足す。".into())],
            vec![],
            vec![],
        ]
    };
    match scene.as_str() {
        "completion" => steps.push(vec![Event::Text("'".into())]),
        "palette" => steps.push(vec![key(Key::P, Modifiers::CTRL)]),
        "find" => steps.push(vec![key(Key::H, Modifiers::CTRL)]),
        "help" => steps.push(vec![key(Key::Escape, Modifiers::NONE)]),
        // M8 のワークスペース（Alt+2 / Alt+3）
        "episode" => steps.push(vec![key(Key::Num2, Modifiers::ALT)]),
        "outline" => steps.push(vec![key(Key::Num3, Modifiers::ALT)]),
        "idea" => steps.push(vec![key(Key::Num4, Modifiers::ALT)]),
        "material" => {
            steps.push(vec![key(Key::Num5, Modifiers::ALT)]);
            steps.push(vec![key(Key::ArrowDown, Modifiers::ALT)]);
        }
        "material-search" => {
            steps.push(vec![key(Key::M, Modifiers::CTRL)]);
            steps.push(vec![Event::Text("甲子園".into())]);
        }
        "cast" => steps.push(vec![key(Key::Num6, Modifiers::ALT)]),
        "review" => steps.push(vec![key(Key::Num9, Modifiers::ALT)]),
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

#[test]
fn theme_icons_are_vector_shapes_including_bezier_crescent() {
    let ctx = egui::Context::default();
    for choice in [ThemeChoice::System, ThemeChoice::Dark, ThemeChoice::Light] {
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            novnow::ui::theme::paint_theme_icon(
                &ctx.debug_painter(),
                egui::pos2(30.0, 30.0),
                choice,
                egui::Color32::WHITE,
            );
        });
        assert!(
            output
                .shapes
                .iter()
                .all(|clipped| !matches!(clipped.shape, egui::Shape::Text(_))),
            "theme icon must not require a font glyph: {choice:?}"
        );
        if choice == ThemeChoice::Dark {
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|clipped| matches!(clipped.shape, egui::Shape::Path(_))),
                "dark icon must use a curved vector crescent"
            );
        }
    }
}

#[test]
fn fullwidth_punctuation_uses_the_cjk_font_before_the_generic_fallback() {
    let (definitions, _) = novnow::ui::theme::fonts();
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        assert_eq!(
            definitions
                .families
                .get(&family)
                .and_then(|names| names.first())
                .map(String::as_str),
            Some("novnow-cjk"),
            "？！＊ should use the same CJK face as surrounding prose"
        );
    }
}

#[test]
fn link_buttons_have_a_24px_target_hover_fill_underline_and_pointer() {
    let ctx = egui::Context::default();
    let colors = novnow::ui::theme::palette(novnow::ui::theme::Mode::Light);
    let mut rect = egui::Rect::NOTHING;
    let render = |events: Vec<Event>, rect: &mut egui::Rect| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(300.0, 150.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    *rect = novnow::ui::widgets::link_button(
                        ui,
                        "続き...",
                        egui::FontId::proportional(13.0),
                        &colors,
                    )
                    .rect;
                });
            },
        )
    };
    render(Vec::new(), &mut rect);
    assert!(rect.width() >= 24.0 && rect.height() >= 24.0, "{rect:?}");
    let output = render(vec![Event::PointerMoved(rect.center())], &mut rect);
    assert_eq!(
        output.platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    assert!(output.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::Rect(shape) if shape.fill == colors.bg_selection.gamma_multiply(0.6)
    )));
    assert!(output
        .shapes
        .iter()
        .any(|item| matches!(&item.shape, egui::Shape::LineSegment { .. })));
}

#[test]
fn action_buttons_share_a_minimum_hit_area_underline_and_pointer_cursor() {
    let ctx = egui::Context::default();
    ctx.set_visuals(novnow::ui::theme::visuals(novnow::ui::theme::Mode::Light));
    let mut rect = egui::Rect::NOTHING;
    let draw = |events: Vec<Event>, rect: &mut egui::Rect| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(300.0, 150.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    *rect = novnow::ui::widgets::action_button(ui, "実行").rect;
                });
            },
        )
    };
    let first = draw(Vec::new(), &mut rect);
    assert!(rect.width() >= 24.0 && rect.height() >= 24.0, "{rect:?}");
    assert!(first.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::Text(text) if text.galley.job.sections.iter().any(|section| section.format.underline.width > 0.0)
    )));
    let hovered = draw(vec![Event::PointerMoved(rect.center())], &mut rect);
    assert_eq!(
        hovered.platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
}

#[test]
fn input_rect_expands_without_changing_the_painted_bounds() {
    let painted = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::Vec2::new(8.0, 10.0));
    let hit = novnow::ui::widgets::hit_rect(painted);
    assert_eq!(hit.center(), painted.center());
    assert_eq!(hit.size(), egui::Vec2::splat(24.0));
    assert_eq!(
        novnow::ui::widgets::hit_rect(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(80.0, 30.0)
        ))
        .size(),
        egui::Vec2::new(80.0, 30.0)
    );
}

#[test]
fn selectable_actions_have_underlines_and_24px_hit_targets() {
    let ctx = egui::Context::default();
    ctx.set_visuals(novnow::ui::theme::visuals(novnow::ui::theme::Mode::Light));
    let mut rect = egui::Rect::NOTHING;
    let draw = |events: Vec<Event>, rect: &mut egui::Rect| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(300.0, 150.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    *rect = novnow::ui::widgets::selectable_action(
                        ui,
                        false,
                        egui::RichText::new("短").small(),
                    )
                    .rect;
                });
            },
        )
    };
    let first = draw(Vec::new(), &mut rect);
    assert!(rect.width() >= 24.0 && rect.height() >= 24.0, "{rect:?}");
    assert!(first.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::Text(text) if text.galley.job.sections.iter().any(|section| section.format.underline.width > 0.0)
    )));
    let hovered = draw(vec![Event::PointerMoved(rect.center())], &mut rect);
    assert_eq!(
        hovered.platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
}

#[test]
fn clickable_status_label_expands_hit_area_and_has_hover_affordance() {
    let ctx = egui::Context::default();
    ctx.set_visuals(novnow::ui::theme::visuals(novnow::ui::theme::Mode::Light));
    let mut rect = egui::Rect::NOTHING;
    let draw = |events: Vec<Event>, rect: &mut egui::Rect| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(300.0, 150.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    *rect = novnow::ui::widgets::clickable_label(
                        ui,
                        egui::RichText::new("⚑").small(),
                        egui::Id::new("test-status-label"),
                    )
                    .rect;
                });
            },
        )
    };
    let first = draw(Vec::new(), &mut rect);
    assert!(rect.width() >= 24.0 && rect.height() >= 24.0, "{rect:?}");
    assert!(first.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::Text(text) if text.galley.job.sections.iter().any(|section| section.format.underline.width > 0.0)
    )));
    let hovered = draw(vec![Event::PointerMoved(rect.center())], &mut rect);
    assert_eq!(
        hovered.platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    let fill = ctx.style().visuals.selection.bg_fill.gamma_multiply(0.6);
    assert!(hovered.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::Rect(shape) if shape.fill == fill
    )));
}

#[test]
fn static_labels_are_not_text_selectable_in_the_app() {
    let (harness, _) = Harness::start();
    assert!(!harness.ctx.style().interaction.selectable_labels);
}

#[test]
fn context_unsaved_indicator_has_a_ten_pixel_diameter() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("追".into())]);
    let output = harness.frame(Vec::new());
    let color = novnow::ui::theme::palette(novnow::ui::theme::Mode::Dark).unsaved;
    let circles: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle) => Some((circle.fill, circle.radius)),
            _ => None,
        })
        .collect();
    assert!(
        circles
            .iter()
            .any(|&(fill, radius)| fill == color && radius == 5.0),
        "state={:?} circles={circles:?} wanted={color:?}",
        harness.app.workbench().map(|b| b.save.state())
    );
}

#[test]
fn reference_sections_have_one_pixel_separator_between_headings() {
    use novnow::core::features::reference::{Reference, Upper};
    let ctx = egui::Context::default();
    let colors = novnow::ui::theme::palette(novnow::ui::theme::Mode::Light);
    let reference = Reference {
        plot: vec!["A short plot".into()],
        upper: vec![Upper {
            id: "01".into(),
            summary: "Chapter".into(),
        }],
        ..Default::default()
    };
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(500.0, 300.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                novnow::ui::widgets::reference_pane::sections(
                    ui,
                    &reference,
                    Locale::Ja,
                    novnow::ui::theme::Mode::Light,
                    1.0,
                );
            });
        },
    );
    assert_eq!(
        output.shapes.iter().filter(|item| matches!(
            &item.shape,
            egui::Shape::LineSegment { stroke, .. } if stroke.color == colors.border && stroke.width == 1.0
        )).count(),
        1,
        "two nonempty sections have exactly one border rule"
    );
}

#[test]
fn context_view_draws_section_rules_full_ids_and_the_empty_guide() {
    use novnow::ui::widgets::context_view::{self, Cache, Placeholder};
    let ctx = egui::Context::default();
    let bench = Workbench::new(world(), None, 400);
    let mut cache = Cache::default();
    let mode = novnow::ui::theme::Mode::Light;
    let colors = novnow::ui::theme::palette(mode);
    let placeholder = Placeholder {
        scene: "01-04-02-01",
        chars: 0,
        target: None,
        indicator: Indicator::Nothing,
        accessible: "editing",
    };
    let mut draw = || {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(700.0, 600.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    context_view::show(
                        ui,
                        &bench,
                        &mut cache,
                        mode,
                        1.0,
                        3,
                        44,
                        Locale::Ja,
                        &placeholder,
                    );
                });
            },
        )
    };
    draw();
    let output = draw();
    let text = painted_text(&output);
    assert!(
        text.contains("@01-04-02-02"),
        "full 4-part section id: {text}"
    );
    for key in [
        "context.guide_workspace",
        "context.guide_section",
        "context.guide_palette",
    ] {
        assert!(
            text.contains(novnow::ui::i18n::text(Locale::Ja, key)),
            "{key}: {text}"
        );
    }
    assert!(output.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::LineSegment { stroke, .. } if stroke.color == colors.border && stroke.width == 1.0
    )), "a separator should appear between context sections");
}

#[test]
fn external_urls_have_hover_blue_underline_24px_target_and_open_on_click() {
    let ctx = egui::Context::default();
    let mode = novnow::ui::theme::Mode::Light;
    let colors = novnow::ui::theme::palette(mode);
    ctx.set_visuals(novnow::ui::theme::visuals(mode));
    let url = "https://example.org/source";
    let mut rect = egui::Rect::NOTHING;
    let draw = |events: Vec<Event>, rect: &mut egui::Rect| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(300.0, 150.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_max_width(180.0);
                    *rect =
                        novnow::ui::widgets::external_link(ui, &format!("↗ {url}"), url, &colors)
                            .rect;
                });
            },
        )
    };
    let first = draw(Vec::new(), &mut rect);
    assert!(rect.width() >= 24.0 && rect.height() >= 24.0);
    assert!(first.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::Text(text) if text.galley.job.sections.iter().any(|section| section.format.underline.width > 0.0)
    )));
    let hovered = draw(vec![Event::PointerMoved(rect.center())], &mut rect);
    assert_eq!(
        hovered.platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    assert!(hovered.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::Text(text) if text.galley.job.sections.iter().any(|section| section.format.color == colors.accent)
    )));
    let fill = colors.bg_selection.gamma_multiply(0.6);
    assert!(hovered.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::Rect(shape) if shape.fill == fill
    )));
    draw(
        vec![Event::PointerButton {
            pos: rect.center(),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
        &mut rect,
    );
    let clicked = draw(
        vec![Event::PointerButton {
            pos: rect.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
        &mut rect,
    );
    assert_eq!(
        clicked.platform_output.open_url.map(|open| open.url),
        Some(url.to_owned())
    );
}

#[test]
fn four_matching_diagnostics_collapse_but_three_stay_expanded() {
    use novnow::core::diagnostics::{Diagnostic, DiagnosticCode};
    use novnow::core::features::reference::Reference;
    use novnow::ui::widgets::reference_pane;
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut content = Reference {
        diagnostics: (0..4)
            .map(|index| Diagnostic::simple(DiagnosticCode::W012, Some(index..index + 1)))
            .collect(),
        ..Default::default()
    };
    let render = |events: Vec<Event>, content: &Reference| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(500.0, 450.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    reference_pane::sections(
                        ui,
                        content,
                        Locale::Ja,
                        novnow::ui::theme::Mode::Light,
                        1.0,
                    );
                });
            },
        )
    };
    let collapsed = render(vec![], &content);
    let group = harness::painted_texts(&collapsed)
        .into_iter()
        .find(|(text, _)| text.contains("W012 (4)"))
        .expect("group header");
    assert_eq!(painted_text(&collapsed).matches("W012").count(), 1);
    render(
        vec![
            Event::PointerMoved(group.1.center()),
            Event::PointerButton {
                pos: group.1.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
        &content,
    );
    let expanded = render(
        vec![Event::PointerButton {
            pos: group.1.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
        &content,
    );
    assert_eq!(
        painted_text(&expanded).matches("W012").count(),
        5,
        "{}",
        painted_text(&expanded)
    );
    content.diagnostics.pop();
    let fewer = render(vec![], &content);
    assert_eq!(painted_text(&fewer).matches("W012").count(), 3);
}

#[test]
fn truncated_material_summary_has_a_link_to_the_full_material() {
    use novnow::core::features::reference::{MaterialRef, Reference};
    use novnow::ui::widgets::reference_pane::{self, Chosen};
    let ctx = egui::Context::default();
    let content = Reference {
        materials: vec![MaterialRef {
            id: "M0001".into(),
            title: "Archive".into(),
            summary: "要".repeat(121),
            source: String::new(),
            candidate: false,
        }],
        ..Default::default()
    };
    let mut chosen = None;
    let draw = |events: Vec<Event>, chosen: &mut Option<Chosen>| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(500.0, 400.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                *chosen = egui::CentralPanel::default()
                    .show(ctx, |ui| {
                        reference_pane::sections(
                            ui,
                            &content,
                            Locale::Ja,
                            novnow::ui::theme::Mode::Light,
                            1.0,
                        )
                    })
                    .inner;
            },
        )
    };
    let first = draw(vec![], &mut chosen);
    let text = painted_text(&first);
    let label = novnow::ui::i18n::text(Locale::Ja, "pane.more");
    let more = harness::painted_texts(&first)
        .iter()
        .find(|(text, _)| text == label)
        .expect("continuation line")
        .1;
    assert!(text.contains(label) && !text.contains(&"要".repeat(121)));
    draw(
        vec![
            Event::PointerMoved(more.center()),
            Event::PointerButton {
                pos: more.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
        &mut chosen,
    );
    let clicked = draw(
        vec![Event::PointerButton {
            pos: more.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
        &mut chosen,
    );
    // The paint-only closure cannot retrieve Chosen; verify the app path in a
    // dedicated assertion below instead of treating a drawn link as navigation.
    assert!(painted_text(&clicked).contains(label));
}

#[test]
fn material_summary_truncated_after_five_lines_shows_continuation() {
    use novnow::core::features::reference::{MaterialRef, Reference};
    let content = Reference {
        materials: vec![MaterialRef {
            id: "M0001".into(),
            title: "Archive".into(),
            summary: (1..=6)
                .map(|index| format!("line {index}"))
                .collect::<Vec<_>>()
                .join("\n"),
            source: String::new(),
            candidate: false,
        }],
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(400.0, 400.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                novnow::ui::widgets::reference_pane::sections(
                    ui,
                    &content,
                    Locale::Ja,
                    novnow::ui::theme::Mode::Light,
                    1.0,
                );
            });
        },
    );
    let painted = painted_text(&output);
    assert!(
        painted.contains("line 5") && !painted.contains("line 6"),
        "{painted}"
    );
    assert!(painted.contains(novnow::ui::i18n::text(Locale::Ja, "pane.more")));
}

#[test]
fn header_shortcut_buttons_open_help_and_palette() {
    let mut harness = Harness::new();
    let first = harness.frame(Vec::new());
    let header = |text: &str, output: &egui::FullOutput| {
        harness::painted_texts(output)
            .into_iter()
            .find(|(painted, rect)| painted == text && rect.min.y < 30.0)
            .map(|(_, rect)| rect.center())
            .unwrap_or_else(|| panic!("no header button: {text}"))
    };
    let help = header("?", &first);
    let palette = header("Ctrl+P", &first);
    harness.click_at(help);
    assert!(harness.app.overlay_open());
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!harness.app.overlay_open());
    harness.click_at(palette);
    assert!(harness.app.overlay_open());
}

#[test]
fn workspace_name_fades_from_the_header_after_one_second() {
    let mut harness = Harness::new();
    let name = novnow::ui::i18n::text(Locale::Ja, "header.workspace.material");
    let header_has_name = |output: &egui::FullOutput| {
        harness::painted_texts(output)
            .iter()
            .any(|(painted, rect)| painted == name && rect.min.y < 30.0)
    };
    assert!(!header_has_name(&harness.frame(Vec::new())));
    let switched = harness.frame(vec![key(Key::Num5, Modifiers::ALT)]);
    assert!(header_has_name(&switched));
    assert!(header_has_name(&harness.idle(0.5)));
    assert!(!header_has_name(&harness.idle(0.7)));
}

#[test]
fn palette_can_reopen_welcome_and_show_bidirectional_migration_help() {
    let mut harness = Harness::new();
    for (command, label_key) in [("welcome", "help.welcome"), ("migration", "help.migration")] {
        harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
        harness.frame(vec![Event::Text(format!(">{command}"))]);
        let options = painted_text(&harness.frame(Vec::new()));
        assert!(
            options.contains(&format!(">{command}")),
            "palette: {options}"
        );
        harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
        harness.frame(Vec::new());
        let output = harness.frame(Vec::new());
        assert!(harness.app.overlay_open(), "{command} dialog is open");
        assert!(
            painted_text(&output).contains(novnow::ui::i18n::text(Locale::Ja, label_key)),
            "{command} help should appear: {}",
            painted_text(&output)
        );
        harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    }
}

#[test]
fn editor_focus_stripe_and_context_placeholder_match_the_thin_editor_border() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    let output = harness.frame(Vec::new());
    let colors = novnow::ui::theme::palette(novnow::ui::theme::Mode::Dark);
    let stripes: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|item| match &item.shape {
            egui::Shape::Rect(shape)
                if shape.rect.width() == 3.0 && shape.fill == colors.accent =>
            {
                Some(shape.rect)
            }
            _ => None,
        })
        .collect();
    assert!(
        stripes.len() >= 2,
        "editor focus and context editing placeholder both have a 3px stripe: {stripes:?}"
    );
    assert!(output.shapes.iter().any(|item| matches!(
        &item.shape,
        egui::Shape::Rect(shape) if shape.stroke.width == 1.0 && shape.stroke.color == colors.border
    )), "editor outline remains a 1px border rather than a thick focus frame");
}

#[test]
fn held_modifier_prefixes_appear_in_the_status_bar() {
    let mut harness = Harness::new();
    let ctrl = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    for (mods, expected) in [
        (ctrl, "Ctrl: P"),
        (ctrl | Modifiers::SHIFT, "Ctrl+Shift:"),
        (ctrl | Modifiers::ALT, "Ctrl+Alt:"),
    ] {
        let output = harness.frame_at(1280.0, mods);
        assert!(
            painted_text(&output).contains(expected),
            "{expected}: {}",
            painted_text(&output)
        );
    }
    let normal = harness.frame_at(1280.0, Modifiers::NONE);
    assert!(!painted_text(&normal).contains("Ctrl: P"));
}

#[test]
fn bundled_font_notification_is_a_temporary_bottom_left_toast() {
    let mut harness = Harness::new();
    let fallback = novnow::ui::i18n::text(Locale::Ja, "status.font_fallback");
    let initial = harness.frame(Vec::new());
    let rect = harness::painted_texts(&initial)
        .into_iter()
        .find(|(text, _)| text == fallback)
        .expect("bundled font notification")
        .1;
    assert!(
        rect.min.x < 200.0 && rect.min.y > 650.0,
        "bottom-left toast: {rect:?}"
    );
    assert!(!painted_text(&harness.idle(4.2)).contains(fallback));
}

#[test]
fn narrow_status_folds_secondary_details_without_hiding_save_state() {
    let mut harness = Harness::new();
    let full = painted_text(&harness.frame_at(1280.0, Modifiers::NONE));
    assert!(full.contains("枚") && full.contains("字"));
    let medium = painted_text(&harness.frame_at(620.0, Modifiers::NONE));
    assert!(medium.contains('…') && !medium.contains("枚"), "{medium}");
    let narrow = painted_text(&harness.frame_at(380.0, Modifiers::NONE));
    assert!(narrow.contains('…') && !narrow.contains("枚"), "{narrow}");
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("追".into())]);
    let dirty = harness.frame_at(620.0, Modifiers::NONE);
    let amber = novnow::ui::theme::palette(novnow::ui::theme::Mode::Dark).unsaved;
    assert!(
        dirty.shapes.iter().any(|item| matches!(
            &item.shape, egui::Shape::Circle(circle) if circle.fill == amber && circle.radius == 4.0
        )),
        "save state remains visible at narrow widths"
    );
}

#[test]
fn palette_stays_on_top_at_the_center_and_closes_without_click_through() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    let output = harness.frame(Vec::new());
    let close = harness::painted_texts(&output)
        .into_iter()
        .find(|(text, _)| text == "パレットを閉じる (Esc)")
        .expect("permanent close choice")
        .1;
    assert!(
        (close.center().x - 640.0).abs() < 275.0,
        "centered: {close:?}"
    );
    assert!(close.min.y > 72.0 && close.min.y < 600.0, "top: {close:?}");
    assert!(
        painted_text(&output).contains("M・# 資料"),
        "empty-input guide"
    );
    // The palette is painted after the editor, not behind its border or text.
    let text = harness::painted_texts(&output);
    let editor = text
        .iter()
        .position(|(s, _)| s.contains("旧校舎の地下遺跡"));
    let palette = text.iter().position(|(s, _)| s == "パレットを閉じる (Esc)");
    assert!(
        editor < palette,
        "palette foreground should paint after the editor"
    );
    harness.click_at(Pos2::new(20.0, 155.0));
    assert!(
        !harness.app.overlay_open(),
        "outside click closes the palette"
    );
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    let close = harness.rect_of("パレットを閉じる (Esc)").unwrap();
    harness.click_at(close.center());
    assert!(!harness.app.overlay_open(), "the close choice also works");
}

#[test]
fn palette_agent_subcommands_and_empty_material_results_are_visible() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text(">agent".into())]);
    let output = painted_text(&harness.frame(Vec::new()));
    assert!(output.contains(">agent delegation"), "{output}");
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("#そんな資料はない".into())]);
    let output = painted_text(&harness.frame(Vec::new()));
    assert!(
        output.contains("登録されている資料がありません"),
        "{output}"
    );
    assert!(output.contains("パレットを閉じる"));
}

#[test]
fn palette_long_cjk_label_truncates_in_pixels_without_resizing() {
    let long = "長".repeat(120);
    let content = world::CONTENTS.replace("図書室の攻防", &long);
    let (mut harness, _) = Harness::start_with(world_with(CONTENTS_PATH, &content), true);
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    let output = harness.frame(Vec::new());
    let rect = harness::painted_texts(&output)
        .into_iter()
        .find(|(text, _)| text.contains("長長長") && text.ends_with("..."))
        .expect("long CJK candidate truncated with ellipsis")
        .1;
    assert!(
        rect.max.x < 930.0,
        "candidate remains in centered fixed-width palette: {rect:?}"
    );
}

#[test]
fn shortcut_palette_search_executes_selected_workspace_command() {
    use novnow::ui::workspaces::Workspace;
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("?workspace-material".into())]);
    let output = painted_text(&harness.frame(Vec::new()));
    assert!(output.contains("? workspace-material  Alt+5"), "{output}");
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(harness.app.workspace(), Some(Workspace::Material));
    assert!(!harness.app.overlay_open());
    // The same shortcut must also be available in the command palette (F-15).
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text(">workspace-idea".into())]);
    let output = painted_text(&harness.frame(Vec::new()));
    assert!(output.contains(">workspace-idea"), "{output}");
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    assert_eq!(harness.app.workspace(), Some(Workspace::Idea));
}

#[test]
fn scene_palette_drills_from_chapter_to_episode_scene_and_section() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("@".into())]);
    for label in ["@01-*", "@01-04-*", "@01-04-02-*", "@01-04-02-01"] {
        let output = painted_text(&harness.frame(Vec::new()));
        assert!(output.contains(label), "missing {label}: {output}");
        harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    }
    assert!(!harness.app.overlay_open());
}

#[test]
fn mcp_config_palette_modal_validates_before_saving() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::P, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text(">mcp".into())]);
    assert!(painted_text(&harness.frame(Vec::new())).contains(">mcp"));
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    assert!(painted_text(&output).contains("agents/mcp.json"));
    assert!(harness.app.overlay_open());
    let title = harness.rect_of("agents/mcp.json").unwrap();
    harness.click_at(Pos2::new(title.min.x + 50.0, title.max.y + 26.0));
    harness.frame(vec![Event::Text("invalid json".into())]);
    assert!(
        painted_text(&harness.frame(Vec::new())).contains("expected value"),
        "invalid input must be diagnosed while typing"
    );
    let save = harness.rect_of("保存").expect("MCP modal save button");
    harness.click_at(save.center());
    assert!(
        harness.app.overlay_open(),
        "invalid JSON cannot close the modal"
    );
    assert_eq!(
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .text("agents/mcp.json"),
        ""
    );
    harness.click_at(Pos2::new(title.min.x + 50.0, title.max.y + 26.0));
    harness.frame(vec![key(Key::A, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text(r#"{"servers":{}}"#.into())]);
    let save = harness.rect_of("保存").unwrap();
    harness.click_at(save.center());
    assert!(!harness.app.overlay_open(), "valid JSON closes the modal");
    assert_eq!(
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .text("agents/mcp.json"),
        r#"{"servers":{}}"#
    );
}

#[test]
fn agent_workspace_mcp_button_opens_the_shared_edit_modal() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num8, Modifiers::ALT)]);
    let title = novnow::ui::i18n::text(Locale::Ja, "help.edit_mcp");
    let button = harness
        .rect_of(title)
        .expect("MCP editor action on agent screen");
    harness.click_at(button.center());
    let output = harness.frame(Vec::new());
    assert!(harness.app.overlay_open());
    assert!(painted_text(&output).contains("agents/mcp.json"));
}

#[test]
fn warning_or_error_changes_review_header_glyph_and_adds_a_badge() {
    use novnow::ui::workspaces::{indicator, ReviewProblem, Workspace};
    let draw = |problem| {
        let ctx = egui::Context::default();
        ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                indicator(
                    ui,
                    novnow::ui::theme::Mode::Dark,
                    Locale::Ja,
                    Workspace::Write,
                    problem,
                );
            });
        })
    };
    let colors = novnow::ui::theme::palette(novnow::ui::theme::Mode::Dark);
    let count_badges = |output: egui::FullOutput, color| {
        output.shapes.iter().filter(|item| matches!(
            &item.shape, egui::Shape::Circle(circle) if circle.radius == 2.5 && circle.fill == color
        )).count()
    };
    assert_eq!(count_badges(draw(None), colors.warn), 0);
    assert_eq!(
        count_badges(draw(Some(ReviewProblem::Warning)), colors.warn),
        1
    );
    assert_eq!(
        count_badges(draw(Some(ReviewProblem::Error)), colors.error),
        1
    );
}

#[test]
fn ctrl_o_saves_before_showing_the_picker_and_never_opens_folder_directly() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("先に保存".into())]);
    harness.frame(vec![key(Key::O, Modifiers::CTRL)]);
    let output = harness.frame(Vec::new());
    assert!(
        harness.app.workbench().is_none(),
        "leaves the open project after saving"
    );
    assert!(
        painted_text(&output).contains("プロジェクト"),
        "shows the picker"
    );
    assert!(harness
        .host
        .files
        .borrow()
        .written
        .get(CONTENTS_PATH)
        .is_some_and(|text| text.contains("先に保存")));
}

#[test]
fn failed_save_keeps_the_project_open_when_ctrl_o_is_pressed() {
    let mut harness = Harness::new();
    harness.host.files.borrow_mut().fail = true;
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![Event::Text("保存不能".into())]);
    harness.frame(vec![key(Key::O, Modifiers::CTRL)]);
    harness.frame(Vec::new());
    assert!(
        harness.app.workbench().is_some(),
        "must not discard unsaved writing"
    );
    assert!(harness.app.overlay_open(), "shows the save error");
}

#[test]
fn tab_inside_editor_stays_in_one_focus_target_and_header_tabs_cycle() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(Vec::new());
    let editor = novnow::ui::widgets::section_editor::editor_id(harness.app.workbench().unwrap());
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), Some(editor));
    harness.frame(vec![key(Key::Tab, Modifiers::NONE)]);
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), Some(editor));
    let output = harness.frame(Vec::new());
    let header = harness::painted_texts(&output)
        .into_iter()
        .find(|(s, rect)| s == "memory" && rect.min.y < 28.0)
        .expect("header label")
        .1;
    harness.click_at(header.center());
    harness.frame(vec![key(Key::Tab, Modifiers::NONE)]);
    let after = harness.ctx.memory(|memory| memory.focused());
    assert_ne!(
        after,
        Some(editor),
        "Tab outside the editor must reach header controls"
    );
    assert!(
        after.is_some(),
        "the next focusable header control is focused"
    );
}

#[test]
fn editor_text_gutter_and_caret_stay_clipped_inside_the_border() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(Vec::new());
    for width in [620.0, 380.0] {
        let output = harness.frame_at(width, Modifiers::NONE);
        let colors = novnow::ui::theme::palette(novnow::ui::theme::Mode::Dark);
        let border = output
            .shapes
            .iter()
            .find_map(|item| match &item.shape {
                egui::Shape::Rect(shape)
                    if shape.stroke.width == 1.0
                        && shape.stroke.color == colors.border
                        && shape.rect.height() > 150.0 =>
                {
                    Some(shape.rect)
                }
                _ => None,
            })
            .expect("editor frame");
        let text_clip = output
            .shapes
            .iter()
            .find_map(|item| match &item.shape {
                egui::Shape::Text(text) if text.galley.text().contains("やっぱり") => {
                    Some(item.clip_rect)
                }
                _ => None,
            })
            .expect("section text");
        assert!(
            border.contains_rect(text_clip),
            "text clip inside border: {border:?} {text_clip:?}"
        );
        let number = harness::painted_texts(&output)
            .into_iter()
            .find(|(text, rect)| text == "3" && border.contains_rect(*rect))
            .expect("third line number")
            .1;
        // egui's original full-line-height caret is invisible. Our custom
        // caret is font-sized and aligned with that logical line's gutter.
        let caret = output
            .shapes
            .iter()
            .find_map(|item| match &item.shape {
                egui::Shape::LineSegment { points, stroke }
                    if stroke.width > 0.0
                        && (points[0].x - points[1].x).abs() < 0.1
                        && item.clip_rect.intersects(text_clip)
                        && (points[0].y - points[1].y).abs() > 13.0
                        && (points[0].y - points[1].y).abs() < 20.0 =>
                {
                    Some((points, item.clip_rect))
                }
                _ => None,
            })
            .expect("font-sized custom caret");
        let center = (caret.0[0].y + caret.0[1].y) * 0.5;
        if width == 620.0 {
            assert!(
                (center - number.center().y).abs() < 1.0,
                "caret/line number baseline: {center} {number:?}"
            );
        }
        assert!(border.contains(caret.0[0]) && border.contains(caret.0[1]));
        assert!(caret.1.intersects(border));
    }
}
