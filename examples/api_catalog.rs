//! 正規操作カタログ（§19.7・§19.8.2）の決定的な JSON を標準出力へ出す。
//!
//! `tests/golden/api_catalog.json` の生成用（`docs/developers.md` §7.2: ゴールデンは手書き禁止、
//! 確定ロジックから生成したものを人間が diff レビューして置く）:
//!
//! ```text
//! cargo run --locked --example api_catalog > tests/golden/api_catalog.json
//! ```
//!
//! `tests/api_parity.rs` が同じ呼び出し（`ops::catalog(Locale::Ja)`）の出力と 1 字节ずつ比べる。

use novnow::app::ops;
use novnow::ui::i18n::Locale;

fn main() {
    let catalog = ops::catalog(Locale::Ja);
    println!("{}", serde_json::to_string_pretty(&catalog).expect("JSON"));
}
