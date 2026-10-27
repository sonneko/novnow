//! UI 層（§3.3）。
//!
//! **`i18n` だけは常にビルドする**（CLI のメッセージも同じカタログを使う、§12.2。egui に依存しない）。
//! それ以外（egui を使う GUI）は `ui` feature のときだけ（`docs/history.md` B-54・B-100）。
//! この形は `tests/layering.rs` の `the_gui_is_compiled_only_with_the_ui_feature` が固定している。
//!
//! 描画と無縁の状態機械（編集・保存・設定・エージェント実行・検索・パレットの論理・コマンド・
//! ワークスペース切替・ホスト）は段階 ③ の H1 で `crate::app`（§19.1）へ移した。旧パス
//! （`ui::workbench` など）は下の再エクスポートでそのまま解決する。

pub mod i18n;

pub use crate::app::{agents, assist, palette, save_machine, settings, workbench};

#[cfg(feature = "ui")]
pub mod app;
#[cfg(feature = "ui")]
pub mod guide;
#[cfg(feature = "ui")]
pub mod palette_items;
#[cfg(feature = "ui")]
pub mod picker;
#[cfg(feature = "ui")]
pub mod shell;
#[cfg(feature = "ui")]
pub mod status;
#[cfg(feature = "ui")]
pub mod theme;
/// Web の GUI（§13.3〜§13.5。IndexedDB のプロジェクト選択と、開いたプロジェクトの Host）。
#[cfg(feature = "ui")]
#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(feature = "ui")]
pub mod widgets;
#[cfg(feature = "ui")]
pub mod workspaces;
