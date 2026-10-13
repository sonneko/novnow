//! 文言カタログ（`src/ui/i18n/{ja,en}.toml`）の CI 検査（`SPECIFICATIONS.md` §12.3）。
//!
//! §12.3「**CI チェック（MUST）**: 2 つのカタログのキー集合が完全一致しない、または
//! プレースホルダ番号集合が一致しない場合はビルド失敗」をここで担保する。
//!
//! GUI（`ui` feature）は M7 まで既定で切れている（`PLANS.md` §9.2 B-54）ので、カタログは
//! `include_str!` で読み、パースは dev-dependency の `toml` で行う。**既定の `cargo test` で走る**
//! （CI の Test ジョブがそのまま検査する）。
//!
//! あわせて `diag.*` が §7 / §7.1 / §7.2 の表の文言と**一字一句**一致すること、core の
//! [`DiagnosticCode`] の全コードが揃っていること、引数の数が [`DiagnosticCode::params`] と
//! 合うことも見る（文言の正本は仕様書。`PLANS.md` §5.5 の作業項目 1）。

use std::collections::{BTreeMap, BTreeSet};

use novnow::core::diagnostics::{DiagnosticCode, CATALOG};

const JA: &str = include_str!("../src/ui/i18n/ja.toml");
const EN: &str = include_str!("../src/ui/i18n/en.toml");
const SPEC: &str = include_str!("../SPECIFICATIONS.md");

/// TOML を `diag.E001` のような**ドット区切りのキー → 文言**に平らにする。
fn flatten(text: &str, label: &str) -> BTreeMap<String, String> {
    let table: toml::Table = text
        .parse()
        .unwrap_or_else(|error| panic!("{label} が TOML として読めない: {error}"));
    let mut out = BTreeMap::new();
    walk("", &toml::Value::Table(table), label, &mut out);
    out
}

fn walk(prefix: &str, value: &toml::Value, label: &str, out: &mut BTreeMap<String, String>) {
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                walk(&path, child, label, out);
            }
        }
        toml::Value::String(text) => {
            out.insert(prefix.to_string(), text.clone());
        }
        other => panic!("{label} の {prefix} が文字列でない: {other:?}"),
    }
}

/// `{0}` 形式のプレースホルダの番号（重複なし・昇順）。
fn placeholders(text: &str) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                if let Ok(index) = after[..close].parse::<usize>() {
                    out.insert(index);
                }
                rest = &after[close + 1..];
            }
            None => break,
        }
    }
    out
}

/// §7 / §7.1 / §7.2 の表の 1 行（コード, ja, en）。
fn spec_rows() -> Vec<(String, String, String)> {
    let start = SPEC.find("## 7. 診断カタログ").expect("§7");
    let end = start + SPEC[start..].find("\n## 8.").expect("§8");
    let mut rows = Vec::new();
    for line in SPEC[start..end].lines() {
        let cells: Vec<&str> = line
            .trim()
            .trim_matches('|')
            .split(" | ")
            .map(str::trim)
            .collect();
        let Some(code) = cells.first() else {
            continue;
        };
        let is_code = code.len() == 4
            && matches!(code.as_bytes()[0], b'E' | b'W' | b'I')
            && code[1..].bytes().all(|byte| byte.is_ascii_digit());
        if !is_code || cells.len() != 6 {
            continue;
        }
        let unquote = |cell: &str| cell.trim_matches('`').to_string();
        rows.push((code.to_string(), unquote(cells[3]), unquote(cells[4])));
    }
    rows
}

#[test]
fn both_catalogs_have_exactly_the_same_keys() {
    // §12.3 の CI チェック（MUST）その 1
    let ja: BTreeSet<String> = flatten(JA, "ja.toml").into_keys().collect();
    let en: BTreeSet<String> = flatten(EN, "en.toml").into_keys().collect();
    let only_ja: Vec<&String> = ja.difference(&en).collect();
    let only_en: Vec<&String> = en.difference(&ja).collect();
    assert!(only_ja.is_empty(), "ja.toml にだけある: {only_ja:?}");
    assert!(only_en.is_empty(), "en.toml にだけある: {only_en:?}");
}

#[test]
fn both_catalogs_use_the_same_placeholders_for_each_key() {
    // §12.3 の CI チェック（MUST）その 2
    let ja = flatten(JA, "ja.toml");
    let en = flatten(EN, "en.toml");
    for (key, text) in &ja {
        let Some(other) = en.get(key) else {
            continue;
        };
        assert_eq!(placeholders(text), placeholders(other), "{key}");
    }
}

#[test]
fn placeholders_are_positional_only() {
    // §12.3「位置引数のみ。名前付きは使わない」
    for (label, text) in [("ja.toml", JA), ("en.toml", EN)] {
        for (key, message) in flatten(text, label) {
            let mut rest = message.as_str();
            while let Some(open) = rest.find('{') {
                let after = &rest[open + 1..];
                let close = after.find('}').unwrap_or(after.len());
                let inside = &after[..close];
                assert!(
                    !inside.is_empty() && inside.bytes().all(|byte| byte.is_ascii_digit()),
                    "{label} の {key} に位置引数でないプレースホルダ: {message}"
                );
                rest = &after[close.min(after.len())..];
            }
        }
    }
}

#[test]
fn every_diagnostic_code_has_a_message_in_both_languages() {
    let ja = flatten(JA, "ja.toml");
    let en = flatten(EN, "en.toml");
    let want: BTreeSet<String> = CATALOG
        .iter()
        .map(|row| format!("diag.{}", row.0.as_str()))
        .collect();
    let have: BTreeSet<String> = ja
        .keys()
        .filter(|key| key.starts_with("diag."))
        .cloned()
        .collect();
    assert_eq!(have, want);
    assert_eq!(want.len(), 80);
    assert!(want.iter().all(|key| en.contains_key(key)));
}

#[test]
fn diag_messages_are_copied_verbatim_from_the_spec() {
    // 文言の正本は仕様書の §7 / §7.1 / §7.2（`PLANS.md` §6.3「仕様書 → 実装」）
    let ja = flatten(JA, "ja.toml");
    let en = flatten(EN, "en.toml");
    let rows = spec_rows();
    assert_eq!(rows.len(), 80);
    for (code, spec_ja, spec_en) in rows {
        let key = format!("diag.{code}");
        assert_eq!(ja.get(&key), Some(&spec_ja), "ja {key}");
        assert_eq!(en.get(&key), Some(&spec_en), "en {key}");
    }
}

#[test]
fn the_number_of_placeholders_matches_what_core_passes() {
    // core の `Diagnostic::new` は `params()` 個の引数を渡す（debug_assert）。文言側が
    // それより多くを要求すると表示で `{n}` が残り、少なければ引数が黙って捨てられる
    let ja = flatten(JA, "ja.toml");
    for row in CATALOG {
        let code: DiagnosticCode = row.0;
        let key = format!("diag.{}", code.as_str());
        let used = placeholders(ja.get(&key).expect("key"));
        let want: BTreeSet<usize> = (0..usize::from(code.params())).collect();
        assert_eq!(used, want, "{key}");
    }
}
