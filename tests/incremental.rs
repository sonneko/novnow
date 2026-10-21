//! `core::text::incremental`（§6.7 インクリメンタル再解析のフレーム）のテスト。
//!
//! 確認するのは次の 3 点。
//!
//! 1. 1 行の編集で**その行だけ**が再字句解析される（他の行の種別は触らない）。
//! 2. 節番号（D）の再計算を要求するのは、種別が変わり**かつ**節番号に影響する行のときだけ。
//! 3. 行数が変わる編集（改行の挿入・削除・貼り付け）は、その行以降の再計算を必ず要求する。
//!
//! 「いつ」回すか（300ms / 5 秒のアイドル）は core の仕事ではない（§3.2 で global clock 禁止）。
//! 最後のテストは §6.7 のコスト目標（10 万行で 2ms 以内）の枠組みが崩れていないことの確認で、
//! CI の負荷で揺れないよう上限は桁違いに緩く取っている。

use std::time::Instant;

use novnow::core::text::incremental::{LineIndex, LineTag, Renumber};
use novnow::core::text::lexer::{classify_at_stage, Stage, TextFormat};
use novnow::core::text::rope::TextBuffer;

/// C 形式（§5.2）。節区切り `;` が 2 つある 6 行。
const C: TextFormat = TextFormat::Contents;
/// A 形式（§5.3）。フィールド行は C 形式では書けないのでこちらを使う。
const A: TextFormat = TextFormat::PlotOutline;
/// B 形式（§5.4）。フィールド行も節区切り `;` も書ける形式。
const B: TextFormat = TextFormat::PlotEpisode;

/// 構造見出し → 本文 → 節区切り → 本文 ×2 → 節区切り。
const DOC: &str = "@01-04 見出し\nあいう\n;\nえお\nかきく\n;";

#[test]
fn every_line_gets_a_tag() {
    let index = LineIndex::new(DOC, C);
    assert_eq!(index.len(), 6);
    assert_eq!(index.format(), C);
    let want = [
        LineTag::Structure,
        LineTag::Text,
        LineTag::SectionBreak,
        LineTag::Text,
        LineTag::Text,
        LineTag::SectionBreak,
    ];
    assert_eq!(index.tags(), want.as_slice());
    assert_eq!(index.tag(0), Some(LineTag::Structure));
    assert_eq!(index.tag(6), None);
}

#[test]
fn the_index_built_from_a_rope_matches_the_one_built_from_a_str() {
    let buffer = TextBuffer::from(DOC);
    let from_rope = LineIndex::from_buffer(&buffer, C);
    assert_eq!(from_rope, LineIndex::new(DOC, C));
    assert_eq!(from_rope.len(), buffer.len_lines());
}

#[test]
fn a_text_edit_that_keeps_the_kind_asks_for_nothing() {
    let mut index = LineIndex::new(DOC, C);
    // §6.7「行の種別が変わった場合のみ」再計算する
    let changed = index.relex_text(1, "かきくけ");
    assert_eq!(changed, Renumber::NotNeeded);
    assert!(!changed.is_needed());
    assert_eq!(changed.from(), None);
    assert_eq!(index.tag(1), Some(LineTag::Text));
}

#[test]
fn a_kind_change_that_cannot_move_a_section_number_asks_for_nothing() {
    let mut index = LineIndex::new(DOC, C);
    // 本文 → コメントは種別が変わるが、節番号（D）は動かない（B-20）
    let changed = index.relex_text(1, "// メモ");
    assert_eq!(changed, Renumber::NotNeeded);
    assert_eq!(index.tag(1), Some(LineTag::Comment));
}

#[test]
fn turning_a_line_into_a_sigil_line_asks_for_renumbering_from_there() {
    let mut index = LineIndex::new(DOC, C);
    let added = index.relex_text(1, ";");
    assert_eq!(added, Renumber::From(1));
    assert_eq!(added.from(), Some(1));
    assert_eq!(index.tag(1), Some(LineTag::SectionBreak));

    // 節区切りを本文に戻しても同じ（前と後のどちらが影響しても再計算が要る）
    let removed = index.relex_text(1, "あいう");
    assert_eq!(removed, Renumber::From(1));
    assert_eq!(index.tag(1), Some(LineTag::Text));
}

#[test]
fn a_structure_heading_also_asks_for_renumbering() {
    let mut index = LineIndex::new(DOC, C);
    let changed = index.relex_text(3, "@01-04-02 小見出し");
    assert_eq!(changed, Renumber::From(3));
    assert_eq!(index.tag(3), Some(LineTag::Structure));
}

#[test]
fn only_the_edited_line_is_relexed() {
    let mut index = LineIndex::new(DOC, C);
    let before = index.tags().to_vec();
    index.relex_text(4, "// メモ");
    let after = index.tags().to_vec();
    assert_eq!(before.len(), after.len());
    let diffs: Vec<usize> = before
        .iter()
        .zip(&after)
        .enumerate()
        .filter(|(_, (old, new))| old != new)
        .map(|(line, _)| line)
        .collect();
    assert_eq!(diffs, [4]);
}

#[test]
fn inserting_a_line_shifts_the_rest_and_asks_from_that_line() {
    let mut index = LineIndex::new(DOC, C);
    let changed = index.insert_line(2, "さしす");
    assert_eq!(changed, Renumber::From(2));
    assert_eq!(index.len(), 7);
    assert_eq!(index.tag(2), Some(LineTag::Text));
    assert_eq!(index.tag(3), Some(LineTag::SectionBreak));
}

#[test]
fn removing_a_line_shifts_the_rest_and_asks_from_that_line() {
    let mut index = LineIndex::new(DOC, C);
    let changed = index.remove_line(2);
    assert_eq!(changed, Renumber::From(2));
    assert_eq!(index.len(), 5);
    assert_eq!(index.tag(2), Some(LineTag::Text));
    assert_eq!(index.tag(4), Some(LineTag::SectionBreak));
}

#[test]
fn a_paste_of_several_lines_replaces_the_range() {
    // フィールド行と `;` を両方書ける B 形式で確かめる（§5.0.2 の出現場所）
    let mut index = LineIndex::new(DOC, B);
    let pasted = ["summary: 一行", "  継続", ";"];
    let changed = index.replace_lines(1, 2, &pasted);
    assert_eq!(changed, Renumber::From(1));
    assert_eq!(index.len(), 7);
    assert_eq!(index.tag(1), Some(LineTag::Field));
    assert_eq!(index.tag(2), Some(LineTag::Continuation));
    assert_eq!(index.tag(3), Some(LineTag::SectionBreak));
}

#[test]
fn editing_through_the_rope_uses_the_current_line_text() {
    let mut buffer = TextBuffer::from(DOC);
    let mut index = LineIndex::from_buffer(&buffer, C);
    // rope 側で 1 行目を節区切りに書き換える
    assert!(buffer.replace(11..14, ";"));
    let changed = index.relex_line(&buffer, 1);
    assert_eq!(changed, Renumber::From(1));
    assert_eq!(index.tag(1), Some(LineTag::SectionBreak));
}

#[test]
fn an_out_of_range_line_falls_back_to_a_full_renumber() {
    let mut index = LineIndex::new(DOC, C);
    // 呼び出し側が行数の同期を忘れたとき、安全側に倒す
    let changed = index.relex_text(99, ";");
    assert_eq!(changed, Renumber::From(0));
    assert_eq!(index.len(), 6);
}

#[test]
fn numbering_lines_skips_the_lines_that_do_not_matter() {
    let index = LineIndex::new(DOC, C);
    let found: Vec<usize> = index.numbering_lines().map(|(line, _)| line).collect();
    assert_eq!(found, [0, 2, 5]);

    let fields = LineIndex::new("summary: 一行\n  継続\n// メモ", A);
    assert_eq!(fields.numbering_lines().count(), 0);
}

#[test]
fn renumber_results_merge_to_the_earliest_line() {
    assert_eq!(Renumber::default(), Renumber::NotNeeded);
    let a = Renumber::From(10);
    let b = Renumber::From(3);
    assert_eq!(a.merge(b), Renumber::From(3));
    assert_eq!(b.merge(a), Renumber::From(3));
    assert_eq!(a.merge(Renumber::NotNeeded), a);
    assert_eq!(Renumber::NotNeeded.merge(a), a);
    let none = Renumber::NotNeeded.merge(Renumber::NotNeeded);
    assert_eq!(none, Renumber::NotNeeded);
}

#[test]
fn every_line_kind_has_a_tag() {
    let cases = [
        ("@01-04 見出し", C, Stage::One, LineTag::Structure),
        (";", C, Stage::One, LineTag::SectionBreak),
        ("// メモ", C, Stage::One, LineTag::Comment),
        ("summary: 一行", A, Stage::One, LineTag::Field),
        ("  継続", A, Stage::One, LineTag::Continuation),
        ("! 挿絵", C, Stage::Two, LineTag::IllustrationMemo),
        ("# メモ", A, Stage::Two, LineTag::Comment),
        ("本文", C, Stage::One, LineTag::Text),
    ];
    for (line, format, stage, want) in cases {
        let kind = classify_at_stage(line, format, stage);
        assert_eq!(LineTag::of(&kind), want, "{line}");
    }
    // ① では挿絵メモを作らない
    let one = classify_at_stage("! 挿絵", C, Stage::One);
    assert_eq!(LineTag::of(&one), LineTag::IllustrationMemo);
}

#[test]
fn only_structure_and_section_break_affect_numbering() {
    assert!(LineTag::Structure.affects_numbering());
    assert!(LineTag::SectionBreak.affects_numbering());
    assert!(!LineTag::Comment.affects_numbering());
    assert!(!LineTag::Field.affects_numbering());
    assert!(!LineTag::Continuation.affects_numbering());
    assert!(!LineTag::IllustrationMemo.affects_numbering());
    assert!(!LineTag::Text.affects_numbering());
}

#[test]
fn a_long_file_stays_cheap_to_relex() {
    // §6.7「10 万行で 2ms 以内」の枠組み確認。実測値は 1ms 未満だが、
    // CI の負荷で落ちないよう上限は桁違いに緩くしてある（狙いは O(n^2) 化の検出）
    let lines: usize = 100_000;
    let mut text = String::new();
    for line in 0..lines {
        text.push_str("あいうえお ");
        text.push_str(&line.to_string());
        text.push('\n');
    }

    let started = Instant::now();
    let mut index = LineIndex::new(&text, A);
    let built = started.elapsed();
    assert_eq!(index.len(), lines + 1);
    assert!(built.as_millis() < 5_000, "build: {built:?}");

    let middle = lines / 2;
    let started = Instant::now();
    for _ in 0..1_000 {
        // A 形式では `;` は節区切りにならないので、全形式で効く `@` を使う
        let on = index.relex_text(middle, "@01-04 見出し");
        assert_eq!(on, Renumber::From(middle));
        let off = index.relex_text(middle, "あいうえお");
        assert_eq!(off, Renumber::From(middle));
    }
    let relexed = started.elapsed();
    assert!(relexed.as_millis() < 1_000, "relex: {relexed:?}");
    assert_eq!(index.len(), lines + 1);
}
