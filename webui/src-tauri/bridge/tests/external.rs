//! `poll_external`（外部変更の監視 1 回分。§8.5・設計 §21.2）。
//!
//! 画面はこのコマンドを 1 秒ごとに呼ぶ。戻り値は Host が見た差分
//! （`Session::poll_external` の戻り値）で、セッションの反応（再読込・衝突・変化なし）は
//! `poll_events` の出来事に出る。

mod common;

use serde_json::{json, Value};

use common::{ok, parse, sample, text, TempDir};
use novnow_native_bridge::NativeSession;

const MANUSCRIPT: &str = "contents/01-04.cnov";
const SCENE: &str = "01-04-01-01";

/// 開いたサンプル作品。
fn opened(dir: &TempDir) -> (NativeSession, std::path::PathBuf) {
    let root = sample(dir, "sample");
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));
    (session, root)
}

/// 節の本文を書く（保存はしない）。
fn write(session: &mut NativeSession, body: &str) {
    let args = json!({ "scene": SCENE, "text": body, "role": "" });
    ok(&session.run("section.write", &args.to_string()));
}

/// 溜まった出来事のうち、`path` の `document_changed` の `change` の並び。
fn document_changes(session: &mut NativeSession, path: &str) -> Vec<String> {
    parse(&session.poll_events())
        .as_array()
        .expect("array")
        .iter()
        .filter(|event| event["kind"] == json!("document_changed") && event["path"] == json!(path))
        .filter_map(|event| event["change"].as_str().map(str::to_owned))
        .collect()
}

/// 外部で足した 1 行を付けて書き直す（長さが変わるので、更新時刻の粒度に依らず見つかる）。
fn edit_outside(root: &std::path::Path, path: &str, line: &str) -> String {
    let file = root.join(path);
    let mut found = std::fs::read_to_string(&file).expect("read");
    found.push_str(line);
    std::fs::write(&file, &found).expect("write");
    found
}

/// 何も変わっていなければ 0 件（出来事も積まない）。
#[test]
fn nothing_changed_means_no_changes() {
    let dir = TempDir::new("external-none");
    let (mut session, _root) = opened(&dir);
    assert_eq!(
        session.poll_external(),
        "{\"ok\":true,\"result\":{\"changes\":[],\"count\":0}}"
    );
    assert_eq!(session.poll_events(), "[]");
}

/// 未保存の変更が無いファイルが外で変わったら、読み直す（§8.5 の Clean）。
#[test]
fn an_external_edit_is_reloaded_when_clean() {
    let dir = TempDir::new("external-clean");
    let (mut session, root) = opened(&dir);
    let edited = edit_outside(&root, MANUSCRIPT, "// 外から足した行\n");

    assert_eq!(
        session.poll_external(),
        format!(
            "{{\"ok\":true,\"result\":{{\"changes\":[{{\"change\":\"updated\",\"path\":\"{MANUSCRIPT}\"}}],\"count\":1}}}}"
        ),
        "本文は載せない（画面は取り直す）"
    );
    assert_eq!(document_changes(&mut session, MANUSCRIPT), ["external"]);
    let document = ok(&session.run("document.get", &json!({ "path": MANUSCRIPT }).to_string()));
    assert_eq!(document["text"], json!(edited), "読み直していない");
    // 1 回分で取り込み終わる
    assert_eq!(ok(&session.poll_external())["count"], json!(0));
}

/// 未保存の変更があるファイルが外で変わったら、自動では反映せず衝突にする（§8.5 の Dirty）。
#[test]
fn an_external_edit_is_a_conflict_when_dirty() {
    let dir = TempDir::new("external-dirty");
    let (mut session, root) = opened(&dir);
    write(&mut session, "未保存の本文。");
    parse(&session.poll_events());
    edit_outside(&root, MANUSCRIPT, "// 外から足した行\n");

    let changes = ok(&session.poll_external());
    assert_eq!(
        changes["changes"],
        json!([{ "change": "updated", "path": MANUSCRIPT }])
    );
    let events = parse(&session.poll_events());
    let kinds: Vec<(&str, &str)> = events
        .as_array()
        .expect("array")
        .iter()
        .map(|event| {
            let detail = event["change"].as_str().or(event["state"].as_str());
            (event["kind"].as_str().unwrap_or(""), detail.unwrap_or(""))
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("document_changed", "conflict"),
            ("save_state_changed", "conflict")
        ],
        "{events}"
    );
    let state = ok(&session.run("save.state", "{}"));
    assert_eq!(state["conflicts"], json!([MANUSCRIPT]), "{state}");
    // 自分の版はそのまま（外部の版で上書きしない）
    let mine = ok(&session.run("section.get", &json!({ "scene": SCENE }).to_string()));
    assert_eq!(mine["text"], json!("未保存の本文。"), "{mine}");
}

/// 消えたファイルは `removed`。読めなかったファイルは `unreadable`（削除ではないので文書は残る）。
#[test]
fn a_removal_and_an_unreadable_file_are_told_apart() {
    let dir = TempDir::new("external-removed");
    let (mut session, root) = opened(&dir);
    let words = ok(&session.run("document.get", "{\"path\":\"words.md\"}"))["text"].clone();

    // UTF-8 として読めない（§4.3 の encoding → S024）
    std::fs::write(root.join("words.md"), [0xff, 0xfe, 0x00, 0x41]).expect("bad bytes");
    let changes = ok(&session.poll_external());
    assert_eq!(changes["count"], json!(1), "{changes}");
    let unreadable = &changes["changes"][0];
    assert_eq!(unreadable["change"], json!("unreadable"), "{changes}");
    assert_eq!(unreadable["code"], json!("S024"), "{changes}");
    assert_eq!(unreadable["path"], json!("words.md"), "{changes}");
    assert!(
        unreadable["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "{changes}"
    );
    let events = parse(&session.poll_events());
    assert_eq!(events[0]["kind"], json!("external_unreadable"), "{events}");
    assert_eq!(events[0]["code"], json!("S024"), "{events}");
    assert_eq!(
        ok(&session.run("document.get", "{\"path\":\"words.md\"}"))["text"],
        words,
        "読めなかっただけで文書が変わった"
    );

    // 本当に消えたファイルだけが `removed`
    std::fs::remove_file(root.join("ideas.md")).expect("remove");
    let changes = ok(&session.poll_external());
    assert_eq!(
        changes["changes"],
        json!([{ "change": "removed", "path": "ideas.md" }]),
        "{changes}"
    );
    assert_eq!(document_changes(&mut session, "ideas.md"), ["external"]);
}

/// 自分の保存は外部変更として扱わない: 保存した直後（未保存の変更が無い間）に監視が回っても、
/// 衝突にも再読込にもならない。
#[test]
fn a_save_polled_while_clean_is_not_a_change_of_the_document() {
    let dir = TempDir::new("external-own-clean");
    let (mut session, _root) = opened(&dir);
    write(&mut session, "一回目の本文。");
    ok(&session.run("save.now", "{}"));
    parse(&session.poll_events());

    ok(&session.poll_external());
    let reactions = document_changes(&mut session, MANUSCRIPT);
    assert!(
        reactions.iter().all(|change| change == "unchanged"),
        "自分の保存に反応した: {reactions:?}"
    );
    let state = ok(&session.run("save.state", "{}"));
    assert_eq!(state["conflicts"], json!([]), "{state}");
    assert_eq!(state["unsaved"], json!([]), "{state}");
    let mine = ok(&session.run("section.get", &json!({ "scene": SCENE }).to_string()));
    assert_eq!(mine["text"], json!("一回目の本文。"), "{mine}");
}

/// 2 つ以上のファイルにまたがる保存は、書いたスタンプを Host が覚える。次の打鍵の後に監視が
/// 回っても、自分の保存を衝突と取り違えない（§8.5・設計 §8.5 の B-116）。
#[test]
fn a_two_file_save_is_not_an_external_change() {
    let dir = TempDir::new("external-own-batch");
    let (mut session, _root) = opened(&dir);
    write(&mut session, "一回目の本文。");
    let words =
        json!({ "path": "words.md", "text": "# 星図\ndefinition: 古い星図。\n", "role": "" });
    ok(&session.run("document.write", &words.to_string()));
    let saved = ok(&session.run("save.now", "{}"));
    assert_eq!(saved["written"], json!([MANUSCRIPT, "words.md"]), "{saved}");
    write(&mut session, "二回目の本文（未保存）。");
    parse(&session.poll_events());

    assert_eq!(
        session.poll_external(),
        "{\"ok\":true,\"result\":{\"changes\":[],\"count\":0}}"
    );
    assert_eq!(session.poll_events(), "[]");
    assert_eq!(
        ok(&session.run("save.state", "{}"))["conflicts"],
        Value::Array(Vec::new())
    );
}

/// 1 ファイルだけの保存でも同じであるべき（設計 §8.5 の B-116「保存完了時に記録したスタンプと
/// 一致する外部通知は無視する」）。
///
/// **今は通らない**: ヘッドレスの `FsHost`（`REF:src/app/fs_host.rs`）の `write` が、書いた
/// スタンプを `Snapshot` に覚えさせない（GUI の Host `REF:src/ui/picker.rs` と 2 ファイル以上の
/// 保存は覚えさせる）。そのため「保存 → 続けて打鍵 → 監視」の順になると、自分の保存が外部変更
/// として届き、未保存の変更があるので衝突になる。`src/` は第 2 のために変えない決まりなので、
/// ここでは直さずに記録する（直ったら `#[ignore]` を外す）。
#[test]
#[ignore = "機能核の FsHost::write が自分の保存を Snapshot に覚えさせない（src/app/fs_host.rs）。直ったら外す"]
fn a_single_file_save_is_not_an_external_change() {
    let dir = TempDir::new("external-own-single");
    let (mut session, _root) = opened(&dir);
    write(&mut session, "一回目の本文。");
    let saved = ok(&session.run("save.now", "{}"));
    assert_eq!(saved["written"], json!([MANUSCRIPT]), "{saved}");
    write(&mut session, "二回目の本文（未保存）。");
    parse(&session.poll_events());

    assert_eq!(
        session.poll_external(),
        "{\"ok\":true,\"result\":{\"changes\":[],\"count\":0}}",
        "自分の保存を外部変更として拾った"
    );
    assert_eq!(session.poll_events(), "[]", "自分の保存で衝突になった");
    assert_eq!(
        ok(&session.run("save.state", "{}"))["conflicts"],
        Value::Array(Vec::new())
    );
}
