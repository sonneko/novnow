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
//!
//! M6 から: §12.7 の表の `cli.*` の行がカタログの写しであること、CLI のソースが使うキーが
//! 両方のカタログにあり、使われていない `cli.*` のキーが無いこと、実行時の `ui::i18n`
//! （読み込み・プレースホルダ・言語の決定 §12.4）が正しいこと。
//!
//! 第 2.11 版から: 診断の「どこ」（`place.*`、§14.2.5）も同じく、`ui::i18n` が使うキーが揃い、
//! 使われていないキーが無いこと。

mod sandbox;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use novnow::core::diagnostics::{Arg, Diagnostic, DiagnosticCode, CATALOG};
use novnow::ports::env::Os;
use novnow::ui::i18n::{self, Locale};

use sandbox::FakeEnv;

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
    assert_eq!(want.len(), 81);
    assert!(want.iter().all(|key| en.contains_key(key)));
}

#[test]
fn diag_messages_are_copied_verbatim_from_the_spec() {
    // 文言の正本は仕様書の §7 / §7.1 / §7.2（`PLANS.md` §6.3「仕様書 → 実装」）
    let ja = flatten(JA, "ja.toml");
    let en = flatten(EN, "en.toml");
    let rows = spec_rows();
    assert_eq!(rows.len(), 81);
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

// ============================================================ M6: CLI の文言と実行時の i18n

#[test]
fn the_cli_rows_of_the_spec_table_are_copied_verbatim() {
    // §12.7 の表（抜粋）のうち、実装済みの `cli.*` の行
    let start = SPEC.find("### 12.7 主要文言表").expect("§12.7");
    let end = start + SPEC[start..].find("\n---").expect("§12.7 の終わり");
    let ja = flatten(JA, "ja.toml");
    let en = flatten(EN, "en.toml");
    let mut seen = 0;
    for line in SPEC[start..end].lines() {
        let cells: Vec<&str> = line
            .trim()
            .trim_matches('|')
            .split(" | ")
            .map(str::trim)
            .collect();
        let [key, spec_ja, spec_en] = cells.as_slice() else {
            continue;
        };
        let key = key.trim_matches('`');
        // M7 で GUI の行（`status.*` `palette.*` `project.pick.*`）も実装した。
        // `agent.*`（M10）と `reader.*`（追加仕様 27）はまだ無い
        if !["cli.", "status.", "palette.", "project.pick."]
            .iter()
            .any(|prefix| key.starts_with(prefix))
        {
            continue;
        }
        assert_eq!(ja.get(key).map(String::as_str), Some(*spec_ja), "ja {key}");
        assert_eq!(en.get(key).map(String::as_str), Some(*spec_en), "en {key}");
        seen += 1;
    }
    assert_eq!(
        seen, 13,
        "§12.7 の cli.* / status.* / palette.* / project.pick.* の行の数"
    );
}

/// `files` の文字列リテラルに現れる `prefix…` のキー（`prefix` は `cli.` など）。
fn keys_in_source(prefix: &str, files: &[PathBuf]) -> BTreeSet<String> {
    let opening = format!("\"{prefix}");
    let mut keys = BTreeSet::new();
    for file in files {
        let source = std::fs::read_to_string(file).expect("read");
        let mut rest = source.as_str();
        while let Some(at) = rest.find(&opening) {
            let after = &rest[at + 1..];
            let end = after.find('"').expect("閉じる引用符");
            keys.insert(after[..end].to_owned());
            rest = &after[end..];
        }
    }
    keys
}

/// `dir` の下の `.rs` を全部。
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// `src/cli/**` と `src/main.rs` の文字列リテラルに現れる `cli.…` のキー。
fn cli_keys_in_source() -> BTreeSet<String> {
    let mut files = vec![PathBuf::from("src/main.rs")];
    rust_files(Path::new("src/cli"), &mut files);
    keys_in_source("cli.", &files)
}

/// カタログ（ja）の `prefix…` のキーのうち、`used` に無いもの。
fn unused_keys(prefix: &str, used: &BTreeSet<String>) -> Vec<String> {
    flatten(JA, "ja.toml")
        .into_keys()
        .filter(|key| key.starts_with(prefix) && !used.contains(key))
        .collect()
}

#[test]
fn every_cli_key_used_in_the_source_exists_and_none_is_unused() {
    let ja = flatten(JA, "ja.toml");
    let en = flatten(EN, "en.toml");
    let used = cli_keys_in_source();
    assert!(
        used.len() > 30,
        "キーが少なすぎる（検査が空振り？）: {used:?}"
    );
    for key in &used {
        assert!(ja.contains_key(key), "ja.toml に {key} が無い");
        assert!(en.contains_key(key), "en.toml に {key} が無い");
    }
    let unused = unused_keys("cli.", &used);
    assert!(unused.is_empty(), "使われていない cli.* のキー: {unused:?}");
}

/// GUI（M7）の文言の領域。`src/ui/**`（カタログ自身を除く）の文字列リテラルから引く。
const GUI_PREFIXES: &[&str] = &[
    "status.",
    "palette.",
    "project.",
    "header.",
    "editor.",
    "pane.",
    "jump.",
    "modal.",
    "command.",
    "help.",
    "find.",
    "search.",
    "delete.",
    "quickfix.",
    "register.",
    "diff.",
    "action.",
    "workspace.",
    "episode.",
    "outline.",
    "idea.",
    "review.",
    "cast.",
    "material.",
];

#[test]
fn every_gui_key_used_in_the_source_exists_and_none_is_unused() {
    let ja = flatten(JA, "ja.toml");
    let en = flatten(EN, "en.toml");
    let mut files = Vec::new();
    rust_files(Path::new("src/ui"), &mut files);
    files.retain(|file| !file.starts_with("src/ui/i18n"));
    let mut total = 0;
    for prefix in GUI_PREFIXES {
        let used = keys_in_source(prefix, &files);
        total += used.len();
        for key in &used {
            assert!(ja.contains_key(key), "ja.toml に {key} が無い");
            assert!(en.contains_key(key), "en.toml に {key} が無い");
        }
        // `project.pick.import`（Zip から読み込む）は §12.7 の表の写しで、Web のプロジェクト選択
        // （§13.4）が使う。Web の画面からの接続は M7 では入れていない（PLANS.md §9.2 B-118）
        let unused: Vec<String> = unused_keys(prefix, &used)
            .into_iter()
            .filter(|key| key != "project.pick.import")
            .collect();
        assert!(
            unused.is_empty(),
            "使われていない {prefix}* のキー: {unused:?}"
        );
    }
    assert!(total > 50, "キーが少なすぎる（検査が空振り？）: {total}");
}

#[test]
fn every_place_key_used_in_the_source_exists_and_none_is_unused() {
    let ja = flatten(JA, "ja.toml");
    let en = flatten(EN, "en.toml");
    let used = keys_in_source("place.", &[PathBuf::from("src/ui/i18n/mod.rs")]);
    assert!(used.len() >= 10, "キーが少なすぎる: {used:?}");
    for key in &used {
        assert!(ja.contains_key(key), "ja.toml に {key} が無い");
        assert!(en.contains_key(key), "en.toml に {key} が無い");
    }
    let unused = unused_keys("place.", &used);
    assert!(
        unused.is_empty(),
        "使われていない place.* のキー: {unused:?}"
    );
}

#[test]
fn the_runtime_catalog_is_the_same_as_the_files() {
    for (locale, text, label) in [(Locale::Ja, JA, "ja.toml"), (Locale::En, EN, "en.toml")] {
        let want = flatten(text, label);
        let catalog = i18n::catalog(locale);
        let keys: BTreeSet<&str> = catalog.keys().collect();
        let want_keys: BTreeSet<&str> = want.keys().map(String::as_str).collect();
        assert_eq!(keys, want_keys, "{label}");
        for (key, message) in &want {
            assert_eq!(catalog.get(key), Some(message.as_str()), "{label} {key}");
            assert_eq!(i18n::text(locale, key), message, "{label} {key}");
        }
    }
}

#[test]
fn placeholders_are_filled_once_and_positionally() {
    assert_eq!(i18n::fill("{0} と {1}", &["a", "b"]), "a と b");
    assert_eq!(i18n::fill("{1}{0}{1}", &["x", "y"]), "yxy");
    // 値の中の `{0}` は埋め直さない
    assert_eq!(i18n::fill("{0} / {1}", &["{1}", "b"]), "{1} / b");
    // 範囲外・数でない・閉じない `{` はそのまま
    assert_eq!(i18n::fill("{2}", &["a"]), "{2}");
    assert_eq!(i18n::fill("{name}", &["a"]), "{name}");
    assert_eq!(i18n::fill("{0", &["a"]), "{0");
    assert_eq!(
        i18n::format(Locale::Ja, "cli.error.unknown_option", &["-x"]),
        "不明なオプションです: -x"
    );
}

#[test]
fn diagnostics_are_rendered_in_both_languages() {
    let found = Diagnostic::new(
        DiagnosticCode::E004,
        Some(0..3),
        vec![
            Arg::text("恩田恵"),
            Arg::text("C"),
            Arg::text("@03-01-02-01"),
            Arg::text("@01 .. @02"),
        ],
    );
    assert_eq!(
        i18n::diagnostic(Locale::Ja, &found),
        "恩田恵 のモード C は @03-01-02-01 では使用できません（有効範囲: @01 .. @02）"
    );
    let counted = Diagnostic::new(
        DiagnosticCode::W030,
        Some(0..1),
        vec![Arg::number(3), Arg::number(4)],
    );
    assert_eq!(
        i18n::diagnostic(Locale::En, &counted),
        "Plot has 3 sections, prose has 4"
    );
}

#[test]
fn the_language_is_decided_in_the_order_of_section_12_4() {
    let env = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .fold(FakeEnv::new(Os::Linux), |env, (name, value)| {
                env.var(name, value)
            })
    };
    // 1. ui.json の language（ja / en だけ。system は飛ばす）
    assert_eq!(
        Locale::detect(Some("en"), &env(&[("NOVNOW_LANG", "ja")])),
        Locale::En
    );
    assert_eq!(
        Locale::detect(Some("system"), &env(&[("NOVNOW_LANG", "en")])),
        Locale::En
    );
    // 2. NOVNOW_LANG（ja… / en… 以外は無視して次へ）
    assert_eq!(
        Locale::detect(
            None,
            &env(&[("NOVNOW_LANG", "en"), ("LANG", "ja_JP.UTF-8")])
        ),
        Locale::En
    );
    assert_eq!(
        Locale::detect(None, &env(&[("NOVNOW_LANG", "ja_JP"), ("LANG", "en_US")])),
        Locale::Ja
    );
    assert_eq!(
        Locale::detect(
            None,
            &env(&[("NOVNOW_LANG", "fr"), ("LANG", "ja_JP.UTF-8")])
        ),
        Locale::Ja
    );
    // 3. OS のロケール（LC_ALL → LC_MESSAGES → LANG。ja* → ja、それ以外 → en）
    assert_eq!(
        Locale::detect(None, &env(&[("LANG", "ja_JP.UTF-8")])),
        Locale::Ja
    );
    assert_eq!(
        Locale::detect(None, &env(&[("LANG", "en_US.UTF-8")])),
        Locale::En
    );
    assert_eq!(Locale::detect(None, &env(&[("LANG", "C")])), Locale::En);
    assert_eq!(
        Locale::detect(None, &env(&[("LC_ALL", "ja_JP.UTF-8"), ("LANG", "en_US")])),
        Locale::Ja
    );
    assert_eq!(
        Locale::detect(None, &env(&[("LC_MESSAGES", "en_GB"), ("LANG", "ja_JP")])),
        Locale::En
    );
    // 4. 既定 ja
    assert_eq!(Locale::detect(None, &env(&[])), Locale::Ja);
    assert_eq!(Locale::from_os_locale("ja-JP"), Locale::Ja);
    assert_eq!(Locale::ALL.map(Locale::code), ["ja", "en"]);
}
