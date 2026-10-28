//! キャラクタ（§5.5）。
//!
//! `characters.md` から作る**意味のかたまり**。行の位置・自由記述・round-trip は
//! `core::syntax::characters_md` が持つ（あちらが正本で、こちらは辞書（M3）と
//! 参照ペイン（§9.5）・配役ビュー（§10.6）が必要なものだけを持つ）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::mode::Mode;
use super::role_pattern::RolePattern;

/// 1 キャラクタ（§5.5.2 `character`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Character {
    /// `# 見出し` の名前（§5.5.2 `char_name`）。
    pub name: String,
    /// `reading:`（§5.5.3。補完の並べ替えと検索に使う）。
    pub reading: Option<String>,
    /// `alias:`（§5.5.3。カンマ区切り。**すべて辞書に登録される**）。
    pub alias: Vec<String>,
    /// `role:`（§5.5.3。物語上の役回り。モード別に上書き可）。
    pub role: Option<String>,
    /// `default-mode:`（§5.5.3。明示が無い節での既定モード、§6.4.3）。
    #[serde(rename = "default-mode")]
    pub default_mode: Option<String>,
    /// `color:`（§5.5.3 `#RRGGBB`）。
    ///
    /// §5.0.6 の表にある（`color` / `色`）ので W011 は出ない。形の検査に対応するコードは
    /// §7 に無いので行わない。
    pub color: Option<String>,
    /// `tags:`（§5.5.3。カンマ区切り）。
    pub tags: Vec<String>,
    /// `icon:`（§5.5.3）。`assets/` 配下の相対パス。
    ///
    /// ここでは値を保持するだけ。E062 / W068–W070 はプロジェクト全体を見る M4 の検査器が出す。
    pub icon: Option<String>,
    /// モード（§5.5.4）。同一 `id` が複数入る（`range` が重ならない限り）。
    pub modes: Vec<Mode>,
    /// キャラ共通の役割語パターン（§5.5.5。`## tone` 直下）。
    pub patterns: Vec<RolePattern>,
    /// `## relations` の行（§5.5.6）。
    pub relations: Vec<Relation>,
    /// §5.5.3 に無いキー（`address:`（§5.5.6）を含む）。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

/// `## relations` の 1 件（§5.5.6 `'参照: 説明`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Relation {
    /// 参照先のキャラ名（`'` を除く）。解決できなければ **E013**（辞書が要るので M3）。
    pub target: String,
    /// 説明。
    pub note: String,
}
