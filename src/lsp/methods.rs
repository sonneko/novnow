//! 標準メソッド → 操作カタログ（§19.4.2）。
//!
//! ここは翻訳だけをする。位置の計算は [`super::convert`]、実体は §19.7 の操作
//! （`session.run("lang.hover", …)`）。**同じ実装を 5 つの口が共有する**（§19.1.2）ので、
//! ここに言語機能そのものは書かない。

use serde_json::{json, Map, Value};

use crate::app::error::Error;
use crate::app::session::Session;
use crate::core::features::semantic_tokens::TokenType;

use super::convert;

/// 標準メソッドの応答。
pub enum Outcome {
    /// `result` に入れる値。
    Value(Value),
    /// 何も無い（`null` を返す。LSP では「見つからない」を `null` で表す）。
    None,
}

/// サーバからクライアントへ出すメッセージ。
pub enum Message {
    /// 通知（応答を待たない）。
    Notification {
        /// メソッド名。
        method: String,
        /// パラメータ。
        params: Value,
    },
    /// 要求（`workspace/applyEdit`。応答は待たずに結果を送る）。
    Request {
        /// 割り当てた ID。
        id: u64,
        /// メソッド名。
        method: String,
        /// パラメータ。
        params: Value,
    },
}

/// 送出待ちのメッセージ（§19.4.4）。
#[derive(Default)]
pub struct Outbox {
    messages: Vec<Message>,
}

impl Outbox {
    /// 通知を積む。
    pub fn notify(&mut self, method: &str, params: Value) {
        self.messages.push(Message::Notification {
            method: method.to_owned(),
            params,
        });
    }

    /// サーバ → クライアントの要求を積み、割り当てた ID を返す。
    pub fn request(&mut self, method: &str, params: Value) -> u64 {
        let id = self
            .messages
            .iter()
            .filter(|m| matches!(m, Message::Request { .. }))
            .count() as u64
            + 1;
        self.messages.push(Message::Request {
            id,
            method: method.to_owned(),
            params,
        });
        id
    }

    /// 積まれたメッセージを取り出す。
    pub fn take(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.messages)
    }
}

/// 操作を呼び、JSON を受け取る。
pub fn run(session: &mut Session, id: &str, args: Value) -> Result<Value, Error> {
    session.run(id, args)
}

/// 文書のテキスト（作業台にあればそれ。無ければ空）。
pub fn text_of(session: &Session, path: &str) -> String {
    session
        .aggregate()
        .document(path)
        .map(|document| document.text.clone())
        .unwrap_or_default()
}

/// 作業台の文書を置き、自動保存を待つ（§8.2）。エディタの全文同期（§19.4.3）。
pub fn put(session: &mut Session, path: &str, text: String) {
    let now = session.clock;
    session.workbench_mut().replace_document(path, text, now);
}

// ================================================================== URI とパス

/// `file://` URI をプロジェクトルートからの相対パスにする。
///
/// ルートの外なら `None`（プロジェクト外の文書は扱わない）。
pub fn path_of(root: &str, uri: &str) -> Option<String> {
    let rest = uri
        .strip_prefix("file://")
        .or_else(|| uri.strip_prefix("file:"))?;
    let decoded = percent_decode(rest);
    // 絶対パスも相対パスのプロジェクトも、先頭の `/` を落としてから比べると同じ形になる。
    let decoded = decoded.trim_start_matches('/');
    let root = root.trim_start_matches('/').trim_end_matches('/');
    let Some(trimmed) = decoded.strip_prefix(root) else {
        // ルートの外（別のプロジェクトやシステムファイル）は扱わない。
        return None;
    };
    let path = trimmed.trim_start_matches('/');
    if path.is_empty() || path.contains("..") {
        return None;
    }
    Some(path.to_owned())
}

/// プロジェクトルートからの相対パスを `file://` URI にする。
pub fn uri_of(root: &str, path: &str) -> String {
    let root = root.trim_end_matches('/');
    let joined = if path.is_empty() {
        root.to_owned()
    } else {
        format!("{root}/{path}")
    };
    let encoded: String = joined
        .chars()
        .map(|ch| match ch {
            ' ' => "%20".to_owned(),
            '#' => "%23".to_owned(),
            '?' => "%3F".to_owned(),
            '%' => "%25".to_owned(),
            _ => ch.to_string(),
        })
        .collect();
    format!("file://{encoded}")
}

/// `%XX` を戻す（LSP の URI はパーセント符号化）。
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            let hex = &text[at + 1..at + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                at += 3;
                continue;
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ================================================================== 位置の受け渡し

/// `{textDocument:{uri}, position:{line,character}}` からパス・テキスト・byte 位置を取る。
pub fn position_of(
    session: &Session,
    params: &Value,
    root: &str,
) -> Option<(String, String, usize)> {
    let uri = params
        .get("textDocument")
        .and_then(|document| document.get("uri"))
        .and_then(Value::as_str)?;
    let path = path_of(root, uri)?;
    let text = text_of(session, &path);
    let at = params
        .get("position")
        .map(|position| convert::offset_of(position, &text))
        .unwrap_or(0);
    Some((path, text, at))
}

/// `{textDocument:{uri}, range:{…}}` からパス・テキスト・byte 範囲を取る。
pub fn range_of(
    session: &Session,
    params: &Value,
    root: &str,
) -> Option<(String, String, std::ops::Range<usize>)> {
    let uri = params
        .get("textDocument")
        .and_then(|document| document.get("uri"))
        .and_then(Value::as_str)?;
    let path = path_of(root, uri)?;
    let text = text_of(session, &path);
    let range = params
        .get("range")
        .map(|range| convert::range_of(range, &text))
        .unwrap_or(0..0);
    Some((path, text, range))
}

/// `{textDocument:{uri}}` からパスとテキストを取る。
pub fn document_of(session: &Session, params: &Value, root: &str) -> Option<(String, String)> {
    let uri = params
        .get("textDocument")
        .and_then(|document| document.get("uri"))
        .and_then(Value::as_str)?;
    let path = path_of(root, uri)?;
    let text = text_of(session, &path);
    Some((path, text))
}

// ================================================================== 診断

/// 診断の重要度を LSP の番号にする（1=Error, 2=Warning, 3=Info, 4=Hint）。
pub fn severity(name: &str) -> u8 {
    match name {
        "error" => 1,
        "warning" => 2,
        "hint" => 4,
        _ => 3,
    }
}

/// `diagnostics.run` の行を LSP の `Diagnostic` にする。
///
/// 操作側は 1 始まりの行・列（§19.1.7）、LSP は 0 始まり。終わり位置は操作が出さないので
/// 行末にする（エディタが行全体を引く）。
pub fn diagnostics_json(text: &str, value: &Value) -> Vec<Value> {
    let lines: Vec<&str> = text.lines().collect();
    let mut items = Vec::new();
    for row in value
        .get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let line = row
            .get("line")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .saturating_sub(1);
        let column = row
            .get("column")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .saturating_sub(1);
        let end = lines
            .get(line as usize)
            .map(|line| convert::utf16_len(line))
            .unwrap_or(column as u32);
        items.push(json!({
            "code": row.get("code"),
            "message": row.get("message"),
            "range": {
                "start": { "line": line, "character": column },
                "end": { "line": line, "character": end },
            },
            "severity": severity(row.get("severity").and_then(Value::as_str).unwrap_or("info")),
            "source": "novnow",
        }));
    }
    items
}

/// 1 文書の診断を出し直す（`textDocument/publishDiagnostics`。§19.4.4）。
pub fn publish(session: &mut Session, out: &mut Outbox, path: &str, root: &str) {
    let mut args = Map::new();
    args.insert("path".to_owned(), json!(path));
    let Ok(value) = run(session, "diagnostics.run", Value::Object(args)) else {
        return;
    };
    let text = text_of(session, path);
    out.notify(
        "textDocument/publishDiagnostics",
        json!({
            "diagnostics": diagnostics_json(&text, &value),
            "uri": uri_of(root, path),
        }),
    );
}

// ================================================================== 文書の同期

/// `textDocument/didOpen`（§19.4.3）。全文を受け取り、診断を出す。
pub fn did_open(session: &mut Session, out: &mut Outbox, params: &Value, root: &str) {
    let Some(document) = params.get("textDocument") else {
        return;
    };
    let Some(uri) = document.get("uri").and_then(Value::as_str) else {
        return;
    };
    let Some(path) = path_of(root, uri) else {
        return;
    };
    let text = document
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    put(session, &path, text);
    publish(session, out, &path, root);
}

/// `textDocument/didChange`（§19.4.3）。全文同期（`textDocumentSync = 1`）なので、
/// 変化はいつも最後の 1 件が全文。
pub fn did_change(session: &mut Session, out: &mut Outbox, params: &Value, root: &str) {
    let Some(document) = params.get("textDocument") else {
        return;
    };
    let Some(uri) = document.get("uri").and_then(Value::as_str) else {
        return;
    };
    let Some(path) = path_of(root, uri) else {
        return;
    };
    let text = params
        .get("contentChanges")
        .and_then(Value::as_array)
        .and_then(|changes| changes.last())
        .and_then(|change| change.get("text"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    put(session, &path, text);
    publish(session, out, &path, root);
}

/// `textDocument/didSave`（§19.4.3）。保存して診断を出し直す。
pub fn did_save(
    session: &mut Session,
    out: &mut Outbox,
    params: &Value,
    root: &str,
) -> Result<Outcome, Error> {
    let mut args = Map::new();
    args.insert("now".to_owned(), json!(true));
    run(session, "save.now", Value::Object(args))?;
    if let Some((path, _)) = document_of(session, params, root) {
        publish(session, out, &path, root);
    }
    Ok(Outcome::None)
}

// ================================================================== 言語機能

/// `textDocument/completion`（`lang.completion`）。
pub fn completion(session: &mut Session, params: &Value, root: &str) -> Result<Outcome, Error> {
    let Some((path, text, at)) = position_of(session, params, root) else {
        return Ok(Outcome::Value(
            json!({ "isIncomplete": false, "items": [] }),
        ));
    };
    let value = run(
        session,
        "lang.completion",
        json!({ "path": path, "pos": at }),
    )?;
    let items: Vec<Value> = value
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            json!({
                "detail": item.get("detail"),
                "insertText": item.get("insert_text"),
                "kind": completion_kind(item.get("kind").and_then(Value::as_str).unwrap_or("")),
                "label": item.get("label"),
                "sortText": item.get("label"),
                "textEdit": {
                    "newText": item.get("insert_text"),
                    "range": convert::range_of_value(&text, item.get("range")),
                },
            })
        })
        .collect();
    Ok(Outcome::Value(
        json!({ "isIncomplete": false, "items": items }),
    ))
}

/// 補完の種別（§6.3 の名前）を LSP の番号にする。無いものは Text（1）。
pub fn completion_kind(kind: &str) -> u8 {
    match kind {
        "Character" => 5,
        "Word" => 20,
        "Material" => 17,
        "Plot" => 16,
        "Scene" => 12,
        "Mode" => 13,
        "Field" => 4,
        _ => 1,
    }
}

/// `textDocument/hover`（`lang.hover`）。中身は Markdown にして返す。
pub fn hover(session: &mut Session, params: &Value, root: &str) -> Result<Outcome, Error> {
    let Some((path, text, at)) = position_of(session, params, root) else {
        return Ok(Outcome::None);
    };
    let value = run(session, "lang.hover", json!({ "path": path, "pos": at }))?;
    let Some(found) = value.get("hover").filter(|hover| !hover.is_null()) else {
        return Ok(Outcome::None);
    };
    Ok(Outcome::Value(json!({
        "contents": { "kind": "markdown", "value": hover_text(found.get("contents")) },
        "range": convert::range_of_value(&text, found.get("range")),
    })))
}

/// 中身を Markdown にする（§6.3 の hover は構造化されているので、ここで文章にする）。
fn hover_text(contents: Option<&Value>) -> String {
    let Some(contents) = contents else {
        return String::new();
    };
    if let Some(character) = contents.get("Character") {
        return vec![
            format!(
                "**{}**",
                character.get("name").and_then(Value::as_str).unwrap_or("")
            ),
            line_of("読み", character.get("reading")),
            line_of("役割", character.get("role")),
        ]
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    }
    if let Some(word) = contents.get("Word") {
        return vec![
            format!(
                "**{}**",
                word.get("term").and_then(Value::as_str).unwrap_or("")
            ),
            line_of("読み", word.get("reading")),
            line_of("分類", word.get("category")),
            line_of("", word.get("definition")),
        ]
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    }
    serde_json::to_string_pretty(contents).unwrap_or_default()
}

/// `名前: 値` の 1 行（値が無ければ空文字）。
fn line_of(label: &str, value: Option<&Value>) -> String {
    match value.and_then(Value::as_str) {
        Some(text) if !text.is_empty() && !label.is_empty() => format!("{label}: {text}"),
        Some(text) if !text.is_empty() => text.to_owned(),
        _ => String::new(),
    }
}

/// `textDocument/definition`（`lang.definition`）。
pub fn definition(session: &mut Session, params: &Value, root: &str) -> Result<Outcome, Error> {
    let Some((path, _text, at)) = position_of(session, params, root) else {
        return Ok(Outcome::None);
    };
    let value = run(
        session,
        "lang.definition",
        json!({ "path": path, "pos": at }),
    )?;
    let Some(found) = value.get("definition").filter(|found| !found.is_null()) else {
        return Ok(Outcome::None);
    };
    Ok(Outcome::Value(location(session, root, found)))
}

/// `textDocument/references`（`lang.references`）。
pub fn references(session: &mut Session, params: &Value, root: &str) -> Result<Outcome, Error> {
    let Some((path, _text, at)) = position_of(session, params, root) else {
        return Ok(Outcome::Value(Value::Array(Vec::new())));
    };
    let value = run(
        session,
        "lang.references",
        json!({ "path": path, "pos": at }),
    )?;
    let items: Vec<Value> = value
        .get("references")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|found| location(session, root, found))
        .collect();
    Ok(Outcome::Value(Value::Array(items)))
}

/// `{path, range}` を LSP の `Location` にする。
fn location(session: &Session, root: &str, found: &Value) -> Value {
    let path = found
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let text = text_of(session, path);
    json!({
        "range": convert::range_of_value(&text, found.get("range")),
        "uri": uri_of(root, path),
    })
}

/// `textDocument/documentSymbol`（`lang.symbols`。無ければ `section.list`）。
pub fn document_symbol(
    session: &mut Session,
    params: &Value,
    root: &str,
) -> Result<Outcome, Error> {
    let Some((path, text)) = document_of(session, params, root) else {
        return Ok(Outcome::Value(Value::Array(Vec::new())));
    };
    let value = run(session, "lang.symbols", json!({ "path": path.clone() }))?;
    let symbols = value
        .get("symbols")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !symbols.is_empty() {
        return Ok(Outcome::Value(Value::Array(
            symbols
                .iter()
                .map(|symbol| symbol_json(&text, symbol))
                .collect(),
        )));
    }
    // 本文以外は節の一覧で代用する（§19.7 では両方が `documentSymbol` を担う）。
    let sections = run(session, "section.list", json!({ "path": path }))?;
    let items: Vec<Value> = sections
        .get("sections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|section| {
            let lines = section.get("lines").and_then(Value::as_array);
            let start = lines
                .and_then(|lines| lines.first())
                .and_then(Value::as_u64)
                .unwrap_or(1);
            let end = lines
                .and_then(|lines| lines.last())
                .and_then(Value::as_u64)
                .unwrap_or(start);
            let range = line_range(&text, start as usize, end as usize);
            json!({
                "children": [],
                "detail": section.get("summary"),
                "kind": 12,
                "name": section.get("id"),
                "range": range,
                "selectionRange": range,
            })
        })
        .collect();
    Ok(Outcome::Value(Value::Array(items)))
}

/// `lang.symbols` の 1 件を LSP の `DocumentSymbol` にする（形はほぼ同じ。位置だけ直す）。
fn symbol_json(text: &str, symbol: &Value) -> Value {
    let children: Vec<Value> = symbol
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|child| symbol_json(text, child))
        .collect();
    json!({
        "children": children,
        "detail": symbol.get("detail"),
        "kind": symbol_kind(symbol.get("kind").and_then(Value::as_str).unwrap_or("")),
        "name": symbol.get("name"),
        "range": convert::range_of_value(text, symbol.get("range")),
        "selectionRange": convert::range_of_value(text, symbol.get("selection_range")),
    })
}

/// 記号の種別を LSP の `SymbolKind` にする。
pub fn symbol_kind(kind: &str) -> u8 {
    match kind {
        "File" => 1,
        "Module" => 2,
        "Namespace" => 3,
        "Package" => 4,
        "Class" => 5,
        "Method" => 6,
        "Property" => 7,
        "Field" => 8,
        "Constructor" => 9,
        "Enum" => 10,
        "Interface" => 11,
        "Function" => 12,
        "Variable" => 13,
        "Constant" => 14,
        "String" => 15,
        "Number" => 16,
        "Boolean" => 17,
        "Array" => 18,
        "Object" => 19,
        "Key" => 20,
        "Null" => 21,
        "EnumMember" => 22,
        "Struct" => 23,
        "Event" => 24,
        "Operator" => 25,
        "TypeParameter" => 26,
        _ => 13,
    }
}

/// 行の範囲（1 始まり・両端を含む）を LSP の `Range` にする。
fn line_range(text: &str, first: usize, last: usize) -> Value {
    let lines: Vec<&str> = text.lines().collect();
    let start = first.saturating_sub(1).min(lines.len().saturating_sub(1));
    let last = last.saturating_sub(1).min(lines.len().saturating_sub(1));
    json!({
        "start": { "line": start, "character": 0 },
        "end": { "line": last, "character": convert::utf16_len(lines.get(last).copied().unwrap_or("")) },
    })
}

/// `workspace/symbol`（`search.project`）。
pub fn workspace_symbol(
    session: &mut Session,
    params: &Value,
    root: &str,
) -> Result<Outcome, Error> {
    let query = params
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if query.is_empty() {
        return Ok(Outcome::Value(Value::Array(Vec::new())));
    }
    let value = run(session, "search.project", json!({ "query": query }))?;
    let items: Vec<Value> = value
        .get("hits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|hit| {
            let path = hit.get("path").and_then(Value::as_str).unwrap_or_default();
            let text = text_of(session, path);
            let line = hit.get("line").and_then(Value::as_u64).unwrap_or(1) as usize;
            let range = line_range(&text, line, line);
            json!({
                "containerName": path,
                "kind": 13,
                "location": { "range": range, "uri": uri_of(root, path) },
                "name": hit.get("text"),
            })
        })
        .collect();
    Ok(Outcome::Value(Value::Array(items)))
}

/// `textDocument/formatting`（`lang.format`）。
pub fn formatting(session: &mut Session, params: &Value, root: &str) -> Result<Outcome, Error> {
    let Some((path, text)) = document_of(session, params, root) else {
        return Ok(Outcome::Value(Value::Array(Vec::new())));
    };
    let value = run(session, "lang.format", json!({ "path": path }))?;
    Ok(Outcome::Value(Value::Array(edits_json(&text, &value))))
}

/// `lang.format` / `lang.code_action` の編集を LSP の `TextEdit` にする。
pub fn edits_json(text: &str, value: &Value) -> Vec<Value> {
    value
        .get("edits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|edit| {
            json!({
                "newText": edit.get("new_text"),
                "range": convert::range_of_value(text, edit.get("range")),
            })
        })
        .collect()
}

/// `textDocument/rename`（`lang.rename_preview` → `lang.rename_apply`）。
///
/// `prepare` は `prepareProvider` の下調べ（変えられるかだけ）。
pub fn rename(
    session: &mut Session,
    params: &Value,
    root: &str,
    prepare: bool,
) -> Result<Outcome, Error> {
    let Some((path, text, at)) = position_of(session, params, root) else {
        return Ok(Outcome::None);
    };
    let to = params
        .get("newName")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if prepare {
        let value = run(
            session,
            "lang.rename_preview",
            json!({ "path": path, "pos": at }),
        )?;
        return Ok(Outcome::Value(json!({
            "placeholder": value.get("from"),
            "range": convert::range_of_value(&text, value.get("range")),
        })));
    }
    let value = run(
        session,
        "lang.rename_apply",
        json!({ "path": path, "pos": at, "to": to }),
    )?;
    Ok(Outcome::Value(workspace_edit(session, root, &value)))
}

/// `WorkspaceEdit`（`{changes:[{path,edits}]}`）を LSP の `{changes:{uri:[TextEdit]}}` にする。
pub fn workspace_edit(session: &Session, root: &str, value: &Value) -> Value {
    let mut changes = Map::new();
    for file in value
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let path = file.get("path").and_then(Value::as_str).unwrap_or_default();
        let text = text_of(session, path);
        let edits = edits_json(&text, file);
        changes.insert(uri_of(root, path), Value::Array(edits));
    }
    json!({ "changes": changes })
}

/// `textDocument/foldingRange`（`lang.folding`）。
///
/// §19.7 では `plot.outline_fold` も `foldingRange` を担うが、それは章 ID を引数に取る
/// 別の形なので、ここでは `workspace/executeCommand`（§19.4.5）から叩く。
pub fn folding(session: &mut Session, params: &Value, root: &str) -> Result<Outcome, Error> {
    let Some((path, text)) = document_of(session, params, root) else {
        return Ok(Outcome::Value(Value::Array(Vec::new())));
    };
    let value = run(session, "lang.folding", json!({ "path": path }))?;
    let items: Vec<Value> = value
        .get("ranges")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|range| folding_json(&text, range))
        .collect();
    Ok(Outcome::Value(Value::Array(items)))
}

/// `{range, depth}` を LSP の `FoldingRange` にする。
fn folding_json(text: &str, range: &Value) -> Value {
    let at = convert::range_of_value(text, range.get("range"));
    let start = at
        .get("start")
        .cloned()
        .unwrap_or(json!({ "line": 0, "character": 0 }));
    let end = at
        .get("end")
        .cloned()
        .unwrap_or(json!({ "line": 0, "character": 0 }));
    json!({
        "endCharacter": end.get("character"),
        "endLine": end.get("line"),
        "kind": "region",
        "startCharacter": start.get("character"),
        "startLine": start.get("line"),
    })
}

/// `textDocument/inlayHint`（`lang.hints`）。
pub fn inlay_hint(session: &mut Session, params: &Value, root: &str) -> Result<Outcome, Error> {
    let Some((path, text, range)) = range_of(session, params, root) else {
        return Ok(Outcome::Value(Value::Array(Vec::new())));
    };
    let value = run(
        session,
        "lang.hints",
        json!({ "end": range.end, "path": path, "start": range.start }),
    )?;
    let items: Vec<Value> = value
        .get("hints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|hint| {
            let at = hint.get("at").and_then(Value::as_u64).unwrap_or(0) as usize;
            json!({
                "kind": inlay_kind(hint.get("kind")),
                "label": inlay_label(hint.get("kind")),
                "paddingLeft": true,
                "position": convert::position_json(&text, at),
            })
        })
        .collect();
    Ok(Outcome::Value(Value::Array(items)))
}

/// インレイヒントの種別を LSP の番号にする（1=Type, 2=Parameter）。
fn inlay_kind(kind: Option<&Value>) -> u8 {
    if kind.and_then(|kind| kind.get("Parameter")).is_some() {
        2
    } else {
        1
    }
}

/// インレイヒントの表示文字列。
fn inlay_label(kind: Option<&Value>) -> String {
    let Some(found) = kind
        .and_then(Value::as_object)
        .and_then(|kind| kind.iter().next())
    else {
        return String::new();
    };
    let (name, value) = found;
    let text = value
        .as_str()
        .map(str::to_owned)
        .or_else(|| serde_json::to_string(value).ok())
        .unwrap_or_default();
    format!("{name}: {text}")
}

/// `textDocument/codeAction`（`lang.code_action`）。
pub fn code_action(session: &mut Session, params: &Value, root: &str) -> Result<Outcome, Error> {
    let Some((path, _text, range)) = range_of(session, params, root) else {
        return Ok(Outcome::Value(Value::Array(Vec::new())));
    };
    let value = run(
        session,
        "lang.code_action",
        json!({ "end": range.end, "path": path, "start": range.start }),
    )?;
    let items: Vec<Value> = value
        .get("actions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|action| {
            json!({
                "command": {
                    "arguments": [json!(action.get("arg"))],
                    "command": "novnow.lang.code_action",
                    "title": action.get("kind"),
                },
                "edit": workspace_edit(session, root, action.get("edit").unwrap_or(&Value::Null)),
                "isPreferred": action.get("preferred"),
                "kind": "quickfix.novnow",
                "title": action.get("kind"),
            })
        })
        .collect();
    Ok(Outcome::Value(Value::Array(items)))
}

/// `textDocument/semanticTokens/full`（`lang.tokens`）。
///
/// LSP は差分の列（相対行・相対列・長さ・種別・修飾）で受け取る。1 行を超えるトークンは
/// 規格に合わないので出さない。
pub fn semantic_tokens(
    session: &mut Session,
    params: &Value,
    root: &str,
) -> Result<Outcome, Error> {
    let Some((path, text)) = document_of(session, params, root) else {
        return Ok(Outcome::Value(json!({ "data": [] })));
    };
    let value = run(session, "lang.tokens", json!({ "path": path }))?;
    let mut data: Vec<u32> = Vec::new();
    let mut line = 0u32;
    let mut character = 0u32;
    for token in value
        .get("tokens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(kind) = token.get("kind").and_then(Value::as_str) else {
            continue;
        };
        let Some(index) = token_index(kind) else {
            continue;
        };
        let at = convert::range_of_value(&text, token.get("range"));
        let (start_line, start_character) =
            position_pair(&at.get("start").cloned().unwrap_or_default());
        let (end_line, end_character) = position_pair(&at.get("end").cloned().unwrap_or_default());
        if start_line != end_line {
            // 行をまたぐトークンは LSP の符号化に載らないので出さない。
            continue;
        }
        let length = end_character.saturating_sub(start_character);
        if length == 0 {
            continue;
        }
        let modifier = if token
            .get("role_language")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            1
        } else {
            0
        };
        data.push(start_line.saturating_sub(line));
        if start_line == line {
            data.push(start_character.saturating_sub(character));
        } else {
            data.push(start_character);
        }
        data.push(length);
        data.push(index as u32);
        data.push(modifier);
        line = start_line;
        character = end_character;
    }
    Ok(Outcome::Value(json!({ "data": data })))
}

/// `semanticTokens` の legend（§6.3 の表の名前。`TokenType::as_str`）。
pub fn legend() -> Vec<String> {
    TokenType::ALL
        .iter()
        .map(|kind| kind.as_str().to_owned())
        .collect()
}

/// JSON の種別名（`"Comment"`）を legend の添え字にする。
fn token_index(kind: &str) -> Option<usize> {
    TokenType::ALL
        .iter()
        .position(|each| format!("{each:?}") == kind)
}

/// LSP の `Position` を（行, 列）にする。
fn position_pair(value: &Value) -> (u32, u32) {
    (
        value.get("line").and_then(Value::as_u64).unwrap_or(0) as u32,
        value.get("character").and_then(Value::as_u64).unwrap_or(0) as u32,
    )
}

/// `workspace/didChangeConfiguration`（`settings.get`）。
pub fn did_change_configuration(session: &mut Session) -> Result<Outcome, Error> {
    let value = run(session, "settings.get", json!({}))?;
    Ok(Outcome::Value(json!({ "settings": value })))
}
