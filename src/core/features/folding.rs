//! 折り畳み範囲（§3.4 `folding::ranges(doc)` → `textDocument/foldingRange`）。
//!
//! **v1 の唯一の用途は `plot-outline` の構成ツリーの章 → 話展開**（§3.4 の注記 A-3、§10.3.1）。
//! なので A 形式（`plots/all.txt` / `plots/AA.txt`）のブロックだけを返す。本文の場面・節の
//! 折り畳み（§9.3.3）は**追加仕様 9**（②）で足す（C / B 形式は空）。
//!
//! 範囲は byte（`PLANS.md` §9.2 B-1）。LSP の `FoldingRange` は行で表すので、アダプタが
//! [`crate::core::text::position::LineTable`] で行に直す。ブロックの末尾の空行は含めない。

use std::ops::Range;

use serde::Serialize;

use crate::core::features::project;
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::syntax::plot_outline;

/// 折り畳み範囲 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoldingRange {
    /// 見出し行の先頭からブロックの最後の行の終わりまで。
    pub range: Range<usize>,
    /// ブロックの階層（1 = 章、2 = 話。§5.3.2）。
    pub depth: usize,
}

/// 文書の折り畳み範囲（§3.4）。2 行以上のブロックだけ（1 行は畳めない）。
pub fn ranges(document: &Document) -> Vec<FoldingRange> {
    if document.format != DocumentFormat::PlotOutline {
        return Vec::new();
    }
    let parsed = plot_outline::parse(&document.text);
    let starts = project::line_starts_of(&document.text);
    let end_of = |line: usize| match starts.get(line + 1) {
        Some(next) => next - 1,
        None => document.text.len(),
    };
    let mut out = Vec::new();
    for block in &parsed.blocks {
        let mut last = block.span.end.saturating_sub(1);
        while last > block.span.start && parsed.lines.get(last).is_some_and(|line| line.is_blank())
        {
            last -= 1;
        }
        if last <= block.span.start {
            continue;
        }
        let start = starts.get(block.span.start).copied().unwrap_or(0);
        out.push(FoldingRange {
            range: start..end_of(last),
            depth: block.depth,
        });
    }
    out
}
