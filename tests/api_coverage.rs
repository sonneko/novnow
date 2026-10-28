//! 正規操作カタログ（§19.7 の 120 操作）**全件**を 1 回ずつ叩く網羅テスト。
//!
//! `REF:tests/api.rs` は操作を 1 つずつ手で書くので、カタログへ操作を足しても誰も呼ばない
//! まま CI が緑になる。ここでは app 層の `OPS` 表（`REF:src/app/ops/mod.rs`）を走査して
//! 全件を呼び、次の 3 つを見る:
//!
//! 1. **落ちないこと** — ハンドラが panic しない（`catch_unwind` で 1 件ずつ隔離する）
//! 2. **分類済みであること** — `Err` なら §19.1.7 の 9 分類のいずれかと診断コードを持つ
//! 3. **到達率** — `Ok` を返す操作の数を固定し、退行（呼び出しが壊れて全部エラーになる等）
//!    を数で捕まえる
//!
//! 結果は表として `target/api-coverage.txt` へ書き、標準出力にも出す（可視化）。
//! `cargo test --locked --test api_coverage -- --nocapture` で一覧できる。
//!
//! メモリホストだけを使い、ファイルシステムに触れない（§19.2.1）。

use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use serde_json::{Map, Value};

use novnow::adapters::project_load::is_document;
use novnow::app::error::Class;
use novnow::app::memory_host::{MemoryFiles, MemoryHost};
use novnow::app::ops::{self, OpDef, OPS};
use novnow::app::session::{Interface, Session};
use novnow::core::model::document::Document;
use novnow::core::policy::ProjectPath;
use novnow::core::template;

/// メモリ上の仮想プロジェクト（`novnow new` のサンプル。§19.2.1）。
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

/// 1 回の呼び出しの結果。
enum Outcome {
    /// `Ok`（操作が実行できた）。
    Done,
    /// 分類済みの `Err`（分類・診断コード・メッセージ）。
    Refused(Class, String, String),
    /// ハンドラが panic した（不合格）。
    Panicked(String),
}

impl Outcome {
    /// 表に出す 1 文字。
    fn mark(&self) -> &'static str {
        match self {
            Outcome::Done => "o",
            Outcome::Refused(..) => "-",
            Outcome::Panicked(..) => "X",
        }
    }
}

/// 雛形プロジェクトの最初の文書パス（`path` 引数の見本に使う。§19.2.1）。
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

/// 引数の見本値。名前ごとに「雛形プロジェクトで意味を持つ値」を返す（無ければ型ごとの既定）。
fn sample(name: &str, kind: &str) -> Value {
    if name == "path" && kind == "string" {
        return Value::String(first_document().to_owned());
    }
    let text = match name {
        "path" => first_document(),
        "scene" | "after" | "before" | "chapter" | "episode" | "level" => "01-01-01-01",
        "query" => "の",
        "replace" => "ほ",
        "role" => "writer",
        "name" => "writer",
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

/// 操作の宣言（`arguments.order`）から引数の見本を作る。関門の `role` も入れる。
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

/// 1 つの操作を新しいセッションで 1 回叩く。
fn exercise(def: &OpDef) -> Outcome {
    let id = def.id;
    let args = sample_args(def);
    let ran = catch_unwind(AssertUnwindSafe(|| {
        let (mut session, _) = open();
        session.set_interface(Interface::Rust);
        session.run(id, args)
    }));
    match ran {
        Ok(Ok(_)) => Outcome::Done,
        Ok(Err(error)) => Outcome::Refused(error.class, error.kind.clone(), error.message.clone()),
        Err(panic) => {
            let message = panic
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic（メッセージなし）".to_owned());
            Outcome::Panicked(message)
        }
    }
}

/// §19.1.7 の 9 分類（エラーの写像先が表のどれかであることの検査に使う）。
const CLASSES: [Class; 9] = [
    Class::Usage,
    Class::Validation,
    Class::NotFound,
    Class::Conflict,
    Class::Io,
    Class::Policy,
    Class::Project,
    Class::Timeout,
    Class::Cancelled,
];

/// 表（可視化）を組み立てる。ドメインごとに 1 行、操作ごとに 1 文字。
fn render(results: &[(&'static OpDef, &Outcome)]) -> String {
    let mut out = String::new();
    out.push_str("# 正規操作カタログのカバレッジ（tests/api_coverage.rs）\n");
    out.push_str("# o = 実行できた / - = 分類済みのエラー / X = panic（不合格）\n\n");
    let mut domain = "";
    let mut done = 0usize;
    let mut refused = 0usize;
    let mut panicked = 0usize;
    for (def, outcome) in results {
        if def.domain() != domain {
            if !domain.is_empty() {
                out.push('\n');
            }
            domain = def.domain();
            out.push_str(&format!("{:<12}", domain));
        }
        out.push_str(outcome.mark());
        match outcome {
            Outcome::Done => done += 1,
            Outcome::Refused(..) => refused += 1,
            Outcome::Panicked(..) => panicked += 1,
        }
    }
    out.push_str(&format!(
        "\n\n{done} 実行 / {refused} 分類済みエラー / {panicked} panic（全 {} 操作）\n",
        results.len()
    ));
    out
}

/// 全 120 操作を叩き、表を書いて、合格条件を検査する。
#[test]
fn every_catalog_operation_is_callable_and_classified() {
    let mut results: Vec<(&'static OpDef, Outcome)> = Vec::with_capacity(OPS.len());
    for def in OPS {
        let outcome = exercise(def);
        results.push((def, outcome));
    }

    // 可視化: 表を標準出力と target/api-coverage.txt へ
    let borrowed: Vec<(&'static OpDef, &Outcome)> = results
        .iter()
        .map(|(def, outcome)| (*def, outcome))
        .collect();
    let table = render(&borrowed);
    print!("{table}");
    let path = std::env::var("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("target"))
        .join(if cfg!(debug_assertions) {
            "debug/api-coverage.txt"
        } else {
            "release/api-coverage.txt"
        });
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, &table).expect("api-coverage.txt が書けない");
    eprintln!("表は {} へ書いた", path.display());

    // 1. panic が 1 件でもあれば不合格（全件を一度に報告する）
    let panics: Vec<String> = results
        .iter()
        .filter_map(|(def, outcome)| match outcome {
            Outcome::Panicked(message) => Some(format!("{}: {message}", def.id)),
            _ => None,
        })
        .collect();
    assert!(
        panics.is_empty(),
        "{} 件の操作が panic した:\n{}\n\n{table}",
        panics.len(),
        panics.join("\n")
    );

    // 2. エラーの形: 分類は §19.1.7 の 9 種類のいずれかで（`Class` が 10 種目に増えたら
    //    仕様書の表も要る）、メッセージは必ずあり（§12.3 の地域化）、`kind` を持つときは
    //    §7.1 の診断コード（`E` / `W` / `I` + 3 桁）の形であること。
    assert_eq!(CLASSES.len(), 9, "§19.1.7 のエラー分類は 9 種類");
    let mut malformed: Vec<String> = Vec::new();
    for (def, outcome) in &results {
        let Outcome::Refused(class, kind, message) = outcome else {
            continue;
        };
        if !CLASSES.contains(class) {
            malformed.push(format!("{}: 分類 {class:?} が §19.1.7 に無い", def.id));
        }
        if message.trim().is_empty() {
            malformed.push(format!(
                "{}: メッセージが空（§12.3 の地域化が無い）",
                def.id
            ));
        }
        if !kind.is_empty() {
            let shaped = kind.len() == 4
                && matches!(kind.as_bytes()[0], b'E' | b'W' | b'I')
                && kind[1..].chars().all(|digit| digit.is_ascii_digit());
            if !shaped {
                malformed.push(format!("{}: 診断コードの形でない {kind:?}", def.id));
            }
        }
    }
    assert!(
        malformed.is_empty(),
        "エラーの形が §19.1.7 / §7.1 に合わない:\n{}\n\n{table}",
        malformed.join("\n")
    );

    // 3. 到達率。見本の引数で実行できる操作の数を固定する（減ったら退行）
    let done = results
        .iter()
        .filter(|(_, outcome)| matches!(outcome, Outcome::Done))
        .count();
    assert_eq!(results.len(), 120, "カタログの全操作数");
    assert!(
        done >= 86,
        "実行できた操作が {done} 件まで減った（期待 86 以上）:\n{table}"
    );

    // 4. カタログ（§19.8.2）と `OPS` 表は同じ 120 件（二重管理が無いことの確認）
    let catalog = ops::catalog(novnow::ui::i18n::Locale::Ja);
    assert_eq!(
        catalog["count"].as_u64(),
        Some(results.len() as u64),
        "`api.list` の件数と OPS 表が違う"
    );
    // 見本の引数は宣言順（`arguments.order`）だけで作る＝宣言に無い引数を読んでいない
    for (def, _) in &results {
        for name in def.argument_order() {
            assert!(
                def.args.iter().any(|arg| arg.name == name) || name == "role",
                "{}: argument_order の {name} が引数宣言に無い",
                def.id
            );
        }
    }
}

/// 可視化の後片付け: 表に全ドメインが出る（＝カタログの全域を走査した）こと。
#[test]
fn the_coverage_table_lists_every_domain_of_the_catalog() {
    let mut results: Vec<(&'static OpDef, Outcome)> = Vec::with_capacity(OPS.len());
    for def in OPS {
        results.push((def, exercise(def)));
    }
    let borrowed: Vec<(&'static OpDef, &Outcome)> = results
        .iter()
        .map(|(def, outcome)| (*def, outcome))
        .collect();
    let table = render(&borrowed);
    for domain in ops::domains() {
        assert!(
            table.contains(domain),
            "表にドメイン {domain} が出ていない（走査漏れ）"
        );
    }
    assert_eq!(
        ops::domains().len(),
        25,
        "§19.7 のドメイン数（25）が変わったなら表の見直しが必要"
    );
    // 各行は「ドメイン名 + 操作 1 文字ずつ」なので、o/-/X の合計が操作数になる
    let marks = table
        .chars()
        .filter(|character| matches!(character, 'o' | '-' | 'X'))
        .count();
    assert!(
        marks >= OPS.len(),
        "表の印（{marks}）が操作数（{}）より少ない",
        OPS.len()
    );
}
