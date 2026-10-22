//! `novnow lsp [<dir>]`（§14.1・§19.4）: LSP サーバとして起動し、エディタにプロジェクトの
//! 言語機能と操作カタログを公開する。
//!
//! - §14.2 の検査（不備なら 3）を先に行う。通ったら標準入出力で `Content-Length` 枠の
//!   JSON-RPC 2.0 を読み書きする（[`crate::lsp::framing`]）。標準入力が閉じたら 0 で終わる。
//! - 実体は [`crate::lsp::server`]（§19.4.2 の標準メソッド + §19.4.5 の
//!   `workspace/executeCommand`）。ここは入口だけ。
//! - 標準出力はプロトコルだけに使い、それ以外は stderr に出す（`novnow mcp` と同じ、B-149）。

use std::io;
use std::path::Path;

use crate::app::error::Class;
use crate::app::session::{Interface, Session};
use crate::lsp;
use crate::ports::env::Env;

use super::open;
use super::{Cli, Exit, Stream};

/// 枠の書き出しを CLI の標準出力に繫ぐ（§19.4.1）。
impl<E: Env> lsp::server::Sink for Cli<'_, E> {
    fn frame(&mut self, body: &str) {
        self.write(Stream::Out, &lsp::framing::encode(body));
    }
}

/// 実行する。`input` が無ければ標準入力を読む。
pub fn run<E: Env>(cli: &mut Cli<'_, E>, dir: &Path) -> Exit {
    if let Err(exit) = open::precheck(cli, dir) {
        return exit;
    }
    let root = dir.to_string_lossy().into_owned();
    let mut session = match Session::open_with(dir, Interface::Lsp) {
        Ok(session) => session,
        Err(error) => {
            cli.line(Stream::Err, &error.message);
            return exit_of(&error.class);
        }
    };
    match cli.take_input() {
        Some(input) => lsp::server::run(&mut session, &root, input, cli),
        None => {
            let stdin = io::stdin();
            let mut lock = stdin.lock();
            lsp::server::run(&mut session, &root, &mut lock, cli)
        }
    }
    Exit::Ok
}

/// 開けなかった理由を終了コードにする（§14.2.5）。
fn exit_of(class: &Class) -> Exit {
    match class {
        Class::Io => Exit::Io,
        Class::Usage => Exit::Usage,
        _ => Exit::Project,
    }
}
