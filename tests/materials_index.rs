//! `core::syntax::materials_index`（`materials/index.json`、§5.8.2）のテスト。
//!
//! `docs/history.md` の完了条件の 1（§5.8.2 の例が期待どおりにパースできる）・2（round-trip が
//! バイト一致）・3（壊れた入力でパニックしない）に対応する。

use novnow::core::diagnostics::{Arg, DiagnosticCode};
use novnow::core::model::material::{
    ConditionKind, Material, MaterialId, MaterialIndex, MaterialStatus, SourceKind,
    PREFIX_MATERIAL, PREFIX_SEARCH,
};
use novnow::core::syntax::materials_index;

/// §5.8.2 の例（スキーマ v1）。**仕様のコード例をそのまま**使う（§5.3 完了条件 1）。
const SPEC_EXAMPLE: &str = r#"{
  "schema_version": 1,
  "next_id": 138,
  "items": [
    {
      "id": "M0137",
      "title": "科学の甲子園 本戦の運営と実技課題",
      "path": "materials/docs/M0137-kagaku-koshien.md",
      "source": { "kind": "url", "url": "https://example.org/report/2026", "accessed_at": "2026-10-01T09:12:00+09:00" },
      "category": "取材",
      "conditions": [
        { "kind": "scope", "value": "@02-03 .. @02-08" },
        { "kind": "ref",   "value": "'科学の甲子園" },
        { "kind": "text",  "value": "大会運営の手順を描写するとき" }
      ],
      "saved_at": "2026-10-01T09:20:33+09:00",
      "updated_at": "2026-10-02T22:10:00+09:00",
      "tags": ["大会", "高校", "運営"],
      "status": "approved",
      "added_by": "human",
      "summary": "本戦の 1 日のタイムテーブルと実技課題の採点基準。",
      "checksum": "sha256:9f2c..."
    }
  ]
}
"#;

// ============================================================ タグ

fn diags_tag(text: &str) -> String {
    let parts: Vec<String> = materials_index::parse(text)
        .diagnostics
        .iter()
        .map(|found| {
            let range = match &found.range {
                Some(found) => format!("{}..{}", found.start, found.end),
                None => "-".to_owned(),
            };
            let args: Vec<String> = found
                .args
                .iter()
                .map(|arg| match arg {
                    novnow::core::diagnostics::Arg::Text(value) => value.clone(),
                    novnow::core::diagnostics::Arg::Number(value) => value.to_string(),
                })
                .collect();
            if args.is_empty() {
                format!("{}@{}", found.code.as_str(), range)
            } else {
                format!("{}@{}({})", found.code.as_str(), range, args.join(","))
            }
        })
        .collect();
    parts.join("\n")
}

/// 資料 1 件だけの index（検査用の JSON を作る）。
fn index_with(items: &str) -> String {
    format!("{{\n  \"schema_version\": 1,\n  \"next_id\": 138,\n  \"items\": [\n{items}\n  ]\n}}\n")
}

/// 最小の資料（§5.8.2 の必須キーだけ）。
fn item(id: &str, path: &str) -> String {
    format!(
        "    {{\n      \"id\": \"{id}\",\n      \"title\": \"t\",\n      \"path\": \"{path}\",\n      \
         \"source\": {{\"kind\": \"custom\", \"text\": \"取材\"}},\n      \"category\": \"取材\",\n      \
         \"conditions\": [],\n      \"saved_at\": \"2026-10-01T09:20:33+09:00\",\n      \
         \"status\": \"approved\",\n      \"added_by\": \"human\"\n    }}"
    )
}

// ============================================================ §5.8.2 の例（受け入れテスト）

#[test]
fn the_spec_example_parses() {
    let parsed = materials_index::parse(SPEC_EXAMPLE);
    assert!(parsed.error.is_none(), "{:?}", parsed.error);
    let index = parsed.index.as_ref().unwrap();
    assert_eq!(index.schema_version, 1);
    assert_eq!(index.next_id, 138);
    assert_eq!(index.items.len(), 1);
    let found = &index.items[0];
    assert_eq!(found.id, "M0137");
    assert_eq!(
        found.material_id(),
        Some(MaterialId {
            prefix: 'M',
            num: 137
        })
    );
    assert_eq!(found.title, "科学の甲子園 本戦の運営と実技課題");
    assert_eq!(found.path, "materials/docs/M0137-kagaku-koshien.md");
    assert_eq!(found.category, "取材");
    assert_eq!(found.saved_at, "2026-10-01T09:20:33+09:00");
    assert_eq!(
        found.updated_at.as_deref(),
        Some("2026-10-02T22:10:00+09:00")
    );
    assert_eq!(found.tags, vec!["大会", "高校", "運営"]);
    assert_eq!(found.status_kind(), Some(MaterialStatus::Approved));
    assert!(found.is_added_by_human());
    assert_eq!(found.agent_role(), None);
    assert_eq!(
        found.summary.as_deref(),
        Some("本戦の 1 日のタイムテーブルと実技課題の採点基準。")
    );
    assert_eq!(found.checksum.as_deref(), Some("sha256:9f2c..."));
}

#[test]
fn the_spec_example_reads_the_source_and_the_conditions() {
    let found = &materials_index::parse(SPEC_EXAMPLE).index.unwrap().items[0];
    assert_eq!(found.source.kind_value(), Some(SourceKind::Url));
    assert_eq!(
        found.source.url.as_deref(),
        Some("https://example.org/report/2026")
    );
    assert_eq!(
        found.source.accessed_at.as_deref(),
        Some("2026-10-01T09:12:00+09:00")
    );
    // §16.4 の 4 種のうち 3 つ（scope / ref / text）
    let kinds: Vec<Option<ConditionKind>> = found
        .conditions
        .iter()
        .map(|found| found.kind_value())
        .collect();
    assert_eq!(
        kinds,
        vec![
            Some(ConditionKind::Scope),
            Some(ConditionKind::Ref),
            Some(ConditionKind::Text)
        ]
    );
    assert_eq!(found.conditions[0].value, "@02-03 .. @02-08");
    assert_eq!(found.conditions[1].value, "'科学の甲子園");
}

#[test]
fn the_spec_example_has_no_diagnostics() {
    assert_eq!(diags_tag(SPEC_EXAMPLE), "");
}

#[test]
fn the_spec_example_round_trips_byte_for_byte() {
    let parsed = materials_index::parse(SPEC_EXAMPLE);
    // JSON は再シリアライズするとキー順と空白が変わるので、**原文をそのまま返す**
    assert_eq!(parsed.to_text(), SPEC_EXAMPLE);
}

#[test]
fn the_spec_example_survives_a_semantic_round_trip() {
    // パース → シリアライズ → パース で同じ木になる（未知キーも含めて）
    let first = materials_index::parse(SPEC_EXAMPLE).index.unwrap();
    let text = serde_json::to_string_pretty(&first).unwrap();
    let second = materials_index::parse(&text).index.unwrap();
    assert_eq!(first, second);
}

// ============================================================ 未知キーの温存（縫い代 1）

#[test]
fn unknown_keys_are_preserved() {
    let text = "{\n  \"schema_version\": 1,\n  \"next_id\": 2,\n  \"items\": [],\n  \
                \"future\": {\"a\": 1},\n  \"flag\": true\n}\n";
    let parsed = materials_index::parse(text);
    let index = parsed.index.as_ref().unwrap();
    assert_eq!(index.unknown.len(), 2);
    assert_eq!(index.unknown["flag"], serde_json::Value::Bool(true));
    // 未知キーは書き戻しても残る（§5.3 完了条件 2 の「未知フィールドキーを含む」側）
    let again: MaterialIndex =
        serde_json::from_str(&serde_json::to_string(index).unwrap()).unwrap();
    assert_eq!(again.unknown, index.unknown);
    assert_eq!(parsed.to_text(), text);
}

#[test]
fn unknown_keys_survive_inside_an_item_and_its_source() {
    let text = index_with(&item("M0001", "materials/docs/M0001-a.md").replace(
        "\"added_by\": \"human\"",
        "\"added_by\": \"human\",\n      \"future\": 1",
    ))
    .replace(
        "\"kind\": \"custom\"",
        "\"kind\": \"custom\", \"where\": \"東京\"",
    );
    let index = materials_index::parse(&text).index.unwrap();
    let found = &index.items[0];
    assert_eq!(found.unknown["future"], serde_json::json!(1));
    assert_eq!(found.source.unknown["where"], serde_json::json!("東京"));
}

#[test]
fn a_reader_like_unknown_key_is_not_read_but_kept() {
    // §4.5.1「`reader` は v1 本体は読まない。未知キーとして温存する」と同じ扱い
    let text = "{\n  \"items\": [],\n  \"reader\": {\"columns\": 18}\n}\n";
    let index = materials_index::parse(text).index.unwrap();
    assert_eq!(index.unknown.len(), 1);
    assert!(index.unknown.contains_key("reader"));
}

// ============================================================ E024（path）

#[test]
fn a_path_outside_materials_reports_e024() {
    for path in [
        "contents/01-01.cnov",
        "../secret.md",
        "/etc/passwd",
        "materials/../x.md",
    ] {
        let text = index_with(&item("M0001", path));
        let parsed = materials_index::parse(&text);
        assert_eq!(parsed.diagnostics.len(), 1, "{path}");
        assert_eq!(parsed.diagnostics[0].code.as_str(), "E024", "{path}");
        // 位置は原文中の path のリテラル
        let range = parsed.diagnostics[0].range.as_ref().unwrap();
        assert_eq!(&text[range.clone()], format!("\"{path}\""), "{path}");
    }
}

#[test]
fn a_path_inside_materials_is_accepted() {
    for path in [
        "materials/docs/M0001-a.md",
        "materials/inbox/M0002-b.md",
        "materials/index.json",
    ] {
        let text = index_with(&item("M0001", path));
        assert_eq!(diags_tag(&text), "", "{path}");
    }
}

#[test]
fn an_empty_path_is_not_reported() {
    // 空の `path` は「materials/ 配下でない」が、§7 に対応コードが無いので診断しない（B-39）
    let text = index_with(&item("M0001", ""));
    assert_eq!(diags_tag(&text), "");
}

// ============================================================ E008（ID の再利用禁止）

#[test]
fn a_duplicate_id_reports_e008() {
    let text = index_with(&format!(
        "{},\n{}",
        item("M0137", "materials/docs/M0137-a.md"),
        item("M0137", "materials/docs/M0137-b.md")
    ));
    let parsed = materials_index::parse(&text);
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(parsed.diagnostics[0].code.as_str(), "E008");
    // 位置は 2 件目の `"M0137"`
    let range = parsed.diagnostics[0].range.as_ref().unwrap();
    assert_eq!(&text[range.clone()], "\"M0137\"");
    assert!(range.start > text.find("\"M0137\"").unwrap());
}

#[test]
fn every_reuse_is_reported() {
    let text = index_with(&format!(
        "{},\n{},\n{}",
        item("M0001", "materials/docs/a.md"),
        item("M0001", "materials/docs/b.md"),
        item("M0001", "materials/docs/c.md")
    ));
    assert_eq!(diags_tag(&text).lines().count(), 2);
}

#[test]
fn m_and_s_share_one_number_space() {
    // §5.8.2「`M` と `S` は同一の連番空間を共有する（MUST）」。接頭辞が違えば別の ID
    let text = index_with(&format!(
        "{},\n{}",
        item("M0137", "materials/docs/M0137-a.md"),
        item("S0138", "materials/docs/S0138-search.md")
    ));
    let parsed = materials_index::parse(&text);
    assert_eq!(parsed.diagnostics.len(), 0);
    let index = parsed.index.as_ref().unwrap();
    assert_eq!(index.max_number(), Some(138));
    // 番号が同じなら接頭辞が違っても**別の資料**（衝突は番号で起きる）
    assert_eq!(index.duplicate_ids().len(), 0);
}

// ============================================================ 採番（§5.8.2 MUST）

#[test]
fn issue_hands_out_the_next_number_and_moves_next_id() {
    let mut index = materials_index::parse(SPEC_EXAMPLE).index.unwrap();
    assert_eq!(index.next_id, 138);
    assert_eq!(index.issue(PREFIX_MATERIAL).to_string(), "M0138");
    assert_eq!(index.next_id, 139);
    assert_eq!(index.issue(PREFIX_MATERIAL).to_string(), "M0139");
    // §5.8.2「`M0137` の次に検索結果を作れば `S0138` になる」
    let mut index = materials_index::parse(SPEC_EXAMPLE).index.unwrap();
    assert_eq!(index.issue(PREFIX_SEARCH).to_string(), "S0138");
}

#[test]
fn issue_never_reuses_a_number_even_when_next_id_is_stale() {
    // `next_id` が既存 ID より小さい（書き換えられた）状態でも再利用しない
    let mut index = MaterialIndex {
        next_id: 1,
        items: vec![material_with_id("M0137")],
        ..MaterialIndex::default()
    };
    assert!(!index.is_next_id_valid());
    assert_eq!(index.issue(PREFIX_MATERIAL).to_string(), "M0138");
    assert_eq!(index.next_id, 139);
    assert!(index.is_next_id_valid());
}

#[test]
fn a_rejected_item_keeps_its_id() {
    // §5.8.2「`rejected` にしても ID は回収しない」
    let mut index = MaterialIndex {
        next_id: 2,
        items: vec![material_with_id("M0001")],
        ..MaterialIndex::default()
    };
    index.items[0].status = "rejected".to_owned();
    assert_eq!(index.items[0].status_kind(), Some(MaterialStatus::Rejected));
    assert_eq!(index.issue(PREFIX_MATERIAL).to_string(), "M0002");
    assert_eq!(index.duplicate_ids().len(), 0);
}

#[test]
fn duplicate_ids_are_listed_for_the_writer() {
    let index = MaterialIndex {
        items: vec![material_with_id("M0001"), material_with_id("M0001")],
        ..MaterialIndex::default()
    };
    assert_eq!(index.duplicate_ids(), vec!["M0001"]);
}

// ============================================================ ID の形（縫い代 7）

#[test]
fn material_ids_read_a_one_char_prefix() {
    assert_eq!(
        MaterialId::parse("M0137"),
        Some(MaterialId {
            prefix: 'M',
            num: 137
        })
    );
    // ② の `S` も**読む**（`docs/plans.md` §4.4 の縫い代 7）
    assert_eq!(
        MaterialId::parse("S0042"),
        Some(MaterialId {
            prefix: 'S',
            num: 42
        })
    );
    // 4 桁以上（§5.8.2）。9999 を超えれば桁が増える
    assert_eq!(MaterialId::parse("M10000").unwrap().to_string(), "M10000");
    assert!(MaterialId::parse("M137").is_none());
    assert!(MaterialId::parse("M013x").is_none());
    assert!(MaterialId::parse("0137").is_none());
    assert!(MaterialId::parse("").is_none());
}

#[test]
fn only_m_is_the_material_prefix() {
    assert!(MaterialId::parse("M0137").unwrap().is_material_prefix());
    assert!(!MaterialId::parse("S0042").unwrap().is_material_prefix());
}

// ============================================================ 壊れた入力（完了条件 3）

#[test]
fn broken_json_returns_the_error_without_panicking() {
    for text in [
        "",
        "{",
        "[]",
        "null",
        "{\"items\": }",
        "{\"items\": [{}]}",
        "{\"next_id\": \"abc\"}",
        "{\"items\": [{\"id\": 1}]}",
    ] {
        let parsed = materials_index::parse(text);
        // 原文は必ずそのまま返る（round-trip）
        assert_eq!(parsed.to_text(), text);
        // 読めれば木があり、読めなければ理由がある（どちらでもパニックしない）
        assert!(parsed.index.is_some() || parsed.error.is_some(), "{text:?}");
    }
}

#[test]
fn a_broken_json_reports_e025_with_the_parser_message() {
    // §4.3-4・§7 の E025「JSON ファイルがパースできない」。位置はファイル全体で、
    // 引数 {0} はパーサのメッセージ
    let text = "{\"items\": }";
    let parsed = materials_index::parse(text);
    assert!(parsed.index.is_none());
    assert!(parsed.error.as_deref().unwrap().contains("expected value"));
    assert_eq!(parsed.diagnostics.len(), 1);
    let found = &parsed.diagnostics[0];
    assert_eq!(found.code, DiagnosticCode::E025);
    assert_eq!(found.range, Some(0..text.len()));
    assert_eq!(found.args.len(), 1);
    let Arg::Text(message) = &found.args[0] else {
        panic!("{:?}", found.args);
    };
    assert!(message.contains("expected value"), "{message}");
    // E025 が出たときは E024 / E008 は報告しない（§5.8.2）
    assert!(!parsed
        .diagnostics
        .iter()
        .any(|item| item.code != DiagnosticCode::E025));
}

#[test]
fn a_broken_json_continues_with_the_default_index_and_keeps_the_original() {
    // §5.8.2「診断を出したうえで空の索引の既定値で続行する」「原文は書き換えない（MUST）」
    let text = "{\"next_id\": 7,";
    let parsed = materials_index::parse(text);
    assert_eq!(parsed.diagnostics.len(), 1);
    let index = parsed.index_or_default();
    assert_eq!(index, MaterialIndex::default());
    assert!(index.items.is_empty());
    assert_eq!(parsed.to_text(), text);
    // パースできているときは木そのものを返す
    let good = index_with(&item("M0001", "materials/docs/a.md"));
    let ok = materials_index::parse(&good);
    assert!(ok.diagnostics.is_empty());
    assert_eq!(ok.index_or_default(), *ok.index.as_ref().unwrap());
}

#[test]
fn unknown_enum_values_do_not_break_the_file() {
    // §4.3-4「作家の原稿を人質に取らない」。未知の `status` / `kind` でも読める
    let text = index_with(
        &item("M0001", "materials/docs/a.md")
            .replace("\"status\": \"approved\"", "\"status\": \"archived\""),
    )
    .replace("\"kind\": \"custom\"", "\"kind\": \"oral\"");
    let parsed = materials_index::parse(&text);
    let index = parsed.index.as_ref().unwrap();
    assert_eq!(index.items[0].status, "archived");
    assert_eq!(index.items[0].status_kind(), None);
    assert_eq!(index.items[0].source.kind, "oral");
    assert_eq!(index.items[0].source.kind_value(), None);
    assert!(parsed.error.is_none());
}

#[test]
fn missing_keys_fall_back_to_defaults() {
    // 必須キーが欠けても 1 ファイル全体を捨てない（§4.3-4）。空の index も読める
    let parsed = materials_index::parse("{}");
    let index = parsed.index.as_ref().unwrap();
    assert_eq!(index.schema_version, 0);
    assert_eq!(index.next_id, 0);
    assert!(index.items.is_empty());
    assert!(index.is_next_id_valid());
    let parsed = materials_index::parse("{\"items\": [{}]}");
    let found = &parsed.index.as_ref().unwrap().items[0];
    assert_eq!(found.id, "");
    assert_eq!(found.title, "");
    assert!(found.conditions.is_empty());
    assert_eq!(found.material_id(), None);
}

// ============================================================ 引き当て

#[test]
fn items_can_be_found_by_id_and_title() {
    let parsed = materials_index::parse(SPEC_EXAMPLE);
    assert!(parsed.find("M0137").is_some());
    assert!(parsed.find("S0138").is_none());
    assert!(parsed
        .find_by_id(MaterialId {
            prefix: 'M',
            num: 137
        })
        .is_some());
    // ゼロ詰めの桁が違っていても番号で引ける
    assert!(parsed
        .find_by_id(MaterialId {
            prefix: 'M',
            num: 137
        })
        .is_some());
    assert!(parsed
        .find_by_title("科学の甲子園 本戦の運営と実技課題")
        .is_some());
    assert_eq!(parsed.approved().len(), 1);
}

#[test]
fn an_empty_file_has_nothing_to_find() {
    let parsed = materials_index::parse("");
    assert!(parsed.find("M0137").is_none());
    assert!(parsed.approved().is_empty());
    assert_eq!(parsed.to_text(), "");
}

/// 検査用の資料を 1 件作る。
fn material_with_id(id: &str) -> Material {
    Material {
        id: id.to_owned(),
        path: "materials/docs/a.md".to_owned(),
        status: "approved".to_owned(),
        ..Material::default()
    }
}
