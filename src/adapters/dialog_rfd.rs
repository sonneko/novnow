//! ネイティブ / ブラウザのファイルダイアログ（`ports::dialog::Dialog` の rfd 実装、§13.1.1）。
//!
//! - フォルダの選択（`Ctrl+O`、§14.4）と Zip の選択（取り込み、§13.5）は**読むだけ**。
//! - 保存ダイアログ（Zip・テキストのエクスポート、§13.1.1 の特例・B-99・B-109）が返したパスは
//!   [`ChosenPath::from_save_dialog`] に包んで返す。書き込みは `FsGate::write_chosen_export` だけが行う。
//! - **パスを覚えない**（rfd に「最後に開いた場所」を渡さない。初期値は呼び出し側が渡す
//!   `suggested` だけ。規則 10）。
//!
//! rfd 0.14 の Linux 実装は xdg-desktop-portal（`ashpd`）で、GTK を使わない。ポータル側が
//! 「最近使ったファイル」を記録することがあるのは novnow の外の挙動（`docs/verify-rule10.md` §4）。

use std::path::{Path, PathBuf};

use crate::ports::dialog::{ChosenPath, Dialog, ExportKind, PickedFile};

/// rfd のダイアログ。
#[derive(Debug, Default, Clone, Copy)]
pub struct RfdDialog;

impl Dialog for RfdDialog {
    #[cfg(not(target_arch = "wasm32"))]
    async fn pick_folder(&self) -> Option<PathBuf> {
        let picked = rfd::AsyncFileDialog::new().pick_folder().await?;
        Some(handle_path(&picked))
    }

    /// ブラウザにフォルダは無い（Web はプロジェクト選択モーダル、§13.4）。
    #[cfg(target_arch = "wasm32")]
    async fn pick_folder(&self) -> Option<PathBuf> {
        None
    }

    async fn pick_text(&self) -> Option<PickedFile> {
        let picked = rfd::AsyncFileDialog::new()
            .add_filter("Text", &["txt", "md"])
            .pick_file()
            .await?;
        Some(PickedFile {
            name: picked.file_name(),
            bytes: picked.read().await,
        })
    }
    async fn pick_zip(&self) -> Option<PickedFile> {
        let picked = rfd::AsyncFileDialog::new()
            .add_filter("Zip", &["zip", "novnow"])
            .pick_file()
            .await?;
        Some(PickedFile {
            name: picked.file_name(),
            bytes: picked.read().await,
        })
    }

    async fn save_zip(&self, suggested: &Path) -> Option<ChosenPath> {
        save(suggested, ExportKind::Zip, "Zip", &["zip", "novnow"]).await
    }

    async fn save_text(&self, suggested: &Path) -> Option<ChosenPath> {
        save(suggested, ExportKind::Text, "Text", &["txt", "md"]).await
    }
}

async fn save(
    suggested: &Path,
    kind: ExportKind,
    filter: &str,
    extensions: &[&str],
) -> Option<ChosenPath> {
    let mut dialog = rfd::AsyncFileDialog::new().add_filter(filter, extensions);
    if let Some(parent) = suggested.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        dialog = dialog.set_directory(parent);
    }
    if let Some(name) = suggested.file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    let picked = dialog.save_file().await?;
    Some(ChosenPath::from_save_dialog(handle_path(&picked), kind))
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_path(handle: &rfd::FileHandle) -> PathBuf {
    handle.path().to_path_buf()
}

/// ブラウザにはパスが無い（名前だけ）。保存ダイアログは Web では使わない（§13.5 はダウンロード）。
#[cfg(target_arch = "wasm32")]
fn handle_path(handle: &rfd::FileHandle) -> PathBuf {
    PathBuf::from(handle.file_name())
}
