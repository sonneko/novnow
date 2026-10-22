//! Rust API（§19.2.1）の**実ファイルシステム**の入口。
//!
//! `REF:tests/api.rs` はメモリホストだけで回す（§19.2.1「テスト用の仮想プロジェクト」）ので、
//! 実ディレクトリを相手にする `Session::open` / `create` / `with_host` はここで見る。
//! 一時ディレクトリは `REF:tests/sandbox/mod.rs` が作り、`Drop` で消す。

mod sandbox;

use serde_json::json;

use novnow::app::error::Class;
use novnow::app::memory_host::MemoryHost;
use novnow::app::session::{CreateOptions, Interface, Session};
use sandbox::TempDir;

/// `create` → `open` の往復（§19.2.1 の入口。§14.2 の起動前チェックと同じ検証を通る）。
#[test]
fn create_writes_a_project_that_open_reads_back() {
    let dir = TempDir::new("api-create");
    let path = dir.join("星屑の時計");
    let mut session = Session::create(
        &path,
        CreateOptions {
            title: "星屑の時計".to_owned(),
            ..CreateOptions::default()
        },
    )
    .expect("create");
    assert_eq!(session.root(), Some(path.as_path()), "ルートが無い");
    let info = session
        .run("project.info", json!({}))
        .expect("project.info");
    assert_eq!(info["title"], json!("星屑の時計"), "{info}");
    assert!(path.join("project.json").exists(), "project.json が無い");
    assert!(
        path.join("contents/01-01.txt").exists(),
        "ひな形の本文が無い"
    );
    drop(session);

    // 開き直して、編集 → `save.now` が実ファイルへ届くこと（§19.1.6 の状態を持つ側）
    let mut session = Session::open(&path).expect("open");
    let list = session
        .run("section.list", json!({}))
        .expect("section.list");
    let scene = list["sections"][0]["id"].as_str().expect("id").to_owned();
    session
        .run("nav.section", json!({ "scene": scene.clone() }))
        .expect("nav.section");
    let before = std::fs::read_to_string(path.join("contents/01-01.txt")).expect("read");
    session
        .run(
            "section.write",
            json!({ "scene": scene.clone(), "text": "ディスクへ書く本文\n" }),
        )
        .expect("section.write");
    let during = std::fs::read_to_string(path.join("contents/01-01.txt")).expect("read");
    assert_eq!(before, during, "save.now の前に書いている（§19.1.6）");
    let saved = session.run("save.now", json!({})).expect("save.now");
    assert_eq!(saved["state"], json!("clean"), "{saved}");
    let after = std::fs::read_to_string(path.join("contents/01-01.txt")).expect("read");
    assert!(
        after.contains("ディスクへ書く本文"),
        "書いた本文がファイルに無い: {after}"
    );
}

/// プロジェクトの無いディレクトリは `project` 分類で断る（§14.2.4・§19.1.7）。
#[test]
fn opening_a_directory_without_a_project_is_a_project_error() {
    let dir = TempDir::new("api-open-empty");
    let error = match Session::open(dir.path()) {
        Ok(_) => panic!("空のディレクトリが開けた"),
        Err(error) => error,
    };
    assert_eq!(error.class, Class::Project, "{error:?}");
    assert_eq!(error.kind, "S022", "{error:?}");
}

/// `with_host` は注入したホストをそのまま使う（§19.2.1「ホストは差し替えられる」）。
///
/// 書き出し先を選んだ書き込み（§13.1.1 の特例の口）が注入したホストへ届くことで確かめる。
#[test]
fn with_host_takes_the_injected_host() {
    let host = MemoryHost::default();
    let files = host.files.clone();
    let mut session = Session::with_host(Box::new(host));
    assert_eq!(session.root(), None, "メモリホストにルートは無い");
    assert_eq!(session.host_mut().stamp(), "20261013-2140", "ホストが違う");
    session
        .run("export.text", json!({ "out": "manuscript.txt" }))
        .expect("export.text");
    let exported = files.borrow().exported.clone();
    assert_eq!(exported.len(), 1, "注入したホストへ書かれていない");
    let (path, _) = exported.iter().next().expect("exported");
    assert!(
        path.ends_with("manuscript.txt"),
        "選んだ先へ書いていない: {exported:?}"
    );
}

/// インターフェースの指定は入口で受け取る（§19.1.6 の状態性の宣言）。
#[test]
fn open_with_declares_the_interface() {
    let dir = TempDir::new("api-open-with");
    let path = dir.join("星屑の時計");
    Session::create(&path, CreateOptions::default()).expect("create");
    let mut session = Session::open_with(&path, Interface::Cli).expect("open_with");
    // 状態を持たないインターフェース（CLI）は操作のたびに書き出す（§19.1.6）
    let list = session
        .run("section.list", json!({}))
        .expect("section.list");
    let scene = list["sections"][0]["id"].as_str().expect("id").to_owned();
    session
        .run(
            "section.write",
            json!({ "scene": scene, "text": "その場で書き出す本文\n" }),
        )
        .expect("section.write");
    let text = std::fs::read_to_string(path.join("contents/01-01.txt")).expect("read");
    assert!(
        text.contains("その場で書き出す本文"),
        "CLI の 1 実行で書き出されていない: {text}"
    );
}
