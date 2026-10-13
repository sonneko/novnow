//! Zip のインポート / エクスポート（§13.5、仕様 第 2.9 版。`PLANS.md` §9.2 B-93）。
//!
//! - **何を入れるか・何を取り込むか**は core の `policy::archive`（`.novnow/` とドットファイル、
//!   単一ルートの剥がし、`..` の拒否）。ここは `zip` クレートでバイト列にするだけ。
//! - [`export`] / [`import`] はバイト列どうしの変換なので、デスクトップと Web で同じコードが動く
//!   （Web はエクスポートをダウンロードし、インポートを新しいプロジェクトレコードに書く、§13.5）。
//! - デスクトップのエクスポートは **`exports/<project-name>.zip`** に書く（[`export_project`]。
//!   保存ダイアログは出さない。プロジェクトの外へは書けないため、§13.5 第 2.9 版）。書き出し先の
//!   Zip 自身は中身に入れない。インポートは選ばれたディレクトリを新しいルートにして書き
//!   （[`import_project`]）、**既存のファイルは上書きしない**。
//! - 展開後の大きさに上限を設ける（壊れた・悪意のある Zip でメモリを使い切らないため）。

use std::io::{Cursor, Read, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::core::policy::archive::{self, ArchiveEntry, SkipReason, Skipped};
use crate::core::policy::ProjectPath;
use crate::ports::storage::StorageError;

/// エクスポートの書き出し先のディレクトリ（§4.1 `exports/`）。
pub const EXPORTS_DIR: &str = "exports";

/// 1 エントリの展開後の上限（256 MiB）。
pub const MAX_ENTRY_BYTES: u64 = 256 * 1024 * 1024;

/// 全エントリの展開後の合計の上限（1 GiB）。
pub const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

/// Zip の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveError {
    /// Zip として読めない・書けない（`zip` クレートのメッセージ。ログ用）。
    Broken(String),
    /// 展開後の大きさが上限を超える。
    TooLarge,
    /// ストレージの失敗。
    Storage(StorageError),
}

impl From<StorageError> for ArchiveError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

fn broken(error: impl std::fmt::Display) -> ArchiveError {
    ArchiveError::Broken(error.to_string())
}

/// `<project-name>.zip`（§13.5）。ファイル名に使えない文字（`/` `\` `:` 制御文字）は `_` にし、
/// ドットで始まる名前は先頭に `_` を足す（隠しファイルにしない）。空なら `project.zip`。
pub fn file_name(project_name: &str) -> String {
    let mut name: String = project_name
        .trim()
        .chars()
        .map(|ch| {
            if matches!(ch, '/' | '\\' | ':') || ch.is_control() {
                '_'
            } else {
                ch
            }
        })
        .collect();
    if name.is_empty() {
        name.push_str("project");
    }
    if name.starts_with('.') {
        name.insert(0, '_');
    }
    format!("{name}.zip")
}

/// デスクトップのエクスポートの書き出し先（`exports/<project-name>.zip`、§13.5 第 2.9 版）。
pub fn export_path(project_name: &str) -> ProjectPath {
    let path = format!("{EXPORTS_DIR}/{}", file_name(project_name));
    // `file_name` が `/` `\` `:` 制御文字を除き、先頭のドットも避けるので必ず作れる
    ProjectPath::new(&path)
        .unwrap_or_else(|_| ProjectPath::new("exports/project.zip").expect("a fixed relative path"))
}

/// ファイル一覧を Deflate の Zip にする（§13.5）。`policy::archive::is_exported` でないもの
/// （`.novnow/`・ドットファイル）は入れない。並びは渡した順。
pub fn export(files: &[(ProjectPath, Vec<u8>)]) -> Result<Vec<u8>, ArchiveError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (path, bytes) in files {
        if !archive::is_exported(path) {
            continue;
        }
        writer.start_file(path.as_str(), options).map_err(broken)?;
        writer.write_all(bytes).map_err(broken)?;
    }
    let cursor = writer.finish().map_err(broken)?;
    Ok(cursor.into_inner())
}

/// Zip から取り込むもの。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Imported {
    /// 取り込むファイル（Zip の順）。
    pub files: Vec<(ProjectPath, Vec<u8>)>,
    /// 取り込まないもの（UI が一覧で示す）。
    pub skipped: Vec<Skipped>,
    /// 剥がした単一のルートディレクトリ。
    pub stripped_root: Option<String>,
}

impl Imported {
    /// テキストだけにする（Web はテキストのみ、§13.3.2）。UTF-8 として読めないものは
    /// [`SkipReason::NotText`] で `skipped` に移す。
    pub fn into_texts(self) -> (Vec<(ProjectPath, String)>, Vec<Skipped>) {
        let mut skipped = self.skipped;
        let mut texts = Vec::new();
        for (path, bytes) in self.files {
            match String::from_utf8(bytes) {
                Ok(text) => texts.push((path, text)),
                Err(_) => skipped.push(Skipped {
                    name: path.as_str().to_owned(),
                    reason: SkipReason::NotText,
                }),
            }
        }
        (texts, skipped)
    }
}

/// Zip を読んで、取り込むものを決める（§13.5。規則は `policy::archive::plan_import`）。
pub fn import(bytes: &[u8]) -> Result<Imported, ArchiveError> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(broken)?;
    let mut entries = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let file = zip.by_index(index).map_err(broken)?;
        entries.push(ArchiveEntry {
            name: file.name().to_owned(),
            is_dir: file.is_dir(),
        });
    }
    let plan = archive::plan_import(&entries);
    let mut total = 0u64;
    let mut files = Vec::with_capacity(plan.entries.len());
    for planned in plan.entries {
        let file = zip.by_index(planned.index).map_err(broken)?;
        let mut data = Vec::new();
        // 宣言された大きさは偽れるので、実際に読んだ量で上限を見る
        file.take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(broken)?;
        let read = data.len() as u64;
        total += read;
        if read > MAX_ENTRY_BYTES || total > MAX_TOTAL_BYTES {
            return Err(ArchiveError::TooLarge);
        }
        files.push((planned.path, data));
    }
    Ok(Imported {
        files,
        skipped: plan.skipped,
        stripped_root: plan.stripped_root,
    })
}

/// [`import_project`] の結果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportReport {
    /// 書いたファイル。
    pub written: Vec<ProjectPath>,
    /// 既にあったので**上書きしなかった**ファイル（§13.5 第 2.9 版）。
    pub kept: Vec<ProjectPath>,
}

/// デスクトップのエクスポート: プロジェクトを Zip にして `exports/<project-name>.zip` に書く
/// （§13.5 第 2.9 版）。書き出し先の Zip 自身は中身に入れない。書いた相対パスを返す。
#[cfg(not(target_arch = "wasm32"))]
pub fn export_project(
    storage: &super::storage_fs::FsStorage,
    project_name: &str,
) -> Result<ProjectPath, ArchiveError> {
    let target = export_path(project_name);
    let mut files = Vec::new();
    for entry in storage.list_files()? {
        if entry.path == target || !archive::is_exported(&entry.path) {
            continue;
        }
        let bytes = storage.read_bytes(&entry.path)?;
        files.push((entry.path, bytes));
    }
    let zip = export(&files)?;
    storage.write_bytes(&target, &zip)?;
    Ok(target)
}

/// デスクトップのインポート: 取り込むファイルを `storage`（新しいルート。`FsStorage::create`）に
/// 書く。**既存のファイルは上書きしない**（§13.5 第 2.9 版）。取り込み後の §4.3 の妥当性検査は
/// 呼び出し側（`project_load::load`）で行い、不備があれば「不備のまま取り込む / 取り消す」を選ばせる。
#[cfg(not(target_arch = "wasm32"))]
pub fn import_project(
    storage: &super::storage_fs::FsStorage,
    imported: &Imported,
) -> Result<ImportReport, ArchiveError> {
    let mut report = ImportReport::default();
    for (path, bytes) in &imported.files {
        if storage.exists(path) {
            report.kept.push(path.clone());
            continue;
        }
        storage.write_bytes(path, bytes)?;
        report.written.push(path.clone());
    }
    Ok(report)
}
