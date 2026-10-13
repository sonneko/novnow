//! ダイアログのポート（§3.1）。ネイティブの実装は `adapters::dialog_rfd`（`ui` feature、M7）。
//!
//! **開くためのダイアログだけ**を持つ。読み取りは「ユーザーが明示的に選んだパス」に限り
//! プロジェクト外でも許される（§13.1.3）が、書き込みは許されない（§13.1.1）ので、保存先を選ぶ
//! ダイアログは無い（Zip のエクスポートは `exports/` に書く、§13.5 第 2.9 版）。選んだパス自体は
//! 履歴以外のどこにも保存しない（§13.1.3）。

use std::future::Future;
use std::path::PathBuf;

/// 選ばれたファイル（Web にはパスが無いので中身で受け取る）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedFile {
    /// ファイル名（表示用）。
    pub name: String,
    /// 中身。
    pub bytes: Vec<u8>,
}

/// ダイアログ。取り消されたら `None`。
pub trait Dialog {
    /// フォルダを選ぶ（§14.4「フォルダを開く」・Zip の取り込み先）。
    fn pick_folder(&self) -> impl Future<Output = Option<PathBuf>>;

    /// Zip を選ぶ（§13.5 のインポート）。
    fn pick_zip(&self) -> impl Future<Output = Option<PickedFile>>;
}
