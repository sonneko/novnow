//! stdio の MCP クライアント（§15.8.1）。`std::process` とスレッドだけで書く（`PLANS.md` §9.2 B-147）。
//!
//! 1 回の呼び出しごとにサーバを起動し、改行区切りの JSON-RPC 2.0 で
//! `initialize` → `notifications/initialized` → `sampling/createMessage`（または `tools/call`）を送って、
//! 応答を受けたらサーバを止める（**1 タスク 1 呼び出し**）。
//!
//! - 待っている間にサーバから来た `ping` には答え、それ以外の要求には -32601（Method not found）を返す。
//! - stderr は捨てる（API キーなどがログに混ざらないように、画面にも出さない）。
//! - タイムアウト（`timeout_ms`）・取り消し（`cancel`）・サーバの終了は、50ms ごとに見る。
//! - 起動できない・途中で終わった → [`McpError::Connect`]（E063）。

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::core::agents::{cap_output, runner};
use crate::ports::mcp::{McpCall, McpClient, McpError, McpReply, McpServer};

/// MCP のプロトコルの版（`initialize` で送る）。
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// 取り消し・タイムアウトを見る間隔。
const TICK: Duration = Duration::from_millis(50);

/// stdio の MCP クライアント。
#[derive(Debug, Clone, Copy, Default)]
pub struct StdioClient;

impl McpClient for StdioClient {
    fn call(
        &self,
        server: &McpServer,
        call: &McpCall,
        cancel: &AtomicBool,
    ) -> Result<McpReply, McpError> {
        if server.command == "__novnow_http_mcp" {
            return super::mcp_http::call(server, call, cancel);
        }
        let deadline = Instant::now() + Duration::from_millis(server.timeout_ms.max(1));
        let mut child = Command::new(&server.command)
            .args(&server.args)
            .envs(server.env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| McpError::Connect {
                server: server.name.clone(),
                message: error.to_string(),
            })?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let (sender, receiver) = mpsc::channel::<Value>();
        if let Some(stdout) = stdout {
            // 読み手のスレッド: 1 行 1 メッセージ。読めない行は飛ばす。サーバの stdout が閉じたら終わる
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else {
                        break;
                    };
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(value) = serde_json::from_str::<Value>(line) {
                        if sender.send(value).is_err() {
                            break;
                        }
                    }
                }
            });
        }
        let mut link = Link {
            server: &server.name,
            stdin,
            receiver,
            deadline,
            cancel,
        };
        let result = link.run(call);
        link.stdin = None;
        stop(&mut child);
        result
    }
}

/// サーバを止める（もう終わっていれば何もしない）。
fn stop(child: &mut Child) {
    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// 1 回分のやり取り。
struct Link<'a> {
    server: &'a str,
    stdin: Option<ChildStdin>,
    receiver: Receiver<Value>,
    deadline: Instant,
    cancel: &'a AtomicBool,
}

impl Link<'_> {
    fn run(&mut self, call: &McpCall) -> Result<McpReply, McpError> {
        self.request(
            1,
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "novnow", "version": env!("CARGO_PKG_VERSION") }
            }),
        )?;
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))?;
        let result = match call {
            McpCall::Sampling { params } => {
                self.request(2, "sampling/createMessage", params.clone())?
            }
            McpCall::Tool { name, arguments } => {
                let result = self.request(
                    2,
                    "tools/call",
                    json!({ "name": name, "arguments": arguments }),
                )?;
                if result.get("isError").and_then(Value::as_bool) == Some(true) {
                    let text = runner::reply_of(&result)
                        .map(|found| found.0)
                        .unwrap_or_default();
                    return Err(McpError::Protocol(text));
                }
                result
            }
        };
        let (text, model) = runner::reply_of(&result)
            .ok_or_else(|| McpError::Protocol("no text content".to_owned()))?;
        let (text, truncated) = cap_output(&text);
        Ok(McpReply {
            text,
            model,
            truncated,
        })
    }

    fn lost(&self, message: impl Into<String>) -> McpError {
        McpError::Connect {
            server: self.server.to_owned(),
            message: message.into(),
        }
    }

    fn send(&mut self, message: &Value) -> Result<(), McpError> {
        let mut line = message.to_string();
        line.push('\n');
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(self.lost("stdin closed"));
        };
        let written = stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.flush());
        written.map_err(|error| self.lost(error.to_string()))
    }

    /// 要求を送り、同じ `id` の応答を待つ（その間に来たサーバからの要求に答える）。
    fn request(&mut self, id: u64, method: &str, params: Value) -> Result<Value, McpError> {
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(McpError::Cancelled);
            }
            let now = Instant::now();
            if now >= self.deadline {
                return Err(McpError::Timeout);
            }
            let wait = TICK.min(self.deadline - now);
            let message = match self.receiver.recv_timeout(wait) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Err(self.lost("server exited")),
            };
            if let Some(method) = message.get("method").and_then(Value::as_str) {
                // サーバからの要求（id がある）か通知（id が無い）
                if let Some(request) = message.get("id") {
                    let reply = if method == "ping" {
                        json!({ "jsonrpc": "2.0", "id": request, "result": {} })
                    } else {
                        json!({
                            "jsonrpc": "2.0",
                            "id": request,
                            "error": { "code": -32601, "message": "Method not found" }
                        })
                    };
                    self.send(&reply)?;
                }
                continue;
            }
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                let text = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("error")
                    .to_owned();
                return Err(McpError::Protocol(text));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }
}
