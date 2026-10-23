//! 応用層（`app`、§19.1「単一実装点」）。**egui / eframe / rfd / fontdb に依存しない**。
//!
//! ここにあるのは「GUI が描画で使う状態と規則」そのもので、GUI（`ui` 層）はその表示層の
//! 1 つにすぎない。同じ状態機械を CLI・MCP・LSP・Python の薄い適合層（§19.3〜§19.6）が
//! 共有するので、5 つのインターフェースが同じ入力に同じ結果を返す（§19.1.1）。
//!
//! 依存の向きは `ui → app → adapters → ports ← core`（`REF:docs/designs/overview.md` §3.3）。
//! 例外は文言カタログ `ui::i18n` だけで、これは CLI が既に持つ例外を app 層へ広げたもの
//! （§19.1.8。`tests/layering.rs` が機械検査する）。

pub mod agents;
pub mod assist;
pub mod command;
pub mod commands;
/// ワーカースレッド 1 本の上で `Session` を所有する口（§19.3.4 の GIL 解放）。
/// Web にはスレッドが無いのでネイティブだけ。
#[cfg(not(target_arch = "wasm32"))]
pub mod driver;
pub mod error;
pub mod event;
// ファイルシステムを後ろに持つホスト（§19.2.1）。Web には FS が無いのでネイティブだけ。
#[cfg(not(target_arch = "wasm32"))]
pub mod fs_host;
pub mod host;
pub mod memory_host;
pub mod ops;
pub mod palette;
pub mod save_machine;
pub mod session;
pub mod settings;
pub mod workbench;
pub mod workspace;
