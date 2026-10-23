//! ガイドモード（§9.10「はじめての手引き」）の画面テスト。
//!
//! 規則（段階の順序・飛ばし・自動で出る条件）は `src/ui/guide.rs` の単体テストが見る。
//! ここでは `NovnowApp` を 1 フレームずつ回して、**画面に出ること・入力を奪わないこと・
//! キーで閉じられること**を確かめる。

#![cfg(feature = "ui")]

mod harness;
mod world;

use egui::{Event, Key, Modifiers};

use novnow::ui::guide::{Advance, STEPS};
use novnow::ui::i18n::{self, Locale};

use harness::{key, painted_text, Harness};

/// 起動直後の重ね物（ウェルカム案内）を閉じてから、ガイドを出す。
fn shown() -> Harness {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!harness.app.overlay_open(), "起動直後の重ね物を閉じる");
    harness.app.start_guide();
    harness
}

/// 段階 1 の文言（両言語で同じキーを使う）。
fn step_text(index: usize) -> &'static str {
    i18n::text(Locale::Ja, STEPS[index].text)
}

#[test]
fn the_guide_paints_the_first_step_and_the_progress() {
    let mut harness = shown();
    // Area は最初のフレームで大きさを測るので、描画は 2 フレーム目から
    harness.frame(Vec::new());
    let text = painted_text(&harness.frame(Vec::new()));
    // 作品は既に開いているので段階 1 は満たされ、段階 2（ボタン）で止まる
    let index = harness.app.guide().index;
    assert!(text.contains(step_text(index)), "{index}: {text}");
    assert!(
        text.contains(i18n::text(Locale::Ja, "guide.title")),
        "{text}"
    );
}

#[test]
fn escape_closes_the_guide_and_remembers_it() {
    let mut harness = shown();
    harness.frame(Vec::new());
    assert!(harness.app.guide().active);

    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!harness.app.guide().active, "Esc で閉じる");
    assert!(harness.app.settings().guide_done, "閉じたことを覚える");
    let closed = painted_text(&harness.frame(Vec::new()));
    assert!(
        !closed.contains(i18n::text(Locale::Ja, "guide.title")),
        "閉じたら描かない"
    );
}

#[test]
fn the_guide_does_not_steal_the_editor_keys() {
    // 段階 2 は「次へ」でだけ進む。ここで Enter を送っても、改行はエディタに入り、段階は動かない。
    let mut harness = shown();
    harness.frame(Vec::new());
    // 段階 1（作品を開いた）は開いているので満たされ、段階 2（ボタン）で止まる
    let index = harness.app.guide().index;
    assert_eq!(STEPS[index].advance, Advance::Button);

    let before = harness.app.workbench().expect("開いている").buffer().len();
    harness.frame(vec![Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    let after = harness.app.workbench().expect("開いている").buffer().len();
    assert!(
        after >= before,
        "Enter は本文に渡る（ガイドは横取りしない）"
    );
    assert_eq!(harness.app.guide().index, index, "段階は動かない");
}

#[test]
fn finishing_the_tour_is_what_marks_it_done() {
    let mut harness = shown();
    harness.frame(Vec::new());
    // 段階 2 で「次へ」（`Alt+Enter`）
    harness.frame(vec![key(Key::Enter, Modifiers::ALT)]);
    assert!(harness.app.guide().active, "途中では閉じない");
    assert!(harness.app.guide().index >= 2);
}

#[test]
fn writing_sections_moves_the_tour_forward_by_itself() {
    // 段階 3 の条件は「節が 1 つ以上」。既に節があるので、観測した時点で先へ進む。
    let mut harness = shown();
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::Enter, Modifiers::ALT)]); // 段階 2 → 3
    let before = harness.app.guide().index;
    harness.frame(Vec::new());
    assert!(
        harness.app.guide().index > before,
        "条件を満たした段階は飛ばす（{} → {}）",
        before,
        harness.app.guide().index
    );
}

#[test]
fn the_guide_stays_out_of_the_way_of_overlays() {
    // ヘルプ（重ね物）を開いているあいだは吹き出しを描かない。
    let mut harness = shown();
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::F1, Modifiers::NONE)]);
    let text = painted_text(&harness.frame(Vec::new()));
    assert!(
        !harness.app.overlay_open() || !text.contains(i18n::text(Locale::Ja, "guide.title")),
        "重ね物の下では吹き出しを描かない"
    );
}
