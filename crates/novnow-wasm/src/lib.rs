//! 第 2 の画面実装（`REF:docs/specifications/webui.md` §21）のための **ブラウザ用の橋渡し**。
//!
//! # ここが持つもの（設計 §21.3 の 3 つの型）
//!
//! | 型 | 役割 |
//! |---|---|
//! | [`NovnowSession`] | `open` / `run` / `poll_events` / `flush`。画面はこの 4 つと JSON だけを知る |
//! | [`ProjectFiles`] | `open_project_files(files_json)` の入力（Zip の取り込みと同じ形の「パス → 中身」） |
//! | [`version`] | 版の文字列（`Session::version()`） |
//!
//! # 設計の要点（`REF:docs/plans.md#webui-w2`）
//!
//! - **メモリ正本＋IndexedDB 書き込みキュー**: `Host` の `write` は同期で結果を返す
//!   （`REF:src/app/host.rs`）。IndexedDB は非同期なので `write` の中で待てない。そこで
//!   起動時にプロジェクトの全文をメモリへ読み、`write` はメモリへ即時に反映して
//!   [`storage::FileStore`] の**書き込みキュー**へ積む。読み取りは常にメモリから返すので
//!   `Session` は同期のままでよい（`REF:docs/developers.md` §8.10 の 30）。
//! - **失敗は保存状態機の `error` として返す**: IndexedDB の失敗を `write` の戻り値に混ぜない。
//!   キューの失敗は [`NovnowSession::flush`]（とその後の `poll_events`）が
//!   `SaveStateChanged(state=error)` として知らせる（§8.3・§21.2.3）。
//! - **`Session` はスレッドを跨がない**: `Session` は `Send` ではないので、ワーカーを挟まず
//!   1 つの `Rc<RefCell<...>>` に閉じる（`REF:docs/developers.md` §8.10 の 31）。
//! - **JSON は文字列で渡す**: `run(op, args)` の `args` と戻り値は**文字列の JSON**。`JsValue` へ
//!   直接写すとキーの順が環境で変わり、§19.1.4 の決定論が崩れる（同 32）。
//!
//! # ビルド
//!
//! ```text
//! wasm-pack build crates/novnow-wasm --target web --out-dir webui/src/wasm
//! ```
//!
//! `wasm-pack` が無い環境（作業サンドボックス）では、同じことを
//! `cargo build --target wasm32-unknown-unknown -p novnow-wasm` ＋
//! `wasm-bindgen --target web` で行う（`REF:scripts/webui-build.sh`）。

mod host;
mod project;
mod storage;

use std::cell::RefCell;
use std::rc::Rc;

use serde::Serialize;
use wasm_bindgen::prelude::*;

use novnow::app::session::Session;

pub use project::ProjectFiles;
pub use storage::{IdbFiles, MemoryFiles};

use host::NovnowHost;

/// 版の文字列（`Session::version()`。`REF:docs/specifications/interfaces.md` §19.8.1）。
#[wasm_bindgen]
pub fn version() -> String {
    Session::version().to_owned()
}

/// ブラウザで動く 1 つのセッション（設計 §21.3 の `NovnowSession`）。
///
/// 画面（`REF:webui/`）はこの型の 5 つの口だけを使う:
///
/// | 口 | 何をするか |
/// |---|---|
/// | `open(root)` | IndexedDB `novnow-webui` からプロジェクトを読んで開く。開いている間は 1 つだけ |
/// | `run(op, args_json)` | 正規操作 1 回。戻り値は**文字列の JSON**（成功・失敗の両方） |
/// | `poll_events()` | 溜まったイベント（`REF:src/app/event.rs` の `AppEvent`）の JSON 配列 |
/// | `flush()` | 未保存の変更を書く。IndexedDB へ流し終えるまで待つ（**中で待つので `await` する**） |
/// | `locale` / `version()` | 文言の言語と版 |
///
/// 失敗は `run` の戻り値に `{"ok": false, "error": {...}}` の形で入る
/// （`REF:src/app/error.rs` の `Error::to_json`）。例外は投げない（画面側の `if` を減らすため）。
#[wasm_bindgen]
pub struct NovnowSession {
    inner: Rc<RefCell<Inner>>,
}

struct Inner {
    /// 開いているセッション（`open` の前は `None`）。
    session: Option<Session>,
    /// 書き込みの後ろに居るストア（メモリ or IndexedDB）。
    store: Rc<dyn storage::FileStore>,
    /// プロジェクトの ID（IndexedDB のキー。§13.3.2）。
    project_id: String,
}

/// `run` の戻り値（成功）。
#[derive(Serialize)]
struct RunOk<'a, T: Serialize> {
    ok: bool,
    result: &'a T,
}

/// `run` の戻り値（失敗）。
#[derive(Serialize)]
struct RunErr {
    ok: bool,
    error: serde_json::Value,
}

#[wasm_bindgen]
impl NovnowSession {
    /// 空のセッション（`open` を呼ぶまで操作できない）。
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        Self {
            inner: Rc::new(RefCell::new(Inner {
                session: None,
                store: Rc::new(MemoryFiles::new()),
                project_id: String::new(),
            })),
        }
    }

    /// IndexedDB `novnow-webui` のプロジェクトを開く（設計 §21.3）。
    ///
    /// `root` はプロジェクトの ID（`sample` など）。`None` なら DB に 1 つだけ在る
    /// プロジェクトを開き、1 つも無ければ付録 C のサンプルを入れてから開く（§13.3.5）。
    ///
    /// 中で IndexedDB を待つので、画面からは `await` する。失敗は `Error`（JS の例外）で返す
    /// （開けなかったことに気づかないまま操作を受け付けないため）。
    pub async fn open(&self, root: Option<String>) -> Result<(), JsValue> {
        let idb = IdbFiles::open().await.map_err(fail)?;
        let project = match root {
            Some(id) if !id.trim().is_empty() => idb.load(id.trim()).await.map_err(fail)?,
            _ => idb.load_or_seed_sample().await.map_err(fail)?,
        };
        let project_id = project.id.clone();
        let store: Rc<dyn storage::FileStore> = Rc::new(idb.clone());
        let session = boot(project, store.clone());
        let mut inner = self.inner.borrow_mut();
        inner.session = Some(session);
        inner.store = store;
        inner.project_id = project_id;
        Ok(())
    }

    /// 第 1 の保存領域を使わずに、メモリだけでプロジェクトを開く（テストと、IndexedDB の
    /// 無い環境での確認用）。`files_json` は [`ProjectFiles`] の形（`{"files": [...]}`）。
    ///
    /// 中で待たない（メモリだけ）ので、画面から `await` しなくてよい。
    pub fn open_in_memory(&self, files_json: &str) -> Result<(), JsValue> {
        let files = ProjectFiles::parse(files_json).map_err(fail)?;
        let store = Rc::new(MemoryFiles::new());
        store.replace(files.files.clone());
        let project = project::from_files(&files, "memory");
        let mut inner = self.inner.borrow_mut();
        inner.session = Some(boot(project, store.clone()));
        inner.store = store;
        inner.project_id = "memory".to_owned();
        Ok(())
    }

    /// 保存領域を使えない環境（IndexedDB の無い iframe・プライベートモードなど）で、
    /// メモリだけでサンプルを開く。
    ///
    /// **開けなかったことを黙って隠さない**: 画面は「保存されない」ことを通知に出す
    /// （§21.1.2「未提供は明示する」の精神）。中で待たないので `await` しなくてよい。
    pub fn open_fallback(&self) -> Result<(), JsValue> {
        let files = novnow::core::template::sample_project()
            .into_iter()
            .filter(|file| {
                novnow::core::model::document::DocumentFormat::of_path(&file.path)
                    != novnow::core::model::document::DocumentFormat::Unknown
            })
            .map(|file| (file.path, file.text))
            .collect::<Vec<_>>();
        let store = Rc::new(MemoryFiles::new());
        store.replace(files.clone());
        let project = project::Loaded::from_documents(&files, "sample", "memory")?;
        let mut inner = self.inner.borrow_mut();
        inner.session = Some(boot(project, store.clone()));
        inner.store = store;
        inner.project_id = "memory".to_owned();
        Ok(())
    }

    /// 正規操作 1 回（`Session::run`）。戻り値は**文字列の JSON**。
    ///
    /// 成功: `{"ok": true, "result": {...}}` / 失敗: `{"ok": false, "error": {"code": ...}}`。
    /// 例外は投げない（画面は `ok` だけを見る）。
    pub fn run(&self, op: &str, args_json: &str) -> String {
        let args = match project::parse_args(args_json) {
            Ok(args) => args,
            Err(message) => {
                return serde_json::to_string(&RunErr {
                    ok: false,
                    error: serde_json::json!({
                        "code": "usage",
                        "kind": "",
                        "message": message,
                        "path": serde_json::Value::Null,
                    }),
                })
                .unwrap_or_else(|_| "{\"ok\":false}".to_owned())
            }
        };
        let mut inner = self.inner.borrow_mut();
        let Some(session) = inner.session.as_mut() else {
            return not_open();
        };
        match session.run(op, args) {
            Ok(value) => serde_json::to_string(&RunOk {
                ok: true,
                result: &value,
            })
            .unwrap_or_else(|_| not_open()),
            Err(error) => serde_json::to_string(&RunErr {
                ok: false,
                error: error.to_json(),
            })
            .unwrap_or_else(|_| not_open()),
        }
    }

    /// 溜まったイベント（`AppEvent` の JSON 配列）。
    pub fn poll_events(&self) -> String {
        let mut inner = self.inner.borrow_mut();
        let Some(session) = inner.session.as_mut() else {
            return "[]".to_owned();
        };
        let events = session.poll_events();
        serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_owned())
    }

    /// 未保存の変更を書く（`Session::flush` と同じ規則。§8.3 の 1 回分）。
    ///
    /// メモリ正本へは即時に反映し、IndexedDB へ**流し終えるまで待つ**。流し込みに失敗したら
    /// `{"ok": true, "result": {...}, "pending": ["path", ...], "error": "理由"}` の形で
    /// 知らせる（メモリには残っているので、次の `flush` でもう一度試す）。
    ///
    /// 書くものが無ければ何もしない（起動直後の `flush` で無駄な往復をしない）。
    pub async fn flush(&self) -> Result<String, JsValue> {
        let handle = self.inner.clone();
        // 同期の部分（`Session::flush`）を先に終える。`Session` を await の向こうへ持ち越さない。
        let pending = {
            let mut inner = handle.borrow_mut();
            let Some(session) = inner.session.as_mut() else {
                return Ok(not_open());
            };
            match session.flush() {
                Ok(value) => {
                    let result = serde_json::to_string(&RunOk {
                        ok: true,
                        result: &value,
                    })
                    .unwrap_or_else(|_| not_open());
                    (
                        result,
                        session
                            .workbench()
                            .unsaved()
                            .map(str::to_owned)
                            .collect::<Vec<_>>(),
                    )
                }
                Err(error) => {
                    let text = serde_json::to_string(&RunErr {
                        ok: false,
                        error: error.to_json(),
                    })
                    .unwrap_or_else(|_| not_open());
                    return Ok(text);
                }
            }
        };
        let (result, unsaved) = pending;
        // 流すかどうかは**書き込みキュー**で決める（`unsaved` は `Session` の未保存一覧で、
        // `save.now` の直後は空になる）。ここを取り違えると、書いたのに流さない。
        let store = handle.borrow().store.clone();
        if store.pending() == 0 {
            return Ok(result);
        }
        let queued = store.pending_paths();
        match store.drain().await {
            Ok(()) => Ok(result),
            Err(message) => {
                let pending = if queued.is_empty() { unsaved } else { queued };
                Ok(with_pending(&result, &pending, &message))
            }
        }
    }

    /// まだ IndexedDB へ流していない書き込み（件数）。
    pub fn pending_writes(&self) -> usize {
        self.inner.borrow().store.pending()
    }

    /// 文言の言語（`ja` / `en`）。
    pub fn locale(&self) -> String {
        let inner = self.inner.borrow();
        match inner.session.as_ref().map(|session| session.locale()) {
            Some(locale) => locale.code().to_owned(),
            None => "ja".to_owned(),
        }
    }

    /// 文言の言語を切り替える（`settings.set` と同じく `ui.json` にも書く）。
    pub fn set_locale(&self, locale: &str) -> Result<(), JsValue> {
        let mut inner = self.inner.borrow_mut();
        let Some(session) = inner.session.as_mut() else {
            return Err(JsValue::from_str("session is not open"));
        };
        let next = match locale {
            "ja" => novnow::ui::i18n::Locale::Ja,
            "en" => novnow::ui::i18n::Locale::En,
            other => return Err(JsValue::from_str(&format!("unknown locale {other}"))),
        };
        session.set_locale(next);
        Ok(())
    }

    /// 文言カタログの 1 行（§12.3）。**写しを作らず**、第 1 と同じ
    /// `REF:src/ui/i18n/ja.toml` / `en.toml` をそのまま引く（§21.2.4）。
    ///
    /// `args_json` は `{0}` の差し込み（`["名前"]` の形。省略可）。キーが無ければ
    /// 空文字を返す（画面は空文字を「無い」として扱う）。
    ///
    /// 画面は `scripts/i18n-ts.py` が作った `webui/src/lib/i18n/catalog.ts` を引く（W3 の
    /// 生成物）。この口は、生成物が無い環境（`npm run dev` をまだ通していない状態）でも
    /// 文言を出せるようにするための予備である（**正本は同じ TOML**）。
    pub fn text(&self, key: &str, args_json: Option<String>) -> String {
        let locale = {
            let inner = self.inner.borrow();
            inner
                .session
                .as_ref()
                .map(|session| session.locale())
                .unwrap_or(novnow::ui::i18n::Locale::Ja)
        };
        let args: Vec<String> = args_json
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
            .unwrap_or_default();
        if args.is_empty() {
            novnow::ui::i18n::text(locale, key).to_owned()
        } else {
            let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
            novnow::ui::i18n::format(locale, key, &borrowed)
        }
    }

    /// 文言カタログの全キー（`?` のヘルプとパレットの候補を作るのに使う。§9.8）。
    ///
    /// `[[key, text], ...]` の JSON。キーは `ja.toml` と `en.toml` で同じ（§12.3）。
    pub fn keys(&self) -> String {
        let locale = {
            let inner = self.inner.borrow();
            inner
                .session
                .as_ref()
                .map(|session| session.locale())
                .unwrap_or(novnow::ui::i18n::Locale::Ja)
        };
        let catalog = novnow::ui::i18n::catalog(locale);
        let rows: Vec<serde_json::Value> = catalog
            .keys()
            .map(|key| serde_json::json!([key, novnow::ui::i18n::text(locale, key)]))
            .collect();
        serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned())
    }

    /// 版（`version()` と同じ。インスタンスからも読めるようにする）。
    pub fn version(&self) -> String {
        Session::version().to_owned()
    }

    /// 開いているプロジェクトの ID（IndexedDB のキー）。
    pub fn project_id(&self) -> String {
        self.inner.borrow().project_id.clone()
    }

    /// Zip の取り込み（設計 §21.3 の `open_project_files`）: `files_json` のファイルを
    /// まとめて書き、開いているセッションへ反映する（`project.import_zip` と同じ規則）。
    pub fn open_project_files(&self, files_json: &str) -> Result<(), JsValue> {
        let files = ProjectFiles::parse(files_json).map_err(fail)?;
        let mut inner = self.inner.borrow_mut();
        let Some(session) = inner.session.as_mut() else {
            return Err(JsValue::from_str("session is not open"));
        };
        for (path, text) in &files.files {
            session
                .run(
                    "document.write",
                    serde_json::json!({ "path": path, "text": text }),
                )
                .map_err(|error| JsValue::from_str(&error.message))?;
        }
        Ok(())
    }
}

impl Default for NovnowSession {
    fn default() -> Self {
        Self::new()
    }
}

/// セッションを作る（`ui.json` の設定も保存領域から読む。§4.4.1）。
fn boot(project: project::Loaded, store: Rc<dyn storage::FileStore>) -> Session {
    let settings = project.settings.clone();
    let host = NovnowHost::new(store, project.name.clone());
    let mut session = Session::from_project(Box::new(host), project.project, &settings);
    session.set_interface(novnow::app::session::Interface::Rust);
    session
}

fn not_open() -> String {
    serde_json::to_string(&RunErr {
        ok: false,
        error: serde_json::json!({
            "code": "usage",
            "kind": "",
            "message": "session is not open",
            "path": serde_json::Value::Null,
        }),
    })
    .unwrap_or_else(|_| "{\"ok\":false}".to_owned())
}

fn with_pending(result: &str, pending: &[String], message: &str) -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(result).unwrap_or_else(|_| serde_json::json!({ "ok": true }));
    if let Some(map) = value.as_object_mut() {
        map.insert("pending".to_owned(), serde_json::json!(pending));
        map.insert("error".to_owned(), serde_json::json!(message));
    }
    serde_json::to_string(&value).unwrap_or_else(|_| result.to_owned())
}

fn fail(message: String) -> JsValue {
    JsValue::from_str(&message)
}
