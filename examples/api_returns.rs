//! 正規操作ごとの**戻り値の形**を、実際に叩いて観察する（`api.list` の `returns` の材料）。
//!
//! ```bash
//! cargo run --locked --quiet --example api_returns
//! ```
//!
//! メモリ上の雛形プロジェクト（§19.2.1）で 1 操作ずつ実行し、`Ok` を返した操作について
//! トップレベルのキーと JSON の型を 1 行で出す。`Err` を返した操作（セッション状態や GUI の
//! 文脈が要るもの）は `-` で出す。`tests/golden/api_returns.txt` と突き合わせるための材料で、
//! 宣言（§19.8.2 の `returns`）が挙動から離れていないかを人が確かめるのに使う。

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Map, Value};

use novnow::adapters::project_load::is_document;
use novnow::app::memory_host::{MemoryFiles, MemoryHost};
use novnow::app::ops::{OpDef, OPS};
use novnow::app::session::{Interface, Session};
use novnow::core::model::document::Document;
use novnow::core::policy::ProjectPath;
use novnow::core::template;

fn open() -> (Session, Rc<RefCell<MemoryFiles>>) {
    let host = MemoryHost::default();
    let files = host.files.clone();
    let documents = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text));
    (
        Session::with_documents(Box::new(host), documents.collect()),
        files,
    )
}

fn first_document() -> &'static str {
    static FOUND: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    FOUND.get_or_init(|| {
        template::new_project("星屑の時計")
            .into_iter()
            .map(|file| file.path.to_owned())
            .find(|path| is_document(&ProjectPath::new(path).expect("path")))
            .unwrap_or_else(|| "contents/01-01.cnov".to_owned())
    })
}

fn sample(name: &str, kind: &str) -> Value {
    let text = match name {
        "path" => first_document(),
        "scene" | "after" | "before" | "chapter" | "episode" | "level" => "01-01-01-01",
        "query" => "の",
        "replace" => "ほ",
        "role" | "name" => "writer",
        "title" => "星屑の時計",
        "text" => "本文の見本",
        "summary" => "要約の見本",
        "key" => "theme",
        "value" => "dark",
        "to" | "from" => "writer",
        "body" | "comment" | "intent" => "見本",
        "workspace" => "write",
        "topic" => "writing",
        "format" => "text",
        "out" => "out.txt",
        "range" | "state" => "",
        "url" => "https://example.com/a.png",
        "bytes" | "text_body" => "",
        "kind" | "scope" | "examples" | "keywords" => "",
        "id" => "01-01-01-01",
        "word" => "星屑の時計",
        "cast" | "alias" => "星野 アカリ",
        "prompt" => "見本",
        "line" | "column" | "start" | "end" | "index" | "offset" => "0",
        _ => "",
    };
    match kind {
        "integer" => text
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or(Value::from(0)),
        "number" => Value::from(0.0),
        "boolean" => Value::Bool(false),
        "array" => Value::Array(Vec::new()),
        "object" => Value::Object(Map::new()),
        _ => Value::String(text.to_owned()),
    }
}

fn sample_args(def: &OpDef) -> Value {
    let mut args = Map::new();
    for name in def.argument_order() {
        let kind = def
            .args
            .iter()
            .find(|arg| arg.name == name)
            .map(|arg| arg.kind)
            .unwrap_or("string");
        args.insert(name.to_owned(), sample(name, kind));
    }
    Value::Object(args)
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn main() {
    for def in OPS {
        let (mut session, _) = open();
        session.set_interface(Interface::Rust);
        match session.run(def.id, sample_args(def)) {
            Ok(value) => {
                let keys: Vec<String> = match &value {
                    Value::Object(map) => map
                        .iter()
                        .map(|(key, found)| format!("{key}:{}", kind_of(found)))
                        .collect(),
                    other => vec![format!(".:{}", kind_of(other))],
                };
                println!("{}\t{}", def.id, keys.join(","));
            }
            Err(_) => println!("{}\t-", def.id),
        }
    }
}
