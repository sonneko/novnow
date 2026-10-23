//! 第 5 の口 — LSP サーバ（§19.4）。
//!
//! エディタ（VS Code、Neovim など）から標準入出力で話し、§19.7 の操作カタログへ届く。
//! 実装は 1 つで、口が 5 つ（§19.1.2）。言語機能そのものは §6.3 の
//! [`crate::core::features`] にあり、ここは **LSP の形への翻訳** だけを担う。
//!
//! 役割は 5 つのファイルに固定（§19.4.1）。
//!
//! * [`framing`] — `Content-Length:` の枠。標準ライブラリだけで読み書きする。
//! * [`convert`] — LSP の行・列（UTF-16）と内部の byte 位置の往復。
//! * [`methods`] — 標準メソッド → 操作カタログ（`session.run("lang.hover", …)`）。
//! * [`commands`] — `workspace/executeCommand` → 操作カタログの全件。
//! * [`server`] — 受け付け・振り分け・通知の送出（[`run`] が入口）。
//!
//! 対応する LSP は 3.17。JSON-RPC 2.0 は [`crate::cli::mcp`] と同じく新クレートを入れずに
//! 手書きで済ませる（§19.1.2）。

pub mod commands;
pub mod convert;
pub mod framing;
pub mod methods;
pub mod server;

pub use server::run;
