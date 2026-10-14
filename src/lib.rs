//! novnow の層（`SPECIFICATIONS.md` §3.3）。依存の向きは `ui → adapters → ports ← core`。
//!
//! - `ui` 層の GUI（egui）部分は **`ui` feature のときだけ**ビルドする（`Cargo.toml` の `[features]`、
//!   `PLANS.md` §9.2 B-54）。文言カタログ `ui::i18n` だけは CLI も使う（§12.2）ので常にビルドする
//!   （`src/ui/mod.rs`、B-100）。
//! - `cli` は CLI のサブコマンド（§14、M6）。`main.rs` と並ぶ**合成の根**の一部で、core・ports・
//!   adapters・`ui::i18n` を組み合わせる。ネイティブ専用（Web には CLI が無い）。

pub mod adapters;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
pub mod core;
pub mod ports;
pub mod ui;
