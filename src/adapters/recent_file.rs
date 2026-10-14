//! デスクトップのパス履歴ファイル（§13.2）。**唯一のプロジェクト外への書き込み**（§13.1.1）。
//!
//! - 場所は §13.2.1 の表（[`location`]）。`NOVNOW_RECENT_FILE` があればそれを使う。
//! - 形と上限は `core::policy::recent`（キーは `path` / `opened_at` だけ・32 件・壊れていたら空）。
//! - 書き込みは関門の [`FsGate::write_history`] だけを通る（一時ファイルを使わず直接上書き、
//!   §13.2.2 第 2.9 版）。履歴の置き場所は関門が作られたときに環境から決まり、ここから別の
//!   パスを渡す口は無い。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::policy::recent::{PathItem, RecentPaths};
use crate::ports::clock::Clock;
use crate::ports::env::{Env, Os};
use crate::ports::storage::StorageError;

use super::clock_system::now_rfc3339;
use super::storage_fs::FsGate;

/// 履歴ファイルの場所を上書きする環境変数（§13.2.1。テスト・ポータブル運用）。
pub const RECENT_FILE_ENV: &str = "NOVNOW_RECENT_FILE";

/// 履歴ファイルを置くディレクトリ名（§13.2.1 の `novnow/`）。
pub const APP_DIR: &str = "novnow";

/// 履歴ファイル名（§13.2.1）。
pub const FILE_NAME: &str = "recent.json";

/// §13.2.1 の場所。決まらなければ `None`（その環境では履歴を書かない）。
///
/// | OS | パス |
/// |---|---|
/// | Linux / BSD | `$XDG_STATE_HOME/novnow/recent.json`（未設定・相対パスなら `~/.local/state/novnow/recent.json`） |
/// | macOS | `~/Library/Application Support/novnow/recent.json` |
/// | Windows | `%APPDATA%\novnow\recent.json` |
pub fn location(env: &impl Env) -> Option<PathBuf> {
    if let Some(path) = env.var(RECENT_FILE_ENV) {
        return Some(PathBuf::from(path));
    }
    let base = match env.os() {
        Os::Linux | Os::OtherUnix => env
            .var("XDG_STATE_HOME")
            .map(PathBuf::from)
            // XDG Base Directory: 相対パスの値は無効として無視する
            .filter(|path| path.is_absolute())
            .or_else(|| Some(env.home_dir()?.join(".local").join("state")))?,
        Os::MacOs => env.home_dir()?.join("Library").join("Application Support"),
        Os::Windows => PathBuf::from(env.var("APPDATA")?),
        Os::Web => return None,
    };
    Some(base.join(APP_DIR).join(FILE_NAME))
}

/// 履歴ファイル。
#[derive(Debug, Clone)]
pub struct RecentFile {
    gate: Arc<FsGate>,
}

impl RecentFile {
    /// 関門を共有して作る。
    pub fn new(gate: Arc<FsGate>) -> Self {
        Self { gate }
    }

    /// 履歴ファイルのパス（決まらない環境では `None`）。
    pub fn path(&self) -> Option<PathBuf> {
        self.gate.history_file()
    }

    /// 読む。無い・読めない・壊れているときは**空**（§13.2.2）。
    pub fn load(&self) -> RecentPaths {
        self.path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| RecentPaths::parse(&text))
            .unwrap_or_default()
    }

    /// プロジェクトを開いたことを記録する（先頭に積む。§14.4「選択に成功したら履歴の先頭に積む」）。
    /// 履歴の置き場所が無い環境では書かずに、積んだ結果だけを返す。
    pub fn push(
        &self,
        project_root: &Path,
        clock: &impl Clock,
    ) -> Result<RecentPaths, StorageError> {
        let mut recent = self.load();
        recent.push(PathItem {
            path: project_root.to_string_lossy().into_owned(),
            opened_at: now_rfc3339(clock),
        });
        self.save(&recent)?;
        Ok(recent)
    }

    /// 履歴から消す（存在しないパスを選んだときの「履歴から削除」、§13.2.2）。
    pub fn remove(&self, path: &str) -> Result<RecentPaths, StorageError> {
        let mut recent = self.load();
        if recent.remove(path) {
            self.save(&recent)?;
        }
        Ok(recent)
    }

    fn save(&self, recent: &RecentPaths) -> Result<(), StorageError> {
        match self.gate.write_history(recent.to_json().as_bytes()) {
            Err(StorageError::Unsupported) => Ok(()),
            other => other,
        }
    }
}
