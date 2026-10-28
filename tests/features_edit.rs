//! `core::features` のうち、文書を書き換えるもの: formatting（§5.12）・rename・
//! completion（§9.7.3）・code_action（§7 のクイックフィックス）。
//!
//! 期待値は**編集を当てた後の文字列**で書く（byte 位置を手で数えない、handoff §4.2）。

mod world;

use novnow::core::diagnostics::DiagnosticCode;
use novnow::core::features::code_action::{self, ActionKind, CodeAction};
use novnow::core::features::completion::{self, CompletionItem, CompletionKind};
use novnow::core::features::project::Project;
use novnow::core::features::rename::{self, RenameError};
use novnow::core::features::{apply_edits, formatting, WorkspaceEdit};
use novnow::core::model::document::Document;

use world::*;

fn find(text: &str, needle: &str, nth: usize) -> usize {
    text.match_indices(needle)
        .nth(nth)
        .map(|(at, _)| at)
        .unwrap_or_else(|| panic!("{needle} が {nth} 回目に見つからない"))
}

/// `edit` を当てた後の `path` の中身（プロジェクトに無ければ空のファイルに当てる）。
fn after(project: &Project, edit: &WorkspaceEdit, path: &str) -> String {
    let text = project.text(path);
    match edit.changes.iter().find(|change| change.path == path) {
        Some(change) => apply_edits(text, &change.edits),
        None => text.to_string(),
    }
}

// ============================================================ formatting（§5.12）

fn formatted(path: &str, text: &str) -> String {
    let project = world_with(path, text);
    let document = doc(&project, path);
    apply_edits(text, &formatting::document(&document, &project))
}

#[test]
fn the_seven_rules_of_5_12() {
    // 1 行末の空白（半角・全角・タブ）／7 全角空白だけの行は空行
    assert_eq!(
        formatted(CONTENTS_PATH, "本文。 \u{3000}\t\n\u{3000}\u{3000}\n続き\n"),
        "本文。\n\n続き\n"
    );
    // 3 末尾に改行 1 個（足す／余分な空行を落とす）
    assert_eq!(formatted(CONTENTS_PATH, "本文"), "本文\n");
    assert_eq!(formatted(CONTENTS_PATH, "本文\n\n\n"), "本文\n");
    // 4 3 行以上の空行は 2 行に
    assert_eq!(formatted(CONTENTS_PATH, "a\n\n\n\n\nb\n"), "a\n\n\nb\n");
    // 5 `;` 行の前後の空白
    assert_eq!(formatted(CONTENTS_PATH, "a\n ;  \nb\n"), "a\n;\nb\n");
    // 6 フィールド行の `key` と `:` の間を詰め、`: ` の後は空白 1 個
    assert_eq!(
        formatted(EPISODE_PATH, "summary :   あらすじ\n"),
        "summary: あらすじ\n"
    );
    assert_eq!(
        formatted(EPISODE_PATH, "summary:あらすじ\n"),
        "summary: あらすじ\n"
    );
    assert_eq!(formatted(EPISODE_PATH, "summary:\n"), "summary:\n");
}

#[test]
fn full_width_colons_keep_what_follows() {
    // B-83: 全角 `：` は空白を求めない（§5.0.5）ので後ろは触らない。key との間だけ詰める
    assert_eq!(
        formatted(EPISODE_PATH, "要約　：あらすじ\n"),
        "要約：あらすじ\n"
    );
    assert_eq!(
        formatted(EPISODE_PATH, "要約：　あらすじ\n"),
        "要約：　あらすじ\n"
    );
}

#[test]
fn the_body_text_is_never_touched() {
    // 句読点・記号・行頭の字下げは本文の文字（§5.12「一切触れない」）
    let body = "@01-04-02 x\n　「待って」と言った。\n\u{3000}地の文 : コロン\n";
    assert_eq!(formatted(CONTENTS_PATH, body), body);
    // C 形式にフィールド行は無い（`key:` に見えても本文）
    assert_eq!(formatted(CONTENTS_PATH, "summary :  x\n"), "summary :  x\n");
    // `*.md` の `;` は節区切りではない
    assert_eq!(
        formatted("words.md", "# 用語\ndefinition: x\n  ;\n"),
        "# 用語\ndefinition: x\n  ;\n"
    );
}

#[test]
fn the_spec_examples_are_already_normalized_and_formatting_is_idempotent() {
    for (path, text) in [
        (CONTENTS_PATH, CONTENTS),
        (EPISODE_PATH, EPISODE),
        (ALL_PATH, ALL),
        ("words.md", WORDS),
        ("characters.md", CHARACTERS),
    ] {
        let project = world_with(path, text);
        let edits = formatting::document(&doc(&project, path), &project);
        assert!(edits.is_empty(), "{path}: {edits:?}");
    }
    let messy = "a  \n\n\n\n ;\nsummary :x\nb";
    let once = formatted(EPISODE_PATH, messy);
    assert_eq!(formatted(EPISODE_PATH, &once), once);
}

// ============================================================ rename

#[test]
fn renaming_a_character_rewrites_every_reference_and_speaker() {
    let project = world();
    let body = doc(&project, CONTENTS_PATH);
    let at = find(&body.text, "'アルス", 0) + 1;
    let prepared = rename::prepare(&body, at, &project).expect("prepare");
    assert_eq!(slice(&body.text, &prepared.range), "アルス");
    assert_eq!(prepared.placeholder, "アルス");
    let edit = rename::apply(&body, at, "アルフレッド", &project).expect("apply");
    // 定義（H1）
    assert!(after(&project, &edit, "characters.md").starts_with("# アルフレッド\n"));
    // 本文の参照と話者注釈
    let new_body = after(&project, &edit, CONTENTS_PATH);
    assert!(new_body.contains("'アルフレッド、ここにいたのね"));
    assert!(new_body.contains("の設計図だ」@アルフレッド"));
    assert!(!new_body.contains("アルス"));
    // プロット（cast / mode / pov / 本文の予定）と全体プロット
    let plot = after(&project, &edit, EPISODE_PATH);
    assert!(plot.contains("cast: 'アルフレッド 'リリア"));
    assert!(plot.contains("mode: 'アルフレッド:A 'リリア:A"));
    assert!(plot.contains("pov: 'アルフレッド"));
    assert!(!plot.contains("'アルス"));
    assert!(!after(&project, &edit, ALL_PATH).contains("'アルス"));
}

#[test]
fn renaming_an_alias_keeps_the_heading() {
    let project = world_with(CONTENTS_PATH, "@01-04-02 x\n'錬金少年 と 'アルス\n");
    let body = doc(&project, CONTENTS_PATH);
    let at = find(&body.text, "'錬金少年", 0) + 1;
    let edit = rename::apply(&body, at, "錬金術師", &project).expect("apply");
    let characters = after(&project, &edit, "characters.md");
    assert!(characters.starts_with("# アルス\n"));
    assert!(characters.contains("alias: 錬金術師\n"));
    assert_eq!(
        after(&project, &edit, CONTENTS_PATH),
        "@01-04-02 x\n'錬金術師 と 'アルス\n"
    );
}

#[test]
fn a_new_name_with_a_shorter_key_as_prefix_is_written_explicitly() {
    // §6.2.2: `'アルス二世` は最短一致で `アルス` に解決されてしまうので明示形にする
    let project = world();
    let body = doc(&project, CONTENTS_PATH);
    let at = find(&body.text, "'リリア", 0) + 1;
    let edit = rename::apply(&body, at, "アルス二世", &project).expect("apply");
    let new_body = after(&project, &edit, CONTENTS_PATH);
    assert!(new_body.contains("'{アルス二世}が隣の椅子"));
    // 話者注釈は完全一致なので明示形にしない（§6.2.5）
    assert!(new_body.contains("@アルス二世:A"));
}

#[test]
fn rename_refuses_bad_names() {
    let project = world();
    let body = doc(&project, CONTENTS_PATH);
    let at = find(&body.text, "'リリア", 0) + 1;
    assert_eq!(
        rename::apply(&body, at, "  ", &project),
        Err(RenameError::Empty)
    );
    assert_eq!(
        rename::apply(&body, at, "リ リア", &project),
        Err(RenameError::Invalid)
    );
    assert_eq!(
        rename::apply(&body, at, "リリア:B", &project),
        Err(RenameError::Invalid)
    );
    // 正規化して既存のキーと同じ（全角英数・別名を含む）
    assert_eq!(
        rename::apply(&body, at, "アルス", &project),
        Err(RenameError::Duplicate)
    );
    assert_eq!(
        rename::apply(&body, at, "錬金少年", &project),
        Err(RenameError::Duplicate)
    );
    // 資料と地の文は改名できない
    let plot = doc(&project, EPISODE_PATH);
    let material = find(&plot.text, "'M0137", 0) + 1;
    assert!(rename::prepare(&plot, material, &project).is_none());
    assert_eq!(
        rename::apply(&plot, material, "M9999", &project),
        Err(RenameError::NotRenamable)
    );
}

#[test]
fn rename_also_works_from_the_definition() {
    let project = world();
    let words = doc(&project, "words.md");
    let at = find(WORDS, "# 旧校舎", 0) + 3;
    let prepared = rename::prepare(&words, at, &project).expect("prepare");
    assert_eq!(prepared.placeholder, "旧校舎");
    let edit = rename::apply(&words, at, "旧講堂", &project).expect("apply");
    assert!(after(&project, &edit, "words.md").contains("# 旧講堂\n"));
}

// ============================================================ completion（§9.7.3）

fn items_at(path: &str, text: &str, pos: usize) -> (Project, Vec<CompletionItem>) {
    let project = world_with(path, text);
    let document = doc(&project, path);
    let items = completion::at(&document, pos, &project);
    (project, items)
}

fn labels(items: &[CompletionItem], kind: CompletionKind) -> Vec<String> {
    items
        .iter()
        .filter(|item| item.kind == kind)
        .map(|item| item.label.clone())
        .collect()
}

#[test]
fn a_quote_offers_characters_words_and_materials_in_that_order() {
    let text = "@01-04-02 x\n'";
    let (_, items) = items_at(CONTENTS_PATH, text, text.len());
    let kinds: Vec<CompletionKind> = items.iter().map(|item| item.kind).collect();
    let first_word = kinds
        .iter()
        .position(|kind| *kind == CompletionKind::Word)
        .expect("word");
    let first_material = kinds
        .iter()
        .position(|kind| *kind == CompletionKind::Material)
        .expect("material");
    assert!(kinds[..first_word]
        .iter()
        .all(|kind| *kind == CompletionKind::Character));
    assert!(first_word < first_material);
    // 五十音（`reading:` 優先）: あるす < りりあ、きゅうこうしゃ < ほしくずのとけい
    assert_eq!(
        labels(&items, CompletionKind::Character),
        vec!["アルス", "リリア"]
    );
    assert_eq!(
        labels(&items, CompletionKind::Word),
        vec!["旧校舎", "星屑の時計"]
    );
    let material = items
        .iter()
        .find(|item| item.kind == CompletionKind::Material)
        .expect("M0137");
    assert_eq!(material.label, "M0137");
    assert_eq!(material.detail, "科学の甲子園 本戦の運営と実技課題");
}

#[test]
fn what_appears_in_this_section_comes_first() {
    // §9.7.3「現在の節に登場 > 同じ話に登場 > … > 五十音」
    let text = "@01-04-02 x\n'リリア が来た。\n'";
    let (_, items) = items_at(CONTENTS_PATH, text, text.len());
    assert_eq!(
        labels(&items, CompletionKind::Character),
        vec!["リリア", "アルス"]
    );
}

#[test]
fn typed_prefixes_filter_and_replace_only_the_typed_part() {
    let text = "@01-04-02 x\n「'リ";
    let (_, items) = items_at(CONTENTS_PATH, text, text.len());
    assert_eq!(labels(&items, CompletionKind::Character), vec!["リリア"]);
    let item = &items[0];
    assert_eq!(slice(text, &item.range), "リ");
    assert_eq!(item.insert_text, "リリア");
    // 別名で打ちかけたら別名を出す（実体ごとに 1 件）
    let alias = "@01-04-02 x\n'錬";
    let (_, items) = items_at(CONTENTS_PATH, alias, alias.len());
    assert_eq!(labels(&items, CompletionKind::Character), vec!["錬金少年"]);
}

#[test]
fn a_word_that_repeats_a_character_name_is_not_listed_twice() {
    // §6.1.4（F-24）: `words.md` の用語が `characters.md` の名前と同じでも、
    // 補完候補は登場人物側の 1 件だけ（用語としては出さない）
    let words = format!("{WORDS}\n# アルス\ndefinition: 錬金術を学ぶ少年。\n");
    let mut project = world_with("words.md", &words);
    let body = "@01-04-02 x\n'アル";
    project.insert(Document::new(CONTENTS_PATH, body));
    let items = completion::at(&doc(&project, CONTENTS_PATH), body.len(), &project);
    assert_eq!(labels(&items, CompletionKind::Character), vec!["アルス"]);
    assert!(labels(&items, CompletionKind::Word).is_empty());
}

#[test]
fn explicit_forms_and_shortest_match_hazards_insert_braces() {
    let text = "@01-04-02 x\n'{星";
    let (_, items) = items_at(CONTENTS_PATH, text, text.len());
    assert_eq!(items[0].insert_text, "星屑の時計}");
    // `リリア` があるので `リリアの花` をそのまま入れると最短一致で `リリア` になる（§6.2.2）
    let words = format!("{WORDS}\n# リリアの花\ndefinition: 花。\n");
    let mut project = world_with("words.md", &words);
    let body = "@01-04-02 x\n'リリアの";
    project.insert(Document::new(CONTENTS_PATH, body));
    let items = completion::at(&doc(&project, CONTENTS_PATH), body.len(), &project);
    let flower = items
        .iter()
        .find(|item| item.label == "リリアの花")
        .expect("flower");
    assert_eq!(flower.insert_text, "{リリアの花}");
}

#[test]
fn at_most_eight_per_kind() {
    let mut words = String::new();
    for index in 0..12 {
        words.push_str(&format!("# 魔導具{index:02}\ndefinition: x\n\n"));
    }
    let mut project = world_with("words.md", &words);
    let body = "@01-04-02 x\n'魔";
    project.insert(Document::new(CONTENTS_PATH, body));
    let items = completion::at(&doc(&project, CONTENTS_PATH), body.len(), &project);
    assert_eq!(
        labels(&items, CompletionKind::Word).len(),
        completion::PER_KIND
    );
}

#[test]
fn a_colon_after_a_character_offers_its_modes_with_out_of_range_ones_faint() {
    // アルスの B は `range: @02 ..`（1 章では灰色で下に並べる、§9.7.3）
    let text = "@01-04-02 x\n「行くぞ」@アルス:";
    let (_, items) = items_at(CONTENTS_PATH, text, text.len());
    let shown: Vec<(String, String, bool)> = items
        .iter()
        .map(|item| (item.label.clone(), item.detail.clone(), item.faint))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("A".to_string(), "探究".to_string(), false),
            ("B".to_string(), "補習".to_string(), true),
        ]
    );
    // `'リリア:B+` の後ろ（2 つ目のモード）も同じ
    let second = "@01-04-02 x\n'リリア:B+";
    let (_, items) = items_at(CONTENTS_PATH, second, second.len());
    assert_eq!(labels(&items, CompletionKind::Mode), vec!["A", "B", "C"]);
}

#[test]
fn an_at_sign_offers_nearby_scene_ids_or_speakers() {
    // 行頭: 次の場面と、今の場面の次の節（§9.7.3「近傍 SceneId 候補」）
    let text = format!("{CONTENTS}\n@");
    let (_, items) = items_at(CONTENTS_PATH, &text, text.len());
    assert_eq!(
        labels(&items, CompletionKind::SceneId),
        vec!["01-04-03", "01-04-02-04"]
    );
    // 全体プロットは次の章
    let all = format!("{ALL}\n@");
    let (_, items) = items_at(ALL_PATH, &all, all.len());
    assert_eq!(labels(&items, CompletionKind::SceneId), vec!["03"]);
    // 行末: キャラ（話者注釈）
    let speaker = "@01-04-02 x\n「待って」@リ";
    let (_, items) = items_at(CONTENTS_PATH, speaker, speaker.len());
    assert_eq!(labels(&items, CompletionKind::Character), vec!["リリア"]);
    // S-08: closing Japanese quotation marks must not swallow the speaker trigger.
    for close in ['」', '』'] {
        let text = format!("@01-04-02 x\n「待って{close}@リ");
        let (_, items) = items_at(CONTENTS_PATH, &text, text.len());
        assert_eq!(labels(&items, CompletionKind::Character), vec!["リリア"]);
        let text = format!("@01-04-02 x\n「待って{close}@リリア:");
        let (_, items) = items_at(CONTENTS_PATH, &text, text.len());
        assert!(!labels(&items, CompletionKind::Mode).is_empty());
    }
}

#[test]
fn plain_text_offers_nothing() {
    let text = "@01-04-02 x\n地の文";
    let (_, items) = items_at(CONTENTS_PATH, text, text.len());
    assert!(items.is_empty());
}

// ============================================================ code_action（§7）

fn actions_for(path: &str, text: &str, code: DiagnosticCode) -> (Project, Vec<CodeAction>) {
    let project = world_with(path, text);
    let document = doc(&project, path);
    let found = code_action::at(&document, 0..text.len(), &project)
        .into_iter()
        .filter(|action| action.diagnostic == code)
        .collect();
    (project, found)
}

fn kinds(actions: &[CodeAction]) -> Vec<(ActionKind, String)> {
    actions
        .iter()
        .map(|action| (action.kind, action.arg.clone()))
        .collect()
}

#[test]
fn e001_registers_or_replaces_with_a_close_key() {
    let text = "@01-04-02 x\n'リリヤ が来た。\n";
    let (project, actions) = actions_for(CONTENTS_PATH, text, DiagnosticCode::E001);
    assert_eq!(
        kinds(&actions),
        vec![
            (ActionKind::RegisterWord, "リリヤ".to_string()),
            (ActionKind::RegisterCharacter, "リリヤ".to_string()),
            (ActionKind::ReplaceWithCandidate, "リリア".to_string()),
        ]
    );
    assert!(after(&project, &actions[0].edit, "words.md").ends_with("\n\n# リリヤ\ndefinition: \n"));
    assert!(after(&project, &actions[1].edit, "characters.md").ends_with("\n# リリヤ\n"));
    assert_eq!(
        after(&project, &actions[2].edit, CONTENTS_PATH),
        "@01-04-02 x\n'リリア が来た。\n"
    );
}

#[test]
fn e002_closes_an_explicit_form_after_the_longest_key() {
    let text = "@01-04-02 x\n'{星屑の時計 の噂\n";
    let (project, actions) = actions_for(CONTENTS_PATH, text, DiagnosticCode::E002);
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::CloseExplicitForm, "星屑の時計".to_string())]
    );
    assert_eq!(
        after(&project, &actions[0].edit, CONTENTS_PATH),
        "@01-04-02 x\n'{星屑の時計} の噂\n"
    );
}

#[test]
fn e003_and_e004_replace_the_mode() {
    let undefined = "@01-04-02 x\n「待って」@リリア:Z\n";
    let (project, actions) = actions_for(CONTENTS_PATH, undefined, DiagnosticCode::E003);
    assert_eq!(
        kinds(&actions),
        vec![
            (ActionKind::ReplaceMode, "A".to_string()),
            (ActionKind::ReplaceMode, "B".to_string()),
            (ActionKind::ReplaceMode, "C".to_string()),
        ]
    );
    assert_eq!(
        after(&project, &actions[0].edit, CONTENTS_PATH),
        "@01-04-02 x\n「待って」@リリア:A\n"
    );
    // E004 はその節で使えるモードだけ（アルスの B は 1 章では範囲外）
    let out_of_range = "@01-04-02 x\n「行くぞ」@アルス:B\n";
    let (_, actions) = actions_for(CONTENTS_PATH, out_of_range, DiagnosticCode::E004);
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::ReplaceMode, "A".to_string())]
    );
}

#[test]
fn e005_keeps_the_first_two_modes() {
    let text = "@01-04-02 x\n'リリア:A+B+C が来た。\n";
    let (project, actions) = actions_for(CONTENTS_PATH, text, DiagnosticCode::E005);
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::KeepFirstTwoModes, "A+B".to_string())]
    );
    assert_eq!(
        after(&project, &actions[0].edit, CONTENTS_PATH),
        "@01-04-02 x\n'リリア:A+B が来た。\n"
    );
}

#[test]
fn e006_offers_the_three_fixes_of_6_6_3() {
    let plot = EPISODE.replace("material: 'M0137", "material: 'M0137:科学の甲子園");
    let (project, actions) = actions_for(EPISODE_PATH, &plot, DiagnosticCode::E006);
    let shown = kinds(&actions);
    assert_eq!(
        shown[0],
        (
            ActionKind::ReplaceTitle,
            "科学の甲子園 本戦の運営と実技課題".to_string()
        )
    );
    assert!(actions[0].preferred);
    assert!(after(&project, &actions[0].edit, EPISODE_PATH)
        .contains("material: 'M0137:科学の甲子園 本戦の運営と実技課題\n"));
    // index 側をファイルの記述に合わせる（原文の書き方のまま値だけ替える）
    assert_eq!(
        shown[1],
        (ActionKind::RewriteIndexTitle, "科学の甲子園".to_string())
    );
    let index = after(&project, &actions[1].edit, "materials/index.json");
    assert!(index.contains("\"title\": \"科学の甲子園\","));
    assert_eq!(index.len(), INDEX.len() - "本戦の運営と実技課題 ".len());
}

#[test]
fn e006_offers_all_three_fixes_when_another_material_has_the_written_title() {
    // §6.6.3 のクイックフィックス 3（同名タイトルを持つ別 ID へ貼り替える）まで並ぶ（§7 の E006 の行）
    let index = INDEX.replacen(
        "  \"next_id\": 138,\n  \"items\": [\n",
        "  \"next_id\": 142,\n  \"items\": [\n    { \"id\": \"M0141\", \"title\": \"科学の甲子園 地方大会の運営\", \"path\": \"materials/docs/M0141.md\", \"source\": { \"kind\": \"custom\", \"text\": \"取材\" }, \"category\": \"取材\", \"conditions\": [], \"saved_at\": \"2026-10-03T10:00:00+09:00\", \"tags\": [], \"status\": \"approved\", \"added_by\": \"human\" },\n",
        1,
    );
    assert_ne!(index, INDEX);
    let plot = EPISODE.replace(
        "material: 'M0137",
        "material: 'M0137:科学の甲子園 地方大会の運営",
    );
    let mut project = world_with(EPISODE_PATH, &plot);
    project.insert(Document::new("materials/index.json", index.as_str()));
    let document = doc(&project, EPISODE_PATH);
    let actions: Vec<CodeAction> = code_action::at(&document, 0..plot.len(), &project)
        .into_iter()
        .filter(|action| action.diagnostic == DiagnosticCode::E006)
        .collect();
    assert_eq!(
        kinds(&actions),
        vec![
            (
                ActionKind::ReplaceTitle,
                "科学の甲子園 本戦の運営と実技課題".to_string()
            ),
            (
                ActionKind::RewriteIndexTitle,
                "科学の甲子園 地方大会の運営".to_string()
            ),
            (ActionKind::UseOtherMaterialId, "M0141".to_string()),
        ]
    );
    assert!(actions[0].preferred, "1 が既定（Enter 一発）");
    assert!(after(&project, &actions[2].edit, EPISODE_PATH)
        .contains("material: 'M0141:科学の甲子園 地方大会の運営\n"));
}

#[test]
fn e007_replaces_with_a_close_material_id() {
    let text = "@01-04-02 x\nmaterial: 'M0138\n";
    let (project, actions) = actions_for(EPISODE_PATH, text, DiagnosticCode::E007);
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::ReplaceMaterialId, "M0137".to_string())]
    );
    assert!(after(&project, &actions[0].edit, EPISODE_PATH).contains("material: 'M0137\n"));
}

#[test]
fn e008_and_e022_renumber_the_header() {
    let duplicate = "@01-04-02 x\n本文\n@01-04-02 y\n本文\n";
    let (project, actions) = actions_for(CONTENTS_PATH, duplicate, DiagnosticCode::E008);
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::Renumber, "01-04-03".to_string())]
    );
    assert_eq!(
        after(&project, &actions[0].edit, CONTENTS_PATH),
        "@01-04-02 x\n本文\n@01-04-03 y\n本文\n"
    );
    let reserved = "@01-04-00 x\n本文\n";
    let (_, actions) = actions_for(CONTENTS_PATH, reserved, DiagnosticCode::E022);
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::Renumber, "01-04-01".to_string())]
    );
}

#[test]
fn e012_registers_the_speaker_and_e013_replaces_a_relation() {
    let (project, actions) = actions_for(
        CONTENTS_PATH,
        "@01-04-02 x\n「来た」@ゲスト\n",
        DiagnosticCode::E012,
    );
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::RegisterCharacter, "ゲスト".to_string())]
    );
    assert!(after(&project, &actions[0].edit, "characters.md").ends_with("「*のね」\n\n# ゲスト\n"));
    let characters = CHARACTERS.replacen("'リリア: 幼馴染。", "'リリヤ: 幼馴染。", 1);
    let (project, actions) = actions_for("characters.md", &characters, DiagnosticCode::E013);
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::ReplaceWithCandidate, "リリア".to_string())]
    );
    let fixed = after(&project, &actions[0].edit, "characters.md");
    assert!(fixed.contains("'リリア: 幼馴染。"), "{fixed}");
}

#[test]
fn e016_e018_e021_fix_definitions() {
    let characters = CHARACTERS.replacen("range: @02 ..", "range: @03 .. @01", 1);
    let (project, actions) = actions_for("characters.md", &characters, DiagnosticCode::E016);
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::SwapRange, "@01 .. @03".to_string())]
    );
    assert!(after(&project, &actions[0].edit, "characters.md").contains("range: @01 .. @03\n"));
    let all = "@01 章\ngoal: g\n;\n";
    let (project, actions) = actions_for(ALL_PATH, all, DiagnosticCode::E018);
    assert_eq!(
        after(&project, &actions[0].edit, ALL_PATH),
        "@01 章\nsummary: \ngoal: g\n;\n"
    );
    let words = "# 新語\nreading: しんご\n";
    let (project, actions) = actions_for("words.md", words, DiagnosticCode::E021);
    assert_eq!(
        after(&project, &actions[0].edit, "words.md"),
        "# 新語\ndefinition: \nreading: しんご\n"
    );
}

#[test]
fn e026_w002_w011_w012_w029_quick_fixes() {
    let (project, actions) = actions_for(
        CONTENTS_PATH,
        "@01-04-02 x\n'星屑の時計:古い の噂\n",
        DiagnosticCode::E026,
    );
    assert_eq!(
        after(&project, &actions[0].edit, CONTENTS_PATH),
        "@01-04-02 x\n'星屑の時計 の噂\n"
    );
    let (project, actions) = actions_for(
        CONTENTS_PATH,
        "@01-04-02 x\nメール @ 宛先\n",
        DiagnosticCode::W002,
    );
    assert_eq!(
        after(&project, &actions[0].edit, CONTENTS_PATH),
        "@01-04-02 x\nメール \\@ 宛先\n"
    );
    let (project, actions) = actions_for(
        ALL_PATH,
        "@01 章\nsummary: s\nsumary: t\n;\n",
        DiagnosticCode::W011,
    );
    assert_eq!(
        kinds(&actions)[0],
        (ActionKind::ReplaceKey, "summary".to_string())
    );
    assert!(after(&project, &actions[0].edit, ALL_PATH).contains("\nsummary: t\n"));
    let (project, actions) = actions_for(
        CONTENTS_PATH,
        "@01-04-02 x\n本文\n;\n;\n本文\n",
        DiagnosticCode::W012,
    );
    assert_eq!(
        after(&project, &actions[0].edit, CONTENTS_PATH),
        "@01-04-02 x\n本文\n;\n本文\n"
    );
    let words = format!("{WORDS}\n# リリアナ\ndefinition: 別人。\n");
    let mut project = world_with("words.md", &words);
    project.insert(Document::new(
        CONTENTS_PATH,
        "@01-04-02 x\n'リリアナ が来た\n",
    ));
    let document = doc(&project, CONTENTS_PATH);
    let actions: Vec<CodeAction> = code_action::at(&document, 0..document.text.len(), &project)
        .into_iter()
        .filter(|action| action.diagnostic == DiagnosticCode::W029)
        .collect();
    assert_eq!(
        kinds(&actions),
        vec![(ActionKind::UseExplicitForm, "リリアナ".to_string())]
    );
    assert_eq!(
        after(&project, &actions[0].edit, CONTENTS_PATH),
        "@01-04-02 x\n'{リリアナ} が来た\n"
    );
}

#[test]
fn only_diagnostics_in_the_range_get_actions() {
    let text = "@01-04-02 x\n'だれか\n'どこか\n";
    let project = world_with(CONTENTS_PATH, text);
    let document = doc(&project, CONTENTS_PATH);
    let second = find(text, "'どこか", 0);
    let actions = code_action::at(&document, second..second + 1, &project);
    assert!(!actions.is_empty());
    assert!(actions.iter().all(|action| action.arg != "だれか"));
}

#[test]
fn closing_japanese_quotes_offer_speaker_and_mode_completion() {
    // S-08: dialogue closing marks, including a separating space, open speaker suggestions.
    for ending in ["」@", "』@", "」 @", "』 @"] {
        let text = format!("@01-04-02 x\n「行くぞ{ending}");
        let (_, items) = items_at(CONTENTS_PATH, &text, text.len());
        assert!(
            labels(&items, CompletionKind::Character).contains(&"アルス".to_owned()),
            "{ending}: {items:?}"
        );
    }
    let text = "@01-04-02 x\n「行くぞ」@アルス:";
    let (_, items) = items_at(CONTENTS_PATH, text, text.len());
    assert!(labels(&items, CompletionKind::Mode).contains(&"A".to_owned()));
}

#[test]
fn w096_quick_fix_matches_the_majority_spacing() {
    // §7.2.2: 多数派（空白あり）へ寄せる 1 操作
    let text = "@01-04-02 x\n\
                「行くぞ！　待て」\n\
                「止まれ！　聞け」\n\
                「来い！　早く」\n\
                「逃げろ！　今だ」\n\
                「見ろ！　あれを」\n\
                「進め！続け」\n";
    let mut project = world_with(CONTENTS_PATH, text);
    project.insert(Document::new(
        "project.toml",
        "schema_version = 2\n\n[features.diagnostics]\nW096 = \"warn\"\n",
    ));
    let document = doc(&project, CONTENTS_PATH);
    let actions: Vec<CodeAction> = code_action::at(&document, 0..document.text.len(), &project)
        .into_iter()
        .filter(|action| action.diagnostic == DiagnosticCode::W096)
        .collect();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].kind, ActionKind::MatchDialogueSpacing);
    let fixed = after(&project, &actions[0].edit, CONTENTS_PATH);
    assert!(fixed.contains("「進め！　続け」"), "{fixed}");
}
