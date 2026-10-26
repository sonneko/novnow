//! 節単位の編集と節の並び（`core::features::sections`、§8.1・§9.3・§9.8.2）と、
//! 参照ペインの中身（`core::features::reference`、§9.5.2）。M7。

mod world;

use novnow::core::diagnostics::DiagnosticCode;
use novnow::core::features::{reference, sections};
use novnow::core::id::SceneId;
use novnow::core::model::document::Document;

use world::{world, world_with, CONTENTS_PATH, EPISODE_PATH};

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

#[test]
fn pasting_text_with_a_break_line_splits_it_into_sections() {
    let current = "前の行。";
    assert_eq!(
        sections::split_pasted_sections(
            current,
            current.len(),
            "あたらしい行。\nつぎの行。\n;\nあとの節。"
        ),
        Some(vec![
            "前の行。あたらしい行。\nつぎの行。".to_owned(),
            "あとの節。".to_owned()
        ]),
        "§8.1.3: 単独 `;` の行で節に分ける（改行はそのまま）"
    );
    // 行末コメント付きの区切りも同じ（§5.2.1 `section_break = ws? , ";" , [ comment ] , eol`）
    assert_eq!(
        sections::split_pasted_sections("", 0, ";  // メモ\nつづき。"),
        Some(vec![String::new(), "つづき。".to_owned()])
    );
}

#[test]
fn pasting_a_scene_header_starts_a_new_section_and_keeps_the_line() {
    assert_eq!(
        sections::split_pasted_sections("", 0, "本文。\n;\n@01-04-03 屋上\n屋上の行。"),
        Some(vec![
            "本文。".to_owned(),
            "@01-04-03 屋上\n屋上の行。".to_owned()
        ]),
        "見出し行は作者が貼ったまま本文ファイルへ入れる"
    );
    // 4 階層（同じ場面の節番号、§5.2.2）も節の切れ目
    assert_eq!(
        sections::split_pasted_sections("", 0, "@01-04-02-03 つづき"),
        Some(vec![String::new(), "@01-04-02-03 つづき".to_owned()])
    );
}

#[test]
fn a_paste_without_breaks_or_headers_is_not_split() {
    assert_eq!(
        sections::split_pasted_sections("本文", "本文".len(), "一行。\n二行目。"),
        None,
        "改行はそのまま入る（分ける必要が無い）"
    );
    assert_eq!(
        sections::split_pasted_sections("", 0, "@メモ\n本文。"),
        None,
        "見出しの形をしていない `@` の行は切れ目ではない"
    );
}

#[test]
fn pasting_in_the_middle_leaves_the_rest_in_the_last_section() {
    let current = "前半後半";
    assert_eq!(
        sections::split_pasted_sections(current, "前半".len(), ";\n新しい節。"),
        Some(vec!["前半".to_owned(), "新しい節。後半".to_owned()])
    );
}

#[test]
fn reference_rows_carry_their_navigation_targets() {
    use novnow::core::features::reference::Nav;
    let project = world();
    let document: Document = project.document(CONTENTS_PATH).cloned().expect("doc");
    let list = sections::all(&project);
    let second = reference::at(&document, list[1].lines.clone(), list[1].id, &project);
    // プロット（と五感・空間・挿絵）は話プロットの当該節
    assert_eq!(second.plot_nav, Some(Nav::Plot(list[1].id)));
    // 上位プロットは全体プロットの @01 ブロック
    assert!(matches!(second.upper[0].nav, Nav::Outline(id) if id.chapter == 1 && id.episode == 0));
    // 配役はキャラの順序
    assert!(second
        .cast
        .iter()
        .all(|cast| matches!(cast.nav, Nav::Character(_))));
    // 用語は全文表示、資料は明示参照＝開く・候補＝追加
    assert!(second
        .words
        .iter()
        .all(|word| matches!(word.nav, Nav::Term(_))));
    for material in &second.materials {
        if material.candidate {
            assert_eq!(material.nav, Nav::AddMaterial(material.id.clone()));
        } else {
            assert_eq!(material.nav, Nav::OpenMaterial(material.id.clone()));
        }
    }
}

#[test]
fn split_body_splits_at_the_byte_without_snapping() {
    // 行の途中でもそのまま割る（§8.1.1。注釈・参照の途中でも安全な位置へ寄せない）
    assert_eq!(
        sections::split_body("abcdef", 3),
        Some(("abc".to_owned(), "def".to_owned()))
    );
    // 行境界で割っても空行は増えない
    assert_eq!(
        sections::split_body("ab\ncd", 3),
        Some(("ab".to_owned(), "cd".to_owned()))
    );
    assert_eq!(
        sections::split_body("ab\ncd", 2),
        Some(("ab".to_owned(), "cd".to_owned()))
    );
    // 話者注釈の途中
    let body = "台詞」@リリア";
    let at = body.find('@').unwrap();
    assert_eq!(
        sections::split_body(body, at),
        Some(("台詞」".to_owned(), "@リリア".to_owned()))
    );
    // 先頭・末尾・文字境界でない位置では割らない
    assert_eq!(sections::split_body("abc", 0), None);
    assert_eq!(sections::split_body("abc", 3), None);
    assert_eq!(sections::split_body("あい", 1), None);
    assert_eq!(sections::split_body("", 0), None);
}

#[test]
fn unwritten_uses_the_w012_definition() {
    assert!(sections::is_unwritten(""));
    assert!(sections::is_unwritten("  \n\n "));
    // 行コメントは内容として数える（§8.1.1）
    assert!(!sections::is_unwritten("// メモ"));
    assert!(!sections::is_unwritten("本文"));
}

#[test]
fn plot_blocks_are_found_by_id_and_removed_with_their_separator() {
    use novnow::core::features::plots;
    let project = world();
    let blocks = plots::section_blocks(&project, id(1, 4, 2, 2));
    assert_eq!(blocks.len(), 1);
    let text = plots::remove_block(project.text(EPISODE_PATH), &blocks[0]);
    assert!(!text.contains("謎を解こうとしていると明かす"), "{text}");
    assert!(text.contains("幼馴染の'リリア"));
    assert!(text.contains("放課後に旧校舎へ向かうことを決める"));
    // 消した後は番号が繰り上がる（位置から決まる、§5.2.2）
    let moved = world_with(EPISODE_PATH, &text);
    let rows = plots::episode_rows(&moved, 1, 4);
    let second = rows
        .iter()
        .find(|row| row.id == id(1, 4, 2, 2))
        .expect("元の 3 番目が 2 番になる");
    assert!(second.summary.contains("呆れつつも") || second.summary.contains("mode"));
    // 同じ場面見出しが重複すると対応は一意でない
    let duplicated = world_with(
        EPISODE_PATH,
        "@01-04-02 題\nプロット一\n;\n@01-04-02 題\nプロット二\n",
    );
    assert_eq!(plots::section_blocks(&duplicated, id(1, 4, 2, 1)).len(), 2);
    // 話プロットが無ければ対応ブロックは無い
    assert!(plots::section_blocks(&project, id(9, 9, 9, 9)).is_empty());
}
