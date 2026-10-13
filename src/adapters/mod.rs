pub mod clock_system;
// ネイティブのファイルダイアログ（rfd）は GUI の部品なので `ui` feature のときだけ
// （`Cargo.toml`、`PLANS.md` §9.2 B-54）。ポートの定義は `ports/dialog.rs` にあり、
// CLI 側はポート経由で「ダイアログなし」の実装を選べる。
#[cfg(feature = "ui")]
pub mod dialog_rfd;
pub mod mcp_stdio;
pub mod storage_fs;
pub mod storage_idb;
