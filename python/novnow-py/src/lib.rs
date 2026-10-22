//! novnow の Python API（§19.3）。**インプロセスのネイティブ拡張**で、Rust API（§19.2）と
//! 同じ機能核（`novnow::app`）を呼ぶ。サブプロセスでも JSON ソケットでもない（§19.3.1）。
//!
//! 表の API（`Session` のコンテキストマネージャ、`session.project.info()` の形のドメイン呼び出し、
//! §19.3.3 の 9 例外）は `python/novnow/__init__.py` が組む。ここが提供する `_core` は:
//!
//! - `open(path)` / `create(path)` → `Session`
//! - `Session.run(op, args)` — 正規操作カタログ（§19.7）の 1 件。**唯一の実装点**（§19.1.2）
//! - `Session.command(name)` / `poll_events()` / `on_event(callback)` / `cancel(handle)` / `close()`
//! - `OPERATIONS` — `api.list` と同じカタログ（`novnow.operations()` が返す）
//! - `NovnowError` — 全例外の親（`class` / `code` / `message` / `path` 属性。§19.1.7）
//!
//! ## GIL（§19.3.4）
//!
//! ブロックする 1 往復は**すべて `Python::allow_threads` の中で待つ**。待っているあいだに
//! ほかの Python スレッドが動く。
//!
//! `novnow::app::Session` は `Send` ではない（ホストが Web の `Rc` を前提にするので
//! `Host: Send` にできない）。だから `allow_threads` の区間に `Session` を持ち込むのではなく、
//! 機能核を **ワーカースレッド 1 本**（`REF:src/app/driver.rs`）に所有させ、この層は
//! `Send` だけの 1 通を渡して応答を待つ。`allow_threads` が閉包と戻り値に課す `Ungil`
//! （＝ `Send`）はその 1 通と応答だけで満たされる。
//!
//! 直列化（§19.3.4）: 1 つの `Session` は `Mutex<Driver>` で守り、**ロックも GIL を解放した
//! 区間の中で取る**。だから複数スレッドからの同時呼び出しはデータ競合でなく直列実行になり、
//! 待たされた側もほかの Python スレッドを止めない。

use std::cell::RefCell;
use std::sync::Mutex;

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::{Map, Value};

use novnow::app::driver::Driver;
use novnow::app::error::{Class, Error};
use novnow::app::session::{CreateOptions, Interface};
use novnow::ui::i18n::Locale;

create_exception!(
    novnow,
    NovnowError,
    PyException,
    "novnow のエラー（§19.3.3）。派生型は Python 側で §19.1.7 の分類ごとに作る。"
);

/// 開いたプロジェクト（§19.3.2 の `Session`）。
///
/// 中身はワーカースレッドへの口（`Driver`）。`Mutex` は §19.3.4 の直列化で、
/// ロックは GIL を解放した区間で取る。
#[pyclass]
struct Session {
    core: Mutex<Driver>,
    /// `on_event` で登録された Python のコールバック（§19.3.5）。GIL を持つときだけ触る。
    callbacks: RefCell<Vec<PyObject>>,
    /// `poll_events()` がまだ返していないイベント（`dict`）。
    pending: RefCell<Vec<PyObject>>,
}

impl Session {
    fn new(driver: Driver) -> Self {
        Self {
            core: Mutex::new(driver),
            callbacks: RefCell::new(Vec::new()),
            pending: RefCell::new(Vec::new()),
        }
    }

    /// 溜まったイベントを取り出し、コールバックへ渡してから返す（§19.2.4・§19.3.5）。
    ///
    /// イベントは `Driver` の 1 本を通って待った側のスレッドへ来ているので、ここで
    /// Python のオブジェクト（`dict`）に写す。コールバックは `Session::push` と同じく
    /// 起きたときに 1 回だけ呼び、`poll_events()` はそれとは別に**同じ列**を返す
    /// （`REF:src/app/session.rs` の `push` / `poll_events` と同じ関係）。
    fn take_events(&self, py: Python<'_>) -> PyResult<Vec<PyObject>> {
        let core = &self.core;
        let drained = py.allow_threads(move || -> PyResult<Result<Vec<Value>, Error>> {
            let Ok(mut driver) = core.lock() else {
                return Err(poisoned());
            };
            Ok(driver
                .poll_events()
                .map(|events| events.iter().map(|event| event.to_json()).collect()))
        })?;
        let values = drained.map_err(into_error)?;
        if !values.is_empty() {
            // JSON → Python の `dict`（§19.3.2）。GIL を持っているのでここで写す
            let dicts = values
                .iter()
                .map(|value| to_python(py, value))
                .collect::<PyResult<Vec<_>>>()?;
            // Python 側から `on_event` を呼ばれても壊れないように、写しを取ってから渡す
            let callbacks = self.callbacks.borrow().clone();
            for dict in &dicts {
                for callback in &callbacks {
                    let _ = callback.call1(py, (dict,));
                }
            }
            self.pending.borrow_mut().extend(dicts);
        }
        Ok(std::mem::take(&mut *self.pending.borrow_mut()))
    }

    /// イベントを流すだけ（操作のあと）。`poll_events()` が列を返す。
    fn deliver(&self, py: Python<'_>) -> PyResult<()> {
        self.take_events(py)?;
        Ok(())
    }
}

/// `novnow.open(path)`（§19.3.2）。
#[pyfunction]
fn open(py: Python<'_>, path: &str) -> PyResult<Session> {
    let root = std::path::PathBuf::from(path);
    let opened = py.allow_threads(move || Driver::open_with(&root, Interface::Python))?;
    Ok(Session::new(opened))
}

/// `novnow.create(path, title=None)`（`project.new`。§19.3.2）。
#[pyfunction]
fn create(py: Python<'_>, path: &str, title: Option<&str>) -> PyResult<Session> {
    let root = std::path::PathBuf::from(path);
    let options = CreateOptions {
        author: String::new(),
        language: "ja".to_owned(),
        title: title.unwrap_or_default().to_owned(),
    };
    let created = py.allow_threads(move || Driver::create(&root, options))?;
    Ok(Session::new(created))
}

#[pymethods]
impl Session {
    /// 正規操作を 1 つ実行する（§19.7）。`args` は JSON と同じ構造の `dict`。
    fn run(&self, py: Python<'_>, op: &str, args: Option<Bound<'_, PyDict>>) -> PyResult<PyObject> {
        let args = match args {
            Some(dict) => from_python(&dict)?,
            None => Value::Object(Map::new()),
        };
        let op = op.to_owned();
        let core = &self.core;
        // GIL を解放して待つ（§19.3.4）。渡す 1 通も応答も Python に触れない
        let outcome = py.allow_threads(move || -> PyResult<Result<Value, Error>> {
            let Ok(mut driver) = core.lock() else {
                return Err(poisoned());
            };
            Ok(driver.run(&op, args))
        })?;
        self.deliver(py)?;
        outcome
            .map(|value| to_python(py, &value))
            .map_err(into_error)
    }

    /// パレットのコマンドを実行する（§19.2.3）。
    fn command(&self, py: Python<'_>, name: &str) -> PyResult<PyObject> {
        let name = name.to_owned();
        let core = &self.core;
        let outcome = py.allow_threads(move || -> PyResult<Result<Value, Error>> {
            let Ok(mut driver) = core.lock() else {
                return Err(poisoned());
            };
            Ok(driver.command(&name))
        })?;
        self.deliver(py)?;
        outcome
            .map(|value| to_python(py, &value))
            .map_err(into_error)
    }

    /// イベントキューを取り出す（§19.2.4・§19.3.5）。登録済みのコールバックにも渡す。
    fn poll_events(&self, py: Python<'_>) -> PyResult<PyObject> {
        let values = self.take_events(py)?;
        let list = PyList::empty_bound(py);
        for value in values {
            list.append(value)?;
        }
        Ok(list.into_py(py))
    }

    /// イベントのコールバックを登録する（§19.3.5）。
    fn on_event(&self, callback: PyObject) -> PyResult<()> {
        self.callbacks.borrow_mut().push(callback);
        Ok(())
    }

    /// 長時間の操作を中断する（§19.1.9・§19.3.5）。
    fn cancel(&self, py: Python<'_>, handle: &str) -> PyResult<PyObject> {
        let handle = handle.to_owned();
        let core = &self.core;
        let outcome = py.allow_threads(move || -> PyResult<Result<Value, Error>> {
            let Ok(mut driver) = core.lock() else {
                return Err(poisoned());
            };
            Ok(driver.cancel(&handle))
        })?;
        self.deliver(py)?;
        outcome
            .map(|value| to_python(py, &value))
            .map_err(into_error)
    }

    /// 文言の言語（§19.1.8）。`ja` / `en`。
    fn set_locale(&self, py: Python<'_>, locale: &str) -> PyResult<()> {
        let locale = match locale {
            "en" => Locale::En,
            _ => Locale::Ja,
        };
        let core = &self.core;
        let outcome = py.allow_threads(move || -> PyResult<Result<(), Error>> {
            let Ok(mut driver) = core.lock() else {
                return Err(poisoned());
            };
            Ok(driver.set_locale(locale))
        })?;
        outcome.map_err(into_error)
    }

    /// プロジェクトルート。
    fn root(&self, py: Python<'_>) -> PyResult<Option<String>> {
        let core = &self.core;
        let root = py.allow_threads(
            move || -> PyResult<Result<Option<std::path::PathBuf>, Error>> {
                let Ok(mut driver) = core.lock() else {
                    return Err(poisoned());
                };
                Ok(driver.root())
            },
        )?;
        Ok(root
            .map_err(into_error)?
            .map(|path| path.to_string_lossy().into_owned()))
    }

    /// 閉じる（§19.3.2「未保存変更の破棄は §19.2.5 と同じ」）。保存はしない。
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        let core = &self.core;
        py.allow_threads(move || -> PyResult<()> {
            let Ok(mut driver) = core.lock() else {
                return Err(poisoned());
            };
            driver.shutdown();
            Ok(())
        })
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _kind: Option<PyObject>,
        _value: Option<PyObject>,
        _traceback: Option<PyObject>,
    ) -> PyResult<bool> {
        self.close(py)?;
        Ok(false)
    }
}

/// `Mutex` が汚染された（ワーカー側でパニックがあった）。novnow の分類エラーではない。
fn poisoned() -> PyErr {
    PyException::new_err("the novnow session thread panicked")
}

/// 分類・S/E 番号・地域化メッセージ・パスを属性に持たせる（§19.3.3）。
fn into_error(error: Error) -> PyErr {
    let failure = NovnowError::new_err((error.message.clone(),));
    Python::with_gil(|py| {
        let instance = failure.value_bound(py);
        let _ = instance.setattr("class", error.class.as_str());
        let _ = instance.setattr("code", error.kind);
        let _ = instance.setattr("message", error.message);
        let _ = instance.setattr("path", error.path);
        let _ = instance.setattr("exit_code", exit_code(&error.class));
    });
    failure
}

/// §19.1.7 の CLI 終了コード（Python 側でも同じ番号を見せる）。
fn exit_code(class: &Class) -> u8 {
    match class {
        Class::Usage => 2,
        Class::Project | Class::NotFound => 3,
        Class::Io | Class::Conflict | Class::Timeout => 4,
        Class::Validation | Class::Policy => 5,
        Class::Cancelled => 0,
    }
}

/// JSON → Python の組み込み型（§19.3.2「dict・list・str・int・bool・None」）。
fn to_python(py: Python<'_>, value: &Value) -> PyResult<PyObject> {
    Ok(match value {
        Value::Null => py.None(),
        Value::Bool(flag) => (*flag).into_py(py),
        Value::Number(number) => {
            if let Some(signed) = number.as_i64() {
                signed.into_py(py)
            } else if let Some(unsigned) = number.as_u64() {
                unsigned.into_py(py)
            } else {
                number.as_f64().unwrap_or(0.0).into_py(py)
            }
        }
        Value::String(text) => text.clone().into_py(py),
        Value::Array(items) => {
            let list = PyList::empty_bound(py);
            for item in items {
                list.append(to_python(py, item)?)?;
            }
            list.into_py(py)
        }
        Value::Object(map) => {
            let dict = PyDict::new_bound(py);
            for (key, item) in map {
                dict.set_item(key, to_python(py, item)?)?;
            }
            dict.into_py(py)
        }
    })
}

/// Python の組み込み型 → JSON（§19.3.2）。
fn from_python(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    if value.is_none() {
        return Ok(Value::Null);
    }
    if let Ok(flag) = value.extract::<bool>() {
        return Ok(Value::Bool(flag));
    }
    if let Ok(signed) = value.extract::<i64>() {
        return Ok(Value::from(signed));
    }
    if let Ok(unsigned) = value.extract::<u64>() {
        return Ok(Value::from(unsigned));
    }
    if let Ok(float) = value.extract::<f64>() {
        return Ok(serde_json::Number::from_f64(float)
            .map(Value::Number)
            .unwrap_or(Value::Null));
    }
    if let Ok(text) = value.extract::<String>() {
        return Ok(Value::String(text));
    }
    if let Ok(list) = value.downcast::<PyList>() {
        let mut items = Vec::new();
        for item in list.iter() {
            items.push(from_python(&item)?);
        }
        return Ok(Value::Array(items));
    }
    if let Ok(dict) = value.downcast::<PyDict>() {
        let mut map = Map::new();
        for (key, item) in dict.iter() {
            let key = key.extract::<String>()?;
            map.insert(key, from_python(&item)?);
        }
        return Ok(Value::Object(map));
    }
    let name = value
        .get_type()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|_| "?".to_owned());
    Err(PyException::new_err(format!(
        "Python の型を JSON に写せません: {name}"
    )))
}

/// `novnow` モジュール（`novnow._core`）。
#[pymodule]
fn _core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("NovnowError", module.py().get_type_bound::<NovnowError>())?;
    module.add_class::<Session>()?;
    module.add_function(&wrap_pyfunction!(open, module)?)?;
    module.add_function(&wrap_pyfunction!(create, module)?)?;
    module.add(
        "OPERATIONS",
        to_python(
            module.py(),
            &novnow::app::ops::catalog(novnow::ui::i18n::Locale::Ja),
        )?,
    )?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
