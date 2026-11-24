//! 新規作成（設計 §21.4 の `open` の `create_options` と、正規操作 `project.new`）:
//! 新しく作るのは、無いディレクトリか空のディレクトリだけ。
//!
//! 機能核の `Session::create`（`project.new` もこれを呼ぶ）は雛形のファイルをそのまま書く。
//! 既にプロジェクトがある場所で呼ぶと `project.toml` や `characters.md` が雛形で置き換わるので、
//! 橋渡しが先に断る（§14.5 の「既存ファイルは絶対に上書きしない」）。

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

/// 正規操作 `project.new` も同じ規則で断る（機能核にそのまま渡すと、雛形で置き換わる）。
/// 断るのは既に何かある場所だけで、作れたときの戻り値は機能核のまま。
#[test]
fn project_new_refuses_an_occupied_directory_and_changes_nothing() {
    let dir = TempDir::new("create-op");
    let root = sample(&dir, "既存の作品");
    // 書き手が育てた辞書（雛形と違う中身）
    dir.write("既存の作品/characters.md", "# 私の人物\n");
    dir.write("メモだけ/memo.txt", "消してはいけないメモ\n");
    let before = tree(dir.path());
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));

    // 開いている作品にも、ほかのファイルがある場所にも作らない
    for occupied in [root.clone(), dir.join("メモだけ")] {
        let args = json!({ "path": text(&occupied), "title": "上書き" });
        let refused = err(&session.run("project.new", &args.to_string()));
        assert_eq!(refused["code"], json!("io"), "{refused}");
        assert_eq!(refused["path"], json!(text(&occupied)), "{refused}");
        assert!(
            refused["message"]
                .as_str()
                .is_some_and(|message| message.contains("AlreadyExists")),
            "{refused}"
        );
    }
    assert_eq!(
        differing(&before, &tree(dir.path())),
        Vec::<String>::new(),
        "断ったのにファイルが変わった"
    );

    // 無い場所には、機能核がそのまま作る（開いているセッションは替わらない）
    let fresh = dir.join("新作");
    let args = json!({ "path": text(&fresh), "title": "新作" });
    let created = ok(&session.run("project.new", &args.to_string()));
    assert_eq!(created["title"], json!("新作"), "{created}");
    assert_eq!(created["path"], json!(text(&fresh)), "{created}");
    assert!(fresh.join("project.toml").is_file(), "project.toml が無い");
    assert_eq!(
        ok(&session.run("project.info", "{}"))["title"],
        json!("星屑の時計")
    );
    // 作った直後の場所は、もう空ではない
    assert_eq!(
        err(&session.run("project.new", &args.to_string()))["code"],
        json!("io")
    );

    // 開く前は、ほかの操作と同じ答え（何も作らない）
    let mut closed = NativeSession::new();
    let args = json!({ "path": text(&dir.join("別の新作")) });
    assert_eq!(
        err(&closed.run("project.new", &args.to_string()))["message"],
        json!("session is not open")
    );
    assert!(!dir.join("別の新作").exists(), "開く前に作った");
}

/// 橋渡しが `path` を読まない形（無い・`null`・配列・オブジェクト）は、機能核が `usage` で断る。
/// 機能核がこれらを受け付けるようになったら、橋渡しの読み方（`create_target`）も合わせる。
#[test]
fn project_new_without_a_readable_path_is_refused_by_the_core() {
    let dir = TempDir::new("create-op-args");
    let root = sample(&dir, "既存の作品");
    let before = tree(dir.path());
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));
    for args in [
        "{}",
        "{\"path\":null}",
        "{\"path\":[\"a\"]}",
        "{\"path\":{\"a\":1}}",
        "{\"title\":\"題だけ\"}",
    ] {
        let refused = err(&session.run("project.new", args));
        assert_eq!(refused["code"], json!("usage"), "{args}: {refused}");
    }
    assert_eq!(
        differing(&before, &tree(dir.path())),
        Vec::<String>::new(),
        "断ったのにファイルが変わった"
    );
}

/// 確かめる場所は、機能核が実際に書く場所（同じ関数で解決する）。`無い/../既存` のように
/// 書いても、既存の作品を指していれば断る（素朴に `stat` すると「無い」に見える書き方）。
#[test]
fn the_occupied_check_resolves_the_path_like_the_core() {
    let dir = TempDir::new("create-resolve");
    let root = sample(&dir, "既存の作品");
    dir.write("既存の作品/characters.md", "# 私の人物\n");
    let before = tree(dir.path());
    let mut session = NativeSession::new();

    let detour = format!("{}/無い/../既存の作品", text(dir.path()));
    let trailing = format!("{}/./既存の作品/", text(dir.path()));
    for path in [detour.as_str(), trailing.as_str()] {
        assert_eq!(
            err(&session.open(path, Some("{}")))["code"],
            json!("io"),
            "{path}"
        );
    }
    ok(&session.open(&text(&root), None));
    for path in [detour.as_str(), trailing.as_str()] {
        let args = json!({ "path": path });
        assert_eq!(
            err(&session.run("project.new", &args.to_string()))["code"],
            json!("io"),
            "{path}"
        );
    }
    assert_eq!(
        differing(&before, &tree(dir.path())),
        Vec::<String>::new(),
        "断ったのにファイルが変わった"
    );

    // 指す先が無ければ断らない（`.` や末尾の `/` があっても、機能核と同じ場所に出来る）
    let fresh = format!("{}/./新作/", text(dir.path()));
    let opened = ok(&session.open(&fresh, Some("{}")));
    assert_eq!(opened["root"], json!(text(&dir.join("新作"))), "{opened}");
    assert!(dir.join("新作/project.toml").is_file());
}

/// 既存の作品を指すシンボリックリンクにも作らない（機能核はリンクを解いた先に書く）。
#[cfg(unix)]
#[test]
fn a_symlink_to_an_occupied_directory_is_refused() {
    let dir = TempDir::new("create-symlink");
    let root = sample(&dir, "既存の作品");
    dir.write("既存の作品/characters.md", "# 私の人物\n");
    std::fs::create_dir_all(dir.join("空の棚")).expect("mkdir");
    std::os::unix::fs::symlink(&root, dir.join("作品への近道")).expect("symlink");
    std::os::unix::fs::symlink(dir.join("空の棚"), dir.join("棚への近道")).expect("symlink");
    let before = tree(dir.path());
    let mut session = NativeSession::new();

    let link = text(&dir.join("作品への近道"));
    assert_eq!(err(&session.open(&link, Some("{}")))["code"], json!("io"));
    ok(&session.open(&text(&root), None));
    let args = json!({ "path": link });
    assert_eq!(
        err(&session.run("project.new", &args.to_string()))["code"],
        json!("io")
    );
    assert_eq!(
        differing(&before, &tree(dir.path())),
        Vec::<String>::new(),
        "断ったのにファイルが変わった"
    );

    // 空のディレクトリを指すリンクなら、その先に作る（リンクそのものは置き換えない）
    let opened = ok(&session.open(&text(&dir.join("棚への近道")), Some("{}")));
    assert_eq!(opened["root"], json!(text(&dir.join("空の棚"))), "{opened}");
    assert!(dir.join("空の棚/project.toml").is_file());
    assert!(std::fs::symlink_metadata(dir.join("棚への近道"))
        .expect("stat")
        .file_type()
        .is_symlink());
}
