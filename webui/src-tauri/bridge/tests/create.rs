//! `open` の `create_options`（設計 §21.4）: 新しく作るのは、無いディレクトリか空のディレクトリだけ。
//!
//! 機能核の `Session::create` は雛形のファイルをそのまま書く。既にプロジェクトがある場所で
//! 呼ぶと `project.toml` や `characters.md` が雛形で置き換わるので、橋渡しが先に断る
//! （§14.5 の「既存のファイルは上書きしない」）。

mod common;

use serde_json::json;

use common::{differing, err, ok, sample, text, tree, TempDir};
use novnow_native_bridge::NativeSession;

/// 無いディレクトリには、親ごと作って開く（`novnow new` と同じ構造）。
#[test]
fn create_makes_a_new_project() {
    let dir = TempDir::new("create");
    let root = dir.join("棚/新作");
    let mut session = NativeSession::new();
    let opened = ok(&session.open(&text(&root), Some("{\"title\":\"星屑の時計\"}")));
    assert_eq!(opened["root"], json!(text(&root)), "{opened}");
    assert!(root.join("project.toml").is_file(), "project.toml が無い");
    let info = ok(&session.run("project.info", "{}"));
    assert_eq!(info["title"], json!("星屑の時計"), "{info}");
    // 作った直後は保存済み（雛形は関門を通って書かれている）
    let state = ok(&session.run("save.state", "{}"));
    assert_eq!(state["unsaved"], json!([]), "{state}");
}

/// 題を省くとディレクトリ名（`Session::create` の規則のまま）。空のディレクトリには作れる。
#[test]
fn create_accepts_an_empty_directory() {
    let dir = TempDir::new("create-empty");
    let root = dir.join("空の棚");
    std::fs::create_dir_all(&root).expect("mkdir");
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), Some("{}")));
    let info = ok(&session.run("project.info", "{}"));
    assert_eq!(info["title"], json!("空の棚"), "{info}");
}

/// 既に何かある場所には作らない。1 バイトも変えず、セッションも開かない。
#[test]
fn create_refuses_an_occupied_directory_and_changes_nothing() {
    let dir = TempDir::new("create-occupied");
    let root = sample(&dir, "既存の作品");
    let before = tree(dir.path());
    let mut session = NativeSession::new();

    let refused = err(&session.open(&text(&root), Some("{\"title\":\"上書き\"}")));
    assert_eq!(refused["code"], json!("io"), "{refused}");
    assert_eq!(refused["path"], json!(text(&root)), "{refused}");
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|message| message.contains("AlreadyExists")),
        "{refused}"
    );
    assert_eq!(
        differing(&before, &tree(dir.path())),
        Vec::<String>::new(),
        "断ったのにファイルが変わった"
    );
    assert_eq!(
        err(&session.run("project.info", "{}"))["message"],
        json!("session is not open")
    );

    // 1 個でもファイルがあれば断る。ファイルそのものを指しても断る
    let other = dir.join("メモだけ");
    dir.write("メモだけ/memo.txt", "消してはいけないメモ\n");
    assert_eq!(
        err(&session.open(&text(&other), Some("{}")))["code"],
        json!("io")
    );
    let file = dir.join("メモだけ/memo.txt");
    assert_eq!(
        err(&session.open(&text(&file), Some("{}")))["code"],
        json!("io")
    );
    assert_eq!(
        std::fs::read_to_string(&file).expect("read"),
        "消してはいけないメモ\n"
    );

    // 既存の作品は、`create_options` なしで開ける（中身はそのまま）
    ok(&session.open(&text(&root), None));
    assert_eq!(
        ok(&session.run("project.info", "{}"))["title"],
        json!("星屑の時計")
    );
}

/// `create_options` はオブジェクトで、値は文字列（§19.2.1 の `CreateOptions`）。
#[test]
fn create_options_must_be_an_object_of_strings() {
    let dir = TempDir::new("create-options");
    let root = dir.join("新作");
    let mut session = NativeSession::new();
    for options in ["[]", "1", "{", "{\"title\":1}"] {
        let refused = err(&session.open(&text(&root), Some(options)));
        assert_eq!(refused["code"], json!("usage"), "{options}: {refused}");
    }
    assert!(!root.exists(), "断ったのにディレクトリを作った");
}
