//! Rust API（§19.2）の CI 検査。**メモリホストだけ**を使い、ファイルシステムに触れない
//! （§19.2.1「テスト用の仮想プロジェクト」）。
//!
//! 見るのは次の 4 つ:
//!
//! 1. 正規操作カタログ（§19.7 の全行）が `api.list` に揃い、決定的な JSON になること（§19.8.2）
//! 2. 型付きハンドル・`run`・`session.command` が**同じ実装**へ届くこと（§19.1.4）
//! 3. 状態性（`M—・C—`）と関門（`M 門番`）が §19.7 の例外列どおりに効くこと
//! 4. エラー 9 分類が §19.1.7 の表（JSON-RPC コード・CLI 終了コード）へ写像できること

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{json, Value};

use novnow::adapters::project_load::is_document;
use novnow::app::commands::{PALETTE_COMMANDS, SHORTCUT_PALETTE_COMMANDS};
use novnow::app::error::{Class, Error};
use novnow::app::memory_host::{MemoryFiles, MemoryHost};
use novnow::app::ops::CATALOG_VERSION;
use novnow::app::session::{canonical, command_names, Interface, Session};
use novnow::core::model::document::Document;
use novnow::core::policy::ProjectPath;
use novnow::core::template;

/// メモリ上の仮想プロジェクト（`novnow new` のサンプル。§19.2.1）。
fn open() -> (Session, Rc<RefCell<MemoryFiles>>) {
    let host = MemoryHost::default();
    let files = host.files.clone();
    let documents = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text));
    (
        Session::with_documents(Box::new(host), documents.collect()),
        files,
    )
}

/// 本文の最初の節の構造 ID（`01-01-01-01` のような 4 階層）。
fn first_section(session: &mut Session) -> String {
    let list = session
        .run("section.list", json!({}))
        .expect("section.list");
    let id = list["sections"][0]["id"].as_str().expect("id").to_owned();
    assert_eq!(id.split('-').count(), 4, "節の ID ではない: {id}");
    id
}

/// 編集中の文書へ移動する（`search.*` は編集中の文書を見るので、先に開く）。
fn open_first_section(session: &mut Session) -> String {
    let scene = first_section(session);
    session
        .run("nav.section", json!({ "scene": scene.clone() }))
        .expect("nav.section");
    scene
}

// =========================================================================== カタログ（§19.8.2）

#[test]
fn the_catalog_lists_every_canonical_operation() {
    let (mut session, _) = open();
    let catalog = session.run("api.list", json!({})).expect("api.list");
    assert_eq!(catalog["count"], json!(121), "§19.7 の行数と合わない");
    assert_eq!(catalog["catalog_version"], json!(CATALOG_VERSION));
    assert_eq!(
        catalog["interfaces"],
        json!(["rust", "python", "lsp", "mcp", "cli"]),
        "5 つのインターフェース（§19.1.1）"
    );
    assert_eq!(catalog["version"], json!(Session::version()));
    let operations = catalog["operations"].as_array().expect("operations");
    assert_eq!(operations.len(), 121);
    for operation in operations {
        for key in [
            "id",
            "domain",
            "verb",
            "description",
            "arguments",
            "returns",
            "gate",
            "stateful",
            "exceptions",
            "lsp",
            "command",
        ] {
            assert!(
                operation.get(key).is_some(),
                "{} に {key} が無い",
                operation["id"]
            );
        }
        let id = operation["id"].as_str().expect("id");
        assert_eq!(
            operation["domain"].as_str().expect("domain"),
            id.split('.').next().expect("domain"),
            "{id} のドメイン"
        );
        assert_eq!(
            operation["arguments"]["type"].as_str(),
            Some("object"),
            "{id} の引数スキーマは JSON Schema（§19.5.3）"
        );
        let description = operation["description"].as_str().expect("description");
        assert!(!description.is_empty(), "{id} の説明が空");
        assert_ne!(description, id, "{id} の説明がカタログから引けていない");
    }
}

#[test]
fn the_catalog_json_is_deterministic() {
    let (mut session, _) = open();
    let one = session.run("api.list", json!({})).expect("api.list");
    let two = session.run("api.list", json!({})).expect("api.list");
    // §19.6.2「キーは辞書順」: 同じ入力から必ず同じバイト列
    assert_eq!(canonical(&one), canonical(&two));
    assert!(
        canonical(&one).starts_with("{\"catalog_version\":"),
        "キーが辞書順になっていない: {}",
        &canonical(&one)[..40.min(canonical(&one).len())]
    );
    // 操作の並びも仕様書の表の順（§19.7）で決定的
    let ids: Vec<&str> = one["operations"]
        .as_array()
        .expect("operations")
        .iter()
        .map(|operation| operation["id"].as_str().expect("id"))
        .collect();
    assert_eq!(ids.first().copied(), Some("project.info"));
    assert_eq!(ids.last().copied(), Some("api.list"));
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "操作 ID が重複している");
}

#[test]
fn the_exceptions_column_of_the_spec_is_in_the_catalog() {
    let (mut session, _) = open();
    let catalog = session.run("api.list", json!({})).expect("api.list");
    let find = |id: &str| -> Value {
        catalog["operations"]
            .as_array()
            .expect("operations")
            .iter()
            .find(|operation| operation["id"] == json!(id))
            .unwrap_or_else(|| panic!("{id} がカタログに無い"))
            .clone()
    };
    // `M—・C—（状態性）`
    assert_eq!(
        find("nav.switcher")["exceptions"],
        json!({ "M": "stateful", "C": "stateful" })
    );
    assert_eq!(find("nav.switcher")["stateful"], json!(true));
    // `M 門番`
    assert_eq!(find("section.write")["gate"], json!(true));
    // `M—（安全: 人間専用）` の 4 つ（§19.4.3 の安全上の除外）
    for id in [
        "agent.delegation_set",
        "agent.mcp_config_set",
        "prompt.reset",
        "settings.set",
    ] {
        assert_eq!(find(id)["exceptions"], json!({ "M": "safety" }), "{id}");
    }
    // LSP 標準メソッド（§19.7 の 3 列目）
    assert_eq!(
        find("lang.completion")["lsp"],
        json!("textDocument/completion")
    );
    assert_eq!(find("project.info")["lsp"], Value::Null);
    // パレットコマンド（§19.2.3）
    assert_eq!(find("save.now")["command"], json!("save"));
    assert_eq!(find("section.insert")["command"], json!("new-section"));
}

// =========================================================================== 単一実装点（§19.1.4）

#[test]
fn the_typed_handles_reach_the_same_operations_as_run() {
    let (mut session, _) = open();
    let by_handle = session.project().info().expect("project.info");
    let by_run = session
        .run("project.info", json!({}))
        .expect("project.info");
    assert_eq!(by_handle, by_run, "型付きハンドルと run が違う結果を返した");
    assert_eq!(
        by_handle["title"],
        json!("星屑の時計"),
        "雛形のタイトルが入っていない"
    );

    let scene = open_first_section(&mut session);
    let text = "書き換えた本文\n二行目";
    session
        .section()
        .write(&scene, text)
        .expect("section.write");
    let read = session
        .run("section.get", json!({ "scene": scene }))
        .expect("section.get");
    assert!(
        read["text"]
            .as_str()
            .expect("text")
            .contains("書き換えた本文"),
        "書いた本文が読めない: {}",
        read["text"]
    );
}

#[test]
fn a_write_stays_unsaved_until_save_now_on_a_stateful_interface() {
    let (mut session, files) = open();
    let scene = first_section(&mut session);
    session
        .run(
            "section.write",
            json!({ "scene": scene, "text": "まだ保存しない本文" }),
        )
        .expect("section.write");
    // Rust（状態を持つインターフェース）は `save.now` まで保持する（§19.1.6）
    assert_eq!(
        session.run("save.state", json!({})).expect("save.state")["state"],
        json!("dirty")
    );
    assert!(
        files.borrow().written.is_empty(),
        "save.now の前に書いている: {:?}",
        files.borrow().written.keys().collect::<Vec<_>>()
    );

    let saved = session.run("save.now", json!({})).expect("save.now");
    assert_eq!(saved["state"], json!("clean"), "§9.6 の状態機");
    let written = files.borrow().written.clone();
    assert!(
        written
            .values()
            .any(|text| text.contains("まだ保存しない本文")),
        "書いた本文が保存されていない: {written:?}"
    );
    assert_eq!(
        session.run("save.state", json!({})).expect("save.state")["state"],
        json!("clean")
    );
}

#[test]
fn a_stateless_interface_flushes_the_same_write_immediately() {
    let (mut session, files) = open();
    session.set_interface(Interface::Mcp);
    // 関門（§19.1.5）を通すため、先に人間の経路で writer を apply に上げる
    session.set_interface(Interface::Rust);
    session
        .run(
            "agent.delegation_set",
            json!({ "role": "writer", "level": "apply" }),
        )
        .expect("agent.delegation_set");
    session.set_interface(Interface::Mcp);

    let scene = first_section(&mut session);
    session
        .run(
            "section.write",
            json!({ "scene": scene, "text": "その場で書き出す本文", "role": "writer" }),
        )
        .expect("section.write");
    let written = files.borrow().written.clone();
    assert!(
        written
            .values()
            .any(|text| text.contains("その場で書き出す本文")),
        "MCP は呼び出しごとに書き出す（§19.1.6）: {written:?}"
    );
}

#[test]
fn the_same_operation_answers_the_same_from_every_interface() {
    let (mut session, _) = open();
    let mut seen = Vec::new();
    for interface in [
        Interface::Rust,
        Interface::Python,
        Interface::Lsp,
        Interface::Mcp,
        Interface::Cli,
    ] {
        session.set_interface(interface);
        seen.push((
            interface.as_str().to_owned(),
            session
                .run("project.info", json!({}))
                .expect("project.info"),
        ));
    }
    let first = &seen[0].1;
    for (name, value) in &seen[1..] {
        assert_eq!(value, first, "{name} だけ違う結果を返した（§19.1.1）");
    }
}

// =========================================================================== 例外列（§19.7）

#[test]
fn operations_are_refused_on_the_interfaces_the_catalog_excludes() {
    let (mut session, _) = open();
    let stateful: Vec<String> = {
        let catalog = session.run("api.list", json!({})).expect("api.list");
        catalog["operations"]
            .as_array()
            .expect("operations")
            .iter()
            .filter(|operation| operation["stateful"] == json!(true))
            .map(|operation| operation["id"].as_str().expect("id").to_owned())
            .collect()
    };
    assert!(
        stateful.len() > 20,
        "状態性の操作が少なすぎる: {stateful:?}"
    );
    // 状態性の操作はどれも MCP では提供しない（§19.7 の `M—（状態性）`）。
    for id in &stateful {
        let def = novnow::app::ops::find(id).expect("OPS 表");
        assert!(def.excepts(Interface::Mcp).is_some(), "{id} に `M—` が無い");
    }
    // 拒否は**例外列どおり**（§19.7）。`C—` の無い操作は CLI でも動く。
    for interface in [Interface::Mcp, Interface::Cli] {
        session.set_interface(interface);
        for def in novnow::app::ops::OPS {
            if def.excepts(interface).is_none() {
                continue;
            }
            let id = def.id;
            let error = session
                .run(id, json!({}))
                .err()
                .unwrap_or_else(|| panic!("{id} が {interface:?} で通ってしまった"));
            assert_eq!(error.class, Class::Usage, "{id} の分類");
            assert!(
                error.message.contains(id),
                "{id} のメッセージに操作名が無い: {}",
                error.message
            );
        }
    }
}

#[test]
fn the_safety_exclusions_are_refused_over_mcp() {
    let (mut session, _) = open();
    session.set_interface(Interface::Mcp);
    for (id, args) in [
        (
            "agent.delegation_set",
            json!({ "role": "writer", "level": "apply" }),
        ),
        ("agent.mcp_config_set", json!({ "text": "{}" })),
        ("prompt.reset", json!({ "name": "writer" })),
        ("settings.set", json!({ "key": "theme", "value": "dark" })),
    ] {
        let error = session
            .run(id, args)
            .err()
            .unwrap_or_else(|| panic!("{id} が MCP で通ってしまった（§19.4.3）"));
        assert_eq!(error.class, Class::Usage, "{id}");
    }
    // 人間の経路（Rust）では通る
    session.set_interface(Interface::Rust);
    session
        .run("settings.set", json!({ "key": "theme", "value": "dark" }))
        .expect("settings.set");
    assert_eq!(
        session.settings().theme,
        novnow::app::settings::ThemeChoice::Dark
    );
}

#[test]
fn a_gated_write_over_mcp_needs_a_role_that_may_write() {
    let (mut session, _) = open();
    let scene = first_section(&mut session);
    session.set_interface(Interface::Mcp);
    let error = session
        .run(
            "section.write",
            json!({ "scene": scene, "text": "関門を通らない本文" }),
        )
        .expect_err("role が無いのに通った");
    assert_eq!(error.class, Class::Policy, "§19.1.5 の関門");
    assert_eq!(error.kind, "E036");

    // 既定の委譲（propose）では role を付けても書けない
    let error = session
        .run(
            "section.write",
            json!({ "scene": scene, "text": "まだ書けない本文", "role": "writer" }),
        )
        .expect_err("propose のまま書けた");
    assert_eq!(error.kind, "E036");
}

/// §19.7 の「書き込みのときだけ関門」3 件（`OpDef::gate_arg`）。
///
/// `search.file` の置換・`lang.code_action` の適用・`export.text` の書き出しは `role` を求めるが、
/// 同じ操作の読み取りだけの呼び出しには `role` を求めない（「テキストの返却のみは無関門」）。
/// 例外列に `M` を入れて操作ごと外すと §19.5.1 の「提供する」に反するので、引き金の引数名で表す。
#[test]
fn the_write_time_gate_binds_only_to_the_writing_call() {
    let (mut session, _) = open();
    open_first_section(&mut session);
    session.set_interface(Interface::Mcp);
    let document = "contents/01-01.cnov";

    // search.file（§19.7「置換は M 門番」）
    let found = session
        .run("search.file", json!({ "query": "の" }))
        .expect("検索だけの呼び出しに role を求めた");
    assert!(
        found["count"].as_u64().expect("count") > 0,
        "雛形の本文に「の」が無い"
    );
    let error = session
        .run(
            "search.file",
            json!({ "query": "の", "replace": "ほ", "role": "writer" }),
        )
        .expect_err("置換が propose のまま通った");
    assert_eq!(error.kind, "E036", "§19.1.5 の関門");
    assert_eq!(error.class, Class::Policy);

    // lang.code_action（§19.7「適用は M 門番」）
    session
        .run(
            "lang.code_action",
            json!({ "path": document, "start": 0, "end": 1 }),
        )
        .expect("候補の列挙に role を求めた");
    let error = session
        .run(
            "lang.code_action",
            json!({ "path": document, "start": 0, "end": 1, "apply": true, "role": "writer" }),
        )
        .expect_err("適用が propose のまま通った");
    assert_eq!(error.kind, "E036");

    // export.text（§19.7「M 門番（書き込み時。テキストの返却のみは無関門）」）
    let returned = session
        .run("export.text", json!({}))
        .expect("テキストの返却に role を求めた");
    assert!(returned["text"].is_string(), "返却の形が違う: {returned}");
    let error = session
        .run("export.text", json!({ "out": "out.txt", "role": "writer" }))
        .expect_err("書き出しが propose のまま通った");
    assert_eq!(error.kind, "E036");

    // 人間経路（Rust）では同じ呼び出しが通る（§19.1.5 は MCP だけの規則）
    session.set_interface(Interface::Rust);
    session
        .run("search.file", json!({ "query": "の", "replace": "ほ" }))
        .expect("人間経路の置換");
}

// =========================================================================== イベント（§19.2.4）

#[test]
fn events_are_queued_and_drained_once() {
    let (mut session, _) = open();
    let scene = first_section(&mut session);
    session
        .run(
            "section.write",
            json!({ "scene": scene, "text": "イベントを出す本文" }),
        )
        .expect("section.write");
    let drained = session
        .run("session.poll_events", json!({}))
        .expect("session.poll_events");
    let events = drained["events"].as_array().expect("events");
    assert!(
        events
            .iter()
            .any(|event| event["kind"] == json!("document_changed")),
        "文書の変化が積まれていない: {events:?}"
    );
    let again = session
        .run("session.poll_events", json!({}))
        .expect("session.poll_events");
    assert_eq!(
        again["count"],
        json!(0),
        "取り出したイベントがまた積まれている"
    );
}

#[test]
fn a_callback_sees_the_same_events() {
    let (mut session, _) = open();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    session.on_event(move |event| {
        sink.borrow_mut().push(event.kind().to_owned());
    });
    let scene = first_section(&mut session);
    session
        .run(
            "section.write",
            json!({ "scene": scene, "text": "コールバックへ届く本文" }),
        )
        .expect("section.write");
    session.run("save.now", json!({})).expect("save.now");
    let kinds = seen.borrow();
    assert!(
        kinds.iter().any(|kind| kind == "document_changed"),
        "コールバックに届いていない: {kinds:?}"
    );
    assert!(
        kinds.iter().any(|kind| kind == "save_state_changed"),
        "保存状態の遷移が届いていない: {kinds:?}"
    );
}

// =========================================================================== パレットコマンド（§19.2.3）

#[test]
fn every_palette_command_is_reachable_by_name() {
    let names = command_names();
    assert_eq!(
        names.len(),
        PALETTE_COMMANDS.len() + SHORTCUT_PALETTE_COMMANDS.len(),
        "パレットの全コマンドが載っていない"
    );
    // §19.2.3: どのコマンド名も `session.command` から到達できる（MUST）。戻りは 4 形のどれか
    // （実行結果 `data`・続き `needs`・移動 `moved`／設定 `key`・GUI だけ `gui_only`）。
    // 状態が足りないときの拒否（検索していない `find-next` など）は到達のうちに入れるが、
    // 「そのコマンドは無い」（`unsupported`）は到達していないので落とす。
    for name in &names {
        let (mut session, _) = open();
        match session.run("session.command", json!({ "name": name })) {
            Ok(result) => {
                assert_eq!(
                    result["command"],
                    json!(*name),
                    "{name} の戻りが別のコマンド"
                );
                let shape = ["data", "gui_only", "key", "moved", "needs"]
                    .iter()
                    .filter(|key| result.get(*key).is_some())
                    .count();
                assert_eq!(shape, 1, "{name} の戻りの形が定まらない: {result}");
            }
            Err(error) => assert_ne!(
                error.kind, "unsupported",
                "{name} がどの実装にも届いていない（{:?} {}）",
                error.class, error.message
            ),
        }
    }
}

#[test]
fn data_commands_run_from_the_palette() {
    // データ操作に相当するコマンドは必ず通る（§19.2.3 MUST）。引数が要るものは、GUI が
    // 入力欄を開くのと同じく `needs`（続き）を返し、その続きは型付き操作の必須引数そのもの。
    let (mut session, _) = open();
    for name in [
        "save",
        "new-section",
        "new-item",
        "capture",
        "drop",
        "format",
        "definition",
        "references",
        "complete",
        "quick-fix",
        "inlay-hints",
        "rename",
        "review",
        "export",
        "find",
        "search",
        "replace",
        "jump",
        "scan",
        "register-term",
        "register-character",
        "material-search",
        "term",
        "modes",
        "fold",
        "unfold",
        "reload",
        "stop-all",
        "recent-next",
        "confirm-next",
        "up",
    ] {
        let result = session
            .run("session.command", json!({ "name": name }))
            .unwrap_or_else(|error| panic!("{name} が実行できなかった（{:?}）", error.class));
        let op = result["op"]
            .as_str()
            .unwrap_or_else(|| panic!("{name} が操作に届かない: {result}"));
        assert!(
            novnow::app::ops::find(op).is_some(),
            "{name} の対応先 {op} がカタログに無い"
        );
        assert!(
            result.get("data").is_some() || result.get("needs").is_some(),
            "{name} が実行にも続きにもなっていない: {result}"
        );
    }
    // 続き（`needs`）は型付き操作の必須引数そのものなので、そのまま渡せる
    let needs = session
        .run("session.command", json!({ "name": "capture" }))
        .expect("capture");
    assert_eq!(needs["op"], json!("idea.capture"), "対応する型付き操作");
    assert_eq!(needs["needs"], json!(["title"]), "必要な引数");
    session
        .run(
            "idea.capture",
            json!({ "title": "続きから拾った着想", "kind": "idea" }),
        )
        .expect("needs のとおりに渡せば通る");
}

#[test]
fn a_command_that_needs_more_state_says_why() {
    // 状態が足りないコマンドは「無い」のではなく、足りない状態のエラー分類で断る（§19.1.7）
    let (mut session, _) = open();
    let error = session
        .run("session.command", json!({ "name": "find-next" }))
        .expect_err("検索していないのに次へ進めた");
    assert_eq!(error.class, Class::Usage, "検索していない {error:?}");
    let error = session
        .run("session.command", json!({ "name": "diff" }))
        .expect_err("衝突が無いのに解決できた");
    assert_eq!(error.class, Class::Conflict, "衝突が無い {error:?}");
    // 知らないコマンド名だけが `usage`（引数の値が不正）
    let error = session
        .run("session.command", json!({ "name": "no-such-command" }))
        .expect_err("知らないコマンドが通った");
    assert_eq!(error.class, Class::Usage, "{error:?}");
}

#[test]
fn a_new_section_appears_in_the_project_once_it_has_text() {
    let (mut session, _) = open();
    let scene = first_section(&mut session);
    // 本文の無い新設は §10.0（空の節を作らない）なので文書には現れない
    let pending = session
        .run("section.insert", json!({}))
        .expect("section.insert");
    assert_eq!(pending["pending"], json!(true), "本文が無いのに節になる");
    let listed = session
        .run("section.list", json!({}))
        .expect("section.list");
    assert_eq!(listed["count"], json!(1), "空の節が一覧に出る");
    // 本文を入れると節になり、一覧・取得・削除のどれからも見える
    let created = session
        .run("section.insert", json!({ "text": "新しい節の本文\n" }))
        .expect("section.insert");
    assert_eq!(
        created["pending"],
        json!(false),
        "本文があるのに節にならない"
    );
    let id = created["id"].as_str().expect("id").to_owned();
    assert_ne!(id, scene, "同じ節を指している");
    let listed = session
        .run("section.list", json!({}))
        .expect("section.list");
    let ids: Vec<&str> = listed["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .map(|item| item["id"].as_str().expect("id"))
        .collect();
    assert!(ids.contains(&id.as_str()), "新しい節が一覧に無い: {ids:?}");
    let fetched = session
        .run("section.get", json!({ "scene": id.clone() }))
        .expect("section.get");
    assert!(
        fetched["text"]
            .as_str()
            .expect("text")
            .contains("新しい節の本文"),
        "本文が入っていない: {fetched}"
    );
    let deleted = session
        .run("section.delete", json!({ "scene": id.clone() }))
        .unwrap_or_else(|error| {
            panic!(
                "新設した節を消せない（{:?} {}）",
                error.class, error.message
            )
        });
    assert_eq!(deleted["deleted"], json!(id));
    let listed = session
        .run("section.list", json!({}))
        .expect("section.list");
    let ids: Vec<&str> = listed["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .map(|item| item["id"].as_str().expect("id"))
        .collect();
    assert!(!ids.contains(&id.as_str()), "消した節が残っている: {ids:?}");
}

#[test]
fn a_settings_command_changes_the_session_settings() {
    let (mut session, files) = open();
    let before = session.settings().font_scale;
    session
        .run("session.command", json!({ "name": "font-up" }))
        .expect("font-up");
    assert!(
        session.settings().font_scale > before,
        "文字サイズが変わらない"
    );
    session
        .run("session.command", json!({ "name": "font-reset" }))
        .expect("font-reset");
    assert_eq!(session.settings().font_scale, 1.0);
    session
        .run("session.command", json!({ "name": "theme" }))
        .expect("theme");
    assert_eq!(
        session.settings().theme,
        novnow::app::settings::ThemeChoice::Dark,
        "system → dark（§9.8.1）"
    );
    // 設定は `.novnow/ui.json` へ書く（§4.4.1）
    assert!(
        files.borrow().written.contains_key(".novnow/ui.json"),
        "設定が書かれていない: {:?}",
        files.borrow().written.keys().collect::<Vec<_>>()
    );
    let unknown = session
        .run("session.command", json!({ "name": "no-such-command" }))
        .expect_err("知らないコマンドが通った");
    assert_eq!(unknown.class, Class::Usage);
}

// =========================================================================== 読み書きのいろいろ

#[test]
fn search_replaces_inside_the_open_document() {
    let (mut session, _) = open();
    open_first_section(&mut session);
    let found = session
        .run("search.file", json!({ "query": "の" }))
        .expect("search.file");
    assert!(
        found["count"].as_u64().expect("count") > 0,
        "雛形の本文に「の」が無い"
    );
    let replaced = session
        .run(
            "search.replace_file",
            json!({ "query": "の", "replace": "ノ" }),
        )
        .expect("search.replace_file");
    assert_eq!(replaced["count"], found["count"], "置換の件数が違う");
    let after = session
        .run("search.file", json!({ "query": "の" }))
        .expect("search.file");
    assert_eq!(after["count"], json!(0), "置換したのに残っている");
}

#[test]
fn a_captured_idea_is_listed_and_the_file_is_saved() {
    let (mut session, files) = open();
    let captured = session
        .run(
            "idea.capture",
            json!({ "title": "時計塔の秘密", "kind": "plot" }),
        )
        .expect("idea.capture");
    let id = captured["captured"].as_str().expect("id").to_owned();
    let list = session.run("idea.list", json!({})).expect("idea.list");
    assert!(
        list["ideas"]
            .as_array()
            .expect("ideas")
            .iter()
            .any(|idea| idea["id"] == json!(id)),
        "捕まえたアイデアが一覧に無い: {list}"
    );
    session.run("save.now", json!({})).expect("save.now");
    let written = files.borrow().written.clone();
    let ideas = written.get("ideas.md").expect("ideas.md が書かれていない");
    assert!(ideas.contains("時計塔の秘密"), "ideas.md: {ideas}");
    assert!(ideas.contains(&id), "ID が書かれていない: {ideas}");
}

#[test]
fn a_proposed_material_lands_in_the_index() {
    let (mut session, files) = open();
    let proposed = session
        .run(
            "materials.propose",
            json!({ "title": "懐中時計の歴史", "category": "取材", "markdown": "# 本文\n" }),
        )
        .expect("materials.propose");
    let id = proposed["id"].as_str().expect("id").to_owned();
    assert!(id.starts_with('M'), "資料の ID: {id}");
    let list = session
        .run("materials.list", json!({}))
        .expect("materials.list");
    assert_eq!(list["count"].as_u64().expect("count"), 1);
    session.run("save.now", json!({})).expect("save.now");
    let written = files.borrow().written.clone();
    let index = written
        .get("materials/index.json")
        .expect("index.json が書かれていない");
    assert!(index.contains(&id), "索引に ID が無い: {index}");
    let body = written
        .get(proposed["path"].as_str().expect("path"))
        .expect("資料 MD が書かれていない");
    assert!(body.contains("懐中時計の歴史"), "資料 MD: {body}");
}

#[test]
fn diagnostics_run_counts_what_core_counts() {
    let (mut session, _) = open();
    let expected: usize = session
        .aggregate()
        .documents()
        .map(|document| novnow::core::diagnostics::document(document, session.aggregate()).len())
        .sum();
    let run = session
        .run("diagnostics.run", json!({}))
        .expect("diagnostics.run");
    assert_eq!(run["count"], json!(expected), "core の検査と件数が違う");
    let rows = run["diagnostics"].as_array().expect("diagnostics");
    for row in rows {
        assert!(row["code"].as_str().expect("code").len() == 4, "{row}");
        assert!(
            ["error", "warning", "info"].contains(&row["severity"].as_str().unwrap_or("")),
            "{row}"
        );
    }
}

#[test]
fn export_text_returns_the_manuscript_and_can_write_a_chosen_file() {
    let (mut session, files) = open();
    let text = session.run("export.text", json!({})).expect("export.text");
    assert!(
        text["chars"].as_u64().expect("chars") > 0,
        "書き出しが空: {text}"
    );
    assert!(
        text["name"].as_str().expect("name").ends_with(".txt"),
        "§17.2 の既定の名前: {}",
        text["name"]
    );
    assert!(
        files.borrow().exported.is_empty(),
        "out を指定していないのに書いている"
    );

    session
        .run("export.text", json!({ "out": "/tmp/novnow-export.txt" }))
        .expect("export.text --out");
    let exported = files.borrow().exported.clone();
    assert_eq!(exported.len(), 1, "{exported:?}");
    assert!(
        exported.contains_key("/tmp/novnow-export.txt"),
        "{exported:?}"
    );
}

// =========================================================================== エラーの写像（§19.1.7）

#[test]
fn an_unknown_operation_is_a_usage_error() {
    let (mut session, _) = open();
    let error = session
        .run("section.nope", json!({}))
        .expect_err("知らない操作が通った");
    assert_eq!(error.class, Class::Usage);
    assert!(error.message.contains("section.nope"), "{}", error.message);
    let not_an_object = session
        .run("project.info", json!("文字列"))
        .expect_err("引数の形が検査されていない");
    assert_eq!(not_an_object.class, Class::Usage);
}

#[test]
fn the_nine_error_classes_map_onto_the_documented_codes() {
    // §19.1.7 の表（JSON-RPC コード・CLI 終了コード §14.2.5 / §19.6.3）
    let rows = [
        (Class::Usage, -32602, 2, true),
        (Class::Project, -32602, 3, true),
        (Class::Io, -32000, 4, true),
        (Class::Validation, -32000, 5, false),
        (Class::Policy, -32000, 5, false),
        (Class::NotFound, -32001, 3, true),
        (Class::Conflict, -32002, 4, true),
        (Class::Timeout, -32003, 4, true),
        (Class::Cancelled, -32800, 0, true),
    ];
    for (class, jsonrpc, exit, protocol) in rows {
        assert_eq!(class.jsonrpc(), jsonrpc, "{class:?} の JSON-RPC コード");
        assert_eq!(class.exit_code(), exit, "{class:?} の終了コード");
        assert_eq!(
            class.is_protocol_error(),
            protocol,
            "{class:?} は結果内に載せるか"
        );
        assert_eq!(
            Class::parse(class.as_str()),
            Some(class),
            "{class:?} の名前が読み戻せない"
        );
    }
    // `error` は §19.6.2 の 4 キーを持つ
    let error = Error::usage(novnow::ui::i18n::Locale::Ja, "scene");
    let json = error.to_json();
    for key in ["code", "kind", "message", "path"] {
        assert!(json.get(key).is_some(), "error に {key} が無い: {json}");
    }
    assert_eq!(json["code"], json!("usage"));
    assert_eq!(json["path"], Value::Null);
}

#[test]
fn the_catalog_is_localized_in_both_languages() {
    let (mut session, _) = open();
    let ja = session.run("api.list", json!({})).expect("api.list");
    session.set_locale(novnow::ui::i18n::Locale::En);
    let en = session.run("api.list", json!({})).expect("api.list");
    assert_ne!(ja, en, "言語を変えても説明が変わらない");
    let descriptions = |catalog: &Value| -> Vec<String> {
        catalog["operations"]
            .as_array()
            .expect("operations")
            .iter()
            .map(|operation| {
                operation["description"]
                    .as_str()
                    .expect("description")
                    .to_owned()
            })
            .collect()
    };
    let (ja, en) = (descriptions(&ja), descriptions(&en));
    assert_eq!(ja.len(), en.len());
    assert!(
        ja.iter().zip(&en).all(|(one, two)| one != two),
        "説明が地域化されていない行がある"
    );
    assert!(
        en.iter().all(|text| text.is_ascii()),
        "en の説明に ASCII 以外が混ざっている"
    );
}

// =========================================================================== 外部変更と衝突（§8.5・§9.6・§19.1.6）

/// 未保存の編集があるファイルが外部で変わると衝突になる（§8.5）。
fn conflict() -> (Session, String, String) {
    let (mut session, files) = open();
    let scene = open_first_section(&mut session);
    let path = "contents/01-01.cnov";
    // 外部の版は「編集前の文書 + 1 行」（自分の編集を含まない形にする）
    let original = session.aggregate().text(path).to_owned();
    session
        .run(
            "section.write",
            json!({ "scene": scene.clone(), "text": "自分の本文\n" }),
        )
        .expect("section.write");
    let external = format!("{original}外部で書き換わった一行\n");
    files
        .borrow_mut()
        .changes
        .push((path.to_owned(), Some(external)));
    let changes = session.poll_external();
    assert_eq!(changes.len(), 1, "外部変更を取り込めていない");
    // 保存状態機は 4 状態（§9.6）。衝突は状態ではなく `conflicts` に出る
    let state = session.run("save.state", json!({})).expect("save.state");
    assert_eq!(state["state"], json!("dirty"), "{state}");
    assert_eq!(state["conflicts"], json!([path]), "{state}");
    (session, scene, path.to_owned())
}

#[test]
fn an_external_change_becomes_a_conflict_and_keep_mine_wins() {
    let (mut session, scene, path) = conflict();
    // 差分を見せてから自分の側を残す（§9.6 の keep mine）
    let diff = session
        .run("save.diff_external", json!({}))
        .expect("save.diff_external");
    assert_eq!(diff["diffs"][0]["path"], json!(path), "{diff}");
    assert!(
        !diff["diffs"][0]["lines"]
            .as_array()
            .expect("lines")
            .is_empty(),
        "差分が出ていない: {diff}"
    );
    let kept = session
        .run("save.keep_mine", json!({}))
        .expect("save.keep_mine");
    assert_eq!(kept["kept"], json!([path]), "{kept}");
    let text = session
        .run("section.get", json!({ "scene": scene }))
        .expect("section.get");
    let text = text["text"].as_str().expect("text");
    assert!(
        text.contains("自分の本文"),
        "自分の側が残っていない: {text}"
    );
    assert!(
        !text.contains("外部で書き換わった一行"),
        "外部の側が混ざっている: {text}"
    );
    let state = session.run("save.state", json!({})).expect("save.state");
    assert_eq!(state["conflicts"], json!([]), "衝突が残っている: {state}");
}

#[test]
fn reload_external_takes_the_other_side() {
    let (mut session, scene, path) = conflict();
    let reloaded = session
        .run("save.reload_external", json!({}))
        .expect("save.reload_external");
    assert_eq!(reloaded["reloaded"], json!([path]), "{reloaded}");
    let text = session
        .run("section.get", json!({ "scene": scene }))
        .expect("section.get");
    let text = text["text"].as_str().expect("text");
    assert!(
        text.contains("外部で書き換わった一行"),
        "外部の側になっていない: {text}"
    );
    assert!(!text.contains("自分の本文"), "自分の側が残っている: {text}");
    let state = session.run("save.state", json!({})).expect("save.state");
    assert_eq!(state["conflicts"], json!([]), "衝突が残っている: {state}");
}

#[test]
fn resolving_a_conflict_twice_is_a_conflict_error() {
    let (mut session, _, _) = conflict();
    session
        .run("save.keep_mine", json!({}))
        .expect("save.keep_mine");
    let error = session
        .run("save.keep_mine", json!({}))
        .expect_err("衝突が無いのに解決できた");
    assert_eq!(error.class, Class::Conflict, "{error:?}");
}

#[test]
fn section_split_splits_at_a_byte_position_and_delete_takes_the_plot_too() {
    let (mut session, _) = open();
    let scene = first_section(&mut session);
    let text = session.section().get(&scene).expect("section.get");
    assert!(!text.is_empty());
    let before = session
        .run("section.list", json!({}))
        .expect("section.list")["count"]
        .as_u64()
        .unwrap();
    // 先頭では割らない（Ok で split=false）
    let refused = session
        .run("section.split", json!({ "scene": scene, "at": 0 }))
        .expect("section.split at 0");
    assert_eq!(refused["split"], json!(false));
    // 途中の文字境界で割る（§8.1.1）
    let at = text.char_indices().nth(3).map(|(at, _)| at).unwrap_or(1);
    let done = session
        .section()
        .split(Some(&scene), at)
        .expect("section.split");
    assert_eq!(done["split"], json!(true));
    let after = session
        .run("section.list", json!({}))
        .expect("section.list")["count"]
        .as_u64()
        .unwrap();
    assert_eq!(after, before + 1, "節が 1 つ増える");
    let upper = session.section().get(&scene).expect("upper");
    assert_eq!(upper, text[..at].trim_end_matches('\n'));
    // section.delete は本文と対応プロットを一緒に消す（§8.1.2）: 消した節の本文は戻らない
    let deleted = session
        .run("section.delete", json!({ "scene": scene }))
        .expect("section.delete");
    assert_eq!(deleted["deleted"], json!(scene));
    let last = session
        .run("section.list", json!({}))
        .expect("section.list")["count"]
        .as_u64()
        .unwrap();
    assert_eq!(last, before, "分割で増えた分が削除で戻る");
}

// =========================================================================== プロットの階層（§10.3.2・§19.5.2）

#[test]
fn plot_write_reaches_every_level_and_refuses_an_unknown_one() {
    let (mut session, _) = open();
    // 全体プロット（`outline` と `all` は同じ場所）
    for level in ["outline", "all"] {
        let written = session
            .run(
                "plot.write",
                json!({ "level": level, "text": "@01\nsummary: 全体\n;\n" }),
            )
            .expect("plot.write outline");
        assert_eq!(written["path"], json!("plots/all.cnov"), "{level}");
        assert_eq!(written["level"], json!(level));
    }
    // 章プロット（`chapter` を省くと編集中の節の章）
    let chapter = session
        .run(
            "plot.write",
            json!({ "level": "chapter", "chapter": 1, "text": "@01\nsummary: 章\n;\n" }),
        )
        .expect("plot.write chapter");
    assert_eq!(chapter["path"], json!("plots/01.cnov"));
    let scene = first_section(&mut session);
    let fallback = session
        .run(
            "plot.write",
            json!({ "level": "chapter", "text": "@01\nsummary: 章\n;\n" }),
        )
        .expect("plot.write chapter without chapter");
    let expected = format!("plots/{}.cnov", &scene[..2]);
    assert_eq!(fallback["path"], json!(expected), "{scene}");
    // 知らない level は引数エラー（本文は変わらない）
    let error = session
        .run("plot.write", json!({ "level": "scene", "text": "だめ" }))
        .expect_err("知らない level");
    assert_eq!(error.class, Class::Usage, "{error:?}");
    // 話プロットは scene の節を差し替える
    let episode = session
        .run(
            "plot.write",
            json!({ "level": "episode", "scene": scene, "text": "@01-01-01-01\n話\n" }),
        )
        .expect("plot.write episode");
    assert_eq!(episode["path"], json!("plots/01-01.cnov"));
}

// =========================================================================== 読み取り専用（§4.1）

#[test]
fn a_read_only_project_reports_w041_and_keeps_working_for_reads() {
    let host = MemoryHost::default();
    let files = host.files.clone();
    // 開く前から書けないプロジェクト（読み取り専用メディア）
    files.borrow_mut().fail = true;
    let documents = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text));
    let mut session = Session::with_documents(Box::new(host), documents.collect());
    assert!(session.read_only(), "書けないプロジェクト");
    let validated = session
        .run("project.validate", json!({}))
        .expect("project.validate");
    let codes: Vec<&str> = validated["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter_map(|found| found["code"].as_str())
        .collect();
    assert!(codes.contains(&"W041"), "{codes:?}");
    let counted = session
        .run("diagnostics.run", json!({}))
        .expect("diagnostics.run");
    let first = &counted["diagnostics"][0];
    assert_eq!(first["code"], json!("W041"));
    assert_eq!(first["path"], json!(""));
    assert_eq!(first["line"], json!(0));
    // 既定プロンプトは書かずに W041 を知らせる（§15.5.4）
    let notice = session.prepare_agents().expect("通知");
    assert!(notice.contains("読み取り専用"), "{notice}");
    assert_eq!(files.borrow().writes, 0, "読み取り専用では書かない");
    // 読み取りは普通にできる
    let scene = first_section(&mut session);
    assert!(!session
        .section()
        .get(&scene)
        .expect("section.get")
        .is_empty());
}

// =========================================================================== Issue #55 Batch 1 回帰検査

#[test]
fn find_next_and_find_previous_commands_step_both_ways_from_first_middle_and_last() {
    let (mut session, _) = open();
    let scene = open_first_section(&mut session);
    session
        .run(
            "section.write",
            json!({ "scene": scene.clone(), "text": "鈴と月の鈴と風の鈴\n" }),
        )
        .expect("section.write");
    // 最初の検索で 0 番目（最初の一致）に位置づく
    let first = session
        .run("search.file", json!({ "query": "鈴" }))
        .expect("search.file");
    assert_eq!(first["count"], json!(3), "{first}");
    let hits = first["hits"].as_array().expect("hits").clone();
    assert_eq!(hits.len(), 3);

    // 最初(0) -> 中央(1) -> 末尾(2) -> 折り返し最初(0)
    let to_mid = session.command("find-next").expect("find-next from first")["data"].clone();
    assert_eq!(
        (&to_mid["selected"], &to_mid["range"]),
        (&json!(1), &hits[1])
    );
    let to_last = session.command("find-next").expect("find-next from middle")["data"].clone();
    assert_eq!(
        (&to_last["selected"], &to_last["range"]),
        (&json!(2), &hits[2])
    );
    let wrapped_first = session.command("find-next").expect("find-next from last")["data"].clone();
    assert_eq!(
        (&wrapped_first["selected"], &wrapped_first["range"]),
        (&json!(0), &hits[0])
    );

    // 逆方向: 最初(0) -> 折り返し末尾(2) -> 中央(1) -> 最初(0)
    let wrapped_last = session
        .command("find-previous")
        .expect("find-previous from first")["data"]
        .clone();
    assert_eq!(
        (&wrapped_last["selected"], &wrapped_last["range"]),
        (&json!(2), &hits[2])
    );
    let back_mid = session
        .command("find-previous")
        .expect("find-previous from last")["data"]
        .clone();
    assert_eq!(
        (&back_mid["selected"], &back_mid["range"]),
        (&json!(1), &hits[1])
    );
    let back_first = session
        .command("find-previous")
        .expect("find-previous from middle")["data"]
        .clone();
    assert_eq!(
        (&back_first["selected"], &back_first["range"]),
        (&json!(0), &hits[0])
    );
}

#[test]
fn regex_replace_expands_numbered_named_and_newline_without_offset_drift() {
    let (mut session, _) = open();
    let scene = open_first_section(&mut session);

    // 1) 番号キャプチャ（複数一致・文字長変化でも範囲がずれない）
    session
        .run(
            "section.write",
            json!({ "scene": scene.clone(), "text": "A=1, BB=222, CCC=33\n" }),
        )
        .expect("section.write");
    let res = session
        .run(
            "search.replace_file",
            json!({
                "query": "([A-Z]+)=(\\d+)",
                "replace": "[$2:$1]",
                "regex": true,
            }),
        )
        .expect("numbered capture");
    assert_eq!(res["count"], json!(3));
    assert_eq!(
        session.section().get(&scene).expect("get"),
        "[1:A], [222:BB], [33:CCC]"
    );

    // 2) 名前付きキャプチャ（`search.file` 側の置換経路でも同じ）
    session
        .run(
            "section.write",
            json!({ "scene": scene.clone(), "text": "id:01/春 id:02/夏風 id:03/秋\n" }),
        )
        .expect("section.write");
    let via_search = session
        .run(
            "search.file",
            json!({
                "query": "id:(?P<num>\\d+)/(?P<word>\\S+)",
                "replace": "${word}(#${num})",
                "regex": true,
            }),
        )
        .expect("named capture via search.file");
    assert_eq!(via_search["count"], json!(3));
    assert_eq!(
        session.section().get(&scene).expect("get"),
        "春(#01) 夏風(#02) 秋(#03)"
    );

    // 3) `\n` 改行展開（リテラル検索と正規表現検索の双方）
    session
        .run(
            "section.write",
            json!({ "scene": scene.clone(), "text": "壱 / 弐 / 参\n" }),
        )
        .expect("section.write");
    session
        .run(
            "search.replace_file",
            json!({
                "query": " / ",
                "replace": "\\n",
                "regex": false,
            }),
        )
        .expect("newline literal");
    assert_eq!(session.section().get(&scene).expect("get"), "壱\n弐\n参");
}

#[test]
fn export_text_selects_markdown_by_format_or_case_insensitive_out_extension() {
    let (base, _) = open();
    let mut proj_settings = base.aggregate().settings().clone();
    proj_settings.export.insert_headings = true;
    let mut docs: Vec<Document> = base
        .aggregate()
        .documents()
        .filter(|d| {
            d.path != "project.toml" && d.path != "plots/all.cnov" && d.path != "plots/01.cnov"
        })
        .cloned()
        .collect();
    docs.push(Document::new(
        "plots/all.cnov",
        "title: 星屑の時計\n\n@01 星の章\n;\n",
    ));
    docs.push(Document::new(
        "plots/01.cnov",
        "title: 星の章\n\n@01-01 屋根裏の星図\n;\n",
    ));
    let project = novnow::core::features::Project::with_documents(proj_settings, docs);
    let host = MemoryHost::default();
    let host_files = host.files.clone();
    let mut session = Session::from_project(
        Box::new(host),
        project,
        &novnow::app::settings::UiSettings::default(),
    );

    let txt = session.run("export.text", json!({})).expect("default txt");
    let txt_body = txt["text"].as_str().expect("txt body").to_owned();
    assert!(txt["name"].as_str().unwrap().ends_with(".txt"), "{txt}");
    assert!(
        !txt_body.starts_with("# "),
        "既定はプレーンテキスト: {txt_body}"
    );
    assert!(txt_body.contains("星の章"), "{txt_body}");

    let md = session
        .run("export.text", json!({ "format": "md" }))
        .expect("format md");
    let md_body = md["text"].as_str().expect("md body").to_owned();
    assert!(md["name"].as_str().unwrap().ends_with(".md"), "{md}");
    assert!(
        md_body.contains("# 星の章"),
        "Markdown 章見出しが無い: {md_body}"
    );
    assert!(
        md_body.contains("## 屋根裏の星図"),
        "Markdown 話見出しが無い: {md_body}"
    );
    assert_ne!(md_body, txt_body);

    let out_lower = session
        .run("export.text", json!({ "out": "/tmp/manuscript.md" }))
        .expect("out .md");
    assert!(out_lower["name"].as_str().unwrap().ends_with(".md"));

    let out_upper = session
        .run("export.text", json!({ "out": "/tmp/MANUSCRIPT.MD" }))
        .expect("out .MD");
    assert!(out_upper["name"].as_str().unwrap().ends_with(".md"));

    let exported = host_files.borrow().exported.clone();
    assert_eq!(exported.get("/tmp/manuscript.md"), Some(&md_body));
    assert_eq!(exported.get("/tmp/MANUSCRIPT.MD"), Some(&md_body));

    // 未知 format と不正拡張子は拒否し書き込まない
    let bad_fmt = session
        .run("export.text", json!({ "format": "docx" }))
        .expect_err("unknown format");
    assert_eq!(bad_fmt.class, Class::Usage);
    let bad_ext = session
        .run("export.text", json!({ "out": "/tmp/manuscript.zip" }))
        .expect_err("disallowed extension");
    assert_eq!(bad_ext.class, Class::Io);
    assert!(!host_files
        .borrow()
        .exported
        .contains_key("/tmp/manuscript.zip"));
}

#[test]
fn poll_events_drains_at_most_limit_in_fifo_order_without_dropping_remainder() {
    let (mut session, _) = open();
    let _ = session.poll_events();
    let keys = [
        "theme",
        "inlay_hints",
        "max_line_chars",
        "font_scale",
        "context_sections",
    ];
    let values = [json!("dark"), json!(false), json!(42), json!(1.2), json!(3)];
    for (k, v) in keys.into_iter().zip(values) {
        session
            .run("settings.set", json!({ "key": k, "value": v }))
            .expect("settings.set");
    }

    let first = session
        .run("session.poll_events", json!({ "limit": 2 }))
        .expect("poll 1");
    let second = session
        .run("session.poll_events", json!({ "limit": 2 }))
        .expect("poll 2");
    let third = session
        .run("session.poll_events", json!({ "limit": 2 }))
        .expect("poll 3");
    let fourth = session
        .run("session.poll_events", json!({ "limit": 2 }))
        .expect("poll 4");

    let keys_of = |v: &Value| -> Vec<String> {
        v["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["key"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(first["count"], json!(2));
    assert_eq!(keys_of(&first), vec!["theme", "inlay_hints"]);
    assert_eq!(second["count"], json!(2));
    assert_eq!(keys_of(&second), vec!["max_line_chars", "font_scale"]);
    assert_eq!(third["count"], json!(1));
    assert_eq!(keys_of(&third), vec!["context_sections"]);
    assert_eq!(fourth["count"], json!(0));

    let bad = session
        .run("session.poll_events", json!({ "limit": -1 }))
        .expect_err("negative limit");
    assert_eq!(bad.class, Class::Usage);
}

#[test]
fn recent_document_records_navigation_deduplicates_and_cycles_both_ways_across_three_docs() {
    let host = MemoryHost::default();
    let mut docs: Vec<Document> = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text))
        .collect();
    docs.push(Document::new(
        "contents/01-02.cnov",
        "@01-02-01 二話\n二話の本文。\n",
    ));
    docs.push(Document::new(
        "contents/02-01.cnov",
        "@02-01-01 二章\n二章の本文。\n",
    ));
    let mut session = Session::with_documents(Box::new(host), docs);

    // A -> B -> C の順に開くので MRU は [C, B, A]
    session
        .run("nav.section", json!({ "scene": "01-01-01-01" }))
        .expect("open A");
    session
        .run("nav.section", json!({ "scene": "01-02-01-01" }))
        .expect("open B");
    session
        .run("nav.section", json!({ "scene": "02-01-01-01" }))
        .expect("open C");

    // recent-next（Ctrl+Tab = back: false）で B -> A -> C -> B と一周以上循環
    let mut forward_paths = Vec::new();
    for _ in 0..4 {
        let step = session.command("recent-next").expect("recent-next")["data"].clone();
        assert_eq!(step["moved"], json!(true));
        forward_paths.push(step["path"].as_str().unwrap().to_owned());
    }
    assert_eq!(
        forward_paths,
        vec![
            "contents/01-02.cnov",
            "contents/01-01.cnov",
            "contents/02-01.cnov",
            "contents/01-02.cnov",
        ]
    );

    // recent-previous（Ctrl+Shift+Tab = back: true）で C -> A -> B と逆順に循環
    let mut backward_paths = Vec::new();
    for _ in 0..3 {
        let step = session.command("recent-previous").expect("recent-previous")["data"].clone();
        assert_eq!(step["moved"], json!(true));
        backward_paths.push(step["path"].as_str().unwrap().to_owned());
    }
    assert_eq!(
        backward_paths,
        vec![
            "contents/02-01.cnov",
            "contents/01-01.cnov",
            "contents/01-02.cnov",
        ]
    );

    // 通常の移動で A を開き直すと重複除去され [A, B, C] に更新される
    session
        .run("nav.section", json!({ "scene": "01-01-01-01" }))
        .expect("reopen A");
    let step1 = session.command("recent-next").expect("step1")["data"].clone();
    let step2 = session.command("recent-next").expect("step2")["data"].clone();
    let step3 = session.command("recent-next").expect("step3")["data"].clone();
    assert_eq!(step1["path"], json!("contents/01-02.cnov"));
    assert_eq!(step2["path"], json!("contents/02-01.cnov"));
    assert_eq!(step3["path"], json!("contents/01-01.cnov"));
}

#[test]
fn settings_set_rejects_unknown_wrong_type_and_out_of_range_without_side_effects() {
    let (mut session, files) = open();
    let _ = session.poll_events();
    let before_settings = session.run("settings.get", json!({})).expect("get before");
    let before_ui_json = files.borrow().written.get(".novnow/ui.json").cloned();

    let invalid_cases = [
        json!({ "key": "unknown_key", "value": 1 }),
        json!({ "key": "schema_version", "value": 2 }),
        json!({ "key": "reference_visible", "value": true }),
        json!({ "key": "font_scale", "value": "big" }),
        json!({ "key": "font_scale", "value": 0.5 }),
        json!({ "key": "font_scale", "value": 1.8 }),
        json!({ "key": "inlay_hints", "value": "true" }),
        json!({ "key": "autosave_ms", "value": 100 }),
        json!({ "key": "autosave_ms", "value": 5000 }),
        json!({ "key": "max_line_chars", "value": 10 }),
        json!({ "key": "max_line_chars", "value": 44.5 }),
        json!({ "key": "context_sections", "value": 0 }),
        json!({ "key": "context_sections", "value": 11 }),
        json!({ "key": "pane_ratio.main", "value": 0.99 }),
        json!({ "key": "theme", "value": "sepia" }),
        json!({ "key": "language", "value": "fr" }),
        json!({ "key": "reference_side", "value": "top" }),
    ];
    for case in invalid_cases {
        let err = session
            .run("settings.set", case.clone())
            .expect_err(&format!("不正入力が通った: {case}"));
        assert_eq!(err.class, Class::Usage, "{case}: {err:?}");
    }
    assert_eq!(
        session.run("settings.get", json!({})).expect("get after"),
        before_settings,
        "失敗した settings.set がメモリ上の設定を変えた"
    );
    assert_eq!(
        files.borrow().written.get(".novnow/ui.json").cloned(),
        before_ui_json,
        "失敗した settings.set が ui.json を書き換えた"
    );
    assert!(
        session.poll_events().is_empty(),
        "失敗した settings.set がイベントを発行した"
    );

    // 有効な変更では set の応答値 == settings.get の値 == 永続化値 かつイベントは 1 件
    let set_scale = session
        .run(
            "settings.set",
            json!({ "key": "font_scale", "value": 1.25 }),
        )
        .expect("valid font_scale");
    let got = session.run("settings.get", json!({})).expect("get");
    let persisted: Value = serde_json::from_str(
        files
            .borrow()
            .written
            .get(".novnow/ui.json")
            .expect("ui.json"),
    )
    .expect("json");
    assert_eq!(set_scale["value"], got["settings"]["font_scale"]);
    assert_eq!(set_scale["value"], persisted["font_scale"]);
    let events = session.poll_events();
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].to_json()["key"], json!("font_scale"));

    // language 変更でもイベントが重複せず 1 件だけ出る
    session
        .run("settings.set", json!({ "key": "language", "value": "en" }))
        .expect("valid language");
    let lang_events = session.poll_events();
    assert_eq!(lang_events.len(), 1, "{lang_events:?}");
}

#[test]
fn clean_external_reload_and_unchanged_never_populate_conflicts() {
    let (mut session, files) = open();
    let scene = open_first_section(&mut session);
    let path = "contents/01-01.cnov";
    let original = session.aggregate().text(path).to_owned();

    // 1) Unchanged: 同一内容の外部通知では衝突は空のまま
    files
        .borrow_mut()
        .changes
        .push((path.to_owned(), Some(original.clone())));
    let unchanged = session.poll_external();
    assert_eq!(unchanged.len(), 1);
    assert!(session.conflicts().is_empty());
    assert_eq!(
        session.run("save.state", json!({})).unwrap()["conflicts"],
        json!([])
    );

    // 2) Clean 外部変更（Reloaded）: 自動再読込され、全衝突公開面が空のまま
    let external_v1 = format!("{original}外部からの追記\n");
    files
        .borrow_mut()
        .changes
        .push((path.to_owned(), Some(external_v1.clone())));
    let reloaded = session.poll_external();
    assert_eq!(reloaded.len(), 1);
    assert!(
        session.conflicts().is_empty(),
        "Reloaded なのに Session::conflicts に入った"
    );
    let state = session.run("save.state", json!({})).unwrap();
    assert_eq!(state["conflicts"], json!([]), "{state}");
    for op in [
        "save.keep_mine",
        "save.reload_external",
        "save.diff_external",
    ] {
        let err = session.run(op, json!({})).expect_err(op);
        assert_eq!(err.class, Class::Conflict, "{op}: {err:?}");
    }

    // 3) Dirty 外部変更でのみ全公開面に同じ衝突が現れ、解決後は全てから消える
    session
        .run(
            "section.write",
            json!({ "scene": scene, "text": "自分の未保存編集\n" }),
        )
        .expect("dirty edit");
    let external_v2 = format!("{external_v1}さらに外部変更\n");
    files
        .borrow_mut()
        .changes
        .push((path.to_owned(), Some(external_v2)));
    session.poll_external();
    assert!(session.conflicts().contains_key(path));
    assert_eq!(
        session.run("save.state", json!({})).unwrap()["conflicts"],
        json!([path])
    );

    session
        .run("save.reload_external", json!({}))
        .expect("resolve");
    assert!(session.conflicts().is_empty());
    assert_eq!(
        session.run("save.state", json!({})).unwrap()["conflicts"],
        json!([])
    );
}
