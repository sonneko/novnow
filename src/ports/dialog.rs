//! ダイアログのポート（§3.1）。ネイティブの実装は `adapters::dialog_rfd`（`ui` feature、M7）。
//!
//! 読み取りは「ユーザーが明示的に選んだパス」に限りプロジェクト外でも許される（§13.1.3）。
//! 書き込みはプロジェクト内と履歴ファイルだけ（§13.1.1）だが、**エクスポート（Zip とテキスト）の
//! 保存先としてユーザーが明示したパスだけは特例**として外でも書ける（§13.1.1 の特例・§13.5・§17.2、
//! ユーザー裁定 2026-10-13・第 2.10 / 2.11 版）。その「ユーザーが明示した」ことを [`ChosenPath`]
//! という型で表し、関門の特例の口（`adapters::storage_fs::FsGate::write_chosen_export`）はこの型しか
//! 受け取らない。明示されたパス自体は履歴以外のどこにも保存しない（§13.1.3。特例の保存先は履歴にも
//! 書かない）。

use std::future::Future;
use std::path::{Path, PathBuf};

pub use crate::core::policy::ExportKind;

/// 選ばれたファイル（Web にはパスが無いので中身で受け取る）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedFile {
    /// ファイル名（表示用）。
    pub name: String,
    /// 中身。
    pub bytes: Vec<u8>,
}

/// ユーザーが保存先として明示したパスと、何を書くか（§13.1.1 の特例、`docs/history.md` B-99・B-109）。
///
/// **作ってよいのは保存ダイアログの実装（[`Dialog::save_zip`] / [`Dialog::save_text`]）・CLI の
/// `novnow export --out <path>`・テストだけ**。関門の特例の口はこの型を**値で**受け取って消費する
/// （`Clone` を持たない）ので、1 回の指定で書けるのは 1 回だけ。型で守れるのは「うっかり」までで、
/// 手で作ればすり抜けられる。作る箇所を `ChosenPath::from_` の grep で数えられるようにするための
/// 目印でもある。
#[derive(Debug, PartialEq, Eq)]
pub struct ChosenPath {
    path: PathBuf,
    kind: ExportKind,
}

impl ChosenPath {
    /// 保存ダイアログの結果から作る（ダイアログの実装とテスト以外から呼ばない）。
    pub fn from_save_dialog(path: PathBuf, kind: ExportKind) -> Self {
        Self { path, kind }
    }

    /// コマンドラインで明示された保存先から作る（`novnow export --out <path>`、§17.2）。
    pub fn from_command_line(path: PathBuf, kind: ExportKind) -> Self {
        Self { path, kind }
    }

    /// 指定されたパス（そのまま。シンボリックリンクの解決は関門が行う）。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 書くエクスポートの種類（拡張子の判定に使う）。
    pub fn kind(&self) -> ExportKind {
        self.kind
    }
}

/// ダイアログ。取り消されたら `None`。
pub trait Dialog {
    /// フォルダを選ぶ（§14.4「フォルダを開く」・Zip の取り込み先）。
    fn pick_folder(&self) -> impl Future<Output = Option<PathBuf>>;

    /// Explicitly pick a manuscript text file; never save its source path.
    fn pick_text(&self) -> impl Future<Output = Option<PickedFile>> {
        async { None }
    }

    /// 移行の入力としてテキストファイルを複数選ぶ（§14.6。デスクトップのみ）。
    fn pick_texts(&self) -> impl Future<Output = Option<Vec<PickedFile>>> {
        async { None }
    }

    /// Zip を選ぶ（§13.5 のインポート）。
    fn pick_zip(&self) -> impl Future<Output = Option<PickedFile>>;

    /// Zip の保存先を選ぶ（§13.5 のデスクトップのエクスポート、第 2.10 版）。`suggested` は初期値
    /// （`exports/<project-name>.zip` の絶対パス。`adapters::zip::export_path`）。上書きの確認は
    /// ダイアログ（OS）に任せる。結果の種類は [`ExportKind::Zip`]。
    fn save_zip(&self, suggested: &Path) -> impl Future<Output = Option<ChosenPath>>;

    /// テキストのエクスポートの保存先を選ぶ（§17.2 の「名前を付けて保存」、第 2.11 版）。`suggested` は
    /// 初期値（`exports/<title>-<範囲>-<日時>.txt` の絶対パス）。結果の種類は [`ExportKind::Text`]。
    fn save_text(&self, suggested: &Path) -> impl Future<Output = Option<ChosenPath>>;
}
