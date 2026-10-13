//! `core::features`（§3.4 の 11 機能）のうち、文書を書き換えないもの:
//! 集約と参照の走査・semantic_tokens・inlay_hint・hover・definition・references・
//! document_symbol・folding と、§5.5 の完了条件「11 機能のシグネチャ」。
//!
//! 書き換えるもの（formatting・rename・completion・code_action）は `tests/features_edit.rs`。
//! フィクスチャは仕様書の例でできたプロジェクト（`tests/world/mod.rs`）。

mod world;

use std::fmt::Debug;
use std::ops::Range;

use serde::Serialize;

use novnow::core::diagnostics::{self, Diagnostic};
use novnow::core::features::code_action::{self, CodeAction};
use novnow::core::features::completion::{self, CompletionItem};
use novnow::core::features::definition;
use novnow::core::features::document_symbol::{self, DocumentSymbol, SymbolKind};
use novnow::core::features::folding::{self, FoldingRange};
use novnow::core::features::formatting;
use novnow::core::features::hover::{self, Hover, HoverContents};
use novnow::core::features::inlay_hint::{self, InlayHint, InlayKind};
use novnow::core::features::project::{self, Project};
use novnow::core::features::references;
use novnow::core::features::rename::{self, PrepareRename, RenameError};
use novnow::core::features::semantic_tokens::{self, SemanticToken, TokenType};
use novnow::core::features::{Location, TextEdit, WorkspaceEdit};
use novnow::core::id::SceneId;
use novnow::core::model::document::Document;
use novnow::core::model::material::MaterialStatus;
use novnow::core::model::project::Project as Settings;
use novnow::core::resolve::dictionary::EntryKind;
use novnow::core::text::lexer::TextFormat;

use world::*;

/// `text` の中で `needle` が `nth` 回目（0 始まり）に現れる byte 位置。
fn find(text: &str, needle: &str, nth: usize) -> usize {
    text.match_indices(needle)
        .nth(nth)
        .map(|(at, _)| at)
        .unwrap_or_else(|| panic!("{needle} が {nth} 回目に見つからない"))
}

// ============================================================ §5.5 完了条件（シグネチャ）

/// 戻り値は LSP の構造体に機械的に写せるプレーンな型（§3.4 MUST）: 複製・比較・直列化できる。
fn plain<T: Serialize + Clone + PartialEq + Debug>() {}

#[test]
fn the_eleven_features_have_the_signatures_of_section_3_4() {
    // §3.4 の表の引数の順（doc → 位置 → project）。`document_symbol` と `folding` は doc だけ
    let _: fn(&Document, &Project) -> Vec<SemanticToken> = semantic_tokens::full;
    let _: fn(&Document, usize, &Project) -> Vec<CompletionItem> = completion::at;
    let _: fn(&Document, usize, &Project) -> Option<Hover> = hover::at;
    let _: fn(&Document, Range<usize>, &Project) -> Vec<InlayHint> = inlay_hint::range;
    let _: fn(&Document, &Project) -> Vec<Diagnostic> = diagnostics::document;
    let _: fn(&Document, Range<usize>, &Project) -> Vec<CodeAction> = code_action::at;
    let _: fn(&Document, usize, &Project) -> Option<Location> = definition::at;
    let _: fn(&Document, usize, &Project) -> Vec<Location> = references::at;
    let _: fn(&Document) -> Vec<DocumentSymbol> = document_symbol::all;
    let _: fn(&Document, &Project) -> Vec<TextEdit> = formatting::document;
    let _: fn(&Document, usize, &Project) -> Option<PrepareRename> = rename::prepare;
    let _: fn(&Document, usize, &str, &Project) -> Result<WorkspaceEdit, RenameError> =
        rename::apply;
    let _: fn(&Document) -> Vec<FoldingRange> = folding::ranges;
    plain::<SemanticToken>();
    plain::<CompletionItem>();
    plain::<Hover>();
    plain::<InlayHint>();
    plain::<Diagnostic>();
    plain::<CodeAction>();
    plain::<Location>();
    plain::<DocumentSymbol>();
    plain::<TextEdit>();
    plain::<PrepareRename>();
    plain::<RenameError>();
    plain::<WorkspaceEdit>();
    plain::<FoldingRange>();
}

#[test]
fn no_feature_panics_at_any_byte_offset() {
    // UI から来た位置が文字の途中でもパニックしない（§4.3-4）。CJK は 1 字 3 byte なので
    // 文字境界でない位置を全部試す
    let project = world();
    for path in [CONTENTS_PATH, EPISODE_PATH, "characters.md"] {
        let document = doc(&project, path);
        for pos in 0..=document.text.len() + 2 {
            let _ = completion::at(&document, pos, &project);
            let _ = hover::at(&document, pos, &project);
            let _ = definition::at(&document, pos, &project);
            let _ = references::at(&document, pos, &project);
            let _ = rename::prepare(&document, pos, &project);
            let _ = rename::apply(&document, pos, "新しい名前", &project);
            let _ = inlay_hint::range(&document, pos..pos + 7, &project);
            let _ = code_action::at(&document, pos..pos + 7, &project);
        }
    }
}

// ============================================================ 集約と参照の走査

#[test]
fn paths_follow_the_digits_of_project_json() {
    let project = world();
    assert_eq!(project.contents_path(1, 4), "contents/01-04.txt");
    assert_eq!(project.episode_plot_path(1, 4), "plots/01-04.txt");
    assert_eq!(project.chapter_plot_path(3), "plots/03.txt");
    let mut settings = Settings::default();
    settings.structure.chapter_digits = 3;
    let wide = Project::new(settings);
    assert_eq!(wide.contents_path(1, 4), "contents/001-04.txt");
    assert_eq!(
        project::episode_of_path("contents/001-04.txt"),
        Some((1, 4))
    );
    assert_eq!(project::episode_of_path("plots/all.txt"), None);
    assert_eq!(project::chapter_of_path("plots/02.txt"), Some(2));
}

#[test]
fn the_dictionary_is_rebuilt_only_for_its_three_sources() {
    let mut project = world();
    let before = project.dictionary().len();
    project.insert(Document::new(CONTENTS_PATH, "「'ゼノ」"));
    assert_eq!(project.dictionary().len(), before);
    let characters = format!("{CHARACTERS}\n# ゼノ\n");
    project.insert(Document::new("characters.md", &characters));
    assert_eq!(project.dictionary().len(), before + 1);
    assert!(project.dictionary().lookup("ゼノ", None).is_some());
    project.remove("characters.md");
    assert!(project.dictionary().lookup("ゼノ", None).is_none());
}

#[test]
fn scan_ranges_cover_only_the_body_part_of_each_line() {
    let show = |raw: &str, format: TextFormat| -> Vec<String> {
        project::scan_ranges(raw, format)
            .iter()
            .map(|range| raw[range.clone()].to_string())
            .collect()
    };
    // フィールド行は値だけ（行末コメントは除く。B-36 で値に残るコメント）
    assert_eq!(
        show("mode: 'リリア:B+C   // 併用", TextFormat::PlotEpisode),
        vec!["'リリア:B+C   "]
    );
    // 見出し・節区切り・コメント行は走査しない
    assert!(show("@01-04-02 'リリア", TextFormat::Contents).is_empty());
    assert!(show(";", TextFormat::Contents).is_empty());
    assert!(show("// 'リリア", TextFormat::Contents).is_empty());
    // `*.md` の `#` 見出しは項目名そのもの
    assert!(show("# リリア", TextFormat::Markdown).is_empty());
    // 行内コメント `/* */` の中は除く
    assert_eq!(
        show("'アルス /* 'リリア */ が来た", TextFormat::Contents),
        vec!["'アルス ", " が来た"]
    );
}

#[test]
fn ref_sites_and_speakers_of_the_spec_body() {
    let project = world();
    let document = doc(&project, CONTENTS_PATH);
    let refs: Vec<String> = project
        .ref_sites(&document)
        .iter()
        .map(|site| slice(&document.text, &site.range()))
        .collect();
    assert_eq!(refs, vec!["'アルス", "'リリア", "'リリア"]);
    let speakers: Vec<(String, Vec<String>)> = project
        .speaker_sites(&document)
        .iter()
        .map(|site| {
            (
                slice(&document.text, &site.name_range),
                site.modes
                    .iter()
                    .map(|(_, range)| slice(&document.text, range))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        speakers,
        vec![
            ("リリア".to_string(), vec!["A".to_string()]),
            ("アルス".to_string(), vec![]),
            ("リリア".to_string(), vec!["B".to_string()]),
        ]
    );
    assert!(project
        .speaker_sites(&document)
        .iter()
        .all(|site| site.entry.is_some()));
}

#[test]
fn every_line_knows_its_section() {
    let project = world();
    let document = doc(&project, CONTENTS_PATH);
    let scene = |section| SceneId {
        chapter: 1,
        episode: 4,
        scene: 2,
        section,
    };
    assert_eq!(project.scene_at(&document, 1), Some(scene(1)));
    assert_eq!(project.scene_at(&document, 5), Some(scene(2)));
    assert_eq!(project.scene_at(&document, 9), Some(scene(3)));
}

// ============================================================ semantic_tokens（§6.3）

/// `種別:原文` を並べる（`role-language` は `+`）。
fn token_tags(text: &str, tokens: &[SemanticToken], line: Range<usize>) -> Vec<String> {
    tokens
        .iter()
        .filter(|token| token.range.start >= line.start && token.range.end <= line.end)
        .map(|token| {
            let mark = if token.role_language { "+" } else { "" };
            format!(
                "{}{mark}:{}",
                token.kind.as_str(),
                slice(text, &token.range)
            )
        })
        .collect()
}

fn line_range(text: &str, index: usize) -> Range<usize> {
    let start: usize = text
        .split('\n')
        .take(index)
        .map(|line| line.len() + 1)
        .sum();
    let len = text.split('\n').nth(index).map_or(0, str::len);
    start..start + len
}

#[test]
fn tokens_of_the_spec_body_follow_section_6_3() {
    let project = world();
    let document = doc(&project, CONTENTS_PATH);
    let tokens = semantic_tokens::full(&document, &project);
    let text = &document.text;
    assert_eq!(
        token_tags(text, &tokens, line_range(text, 0)),
        vec![
            "sigil.structure:@",
            "scene.id:01-04-02",
            "scene.title:図書室の攻防"
        ]
    );
    assert_eq!(
        token_tags(text, &tokens, line_range(text, 1)),
        vec![
            "dialogue.bracket:「",
            "dialogue.text:やっぱり",
            "sigil.ref:'",
            "ref.character:アルス",
            "dialogue.text:、ここにいた",
            // リリアの共通の tone「*のね」（tests/world）。下線だけで種別は台詞のまま
            "dialogue.text+:のね",
            "dialogue.bracket:」",
            "sigil.structure:@",
            "speaker:リリア",
            "modifier.mode::A",
        ]
    );
    assert_eq!(
        token_tags(text, &tokens, line_range(text, 4)),
        vec!["section.break:;"]
    );
    // 行末コメント（§5.0.2）はコメントだけ。中は何も色付けしない
    let last = token_tags(text, &tokens, line_range(text, 9));
    assert_eq!(
        last.last().map(String::as_str),
        Some("comment:// ここでモードが切り替わる")
    );
}

#[test]
fn tokens_never_overlap_and_are_sorted() {
    // LSP の semanticTokens/full は重なりを表せない（B-80）
    let project = world();
    for path in [
        CONTENTS_PATH,
        EPISODE_PATH,
        ALL_PATH,
        "characters.md",
        "words.md",
    ] {
        let document = doc(&project, path);
        let tokens = semantic_tokens::full(&document, &project);
        assert!(!tokens.is_empty(), "{path}");
        for pair in tokens.windows(2) {
            assert!(pair[0].range.end <= pair[1].range.start, "{path}: {pair:?}");
        }
        assert!(tokens
            .iter()
            .all(|token| token.range.start < token.range.end));
    }
}

#[test]
fn plot_tokens_mark_field_keys_and_references() {
    let project = world();
    let document = doc(&project, EPISODE_PATH);
    let tokens = semantic_tokens::full(&document, &project);
    let text = &document.text;
    assert_eq!(
        token_tags(text, &tokens, line_range(text, 2)),
        vec![
            "field.key:mode",
            "sigil.ref:'",
            "ref.character:アルス",
            "modifier.mode::A",
            "sigil.ref:'",
            "ref.character:リリア",
            "modifier.mode::A",
        ]
    );
    // 資料 ID（§6.6）は `ref.material`
    let material = find(text, "'M0137", 0);
    assert!(tokens
        .iter()
        .any(|token| token.kind == TokenType::RefMaterial && token.range.start == material + 1));
}

#[test]
fn unresolved_references_and_explicit_forms() {
    let text = "@01-04-02 x\n'だれか と '{星屑の時計}\n";
    let project = world_with(CONTENTS_PATH, text);
    let document = doc(&project, CONTENTS_PATH);
    let tokens = semantic_tokens::full(&document, &project);
    assert_eq!(
        token_tags(text, &tokens, line_range(text, 1)),
        vec![
            "sigil.ref:'",
            "ref.unresolved:だれか",
            "sigil.ref:'{",
            "ref.word:星屑の時計",
            "sigil.ref:}",
        ]
    );
}

#[test]
fn rubies_are_split_into_base_and_reading() {
    let text = "@01-04-02 x\n｜星屑《ほしくず》と時計《とけい》\n";
    let project = world_with(CONTENTS_PATH, text);
    let document = doc(&project, CONTENTS_PATH);
    let tokens = semantic_tokens::full(&document, &project);
    assert_eq!(
        token_tags(text, &tokens, line_range(text, 1)),
        vec![
            "ruby.base:星屑",
            "ruby.text:ほしくず",
            "ruby.base:時計",
            "ruby.text:とけい",
        ]
    );
}

#[test]
fn role_language_underlines_follow_the_speaker_rules_of_6_5_3() {
    // リリアの C は「*わよ」、共通の tone は「*のね」（tests/world の characters.md）
    let text = "@01-04-01 x\n「行くわよ」@リリア:C\n「付き合ってあげるわよ、いいのね」\n;\n「本当にいいのね」\n";
    let project = world_with(CONTENTS_PATH, text);
    let document = doc(&project, CONTENTS_PATH);
    let tokens = semantic_tokens::full(&document, &project);
    let marked = |line: usize| -> Vec<String> {
        tokens
            .iter()
            .filter(|token| token.role_language)
            .filter(|token| {
                let range = line_range(text, line);
                token.range.start >= range.start && token.range.end <= range.end
            })
            .map(|token| slice(text, &token.range))
            .collect()
    };
    // 行末の話者注釈（モード C）→ `*わよ` と tone の両方が候補
    assert_eq!(marked(1), vec!["わよ"]);
    // 注釈の無い行は直前の話者（リリア）の継続。ただしモード C は「その行のみ」（§6.4.3）
    // なので全体プロットの `cast: 'リリア:A` になり、`*わよ` は当たらない。tone は当たる
    assert_eq!(marked(2), vec!["のね"]);
    // `;` を跨いだら継続しない（B-81）。話プロットにこの場面は無く pov も無いので話者不明
    assert!(marked(4).is_empty());
    // 下線は色を変えない（種別は台詞のまま）
    assert!(tokens
        .iter()
        .filter(|token| token.role_language)
        .all(|token| token.kind == TokenType::DialogueText));
}

#[test]
fn the_plot_pov_is_the_last_resort_for_the_speaker() {
    // 話プロットの 01-04-02 は `pov: 'アルス`。アルスの tone は「*だ」
    let text = "@01-04-02 x\n「これは設計図だ」\n";
    let project = world_with(CONTENTS_PATH, text);
    let document = doc(&project, CONTENTS_PATH);
    let marked: Vec<String> = semantic_tokens::full(&document, &project)
        .iter()
        .filter(|token| token.role_language)
        .map(|token| slice(text, &token.range))
        .collect();
    assert_eq!(marked, vec!["だ"]);
}

#[test]
fn every_token_type_has_the_name_of_section_6_3() {
    let names: Vec<&str> = TokenType::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(names.len(), 21);
    assert!(names.contains(&"dialogue.text"));
    assert!(names.contains(&"ref.unresolved"));
    assert_eq!(semantic_tokens::ROLE_LANGUAGE, "role-language");
}

// ============================================================ inlay_hint（§6.6.2）

#[test]
fn material_titles_and_mode_labels_are_hinted() {
    let project = world();
    let document = doc(&project, EPISODE_PATH);
    let text = &document.text;
    let hints = inlay_hint::range(&document, 0..text.len(), &project);
    let material_end = find(text, "'M0137", 0) + "'M0137".len();
    assert!(hints.contains(&InlayHint {
        at: material_end,
        kind: InlayKind::MaterialTitle {
            title: "科学の甲子園 本戦の運営と実技課題".to_string(),
            truncated: false,
            status: Some(MaterialStatus::Approved),
        },
    }));
    // `mode: 'リリア:B+C` → 2 つの表示名（書かれた順）
    let combined = find(text, "'リリア:B+C", 0) + "'リリア:B+C".len();
    assert!(hints.contains(&InlayHint {
        at: combined,
        kind: InlayKind::ModeLabels {
            labels: vec!["呆れ（強）".to_string(), "好奇心".to_string()],
        },
    }));
}

#[test]
fn speaker_annotations_get_mode_labels_too() {
    let project = world();
    let document = doc(&project, CONTENTS_PATH);
    let text = &document.text;
    let hints = inlay_hint::range(&document, 0..text.len(), &project);
    let labels: Vec<(String, Vec<String>)> = hints
        .iter()
        .filter_map(|hint| match &hint.kind {
            InlayKind::ModeLabels { labels } => Some((
                text[..hint.at].chars().rev().take(2).collect(),
                labels.clone(),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        labels,
        vec![
            (":A".chars().rev().collect(), vec!["呆れ".to_string()]),
            (":B".chars().rev().collect(), vec!["呆れ（強）".to_string()]),
        ]
    );
}

#[test]
fn hints_outside_the_range_are_left_out() {
    let project = world();
    let document = doc(&project, CONTENTS_PATH);
    let first = line_range(&document.text, 1);
    let hints = inlay_hint::range(&document, first, &project);
    assert_eq!(hints.len(), 1);
}

// ============================================================ hover

#[test]
fn hovering_a_character_shows_its_modes_for_this_section() {
    let project = world();
    let document = doc(&project, CONTENTS_PATH);
    let at = find(&document.text, "'アルス", 0) + 2;
    let found = hover::at(&document, at, &project).expect("hover");
    assert_eq!(slice(&document.text, &found.range), "'アルス");
    let HoverContents::Character {
        name,
        reading,
        aliases,
        modes,
        selected,
        others,
        ..
    } = found.contents
    else {
        panic!("{:?}", found.contents);
    };
    assert_eq!(name, "アルス");
    assert_eq!(reading.as_deref(), Some("あるす"));
    assert_eq!(aliases, vec!["錬金少年"]);
    // 1 章では B（`range: @02 ..`）は使えない
    let usable: Vec<(String, bool)> = modes
        .iter()
        .map(|mode| (format!("{} {}", mode.id, mode.label), mode.available))
        .collect();
    assert_eq!(
        usable,
        vec![("A 探究".to_string(), true), ("B 補習".to_string(), false)]
    );
    assert!(selected.is_empty());
    assert!(others.is_empty());
}

#[test]
fn hovering_a_speaker_shows_the_selected_modes() {
    let project = world();
    let document = doc(&project, CONTENTS_PATH);
    let at = find(&document.text, "@リリア:A", 0) + 1;
    let found = hover::at(&document, at, &project).expect("hover");
    let HoverContents::Character { selected, .. } = found.contents else {
        panic!("not a character");
    };
    assert_eq!(selected, vec!["A"]);
}

#[test]
fn hovering_words_and_materials() {
    let project = world();
    let document = doc(&project, EPISODE_PATH);
    let word = find(&document.text, "'星屑の時計", 0) + 1;
    let HoverContents::Word {
        term,
        category,
        aliases,
        definition,
        ..
    } = hover::at(&document, word, &project).expect("word").contents
    else {
        panic!("not a word");
    };
    assert_eq!(term, "星屑の時計");
    assert_eq!(category.as_deref(), Some("魔導具"));
    assert_eq!(aliases, vec!["星屑時計"]);
    assert!(definition.is_some_and(|text| text.starts_with("旧校舎の地下遺跡")));
    let material = find(&document.text, "'M0137", 0) + 1;
    let HoverContents::Material { id, status, .. } = hover::at(&document, material, &project)
        .expect("material")
        .contents
    else {
        panic!("not a material");
    };
    assert_eq!(id, "M0137");
    assert_eq!(status, Some(MaterialStatus::Approved));
}

#[test]
fn hovering_nothing_or_an_unresolved_reference_is_none() {
    let project = world_with(CONTENTS_PATH, "@01-04-02 x\n地の文 'だれか\n");
    let document = doc(&project, CONTENTS_PATH);
    assert!(hover::at(&document, find(&document.text, "地", 0), &project).is_none());
    assert!(hover::at(&document, find(&document.text, "'だれか", 0) + 1, &project).is_none());
}

#[test]
fn hovering_shows_other_kinds_with_the_same_key() {
    // §6.1.1「参照ペインに『別種別の同名項目あり』を併記」
    let words = format!("{WORDS}\n# リリア\ndefinition: 同名の花。\n");
    let project = world_with("words.md", &words);
    let document = doc(&project, CONTENTS_PATH);
    let at = find(&document.text, "'リリア", 0) + 1;
    let HoverContents::Character { others, .. } =
        hover::at(&document, at, &project).expect("hover").contents
    else {
        panic!("not a character");
    };
    assert_eq!(others, vec![EntryKind::Word]);
}

// ============================================================ definition / references

#[test]
fn definitions_point_at_the_heading_name() {
    let project = world();
    let body = doc(&project, CONTENTS_PATH);
    let at = |needle: &str, nth: usize| find(&body.text, needle, nth) + 1;
    let found = definition::at(&body, at("'リリア", 0), &project).expect("definition");
    assert_eq!(found.path, "characters.md");
    assert_eq!(slice(CHARACTERS, &found.range), "リリア");
    // 話者注釈からも
    let speaker = definition::at(&body, at("@アルス", 0), &project).expect("speaker");
    assert_eq!(slice(CHARACTERS, &speaker.range), "アルス");
    // 別名で書いても表示名（H1）へ
    let alias = world_with(CONTENTS_PATH, "@01-04-02 x\n'錬金少年 が来た\n");
    let alias_doc = doc(&alias, CONTENTS_PATH);
    let found = definition::at(&alias_doc, 13, &alias).expect("alias");
    assert_eq!(slice(CHARACTERS, &found.range), "アルス");
}

#[test]
fn definitions_of_words_and_materials() {
    let project = world();
    let plot = doc(&project, EPISODE_PATH);
    let word =
        definition::at(&plot, find(&plot.text, "'星屑の時計", 0) + 1, &project).expect("word");
    assert_eq!(word.path, "words.md");
    assert_eq!(slice(WORDS, &word.range), "星屑の時計");
    // 資料 MD がプロジェクトに無ければ index.json の ID
    let material =
        definition::at(&plot, find(&plot.text, "'M0137", 0) + 1, &project).expect("index");
    assert_eq!(material.path, "materials/index.json");
    assert_eq!(slice(INDEX, &material.range), "M0137");
    // あればその 1 行目
    let path = "materials/docs/M0137-kagaku-koshien.md";
    let with_doc = world_with(path, "# 科学の甲子園 本戦の運営と実技課題\n本文\n");
    let found = definition::at(&plot, find(&plot.text, "'M0137", 0) + 1, &with_doc).expect("doc");
    assert_eq!(found.path, path);
    assert_eq!(found.range, 0.."# 科学の甲子園 本戦の運営と実技課題".len());
}

#[test]
fn references_collect_every_document() {
    let project = world();
    let body = doc(&project, CONTENTS_PATH);
    let found = references::at(&body, find(&body.text, "'リリア", 0) + 1, &project);
    let paths: Vec<&str> = found
        .iter()
        .map(|location| location.path.as_str())
        .collect();
    // 全体プロット 4 件（@01 の summary / turn / cast と @02 の cast）、関係（characters.md）1 件、
    // 本文の参照 2 件と話者注釈 2 件、話プロット（cast / mode / 本文の予定 / 節の mode / 節の本文）
    assert_eq!(
        paths
            .iter()
            .filter(|path| **path == "characters.md")
            .count(),
        1
    );
    assert_eq!(
        paths.iter().filter(|path| **path == CONTENTS_PATH).count(),
        4
    );
    assert_eq!(paths.iter().filter(|path| **path == ALL_PATH).count(), 4);
    assert!(paths.iter().filter(|path| **path == EPISODE_PATH).count() >= 5);
    // パス → 位置の順
    let mut sorted = found.clone();
    sorted.sort_by(|a, b| (a.path.as_str(), a.range.start).cmp(&(b.path.as_str(), b.range.start)));
    assert_eq!(found, sorted);
    // 定義側（H1）からも同じ一覧
    let characters = doc(&project, "characters.md");
    let from_heading = references::at(&characters, find(CHARACTERS, "# リリア", 0) + 3, &project);
    assert_eq!(from_heading, found);
}

// ============================================================ document_symbol / folding

fn symbol_tags(text: &str, symbols: &[DocumentSymbol]) -> Vec<String> {
    symbols
        .iter()
        .map(|symbol| {
            let children: Vec<String> = symbol_tags(text, &symbol.children);
            let mut tag = format!(
                "{:?}:{}:{}|{}",
                symbol.kind,
                symbol.name,
                symbol.detail,
                slice(text, &symbol.selection_range)
            );
            if !children.is_empty() {
                tag.push_str(&format!(" [{}]", children.join(", ")));
            }
            tag
        })
        .collect()
}

#[test]
fn symbols_of_the_body_and_the_plots() {
    let project = world();
    let body = doc(&project, CONTENTS_PATH);
    assert_eq!(
        symbol_tags(&body.text, &document_symbol::all(&body)),
        vec!["Scene:01-04-02:図書室の攻防|@01-04-02 [Section:1:|, Section:2:|, Section:3:|]"]
    );
    let all = doc(&project, ALL_PATH);
    assert_eq!(
        symbol_tags(&all.text, &document_symbol::all(&all)),
        vec!["Chapter:01:失われた刻印|@01", "Chapter:02:補習と地図|@02"]
    );
    let plot = doc(&project, EPISODE_PATH);
    let symbols = document_symbol::all(&plot);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].kind, SymbolKind::Scene);
    assert_eq!(symbols[0].children.len(), 4);
}

#[test]
fn symbols_of_the_markdown_files() {
    let project = world();
    let characters = doc(&project, "characters.md");
    assert_eq!(
        symbol_tags(&characters.text, &document_symbol::all(&characters)),
        vec![
            "Character:アルス:|アルス [Mode:A:探究|A, Mode:B:補習|B]",
            "Character:リリア:|リリア [Mode:A:呆れ|A, Mode:B:呆れ（強）|B, Mode:C:好奇心|C]",
        ]
    );
    let words = doc(&project, "words.md");
    assert_eq!(
        symbol_tags(&words.text, &document_symbol::all(&words)),
        vec!["Word:星屑の時計:|星屑の時計", "Word:旧校舎:|旧校舎"]
    );
    // シンボル全体の範囲はその項目の行（末尾の空行を含まない）
    let first = &document_symbol::all(&words)[0];
    assert!(slice(WORDS, &first.range).ends_with("第 3 章で贋作であることが判明する。"));
}

#[test]
fn folding_is_the_chapter_to_episode_view_of_plot_outline() {
    let project = world();
    let all = doc(&project, ALL_PATH);
    let ranges = folding::ranges(&all);
    let tags: Vec<(usize, String, String)> = ranges
        .iter()
        .map(|found| {
            let text = slice(&all.text, &found.range);
            let first = text.lines().next().unwrap_or_default().to_string();
            let last = text.lines().last().unwrap_or_default().to_string();
            (found.depth, first, last)
        })
        .collect();
    assert_eq!(
        tags,
        vec![
            (1, "@01 失われた刻印".to_string(), ";".to_string()),
            (1, "@02 補習と地図".to_string(), ";".to_string()),
        ]
    );
    // 本文・話プロットの折り畳みは追加仕様 9（②）
    assert!(folding::ranges(&doc(&project, CONTENTS_PATH)).is_empty());
    assert!(folding::ranges(&doc(&project, EPISODE_PATH)).is_empty());
}
