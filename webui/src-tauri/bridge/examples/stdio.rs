//! コマンド 4 つを標準入出力から叩く（窓なし）。
//!
//! Tauri の窓を組めない環境（WebView の開発ライブラリが無い作業サンドボックス・CI の
//! `webui` ジョブ）でも、ネイティブの橋渡しを**同じ関数のまま**動かすための口。
//! `REF:webui/tests/l6.test.mjs` がこれを起こし、ブラウザ用の橋渡し（wasm）と同じ操作列を
//! 流して、戻り値と保存後のファイルを突き合わせる（`REF:docs/plans.md#webui-w5` の受け入れ）。
//!
//! 入力は 1 行 1 通の JSON、出力は 1 行 1 通（コマンドが返した**文字列の JSON をそのまま**）。
//!
//! ```text
//! {"cmd":"open","path":"/path/to/project","create_options":null}
//! {"cmd":"run","op":"project.info","args":"{}"}
//! {"cmd":"poll_events"}
//! {"cmd":"poll_external"}
//! ```
//!
//! `args` と `create_options` は Tauri のコマンドと同じく**文字列**で渡す。ここはファイルに
//! 何も書かない（書くのは `FsHost` だけ）。

use std::io::{BufRead, Write};

use novnow_native_bridge::NativeSession;
use serde_json::Value;

fn main() {
    let mut session = NativeSession::new();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let reply = answer(&mut session, &line);
        if writeln!(stdout, "{reply}")
            .and_then(|()| stdout.flush())
            .is_err()
        {
            break;
        }
    }
}

/// 1 通を処理する。読めない行は、コマンドの失敗と同じ形で返す（黙って捨てない）。
fn answer(session: &mut NativeSession, line: &str) -> String {
    let request: Value = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(error) => return refuse(&error.to_string()),
    };
    let field = |name: &str| request.get(name).and_then(Value::as_str);
    match field("cmd") {
        Some("open") => session.open(field("path").unwrap_or_default(), field("create_options")),
        Some("run") => session.run(
            field("op").unwrap_or_default(),
            field("args").unwrap_or_default(),
        ),
        Some("poll_events") => session.poll_events(),
        Some("poll_external") => session.poll_external(),
        other => refuse(&format!("unknown cmd {}", other.unwrap_or("(none)"))),
    }
}

/// この口自身の失敗（`run` の失敗と同じ形）。
fn refuse(message: &str) -> String {
    let error = serde_json::json!({
        "code": "usage",
        "kind": "",
        "message": message,
        "path": Value::Null,
    });
    format!("{{\"ok\":false,\"error\":{error}}}")
}
