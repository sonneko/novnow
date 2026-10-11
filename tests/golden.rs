//! ゴールデンテスト（I4、`PLANS.md` §6.3）。
//!
//! - 置き場所は `tests/golden/<形式>/<ケース>.txt` ＋ 同名の `<ケース>.expected.json`。
//!   `<形式>` のディレクトリ名は §3.3 のモジュール名（`contents` / `plot_outline` /
//!   `plot_episode` / `characters_md` / `words_md` / `ideas_md` / `materials_index` /
//!   `prompt_md`）。
//! - 入力は**仕様書のコード例をそのまま**書いたもの（§6.3「仕様が正本であることを機械的に担保する」）。
//! - 期待値 JSON は**手で書く**（自動生成した期待値は仕様の検証にならない）。
//! - 比較は `serde_json` だけ（§6.3「新しいクレートを足さない」。`insta` は入れない）。
//! - 失敗時は**実測の JSON をそのまま stderr に出す**ので、仕様書と突き合わせて手で貼り替える
//!   （「スナップショット更新コマンド」は持たない）。
//! - `.txt` だけあって `.expected.json` が無い（または逆）ケースは**テストを落とす**
//!   （片方だけ足して緑になるのを防ぐため）。
//! - `.expected.json` には**診断も入れる**（コード文字列と byte range）。§5.3 完了条件 3 の
//!   「壊れた入力でパニックせず、診断を返して部分木を作る」を 1 ファイルで固定できる。
//! - round-trip（§5.3 完了条件 2）は JSON には入れず、**全ケースで機械的に検査する**。
//!   `materials/index.json` は再シリアライズするとキー順と空白が変わるので原文をそのまま返す
//!   （`core::syntax::materials_index` の doc）。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use novnow::core::diagnostics::{Arg, Diagnostic};
use novnow::core::id::SceneId;
use novnow::core::syntax::{
    characters_md, contents, ideas_md, materials_index, plot_episode, plot_outline, prompt_md,
    words_md,
};
use novnow::core::text::lexer::FieldKey;

/// ゴールデンの置き場所（§6.3）。
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden");

/// `<形式>` のディレクトリ名（§3.3 のモジュール名に合わせる、§6.3）。
const FORMS: &[&str] = &[
    "contents",
    "plot_outline",
    "plot_episode",
    "characters_md",
    "words_md",
    "ideas_md",
    "materials_index",
    "prompt_md",
];

/// 期待値 JSON の拡張子（`<ケース>.txt` に対する `<ケース>.expected.json`）。
const EXPECTED: &str = "expected.json";

// ============================================================ テスト

#[test]
fn every_form_has_a_directory() {
    for form in FORMS {
        let dir = Path::new(ROOT).join(form);
        assert!(dir.is_dir(), "{} が無い（§6.3）", dir.display());
        assert!(!cases(form).is_empty(), "{} にケースが無い", form);
    }
}

#[test]
fn every_case_has_both_files() {
    // §6.3「`.txt` だけあって `.expected.json` が無い（または逆）ケースはテストを落とす」
    let mut problems = Vec::new();
    for form in FORMS {
        for entry in entries(form) {
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let is_case = path.extension().is_some_and(|found| found == "txt");
            let is_expected = name.ends_with(&format!(".{EXPECTED}"));
            if is_case && !expected_of(&path).is_file() {
                problems.push(format!("{form}/{name}: .expected.json が無い"));
            }
            if is_expected && !case_of(&path).is_file() {
                problems.push(format!("{form}/{name}: 対応する .txt が無い"));
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn cases_match_their_expected_json() {
    let mut failures = Vec::new();
    for form in FORMS {
        for path in cases(form) {
            if let Err(problem) = check(form, &path) {
                failures.push(problem);
            }
        }
    }
    assert!(failures.is_empty(), "\n\n{}", failures.join("\n\n"));
}

/// 1 ケースを検査する。round-trip（完了条件 2）は**必ず**見る。
fn check(form: &str, path: &Path) -> Result<(), String> {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let text = fs::read_to_string(path).map_err(|error| format!("{form}/{name}: {error}"))?;
    let expected_path = expected_of(path);
    let expected_text =
        fs::read_to_string(&expected_path).map_err(|error| format!("{form}/{name}: {error}"))?;
    let expected: Value = serde_json::from_str(&expected_text)
        .map_err(|error| format!("{form}/{name}: 期待値 JSON が読めない: {error}"))?;

    // §5.3 完了条件 2「未知フィールドキーを含むファイルを読んで書き戻したとき、
    // バイト列が完全に一致する」
    let round_trip = round_trip(form, &text);
    if round_trip != text {
        return Err(format!(
            "{form}/{name}: round-trip が一致しない\n--- 入力 ---\n{text:?}\n--- 出力 ---\n{:?}",
            round_trip
        ));
    }

    let actual = project(form, &text);
    if actual == expected {
        return Ok(());
    }
    // §6.3「失敗時は実測の JSON をそのまま stderr に出す」
    let pretty = serde_json::to_string_pretty(&actual).unwrap_or_default();
    eprintln!("--- {form}/{name} の実測 ---\n{pretty}\n---");
    Err(format!(
        "{form}/{name}: 期待値と実測が違う（実測は stderr に出した）\n期待: {}\n実測: {}",
        one_line(&expected),
        one_line(&actual)
    ))
}

/// 1 行に潰した JSON（差分のメッセージ用）。
fn one_line(found: &Value) -> String {
    serde_json::to_string(found).unwrap_or_default()
}

/// `form` のディレクトリの中身（順は不定。`cases` で並べ替える）。
fn entries(form: &str) -> Vec<fs::DirEntry> {
    let dir = Path::new(ROOT).join(form);
    let Ok(found) = fs::read_dir(dir) else {
        return Vec::new();
    };
    found.flatten().collect()
}

/// `form` のケース（`.txt`）を名前の順に返す（§6.3「ケースの発見は `std::fs::read_dir`」。
/// テストは core ではないので fs を使える。`tests/layering.rs` が既に同じことをしている）。
fn cases(form: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = entries(form)
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    found.sort();
    found
}

/// `<ケース>.txt` に対する `<ケース>.expected.json`。
fn expected_of(path: &Path) -> PathBuf {
    path.with_extension(EXPECTED)
}

/// `<ケース>.expected.json` に対する `<ケース>.txt`。
fn case_of(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let stem = name.strip_suffix(&format!(".{EXPECTED}")).unwrap_or(&name);
    path.with_file_name(format!("{stem}.txt"))
}

/// 読んで書き戻す（§5.3 完了条件 2）。
fn round_trip(form: &str, text: &str) -> String {
    match form {
        "contents" => contents::parse(text).to_text(),
        "plot_outline" => plot_outline::parse(text).to_text(),
        "plot_episode" => plot_episode::parse(text).to_text(),
        "characters_md" => characters_md::parse(text).to_text(),
        "words_md" => words_md::parse(text).to_text(),
        "ideas_md" => ideas_md::parse(text).to_text(),
        "prompt_md" => prompt_md::parse(text).to_text(),
        // JSON は再シリアライズするとキー順と空白が変わるので原文をそのまま返す
        "materials_index" => materials_index::parse(text).to_text().to_owned(),
        _ => String::new(),
    }
}

// ============================================================ 実測値の組み立て

/// `form` の木を期待値 JSON と同じ形に写す。
fn project(form: &str, text: &str) -> Value {
    let tree = match form {
        "contents" => contents_tree(text),
        "plot_outline" => plot_outline_tree(text),
        "plot_episode" => plot_episode_tree(text),
        "characters_md" => characters_tree(text),
        "words_md" => words_tree(text),
        "ideas_md" => ideas_tree(text),
        "materials_index" => materials_tree(text),
        "prompt_md" => prompt_tree(text),
        _ => Value::Null,
    };
    json!({ "form": form, "lines": text.split('\n').count(), "tree": tree })
}

fn contents_tree(text: &str) -> Value {
    let parsed = contents::parse(text);
    json!({
        "diagnostics": diagnostics(&parsed.diagnostics),
        "sections": parsed.sections.iter().map(|found| json!({
            "scene": found.scene.map(id_value),
            "number": found.number,
            "span": span(&found.span),
            "content": span(&found.content),
            "header": found.scene_header,
        })).collect::<Vec<Value>>(),
    })
}

fn plot_outline_tree(text: &str) -> Value {
    let parsed = plot_outline::parse(text);
    json!({
        "diagnostics": diagnostics(&parsed.diagnostics),
        "preamble": {
            "span": span(&parsed.preamble.span),
            "fields": keys(&parsed, &parsed.preamble.fields),
        },
        "blocks": parsed.blocks.iter().map(|found| json!({
            "id": found.id.map(id_value),
            "depth": found.depth,
            "span": span(&found.span),
            "end": found.end,
            "fields": keys(&parsed, &found.fields),
            "notes": found.notes.len(),
        })).collect::<Vec<Value>>(),
    })
}

fn plot_episode_tree(text: &str) -> Value {
    let parsed = plot_episode::parse(text);
    json!({
        "diagnostics": diagnostics(&parsed.diagnostics),
        "preamble": {
            "span": span(&parsed.preamble.span),
            "fields": episode_keys(&parsed, &parsed.preamble.fields),
        },
        "scenes": parsed.scenes.iter().map(|scene| json!({
            "id": scene.id.map(id_value),
            "depth": scene.depth,
            "span": span(&scene.span),
            "fields": episode_keys(&parsed, &scene.fields),
            "sections": scene.sections.iter().map(|found| json!({
                "number": found.number,
                "span": span(&found.span),
                "content": span(&found.content),
                "break_line": found.break_line,
                "fields": episode_keys(&parsed, &found.fields),
                "points": found.points.len(),
            })).collect::<Vec<Value>>(),
        })).collect::<Vec<Value>>(),
    })
}

fn characters_tree(text: &str) -> Value {
    let parsed = characters_md::parse(text);
    json!({
        "diagnostics": diagnostics(&parsed.diagnostics),
        "preamble": {
            "span": span(&parsed.preamble.span),
            "fields": md_keys(&parsed.lines, &parsed.preamble.fields),
        },
        "characters": parsed.characters.iter().map(|found| json!({
            "name": found.name,
            "span": span(&found.span),
            "fields": md_keys(&parsed.lines, &found.fields),
            "free": found.free.len(),
            "sections": found.sections.iter().map(|section| json!({
                "key": section.key,
                "key_text": section.key_text,
                "span": span(&section.span),
                "fields": md_keys(&parsed.lines, &section.fields),
                "bullets": section.bullets.len(),
                "patterns": patterns(&parsed.lines, &section.patterns),
                "free": section.free.len(),
                "relations": section.relations.iter().map(|relation| json!({
                    "name": relation.name,
                    "note": relation.note,
                })).collect::<Vec<Value>>(),
                "modes": section.modes.iter().map(|mode| json!({
                    "id": mode.id,
                    "label": mode.label,
                    "valid_id": mode.valid_id,
                    "span": span(&mode.span),
                    "fields": md_keys(&parsed.lines, &mode.fields),
                    "patterns": patterns(&parsed.lines, &mode.patterns),
                    "free": mode.free.len(),
                    "range": mode.range.as_ref().map(|found| found.text),
                })).collect::<Vec<Value>>(),
            })).collect::<Vec<Value>>(),
        })).collect::<Vec<Value>>(),
    })
}

fn words_tree(text: &str) -> Value {
    let parsed = words_md::parse(text);
    json!({
        "diagnostics": diagnostics(&parsed.diagnostics),
        "preamble": {
            "span": span(&parsed.preamble.span),
            "fields": md_keys(&parsed.lines, &parsed.preamble.fields),
        },
        "words": parsed.words.iter().map(|found| json!({
            "term": found.term,
            "span": span(&found.span),
            "fields": md_keys(&parsed.lines, &found.fields),
            "free": found.free.len(),
            "range": found.range.as_ref().map(|found| found.text),
        })).collect::<Vec<Value>>(),
    })
}

fn ideas_tree(text: &str) -> Value {
    let parsed = ideas_md::parse(text);
    json!({
        "diagnostics": diagnostics(&parsed.diagnostics),
        "preamble": {
            "span": span(&parsed.preamble.span),
            "fields": md_keys(&parsed.lines, &parsed.preamble.fields),
        },
        "ideas": parsed.ideas.iter().map(|found| json!({
            "id": found.id,
            "number": found.number,
            "valid_id": found.valid_id,
            "title": found.title,
            "span": span(&found.span),
            "fields": md_keys(&parsed.lines, &found.fields),
            "free": found.free.len(),
        })).collect::<Vec<Value>>(),
    })
}

fn materials_tree(text: &str) -> Value {
    let parsed = materials_index::parse(text);
    json!({
        "diagnostics": diagnostics(&parsed.diagnostics),
        "error": parsed.error,
        "index": parsed.index.as_ref().map(|found| serde_json::to_value(found).unwrap_or(Value::Null)),
    })
}

fn prompt_tree(text: &str) -> Value {
    let parsed = prompt_md::parse(text);
    json!({
        "diagnostics": diagnostics(&parsed.diagnostics),
        "role": parsed.role.as_ref().map(|found| json!({
            "name": found.name,
            "valid": found.valid,
            "builtin": found.builtin,
        })),
        "fields": md_keys(&parsed.lines, &parsed.fields),
        "sections": parsed.sections.iter().map(|found| json!({
            "key": found.key,
            "key_text": found.key_text,
            "span": span(&found.span),
            "fields": md_keys(&parsed.lines, &found.fields),
            "bullets": found.bullets.len(),
            "free": found.free.len(),
        })).collect::<Vec<Value>>(),
        "variables": parsed.variables.iter().map(|found| json!({
            "name": found.name,
            "line": found.line,
            "known": found.known,
        })).collect::<Vec<Value>>(),
    })
}

// ============================================================ 部品

fn diagnostics(found: &[Diagnostic]) -> Value {
    Value::Array(found.iter().map(diagnostic).collect())
}

/// 診断 1 件（§6.3「`.expected.json` には診断も入れる（コード文字列と byte range）」）。
fn diagnostic(found: &Diagnostic) -> Value {
    json!({
        "code": found.code.as_str(),
        "range": found.range.as_ref().map(span),
        "args": found.args.iter().map(arg).collect::<Vec<Value>>(),
    })
}

fn arg(found: &Arg) -> Value {
    match found {
        Arg::Text(value) => json!(value),
        Arg::Number(value) => json!(value),
    }
}

fn span(found: &std::ops::Range<usize>) -> Value {
    json!([found.start, found.end])
}

fn id_value(found: SceneId) -> Value {
    serde_json::to_value(found).unwrap_or(Value::Null)
}

/// A 形式のフィールドの**書かれたままのキー**（§5.0.6「保存時に正準形へ正規化しない」）。
fn keys(parsed: &plot_outline::Outline<'_>, indices: &[usize]) -> Value {
    let found: Vec<Value> = indices
        .iter()
        .filter_map(|&index| parsed.field(index))
        .map(|field| key_value(field.key, field.key_text))
        .collect();
    Value::Array(found)
}

/// B 形式のフィールドのキー。
fn episode_keys(parsed: &plot_episode::EpisodePlot<'_>, indices: &[usize]) -> Value {
    let found: Vec<Value> = indices
        .iter()
        .filter_map(|&index| parsed.field(index))
        .map(|field| key_value(field.key, field.key_text))
        .collect();
    Value::Array(found)
}

/// MD 方言のフィールドのキー。
fn md_keys(lines: &[characters_md::Line<'_>], indices: &[usize]) -> Value {
    let found: Vec<Value> = indices
        .iter()
        .filter_map(|&index| match lines.get(index) {
            Some(characters_md::Line::Field(field)) => Some(field),
            _ => None,
        })
        .map(|field| key_value(field.key, field.key_text))
        .collect();
    Value::Array(found)
}

/// キーは「表にあれば正準形、無ければ書かれたまま」（未知キーを区別できるように）。
fn key_value(key: FieldKey, key_text: &str) -> Value {
    match key.ascii_key() {
        Some(found) => json!(found),
        None => json!({ "unknown": key_text }),
    }
}

/// 役割語パターンは**パターン本体だけ**を並べる（§5.5.5。位置は unit テストが見る）。
fn patterns(lines: &[characters_md::Line<'_>], indices: &[usize]) -> Value {
    let found: Vec<Value> = indices
        .iter()
        .filter_map(|&index| match lines.get(index) {
            Some(characters_md::Line::Pattern(line)) => Some(line),
            _ => None,
        })
        .flat_map(|line| line.patterns.iter().map(|pattern| json!(pattern.text)))
        .collect();
    Value::Array(found)
}
