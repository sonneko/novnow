//! 節エディタの補助（検索・置換・全体検索・ホバーの中身）。**egui に依存しない**。
//!
//! - `Ctrl+F` / `Ctrl+H` / `F3`（§9.8.1）: 今のファイルの中の**文字どおりの一致**（大小文字を区別する。
//!   正規表現と大小文字無視の切替は ② の §9.7.4）。
//! - `Ctrl+Shift+F`（§9.8.1）: 本文（`contents/`）の全ファイル。M7 は `write` ワークスペースしか無いので、
//!   移動できる本文に限る（プロットや辞書は M8 のワークスペースで開く。`PLANS.md` §9.2 B-119）。
//! - `Ctrl+Shift+M` / `Ctrl+Shift+W`（§9.8.3）: `core::features::hover` の中身を行にする（文言は呼び出し側）。

use std::ops::Range;

use crate::core::features::hover::{Hover, HoverContents};
use crate::core::features::Project;
use crate::core::model::document::DocumentFormat;

/// `text` の中の `query` の一致（重ならない・先頭から）。`query` が空なら空。
pub fn find_all(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    text.match_indices(query)
        .map(|(at, found)| at..at + found.len())
        .collect()
}

/// 一致の中から、`here`（byte）の次（`forward`）/ 前の一致を選ぶ。端に来たら反対の端へ回る。
pub fn step(matches: &[Range<usize>], here: Range<usize>, forward: bool) -> Option<Range<usize>> {
    if matches.is_empty() {
        return None;
    }
    let found = if forward {
        matches
            .iter()
            .find(|found| found.start >= here.end && *found != &here)
            .or_else(|| matches.first())
    } else {
        matches
            .iter()
            .rev()
            .find(|found| found.end <= here.start)
            .or_else(|| matches.last())
    };
    found.cloned()
}

/// 全体検索の結果 1 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 本文のパス。
    pub path: String,
    /// 行番号（1 始まり）。
    pub line: usize,
    /// 行の中身（前後の空白を除く。長ければ 80 字で切る）。
    pub text: String,
    /// 一致の位置（ファイルの中の byte）。
    pub range: Range<usize>,
}

/// 全体検索の結果の上限。
pub const SEARCH_LIMIT: usize = 200;

/// 本文の全ファイルを検索する（パスの順 → 位置の順、最大 [`SEARCH_LIMIT`] 件）。
pub fn search(project: &Project, query: &str) -> Vec<Hit> {
    let mut out = Vec::new();
    if query.is_empty() {
        return out;
    }
    for document in project.documents() {
        if document.format != DocumentFormat::Contents {
            continue;
        }
        let starts = crate::core::features::project::line_starts_of(&document.text);
        for range in find_all(&document.text, query) {
            let line = crate::core::features::project::line_of(&document.text, range.start);
            let start = starts.get(line).copied().unwrap_or(0);
            let end = document.text[start..]
                .find('\n')
                .map_or(document.text.len(), |at| start + at);
            let shown: String = document.text[start..end].trim().chars().take(80).collect();
            out.push(Hit {
                path: document.path.clone(),
                line: line + 1,
                text: shown,
                range,
            });
            if out.len() >= SEARCH_LIMIT {
                return out;
            }
        }
    }
    out
}

/// ホバーの中身の行（`Ctrl+Shift+M` = キャラの全モード、`Ctrl+Shift+W` = 用語の全文）。
/// `modes` が `true` ならキャラのときだけ、`false` なら用語のときだけ返す（合わなければ `None`）。
/// 1 行目は見出し（名前・用語）。モードの行は `A 呆れ  @01 ..`、この節で使えないモードは先頭に `×`。
pub fn info_lines(hover: &Hover, modes: bool) -> Option<Vec<String>> {
    match (&hover.contents, modes) {
        (
            HoverContents::Character {
                name,
                reading,
                modes: list,
                ..
            },
            true,
        ) => {
            let mut out = vec![match reading {
                Some(reading) => format!("{name}（{reading}）"),
                None => name.clone(),
            }];
            for mode in list {
                let mark = if mode.available { " " } else { "×" };
                let label = if mode.label.is_empty() {
                    mode.id.clone()
                } else {
                    format!("{} {}", mode.id, mode.label)
                };
                out.push(
                    format!("{mark} {label}  {}", mode.range)
                        .trim_end()
                        .to_owned(),
                );
            }
            Some(out)
        }
        (
            HoverContents::Word {
                term, definition, ..
            },
            false,
        ) => {
            let mut out = vec![term.clone()];
            if let Some(definition) = definition {
                out.extend(definition.lines().map(str::to_owned));
            }
            Some(out)
        }
        _ => None,
    }
}
