//! `project.json`（§4.5）の読み込みと検査（仕様 第 2.9 版。`docs/history.md` B-88・B-89）。
//!
//! 設定の読み込み（M5）はここを通る。`features::Project` は `project.json` の文書を受け取ると
//! [`parse`] の [`ProjectJson::settings`] を設定として使い、`diagnostics::document` は
//! [`ProjectJson::diagnostics`] を `project.json` の上に出す。
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **E025** | JSON として読めない（§4.3-4。型の合わない値を含む） | ファイル全体 |
//! | **E027** | `lexicon.word_classes` の正規表現が不正（§6.5.1、`docs/history.md` A-24） | その値の文字列リテラル |
//!
//! E025 のときは**既定値で続行する**（§4.3-4「既定値で開く」。P4: 作家の原稿を人質に取らない）。
//! 原文は 1 バイトも書き換えない。E027 の語クラスは、マッチャ（`resolve::role_language`）が
//! **そのクラスを使うパターンだけ**を捨てる（§6.5.1。他のパターンは生かす）。
//!
//! `.novnow/ui.json`（§4.4.1）と `.novnow/stats.json`（§4.4.2）は壊れていても**診断を出さない**
//! （どちらも仕様が「エラーを出さない」「統計の履歴が消えるだけ」と定める）ので、ここには無い。

use std::ops::Range;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::model::project::Project as Settings;
use crate::core::text::lexer;

/// `project.json` を読んだ結果。
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectJson {
    /// 設定。JSON として読めなければ**既定値**（[`Settings::default`]、§4.3-4）。
    pub settings: Settings,
    /// JSON として読めなかった理由（serde のメッセージ。E025 の `{0}` と同じ）。
    pub error: Option<String>,
    /// 診断（E025 / E027）。範囲は渡したテキストの byte。
    pub diagnostics: Vec<Diagnostic>,
}

impl ProjectJson {
    /// JSON として読めたか。
    pub fn is_valid(&self) -> bool {
        self.error.is_none()
    }
}

/// `project.json` のテキストを読む。**パニックしない**。
///
/// 先頭の BOM は読み飛ばす（§5.0.4。診断の位置は渡したテキストのまま数える）。
pub fn parse(text: &str) -> ProjectJson {
    let body = lexer::strip_bom(text);
    let offset = text.len() - body.len();
    match serde_json::from_str::<Settings>(body) {
        Ok(settings) => {
            let diagnostics = word_class_diagnostics(text, offset, &settings);
            ProjectJson {
                settings,
                error: None,
                diagnostics,
            }
        }
        Err(error) => {
            let message = error.to_string();
            ProjectJson {
                settings: Settings::default(),
                diagnostics: vec![Diagnostic::new(
                    DiagnosticCode::E025,
                    Some(0..text.len()),
                    vec![Arg::text(message.clone())],
                )],
                error: Some(message),
            }
        }
    }
}

/// 設定だけ（読めなければ既定値）。
pub fn settings(text: &str) -> Settings {
    parse(text).settings
}

/// E027（§6.5.1・第 2.9 版）: 正規表現として不正な語クラスごとに 1 件。
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

/// `lexicon.word_classes` の `name` の値の**文字列リテラル**の範囲（`"…"` を含む）。
///
/// serde は位置を持たないので、`docs/history.md` B-45（`materials/index.json`）と同じく
/// **原文からリテラルを探す**。`"word_classes"` より後ろで、キーのリテラル・`:`・値のリテラルが
/// 空白だけを挟んで並ぶ所を採る。見つからない書き方（`\u` エスケープ等）なら、キーのリテラル →
/// `"word_classes"` → ファイル全体の順に広げる（位置がずれても診断が消えるよりはよい）。
fn value_range(text: &str, offset: usize, name: &str, value: &str) -> Range<usize> {
    let whole = 0..text.len();
    let Some(classes) = text[offset..]
        .find("\"word_classes\"")
        .map(|at| at + offset)
    else {
        return whole;
    };
    let (Ok(key), Ok(literal)) = (serde_json::to_string(name), serde_json::to_string(value)) else {
        return classes..classes + "\"word_classes\"".len();
    };
    let mut key_range = None;
    for (at, _) in text[classes..].match_indices(&key) {
        let key_start = classes + at;
        let after_key = key_start + key.len();
        key_range.get_or_insert(key_start..after_key);
        let rest = &text[after_key..];
        let trimmed = rest.trim_start();
        let Some(after_colon) = trimmed.strip_prefix(':') else {
            continue;
        };
        let value_rest = after_colon.trim_start();
        if value_rest.starts_with(&literal) {
            let start = text.len() - value_rest.len();
            return start..start + literal.len();
        }
    }
    key_range.unwrap_or(classes..classes + "\"word_classes\"".len())
}
