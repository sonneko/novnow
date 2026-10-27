//! 凍結された MCP 14 ツール（§19.8.3）のゴールデン検査。
//!
//! `REF:tests/cli.rs` の既存分は「`tools/list` の先頭が 14 件ある」ことを見るが、比較相手が
//! `server::tools()`（＝ 実装そのもの）だったので、定義を変えてもテストが通っていた。
//! ここでは **`REF:tests/golden/mcp_tools.json` と 1 バイトずつ比べる**ので、`TOOLS` を
//! 触れば必ず差分が出る（`docs/developers.md` §7.2: ゴールデンは手書き禁止・差分は退行）。
//!
//! 生成:
//! ```text
//! cargo run --locked --example mcp_tools > tests/golden/mcp_tools.json
//! ```

use std::collections::BTreeSet;

use serde_json::Value;

use novnow::app::ops;
use novnow::app::session::Interface;
use novnow::core::agents::server;
use novnow::ui::i18n::Locale;

/// 凍結された 14 ツールの定義（`REF:examples/mcp_tools.rs` が生成した正本）。
const GOLDEN: &str = include_str!("golden/mcp_tools.json");

/// ゴールデンを読む（14 件）。
fn golden() -> Vec<Value> {
    let parsed: Value = serde_json::from_str(GOLDEN).expect("golden が JSON ではない");
    let tools = parsed.as_array().expect("golden は配列").clone();
    assert_eq!(tools.len(), 14, "凍結は 14 件（§19.8.3）: {}", tools.len());
    tools
}

/// `server::tools()` の出力がゴールデンと 1 バイト一致する（§19.8.3）。
#[test]
fn the_frozen_fourteen_definitions_match_the_golden_bytes() {
    let generated = serde_json::to_string_pretty(&server::tools()).expect("JSON");
    assert_eq!(
        format!("{generated}\n"),
        GOLDEN,
        "凍結 14 ツールの定義が変わった。意図した変更なら `cargo run --locked --example mcp_tools \
         > tests/golden/mcp_tools.json` で再生成し、人間が diff をレビューする（§7.2）"
    );
}

/// 凍結 14 件の名前・説明・引数がゴールデンどおりで、実装（`server::is_tool`）とも一致する。
#[test]
fn the_frozen_names_descriptions_and_arguments_are_the_golden_ones() {
    for tool in golden() {
        let name = tool["name"].as_str().expect("name");
        assert!(
            server::is_tool(name),
            "{name} はゴールデンにあるのに `TOOLS` に無い"
        );
        assert!(
            tool["description"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "{name} の説明が空"
        );
        // 引数の集合も固定（`inputSchema.properties`）
        let properties = tool["inputSchema"]["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{name} の inputSchema.properties が無い"));
        for (argument, schema) in properties {
            let kind = schema["type"].as_str().unwrap_or("");
            assert!(
                ["string", "integer"].contains(&kind),
                "{name}.{argument} の型が凍結の形と違う: {kind}"
            );
        }
    }
    // 逆方向（`TOOLS` にあってゴールデンに無い名前が無いこと）
    let names: BTreeSet<String> = golden()
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_owned())
        .collect();
    assert_eq!(names.len(), 14, "名前の重複がある");
}

/// 全ツールが §19.5.3 の JSON Schema を持つ（型・必須・プロパティの形を機械検査する）。
///
/// カタログ由来のツールは、`properties` の名前と型が `OPS` の宣言（`ArgDef`）と一致し、
/// `required` が必須宣言と一致することも見る（`REF:src/app/ops/mod.rs` の `input_schema`）。
#[test]
fn every_tool_carries_a_well_formed_input_schema() {
    const KINDS: [&str; 6] = ["string", "integer", "number", "boolean", "array", "object"];
    let frozen = Value::Array(golden());
    let listed = ops::mcp_tools(Locale::Ja, &frozen);
    let tools = listed.as_array().expect("tools/list の配列");
    assert!(tools.len() > 14, "カタログが載っていない: {}", tools.len());

    for tool in tools {
        let name = tool["name"].as_str().expect("name");
        let schema = &tool["inputSchema"];
        assert_eq!(
            schema["type"],
            serde_json::json!("object"),
            "{name}: inputSchema.type が object ではない"
        );
        let properties = schema["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{name}: properties がオブジェクトではない"));
        for (argument, property) in properties {
            let kind = property["type"]
                .as_str()
                .unwrap_or_else(|| panic!("{name}.{argument}: type が無い（§19.5.3）"));
            assert!(
                KINDS.contains(&kind),
                "{name}.{argument}: JSON Schema の型ではない: {kind}"
            );
        }
        let required: BTreeSet<String> = schema
            .get("required")
            .map(|value| {
                value
                    .as_array()
                    .unwrap_or_else(|| panic!("{name}: required が配列ではない"))
                    .iter()
                    .map(|item| item.as_str().expect("required の要素").to_owned())
                    .collect()
            })
            .unwrap_or_default();
        for argument in &required {
            assert!(
                properties.contains_key(argument),
                "{name}: required の {argument} が properties に無い"
            );
        }

        // カタログ由来のツールは OPS の宣言と突き合わせる（凍結 14 件は上のゴールデン検査）
        let Some(id) = name.strip_prefix("novnow.") else {
            continue;
        };
        let Some(def) = ops::find(id) else {
            assert!(
                server::is_tool(name),
                "{name} はカタログにも凍結 14 件にも無い"
            );
            continue;
        };
        if server::is_tool(name) {
            continue; // 凍結 14 件と重なる名前は凍結側の定義が勝つ（§19.8.3）
        }
        let mut declared: BTreeSet<String> =
            def.args.iter().map(|arg| arg.name.to_owned()).collect();
        if def.gate {
            declared.insert("role".to_owned());
        }
        let listed_names: BTreeSet<String> = properties.keys().cloned().collect();
        assert_eq!(
            listed_names, declared,
            "{name}: inputSchema の引数名が OPS の宣言と違う"
        );
        // 位置引数の束縛順（§19.3.2）も宣言と一致する（Python が使う）
        assert_eq!(
            def.argument_order(),
            def.args
                .iter()
                .map(|arg| arg.name)
                .chain(
                    if def.gate && !def.args.iter().any(|arg| arg.name == "role") {
                        Some("role")
                    } else {
                        None
                    }
                )
                .collect::<Vec<_>>(),
            "{name}: argument_order が宣言順と違う"
        );
        let unique: BTreeSet<&str> = def.argument_order().into_iter().collect();
        assert_eq!(
            unique.len(),
            def.argument_order().len(),
            "{name}: argument_order に重複がある"
        );
        for arg in def.args {
            assert_eq!(
                properties[arg.name]["type"].as_str(),
                Some(arg.kind),
                "{}.{}: 型が宣言と違う",
                name,
                arg.name
            );
            // 無条件関門の `role` は引数の宣言に関わらず必須になる（§19.1.5）。
            // 書き込み時だけの関門（`gate_arg`）は `role` を必須にしない（§19.7）
            let mandatory =
                arg.required || (def.gate && def.gate_arg.is_none() && arg.name == "role");
            assert_eq!(
                required.contains(arg.name),
                mandatory,
                "{}.{}: required の宣言が違う",
                name,
                arg.name
            );
        }
        // 無条件関門（§19.1.5）は `role` を必須にする。`role` を引数として宣言する操作
        // （`agent.task_create` など）は関門でなくても必須になる
        let role_required = (def.gate && def.gate_arg.is_none())
            || def
                .args
                .iter()
                .any(|arg| arg.name == "role" && arg.required);
        assert_eq!(
            required.contains("role"),
            role_required,
            "{name}: 関門と `role` の必須宣言が合わない"
        );
    }
}

/// ツール表の中身 = 凍結 14 件 + 「M—」でない操作のうち凍結と名前が重ならないもの。
///
/// 仕様書 §19.5.1 の「全件」を数で固定する（`REF:docs/designs/interfaces.md` §19.5 の記述と
/// 実数がずれていないかの機械検査）。
#[test]
fn the_tool_table_is_the_frozen_fourteen_plus_every_operation_mcp_offers() {
    let frozen = golden();
    let mut expected: BTreeSet<String> = frozen
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_owned())
        .collect();
    let mut offered = 0usize;
    let mut refused = 0usize;
    for def in ops::OPS {
        let name = format!("novnow.{}", def.id);
        if def.excepts(Interface::Mcp).is_none() {
            offered += 1;
            expected.insert(name);
        } else {
            refused += 1;
        }
    }
    let listed = ops::mcp_tools(Locale::Ja, &Value::Array(frozen));
    let names: BTreeSet<String> = listed
        .as_array()
        .expect("配列")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_owned())
        .collect();

    assert_eq!(
        names, expected,
        "ツール表が「凍結 14 + MCP が提供する操作」と一致しない"
    );
    // 内訳も固定する（`docs/history.md` のの H4 と設計書 §19.5 の記述の正本）
    assert_eq!(offered + refused, 120, "カタログの全操作数");
    assert_eq!(refused, 35, "「M—」で MCP が提供しない操作数");
    assert_eq!(names.len(), 87, "ツール表の全件数（凍結 14 + 正規操作 73）");
    assert!(
        refused > 0 && offered > 0,
        "内訳が壊れている: offered={offered} refused={refused}"
    );
}
