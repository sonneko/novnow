//! `project.toml` の読み込みと検査（`core::syntax::project_toml`、仕様 §4.5）。
//!
//! - **E025**: TOML として読めない → 既定値で続行（§4.3-4）。
//! - **E027**: `lexicon.word_classes` の正規表現が不正（§6.5.1、`docs/history.md` A-24・B-88）。
//! - 設定の読み込み: `features::Project` は `project.toml` の文書の中身を設定にする（B-89）。
//! - 書き戻しは無損失編集（§4.5、Issue #29 B10）。

use novnow::core::diagnostics::{self, Arg, DiagnosticCode};
use novnow::core::features::Project;
use novnow::core::model::document::Document;
use novnow::core::model::project::Project as Settings;
use novnow::core::resolve::role_language::MatcherSet;
use novnow::core::syntax::project_toml;

/// 仕様 §4.5 の TOML の例（仕様書から直接読む。Issue #29 B10 でこの形へ戻した）。
fn spec_example() -> &'static str {
    let spec = include_str!("../docs/specifications/project.md");
    let section = spec
        .split("### 4.5 `project.toml` スキーマ")
        .nth(1)
        .expect("§4.5 がある");
    let fenced = section.split("```toml\n").nth(1).expect("toml の例がある");
    // 借用を静的にするためにリークする（テスト専用）
    Box::leak(
        fenced
            .split("\n```")
            .next()
            .expect("例の終わり")
            .to_owned()
            .into_boxed_str(),
    )
}

/// §4.5 の例の `体言` を `value`（TOML の値リテラル）に差し替えたもの。
fn with_word_class(value: &str) -> String {
    let original = "\"体言\" = '[\\p{Han}\\p{Katakana}\\p{Hiragana}A-Za-z0-9ー]{1,24}'";
    let example = spec_example();
    assert!(example.contains(original), "§4.5 の例が変わった");
    example.replace(original, &format!("\"体言\" = {value}"))
}

#[test]
fn the_spec_example_reads_without_diagnostics() {
    let parsed = project_toml::parse(spec_example());
    assert!(parsed.is_valid(), "{:?}", parsed.error);
    assert_eq!(parsed.diagnostics, vec![]);
    assert_eq!(parsed.settings.schema_version, 2);
    assert_eq!(parsed.settings.title, "星屑の時計");
    assert_eq!(parsed.settings.lexicon.word_classes.len(), 1);
    // §4.5 の `reader` は本体では読まない（未知キーとして温存）
    assert!(parsed.settings.unknown.contains_key("reader"));
}

#[test]
fn broken_toml_is_e025_over_the_whole_file_and_falls_back_to_defaults() {
    let text = "title = \"星屑の時計\n";
    let parsed = project_toml::parse(text);
    assert!(!parsed.is_valid());
    assert_eq!(parsed.settings, Settings::default());
    assert_eq!(parsed.diagnostics.len(), 1);
    let found = &parsed.diagnostics[0];
    assert_eq!(found.code, DiagnosticCode::E025);
    assert_eq!(found.range, Some(0..text.len()));
    assert_eq!(
        found.args,
        vec![Arg::text(parsed.error.clone().expect("理由"))]
    );
}

#[test]
fn a_value_of_the_wrong_type_is_also_e025() {
    // 型が合わない値も「TOML として読み込めない」（設定として使えない）
    let parsed = project_toml::parse("schema_version = \"2\"\n");
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(parsed.diagnostics[0].code, DiagnosticCode::E025);
    assert_eq!(parsed.settings, Settings::default());
}

#[test]
fn a_broken_word_class_is_e027_on_its_value_literal() {
    let text = with_word_class("\"[\"");
    let parsed = project_toml::parse(&text);
    assert!(parsed.is_valid(), "TOML としては読める");
    assert_eq!(parsed.diagnostics.len(), 1);
    let found = &parsed.diagnostics[0];
    assert_eq!(found.code, DiagnosticCode::E027);
    assert_eq!(found.args, vec![Arg::text("体言")]);
    let range = found.range.clone().expect("位置");
    assert_eq!(&text[range], "\"[\"");
    // 設定は読めているので、壊れた値もそのまま持つ（原文は書き換えない）
    assert_eq!(parsed.settings.lexicon.word_classes["体言"], "[");
}

#[test]
fn only_patterns_using_the_broken_class_are_dropped() {
    // §6.5.1: 他のパターンは生かす
    let settings = project_toml::settings(&with_word_class("\"(\""));
    let patterns = [
        novnow::core::model::role_pattern::RolePattern::new("(体言)！"),
        novnow::core::model::role_pattern::RolePattern::new("*かな"),
        novnow::core::model::role_pattern::RolePattern::new("(数)回"),
    ];
    let set = MatcherSet::new(&patterns, &settings.lexicon);
    let sources: Vec<&str> = set.matchers().iter().map(|found| found.source()).collect();
    assert_eq!(sources, vec!["*かな", "(数)回"]);
}

#[test]
fn every_broken_class_gets_its_own_e027_in_key_order() {
    let text = "schema_version = 2\n\n[lexicon.word_classes]\n\"用言\" = \"(\"\n\"体言\" = \"[a-z]+\"\n\"敬称\" = \"さん|様)\"\n";
    let parsed = project_toml::parse(text);
    let names: Vec<&Arg> = parsed
        .diagnostics
        .iter()
        .map(|found| &found.args[0])
        .collect();
    // BTreeMap のキー順（UTF-8 の byte 順）
    assert_eq!(names, vec![&Arg::text("敬称"), &Arg::text("用言")]);
    let slices: Vec<&str> = parsed
        .diagnostics
        .iter()
        .map(|found| &text[found.range.clone().expect("位置")])
        .collect();
    assert_eq!(slices, vec!["\"さん|様)\"", "\"(\""]);
}

#[test]
fn an_escaped_value_falls_back_to_the_table_heading() {
    // `"\u005B"` は `[`。リテラルは原文に無いので、見出しへ倒す
    let text = "[lexicon.word_classes]\n\"体言\" = \"\\u005B\"\n";
    let parsed = project_toml::parse(text);
    assert_eq!(parsed.diagnostics.len(), 1);
    let range = parsed.diagnostics[0].range.clone().expect("位置");
    assert_eq!(&text[range], "word_classes");
}

#[test]
fn the_same_name_before_word_classes_is_not_taken() {
    // 「体言」が word_classes より前（タイトル）にも現れても、値の位置は word_classes の中
    let text = "title = \"体言\"\n\n[lexicon.word_classes]\n\"体言\" = \"[\"\n";
    let parsed = project_toml::parse(text);
    let range = parsed.diagnostics[0].range.clone().expect("位置");
    assert_eq!(&text[range.clone()], "\"[\"");
    assert!(range.start > text.find("word_classes").expect("key"));
}

#[test]
fn a_bom_is_skipped_and_positions_count_it() {
    let text = format!("\u{feff}{}", with_word_class("\"[\""));
    let parsed = project_toml::parse(&text);
    assert!(parsed.is_valid());
    let range = parsed.diagnostics[0].range.clone().expect("位置");
    assert_eq!(&text[range], "\"[\"");
}

#[test]
fn the_aggregate_takes_its_settings_from_the_project_toml_document() {
    let mut project = Project::default();
    let custom = "schema_version = 2\ntitle = \"題\"\n\n[structure]\nchapter_digits = 3\n";
    project.insert(Document::new("project.toml", custom));
    assert_eq!(project.settings().title, "題");
    assert_eq!(project.settings().structure.chapter_digits, 3);
    // 壊れたら既定値（§4.3-4）
    project.insert(Document::new("project.toml", "title = "));
    assert_eq!(project.settings(), &Settings::default());
    // 戻すと読み直す
    project.insert(Document::new("project.toml", custom));
    assert_eq!(project.settings().title, "題");
    // 文書を外すと既定値
    project.remove("project.toml");
    assert_eq!(project.settings(), &Settings::default());
}

#[test]
fn with_documents_prefers_the_project_toml_document() {
    let given = Settings {
        title: "引数".to_owned(),
        ..Settings::default()
    };
    let project =
        Project::with_documents(given, [Document::new("project.toml", "title = \"文書\"\n")]);
    assert_eq!(project.settings().title, "文書");
}

#[test]
fn document_diagnostics_show_e025_and_e027_on_project_toml() {
    let project = Project::default();
    let broken = Document::new("project.toml", "title = ");
    let found = diagnostics::document(&broken, &project);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, DiagnosticCode::E025);

    let text = with_word_class("\"[\"");
    let document = Document::new("project.toml", text.clone());
    let found = diagnostics::document(&document, &project);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, DiagnosticCode::E027);
    assert_eq!(&text[found[0].range.clone().expect("位置")], "\"[\"");
    assert!(found[0].is_underlined());
}

#[test]
fn lossless_edit_keeps_comments_order_and_unknown_keys() {
    // §4.5 の MUST: 変更したキー以外はコメント・書式・キー順・未知キーを保持する
    let text = "# 作品の設定\nschema_version = 2\ntitle = \"旧題\" # メモ\nmy_extension = { keep = true }\n";
    let edited = project_toml::edit(text, |document| {
        project_toml::set_string(document.as_table_mut(), "title", "新題");
    })
    .expect("編集できる");
    assert!(edited.contains("# 作品の設定"), "{edited}");
    assert!(edited.contains("# メモ"), "{edited}");
    assert!(
        edited.contains("my_extension = { keep = true }"),
        "{edited}"
    );
    assert!(edited.contains("title = \"新題\""), "{edited}");
    assert_eq!(project_toml::settings(&edited).title, "新題");
}

#[test]
fn broken_toml_is_never_overwritten() {
    // §4.5 の MUST: TOML として読めないファイルは edit が None を返す
    assert!(project_toml::edit("title = ", |_| {}).is_none());
}

#[test]
fn features_diagnostics_is_read_from_the_table() {
    let text = "[features.diagnostics]\nW083 = \"off\"\nwriting = \"error\"\ndefault = \"warn\"\n";
    let parsed = project_toml::parse(text);
    assert!(parsed.is_valid(), "{:?}", parsed.error);
    let map = &parsed.settings.features.diagnostics;
    assert_eq!(map.get("W083").map(String::as_str), Some("off"));
    assert_eq!(map.get("writing").map(String::as_str), Some("error"));
    assert_eq!(map.get("default").map(String::as_str), Some("warn"));
}

#[test]
fn set_diagnostic_level_writes_the_features_table_losslessly() {
    // §10.9.6: 既存キーは装飾を残して更新し、無ければコードキーを足す
    let source = "# 先頭コメント\nschema_version = 2\n\n[features.diagnostics]\nW012 = \"off\" # 行末コメント\n";
    let edited =
        project_toml::set_diagnostic_level(source, "W012", "error").expect("既存キーの更新");
    assert!(
        edited.contains("W012 = \"error\" # 行末コメント"),
        "{edited}"
    );
    assert!(edited.starts_with("# 先頭コメント"));

    let edited =
        project_toml::set_diagnostic_level(&edited, "W096", "warn").expect("新しいコードキー");
    let settings = project_toml::settings(&edited);
    assert_eq!(settings.features.diagnostics["W012"], "error");
    assert_eq!(settings.features.diagnostics["W096"], "warn");

    // テーブル自体が無い場合も作る。壊れた TOML は直さない
    let bare = "schema_version = 2\n";
    let edited = project_toml::set_diagnostic_level(bare, "W096", "off").expect("作る");
    assert_eq!(
        project_toml::settings(&edited).features.diagnostics["W096"],
        "off"
    );
    assert!(project_toml::set_diagnostic_level("title = ", "W096", "off").is_none());
}

#[test]
fn set_value_keeps_the_existing_comment_for_any_type() {
    // 文字列以外（真偽・配列）の差し替えでも行末コメントを落とさない（§4.5）
    let source = "[agents]\ncustom_roles = [\"a\"] # 役割\n\n[agents.local_llm]\nenabled = false # 自宅のみ\n";
    let edited = project_toml::edit(source, |document| {
        let agents = document["agents"].as_table_like_mut().expect("agents");
        let mut array = toml_edit::Array::new();
        array.push("b");
        project_toml::set_value(agents, "custom_roles", toml_edit::Value::Array(array));
        let local = agents
            .get_mut("local_llm")
            .and_then(toml_edit::Item::as_table_like_mut)
            .expect("local_llm");
        project_toml::set_value(local, "enabled", toml_edit::Value::from(true));
    })
    .expect("編集できる");
    assert!(edited.contains("custom_roles = [\"b\"] # 役割"), "{edited}");
    assert!(edited.contains("enabled = true # 自宅のみ"), "{edited}");
}
