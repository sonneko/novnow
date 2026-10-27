//! セッションのワーカースレッド（§19.3.4）。
//!
//! `REF:src/app/driver.rs` は「`Session` をワーカースレッド 1 本のうえで所有し、呼び出し側は
//! `Send` だけのデータを渡して待つ」口。Python API（`REF:python/novnow-py/src/lib.rs`）は
//! この待機区間を `Python::allow_threads` で包んで GIL を解放する。つまり **GIL 解放が
//! 成り立つことの条件（`Driver: Send`・1 通 1 往復・直列化）はここで検証する**。
//! pyo3 の側（`#[pyclass]` の薄さ）だけが CI のホイール行列（`REF:.github/workflows/wheels.yml`）
//! 側の検証になる。

mod sandbox;

use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{json, Value};

use novnow::app::driver::Driver;
use novnow::app::error::Class;
use novnow::app::session::CreateOptions;
use sandbox::TempDir;

/// `Driver: Send`（§19.3.4「GIL を解放する」の前提）。
///
/// `Session` 自体は `Send` ではない（ホストが Web の `Rc` を前提にするので `Host: Send`
/// にできない）。`Driver` が `Send` であることが、待機区間に Python のオブジェクトを
/// 持ち込まないこと（pyo3 の `Ungil`）を満たす側の条件になる。
#[test]
fn the_driver_can_cross_threads_even_though_a_session_cannot() {
    fn assert_send<T: Send>() {}
    // 1 通も応答も Python に触れない（`Send` だけ）ことの機械検査
    assert_send::<Driver>();
    // 動かせることは下の `the_driver_itself_can_be_moved_to_another_thread` で実証する
}

/// pyo3 の `Python::allow_threads` が閉包と戻り値に課す `Ungil`（＝ `Send`）を、
/// `REF:python/novnow-py/src/lib.rs` が実際にまたがせている型すべてについて確かめる。
///
/// pyo3 はこのサンドボックスからビルドできないので、GIL 解放の型条件はこちらで固定する
/// （pyo3 側の検証は `REF:.github/workflows/wheels.yml`）。
#[test]
fn everything_the_python_layer_moves_across_allow_threads_is_send() {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use novnow::app::error::Error;
    use novnow::app::event::AppEvent;
    use novnow::ui::i18n::Locale;

    fn assert_send<T: Send + ?Sized>() {}

    // 閉包が借りるもの（`&Mutex<Driver>` が `Send` ＝ `Mutex<Driver>: Sync` ＝ `Driver: Send`）
    assert_send::<Mutex<Driver>>();
    // 閉包が所有するもの
    assert_send::<String>();
    assert_send::<Value>();
    assert_send::<Locale>();
    // 戻り値
    assert_send::<Value>();
    assert_send::<Error>();
    assert_send::<Vec<AppEvent>>();
    assert_send::<Option<PathBuf>>();
    assert_send::<Result<Value, Error>>();
}

/// 新規作成 → 操作 → 保存が、ワーカースレッドの上で実ディレクトリへ届く。
#[test]
fn a_driver_creates_a_project_and_runs_operations_on_it() {
    let dir = TempDir::new("driver-create");
    let path = dir.join("星屑の時計");
    let mut driver = Driver::create(
        &path,
        CreateOptions {
            title: "星屑の時計".to_owned(),
            ..CreateOptions::default()
        },
    )
    .expect("create");

    assert_eq!(
        driver.root().expect("root").as_deref(),
        Some(path.as_path())
    );
    let info = driver.run("project.info", json!({})).expect("project.info");
    assert_eq!(info["title"], json!("星屑の時計"), "{info}");
    assert!(path.join("project.toml").exists(), "project.toml が無い");

    let scene = first_section(&mut driver);
    driver
        .run(
            "section.write",
            json!({ "scene": scene, "text": "ワーカースレッドから書いた本文" }),
        )
        .expect("section.write");
    driver.run("save.now", json!({})).expect("save.now");

    // 保存は実ファイルへ届いている（§19.1.6 の状態を持つ側）
    drop(driver);
    let mut again = Driver::open(&path).expect("open");
    let read = again
        .run("section.get", json!({ "scene": scene }))
        .expect("section.get");
    assert!(
        read["text"]
            .as_str()
            .expect("text")
            .contains("ワーカースレッドから書いた本文"),
        "書き直した本文が残っていない: {}",
        read["text"]
    );
}

/// 失敗も分類ごとに戻る（1 通 1 往復で `Error` が `Send` であること）。
#[test]
fn a_failure_comes_back_with_its_class() {
    let (mut driver, _dir) = memory_driver();
    let error = driver
        .run("no.such_op", json!({}))
        .expect_err("あるはずのない操作");
    assert_eq!(error.class, Class::Usage, "{error:?}");
    assert!(
        !error.message.is_empty(),
        "メッセージが空（§19.1.7 の分類別の文言）"
    );
}

/// イベント（§19.2.4）は別の 1 本を通って、待った側のスレッドへ届く（§19.3.5）。
#[test]
fn events_reach_the_waiting_thread() {
    let (mut driver, _dir) = memory_driver();
    let scene = first_section(&mut driver);
    driver
        .run(
            "section.write",
            json!({ "scene": scene, "text": "イベントを出す本文" }),
        )
        .expect("section.write");

    let events = driver.poll_events().expect("poll_events");
    let kinds: Vec<String> = events
        .iter()
        .map(|event| event.to_json()["kind"].as_str().unwrap_or("").to_owned())
        .collect();
    assert!(
        kinds.iter().any(|kind| kind == "document_changed"),
        "document_changed が届いていない: {kinds:?}"
    );
    // 取り出した分は消える（`Session::poll_events` と同じ）
    let rest = driver.poll_events().expect("poll_events");
    assert!(
        rest.iter()
            .all(|event| event.to_json()["kind"].as_str() != Some("document_changed")),
        "同じイベントが 2 回来た: {rest:?}"
    );
}

/// 複数スレッドからの同時呼び出しはデータ競合でなく直列実行（§19.3.4 MUST）。
#[test]
fn concurrent_callers_are_serialised() {
    let (driver, _dir) = memory_driver();
    let shared = Arc::new(Mutex::new(driver));
    let mut workers = Vec::new();
    for worker in 0..4u8 {
        let shared = Arc::clone(&shared);
        workers.push(thread::spawn(move || {
            let mut seen = Vec::new();
            for step in 0..25u8 {
                let mut driver = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let value: Value = driver
                    .run("project.info", json!({}))
                    .unwrap_or_else(|error| panic!("worker {worker} step {step}: {error:?}"));
                seen.push(value["title"].as_str().unwrap_or("").to_owned());
            }
            seen
        }));
    }
    let mut total = 0;
    for worker in workers {
        let seen = worker.join().expect("ワーカーがパニックした");
        assert_eq!(seen.len(), 25, "応答の数が合わない");
        assert!(
            seen.iter().all(|title| !title.is_empty()),
            "応答が壊れている: {seen:?}"
        );
        total += seen.len();
    }
    assert_eq!(total, 100, "100 往復すべてが返っていない");
}

/// `Driver` を丸ごと別のスレッドへ動かしても同じ（`Send` の実証）。
#[test]
fn the_driver_itself_can_be_moved_to_another_thread() {
    let (driver, dir) = memory_driver();
    let moved = thread::spawn(move || {
        let mut driver = driver;
        let info = driver.run("project.info", json!({})).expect("project.info");
        (info, driver.root().expect("root"))
    })
    .join()
    .expect("動かせなかった");
    assert_eq!(
        moved.1.as_deref(),
        Some(dir.join("話").as_path()),
        "動かした先でルートが変わっている"
    );
    assert_eq!(moved.0["title"], json!("話"), "{}", moved.0);
}

/// `shutdown`（§19.3.2 の `close`）は保存せずワーカーを畳み、その後の呼び出しは `io`。
#[test]
fn shutdown_stops_the_worker_and_keeps_only_what_was_saved() {
    let dir = TempDir::new("driver-close");
    let path = dir.join("閉じる話");
    let mut driver = Driver::create(
        &path,
        CreateOptions {
            title: "閉じる話".to_owned(),
            ..CreateOptions::default()
        },
    )
    .expect("create");
    let scene = first_section(&mut driver);
    driver
        .run(
            "section.write",
            json!({ "scene": scene, "text": "保存した本文" }),
        )
        .expect("section.write");
    driver.run("save.now", json!({})).expect("save.now");
    driver
        .run(
            "section.write",
            json!({ "scene": scene, "text": "保存しない本文" }),
        )
        .expect("section.write");
    driver.shutdown();

    // ワーカーが無いので `io`（§19.1.7）。データ競合やパニックにはならない
    let error = driver
        .run("project.info", json!({}))
        .expect_err("閉じた後は動かない");
    assert_eq!(error.class, Class::Io, "{error:?}");

    // 保存した分だけ残り、未保存の分は捨てる（§19.3.2「未保存変更の破棄は §19.2.5 と同じ」）
    let mut again = Driver::open(&path).expect("open");
    let read = again
        .run("section.get", json!({ "scene": scene }))
        .expect("section.get");
    let text = read["text"].as_str().expect("text");
    assert!(text.contains("保存した本文"), "保存が効いていない: {text}");
    assert!(
        !text.contains("保存しない本文"),
        "未保存の分が残っている: {text}"
    );
}

/// `Drop` は未保存の変更を捨ててワーカーを畳む（§19.2.5）。
#[test]
fn dropping_a_driver_discards_unsaved_changes() {
    let dir = TempDir::new("driver-drop");
    let path = dir.join("捨てる話");
    let mut driver = Driver::create(
        &path,
        CreateOptions {
            title: "捨てる話".to_owned(),
            ..CreateOptions::default()
        },
    )
    .expect("create");
    let scene = first_section(&mut driver);
    driver
        .run(
            "section.write",
            json!({ "scene": scene, "text": "保存しない本文" }),
        )
        .expect("section.write");
    drop(driver);

    let mut again = Driver::open(&path).expect("open");
    let read = again
        .run("section.get", json!({ "scene": scene }))
        .expect("section.get");
    assert!(
        !read["text"]
            .as_str()
            .expect("text")
            .contains("保存しない本文"),
        "捨てたはずの本文が残っている: {}",
        read["text"]
    );
}

/// ひな形のプロジェクトを開いた `Driver` と、その一時ディレクトリ。
///
/// `Driver::adopt` はホストに `Send` を要求する（`MemoryHost` は `Rc` を共有するので
/// `Send` ではない）ため、`REF:tests/api_fs.rs` と同じく実ディレクトリで開く。
/// 返した `TempDir` は呼び出し側が束縛して生かしておく（`Drop` で消える）。
fn memory_driver() -> (Driver, TempDir) {
    let dir = TempDir::new("driver-memory");
    let path = dir.join("話");
    let driver = Driver::create(
        &path,
        CreateOptions {
            title: "話".to_owned(),
            ..CreateOptions::default()
        },
    )
    .expect("create");
    (driver, dir)
}

/// 1 番目の節の ID（`01-01-01-01`）。
fn first_section(driver: &mut Driver) -> String {
    let list = driver.run("section.list", json!({})).expect("section.list");
    let id = list["sections"][0]["id"].as_str().expect("id").to_owned();
    assert_eq!(id.split('-').count(), 4, "節の ID ではない: {id}");
    id
}
