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
//! 直列化（§19.3.4）: 1 つの `Session` は `Mutex` で守り、複数スレッドからの同時呼び出しは
//! データ競合でなく直列実行になる。

use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::{Map, Value};

use novnow::app::error::{Class, Error};
use novnow::app::session::{Interface, Session as Core};

create_exception!(
    novnow,
    NovnowError,
    PyException,
    "novnow のエラー（§19.3.3）。派生型は Python 側で §19.1.7 の分類ごとに作る。"
);

/// 直列化された機能核（§19.3.4）。
struct Guarded(Mutex<Core>);

impl Guarded {
    fn lock(&self) -> PyResult<MutexGuard<'_, Core>> {
        self.0
            .lock()
            .map_err(|error: PoisonError<_>| PyException::new_err(error.to_string()))
    }
}

/// 開いたプロジェクト（§19.3.2 の `Session`）。
#[pyclass]
struct Session {
    inner: Guarded,
}

/// `novnow.open(path)`（§19.3.2）。
#[pyfunction]
fn open(path: &str) -> PyResult<Session> {
    let core = Core::open_with(std::path::Path::new(path), Interface::Python)
        .map_err(|error| into_error(error))?;
    Ok(Session {
        inner: Guarded(Mutex::new(core)),
    })
}

/// `novnow.create(path, title=None)`（`project.new`。§19.3.2）。
#[pyfunction]
fn create(path: &str, title: Option<&str>) -> PyResult<Session> {
    let options = novnow::app::session::CreateOptions {
        author: String::new(),
        language: "ja".to_owned(),
        title: title.unwrap_or_default().to_owned(),
    };
    let core = Core::create(std::path::Path::new(path), options).map_err(into_error)?;
    let root = core
        .root()
        .map(|root| root.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(path));
    drop(core);
    open(&root.to_string_lossy())
}

#[pymethods]
impl Session {
    /// 正規操作を 1 つ実行する（§19.7）。`args` は JSON と同じ構造の `dict`。
    fn run(&self, py: Python<'_>, op: &str, args: Option<Bound<'_, PyDict>>) -> PyResult<PyObject> {
        let args = match args {
            Some(dict) => from_python(&dict)?,
            None => Value::Object(Map::new()),
        };
        let session = self.inner.lock()?;
        let result = session.run(op, args);
        drop(session);
        let _ = py;
        match result {
            Ok(value) => to_python(py, &value),
            Err(error) => Err(into_error(error)),
        }
    }

    /// パレットのコマンドを実行する（§19.2.3）。
    fn command(&self, py: Python<'_>, name: &str) -> PyResult<PyObject> {
        let session = self.inner.lock()?;
        let result = session.command(name);
        drop(session);
        match result {
            Ok(value) => to_python(py, &value),
            Err(error) => Err(into_error(error)),
        }
    }

    /// イベントキューを取り出す（§19.2.4・§19.3.5）。
    fn poll_events(&self, py: Python<'_>) -> PyResult<PyObject> {
        let mut session = self.inner.lock()?;
        let events: Vec<Value> = session
            .poll_events()
            .into_iter()
            .map(|event| event.to_json())
            .collect();
        drop(session);
        let value = serde_json::to_value(events).unwrap_or(Value::Array(Vec::new()));
        to_python(py, &value)
    }

    /// イベントのコールバックを登録する（§19.3.5）。
    fn on_event(&self, callback: PyObject) -> PyResult<()> {
        let mut session = self.inner.lock()?;
        session.on_event(move |event| {
            Python::with_gil(|py| {
                let value = to_python(py, &event.to_json()).ok();
                if let Some(value) = value {
                    let _ = callback.call1(py, (value,));
                }
            });
        });
        Ok(())
    }

    /// 長時間の操作を中断する（§19.1.9・§19.3.5）。
    fn cancel(&self, py: Python<'_>, handle: &str) -> PyResult<PyObject> {
        let mut session = self.inner.lock()?;
        let result = session.cancel(handle);
        drop(session);
        match result {
            Ok(value) => to_python(py, &value),
            Err(error) => Err(into_error(error)),
        }
    }

    /// 文言の言語（§19.1.8）。`ja` / `en`。
    fn set_locale(&self, locale: &str) -> PyResult<()> {
        let locale = match locale {
            "en" => novnow::ui::i18n::Locale::En,
            _ => novnow::ui::i18n::Locale::Ja,
        };
        let mut session = self.inner.lock()?;
        session.set_locale(locale);
        Ok(())
    }

    /// プロジェクトルート。
    fn root(&self) -> PyResult<Option<String>> {
        let session = self.inner.lock()?;
        Ok(session
            .root()
            .map(|path| path.to_string_lossy().into_owned()))
    }

    /// 閉じる（§19.3.2。未保存変更の破棄は §19.2.5 と同じ）。
    fn close(&self) -> PyResult<()> {
        let mut session = self.inner.lock()?;
        session.flush();
        Ok(())
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        _kind: Option<PyObject>,
        _value: Option<PyObject>,
        _traceback: Option<PyObject>,
    ) -> PyResult<bool> {
        self.close()?;
        Ok(false)
    }
}

/// 分類・S/E 番号・地域化メッセージ・パスを属性に持たせる（§19.3.3）。
fn into_error(error: Error) -> PyErr {
    let failure = NovnowError::new_err((error.message.clone(),));
    Python::with_gil(|py| {
        let instance = failure.value(py);
        let _ = instance.set_attr("class", error.class.as_str());
        let _ = instance.set_attr("code", error.kind);
        let _ = instance.set_attr("message", error.message);
        let _ = instance.set_attr("path", error.path);
        let _ = instance.set_attr("exit_code", exit_code(&error.class));
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
    Err(PyException::new_err(format!(
        "Python の型を JSON に写せません: {}",
        value.get_type().name().unwrap_or("?")
    )))
}

/// `novnow` モジュール（`novnow._core`）。
#[pymodule]
fn _core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("NovnowError", module.py().get_type_bound::<NovnowError>())?;
    module.add_class::<Session>()?;
    module.add_function(wrap_pyfunction!(open, module)?)?;
    module.add_function(wrap_pyfunction!(create, module)?)?;
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
