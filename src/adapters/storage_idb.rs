//! Web のストレージ: IndexedDB（§13.3、`PLANS.md` §5.6 作業項目 3・§9.2 B-96）。
//!
//! # 2 つの部分
//!
//! | 部分 | 中身 | どこで動くか・どう確かめるか |
//! |---|---|---|
//! | 純粋な部分（このファイルの上半分） | レコードの形（§13.3.2）・`app` ストアの許可キー（§13.3.4）・残容量の判定（W062）・LocalStorage（v0）の取り込みの計画（§13.3.6）・ID の採番 | どこでも。`tests/storage_idb.rs` |
//! | ブラウザとの接続（`web`、`wasm32` だけ） | `IndexedDB` の DB `novnow` v1・ストア 3 つ・トランザクション境界（§13.3.3）・[`Storage`](crate::ports::storage::Storage) の実装・`navigator.storage`・ダウンロード | ブラウザ。CI の WASM ジョブはコンパイルだけを確かめる（実行の確認は M7 の Web UI で行う） |
//!
//! # 規則 10 の Web 版（§13.3.4）
//!
//! - プロジェクトの中身は `files` ストアの**そのプロジェクトのレコード**にしか書かない
//!   （[`IdbStorage`](self) は 1 つの `project_id` に縛られる。パスは `ProjectPath`）。
//! - `app` ストアのキーは **`recent_projects` だけ**（[`APP_KEYS`]）。書く口は履歴の口
//!   （`Idb::push_recent`）しか無く、任意のキーを書く API を持たない。
//! - IndexedDB が使えないときは **LocalStorage へフォールバックしない**（MUST、§13.3.7）。E062。
//!
//! # LocalStorage（v0）からの取り込み（§13.3.6）
//!
//! 手順 3 の「v0 → v1 移行（§20）」は M11 の仕事なので、変換は**呼び出し側が渡す関数**にした。
//! M11 までは [`convert_v0`] が失敗を返すので、v0 のデータがあれば **E061**（LocalStorage は消さず、
//! IndexedDB にも何も書かない）。変換・書き込みが成功したときだけ LocalStorage の該当キーを消して
//! **I053** を出す。

use serde::{Deserialize, Serialize};

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::policy::archive::{self, ArchiveEntry, Skipped};
use crate::core::policy::recent::RecentProjects;
use crate::core::policy::{ProjectPath, Stamp};
use crate::ports::storage::StorageError;

use super::clock_system::parse_utc;

/// DB 名（§13.3.2）。
pub const DB_NAME: &str = "novnow";
/// DB の版（§13.3.2）。
pub const DB_VERSION: u32 = 1;
/// プロジェクトのストア（keyPath `id`）。
pub const PROJECTS: &str = "projects";
/// ファイルのストア（keyPath `[project_id, path]`）。
pub const FILES: &str = "files";
/// アプリのストア（keyPath `key`）。
pub const APP: &str = "app";
/// `projects` の索引（`updated_at`）。
pub const BY_UPDATED: &str = "by_updated";
/// `files` の索引（`project_id`）。
pub const BY_PROJECT: &str = "by_project";
/// `app` ストアの唯一の許可キー（§13.3.4）。
pub const RECENT_PROJECTS_KEY: &str = "recent_projects";
/// `app` ストアに書いてよいキーの全部（§13.3.4 MUST。テストで固定する）。
pub const APP_KEYS: &[&str] = &[RECENT_PROJECTS_KEY];
/// サンプルプロジェクトの ID（§13.3.5）。
pub const SAMPLE_ID: &str = "sample";
/// 残容量の警告の閾値（§13.3.7「残り 10MB 未満なら W062」）。
pub const LOW_SPACE_BYTES: f64 = 10.0 * 1024.0 * 1024.0;
/// v0 の LocalStorage のファイル一覧のキー（§13.3.6）。
pub const LEGACY_INDEX_KEY: &str = "novnow_file_index";
/// v0 の LocalStorage のファイルのキーの接頭辞（§13.3.6）。
pub const LEGACY_FILE_PREFIX: &str = "novnow_file:";
/// プロジェクトの形式の版（`projects.schema_version`）。
pub const PROJECT_SCHEMA_VERSION: u32 = 1;

// ============================================================ レコード（§13.3.2）

/// プロジェクトの出自（§13.3.2 `origin`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// 初回に投入したサンプル（§13.3.5）。
    Sample,
    /// 新規作成。
    New,
    /// Zip の取り込み（§13.5）。
    Import,
    /// LocalStorage（v0）からの移行（§13.3.6）。
    Migrated,
}

impl Origin {
    /// レコードに書く文字列。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sample => "sample",
            Self::New => "new",
            Self::Import => "import",
            Self::Migrated => "migrated",
        }
    }
}

/// `projects` のレコード。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRecord {
    /// UUIDv4 か `sample`（`migrated-YYYYMMDD` も）。
    pub id: String,
    /// 表示名（選択モーダルの一覧、§13.4）。
    pub name: String,
    /// 作成日時（UTC の RFC 3339。`clock_system::rfc3339_millis`）。
    pub created_at: String,
    /// 更新日時（同。ファイルを保存するたびに更新する、§13.3.3）。
    pub updated_at: String,
    /// 形式の版。
    pub schema_version: u32,
    /// 出自（[`Origin::as_str`]）。
    pub origin: String,
}

impl ProjectRecord {
    /// 新しいレコード（作成日時 = 更新日時 = `now`）。
    pub fn new(id: &str, name: &str, origin: Origin, now: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
            schema_version: PROJECT_SCHEMA_VERSION,
            origin: origin.as_str().to_owned(),
        }
    }
}

/// `files` のレコード（1 ファイル = 1 レコード。テキストのみ、§13.3.2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    /// プロジェクトの ID。
    pub project_id: String,
    /// プロジェクトルートからの相対パス（先頭スラッシュ禁止、§13.3.2）。
    pub path: String,
    /// 中身。
    pub text: String,
    /// 更新日時（UTC の RFC 3339）。
    pub updated_at: String,
}

impl FileRecord {
    /// 新しいレコード。
    pub fn new(project_id: &str, path: &ProjectPath, text: &str, now: &str) -> Self {
        Self {
            project_id: project_id.to_owned(),
            path: path.as_str().to_owned(),
            text: text.to_owned(),
            updated_at: now.to_owned(),
        }
    }

    /// パス（相対パスとして読めないレコードは `None`。手で壊された DB でも止まらない）。
    pub fn project_path(&self) -> Option<ProjectPath> {
        ProjectPath::new(&self.path).ok()
    }

    /// 外部変更の検出（§8.5。別のタブが同じプロジェクトを書いたとき）に使うスタンプ。
    pub fn stamp(&self) -> Stamp {
        Stamp {
            modified_ms: parse_utc(&self.updated_at),
            len: self.text.len() as u64,
        }
    }
}

/// `app` のレコード（§13.3.4）。作る口は [`AppRecord::recent`] だけ（キーは固定）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppRecord {
    /// キー（常に [`RECENT_PROJECTS_KEY`]）。
    pub key: String,
    /// 値（`RecentProjects` の JSON）。
    pub value: serde_json::Value,
}

impl AppRecord {
    /// `recent_projects` のレコード。
    pub fn recent(recent: &RecentProjects) -> Self {
        Self {
            key: RECENT_PROJECTS_KEY.to_owned(),
            value: serde_json::to_value(recent).unwrap_or(serde_json::Value::Null),
        }
    }
}

// ============================================================ 判定・採番

/// 残りが 10MB 未満か（§13.3.7、W062）。
pub fn is_low_space(quota: f64, usage: f64) -> bool {
    quota - usage < LOW_SPACE_BYTES
}

/// W062「保存領域の空きが少なくなっています」（§7.1）。
pub fn low_space_notice() -> Diagnostic {
    Diagnostic::simple(DiagnosticCode::W062, None)
}

/// E062「この環境ではデータを保存できません」（§7.1・§13.3.7。起動を中止して全画面で示す）。
pub fn unavailable_notice() -> Diagnostic {
    Diagnostic::simple(DiagnosticCode::E062, None)
}

/// `migrated-YYYYMMDD`（§13.3.6）。同じ日に 2 回目があれば `-2`, `-3`… を付ける。
pub fn migrated_id(yyyymmdd: &str, taken: &[String]) -> String {
    let base = format!("migrated-{yyyymmdd}");
    if !taken.contains(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or(base)
}

/// 乱数 16 byte から UUIDv4 の文字列を作る（§13.3.2 の `id`。乱数は `crypto.getRandomValues`）。
pub fn uuid_v4(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

// ============================================================ LocalStorage（v0）の取り込み（§13.3.6）

/// 取り込みの失敗（**E061**）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyError {
    /// `novnow_file_index` が JSON の文字列配列として読めない。
    BrokenIndex,
    /// 一覧にあるファイルのキーが無い。
    MissingFile(String),
    /// v0 → v1 の変換に失敗した（M11 までは常にこれ）。
    Convert(String),
    /// IndexedDB への書き込みに失敗した。
    Storage(StorageError),
}

/// LocalStorage から読んだ v0 のデータ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyData {
    /// ファイル（v0 の Web 版のルート `my-novel/` は剥がしてある）。
    pub files: Vec<(ProjectPath, String)>,
    /// 成功したときに消すキー（一覧のキーと全ファイルのキー）。
    pub keys: Vec<String>,
    /// 取り込まないもの（相対パスにできない名前など）。
    pub skipped: Vec<Skipped>,
}

/// 取り込みの結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyOutcome {
    /// v0 のデータが無い。
    Nothing,
    /// 取り込んだ（新しいプロジェクトの ID。**I053**）。
    Migrated(String),
    /// 失敗した（LocalStorage は消していない。**E061**）。
    Failed(LegacyError),
}

impl LegacyOutcome {
    /// 出す通知（I053 / E061）。
    pub fn diagnostic(&self) -> Option<Diagnostic> {
        match self {
            Self::Nothing => None,
            Self::Migrated(id) => Some(Diagnostic::new(
                DiagnosticCode::I053,
                None,
                vec![Arg::text(id.as_str())],
            )),
            Self::Failed(_) => Some(Diagnostic::simple(DiagnosticCode::E061, None)),
        }
    }
}

/// v0 の LocalStorage を読む（§13.3.6 の手順 1・2）。`get` は LocalStorage の `getItem`。
///
/// ファイルの名前は Zip の取り込みと同じ規則（`policy::archive::plan_import`）で相対パスにする
/// （v0 の Web 版は全ファイルを `my-novel/` の下に置いていたので、単一ルートとして剥がれる）。
pub fn read_legacy(
    index_json: &str,
    get: impl Fn(&str) -> Option<String>,
) -> Result<LegacyData, LegacyError> {
    let index: Vec<String> =
        serde_json::from_str(index_json).map_err(|_| LegacyError::BrokenIndex)?;
    let mut keys = vec![LEGACY_INDEX_KEY.to_owned()];
    let mut texts = Vec::with_capacity(index.len());
    for path in &index {
        let key = format!("{LEGACY_FILE_PREFIX}{path}");
        let text = get(&key).ok_or_else(|| LegacyError::MissingFile(path.clone()))?;
        keys.push(key);
        texts.push(text);
    }
    let entries: Vec<ArchiveEntry> = index
        .iter()
        .map(|path| ArchiveEntry::file(path.as_str()))
        .collect();
    let plan = archive::plan_import(&entries);
    let files = plan
        .entries
        .into_iter()
        .map(|planned| (planned.path, texts[planned.index].clone()))
        .collect();
    Ok(LegacyData {
        files,
        keys,
        skipped: plan.skipped,
    })
}

/// v0 → v1 の変換（§13.3.6 の手順 3、§20）。**M11 で実装する**（`PLANS.md` §5.12）。
///
/// それまでは失敗を返す。取り込みは全体が失敗し（E061）、LocalStorage は消えず、IndexedDB にも
/// 何も書かない（変換していない v0 のファイルを v1 のプロジェクトとして置かないため。B-96）。
pub fn convert_v0(
    _files: Vec<(ProjectPath, String)>,
) -> Result<Vec<(ProjectPath, String)>, LegacyError> {
    Err(LegacyError::Convert(
        "v0 -> v1 conversion is implemented in M11 (SPECIFICATIONS.md section 20)".to_owned(),
    ))
}

#[cfg(target_arch = "wasm32")]
pub use self::web::{download, estimate, request_persist, Idb, IdbStorage};

/// ブラウザとの接続（`wasm32` だけ）。
#[cfg(target_arch = "wasm32")]
mod web {
    use std::io;

    use js_sys::{Array, Function, Promise};
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        DomException, Event, IdbDatabase, IdbObjectStoreParameters, IdbOpenDbRequest, IdbRequest,
        IdbTransaction, IdbTransactionMode, StorageEstimate,
    };

    use super::{
        convert_v0, migrated_id, read_legacy, uuid_v4, AppRecord, FileRecord, LegacyError,
        LegacyOutcome, Origin, ProjectRecord, APP, BY_PROJECT, BY_UPDATED, DB_NAME, DB_VERSION,
        FILES, LEGACY_INDEX_KEY, PROJECTS, RECENT_PROJECTS_KEY, SAMPLE_ID,
    };
    use crate::adapters::clock_system::{rfc3339_millis, yyyymmdd, SystemClock};
    use crate::adapters::zip::{self as archive_io, ArchiveError};
    use crate::core::policy::archive::Skipped;
    use crate::core::policy::recent::{ProjectItem, RecentProjects};
    use crate::core::policy::{FileEntry, ProjectPath, Stamp};
    use crate::ports::clock::Clock;
    use crate::ports::storage::{Storage, StorageError};

    type Handler = Closure<dyn FnMut(Event)>;

    // ---------------------------------------------------------------- JS との橋渡し

    /// serde の値を JS の値にする（JSON を経由する。クレートを足さない）。
    fn to_js<T: Serialize>(value: &T) -> Result<JsValue, StorageError> {
        let text = serde_json::to_string(value).map_err(|error| other(error.to_string()))?;
        js_sys::JSON::parse(&text).map_err(js_error)
    }

    /// JS の値を serde の値にする。
    fn from_js<T: DeserializeOwned>(value: &JsValue) -> Result<T, StorageError> {
        let text: String = js_sys::JSON::stringify(value).map_err(js_error)?.into();
        serde_json::from_str(&text).map_err(|error| other(error.to_string()))
    }

    fn other(message: String) -> StorageError {
        StorageError::Io {
            path: String::new(),
            kind: io::ErrorKind::Other,
            message,
        }
    }

    /// JS の例外を [`StorageError`] にする（`QuotaExceededError` は保存失敗、§13.3.7）。
    fn js_error(value: JsValue) -> StorageError {
        match value.dyn_ref::<DomException>() {
            Some(exception) if exception.name() == "QuotaExceededError" => {
                StorageError::QuotaExceeded
            }
            Some(exception) => other(format!("{}: {}", exception.name(), exception.message())),
            None => other(format!("{value:?}")),
        }
    }

    fn unavailable(reason: &str) -> StorageError {
        StorageError::Unavailable(reason.to_owned())
    }

    fn now() -> String {
        rfc3339_millis(SystemClock.now_millis())
    }

    /// 要求の完了を待つ（`onsuccess` で結果、`onerror` で例外）。
    async fn wait(request: &IdbRequest) -> Result<JsValue, StorageError> {
        let mut handlers: Option<(Handler, Handler)> = None;
        let promise = Promise::new(&mut |resolve: Function, reject: Function| {
            let ok = request.clone();
            let on_success: Handler = Closure::new(move |_: Event| {
                let value = ok.result().unwrap_or(JsValue::UNDEFINED);
                let _ = resolve.call1(&JsValue::UNDEFINED, &value);
            });
            let failed = request.clone();
            let on_error: Handler = Closure::new(move |_: Event| {
                let value = failed
                    .error()
                    .ok()
                    .flatten()
                    .map(JsValue::from)
                    .unwrap_or(JsValue::UNDEFINED);
                let _ = reject.call1(&JsValue::UNDEFINED, &value);
            });
            request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
            request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            handlers = Some((on_success, on_error));
        });
        let result = JsFuture::from(promise).await;
        request.set_onsuccess(None);
        request.set_onerror(None);
        drop(handlers);
        result.map_err(js_error)
    }

    /// トランザクションの完了を待つ（§8.3「トランザクション成功 = 保存成功」）。
    async fn commit(transaction: &IdbTransaction) -> Result<(), StorageError> {
        let mut handlers: Option<(Handler, Handler, Handler)> = None;
        let promise = Promise::new(&mut |resolve: Function, reject: Function| {
            let on_complete: Handler = Closure::new(move |_: Event| {
                let _ = resolve.call0(&JsValue::UNDEFINED);
            });
            let failed = transaction.clone();
            let reject_abort = reject.clone();
            let on_error: Handler = Closure::new(move |_: Event| {
                let value = failed
                    .error()
                    .map(JsValue::from)
                    .unwrap_or(JsValue::UNDEFINED);
                let _ = reject.call1(&JsValue::UNDEFINED, &value);
            });
            let aborted = transaction.clone();
            let on_abort: Handler = Closure::new(move |_: Event| {
                let value = aborted
                    .error()
                    .map(JsValue::from)
                    .unwrap_or(JsValue::UNDEFINED);
                let _ = reject_abort.call1(&JsValue::UNDEFINED, &value);
            });
            transaction.set_oncomplete(Some(on_complete.as_ref().unchecked_ref()));
            transaction.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            transaction.set_onabort(Some(on_abort.as_ref().unchecked_ref()));
            handlers = Some((on_complete, on_error, on_abort));
        });
        let result = JsFuture::from(promise).await;
        transaction.set_oncomplete(None);
        transaction.set_onerror(None);
        transaction.set_onabort(None);
        drop(handlers);
        result.map(|_| ()).map_err(js_error)
    }

    /// `files` のキー `[project_id, path]`。
    fn file_key(project_id: &str, path: &str) -> JsValue {
        Array::of2(&JsValue::from_str(project_id), &JsValue::from_str(path)).into()
    }

    // ---------------------------------------------------------------- DB

    /// 開いた DB（§13.3.2）。
    #[derive(Debug, Clone)]
    pub struct Idb {
        db: IdbDatabase,
    }

    impl Idb {
        /// DB `novnow` v1 を開く（無ければストア 3 つと索引 2 つを作る）。使えなければ
        /// [`StorageError::Unavailable`]（**E062**。LocalStorage へはフォールバックしない、§13.3.7）。
        pub async fn open() -> Result<Self, StorageError> {
            let window = web_sys::window().ok_or_else(|| unavailable("no window"))?;
            let factory = window
                .indexed_db()
                .ok()
                .flatten()
                .ok_or_else(|| unavailable("indexedDB is not available"))?;
            let request: IdbOpenDbRequest = factory
                .open_with_u32(DB_NAME, DB_VERSION)
                .map_err(|_| unavailable("indexedDB.open failed"))?;
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
            let db = opened.map_err(|_| unavailable("indexedDB.open was rejected"))?;
            Ok(Self {
                db: db.unchecked_into(),
            })
        }

        /// プロジェクトのストレージ（そのプロジェクトのレコードの中だけを読み書きする）。
        pub fn storage(&self, project_id: &str) -> IdbStorage {
            IdbStorage {
                db: self.db.clone(),
                project_id: project_id.to_owned(),
            }
        }

        fn transaction(
            &self,
            stores: &[&str],
            mode: IdbTransactionMode,
        ) -> Result<IdbTransaction, StorageError> {
            let names = Array::new();
            for store in stores {
                names.push(&JsValue::from_str(store));
            }
            self.db
                .transaction_with_str_sequence_and_mode(&names, mode)
                .map_err(js_error)
        }

        /// 全プロジェクト（更新の新しい順。選択モーダル、§13.4）。
        pub async fn projects(&self) -> Result<Vec<ProjectRecord>, StorageError> {
            let transaction = self.transaction(&[PROJECTS], IdbTransactionMode::Readonly)?;
            let store = transaction.object_store(PROJECTS).map_err(js_error)?;
            let request = store.get_all().map_err(js_error)?;
            let mut records: Vec<ProjectRecord> = from_js(&wait(&request).await?)?;
            records.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
            Ok(records)
        }

        /// プロジェクトを作る（`projects` ＋ `files` を 1 トランザクション、§13.3.3）。
        /// 同じ ID があれば失敗する（既存に上書きしない、§13.5 MUST）。
        pub async fn create_project(
            &self,
            record: &ProjectRecord,
            files: &[(ProjectPath, String)],
        ) -> Result<(), StorageError> {
            let transaction =
                self.transaction(&[PROJECTS, FILES], IdbTransactionMode::Readwrite)?;
            let projects = transaction.object_store(PROJECTS).map_err(js_error)?;
            let store = transaction.object_store(FILES).map_err(js_error)?;
            projects.add(&to_js(record)?).map_err(js_error)?;
            for (path, text) in files {
                let file = FileRecord::new(&record.id, path, text, &record.updated_at);
                store.put(&to_js(&file)?).map_err(js_error)?;
            }
            commit(&transaction).await
        }

        /// プロジェクトを消す（`by_project` で全ファイル ＋ `projects` を 1 トランザクション、§13.3.3）。
        pub async fn delete_project(&self, project_id: &str) -> Result<(), StorageError> {
            let transaction =
                self.transaction(&[PROJECTS, FILES], IdbTransactionMode::Readwrite)?;
            let store = transaction.object_store(FILES).map_err(js_error)?;
            let index = store.index(BY_PROJECT).map_err(js_error)?;
            let request = index
                .get_all_keys_with_key(&JsValue::from_str(project_id))
                .map_err(js_error)?;
            let keys: Array = wait(&request).await?.unchecked_into();
            for key in keys.iter() {
                store.delete(&key).map_err(js_error)?;
            }
            let projects = transaction.object_store(PROJECTS).map_err(js_error)?;
            projects
                .delete(&JsValue::from_str(project_id))
                .map_err(js_error)?;
            commit(&transaction).await
        }

        /// 表示名を変える（`F2`、§13.4）。
        pub async fn rename_project(
            &self,
            project_id: &str,
            name: &str,
        ) -> Result<(), StorageError> {
            let transaction = self.transaction(&[PROJECTS], IdbTransactionMode::Readwrite)?;
            let projects = transaction.object_store(PROJECTS).map_err(js_error)?;
            let request = projects
                .get(&JsValue::from_str(project_id))
                .map_err(js_error)?;
            let found = wait(&request).await?;
            if found.is_undefined() {
                return Err(other(format!("no project {project_id}")));
            }
            let mut record: ProjectRecord = from_js(&found)?;
            record.name = name.to_owned();
            record.updated_at = now();
            projects.put(&to_js(&record)?).map_err(js_error)?;
            commit(&transaction).await
        }

        /// 全ファイル（エクスポート。読み取り専用トランザクション、§13.3.3）。
        pub async fn export_files(
            &self,
            project_id: &str,
        ) -> Result<Vec<(ProjectPath, String)>, StorageError> {
            let mut files: Vec<(ProjectPath, String)> = self
                .storage(project_id)
                .records()
                .await?
                .into_iter()
                .filter_map(|record| Some((record.project_path()?, record.text)))
                .collect();
            files.sort_by(|left, right| left.0.cmp(&right.0));
            Ok(files)
        }

        /// Zip のエクスポート（§13.5）。ダウンロードは [`download`]。
        pub async fn export_zip(&self, project_id: &str) -> Result<Vec<u8>, ArchiveError> {
            let files: Vec<(ProjectPath, Vec<u8>)> = self
                .export_files(project_id)
                .await?
                .into_iter()
                .map(|(path, text)| (path, text.into_bytes()))
                .collect();
            archive_io::export(&files)
        }

        /// Zip の取り込み: **新しいプロジェクトレコード**を作る（既存に上書きしない、§13.5 MUST）。
        /// テキストとして読めないエントリは取り込まずに返す。§4.3 の検査は呼び出し側
        /// （`project_load::load` に [`Idb::storage`] を渡す）。
        pub async fn import_zip(
            &self,
            name: &str,
            bytes: &[u8],
        ) -> Result<(ProjectRecord, Vec<Skipped>), ArchiveError> {
            let (files, skipped) = archive_io::import(bytes)?.into_texts();
            let record = ProjectRecord::new(&random_id()?, name, Origin::Import, &now());
            self.create_project(&record, &files).await?;
            Ok((record, skipped))
        }

        /// `projects` が空なら `sample` を 1 つだけ作る（§13.3.5）。作ったら `true`。中身（付録 C）は
        /// 呼び出し側が渡す。**作った後も自動で開かない**（選択モーダルを出す。MUST）。
        pub async fn seed_sample(
            &self,
            files: &[(ProjectPath, String)],
        ) -> Result<bool, StorageError> {
            if !self.projects().await?.is_empty() {
                return Ok(false);
            }
            let record = ProjectRecord::new(SAMPLE_ID, SAMPLE_ID, Origin::Sample, &now());
            self.create_project(&record, files).await?;
            Ok(true)
        }

        /// プロジェクト ID の履歴（§13.3.4）。無い・壊れていれば空。
        pub async fn recent(&self) -> Result<RecentProjects, StorageError> {
            let transaction = self.transaction(&[APP], IdbTransactionMode::Readonly)?;
            let store = transaction.object_store(APP).map_err(js_error)?;
            let request = store
                .get(&JsValue::from_str(RECENT_PROJECTS_KEY))
                .map_err(js_error)?;
            let found = wait(&request).await?;
            if found.is_undefined() {
                return Ok(RecentProjects::default());
            }
            let record: Option<AppRecord> = from_js(&found).ok();
            Ok(RecentProjects::from_value(record.map(|found| found.value)))
        }

        /// 開いたプロジェクトを履歴の先頭に積む。`app` ストアに書く**唯一の口**（§13.3.4）。
        pub async fn push_recent(
            &self,
            project_id: &str,
            opened_at: &str,
        ) -> Result<RecentProjects, StorageError> {
            let mut recent = self.recent().await?;
            recent.push(ProjectItem {
                id: project_id.to_owned(),
                opened_at: opened_at.to_owned(),
            });
            let transaction = self.transaction(&[APP], IdbTransactionMode::Readwrite)?;
            let store = transaction.object_store(APP).map_err(js_error)?;
            store
                .put(&to_js(&AppRecord::recent(&recent))?)
                .map_err(js_error)?;
            commit(&transaction).await?;
            Ok(recent)
        }

        /// LocalStorage（v0）からの取り込み（§13.3.6）。変換は M11 まで失敗する（[`convert_v0`]）。
        pub async fn import_local_storage(&self, clock: &impl Clock) -> LegacyOutcome {
            self.import_local_storage_with(clock, convert_v0).await
        }

        /// 変換を渡して取り込む（M11 で `convert_v0` の代わりに移行器を渡す）。
        pub async fn import_local_storage_with(
            &self,
            clock: &impl Clock,
            convert: impl FnOnce(
                Vec<(ProjectPath, String)>,
            ) -> Result<Vec<(ProjectPath, String)>, LegacyError>,
        ) -> LegacyOutcome {
            let Some(storage) =
                web_sys::window().and_then(|window| window.local_storage().ok().flatten())
            else {
                return LegacyOutcome::Nothing;
            };
            let Some(index) = storage.get_item(LEGACY_INDEX_KEY).ok().flatten() else {
                return LegacyOutcome::Nothing;
            };
            let legacy = match read_legacy(&index, |key| storage.get_item(key).ok().flatten()) {
                Ok(legacy) => legacy,
                Err(error) => return LegacyOutcome::Failed(error),
            };
            let files = match convert(legacy.files) {
                Ok(files) => files,
                Err(error) => return LegacyOutcome::Failed(error),
            };
            let taken: Vec<String> = match self.projects().await {
                Ok(records) => records.into_iter().map(|record| record.id).collect(),
                Err(error) => return LegacyOutcome::Failed(LegacyError::Storage(error)),
            };
            let date = yyyymmdd(clock.now_millis(), clock.utc_offset_minutes());
            let id = migrated_id(&date, &taken);
            let record = ProjectRecord::new(&id, &id, Origin::Migrated, &now());
            if let Err(error) = self.create_project(&record, &files).await {
                return LegacyOutcome::Failed(LegacyError::Storage(error));
            }
            for key in &legacy.keys {
                let _ = storage.remove_item(key);
            }
            LegacyOutcome::Migrated(id)
        }
    }

    /// ストア 3 つと索引 2 つを作る（`onupgradeneeded`）。
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

    /// UUIDv4（`crypto.getRandomValues`）。
    fn random_id() -> Result<String, StorageError> {
        let crypto = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
            .map_err(js_error)?;
        let fill = js_sys::Reflect::get(&crypto, &JsValue::from_str("getRandomValues"))
            .map_err(js_error)?;
        let fill: Function = fill.dyn_into().map_err(js_error)?;
        let buffer = js_sys::Uint8Array::new_with_length(16);
        fill.call1(&crypto, &buffer).map_err(js_error)?;
        let mut bytes = [0u8; 16];
        buffer.copy_to(&mut bytes);
        Ok(uuid_v4(bytes))
    }

    // ---------------------------------------------------------------- プロジェクト 1 つ

    /// プロジェクト 1 つ分のストレージ（[`Storage`] の実装）。`project_id` のレコードだけを扱う。
    #[derive(Debug, Clone)]
    pub struct IdbStorage {
        db: IdbDatabase,
        project_id: String,
    }

    impl IdbStorage {
        /// プロジェクトの ID。
        pub fn project_id(&self) -> &str {
            &self.project_id
        }

        fn transaction(
            &self,
            stores: &[&str],
            mode: IdbTransactionMode,
        ) -> Result<IdbTransaction, StorageError> {
            Idb {
                db: self.db.clone(),
            }
            .transaction(stores, mode)
        }

        /// このプロジェクトの全レコード。
        async fn records(&self) -> Result<Vec<FileRecord>, StorageError> {
            let transaction = self.transaction(&[FILES], IdbTransactionMode::Readonly)?;
            let store = transaction.object_store(FILES).map_err(js_error)?;
            let index = store.index(BY_PROJECT).map_err(js_error)?;
            let request = index
                .get_all_with_key(&JsValue::from_str(&self.project_id))
                .map_err(js_error)?;
            from_js(&wait(&request).await?)
        }

        async fn record(&self, path: &ProjectPath) -> Result<Option<FileRecord>, StorageError> {
            let transaction = self.transaction(&[FILES], IdbTransactionMode::Readonly)?;
            let store = transaction.object_store(FILES).map_err(js_error)?;
            let request = store
                .get(&file_key(&self.project_id, path.as_str()))
                .map_err(js_error)?;
            let found = wait(&request).await?;
            if found.is_undefined() {
                return Ok(None);
            }
            from_js(&found).map(Some)
        }

        /// 1 ファイルを書いて `projects.updated_at` を更新する（同一トランザクション、§13.3.3）。
        async fn put(&self, path: &ProjectPath, text: String) -> Result<Stamp, StorageError> {
            let stamp_time = now();
            let transaction =
                self.transaction(&[FILES, PROJECTS], IdbTransactionMode::Readwrite)?;
            let files = transaction.object_store(FILES).map_err(js_error)?;
            let record = FileRecord::new(&self.project_id, path, &text, &stamp_time);
            files.put(&to_js(&record)?).map_err(js_error)?;
            let projects = transaction.object_store(PROJECTS).map_err(js_error)?;
            let request = projects
                .get(&JsValue::from_str(&self.project_id))
                .map_err(js_error)?;
            let found = wait(&request).await?;
            if !found.is_undefined() {
                let mut project: ProjectRecord = from_js(&found)?;
                project.updated_at = stamp_time.clone();
                projects.put(&to_js(&project)?).map_err(js_error)?;
            }
            commit(&transaction).await?;
            Ok(record.stamp())
        }
    }

    impl Storage for IdbStorage {
        async fn list(&self) -> Result<Vec<FileEntry>, StorageError> {
            let mut entries: Vec<FileEntry> = self
                .records()
                .await?
                .into_iter()
                .filter_map(|record| {
                    Some(FileEntry {
                        path: record.project_path()?,
                        stamp: record.stamp(),
                    })
                })
                .collect();
            entries.sort_by(|left, right| left.path.cmp(&right.path));
            Ok(entries)
        }

        async fn dirs(&self) -> Result<Option<Vec<ProjectPath>>, StorageError> {
            // IndexedDB にディレクトリは無い（§4.3 第 2.9 版: 必須ディレクトリは常にあるとみなす）
            Ok(None)
        }

        async fn read(&self, path: &ProjectPath) -> Result<String, StorageError> {
            self.record(path)
                .await?
                .map(|record| record.text)
                .ok_or_else(|| StorageError::NotFound(path.clone()))
        }

        async fn write(&self, path: &ProjectPath, text: &str) -> Result<Stamp, StorageError> {
            self.put(path, text.to_owned()).await
        }

        async fn append(&self, path: &ProjectPath, text: &str) -> Result<Stamp, StorageError> {
            let mut current = self
                .record(path)
                .await?
                .map(|record| record.text)
                .unwrap_or_default();
            current.push_str(text);
            self.put(path, current).await
        }

        async fn remove(&self, path: &ProjectPath) -> Result<(), StorageError> {
            if self.record(path).await?.is_none() {
                return Err(StorageError::NotFound(path.clone()));
            }
            let transaction = self.transaction(&[FILES], IdbTransactionMode::Readwrite)?;
            let store = transaction.object_store(FILES).map_err(js_error)?;
            store
                .delete(&file_key(&self.project_id, path.as_str()))
                .map_err(js_error)?;
            commit(&transaction).await
        }
    }

    // ---------------------------------------------------------------- navigator.storage・ダウンロード

    /// `navigator.storage.persist()` を要求する（拒否されても続行、§13.3.7）。
    pub async fn request_persist() {
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(promise) = window.navigator().storage().persist() {
            let _ = JsFuture::from(promise).await;
        }
    }

    /// `navigator.storage.estimate()` の `(quota, usage)`（byte）。分からなければ `None`。
    /// 保存前に [`super::is_low_space`] で W062 を判定する（§13.3.7）。
    pub async fn estimate() -> Option<(f64, f64)> {
        let window = web_sys::window()?;
        let promise = window.navigator().storage().estimate().ok()?;
        let value = JsFuture::from(promise).await.ok()?;
        let estimate: StorageEstimate = value.unchecked_into();
        Some((estimate.get_quota()?, estimate.get_usage()?))
    }

    /// バイト列をダウンロードさせる（Zip のエクスポート、§13.5。保存先はブラウザが決める）。
    pub fn download(file_name: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let array = js_sys::Uint8Array::from(bytes);
        let parts = Array::new();
        parts.push(&array.buffer());
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/zip");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
            .map_err(js_error)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error)?;
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| unavailable("no document"))?;
        let anchor: web_sys::HtmlElement = document
            .create_element("a")
            .map_err(js_error)?
            .dyn_into()
            .map_err(|_| other("not an HtmlElement".to_owned()))?;
        anchor.set_attribute("href", &url).map_err(js_error)?;
        anchor
            .set_attribute("download", file_name)
            .map_err(js_error)?;
        anchor.click();
        let _ = web_sys::Url::revoke_object_url(&url);
        Ok(())
    }
}
