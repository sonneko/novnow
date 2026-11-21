//! 書き込みの後ろに居るストア（メモリ正本＋**書き込みキュー**）。
//!
//! # なぜ「後ろ」を分けるのか
//!
//! `Host`（`REF:src/app/host.rs`）の `write` は同期で結果を返す。IndexedDB は非同期なので
//! `write` の中で待てない（`REF:docs/developers.md` §8.10 の 30）。そこで:
//!
//! 1. `Host::write` は [`FileStore::put`]（同期）だけを呼ぶ。メモリの正本へ即時に反映し、
//!    書き込みキューへ積む。
//! 2. [`FileStore::drain`]（非同期）がキューを IndexedDB へ流す。待つのは
//!    [`crate::NovnowSession::flush`] と起動直後だけ。
//! 3. 流し込みに失敗したら**キューに残す**（メモリの正本は正しいので、次の `flush` で
//!    もう一度試せる）。失敗そのものは保存状態機の `error` として画面へ返す（§8.3）。
//!
//! # 保存領域の分離（`REF:docs/specifications/webui.md` §21.3.2）
//!
//! 第 2 の DB 名は [`DB_NAME`]（`novnow-webui`）。第 1 の `novnow`（§13.3.2）は**読まない**。
//! 移行は利用者が明示した Zip の入出力だけで行う。
//!
//! # 2 つの実装
//!
//! | 実装 | 使う場面 |
//! |---|---|
//! | [`IdbFiles`] | ブラウザ（IndexedDB）。`webui` の本番 |
//! | [`MemoryFiles`] | テストと、IndexedDB の無い環境（`open_in_memory`） |

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use novnow::core::policy::ProjectPath;
use novnow::ports::storage::{Storage, StorageError};
use wasm_bindgen::{JsCast, JsValue};

/// 第 2 のブラウザ版の DB 名（`REF:docs/specifications/webui.md` §21.3.2。第 1 の `novnow` と別）。
pub const DB_NAME: &str = "novnow-webui";
/// 検証版の DB 名（`REF:docs/designs/webui.md` §21.3。公開版と同じ origin に置いても混ざらない）。
pub const NIGHTLY_DB_NAME: &str = "novnow-webui-nightly";
/// DB の版（スキーマは §13.3.2 と同じ。ストア 4 つと索引 3 つ）。
pub const DB_VERSION: u32 = 2;
/// プロジェクトのストア（keyPath `id`）。
pub const PROJECTS: &str = "projects";
/// ファイルのストア（keyPath `[project_id, path]`）。
pub const FILES: &str = "files";
/// アプリのストア（keyPath `key`）。許可キーは `recent_projects` だけ（§13.3.4）。
pub const APP: &str = "app";
/// `projects` の索引（`updated_at`）。
pub const BY_UPDATED: &str = "by_updated";
/// `files` の索引（`project_id`）。
pub const BY_PROJECT: &str = "by_project";
/// `app` ストアの唯一の許可キー（§13.3.4）。
pub const RECENT_PROJECTS_KEY: &str = "recent_projects";
/// サンプルプロジェクトの ID（§13.3.5。第 1 と同じ名前にして、Zip で移したときに分かりやすくする）。
pub const SAMPLE_ID: &str = "sample";

/// 1 回に流し込む書き込みの上限（QuotaExceeded のときに切り分けられるようにする）。
const DRAIN_CHUNK: usize = 64;

/// 配信の入口（`window.location.pathname`）から DB 名を決める（§21.3.1・§21.3.2）。
///
/// 公開版と検証版は同じ origin の別の入口に置かれるので、`pathname` に `/nightly` を含む
/// ときだけ [`NIGHTLY_DB_NAME`] を使う（第 1 の `REF:src/adapters/storage_idb.rs` の
/// `/novnow/nightly-app/` と同じ考え方）。**第 1 の `novnow` は決して選ばない。**
pub fn db_name(pathname: &str) -> &'static str {
    // `…/nightly/` と `…/webui-nightly/` の両方を受ける（配信の置き方は Pages 側で決まる）。
    let nightly = pathname
        .split('/')
        .any(|segment| segment == "nightly" || segment.ends_with("-nightly"));
    if nightly {
        NIGHTLY_DB_NAME
    } else {
        DB_NAME
    }
}

/// 今の入口の `pathname`。ブラウザの外（`webui` の Node テスト）では空文字にする。
///
/// `window` ではなく `globalThis.location` を読む（`worker` など `window` の無い環境でも
/// 同じ関数で済ませる。`REF:docs/developers.md` §8.10 の 30）。
fn location_pathname() -> String {
    let Ok(found) = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("location")) else {
        return String::new();
    };
    if found.is_undefined() || found.is_null() {
        return String::new();
    }
    let location: web_sys::Location = found.unchecked_into();
    location.pathname().unwrap_or_default()
}

/// 書き込みの後ろ（メモリ正本と、その先の保存領域）。
pub trait FileStore {
    /// メモリの正本へ書き、非同期の書き込みキューへ積む（**同期**）。
    fn put(&self, path: &str, text: &str) -> Result<(), String>;
    /// メモリの正本から消し、削除のキューへ積む（**同期**）。
    fn remove(&self, path: &str) -> Result<(), String>;
    /// メモリの正本の全文（パスの昇順）。
    fn files(&self) -> Vec<(String, String)>;
    /// まだ流していない書き込みの件数。
    fn pending(&self) -> usize;
    /// まだ流していない書き込みのパス（重複なし・昇順）。
    fn pending_paths(&self) -> Vec<String>;
    /// キューを保存領域へ流す（**非同期**）。失敗したらキューに残す。
    fn drain(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>>;
}

/// 読み取り専用の見方（`project_load::load` に渡す `Storage`）。
///
/// メモリの正本はその場で答えるので、`Future` は 1 回の `poll` で完了する。
pub struct MemoryStorage {
    files: Rc<RefCell<BTreeMap<String, String>>>,
}

impl MemoryStorage {
    fn entries(&self) -> BTreeMap<String, String> {
        self.files.borrow().clone()
    }
}

impl Storage for MemoryStorage {
    async fn list(&self) -> Result<Vec<novnow::core::policy::FileEntry>, StorageError> {
        let files = self.entries();
        let mut entries: Vec<novnow::core::policy::FileEntry> = Vec::new();
        for (path, text) in &files {
            let Ok(project_path) = ProjectPath::new(path) else {
                continue;
            };
            entries.push(novnow::core::policy::FileEntry {
                path: project_path,
                stamp: novnow::core::policy::Stamp {
                    modified_ms: None,
                    len: text.len() as u64,
                },
            });
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(entries)
    }

    async fn dirs(&self) -> Result<Option<Vec<ProjectPath>>, StorageError> {
        // ディレクトリを持たない（§4.3 第 2.9 版: 必須ディレクトリは常にあるとみなす）
        Ok(None)
    }

    async fn read(&self, path: &ProjectPath) -> Result<String, StorageError> {
        self.files
            .borrow()
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| StorageError::NotFound(path.clone()))
    }

    async fn write(&self, _path: &ProjectPath, _text: &str) -> Result<Stamp, StorageError> {
        Err(StorageError::Unsupported)
    }

    async fn append(&self, _path: &ProjectPath, _text: &str) -> Result<Stamp, StorageError> {
        Err(StorageError::Unsupported)
    }

    async fn remove(&self, _path: &ProjectPath) -> Result<(), StorageError> {
        Err(StorageError::Unsupported)
    }
}

/// スタンプの別名（`Storage` の戻り値に使う）。
use novnow::core::policy::Stamp;

// ============================================================ メモリ

/// メモリだけのストア（テスト・IndexedDB の無い環境）。
///
/// 中身は `Rc<RefCell<..>>` で共有するので、`Host` に渡した後もテストから読める
/// （`REF:src/app/memory_host.rs` の `MemoryHost` と同じ考え方）。
#[derive(Debug, Clone, Default)]
pub struct MemoryFiles {
    inner: Rc<RefCell<MemoryInner>>,
}

#[derive(Debug, Default)]
struct MemoryInner {
    /// 正本（パス → 中身）。
    files: BTreeMap<String, String>,
    /// 流し終えていない書き込み（パス → 中身の最後の値）。
    queue: BTreeMap<String, String>,
    /// 流し終えていない削除。
    deletions: Vec<String>,
    /// 流した回数（テストの確認用）。
    drains: usize,
    /// 次に流すときに失敗させる（失敗の経路の確認用）。
    fail_next: bool,
    /// 流し込みに失敗した回数。
    failures: usize,
    /// 流し終えた書き込みの並び（テストの確認用）。
    written: VecDeque<String>,
}

impl MemoryFiles {
    /// 空のメモリ。
    pub fn new() -> Self {
        Self::default()
    }

    /// 中身を丸ごと置き換える（起動時の読み込み）。
    pub fn replace(&self, files: Vec<(String, String)>) {
        let mut inner = self.inner.borrow_mut();
        inner.files = files.into_iter().collect();
        inner.queue.clear();
        inner.deletions.clear();
    }

    /// 次の流し込みを 1 回失敗させる（失敗の経路の確認用）。
    pub fn fail_next_drain(&self) {
        self.inner.borrow_mut().fail_next = true;
    }

    /// 流し終えた書き込みのパス（順に）。
    pub fn drained(&self) -> Vec<String> {
        self.inner.borrow().written.iter().cloned().collect()
    }

    /// 流した回数。
    pub fn drains_count(&self) -> usize {
        self.inner.borrow().drains
    }

    /// 流し込みに失敗した回数。
    pub fn failures(&self) -> usize {
        self.inner.borrow().failures
    }

    /// 1 ファイルの中身（テスト用）。
    pub fn text(&self, path: &str) -> Option<String> {
        self.inner.borrow().files.get(path).cloned()
    }

    /// 読み取り専用の見方（`project_load::load` に渡す）。
    pub fn storage(&self) -> MemoryStorage {
        MemoryStorage {
            files: Rc::new(RefCell::new(self.inner.borrow().files.clone())),
        }
    }

    /// キューを取り出す（IndexedDB へ流すため）。
    fn take_queue(&self) -> (Vec<(String, String)>, Vec<String>) {
        let mut inner = self.inner.borrow_mut();
        (
            std::mem::take(&mut inner.queue).into_iter().collect(),
            std::mem::take(&mut inner.deletions),
        )
    }

    /// 流せなかった分をキューへ戻す（次の `flush` でもう一度試す）。
    fn restore(&self, writes: Vec<(String, String)>, removals: Vec<String>) {
        let mut inner = self.inner.borrow_mut();
        inner.failures += 1;
        for (path, text) in writes {
            inner.queue.entry(path).or_insert(text);
        }
        for path in removals {
            if !inner.deletions.iter().any(|found| found == &path) {
                inner.deletions.push(path);
            }
        }
    }

    /// 流し終えた書き込みを記録する（`failures` を戻す）。
    fn record_drained(&self, paths: &[String]) {
        let mut inner = self.inner.borrow_mut();
        for path in paths {
            inner.written.push_back(path.clone());
        }
    }
}

impl FileStore for MemoryFiles {
    fn put(&self, path: &str, text: &str) -> Result<(), String> {
        let mut inner = self.inner.borrow_mut();
        inner.files.insert(path.to_owned(), text.to_owned());
        inner.deletions.retain(|found| found != path);
        inner.queue.insert(path.to_owned(), text.to_owned());
        Ok(())
    }

    fn remove(&self, path: &str) -> Result<(), String> {
        let mut inner = self.inner.borrow_mut();
        inner.files.remove(path);
        inner.queue.remove(path);
        if !inner.deletions.iter().any(|found| found == path) {
            inner.deletions.push(path.to_owned());
        }
        Ok(())
    }

    fn files(&self) -> Vec<(String, String)> {
        self.inner
            .borrow()
            .files
            .iter()
            .map(|(path, text)| (path.clone(), text.clone()))
            .collect()
    }

    fn pending(&self) -> usize {
        let inner = self.inner.borrow();
        inner.queue.len() + inner.deletions.len()
    }

    fn pending_paths(&self) -> Vec<String> {
        let inner = self.inner.borrow();
        let mut paths: Vec<String> = inner.queue.keys().cloned().collect();
        paths.extend(inner.deletions.iter().cloned());
        paths.sort();
        paths.dedup();
        paths
    }

    fn drain(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>> {
        Box::pin(async move {
            let (writes, removals, fail) = {
                let mut inner = self.inner.borrow_mut();
                inner.drains += 1;
                let fail = std::mem::take(&mut inner.fail_next);
                (
                    std::mem::take(&mut inner.queue),
                    std::mem::take(&mut inner.deletions),
                    fail,
                )
            };
            if fail {
                let mut inner = self.inner.borrow_mut();
                inner.failures += 1;
                inner.queue.extend(writes);
                inner.deletions.extend(removals);
                return Err("memory drain failed".to_owned());
            }
            self.record_drained(&writes.keys().cloned().collect::<Vec<_>>());
            Ok(())
        })
    }
}

// ============================================================ IndexedDB

/// IndexedDB `novnow-webui` のストア（ブラウザ）。
///
/// メモリの正本を持ち、`put` / `remove` は正本とキューだけを触る（同期）。IndexedDB へは
/// [`FileStore::drain`] がまとめて流す。起動時の読み込み（[`IdbFiles::load`] /
/// [`IdbFiles::load_or_seed_sample`]）と、流し込みだけが非同期になる。
#[derive(Debug, Clone)]
pub struct IdbFiles {
    db: idb::Db,
    project_id: Rc<RefCell<String>>,
    memory: MemoryFiles,
}

impl IdbFiles {
    /// DB を開く（無ければストアを作る）。
    ///
    /// 名前は配信の入口で決まる（[`db_name`]。公開版と検証版が同じ origin に同居しても混ざらない）。
    pub async fn open() -> Result<Self, String> {
        let db = idb::Db::open(db_name(&location_pathname()), DB_VERSION).await?;
        Ok(Self {
            db,
            project_id: Rc::new(RefCell::new(String::new())),
            memory: MemoryFiles::new(),
        })
    }

    /// プロジェクトの ID（開くまでは空）。
    pub fn project_id(&self) -> String {
        self.project_id.borrow().clone()
    }

    /// メモリの正本（テスト用）。
    pub fn memory(&self) -> &MemoryFiles {
        &self.memory
    }

    /// `id` のプロジェクトを読む（1 つも無ければエラー）。
    ///
    /// 1. `files` を全部読み、メモリの正本へ入れる（読めないレコードは飛ばす）。
    /// 2. 正本を `REF:src/adapters/project_load.rs` の `load` に渡す（**第 1 と同じ手順**で
    ///    §4.3 の判定と文書の正規化を行う。写しを作らない）。
    /// 3. `project.toml` と `.novnow/ui.json` から題と設定を読む。
    pub async fn load(&self, id: &str) -> Result<crate::project::Loaded, String> {
        let record = self
            .db
            .project(id)
            .await?
            .ok_or_else(|| format!("project {id} was not found"))?;
        let storage = self.db.storage(id)?;
        let mut files: Vec<(String, String)> = Vec::new();
        for entry in storage.list().await.map_err(|error| describe(&error))? {
            match storage.read(&entry.path).await {
                Ok(text) => files.push((entry.path.as_str().to_owned(), text)),
                // 読めないレコード（バイナリとして入った画像など）は正本に入れない。
                // 文字コードの不正は起動を止めない（§4.3-3 と同じ扱い）。
                Err(StorageError::Encoding(_)) | Err(StorageError::Unsupported) => continue,
                Err(error) => return Err(describe(&error)),
            }
        }
        self.memory.replace(files);
        *self.project_id.borrow_mut() = record.id.clone();
        crate::project::load(self, &record.name, &record.id)
    }

    /// `projects` が空ならサンプルを 1 つ入れてから、1 つ目のプロジェクトを開く（§13.3.5）。
    pub async fn load_or_seed_sample(&self) -> Result<crate::project::Loaded, String> {
        let projects = self.db.projects().await?;
        let id = match projects.into_iter().next() {
            Some(record) => record.id,
            None => {
                let files: Vec<(String, String)> = novnow::core::template::sample_project()
                    .into_iter()
                    .map(|file| (file.path, file.text))
                    .collect();
                self.db.create_project(SAMPLE_ID, SAMPLE_ID, &files).await?;
                SAMPLE_ID.to_owned()
            }
        };
        self.load(&id).await
    }

    /// `recent_projects`（§13.3.4。`app` ストアに書く唯一の口）。
    pub async fn recent(&self) -> Result<serde_json::Value, String> {
        self.db.recent().await
    }

    /// 開いたことを履歴の先頭へ積む（§13.3.4）。
    pub async fn push_recent(&self, opened_at: &str) -> Result<(), String> {
        let id = self.project_id();
        if id.is_empty() {
            return Ok(());
        }
        self.db.push_recent(&id, opened_at).await
    }
}

/// メモリの正本を `project_load::load` に渡すための `Storage`（読み取りだけ）。
impl Storage for IdbFiles {
    async fn list(&self) -> Result<Vec<novnow::core::policy::FileEntry>, StorageError> {
        self.memory.storage().list().await
    }

    async fn dirs(&self) -> Result<Option<Vec<ProjectPath>>, StorageError> {
        Ok(None)
    }

    async fn read(&self, path: &ProjectPath) -> Result<String, StorageError> {
        self.memory.storage().read(path).await
    }

    async fn write(&self, _path: &ProjectPath, _text: &str) -> Result<Stamp, StorageError> {
        Err(StorageError::Unsupported)
    }

    async fn append(&self, _path: &ProjectPath, _text: &str) -> Result<Stamp, StorageError> {
        Err(StorageError::Unsupported)
    }

    async fn remove(&self, _path: &ProjectPath) -> Result<(), StorageError> {
        Err(StorageError::Unsupported)
    }
}

impl FileStore for IdbFiles {
    fn put(&self, path: &str, text: &str) -> Result<(), String> {
        self.memory.put(path, text)
    }

    fn remove(&self, path: &str) -> Result<(), String> {
        self.memory.remove(path)
    }

    fn files(&self) -> Vec<(String, String)> {
        self.memory.files()
    }

    fn pending(&self) -> usize {
        self.memory.pending()
    }

    fn pending_paths(&self) -> Vec<String> {
        self.memory.pending_paths()
    }

    /// キューを 1 つの IndexedDB トランザクションにまとめて流す（§8.3・§13.3.3）。
    ///
    /// 失敗したら**何も書けていない**（トランザクションを中断する）ので、流せなかった分は
    /// キューへ戻して次の `flush` でもう一度試す（メモリの正本は正しいまま）。
    fn drain(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>> {
        let memory = self.memory.clone();
        let db = self.db.clone();
        let project_id = self.project_id();
        Box::pin(async move {
            if project_id.is_empty() {
                return Err("no project is open".to_owned());
            }
            let (writes, removals) = memory.take_queue();
            if writes.is_empty() && removals.is_empty() {
                return Ok(());
            }
            let mut done = 0usize;
            for chunk in writes.chunks(DRAIN_CHUNK) {
                match db.write_batch(&project_id, chunk, &[]).await {
                    Ok(()) => done += chunk.len(),
                    Err(error) => {
                        memory.restore(writes[done..].to_vec(), removals);
                        return Err(error);
                    }
                }
            }
            for (index, path) in removals.iter().enumerate() {
                if let Err(error) = db.remove_file(&project_id, path).await {
                    memory.restore(Vec::new(), removals[index..].to_vec());
                    return Err(error);
                }
            }
            memory.record_drained(
                &writes
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect::<Vec<_>>(),
            );
            Ok(())
        })
    }
}

/// 失敗の理由を表示用の文字列にする（保存状態機の `error` に載せる文言）。
pub fn describe(error: &StorageError) -> String {
    match error {
        StorageError::QuotaExceeded => "quota exceeded".to_owned(),
        StorageError::Unavailable(message) => format!("storage is unavailable: {message}"),
        StorageError::Policy(violation) => format!("policy violation: {violation:?}"),
        StorageError::InvalidPath(problem) => format!("invalid path: {problem:?}"),
        StorageError::NotFound(path) => format!("not found: {}", path.as_str()),
        StorageError::Encoding(path) => format!("not UTF-8: {}", path.as_str()),
        StorageError::Io { path, message, .. } => format!("{path}: {message}"),
        StorageError::Unsupported => "unsupported".to_owned(),
    }
}

/// パスを `ProjectPath` にする（規則 10 の等価物。絶対パス・`..` はここで弾かれる）。
fn project_path(path: &str) -> Result<ProjectPath, String> {
    ProjectPath::new(path).map_err(|error| format!("invalid path {path}: {error:?}"))
}

/// 今の時刻を UTC の RFC 3339（ミリ秒まで）にする。
fn now_rfc3339() -> String {
    novnow::adapters::clock_system::rfc3339_millis(js_sys::Date::now() as i64)
}

/// IndexedDB の生の口（スキーマは §13.3.2 と同じ。第 2 は DB 名だけが違う）。
///
/// `REF:src/adapters/storage_idb.rs` の `Idb` をそのまま使う手もあるが、DB 名が定数
/// （§13.3.2 の `novnow`）なので第 2 の領域を作れない。スキーマを共有するため、ここでは
/// 同じ形（ストア 4 つ・索引 3 つ・キー `[project_id, path]`）を作り直している。
/// **第 1 の保存領域は読まない**（§21.3.2）。
mod idb {
    use js_sys::{Array, Function, Promise};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        Event, IdbDatabase, IdbFactory, IdbObjectStoreParameters, IdbOpenDbRequest, IdbRequest,
        IdbTransaction, IdbTransactionMode,
    };

    use super::{project_path, APP, BY_PROJECT, BY_UPDATED, FILES, PROJECTS};

    type Handler = Closure<dyn FnMut(Event)>;

    /// 保存領域が無い環境の説明（`npm run dev` を HTTPS 以外で開いた場合など）。
    const MISSING: &str =
        "indexedDB is not available in this context (open the page over http://localhost or https)";

    /// 開いた DB。
    #[derive(Debug, Clone)]
    pub struct Db {
        db: IdbDatabase,
    }

    /// `projects` のレコード（§13.3.2 と同じ形）。
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct ProjectRecord {
        /// UUIDv4 か `sample`。
        pub id: String,
        /// 表示名。
        pub name: String,
        /// 作成日時（UTC の RFC 3339）。
        pub created_at: String,
        /// 更新日時（同）。
        pub updated_at: String,
        /// 形式の版（§13.3.2 の `schema_version`）。
        pub schema_version: u32,
        /// 出自（`sample` / `new` / `import`）。
        pub origin: String,
    }

    /// `files` のレコード（§13.3.2 と同じ形）。
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct FileRecord {
        /// プロジェクトの ID。
        pub project_id: String,
        /// ルートからの相対パス。
        pub path: String,
        /// 中身。
        pub text: String,
        /// バイナリ（第 2 では使わない。第 1 が書いたレコードを読めるように残す）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub bytes: Option<Vec<u8>>,
        /// 更新日時（UTC の RFC 3339）。
        pub updated_at: String,
    }

    impl Db {
        /// DB を開く（無ければストアを作る）。
        ///
        /// `window` ではなく `globalThis` から `indexedDB` を取る（Web Worker でも同じ
        /// コードで動くようにするため。`REF:docs/designs/webui.md` §21.3）。
        pub async fn open(name: &str, version: u32) -> Result<Self, String> {
            let found = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("indexedDB"))
                .map_err(|_| MISSING.to_owned())?;
            if found.is_undefined() || found.is_null() {
                return Err(MISSING.to_owned());
            }
            let factory: IdbFactory = found.unchecked_into();
            let request: IdbOpenDbRequest = factory
                .open_with_u32(name, version)
                .map_err(|_| "indexedDB.open failed".to_owned())?;
            let upgrading = request.clone();
            let on_upgrade: Handler = Closure::new(move |_: Event| {
                if let Ok(result) = upgrading.result() {
                    let _ = create_stores(&result.unchecked_into());
                }
            });
            request.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
            let opened = wait(&request).await;
            request.set_onupgradeneeded(None);
            drop(on_upgrade);
            let db: IdbDatabase = opened?.unchecked_into();
            Ok(Self { db })
        }

        fn transaction(
            &self,
            stores: &[&str],
            mode: IdbTransactionMode,
        ) -> Result<IdbTransaction, String> {
            let names = Array::new();
            for store in stores {
                names.push(&JsValue::from_str(store));
            }
            self.db
                .transaction_with_str_sequence_and_mode(&names, mode)
                .map_err(|_| "transaction failed".to_owned())
        }

        /// ストア `id` のレコード（1 つ分のストレージ）。
        pub fn storage(&self, project_id: &str) -> Result<Store, String> {
            Ok(Store {
                db: self.db.clone(),
                project_id: project_id.to_owned(),
            })
        }

        /// 全プロジェクト（更新の新しい順）。
        pub async fn projects(&self) -> Result<Vec<ProjectRecord>, String> {
            let transaction = self.transaction(&[PROJECTS], IdbTransactionMode::Readonly)?;
            let store = transaction
                .object_store(PROJECTS)
                .map_err(|_| "no projects store".to_owned())?;
            let request = store.get_all().map_err(|_| "get_all failed".to_owned())?;
            let value = wait(&request).await?;
            let mut records: Vec<ProjectRecord> = from_js(&value)?;
            records.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
            Ok(records)
        }

        /// `id` のプロジェクト。
        pub async fn project(&self, id: &str) -> Result<Option<ProjectRecord>, String> {
            let transaction = self.transaction(&[PROJECTS], IdbTransactionMode::Readonly)?;
            let store = transaction
                .object_store(PROJECTS)
                .map_err(|_| "no projects store".to_owned())?;
            let request = store
                .get(&JsValue::from_str(id))
                .map_err(|_| "get failed".to_owned())?;
            let value = wait(&request).await?;
            if value.is_undefined() {
                return Ok(None);
            }
            Ok(Some(from_js(&value)?))
        }

        /// プロジェクトを 1 つ作る（`projects` と `files` を 1 トランザクション、§13.3.3）。
        pub async fn create_project(
            &self,
            id: &str,
            name: &str,
            files: &[(String, String)],
        ) -> Result<(), String> {
            let now = super::now_rfc3339();
            let record = ProjectRecord {
                id: id.to_owned(),
                name: name.to_owned(),
                created_at: now.clone(),
                updated_at: now.clone(),
                schema_version: 1,
                origin: if id == super::SAMPLE_ID {
                    "sample"
                } else {
                    "new"
                }
                .to_owned(),
            };
            let transaction =
                self.transaction(&[PROJECTS, FILES], IdbTransactionMode::Readwrite)?;
            let projects = transaction
                .object_store(PROJECTS)
                .map_err(|_| "no projects store".to_owned())?;
            projects
                .put(&to_js(&record)?)
                .map_err(|_| "put failed".to_owned())?;
            let store = transaction
                .object_store(FILES)
                .map_err(|_| "no files store".to_owned())?;
            for (path, text) in files {
                project_path(path)?;
                store
                    .put(&to_js(&FileRecord {
                        project_id: id.to_owned(),
                        path: path.clone(),
                        text: text.clone(),
                        bytes: None,
                        updated_at: now.clone(),
                    })?)
                    .map_err(|_| "put failed".to_owned())?;
            }
            commit(&transaction).await
        }

        /// 書き込みを 1 トランザクションでまとめる（§8.3・§13.3.3）。
        ///
        /// 失敗したらトランザクションを中断する（**部分的に書けた状態を残さない**）。
        pub async fn write_batch(
            &self,
            project_id: &str,
            writes: &[(String, String)],
            removals: &[String],
        ) -> Result<(), String> {
            let now = super::now_rfc3339();
            let transaction =
                self.transaction(&[FILES, PROJECTS], IdbTransactionMode::Readwrite)?;
            let prepared: Result<Vec<(String, JsValue)>, String> = writes
                .iter()
                .map(|(path, text)| {
                    project_path(path)?;
                    Ok((
                        path.clone(),
                        to_js(&FileRecord {
                            project_id: project_id.to_owned(),
                            path: path.clone(),
                            text: text.clone(),
                            bytes: None,
                            updated_at: now.clone(),
                        })?,
                    ))
                })
                .collect();
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(message) => {
                    let _ = transaction.abort();
                    return Err(message);
                }
            };
            let store = match transaction.object_store(FILES) {
                Ok(store) => store,
                Err(_) => {
                    let _ = transaction.abort();
                    return Err("no files store".to_owned());
                }
            };
            for (_, value) in &prepared {
                if store.put(value).is_err() {
                    let _ = transaction.abort();
                    return Err("put failed".to_owned());
                }
            }
            for path in removals {
                if store.delete(&file_key(project_id, path)).is_err() {
                    let _ = transaction.abort();
                    return Err("delete failed".to_owned());
                }
            }
            let projects = transaction
                .object_store(PROJECTS)
                .map_err(|_| "no projects store".to_owned())?;
            let request = projects
                .get(&JsValue::from_str(project_id))
                .map_err(|_| "get failed".to_owned())?;
            let found = wait(&request).await?;
            if !found.is_undefined() {
                let mut record: ProjectRecord = from_js(&found)?;
                record.updated_at = now;
                projects
                    .put(&to_js(&record)?)
                    .map_err(|_| "put failed".to_owned())?;
            }
            commit(&transaction).await
        }

        /// 1 ファイルを消す。
        pub async fn remove_file(&self, project_id: &str, path: &str) -> Result<(), String> {
            let transaction = self.transaction(&[FILES], IdbTransactionMode::Readwrite)?;
            let store = transaction
                .object_store(FILES)
                .map_err(|_| "no files store".to_owned())?;
            store
                .delete(&file_key(project_id, path))
                .map_err(|_| "delete failed".to_owned())?;
            commit(&transaction).await
        }

        /// `recent_projects`（§13.3.4。`app` ストアに書く唯一の口）。
        pub async fn recent(&self) -> Result<serde_json::Value, String> {
            let transaction = self.transaction(&[APP], IdbTransactionMode::Readonly)?;
            let store = transaction
                .object_store(APP)
                .map_err(|_| "no app store".to_owned())?;
            let request = store
                .get(&JsValue::from_str(super::RECENT_PROJECTS_KEY))
                .map_err(|_| "get failed".to_owned())?;
            let found = wait(&request).await?;
            if found.is_undefined() {
                return Ok(serde_json::json!({ "items": [] }));
            }
            let text: String = js_sys::JSON::stringify(&found)
                .map_err(|_| "stringify failed".to_owned())?
                .into();
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|error| error.to_string())?;
            Ok(value
                .get("value")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({ "items": [] })))
        }

        /// 開いたプロジェクトを履歴の先頭へ積む（§13.3.4）。
        pub async fn push_recent(&self, project_id: &str, opened_at: &str) -> Result<(), String> {
            let mut recent = self.recent().await?;
            let items = recent
                .get_mut("items")
                .and_then(|value| value.as_array_mut());
            match items {
                Some(items) => {
                    items.retain(|item| {
                        item.get("id").and_then(|id| id.as_str()) != Some(project_id)
                    });
                    items.insert(
                        0,
                        serde_json::json!({ "id": project_id, "opened_at": opened_at }),
                    );
                    items.truncate(20);
                }
                None => {
                    recent = serde_json::json!({
                        "items": [{ "id": project_id, "opened_at": opened_at }]
                    });
                }
            }
            let transaction = self.transaction(&[APP], IdbTransactionMode::Readwrite)?;
            let store = transaction
                .object_store(APP)
                .map_err(|_| "no app store".to_owned())?;
            let record = serde_json::json!({
                "key": super::RECENT_PROJECTS_KEY,
                "value": recent,
            });
            store
                .put(&to_js(&record)?)
                .map_err(|_| "put failed".to_owned())?;
            commit(&transaction).await
        }
    }

    /// プロジェクト 1 つ分（読み取りだけ。書きは [`Db::write_batch`]）。
    #[derive(Debug, Clone)]
    pub struct Store {
        db: IdbDatabase,
        project_id: String,
    }

    impl Store {
        fn transaction(
            &self,
            stores: &[&str],
            mode: IdbTransactionMode,
        ) -> Result<IdbTransaction, String> {
            Db {
                db: self.db.clone(),
            }
            .transaction(stores, mode)
        }
    }

    impl novnow::ports::storage::Storage for Store {
        async fn list(
            &self,
        ) -> Result<Vec<novnow::ports::storage::FileEntry>, novnow::ports::storage::StorageError>
        {
            use novnow::ports::storage::StorageError;
            let transaction = self
                .transaction(&[FILES], IdbTransactionMode::Readonly)
                .map_err(|_| StorageError::Unavailable("transaction".to_owned()))?;
            let store = transaction
                .object_store(FILES)
                .map_err(|_| StorageError::Unavailable("files store".to_owned()))?;
            let index = store
                .index(BY_PROJECT)
                .map_err(|_| StorageError::Unavailable("index".to_owned()))?;
            let request = index
                .get_all_with_key(&JsValue::from_str(&self.project_id))
                .map_err(|_| StorageError::Unavailable("get_all".to_owned()))?;
            let value = wait(&request).await.map_err(StorageError::Unavailable)?;
            let records: Vec<FileRecord> = from_js(&value).map_err(StorageError::Unavailable)?;
            let mut entries: Vec<novnow::ports::storage::FileEntry> = records
                .into_iter()
                .filter_map(|record| {
                    Some(novnow::ports::storage::FileEntry {
                        path: project_path(&record.path).ok()?,
                        stamp: novnow::ports::storage::Stamp {
                            modified_ms: None,
                            len: record.text.len() as u64,
                        },
                    })
                })
                .collect();
            entries.sort_by(|left, right| left.path.cmp(&right.path));
            Ok(entries)
        }

        async fn dirs(
            &self,
        ) -> Result<
            Option<Vec<novnow::core::policy::ProjectPath>>,
            novnow::ports::storage::StorageError,
        > {
            // IndexedDB にディレクトリは無い（§4.3 第 2.9 版）
            Ok(None)
        }

        async fn read(
            &self,
            path: &novnow::core::policy::ProjectPath,
        ) -> Result<String, novnow::ports::storage::StorageError> {
            use novnow::ports::storage::StorageError;
            let transaction = self
                .transaction(&[FILES], IdbTransactionMode::Readonly)
                .map_err(|_| StorageError::Unavailable("transaction".to_owned()))?;
            let store = transaction
                .object_store(FILES)
                .map_err(|_| StorageError::Unavailable("files store".to_owned()))?;
            let request = store
                .get(&file_key(&self.project_id, path.as_str()))
                .map_err(|_| StorageError::Unavailable("get".to_owned()))?;
            let found = wait(&request).await.map_err(StorageError::Unavailable)?;
            if found.is_undefined() {
                return Err(StorageError::NotFound(path.clone()));
            }
            let record: FileRecord = from_js(&found).map_err(StorageError::Unavailable)?;
            match record.bytes {
                Some(bytes) => {
                    String::from_utf8(bytes).map_err(|_| StorageError::Encoding(path.clone()))
                }
                None => Ok(record.text),
            }
        }

        async fn write(
            &self,
            _path: &novnow::core::policy::ProjectPath,
            _text: &str,
        ) -> Result<novnow::ports::storage::Stamp, novnow::ports::storage::StorageError> {
            // 書き込みは [`Db::write_batch`] にまとめる（1 回分の保存を 1 トランザクション）
            Err(novnow::ports::storage::StorageError::Unsupported)
        }

        async fn append(
            &self,
            _path: &novnow::core::policy::ProjectPath,
            _text: &str,
        ) -> Result<novnow::ports::storage::Stamp, novnow::ports::storage::StorageError> {
            Err(novnow::ports::storage::StorageError::Unsupported)
        }

        async fn remove(
            &self,
            _path: &novnow::core::policy::ProjectPath,
        ) -> Result<(), novnow::ports::storage::StorageError> {
            Err(novnow::ports::storage::StorageError::Unsupported)
        }
    }

    /// ストア 4 つと索引 3 つを作る（§13.3.2 と同じ形）。
    fn create_stores(db: &IdbDatabase) -> Result<(), JsValue> {
        let names = db.object_store_names();
        if !names.contains(PROJECTS) {
            let params = IdbObjectStoreParameters::new();
            params.set_key_path(&JsValue::from_str("id"));
            let store = db.create_object_store_with_optional_parameters(PROJECTS, &params)?;
            store.create_index_with_str(BY_UPDATED, "updated_at")?;
        }
        if !names.contains(FILES) {
            let params = IdbObjectStoreParameters::new();
            let key_path = Array::of2(&JsValue::from_str("project_id"), &JsValue::from_str("path"));
            params.set_key_path(&key_path);
            let store = db.create_object_store_with_optional_parameters(FILES, &params)?;
            store.create_index_with_str(BY_PROJECT, "project_id")?;
        }
        if !names.contains(APP) {
            let params = IdbObjectStoreParameters::new();
            params.set_key_path(&JsValue::from_str("key"));
            db.create_object_store_with_optional_parameters(APP, &params)?;
        }
        Ok(())
    }

    fn file_key(project_id: &str, path: &str) -> JsValue {
        Array::of2(&JsValue::from_str(project_id), &JsValue::from_str(path)).into()
    }

    fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, String> {
        let text = serde_json::to_string(value).map_err(|error| error.to_string())?;
        js_sys::JSON::parse(&text).map_err(|_| "JSON.parse failed".to_owned())
    }

    fn from_js<T: serde::de::DeserializeOwned>(value: &JsValue) -> Result<T, String> {
        let text: String = js_sys::JSON::stringify(value)
            .map_err(|_| "JSON.stringify failed".to_owned())?
            .into();
        serde_json::from_str(&text).map_err(|error| error.to_string())
    }

    /// 要求が終わるまで待つ（`REF:src/adapters/storage_idb.rs` の `wait` と同じ形）。
    async fn wait(request: &IdbRequest) -> Result<JsValue, String> {
        let mut handlers: Option<(Handler, Handler)> = None;
        let promise = Promise::new(&mut |resolve: Function, reject: Function| {
            let ok = request.clone();
            let on_success: Handler = Closure::new(move |_: Event| {
                let value = ok.result().unwrap_or(JsValue::UNDEFINED);
                let _ = resolve.call1(&JsValue::UNDEFINED, &value);
            });
            let failed = request.clone();
            let on_error: Handler = Closure::new(move |_: Event| {
                let message = failed
                    .error()
                    .ok()
                    .flatten()
                    .map(|error| {
                        let exception: web_sys::DomException = error.unchecked_into();
                        exception.name()
                    })
                    .unwrap_or_else(|| "request failed".to_owned());
                let _ = reject.call1(&JsValue::UNDEFINED, &JsValue::from_str(&message));
            });
            request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
            request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            handlers = Some((on_success, on_error));
        });
        let result = JsFuture::from(promise).await;
        request.set_onsuccess(None);
        request.set_onerror(None);
        drop(handlers);
        result.map_err(|value| {
            value
                .as_string()
                .unwrap_or_else(|| "request failed".to_owned())
        })
    }

    /// トランザクションの完了（`complete` か `abort`）まで待つ。
    async fn commit(transaction: &IdbTransaction) -> Result<(), String> {
        let mut handlers: Option<(Handler, Handler)> = None;
        let promise = Promise::new(&mut |resolve: Function, reject: Function| {
            let on_complete: Handler = Closure::new(move |_: Event| {
                let _ = resolve.call0(&JsValue::UNDEFINED);
            });
            let failed = transaction.clone();
            let reject_abort = reject.clone();
            let on_error: Handler = Closure::new(move |_: Event| {
                let message = failed
                    .error()
                    .map(|error| {
                        let exception: web_sys::DomException = error.unchecked_into();
                        exception.name()
                    })
                    .unwrap_or_else(|| "transaction failed".to_owned());
                let _ = reject_abort.call1(&JsValue::UNDEFINED, &JsValue::from_str(&message));
            });
            transaction.set_oncomplete(Some(on_complete.as_ref().unchecked_ref()));
            transaction.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            handlers = Some((on_complete, on_error));
        });
        let result = JsFuture::from(promise).await;
        transaction.set_oncomplete(None);
        transaction.set_onerror(None);
        drop(handlers);
        result.map(|_| ()).map_err(|value| {
            value
                .as_string()
                .unwrap_or_else(|| "transaction failed".to_owned())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 書いたものがキューに積まれ、流すと空になる（メモリ正本は変わらない）。
    #[test]
    fn put_queues_until_drain() {
        let store = MemoryFiles::new();
        store.put("src/body.md", "一").unwrap();
        assert_eq!(store.pending(), 1);
        assert_eq!(store.pending_paths(), vec!["src/body.md".to_owned()]);
        assert_eq!(store.text("src/body.md").as_deref(), Some("一"));
        let drained = futures_lite_block(store.drain());
        assert!(drained.is_ok());
        assert_eq!(store.pending(), 0);
        assert_eq!(store.drains_count(), 1);
        assert_eq!(store.drained(), vec!["src/body.md".to_owned()]);
    }

    /// 同じパスを 2 回書いたらキューは 1 件（最後の値だけを流す）。
    #[test]
    fn queue_keeps_the_last_text() {
        let store = MemoryFiles::new();
        store.put("src/body.md", "一").unwrap();
        store.put("src/body.md", "二").unwrap();
        assert_eq!(store.pending(), 1);
        assert!(futures_lite_block(store.drain()).is_ok());
        assert_eq!(store.text("src/body.md").as_deref(), Some("二"));
    }

    /// 流し込みに失敗したらキューに残り、次の流し込みで成功する（§8.3 の再試行）。
    #[test]
    fn failed_drain_keeps_the_queue() {
        let store = MemoryFiles::new();
        store.put("src/body.md", "一").unwrap();
        store.fail_next_drain();
        assert!(futures_lite_block(store.drain()).is_err());
        assert_eq!(store.pending(), 1);
        assert_eq!(store.failures(), 1);
        assert_eq!(store.text("src/body.md").as_deref(), Some("一"));
        assert!(futures_lite_block(store.drain()).is_ok());
        assert_eq!(store.pending(), 0);
        assert_eq!(store.drained(), vec!["src/body.md".to_owned()]);
    }

    /// 消したものは削除のキューに積まれ、流すと消える。
    #[test]
    fn remove_queues_a_deletion() {
        let store = MemoryFiles::new();
        store.replace(vec![("src/body.md".to_owned(), "一".to_owned())]);
        store.remove("src/body.md").unwrap();
        assert!(store.text("src/body.md").is_none());
        assert_eq!(store.pending(), 1);
        assert!(futures_lite_block(store.drain()).is_ok());
        assert_eq!(store.pending(), 0);
    }

    /// メモリの正本を `Storage` として読める（`project_load::load` に渡す口）。
    #[test]
    fn memory_is_readable_as_storage() {
        let store = MemoryFiles::new();
        store.replace(vec![
            ("src/body.md".to_owned(), "一".to_owned()),
            ("project.toml".to_owned(), "schema = 1\n".to_owned()),
        ]);
        let storage = store.storage();
        let listed = futures_lite_block(storage.list()).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].path.as_str(), "project.toml");
        let text = futures_lite_block(storage.read(&listed[0].path)).unwrap();
        assert_eq!(text, "schema = 1\n");
    }

    /// 配信の入口で DB 名が決まる（公開版と検証版が混ざらない。§21.3.1）。
    #[test]
    fn db_name_follows_the_entry_point() {
        assert_eq!(db_name("/novnow/webui/"), DB_NAME);
        assert_eq!(db_name("/novnow/webui/nightly/"), NIGHTLY_DB_NAME);
        assert_eq!(db_name("/novnow/webui-nightly/index.html"), NIGHTLY_DB_NAME);
        // どちらの入口でも第 1 の `novnow` は選ばない（§21.3.2）。
        for path in [
            "/",
            "/novnow/app/",
            "/novnow/webui/",
            "/novnow/webui/nightly/",
        ] {
            assert_ne!(db_name(path), "novnow");
        }
    }

    /// 1 回の poll で終わる `Future` を回す（テスト用。`block_on` は wasm に無い）。
    fn futures_lite_block<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        fn noop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(clone(std::ptr::null())) };
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("the memory store must answer synchronously"),
        }
    }
}
