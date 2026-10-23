//! `novnow api list [--json] [--lang ja|en]`（§19.6.1・§19.8.1）。
//!
//! 正規操作カタログ（§19.7 の全行）を出す。カタログは静的なのでプロジェクトは開かない。
//! 人間可読の既定出力は 1 行 1 操作（`M` は関門のある操作。§19.7 の例外列）。

use crate::app::ops;
use crate::ports::env::Env;

use super::json;
use super::{Cli, Exit, Stream};

/// 実行する。`verb` は `list` だけ。
pub fn run<E: Env>(cli: &mut Cli<'_, E>, verb: &str, as_json: bool, lang: Option<&str>) -> Exit {
    if verb != "list" {
        cli.line(Stream::Err, &cli.format("cli.error.unknown_verb", &[verb]));
        return Exit::Usage;
    }
    let locale = json::locale_of(lang).unwrap_or(cli.locale());
    let catalog = ops::catalog(locale);
    if as_json {
        cli.line(Stream::Out, &json::envelope("api.list", &catalog));
        return Exit::Ok;
    }
    for operation in catalog["operations"].as_array().into_iter().flatten() {
        let id = operation["id"].as_str().unwrap_or_default();
        let description = operation["description"].as_str().unwrap_or_default();
        let gate = if operation["gate"].as_bool().unwrap_or(false) {
            "M"
        } else {
            " "
        };
        cli.line(Stream::Out, &format!("{gate} {id:<32}{description}"));
    }
    Exit::Ok
}
