//! `novnow mcp <dir>`（§14.1・§15.8.3）。**M6 では枠だけ**（中身は M10、`PLANS.md` §5.11）。
//!
//! §14.2 の検査（不備なら 3）を先に行い、通ったら「まだ実装されていない」と出して 2 で終わる
//! （`PLANS.md` §9.2 B-105）。

use std::path::Path;

use crate::ports::env::Env;

use super::{open, Cli, Exit};

/// 実行する。
pub fn run<E: Env>(cli: &mut Cli<'_, E>, dir: &Path) -> Exit {
    if let Err(exit) = open::precheck(cli, dir) {
        return exit;
    }
    cli.not_implemented("mcp")
}
