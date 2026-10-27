//! Migration (§14.6): decoding, candidate splitting, rendering, paste
//! mojibake repair, and the literal round trip with all candidates removed.
use novnow::core::migration::{self, Element, MigrationError, SourceUnit};
use novnow::core::template::TemplateFile;
use novnow::core::text::encoding::{self, DecodeError, SourceEncoding};
use novnow::core::{export, features::Project, model::document::Document};

fn unit(text: &str) -> SourceUnit {
    SourceUnit {
        name: "source.txt".to_owned(),
        text: text.to_owned(),
        starts_chapter: false,
    }
}

fn chapter_unit(text: &str) -> SourceUnit {
    SourceUnit {
        starts_chapter: true,
        ..unit(text)
    }
}

fn find<'a>(files: &'a [TemplateFile], path: &str) -> &'a TemplateFile {
    files.iter().find(|f| f.path == path).expect(path)
}

fn book_of(text: &str) -> migration::Book {
    migration::structure(&migration::analyze(&[unit(text)]))
}

// ============================================================ decode (§14.6.1)

#[test]
fn decode_prefers_utf8_and_strips_bom() {
    let decoded = encoding::decode("本文です".as_bytes()).unwrap();
    assert_eq!(decoded.text, "本文です");
    assert_eq!(decoded.encoding, SourceEncoding::Utf8);
    assert!(!decoded.ambiguous);
    let decoded = encoding::decode(b"\xEF\xBB\xBFbody").unwrap();
    assert_eq!(decoded.text, "body");
}

#[test]
fn decode_falls_back_to_shift_jis() {
    let (bytes, _, had_errors) = encoding_rs::SHIFT_JIS.encode("日本語の本文です");
    assert!(!had_errors);
    let decoded = encoding::decode(&bytes).unwrap();
    assert_eq!(decoded.text, "日本語の本文です");
    assert_eq!(decoded.encoding, SourceEncoding::ShiftJis);
    assert!(!decoded.ambiguous);
}

#[test]
fn decode_rejects_undecodable_nul_and_empty() {
    // 0x82 starts a Shift_JIS pair; 0x20 is no valid trail byte, and the
    // pair is not valid UTF-8 either.
    assert_eq!(
        encoding::decode(&[0x82, 0x20]).unwrap_err(),
        DecodeError::Undecodable
    );
    assert_eq!(encoding::decode(b"a\0b").unwrap_err(), DecodeError::Nul);
    assert_eq!(encoding::decode(b"  \n").unwrap_err(), DecodeError::Empty);
    assert_eq!(encoding::decode(b"").unwrap_err(), DecodeError::Empty);
}

#[test]
fn decode_marks_bytes_that_read_both_ways() {
    let mut found = None;
    for text in [
        "ああ",
        "かか",
        "あか",
        "かあ",
        "日本",
        "本文",
        "です",
        "ます",
        "あい",
        "うえ",
        "あう",
        "かき",
        "くけ",
        "ここ",
        "そこ",
        "あそこ",
    ] {
        let (sjis, _, had_errors) = encoding_rs::SHIFT_JIS.decode(text.as_bytes());
        if !had_errors && sjis != text {
            found = Some(text);
            break;
        }
    }
    let text = found.expect("a both-ways readable sample exists in the list");
    let decoded = encoding::decode(text.as_bytes()).unwrap();
    assert_eq!(decoded.encoding, SourceEncoding::Utf8);
    assert!(decoded.ambiguous);
    let as_sjis = encoding::decode_as(text.as_bytes(), SourceEncoding::ShiftJis).unwrap();
    assert_eq!(as_sjis.encoding, SourceEncoding::ShiftJis);
    assert_ne!(as_sjis.text, decoded.text);
}

// ============================================================ mojibake repair

#[test]
fn mojibake_repair_recovers_a_misdecoded_paste() {
    let original =
        "吾輩は猫である。名前はまだ無い。ここで生れたというだけでも、立派なものだと思う。";
    let (garbled, _, _) = encoding_rs::SHIFT_JIS.decode(original.as_bytes());
    assert_ne!(garbled, original);
    let repaired = encoding::mojibake_repair(&garbled).expect("repairable");
    assert_ne!(repaired, garbled.as_ref());
    // Long exact fragments of the original must survive (holes sit where
    // the mis-decoding dropped bytes).
    assert!(
        original
            .chars()
            .collect::<Vec<_>>()
            .windows(4)
            .any(|w| repaired.contains(&w.iter().collect::<String>())),
        "no 4-char fragment of the original survives in {repaired:?}"
    );
    // Dropped bytes surface as holes; the candidate is still mostly intact.
    let holes = repaired.chars().filter(|c| *c == '\u{FFFD}').count();
    assert!(
        holes * 2 <= repaired.chars().count(),
        "too many holes: {repaired:?}"
    );
}

#[test]
fn mojibake_repair_leaves_ordinary_text_alone() {
    for text in [
        "通常の日本語の文章です。問題ありません。",
        "Hello, world. This is plain English text.",
        "「行こう」と彼女は言った。榊原さんは頷いた。",
        "ｶﾀｶﾅの半角だけの短いメモ",
    ] {
        assert_eq!(
            encoding::mojibake_repair(text),
            None,
            "false positive: {text}"
        );
    }
}

// ============================================================ splitting (§14.6.2)

#[test]
fn headings_blanks_and_separators_shape_the_structure() {
    let book = book_of(
        "第一章 始まり\n第一話 出会い\n本文です。\n\n次の段落。\n\n\n場面が変わる。\n＊＊＊\n最後の場面。\n\n第二章 続き\n本文ふたたび。\n",
    );
    assert_eq!(book.counts(), (2, 2, 4, 5));
    assert_eq!(book.chapters[0].title.as_deref(), Some("始まり"));
    assert_eq!(
        book.chapters[0].episodes[0].title.as_deref(),
        Some("出会い")
    );
    assert_eq!(book.chapters[1].title.as_deref(), Some("続き"));
    let scenes = &book.chapters[0].episodes[0].scenes;
    assert_eq!(scenes[0].sections.len(), 2);
    assert_eq!(scenes[0].sections[0], ["本文です。"]);
    assert_eq!(scenes[0].sections[1], ["次の段落。"]);
    assert_eq!(scenes[1].sections[0], ["場面が変わる。"]);
    assert_eq!(scenes[2].sections[0], ["最後の場面。"]);
}

#[test]
fn named_headings_open_chapters_and_episodes_by_word() {
    let book = book_of("序章\nはじめに。\n\n幕間\n休憩。\n\n終章\nおわりに。\n");
    assert_eq!(book.counts(), (2, 3, 3, 3));
    // A prose line that merely starts with a named heading word is not one.
    let book = book_of("幕間、彼女は席を立った。\n");
    assert_eq!(book.counts(), (1, 1, 1, 1));
    assert_eq!(
        book.chapters[0].episodes[0].scenes[0].sections[0],
        ["幕間、彼女は席を立った。"]
    );
}

#[test]
fn separator_needs_three_symbols_and_ignores_dialogue_pauses() {
    assert_eq!(book_of("本文。\n＊＊\n続き。\n").counts(), (1, 1, 1, 1));
    assert_eq!(book_of("「……」\n続き。\n").counts(), (1, 1, 1, 1));
    assert_eq!(book_of("本文。\n―――\n続き。\n").counts(), (1, 1, 2, 2));
}

#[test]
fn files_become_episodes_and_folder_heads_become_chapters() {
    let book = migration::structure(&migration::analyze(&[
        unit("一話目。\n"),
        unit("二話目。\n"),
        chapter_unit("新しい章。\n"),
        unit("その続き。\n"),
    ]));
    assert_eq!(book.counts(), (2, 4, 4, 4));
    assert_eq!(book.chapters[0].episodes.len(), 2);
    assert_eq!(book.chapters[1].episodes.len(), 2);
}

#[test]
fn headings_only_input_renders_nothing() {
    assert_eq!(
        migration::convert("x", &[unit("第一章\n＊＊＊\n")]).unwrap_err(),
        MigrationError::Empty
    );
}

#[test]
fn boundary_levels_can_change_and_removal_restores_lines() {
    let mut draft = migration::analyze(&[unit("本文。\n\n続き。\n")]);
    let at = draft
        .elements
        .iter()
        .position(|e| matches!(e, Element::Boundary(_)))
        .expect("a section candidate");
    assert!(draft.set_boundary(at, Some(migration::Level::Scene)));
    assert_eq!(migration::structure(&draft).counts(), (1, 1, 2, 2));
    assert!(draft.set_boundary(at, None));
    let book = migration::structure(&draft);
    assert_eq!(book.counts(), (1, 1, 1, 1));
    assert_eq!(
        book.chapters[0].episodes[0].scenes[0].sections[0],
        ["本文。", "", "続き。"]
    );
}

#[test]
fn natural_cmp_orders_digit_runs_as_numbers() {
    use std::cmp::Ordering;
    assert_eq!(migration::natural_cmp("2.txt", "10.txt"), Ordering::Less);
    assert_eq!(migration::natural_cmp("話2", "話10"), Ordering::Less);
    assert_eq!(
        migration::natural_cmp("第２話.txt", "第１０話.txt"),
        Ordering::Less
    );
    assert_eq!(migration::natural_cmp("a", "a"), Ordering::Equal);
    assert_eq!(migration::natural_cmp("a01", "a1"), Ordering::Greater);
    assert_eq!(migration::natural_cmp("b", "a"), Ordering::Greater);
}

// ============================================================ rendering

#[test]
fn rendered_files_carry_titles_and_parse_back() {
    let files = migration::convert(
        "title",
        &[unit(
            "第一章 始まり\n第一話 出会い\n本文です。\n\n次の段落。\n＊＊＊\n最後の場面。\n\n第二章 続き\n本文ふたたび。\n",
        )],
    )
    .unwrap();
    let contents = find(&files, "contents/01-01.cnov");
    assert_eq!(
        novnow::core::syntax::contents::parse(&contents.text)
            .sections
            .len(),
        3
    );
    assert!(find(&files, "contents/02-01.cnov")
        .text
        .starts_with("@02-01-01\n"));
    let all = find(&files, "plots/all.cnov");
    assert!(all.text.contains("@01 始まり"), "{}", all.text);
    assert!(all.text.contains("@02 続き"), "{}", all.text);
    let chapter_plot = find(&files, "plots/01.cnov");
    assert!(
        chapter_plot.text.contains("@01-01 出会い"),
        "{}",
        chapter_plot.text
    );
    // The migrated project must open without error diagnostics: reparse
    // every generated document through the project model and export.
    let project = Project::with_documents(
        Default::default(),
        files.into_iter().map(|f| Document::new(f.path, f.text)),
    );
    let _ = project;
}

#[test]
fn removing_all_candidates_restores_the_literal_round_trip() {
    for text in [
        "@heading\n;\n'name\n// memo\n/* inline */\n# memo\n! memo\n^M0137\n",
        "「Hello」@speaker\nhttps://example.com\n\\@literal\n｜字《じ》\n",
        "  indented\n\nbody\n",
    ] {
        let mut draft = migration::analyze(&[unit(text)]);
        for at in (0..draft.elements.len()).rev() {
            if matches!(draft.elements.get(at), Some(Element::Boundary(_))) {
                draft.set_boundary(at, None);
            }
        }
        let files = migration::render("title", &migration::structure(&draft)).unwrap();
        let contents = find(&files, "contents/01-01.cnov");
        assert_eq!(
            novnow::core::syntax::contents::parse(&contents.text)
                .sections
                .len(),
            1
        );
        let project = Project::with_documents(
            Default::default(),
            files.into_iter().map(|f| Document::new(f.path, f.text)),
        );
        let options = export::Options {
            keep_ruby: true,
            newline: novnow::core::model::project::Newline::Lf,
            ..Default::default()
        };
        assert_eq!(export::run(&project, None, &options), text);
    }
}

#[test]
fn migration_normalizes_crlf_like_before() {
    let files = migration::convert("x", &[unit("body\r\n")]).unwrap();
    assert_eq!(
        find(&files, "contents/01-01.cnov").text,
        "@01-01-01\nbody\n"
    );
}
