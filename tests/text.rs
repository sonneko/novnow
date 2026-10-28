//! `core::text::charclass`（文字種）と `core::text::sentence`（文の区切り）のテスト。
//!
//! ここで固定するのは**いまの範囲**であって「正しい範囲」ではない。半角カナ・拡張 A・
//! 互換漢字を日本語に数えるかは仕様の判断なので、判断が変わったらこのファイルの期待値を
//! 直す（`docs/history.md` の 2026-10-28 の項目を参照）。

use novnow::core::text::charclass::{self, CharClass};
use novnow::core::text::sentence;

#[test]
fn the_character_classes_match_the_morpheme_boundary_rule() {
    // §6.2.6 の「漢字・カタカナ・ひらがな・英数字」の境界
    assert_eq!(charclass::classify('あ'), CharClass::Hiragana);
    assert_eq!(charclass::classify('ゝ'), CharClass::Hiragana);
    assert_eq!(charclass::classify('ア'), CharClass::Katakana);
    assert_eq!(charclass::classify('ー'), CharClass::Katakana);
    assert_eq!(charclass::classify('ﾞ'), CharClass::Katakana); // 半角の濁点
    assert_eq!(charclass::classify('漢'), CharClass::Kanji);
    assert_eq!(charclass::classify('々'), CharClass::Kanji);
    assert_eq!(charclass::classify('〆'), CharClass::Kanji);
    assert_eq!(charclass::classify('Ｍ'), CharClass::Alphanumeric);
    assert_eq!(charclass::classify('7'), CharClass::Alphanumeric);
    assert_eq!(charclass::classify('７'), CharClass::Alphanumeric);
    assert_eq!(charclass::classify('、'), CharClass::Other);
    assert_eq!(charclass::classify('・'), CharClass::Other);
    // 拡張 A（U+3400）は §6.2.6 の列挙に無いので Other
    assert_eq!(charclass::classify('㐀'), CharClass::Other);
}

#[test]
fn ruby_base_kanji_includes_extended_and_compatible_ideographs() {
    assert!(charclass::is_ruby_base_kanji('漢'));
    assert!(charclass::is_ruby_base_kanji('㐀')); // 拡張 A U+3400
    assert!(charclass::is_ruby_base_kanji('﨑')); // 互換漢字 U+FA11
    assert!(!charclass::is_ruby_base_kanji('々')); // 繰返し字は別の述語
    assert!(!charclass::is_ruby_base_kanji('あ'));
    assert!(!charclass::is_ruby_base_kanji('ｱ'));

    assert!(charclass::is_kanji_mark('々'));
    assert!(charclass::is_kanji_mark('〻'));
    assert!(!charclass::is_kanji_mark('〆'));
}

#[test]
fn the_body_language_rule_counts_only_the_plain_scripts() {
    for c in ['あ', 'ア', 'ー', '漢'] {
        assert!(charclass::is_body_japanese(c), "{c} は日本語に数える");
    }
    // 半角カナ・`ゝ`・拡張 A・互換漢字・繰返し字は数えない（§7.2.2 の実装値）
    for c in ['ｱ', 'ｰ', 'ゝ', '㐀', '﨑', '々', '、'] {
        assert!(!charclass::is_body_japanese(c), "{c} は数えない");
    }
    assert!(charclass::is_body_latin('a'));
    assert!(charclass::is_body_latin('Z'));
    assert!(!charclass::is_body_latin('ａ')); // 全角英字は数えない
    assert!(!charclass::is_body_latin('д')); // キリル文字はどちらでもない
}

#[test]
fn word_characters_follow_the_unicode_properties() {
    // 文字プロパティで判定するので、範囲で絞る `classify` より広い
    // `・`（U+30FB）は仮名の範囲（U+3041–U+30FF）に入るので語になる
    for c in [
        'あ', 'ア', '漢', '㐀', '﨑', '々', 'ｱ', 'ー', '・', 'A', 'ａ', '１', '①', 'д',
    ] {
        assert!(charclass::is_word_char(c), "{c} は語になる");
    }
    // 標点と空白は語にならない
    for c in ['、', '。', '「', '」', '『', '…', ' ', '　'] {
        assert!(!charclass::is_word_char(c), "{c} は語にならない");
    }
}

#[test]
fn the_mojibake_gates_use_kana_and_halfwidth_katakana() {
    assert!(charclass::is_kana('あ'));
    assert!(charclass::is_kana('ア'));
    assert!(!charclass::is_kana('ー'));
    assert!(!charclass::is_kana('漢'));
    assert!(!charclass::is_kana('ｱ')); // 半角カナは別の述語（指紋側）

    assert!(charclass::is_halfwidth_katakana('ｱ'));
    assert!(charclass::is_halfwidth_katakana('ｰ'));
    assert!(!charclass::is_halfwidth_katakana('ﾞ')); // U+FF9E は含めない
    assert!(!charclass::is_halfwidth_katakana('ア'));
}

#[test]
fn sentences_keep_their_stop_and_report_byte_ranges() {
    let text = "走った。止まれ！なぜ？";
    let found: Vec<_> = sentence::split(text).collect();
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].1, "走った。");
    assert_eq!(found[1].1, "止まれ！");
    assert_eq!(found[2].1, "なぜ？");
    assert_eq!(&text[found[1].0.clone()], "止まれ！");
    // 範囲は連結した本文の中の位置
    assert_eq!(found[0].0, 0..12);
    assert_eq!(found[1].0, 12..24);
    assert_eq!(found[2].0, 24..33);
}

#[test]
fn sentences_do_not_stop_at_an_ellipsis() {
    // §9.4.4 の W 系は `。！？` だけで切る（役割語の §6.5.2 とは単位が違う）
    let found: Vec<_> = sentence::split("まだ……そうか。").collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1, "まだ……そうか。");

    // 改行は連続を切らない（節の中で連結してから渡す前提）
    let found: Vec<_> = sentence::split("一行目\n二行目。").collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1, "一行目\n二行目。");
}

#[test]
fn sentences_without_a_stop_and_empty_text() {
    let found: Vec<_> = sentence::split("句点の無い行").collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1, "句点の無い行");
    assert_eq!(found[0].0, 0.."句点の無い行".len());
    assert_eq!(sentence::split("").count(), 0);
}

#[test]
fn sentence_stops_are_the_three_marks_of_the_writing_checks() {
    assert_eq!(sentence::STOPS, &['。', '！', '？']);
}
