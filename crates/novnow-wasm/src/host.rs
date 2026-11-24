//! ブラウザ用の `Host`（メモリ正本＋書き込みキュー。設計 §21.3）。
//!
//! `REF:src/app/memory_host.rs` の `MemoryHost` を土台にしている（`REF:docs/plans.md#webui-w2`）。
//! 違いは次の 3 点だけ。
//!
//! | 点 | `MemoryHost`（テスト） | ここ（ブラウザ） |
//! |---|---|---|
//! | 置き場所 | `Rc<RefCell<MemoryFiles>>` | [`FileStore`]（メモリ＋IndexedDB のキュー） |
//! | 時計 | 固定の文字列 | ブラウザの今の時刻（`Date.now()`） |
//! | 保存の書き込み先 | メモリの地図 | メモリの正本（IndexedDB へは [`crate::NovnowSession::flush`] が流す） |
//!
//! `Session` は同期なので、[`Host::write`] は**必ず成功を返す**（メモリの正本は必ず書ける）。
//! IndexedDB の失敗は `write` の戻り値に混ぜず、[`NovnowSession::flush`](crate::NovnowSession::flush)
//! の戻り値と `SaveStateChanged(state=error)` で知らせる（`REF:docs/developers.md` §8.10 の 30）。
//!
//! 外部変更の監視は**しない**（§21.2.3・§13.3 のブラウザの規則）。MCP も呼べない（§15.8.2）。

use std::collections::BTreeMap;
use std::rc::Rc;

use novnow::adapters::clock_system;
use novnow::app::host::{save_now, ExternalChange, Host, SaveOutcome};

use crate::storage::FileStore;

/// ブラウザの 1 プロジェクト（メモリ正本＋書き込みキュー）。
pub struct NovnowHost {
    store: Rc<dyn FileStore>,
    /// ヘッダと `project.info` に出す名前。
    name: String,
    /// 開始から経過した秒（保存状態機のタイマ。§9.6）。
    started_ms: f64,
}

impl NovnowHost {
    /// `store` を後ろに持つホスト。
    pub fn new(store: Rc<dyn FileStore>, name: String) -> Self {
        Self {
            store,
            name,
            started_ms: js_sys::Date::now(),
        }
    }
}

impl Host for NovnowHost {
    /// メモリの正本へ書く（**同期**）。IndexedDB へは [`crate::NovnowSession::flush`] が流す。
    fn write(&mut self, path: &str, text: &str) -> Result<(), String> {
        self.store.put(path, text)
    }

    /// ブラウザでは外部変更を監視しない（§21.2.3・§13.3）。
    fn poll(&mut self) -> Vec<ExternalChange> {
        Vec::new()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    /// テキストのエクスポートの既定の名前（`YYYYMMDD-HHMM`。§17.2）。
    fn stamp(&self) -> String {
        let now = js_sys::Date::new_0();
        format!(
            "{:04}{:02}{:02}-{:02}{:02}",
            now.get_full_year(),
            now.get_month() + 1,
            now.get_date(),
            now.get_hours(),
            now.get_minutes()
        )
    }

    /// RFC 3339（アイデアの `created`・資料の `saved_at`。§13.2.2）。
    fn now(&self) -> String {
        let offset = -(js_sys::Date::new_0().get_timezone_offset() as i32);
        clock_system::rfc3339(js_sys::Date::now() as i64, offset)
    }

    /// 単調増加の秒（保存状態機のタイマ、§9.6）。`Instant` は wasm で使えないので
    /// `Date.now()` の差で作る（`REF:docs/developers.md` §8.2 の 7）。
    fn seconds(&self) -> f64 {
        (js_sys::Date::now() - self.started_ms) / 1000.0
    }

    fn remove(&mut self, path: &str) -> Result<(), String> {
        self.store.remove(path)
    }

    /// 利用者が明示した書き出し先へ書く（`export.text --out`・`project.export_zip`）。
    /// ブラウザでは `out` の basename を `exports/` の下に保存し、書いたパスを返す。
    fn write_chosen(
        &mut self,
        path: &str,
        bytes: &[u8],
        kind: novnow::ports::dialog::ExportKind,
    ) -> Result<String, String> {
        if !kind.accepts(std::path::Path::new(path)) {
            return Err("E060".to_owned());
        }
        let text = String::from_utf8(bytes.to_vec())
            .map_err(|error| format!("the export is not UTF-8: {error}"))?;
        let stored = format!("exports/{}", file_name(path));
        self.store.put(&stored, &text)?;
        Ok(path.to_owned())
    }

    fn append(&mut self, path: &str, text: &str) -> Result<(), String> {
        let mut current = self
            .store
            .files()
            .into_iter()
            .find(|(found, _)| found == path)
            .map(|(_, text)| text)
            .unwrap_or_default();
        current.push_str(text);
        self.store.put(path, &current)
    }

    /// メモリの正本があるので、控えも戻しもできる（§19.1.6 の原子性）。
    fn backup(&mut self, paths: &[String]) -> Option<Vec<(String, Option<String>)>> {
        let files: BTreeMap<String, String> = self.store.files().into_iter().collect();
        Some(
            paths
                .iter()
                .map(|path| (path.clone(), files.get(path).cloned()))
                .collect(),
        )
    }

    fn restore(&mut self, backup: Vec<(String, Option<String>)>) -> Vec<(String, String)> {
        let mut failures = Vec::new();
        for (path, text) in backup {
            let outcome = match text {
                Some(text) => self.store.put(&path, &text),
                None => self.store.remove(&path),
            };
            if let Err(message) = outcome {
                failures.push((path, message));
            }
        }
        failures
    }

    /// 保存 1 回分（§8.3）。メモリの正本へはその場で書けるので、常に同期の結果を返す。
    fn save(
        &mut self,
        writes: Vec<(String, String)>,
        removals: Vec<String>,
    ) -> Option<SaveOutcome> {
        Some(save_now(self, writes, removals))
    }

    fn unsaved_changed(&mut self, _unsaved: bool) {
        // タブを閉じるときの確認（§13.3.7）。`beforeunload` は画面側が登録する。
        if let Some(window) = web_sys::window() {
            let event = web_sys::Event::new("novnow:unsaved").ok();
            if let Some(event) = event {
                let _ = window.dispatch_event(&event);
            }
        }
    }
}

/// パスの最後の要素（`a/b/c.txt` → `c.txt`）。
fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
}
