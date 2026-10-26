//! `project.json` の読み込みと検査（`core::syntax::project_json`、仕様 第 2.9 版）。
//!
//! - **E025**: JSON として読めない → 既定値で続行（§4.3-4）。
//! - **E027**: `lexicon.word_classes` の正規表現が不正（§6.5.1、`docs/history.md` A-24・B-88）。
//! - 設定の読み込み: `features::Project` は `project.json` の文書の中身を設定にする（B-89）。

use novnow::core::diagnostics::{self, Arg, DiagnosticCode};
use novnow::core::features::Project;
use novnow::core::model::document::Document;
use novnow::core::model::project::Project as Settings;
use novnow::core::resolve::role_language::MatcherSet;
use novnow::core::syntax::project_json;

/// 旧形式 `project.json` の例（従来の仕様 §4.5 の例をそのまま埋め込む）。
///
/// Issue #29 の仕様更新で §4.5 は `project.toml`（`schema_version = 2`）へ移行したが、
/// 実装の追随は別バッチ（B10）である。ここでは現行の JSON パーサを守るため、旧形式の例を
/// テスト内に固定し、B10 で §4.5 の TOML の例を仕様書から読む形へ戻す。
fn spec_example() -> &'static str {
    r#"{
  "schema_version": 1,
  "title": "星屑の時計",
  "author": "",
  "content_encoding": "utf-8",
  "newline": "lf",
  "structure": {
    "scene_id_format": "A-B-C-D",
    "chapter_digits": 2,
    "episode_digits": 2,
    "scene_digits": 2,
    "section_digits": 2,
    "allow_chapter_zero": true
  },
  "writing": {
    "section_target_chars": 400,
    "section_warn_chars": 1200,
    "episode_target_chars": 6000,
    "manuscript_page_chars": 400,
    "max_consecutive_speeches": 3,
    "dialogue_ratio_target": 0.3,
    "long_sentence_chars": 120,
    "particle_run_threshold": 4
  },
  "lexicon": {
    "word_classes": {
      "体言": "[\\p{Han}\\p{Katakana}\\p{Hiragana}A-Za-z0-9ー]{1,24}"
    }
  },
  "export": {
    "paragraph_break": "blank-line",
    "indent_first_line": false,
    "keep_ruby": true,
    "insert_headings": false,
    "include_blurb": false,
    "newline": "crlf"
  },
  "reader": { "columns": 18, "chars_per_column": 42, "ruby": true },
  "agents": {
    "enabled": true,
    "delegation": "propose",
    "roles": {},
    "limits": { "max_tasks_per_session": 100, "max_chain_hops": 10, "max_input_chars_per_session": 2000000 },
    "max_chars_per_request": 12000,
    "integrator": "human",
    "max_parallel": 2,
    "custom_roles": [],
    "local_llm": { "enabled": false, "url": "http://localhost:11434", "model": "" }
  }
}
"#
}

/// §4.5 の例の `体言` を `value` に差し替えたもの。
fn with_word_class(value: &str) -> String {
    let original = r#""体言": "[\\p{Han}\\p{Katakana}\\p{Hiragana}A-Za-z0-9ー]{1,24}""#;
    let example = spec_example();
    assert!(example.contains(original), "§4.5 の例が変わった");
    example.replace(original, &format!(r#""体言": {value}"#))
}

#[test]
fn the_spec_example_reads_without_diagnostics() {
    let parsed = project_json::parse(spec_example());
    assert!(parsed.is_valid());
    assert_eq!(parsed.diagnostics, vec![]);
    assert_eq!(parsed.settings.title, "星屑の時計");
    assert_eq!(parsed.settings.lexicon.word_classes.len(), 1);
    // §4.5 の `reader` は v0.1.0 本体では読まない（未知キーとして温存）
    assert!(parsed.settings.unknown.contains_key("reader"));
}

#[test]
fn broken_json_is_e025_over_the_whole_file_and_falls_back_to_defaults() {
    let text = "{ \"title\": \"星屑の時計\", ";
    let parsed = project_json::parse(text);
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
    // serde の型が合わない値も「JSON として読み込めない」（設定として使えない）
    let parsed = project_json::parse(r#"{ "schema_version": "1" }"#);
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(parsed.diagnostics[0].code, DiagnosticCode::E025);
    assert_eq!(parsed.settings, Settings::default());
}

#[test]
fn a_broken_word_class_is_e027_on_its_value_literal() {
    let text = with_word_class(r#""[""#);
    let parsed = project_json::parse(&text);
    assert!(parsed.is_valid(), "JSON としては読める");
    assert_eq!(parsed.diagnostics.len(), 1);
    let found = &parsed.diagnostics[0];
    assert_eq!(found.code, DiagnosticCode::E027);
    assert_eq!(found.args, vec![Arg::text("体言")]);
    let range = found.range.clone().expect("位置");
    assert_eq!(&text[range], r#""[""#);
    // 設定は読めているので、壊れた値もそのまま持つ（原文は書き換えない）
    assert_eq!(parsed.settings.lexicon.word_classes["体言"], "[");
}

#[test]
fn only_patterns_using_the_broken_class_are_dropped() {
    // §6.5.1（第 2.9 版）: 他のパターンは生かす
    let settings = project_json::settings(&with_word_class(r#""(""#));
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
    let text = r#"{
  "schema_version": 1,
  "lexicon": { "word_classes": { "用言": "(", "体言": "[a-z]+", "敬称": "さん|様)" } }
}"#;
    let parsed = project_json::parse(text);
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
    assert_eq!(slices, vec![r#""さん|様)""#, r#""(""#]);
}

#[test]
fn an_escaped_value_falls_back_to_the_key() {
    // `\u005b` は `[`。serde の書き方と違うので値のリテラルは探せず、キーに倒す
    let text = r#"{ "lexicon": { "word_classes": { "体言": "\u005b" } } }"#;
    let parsed = project_json::parse(text);
    assert_eq!(parsed.diagnostics.len(), 1);
    let range = parsed.diagnostics[0].range.clone().expect("位置");
    assert_eq!(&text[range], r#""体言""#);
}

#[test]
fn the_same_name_before_word_classes_is_not_taken() {
    // `"体言"` が `word_classes` より前（タイトル）にも現れても、値の位置は word_classes の中
    let text = r#"{ "title": "体言", "lexicon": { "word_classes": { "体言": "[" } } }"#;
    let parsed = project_json::parse(text);
    let range = parsed.diagnostics[0].range.clone().expect("位置");
    assert_eq!(&text[range.clone()], r#""[""#);
    assert!(range.start > text.find("word_classes").expect("key"));
}

#[test]
fn a_bom_is_skipped_and_positions_count_it() {
    let text = format!("\u{feff}{}", with_word_class(r#""[""#));
    let parsed = project_json::parse(&text);
    assert!(parsed.is_valid());
    let range = parsed.diagnostics[0].range.clone().expect("位置");
    assert_eq!(&text[range], r#""[""#);
}

#[test]
fn the_aggregate_takes_its_settings_from_the_project_json_document() {
    let mut project = Project::default();
    let custom = r#"{ "schema_version": 1, "title": "題", "structure": { "chapter_digits": 3 } }"#;
    project.insert(Document::new("project.json", custom));
    assert_eq!(project.settings().title, "題");
    assert_eq!(project.settings().structure.chapter_digits, 3);
    // 壊れたら既定値（§4.3-4）
    project.insert(Document::new("project.json", "{"));
    assert_eq!(project.settings(), &Settings::default());
    // 戻すと読み直す
    project.insert(Document::new("project.json", custom));
    assert_eq!(project.settings().title, "題");
    // 文書を外すと既定値
    project.remove("project.json");
    assert_eq!(project.settings(), &Settings::default());
}

#[test]
fn with_documents_prefers_the_project_json_document() {
    let given = Settings {
        title: "引数".to_owned(),
        ..Settings::default()
    };
    let project = Project::with_documents(
        given,
        [Document::new("project.json", r#"{ "title": "文書" }"#)],
    );
    assert_eq!(project.settings().title, "文書");
}

#[test]
fn document_diagnostics_show_e025_and_e027_on_project_json() {
    let project = Project::default();
    let broken = Document::new("project.json", "{");
    let found = diagnostics::document(&broken, &project);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, DiagnosticCode::E025);

    let text = with_word_class(r#""[""#);
    let document = Document::new("project.json", text.clone());
    let found = diagnostics::document(&document, &project);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, DiagnosticCode::E027);
    assert_eq!(&text[found[0].range.clone().expect("位置")], r#""[""#);
    assert!(found[0].is_underlined());
}
