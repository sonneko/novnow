//! 第 2 の画面実装（`REF:docs/specifications/webui.md` §21）の **ネイティブ実行**（Tauri 2）。
//!
//! # ここが持つもの（設計 §21.4）
//!
//! コマンドは **4 つだけ**。引数と戻り値は**文字列の JSON** で、ブラウザ用の橋渡し
//! （`REF:crates/novnow-wasm/src/lib.rs`）と同じ形である（画面から見て 2 つの橋渡しが同じ）。
//!
//! | コマンド | 引数 | 戻り値 |
//! |---|---|---|
//! | `open` | `path`, `create_options`（省ける） | `run` と同じ形。`result` は `locale` / `root` / `version` |
//! | `run` | `op`, `args`（省ける） | `{"ok":true,"result":…}` / `{"ok":false,"error":{…}}` |
//! | `poll_events` | なし | `AppEvent` の配列 |
//! | `poll_external` | なし | `run` と同じ形。`result` は `changes` / `count` |
//!
//! 引数の名前は Rust と同じ綴り（`create_options`）で渡す（`rename_all = "snake_case"`）。
//!
//! # 中身は `bridge/` にある
//!
//! このファイルは Tauri への配線だけを持つ。コマンドの中身（`Session` の所有・JSON の形・
//! 新規作成の確認）は `REF:webui/src-tauri/bridge/src/lib.rs` にあり、Tauri 無しで組めて試せる。
//!
//! - **`Session` は 1 つだけ、`Mutex` で持つ**: 状態は `Mutex<NativeSession>` 1 つ
//!   （`REF:docs/developers.md` §8.10 の 31）。`Session` 自体は `Send` でないので、
//!   `NativeSession` が持ち主のスレッドの中に置いている。
//! - **Host は `FsHost` をそのまま使う**: 書き込みは関門 `FsGate`（規則 10）だけを通る。
//!   ここにも `bridge/` にも、ファイルに書く API は無い。
//! - **許可は広げない**: プラグインは保存・選択のダイアログだけ（`capabilities/default.json`）。
//!   ファイル入出力・通信・自動更新の口は置かない（§21.4.1・§21.4.3）。

// リリースビルドの Windows で、余計なコンソール窓を出さない。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Mutex, MutexGuard, PoisonError};

use novnow_native_bridge::NativeSession;
use tauri::State;

/// アプリが持つ状態: セッション 1 つ（開いている間は 1 つだけ）。
struct Shared(Mutex<NativeSession>);

impl Shared {
    /// ロックを取る。毒になっていても続ける（`NativeSession` の呼び出しは panic を外へ出さない）。
    fn lock(&self) -> MutexGuard<'_, NativeSession> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// プロジェクトを開く。`create_options`（JSON のオブジェクト）があれば新しく作ってから開く。
#[tauri::command(rename_all = "snake_case")]
fn open(state: State<'_, Shared>, path: String, create_options: Option<String>) -> String {
    state.lock().open(&path, create_options.as_deref())
}

/// 正規操作 1 回（`Session::run`）。
#[tauri::command(rename_all = "snake_case")]
fn run(state: State<'_, Shared>, op: String, args: Option<String>) -> String {
    state.lock().run(&op, args.as_deref().unwrap_or_default())
}

/// 溜まったイベントを取り出す。
#[tauri::command(rename_all = "snake_case")]
fn poll_events(state: State<'_, Shared>) -> String {
    state.lock().poll_events()
}

/// 外部変更の監視 1 回分（画面が 1 秒ごとに呼ぶ。§8.5）。
#[tauri::command(rename_all = "snake_case")]
fn poll_external(state: State<'_, Shared>) -> String {
    state.lock().poll_external()
}

fn main() {
    // 機能核が作る JSON のキーの順（辞書順）がブラウザ版と同じであること。依存のどこかが
    // `serde_json` の `preserve_order` を立てると崩れるので、崩れた版は動かさない。
    assert!(
        novnow_native_bridge::deterministic_json(),
        "serde_json does not sort object keys in this build; saved files would differ from the browser build"
    );
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Shared(Mutex::new(NativeSession::new())))
        .invoke_handler(tauri::generate_handler![
            open,
            run,
            poll_events,
            poll_external
        ])
        .run(tauri::generate_context!())
        .expect("failed to run the novnow webui native shell");
}

#[cfg(test)]
mod tests {
    /// Tauri を含む依存の組み合わせでも、JSON のキーは辞書順のまま（ブラウザ版と同じバイト列）。
    #[test]
    fn json_keys_stay_sorted_with_the_tauri_dependencies() {
        assert!(novnow_native_bridge::deterministic_json());
    }
}
