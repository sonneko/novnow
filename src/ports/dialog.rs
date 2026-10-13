//! ダイアログのポート（§3.1）。ネイティブの実装は `adapters::dialog_rfd`（`ui` feature、M7）。
//!
//! 読み取りは「ユーザーが明示的に選んだパス」に限りプロジェクト外でも許される（§13.1.3）。
//! 書き込みはプロジェクト内と履歴ファイルだけ（§13.1.1）だが、**保存ダイアログで指定された Zip の
//! 保存先だけは特例**として外でも書ける（§13.1.1 の特例・§13.5、ユーザー裁定 2026-10-13・第 2.10 版）。
//! その「ユーザーがダイアログで指定した」ことを [`ChosenPath`] という型で表し、関門の特例の口
//! （`adapters::storage_fs::FsGate::write_chosen_export`）はこの型しか受け取らない。
//! 選んだパス自体は履歴以外のどこにも保存しない（§13.1.3。特例の保存先は履歴にも書かない）。

use std::future::Future;
use std::path::{Path, PathBuf};

/// 選ばれたファイル（Web にはパスが無いので中身で受け取る）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedFile {
    /// ファイル名（表示用）。
    pub name: String,
    /// 中身。
    pub bytes: Vec<u8>,
}

/// 保存ダイアログでユーザーが指定したパス（§13.1.1 の特例、`PLANS.md` §9.2 B-99）。
///
/// **作ってよいのは保存ダイアログの実装（[`Dialog::save_zip`]）とテストだけ**。関門の特例の口は
/// この型を**値で**受け取って消費する（`Clone` を持たない）ので、1 回の指定で書けるのは 1 回だけ。
/// 型で守れるのは「うっかり」までで、手で作ればすり抜けられる。作る箇所を
/// `ChosenPath::from_save_dialog` の grep で数えられるようにするための目印でもある。
#[derive(Debug, PartialEq, Eq)]
pub struct ChosenPath(PathBuf);

impl ChosenPath {
    /// 保存ダイアログの結果から作る（ダイアログの実装とテスト以外から呼ばない）。
    pub fn from_save_dialog(path: PathBuf) -> Self {
        Self(path)
    }

    /// 指定されたパス（ダイアログが返したまま。シンボリックリンクの解決は関門が行う）。
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// ダイアログ。取り消されたら `None`。
pub trait Dialog {
    /// フォルダを選ぶ（§14.4「フォルダを開く」・Zip の取り込み先）。
    fn pick_folder(&self) -> impl Future<Output = Option<PathBuf>>;

    /// Zip を選ぶ（§13.5 のインポート）。
    fn pick_zip(&self) -> impl Future<Output = Option<PickedFile>>;

    /// Zip の保存先を選ぶ（§13.5 のデスクトップのエクスポート、第 2.10 版）。`suggested` は初期値
    /// （`exports/<project-name>.zip` の絶対パス。`adapters::zip::export_path`）。上書きの確認は
    /// ダイアログ（OS）に任せる。
    fn save_zip(&self, suggested: &Path) -> impl Future<Output = Option<ChosenPath>>;
}
