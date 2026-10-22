//! `workspace/executeCommand` → 操作カタログの全件（§19.4.5）。
//!
//! MCP のツール表（§19.5）と同じ発想で、**カタログに載る操作はどれもコマンドとして
//! 叩ける**。エディタ固有の拡張（コマンドパレット、ボタン）はここへ集約するので、
//! 口ごとの実装は増えない（§19.1.2）。

use serde_json::{json, Map, Value};

use crate::app::error::Error;
use crate::app::ops;
use crate::app::session::Session;

use super::methods::{self, Outbox};

/// パレットのコマンドを実行する特別なコマンド名。
pub const COMMAND: &str = "novnow.command";

/// 編集をクライアントに適用してもらう特別なコマンド名（§19.4.4）。
pub const APPLY_EDIT: &str = "novnow.applyEdit";

/// `executeCommandProvider` に載せる名前（操作 ID の全件 + 特別な 2 つ）。
pub fn names() -> Vec<String> {
    let mut names: Vec<String> = ops::OPS.iter().map(|op| op.id.to_owned()).collect();
    names.push(COMMAND.to_owned());
    names.push(APPLY_EDIT.to_owned());
    names.sort();
    names
}

/// 1 件のコマンドを実行する。
///
/// `arguments[0]` を操作の引数（オブジェクト）として渡す。引数無しなら空のオブジェクト。
pub fn execute(
    session: &mut Session,
    out: &mut Outbox,
    params: &Value,
    root: &str,
) -> Result<Value, Error> {
    let command = params
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let arguments = params
        .get("arguments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    match command {
        COMMAND => {
            let name = arguments
                .first()
                .and_then(Value::as_str)
                .unwrap_or_default();
            session.command(name)
        }
        APPLY_EDIT => {
            let edit = arguments.first().cloned().unwrap_or(Value::Null);
            out.request("workspace/applyEdit", edit);
            Ok(json!({ "applied": true }))
        }
        _ if ops::find(command).is_some() => {
            let args = arguments
                .first()
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new()));
            let value = methods::run(session, command, args)?;
            // 編集を返す操作は、その場でクライアントに適用を頼む（§19.4.4）。
            if value.get("changes").is_some() {
                let edit = methods::workspace_edit(session, root, &value);
                out.request("workspace/applyEdit", edit);
            }
            Ok(value)
        }
        _ => Err(Error::not_found(session.locale, command, None)),
    }
}
