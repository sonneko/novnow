//! novnow の 4 層（`SPECIFICATIONS.md` §3.3）。依存の向きは `ui → adapters → ports ← core`。
//!
//! `ui` 層は **`ui` feature のときだけ**ビルドする（`Cargo.toml` の `[features]`、
//! `PLANS.md` §9.2 B-54）。M7 まで中身は空の stub で、egui / eframe を既定のビルドに
//! 引き込まないため。core / ports / adapters は feature なしでビルドできる。

pub mod adapters;
pub mod core;
pub mod ports;
#[cfg(feature = "ui")]
pub mod ui;
