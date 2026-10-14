//! 節単位の編集と節の並び（`core::features::sections`、§8.1・§9.3・§9.8.2）と、
//! 参照ペインの中身（`core::features::reference`、§9.5.2）。M7。

mod world;

use novnow::core::diagnostics::DiagnosticCode;
use novnow::core::features::{reference, sections};
use novnow::core::id::SceneId;
use novnow::core::model::document::Document;

use world::{world, world_with, CONTENTS_PATH};

fn id(chapter: u16, episode: u16, scene: u16, section: u16) -> SceneId {
    SceneId {
        chapter,
        episode,
        scene,
        section,
    }
}

#[test]
fn the_sections_of_the_spec_example_are_listed_in_order() {
    let project = world();
    let list = sections::all(&project);
    let ids: Vec<SceneId> = list.iter().map(|item| item.id).collect();
    assert_eq!(ids, vec![id(1, 4, 2, 1), id(1, 4, 2, 2), id(1, 4, 2, 3)]);
    assert_eq!(list[0].scene_title, "図書室の攻防");
    let text = project.text(CONTENTS_PATH);
    assert_eq!(
        sections::text_of(text, &list[1]),
        "「旧校舎の地下遺跡に眠っているっていう'『星屑の時計』の設計図だ」@アルス\n\n'リリアは溜め息をついたが、図面の精巧な書き込みに目を奪われた。"
    );
    assert_eq!(
        sections::text_of(text, &list[2]),
        "「……付き合ってあげるわよ」@リリア:B    // ここでモードが切り替わる"
    );
}

#[test]
fn splicing_a_section_keeps_the_rest_of_the_file_byte_for_byte() {
    let project = world();
    let list = sections::all(&project);
    let text = project.text(CONTENTS_PATH);
    // 書き戻して読み直すと元どおり（何も変えなければ同じ文字列）
    for item in &list {
        let body = sections::text_of(text, item);
        assert_eq!(sections::splice(text, item, &body), text);
    }
    let edited = sections::splice(text, &list[1], "書き直した節。");
    assert!(edited.contains("\n;\n書き直した節。\n;\n"), "{edited}");
    assert!(edited.starts_with("@01-04-02 図書室の攻防\n「やっぱり"));
    assert!(edited.ends_with("// ここでモードが切り替わる\n"));
}

#[test]
fn a_new_section_is_made_after_the_current_one() {
    let project = world();
    let list = sections::all(&project);
    let text = project.text(CONTENTS_PATH);
    let (inserted, line) = sections::insert_after(text, &list[0]);
    let updated = world_with(CONTENTS_PATH, &inserted);
    let after = sections::all(&updated);
    assert_eq!(after.len(), 4);
    assert_eq!(after[1].id, id(1, 4, 2, 2));
    assert_eq!(after[1].lines.start, line);
    assert!(after[1].lines.is_empty(), "新しい節は空: {:?}", after[1]);
    // 空の節に書くと、その位置に行が入る
    let written = sections::splice(&inserted, &after[1], "新しい節の本文。");
    let again = world_with(CONTENTS_PATH, &written);
    let list = sections::all(&again);
    assert_eq!(
        sections::text_of(again.text(CONTENTS_PATH), &list[1]),
        "新しい節の本文。"
    );
    assert_eq!(list.len(), 4);
}

#[test]
fn moving_by_scene_file_and_jump() {
    let project = world_with(
        "contents/01-05.txt",
        "@01-05-01 屋上\n一節目\n;\n二節目\n@01-05-02 帰り道\n三節目\n",
    );
    let list = sections::all(&project);
    let ids: Vec<SceneId> = list.iter().map(|item| item.id).collect();
    assert_eq!(
        ids,
        vec![
            id(1, 4, 2, 1),
            id(1, 4, 2, 2),
            id(1, 4, 2, 3),
            id(1, 5, 1, 1),
            id(1, 5, 1, 2),
            id(1, 5, 2, 1),
        ]
    );
    assert_eq!(sections::scene_step(&list, 1, true), Some(3));
    assert_eq!(sections::scene_step(&list, 4, true), Some(5));
    assert_eq!(sections::scene_step(&list, 5, false), Some(3));
    assert_eq!(sections::scene_step(&list, 4, false), Some(0));
    assert_eq!(sections::scene_step(&list, 0, false), None);
    assert_eq!(sections::file_step(&list, 1, true), Some(3));
    assert_eq!(sections::file_step(&list, 5, false), Some(0));
    assert_eq!(sections::file_step(&list, 5, true), None);
    // §9.7.2: 区切り無しも受ける。階層が足りなければその範囲の最初の節
    assert_eq!(sections::jump(&list, "0105"), Some(3));
    assert_eq!(sections::jump(&list, "@01-05-02"), Some(5));
    assert_eq!(sections::jump(&list, "01-04-02-03"), Some(2));
    assert_eq!(sections::jump(&list, "010402"), Some(0));
    assert_eq!(sections::jump(&list, "99"), None);
    assert_eq!(sections::jump(&list, "abc"), None);
    // 位置の復元（節が消えたら同じ場面の近い節）
    assert_eq!(
        sections::position_of(&list, "contents/01-05.txt", id(1, 5, 1, 2)),
        Some(4)
    );
    assert_eq!(
        sections::position_of(&list, "contents/01-05.txt", id(1, 5, 1, 9)),
        Some(4)
    );
}

#[test]
fn characters_are_counted_without_whitespace() {
    assert_eq!(sections::count_chars("あい う\nえ　お "), 5);
    assert_eq!(sections::count_chars(""), 0);
}

#[test]
fn the_reference_pane_shows_the_plot_cast_words_materials_and_diagnostics() {
    let project = world();
    let document: Document = project.document(CONTENTS_PATH).cloned().expect("doc");
    let list = sections::all(&project);
    // 2 節目: プロットは話プロットの 2 節目、資料は話プロットではなく本文の参照から
    let second = reference::at(&document, list[1].lines.clone(), list[1].id, &project);
    assert_eq!(
        second.plot,
        vec!["'アルス は地下遺跡の'星屑の時計 の謎を解こうとしていると明かす。".to_owned()]
    );
    let names: Vec<&str> = second.cast.iter().map(|cast| cast.name.as_str()).collect();
    assert_eq!(names, vec!["アルス", "リリア"]);
    // 上位プロット: 全体プロットの @01 の summary（章プロットは無い）
    assert_eq!(second.upper.len(), 1);
    assert_eq!(second.upper[0].id, "01");
    assert!(second.upper[0]
        .summary
        .starts_with("'アルス が地下遺跡に惹かれ"));
    // 3 節目: 話者注釈の修飾子（`@リリア:B`）は「その行のみ」なので、ペインは節の既定で決まる
    let third = reference::at(&document, list[2].lines.clone(), list[2].id, &project);
    assert_eq!(third.cast.len(), 1);
    assert_eq!(third.cast[0].name, "リリア");
    // 話プロットの 3 節目 `mode: 'リリア:B+C`（併用）
    assert_eq!(third.cast[0].modes, vec!["B 呆れ（強）", "C 好奇心"]);
    // 適用パターン = モード専用 ∪ キャラ共通（§5.5.5）
    assert_eq!(third.cast[0].tone, vec!["*わよ", "*のね"]);
}

#[test]
fn the_reference_pane_lists_words_materials_and_this_sections_errors() {
    let text = "@01-04-02 図書室の攻防\n'旧校舎の扉を開ける。'M0137 を見ながら。\n'未登録の語\n;\n次の節。\n";
    let project = world_with(CONTENTS_PATH, text);
    let document: Document = project.document(CONTENTS_PATH).cloned().expect("doc");
    let list = sections::all(&project);
    let first = reference::at(&document, list[0].lines.clone(), list[0].id, &project);
    assert_eq!(first.words.len(), 1);
    assert_eq!(first.words[0].term, "旧校舎");
    assert!(!first.words[0].definition.is_empty());
    assert_eq!(first.materials.len(), 1);
    assert_eq!(first.materials[0].id, "M0137");
    assert_eq!(
        first.materials[0].title,
        "科学の甲子園 本戦の運営と実技課題"
    );
    let codes: Vec<DiagnosticCode> = first.diagnostics.iter().map(|found| found.code).collect();
    assert!(codes.contains(&DiagnosticCode::E001), "{codes:?}");
    // 2 節目には 1 節目の診断を出さない
    let second = reference::at(&document, list[1].lines.clone(), list[1].id, &project);
    assert!(
        second
            .diagnostics
            .iter()
            .all(|found| found.code != DiagnosticCode::E001),
        "{:?}",
        second.diagnostics
    );
}

#[test]
fn the_reference_pane_shows_only_the_personality_of_the_effective_mode() {
    // §19.6「参照ペインに有効モードの性格だけが出る（他モードが出ないことをテスト）」
    let characters = world::CHARACTERS
        .replace(
            "### A 呆れ\nrange: @01 ..\n",
            "### A 呆れ\nrange: @01 ..\npersonality: 突き放す\n",
        )
        .replace(
            "### B 呆れ（強）\nrange: @01 ..\n",
            "### B 呆れ（強）\nrange: @01 ..\npersonality: 本気で怒る\n",
        );
    assert_ne!(characters, world::CHARACTERS);
    let project = world_with("characters.md", &characters);
    let document: Document = project.document(CONTENTS_PATH).cloned().expect("doc");
    let list = sections::all(&project);
    let lilia = |index: usize| {
        let content = reference::at(
            &document,
            list[index].lines.clone(),
            list[index].id,
            &project,
        );
        content
            .cast
            .into_iter()
            .find(|cast| cast.name == "リリア")
            .expect("リリア")
    };
    // 1 節目は場面の `mode: 'リリア:A`
    let first = lilia(0);
    assert_eq!(first.modes, vec!["A 呆れ"]);
    assert_eq!(first.personality.as_deref(), Some("突き放す"));
    // 3 節目は `mode: 'リリア:B+C`。A の性格は出ない
    let third = lilia(2);
    assert_eq!(third.modes, vec!["B 呆れ（強）", "C 好奇心"]);
    assert_eq!(third.personality.as_deref(), Some("本気で怒る"));
}

#[test]
fn removing_a_section_takes_its_separator_with_it() {
    let project = world();
    let list = sections::all(&project);
    let text = project.text(CONTENTS_PATH);
    // 真ん中の節: 後ろの `;` ごと消える
    let removed = sections::remove(text, &list[1]);
    let after = sections::all(&world_with(CONTENTS_PATH, &removed));
    assert_eq!(after.len(), 2);
    assert!(!removed.contains("旧校舎の地下遺跡"));
    assert_eq!(removed.matches("\n;\n").count(), 1);
    // 最後の節: 前の `;` が消え、ファイル末尾の改行は残る
    let removed = sections::remove(text, &list[2]);
    assert!(removed.ends_with("目を奪われた。\n"), "{removed:?}");
    assert_eq!(sections::all(&world_with(CONTENTS_PATH, &removed)).len(), 2);
    // 場面に 1 節しか無ければ、節の行だけ（見出しは残る）
    let single = "@01-04-02 図書室の攻防\n一節だけ。\n";
    let project = world_with(CONTENTS_PATH, single);
    let list = sections::all(&project);
    assert_eq!(
        sections::remove(single, &list[0]),
        "@01-04-02 図書室の攻防\n"
    );
}
