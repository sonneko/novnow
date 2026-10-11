//! アイデア（§5.7 `ideas.md`）。
//!
//! 「捨てやすく、拾いやすい」ことが最優先（§10.4）。採番（最大値 +1、再利用禁止）は
//! 編集操作（M8 の `idea` ワークスペース）の仕事で、ここは 1 件のかたまりを持つだけ。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// ID のシジル（§5.7「H1 は `I` + 4 桁 ID + 空白 + タイトル」）。
pub const ID_SIGIL: char = 'I';

/// `status:` の値（§5.7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum IdeaStatus {
    /// 受信箱（既定）。
    #[default]
    #[serde(rename = "inbox")]
    Inbox,
    /// 取っておく。
    #[serde(rename = "keep")]
    Keep,
    /// 昇格済み（`promoted-to` を持つ）。
    #[serde(rename = "promoted")]
    Promoted,
    /// 捨てた。**既定で非表示**（フィルタで表示可、§5.7）。
    #[serde(rename = "dropped")]
    Dropped,
}

impl IdeaStatus {
    /// 文字列から引く。表に無ければ `None`（**診断コードは §7 に無い**ので診断しない）。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "inbox" => Some(Self::Inbox),
            "keep" => Some(Self::Keep),
            "promoted" => Some(Self::Promoted),
            "dropped" => Some(Self::Dropped),
            _ => None,
        }
    }

    /// §5.7「`status: dropped` のものは既定で非表示」。
    pub fn is_hidden_by_default(self) -> bool {
        matches!(self, Self::Dropped)
    }
}

/// 1 アイデア（§5.7）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Idea {
    /// **書かれたままの** ID（`I0007`）。
    ///
    /// 文字列で持つのは形が壊れていてもファイル全体を道連れにしないため（§4.3-4）。
    pub id: String,
    /// タイトル（ID の後ろ）。
    pub title: String,
    /// `status:`（§5.0.6 の表に無いキーなので `ideas.md` では **W011**。A-15）。
    pub status: String,
    /// `created:`（RFC 3339）。
    pub created: String,
    /// `tags:`（カンマ区切り）。
    pub tags: Vec<String>,
    /// `relates:` の**書かれたままの値**（`@02, 'アルス`）。
    ///
    /// 参照の解決は辞書が要るので M3（§6.4）。
    pub relates: String,
    /// `promoted-to:`（§5.7「昇格時のみ」）。
    #[serde(rename = "promoted-to")]
    pub promoted_to: String,
    /// 本文（§5.7「本文は自由記述。改行もそのまま保持する」）。
    pub body: String,
    /// §5.7 に無いキー。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Idea {
    /// ID の数字部分（§5.7「最大値 +1 を採番」に使う）。形が合わなければ `None`。
    pub fn number(&self) -> Option<u32> {
        let digits = self.id.strip_prefix(ID_SIGIL)?;
        if digits.len() < 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    }

    /// `status:` を §5.7 の 4 値として読む。表に無ければ `None`。
    pub fn status_kind(&self) -> Option<IdeaStatus> {
        IdeaStatus::parse(&self.status)
    }

    /// 既定で隠すか（§5.7）。`status` が読めなければ隠さない。
    pub fn is_hidden(&self) -> bool {
        self.status_kind()
            .is_some_and(IdeaStatus::is_hidden_by_default)
    }
}
