//! `novnow export <dir> [options]`（§14.1・§17.2）。**M6 では枠だけ**（中身は M9、`PLANS.md` §5.10）。
//!
//! §14.2 の検査（不備なら 3）を先に行い、通ったら「まだ実装されていない」と出して 2 で終わる
//! （`PLANS.md` §9.2 B-105）。オプションは M9 で読む。

use std::ffi::OsString;
use std::path::Path;

use crate::ports::env::Env;

use super::{open, Cli, Exit};

/// 実行する。
pub fn run<E: Env>(cli: &mut Cli<'_, E>, dir: &Path, options: &[OsString]) -> Exit {
    let _ = options;
    if let Err(exit) = open::precheck(cli, dir) {
        return exit;
    }
    cli.not_implemented("export")
}
