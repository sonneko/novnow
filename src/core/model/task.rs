//! AI タスク（§5.10.1 `agents/tasks/T0042.md`）。
//!
//! タスクの**ファイルは MD**（§5.10.1）なので、この型はストレージ形式そのものではない
//! （パースは M10 の `agents`）。§15.7 のタスク管理と監査ログ（§5.10.5）が扱うかたまり。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::delegation::Delegation;

/// `status:` の値（§5.10.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum TaskStatus {
    /// 下書き。
    #[default]
    #[serde(rename = "draft")]
    Draft,
    /// 実行待ち。
    #[serde(rename = "ready")]
    Ready,
    /// 実行中。
    #[serde(rename = "running")]
    Running,
    /// 人間の確認待ち。
    #[serde(rename = "review")]
    Review,
    /// 採用。
    #[serde(rename = "accepted")]
    Accepted,
    /// 不採用。
    #[serde(rename = "rejected")]
    Rejected,
    /// 取り消し。
    #[serde(rename = "canceled")]
    Canceled,
    /// 自動適用済み（委譲レベル `apply` / `autonomous`、§15.1.1）。
    #[serde(rename = "applied")]
    Applied,
}

impl TaskStatus {
    /// 文字列から引く。表に無ければ `None`（§4.3-4: 壊れた値でファイル全体を捨てない）。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "draft" => Some(Self::Draft),
            "ready" => Some(Self::Ready),
            "running" => Some(Self::Running),
            "review" => Some(Self::Review),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "canceled" => Some(Self::Canceled),
            "applied" => Some(Self::Applied),
            _ => None,
        }
    }

    /// 終端の状態か（これ以上進まない）。
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::Rejected | Self::Canceled | Self::Applied
        )
    }
}

/// `initiator:` の値（§5.10.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum Initiator {
    /// 人間が起動した。
    #[default]
    #[serde(rename = "human")]
    Human,
    /// 委譲レベル `autonomous` の integrator が自動起動した（§15.1.1・A8）。
    #[serde(rename = "agent")]
    Agent,
}

impl Initiator {
    /// 文字列から引く。表に無ければ `None`。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "human" => Some(Self::Human),
            "agent" => Some(Self::Agent),
            _ => None,
        }
    }
}

/// 1 タスク（§5.10.1）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Task {
    /// タスク ID（`T0042`。H1 のテキスト）。
    pub id: String,
    /// 役割（§15.2 の 7 役割またはカスタムロール、§5.9.2）。
    pub role: String,
    /// 対象（`@01-04-02-03`。**書かれたまま**。解決は `core::id`）。
    pub scope: String,
    /// 参考例（`<範囲式> [contents|plot|both]`）。複数指定可。
    pub examples: Vec<String>,
    /// 意図（`初稿生成` 等）。
    pub intent: String,
    /// `status:`。文字列で持つのは未知の値で道連れにしないため（§4.3-4）。
    pub status: String,
    /// `initiator:`（`human` | `agent`）。
    pub initiator: String,
    /// **実行時点の**委譲レベル（監査用、§15.1.1）。
    pub delegation: Delegation,
    /// 使うプロンプト（`prompts/writer.md`）。
    pub prompt: String,
    /// 作成日時（RFC 3339）。
    pub created_at: String,
    /// 更新日時（RFC 3339）。
    pub updated_at: String,
    /// コンテキストのハッシュ（§15.4。`sha256:…`）。
    #[serde(rename = "context-hash")]
    pub context_hash: String,
    /// 結果ファイル（`agents/results/T0042.md`、§5.10.2）。
    pub result: String,
    /// MCP サーバ名（§5.11 / §15.8.1）。
    pub server: String,
    /// 人間のコメント（§15.5.3 の `{{user_comment}}` に入る）。
    #[serde(rename = "human-comment")]
    pub human_comment: String,
    /// 人間が外したブロック（§15.7.2 のトグル。`materials, history` のようにブロック名を `,` 区切り。
    /// 第 2.16 版、`docs/history.md` B-138）。
    pub exclude: String,
    /// §5.10.1 に無いキー。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Task {
    /// `status` を §5.10.1 の 8 値として読む。表に無ければ `None`。
    pub fn status_kind(&self) -> Option<TaskStatus> {
        TaskStatus::parse(&self.status)
    }

    /// `initiator` を §5.10.1 の 2 値として読む。表に無ければ `None`。
    pub fn initiator_kind(&self) -> Option<Initiator> {
        Initiator::parse(&self.initiator)
    }

    /// 対象を SceneId として読む（§5.10.1 `scope: @01-04-02-03`）。形が合わなければ `None`。
    pub fn scope_id(&self) -> Option<crate::core::id::SceneId> {
        crate::core::id::PartialSceneId::parse_with_sigil(&self.scope)
            .ok()
            .map(|found| found.expand_low())
    }
}
