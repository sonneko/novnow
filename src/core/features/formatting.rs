//! 正規化（§3.4 `formatting::document(doc, project)` → `textDocument/formatting`、§5.12）。
//!
//! §5.12 の 7 項目**だけ**を行う（それ以外の整形は禁止、MUST）。本文の文字（句読点・記号・
//! 改行位置）には一切触れない。実行するのは明示操作（パレット `format`）のときだけで、
//! 保存時に自動では呼ばない（§5.12）。
//!
//! | §5.12 | 処理 | ここでの実装 |
//! |---|---|---|
//! | 1 | 行末の空白削除 | 半角・全角空白とタブ（[`lexer::is_space`]）。行末の `\r` も落とす |
//! | 2 | 改行コードを `project.json.newline` に統一 | **保存時**（[`lexer::normalize_for_save`]）。文書はメモリ上では常に LF（§5.0.4）なので、ここでは行末の `\r` を落とすだけ（`docs/history.md` B-83） |
//! | 3 | ファイル末尾に改行 1 個 | 末尾の空行を落とし、無ければ足す。空の文書は空のまま |
//! | 4 | 3 行以上連続する空行を 2 行に圧縮 | 1 の後で空になった行を空行と数える |
//! | 5 | `;` 行の前後の空白除去 | A / B / C 形式の節区切り行（[`LineKind::SectionBreak`]）の行頭の空白 |
//! | 6 | フィールド行の `key` と `:` の間の空白を除去し、`: ` の後は空白 1 個に統一 | 半角 `:` は `key: 値`（値が空なら `key:`）。**全角 `：` の後ろは触らない**（§5.0.5 は全角コロンの後に空白を求めない。和文の書き方を変えない、B-83） |
//! | 7 | 全角空白のみの行は空行に変換 | 1 に含まれる |
//!
//! 戻り値は**変わった行ごと**の編集（位置の昇順・重なりなし）。何も変わらなければ空。

use crate::core::features::project::{self, Project};
use crate::core::features::TextEdit;
use crate::core::model::document::Document;
use crate::core::text::lexer::{self, LineKind, TextFormat};

/// 何個まで空行を続けてよいか（§5.12-4）。
const MAX_BLANK_RUN: usize = 2;

/// 文書の正規化の編集（§3.4 / §5.12）。
pub fn document(document: &Document, project: &Project) -> Vec<TextEdit> {
    let _ = project;
    let text = document.text.as_str();
    if text.is_empty() {
        return Vec::new();
    }
    let format = document.format.text_format();
    let ends_with_newline = text.ends_with('\n');
    let mut lines: Vec<project::LineRef<'_>> = project::lines_of(text).collect();
    if ends_with_newline {
        // 最後の改行の後ろの「空の行」は行として数えない
        lines.pop();
    }
    let formatted: Vec<String> = lines
        .iter()
        .map(|line| format_line(line.text, format))
        .collect();

    // §5.12-4 と §5.12-3（空行の圧縮と、末尾の空行の削除）
    let mut keep = vec![true; lines.len()];
    let mut run = 0usize;
    for (index, line) in formatted.iter().enumerate() {
        if line.is_empty() {
            run += 1;
            if run > MAX_BLANK_RUN {
                keep[index] = false;
            }
        } else {
            run = 0;
        }
    }
    for index in (0..formatted.len()).rev() {
        if !formatted[index].is_empty() {
            break;
        }
        keep[index] = false;
    }

    let mut edits = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let end = line.start + line.text.len();
        if !keep[index] {
            // 行と、その行の改行を消す（最後の行で改行が無ければ直前の改行を消す）
            let range = if end < text.len() {
                line.start..end + 1
            } else {
                line.start.saturating_sub(1)..end
            };
            edits.push(TextEdit {
                range,
                new_text: String::new(),
            });
        } else if formatted[index] != line.text {
            edits.push(TextEdit {
                range: line.start..end,
                new_text: formatted[index].clone(),
            });
        }
    }
    // §5.12-3: 末尾に改行が無ければ足す（中身が残る場合だけ）
    if !ends_with_newline && keep.iter().any(|kept| *kept) {
        edits.push(TextEdit {
            range: text.len()..text.len(),
            new_text: "\n".to_string(),
        });
    }
    merge(edits)
}

/// 1 行ぶんの §5.12-1・5・6・7。
fn format_line(raw: &str, format: Option<TextFormat>) -> String {
    let trimmed = raw
        .trim_end_matches('\r')
        .trim_end_matches(lexer::is_space)
        .trim_end_matches('\r');
    let Some(format) = format else {
        return trimmed.to_string();
    };
    match lexer::classify(trimmed, format) {
        // §5.12-5（`;` は A / B / C 形式だけが節区切りとして読む。`*.md` では通常テキスト）
        LineKind::SectionBreak => lexer::after_indent(trimmed).to_string(),
        LineKind::Field(field) => format_field(trimmed, &field),
        _ => trimmed.to_string(),
    }
}

/// §5.12-6: `key  :  値` → `key: 値`。全角 `：` は key との間だけ詰める。
fn format_field(raw: &str, field: &lexer::FieldLine<'_>) -> String {
    let indent_len = raw.len() - lexer::after_indent(raw).len();
    let indent = &raw[..indent_len];
    let after_key = &raw[indent_len + field.key_text.len()..];
    let colon_part = after_key.trim_start_matches(lexer::is_space);
    let Some(colon) = colon_part.chars().next() else {
        return raw.to_string();
    };
    let rest = &colon_part[colon.len_utf8()..];
    if colon == ':' {
        if field.value.is_empty() {
            format!("{indent}{}:", field.key_text)
        } else {
            format!("{indent}{}: {}", field.key_text, field.value)
        }
    } else {
        format!("{indent}{}{colon}{rest}", field.key_text)
    }
}

/// 接した編集を 1 つにまとめる（UI の取り消し単位を細かくしすぎない）。
fn merge(edits: Vec<TextEdit>) -> Vec<TextEdit> {
    let mut out: Vec<TextEdit> = Vec::with_capacity(edits.len());
    for edit in edits {
        match out.last_mut() {
            Some(last)
                if last.range.end == edit.range.start
                    && last.new_text.is_empty()
                    && edit.new_text.is_empty() =>
            {
                last.range.end = edit.range.end;
            }
            _ => out.push(edit),
        }
    }
    out
}
