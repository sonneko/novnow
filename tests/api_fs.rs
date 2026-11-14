//! Rust API（§19.2.1）の**実ファイルシステム**の入口。
//!
//! `REF:tests/api.rs` はメモリホストだけで回す（§19.2.1「テスト用の仮想プロジェクト」）ので、
//! 実ディレクトリを相手にする `Session::open` / `create` / `with_host` はここで見る。
//! 一時ディレクトリは `REF:tests/sandbox/mod.rs` が作り、`Drop` で消す。

mod sandbox;

use serde_json::{json, Value};

use novnow::adapters::project_load::is_document;
use novnow::app::error::Class;
use novnow::app::event::AppEvent;
use novnow::app::host::ReadFailure;
use novnow::app::memory_host::MemoryHost;
use novnow::app::session::{CreateOptions, Interface, Session};
use novnow::core::model::document::Document;
use novnow::core::policy::ProjectPath;
use novnow::core::template;
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
    assert!(path.join("project.toml").exists(), "project.toml が無い");
    assert!(
        path.join("contents/01-01.cnov").exists(),
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
    let before = std::fs::read_to_string(path.join("contents/01-01.cnov")).expect("read");
    session
        .run(
            "section.write",
            json!({ "scene": scene.clone(), "text": "ディスクへ書く本文\n" }),
        )
        .expect("section.write");
    let during = std::fs::read_to_string(path.join("contents/01-01.cnov")).expect("read");
    assert_eq!(before, during, "save.now の前に書いている（§19.1.6）");
    let saved = session.run("save.now", json!({})).expect("save.now");
    assert_eq!(saved["state"], json!("clean"), "{saved}");
    let after = std::fs::read_to_string(path.join("contents/01-01.cnov")).expect("read");
    assert!(
        after.contains("ディスクへ書く本文"),
        "書いた本文がファイルに無い: {after}"
    );
}

/// プロジェクトの無いディレクトリは `project` 分類で断る（§4.3・§19.1.7）。
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
/// 雛形のプロジェクトを入れる: 当てはまる節が無い書き出しは**何も書かない**（§17.2）ので、
/// 空のプロジェクトでは注入が効いているか確かめられない。
#[test]
fn with_host_takes_the_injected_host() {
    let host = MemoryHost::default();
    let files = host.files.clone();
    let documents = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text))
        .collect();
    let mut session = Session::with_documents(Box::new(host), documents);
    assert_eq!(session.root(), None, "メモリホストにルートは無い");
    assert_eq!(session.host_mut().stamp(), "20261013-2140", "ホストが違う");
    let exported_now = session
        .run("export.text", json!({ "out": "manuscript.txt" }))
        .expect("export.text");
    assert!(
        exported_now["sections"].as_u64().expect("sections") > 0,
        "雛形の節が 1 つも選ばれていない: {exported_now}"
    );
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
    let text = std::fs::read_to_string(path.join("contents/01-01.cnov")).expect("read");
    assert!(
        text.contains("その場で書き出す本文"),
        "CLI の 1 実行で書き出されていない: {text}"
    );
}

// =========================================================================== Issue #61 回帰検査

/// 実ディスクでも 2 ファイル以上の保存は**原子的**（§8.3・§19.1.6）。
///
/// 途中の書き込みが失敗したら、成功していた書き込みも元へ戻り、新しいファイルも残らない。
/// 失敗は一時ファイル（`.<name>.novnow-tmp`）の場所をディレクトリでふさいで起こす
/// （`REF:tests/storage_fs.rs` の `structural_batch_rolls_back_already_written_files_on_mid_batch_failure`
/// と同じ手口）。
#[test]
fn a_failed_multi_file_save_on_disk_restores_every_file() {
    let dir = TempDir::new("api-atomic-save");
    let root = dir.join("星屑の時計");
    Session::create(&root, CreateOptions::default()).expect("create");
    let mut session = Session::open(&root).expect("open");

    let list = session
        .run("section.list", json!({}))
        .expect("section.list");
    let scene = list["sections"][0]["id"].as_str().expect("id").to_owned();
    session
        .run("nav.section", json!({ "scene": scene.clone() }))
        .expect("nav.section");
    let manuscript = root.join("contents/01-01.cnov");
    let characters = root.join("characters.md");
    let before_manuscript = std::fs::read(&manuscript).expect("read");
    let before_characters = std::fs::read(&characters).expect("read");

    // 束を 2 ファイル以上にする: 本文の書き換え + 資料のリネーム（書き 1・消し 1）
    session
        .run(
            "section.write",
            json!({ "scene": scene, "text": "ディスクへは書けない本文\n" }),
        )
        .expect("section.write");
    session
        .run(
            "document.rename",
            json!({ "from": "characters.md", "to": "cast.md" }),
        )
        .expect("document.rename");

    // `cast.md` の一時ファイルをふさぐ（リネームの先が書けない）
    std::fs::create_dir(root.join(".cast.md.novnow-tmp")).expect("blocker");
    let error = session
        .run("save.now", json!({}))
        .expect_err("書けないのに保存が通った");
    assert_eq!(error.class, Class::Io, "{error:?}");

    // どのファイルも操作前のまま: 成功していた本文の書き込みも戻り、新しいファイルも残らない
    assert_eq!(
        std::fs::read(&manuscript).expect("read"),
        before_manuscript,
        "先に書けたファイルが戻っていない（部分コミット）"
    );
    assert_eq!(
        std::fs::read(&characters).expect("read"),
        before_characters,
        "characters.md が変わっている"
    );
    assert!(!root.join("cast.md").exists(), "消せなかった先が残っている");
    // 一時ファイルの散らかしもない（ふさいだディレクトリ以外）
    let leftovers: Vec<String> = std::fs::read_dir(&root)
        .expect("read_dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".novnow-tmp") && name != ".cast.md.novnow-tmp")
        .collect();
    assert!(
        leftovers.is_empty(),
        "一時ファイルが残っている: {leftovers:?}"
    );

    // セッションも嘘を言わない: 保存は失敗した状態・未保存のまま・消すものも残ったまま（§9.6）
    let state = session.run("save.state", json!({})).expect("save.state");
    assert_eq!(state["state"], json!("error"), "{state}");
    let unsaved: Vec<&str> = state["unsaved"]
        .as_array()
        .expect("unsaved")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    assert!(
        unsaved.contains(&"cast.md") && unsaved.contains(&"contents/01-01.cnov"),
        "失敗した書き込みを保存済みにしている: {state}"
    );

    // ふさぎを外すと、同じ束がそのまま書ける
    std::fs::remove_dir(root.join(".cast.md.novnow-tmp")).expect("unblock");
    let saved = session.run("save.now", json!({})).expect("save.now");
    assert_eq!(saved["state"], json!("clean"), "{saved}");
    assert!(
        root.join("cast.md").exists(),
        "リネームの先が書かれていない"
    );
    assert!(!characters.exists(), "リネームの元が消えていない: {saved}");
    let after = std::fs::read_to_string(&manuscript).expect("read");
    assert!(
        after.contains("ディスクへは書けない本文"),
        "書き直した本文が入っていない: {after}"
    );
}

/// 実ディスクで**読めなかった**外部変更は削除と区別する（§8.5・§4.3）。
///
/// 文字コード不正（S024）も読み取り権限の失敗（S030）も、利用者の削除操作ではないので
/// 文書を消してはいけない。本当に消えたファイルだけが文書から外れる。
#[test]
fn an_unreadable_file_on_disk_is_not_a_removal() {
    let dir = TempDir::new("api-unreadable");
    let root = dir.join("星屑の時計");
    Session::create(&root, CreateOptions::default()).expect("create");
    let mut session = Session::open(&root).expect("open");
    let manuscript = "contents/01-01.cnov";

    // 1) 文字コード不正（§4.3 の encoding → S024）
    std::fs::write(root.join(manuscript), [0xff, 0xfe, 0x00, 0x41]).expect("bad bytes");
    let changes = session.poll_external();
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].path(), manuscript, "{changes:?}");
    assert!(
        !changes[0].removed(),
        "読めなかったものを削除としている: {changes:?}"
    );
    assert_eq!(
        changes[0].failure().map(ReadFailure::kind),
        Some("S024"),
        "{changes:?}"
    );
    assert!(
        session.aggregate().document(manuscript).is_some(),
        "復号できなかっただけで文書が消えた"
    );
    let events: Vec<Value> = session
        .poll_events()
        .iter()
        .map(AppEvent::to_json)
        .collect();
    let found = events
        .iter()
        .find(|event| event["kind"] == json!("external_unreadable"))
        .unwrap_or_else(|| panic!("S024 の出来事が無い: {events:?}"));
    assert_eq!(found["code"], json!("S024"), "{found}");
    assert_eq!(found["path"], json!(manuscript), "{found}");

    // 2) 本当に消えたファイルだけが文書から外れる（区別の反対側）
    std::fs::remove_file(root.join(manuscript)).expect("remove");
    let changes = session.poll_external();
    let change = changes
        .iter()
        .find(|change| change.path() == manuscript)
        .unwrap_or_else(|| panic!("削除の変更が無い: {changes:?}"));
    assert!(change.removed(), "{change:?}");
    assert!(change.failure().is_none(), "{change:?}");
    assert!(
        session.aggregate().document(manuscript).is_none(),
        "消えた文書が残っている"
    );
}

/// 読み取り権限の失敗（§4.3 の I/O → S030）も削除とは区別する。権限の作り方は OS 依存なので
/// Unix だけ（`REF:tests/cli.rs` と同じ流儀）。
#[test]
#[cfg(unix)]
fn a_file_without_read_permission_is_not_a_removal() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new("api-unreadable-io");
    let root = dir.join("星屑の時計");
    Session::create(&root, CreateOptions::default()).expect("create");
    let mut session = Session::open(&root).expect("open");

    let words = root.join("words.md");
    std::fs::write(&words, "# 星図\ndefinition: 古い星図。\n").expect("write");
    std::fs::set_permissions(&words, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    let changes = session.poll_external();
    let change = changes
        .iter()
        .find(|change| change.path() == "words.md")
        .unwrap_or_else(|| panic!("words.md の変更が無い: {changes:?}"));
    assert!(
        !change.removed(),
        "読めなかったものを削除としている: {change:?}"
    );
    assert_eq!(
        change.failure().map(ReadFailure::kind),
        Some("S030"),
        "{change:?}"
    );
    assert!(
        session.aggregate().document("words.md").is_some(),
        "権限が無かっただけで文書が消えた"
    );
    let events: Vec<Value> = session
        .poll_events()
        .iter()
        .map(AppEvent::to_json)
        .collect();
    let found = events
        .iter()
        .find(|event| event["kind"] == json!("external_unreadable"))
        .unwrap_or_else(|| panic!("S030 の出来事が無い: {events:?}"));
    assert_eq!(found["code"], json!("S030"), "{found}");
    assert_eq!(found["path"], json!("words.md"), "{found}");
    // 権限を戻すと読めるようになる（文書は残っていたので中身が更新される）
    std::fs::set_permissions(&words, std::fs::Permissions::from_mode(0o644)).expect("chmod back");
    std::fs::write(&words, "# 星図\ndefinition: 読み直せた星図。\n").expect("write again");
    let changes = session.poll_external();
    let change = changes
        .iter()
        .find(|change| change.path() == "words.md")
        .unwrap_or_else(|| panic!("words.md の変更が無い: {changes:?}"));
    assert!(change.failure().is_none(), "{change:?}");
    assert_eq!(
        change.text().map(str::to_owned),
        Some("# 星図\ndefinition: 読み直せた星図。\n".to_owned()),
        "{change:?}"
    );
}
