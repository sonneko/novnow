//! `novnow mcp <dir>`（§14.1・§15.8.3）: MCP サーバとして起動し、外部の AI クライアントにプロジェクトの
//! ドメイン API（`core::agents::server` のツール）を公開する。
//!
//! - §14.2 の検査（不備なら 3）を先に行う。通ったら、`agents.enabled` なら `prompts/` の欠けた既定
//!   ファイルを書き出し（§15.5.4。`prompts/` 自体が無ければ I050 を stderr に出す）、標準入出力で
//!   改行区切りの JSON-RPC 2.0 を読み書きする。標準入力が閉じたら 0 で終わる。
//! - `initialize` / `ping` / `tools/list` / `tools/call` に答える。知らない要求は -32601。
//!   ツール表は §19.5.1: 既存 14 ツール（凍結）+ 正規操作カタログ（§19.7）の「M—」以外の全件。
//! - `tools/call` のたびにプロジェクトを読み直す（GUI と同時に開いていても古い中身で書かない）。
//!   書き込みは開いたプロジェクトの中だけ（`FsStorage` → `FsGate`、規則 10）。
//! - 結果は JSON のテキスト 1 つ。失敗は `isError: true` と、カタログの文言（E036 などは診断の文言）。
//!   標準出力はプロトコルだけに使い、それ以外は stderr に出す（`docs/history.md` B-149）。

use std::collections::BTreeMap;
use std::io::{self, BufRead};
use std::path::Path;

use serde_json::{json, Value};

use crate::adapters::clock_system::{self, SystemClock};
use crate::adapters::project_load;
use crate::adapters::storage_fs::FsStorage;
use crate::adapters::{block_on, mcp_stdio};
use crate::app::error::{Class, Error};
use crate::app::ops::{self, Args};
use crate::app::session::{Interface, Session};
use crate::core::agents::server::{self, ToolError};
use crate::core::agents::{prompts, Plan};
use crate::core::diagnostics::{Diagnostic, DiagnosticCode};
use crate::core::policy::write_policy::ProjectPath;
use crate::ports::env::Env;
use crate::ports::storage::{Storage, StorageError};
use crate::ui::i18n;

use super::open::{describe, open_storage};
use super::{open, Cli, Exit, Stream};

/// 実行する。`input` が無ければ標準入力を読む。
pub fn run<E: Env>(cli: &mut Cli<'_, E>, dir: &Path) -> Exit {
    if let Err(exit) = open::precheck(cli, dir) {
        return exit;
    }
    let storage = match open_storage(cli, dir) {
        Ok(storage) => storage,
        Err(exit) => return exit,
    };
    let title = storage
        .root()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    write_prompts(cli, &storage);
    match cli.take_input() {
        Some(input) => serve(cli, &storage, &title, input),
        None => {
            let stdin = io::stdin();
            let mut lock = stdin.lock();
            serve(cli, &storage, &title, &mut lock)
        }
    }
}

/// 欠けた既定のプロンプトを書く（§15.5.4。あるファイルには触れない）。
fn write_prompts<E: Env>(cli: &mut Cli<'_, E>, storage: &FsStorage) {
    // 読み取り専用なら書かない（§4.1 の W041。知らせは stderr に 1 行）
    if !storage.writable() {
        cli.line(
            Stream::Err,
            &i18n::diagnostic(
                cli.locale(),
                &Diagnostic::simple(DiagnosticCode::W041, None),
            ),
        );
        return;
    }
    let Ok(loaded) = block_on(project_load::load(storage)) else {
        return;
    };
    if !loaded.project.settings().agents.enabled {
        return;
    }
    let (missing, absent) = prompts::missing(&loaded.project);
    let mut written = false;
    for (path, text) in missing {
        // 一覧に無くてもディスクに何かある（外を指すシンボリックリンク）なら触れない（B-155）
        if let Ok(path) = ProjectPath::new(path) {
            if !storage.exists(&path) {
                written |= block_on(storage.write(&path, text)).is_ok();
            }
        }
    }
    if written && absent {
        let message = i18n::diagnostic(
            cli.locale(),
            &Diagnostic::simple(DiagnosticCode::I050, None),
        );
        cli.line(Stream::Err, &message);
    }
}

fn id_key(id: &Value) -> Option<String> {
    match id {
        Value::Number(num) => Some(format!("n:{num}")),
        Value::String(text) => Some(format!("s:{text}")),
        _ => None,
    }
}

fn cancel_target(message: &Value) -> Option<String> {
    let method = message.get("method").and_then(Value::as_str)?;
    if !matches!(method, "notifications/cancelled" | "$/cancelRequest") {
        return None;
    }
    let params = message.get("params")?;
    params
        .get("requestId")
        .or_else(|| params.get("id"))
        .and_then(id_key)
}

fn read_available_lines(buf: &mut Vec<u8>, input: &mut dyn BufRead) -> Vec<String> {
    let take_lines = |buffer: &mut Vec<u8>| -> Vec<String> {
        let mut lines = Vec::new();
        while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
            let chunk: Vec<u8> = buffer.drain(..=pos).collect();
            lines.push(String::from_utf8_lossy(&chunk).into_owned());
        }
        lines
    };
    let ready = take_lines(buf);
    if !ready.is_empty() {
        return ready;
    }
    loop {
        let chunk = match input.fill_buf() {
            Ok([]) | Err(_) => {
                if buf.is_empty() {
                    return Vec::new();
                }
                let tail = String::from_utf8_lossy(buf).into_owned();
                buf.clear();
                return vec![tail];
            }
            Ok(chunk) => chunk,
        };
        let count = chunk.len();
        buf.extend_from_slice(chunk);
        input.consume(count);
        let lines = take_lines(buf);
        if !lines.is_empty() {
            return lines;
        }
    }
}

fn serve<E: Env>(
    cli: &mut Cli<'_, E>,
    storage: &FsStorage,
    title: &str,
    input: &mut dyn BufRead,
) -> Exit {
    let mut raw_buf = Vec::new();
    let mut cancelled: BTreeMap<String, usize> = BTreeMap::new();
    loop {
        let lines = read_available_lines(&mut raw_buf, input);
        if lines.is_empty() {
            return Exit::Ok;
        }
        let mut parsed: Vec<Option<Result<Value, ()>>> = Vec::with_capacity(lines.len());
        for line in &lines {
            let text = line.trim();
            if text.is_empty() {
                parsed.push(None);
            } else {
                parsed.push(Some(serde_json::from_str::<Value>(text).map_err(|_| ())));
            }
        }
        let mut consumed = vec![false; parsed.len()];

        for idx in 0..parsed.len() {
            let Some(entry) = &parsed[idx] else {
                continue;
            };
            let Ok(message) = entry else {
                reply(
                    cli,
                    &json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } }),
                );
                continue;
            };
            if let Some(target) = cancel_target(message) {
                if !consumed[idx] {
                    *cancelled.entry(target).or_insert(0) += 1;
                }
                continue;
            }
            let Some(id) = message.get("id").cloned() else {
                // 通知（`notifications/initialized` など）には答えない
                continue;
            };
            if let Some(req_key) = id_key(&id) {
                let mut is_cancelled = false;
                if let Some(count) = cancelled.get_mut(&req_key) {
                    if *count <= 1 {
                        cancelled.remove(&req_key);
                    } else {
                        *count -= 1;
                    }
                    is_cancelled = true;
                } else {
                    for (later_idx, later) in parsed.iter().enumerate().skip(idx + 1) {
                        let Some(Ok(later_msg)) = later else {
                            continue;
                        };
                        if !consumed[later_idx]
                            && cancel_target(later_msg).as_deref() == Some(req_key.as_str())
                        {
                            consumed[later_idx] = true;
                            is_cancelled = true;
                            break;
                        }
                        if later_msg.get("id").and_then(id_key).as_deref() == Some(req_key.as_str())
                        {
                            break;
                        }
                    }
                }
                if is_cancelled {
                    let err = Error::cancelled(cli.locale());
                    reply(
                        cli,
                        &json!({ "jsonrpc": "2.0", "id": id, "result": tool_failure(&err) }),
                    );
                    continue;
                }
            }

            let method = message.get("method").and_then(Value::as_str).unwrap_or("");
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let result = match method {
                "initialize" => {
                    if let Some(tag) = params.get("locale").and_then(Value::as_str).or_else(|| {
                        params
                            .get("capabilities")
                            .and_then(|c| c.get("locale"))
                            .and_then(Value::as_str)
                    }) {
                        if tag == "en" || tag.starts_with("en-") {
                            cli.set_locale(i18n::Locale::En);
                        } else if tag == "ja" || tag.starts_with("ja-") {
                            cli.set_locale(i18n::Locale::Ja);
                        }
                    }
                    Ok(json!({
                        "protocolVersion": mcp_stdio::PROTOCOL_VERSION,
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "novnow", "version": env!("CARGO_PKG_VERSION") }
                    }))
                }
                "ping" => Ok(json!({})),
                "tools/list" => {
                    Ok(json!({ "tools": ops::mcp_tools(cli.locale(), &server::tools()) }))
                }
                "tools/call" => Ok(call(cli, storage, title, &params)),
                _ => Err(json!({ "code": -32601, "message": "Method not found" })),
            };
            let response = match result {
                Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
            };
            reply(cli, &response);
        }
    }
}

fn reply<E: Env>(cli: &mut Cli<'_, E>, message: &Value) {
    cli.line(Stream::Out, &message.to_string());
}

/// `tools/call` の結果（MCP の `CallToolResult`）。
///
/// 振り分け（§19.1.4「単一実装点」・§19.8.3「既存 14 ツールの凍結」）:
///
/// 1. 既存 14 ツールは `REF:src/core/agents/server.rs` の実装をそのまま使う（出力バイト不変）。
///    例外は `novnow.plot.write` の `outline` / `chapter`（§19.5.2 で追加）だけで、これは正規操作の
///    実装（`REF:src/app/ops/editing.rs`）へ回す。
/// 2. それ以外の `novnow.<正規操作 ID>` は `OPS` 表の実装を `Interface::Mcp` の 1 回分のセッションで
///    実行する（§19.1.6「呼び出しごとに読み直す」）。関門（§19.1.5）と「M—」例外・人間の専権事項
///    4 件は `Session::run` が断る。
/// 3. どちらでもない名前は未知のツール（既存の文言）。
fn call<E: Env>(cli: &mut Cli<'_, E>, storage: &FsStorage, title: &str, params: &Value) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return tool_failure(&Error::missing(cli.locale(), "name"));
    };
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => json!({}),
        Some(val @ Value::Object(_)) => val.clone(),
        Some(_) => return tool_failure(&Error::usage(cli.locale(), "arguments")),
    };
    let level = arguments
        .get("level")
        .and_then(Value::as_str)
        .unwrap_or("episode");
    let frozen = server::is_tool(name) && !(name == "novnow.plot.write" && level != "episode");
    if !frozen {
        let Some(op) = ops::op_of_tool(name) else {
            let err = Error::new(Class::NotFound, cli.format("cli.mcp.unknown_tool", &[name]));
            return tool_failure(&err);
        };
        return call_op(cli, storage, op, &arguments);
    }
    let loaded = match block_on(project_load::load(storage)) {
        Ok(loaded) => loaded,
        Err(error) => {
            return tool_failure(&storage_to_app_error(cli, &error));
        }
    };
    let now = clock_system::now_rfc3339(&SystemClock);
    let outcome = server::call(&loaded.project, name, &arguments, &now, title);
    if let Err(error) = execute(cli, storage, &outcome.plan) {
        return tool_failure(&error);
    }
    match outcome.result {
        Ok(value) => answer(&value),
        Err(error) => tool_failure(&tool_to_app_error(cli, &error)),
    }
}

/// 正規操作を MCP の 1 回分（状態を持たない。§19.1.6）として実行する。
fn call_op<E: Env>(
    cli: &mut Cli<'_, E>,
    storage: &FsStorage,
    op: &str,
    arguments: &Value,
) -> Value {
    let mut session = match Session::open_with(storage.root(), Interface::Mcp) {
        Ok(session) => session,
        Err(error) => return tool_failure(&error),
    };
    session.set_locale(cli.locale());
    let args = match arguments {
        Value::Object(map) => Value::Object(map.clone()),
        Value::Null => Value::Object(Args::new()),
        _ => return tool_failure(&Error::usage(session.locale, "arguments")),
    };
    match session.run(op, args) {
        Ok(data) => answer(&data),
        Err(error) => tool_failure(&error),
    }
}

/// 成功の `CallToolResult`（結果は JSON のテキスト 1 つ）。
fn answer(value: &Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": value.to_string() }],
        "isError": false
    })
}

/// 失敗の `CallToolResult`（§19.1.7・§15.8.3）。
///
/// MCP 従来どおり `isError: true` と `content[0].text` を維持しつつ、`error`（`code`・`kind`・
/// `message`・`path`）と `data`（`jsonrpc` 符号を含む）を揃えて 9 分類を欠落なく伝える。
pub fn tool_failure(error: &Error) -> Value {
    let mut data = error.to_json();
    if let Some(map) = data.as_object_mut() {
        map.insert("jsonrpc".to_owned(), json!(error.class.jsonrpc()));
    }
    json!({
        "content": [{ "type": "text", "text": error.message }],
        "data": data,
        "error": error.to_json(),
        "isError": true,
    })
}

/// 計画を実行する（書き込みはプロジェクトの中だけ。ログは追記）。
fn execute<E: Env>(cli: &Cli<'_, E>, storage: &FsStorage, plan: &Plan) -> Result<(), Error> {
    for change in &plan.writes {
        let path = ProjectPath::new(&change.path).map_err(|violation| {
            Error::policy(cli.locale(), "E060", format!("{violation:?}"))
                .with_path(change.path.clone())
        })?;
        block_on(storage.write(&path, &change.text))
            .map_err(|err| storage_to_app_error(cli, &err))?;
    }
    for event in &plan.log {
        let log_path = event.path();
        let path = ProjectPath::new(&log_path).map_err(|violation| {
            Error::policy(cli.locale(), "E060", format!("{violation:?}"))
                .with_path(log_path.clone())
        })?;
        block_on(storage.append(&path, &event.line()))
            .map_err(|err| storage_to_app_error(cli, &err))?;
    }
    Ok(())
}

fn storage_to_app_error<E: Env>(cli: &Cli<'_, E>, error: &StorageError) -> Error {
    let message = describe(cli, error);
    match error {
        StorageError::Policy(violation) => {
            let diag = violation.diagnostic();
            Error::policy(cli.locale(), diag.code.as_str(), message)
        }
        StorageError::NotFound(path) => {
            Error::not_found(cli.locale(), path.as_str(), Some(path.as_str()))
        }
        StorageError::Encoding(path) => Error::io(message, Some(path.as_str())).with_kind("S024"),
        StorageError::Io { path, .. } => Error::io(message, Some(path.as_str())).with_kind("S030"),
        _ => Error::io(message, None).with_kind("S030"),
    }
}

fn tool_to_app_error<E: Env>(cli: &Cli<'_, E>, error: &ToolError) -> Error {
    let message = message_of(cli, error);
    match error {
        ToolError::Unknown(_) => Error::new(Class::NotFound, message),
        ToolError::Argument(_) => Error::new(Class::Usage, message),
        ToolError::Diagnostic(found) => {
            let class = if found.code == DiagnosticCode::E036 {
                Class::Policy
            } else {
                Class::Validation
            };
            Error::new(class, message).with_kind(found.code.as_str())
        }
        ToolError::Missing(_) => Error::new(Class::NotFound, message),
        ToolError::Gate { .. } => Error::new(Class::Validation, message),
    }
}

fn message_of<E: Env>(cli: &Cli<'_, E>, error: &ToolError) -> String {
    match error {
        ToolError::Unknown(name) => cli.format("cli.mcp.unknown_tool", &[name]),
        ToolError::Argument(name) => cli.format("cli.mcp.argument", &[name]),
        ToolError::Diagnostic(found) => i18n::diagnostic(cli.locale(), found),
        ToolError::Missing(what) => cli.format("cli.mcp.missing", &[what]),
        ToolError::Gate { proposal, codes } => cli.format("cli.mcp.gate", &[proposal, codes]),
    }
}
