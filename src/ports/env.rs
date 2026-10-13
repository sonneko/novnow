//! 実行環境のポート（§3.1）。環境変数・OS・ロケール。
//!
//! M5 では履歴ファイルの置き場所（§13.2.1: `NOVNOW_RECENT_FILE` / `XDG_STATE_HOME` / `HOME` /
//! `APPDATA`）に使う。M6 で言語の決定（§12.4 の `NOVNOW_LANG` と OS のロケール、[`Env::locale`]）と
//! 色付けの判定（`NO_COLOR`）に広げた。端末かどうか（§14.3）は標準入出力の性質なので
//! ここではなく `cli::Terminal` が持つ。実装は `adapters::env_system`、テストは表を引くだけの
//! 偽物を使う（プロセスの環境変数を書き換えない）。

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

    /// OS / ブラウザのロケール（§12.4 の 3。`ja_JP.UTF-8`・`ja-JP`・`C` のような文字列のまま）。
    /// 分からなければ `None`。既定の実装は POSIX の順の環境変数（[`posix_locale`]）。
    fn locale(&self) -> Option<String> {
        posix_locale(self)
    }
}

/// POSIX のロケールの環境変数を優先順（`LC_ALL` → `LC_MESSAGES` → `LANG`）に引く（空は無いものとする）。
pub fn posix_locale<E: Env + ?Sized>(env: &E) -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| env.var(name).filter(|value| !value.is_empty()))
}
