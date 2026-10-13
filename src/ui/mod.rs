//! UI 層（§3.3）。
//!
//! **`i18n` だけは常にビルドする**（CLI のメッセージも同じカタログを使う、§12.2。egui に依存しない）。
//! それ以外（egui を使う GUI）は `ui` feature のときだけ（`PLANS.md` §9.2 B-54・B-100）。
//! この形は `tests/layering.rs` の `the_gui_is_compiled_only_with_the_ui_feature` が固定している。

pub mod i18n;

#[cfg(feature = "ui")]
pub mod shell;
#[cfg(feature = "ui")]
pub mod theme;
#[cfg(feature = "ui")]
pub mod widgets;
#[cfg(feature = "ui")]
pub mod workspaces;
