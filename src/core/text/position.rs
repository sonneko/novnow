//! 位置の相互変換（`SPECIFICATIONS.md` §3.4）。
//!
//! core は内部で **byte offset** を使う。LSP は UTF-16 を既定とするため、
//! `Utf8Byte ⇔ CharIndex ⇔ Utf16CodeUnit` の相互変換をこの 1 ファイルに集約する。
//! 変換規則が 2 箇所に散らばると、カーソル位置とバイト位置がずれたときに原因を
//! 特定できなくなる（§3.4「LSP 化の接続点」の前提）。
//!
//! **行の区切りは LF のみ**（§5.0.4「改行 LF / CRLF / CR を受理し、内部表現は LF」）。
//! CR や Unicode 改行（U+2028 等）は行区切りとして扱わない。CR は読み込み時の正規化で
//! LF になり、U+2028 は本文の 1 文字としてそのまま残る（§5.0.4「Unicode 正規化は行わない」）。
//! ropey 側も LF のみを行区切りにするよう feature を落としてある（`Cargo.toml`）。
//!
//! **桁あふれと範囲外**の方針: すべて **クランプ**する（パニックしない）。
//! 編集中の文書は「カーソルが行末を超えた位置」を一時的に持ち得るので、
//! 落とすより丸める方が呼び出し側が単純になる。境界にちょうど乗っている値はそのまま返る。

use std::ops::Range;

/// LSP 互換の位置。`line` / `character` はどちらも **0 始まり**。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Position {
    /// 行番号。
    pub line: usize,
    /// 行頭からの **UTF-16 コードユニット** 数（LSP の `character` 既定）。
    pub character: usize,
}

/// `character` を **char（Unicode スカラー値）数**で表す位置。
///
/// Rust 側の索引（`str::chars`）と 1:1 で対応する。LSP との受け渡しは [`Position`] を使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CharPosition {
    /// 行番号。
    pub line: usize,
    /// 行頭からの char 数。
    pub character: usize,
}

impl Position {
    /// 行の先頭（`character = 0`）。
    pub fn at_line_start(line: usize) -> Self {
        Self { line, character: 0 }
    }
}

impl CharPosition {
    /// 行の先頭（`character = 0`）。
    pub fn at_line_start(line: usize) -> Self {
        Self { line, character: 0 }
    }
}

/// 行頭の byte offset の表。
///
/// 文書本体（`&str`）は保持しない。行と byte の対応だけを O(log n) で引くための索引で、
/// 列（char / UTF-16）の変換は行のスライスを渡して行う（[`byte_col_to_utf16_col`] 等）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LineTable {
    /// 各行の先頭 byte offset。`starts[0] == 0`。
    starts: Vec<usize>,
    /// 文書の byte 長。
    text_len: usize,
}

impl LineTable {
    /// LF を行区切りとして表を作る。
    ///
    /// 空文書は「空の 1 行」を持つ（`line_count() == 1`）。末尾が LF で終わる文書は、
    /// その LF の後に空の 1 行がある数え方になる（ropey の `len_lines` と同じ）。
    pub fn new(text: &str) -> Self {
        let mut starts = Vec::new();
        starts.push(0);
        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                starts.push(idx + 1);
            }
        }
        Self {
            starts,
            text_len: text.len(),
        }
    }

    /// 行頭の byte offset が既に分かっているときの構築（[`super::rope::TextBuffer::line_table`] 用）。
    ///
    /// `starts` は **昇順**で `starts[0] == 0`、`text_len` は文書の byte 長であること。
    /// 空の `starts` は渡さない（最低でも `[0]`）。
    pub fn from_line_starts(starts: Vec<usize>, text_len: usize) -> Self {
        debug_assert!(!starts.is_empty());
        debug_assert!(starts[0] == 0);
        debug_assert!(starts.windows(2).all(|w| w[0] < w[1]));
        debug_assert!(starts[starts.len() - 1] <= text_len);
        Self { starts, text_len }
    }

    /// 索引を作った文書の byte 長。
    pub fn text_len(&self) -> usize {
        self.text_len
    }

    /// 行数（1 以上）。
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// `line` 行目の先頭 byte offset。範囲外なら `None`。
    pub fn line_start(&self, line: usize) -> Option<usize> {
        self.starts.get(line).copied()
    }

    /// `line` 行目の終端 byte offset（**改行を含まない**）。範囲外なら `None`。
    pub fn line_end(&self, line: usize) -> Option<usize> {
        if self.starts.get(line).is_none() {
            return None;
        }
        match self.starts.get(line + 1) {
            Some(&next) => Some(next - 1),
            None => Some(self.text_len),
        }
    }

    /// `line` 行目の byte 範囲（改行を含まない）。範囲外なら `None`。
    pub fn line_range(&self, line: usize) -> Option<Range<usize>> {
        let start = self.line_start(line)?;
        let end = self.line_end(line)?;
        Some(start..end)
    }

    /// `byte` が属する行。文書の範囲外はクランプする（パニックしない）。
    pub fn line_of_byte(&self, byte: usize) -> usize {
        if byte >= self.text_len {
            return self.starts.len() - 1;
        }
        // starts は昇順なので、byte を超えない最後の要素が行頭。
        let next = self.starts.partition_point(|&start| start <= byte);
        next.saturating_sub(1)
    }

    /// byte offset → LSP 位置。
    ///
    /// `byte` が char の境界でないときは **手前の境界に丸める**。範囲外はクランプする。
    /// `text` は **この表を作ったのと同じ文字列**であること（長さが違うと結果は未定義）。
    pub fn position_of_byte(&self, text: &str, byte: usize) -> Position {
        let byte = clamp_to_boundary(text, byte.min(self.text_len));
        let line = self.line_of_byte(byte);
        let start = self.line_start(line).unwrap_or(0);
        let column = utf16_len(line_slice(text, start, byte));
        Position {
            line,
            character: column,
        }
    }

    /// byte offset → char 基準の位置。
    pub fn char_position_of_byte(&self, text: &str, byte: usize) -> CharPosition {
        let byte = clamp_to_boundary(text, byte.min(self.text_len));
        let line = self.line_of_byte(byte);
        let start = self.line_start(line).unwrap_or(0);
        CharPosition {
            line,
            character: count_chars(line_slice(text, start, byte)),
        }
    }

    /// LSP 位置 → byte offset。行・列とも範囲外はクランプする。
    pub fn byte_of_position(&self, text: &str, position: Position) -> usize {
        let line = position.line.min(self.starts.len().saturating_sub(1));
        let start = self.line_start(line).unwrap_or(0);
        let end = self.line_end(line).unwrap_or(self.text_len);
        let slice = line_slice(text, start, end);
        start + utf16_col_to_byte_col(slice, position.character)
    }

    /// char 基準の位置 → byte offset。
    pub fn byte_of_char_position(&self, text: &str, position: CharPosition) -> usize {
        let line = position.line.min(self.starts.len().saturating_sub(1));
        let start = self.line_start(line).unwrap_or(0);
        let end = self.line_end(line).unwrap_or(self.text_len);
        let slice = line_slice(text, start, end);
        start + char_col_to_byte_col(slice, position.character)
    }

    /// LSP 位置 → char 基準の位置。
    pub fn char_position_of_position(&self, text: &str, position: Position) -> CharPosition {
        let byte = self.byte_of_position(text, position);
        self.char_position_of_byte(text, byte)
    }

    /// char 基準の位置 → LSP 位置。
    pub fn position_of_char_position(&self, text: &str, position: CharPosition) -> Position {
        let byte = self.byte_of_char_position(text, position);
        self.position_of_byte(text, byte)
    }
}

/// `s` を UTF-16 で符号化したときのコードユニット数。
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// `s` の char 数。
pub fn count_chars(s: &str) -> usize {
    s.chars().count()
}

/// 行内の byte 列 → UTF-16 列。
///
/// `byte_col` が char の境界でなければ手前の境界に丸め、行末を超えていれば行末にクランプする。
pub fn byte_col_to_utf16_col(line: &str, byte_col: usize) -> usize {
    let byte_col = clamp_to_boundary(line, byte_col.min(line.len()));
    utf16_len(&line[..byte_col])
}

/// 行内の UTF-16 列 → byte 列。
///
/// サロゲートペアの**間**（2 単位の char の途中）を指すときは、その char の先頭に丸める。
/// LSP クライアントが絵文字の途中を指してきても、Rust 側の境界に乗るようにするため。
pub fn utf16_col_to_byte_col(line: &str, utf16_col: usize) -> usize {
    let mut units = 0;
    for (idx, ch) in line.char_indices() {
        if units >= utf16_col {
            return idx;
        }
        units += ch.len_utf16();
        // 途中で目標を跨いだ（＝サロゲートペアの内側）なら、この char の先頭を返す。
        if units > utf16_col {
            return idx;
        }
    }
    line.len()
}

/// 行内の byte 列 → char 列。
pub fn byte_col_to_char_col(line: &str, byte_col: usize) -> usize {
    let byte_col = clamp_to_boundary(line, byte_col.min(line.len()));
    count_chars(&line[..byte_col])
}

/// 行内の char 列 → byte 列。範囲外は行末にクランプする。
pub fn char_col_to_byte_col(line: &str, char_col: usize) -> usize {
    match line.char_indices().nth(char_col) {
        Some((byte_idx, _)) => byte_idx,
        None => line.len(),
    }
}

/// 行内の char 列 → UTF-16 列。範囲外は行末にクランプする。
pub fn char_col_to_utf16_col(line: &str, char_col: usize) -> usize {
    let byte_col = char_col_to_byte_col(line, char_col);
    byte_col_to_utf16_col(line, byte_col)
}

/// 行内の UTF-16 列 → char 列。サロゲートペアの内側はその char の先頭に丸める。
pub fn utf16_col_to_char_col(line: &str, utf16_col: usize) -> usize {
    let byte_col = utf16_col_to_byte_col(line, utf16_col);
    byte_col_to_char_col(line, byte_col)
}

/// `byte` が char の境界でなければ、手前の境界に丸める。範囲外は末尾にクランプする。
fn clamp_to_boundary(text: &str, byte: usize) -> usize {
    if byte >= text.len() {
        return text.len();
    }
    let mut idx = byte;
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// `text[start..end]` を安全に取る（境界は丸める）。
fn line_slice(text: &str, start: usize, end: usize) -> &str {
    let start = clamp_to_boundary(text, start);
    let end = clamp_to_boundary(text, end.max(start));
    &text[start..end]
}
