//! メモリ上の `Host`（§19.2.1 の `Session::with_host`）。**egui に依存しない**。
//!
//! ファイルシステムに一切触れずにセッションの全工程（開く → 編集 → 外部変更 → 衝突解決 →
//! 保存 → イベント確認）を検証するためのホスト。GUI のテスト（`REF:tests/gui.rs`）が使って
//! いたものを公開 API へ昇格した（§19.2.1「テスト用ホストはメモリ」）。

use std::path::PathBuf;

use crate::app::host::{save_now, ExportFailure, ExternalChange, Host, ReadFailure, SaveOutcome};
use crate::ports::dialog::ChosenPath;

/// [`MemoryHost`] の時計（`YYYYMMDD-HHMM`。テストで既定のファイル名を決め打ちできるように固定）。
pub const MEMORY_STAMP: &str = "20261013-2140";

/// [`MemoryHost`] の時計（RFC 3339）。
pub const MEMORY_NOW: &str = "2026-10-13T21:40:00+09:00";

/// メモリの `Host`（テストと、ファイルを持たない環境の確認用）。中身は `Rc` で共有するので、
/// `Session` に渡した後も外から読める。
#[derive(Debug, Default, Clone)]
pub struct MemoryHost {
    /// 共有の中身。
    pub files: std::rc::Rc<std::cell::RefCell<MemoryFiles>>,
}

/// [`MemoryHost`] の中身。
#[derive(Debug, Default)]
pub struct MemoryFiles {
    /// 書かれたファイル（パス → 中身）。
    pub written: std::collections::BTreeMap<String, String>,
    /// 書き込みの回数。
    pub writes: usize,
    /// 書き込みを失敗させる。
    pub fail: bool,
    /// N 番目（0 始まり）の**書き込みの呼び出し**だけを失敗させる（§19.1.6 の原子性の検査）。
    pub fail_write_at: Option<usize>,
    /// N 番目（0 始まり）の**削除の呼び出し**だけを失敗させる。
    pub fail_remove_at: Option<usize>,
    /// 書き込みの呼び出し回数（成功・失敗を問わない。`fail_write_at` の番号）。
    pub write_calls: usize,
    /// 削除の呼び出し回数（成功・失敗を問わない。`fail_remove_at` の番号）。
    pub remove_calls: usize,
    /// 次の監視で「変わったが読めなかった」ものとして返すもの（§8.5。削除とは区別する）。
    pub unreadable: Vec<(String, ReadFailure)>,
    /// 次の監視で外部変更として返すもの。
    pub changes: Vec<(String, Option<String>)>,
    /// 次の「名前を付けて保存」のダイアログで選ばれるパス（`None` なら取り消し）。
    pub chosen: Option<String>,
    /// 「名前を付けて保存」で書かれたもの（パス → 中身）。
    pub exported: std::collections::BTreeMap<String, String>,
    /// 消したファイル（順に）。
    pub removed: Vec<String>,
    /// 追記されたもの（監査ログ。パス → 中身）。
    pub appended: std::collections::BTreeMap<String, String>,
    /// LLM を呼べる環境にする（MCP の台本を使う）。
    pub mcp: bool,
    /// MCP の台本: 呼び出しが終わるたびに先頭から 1 つずつ返す（無ければ `text.`）。
    pub replies: std::collections::VecDeque<
        Result<crate::ports::mcp::McpReply, crate::ports::mcp::McpError>,
    >,
    /// 始まった呼び出し（順に。番号は 1 から）。
    pub jobs: Vec<crate::app::agents::Job>,
    /// まだ終わっていない呼び出しの番号。
    pub pending: Vec<u64>,
    /// 呼び出しを終わらせずに止めておく（実行中の表示・並列の確かめ）。
    pub hold: bool,
    /// 取り消された呼び出しの番号。
    pub canceled: Vec<u64>,
    /// 環境変数。
    pub env: std::collections::BTreeMap<String, String>,
    /// 一覧に無いがディスクにあることにするパス（外を指すシンボリックリンクの代わり）。
    pub occupied: Vec<String>,
    /// 保存を非同期にする（Web の IndexedDB の代わり。`save` は始めるだけで、[`MemoryFiles::release_save`]
    /// で終わらせる。B-156）。
    pub async_saves: bool,
    /// 始まったまま終わっていない保存（`async_saves`）。
    pub held_save: Option<HeldSave>,
    /// 終わった保存の結果（次のフレームで `save_finished` が返す）。
    pub finished_save: Option<SaveOutcome>,
}

/// [`MemoryFiles::held_save`] の中身（書くものと、消すもの）。
pub type HeldSave = (Vec<(String, String)>, Vec<String>);

impl MemoryFiles {
    /// 1 ファイルを書く（[`Host::write`] と同じ失敗の注入を通す）。
    fn put(&mut self, path: &str, text: &str) -> Result<(), String> {
        let index = self.write_calls;
        self.write_calls += 1;
        // `fail` は全部、`fail_write_at` は N 番目の呼び出しだけを失敗させる（失敗の注入）。
        if self.fail || self.fail_write_at == Some(index) {
            return Err("disk full".to_owned());
        }
        self.writes += 1;
        self.written.insert(path.to_owned(), text.to_owned());
        Ok(())
    }

    /// 1 ファイルを消す（[`Host::remove`] と同じ失敗の注入を通す）。
    fn drop_file(&mut self, path: &str) -> Result<(), String> {
        let index = self.remove_calls;
        self.remove_calls += 1;
        if self.fail_remove_at == Some(index) {
            return Err(format!("cannot remove {path}"));
        }
        self.written.remove(path);
        self.removed.push(path.to_owned());
        Ok(())
    }

    /// 控えへ戻す（ロールバック）。メモリなので戻せないことは無い。
    fn roll_back(&mut self, backup: Vec<(String, Option<String>)>) {
        for (path, text) in backup {
            let existed = text.is_some();
            match text {
                Some(text) => {
                    self.written.insert(path.clone(), text);
                }
                None => {
                    self.written.remove(&path);
                }
            }
            if existed {
                // 戻した削除は「消した」事実が無かったことになる。
                self.removed.retain(|removed| removed != &path);
            }
        }
    }

    /// 止めておいた保存を終わらせる（書き込みは `fail` / `fail_write_at` に従う）。
    ///
    /// 2 つ以上のファイルにまたがるときは原子性を持つ（§19.1.6）: 失敗したら成功済みの
    /// 書き込み・削除も元へ戻す（部分コミットを残さない）。
    pub fn release_save(&mut self) {
        let Some((writes, removals)) = self.held_save.take() else {
            return;
        };
        let backup: Option<Vec<(String, Option<String>)>> = (writes.len() + removals.len() > 1)
            .then(|| {
                writes
                    .iter()
                    .map(|(path, _)| path)
                    .chain(removals.iter())
                    .map(|path| (path.clone(), self.written.get(path).cloned()))
                    .collect()
            });
        let mut outcome = SaveOutcome::default();
        for (path, text) in &writes {
            match self.put(path, text) {
                Ok(()) => outcome.written.push(path.clone()),
                Err(message) => outcome.failures.push((path.clone(), message)),
            }
        }
        let mut left: Vec<String> = Vec::new();
        if outcome.failures.is_empty() {
            for path in &removals {
                if let Err(message) = self.drop_file(path) {
                    outcome.failures.push((path.clone(), message));
                    left.push(path.clone());
                }
            }
        }
        if !outcome.failures.is_empty() {
            match backup {
                Some(backup) => {
                    outcome.written.clear();
                    self.roll_back(backup);
                    outcome.removals_left = removals;
                }
                None => {
                    outcome.removals_left = if left.is_empty() { removals } else { left };
                }
            }
        }
        self.finished_save = Some(outcome);
    }
}

impl Host for MemoryHost {
    /// テスト用: `fail` を立てると読み取り専用のプロジェクトとして振る舞う（§4.1 の W041）。
    fn writable(&self) -> bool {
        !self.files.borrow().fail
    }

    fn write(&mut self, path: &str, text: &str) -> Result<(), String> {
        self.files.borrow_mut().put(path, text)
    }

    fn poll(&mut self) -> Vec<ExternalChange> {
        let (changes, unreadable) = {
            let mut files = self.files.borrow_mut();
            (
                std::mem::take(&mut files.changes),
                std::mem::take(&mut files.unreadable),
            )
        };
        changes
            .into_iter()
            .map(|(path, text)| match text {
                Some(text) => ExternalChange::Updated(path, text),
                None => ExternalChange::Removed(path),
            })
            .chain(
                unreadable
                    .into_iter()
                    .map(|(path, failure)| ExternalChange::Unreadable(path, failure)),
            )
            .collect()
    }

    fn backup(&mut self, paths: &[String]) -> Option<Vec<(String, Option<String>)>> {
        let files = self.files.borrow();
        Some(
            paths
                .iter()
                .map(|path| (path.clone(), files.written.get(path).cloned()))
                .collect(),
        )
    }

    fn restore(&mut self, backup: Vec<(String, Option<String>)>) -> Vec<(String, String)> {
        self.files.borrow_mut().roll_back(backup);
        Vec::new()
    }

    fn save(
        &mut self,
        writes: Vec<(String, String)>,
        removals: Vec<String>,
    ) -> Option<SaveOutcome> {
        if self.files.borrow().async_saves {
            self.files.borrow_mut().held_save = Some((writes, removals));
            return None;
        }
        Some(save_now(self, writes, removals))
    }

    fn save_finished(&mut self) -> Option<SaveOutcome> {
        self.files.borrow_mut().finished_save.take()
    }

    fn name(&self) -> String {
        "memory".into()
    }

    fn stamp(&self) -> String {
        MEMORY_STAMP.into()
    }

    fn now(&self) -> String {
        MEMORY_NOW.into()
    }

    fn remove(&mut self, path: &str) -> Result<(), String> {
        self.files.borrow_mut().drop_file(path)
    }

    fn choose_export(&mut self, suggested: &str) -> Option<ChosenPath> {
        let _ = suggested;
        let path = self.files.borrow_mut().chosen.take()?;
        Some(ChosenPath::from_save_dialog(
            PathBuf::from(path),
            crate::ports::dialog::ExportKind::Text,
        ))
    }

    fn write_export(&mut self, chosen: ChosenPath, text: &str) -> Result<String, ExportFailure> {
        let name = chosen.path().to_string_lossy().into_owned();
        // 関門と同じ名前の規則（§13.1.1 の特例: `.txt` / `.md` だけ）
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".txt") && !lower.ends_with(".md") {
            let violation = crate::core::policy::PolicyViolation {
                target: chosen.path().to_path_buf(),
                reason: crate::core::policy::Violation::NotAnExport,
            };
            return Err(ExportFailure::Refused(violation.diagnostic()));
        }
        let mut files = self.files.borrow_mut();
        if files.fail {
            return Err(ExportFailure::Failed("disk full".into()));
        }
        files.exported.insert(name.clone(), text.to_owned());
        Ok(name)
    }

    fn write_chosen(
        &mut self,
        path: &str,
        bytes: &[u8],
        kind: crate::ports::dialog::ExportKind,
    ) -> Result<String, String> {
        if !kind.accepts(std::path::Path::new(path)) {
            return Err("E060".into());
        }
        let text = String::from_utf8(bytes.to_vec()).map_err(|error| error.to_string())?;
        let mut files = self.files.borrow_mut();
        if files.fail {
            return Err("disk full".into());
        }
        files.exported.insert(path.to_owned(), text);
        Ok(path.to_owned())
    }

    fn append(&mut self, path: &str, text: &str) -> Result<(), String> {
        let mut files = self.files.borrow_mut();
        if files.fail {
            return Err("disk full".into());
        }
        files
            .appended
            .entry(path.to_owned())
            .or_default()
            .push_str(text);
        Ok(())
    }

    fn mcp_available(&self) -> bool {
        self.files.borrow().mcp
    }

    fn mcp_start(&mut self, job: crate::app::agents::Job) -> Option<u64> {
        let mut files = self.files.borrow_mut();
        if !files.mcp {
            return None;
        }
        files.jobs.push(job);
        let number = files.jobs.len() as u64;
        files.pending.push(number);
        Some(number)
    }

    fn mcp_poll(&mut self) -> Vec<crate::app::agents::Finished> {
        let mut files = self.files.borrow_mut();
        if files.hold {
            return Vec::new();
        }
        let pending = std::mem::take(&mut files.pending);
        pending
            .into_iter()
            .map(|number| {
                let reply = files.replies.pop_front().unwrap_or_else(|| {
                    Ok(crate::ports::mcp::McpReply {
                        text: "text.".to_owned(),
                        model: "memory".to_owned(),
                        truncated: false,
                    })
                });
                (number, reply, 10)
            })
            .collect()
    }

    fn mcp_cancel(&mut self, job: u64) {
        let mut files = self.files.borrow_mut();
        files.pending.retain(|number| *number != job);
        files.canceled.push(job);
    }

    fn env_var(&self, name: &str) -> Option<String> {
        self.files.borrow().env.get(name).cloned()
    }

    fn occupied(&self, path: &str) -> bool {
        self.files
            .borrow()
            .occupied
            .iter()
            .any(|found| found == path)
    }
}
