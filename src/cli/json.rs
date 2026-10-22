//! `--json` の封筒・決定的な出力・終了コード（§19.6.2・§19.1.7）。
//!
//! 1 コマンド = 1 [`Session`]: 起動検証（[`super::open`]）→ 操作の実行 → 書き込みの
//! フラッシュ → 分類に応じた終了コード。書き込みは操作の最後にまとめて出す（§19.1.6）ので、
//! 割り込みシグナルで落ちたときはフラッシュに到達せず、部分書き込みは残らない。
//!
//! 出力の規則（§19.6.2）:
//!
//! - `--json` あり → 封筒 `{ok, op, data}` / `{ok, op, error}` を標準出力へ 1 行。
//! - `--json` なし → 人間可読な地域化テキスト（§7.4）。データそのものは地域化しない。
//! - 進捗は標準エラーへ。標準出力は結果だけ（パイプを妨げない）。

use std::io::Read;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::app::error::{Class, Error};
use crate::app::ops::{self, ArgDef};
use crate::app::session::{Interface, Session};
use crate::ports::env::Env;
use crate::ui::i18n::Locale;

use super::{Cli, Exit, Stream};

/// 成功の封筒（§19.6.2）。キーは辞書順（決定的。§19.1.4）。
pub fn envelope(op: &str, data: &Value) -> String {
    json!({ "data": data, "ok": true, "op": op }).to_string()
}

/// 失敗の封筒（§19.6.2）。`code` は分類、`kind` は S / E 番号。
pub fn failure(op: &str, error: &Error) -> String {
    json!({
        "error": {
            "code": error.class.as_str(),
            "kind": error.kind,
            "message": error.message,
            "path": error.path,
        },
        "ok": false,
        "op": op,
    })
    .to_string()
}

/// 分類 → 終了コード（§19.1.7 の表。§14.2.5 の 0・2・3・4・5 だけを使う）。
pub fn exit_of(class: &Class) -> Exit {
    match class {
        Class::Usage => Exit::Usage,
        Class::Project | Class::NotFound => Exit::Project,
        Class::Io | Class::Conflict | Class::Timeout => Exit::Io,
        Class::Validation | Class::Policy => Exit::Diagnostics,
        // 能動的なキャンセルは誤りではない（部分書き込みも残さない）。
        Class::Cancelled => Exit::Ok,
    }
}

/// `--lang` の値（§19.1.8。既定は `ja`）。
pub fn locale_of(lang: Option<&str>) -> Option<Locale> {
    match lang {
        Some("ja") => Some(Locale::Ja),
        Some("en") => Some(Locale::En),
        Some(_) | None => None,
    }
}

/// 失敗を出して終了コードを返す。
fn fail<E: Env>(cli: &mut Cli<'_, E>, op: &str, error: &Error, as_json: bool) -> Exit {
    if as_json {
        cli.line(Stream::Out, &failure(op, error));
    } else {
        cli.line(Stream::Err, &error.message);
    }
    exit_of(&error.class)
}

/// 操作を 1 つ実行する（§19.6.1 の `<ドメイン> <動詞> <dir> [オペランド] [--オプション]`）。
pub fn run<E: Env>(
    cli: &mut Cli<'_, E>,
    id: &str,
    dir: &Path,
    operand: Option<&str>,
    options: &[(String, String)],
    as_json: bool,
    lang: Option<&str>,
) -> Exit {
    // 相対パスは CLI の作業ディレクトリ基準（§14.1）。検査が返す絶対ルートを開く。
    let root = match super::open::precheck(cli, dir) {
        Ok(root) => root,
        Err(exit) => return exit,
    };
    if let Some(locale) = locale_of(lang) {
        cli.set_locale(locale);
    }
    let mut session = match Session::open_with(&root, Interface::Cli) {
        Ok(session) => session,
        Err(error) => return fail(cli, id, &error, as_json),
    };
    let args = match arguments(cli, id, operand, options) {
        Ok(args) => args,
        Err(message) => {
            let error = Error::usage(session.locale, &message);
            return fail(cli, id, &error, as_json);
        }
    };
    match session.run(id, args) {
        Ok(data) => {
            if as_json {
                cli.line(Stream::Out, &envelope(id, &data));
            } else {
                cli.write(Stream::Out, &human(&data));
            }
            Exit::Ok
        }
        Err(error) => fail(cli, id, &error, as_json),
    }
}

/// 引数を作る。`--<引数名> <値>` が優先し、オペランドは残りの最初の引数に入る。
///
/// `-` をオペランドに指定した箇所は標準入力から読む（§19.6.2）。
fn arguments<E: Env>(
    cli: &mut Cli<'_, E>,
    id: &str,
    operand: Option<&str>,
    options: &[(String, String)],
) -> Result<Value, String> {
    let Some(def) = ops::find(id) else {
        return Err(id.to_owned());
    };
    let mut args = Map::new();
    for (name, value) in options {
        let kind = def
            .args
            .iter()
            .find(|arg| arg.name == name)
            .map(|arg| arg.kind)
            .unwrap_or("string");
        args.insert(name.clone(), coerce(value, kind));
    }
    if let Some(operand) = operand {
        let text = if operand == "-" {
            stdin(cli)
        } else {
            operand.to_owned()
        };
        let target = def
            .args
            .iter()
            .find(|arg| !args.contains_key(arg.name))
            .map(|arg| (arg.name, arg.kind));
        match target {
            Some((name, kind)) => {
                args.insert(name.to_owned(), coerce(&text, kind));
            }
            None => return Err(format!("{id}: {operand}")),
        }
    }
    Ok(Value::Object(args))
}

/// 標準入力を読む（テストでは [`Cli::take_input`] の代わりを使う）。
fn stdin<E: Env>(cli: &mut Cli<'_, E>) -> String {
    let mut text = String::new();
    match cli.take_input() {
        Some(input) => {
            let _ = input.read_to_string(&mut text);
        }
        None => {
            let stdin = std::io::stdin();
            let mut lock = stdin.lock();
            let _ = lock.read_to_string(&mut text);
        }
    }
    text
}

/// 宣言された型に合わせる（読めなければ文字列のまま。操作側が usage を返す）。
fn coerce(text: &str, kind: &str) -> Value {
    match kind {
        "integer" => text
            .parse::<i64>()
            .map(|number| json!(number))
            .unwrap_or_else(|_| json!(text)),
        "number" => text
            .parse::<f64>()
            .map(|number| json!(number))
            .unwrap_or_else(|_| json!(text)),
        "boolean" => match text {
            "true" | "1" => json!(true),
            "false" | "0" => json!(false),
            _ => json!(text),
        },
        "object" | "array" => serde_json::from_str(text).unwrap_or_else(|_| json!(text)),
        _ => json!(text),
    }
}

/// 引数の宣言（CLI の `--<名前>` とオペランドの当て先）。
pub fn schema(id: &str) -> Option<&'static [ArgDef]> {
    ops::find(id).map(|def| def.args)
}

/// 人間可読な出力（§19.6.2）。キーは辞書順、入れ子は 2 空白字下げ。
pub fn human(data: &Value) -> String {
    let mut out = String::new();
    render(&mut out, data, 0);
    out
}

fn render(out: &mut String, value: &Value, depth: usize) {
    let pad = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                match value {
                    Value::Object(inner) if !inner.is_empty() => {
                        out.push_str(&format!("{pad}{key}:\n"));
                        render(out, value, depth + 1);
                    }
                    Value::Array(items) if !items.is_empty() => {
                        out.push_str(&format!("{pad}{key}:\n"));
                        render(out, value, depth + 1);
                    }
                    _ => out.push_str(&format!("{pad}{key}: {}\n", scalar(value))),
                }
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                match item {
                    Value::Object(_) | Value::Array(_) => {
                        out.push_str(&format!("{pad}{}.\n", index + 1));
                        render(out, item, depth + 1);
                    }
                    _ => out.push_str(&format!("{pad}- {}\n", scalar(item))),
                }
            }
        }
        _ => out.push_str(&format!("{pad}{}\n", scalar(value))),
    }
}

/// 1 つの値の表示（文字列はそのまま、他は JSON の形）。
fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
