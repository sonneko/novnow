//! スレッドの約束（設計 §21.4・`REF:docs/developers.md` §8.10 の 31）。
//!
//! `Session` は `Send` ではないので、持ち主のスレッド 1 本の中に置く。`NativeSession` は
//! そこへの口で、Tauri 側は `Mutex<NativeSession>` を 1 つだけ持つ。

mod common;

use std::sync::{Arc, Mutex};

use serde_json::json;

use common::{ok, sample, text, TempDir};
use novnow_native_bridge::NativeSession;

/// Tauri が管理する状態に要る境界（`Send + Sync`）。
fn managed<T: Send + Sync + 'static>(value: T) -> T {
    value
}

/// 複数のスレッドから同じセッションを叩いても、1 通ずつ順に処理される。
#[test]
fn the_session_is_shared_through_one_mutex() {
    let dir = TempDir::new("threads");
    let root = sample(&dir, "sample");
    let shared = Arc::new(managed(Mutex::new(NativeSession::new())));
    ok(&shared.lock().expect("lock").open(&text(&root), None));

    let workers: Vec<_> = (0..8)
        .map(|index| {
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || {
                for round in 0..5 {
                    let title = format!("思いつき {index}-{round}");
                    let args = json!({ "title": title, "role": "" });
                    let captured = shared
                        .lock()
                        .expect("lock")
                        .run("idea.capture", &args.to_string());
                    ok(&captured);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().expect("worker");
    }

    let mut session = shared.lock().expect("lock");
    let ideas = ok(&session.run("idea.list", "{}"));
    // サンプルの 1 件 + 8 スレッド × 5 件（1 件も失われない）
    assert_eq!(ideas["count"], json!(41), "{ideas}");
    ok(&session.run("save.now", "{}"));
}

/// 口は別のスレッドへ動かせる（`Session` 自体は持ち主のスレッドから出ない）。
#[test]
fn the_handle_moves_between_threads() {
    let dir = TempDir::new("threads-move");
    let root = sample(&dir, "sample");
    let mut session = NativeSession::new();
    ok(&session.open(&text(&root), None));
    let moved = std::thread::spawn(move || {
        let info = ok(&session.run("project.info", "{}"));
        (session, info)
    });
    let (mut session, info) = moved.join().expect("thread");
    assert_eq!(info["title"], json!("星屑の時計"));
    assert_eq!(
        ok(&session.run("project.info", "{}"))["title"],
        json!("星屑の時計")
    );
}

/// 捨てると持ち主のスレッドも畳まれ、同じ作品をもう一度開ける。
#[test]
fn dropping_the_handle_releases_the_project() {
    let dir = TempDir::new("threads-drop");
    let root = sample(&dir, "sample");
    for _ in 0..3 {
        let mut session = NativeSession::new();
        ok(&session.open(&text(&root), None));
        ok(&session.run("project.info", "{}"));
    }
    // 一度も開かずに捨ててもよい（スレッドを起こしていない）
    drop(NativeSession::new());
}
