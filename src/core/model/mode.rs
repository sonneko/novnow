//! モード（§5.5.4）。
//!
//! `characters.md` の `### <mode_id> <表示名>` から作る**意味のかたまり**。行の位置や
//! round-trip は `core::syntax::characters_md` が持ち、ここは解決（M3 `resolve/modes.rs`）と
//! 参照ペイン（§9.5）が必要なものだけを持つ。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::role_pattern::RolePattern;

/// 1 モード（§5.5.4）。
///
/// **同一 `mode_id` を複数定義できる**（`range` が重ならない限り）。なので 1 キャラクタの
/// モードは `Vec<Mode>` で、同じ `id` が複数入る（§5.5.4「章が進むにつれモードの性格が
/// 少しずつ変化する」）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mode {
    /// `mode_id`（§5.5.2 `/[A-Za-z][A-Za-z0-9_]{0,15}/`）。
    pub id: String,
    /// 表示名（§5.5.2 `mode_label`）。無ければ空。
    pub label: String,
    /// **書かれたままの**範囲式（§5.1）。`None` は全範囲（§5.5.4「`range` 省略時は全範囲」）。
    ///
    /// 文字列で持つのは、形が壊れていても 1 ファイル全体を道連れにしないため（§4.3-4）。
    /// 範囲式としての検査は `core::id::RangeExpr`（E016 / E017 は `syntax` 側が出す）。
    pub range: Option<String>,
    /// `personality:`（§5.5.4）。
    pub personality: Option<String>,
    /// `emotion:`（§5.5.4）。
    pub emotion: Option<String>,
    /// `role:`（§5.5.4。キャラ直下の `role` を上書きする）。
    pub role: Option<String>,
    /// `note:`（§5.5.4）。
    pub note: Option<String>,
    /// このモード専用の役割語パターン（§5.5.5）。
    ///
    /// **1 件でもあればキャラ共通を上書きする**（和集合ではない、§5.5.6 の `address:` と同じ
    /// 流儀。§5.5.5「モード専用 ∪ キャラ共通」はモード専用がある場合の合集合のことで、
    /// 実装は M3 の `resolve/role_language.rs`）。
    pub patterns: Vec<RolePattern>,
    /// §5.5.4 に無いキー（② の追加仕様を含む）。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Mode {
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
