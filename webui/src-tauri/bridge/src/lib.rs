//! 第 2 の画面実装（`REF:docs/specifications/webui.md` §21）の **ネイティブ用の橋渡し**。
//! Tauri のコマンド 4 つ（`REF:webui/src-tauri/src/main.rs`）の中身で、**Tauri に依存しない**。
//!
//! # ここが持つもの（設計 §21.4）
//!
//! | 口 | 何をするか | 戻り値（**文字列の JSON**） |
//! |---|---|---|
//! | [`NativeSession::open`] | ディレクトリのプロジェクトを開く（`create_options` があれば新しく作る） | `run` と同じ形。成功の `result` は `locale` / `root` / `version` |
//! | [`NativeSession::run`] | 正規操作 1 回（`Session::run`） | `{"ok":true,"result":…}` / `{"ok":false,"error":{…}}` |
//! | [`NativeSession::poll_events`] | 溜まったイベント（`REF:src/app/event.rs` の `AppEvent`） | JSON の配列 |
//! | [`NativeSession::poll_external`] | 外部変更の監視 1 回分（§8.5） | `run` と同じ形。`result` は `changes` / `count` |
//!
//! ブラウザ用の橋渡し（`REF:crates/novnow-wasm/src/lib.rs`）と**同じ形**の JSON を返す。
//! 画面から見て 2 つの橋渡しが同じになるよう、開く前の操作・引数の誤りの文言も同じにしてある。
//!
//! # 設計の要点
//!
//! - **Host は `FsHost` をそのまま使う**（`REF:src/app/fs_host.rs`）: `Session::open` /
//!   `Session::create` が作る既定ホストで、書き込みはすべて関門 `FsGate`（規則 10・§13.1.2）を
//!   通る。このクレートは**ファイルに書く API を 1 つも呼ばない**（`REF:webui/tests/l6.test.mjs`
//!   が `REF:tests/layering.rs` と同じ一覧で確かめる）。
//! - **`Session` はスレッドを跨がない**: `Session` は `Send` ではない
//!   （`REF:src/app/driver.rs` の注記）。一方で Tauri が管理する状態は `Send + Sync` が要る。
//!   そこで `Session` は**持ち主のスレッド 1 本の中で作り、そこで使い、そこで捨てる**。
//!   [`NativeSession`] はそのスレッドへの口で、`Send` である。口は `&mut self` なので、
//!   Tauri 側は `Mutex<NativeSession>` を 1 つだけ持てばよい（設計 §21.4 の「`Session` を
//!   1 つだけ `Mutex` で持つ」）。
//! - **JSON は文字列で渡す**: 引数と戻り値は文字列の JSON。キーの順は `serde_json` の既定
//!   （辞書順）で決まり、ブラウザ版と同じバイト列になる（`REF:docs/developers.md` §8.10 の 32）。
//!   [`deterministic_json`] がこの前提を確かめる。
//! - **panic した操作の後のセッションは使わない**: 関門はデバッグビルドで違反を `panic!` に
//!   する（§13.1.2）。持ち主のスレッドは panic を受け止めて `io` のエラーを返し、
//!   **セッションを捨てる**（途中で止まった状態を保存させない）。画面は開き直す。
//!
//! ```no_run
//! use novnow_native_bridge::NativeSession;
//!
//! let mut session = NativeSession::new();
//! let opened = session.open("my-novel", None);
//! assert!(opened.starts_with("{\"ok\":true"));
//! let info = session.run("project.info", "{}");
//! let _ = info;
//! ```

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use serde_json::{json, Map, Value};

use novnow::app::error::{Class, Error};
use novnow::app::host::ExternalChange;
use novnow::app::session::{CreateOptions, Session};
use novnow::ports::storage::StorageError;
use novnow::ui::i18n::{self, Locale};

/// 開く前の操作の文言（`REF:crates/novnow-wasm/src/lib.rs` の `not_open` と同じ）。
const NOT_OPEN: &str = "session is not open";

/// 引数がオブジェクトでないときの文言（`REF:crates/novnow-wasm/src/project.rs` の
/// `parse_args` と同じ）。
const NOT_AN_OBJECT: &str = "arguments must be a JSON object";

/// セッションが無い間の文言の言語（`REF:src/app/driver.rs` と同じく既定の `ja`）。
const DEFAULT_LOCALE: Locale = Locale::Ja;

/// 持ち主のスレッドへ渡す 1 通。`Session` はスレッドの中から出ない。
type Task = Box<dyn FnOnce(&mut Option<Session>) + Send + 'static>;

/// ネイティブで動く 1 つのセッション（設計 §21.4）。
///
/// 開いている間は 1 つだけ。破棄（drop）は未保存の変更を捨てる（§19.2.5 と同じ）。
/// `Send` なので `Mutex<NativeSession>` を Tauri の状態として持てる。
#[derive(Default)]
pub struct NativeSession {
    /// 持ち主のスレッド（最初の呼び出しで起こす）。
    owner: Option<Owner>,
}

/// `Session` の持ち主のスレッドと、そこへの 1 本。
struct Owner {
    /// スレッドへの 1 本（捨てるとスレッドが終わる）。
    tasks: Sender<Task>,
    /// スレッド（`Drop` で待つ）。
    thread: JoinHandle<()>,
}

/// 持ち主のスレッドを使えなかった理由。
enum Stopped {
    /// スレッドを起こせなかった。
    Start,
    /// スレッドが居なくなった。
    Gone,
    /// 操作が panic した（文言は panic のメッセージ）。セッションは捨ててある。
    Panicked(String),
}

impl NativeSession {
    /// 空のセッション（[`NativeSession::open`] を呼ぶまで操作できない）。
    pub fn new() -> Self {
        Self::default()
    }

    /// プロジェクトを開く（設計 §21.4 の `open(path, create_options)`）。
    ///
    /// - `path`: プロジェクトのルート（ディレクトリ）。
    /// - `create_options`: `None`・空文字・`null` なら既存のプロジェクトを開く
    ///   （`Session::open`。§14.2 の起動前チェックと同じ検証）。JSON のオブジェクトなら
    ///   **新しく作ってから**開く（`Session::create`。キーは `title` / `author` / `language`
    ///   で、どれも省ける）。
    ///
    /// 新しく作るのは、**無いディレクトリか空のディレクトリ**だけにする。`Session::create` は
    /// 雛形のファイルをそのまま書くので、既にファイルがある場所で呼ぶと同じ名前のファイルが
    /// 置き換わる。ここで先に確かめて断る（§14.5 の「既存のファイルは上書きしない」を守る）。
    ///
    /// 成功: `{"ok":true,"result":{"locale":"ja","root":"<絶対パス>","version":"<版>"}}`。
    /// 画面の橋渡しの同期の口（`projectId` / `locale` / `version`）がそのまま使える 3 つである。
    /// 失敗: `{"ok":false,"error":{…}}`（開いていたセッションはそのまま残る）。
    pub fn open(&mut self, path: &str, create_options: Option<&str>) -> String {
        if path.trim().is_empty() {
            return fail(&Error::missing(DEFAULT_LOCALE, "path"));
        }
        let create = match parse_create_options(create_options) {
            Ok(create) => create,
            Err(error) => return fail(&error),
        };
        let path = path.to_owned();
        let called = self.call(move |slot| {
            let root = Path::new(&path);
            let opened = match create {
                Some(options) => match occupied(root) {
                    Ok(false) => Session::create(root, options),
                    Ok(true) => Err(exists(&path)),
                    Err(error) => Err(Error::io(error.to_string(), Some(path.as_str()))),
                },
                None => Session::open(root),
            };
            match opened {
                Ok(session) => {
                    let result = json!({
                        "locale": session.locale().code(),
                        "root": session
                            .root()
                            .map(|root| root.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        "version": Session::version(),
                    });
                    // 開けてから差し替える（それまでのセッションはここで捨てる。§19.2.5）
                    *slot = Some(session);
                    ok(&result)
                }
                Err(error) => fail(&error),
            }
        });
        called.unwrap_or_else(|stopped| fail(&stopped.error()))
    }

    /// 正規操作 1 回（`Session::run`）。戻り値は**文字列の JSON**。
    ///
    /// 成功: `{"ok":true,"result":{…}}` / 失敗: `{"ok":false,"error":{"code":…}}`。
    /// `args` は文字列の JSON（空文字・`null` は空のオブジェクト）。
    pub fn run(&mut self, op: &str, args: &str) -> String {
        let args = match parse_args(args) {
            Ok(args) => args,
            Err(message) => return fail(&Error::new(Class::Usage, message)),
        };
        let op = op.to_owned();
        let called = self.call(move |slot| match slot.as_mut() {
            Some(session) => match session.run(&op, args) {
                Ok(value) => ok(&value),
                Err(error) => fail(&error),
            },
            None => not_open(),
        });
        called.unwrap_or_else(|stopped| fail(&stopped.error()))
    }

    /// 溜まったイベント（`AppEvent` の JSON 配列）。開く前は空の配列。
    pub fn poll_events(&mut self) -> String {
        let called = self.call(|slot| match slot.as_mut() {
            Some(session) => {
                let events = session.poll_events();
                serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_owned())
            }
            None => "[]".to_owned(),
        });
        called.unwrap_or_else(|_| "[]".to_owned())
    }

    /// 外部変更の監視 1 回分（`Session::poll_external`。§8.5・設計 §21.2）。
    ///
    /// 画面はこれを **1 秒ごと**に呼ぶ。変更はセッションへ取り込まれ、出来事
    /// （`document_changed`・`save_state_changed`・`external_unreadable`）は
    /// [`NativeSession::poll_events`] で取り出せる。
    ///
    /// 戻り値は `run` と同じ形で、`result` は
    /// `{"changes":[{"change":"updated","path":"…"}],"count":1}`。`change` は
    /// `updated` / `removed` / `unreadable`（読めなかったものは `code` と `message` も持つ。
    /// 削除ではないので文書は残る）。本文は載せない（画面は `section.get` などで取り直す）。
    /// 開く前は 0 件。
    pub fn poll_external(&mut self) -> String {
        let called = self.call(|slot| {
            let changes: Vec<Value> = match slot.as_mut() {
                Some(session) => session.poll_external().iter().map(change).collect(),
                None => Vec::new(),
            };
            ok(&json!({ "count": changes.len(), "changes": changes }))
        });
        called.unwrap_or_else(|stopped| fail(&stopped.error()))
    }

    /// 持ち主のスレッドで `work` を走らせ、結果を待つ（**ブロックする**）。
    fn call<T: Send + 'static>(
        &mut self,
        work: impl FnOnce(&mut Option<Session>) -> T + Send + 'static,
    ) -> Result<T, Stopped> {
        // 居なくなったスレッド（panic が受け止められなかった場合）は起こし直す。
        if self
            .owner
            .as_ref()
            .is_some_and(|owner| owner.thread.is_finished())
        {
            self.owner = None;
        }
        if self.owner.is_none() {
            self.owner = Some(Owner::start()?);
        }
        let Some(owner) = self.owner.as_ref() else {
            return Err(Stopped::Start);
        };
        let (reply, wait) = mpsc::channel::<Result<T, String>>();
        let task: Task = Box::new(move |slot| {
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| work(slot)));
            let answer = match outcome {
                Ok(value) => Ok(value),
                Err(payload) => {
                    // 途中で止まった操作の後の状態は保存させない（開き直してもらう）
                    let _ = panic::catch_unwind(AssertUnwindSafe(|| drop(slot.take())));
                    Err(panic_message(payload.as_ref()))
                }
            };
            let _ = reply.send(answer);
        });
        owner.tasks.send(task).map_err(|_| Stopped::Gone)?;
        match wait.recv() {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(message)) => Err(Stopped::Panicked(message)),
            Err(_) => Err(Stopped::Gone),
        }
    }
}

impl Drop for NativeSession {
    /// 捨てるときに持ち主のスレッドを畳む（未保存の変更は捨てる。§19.2.5）。
    fn drop(&mut self) {
        if let Some(Owner { tasks, thread }) = self.owner.take() {
            drop(tasks);
            let _ = thread.join();
        }
    }
}

impl Owner {
    /// 持ち主のスレッドを起こす。
    fn start() -> Result<Self, Stopped> {
        let (tasks, queue) = mpsc::channel::<Task>();
        let thread = std::thread::Builder::new()
            .name("novnow-session".to_owned())
            .spawn(move || serve(queue))
            .map_err(|_| Stopped::Start)?;
        Ok(Self { tasks, thread })
    }
}

/// 持ち主のスレッド（1 本）。`Session` はここで作り、ここで捨てる。届いた 1 通を 1 通ずつ
/// 順に処理するので、複数スレッドからの呼び出しは**直列**になる。
fn serve(queue: Receiver<Task>) {
    let mut session: Option<Session> = None;
    for task in queue {
        task(&mut session);
    }
}

impl Stopped {
    /// エラー応答（§19.1.7 の `io`）。文言は `REF:src/app/driver.rs` と同じカタログの行。
    fn error(&self) -> Error {
        let message = match self {
            Self::Start => i18n::format(DEFAULT_LOCALE, "api.error.thread_start", &[]),
            Self::Gone => i18n::format(DEFAULT_LOCALE, "api.error.thread_stopped", &[]),
            Self::Panicked(message) if message.is_empty() => {
                i18n::text(DEFAULT_LOCALE, "api.error.generic").to_owned()
            }
            Self::Panicked(message) => message.clone(),
        };
        Error::new(Class::Io, message)
    }
}

/// panic のメッセージ（文字列でない payload は空）。
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        String::new()
    }
}

/// `serde_json` がキーを辞書順に並べるか（ブラウザ版と同じバイト列になる前提）。
///
/// `serde_json` の `preserve_order` が依存のどこかで有効になると、機能核が作る JSON
/// （`run` の戻り値と、`.novnow/ui.json` などの保存物）のキーが**挿入順**に変わり、
/// ブラウザ版と 1 バイトも違わないという受け入れ（`REF:docs/plans.md#webui-w5`）が崩れる。
/// Tauri 側は起動時にこれを確かめ、`false` なら動かさない。
pub fn deterministic_json() -> bool {
    let sample = json!({ "b": [1.5, true, null], "a": { "d": "é", "c": 1 } });
    serde_json::to_string(&sample).is_ok_and(|text| text == SORTED_SAMPLE)
}

/// [`deterministic_json`] の期待値（キーは辞書順・空白なし）。
const SORTED_SAMPLE: &str = r#"{"a":{"c":1,"d":"é"},"b":[1.5,true,null]}"#;

/// `run` の成功の形（`REF:crates/novnow-wasm/src/lib.rs` の `RunOk` と同じ並び）。
fn ok(result: &Value) -> String {
    format!("{{\"ok\":true,\"result\":{}}}", text(result))
}

/// `run` の失敗の形（`REF:crates/novnow-wasm/src/lib.rs` の `RunErr` と同じ並び）。
fn fail(error: &Error) -> String {
    format!("{{\"ok\":false,\"error\":{}}}", text(&error.to_json()))
}

/// 開く前の操作（例外にしない。画面は `ok` だけを見る）。
fn not_open() -> String {
    fail(&Error::new(Class::Usage, NOT_OPEN))
}

/// JSON の文字列（`Value` の直列化は失敗しないが、失敗を握りつぶさず `null` にする）。
fn text(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}

/// `run` の引数（`null` / 空文字は空のオブジェクト）。
/// `REF:crates/novnow-wasm/src/project.rs` の `parse_args` と同じ規則・同じ文言。
fn parse_args(text: &str) -> Result<Value, String> {
    let text = text.trim();
    if text.is_empty() || text == "null" {
        return Ok(Value::Object(Map::new()));
    }
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    match value {
        Value::Object(map) => Ok(Value::Object(map)),
        Value::Null => Ok(Value::Object(Map::new())),
        _ => Err(NOT_AN_OBJECT.to_owned()),
    }
}

/// `open` の `create_options`（§19.2.1 の `CreateOptions`。`None` は「既存を開く」）。
fn parse_create_options(text: Option<&str>) -> Result<Option<CreateOptions>, Error> {
    let Some(text) = text.map(str::trim) else {
        return Ok(None);
    };
    if text.is_empty() || text == "null" {
        return Ok(None);
    }
    let usage = || Error::usage(DEFAULT_LOCALE, "create_options");
    let value: Value = serde_json::from_str(text).map_err(|_| usage())?;
    let map = match value {
        Value::Object(map) => map,
        Value::Null => return Ok(None),
        _ => return Err(usage()),
    };
    let field = |name: &str| match map.get(name) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(_) => Err(Error::usage(DEFAULT_LOCALE, name)),
    };
    Ok(Some(CreateOptions {
        title: field("title")?,
        author: field("author")?,
        language: field("language")?,
    }))
}

/// 新しく作る場所に何かあるか（無い・空のディレクトリなら `false`）。**読むだけ**。
fn occupied(root: &Path) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() => Ok(std::fs::read_dir(root)?.next().is_some()),
        // ファイル・シンボリックリンクは「ある」（辿って中を確かめない）
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// 「既にある」の失敗（`io`。文言は関門の I/O エラーと同じ作り）。
fn exists(path: &str) -> Error {
    let kind = std::io::ErrorKind::AlreadyExists;
    let error = StorageError::Io {
        path: path.to_owned(),
        kind,
        message: std::io::Error::from(kind).to_string(),
    };
    Error::io(format!("{error:?}"), Some(path))
}

/// 外部変更 1 件の JSON（本文は載せない）。
fn change(found: &ExternalChange) -> Value {
    match found {
        ExternalChange::Updated(path, _) => json!({ "change": "updated", "path": path }),
        ExternalChange::Removed(path) => json!({ "change": "removed", "path": path }),
        ExternalChange::Unreadable(path, failure) => json!({
            "change": "unreadable",
            "code": failure.kind(),
            "message": failure.message(),
            "path": path,
        }),
    }
}

/// [`NativeSession`] は `Send`（Tauri の状態は `Mutex<NativeSession>`）。崩れたらここで落ちる。
const _: fn() = || {
    fn sendable<T: Send>() {}
    sendable::<NativeSession>();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_follow_the_browser_bridge() {
        assert_eq!(parse_args(""), Ok(json!({})));
        assert_eq!(parse_args("  "), Ok(json!({})));
        assert_eq!(parse_args("null"), Ok(json!({})));
        assert_eq!(parse_args("{\"a\":1}"), Ok(json!({ "a": 1 })));
        assert_eq!(parse_args("[1]"), Err(NOT_AN_OBJECT.to_owned()));
        assert_eq!(parse_args("1"), Err(NOT_AN_OBJECT.to_owned()));
        assert!(parse_args("{").is_err());
    }

    #[test]
    fn create_options_are_optional() {
        assert!(matches!(parse_create_options(None), Ok(None)));
        assert!(matches!(parse_create_options(Some("")), Ok(None)));
        assert!(matches!(parse_create_options(Some("null")), Ok(None)));
        let empty = parse_create_options(Some("{}")).expect("an object creates");
        assert_eq!(empty, Some(CreateOptions::default()));
        let titled = parse_create_options(Some("{\"title\":\"星屑の時計\",\"language\":\"ja\"}"))
            .expect("strings are accepted");
        assert_eq!(
            titled,
            Some(CreateOptions {
                title: "星屑の時計".to_owned(),
                author: String::new(),
                language: "ja".to_owned(),
            })
        );
    }

    #[test]
    fn create_options_reject_other_shapes() {
        for text in [
            "[]",
            "1",
            "\"title\"",
            "{",
            "{\"title\":1}",
            "{\"author\":[]}",
        ] {
            let error = parse_create_options(Some(text)).expect_err(text);
            assert_eq!(error.class, Class::Usage, "{text}");
        }
    }

    #[test]
    fn envelopes_match_the_browser_bridge() {
        assert_eq!(
            ok(&json!({ "b": 1, "a": "x" })),
            "{\"ok\":true,\"result\":{\"a\":\"x\",\"b\":1}}"
        );
        assert_eq!(
            not_open(),
            "{\"ok\":false,\"error\":{\"code\":\"usage\",\"kind\":\"\",\
             \"message\":\"session is not open\",\"path\":null}}"
        );
    }

    #[test]
    fn json_keys_are_sorted() {
        assert!(deterministic_json());
    }

    #[test]
    fn changes_do_not_carry_the_text() {
        let updated = ExternalChange::Updated("a.nov".to_owned(), "本文".to_owned());
        assert_eq!(
            text(&change(&updated)),
            "{\"change\":\"updated\",\"path\":\"a.nov\"}"
        );
        let removed = ExternalChange::Removed("b.nov".to_owned());
        assert_eq!(
            text(&change(&removed)),
            "{\"change\":\"removed\",\"path\":\"b.nov\"}"
        );
        let unreadable = ExternalChange::Unreadable(
            "c.nov".to_owned(),
            novnow::app::host::ReadFailure::Encoding("bad".to_owned()),
        );
        assert_eq!(
            text(&change(&unreadable)),
            "{\"change\":\"unreadable\",\"code\":\"S024\",\"message\":\"bad\",\"path\":\"c.nov\"}"
        );
    }

    #[test]
    fn panic_messages_are_kept() {
        let literal = panic::catch_unwind(|| panic!("plain")).expect_err("panics");
        assert_eq!(panic_message(literal.as_ref()), "plain");
        let formatted = panic::catch_unwind(|| panic!("with {}", 1)).expect_err("panics");
        assert_eq!(panic_message(formatted.as_ref()), "with 1");
        let other = panic::catch_unwind(|| panic::panic_any(7)).expect_err("panics");
        assert_eq!(panic_message(other.as_ref()), "");
    }
}
