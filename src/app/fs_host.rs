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
use crate::app::host::Host;
use crate::core::policy::external_change::{ChangeKind, Snapshot};
use crate::core::policy::write_policy::ProjectPath;
use crate::core::text::lexer;
use crate::ports::clock::Clock;
use crate::ports::dialog::ChosenPath;
use crate::ports::env::Env;
use crate::ports::mcp::McpClient;
use crate::ports::storage::Storage;

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

    fn poll(&mut self) -> Vec<(String, Option<String>)> {
        let Ok(changes) = block_on(project_load::poll(&self.storage, &mut self.snapshot)) else {
            return Vec::new();
        };
        changes
            .into_iter()
            .filter(|change| project_load::is_document(&change.path))
            .map(|change| {
                let text = match change.kind {
                    ChangeKind::Removed => None,
                    _ => self
                        .storage
                        .read_bytes(&change.path)
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                        .map(|text| lexer::normalize_on_load(&text).into_owned()),
                };
                (change.path.as_str().to_owned(), text)
            })
            .collect()
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
