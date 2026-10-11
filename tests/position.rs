//! `core::text::position` と `core::text::rope` のテスト。
//!
//! `PLANS.md` §5.2 完了条件「`tests/position.rs`: 日本語 + 絵文字混在で byte/char/utf16 の
//! 往復が一致する」に対応する。§3.4「core は内部的に byte offset を使う。
//! `Utf8Byte ⇔ CharIndex ⇔ Utf16CodeUnit` を相互変換する（LSP は UTF-16 既定のため MUST）」。

use novnow::core::text::position;
use novnow::core::text::rope::TextBuffer;

/// 日本語（3 バイト）・ASCII・絵文字（サロゲートペア）が混ざる 3 行の本文。
///
/// ```text
/// 行 0: あ a 😀 い   （byte 0..11, 改行は 11）
/// 行 1: b             （byte 12..13, 改行は 13）
/// 行 2: う 😀         （byte 14..21）
/// ```
const MIXED: &str = "あa😀い\nb\nう😀";

/// LSP 位置（UTF-16 列）を作る。
fn pos(line: usize, character: usize) -> position::Position {
    position::Position { line, character }
}

/// char 基準の位置を作る。
fn cpos(line: usize, character: usize) -> position::CharPosition {
    position::CharPosition { line, character }
}

/// `text` の char 境界の byte offset だけを列挙する。
fn boundaries(text: &str) -> Vec<usize> {
    (0..=text.len()).filter(|&b| text.is_char_boundary(b)).collect()
}

// ------------------------------------------------------------- LineTable

#[test]
fn line_table_counts_lines() {
    assert_eq!(position::LineTable::new("").line_count(), 1);
    assert_eq!(position::LineTable::new("a").line_count(), 1);
    // 末尾の改行の後は「空の 1 行」（ropey の len_lines と同じ数え方）
    assert_eq!(position::LineTable::new("a\n").line_count(), 2);
    assert_eq!(position::LineTable::new("a\nb").line_count(), 2);
    assert_eq!(position::LineTable::new("a\nb\n").line_count(), 3);
    assert_eq!(position::LineTable::new(MIXED).line_count(), 3);
}

#[test]
fn line_table_ranges_exclude_the_newline() {
    let table = position::LineTable::new(MIXED);
    assert_eq!(table.text_len(), 21);
    assert_eq!(table.line_range(0), Some(0..11));
    assert_eq!(table.line_range(1), Some(12..13));
    assert_eq!(table.line_range(2), Some(14..21));
    assert_eq!(table.line_range(3), None);
    assert_eq!(table.line_start(1), Some(12));
    assert_eq!(table.line_end(1), Some(13));
    assert_eq!(table.line_end(2), Some(21));
}

#[test]
fn line_table_locates_the_line_of_a_byte() {
    let table = position::LineTable::new(MIXED);
    assert_eq!(table.line_of_byte(0), 0);
    assert_eq!(table.line_of_byte(3), 0);
    // 改行そのものは前の行に属する
    assert_eq!(table.line_of_byte(11), 0);
    assert_eq!(table.line_of_byte(12), 1);
    assert_eq!(table.line_of_byte(13), 1);
    assert_eq!(table.line_of_byte(14), 2);
    assert_eq!(table.line_of_byte(20), 2);
    // 範囲外は最後の行にクランプする（パニックしない）
    assert_eq!(table.line_of_byte(21), 2);
    assert_eq!(table.line_of_byte(9999), 2);
}

#[test]
fn utf16_len_counts_a_surrogate_pair_as_two() {
    assert_eq!(position::utf16_len("あ"), 1);
    assert_eq!(position::utf16_len("😀"), 2);
    assert_eq!(position::utf16_len("あa😀い"), 5);
    assert_eq!(position::count_chars("あa😀い"), 4);
    assert_eq!(position::utf16_len(""), 0);
}

#[test]
fn byte_column_converts_to_utf16_column() {
    let line = "あa😀い";
    assert_eq!(position::byte_col_to_utf16_col(line, 0), 0);
    assert_eq!(position::byte_col_to_utf16_col(line, 3), 1);
    assert_eq!(position::byte_col_to_utf16_col(line, 4), 2);
    assert_eq!(position::byte_col_to_utf16_col(line, 8), 4);
    assert_eq!(position::byte_col_to_utf16_col(line, 11), 5);
    // char の境界でない byte は手前の境界に丸める
    assert_eq!(position::byte_col_to_utf16_col(line, 1), 0);
    assert_eq!(position::byte_col_to_utf16_col(line, 5), 2);
    assert_eq!(position::byte_col_to_utf16_col(line, 6), 2);
    // 行末を超えたらクランプ
    assert_eq!(position::byte_col_to_utf16_col(line, 999), 5);
}

#[test]
fn utf16_column_converts_to_byte_column() {
    let line = "あa😀い";
    assert_eq!(position::utf16_col_to_byte_col(line, 0), 0);
    assert_eq!(position::utf16_col_to_byte_col(line, 1), 3);
    assert_eq!(position::utf16_col_to_byte_col(line, 2), 4);
    // サロゲートペアの内側（列 3）はその文字の先頭に丸める
    assert_eq!(position::utf16_col_to_byte_col(line, 3), 4);
    assert_eq!(position::utf16_col_to_byte_col(line, 4), 8);
    assert_eq!(position::utf16_col_to_byte_col(line, 5), 11);
    assert_eq!(position::utf16_col_to_byte_col(line, 99), 11);
    assert_eq!(position::utf16_col_to_byte_col("", 3), 0);
}

#[test]
fn char_column_conversions() {
    let line = "あa😀い";
    assert_eq!(position::byte_col_to_char_col(line, 8), 2);
    assert_eq!(position::byte_col_to_char_col(line, 5), 2);
    assert_eq!(position::char_col_to_byte_col(line, 2), 4);
    assert_eq!(position::char_col_to_byte_col(line, 4), 11);
    assert_eq!(position::char_col_to_byte_col(line, 99), 11);
    assert_eq!(position::char_col_to_utf16_col(line, 2), 2);
    assert_eq!(position::char_col_to_utf16_col(line, 3), 4);
    assert_eq!(position::utf16_col_to_char_col(line, 3), 2);
    assert_eq!(position::utf16_col_to_char_col(line, 4), 3);
}

#[test]
fn positions_round_trip_on_every_char_boundary() {
    let table = position::LineTable::new(MIXED);
    for byte in boundaries(MIXED) {
        let lsp = table.position_of_byte(MIXED, byte);
        assert_eq!(table.byte_of_position(MIXED, lsp), byte, "byte {byte}");

        let chars = table.char_position_of_byte(MIXED, byte);
        assert_eq!(table.byte_of_char_position(MIXED, chars), byte, "byte {byte}");

        // 2 つの位置表現も互いに往復できる
        assert_eq!(table.position_of_char_position(MIXED, chars), lsp);
        assert_eq!(table.char_position_of_position(MIXED, lsp), chars);
    }
}

#[test]
fn positions_have_expected_values() {
    let table = position::LineTable::new(MIXED);
    assert_eq!(table.position_of_byte(MIXED, 0), pos(0, 0));
    assert_eq!(table.position_of_byte(MIXED, 3), pos(0, 1));
    assert_eq!(table.position_of_byte(MIXED, 8), pos(0, 4));
    assert_eq!(table.position_of_byte(MIXED, 11), pos(0, 5));
    assert_eq!(table.position_of_byte(MIXED, 12), pos(1, 0));
    assert_eq!(table.position_of_byte(MIXED, 13), pos(1, 1));
    assert_eq!(table.position_of_byte(MIXED, 14), pos(2, 0));
    assert_eq!(table.position_of_byte(MIXED, 21), pos(2, 3));

    assert_eq!(table.char_position_of_byte(MIXED, 8), cpos(0, 3));
    assert_eq!(table.char_position_of_byte(MIXED, 14), cpos(2, 0));
}

#[test]
fn positions_clamp_instead_of_panicking() {
    let table = position::LineTable::new(MIXED);
    // 列が長い → 行末
    assert_eq!(table.byte_of_position(MIXED, pos(0, 99)), 11);
    assert_eq!(table.byte_of_position(MIXED, pos(2, 99)), 21);
    // 行番号が大きい → 最後の行
    assert_eq!(table.byte_of_position(MIXED, pos(99, 0)), 14);
    // サロゲートペアの内側 → その文字の先頭
    assert_eq!(table.byte_of_position(MIXED, pos(2, 2)), 17);
    assert_eq!(table.byte_of_position(MIXED, pos(2, 3)), 21);
    // byte が char 境界でない → 手前の境界
    assert_eq!(table.position_of_byte(MIXED, 5), pos(0, 2));
    // 文書の外 → 末尾にクランプ
    assert_eq!(table.position_of_byte(MIXED, 9999), pos(2, 3));
    assert_eq!(table.char_position_of_byte(MIXED, 9999), cpos(2, 2));
}

// ------------------------------------------------------------ TextBuffer

#[test]
fn text_buffer_matches_line_table() {
    // 実装が 2 箇所（ropey の索引 / LineTable）に散らないことの確認。
    // 同じ本文に対して両者が同じ位置を返さなければ、カーソルがずれる。
    let texts = ["", "a", "a\n", "a\nb\n", MIXED, "日本語の本文\n2 行目\n"];
    for text in texts {
        let buffer = TextBuffer::from(text);
        let table = position::LineTable::new(text);
        assert_eq!(buffer.len_lines(), table.line_count(), "text: {text:?}");
        assert_eq!(buffer.len_bytes(), table.text_len(), "text: {text:?}");
        assert_eq!(buffer.line_table(), table, "text: {text:?}");

        for byte in boundaries(text) {
            assert_eq!(
                buffer.position_of_byte(byte),
                Some(table.position_of_byte(text, byte)),
                "text: {text:?} byte {byte}"
            );
            assert_eq!(
                buffer.char_position_of_byte(byte),
                Some(table.char_position_of_byte(text, byte)),
                "text: {text:?} byte {byte}"
            );
        }

        for line in 0..table.line_count() {
            let range = table.line_range(line).expect("line range");
            assert_eq!(
                buffer.line_content(line),
                Some(text[range].to_string()),
                "text: {text:?} line {line}"
            );
        }
    }
}

#[test]
fn text_buffer_exposes_lines() {
    let buffer = TextBuffer::from(MIXED);
    assert_eq!(buffer.len_lines(), 3);
    assert_eq!(buffer.len_bytes(), 21);
    assert_eq!(buffer.len_chars(), 9);
    assert!(!buffer.is_empty());
    // `line` は改行込み、`line_content` は改行なし
    assert_eq!(buffer.line(0), Some("あa😀い\n".to_string()));
    assert_eq!(buffer.line_content(0), Some("あa😀い".to_string()));
    assert_eq!(buffer.line(1), Some("b\n".to_string()));
    assert_eq!(buffer.line(2), Some("う😀".to_string()));
    assert_eq!(buffer.line_content(2), Some("う😀".to_string()));
    assert_eq!(buffer.line(3), None);
    assert_eq!(buffer.to_string(), MIXED);

    let empty = TextBuffer::new();
    assert!(empty.is_empty());
    assert_eq!(empty.len_lines(), 1);
    assert_eq!(empty.line_content(0), Some(String::new()));
}

#[test]
fn text_buffer_reads_byte_ranges() {
    let buffer = TextBuffer::from(MIXED);
    assert_eq!(buffer.byte_range(0..11), Some("あa😀い".to_string()));
    assert_eq!(buffer.byte_range(12..13), Some("b".to_string()));
    assert_eq!(buffer.byte_range(14..21), Some("う😀".to_string()));
    // char の境界でない範囲・はみ出す範囲は None（パニックしない）
    assert_eq!(buffer.byte_range(0..2), None);
    assert_eq!(buffer.byte_range(0..999), None);
    assert_eq!(buffer.byte_range(999..1000), None);
}

#[test]
fn text_buffer_converts_indexes() {
    let buffer = TextBuffer::from(MIXED);
    assert_eq!(buffer.char_to_byte(2), Some(4));
    assert_eq!(buffer.byte_to_char(4), Some(2));
    assert_eq!(buffer.byte_to_line(12), Some(1));
    assert_eq!(buffer.line_to_byte(2), Some(14));
    assert_eq!(buffer.line_to_char(1), Some(5));
    assert_eq!(buffer.line_to_char(2), Some(7));
    // 範囲外は None
    assert_eq!(buffer.char_to_byte(999), None);
    assert_eq!(buffer.line_to_byte(99), None);
    assert_eq!(buffer.byte_to_line(9999), None);
}

#[test]
fn text_buffer_edits_by_char_index() {
    let mut buffer = TextBuffer::from("あa\nb");
    assert!(buffer.insert(1, "X"));
    assert_eq!(buffer.to_string(), "あXa\nb");

    assert!(buffer.remove(0..1));
    assert_eq!(buffer.to_string(), "Xa\nb");

    assert!(buffer.replace(0..1, "日本語"));
    assert_eq!(buffer.to_string(), "日本語a\nb");
    assert_eq!(buffer.len_lines(), 2);

    // 範囲外は何もせず false
    assert!(!buffer.insert(999, "x"));
    assert!(!buffer.remove(0..999));
    assert_eq!(buffer.to_string(), "日本語a\nb");

    // 編集後は位置の変換も新しい本文に追随する。
    // "日本語a\nb" の byte 3 は「1 文字目」だが UTF-16 列は 3（ここが char 列と違う）。
    assert_eq!(buffer.position_of_byte(0), Some(pos(0, 0)));
    assert_eq!(buffer.position_of_byte(3), Some(pos(0, 3)));
    assert_eq!(buffer.char_position_of_byte(3), Some(cpos(0, 1)));
    assert_eq!(buffer.byte_of_position(pos(0, 3)), Some(3));
    assert_eq!(buffer.byte_of_char_position(cpos(0, 1)), Some(3));
}

#[test]
fn text_buffer_is_built_from_strings() {
    let owned = TextBuffer::from(String::from("あ\n"));
    assert_eq!(owned.len_lines(), 2);
    assert_eq!(owned.to_string(), "あ\n");

    let borrowed = TextBuffer::from("あ\n");
    assert_eq!(borrowed, owned);
}
