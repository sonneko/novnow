//! `words.md`（§5.6）のパーサ。
//!
//! ```md
//! # 星屑の時計
//! reading: ほしくずのとけい
//! category: 魔導具
//! alias: 星屑時計
//! definition: 旧校舎の地下遺跡に眠るとされる幻の魔導具。古代錬金術の存在証明とされる。
//! range: @01 ..
//! note: 第 3 章で贋作であることが判明する。
//! ```
//!
//! §5.6「`characters.md` と同一の MD 方言を使う（実装・ハイライト・補完を共通化するため）」
//! なので、行の分類は [`super::characters_md`] のものをそのまま使う。違うのは木の方で、
//! H1 = 用語、直下はフィールド行と自由記述だけ（**セクションは無い**）。
//!
//! # 診断（M2 で出すもの）
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **W011** | §5.0.6 の表に無いフィールドキー | キー |
//! | **E002** | `range:` の値が範囲式としてパースできない（§5.1） | 値 |
//! | **E016** | `range:` の左端 > 右端（§5.1） | 値 |
//! | **E021** | `definition` が無い（§5.6 MUST） | H1 の行 |
//!
//! 並びは**行順の W011** のあとに**用語順の E002 / E016 / E021**。
//!
//! # M2 で出さない診断
//!
//! - **W022**（`range` 外の節で参照された、§5.6）: 本文と突き合わせる検査なので M4。
//! - **E022**（用語に修飾子）・**W026**（辞書キーの種別衝突）・**W027**（より長い項目にも
//!   一致しうる）: 辞書を組む M3（§6.4・`resolve/dictionary.rs`）。
//! - 用語名の重複: §7 に対応コードが無いので診断しない（辞書側の M3）。

use std::ops::Range;

use serde::Serialize;

use crate::core::diagnostics::{Diagnostic, DiagnosticCode};
use crate::core::id::RangeExpr;
use crate::core::text::lexer::FieldKey;

use super::characters_md::{self, RangeValue};
use super::plot_outline::{self, Field};

// MD 方言の行は `characters_md` のものが正（§5.6「同一の MD 方言」）。
// このモジュールだけで木を辿れるよう、同じ名前で公開し直す（B 形式が A 形式の
// `Field` 等を `pub use` したのと同じ流儀。`docs/history.md` B-40）。
pub use super::characters_md::{Bullet, Heading, Line, Pattern, PatternLine};

/// `words.md` 1 ファイル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Words<'a> {
    /// 全行（書かれたまま）。[`Words::to_text`] はこれを LF で連結する。
    pub lines: Vec<Line<'a>>,
    /// 最初の H1 より前の行。
    pub preamble: Preamble,
    /// 用語（H1 の順）。
    pub words: Vec<Word<'a>>,
    /// 診断（§7）。位置は `text` の中の byte range。
    pub diagnostics: Vec<Diagnostic>,
}

/// `words.md` のテキストをパースする。
///
/// `text` は **LF 改行**であること（§5.0.4）。壊れた入力でも部分木と診断を返す
/// （`docs/history.md` の完了条件 3）。
pub fn parse(text: &str) -> Words<'_> {
    let mut lines: Vec<Line<'_>> = text.split('\n').map(characters_md::parse_line).collect();
    characters_md::attach_continuations(&mut lines);
    let mut diagnostics = Vec::new();
    let (preamble, words) = collect(&lines, &mut diagnostics);
    let offsets = characters_md::line_offsets(&lines);
    warn_words(&lines, &words, &offsets, &mut diagnostics);
    Words {
        lines,
        preamble,
        words,
        diagnostics,
    }
}

impl<'a> Words<'a> {
    /// 入力と**バイト単位で一致する**テキストに戻す（`docs/history.md` の完了条件 2）。
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(line.raw());
        }
        out
    }

    /// `lines[index]` を含む用語。前文の行は `None`。
    pub fn word_of(&self, index: usize) -> Option<&Word<'a>> {
        self.words.iter().find(|found| found.span.contains(&index))
    }

    /// `lines[index]` が見出し行ならその中身。
    pub fn heading(&self, index: usize) -> Option<&Heading<'a>> {
        match self.lines.get(index) {
            Some(Line::Heading(found)) => Some(found),
            _ => None,
        }
    }

    /// `lines[index]` がフィールド行ならその中身。
    pub fn field(&self, index: usize) -> Option<&Field<'a>> {
        match self.lines.get(index) {
            Some(Line::Field(found)) => Some(found),
            _ => None,
        }
    }

    /// `index` 番目の用語の `key` のフィールド（出現順）。
    pub fn word_fields(&self, index: usize, key: FieldKey) -> Vec<&Field<'a>> {
        match self.words.get(index) {
            Some(found) => pick(&self.lines, &found.fields, key),
            None => Vec::new(),
        }
    }

    /// `index` 番目の用語の `definition`（§5.6 の必須フィールド）。無ければ `None`（**E021**）。
    ///
    /// 継続行まで含めた値が要るときは [`Field::full_value`]。
    pub fn definition(&self, index: usize) -> Option<&Field<'a>> {
        self.word_fields(index, FieldKey::Definition)
            .first()
            .copied()
    }

    /// 表に無いキー（[`FieldKey::Unknown`]）を**書かれたままのキー**で引く。
    ///
    /// §5.0.6 の表に無いキーも値を改変せずに保持する（縫い代 2）ので、読む手段が要る。
    pub fn unknown_field(&self, index: usize, key_text: &str) -> Option<&Field<'a>> {
        let found = self.words.get(index)?;
        characters_md::unknown_field(&self.lines, &found.fields, key_text)
    }

    /// `index` 番目の用語の `range`（§5.6）。書かれていなければ `None`（＝全範囲）。
    pub fn range(&self, index: usize) -> Option<RangeExpr> {
        match &self.words.get(index)?.range {
            Some(found) => found.expr(),
            // §5.5.4 と同じく、書かなければ範囲の制限なし
            None => Some(characters_md::all_range()),
        }
    }
}

/// 最初の H1 より前の行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preamble {
    /// 行範囲（`Words::lines` の中）。H1 が無ければ全行。
    pub span: Range<usize>,
    /// フィールド行の index（出現順）。
    pub fields: Vec<usize>,
}

/// 1 用語（§5.6）。`# 用語名` から次の `# ` まで。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Word<'a> {
    /// H1 の行 index。
    pub heading: usize,
    /// 用語名（見出しのテキスト全体）。
    pub term: &'a str,
    /// `term` の行内 byte 位置。
    pub term_at: usize,
    /// 行範囲。H1 を含み、次の H1 を含まない。
    pub span: Range<usize>,
    /// フィールド行の index（出現順）。§5.6 の `reading` `category` `alias` `definition`
    /// `range` `note`。
    pub fields: Vec<usize>,
    /// 自由記述の index（出現順）。
    pub free: Vec<usize>,
    /// `range:` の値。**書かれていなければ全範囲**。
    pub range: Option<RangeValue<'a>>,
}

// ============================================================ 木と診断

/// 解析中の用語。
struct OpenWord<'a> {
    /// H1 の行 index。
    heading: usize,
    /// 用語名。
    term: &'a str,
    /// `term` の行内 byte 位置。
    term_at: usize,
    /// フィールド行の index。
    fields: Vec<usize>,
    /// 自由記述の index。
    free: Vec<usize>,
}

/// 行の並びから前文と用語を作り、行順の診断（W011）を出す。
fn collect<'a>(lines: &[Line<'a>], out: &mut Vec<Diagnostic>) -> (Preamble, Vec<Word<'a>>) {
    let head = lines
        .iter()
        .position(|line| matches!(line, Line::Heading(found) if found.level == 1));
    let mut preamble = Preamble {
        span: 0..head.unwrap_or(lines.len()),
        fields: Vec::new(),
    };
    let mut words: Vec<Word<'a>> = Vec::new();
    let mut open: Option<OpenWord<'a>> = None;
    let mut at = 0;
    for (index, line) in lines.iter().enumerate() {
        match line {
            // §5.6 に H2 / H3 は無いので、`## ` も `### ` も用語を閉じるだけ
            // （行は [`Line::Heading`] のまま残るので round-trip は保たれる）
            Line::Heading(found) if found.level == 1 => {
                if let Some(closed) = open.take() {
                    words.push(finish_word(closed, index, lines));
                }
                open = Some(OpenWord {
                    heading: index,
                    term: found.text,
                    term_at: found.text_at,
                    fields: Vec::new(),
                    free: Vec::new(),
                });
            }
            Line::Heading(_) => {}
            Line::Field(found) => {
                // §5.0.6「未知のキーは W011 とし、値は改変せず保持する」（行順）
                plot_outline::warn_unknown_key(found, at, out);
                match open.as_mut() {
                    Some(found) => found.fields.push(index),
                    None => preamble.fields.push(index),
                }
            }
            Line::Text(_) => {
                if let Some(found) = open.as_mut() {
                    found.free.push(index);
                }
            }
            // 箇条書きとパターン行は §5.6 に無いので木に載せない（行は保持される）。
            // 継続行は持ち主のフィールドに既に入っている
            Line::Bullet(_)
            | Line::Pattern(_)
            | Line::Continuation(_)
            | Line::Comment(_)
            | Line::Blank(_) => {}
        }
        at += line.raw().len() + 1;
    }
    if let Some(closed) = open.take() {
        words.push(finish_word(closed, lines.len(), lines));
    }
    (preamble, words)
}

/// 用語を確定する。
fn finish_word<'a>(open: OpenWord<'a>, stop: usize, lines: &[Line<'a>]) -> Word<'a> {
    let OpenWord {
        heading,
        term,
        term_at,
        fields,
        free,
    } = open;
    let range = fields
        .iter()
        .find_map(|&index| characters_md::range_value(lines, index));
    Word {
        heading,
        term,
        term_at,
        span: heading..stop,
        fields,
        free,
        range,
    }
}

/// 用語順の診断（**E002** / **E016** / **E021**）を出す。
fn warn_words(
    lines: &[Line<'_>],
    words: &[Word<'_>],
    offsets: &[usize],
    out: &mut Vec<Diagnostic>,
) {
    for found in words {
        if let Some(range) = &found.range {
            characters_md::check_range(range, offsets[range.line] + range.at, out);
        }
        // §5.6「H1 = 用語。`definition` 必須（欠落は E021）」。位置は H1 の行全体
        // （A 形式の E018 がブロックの見出し行を指すのと同じ）
        let defined = found.fields.iter().any(|&index| {
            matches!(lines.get(index), Some(Line::Field(field)) if field.key == FieldKey::Definition)
        });
        if !defined {
            let start = offsets[found.heading];
            // offsets は `lines.len() + 1` 要素あるので +1 は必ずある。末尾の改行分を引く
            let end = offsets[found.heading + 1] - 1;
            out.push(Diagnostic::new(
                DiagnosticCode::E021,
                Some(start..end),
                Vec::new(),
            ));
        }
    }
}

/// `indices` の中から `key` のフィールド行を集める（A / B 形式の `pick` と同じ）。
fn pick<'s, 'a>(lines: &'s [Line<'a>], indices: &[usize], key: FieldKey) -> Vec<&'s Field<'a>> {
    indices
        .iter()
        .filter_map(|&index| match lines.get(index) {
            Some(Line::Field(found)) if found.key == key => Some(found),
            _ => None,
        })
        .collect()
}
