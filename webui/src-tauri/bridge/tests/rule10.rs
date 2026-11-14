//! 規則 10（§13.1・§21.4.1）: ネイティブの橋渡しが書けるのは、開いているプロジェクトの配下と、
//! 利用者が明示した書き出し先 1 個だけ。
//!
//! 橋渡しは自分ではファイルを書かない（`REF:webui/tests/l6.test.mjs` が書き込み API の不在を
//! 見る）。ここでは**実際に動かして**、プロジェクトの外に何も増えないことを確かめる
//! （`REF:docs/verify-rule10.md` §3.2 のファイル差分と同じ考え方を、砂場の中で行う）。

mod common;

use serde_json::json;

use common::{differing, err, ok, parse, sample, text, tree, TempDir};
use novnow_native_bridge::NativeSession;

const SCENE: &str = "01-04-01-01";

/// 砂場（プロジェクトと、その外に置いた既存のファイル）。
fn sandbox(label: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new(label);
    let root = sample(&dir, "project");
    dir.write("outside/notes.txt", "プロジェクトの外のメモ\n");
    dir.write("neighbour/project.toml", "title = \"隣の作品\"\n");
    (dir, root)
}

/// 砂場のうち、プロジェクトの外で変わったパス。
fn changed_outside(
    dir: &TempDir,
    before: &std::collections::BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    differing(before, &tree(dir.path()))
        .into_iter()
        .filter(|path| !path.starts_with("project/"))
        .collect()
}

/// 一通りの操作（開く・打鍵・辞書とプロットの編集・設定・保存・書き出し・監視）のあと、
/// プロジェクトの外は 1 バイトも変わっていない。中にも一時ファイルを残さない。
#[test]
fn a_full_session_writes_only_inside_the_project() {
    let (dir, root) = sandbox("rule10-session");
    let before = tree(dir.path());
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));

    let calls = [
        (
            "section.write",
            json!({ "scene": SCENE, "text": "規則 10 の確認。\n二行目。", "role": "" }),
        ),
        (
            "document.write",
            json!({ "path": "words.md", "text": "# 星図\ndefinition: 古い星図。\n", "role": "" }),
        ),
        ("settings.set", json!({ "key": "theme", "value": "dark" })),
        ("save.now", json!({})),
        ("export.text", json!({})),
        ("project.validate", json!({})),
    ];
    for (op, args) in calls {
        ok(&session.run(op, &args.to_string()));
    }
    ok(&session.poll_external());
    parse(&session.poll_events());
    drop(session);

    assert_eq!(
        changed_outside(&dir, &before),
        Vec::<String>::new(),
        "プロジェクトの外が変わった"
    );
    let inside = tree(&root);
    assert!(
        inside.get("contents/01-04.cnov") != before.get("project/contents/01-04.cnov"),
        "本文が保存されていない"
    );
    assert!(
        inside.contains_key(".novnow/ui.json"),
        "設定が書かれていない"
    );
    let leftovers: Vec<&String> = inside
        .keys()
        .filter(|path| path.ends_with(".novnow-tmp"))
        .collect();
    assert!(leftovers.is_empty(), "一時ファイルが残った: {leftovers:?}");
}

/// 利用者が明示した書き出し先（§13.1.1 の特例）には、その 1 個だけを書く。
#[test]
fn an_explicit_export_writes_exactly_the_chosen_file() {
    let (dir, root) = sandbox("rule10-export");
    let before = tree(dir.path());
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));
    let expected = ok(&session.run("export.text", "{}"))["text"]
        .as_str()
        .expect("text")
        .to_owned();

    let out = dir.join("outside/書き出し.txt");
    let written = ok(&session.run(
        "export.text",
        &json!({ "out": text(&out), "role": "" }).to_string(),
    ));
    assert_eq!(written["out"], json!(text(&out)), "{written}");
    assert_eq!(
        std::fs::read_to_string(&out).expect("read"),
        expected,
        "書き出した中身が `export.text` の本文と違う"
    );
    assert_eq!(
        changed_outside(&dir, &before),
        ["outside/書き出し.txt"],
        "指定した 1 個のほかに外が変わった"
    );
    assert_eq!(
        tree(&root).len(),
        before
            .keys()
            .filter(|path| path.starts_with("project/"))
            .count()
    );
}

/// 書き出しの種類に合わない名前（E060）には書かない。
///
/// 関門はデバッグビルドで違反を `panic!` にする（§13.1.2）。橋渡しは panic を受け止めて
/// `io` の失敗を返し、セッションを捨てる（開き直せる）。リリースビルドでは E060 が返り、
/// セッションはそのまま残る。どちらでも**何も書かれない**。
#[test]
fn a_refused_export_writes_nothing() {
    let (dir, root) = sandbox("rule10-refused");
    let before = tree(dir.path());
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));

    let out = dir.join("outside/書き出し.pdf");
    let refused = err(&session.run(
        "export.text",
        &json!({ "out": text(&out), "role": "" }).to_string(),
    ));
    assert_eq!(refused["code"], json!("io"), "{refused}");
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|message| message.contains("NotAnExport")),
        "{refused}"
    );
    assert!(!out.exists(), "拒否したのにファイルができた");
    assert_eq!(
        differing(&before, &tree(dir.path())),
        Vec::<String>::new(),
        "拒否したのに何かが変わった"
    );

    let next = session.run("project.info", "{}");
    if cfg!(debug_assertions) {
        assert_eq!(
            err(&next)["message"],
            json!("session is not open"),
            "panic した操作の後のセッションが残っている"
        );
        // 持ち主のスレッドは生きているので、開き直せる
        ok(&session.open(&text(&root), None));
    }
    assert_eq!(
        ok(&session.run("project.info", "{}"))["title"],
        json!("星屑の時計")
    );
}

/// プロジェクトの外を指すパス（`..`）は保存できない。外には何も増えない。
#[test]
fn a_path_escaping_the_project_is_never_written() {
    let (dir, root) = sandbox("rule10-escape");
    let before = tree(dir.path());
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));

    // 機能核は書き込みの時点で断る（メモリには載るが、関門を通らない）
    let accepted = session.run(
        "document.write",
        &json!({ "path": "../outside/escape.md", "text": "外へ", "role": "" }).to_string(),
    );
    let saved = session.run("save.now", "{}");
    assert!(
        parse(&accepted)["ok"] == json!(false) || parse(&saved)["ok"] == json!(false),
        "外を指すパスが保存できた: {accepted} / {saved}"
    );
    drop(session);
    assert_eq!(
        differing(&before, &tree(dir.path())),
        Vec::<String>::new(),
        "外を指すパスで何かが変わった"
    );
}
