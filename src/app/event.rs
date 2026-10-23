//! セッションのイベント（§19.2.4）。**egui に依存しない**。
//!
//! GUI が描画で表示する状態変化を、Rust の [`AppEvent`] として 1 箇所に定義する。同じ
//! データ構造を LSP のカスタム通知（§19.4.4 の `novnow/*` 4 種）と Python の
//! `session.poll_events()`（§19.3.5）が共有する（§19.2.4 MUST）。
//!
//! シリアライズは `serde` の内部タグ形式（`{ "kind": "...", ... }`）で、キー順は構造体の
//! 宣言順（§19.1.4 の決定論: 同じ状態への同じ操作は同じバイト列）。

use serde::Serialize;

/// セッションが外へ知らせる出来事。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppEvent {
    /// 診断の更新（§19.4.4 `novnow/diagnosticsChanged`）。
    DiagnosticsChanged {
        /// スコープ（`single` / `file` / `full`）。
        scope: String,
        /// 件数。
        count: usize,
        /// 対象のパス（`full` のときは空）。
        path: String,
    },
    /// 保存状態機の状態遷移（§19.4.4 `novnow/saveStateChanged`）。
    SaveStateChanged {
        /// §9.6 の 6 状態（`idle` / `dirty` / `saving` / `saved` / `error` / `conflict`）。
        state: String,
        /// 失敗の理由（あれば）。
        message: String,
    },
    /// エージェントタスクの進捗（§19.4.4 `novnow/agentProgress`）。
    AgentProgress {
        /// タスク ID（`T012`）。全体（`stop-all` など）のときは空。
        task: String,
        /// 段階（`queued` / `running` / `done` / `failed` / `blocked` / `cancelled`）。
        stage: String,
        /// 表示用のメッセージ。
        message: String,
    },
    /// 素材インボックスの変化（§19.4.4 `novnow/materialsChanged`）。
    MaterialsChanged {
        /// 素材 ID（`M0137`）。
        id: String,
        /// 変化（`proposed` / `approved` / `rejected` / `updated` / `attached`）。
        change: String,
    },
    /// 文書が変わった（保存・外部変更・リネーム）。
    DocumentChanged {
        /// パス。
        path: String,
        /// 変化（`written` / `external` / `renamed` / `removed`）。
        change: String,
    },
    /// 書き込みの失敗（§13.3.7 の容量不足など）。
    WriteFailed {
        /// パス。
        path: String,
        /// 理由。
        message: String,
    },
    /// 設定が変わった（§19.4.2 `workspace/didChangeConfiguration` 相当）。
    SettingsChanged {
        /// 変わったキー（`.novnow/ui.json` のキー名）。
        key: String,
    },
    /// 操作が中断された（§19.1.9）。
    Cancelled {
        /// 中断した操作の正規操作 ID。
        op: String,
        /// 対象（タスク ID・ハンドル番号の文字列）。
        handle: String,
    },
}

impl AppEvent {
    /// 対応する LSP カスタム通知の名前（§19.4.4）。対応が無ければ `None`。
    pub fn notification(&self) -> Option<&'static str> {
        Some(match self {
            Self::DiagnosticsChanged { .. } => "novnow/diagnosticsChanged",
            Self::SaveStateChanged { .. } => "novnow/saveStateChanged",
            Self::AgentProgress { .. } => "novnow/agentProgress",
            Self::MaterialsChanged { .. } => "novnow/materialsChanged",
            Self::DocumentChanged { .. }
            | Self::WriteFailed { .. }
            | Self::SettingsChanged { .. }
            | Self::Cancelled { .. } => return None,
        })
    }

    /// JSON（`session.poll_events`・CLI の `--json`・Python のイベントが共有する形）。
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "kind".to_owned(),
            serde_json::Value::String(self.kind().to_owned()),
        );
        match self {
            Self::DiagnosticsChanged { scope, count, path } => {
                map.insert("scope".into(), serde_json::json!(scope));
                map.insert("count".into(), serde_json::json!(count));
                map.insert("path".into(), serde_json::json!(path));
            }
            Self::SaveStateChanged { state, message } => {
                map.insert("state".into(), serde_json::json!(state));
                map.insert("message".into(), serde_json::json!(message));
            }
            Self::AgentProgress {
                task,
                stage,
                message,
            } => {
                map.insert("task".into(), serde_json::json!(task));
                map.insert("stage".into(), serde_json::json!(stage));
                map.insert("message".into(), serde_json::json!(message));
            }
            Self::MaterialsChanged { id, change } => {
                map.insert("id".into(), serde_json::json!(id));
                map.insert("change".into(), serde_json::json!(change));
            }
            Self::DocumentChanged { path, change } => {
                map.insert("path".into(), serde_json::json!(path));
                map.insert("change".into(), serde_json::json!(change));
            }
            Self::WriteFailed { path, message } => {
                map.insert("path".into(), serde_json::json!(path));
                map.insert("message".into(), serde_json::json!(message));
            }
            Self::SettingsChanged { key } => {
                map.insert("key".into(), serde_json::json!(key));
            }
            Self::Cancelled { op, handle } => {
                map.insert("op".into(), serde_json::json!(op));
                map.insert("handle".into(), serde_json::json!(handle));
            }
        }
        if let Some(name) = self.notification() {
            map.insert(
                "notification".into(),
                serde_json::Value::String(name.to_owned()),
            );
        }
        serde_json::Value::Object(map)
    }

    /// 種別名（`diagnostics_changed` など。`session.poll_events` の JSON と同じ）。
    pub fn kind(&self) -> &'static str {
        match self {
            Self::DiagnosticsChanged { .. } => "diagnostics_changed",
            Self::SaveStateChanged { .. } => "save_state_changed",
            Self::AgentProgress { .. } => "agent_progress",
            Self::MaterialsChanged { .. } => "materials_changed",
            Self::DocumentChanged { .. } => "document_changed",
            Self::WriteFailed { .. } => "write_failed",
            Self::SettingsChanged { .. } => "settings_changed",
            Self::Cancelled { .. } => "cancelled",
        }
    }
}
