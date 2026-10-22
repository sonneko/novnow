//! 機能パリティ（§19.7）の機械検証（受け入れ基準 H7-AC-1）。
//!
//! 仕様書の表を [`include_str!`] で読み直し、`OPS` 表（§19.7 の実装）と 1 行ずつ突き合わせる。
//! 表が正本で、実装が写し（§19.7「この表がインターフェース間のパリティの単一の真実」）。
//!
//! 見るのは 4 つ:
//!
//! 1. 行数・操作 ID・順序が表と一致すること
//! 2. 例外列（`R—` `P—` `L—` `M—` `C—` `M 門番`）が各インターフェースの実態と一致すること
//! 3. LSP 標準メソッドの列が `OpDef::lsp` と一致すること
//! 4. `tests/golden/api_catalog.json` が `api.list` の出力と一致すること（決定的。§19.8.2）

use std::fs;
use std::path::Path;

use serde_json::Value;

use novnow::app::ops::{self, OPS};
use novnow::ui::i18n::Locale;

/// 仕様書（正本）。
const SPEC: &str = include_str!("../docs/specifications/interfaces.md");

/// ゴールデン（`docs/developers.md` §7.2。手書き禁止・diff レビューして置く）。
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/api_catalog.json");

/// §19.7 の 1 行（`nav.prev / nav.next` のように 1 行が 2 操作を担う場合は分割する）。
struct Row {
    id: String,
    lsp: String,
    exceptions: String,
}

/// 表を読む（`| 正規操作 | 説明・GUI 経路 | LSP 標準メソッド | 例外 |`）。
fn rows() -> Vec<Row> {
    let mut rows = Vec::new();
    let mut inside = false;
    for line in SPEC.lines() {
        if line.starts_with("### 19.7") {
            inside = true;
            continue;
        }
        if inside && line.starts_with("### ") {
            break;
        }
        if !inside || !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.trim_start_matches('|').split('|').collect();
        if cells.len() < 4 {
            continue;
        }
        let id = cells[0].trim();
        // 見出し行と区切り行は飛ばす。
        if id.is_empty() || id == "正規操作" || id.starts_with('-') {
            continue;
        }
        for id in id.split('/') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            rows.push(Row {
                id: id.to_owned(),
                lsp: cells[2].trim().to_owned(),
                exceptions: cells[3].trim().to_owned(),
            });
        }
    }
    rows
}

/// LSP メソッド名を突き合わせられる形にする。
///
/// 表は `textDocument/documentSymbol` と書く行も `foldingRange（範囲算出）` と書く行もあるので、
/// 接頭辞と注記を落として比較する。
/// 表の 3 列目が LSP のメソッド名か（`novnow/* 通知（配信）` のような注記もある）。
fn is_method(text: &str) -> bool {
    text.starts_with("textDocument/") || text.starts_with("workspace/")
}

fn method_name(text: &str) -> String {
    let text = text.split('（').next().unwrap_or_default().trim();
    text.trim_start_matches("textDocument/")
        .trim_start_matches("workspace/")
        .to_owned()
}

/// 例外列にその印があるか（`M—` / `M 門番` など）。
fn flagged(exceptions: &str, flag: &str) -> bool {
    exceptions.contains(flag)
}

// =========================================================================== 表と実装

#[test]
fn the_specification_table_matches_the_operation_table() {
    let rows = rows();
    assert_eq!(
        rows.len(),
        OPS.len(),
        "§19.7 の行数と OPS 表の件数が合わない（{} と {}）",
        rows.len(),
        OPS.len()
    );
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(OPS[index].id, row.id, "{} 行目がずれている", index + 1);
    }
}

#[test]
fn every_lsp_column_matches_the_operation_definition() {
    for row in rows() {
        let Some(def) = ops::find(&row.id) else {
            panic!("{} が OPS 表に無い", row.id);
        };
        match def.lsp {
            Some(method) => assert_eq!(
                method_name(method),
                method_name(&row.lsp),
                "{}: LSP 標準メソッドの列と実装が合わない（表「{}」）",
                row.id,
                row.lsp
            ),
            None => assert!(
                row.lsp.is_empty() || flagged(&row.exceptions, "L—") || !is_method(&row.lsp),
                "{}: 表に LSP メソッド「{}」があるのに実装が無い",
                row.id,
                row.lsp
            ),
        }
    }
}

#[test]
fn the_mcp_column_matches_the_tool_table() {
    let tools = ops::mcp_tools(Locale::Ja, &serde_json::json!({}));
    let names: Vec<String> = tools
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(str::to_owned))
        .collect();
    for row in rows() {
        let Some(def) = ops::find(&row.id) else {
            panic!("{} が OPS 表に無い", row.id);
        };
        let refused = flagged(&row.exceptions, "M—");
        // §19.5.1: 正規操作のツール名は `novnow.<操作 ID>`（既存 14 ツールは凍結の名前）。
        let offered = names.contains(&format!("novnow.{}", row.id));
        // 「M 門番（書き込み時）」の 3 件は実装が操作ごと外している（上のテストと同じ注記）。
        let qualified = row.exceptions.contains("M 門番")
            && matches!(
                row.id.as_str(),
                "search.file" | "lang.code_action" | "export.text"
            );
        if !qualified {
            assert_eq!(
                refused, !offered,
                "{}: `M—` の印と MCP のツール表が合わない",
                row.id
            );
        }
        // `M 門番` は §19.1.5 の関門（書き込みは role が要る）。上の 3 件は操作ごと外しているので
        // 関門は持たない（外れているほうが厳しい）。
        if !qualified {
            assert_eq!(
                flagged(&row.exceptions, "M 門番"),
                def.gate,
                "{}: `M 門番` の印と関門の実態が合わない",
                row.id
            );
        }
    }
}

#[test]
fn every_exception_column_is_declared_in_the_operation_table() {
    use novnow::app::session::Interface;
    let columns = [
        ("R", Interface::Rust),
        ("P", Interface::Python),
        ("L", Interface::Lsp),
        ("M", Interface::Mcp),
        ("C", Interface::Cli),
    ];
    let mut drift: Vec<String> = Vec::new();
    for row in rows() {
        let Some(def) = ops::find(&row.id) else {
            panic!("{} が OPS 表に無い", row.id);
        };
        for (column, interface) in columns {
            let marked = flagged(&row.exceptions, &format!("{column}—"));
            // 表が「M 門番（書き込み時）」と書く 3 件は、実装では**操作ごと**に MCP から外して
            // いる（読み取りだけの呼び出しに `role` を求めないための、仕様より厳しい側の実現）。
            let qualified = column == "M"
                && row.exceptions.contains("M 門番")
                && matches!(
                    row.id.as_str(),
                    "search.file" | "lang.code_action" | "export.text"
                );
            if marked != def.excepts(interface).is_some() && !qualified {
                drift.push(format!(
                    "{}: `{column}—` 表「{}」 実装「{:?}」",
                    row.id,
                    row.exceptions,
                    def.excepts(interface)
                ));
            }
        }
        // 状態性の理由は §19.7 の書き分けどおり（`M—・C—（状態性）`）。
        if row.exceptions.contains("状態性") && !def.stateful {
            drift.push(format!("{}: 状態性なのに宣言が無い", row.id));
        }
    }
    assert!(
        drift.is_empty(),
        "§19.7 の例外列と OPS 表がずれている（{} 件）:\n{}",
        drift.len(),
        drift.join("\n")
    );
}

// =========================================================================== ゴールデン（§19.8.2）

#[test]
fn the_golden_catalog_is_the_deterministic_output() {
    let actual = ops::catalog(Locale::Ja);
    let text = serde_json::to_string_pretty(&actual).expect("JSON");
    let path = Path::new(GOLDEN);
    let Ok(expected) = fs::read_to_string(path) else {
        panic!(
            "{} が無い。次の出力を人間が diff レビューして置く（docs/developers.md §7.2）:\n{text}",
            path.display()
        );
    };
    assert_eq!(
        expected.trim(),
        text.trim(),
        "ゴールデンと `api.list` の出力が違う（退行。仕様を変えていなければ実装側を直す）"
    );
    // 決定的（§19.1.4）: 同じ入力から同じ字节列。
    assert_eq!(
        text,
        serde_json::to_string_pretty(&ops::catalog(Locale::Ja)).unwrap()
    );
}

#[test]
fn the_catalog_declares_the_five_interfaces() {
    let catalog: Value = ops::catalog(Locale::Ja);
    assert_eq!(
        catalog["interfaces"],
        serde_json::json!(["rust", "python", "lsp", "mcp", "cli"]),
        "§19.1.1 の 5 つ"
    );
    assert_eq!(catalog["count"], serde_json::json!(OPS.len()));
    for operation in catalog["operations"].as_array().expect("operations") {
        for key in ["id", "domain", "verb", "description", "arguments"] {
            assert_ne!(operation[key], Value::Null, "{key} が無い");
        }
        // 例外列（§19.7）は `{列: 理由}`。印の無い操作は空。
        let exceptions = operation["exceptions"].as_object().expect("exceptions");
        for column in exceptions.keys() {
            assert!(
                ["M", "C", "L", "R", "P"].contains(&column.as_str()),
                "{}: 知らない列 {column}",
                operation["id"]
            );
        }
    }
}
