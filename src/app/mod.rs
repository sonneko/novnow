//! 応用層（`app`、§19.1「単一実装点」）。**egui / eframe / rfd / fontdb に依存しない**。
//!
//! ここにあるのは「GUI が描画で使う状態と規則」そのもので、GUI（`ui` 層）はその表示層の
//! 1 つにすぎない。同じ状態機械を CLI・MCP・LSP・Python の薄い适配層（§19.3〜§19.6）が
//! 共有するので、5 つのインターフェースが同じ入力に同じ結果を返す（§19.1.1）。
//!
//! 依存の向きは `ui → app → adapters → ports ← core`（`REF:docs/designs/overview.md` §3.3）。
//! 例外は文言カタログ `ui::i18n` だけで、これは CLI が既に持つ例外を app 層へ広げたもの
//! （§19.1.8。`tests/layering.rs` が機械検査する）。

pub mod agents;
pub mod assist;
pub mod command;
pub mod commands;
pub mod host;
pub mod palette;
pub mod save_machine;
pub mod settings;
pub mod workbench;
pub mod workspace;
