//! stdio の MCP クライアント（§15.8.1）のテスト。サーバは `sh` の小さなスクリプト（Unix だけ）。
//!
//! クライアントが送る `id` は `initialize` = 1・呼び出し = 2 で固定なので、スクリプトは行を読んで
//! 決まった応答を返すだけでよい。

#![cfg(unix)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use novnow::adapters::mcp_stdio::StdioClient;
use novnow::core::agents::OUTPUT_LIMIT_CHARS;
use novnow::ports::mcp::{McpCall, McpClient, McpError, McpServer};
use serde_json::json;

const INIT: &str = r#"read line; echo '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"fake"}}}'; read line; read line; "#;

fn server(script: &str, timeout_ms: u64) -> McpServer {
    McpServer {
        name: "fake".to_owned(),
        command: "sh".to_owned(),
        args: vec!["-c".to_owned(), format!("{INIT}{script}")],
        env: vec![("NOVNOW_TEST_KEY".to_owned(), "k-123".to_owned())],
        timeout_ms,
    }
}

fn sampling() -> McpCall {
    McpCall::Sampling {
        params: json!({ "messages": [], "maxTokens": 10 }),
    }
}

fn call(server: &McpServer, call: &McpCall) -> Result<novnow::ports::mcp::McpReply, McpError> {
    StdioClient.call(server, call, &AtomicBool::new(false))
}

#[test]
fn a_sampling_call_answers_pings_and_returns_the_text_and_the_model() {
    let script = r#"echo '{"jsonrpc":"2.0","id":"p1","method":"ping"}'; read pong; case "$pong" in *'"id":"p1"'*) ;; *) exit 3;; esac; echo '{"jsonrpc":"2.0","method":"notifications/message","params":{}}'; echo '{"jsonrpc":"2.0","id":2,"result":{"role":"assistant","content":{"type":"text","text":"本文です。"},"model":"fake-1"}}'"#;
    let reply = call(&server(script, 5_000), &sampling()).unwrap();
    assert_eq!(reply.text, "本文です。");
    assert_eq!(reply.model, "fake-1");
    assert!(!reply.truncated);
}

#[test]
fn the_resolved_environment_reaches_the_server() {
    let script = r#"printf '{"jsonrpc":"2.0","id":2,"result":{"content":{"type":"text","text":"%s"}}}\n' "$NOVNOW_TEST_KEY""#;
    let reply = call(&server(script, 5_000), &sampling()).unwrap();
    assert_eq!(reply.text, "k-123");
}

#[test]
fn other_server_requests_get_method_not_found() {
    let script = r#"echo '{"jsonrpc":"2.0","id":9,"method":"roots/list"}'; read answer; case "$answer" in *-32601*) echo '{"jsonrpc":"2.0","id":2,"result":{"content":{"type":"text","text":"ok"}}}';; *) exit 4;; esac"#;
    assert_eq!(
        call(&server(script, 5_000), &sampling()).unwrap().text,
        "ok"
    );
}

#[test]
fn tools_calls_read_the_content_array_and_is_error() {
    let script = r#"case "$line" in *'"name":"generate"'*) echo '{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]}}';; *) exit 5;; esac"#;
    let tool = McpCall::Tool {
        name: "generate".to_owned(),
        arguments: json!({ "prompt": "x" }),
    };
    assert_eq!(call(&server(script, 5_000), &tool).unwrap().text, "a\nb");
    let failing = r#"echo '{"jsonrpc":"2.0","id":2,"result":{"isError":true,"content":[{"type":"text","text":"quota"}]}}'"#;
    assert_eq!(
        call(&server(failing, 5_000), &tool).unwrap_err(),
        McpError::Protocol("quota".to_owned())
    );
}

#[test]
fn error_responses_and_missing_text_are_protocol_errors() {
    let script = r#"echo '{"jsonrpc":"2.0","id":2,"error":{"code":-1,"message":"rate limited"}}'"#;
    assert_eq!(
        call(&server(script, 5_000), &sampling()).unwrap_err(),
        McpError::Protocol("rate limited".to_owned())
    );
    let empty = r#"echo '{"jsonrpc":"2.0","id":2,"result":{"content":[]}}'"#;
    assert!(matches!(
        call(&server(empty, 5_000), &sampling()),
        Err(McpError::Protocol(_))
    ));
}

#[test]
fn long_outputs_are_cut_at_twenty_thousand_chars() {
    let script = r#"text=$(yes a | head -c 40010 | tr -d '\n'); printf '{"jsonrpc":"2.0","id":2,"result":{"content":{"type":"text","text":"%s"}}}\n' "$text""#;
    let reply = call(&server(script, 10_000), &sampling()).unwrap();
    assert!(reply.truncated);
    assert_eq!(reply.text.chars().count(), OUTPUT_LIMIT_CHARS);
}

#[test]
fn a_missing_command_or_an_exiting_server_is_a_connect_error() {
    let missing = McpServer {
        command: "/nonexistent/novnow-test-mcp".to_owned(),
        ..server("", 1_000)
    };
    assert!(matches!(
        call(&missing, &sampling()),
        Err(McpError::Connect { server, .. }) if server == "fake"
    ));
    let exiting = McpServer {
        args: vec!["-c".to_owned(), "read line; exit 0".to_owned()],
        ..server("", 5_000)
    };
    assert!(matches!(
        call(&exiting, &sampling()),
        Err(McpError::Connect { .. })
    ));
    assert_eq!(
        McpError::Connect {
            server: "llm".to_owned(),
            message: "x".to_owned()
        }
        .log_text(),
        "E063 llm"
    );
}

#[test]
fn a_silent_server_times_out() {
    let started = Instant::now();
    let result = call(&server("sleep 5", 300), &sampling());
    assert_eq!(result.unwrap_err(), McpError::Timeout);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_call_can_be_cancelled() {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let waiter = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        flag.store(true, Ordering::Relaxed);
    });
    let started = Instant::now();
    let result = StdioClient.call(&server("sleep 5", 10_000), &sampling(), &cancel);
    waiter.join().unwrap();
    assert_eq!(result.unwrap_err(), McpError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(3));
}
