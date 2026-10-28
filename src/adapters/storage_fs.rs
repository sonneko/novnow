//! ネイティブのファイルシステム（§8.3・§13.1、`docs/history.md` B-91）。
//!
//! # 書き込みの関門（[`FsGate`]）
//!
//! **書き込みの system call はこのファイルにしか無い**（`OpenOptions`・`fs::rename`・
//! `fs::remove_file`・`fs::create_dir_all` 等。`tests/layering.rs` が `src/` 全体を検査する、
//! §13.1.2 第 2.9 版）。そのすべてが [`FsGate`] のメソッドの中にあり、毎回次の順で判定する。
//!
//! 1. 書き込み先を**シンボリックリンク解決後の絶対パス**にする（[`resolve`]: 存在する最も深い
//!    祖先を `canonicalize` し、まだ無い残りをつなぐ）。解決できなければ書かない
//!    （[`Violation::Unresolved`]）。
//! 2. core の [`WritePolicy`] で判定する（`..` は字句で正規化。ルートより上・プロジェクトの外は拒否）。
//! 3. 違反なら**デバッグビルドは `panic!`、リリースビルドは書かずに E060**（§13.1.2）。
//!    ユーザーが選んだパスは、書く前に panic しない [`FsGate::check`] で確かめる（第 2.9 版）。
//!
//! 書ける場所は「今開いているプロジェクトのルート配下」と「履歴ファイル 1 個」（§13.1.1）だけで、
//! 履歴ファイルは [`FsGate::write_history`] からしか書けない（他の口は履歴を許さない）。履歴の
//! 置き場所は [`FsGate::new`] が環境から決める（§13.2.1。引数でパスを渡す口は無い）。
//!
//! **特例**（§13.1.1、ユーザー裁定 2026-10-13・第 2.10 / 2.11 版、`docs/history.md` B-99・B-109）:
//! エクスポートの保存先としてユーザーが明示したパス（[`ChosenPath`]。保存ダイアログと
//! `novnow export --out`）は [`FsGate::write_chosen_export`] からだけ、プロジェクトの外でも書ける。
//! 種類ごとの拡張子（Zip は `.zip`、テキストは `.txt` / `.md`）のファイル 1 個・その 1 回だけ
//! （パスは覚えない）・一時ファイルを使わず直接・親ディレクトリは作らない。他の口は特例を使えない
//! （`WritePolicy::check` が特例を知らない）。
//!
//! # 原子的書き込み（§8.3）
//!
//! 同じディレクトリに `.<name>.novnow-tmp` を作る → 全部書く → `fsync` → `rename`。一時ファイルも
//! プロジェクト内（§13.1.1）。失敗したら一時ファイルを消して `Err`（呼び出し側は編集内容を
//! メモリに持ったまま再試行する、§8.3）。**履歴ファイルだけは一時ファイルを使わず直接上書き**する
//! （プロジェクト外に一時ファイルを置けないため。§13.2.2 第 2.9 版、B-92）。

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::UNIX_EPOCH;

use crate::core::policy::write_policy::normalize;
use crate::core::policy::{
    Allowed, FileEntry, PolicyViolation, ProjectPath, Stamp, Violation, WritePolicy,
};
use crate::ports::dialog::ChosenPath;
use crate::ports::env::Env;
use crate::ports::storage::{Storage, StorageError};

use super::recent_file;

/// 原子的保存の一時ファイルの接尾辞（§8.3 `.<name>.novnow-tmp`）。
pub const TMP_SUFFIX: &str = ".novnow-tmp";

/// `.git/`（§14.2.1「`.git/` を読まない」。一覧でも中に入らない）。
pub const GIT_DIR: &str = ".git";

// ============================================================ 関門

/// 書き込みの関門（§13.1.2）。プロセスに 1 つ作り、[`FsStorage`] と履歴（`recent_file`）で共有する。
#[derive(Debug)]
pub struct FsGate {
    policy: RwLock<WritePolicy>,
}

impl FsGate {
    /// 履歴ファイルの置き場所を**環境から**決めて作る（§13.2.1。`NOVNOW_RECENT_FILE` で上書き可）。
    /// プロジェクトはまだ開いていない（書けるのは履歴だけ）。
    pub fn new(env: &impl Env) -> Self {
        let history = recent_file::location(env).and_then(|path| resolve(&path).ok());
        Self {
            policy: RwLock::new(WritePolicy::new(history)),
        }
    }

    /// 今の許可表（写し）。
    pub fn policy(&self) -> WritePolicy {
        self.read_policy().clone()
    }

    /// 履歴ファイル（解決済みの絶対パス）。置き場所が決まらない環境では `None`。
    pub fn history_file(&self) -> Option<PathBuf> {
        self.read_policy().history_file().map(Path::to_path_buf)
    }

    /// 開いているプロジェクトのルート（解決済みの絶対パス）。
    pub fn project_root(&self) -> Option<PathBuf> {
        self.read_policy().project_root().map(Path::to_path_buf)
    }

    /// 既存のディレクトリをプロジェクトとして開く（それまでのプロジェクトは閉じる）。
    /// 解決済みの絶対パス（`canonicalize`）を返す。
    pub fn open_project(&self, root: &Path) -> Result<PathBuf, StorageError> {
        let canonical = fs::canonicalize(root).map_err(|error| io_error(root, &error))?;
        if !canonical.is_dir() {
            return Err(StorageError::Io {
                path: root.to_string_lossy().into_owned(),
                kind: io::ErrorKind::NotADirectory,
                message: "not a directory".to_owned(),
            });
        }
        self.write_policy()
            .open(&canonical)
            .map_err(StorageError::Policy)?;
        Ok(canonical)
    }

    /// 新しいプロジェクトのルートを（無ければ作って）開く（Zip の取り込み先 §13.5・M6 の `novnow new`）。
    pub fn create_project(&self, root: &Path) -> Result<PathBuf, StorageError> {
        let resolved = resolve(root).map_err(|error| io_error(root, &error))?;
        let normal = normalize(&resolved).ok_or_else(|| {
            StorageError::Policy(PolicyViolation {
                target: root.to_path_buf(),
                reason: Violation::Escape,
            })
        })?;
        self.write_policy()
            .open(&normal)
            .map_err(StorageError::Policy)?;
        self.create_dir_all(&normal)?;
        self.open_project(&normal)
    }

    /// プロジェクトを閉じる（以後は履歴しか書けない）。
    pub fn close_project(&self) {
        self.write_policy().close();
    }

    /// プロジェクト内に**ファイルを書いてよいか**の事前判定（**panic しない**）。ユーザーが選んだ
    /// パス（保存先の指定など）はこれで確かめてから書く（§13.1.2 第 2.9 版。違反は E060）。
    /// 書いてよければ解決済みの絶対パスを返す。
    pub fn check(&self, target: &Path) -> Result<PathBuf, PolicyViolation> {
        self.judge_file(target, Allowed::Project)
    }

    /// テキスト・バイト列を原子的に書く（§8.3）。プロジェクト内だけ。
    pub fn write_atomic(&self, target: &Path, bytes: &[u8]) -> Result<Stamp, StorageError> {
        let resolved = self.guard_file(target, Allowed::Project)?;
        let (parent, name) = split(&resolved)?;
        self.create_dir_all(&parent)?;
        let tmp = parent.join(format!(".{name}{TMP_SUFFIX}"));
        // 前回の失敗で残った一時ファイル（やシンボリックリンク）はリンクごと消してから作る
        self.remove_entry_if_exists(&tmp)?;
        let tmp = self.guard_file(&tmp, Allowed::Project)?;
        let written = write_then_rename(&tmp, &resolved, bytes);
        if let Err(error) = written {
            let _ = fs::remove_file(&tmp);
            return Err(io_error(target, &error));
        }
        stamp_of(&resolved).map_err(|error| io_error(target, &error))
    }

    /// 末尾に追記する（`agents/log/*.jsonl`）。プロジェクト内だけ。
    pub fn append(&self, target: &Path, bytes: &[u8]) -> Result<Stamp, StorageError> {
        let resolved = self.guard_file(target, Allowed::Project)?;
        let (parent, _) = split(&resolved)?;
        self.create_dir_all(&parent)?;
        append_bytes(&resolved, bytes).map_err(|error| io_error(target, &error))?;
        stamp_of(&resolved).map_err(|error| io_error(target, &error))
    }

    /// ファイルを消す。シンボリックリンクは**リンク自身**を消す（行き先には触れない）ので、
    /// 判定はリンクが置かれている場所で行う。プロジェクト内だけ。
    pub fn remove_file(&self, target: &Path) -> Result<(), StorageError> {
        let entry = self.guard_entry(target)?;
        fs::remove_file(&entry).map_err(|error| io_error(target, &error))
    }

    /// ディレクトリを（親ごと）作る。プロジェクト内だけ。
    pub fn create_dir_all(&self, dir: &Path) -> Result<(), StorageError> {
        let resolved = self.guard_dir(dir, false)?;
        fs::create_dir_all(&resolved).map_err(|error| io_error(dir, &error))
    }

    /// 履歴ファイルを書く（§13.2）。**一時ファイルを使わず直接上書き**し、親ディレクトリが
    /// 無ければ作る（§13.2.2 第 2.9 版）。置き場所が決まらない環境では [`StorageError::Unsupported`]。
    pub fn write_history(&self, bytes: &[u8]) -> Result<(), StorageError> {
        let Some(history) = self.history_file() else {
            return Err(StorageError::Unsupported);
        };
        let resolved = self.guard_file(&history, Allowed::History)?;
        let (parent, _) = split(&resolved)?;
        let parent = self.guard_dir(&parent, true)?;
        overwrite(&parent, &resolved, bytes).map_err(|error| io_error(&history, &error))
    }

    /// 明示されたエクスポートの保存先の事前判定（**panic しない**。§13.1.1 の特例・§13.1.2）。
    /// 書いてよければ解決済みの絶対パスを返す。種類の拡張子でない・履歴ファイルは
    /// [`Violation::NotAnExport`]（表示は E060）。
    pub fn check_chosen_export(&self, chosen: &ChosenPath) -> Result<PathBuf, PolicyViolation> {
        self.judge_chosen_export(chosen)
            .map(|(resolved, _)| resolved)
    }

    /// 明示された保存先にエクスポートを書く（§13.1.1 の特例、第 2.10 / 2.11 版）。書いた絶対パスを返す。
    ///
    /// `chosen` は**消費する**（許可はこの 1 回だけ。関門はパスを覚えない）。プロジェクトの中なら
    /// ふつうの原子的保存（[`Self::write_atomic`]）。外なら**一時ファイルを使わずに直接上書き**し、
    /// **親ディレクトリは作らない**（無ければ I/O エラー。ダイアログで選べるのは既存のディレクトリ）。
    /// 違反は他の口と同じく debug で panic・release で E060 なので、ダイアログの結果は先に
    /// [`Self::check_chosen_export`] で確かめること（`adapters::zip::export_to_chosen` はそうしている）。
    pub fn write_chosen_export(
        &self,
        chosen: ChosenPath,
        bytes: &[u8],
    ) -> Result<PathBuf, StorageError> {
        let target = chosen.path();
        let (resolved, allowed) = self.judge_chosen_export(&chosen).map_err(deny)?;
        if allowed == Allowed::Project {
            self.write_atomic(&resolved, bytes)?;
            return Ok(resolved);
        }
        let (parent, _) = split(&resolved)?;
        if !parent.is_dir() {
            return Err(StorageError::Io {
                path: target.to_string_lossy().into_owned(),
                kind: io::ErrorKind::NotFound,
                message: "the parent directory does not exist".to_owned(),
            });
        }
        write_in_place(&resolved, bytes).map_err(|error| io_error(target, &error))?;
        Ok(resolved)
    }

    // ---------------------------------------------------------------- 判定

    /// 明示された保存先を解決して判定する（panic しない）。
    fn judge_chosen_export(
        &self,
        chosen: &ChosenPath,
    ) -> Result<(PathBuf, Allowed), PolicyViolation> {
        let target = chosen.path();
        let resolved = resolve(target).map_err(|_| unresolved(target))?;
        let allowed = self
            .read_policy()
            .check_chosen_export(&resolved, chosen.kind())?;
        let normal = normalize(&resolved).ok_or_else(|| PolicyViolation {
            target: target.to_path_buf(),
            reason: Violation::Escape,
        })?;
        Ok((normal, allowed))
    }

    /// ファイルの書き込み先を解決して判定する（panic しない）。`want` の理由で許されたときだけ通す
    /// （履歴の口からはプロジェクト内を、プロジェクトの口からは履歴を書かせない）。
    fn judge_file(&self, target: &Path, want: Allowed) -> Result<PathBuf, PolicyViolation> {
        let resolved = resolve(target).map_err(|_| unresolved(target))?;
        let allowed = self.read_policy().check(&resolved)?;
        if allowed != want {
            return Err(PolicyViolation {
                target: target.to_path_buf(),
                reason: Violation::Outside,
            });
        }
        normalize(&resolved).ok_or_else(|| PolicyViolation {
            target: target.to_path_buf(),
            reason: Violation::Escape,
        })
    }

    /// [`Self::judge_file`] の結果を、違反なら [`deny`] にする。
    fn guard_file(&self, target: &Path, want: Allowed) -> Result<PathBuf, StorageError> {
        self.judge_file(target, want).map_err(deny)
    }

    /// ディレクトリの作成先を判定する（panic しない）。`history` なら履歴ファイルの祖先も許す。
    fn judge_dir(&self, dir: &Path, history: bool) -> Result<PathBuf, PolicyViolation> {
        let resolved = resolve(dir).map_err(|_| unresolved(dir))?;
        let allowed = self.read_policy().check_dir(&resolved)?;
        let ok = allowed == Allowed::Project || (history && allowed == Allowed::HistoryDir);
        if !ok {
            return Err(PolicyViolation {
                target: dir.to_path_buf(),
                reason: Violation::Outside,
            });
        }
        normalize(&resolved).ok_or_else(|| PolicyViolation {
            target: dir.to_path_buf(),
            reason: Violation::Escape,
        })
    }

    /// [`Self::judge_dir`] の結果を、違反なら [`deny`] にする。
    fn guard_dir(&self, dir: &Path, history: bool) -> Result<PathBuf, StorageError> {
        self.judge_dir(dir, history).map_err(deny)
    }

    /// エントリ（最後の要素はシンボリックリンクでも辿らない）の置き場所を判定する（panic しない）。
    fn judge_entry(&self, target: &Path) -> Result<PathBuf, PolicyViolation> {
        let absolute = std::path::absolute(target).map_err(|_| unresolved(target))?;
        let (parent, name) = split_path(&absolute).ok_or_else(|| unresolved(target))?;
        let parent = resolve(&parent).map_err(|_| unresolved(target))?;
        let entry = parent.join(name);
        match self.read_policy().check(&entry)? {
            Allowed::Project => normalize(&entry).ok_or_else(|| PolicyViolation {
                target: target.to_path_buf(),
                reason: Violation::Escape,
            }),
            _ => Err(PolicyViolation {
                target: target.to_path_buf(),
                reason: Violation::Outside,
            }),
        }
    }

    /// [`Self::judge_entry`] の結果を、違反なら [`deny`] にする。
    fn guard_entry(&self, target: &Path) -> Result<PathBuf, StorageError> {
        self.judge_entry(target).map_err(deny)
    }

    /// エントリがあれば消す（無ければ何もしない）。
    fn remove_entry_if_exists(&self, target: &Path) -> Result<(), StorageError> {
        if fs::symlink_metadata(target).is_err() {
            return Ok(());
        }
        let entry = self.guard_entry(target)?;
        match fs::remove_file(&entry) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error(target, &error)),
        }
    }

    fn read_policy(&self) -> RwLockReadGuard<'_, WritePolicy> {
        self.policy.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write_policy(&self) -> RwLockWriteGuard<'_, WritePolicy> {
        self.policy.write().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 違反の扱い（§13.1.2）: **デバッグビルドは `panic!`**、リリースビルドは E060 を返す。
///
/// 判定はロックを外してから呼ぶ（panic でロックが毒にならないように）。panic の文言は
/// [`PolicyViolation`] の `Display`（`E060 write policy violation …`）。
fn deny(violation: PolicyViolation) -> StorageError {
    if cfg!(debug_assertions) {
        panic!("novnow: {violation}");
    }
    StorageError::Policy(violation)
}

fn unresolved(target: &Path) -> PolicyViolation {
    PolicyViolation {
        target: target.to_path_buf(),
        reason: Violation::Unresolved,
    }
}

/// 末尾に足して `fsync` する。
fn append_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_data()
}

/// 親ディレクトリを作り、一時ファイルを使わずに上書きする（履歴ファイル、§13.2.2 第 2.9 版）。
fn overwrite(parent: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(parent)?;
    write_in_place(path, bytes)
}

/// 一時ファイルを使わずに上書きして `fsync` する（プロジェクトの外には一時ファイルを置けない、
/// §13.1.1。履歴ファイルと保存ダイアログで指定された Zip）。
fn write_in_place(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// 一時ファイルに書いて `fsync` し、`rename` で置き換える（§8.3）。
fn write_then_rename(tmp: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    // 置き換える前のファイルの権限を引き継ぐ（chmod 600 の原稿が 644 に戻らないように）
    if let Ok(metadata) = fs::metadata(target) {
        let _ = fs::set_permissions(tmp, metadata.permissions());
    }
    fs::rename(tmp, target)?;
    sync_dir(target);
    Ok(())
}

/// `rename` を確定させるためにディレクトリを `fsync` する（Unix だけ。失敗は無視する）。
fn sync_dir(target: &Path) {
    #[cfg(unix)]
    if let Some(parent) = target.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = target;
}

// ============================================================ パスの解決

/// 書き込み先を**シンボリックリンク解決後の絶対パス**にする（§13.1.2）。
///
/// 存在する最も深い祖先を `canonicalize` し、まだ無い残りの要素を字句のままつなぐ（無い要素は
/// シンボリックリンクでありえないので、残りの `..` は core の字句の正規化で正しく処理できる）。
/// 存在する祖先が壊れたシンボリックリンクなら `Err`（行き先を確かめられない）。
pub fn resolve(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let components: Vec<Component<'_>> = absolute.components().collect();
    for split in (1..=components.len()).rev() {
        let prefix: PathBuf = components[..split].iter().collect();
        match fs::symlink_metadata(&prefix) {
            Ok(_) => {
                let mut resolved = fs::canonicalize(&prefix)?;
                for component in &components[split..] {
                    resolved.push(component.as_os_str());
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(absolute)
}

/// 親ディレクトリとファイル名に分ける。
fn split_path(path: &Path) -> Option<(PathBuf, String)> {
    let name = path.file_name()?.to_str()?.to_owned();
    Some((path.parent()?.to_path_buf(), name))
}

fn split(path: &Path) -> Result<(PathBuf, String), StorageError> {
    split_path(path).ok_or_else(|| StorageError::Io {
        path: path.to_string_lossy().into_owned(),
        kind: io::ErrorKind::InvalidInput,
        message: "no file name".to_owned(),
    })
}

fn stamp_of(path: &Path) -> io::Result<Stamp> {
    let metadata = fs::metadata(path)?;
    Ok(stamp_from(&metadata))
}

fn stamp_from(metadata: &fs::Metadata) -> Stamp {
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok());
    Stamp {
        modified_ms,
        len: metadata.len(),
    }
}

fn io_error(path: &Path, error: &io::Error) -> StorageError {
    StorageError::Io {
        path: path.to_string_lossy().into_owned(),
        kind: error.kind(),
        message: error.to_string(),
    }
}

// ============================================================ プロジェクトのストレージ

/// ネイティブのプロジェクト 1 つ分のストレージ（[`Storage`] の実装）。
///
/// 書き込みは全部 [`FsGate`] を通る。関門が別のプロジェクトを開くと、古い `FsStorage` からは
/// 書けなくなる（§13.1.1「**現在開いている**プロジェクトルート配下」）。
#[derive(Debug, Clone)]
pub struct FsStorage {
    root: PathBuf,
    gate: Arc<FsGate>,
}

impl FsStorage {
    /// 既存のディレクトリを開く（関門の許可をこのルートに切り替える）。
    pub fn open(gate: Arc<FsGate>, root: &Path) -> Result<Self, StorageError> {
        let root = gate.open_project(root)?;
        Ok(Self { root, gate })
    }

    /// 新しいルート（無ければ作る）を開く（Zip の取り込み先・`novnow new`）。
    pub fn create(gate: Arc<FsGate>, root: &Path) -> Result<Self, StorageError> {
        let root = gate.create_project(root)?;
        Ok(Self { root, gate })
    }

    /// 新しいルートを排他的に確保する。既にあるディレクトリへは決して相乗りしない。
    /// 親は既に在ること。書き込みに失敗すると中途半端なプロジェクトが残りうる。
    pub fn create_fresh(gate: Arc<FsGate>, root: &Path) -> Result<Self, StorageError> {
        let parent = root
            .parent()
            .ok_or_else(|| io_error(root, &io::Error::other("missing parent")))?;
        let parent = std::fs::canonicalize(parent).map_err(|e| io_error(parent, &e))?;
        let name = root
            .file_name()
            .ok_or_else(|| io_error(root, &io::Error::other("missing name")))?;
        let root = parent.join(name);
        match std::fs::symlink_metadata(&root) {
            Ok(_) => {
                return Err(io_error(
                    &root,
                    &io::Error::from(io::ErrorKind::AlreadyExists),
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(&root, &error)),
        }
        gate.write_policy()
            .open(&root)
            .map_err(StorageError::Policy)?;
        gate.check(&root).map_err(StorageError::Policy)?;
        std::fs::create_dir(&root).map_err(|e| io_error(&root, &e))?;
        Self::open(gate, &root)
    }

    /// ルート（解決済みの絶対パス）。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 関門。
    pub fn gate(&self) -> &Arc<FsGate> {
        &self.gate
    }

    /// 相対パスのネイティブのパス。
    pub fn absolute(&self, path: &ProjectPath) -> PathBuf {
        path.under(&self.root)
    }

    /// あるか（ファイル・ディレクトリを問わない）。
    pub fn exists(&self, path: &ProjectPath) -> bool {
        fs::symlink_metadata(self.absolute(path)).is_ok()
    }

    /// 全ファイルの一覧（[`Storage::list`] の同期版。Zip のエクスポート等）。
    pub fn list_files(&self) -> Result<Vec<FileEntry>, StorageError> {
        self.walk().map(|(files, _)| files)
    }

    /// 全ディレクトリの一覧（[`Storage::dirs`] の同期版）。
    pub fn list_dirs(&self) -> Result<Vec<ProjectPath>, StorageError> {
        self.walk().map(|(_, dirs)| dirs)
    }

    /// バイト列を読む（Zip のエクスポート・バイナリの資料）。行き先がルートの外のもの（外を指す
    /// シンボリックリンク）は無いものとして扱う（`docs/history.md` B-155）。
    pub fn read_bytes(&self, path: &ProjectPath) -> Result<Vec<u8>, StorageError> {
        let absolute = self.absolute(path);
        if !self.inside(&absolute) {
            return Err(StorageError::NotFound(path.clone()));
        }
        fs::read(&absolute).map_err(|error| self.map_read_error(path, &error))
    }

    /// 構造の移動を 1 つの復旧可能な束として保存する。変える前の全 byte を控え、
    /// 書き込み・削除のどれかが失敗したら `FsGate` 経由で束ごと元に戻す。
    pub fn write_batch(
        &self,
        writes: &[(ProjectPath, String)],
        removals: &[ProjectPath],
    ) -> Result<Vec<(ProjectPath, Stamp)>, String> {
        let mut backups = std::collections::BTreeMap::new();
        for path in writes.iter().map(|(p, _)| p).chain(removals) {
            self.gate
                .guard_file(&self.absolute(path), Allowed::Project)
                .map_err(|e| format!("{e:?}"))?;
            let original = match self.read_bytes(path) {
                Ok(bytes) => Some(bytes),
                Err(StorageError::NotFound(_)) => None,
                Err(e) => return Err(format!("{e:?}")),
            };
            backups.insert(path.clone(), original);
        }
        let result = (|| {
            let mut stamps = Vec::new();
            for (path, text) in writes {
                stamps.push((
                    path.clone(),
                    self.write_bytes(path, text.as_bytes())
                        .map_err(|e| format!("{e:?}"))?,
                ));
            }
            for path in removals {
                match self.gate.remove_file(&self.absolute(path)) {
                    Ok(()) => {}
                    Err(e) if !self.absolute(path).exists() => {
                        let _ = e;
                    }
                    Err(e) => return Err(format!("{e:?}")),
                }
            }
            Ok(stamps)
        })();
        if let Err(error) = result {
            let mut errors = vec![error];
            for (path, bytes) in backups {
                let restored = if let Some(bytes) = bytes {
                    self.write_bytes(&path, &bytes).map(|_| ())
                } else if self.absolute(&path).exists() {
                    self.gate.remove_file(&self.absolute(&path))
                } else {
                    Ok(())
                };
                if let Err(e) = restored {
                    errors.push(format!("rollback {}: {e:?}", path.as_str()));
                }
            }
            return Err(errors.join("; "));
        }
        result
    }

    /// 行き先（リンクを解決した実体）がルートの中か。解決できないものは `true`（読み込みのエラーに任せる）。
    fn inside(&self, absolute: &Path) -> bool {
        fs::canonicalize(absolute).map_or(true, |real| real.starts_with(&self.root))
    }

    /// バイト列を原子的に書く（Zip のエクスポートの書き出し先 `exports/<name>.zip` 等）。
    pub fn write_bytes(&self, path: &ProjectPath, bytes: &[u8]) -> Result<Stamp, StorageError> {
        self.gate.write_atomic(&self.absolute(path), bytes)
    }

    fn map_read_error(&self, path: &ProjectPath, error: &io::Error) -> StorageError {
        if error.kind() == io::ErrorKind::NotFound {
            StorageError::NotFound(path.clone())
        } else {
            io_error(&self.absolute(path), error)
        }
    }

    fn read_text(&self, path: &ProjectPath) -> Result<String, StorageError> {
        let bytes = self.read_bytes(path)?;
        String::from_utf8(bytes).map_err(|_| StorageError::Encoding(path.clone()))
    }

    /// ルートの下を歩く（`.git/` には入らない・シンボリックリンクのディレクトリは辿らない・
    /// ルートの外を指すシンボリックリンクのファイルは除く・一時ファイルは除く・相対パスにできない名前は除く）。
    fn walk(&self) -> Result<(Vec<FileEntry>, Vec<ProjectPath>), StorageError> {
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        let mut pending: Vec<(PathBuf, String)> = vec![(self.root.clone(), String::new())];
        while let Some((dir, prefix)) = pending.pop() {
            let entries = fs::read_dir(&dir).map_err(|error| io_error(&dir, &error))?;
            for entry in entries {
                let entry = entry.map_err(|error| io_error(&dir, &error))?;
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if name == GIT_DIR || name.ends_with(TMP_SUFFIX) {
                    continue;
                }
                let relative = format!("{prefix}{name}");
                let Ok(path) = ProjectPath::new(&relative) else {
                    continue;
                };
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    dirs.push(path);
                    pending.push((entry.path(), format!("{relative}/")));
                } else if let Ok(metadata) = fs::metadata(entry.path()) {
                    // シンボリックリンクはファイルを指すときだけ（ディレクトリは辿らない）。行き先がルートの
                    // 外なら読まない（書けないものはプロジェクトに含めない。§19.8「プロンプトがプロジェクト外から
                    // 読み込まれない」を全ファイルに。`docs/history.md` B-155）
                    if metadata.is_file() && (!kind.is_symlink() || self.inside(&entry.path())) {
                        files.push(FileEntry {
                            path,
                            stamp: stamp_from(&metadata),
                        });
                    }
                }
            }
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        dirs.sort();
        Ok((files, dirs))
    }
}

impl Storage for FsStorage {
    async fn read_binary(&self, path: &ProjectPath) -> Result<Vec<u8>, StorageError> {
        self.read_bytes(path)
    }
    async fn list(&self) -> Result<Vec<FileEntry>, StorageError> {
        self.list_files()
    }

    async fn dirs(&self) -> Result<Option<Vec<ProjectPath>>, StorageError> {
        self.list_dirs().map(Some)
    }

    async fn read(&self, path: &ProjectPath) -> Result<String, StorageError> {
        self.read_text(path)
    }

    async fn write(&self, path: &ProjectPath, text: &str) -> Result<Stamp, StorageError> {
        self.gate
            .write_atomic(&self.absolute(path), text.as_bytes())
    }

    async fn append(&self, path: &ProjectPath, text: &str) -> Result<Stamp, StorageError> {
        self.gate.append(&self.absolute(path), text.as_bytes())
    }

    async fn remove(&self, path: &ProjectPath) -> Result<(), StorageError> {
        if !self.exists(path) {
            return Err(StorageError::NotFound(path.clone()));
        }
        self.gate.remove_file(&self.absolute(path))
    }
}
