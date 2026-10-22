//! LSP（§19.4）の CI 検査。メモリホストの仮想プロジェクトだけを相手にし、実物のエディタは
//! 起動しない（`tests/api.rs` と同じ流儀）。
//!
//! 見るのは次の 4 つ:
//!
//! 1. 枠（`Content-Length`）の読み書きが往復し、壊れた枠で落ちないこと（§19.4.1）
//! 2. 行・列（UTF-16）と byte 位置の往復（§19.4.1。受け入れ基準 H5-AC-2）
//! 3. §19.4.2 の標準メソッドがどれも**操作カタログへ届き**、LSP の形で返ること
//! 4. §19.4.4 の通知（`novnow/*` の 4 つと `publishDiagnostics`）が出ること

use std::io::Cursor;

use serde_json::{json, Value};

use novnow::adapters::project_load::is_document;
use novnow::app::commands::PALETTE_COMMANDS;
use novnow::app::event::AppEvent;
use novnow::app::memory_host::MemoryHost;
use novnow::app::ops::OPS;
use novnow::app::session::{Interface, Session};
use novnow::core::features::project::line_starts_of;
use novnow::core::model::document::Document;
use novnow::core::policy::ProjectPath;
use novnow::core::template;
use novnow::lsp::methods::uri_of;
use novnow::lsp::{commands, convert, framing, server};

/// プロジェクトルート（URI と相対パスの往復にだけ使う。メモリホストなので実体は無い）。
const ROOT: &str = "/tmp/novel";

/// サンプルの本文（`novnow new` が入れる 1 本目）。
const MANUSCRIPT: &str = "contents/01-01.txt";

/// メモリ上の仮想プロジェクトを LSP として開く（§19.2.1）。
fn open() -> Session {
    let host = MemoryHost::default();
    let documents = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text));
    let mut session = Session::with_documents(Box::new(host), documents.collect());
    session.set_interface(Interface::Lsp);
    session
}

/// 要求の 1 通（枠つき）。
fn request(id: u64, method: &str, params: Value) -> String {
    let body = json!({ "id": id, "jsonrpc": "2.0", "method": method, "params": params });
    framing::encode(&body.to_string())
}

/// 通知の 1 通（枠つき）。
fn notify(method: &str, params: Value) -> String {
    let body = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    framing::encode(&body.to_string())
}

/// 通話を 1 本まるごと流し、出てきたメッセージを順番に返す。
fn talk(session: &mut Session, bodies: &[String]) -> Vec<Value> {
    let mut input = Cursor::new(bodies.concat());
    let mut out = server::Writer(Vec::new());
    server::run(session, ROOT, &mut input, &mut out);
    let mut written = Cursor::new(out.0);
    let mut messages = Vec::new();
    while let Some(body) = framing::read_message(&mut written) {
        messages.push(serde_json::from_str::<Value>(&body).expect("JSON"));
    }
    messages
}

/// `id` の応答の `result`（無ければ落ちる）。
fn result_of(messages: &[Value], id: u64) -> Value {
    messages
        .iter()
        .find(|message| message["id"] == json!(id) && message.get("result").is_some())
        .map(|message| message["result"].clone())
        .unwrap_or_else(|| panic!("id {id} の結果が無い: {messages:?}"))
}

/// `id` の応答の `error`（無ければ落ちる）。
fn error_of(messages: &[Value], id: u64) -> Value {
    messages
        .iter()
        .find(|message| message["id"] == json!(id) && message.get("error").is_some())
        .map(|message| message["error"].clone())
        .unwrap_or_else(|| panic!("id {id} の誤りが無い: {messages:?}"))
}

/// 名前の通知のパラメータを全部。
fn notifications(messages: &[Value], method: &str) -> Vec<Value> {
    messages
        .iter()
        .filter(|message| message["method"] == json!(method))
        .map(|message| message["params"].clone())
        .collect()
}

// =========================================================================== 枠（§19.4.1）

#[test]
fn framing_round_trips_a_message() {
    let body = json!({ "あ": "い" }).to_string();
    let framed = framing::encode(&body);
    assert!(
        framed.starts_with(&format!("Content-Length: {}\r\n\r\n", body.len())),
        "長さは byte 数: {framed:?}"
    );
    let mut input = Cursor::new(framed);
    assert_eq!(
        framing::read_message(&mut input).as_deref(),
        Some(body.as_str())
    );

    // 頭は大文字小文字を問わず、知らない頭は無視する。
    let mut input = Cursor::new(format!(
        "content-length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{body}",
        body.len()
    ));
    assert_eq!(
        framing::read_message(&mut input).as_deref(),
        Some(body.as_str())
    );

    // 続けて 2 通。3 通目は口が閉じている。
    let mut input = Cursor::new(format!("{}{}", framing::encode("1"), framing::encode("22")));
    assert_eq!(framing::read_message(&mut input).as_deref(), Some("1"));
    assert_eq!(framing::read_message(&mut input).as_deref(), Some("22"));
    assert_eq!(framing::read_message(&mut input), None, "正常な切断");

    // 壊れた枠はどれも `None`（切断と同じ扱い）。
    assert_eq!(
        framing::read_message(&mut Cursor::new("\r\n\r\n{}".to_owned())),
        None,
        "長さが無い"
    );
    assert_eq!(
        framing::read_message(&mut Cursor::new("Content-Length: 0\r\n\r\n".to_owned())),
        None,
        "長さが 0"
    );
    assert_eq!(
        framing::read_message(&mut Cursor::new(format!(
            "Content-Length: {}\r\n\r\n{{",
            framing::MAX_MESSAGE + 1
        ))),
        None,
        "上限を超えた枠"
    );
    assert_eq!(
        framing::read_message(&mut Cursor::new("Content-Length: 9\r\n\r\nあ".to_owned())),
        None,
        "本文が足りない"
    );
}

// =========================================================================== 位置（§19.4.1）

#[test]
fn utf16_positions_round_trip() {
    let texts = [
        "あいうえお\nかきくけこ",
        "a🌟b\n😀c",
        "// 注釈の行\n@01-01-01 屋根裏の星図",
        "「これ、どこの空の地図かな」@星野ひかり",
        "",
        "\n\n",
        "漢字(かんじ) と 半角ｶﾅ",
        // 結合文字（§19.4.1）: 合成済みも分解済みも 1 文字ずつ数える
        "e\u{301} = é、か\u{3099} = が",
    ];
    for text in texts {
        let starts = line_starts_of(text);
        for (line, start) in starts.iter().enumerate() {
            let end = starts.get(line + 1).copied().unwrap_or(text.len());
            let line_text = text[*start..end].trim_end_matches('\n');
            let mut units = 0u32;
            for ch in line_text.chars() {
                let at = convert::offset(text, line as u32, units);
                assert_eq!(
                    convert::position(text, at),
                    (line as u32, units),
                    "{text:?} の {line} 行 {units} 列"
                );
                let range = convert::range_json(text, &(at..at + ch.len_utf8()));
                assert_eq!(convert::range_of(&range, text), at..at + ch.len_utf8());
                units += ch.len_utf16() as u32;
            }
            assert_eq!(convert::utf16_len(line_text), units);
        }
        // 範囲外は丸める（落ちない・範囲内）。
        let at = convert::offset(text, 99, 99);
        assert!(at <= text.len(), "{text:?} で {at}");
        assert!(convert::position(text, text.len()).0 < 100);
    }
}

// =========================================================================== 標準メソッド（§19.4.2）

/// 本文の `星図`（解決済みの参照）の位置。
fn word_position(session: &mut Session) -> (String, Value) {
    let text = session
        .run("document.get", json!({ "path": MANUSCRIPT }))
        .expect("document.get")["text"]
        .as_str()
        .expect("text")
        .to_owned();
    let at = text.find("星図が出てきた").expect("参照のある行");
    let (line, character) = convert::position(&text, at);
    (text, json!({ "line": line, "character": character }))
}

#[test]
fn the_standard_methods_answer_from_the_catalog() {
    let mut session = open();
    let (text, at) = word_position(&mut session);
    let uri = uri_of(ROOT, MANUSCRIPT);
    let whole = json!({
        "end": { "character": 0, "line": text.lines().count() as u32 },
        "start": { "character": 0, "line": 0 },
    });
    let bodies = vec![
        request(
            1,
            "initialize",
            json!({ "capabilities": {}, "rootUri": uri_of(ROOT, "") }),
        ),
        notify("initialized", json!({})),
        notify(
            "textDocument/didOpen",
            json!({ "textDocument": { "languageId": "novnow", "text": text, "uri": uri, "version": 1 } }),
        ),
        request(
            2,
            "textDocument/completion",
            json!({ "position": at, "textDocument": { "uri": uri } }),
        ),
        request(
            3,
            "textDocument/hover",
            json!({ "position": at, "textDocument": { "uri": uri } }),
        ),
        request(
            4,
            "textDocument/definition",
            json!({ "position": at, "textDocument": { "uri": uri } }),
        ),
        request(
            5,
            "textDocument/references",
            json!({ "context": { "includeDeclaration": true }, "position": at, "textDocument": { "uri": uri } }),
        ),
        request(
            6,
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri } }),
        ),
        request(7, "workspace/symbol", json!({ "query": "星図" })),
        request(
            8,
            "textDocument/formatting",
            json!({ "options": { "tabSize": 4 }, "textDocument": { "uri": uri } }),
        ),
        request(
            9,
            "textDocument/prepareRename",
            json!({ "position": at, "textDocument": { "uri": uri } }),
        ),
        request(
            10,
            "textDocument/rename",
            json!({ "newName": "星座図", "position": at, "textDocument": { "uri": uri } }),
        ),
        request(
            11,
            "textDocument/foldingRange",
            json!({ "textDocument": { "uri": uri } }),
        ),
        request(
            12,
            "textDocument/inlayHint",
            json!({ "range": whole, "textDocument": { "uri": uri } }),
        ),
        request(
            13,
            "textDocument/semanticTokens/full",
            json!({ "textDocument": { "uri": uri } }),
        ),
        request(
            14,
            "textDocument/codeAction",
            json!({ "context": { "diagnostics": [] }, "range": whole, "textDocument": { "uri": uri } }),
        ),
        request(
            15,
            "workspace/executeCommand",
            json!({ "arguments": [{}], "command": "api.list" }),
        ),
        notify(
            "textDocument/didChange",
            json!({ "contentChanges": [{ "text": text }], "textDocument": { "uri": uri, "version": 2 } }),
        ),
        notify(
            "textDocument/didSave",
            json!({ "textDocument": { "uri": uri } }),
        ),
        notify(
            "workspace/didChangeConfiguration",
            json!({ "settings": {} }),
        ),
        request(16, "shutdown", Value::Null),
        notify("exit", Value::Null),
    ];
    let messages = talk(&mut session, &bodies);

    // どれも誤りでない（§19.4.2 の 11 メソッドが実在の操作へ届いている）。
    for message in &messages {
        assert!(message.get("error").is_none(), "誤りが返った: {message}");
    }

    // 能力の宣言（§19.4.2）。
    let capabilities = result_of(&messages, 1)["capabilities"].clone();
    assert_eq!(
        capabilities["textDocumentSync"],
        json!(1),
        "全文同期（§19.4.3）"
    );
    for key in [
        "codeActionProvider",
        "completionProvider",
        "definitionProvider",
        "documentFormattingProvider",
        "documentSymbolProvider",
        "executeCommandProvider",
        "foldingRangeProvider",
        "hoverProvider",
        "inlayHintProvider",
        "referencesProvider",
        "renameProvider",
        "semanticTokensProvider",
        "workspaceSymbolProvider",
    ] {
        assert_ne!(capabilities[key], Value::Null, "{key} を宣言していない");
    }
    assert_eq!(
        capabilities["executeCommandProvider"]["commands"]
            .as_array()
            .expect("commands")
            .len(),
        OPS.len() + 2,
        "操作の全件 + `novnow.command` + `novnow.applyEdit`（§19.4.5）"
    );
    assert_eq!(
        capabilities["semanticTokensProvider"]["legend"]["tokenTypes"]
            .as_array()
            .expect("tokenTypes")
            .len(),
        21,
        "§6.3 の表の 21 種別"
    );

    // hover は §6.3 の中身を Markdown で返す。
    let hover = result_of(&messages, 3);
    assert_eq!(hover["contents"]["kind"], json!("markdown"));
    assert!(
        hover["contents"]["value"]
            .as_str()
            .expect("value")
            .contains("星図"),
        "用語の中身が出ていない: {hover}"
    );

    // definition は辞書の場所、references は 4 箇所。
    assert!(
        result_of(&messages, 4)["uri"]
            .as_str()
            .expect("uri")
            .contains("words.md"),
        "辞書へ飛んでいない"
    );
    assert_eq!(
        result_of(&messages, 5).as_array().expect("locations").len(),
        4
    );

    // documentSymbol / foldingRange / semanticTokens の形。
    assert!(!result_of(&messages, 6)
        .as_array()
        .expect("symbols")
        .is_empty());
    assert!(result_of(&messages, 11).as_array().is_some());
    let tokens = result_of(&messages, 13);
    let data = tokens["data"].as_array().expect("data");
    assert_eq!(data.len() % 5, 0, "semanticTokens は 5 つ組の並び");
    assert!(!data.is_empty(), "トークンが 1 つも出ていない");

    // completion は形が正しければよい（候補の有無は §6.3 の検査が見る）。
    assert!(result_of(&messages, 2)["items"].as_array().is_some());

    // workspace/symbol はプロジェクト横断の検索。
    assert!(!result_of(&messages, 7)
        .as_array()
        .expect("symbols")
        .is_empty());

    // executeCommand は操作カタログへ届く（§19.4.5）。
    assert_eq!(result_of(&messages, 15)["count"], json!(119));

    // 診断は `didOpen` と `didChange` の後に出る（§19.4.4）。
    let published = notifications(&messages, "textDocument/publishDiagnostics");
    assert!(published.len() >= 2, "診断が出ていない: {messages:?}");
    assert_eq!(published[0]["uri"], json!(uri));
    assert!(published[0]["diagnostics"].as_array().is_some());
}

// =========================================================================== 通知（§19.4.4）

#[test]
fn the_four_novnow_notifications_arrive() {
    let mut session = open();
    session.push(AppEvent::DiagnosticsChanged {
        count: 2,
        path: MANUSCRIPT.to_owned(),
        scope: "file".to_owned(),
    });
    session.push(AppEvent::SaveStateChanged {
        message: String::new(),
        state: "saving".to_owned(),
    });
    session.push(AppEvent::AgentProgress {
        message: "書いている".to_owned(),
        stage: "running".to_owned(),
        task: "T012".to_owned(),
    });
    session.push(AppEvent::MaterialsChanged {
        change: "proposed".to_owned(),
        id: "M0137".to_owned(),
    });
    let messages = talk(
        &mut session,
        &[
            request(1, "initialize", json!({})),
            notify("initialized", json!({})),
            request(2, "shutdown", Value::Null),
            notify("exit", Value::Null),
        ],
    );
    for (method, key, value) in [
        ("novnow/diagnosticsChanged", "count", json!(2)),
        ("novnow/saveStateChanged", "state", json!("saving")),
        ("novnow/agentProgress", "stage", json!("running")),
        ("novnow/materialsChanged", "change", json!("proposed")),
    ] {
        let found = notifications(&messages, method);
        assert_eq!(found.len(), 1, "{method} が出ていない: {messages:?}");
        assert_eq!(found[0][key], value);
    }
}

// =========================================================================== 操作カタログ（§19.4.5）

#[test]
fn every_catalog_operation_is_a_command() {
    let names = commands::names();
    assert_eq!(names.len(), OPS.len() + 2, "操作の全件 + 特別な 2 つ");
    assert!(names.contains(&"lang.hover".to_owned()));
    assert!(names.contains(&"settings.set".to_owned()));
    assert!(names.contains(&commands::COMMAND.to_owned()));
    assert!(names.contains(&commands::APPLY_EDIT.to_owned()));
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "順不同だとエディタの表示がぶれる");
}

#[test]
fn execute_command_reaches_the_catalog_and_can_apply_an_edit() {
    let mut session = open();
    let palette = PALETTE_COMMANDS[0].0;
    let bodies = vec![
        request(1, "initialize", json!({})),
        notify("initialized", json!({})),
        request(
            2,
            "workspace/executeCommand",
            json!({ "arguments": [{}], "command": "api.list" }),
        ),
        request(
            3,
            "workspace/executeCommand",
            json!({ "arguments": [palette], "command": commands::COMMAND }),
        ),
        request(
            4,
            "workspace/executeCommand",
            json!({ "arguments": [{}], "command": "no.such.operation" }),
        ),
        request(
            5,
            "workspace/executeCommand",
            json!({ "arguments": [{ "changes": {} }], "command": commands::APPLY_EDIT }),
        ),
        notify("exit", Value::Null),
    ];
    let messages = talk(&mut session, &bodies);
    assert_eq!(result_of(&messages, 2)["count"], json!(119), "§19.7 の全行");
    assert_eq!(
        result_of(&messages, 3)["command"],
        json!(palette),
        "パレットのコマンド"
    );
    assert_eq!(
        error_of(&messages, 4)["code"],
        json!(-32601),
        "知らないコマンド"
    );
    assert_eq!(result_of(&messages, 5)["applied"], json!(true));
    let edits = notifications(&messages, "workspace/applyEdit");
    assert_eq!(
        edits.len(),
        1,
        "`workspace/applyEdit` が出ていない: {messages:?}"
    );
    assert_eq!(edits[0], json!({ "changes": {} }));
}

#[test]
fn a_request_before_initialize_is_refused() {
    let mut session = open();
    let messages = talk(
        &mut session,
        &[
            request(1, "textDocument/hover", json!({})),
            request(2, "initialize", json!({})),
            notify("exit", Value::Null),
        ],
    );
    assert_eq!(
        error_of(&messages, 1)["code"],
        json!(-32002),
        "`ServerNotInitialized`"
    );
    assert!(result_of(&messages, 2)["capabilities"]["hoverProvider"]
        .as_bool()
        .unwrap_or(false));
}

#[test]
fn an_unknown_method_is_reported_and_the_session_survives() {
    let mut session = open();
    let messages = talk(
        &mut session,
        &[
            request(1, "initialize", json!({})),
            notify("initialized", json!({})),
            request(2, "textDocument/noSuchFeature", json!({})),
            request(
                3,
                "workspace/executeCommand",
                json!({ "arguments": [{}], "command": "project.info" }),
            ),
            notify("exit", Value::Null),
        ],
    );
    assert_eq!(error_of(&messages, 2)["code"], json!(-32601));
    assert!(result_of(&messages, 3).is_object(), "会話は続く");
}

#[test]
fn uris_round_trip_through_the_project_root() {
    let uri = uri_of(ROOT, MANUSCRIPT);
    assert!(uri.starts_with("file:///tmp/novel/"), "{uri}");
    let mut session = open();
    let text = session
        .run("document.get", json!({ "path": MANUSCRIPT }))
        .expect("document.get")["text"]
        .as_str()
        .expect("text")
        .to_owned();
    let _ = &text;
    assert_eq!(
        novnow::lsp::methods::path_of(ROOT, &uri).as_deref(),
        Some(MANUSCRIPT),
        "URI から相対パスへ戻れない: {uri}"
    );
    // ルートの外・`..` は扱わない。
    assert_eq!(
        novnow::lsp::methods::path_of(ROOT, "file:///etc/passwd"),
        None
    );
    assert_eq!(
        novnow::lsp::methods::path_of(ROOT, "file:///tmp/novel/../secret"),
        None
    );
    // 空白と日本語はパーセント符号化をまたいでも戻る。
    let spaced = uri_of(ROOT, "plots/01 下書.txt");
    assert!(spaced.contains("%20"), "{spaced}");
    assert_eq!(
        novnow::lsp::methods::path_of(ROOT, &spaced).as_deref(),
        Some("plots/01 下書.txt")
    );
}
