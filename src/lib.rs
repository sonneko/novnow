//! novnow の層（`SPECIFICATIONS.md` §3.3）。依存の向きは `ui → adapters → ports ← core`。
//!
//! - `ui` 層の GUI（egui）部分は **`ui` feature のときだけ**ビルドする（`Cargo.toml` の `[features]`、
//!   `PLANS.md` §9.2 B-54）。文言カタログ `ui::i18n` だけは CLI も使う（§12.2）ので常にビルドする
//!   （`src/ui/mod.rs`、B-100）。
//! - `app` は描画と無縁の状態機械と外部インターフェースの機能核（§19.1「単一実装点」）。
//!   常にビルドし、GUI のクレートを使わない（`tests/layering.rs` が検査する）。
//! - `cli` は CLI のサブコマンド（§14、M6）。`main.rs` と並ぶ**合成の根**の一部で、core・ports・
//!   adapters・`ui::i18n` を組み合わせる。ネイティブ専用（Web には CLI が無い）。

pub mod adapters;
pub mod app;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
pub mod core;
/// 第 5 の口（§19.4）。エディタと話す LSP サーバ。標準入出力だけを使うのでネイティブ専用
/// （Web には CLI と同じく無い）。
#[cfg(not(target_arch = "wasm32"))]
pub mod lsp;
pub mod ports;
pub mod ui;
