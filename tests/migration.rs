//! Literal migration round trips and rejection boundaries.
use novnow::core::{export, features::Project, migration, model::document::Document};

#[test]
fn migration_has_one_section_and_exports_literals() {
    for text in [
        "@heading\n;\n'name\n// memo\n/* inline */\n# memo\n! memo\n^M0137\n",
        "「Hello」@speaker\nhttps://example.com\n\\@literal\n｜字《じ》\n",
        "  indented\n\nbody\n",
    ] {
        let files = migration::convert("title", text).unwrap();
        let contents = files
            .iter()
            .find(|f| f.path == "contents/01-01.cnov")
            .unwrap();
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
fn migration_normalizes_bom_crlf_and_rejects_empty_or_nul() {
    assert_eq!(
        migration::convert("x", "\u{feff}\r\n ").unwrap_err(),
        migration::MigrationError::Empty
    );
    assert_eq!(
        migration::convert("x", "a\0b").unwrap_err(),
        migration::MigrationError::Nul
    );
    let files = migration::convert("x", "\u{feff}body\r\n").unwrap();
    assert_eq!(
        files
            .iter()
            .find(|f| f.path == "contents/01-01.cnov")
            .unwrap()
            .text,
        "@01-01-01\nbody\n"
    );
}
