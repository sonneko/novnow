//! プロットのワークスペースの行の並び（`core::features::plots`、M8、`PLANS.md` §9.2 B-127）。

mod novel;
mod world;

use novnow::core::export::State;
use novnow::core::features::plots::{self, EpisodeRow};
use novnow::core::features::sections;
use novnow::core::id::{Digits, SceneId};

fn id(chapter: u16, episode: u16, scene: u16, section: u16) -> SceneId {
    SceneId {
        chapter,
        episode,
        scene,
        section,
    }
}

fn digits() -> Digits {
    novel::novel().digits()
}

/// 行の中身（編集する行を LF でつないだもの）。
fn text_of(project: &novnow::core::features::Project, path: &str, row: &EpisodeRow) -> String {
    sections::text_of_lines(project.text(path), &row.lines)
}

#[test]
fn episode_rows_are_scene_headers_and_sections_with_their_effective_state() {
    let project = novel::novel();
    let rows = plots::episode_rows(&project, 1, 1);
    let ids: Vec<SceneId> = rows.iter().map(|row| row.id).collect();
    assert_eq!(
        ids,
        vec![
            id(1, 1, 1, 0),
            id(1, 1, 1, 1),
            id(1, 1, 1, 2),
            id(1, 1, 1, 3),
            id(1, 1, 2, 0),
            id(1, 1, 2, 1),
        ]
    );
    let path = "plots/01-01.txt";
    assert!(rows[0].is_scene());
    assert_eq!(rows[0].summary, "放課後");
    assert_eq!(text_of(&project, path, &rows[0]), "@01-01-01 放課後");
    assert_eq!(rows[0].state, None, "場面の行に状態は付けない");
    assert_eq!(
        text_of(&project, path, &rows[1]),
        "二人が図書室で話す。\nstate: fixed"
    );
    assert_eq!(rows[1].summary, "二人が図書室で話す。");
    // 実効の状態（B-122）: 節 → 全体プロットの章 → 節 → 章
    let states: Vec<Option<State>> = rows.iter().map(|row| row.state).collect();
    assert_eq!(
        states,
        vec![
            None,
            Some(State::Fixed),
            Some(State::Review),
            Some(State::Draft),
            None,
            Some(State::Review),
        ]
    );
    assert_eq!(rows[4].summary, "帰り道");
    assert_eq!(text_of(&project, path, &rows[5]), "二人で帰る。");
}

#[test]
fn a_missing_episode_plot_has_no_rows_and_the_nearest_row_falls_back() {
    let project = novel::novel();
    assert!(plots::episode_rows(&project, 1, 2).is_empty());
    let rows = plots::episode_rows(&project, 1, 1);
    assert_eq!(plots::row_near(&rows, id(1, 1, 1, 2)), Some(2));
    assert_eq!(
        plots::row_near(&rows, id(1, 1, 2, 9)),
        Some(4),
        "無い節は同じ場面の見出し"
    );
    assert_eq!(
        plots::row_near(&rows, id(1, 1, 7, 1)),
        Some(0),
        "無い場面は先頭"
    );
    assert_eq!(plots::row_near(&[], id(1, 1, 1, 1)), None);
}

/// 挿入した後の話プロットを読み直した行。
fn rows_after(text: &str) -> (novnow::core::features::Project, Vec<EpisodeRow>) {
    let project = novel::novel_with("plots/01-01.txt", text);
    let rows = plots::episode_rows(&project, 1, 1);
    (project, rows)
}

#[test]
fn inserting_after_a_closed_section_adds_one_break_and_an_empty_section() {
    let project = novel::novel();
    let rows = plots::episode_rows(&project, 1, 1);
    let text = project.text("plots/01-01.txt");
    let (updated, at) = plots::insert_section(text, &rows, Some(1), id(1, 1, 1, 0), digits());
    let (_, after) = rows_after(&updated);
    let ids: Vec<SceneId> = after.iter().map(|row| row.id).collect();
    assert_eq!(ids.len(), rows.len() + 1);
    assert_eq!(after[2].id, id(1, 1, 1, 2));
    assert!(after[2].lines.is_empty(), "新しい節は空");
    assert_eq!(after[2].lines.start, at);
    assert_eq!(
        after[3].summary, "'アルス が資料を見せる。",
        "元の 2 番目は 3 番目"
    );
    // 書くと `;` の前に入る
    let written = sections::splice_lines(&updated, &after[2].lines, "新しい節");
    let (_, again) = rows_after(&written);
    assert_eq!(again[2].summary, "新しい節");
    assert_eq!(again[3].summary, "'アルス が資料を見せる。");
}

#[test]
fn inserting_after_the_last_unclosed_section_closes_it_first() {
    let project = novel::novel();
    let rows = plots::episode_rows(&project, 1, 1);
    let text = project.text("plots/01-01.txt");
    // `@01-01-02` の節 1 は `;` で閉じられていない（ファイルの最後）
    let (updated, at) = plots::insert_section(text, &rows, Some(5), id(1, 1, 2, 0), digits());
    let (_, after) = rows_after(&updated);
    assert_eq!(after.len(), rows.len() + 1);
    assert_eq!(after[5].summary, "二人で帰る。");
    assert_eq!(after[6].id, id(1, 1, 2, 2));
    assert_eq!(after[6].lines.start, at);
    assert!(after[6].lines.is_empty());
    assert!(updated.ends_with("二人で帰る。\n;\n;\n"), "{updated:?}");
}

#[test]
fn inserting_at_a_scene_row_puts_an_empty_first_section_before_the_old_first() {
    let project = novel::novel();
    let rows = plots::episode_rows(&project, 1, 1);
    let text = project.text("plots/01-01.txt");
    let (updated, at) = plots::insert_section(text, &rows, Some(4), id(1, 1, 2, 0), digits());
    let (_, after) = rows_after(&updated);
    assert_eq!(after.len(), rows.len() + 1);
    assert_eq!(after[5].id, id(1, 1, 2, 1));
    assert!(after[5].lines.is_empty());
    assert_eq!(after[5].lines.start, at);
    assert_eq!(after[6].summary, "二人で帰る。");
}

#[test]
fn inserting_into_a_missing_episode_plot_writes_a_scene_header() {
    let (text, at) = plots::insert_section("", &[], None, id(1, 2, 3, 0), digits());
    assert_eq!(text, "@01-02-03\n");
    assert_eq!(at, 1);
    // 見出しだけの場面には、見出しの直後に空の節の行がある（書き始める位置）
    let project = novel::novel_with("plots/01-02.txt", &text);
    let rows = plots::episode_rows(&project, 1, 2);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].id, id(1, 2, 3, 1));
    assert_eq!(rows[1].lines, 1..1);
    let written = sections::splice_lines(&text, &(at..at), "最初の節");
    assert_eq!(written, "@01-02-03\n最初の節\n");
    let project = novel::novel_with("plots/01-02.txt", &written);
    let rows = plots::episode_rows(&project, 1, 2);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].id, id(1, 2, 3, 1));
    assert_eq!(rows[1].summary, "最初の節");
    // 前文だけのファイル: 空行を挟んで足す
    let (text, at) = plots::insert_section("// メモ\n", &[], None, id(1, 2, 1, 0), digits());
    assert_eq!(text, "// メモ\n\n@01-02-01\n");
    assert_eq!(at, 3);
}

#[test]
fn outline_blocks_list_chapters_and_episodes_with_their_summary_and_state() {
    let project = novel::novel();
    let all = plots::outline_blocks(&project, None);
    let ids: Vec<SceneId> = all.iter().map(|block| block.id).collect();
    assert_eq!(ids, vec![id(1, 0, 0, 0), id(2, 0, 0, 0)]);
    assert_eq!(all[0].title, "失われた刻印");
    assert_eq!(all[0].summary, "'アルス が地下遺跡に惹かれる導入章。");
    assert_eq!(all[0].state, Some(State::Review));
    assert_eq!(all[1].state, None);
    assert_eq!(all[0].path, "plots/all.txt");
    // 見出しから（`;` と末尾の空行を含まない）
    assert_eq!(
        sections::text_of_lines(project.text("plots/all.txt"), &all[0].lines),
        "@01 失われた刻印\nsummary: 'アルス が地下遺跡に惹かれる導入章。\nstate: review"
    );
    let chapter = plots::outline_blocks(&project, Some(1));
    assert_eq!(chapter.len(), 2);
    assert_eq!(chapter[1].id, id(1, 2, 0, 0));
    assert_eq!(chapter[1].state, Some(State::Fixed));
    let block = plots::outline_block(&project, 1, 2).expect("@01-02");
    assert_eq!(block.title, "旧校舎へ");
    assert!(
        plots::outline_block(&project, 2, 1).is_none(),
        "章プロットが無い"
    );
}

#[test]
fn chapters_and_episodes_come_from_plots_and_prose_files() {
    let project = novel::novel();
    assert_eq!(plots::chapters(&project), vec![1, 2]);
    assert_eq!(plots::episodes(&project, 1), vec![1, 2]);
    assert_eq!(plots::episodes(&project, 2), vec![1]);
    assert_eq!(plots::all_episodes(&project), vec![(1, 1), (1, 2), (2, 1)]);
}

#[test]
fn foreshadows_pair_seeds_and_payoffs_and_list_the_open_ones_first() {
    let project = novel::novel_with(
        "plots/01.txt",
        "@01-01 図書室の密談\nseed: F01 古い写し -> @02-01\nseed: F02 設計図の欠け\n;\n\n@01-02 旧校舎へ\npayoff: F03\n;\n",
    );
    let mut project = project;
    project.insert(novnow::core::model::document::Document::new(
        "plots/02-01.txt",
        "@02-01-01 補習\npayoff: F01\n補習で写しの意味に気づく。\n;\n",
    ));
    let found = plots::foreshadows(&project);
    let tags: Vec<&str> = found.iter().map(|item| item.tag.as_str()).collect();
    assert_eq!(tags, vec!["F02", "F01", "F03"], "未回収が上");
    assert!(found[0].open());
    assert_eq!(found[0].seeds[0].note, "設計図の欠け");
    assert_eq!(found[0].seeds[0].block, Some(id(1, 1, 0, 0)));
    let f01 = &found[1];
    assert!(!f01.open());
    assert_eq!(f01.seeds[0].target, "@02-01");
    assert_eq!(f01.payoffs[0].path, "plots/02-01.txt");
    assert_eq!(f01.payoffs[0].block, Some(id(2, 1, 1, 0)));
    assert!(found[2].seeds.is_empty(), "設置の無い回収");
}
