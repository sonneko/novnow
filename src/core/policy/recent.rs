//! パス履歴の形と上限（§13.2.2 / §13.3.4）。
//!
//! デスクトップの `recent.json`（[`RecentPaths`]、項目は `{path, opened_at}`）と Web の `app` ストアの
//! `recent_projects`（[`RecentProjects`]、項目は `{id, opened_at}`）が**同じ規則**を使う。
//!
//! - **これ以外のキーを足してはならない**（§13.2.2 MUST）。型がそのキーしか持たないので、
//!   手で足されたキーは読むときに落ち、書き戻すときに消える（温存しない）。
//! - 上限 **32 件**。超えたら古い順に捨てる。同じ項目は 1 件にまとめて先頭へ移す（第 2.9 版）。
//! - 壊れていたら**黙って空**として扱う（§13.2.2）。`schema_version` が 1 でないものも読めないので空。
//! - `opened_at` は順序を保つためだけの値で、並びは**配列の順**（先頭が最新）が正。

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// 履歴の上限（§13.2.2）。
pub const RECENT_MAX: usize = 32;

/// 履歴の形式の版（§13.2.2 / §13.3.4）。
pub const RECENT_SCHEMA_VERSION: u32 = 1;

/// 履歴の 1 項目が持つ「同じものか」の鍵。
pub trait RecentItem {
    /// 同じ項目かを決める鍵（デスクトップはパス、Web はプロジェクト ID）。
    fn key(&self) -> &str;
}

/// デスクトップの項目（§13.2.2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathItem {
    /// プロジェクトディレクトリの絶対パス。
    pub path: String,
    /// 開いた日時（RFC 3339）。
    pub opened_at: String,
}

impl RecentItem for PathItem {
    fn key(&self) -> &str {
        &self.path
    }
}

/// Web の項目（§13.3.4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectItem {
    /// プロジェクト ID（`projects` ストアの `id`）。
    pub id: String,
    /// 開いた日時（RFC 3339）。
    pub opened_at: String,
}

impl RecentItem for ProjectItem {
    fn key(&self) -> &str {
        &self.id
    }
}

/// 履歴（先頭が最新）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recent<T> {
    /// 形式の版（1）。
    pub schema_version: u32,
    /// 項目（先頭が最新）。
    pub items: Vec<T>,
}

/// デスクトップの `recent.json`（§13.2）。
pub type RecentPaths = Recent<PathItem>;

/// Web の `recent_projects`（§13.3.4）。
pub type RecentProjects = Recent<ProjectItem>;

impl<T> Default for Recent<T> {
    fn default() -> Self {
        Self {
            schema_version: RECENT_SCHEMA_VERSION,
            items: Vec::new(),
        }
    }
}

impl<T: RecentItem + Serialize + DeserializeOwned> Recent<T> {
    /// JSON から読む。壊れていれば**空**（§13.2.2「黙って空の履歴として扱い、上書きする」）。
    pub fn parse(text: &str) -> Self {
        Self::from_value(serde_json::from_str(text).ok())
    }

    /// JSON の値から読む（Web の `app` ストアは値をそのまま持つ）。読めなければ空。
    pub fn from_value(value: Option<serde_json::Value>) -> Self {
        let parsed = value.and_then(|found| serde_json::from_value::<Self>(found).ok());
        match parsed {
            Some(found) if found.schema_version == RECENT_SCHEMA_VERSION => {
                let mut out = Self::default();
                // 古いものから積み直すと、重複の解消と上限の規則が書くときと同じになる
                for item in found.items.into_iter().rev() {
                    out.push(item);
                }
                out
            }
            _ => Self::default(),
        }
    }

    /// 書き出す JSON（キーは仕様の 2 つだけ）。
    pub fn to_json(&self) -> String {
        // 型が Serialize できない値を持つことは無い（文字列と数だけ）
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }

    /// 開いた項目を先頭に積む。同じ鍵の古い項目は消し、32 件を超えたら古い順に捨てる。
    pub fn push(&mut self, item: T) {
        self.items.retain(|found| found.key() != item.key());
        self.items.insert(0, item);
        self.items.truncate(RECENT_MAX);
    }

    /// 項目を消す（「履歴から削除」、§13.2.2）。消したら `true`。
    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|found| found.key() != key);
        self.items.len() != before
    }

    /// 鍵の一覧（先頭が最新）。
    pub fn keys(&self) -> Vec<&str> {
        self.items.iter().map(RecentItem::key).collect()
    }
}
