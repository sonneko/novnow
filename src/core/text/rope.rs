//! テキストバッファ（`ropey` ラッパ）。
//!
//! §3.2 で core に許可された `ropey` は **このファイルに閉じ込める**。他の core モジュールは
//! [`TextBuffer`] の API だけを使い、`Rope` / `RopeSlice` を直接型に露出させない。
//! ラッパを噛ませる理由は 2 つ:
//!
//! 1. ropey は索引が **char 基準**で、core の内部表現は **byte 基準**（§3.4）。変換を
//!    ここで一元化し、列の計算は必ず [`super::position`] の関数を通す（実装を 2 箇所に
//!    散らさない）。
//! 2. ropey の既定 feature `unicode_lines` は U+2028 等も行区切りにするため、本文に
//!    それが含まれると行番号がずれる。§5.0.4「内部表現は LF」に合わせ、`Cargo.toml` で
//!    `default-features = false` にして **LF のみ**を行区切りにしている。
//!
//! 範囲外や境界外れの索引は `None` を返す（パニックしない）。ropey の `try_*` 系を使う。

use std::fmt;
use std::ops::Range;

use ropey::Rope;

use super::position::{
    byte_col_to_char_col, byte_col_to_utf16_col, char_col_to_byte_col, utf16_col_to_byte_col,
    CharPosition, LineTable, Position,
};

/// 編集中の本文。行区切りは LF のみ。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextBuffer {
    rope: Rope,
}

impl TextBuffer {
    /// 空のバッファ。
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    /// byte 長。
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// char 長。
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// 行数。空のバッファも「空の 1 行」を持つので 1。
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// 中身が無いか。
    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// `line_idx` 行目を**改行込みで**返す（ropey と同じ数え方）。範囲外なら `None`。
    pub fn line(&self, line_idx: usize) -> Option<String> {
        self.rope.get_line(line_idx).map(|slice| slice.to_string())
    }

    /// `line_idx` 行目を**改行を除いて**返す。範囲外なら `None`。
    ///
    /// 字句解析（[`super::lexer`]）は行の内容だけを必要とし、改行の有無は行の種別に影響しない。
    pub fn line_content(&self, line_idx: usize) -> Option<String> {
        let mut text = self.line(line_idx)?;
        if text.ends_with('\n') {
            text.pop();
        }
        Some(text)
    }

    /// byte 範囲を取り出す。範囲がはみ出すか char の境界でなければ `None`。
    pub fn byte_range(&self, byte_range: Range<usize>) -> Option<String> {
        self.rope
            .get_byte_slice(byte_range)
            .map(|slice| slice.to_string())
    }

    /// char 索引 → byte 索引。
    pub fn char_to_byte(&self, char_idx: usize) -> Option<usize> {
        self.rope.try_char_to_byte(char_idx).ok()
    }

    /// byte 索引 → char 索引。
    pub fn byte_to_char(&self, byte_idx: usize) -> Option<usize> {
        self.rope.try_byte_to_char(byte_idx).ok()
    }

    /// byte 索引 → 行番号。
    pub fn byte_to_line(&self, byte_idx: usize) -> Option<usize> {
        self.rope.try_byte_to_line(byte_idx).ok()
    }

    /// 行番号 → その行の先頭の byte 索引。
    pub fn line_to_byte(&self, line_idx: usize) -> Option<usize> {
        self.rope.try_line_to_byte(line_idx).ok()
    }

    /// 行番号 → その行の先頭の char 索引。
    pub fn line_to_char(&self, line_idx: usize) -> Option<usize> {
        self.rope.try_line_to_char(line_idx).ok()
    }

    /// byte 索引 → LSP 位置（`character` は UTF-16 コードユニット）。
    ///
    /// 列の計算は [`super::position::byte_col_to_utf16_col`] に委譲する。
    /// 行末（改行の直前）を超える列は行末にクランプする。
    pub fn position_of_byte(&self, byte_idx: usize) -> Option<Position> {
        let (line, content, column_byte) = self.split_offset(byte_idx)?;
        Some(Position {
            line,
            character: byte_col_to_utf16_col(&content, column_byte),
        })
    }

    /// LSP 位置 → byte 索引。行が範囲外なら `None`、列は行末にクランプ。
    pub fn byte_of_position(&self, position: Position) -> Option<usize> {
        let (line, content) = self.line_at(position.line)?;
        Some(line + utf16_col_to_byte_col(&content, position.character))
    }

    /// byte 索引 → char 基準の位置。
    pub fn char_position_of_byte(&self, byte_idx: usize) -> Option<CharPosition> {
        let (line, content, column_byte) = self.split_offset(byte_idx)?;
        Some(CharPosition {
            line,
            character: byte_col_to_char_col(&content, column_byte),
        })
    }

    /// char 基準の位置 → byte 索引。
    pub fn byte_of_char_position(&self, position: CharPosition) -> Option<usize> {
        let (start, content) = self.line_at(position.line)?;
        Some(start + char_col_to_byte_col(&content, position.character))
    }

    /// 行頭の byte offset 表を作る（[`super::position::LineTable`]）。
    ///
    /// 本文を丸ごと `String` にせず、行の byte 長を積み上げて作る。
    pub fn line_table(&self) -> LineTable {
        let mut starts = Vec::with_capacity(self.rope.len_lines());
        let mut acc = 0;
        for line in self.rope.lines() {
            starts.push(acc);
            acc += line.len_bytes();
        }
        LineTable::from_line_starts(starts, self.rope.len_bytes())
    }

    /// `char_idx` の位置に `text` を挿入する。範囲外なら何もせず `false`。
    pub fn insert(&mut self, char_idx: usize, text: &str) -> bool {
        self.rope.try_insert(char_idx, text).is_ok()
    }

    /// char 範囲を削除する。範囲がはみ出せば何もせず `false`。
    pub fn remove(&mut self, char_range: Range<usize>) -> bool {
        self.rope.try_remove(char_range).is_ok()
    }

    /// char 範囲を `text` で置き換える。範囲がはみ出せば何もせず `false`。
    pub fn replace(&mut self, char_range: Range<usize>, text: &str) -> bool {
        let start = char_range.start;
        if self.rope.try_remove(char_range).is_err() {
            return false;
        }
        // 削除に成功したなら `start` は削除後の本文でも有効な char 境界。
        self.rope.try_insert(start, text).is_ok()
    }

    /// 内部の rope。ropey の反復子（`chunks` 等）を直接使う必要があるときだけ。
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// `byte_idx` を「行番号・行の内容・行内の byte 列」に分解する。
    fn split_offset(&self, byte_idx: usize) -> Option<(usize, String, usize)> {
        let line = self.rope.try_byte_to_line(byte_idx).ok()?;
        let start = self.rope.try_line_to_byte(line).ok()?;
        let content = self.line_content(line)?;
        let column = byte_idx.saturating_sub(start).min(content.len());
        Some((line, content, column))
    }

    /// `line_idx` 行の先頭 byte と行の内容を返す。行が範囲外なら `None`。
    fn line_at(&self, line_idx: usize) -> Option<(usize, String)> {
        if line_idx >= self.rope.len_lines() {
            return None;
        }
        let start = self.rope.try_line_to_byte(line_idx).ok()?;
        let content = self.line_content(line_idx)?;
        Some((start, content))
    }
}

impl From<&str> for TextBuffer {
    fn from(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }
}

impl From<String> for TextBuffer {
    fn from(text: String) -> Self {
        Self {
            rope: Rope::from(text),
        }
    }
}

impl fmt::Display for TextBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.rope, f)
    }
}
