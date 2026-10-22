//! 位置の往復 — LSP の行・列（UTF-16）⇔ 内部の byte 位置（§19.4.1）。
//!
//! LSP の `character` は **UTF-16 コードユニット**、内部は byte。日本語は 1 文字 3 byte
//! （UTF-8）で 1 ユニット（UTF-16）、サロゲートペアは 1 文字 4 byte で 2 ユニット。
//! 両者の往復は受け入れ基準 H5-AC-2 の性質テストで確かめる。

use std::ops::Range;

use serde_json::{json, Value};

use crate::core::features::project as project_features;

/// 行・列（0 始まり）を byte 位置にする。
///
/// 行が行の数を超えたら最後の行に、列が行の長さを超えたら行末に丸める（LSP の範囲の
/// 丸めと同じ扱い）。行・列が文字の境目に乗っていなければ、その文字の頭に丸める。
pub fn offset(text: &str, line: u32, character: u32) -> usize {
    let starts = project_features::line_starts_of(text);
    let index = (line as usize).min(starts.len().saturating_sub(1));
    let start = starts[index];
    let mut end = starts.get(index + 1).copied().unwrap_or(text.len());
    // 行末の改行は行に含めない（列が行末を指すときの丸め先）。
    if end > start && text[..end].ends_with('\n') {
        end -= 1;
    }
    let mut units = 0u32;
    let mut at = start;
    for (delta, ch) in text[start.min(text.len())..end].char_indices() {
        if units == character {
            return start + delta;
        }
        units += ch.len_utf16() as u32;
        at = start + delta + ch.len_utf8();
    }
    // 列が行末と同じか、それを超えた（または空行）→ 行末に丸める。
    at.min(end)
}

/// byte 位置を行・列（0 始まり）にする。
pub fn position(text: &str, byte: usize) -> (u32, u32) {
    let starts = project_features::line_starts_of(text);
    let line = project_features::line_of(text, byte.min(text.len()));
    let line_start = starts.get(line).copied().unwrap_or(0);
    let mut units = 0u32;
    for (_, ch) in text[line_start..byte.min(text.len()).max(line_start)].char_indices() {
        units += ch.len_utf16() as u32;
    }
    (line as u32, units)
}

/// byte 位置を LSP の `Position` にする。
pub fn position_json(text: &str, byte: usize) -> Value {
    let (line, character) = position(text, byte);
    json!({ "line": line, "character": character })
}

/// byte の範囲を LSP の `Range` にする。
pub fn range_json(text: &str, range: &Range<usize>) -> Value {
    json!({
        "start": position_json(text, range.start),
        "end": position_json(text, range.end),
    })
}

/// LSP の `Position`（`{line, character}`）を byte 位置にする。
pub fn offset_of(value: &Value, text: &str) -> usize {
    let line = value.get("line").and_then(Value::as_u64).unwrap_or(0) as u32;
    let character = value.get("character").and_then(Value::as_u64).unwrap_or(0) as u32;
    offset(text, line, character)
}

/// LSP の `Range` を byte の範囲にする。
pub fn range_of(value: &Value, text: &str) -> Range<usize> {
    let start = value
        .get("start")
        .map(|at| offset_of(at, text))
        .unwrap_or(0);
    let end = value
        .get("end")
        .map(|at| offset_of(at, text))
        .unwrap_or(start);
    start.min(end)..start.max(end)
}

/// 文字列の UTF-16 長（列の計算と同じ数え方）。
pub fn utf16_len(text: &str) -> u32 {
    text.chars().map(|ch| ch.len_utf16() as u32).sum()
}

/// 内部の byte 範囲（`{start, end}`）を LSP の `Range` にする。
///
/// 形が無ければ文頭からの空範囲（LSP では「範囲なし」を空範囲で表すのが安全）。
pub fn range_of_value(text: &str, value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 },
        });
    };
    let start = value.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
    let end = value
        .get("end")
        .and_then(Value::as_u64)
        .map(|end| end as usize)
        .unwrap_or(start);
    range_json(text, &(start..end))
}
