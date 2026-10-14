//! `novnow migrate <dir>`（§14.1・§20）。**M6 では枠だけ**（中身は M11、`PLANS.md` §5.12）。
//!
//! 移行の対象は v0 形式（§14.2 の検査に落ちる側）なので、§14.2 の検査はしない。ディレクトリとして
//! 開けるか（見つからない → 3、ディレクトリでない → 3、読めない → 4）だけを確かめ、
//! 「まだ実装されていない」と出して 2 で終わる（`PLANS.md` §9.2 B-105）。

use std::path::Path;

use crate::ports::env::Env;

use super::{open, Cli, Exit};

/// 実行する。
pub fn run<E: Env>(cli: &mut Cli<'_, E>, dir: &Path) -> Exit {
    if let Err(exit) = open::open_storage(cli, dir) {
        return exit;
    }
    cli.not_implemented("migrate")
}
