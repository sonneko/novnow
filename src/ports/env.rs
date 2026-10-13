//! 実行環境のポート（§3.1）。環境変数と OS。
//!
//! M5 では履歴ファイルの置き場所（§13.2.1: `NOVNOW_RECENT_FILE` / `XDG_STATE_HOME` / `HOME` /
//! `APPDATA`）に使う。M6 で言語の決定（§12.4 の `NOVNOW_LANG` / `LANG` / `LC_ALL`）・
//! 色付けの判定（`NO_COLOR`）・端末の判定（§14.3）に広げる。実装は `adapters::env_system`、
//! テストは表を引くだけの偽物を使う（プロセスの環境変数を書き換えない）。

use std::path::PathBuf;

/// OS（§13.2.1 の表の行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    /// Linux。
    Linux,
    /// macOS。
    MacOs,
    /// Windows。
    Windows,
    /// その他の Unix（BSD 等。§13.2.1 では Linux と同じ行）。
    OtherUnix,
    /// ブラウザ（環境変数もファイルシステムも無い）。
    Web,
}

/// 実行環境。
pub trait Env {
    /// 環境変数（無い・空・UTF-8 でないときは `None`）。
    fn var(&self, name: &str) -> Option<String>;

    /// OS。
    fn os(&self) -> Os;

    /// ホームディレクトリ（§13.2.1 の `~`）。分からなければ `None`。
    fn home_dir(&self) -> Option<PathBuf>;
}
