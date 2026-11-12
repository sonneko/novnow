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
const MANUSCRIPT: &str = "contents/01-01.cnov";

/// メモリ上の仮想プロジェクトを LSP として開く（§19.2.1）。
fn open() -> Session {
    let host = MemoryHost::default();
    let documents = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
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
        // 当たらない検索語では何も出ないこと（検索が実際に絞っているかの確認）
        request(
            16,
            "workspace/symbol",
            json!({ "query": "zzz-存在しない語" }),
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
    // 形だけでなく**中身**も見る: LSP の SymbolInformation（§19.4.2）として必要な
    // フィールドが揃い、範囲が入れ子になり、名前が本文の見出しと一致すること。
    let symbols = result_of(&messages, 6).as_array().expect("symbols").clone();
    assert!(!symbols.is_empty(), "documentSymbol が 1 件も出ていない");
    fn line_of(value: &serde_json::Value, edge: &str) -> u64 {
        value[edge]["line"]
            .as_u64()
            .unwrap_or_else(|| panic!("{edge}.line が無い: {value}"))
    }
    fn check_range(value: &serde_json::Value, what: &str) {
        for edge in ["start", "end"] {
            let point = &value[edge];
            assert!(
                point["line"].as_u64().is_some() && point["character"].as_u64().is_some(),
                "{what}.{edge} が LSP の Position ではない: {value}"
            );
        }
        assert!(
            line_of(value, "start") <= line_of(value, "end"),
            "{what} の範囲が逆: {value}"
        );
    }
    fn check_symbol(symbol: &serde_json::Value) {
        let name = symbol["name"]
            .as_str()
            .unwrap_or_else(|| panic!("name が無い: {symbol}"));
        assert!(!name.trim().is_empty(), "名前が空: {symbol}");
        let kind = symbol["kind"]
            .as_u64()
            .unwrap_or_else(|| panic!("{name}: SymbolKind（数）が無い: {symbol}"));
        assert!(
            (1..=26).contains(&kind),
            "{name}: SymbolKind が LSP の範囲外: {kind}"
        );
        check_range(&symbol["range"], name);
        check_range(&symbol["selectionRange"], name);
        // selectionRange は range の中に入る（LSP の不変条件）
        assert!(
            line_of(&symbol["selectionRange"], "start") >= line_of(&symbol["range"], "start")
                && line_of(&symbol["selectionRange"], "end") <= line_of(&symbol["range"], "end"),
            "{name}: selectionRange が range の外: {symbol}"
        );
        if let Some(children) = symbol["children"].as_array() {
            for child in children {
                check_symbol(child);
            }
        }
    }
    let mut names: Vec<String> = Vec::new();
    fn collect(symbol: &serde_json::Value, into: &mut Vec<String>) {
        into.push(symbol["name"].as_str().expect("name").to_owned());
        if let Some(children) = symbol["children"].as_array() {
            for child in children {
                collect(child, into);
            }
        }
    }
    for symbol in &symbols {
        check_symbol(symbol);
        collect(symbol, &mut names);
    }
    assert!(
        names.iter().any(|name| !name.is_empty()),
        "見出しの名前が 1 つも取れていない: {names:?}"
    );
    assert_eq!(
        names.len(),
        names
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        "同じ見出しが二重に出ている: {names:?}"
    );
    assert!(result_of(&messages, 11).as_array().is_some());
    let tokens = result_of(&messages, 13);
    let data = tokens["data"].as_array().expect("data");
    assert_eq!(data.len() % 5, 0, "semanticTokens は 5 つ組の並び");
    assert!(!data.is_empty(), "トークンが 1 つも出ていない");

    // completion は形が正しければよい（候補の有無は §6.3 の検査が見る）。
    assert!(result_of(&messages, 2)["items"].as_array().is_some());

    // workspace/symbol はプロジェクト横断の検索。**中身**も見る:
    // 検索語（`星図`）を含む名前だけが出て、場所が実在の文書を指すこと。
    let found = result_of(&messages, 7).as_array().expect("symbols").clone();
    assert!(!found.is_empty(), "workspace/symbol が 1 件も出ていない");
    for symbol in &found {
        let name = symbol["name"]
            .as_str()
            .unwrap_or_else(|| panic!("name が無い: {symbol}"));
        assert!(
            name.contains("星図"),
            "検索語「星図」を含まない名前が混ざっている: {name}"
        );
        assert!(
            symbol["kind"].as_u64().is_some(),
            "{name}: SymbolKind が無い: {symbol}"
        );
        let location = &symbol["location"];
        let location_uri = location["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: location.uri が無い: {symbol}"));
        assert!(
            location_uri.starts_with("file://"),
            "{name}: file URI ではない: {location_uri}"
        );
        assert!(
            location_uri.ends_with(".cnov") || location_uri.ends_with(".md"),
            "{name}: 文書ではない場所を指している: {location_uri}"
        );
        check_range(&location["range"], name);
        // どの文書で見つかったかも返す（LSP の SymbolInformation.containerName）
        let container = symbol["containerName"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: containerName が無い: {symbol}"));
        assert!(
            !container.is_empty() && !container.starts_with('/'),
            "{name}: containerName がプロジェクト相対パスではない: {container}"
        );
        assert!(
            location_uri.contains(container),
            "{name}: containerName（{container}）と location.uri（{location_uri}）が食い違う"
        );
    }

    // 当たらない検索語は 0 件（`found` が「全部返しているだけ」でないことの確認）
    assert_eq!(
        result_of(&messages, 16).as_array().expect("symbols").len(),
        0,
        "検索語を無視して全部返している"
    );

    // executeCommand は操作カタログへ届く（§19.4.5）。
    assert_eq!(result_of(&messages, 15)["count"], json!(121));

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
    assert_eq!(result_of(&messages, 2)["count"], json!(121), "§19.7 の全行");
    assert_eq!(
        result_of(&messages, 3)["command"],
        json!(palette),
        "パレットのコマンド"
    );
    assert_eq!(
        error_of(&messages, 4)["code"],
        json!(-32001),
        "知らないコマンドは not_found（§19.1.7）"
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

#[test]
fn semantic_tokens_preserve_reference_boundaries_and_utf16_positions() {
    let mut session = open();
    let text =
        "@01-01 😀見出し\r\n😀'星図が出てきた。'{星図}を読む。\r\n「'星図を見る」 // '星図\r\n";
    let uri = uri_of(ROOT, MANUSCRIPT);
    let messages = talk(
        &mut session,
        &[
            request(1, "initialize", json!({"capabilities": {}})),
            notify("initialized", json!({})),
            notify(
                "textDocument/didOpen",
                json!({"textDocument": {
                    "uri": uri, "languageId": "novnow", "version": 1, "text": text
                }}),
            ),
            request(
                2,
                "textDocument/semanticTokens/full",
                json!({"textDocument": {"uri": uri}}),
            ),
        ],
    );
    let expected = session
        .run("lang.tokens", json!({"path": MANUSCRIPT}))
        .unwrap();
    let result = result_of(&messages, 2);
    let data = result["data"].as_array().unwrap();
    let legend = result_of(&messages, 1)["capabilities"]["semanticTokensProvider"]["legend"]
        ["tokenTypes"]
        .clone();
    assert_eq!(data.len(), expected["tokens"].as_array().unwrap().len() * 5);
    let (mut line, mut column) = (0u32, 0u32);
    let mut references = Vec::new();
    for (encoded, token) in data
        .as_chunks::<5>()
        .0
        .iter()
        .zip(expected["tokens"].as_array().unwrap())
    {
        let delta_line = encoded[0].as_u64().unwrap() as u32;
        line += delta_line;
        column = if delta_line == 0 { column } else { 0 } + encoded[1].as_u64().unwrap() as u32;
        let range = convert::range_of_value(text, token.get("range"));
        assert_eq!(json!({"line": line, "character": column}), range["start"]);
        assert_eq!(
            column as u64 + encoded[2].as_u64().unwrap(),
            range["end"]["character"].as_u64().unwrap()
        );
        let kind = legend[encoded[3].as_u64().unwrap() as usize]
            .as_str()
            .unwrap();
        assert!(
            !kind.contains('.'),
            "VSCode treats dots as modifier separators"
        );
        if kind == "ref-word" {
            references.push((line, column, encoded[2].as_u64().unwrap()));
        }
    }
    // The following prose is not part of the reference. The emoji occupies two UTF-16 units.
    assert!(references.contains(&(1, 3, 2)), "{references:?}");
    assert!(references.contains(&(1, 13, 2)), "{references:?}");
    assert!(references.contains(&(2, 2, 2)), "{references:?}");
}

#[test]
fn windows_file_uris_resolve_to_project_documents() {
    use novnow::lsp::methods::{path_of, semantic_tokens, Outcome};
    let uri = "file:///d%3A/novel/contents/01-01.cnov";
    for root in [r"D:\novel", r"d:\novel\", r"\\?\D:\novel", "D:/novel"] {
        assert_eq!(path_of(root, uri).as_deref(), Some(MANUSCRIPT));
        assert_eq!(
            uri_of(root, MANUSCRIPT),
            "file:///d:/novel/contents/01-01.cnov"
        );
        let mut session = open();
        let Outcome::Value(tokens) =
            semantic_tokens(&mut session, &json!({"textDocument": {"uri": uri}}), root).unwrap()
        else {
            panic!("expected tokens")
        };
        assert!(!tokens["data"].as_array().unwrap().is_empty(), "{root}");
    }
    for root in [r"\\server\share\novel", r"\\?\UNC\server\share\novel"] {
        let uri = "file://server/share/novel/contents/01-01.cnov";
        assert_eq!(uri_of(root, MANUSCRIPT), uri);
        assert_eq!(path_of(root, uri).as_deref(), Some(MANUSCRIPT));
    }
    for uri in [
        "file:///d:/novel-other/contents/01-01.cnov",
        "file:///e:/novel/contents/01-01.cnov",
        "file:///d:/novel/%2e%2e/secret",
    ] {
        assert_eq!(path_of(r"D:\novel", uri), None);
    }
    assert_eq!(path_of(ROOT, "file:///tmp/novel-other/file.cnov"), None);
    assert_eq!(path_of(ROOT, "file:///tmp/Novel/file.cnov"), None);
}

// =========================================================================== Issue #55 回帰（Items 14 / 22 / 23）

fn apply_lsp_workspace_edit(
    before: &std::collections::BTreeMap<String, String>,
    ws_edit: &Value,
    root: &str,
) -> std::collections::BTreeMap<String, String> {
    let mut out = before.clone();
    let Some(changes) = ws_edit.get("changes").and_then(Value::as_object) else {
        return out;
    };
    for (uri, raw_edits) in changes {
        let path = novnow::lsp::methods::path_of(root, uri).expect("workspace_edit URI");
        let original = before
            .get(&path)
            .unwrap_or_else(|| panic!("改名前テキストに {path} が無い"));
        let mut pairs: Vec<(std::ops::Range<usize>, String)> = raw_edits
            .as_array()
            .expect("TextEdit[]")
            .iter()
            .map(|edit| {
                let range = convert::range_of(&edit["range"], original);
                let new_text = edit["newText"].as_str().expect("newText").to_owned();
                (range, new_text)
            })
            .collect();
        pairs.sort_by_key(|a| std::cmp::Reverse(a.0.start));
        let mut next = original.clone();
        for (range, new_text) in pairs {
            next.replace_range(range, &new_text);
        }
        out.insert(path, next);
    }
    out
}

fn multi_file_surrogate_docs() -> Vec<Document> {
    let mut docs: Vec<Document> = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text))
        .collect();
    for doc in &mut docs {
        if doc.path == MANUSCRIPT {
            doc.text =
                "@01-01-01 😀始まり\n😀'星図を広げて、もう一度'{星図}を確かめた。\n".to_owned();
        }
    }
    docs.push(Document::new(
        "contents/01-02.cnov",
        "@01-02-01 😀続き\n港でも😀'星図が頼りだった。\n",
    ));
    docs
}

#[test]
fn prepare_rename_and_rename_match_canonical_operations_across_utf16_surrogates_and_multiple_files()
{
    use std::collections::BTreeMap;

    let docs = multi_file_surrogate_docs();
    let manuscript_text = docs
        .iter()
        .find(|d| d.path == MANUSCRIPT)
        .unwrap()
        .text
        .clone();
    // 2行目 `😀'星図` の `星` のバイト位置と UTF-16 位置
    let byte_pos = manuscript_text.find("'星図").expect("参照がある") + 1;
    let utf16_pos = convert::position_json(&manuscript_text, byte_pos);
    // 改名不能な地の文（`を広げて` の `広`）の位置
    let prose_pos = convert::position_json(
        &manuscript_text,
        manuscript_text.find("広げて").expect("地の文"),
    );

    // 1) Rust 正規操作（`lang.rename_preview` + `lang.rename_apply`）
    let mut rust_session = Session::with_documents(Box::new(MemoryHost::default()), docs.clone());
    rust_session.set_interface(Interface::Rust);
    let preview = rust_session
        .run(
            "lang.rename_preview",
            json!({ "path": MANUSCRIPT, "pos": byte_pos, "to": "星座図" }),
        )
        .expect("rename_preview");
    assert_eq!(preview["prepared"]["placeholder"], json!("星図"));
    let expected_utf16_range =
        convert::range_of_value(&manuscript_text, preview["prepared"].get("range"));

    let before_map: BTreeMap<String, String> = rust_session
        .aggregate()
        .documents()
        .map(|d| (d.path.clone(), d.text.clone()))
        .collect();

    rust_session
        .run(
            "lang.rename_apply",
            json!({ "path": MANUSCRIPT, "pos": byte_pos, "to": "星座図" }),
        )
        .expect("rename_apply");
    let rust_after: BTreeMap<String, String> = rust_session
        .aggregate()
        .documents()
        .map(|d| (d.path.clone(), d.text.clone()))
        .collect();

    // 2) LSP `textDocument/prepareRename` と `textDocument/rename`
    let mut lsp_session = Session::with_documents(Box::new(MemoryHost::default()), docs.clone());
    lsp_session.set_interface(Interface::Lsp);
    let uri = uri_of(ROOT, MANUSCRIPT);
    let messages = talk(
        &mut lsp_session,
        &[
            request(1, "initialize", json!({})),
            notify("initialized", json!({})),
            request(
                2,
                "textDocument/prepareRename",
                json!({ "position": prose_pos, "textDocument": { "uri": uri } }),
            ),
            request(
                3,
                "textDocument/prepareRename",
                json!({ "position": utf16_pos, "textDocument": { "uri": uri } }),
            ),
            request(
                4,
                "textDocument/rename",
                json!({
                    "newName": "星座図",
                    "position": utf16_pos,
                    "textDocument": { "uri": uri },
                }),
            ),
            notify("exit", Value::Null),
        ],
    );
    assert_eq!(
        result_of(&messages, 2),
        Value::Null,
        "改名不能位置では prepareRename は null を返す"
    );
    let prepared = result_of(&messages, 3);
    assert_eq!(prepared["placeholder"], json!("星図"));
    assert_eq!(
        prepared["range"], expected_utf16_range,
        "サロゲートペア後ろの UTF-16 範囲が正規操作と一致する"
    );

    let ws_edit = result_of(&messages, 4);
    let edit_uris: Vec<&String> = ws_edit["changes"]
        .as_object()
        .expect("changes object")
        .keys()
        .collect();
    assert!(
        edit_uris.iter().any(|u| u.ends_with("contents/01-01.cnov"))
            && edit_uris.iter().any(|u| u.ends_with("contents/01-02.cnov"))
            && edit_uris.iter().any(|u| u.ends_with("words.md")),
        "複数ファイル（原稿 2 本 + 辞書）すべてに編集が出る: {edit_uris:?}"
    );
    let client_applied = apply_lsp_workspace_edit(&before_map, &ws_edit, ROOT);
    assert_eq!(
        client_applied, rust_after,
        "LSP WorkspaceEdit を改名前テキストへ適用した結果が Rust 正規操作と完全一致する"
    );

    // 3) CLI `novnow lang rename_apply` とも全文書が一致する
    let temp = std::env::temp_dir().join(format!("novnow-lsp-rename-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("mkdir");
    for file in template::new_project("星屑の時計") {
        let target = temp.join(&file.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        let text = before_map.get(&file.path).cloned().unwrap_or(file.text);
        std::fs::write(target, text).expect("write");
    }
    std::fs::write(
        temp.join("contents/01-02.cnov"),
        before_map.get("contents/01-02.cnov").unwrap(),
    )
    .expect("write 01-02");

    let mut cli_session = Session::open_with(&temp, Interface::Cli).expect("open cli");
    cli_session
        .run(
            "lang.rename_apply",
            json!({ "path": MANUSCRIPT, "pos": byte_pos, "to": "星座図" }),
        )
        .expect("cli rename_apply");
    let cli_after: BTreeMap<String, String> = cli_session
        .aggregate()
        .documents()
        .map(|d| (d.path.clone(), d.text.clone()))
        .collect();
    let _ = std::fs::remove_dir_all(&temp);

    assert_eq!(
        cli_after, rust_after,
        "CLI と Rust と LSP の改名後の全文書が完全一致する"
    );
}

#[test]
fn nine_error_classes_map_consistently_across_rust_cli_lsp_and_mcp() {
    use novnow::app::error::{Class, Error};
    use novnow::cli::{json as cli_json, mcp as cli_mcp};

    type ExpectedRow<'a> = (Class, &'a str, i64, u8, &'a str, Option<&'a str>);
    let expected: [ExpectedRow<'_>; 9] = [
        (Class::Usage, "usage", -32602, 2, "", None),
        (
            Class::Project,
            "project",
            -32602,
            3,
            "S021",
            Some("project.toml"),
        ),
        (
            Class::Io,
            "io",
            -32000,
            4,
            "S030",
            Some("contents/01-01.cnov"),
        ),
        (
            Class::Validation,
            "validation",
            -32000,
            5,
            "E001",
            Some("contents/01-01.cnov"),
        ),
        (Class::Policy, "policy", -32000, 5, "E036", None),
        (
            Class::NotFound,
            "not_found",
            -32001,
            3,
            "",
            Some("contents/99.cnov"),
        ),
        (
            Class::Conflict,
            "conflict",
            -32002,
            4,
            "",
            Some("contents/01-01.cnov"),
        ),
        (Class::Timeout, "timeout", -32003, 4, "", None),
        (Class::Cancelled, "cancelled", -32800, 0, "", None),
    ];

    assert_eq!(Class::ALL.len(), 9);
    for ((class, code_str, rpc_code, exit_code, kind, path), listed) in
        expected.into_iter().zip(Class::ALL)
    {
        assert_eq!(class, listed);
        let mut err = Error::new(class, format!("msg-{code_str}")).with_kind(kind);
        if let Some(p) = path {
            err = err.with_path(p);
        }

        // Rust
        assert_eq!(err.class.as_str(), code_str);
        assert_eq!(err.class.jsonrpc(), rpc_code);
        assert_eq!(err.class.exit_code(), exit_code);
        let structured = err.to_json();
        assert_eq!(structured["code"], json!(code_str));
        assert_eq!(structured["kind"], json!(kind));
        assert_eq!(structured["message"], json!(format!("msg-{code_str}")));
        assert_eq!(
            structured["path"],
            path.map(|p| json!(p)).unwrap_or(Value::Null)
        );

        // CLI (`--json` 封筒 + 終了コード)
        assert_eq!(cli_json::exit_of(&class).code(), exit_code);
        let cli_env: Value =
            serde_json::from_str(&cli_json::failure("section.get", &err)).expect("cli json");
        assert_eq!(cli_env["ok"], json!(false));
        assert_eq!(cli_env["error"], structured);

        // LSP 符号表
        assert_eq!(server::code_of(&class), rpc_code);

        // MCP ツール応答
        let mcp_res = cli_mcp::tool_failure(&err);
        assert_eq!(mcp_res["isError"], json!(true));
        assert_eq!(mcp_res["content"][0]["text"], json!(err.message));
        assert_eq!(mcp_res["error"], structured);
        assert_eq!(mcp_res["data"]["code"], json!(code_str));
        assert_eq!(mcp_res["data"]["kind"], json!(kind));
        assert_eq!(mcp_res["data"]["jsonrpc"], json!(rpc_code));
    }

    // 実 LSP セッションで `Io` エラーが `-32000` + `error.data` になることを検証
    let failing_host = MemoryHost::default();
    failing_host.files.borrow_mut().fail = true;
    let docs: Vec<Document> = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text))
        .collect();
    let mut io_session = Session::with_documents(Box::new(failing_host), docs);
    io_session.set_interface(Interface::Lsp);
    let io_messages = talk(
        &mut io_session,
        &[
            request(1, "initialize", json!({})),
            notify("initialized", json!({})),
            request(
                2,
                "workspace/executeCommand",
                json!({
                    "command": "section.write",
                    "arguments": [{ "scene": "@01-01-01-01", "text": "変更" }],
                }),
            ),
            request(
                3,
                "workspace/executeCommand",
                json!({ "command": "save.now", "arguments": [{}] }),
            ),
            notify("exit", Value::Null),
        ],
    );
    let io_err = error_of(&io_messages, 3);
    assert_eq!(io_err["code"], json!(-32000), "Io は -32000（§19.1.7）");
    assert_eq!(io_err["data"]["code"], json!("io"));
    assert!(!io_err["data"]["message"].as_str().unwrap().is_empty());
}

#[test]
fn cancel_request_returns_minus_32800_without_side_effects_and_did_change_configuration_applies_and_reloads(
) {
    let mut session = open();
    let before_body = session
        .run("section.get", json!({ "scene": "@01-01-01-01" }))
        .expect("section.get")["text"]
        .as_str()
        .expect("text")
        .to_owned();

    let uri = uri_of(ROOT, MANUSCRIPT);
    let doc_text = session
        .run("document.get", json!({ "path": MANUSCRIPT }))
        .expect("document.get")["text"]
        .as_str()
        .expect("text")
        .to_owned();

    let messages = talk(
        &mut session,
        &[
            request(1, "initialize", json!({})),
            notify("initialized", json!({})),
            notify(
                "textDocument/didOpen",
                json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": "novnow",
                        "version": 1,
                        "text": doc_text,
                    }
                }),
            ),
            // 1) 通常順: リクエストの直後に `$/cancelRequest`
            request(
                20,
                "workspace/executeCommand",
                json!({
                    "command": "section.write",
                    "arguments": [{ "scene": "@01-01-01-01", "text": "副作用で書いてはいけない" }],
                }),
            ),
            notify("$/cancelRequest", json!({ "id": 20 })),
            // 2) 先行順: `$/cancelRequest` が先に届いた場合
            notify("$/cancelRequest", json!({ "id": 21 })),
            request(
                21,
                "workspace/executeCommand",
                json!({
                    "command": "idea.capture",
                    "arguments": [{ "title": "作られない着想", "kind": "memo" }],
                }),
            ),
            // 3) 続くリクエストは正常に処理される
            request(
                22,
                "workspace/executeCommand",
                json!({ "command": "idea.list", "arguments": [{}] }),
            ),
            // 4) `workspace/didChangeConfiguration` で UI 設定と言語を反映
            notify(
                "workspace/didChangeConfiguration",
                json!({
                    "settings": {
                        "novnow": {
                            "theme": "light",
                            "context_sections": 5,
                            "locale": "en",
                        }
                    }
                }),
            ),
            request(
                23,
                "workspace/executeCommand",
                json!({ "command": "settings.get", "arguments": [{}] }),
            ),
            request(
                24,
                "workspace/executeCommand",
                json!({ "command": "no.such.op", "arguments": [{}] }),
            ),
            notify("exit", Value::Null),
        ],
    );

    // キャンセル済み 2 件はどちらも `-32800` + `data.code == "cancelled"`
    for cancelled_id in [20u64, 21u64] {
        let err = error_of(&messages, cancelled_id);
        assert_eq!(err["code"], json!(-32800), "id {cancelled_id}: {err}");
        assert_eq!(err["data"]["code"], json!("cancelled"));
    }

    // 無副作用性の確認: 本文も着想一覧も一切変わっていない
    let after_body = session
        .run("section.get", json!({ "scene": "@01-01-01-01" }))
        .expect("section.get")["text"]
        .as_str()
        .expect("text")
        .to_owned();
    assert_eq!(after_body, before_body, "キャンセルされた書き込みが漏れた");
    assert_eq!(
        result_of(&messages, 22)["ideas"].as_array().unwrap().len(),
        0,
        "キャンセルされた idea.capture が実行された"
    );

    // 設定変更の反映と言語切り替え（英語エラー文言）の確認
    let updated_settings = result_of(&messages, 23);
    assert_eq!(updated_settings["settings"]["theme"], json!("light"));
    assert_eq!(updated_settings["settings"]["context_sections"], json!(5));
    assert_eq!(updated_settings["language"], json!("en"));
    let not_found_en = error_of(&messages, 24);
    assert_eq!(not_found_en["code"], json!(-32001));
    assert!(
        not_found_en["message"]
            .as_str()
            .unwrap()
            .contains("Not found"),
        "locale=en が反映されていない: {not_found_en}"
    );

    // 実ファイルシステム上のプロジェクトで外部 `project.toml` 変更が `didChangeConfiguration` で再読込されること
    let temp = std::env::temp_dir().join(format!("novnow-lsp-cfg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("mkdir");
    for file in template::new_project("初期タイトル") {
        let target = temp.join(&file.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(target, file.text).expect("write");
    }
    let mut fs_session = Session::open_with(&temp, Interface::Lsp).expect("open fs lsp");
    let toml_path = temp.join("project.toml");
    let updated_toml = std::fs::read_to_string(&toml_path)
        .expect("read toml")
        .replace("初期タイトル", "再読込後のタイトル");
    std::fs::write(&toml_path, updated_toml).expect("write toml");
    std::fs::create_dir_all(temp.join(".novnow")).expect("mkdir .novnow");
    std::fs::write(
        temp.join(".novnow/ui.json"),
        r#"{ "theme": "dark", "context_sections": 7 }"#,
    )
    .expect("write ui.json");

    let root_str = temp.to_string_lossy().into_owned();
    let mut input = Cursor::new(
        [
            request(1, "initialize", json!({})),
            notify("initialized", json!({})),
            notify(
                "workspace/didChangeConfiguration",
                json!({ "settings": {} }),
            ),
            request(
                2,
                "workspace/executeCommand",
                json!({ "command": "project.info", "arguments": [{}] }),
            ),
            request(
                3,
                "workspace/executeCommand",
                json!({ "command": "settings.get", "arguments": [{}] }),
            ),
            notify("exit", Value::Null),
        ]
        .concat(),
    );
    let mut out = server::Writer(Vec::new());
    server::run(&mut fs_session, &root_str, &mut input, &mut out);
    let _ = std::fs::remove_dir_all(&temp);

    let mut written = Cursor::new(out.0);
    let mut fs_messages = Vec::new();
    while let Some(body) = framing::read_message(&mut written) {
        fs_messages.push(serde_json::from_str::<Value>(&body).expect("JSON"));
    }
    assert_eq!(
        result_of(&fs_messages, 2)["title"],
        json!("再読込後のタイトル")
    );
    assert_eq!(
        result_of(&fs_messages, 3)["settings"]["context_sections"],
        json!(7)
    );
}
