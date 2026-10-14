//! MCP クライアントのポート（§3.1・§15.8.1）。実装は `adapters::mcp_stdio`。
//!
//! §15.8.1: `agents/mcp.json` に定義されたサーバへ接続し、**1 タスク 1 呼び出し**で `sampling`
//! （モデル生成）か、許可された `tools`（`allow_tools`）を呼ぶ。既定のタイムアウトは 60 秒で、
//! キャンセルできる。サーバに繋がらなければ **E063**（タスクは `ready` に戻り、自動再試行しない）。
//!
//! 呼び出しは**同期**で、呼ぶ側（GUI）がスレッドに載せる（tokio の `process` を使わない。vendored の
//! クレートだけで足りるように、`PLANS.md` §9.2 B-147）。取り消しは `cancel` の旗で伝える。

use std::sync::atomic::AtomicBool;

use serde_json::Value;

/// 既定のタイムアウト（§15.8.1「既定 60s」）。
pub const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// 起動するサーバ（`agents/mcp.json` の 1 件。環境変数は解決済み）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    /// `servers` のキー。
    pub name: String,
    /// コマンド。
    pub command: String,
    /// 引数。
    pub args: Vec<String>,
    /// 足す環境変数（解決済みの値。ログにも画面にも出さない）。
    pub env: Vec<(String, String)>,
    /// タイムアウト（ミリ秒）。
    pub timeout_ms: u64,
}

/// 1 回の呼び出し。
#[derive(Debug, Clone, PartialEq)]
pub enum McpCall {
    /// `sampling/createMessage`（モデル生成）。中身は MCP の `params` そのもの。
    Sampling {
        /// `params`。
        params: Value,
    },
    /// `tools/call`（`allow_tools` に載っているものだけ）。
    Tool {
        /// ツール名。
        name: String,
        /// 引数。
        arguments: Value,
    },
}

/// 応答（本文は 20,000 字で切ったもの、§15.9）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpReply {
    /// 本文。
    pub text: String,
    /// サーバが返したモデル識別子（無ければ空）。
    pub model: String,
    /// 切ったか。
    pub truncated: bool,
}

/// 呼び出しの失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpError {
    /// サーバに接続できない・途中で落ちた（**E063**）。
    Connect {
        /// サーバ名（`agents/mcp.json` のキー）。
        server: String,
        /// 理由（ログ用）。
        message: String,
    },
    /// タイムアウト。
    Timeout,
    /// 人間が取り消した。
    Cancelled,
    /// `allow_tools` に無いツール。
    ToolNotAllowed(String),
    /// 応答が MCP として読めない・サーバがエラーを返した。
    Protocol(String),
}

impl McpError {
    /// 監査ログの `error` に書く短い理由（秘密情報を入れない）。
    pub fn log_text(&self) -> String {
        match self {
            Self::Connect { server, .. } => format!("E063 {server}"),
            Self::Timeout => "timeout".to_owned(),
            Self::Cancelled => "canceled".to_owned(),
            Self::ToolNotAllowed(name) => format!("tool not allowed: {name}"),
            Self::Protocol(message) => {
                let short: String = message.chars().take(200).collect();
                format!("protocol: {short}")
            }
        }
    }
}

/// MCP クライアント。
pub trait McpClient {
    /// `server` に 1 回だけ呼び出しを送る（終わるまで戻らない）。`cancel` が立ったら止める。
    fn call(
        &self,
        server: &McpServer,
        call: &McpCall,
        cancel: &AtomicBool,
    ) -> Result<McpReply, McpError>;
}
