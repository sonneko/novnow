//! 受け付け・振り分け・通知の送出（§19.4.1）。
//!
//! 標準入出力で JSON-RPC 2.0 を回す。1 メッセージ = 1 リクエスト/通知。応答を待たない
//! 通知（§19.4.4 の `novnow/*`）は、リクエストの合間に溜まったイベントから作る。

use std::io::BufRead;

use serde_json::{json, Value};

use crate::app::error::{Class, Error};
use crate::app::session::Session;

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

/// サーバを 1 本動かす。エディタが口を閉じるまで回る。
///
/// `root` はプロジェクトルート（URI と相対パスの往復に使う）。
pub fn run(session: &mut Session, root: &str, input: &mut dyn BufRead, out: &mut dyn Sink) {
    let mut initialized = false;
    let mut outbox = Outbox::default();
    while let Some(body) = framing::read_message(input) {
        drain(session, &mut outbox, root);
        let Ok(value) = serde_json::from_str::<Value>(&body) else {
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
        let method = method.to_owned();
        let params = value.get("params").cloned().unwrap_or(Value::Null);
        match handle(
            &mut initialized,
            session,
            &mut outbox,
            root,
            &method,
            &params,
        ) {
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
            Err(error) => {
                if let Some(id) = id {
                    let code = if initialized {
                        code_of(&error.class)
                    } else {
                        NOT_INITIALIZED
                    };
                    write_error(out, id, code, &error.message);
                }
            }
        }
        flush(&mut outbox, out);
        if method == "exit" {
            break;
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

/// 1 件のメソッドを捌く。
///
/// `Ok(None)` は通知（応答しない）。`Ok(Some(_))` は応答する結果。
#[allow(clippy::too_many_arguments)]
fn handle(
    initialized: &mut bool,
    session: &mut Session,
    out: &mut Outbox,
    root: &str,
    method: &str,
    params: &Value,
) -> Result<Option<Outcome>, Error> {
    if !*initialized && !matches!(method, "initialize" | "initialized" | "exit") {
        return Err(Error::usage(session.locale, method));
    }
    match method {
        "initialize" => Ok(Some(Outcome::Value(initialize(session)))),
        "initialized" | "exit" | "$/cancelRequest" => Ok(None),
        "shutdown" => Ok(Some(Outcome::None)),
        "textDocument/didOpen" => {
            methods::did_open(session, out, params, root);
            Ok(None)
        }
        "textDocument/didChange" => {
            methods::did_change(session, out, params, root);
            Ok(None)
        }
        "textDocument/didSave" => {
            methods::did_save(session, out, params, root)?;
            Ok(None)
        }
        "textDocument/didClose" => Ok(None),
        "textDocument/completion" => Ok(Some(methods::completion(session, params, root)?)),
        "textDocument/hover" => Ok(Some(methods::hover(session, params, root)?)),
        "textDocument/definition" => Ok(Some(methods::definition(session, params, root)?)),
        "textDocument/references" => Ok(Some(methods::references(session, params, root)?)),
        "textDocument/documentSymbol" => Ok(Some(methods::document_symbol(session, params, root)?)),
        "textDocument/formatting" => Ok(Some(methods::formatting(session, params, root)?)),
        "textDocument/rename" => Ok(Some(methods::rename(session, params, root, false)?)),
        "textDocument/prepareRename" => Ok(Some(methods::rename(session, params, root, true)?)),
        "textDocument/foldingRange" => Ok(Some(methods::folding(session, params, root)?)),
        "textDocument/inlayHint" => Ok(Some(methods::inlay_hint(session, params, root)?)),
        "textDocument/semanticTokens/full" => {
            Ok(Some(methods::semantic_tokens(session, params, root)?))
        }
        "textDocument/codeAction" => Ok(Some(methods::code_action(session, params, root)?)),
        "workspace/symbol" => Ok(Some(methods::workspace_symbol(session, params, root)?)),
        "workspace/executeCommand" => {
            let value = commands::execute(session, out, params, root)?;
            Ok(Some(Outcome::Value(value)))
        }
        "workspace/didChangeConfiguration" => Ok(Some(methods::did_change_configuration(session)?)),
        _ => Err(Error::not_found(session.locale, method, None)),
    }
    .inspect(|_outcome| {
        if method == "initialized" {
            *initialized = true;
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
                "triggerCharacters": ["'", "@", ";", ":"],
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

/// 誤りの分類を JSON-RPC の符号にする（§19.1.4）。
pub fn code_of(class: &Class) -> i64 {
    match class {
        Class::Usage | Class::Project | Class::Conflict => INVALID_PARAMS,
        Class::NotFound => METHOD_NOT_FOUND,
        Class::Timeout => -32003,
        Class::Cancelled => -32800,
        Class::Io | Class::Validation | Class::Policy => INTERNAL_ERROR,
    }
}

/// 応答を書き出す。
fn write_result(out: &mut dyn Sink, id: Value, result: Value) {
    let body = json!({ "id": id, "jsonrpc": "2.0", "result": result });
    out.frame(&body.to_string());
}

/// 誤りを書き出す。
fn write_error(out: &mut dyn Sink, id: Value, code: i64, message: &str) {
    let body = json!({
        "error": { "code": code, "message": message },
        "id": id,
        "jsonrpc": "2.0",
    });
    out.frame(&body.to_string());
}
