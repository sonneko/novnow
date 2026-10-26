//! `project.toml`（§4.5）の読み込みと検査（Issue #29 B10 で JSON から移行）。
//!
//! 設定の読み込み（M5）はここを通る。`features::Project` は `project.toml` の文書を受け取ると
//! [`parse`] の [`ProjectToml::settings`] を設定として使い、`diagnostics::document` は
//! [`ProjectToml::diagnostics`] を `project.toml` の上に出す。
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **E025** | TOML として読めない（§4.3-4。型の合わない値を含む） | ファイル全体 |
//! | **E027** | `lexicon.word_classes` の正規表現が不正（§6.5.1、`docs/history.md` A-24） | その値の文字列リテラル |
//!
//! E025 のときは**既定値で続行する**（§4.3-4「既定値で開く」。P4: 作家の原稿を人質に取らない）。
//! 原文は 1 バイトも書き換えない。E027 の語クラスは、マッチャ（`resolve::role_language`）が
//! **そのクラスを使うパターンだけ**を捨てる（§6.5.1。他のパターンは生かす）。
//!
//! 書き戻しは **無損失編集**（§4.5）。アプリが設定を書き換えるときは [`edit`] で
//! `toml_edit` の文書を直に直し、コメント・書式・キー順・未知キーを保持する。
//! TOML として読めないファイルは [`edit`] が `None` を返し、呼び出し側は上書きしない（MUST）。
//!
//! `.novnow/ui.json`（§4.4.1）と `.novnow/stats.json`（§4.4.2）は壊れていても**診断を出さない**
//! （どちらも仕様が「エラーを出さない」「統計の履歴が消えるだけ」と定める）ので、ここには無い。

use std::ops::Range;

use serde::Deserialize;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::model::project::Project as Settings;
use crate::core::text::lexer;

/// `project.toml` を読んだ結果。
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectToml {
    /// 設定。TOML として読めなければ**既定値**（[`Settings::default`]、§4.3-4）。
    pub settings: Settings,
    /// TOML として読めなかった理由（serde / toml のメッセージ。E025 の `{0}` と同じ）。
    pub error: Option<String>,
    /// 診断（E025 / E027）。範囲は渡したテキストの byte。
    pub diagnostics: Vec<Diagnostic>,
}

impl ProjectToml {
    /// TOML として読めたか。
    pub fn is_valid(&self) -> bool {
        self.error.is_none()
    }
}

/// `project.toml` のテキストを読む。**パニックしない**。
///
/// 先頭の BOM は読み飛ばす（§5.0.4。診断の位置は渡したテキストのまま数える）。
/// 型付き検証は TOML の値を JSON の値へ写してから `serde` で行う（未知キーの保持に使う
/// `serde_json::Value` と形を揃えるため。日時は文字列として保持する）。
pub fn parse(text: &str) -> ProjectToml {
    let body = lexer::strip_bom(text);
    let offset = text.len() - body.len();
    // toml のエラー表示は複数行（位置の図入り）なので、1 行のメッセージだけを使う
    let parsed = toml::from_str::<toml::Value>(body)
        .map_err(|error| error.message().to_owned())
        .and_then(|value| {
            Settings::deserialize(to_json(&value)).map_err(|error| error.to_string())
        });
    match parsed {
        Ok(settings) => {
            let diagnostics = word_class_diagnostics(text, offset, &settings);
            ProjectToml {
                settings,
                error: None,
                diagnostics,
            }
        }
        Err(message) => ProjectToml {
            settings: Settings::default(),
            diagnostics: vec![Diagnostic::new(
                DiagnosticCode::E025,
                Some(0..text.len()),
                vec![Arg::text(message.clone())],
            )],
            error: Some(message),
        },
    }
}

/// 設定だけ（読めなければ既定値）。
pub fn settings(text: &str) -> Settings {
    parse(text).settings
}

/// TOML の値を JSON の値へ写す（日時は文字列、無限大・非数は 0）。
fn to_json(value: &toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(text) => serde_json::Value::String(text.clone()),
        toml::Value::Integer(number) => serde_json::Value::Number((*number).into()),
        toml::Value::Float(number) => serde_json::Number::from_f64(*number).map_or(
            serde_json::Value::Number(0.into()),
            serde_json::Value::Number,
        ),
        toml::Value::Boolean(flag) => serde_json::Value::Bool(*flag),
        toml::Value::Datetime(when) => serde_json::Value::String(when.to_string()),
        toml::Value::Array(items) => serde_json::Value::Array(items.iter().map(to_json).collect()),
        toml::Value::Table(table) => serde_json::Value::Object(
            table
                .iter()
                .map(|(key, item)| (key.clone(), to_json(item)))
                .collect(),
        ),
    }
}

/// 無損失編集（§4.5）: TOML 文書へ `apply` の変更だけを反映したテキストを返す。
/// TOML として読めなければ `None`（呼び出し側はファイルを上書きしてはならない）。
pub fn edit(text: &str, apply: impl FnOnce(&mut toml_edit::DocumentMut)) -> Option<String> {
    let mut document: toml_edit::DocumentMut = text.parse().ok()?;
    apply(&mut document);
    Some(document.to_string())
}

/// テーブルへ文字列のキーを書く。既存の値がある場合は、その前後の装飾
/// （空白と行末コメント）を保ったまま値だけを差し替える（§4.5 の無損失編集）。
pub fn set_string(table: &mut dyn toml_edit::TableLike, key: &str, value: &str) {
    if let Some(existing) = table.get_mut(key).and_then(|item| item.as_value_mut()) {
        let decor = existing.decor().clone();
        *existing = toml_edit::Value::from(value);
        *existing.decor_mut() = decor;
        return;
    }
    table.insert(key, toml_edit::value(value));
}

/// E027（§6.5.1）: 正規表現として不正な語クラスごとに 1 件。
fn word_class_diagnostics(text: &str, offset: usize, settings: &Settings) -> Vec<Diagnostic> {
    let lexicon = &settings.lexicon;
    lexicon
        .invalid_word_classes()
        .into_iter()
        .map(|name| {
            let value = lexicon.word_classes.get(name).map_or("", String::as_str);
            let range = value_range(text, offset, name, value);
            Diagnostic::new(DiagnosticCode::E027, Some(range), vec![Arg::text(name)])
        })
        .collect()
}

/// `[lexicon.word_classes]` の `name` の値の**文字列リテラル**の範囲（引用符を含む）。
///
/// serde は位置を持たないので、原文からリテラルを探す。`word_classes` より後ろで、
/// 値の文字列が基本文字列（`"…"`) かリテラル文字列（`'…'`) として現れる所を採る。
/// 見つからない書き方（複数行文字列等）なら `word_classes` の見出し → ファイル全体の順に広げる。
fn value_range(text: &str, offset: usize, _name: &str, value: &str) -> Range<usize> {
    let whole = 0..text.len();
    let Some(classes) = text[offset..].find("word_classes").map(|at| at + offset) else {
        return whole;
    };
    for quote in ['"', '\''] {
        let literal = format!("{quote}{value}{quote}");
        if let Some(at) = text[classes..].find(&literal) {
            let start = classes + at;
            return start..start + literal.len();
        }
    }
    classes..classes + "word_classes".len()
}
