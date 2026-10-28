//! 資料（§5.8.2 `materials/index.json`）。
//!
//! `index.json` は**機械が採番・管理する唯一の JSON**（§5.5.1「人間が直接書く情報は MD、
//! 機械が採番・管理する情報は JSON」）。なのでこの型がそのままストレージ形式で、
//! `Serialize` + `Deserialize` の両方が要る（`docs/history.md` B-30）。

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 資料を置くディレクトリ（§5.8.1）。`path` はここから外に出てはならない（**E024**）。
pub const MATERIALS_DIR: &str = "materials/";

/// 通常資料の接頭辞（§5.8.2・追加仕様 11）。
pub const PREFIX_MATERIAL: char = 'M';

/// 検索結果の保存の接頭辞（§16.3.1）。
///
/// `M`（通常資料）と `S`（検索結果の保存）は同一の連番空間を共有する（§5.8.2 MUST）。
/// 接頭辞は 1 文字として読み、`M` 以外も**読める**（`docs/plans.md` §4.4 縫い代 7）。
pub const PREFIX_SEARCH: char = 'S';

/// ID の最少桁数（§5.8.2「4 桁以上のゼロ詰め連番」）。
pub const ID_DIGITS: usize = 4;

/// 資料 ID（§5.8.2）。`M0137` / `S0138`。
///
/// **接頭辞は 1 文字として読む**（`docs/plans.md` §4.4 縫い代 7）。`S`（検索結果の保存、§16.3.1）も
/// `M` と同じ連番空間なので拒否してはならない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialId {
    /// 接頭辞（`M` / `S`。それ以外も**読む**）。
    pub prefix: char,
    /// 連番。
    pub num: u32,
}

impl MaterialId {
    /// `M0137` の形を読む。接頭辞が ascii 英字で、残りが 4 桁以上の数字のときだけ `Some`。
    pub fn parse(text: &str) -> Option<Self> {
        let mut chars = text.chars();
        let prefix = chars.next()?;
        if !prefix.is_ascii_alphabetic() {
            return None;
        }
        let digits: String = chars.collect();
        if digits.len() < ID_DIGITS || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some(Self {
            prefix,
            num: digits.parse().ok()?,
        })
    }

    /// 通常資料の接頭辞（`M`）か。検索結果の保存（`S`、§16.3.1）と区別する。
    pub fn is_material_prefix(self) -> bool {
        self.prefix == PREFIX_MATERIAL
    }
}

impl fmt::Display for MaterialId {
    /// `M%04d`（§5.8.2「ID 採番」）。9999 を超えれば桁は自然に増える。
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}{:04}", self.prefix, self.num)
    }
}

/// `materials/index.json`（§5.8.2 スキーマ v1）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialIndex {
    /// スキーマの版（§5.8.2 の例は `1`）。
    pub schema_version: u32,
    /// 次に発行する連番（§5.8.2「`next_id` を読み、`M%04d` を発行し、インクリメントして
    /// 同一トランザクションで保存する」MUST）。
    pub next_id: u32,
    /// 資料（書いた順）。
    pub items: Vec<Material>,
    /// §5.8.2 に無いキー。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl MaterialIndex {
    /// `items` の中の最大の連番（接頭辞は問わない。`M` と `S` は同一の連番空間、§5.8.2）。
    pub fn max_number(&self) -> Option<u32> {
        self.items
            .iter()
            .filter_map(|item| item.material_id())
            .map(|id| id.num)
            .max()
    }

    /// `next_id` が単調増加の規則（§5.8.2 MUST）を満たすか。
    ///
    /// 既存の ID より小さい `next_id` は**再利用**（衝突）を生むので `false`。
    /// 対応する診断コードは §7 に無いので、これは検査器（M5 の保存）が使う情報。
    pub fn is_next_id_valid(&self) -> bool {
        match self.max_number() {
            Some(max) => self.next_id > max,
            None => true,
        }
    }

    /// 重複している ID（§5.8.2「単調増加・再利用禁止」）。**E008** の条件。
    pub fn duplicate_ids(&self) -> Vec<&str> {
        let mut seen: Vec<&str> = Vec::new();
        let mut duplicates = Vec::new();
        for item in &self.items {
            if seen.contains(&item.id.as_str()) {
                duplicates.push(item.id.as_str());
            } else {
                seen.push(item.id.as_str());
            }
        }
        duplicates
    }

    /// 次の ID を発行して `next_id` を進める（§5.8.2「ID 採番」MUST）。
    ///
    /// `prefix` は [`PREFIX_MATERIAL`]（通常資料）か [`PREFIX_SEARCH`]（検索結果の保存、§16.3.1）。
    /// 保存は**同一トランザクション**で行うこと（呼び出し側＝M5 の `adapters`）。
    pub fn issue(&mut self, prefix: char) -> MaterialId {
        let num = self.next_id.max(self.max_number().map_or(0, |max| max + 1));
        self.next_id = num + 1;
        MaterialId { prefix, num }
    }
}

/// 資料 1 件（§5.8.2 の `items` の要素）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Material {
    /// **書かれたままの** ID（`M0137`）。
    ///
    /// 文字列で持つのは形が壊れていても index 全体を道連れにしないため（§4.3-4）。
    /// [`Material::material_id`] で [`MaterialId`] として読む。
    pub id: String,
    /// タイトル（§5.8.2 必須。1〜80 字。参照の照合対象、§6.6）。
    pub title: String,
    /// プロジェクトルートからの相対パス（§5.8.2 必須。`materials/` 配下に限る。外は **E024**）。
    pub path: String,
    /// 出典（§5.8.2 必須）。
    pub source: Source,
    /// 分類（§5.8.2 必須。自由文字列）。
    pub category: String,
    /// 「必要になる条件」（§5.8.2 必須、**空配列可**）。
    pub conditions: Vec<Condition>,
    /// 保存日時（§5.8.2 必須。RFC 3339、タイムゾーン付き）。
    pub saved_at: String,
    /// 更新日時（任意）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// タグ（§5.8.2 の例にある任意の配列）。
    pub tags: Vec<String>,
    /// `approved` | `proposed` | `rejected`（§5.8.2 必須）。
    ///
    /// 文字列で持つのは未知の値でファイル全体を道連れにしないため（§4.3-4）。
    /// [`Material::status_kind`] で [`MaterialStatus`] として読む。
    pub status: String,
    /// `human` または `agent:<role>`（§5.8.2 必須）。
    pub added_by: String,
    /// 短文（§5.8.2 任意。≤ 120 字。参照ペインとコンテキストに入る）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// MD 本文の SHA-256（§5.8.2 任意。外部編集の検出に使う）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    /// §5.8.2 に無いキー。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Material {
    /// ID を [`MaterialId`] として読む。形が合わなければ `None`。
    pub fn material_id(&self) -> Option<MaterialId> {
        MaterialId::parse(&self.id)
    }

    /// `status` を §5.8.2 の 3 値として読む。表に無ければ `None`。
    pub fn status_kind(&self) -> Option<MaterialStatus> {
        MaterialStatus::parse(&self.status)
    }

    /// `path` が `materials/` 配下か（§5.8.2。外は **E024**）。
    ///
    /// 絶対パスと `../` も外とみなす（§13.1 規則 10「プロジェクト外に出てよいのは履歴パスのみ」）。
    pub fn is_path_inside_materials(&self) -> bool {
        !self.path.starts_with('/')
            && !self.path.contains("../")
            && self.path.starts_with(MATERIALS_DIR)
    }

    /// `added_by` が人間か（§5.8.2 `human` または `agent:<role>`）。
    pub fn is_added_by_human(&self) -> bool {
        self.added_by == ADDED_BY_HUMAN
    }

    /// `added_by` が `agent:<role>` ならその役割名。
    pub fn agent_role(&self) -> Option<&str> {
        self.added_by.strip_prefix(AGENT_PREFIX)
    }
}

/// `added_by` の人間（§5.8.2）。
pub const ADDED_BY_HUMAN: &str = "human";

/// `added_by` のエージェント前置き（§5.8.2 `agent:<role>`）。
pub const AGENT_PREFIX: &str = "agent:";

/// `status` の値（§5.8.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialStatus {
    /// 採用済み（`materials/docs/`）。
    #[serde(rename = "approved")]
    Approved,
    /// 提案中（`materials/inbox/`）。
    #[serde(rename = "proposed")]
    Proposed,
    /// 不採用。**ID は回収しない**（§5.8.2）。
    #[serde(rename = "rejected")]
    Rejected,
}

impl MaterialStatus {
    /// 文字列から引く。表に無ければ `None`（対応する診断コードは §7 に無い）。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "approved" => Some(Self::Approved),
            "proposed" => Some(Self::Proposed),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// 出典（§5.8.2 `source`）。
///
/// `{"kind":"url",...}` / `{"kind":"custom",...}` / `{"kind":"search",...}`（§16.3.1）の 3 種。
/// **内部タグ付き enum にしない**のは、未知の `kind` や未知のキーで index 全体を道連れに
/// しないため（§4.3-4）。`kind` は文字列で持ち、[`Source::kind_value`] で引く。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Source {
    /// `url` | `custom` | `search`（§5.8.2 / §16.3.1）。
    pub kind: String,
    /// `kind: url` の URL。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// `kind: custom` の自由文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 参照日時（RFC 3339）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessed_at: Option<String>,
    /// `kind: search` の検索語（§16.3.1）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// `kind: search` の検索日時（§16.3.1）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searched_at: Option<String>,
    /// 出典のその他のキー。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

/// `source.kind` の値（§5.8.2 / §16.3.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    /// URL。
    #[serde(rename = "url")]
    Url,
    /// 人手の自由文。
    #[serde(rename = "custom")]
    Custom,
    /// プロジェクト内検索の結果（§16.3.1）。
    #[serde(rename = "search")]
    Search,
}

impl Source {
    /// 出典が無いか（**W065** の条件、§16.7）。`kind: search`（§16.3.1）は検索語そのものが出典なので
    /// 無いことにはしない。それ以外は `url` も `text` も空（空白だけを含む）なら無い。
    pub fn is_missing(&self) -> bool {
        if self.kind_value() == Some(SourceKind::Search) {
            return false;
        }
        let url = self.url.as_deref().unwrap_or("").trim();
        let text = self.text.as_deref().unwrap_or("").trim();
        url.is_empty() && text.is_empty()
    }

    /// 表示する出典（URL があれば URL、無ければ自由文。どちらも無ければ空）。
    pub fn shown(&self) -> &str {
        self.url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .or(self.text.as_deref())
            .unwrap_or("")
    }

    /// `kind` を §5.8.2 の 3 値として読む。表に無ければ `None`。
    pub fn kind_value(&self) -> Option<SourceKind> {
        match self.kind.as_str() {
            "url" => Some(SourceKind::Url),
            "custom" => Some(SourceKind::Custom),
            "search" => Some(SourceKind::Search),
            _ => None,
        }
    }
}

/// 「必要になる条件」1 件（§5.8.2 `conditions` / §16.4）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Condition {
    /// `scope` | `ref` | `tag` | `text`（§16.4 の 4 種）。
    pub kind: String,
    /// 条件の中身（§5.8.2 の例: `@02-03 .. @02-08` / `'科学の甲子園` / 自由文）。
    pub value: String,
    /// §16.4 に無いキー。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

/// `conditions[].kind` の値（§16.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConditionKind {
    /// 範囲（`@02-03 .. @02-08`）。
    #[serde(rename = "scope")]
    Scope,
    /// 参照（`'科学の甲子園`）。
    #[serde(rename = "ref")]
    Ref,
    /// タグ。
    #[serde(rename = "tag")]
    Tag,
    /// 自由文。
    #[serde(rename = "text")]
    Text,
}

impl Condition {
    /// `kind` を §16.4 の 4 値として読む。表に無ければ `None`。
    pub fn kind_value(&self) -> Option<ConditionKind> {
        match self.kind.as_str() {
            "scope" => Some(ConditionKind::Scope),
            "ref" => Some(ConditionKind::Ref),
            "tag" => Some(ConditionKind::Tag),
            "text" => Some(ConditionKind::Text),
            _ => None,
        }
    }
}
