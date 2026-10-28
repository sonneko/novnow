//! 用語（§5.6 `words.md`）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 1 用語（§5.6）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Word {
    /// `# 見出し` の用語名。
    pub term: String,
    /// `reading:`。
    pub reading: Option<String>,
    /// `category:`（§5.6 の例では `魔導具` `舞台`）。
    pub category: Option<String>,
    /// `alias:`（カンマ区切り）。
    pub alias: Vec<String>,
    /// `definition:`（§5.6 **必須**。欠落は **E021**）。
    pub definition: String,
    /// **書かれたままの**範囲式（§5.1）。`None` は範囲の制限なし。
    ///
    /// `range` を書くと、その範囲外の節で参照された場合に **W022**（まだ登場していない用語の
    /// 先出し検出）になる。ただし W022 は仕様にあるが実装はまだ発行しない（§7 の冒頭。本文と
    /// 突き合わせる検査が要る）。値は読んで保つ。
    pub range: Option<String>,
    /// `note:`。
    pub note: Option<String>,
    /// §5.6 に無いキー。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Word {
    /// 範囲式（§5.1）。書かれていなければ全範囲、形が壊れていれば `None`。
    pub fn range_expr(&self) -> Option<crate::core::id::RangeExpr> {
        match &self.range {
            Some(text) => crate::core::id::RangeExpr::parse(text).ok(),
            None => Some(crate::core::id::RangeExpr {
                terms: vec![crate::core::id::RangeTerm {
                    start: None,
                    end: None,
                }],
            }),
        }
    }
}
