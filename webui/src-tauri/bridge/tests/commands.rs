//! コマンド 4 つの形（設計 §21.4）。ブラウザ用の橋渡し
//! （`REF:crates/novnow-wasm/src/lib.rs`）と同じ JSON を返すことを、Rust 側から確かめる。
//!
//! ブラウザ版との突き合わせそのもの（同じ操作列 → 同じ戻り値・同じファイル）は
//! `REF:webui/tests/l6.test.mjs` が wasm を相手に行う。

mod common;

use serde_json::{json, Value};

use common::{err, ok, parse, sample, text, tree, TempDir};
use novnow::app::session::Session;
use novnow_native_bridge::NativeSession;

/// 開く前は、ブラウザ版と同じ文言で断る（例外にしない）。
#[test]
fn before_open_the_commands_answer_like_the_browser_bridge() {
    let mut session = NativeSession::new();
    assert_eq!(
        session.run("project.info", "{}"),
        "{\"ok\":false,\"error\":{\"code\":\"usage\",\"kind\":\"\",\
         \"message\":\"session is not open\",\"path\":null}}"
    );
    assert_eq!(session.poll_events(), "[]");
    assert_eq!(
        session.poll_external(),
        "{\"ok\":true,\"result\":{\"changes\":[],\"count\":0}}"
    );
}

/// `open` は `run` と同じ形で、橋渡しの同期の口が要る 3 つ（`locale` / `root` / `version`）を返す。
#[test]
fn open_reports_root_locale_and_version() {
    let dir = TempDir::new("open");
    let root = sample(&dir, "星屑の時計");
    let before = tree(&root);
    let mut session = NativeSession::new();
    let opened = session.open(&text(&root), None);
    assert_eq!(
        opened,
        format!(
            "{{\"ok\":true,\"result\":{{\"locale\":\"ja\",\"root\":{},\"version\":\"{}\"}}}}",
            Value::String(text(&root)),
            Session::version()
        )
    );
    // `create_options` の空文字・`null` も「既存を開く」
    assert_eq!(session.open(&text(&root), Some("")), opened);
    assert_eq!(session.open(&text(&root), Some("null")), opened);
    drop(session);
    assert_eq!(tree(&root), before, "開いただけでファイルが変わった");
}

/// 戻り値は `Session::run` の JSON を包んだだけ（§19.1.4 の単一実装点。二重に包まない）。
#[test]
fn run_wraps_the_json_of_the_session() {
    let dir = TempDir::new("run");
    let root = sample(&dir, "sample");
    let mut direct = Session::open(&root).expect("open");
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));
    let calls = [
        ("project.info", json!({})),
        ("section.list", json!({})),
        ("structure.breadcrumb", json!({})),
        ("section.get", json!({ "scene": "01-04-01-01" })),
        ("settings.get", json!({})),
        ("save.state", json!({})),
        ("project.validate", json!({})),
        ("api.list", json!({})),
    ];
    for (op, args) in calls {
        let expected = direct.run(op, args.clone()).expect(op);
        let expected = format!(
            "{{\"ok\":true,\"result\":{}}}",
            serde_json::to_string(&expected).expect("json")
        );
        assert_eq!(session.run(op, &args.to_string()), expected, "{op}");
    }
    // 引数は省ける（空文字・`null` は空のオブジェクト）
    let info = session.run("project.info", "{}");
    assert_eq!(session.run("project.info", ""), info);
    assert_eq!(session.run("project.info", "null"), info);
    // 同じ操作は同じバイト列（キーの順も含めて。§19.1.4）
    assert_eq!(session.run("api.list", "{}"), session.run("api.list", "{}"));
}

/// 失敗は機能核の `Error::to_json` のまま返る（`code` / `kind` / `message` / `path`）。
#[test]
fn failures_keep_the_shape_of_the_core() {
    let dir = TempDir::new("errors");
    let root = sample(&dir, "sample");
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));

    let unknown = session.run("no.such", "{}");
    let mut direct = Session::open(&root).expect("open");
    let expected = direct.run("no.such", json!({})).expect_err("unknown op");
    assert_eq!(
        unknown,
        format!("{{\"ok\":false,\"error\":{}}}", expected.to_json())
    );
    assert_eq!(err(&unknown)["code"], json!("usage"));

    // 引数の誤りはブラウザ版と同じ文言
    let array = err(&session.run("project.info", "[1]"));
    assert_eq!(
        array,
        json!({ "code": "usage", "kind": "", "message": "arguments must be a JSON object", "path": null })
    );
    let broken = err(&session.run("project.info", "{"));
    assert_eq!(broken["code"], json!("usage"));
    assert!(broken["message"]
        .as_str()
        .is_some_and(|text| !text.is_empty()));

    // 無い文書は not_found（キーは 4 つとも揃っている）
    let missing = err(&session.run("document.get", "{\"path\":\"nope.md\"}"));
    assert_eq!(missing["code"], json!("not_found"), "{missing}");
    assert_eq!(missing["path"], json!("nope.md"), "{missing}");
    let keys: Vec<&str> = missing
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["code", "kind", "message", "path"]);
}

/// イベントは `AppEvent` の配列（ブラウザ版と同じ直列化。取り出したら空になる）。
#[test]
fn events_are_drained_in_order() {
    let dir = TempDir::new("events");
    let root = sample(&dir, "sample");
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));
    assert_eq!(session.poll_events(), "[]");
    let written = ok(&session.run(
        "section.write",
        "{\"scene\":\"01-04-01-01\",\"text\":\"書き換えた本文。\",\"role\":\"\"}",
    ));
    assert_eq!(written["save"]["state"], json!("dirty"), "{written}");
    let saved = ok(&session.run("save.now", "{}"));
    assert_eq!(saved["written"], json!(["contents/01-04.cnov"]), "{saved}");
    let raw = session.poll_events();
    let events = parse(&raw);
    let kinds: Vec<&str> = events
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|event| event["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"document_changed"), "{raw}");
    assert_eq!(kinds.last(), Some(&"save_state_changed"), "{raw}");
    // キーの順は宣言順（`kind` が先頭。`REF:src/app/event.rs`）で、ブラウザ版と同じ文字列になる
    assert!(
        raw.ends_with("{\"kind\":\"save_state_changed\",\"state\":\"clean\",\"message\":\"\"}]"),
        "{raw}"
    );
    assert_eq!(session.poll_events(), "[]", "取り出した後は空");
}

/// 開けなかったら、開いていたセッションはそのまま残る。開けたら差し替わる。
#[test]
fn open_replaces_the_session_only_on_success() {
    let dir = TempDir::new("reopen");
    let first = sample(&dir, "first");
    let second = dir.join("second");
    let mut session = NativeSession::new();
    ok(&session.open(&text(&first), None));
    ok(&session.open(&text(&second), Some("{\"title\":\"二作目\"}")));
    assert_eq!(
        ok(&session.run("project.info", "{}"))["title"],
        json!("二作目")
    );

    // 無いディレクトリ: io。必須のものが無いディレクトリ: project（S022）
    let missing = err(&session.open(&text(&dir.join("nowhere")), None));
    assert_eq!(missing["code"], json!("io"), "{missing}");
    assert_eq!(missing["path"], json!(text(&dir.join("nowhere"))));
    std::fs::create_dir_all(dir.join("empty")).expect("mkdir");
    let incomplete = err(&session.open(&text(&dir.join("empty")), None));
    assert_eq!(incomplete["code"], json!("project"), "{incomplete}");
    assert_eq!(incomplete["kind"], json!("S022"), "{incomplete}");
    let blank = err(&session.open("  ", None));
    assert_eq!(blank["code"], json!("usage"), "{blank}");

    // 失敗の後も、開いていた「二作目」がそのまま使える
    assert_eq!(
        ok(&session.run("project.info", "{}"))["title"],
        json!("二作目")
    );
    ok(&session.open(&text(&first), None));
    assert_eq!(
        ok(&session.run("project.info", "{}"))["title"],
        json!("星屑の時計")
    );
}
