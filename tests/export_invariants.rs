//! エクスポートの不変条件（§17.1「不変条件（MUST）」、M9）。
//!
//! 「サンプルと実プロジェクトで検証する」: 雛形（`novnow new` が作るサンプル、§14.5）・仕様書の例
//! （`tests/world`）・実プロジェクト（`tests/novel`）の 3 つを、§4.5 の `export` の全組み合わせ
//! （区切り 3 種 × ルビ × 見出し × 字下げ × 改行 × 形式）と、範囲・状態の絞り込みで書き出し、
//! 結果に次が残っていないことを確かめる。
//!
//! 1. `@` で始まる行
//! 2. `;` だけの行
//! 3. `'` のシジル（参照の始まりになれる `'`。英語のアポストロフィ `don't` は残ってよい）
//! 4. `//`（コメントの始まりになれるもの。URL の `://` は §5.0.2 でコメントではない）
//!
//! 5〜7（`# ` 行・`! ` 行・`^M`）の検査は ② の追加仕様（`docs/history.md` の作業項目 1）。
//! エスケープで書いた文字（`\@` `\'` `\;` `\/`）は作家が書いたリテラルなので、ここでの入力には
//! 行頭の `\@` と、参照の位置の `\'` を置かない（`tests/export.rs` が別に見る）。

mod novel;
mod world;

use novnow::adapters::project_load::is_document;
use novnow::core::export::{self, parse_range, select, Break, Format, Options, State};
use novnow::core::features::Project;
use novnow::core::model::document::Document;
use novnow::core::model::project::Newline;
use novnow::core::policy::ProjectPath;
use novnow::core::template;

/// 雛形（`novnow new` のサンプル）。
fn sample() -> Project {
    let documents = template::new_project("星屑の時計")
        .into_iter()
        .filter(|file| is_document(&ProjectPath::new(&file.path).expect("path")))
        .map(|file| Document::new(file.path, file.text));
    Project::with_documents(Default::default(), documents)
}

fn projects() -> Vec<(&'static str, Project)> {
    vec![
        ("サンプル", sample()),
        ("仕様書の例", world::world()),
        ("実プロジェクト", novel::novel()),
    ]
}

/// §4.5 の `export` の全組み合わせ。
fn every_option() -> Vec<Options> {
    let mut out = Vec::new();
    for paragraph_break in [Break::BlankLine, Break::None, Break::Separator] {
        for keep_ruby in [true, false] {
            for insert_headings in [true, false] {
                for indent_first_line in [true, false] {
                    for newline in [Newline::Crlf, Newline::Lf] {
                        for format in [Format::Text, Format::Markdown] {
                            out.push(Options {
                                paragraph_break,
                                indent_first_line,
                                keep_ruby,
                                insert_headings,
                                newline,
                                format,
                                state: None,
                                ..Options::default()
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

/// 不変条件 1〜4 に反する行（`(行番号, 理由, 行)`）。
fn violations(text: &str) -> Vec<(usize, &'static str, String)> {
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_matches(|c: char| c.is_whitespace());
        if trimmed.starts_with('@') {
            out.push((index, "@ で始まる行", line.to_owned()));
        }
        if trimmed == ";" {
            out.push((index, "; だけの行", line.to_owned()));
        }
        for (at, _) in line.match_indices('\'') {
            let after_word = line[..at]
                .chars()
                .next_back()
                .is_some_and(|prev| prev.is_ascii_alphanumeric());
            if !after_word {
                out.push((index, "' のシジル", line.to_owned()));
            }
        }
        for (at, _) in line.match_indices("//") {
            if !line[..at].ends_with(':') {
                out.push((index, "//", line.to_owned()));
            }
        }
    }
    out
}

#[test]
fn no_annotation_survives_any_option() {
    for (name, project) in projects() {
        for options in every_option() {
            let text = export::run(&project, None, &options);
            assert!(!text.is_empty(), "{name}: 何も出ていない {options:?}");
            let found = violations(&text);
            assert!(found.is_empty(), "{name} {options:?}: {found:#?}\n{text}");
            // 改行は指定どおりで、末尾は改行 1 個
            let newline = options.newline.text();
            assert!(text.ends_with(newline), "{name} {options:?}");
            assert!(!text.ends_with(&newline.repeat(2)), "{name} {options:?}");
            if options.newline == Newline::Lf {
                assert!(!text.contains('\r'), "{name} {options:?}");
            } else {
                assert!(!text.replace("\r\n", "").contains(['\r', '\n']), "{name}");
            }
        }
    }
}

#[test]
fn no_annotation_survives_any_range_or_state() {
    let project = novel::novel();
    let ranges = [
        None,
        Some("@01"),
        Some("@01-01"),
        Some("@01-01-01"),
        Some("@01-01-01-03"),
        Some("@01-01-02 .. @02"),
        Some("@01-02-01-02"),
    ];
    for range in ranges {
        let parsed = range.map(|text| parse_range(text).expect(text));
        for state in [
            None,
            Some(State::Draft),
            Some(State::Review),
            Some(State::Fixed),
        ] {
            let options = Options {
                state,
                ..Options::from_settings(&project.settings().export)
            };
            let text = export::run(&project, parsed.as_ref(), &options);
            let found = violations(&text);
            assert!(found.is_empty(), "{range:?} {state:?}: {found:#?}\n{text}");
            if select(&project, parsed.as_ref(), state).is_empty() {
                assert_eq!(text, "", "{range:?} {state:?}");
            }
        }
    }
}

#[test]
fn the_checker_itself_catches_annotations() {
    // 検査の取りこぼしが無いこと（壊れた出力を渡せば必ず引っかかる）
    for (bad, reason) in [
        ("@01-01-01 見出し\n", "@ で始まる行"),
        ("本文\n  ;\n", "; だけの行"),
        ("「'アルス」\n", "' のシジル"),
        ("本文 // メモ\n", "//"),
    ] {
        let found = violations(bad);
        assert!(
            found.iter().any(|(_, why, _)| *why == reason),
            "{bad:?}: {found:?}"
        );
    }
    for good in [
        "don't stop\n",
        "https://example.jp/a\n",
        "メールは a@example.jp\n",
        "　　＊\n",
    ] {
        assert!(violations(good).is_empty(), "{good:?}");
    }
}
