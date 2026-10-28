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
