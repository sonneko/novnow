//! 凍結された MCP 14 ツールの定義 JSON（§19.8.3）を標準出力へ出す。
//!
//! `tests/golden/mcp_tools.json` の生成用（`docs/developers.md` §7.2: ゴールデンは手書き禁止、
//! 確定ロジックから生成したものを人間が diff レビューして置く）:
//!
//! ```text
//! cargo run --locked --example mcp_tools > tests/golden/mcp_tools.json
//! ```
//!
//! `REF:tests/mcp_frozen.rs` が (1) `server::tools()` と (2) 実 `tools/list` の先頭 14 件を
//! このファイルと 1 バイトずつ比べる。`REF:src/core/agents/server.rs` の `TOOLS` を変えると
//! ここで差分が出る（既存の MCP クライアントとの互換が壊れる）ので、意図した変更のときだけ
//! 人間が diff をレビューして再生成する。

use novnow::core::agents::server;

fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&server::tools()).expect("JSON")
    );
}
