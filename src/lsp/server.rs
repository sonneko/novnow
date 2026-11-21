//! 受け付け・振り分け・通知の送出（§19.4.1）。
//!
//! 標準入出力で JSON-RPC 2.0 を回す。1 メッセージ = 1 リクエスト/通知。応答を待たない
//! 通知（§19.4.4 の `novnow/*`）は、リクエストの合間に溜まったイベントから作る。

use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;

use serde_json::{json, Value};

use crate::app::error::{Class, Error};
use crate::app::session::Session;
use crate::ui::i18n::Locale;

use super::commands;
use super::framing;
use super::methods::{self, Message, Outbox, Outcome};

/// 対応する LSP の版。
pub const PROTOCOL: &str = "3.17";

/// JSON-RPC 2.0 の誤り（§19.1.4 の写し。MCP と同じ表）。
pub const PARSE_ERROR: i64 = -32700;
/// 形が壊れたリクエスト。
pub const INVALID_REQUEST: i64 = -32600;
/// 知らないメソッド。
pub const METHOD_NOT_FOUND: i64 = -32601;
/// 引数の誤り。
pub const INVALID_PARAMS: i64 = -32602;
/// サーバ側の失敗。
pub const INTERNAL_ERROR: i64 = -32603;
/// I/O などのサーバエラー（§19.1.7）。
pub const SERVER_ERROR: i64 = -32000;
/// リクエストの中断（`$/cancelRequest`、§19.1.7・§19.1.9）。
pub const REQUEST_CANCELLED: i64 = -32800;

/// 枠付きのメッセージを受け取る口。
///
/// 実体は CLI 側の標準出力（`novnow lsp`）でも、テストの内存でもよい。書き込みの失敗は
/// 口が閉じた印なので、ここでは握りつぶして次の周回で `read_message` が `None` を返すのに任せる。
pub trait Sink {
    /// 1 メッセージを枠に入れて出す。
    fn frame(&mut self, body: &str);
}

/// 任意の [`std::io::Write`] を [`Sink`] にする包み（テストと `Writer(Vec::new())`）。
///
/// 総当たりの `impl<W: Write> Sink for W` は CLI 側の独自実装と衝突するので、包む。
pub struct Writer<W>(pub W);

impl<W: std::io::Write> Sink for Writer<W> {
    fn frame(&mut self, body: &str) {
        let _ = framing::write_message(&mut self.0, body);
    }
}

#[derive(Default)]
struct ServerState {
    initialized: bool,
    cancelled: BTreeMap<String, usize>,
    /// 要求 ID → その要求が起こしたエージェントタスク（§19.1.9 の取り消し対象）。
    ///
    /// `$/cancelRequest` は**要求 ID に対する**取り消しなので、その要求が起こしたタスクだけを
    /// 止める。他の自律タスク（running / queue）には触れない。
    started: BTreeMap<String, Vec<String>>,
    open_docs: BTreeSet<String>,
}

impl ServerState {
    fn add_cancel(&mut self, key: String) {
        *self.cancelled.entry(key).or_insert(0) += 1;
    }

    fn take_cancel(&mut self, key: &str) -> bool {
        let Some(count) = self.cancelled.get_mut(key) else {
            return false;
        };
        if *count <= 1 {
            self.cancelled.remove(key);
        } else {
            *count -= 1;
        }
        true
    }
}

fn id_key(id: &Value) -> Option<String> {
    match id {
        Value::Number(num) => Some(format!("n:{num}")),
        Value::String(text) => Some(format!("s:{text}")),
        _ => None,
    }
}

fn cancel_target_key(msg: &Value) -> Option<String> {
    if msg.get("method").and_then(Value::as_str) != Some("$/cancelRequest") {
        return None;
    }
    msg.get("params")
        .and_then(|params| params.get("id"))
        .and_then(id_key)
}

/// いま実行中・待機中のエージェントタスク（要求が起こしたものを比べるための写し）。
fn held_tasks(session: &Session) -> Vec<String> {
    let mut tasks: Vec<String> = session
        .agents
        .running
        .iter()
        .map(|found| found.task.clone())
        .collect();
    tasks.extend(session.agents.queue.iter().map(|(task, _, _)| task.clone()));
    tasks.sort();
    tasks
}

/// 1 つの要求が起こしたタスクだけを止める（§19.1.9「実行中の要求の取り消し」）。
///
/// すでに終わったタスク（running / queue に無い）では何もしない＝**完了済み ID の取り消しで
/// 結果や部分書き込みが変わらない**。総停止（`agent.stop_all`。§15.7.4）は委譲レベルまで
/// 下げてしまうので、ここでは使わない。
fn cancel_tasks(session: &mut Session, tasks: &[String]) {
    for task in tasks {
        if !session.agents.holds(task) {
            continue;
        }
        let id = task.clone();
        {
            let Session {
                bench,
                host,
                agents,
                clock,
                ..
            } = session;
            crate::app::agents::cancel(&id, bench, host.as_mut(), agents, *clock);
        }
        session.push(crate::app::event::AppEvent::Cancelled {
            op: "agent.task_run".to_owned(),
            handle: id,
        });
    }
}

/// サーバを 1 本動かす。エディタが口を閉じるまで回る。
///
/// `root` はプロジェクトルート（URI と相対パスの往復に使う）。
pub fn run(session: &mut Session, root: &str, input: &mut dyn BufRead, out: &mut dyn Sink) {
    let mut state = ServerState::default();
    let mut outbox = Outbox::default();
    let mut raw_buf = Vec::new();
    loop {
        let bodies = framing::read_available_messages(&mut raw_buf, input);
        if bodies.is_empty() {
            break;
        }
        let mut parsed: Vec<Result<Value, ()>> = Vec::with_capacity(bodies.len());
        for body in &bodies {
            parsed.push(serde_json::from_str::<Value>(body).map_err(|_| ()));
        }
        let mut consumed_cancels = vec![false; parsed.len()];

        for idx in 0..parsed.len() {
            drain(session, &mut outbox, root);
            let Ok(value) = &parsed[idx] else {
                write_error(out, Value::Null, PARSE_ERROR, "parse error");
                continue;
            };
            let id = value.get("id").cloned();
            let Some(method) = value.get("method").and_then(Value::as_str) else {
                write_error(
                    out,
                    value.get("id").cloned().unwrap_or(Value::Null),
                    INVALID_REQUEST,
                    "invalid request",
                );
                continue;
            };
            // すでに終わった（running / queue から外れた）タスクの対応は忘れる。完了済み ID
            // の取り消しで結果や部分書き込みが変わらないようにする（§19.1.9）。
            state.started.retain(|_, tasks| {
                tasks.retain(|task| session.agents.holds(task));
                !tasks.is_empty()
            });
            if method == "$/cancelRequest" {
                if !consumed_cancels[idx] {
                    if let Some(key) = cancel_target_key(value) {
                        state.add_cancel(key.clone());
                        // 取り消すのは**この要求 ID に対する**ものだけ（§19.1.9）。
                        // 知らない ID・完了済みの ID・エージェントでない要求の ID では何も止まらない。
                        if let Some(tasks) = state.started.remove(&key) {
                            cancel_tasks(session, &tasks);
                        }
                    }
                }
                drain(session, &mut outbox, root);
                flush(&mut outbox, out);
                continue;
            }
            if let Some(req_id) = &id {
                if let Some(req_key) = id_key(req_id) {
                    let mut cancelled_now = state.take_cancel(&req_key);
                    if !cancelled_now {
                        for (later_idx, item) in parsed.iter().enumerate().skip(idx + 1) {
                            let Ok(later_val) = item else {
                                continue;
                            };
                            if !consumed_cancels[later_idx]
                                && cancel_target_key(later_val).as_deref() == Some(req_key.as_str())
                            {
                                consumed_cancels[later_idx] = true;
                                cancelled_now = true;
                                break;
                            }
                            if later_val.get("id").and_then(id_key).as_deref()
                                == Some(req_key.as_str())
                            {
                                break;
                            }
                        }
                    }
                    if cancelled_now {
                        // 処理を始める前に取り消された: その要求が起こしたタスクだけを止める
                        // （§19.1.9）。他の自律タスクは動き続ける。
                        if let Some(tasks) = state.started.remove(&req_key) {
                            cancel_tasks(session, &tasks);
                        }
                        let err = Error::cancelled(session.locale);
                        write_app_error(out, req_id.clone(), REQUEST_CANCELLED, &err);
                        flush(&mut outbox, out);
                        continue;
                    }
                }
            }

            let method = method.to_owned();
            let params = value.get("params").cloned().unwrap_or(Value::Null);
            // 要求 ID → タスクの対応を取るため、処理の前後で実行中・待機中を比べる
            let request_key = id.as_ref().and_then(id_key);
            let before = request_key.is_some().then(|| held_tasks(session));
            match handle(&mut state, session, &mut outbox, root, &method, &params) {
                Ok(Some(outcome)) => {
                    if let Some(id) = id {
                        let result = match outcome {
                            Outcome::Value(value) => value,
                            Outcome::None => Value::Null,
                        };
                        write_result(out, id, result);
                    }
                }
                Ok(None) => {}
                Err(failure) => {
                    if let Some(id) = id {
                        match failure {
                            Failure::UnknownMethod => {
                                write_error(out, id, METHOD_NOT_FOUND, "method not found");
                            }
                            Failure::Op(error) => {
                                let code = if state.initialized {
                                    code_of(&error.class)
                                } else {
                                    NOT_INITIALIZED
                                };
                                write_app_error(out, id, code, &error);
                            }
                        }
                    }
                }
            }
            if let (Some(key), Some(before)) = (request_key, before) {
                let started: Vec<String> = held_tasks(session)
                    .into_iter()
                    .filter(|task| !before.contains(task))
                    .collect();
                if started.is_empty() {
                    state.started.remove(&key);
                } else {
                    state.started.entry(key).or_default().extend(started);
                }
            }
            flush(&mut outbox, out);
            if method == "exit" {
                return;
            }
        }
    }
}

/// 溜まったイベントを通知にする（§19.4.4）。
fn drain(session: &mut Session, out: &mut Outbox, root: &str) {
    for event in session.poll_events() {
        match event.kind() {
            "diagnostics_changed" => {
                out.notify("novnow/diagnosticsChanged", event.to_json());
            }
            "save_state_changed" => {
                out.notify("novnow/saveStateChanged", event.to_json());
            }
            "agent_progress" => {
                out.notify("novnow/agentProgress", event.to_json());
            }
            "materials_changed" => {
                out.notify("novnow/materialsChanged", event.to_json());
            }
            "document_changed" => {
                // 外部変更・リネームは診断の出し直しで伝える。
                if let Some(path) = event
                    .to_json()
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                {
                    methods::publish(session, out, &path, root);
                }
            }
            _ => {}
        }
    }
}

/// 溜まった通知・要求を書き出す。
fn flush(outbox: &mut Outbox, out: &mut dyn Sink) {
    for message in outbox.take() {
        match message {
            Message::Notification { method, params } => {
                let body = json!({ "jsonrpc": "2.0", "method": method, "params": params });
                out.frame(&body.to_string());
            }
            Message::Request { id, method, params } => {
                let body =
                    json!({ "id": id, "jsonrpc": "2.0", "method": method, "params": params });
                out.frame(&body.to_string());
            }
        }
    }
}

/// 捌ききれなかった理由。
pub enum Failure {
    /// 操作が誤りを返した（分類は §19.1.7 の符号へ写す）。
    Op(Error),
    /// そのメソッドを知らない（JSON-RPC 本来の -32601）。
    UnknownMethod,
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Op(error)
    }
}

fn apply_init_locale(session: &mut Session, params: &Value) {
    let raw = params
        .get("initializationOptions")
        .and_then(|opts| opts.get("locale"))
        .and_then(Value::as_str)
        .or_else(|| params.get("locale").and_then(Value::as_str));
    if let Some(tag) = raw {
        if tag == "en" || tag.starts_with("en-") || tag.starts_with("en_") {
            session.set_locale(Locale::En);
        } else if tag == "ja" || tag.starts_with("ja-") || tag.starts_with("ja_") {
            session.set_locale(Locale::Ja);
        }
    }
}

/// 1 件のメソッドを捌く。
///
/// `Ok(None)` は通知（応答しない）。`Ok(Some(_))` は応答する結果。
#[allow(clippy::too_many_arguments)]
fn handle(
    state: &mut ServerState,
    session: &mut Session,
    out: &mut Outbox,
    root: &str,
    method: &str,
    params: &Value,
) -> Result<Option<Outcome>, Failure> {
    if !state.initialized && !matches!(method, "initialize" | "initialized" | "exit") {
        return Err(Failure::Op(Error::usage(session.locale, method)));
    }
    match method {
        "initialize" => {
            apply_init_locale(session, params);
            Ok(Some(Outcome::Value(initialize(session))))
        }
        "initialized" | "exit" | "$/cancelRequest" => Ok(None),
        "shutdown" => Ok(Some(Outcome::None)),
        "textDocument/didOpen" => {
            if let Some(uri) = params
                .get("textDocument")
                .and_then(|doc| doc.get("uri"))
                .and_then(Value::as_str)
            {
                if let Some(path) = methods::path_of(root, uri) {
                    state.open_docs.insert(path);
                }
            }
            methods::did_open(session, out, params, root);
            Ok(None)
        }
        "textDocument/didChange" => {
            methods::did_change(session, out, params, root);
            Ok(None)
        }
        "textDocument/didSave" => {
            methods::did_save(session, out, params, root).map_err(Failure::Op)?;
            Ok(None)
        }
        "textDocument/didClose" => {
            if let Some(uri) = params
                .get("textDocument")
                .and_then(|doc| doc.get("uri"))
                .and_then(Value::as_str)
            {
                if let Some(path) = methods::path_of(root, uri) {
                    state.open_docs.remove(&path);
                }
            }
            Ok(None)
        }
        "textDocument/completion" => Ok(Some(
            methods::completion(session, params, root).map_err(Failure::Op)?,
        )),
        "textDocument/hover" => Ok(Some(
            methods::hover(session, params, root).map_err(Failure::Op)?,
        )),
        "textDocument/definition" => Ok(Some(
            methods::definition(session, params, root).map_err(Failure::Op)?,
        )),
        "textDocument/references" => Ok(Some(
            methods::references(session, params, root).map_err(Failure::Op)?,
        )),
        "textDocument/documentSymbol" => Ok(Some(
            methods::document_symbol(session, params, root).map_err(Failure::Op)?,
        )),
        "textDocument/formatting" => Ok(Some(
            methods::formatting(session, params, root).map_err(Failure::Op)?,
        )),
        "textDocument/rename" => Ok(Some(methods::rename(session, params, root, false)?)),
        "textDocument/prepareRename" => Ok(Some(methods::rename(session, params, root, true)?)),
        "textDocument/foldingRange" => Ok(Some(
            methods::folding(session, params, root).map_err(Failure::Op)?,
        )),
        "textDocument/inlayHint" => Ok(Some(
            methods::inlay_hint(session, params, root).map_err(Failure::Op)?,
        )),
        "textDocument/semanticTokens/full" => Ok(Some(
            methods::semantic_tokens(session, params, root).map_err(Failure::Op)?,
        )),
        "textDocument/codeAction" => Ok(Some(
            methods::code_action(session, params, root).map_err(Failure::Op)?,
        )),
        "workspace/symbol" => Ok(Some(
            methods::workspace_symbol(session, params, root).map_err(Failure::Op)?,
        )),
        "workspace/executeCommand" => {
            let value = commands::execute(session, out, params, root).map_err(Failure::Op)?;
            Ok(Some(Outcome::Value(value)))
        }
        "workspace/didChangeConfiguration" => Ok(Some(
            methods::did_change_configuration(session, out, params, root, &state.open_docs)
                .map_err(Failure::Op)?,
        )),
        _ => Err(Failure::UnknownMethod),
    }
    .inspect(|_outcome| {
        if method == "initialized" {
            state.initialized = true;
        }
    })
}

/// `initialize` 前の要求に返す符号（LSP の `ServerNotInitialized`）。
pub const NOT_INITIALIZED: i64 = -32002;

/// `initialize` の応答（能力の宣言。§19.4.2）。
pub fn initialize(session: &Session) -> Value {
    let locale = session.locale;
    let _ = locale;
    json!({
        "capabilities": {
            "codeActionProvider": { "codeActionKinds": ["quickfix.novnow"], "resolveProvider": false },
            "completionProvider": {
                "resolveProvider": false,
                "triggerCharacters": ["'", "@", ":"],
            },
            "definitionProvider": true,
            "documentFormattingProvider": true,
            "documentSymbolProvider": true,
            "executeCommandProvider": { "commands": commands::names() },
            "foldingRangeProvider": true,
            "hoverProvider": true,
            "inlayHintProvider": { "resolveProvider": false },
            "referencesProvider": true,
            "renameProvider": { "prepareProvider": true },
            "semanticTokensProvider": {
                "full": true,
                "legend": {
                    "tokenModifiers": ["roleLanguage"],
                    "tokenTypes": methods::legend(),
                },
                "range": false,
            },
            "textDocumentSync": 1,
            "workspaceSymbolProvider": true,
        },
        "serverInfo": {
            "name": "novnow",
            "version": env!("CARGO_PKG_VERSION"),
        },
    })
}

/// 誤りの分類を JSON-RPC の符号にする（**§19.1.7 の表**。5 つの口で共通）。
pub fn code_of(class: &Class) -> i64 {
    class.jsonrpc()
}

/// 応答を書き出す。
fn write_result(out: &mut dyn Sink, id: Value, result: Value) {
    let body = json!({ "id": id, "jsonrpc": "2.0", "result": result });
    out.frame(&body.to_string());
}

/// 誤りを書き出す（プロトコル層だけの誤り）。
fn write_error(out: &mut dyn Sink, id: Value, code: i64, message: &str) {
    let body = json!({
        "error": { "code": code, "message": message },
        "id": id,
        "jsonrpc": "2.0",
    });
    out.frame(&body.to_string());
}

/// 分類・S/E 番号・メッセージ・該当パス付きの誤りを書き出す（§19.1.7）。
fn write_app_error(out: &mut dyn Sink, id: Value, code: i64, error: &Error) {
    let body = json!({
        "error": {
            "code": code,
            "data": error.to_json(),
            "message": error.message,
        },
        "id": id,
        "jsonrpc": "2.0",
    });
    out.frame(&body.to_string());
}
