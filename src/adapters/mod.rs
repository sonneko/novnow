//! ポートの実装（§3.1・§3.3）。**I/O を持つのはこの層だけ**。
//!
//! | モジュール | 実装するもの | 環境 |
//! |---|---|---|
//! | [`storage_fs`] | `Storage`（ネイティブ fs）＋ **書き込みの関門** `FsGate`（§13.1.2。書き込みの system call はここにしか無い） | ネイティブ |
//! | [`storage_idb`] | `Storage`（IndexedDB、§13.3） | Web（純粋な部分はどこでも） |
//! | [`recent_file`] | パス履歴ファイル（§13.2。唯一のプロジェクト外への書き込み） | ネイティブ |
//! | [`zip`] | Zip のインポート / エクスポート（§13.5） | 両方 |
//! | [`project_load`] | 一覧 → §4.3 の判定 → 文書の読み込み（どの `Storage` でも同じ手順） | 両方 |
//! | [`env_system`] / [`clock_system`] | `Env` / `Clock` | 両方 |
//! | [`mcp_stdio`] | `McpClient`（M10） | ネイティブ |
//! | `dialog_rfd` | `Dialog`（`ui` feature、M7） | ネイティブ |

pub mod clock_system;
// ネイティブのファイルダイアログ（rfd）は GUI の部品なので `ui` feature のときだけ
// （`Cargo.toml`、`PLANS.md` §9.2 B-54）。ポートの定義は `ports/dialog.rs` にあり、
// CLI 側はポート経由で「ダイアログなし」の実装を選べる。
#[cfg(feature = "ui")]
pub mod dialog_rfd;
pub mod env_system;
pub mod mcp_stdio;
pub mod project_load;
#[cfg(not(target_arch = "wasm32"))]
pub mod recent_file;
#[cfg(not(target_arch = "wasm32"))]
pub mod storage_fs;
pub mod storage_idb;
pub mod zip;

/// `Future` を今のスレッドで最後まで待つ（ネイティブ専用。`PLANS.md` §9.2 B-90）。
///
/// ネイティブの `Storage`（`storage_fs::FsStorage`）は中で同期 I/O をして即座に完了するので、
/// CLI（M6 の `check` 等）やテストはこれで待てばよい。待ちが生じる `Future` でも動く
/// （起こされるまでスレッドを止める）。Web では使わない（`wasm_bindgen_futures::spawn_local`）。
#[cfg(not(target_arch = "wasm32"))]
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    use std::thread::{self, Thread};

    struct ThreadWaker(Thread);

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}
