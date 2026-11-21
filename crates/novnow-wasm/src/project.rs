//! プロジェクトの読み込みと、画面とのあいだで行き来する JSON（設計 §21.3）。
//!
//! # 読み込みは第 1 と同じ手順
//!
//! 「一覧 → §4.3 の判定 → 文書の読み込み」は `REF:src/adapters/project_load.rs` が 1 箇所に
//! 持っている。第 2 も**同じ関数**を通す（写しを作らない）。違いは後ろに居るストレージだけ
//! （第 1 は IndexedDB のレコード、第 2 はメモリの正本）。
//!
//! # 待たない理由
//!
//! `project_load::load` は非同期だが、メモリの正本の `Future` は**その場で完了する**
//! （`poll` が 1 回で `Ready`）。このモジュールの [`load`] は 1 回だけ `poll` し、
//! `Pending` が返ったら「このストアは非同期すぎる」とみなしてエラーにする（黙って
//! 別の結果を返さない。§21.1.2 の「未提供は未提供と表示する」）。

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use serde_json::Value;

use novnow::adapters::project_load;
use novnow::app::settings::UiSettings;
use novnow::core::features::Project;
use novnow::ports::storage::Storage;

use crate::storage::describe;

/// `open_project_files` と `open_in_memory` の入力（`{"files": [{"path": ..., "text": ...}]}`）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ProjectFiles {
    /// パス（プロジェクトルートからの相対）と中身の並び。
    pub files: Vec<(String, String)>,
}

impl ProjectFiles {
    /// JSON を読む。`{"files": [{"path": "a", "text": "b"}]}` と
    /// `{"a": "b"}`（辞書形）の両方を受ける。
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let entries = value
            .get("files")
            .and_then(Value::as_array)
            .or_else(|| value.as_array());
        if let Some(entries) = entries {
            let mut files = Vec::with_capacity(entries.len());
            for entry in entries {
                let path = entry
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "every file needs a path".to_owned())?;
                let text = entry
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("{path} needs text"))?;
                files.push((path.to_owned(), text.to_owned()));
            }
            return Ok(Self { files });
        }
        match value.as_object() {
            Some(map) => Ok(Self {
                files: map
                    .iter()
                    .filter_map(|(path, text)| Some((path.clone(), text.as_str()?.to_owned())))
                    .collect(),
            }),
            None => Err("expected {\"files\": [...]}".to_owned()),
        }
    }
}

/// `run` の引数（`null` / 空文字は空のオブジェクト）。
pub fn parse_args(text: &str) -> Result<Value, String> {
    let text = text.trim();
    if text.is_empty() || text == "null" {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    match value {
        Value::Object(map) => Ok(Value::Object(map)),
        Value::Null => Ok(Value::Object(serde_json::Map::new())),
        _ => Err("arguments must be a JSON object".to_owned()),
    }
}

/// 読み込んだプロジェクト（`Session` を作るのに要るもの）。
pub struct Loaded {
    /// 文書を入れた集約（§3.4）。
    pub project: Project,
    /// `.novnow/ui.json`（無ければ既定値。§4.4.1「壊れていても既定値で動く」）。
    pub settings: UiSettings,
    /// 表示用の名前（ヘッダと `project.info`）。
    pub name: String,
    /// プロジェクトの ID（IndexedDB のキー）。
    pub id: String,
}

/// メモリの正本を第 1 と同じ手順で読む（[`load`] の入口）。
pub fn load(store: &impl Storage, name: &str, id: &str) -> Result<Loaded, String> {
    let loaded = poll_once(project_load::load(store))?.map_err(|error| describe(&error))?;
    let settings = loaded
        .project
        .text(novnow::app::settings::UI_JSON)
        .trim()
        .to_owned();
    let settings = if settings.is_empty() {
        UiSettings::default()
    } else {
        UiSettings::parse(&settings)
    };
    if let Err(problem) = &loaded.validation {
        // 起動は止めない（§13.5 の取り込みと同じ判断）。ただし黙って開かない:
        // 画面は `project.validate` で同じ判定をいつでも見られる。
        web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(&format!(
            "novnow-webui: project {} is incomplete: {problem:?}",
            name
        )));
    }
    Ok(Loaded {
        project: loaded.project,
        settings,
        name: name.to_owned(),
        id: id.to_owned(),
    })
}

impl Loaded {
    /// 「パス → 中身」の並びから組む（**判定を通さない**。メモリだけの確認用）。
    pub fn from_documents(
        files: &[(String, String)],
        name: &str,
        id: &str,
    ) -> Result<Self, String> {
        let files = ProjectFiles {
            files: files.to_vec(),
        };
        Ok(from_files(&files, name).with_id(id))
    }

    /// ID を差し替える（メモリだけのときの表示用）。
    fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_owned();
        self
    }
}

/// ファイルの並びからプロジェクトを組む（**判定を通さない**。メモリの確認用）。
pub fn from_files(files: &ProjectFiles, name: &str) -> Loaded {
    let documents = files
        .files
        .iter()
        .filter(|(path, _)| {
            novnow::core::model::document::DocumentFormat::of_path(path)
                != novnow::core::model::document::DocumentFormat::Unknown
        })
        .map(|(path, text)| novnow::core::model::document::Document::new(path, text))
        .collect::<Vec<_>>();
    let project = Project::with_documents(Default::default(), documents);
    let settings = UiSettings::default();
    Loaded {
        project,
        settings,
        name: name.to_owned(),
        id: "memory".to_owned(),
    }
}

/// [`project_load::load`] の結果を、その場で完了する `Future` から取り出す。
///
/// メモリの正本は `poll` が 1 回で `Ready` を返す。`Pending` が返るのは「待つ相手が居る」
/// ということなので、この口では扱わない（呼び出し側に理由を返す）。
fn poll_once<T, F: Future<Output = T>>(future: F) -> Result<T, String> {
    let mut future: Pin<Box<F>> = Box::pin(future);
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => Ok(value),
        Poll::Pending => Err("the store does not answer synchronously".to_owned()),
    }
}

/// 何もしない `Waker`（`std` に `Waker::noop` が無い版でも動くように自前で作る）。
fn noop_waker() -> std::task::Waker {
    use std::task::{RawWaker, RawWakerVTable, Waker};

    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    // SAFETY: the vtable's functions ignore the data pointer and do nothing.
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}
