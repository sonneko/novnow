//! キャッシュ（B13-4 / B13-5）が「毎回作り直した場合」と同じ結果になること。
//!
//! `Project` は節の並び・文字数・プロジェクト全体メタデータを版番号つきで覚えている
//! （`REF:docs/designs/overview.md` §3.5）。無効化の漏れは「古い値が残る」形でしか現れないので、
//! 編集の操作列を通した後で**作り直した値**と突き合わせる（`REF:docs/plans.md#perf-b13` §9.6）。

mod world;

use novnow::core::features::{metadata, sections, Project};
use novnow::core::model::document::Document;
use novnow::core::resolve::dictionary::SourceFile;

use world::*;

/// 編集を 1 回通した後のプロジェクト（本文の差し替え → 追加 → 削除）。
fn edited() -> Project {
    let mut project = world();
    let text = project
        .text(CONTENTS_PATH)
        .replace("図書室の攻防", "図書室の攻防（改）");
    project.insert(Document::new(CONTENTS_PATH, text));
    project.insert(Document::new(
        "contents/02-01.cnov",
        "@02-01-01 雨の教室\n「'アルス、帰ろう」@リリア\n;\n",
    ));
    project
}

/// キャッシュを通さずに節の並びを作る（`sections::of_document` を全ファイルに回す）。
fn uncached_sections(project: &Project) -> Vec<sections::SectionRef> {
    let mut out = Vec::new();
    for document in project.documents() {
        if document.path.starts_with("contents/") && document.path.ends_with(".cnov") {
            out.extend(sections::of_document(document, project));
        }
    }
    out
}

fn signature(item: &sections::SectionRef) -> String {
    format!(
        "{} {} {:?} {:?} {:?} {}",
        item.path,
        item.id.display(project_digits()),
        item.lines,
        item.span,
        item.header,
        item.scene_title
    )
}

fn project_digits() -> novnow::core::id::Digits {
    novnow::core::id::Digits::default()
}

#[test]
fn sections_match_a_full_rebuild_after_edits() {
    let project = edited();
    let cached: Vec<String> = sections::all(&project).iter().map(signature).collect();
    let fresh: Vec<String> = uncached_sections(&project).iter().map(signature).collect();
    assert_eq!(cached, fresh);
}

#[test]
fn section_chars_match_a_full_recount() {
    let project = edited();
    let cached = project.section_chars();
    for document in project.documents() {
        let Some(chars) = cached.get(&document.path) else {
            continue;
        };
        let counted: usize = sections::of_document(document, &project)
            .iter()
            .map(|item| sections::count_chars(&sections::text_of(&document.text, item)))
            .sum();
        assert_eq!(*chars, counted, "{}", document.path);
    }
}

#[test]
fn records_match_a_fresh_parse() {
    let project = edited();
    let cached = metadata::records(&project);
    let fresh = Project::with_documents(
        project.settings().clone(),
        project.documents().cloned().collect::<Vec<_>>(),
    );
    let fresh = metadata::records(&fresh);
    let render = |rows: &[metadata::Record]| -> Vec<String> {
        rows.iter()
            .map(|row| {
                format!(
                    "{} {} {:?} {:?}",
                    row.path,
                    row.id.display(project_digits()),
                    row.depth,
                    row.range
                )
            })
            .collect()
    };
    assert_eq!(render(&cached), render(&fresh));
}

#[test]
fn metadata_cache_follows_plot_edits() {
    let mut project = world();
    let before = metadata::records(&project).len();
    project.insert(Document::new(
        "plots/01-04.cnov",
        "@01-04-09 追加の場面\nwhen: 406 12:00\n本文。\n;\n",
    ));
    let after = metadata::records(&project);
    assert_ne!(after.len(), before);
    assert_eq!(
        after
            .iter()
            .filter(|row| row.path == "plots/01-04.cnov" && row.depth == 3)
            .count(),
        1
    );
}

#[test]
fn dictionary_index_follows_the_source_files() {
    let mut project = world();
    assert!(project.dictionary_diagnostics(SourceFile::Words).is_empty());
    // §6.2.1 の上限（`MAX_KEY_BYTES = 96`）を超えるキーだけが E028 になる
    let long = "あ".repeat(100);
    let mut words = String::new();
    for index in 0..3 {
        words.push_str(&format!(
            "# {long}{index}\nreading: ながすぎるご{index}\n\n"
        ));
    }
    project.insert(Document::new("words.md", words));
    assert!(!project.dictionary_diagnostics(SourceFile::Words).is_empty());
    assert!(project
        .dictionary_diagnostics(SourceFile::Characters)
        .is_empty());
    project.insert(Document::new("words.md", WORDS));
    assert!(project.dictionary_diagnostics(SourceFile::Words).is_empty());
}

#[test]
fn project_stays_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Project>();
}
