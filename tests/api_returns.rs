//! 戻り値の形（§19.8.2 の `returns`）が**挙動と一致する**ことの検査。
//!
//! 1. 宣言（`REF:src/app/ops/returns.rs`）が全操作ぶんあり、余分が無く、ID の昇順であること
//! 2. 実際に叩いて返ってきたトップレベルのキーが、宣言した `properties` の中にあること
//! 3. 値の型が宣言した型（`|` は「どれか」、`any` は無検査）に合うこと
//!
//! 2 と 3 が本質で、「仕様（宣言）＝実装（挙動）」を機械で突き合わせる。叩いて `Err` に
//! なった操作（セッション状態が要るもの）は 1 の検査だけを受ける。

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Map, Value};

use novnow::adapters::project_load::is_document;
use novnow::app::memory_host::{MemoryFiles, MemoryHost};
use novnow::app::ops::{self, returns, OpDef, OPS};
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

/// 引数の見本（`REF:tests/api_coverage.rs` と同じ規則。ここでは型だけ合えばよい）。
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
        "url" => "https://example.com/a.png",
        "id" => "01-01-01-01",
        "word" => "星屑の時計",
        "cast" | "alias" => "星野 アカリ",
        "prompt" => "見本",
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

#[test]
fn every_operation_declares_its_return_shape() {
    let declared: Vec<&str> = returns::RETURNS.iter().map(|(id, _)| *id).collect();
    let listed: Vec<&str> = OPS.iter().map(|def| def.id).collect();
    let mut sorted = declared.clone();
    sorted.sort_unstable();
    assert_eq!(declared, sorted, "戻り値の表は ID の昇順であること");
    let mut missing: Vec<&str> = listed
        .iter()
        .copied()
        .filter(|id| !declared.contains(id))
        .collect();
    missing.sort_unstable();
    assert!(missing.is_empty(), "戻り値の宣言が無い操作: {missing:?}");
    let mut extra: Vec<&str> = declared
        .iter()
        .copied()
        .filter(|id| !listed.contains(id))
        .collect();
    extra.sort_unstable();
    assert!(extra.is_empty(), "カタログに無い操作の宣言: {extra:?}");
    assert_eq!(declared.len(), listed.len(), "件数が合わない");
}

#[test]
fn the_declared_shape_matches_what_the_operations_return() {
    let mut problems: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for def in OPS {
        let (mut session, _) = open();
        session.set_interface(Interface::Rust);
        let Ok(value) = session.run(def.id, sample_args(def)) else {
            continue;
        };
        let Value::Object(found) = &value else {
            problems.push(format!(
                "{}: 戻り値がオブジェクトでない（{}）",
                def.id,
                kind_of(&value)
            ));
            continue;
        };
        checked += 1;
        let declared = returns::returns_of(def.id);
        for (key, value) in found {
            let Some((_, kind)) = declared.iter().find(|(name, _)| name == key) else {
                problems.push(format!("{}: 宣言に無いキー {key}", def.id));
                continue;
            };
            if *kind == "any" {
                continue;
            }
            let observed = kind_of(value);
            let accepted: Vec<&str> = kind.split('|').collect();
            if !accepted.contains(&observed) {
                problems.push(format!("{}: {key} が {observed}（宣言は {kind}）", def.id));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "戻り値の宣言が挙動と合わない:\n{}",
        problems.join("\n")
    );
    assert!(
        checked >= 80,
        "挙動を確かめられた操作が {checked} 件しかない"
    );
}

#[test]
fn the_catalog_exposes_the_return_shape_as_json_schema() {
    let (mut session, _) = open();
    let catalog = session
        .run("api.list", Value::Object(Map::new()))
        .expect("api.list");
    let operations = catalog["operations"].as_array().expect("operations");
    for operation in operations {
        let id = operation["id"].as_str().expect("id");
        let returns = &operation["returns"];
        assert_eq!(returns["type"], "object", "{id}: 戻り値はオブジェクト");
        assert!(returns["properties"].is_object(), "{id}: properties が無い");
        let declared = returns::returns_of(id);
        let properties = returns["properties"].as_object().expect("properties");
        assert_eq!(
            properties.len(),
            declared.len(),
            "{id}: 宣言したキーの数と properties が合わない"
        );
        for (key, schema) in properties {
            assert!(
                schema.is_object(),
                "{id}: {key} のスキーマがオブジェクトでない"
            );
        }
    }
    assert_eq!(
        ops::catalog(novnow::ui::i18n::Locale::Ja)["count"],
        Value::from(operations.len() as u64)
    );
}
