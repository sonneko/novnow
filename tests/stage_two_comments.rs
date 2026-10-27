//! Stage two #15: lossless parsing and annotation-free export.
use novnow::core::export::{self, Options};
use novnow::core::features::Project;
use novnow::core::model::document::Document;
use novnow::core::model::project::Newline;
use novnow::core::syntax::{contents, plot_episode, plot_outline};
use novnow::core::text::lexer::{self, LineKind, Sigil, TextFormat};

#[test]
fn hash_comments_are_txt_only_and_require_space() {
    for format in [
        TextFormat::Contents,
        TextFormat::PlotOutline,
        TextFormat::PlotEpisode,
    ] {
        for text in ["# memo", "#　memo", " # memo"] {
            assert!(matches!(
                lexer::classify(text, format),
                LineKind::Comment {
                    sigil: Sigil::HashComment,
                    ..
                }
            ));
        }
        for text in ["#1", "#", "\\# memo", "body # memo", "  # memo"] {
            assert!(!matches!(
                lexer::classify(text, format),
                LineKind::Comment { .. }
            ));
        }
    }
    assert!(matches!(
        lexer::classify("# heading", TextFormat::Markdown),
        LineKind::Text(_)
    ));
}

#[test]
fn hash_comments_preserve_the_entire_line_in_all_txt_parsers() {
    let text = "@01-01-01\n# memo // nested marker\nbody\n";
    let parsed = contents::parse(text);
    assert_eq!(parsed.to_text(), text);
    let contents::Line::Comment(comment) = &parsed.lines[1] else {
        panic!("comment")
    };
    assert_eq!(comment.comment.range, 0.."# memo // nested marker".len());
    assert_eq!(plot_episode::parse(text).to_text(), text);
    let outline = "@01 title\n# memo // nested marker\nsummary: body\n";
    assert_eq!(plot_outline::parse(outline).to_text(), outline);
}

#[test]
fn export_drops_hash_comment_lines_but_not_hash_number_or_inline_hash() {
    let project = Project::with_documents(
        Default::default(),
        [Document::new(
            "contents/01-01.cnov",
            "@01-01-01\n# memo\n#　memo // marker\nbody # inline\n#1\n",
        )],
    );
    let options = Options {
        newline: Newline::Lf,
        ..Default::default()
    };
    assert_eq!(export::run(&project, None, &options), "body # inline\n#1\n");
}
