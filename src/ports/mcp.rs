//! MCP クライアントのポート（§3.1・§15.8.1）。実装は `adapters::mcp_stdio`（M10）。
//!
//! §15.8.1: `agents/mcp.json` に定義されたサーバへ接続し、**1 タスク 1 呼び出し**で `sampling`
//! （モデル生成）か、許可された `tools`（`allow_tools`）を呼ぶ。既定のタイムアウトは 60 秒で、
//! キャンセルできる。サーバに繋がらなければ **E063**（タスクは `ready` のまま、自動再試行しない）。
//! M5 では形だけを決める（M10 で中身を作るときに広げてよい）。

use std::future::Future;

use serde_json::Value;

/// 既定のタイムアウト（§15.8.1「既定 60s」）。
pub const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// 1 回の呼び出し。
#[derive(Debug, Clone, PartialEq)]
pub enum McpCall {
    /// `sampling/createMessage`（モデル生成）。中身は MCP の要求そのもの。
    Sampling {
        /// 要求の本体。
        request: Value,
    },
    /// `tools/call`（`allow_tools` に載っているものだけ）。
    Tool {
        /// ツール名。
        name: String,
        /// 引数。
        arguments: Value,
    },
}

/// 呼び出しの失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpError {
    /// サーバに接続できない（**E063**）。
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
    /// 応答が MCP として読めない。
    Protocol(String),
}

/// MCP クライアント。
pub trait McpClient {
    /// `server`（`agents/mcp.json` のキー）に 1 回だけ呼び出しを送る。
    fn call(&self, server: &str, call: McpCall) -> impl Future<Output = Result<Value, McpError>>;
}
