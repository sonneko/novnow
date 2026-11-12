//! ファイルシステムを後ろに持つ `Host`（§19.2.1 の既定ホスト）。ネイティブ専用。
//!
//! GUI の `NativeHost`（`REF:src/ui/app.rs`）と同じ口を、描画・ダイアログ・egui なしで使う。
//! 書き込みはすべて `FsStorage` → `FsGate`（規則 10・§13.1.2）を通り、外部変更の監視は
//! `REF:src/adapters/project_load.rs` の `poll` を使う。

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use crate::adapters::block_on;
use crate::adapters::clock_system::{self, SystemClock};
use crate::adapters::env_system::SystemEnv;
use crate::adapters::mcp_stdio::StdioClient;
use crate::adapters::project_load;
use crate::adapters::storage_fs::{FsGate, FsStorage};
use crate::app::agents::{Finished, Job};
use crate::app::error::{Error, Result};
use crate::app::host::{save_now, ExternalChange, Host, ReadFailure, SaveOutcome};
use crate::core::policy::external_change::{ChangeKind, Snapshot};
use crate::core::policy::write_policy::ProjectPath;
use crate::core::text::lexer;
use crate::ports::clock::Clock;
use crate::ports::dialog::ChosenPath;
use crate::ports::env::Env;
use crate::ports::mcp::McpClient;
use crate::ports::storage::{Storage, StorageError};

/// LLM 呼び出しのワーカ（`mcp_start` / `mcp_poll`）。
struct Workers {
    next: u64,
    sender: mpsc::Sender<Finished>,
    receiver: mpsc::Receiver<Finished>,
    cancels: Vec<(u64, Arc<AtomicBool>)>,
}

impl Workers {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            next: 1,
            sender,
            receiver,
            cancels: Vec::new(),
        }
    }

    fn start(&mut self, job: Job) -> Option<u64> {
        let number = self.next;
        self.next += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("novnow-api-{number}"))
            .spawn(move || {
                let started = std::time::Instant::now();
                let result = StdioClient.call(&job.server, &job.call, &flag);
                let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                let _ = sender.send((number, result, elapsed));
            });
        if spawned.is_err() {
            return None;
        }
        self.cancels.push((number, cancel));
        Some(number)
    }

    fn poll(&mut self) -> Vec<Finished> {
        let done: Vec<Finished> = self.receiver.try_iter().collect();
        self.cancels
            .retain(|(number, _)| !done.iter().any(|(finished, _, _)| finished == number));
        done
    }

    fn cancel(&mut self, number: u64) {
        if let Some((_, flag)) = self.cancels.iter().find(|(held, _)| *held == number) {
            flag.store(true, Ordering::SeqCst);
        }
        self.cancels.retain(|(held, _)| *held != number);
    }
}

/// 実ファイルシステムの `Host`。
pub struct FsHost {
    storage: FsStorage,
    snapshot: Snapshot,
    mcp: Workers,
    probes: Workers,
    env: SystemEnv,
}

impl FsHost {
    /// 既存のプロジェクトを開く（§14.2 の検査は呼び出し側）。
    pub fn open(root: &Path) -> Result<Self> {
        let gate = Arc::new(FsGate::new(&SystemEnv));
        let storage = FsStorage::open(gate, root)
            .map_err(|error| Error::io(format!("{error:?}"), Some(&display(root))))?;
        Ok(Self::with_storage(storage))
    }

    /// 新しいプロジェクトのディレクトリを作る（`novnow new` と同じ構造は呼び出し側が書く）。
    pub fn create(root: &Path) -> Result<Self> {
        let gate = Arc::new(FsGate::new(&SystemEnv));
        let storage = FsStorage::create(gate, root)
            .map_err(|error| Error::io(format!("{error:?}"), Some(&display(root))))?;
        Ok(Self::with_storage(storage))
    }

    fn with_storage(storage: FsStorage) -> Self {
        let snapshot = block_on(storage.list())
            .map(Snapshot::from_entries)
            .unwrap_or_default();
        Self {
            storage,
            snapshot,
            mcp: Workers::new(),
            probes: Workers::new(),
            env: SystemEnv,
        }
    }

    /// プロジェクトのルート（絶対パス）。
    pub fn root(&self) -> &Path {
        self.storage.root()
    }

    /// ストレージ（Zip の書き出し・一括読み込み用）。
    pub fn storage(&self) -> &FsStorage {
        &self.storage
    }

    fn path(&self, path: &str) -> Result<ProjectPath> {
        ProjectPath::new(path).map_err(|error| {
            Error::usage(crate::ui::i18n::Locale::Ja, &format!("{path}: {error:?}"))
        })
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// 外部変更 1 件を読む（§8.5）。GUI の `Host`（`REF:src/ui/picker.rs`）と同じ 1 実装を共有する。
///
/// **読み取りの失敗と UTF-8 復号の失敗は削除と区別する**（[`ExternalChange::Unreadable`]）。
/// どちらも利用者の削除操作ではないので、これで文書を消してはいけない（§4.3 の encoding /
/// I/O エラーの扱い）。一覧に出た直後に消えた競合だけは「消えた」として扱う（次の監視を待たない）。
pub fn external_change(
    storage: &FsStorage,
    path: &ProjectPath,
    kind: ChangeKind,
) -> ExternalChange {
    let name = path.as_str().to_owned();
    if kind == ChangeKind::Removed {
        return ExternalChange::Removed(name);
    }
    match storage.read_bytes(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => ExternalChange::Updated(name, lexer::normalize_on_load(&text).into_owned()),
            Err(error) => {
                ExternalChange::Unreadable(name, ReadFailure::Encoding(error.to_string()))
            }
        },
        Err(StorageError::NotFound(_)) => ExternalChange::Removed(name),
        Err(error) => ExternalChange::Unreadable(name, ReadFailure::Io(format!("{error:?}"))),
    }
}

impl Host for FsHost {
    fn writable(&self) -> bool {
        self.storage.writable()
    }

    fn write(&mut self, path: &str, text: &str) -> std::result::Result<(), String> {
        let target = self.path(path).map_err(|error| error.message)?;
        block_on(self.storage.write(&target, text))
            .map(|_| ())
            .map_err(|error| format!("{error:?}"))
    }

    fn poll(&mut self) -> Vec<ExternalChange> {
        let Ok(changes) = block_on(project_load::poll(&self.storage, &mut self.snapshot)) else {
            return Vec::new();
        };
        changes
            .into_iter()
            .filter(|change| project_load::is_document(&change.path))
            .map(|change| external_change(&self.storage, &change.path, change.kind))
            .collect()
    }

    /// 保存 1 回分（§8.3・§19.1.6）。
    ///
    /// 2 つ以上のファイルにまたがるときは [`FsStorage::write_batch`]（変える前の全 byte の
    /// 控え + rollback）で**原子的**に行う: 途中の失敗ではどのファイルも変更されていない状態へ
    /// 戻し、戻せなかったものだけ失敗として報告する。1 ファイルだけの保存は `write_atomic`
    /// （一時ファイル + rename）でそもそも原子的なので、既定の実装（[`save_now`]）を使う。
    fn save(
        &mut self,
        writes: Vec<(String, String)>,
        removals: Vec<String>,
    ) -> Option<SaveOutcome> {
        if writes.len() + removals.len() <= 1 {
            return Some(save_now(self, writes, removals));
        }
        let mut targets: Vec<ProjectPath> = Vec::with_capacity(writes.len() + removals.len());
        for path in writes.iter().map(|(path, _)| path).chain(removals.iter()) {
            match self.path(path) {
                Ok(target) => targets.push(target),
                // 関門が拒むパス（§13.1.1）: 1 つも書かない・消さない。
                Err(error) => {
                    return Some(SaveOutcome {
                        failures: vec![(path.clone(), error.message)],
                        removals_left: removals,
                        ..SaveOutcome::default()
                    })
                }
            }
        }
        let written: Vec<String> = writes.iter().map(|(path, _)| path.clone()).collect();
        let texts: Vec<String> = writes.into_iter().map(|(_, text)| text).collect();
        let (write_targets, remove_targets) = targets.split_at(written.len());
        let batch: Vec<(ProjectPath, String)> = write_targets.iter().cloned().zip(texts).collect();
        let doomed = remove_targets.to_vec();
        match self.storage.write_batch(&batch, &doomed) {
            Ok(stamps) => {
                for (path, stamp) in stamps {
                    // 自分の書き込み・削除を外部変更として拾わない（§8.5）。
                    self.snapshot.acknowledge(path, stamp);
                }
                for path in &doomed {
                    self.snapshot.forget(path);
                }
                Some(SaveOutcome {
                    written,
                    ..SaveOutcome::default()
                })
            }
            Err(failure) => Some(SaveOutcome {
                failures: failure.failures(),
                removals_left: removals,
                ..SaveOutcome::default()
            }),
        }
    }

    fn name(&self) -> String {
        self.storage
            .root()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn stamp(&self) -> String {
        let clock = SystemClock;
        clock_system::compact_stamp(clock.now_millis(), clock.utc_offset_minutes())
    }

    fn seconds(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |found| found.as_secs_f64())
    }

    fn now(&self) -> String {
        clock_system::now_rfc3339(&SystemClock)
    }

    fn remove(&mut self, path: &str) -> std::result::Result<(), String> {
        let target = self.path(path).map_err(|error| error.message)?;
        block_on(self.storage.remove(&target)).map_err(|error| format!("{error:?}"))
    }

    fn write_chosen(
        &mut self,
        path: &str,
        bytes: &[u8],
        kind: crate::ports::dialog::ExportKind,
    ) -> std::result::Result<String, String> {
        let chosen = ChosenPath::from_command_line(std::path::PathBuf::from(path), kind);
        let written = self
            .storage
            .gate()
            .write_chosen_export(chosen, bytes)
            .map_err(|error| format!("{error:?}"))?;
        Ok(written.to_string_lossy().into_owned())
    }

    fn append(&mut self, path: &str, text: &str) -> std::result::Result<(), String> {
        let target = self.path(path).map_err(|error| error.message)?;
        block_on(self.storage.append(&target, text)).map_err(|error| format!("{error:?}"))?;
        Ok(())
    }

    fn mcp_available(&self) -> bool {
        true
    }

    fn mcp_start(&mut self, job: Job) -> Option<u64> {
        self.mcp.start(job)
    }

    fn mcp_poll(&mut self) -> Vec<Finished> {
        self.mcp.poll()
    }

    fn local_probe_start(&mut self, job: Job) -> Option<u64> {
        self.probes.start(job)
    }

    fn local_probe_poll(&mut self) -> Vec<Finished> {
        self.probes.poll()
    }

    fn mcp_cancel(&mut self, job: u64) {
        self.mcp.cancel(job);
        self.probes.cancel(job);
    }

    fn env_var(&self, name: &str) -> Option<String> {
        self.env.var(name)
    }

    fn occupied(&self, path: &str) -> bool {
        let Ok(target) = self.path(path) else {
            return false;
        };
        self.storage.exists(&target)
    }
}
