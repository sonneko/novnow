//! ワークスペース（M8、§10）のテスト。切替の規則・キー・`Workbench` の Slice は egui の画面を使わずに、
//! 画面の MUST（SceneId の維持・親プロットが常に見える・切替で保存）は `NovnowApp` を 1 フレームずつ
//! 回して確かめる。

#![cfg(feature = "ui")]

mod harness;
mod novel;
mod world;

use egui::{Event, Key, Modifiers};

use novnow::core::id::SceneId;
use novnow::core::model::document::Document;
use novnow::ui::shell::{self, Command};
use novnow::ui::workbench::{Nav, Reaction, Workbench};
use novnow::ui::workspaces::{Switcher, Workspace};

use harness::{key, painted_text, Harness};

fn id(chapter: u16, episode: u16, scene: u16, section: u16) -> SceneId {
    SceneId {
        chapter,
        episode,
        scene,
        section,
    }
}

const ALT: Modifiers = Modifiers {
    alt: true,
    ctrl: false,
    shift: false,
    mac_cmd: false,
    command: false,
};

const CTRL: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: false,
    mac_cmd: false,
    command: true,
};

const CTRL_ALT: Modifiers = Modifiers {
    alt: true,
    ctrl: true,
    shift: false,
    mac_cmd: false,
    command: true,
};

const CTRL_SHIFT: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: true,
    mac_cmd: false,
    command: true,
};

// ============================================================ 切替の規則（§10.0）

#[test]
fn workspaces_are_numbered_like_the_spec_table_and_reader_agent_are_reserved() {
    let ids: Vec<&str> = Workspace::ALL.iter().map(|found| found.id()).collect();
    assert_eq!(
        ids,
        vec![
            "write",
            "plot-episode",
            "plot-outline",
            "idea",
            "material",
            "cast",
            "reader",
            "agent",
            "review"
        ]
    );
    for (index, workspace) in Workspace::ALL.iter().enumerate() {
        assert_eq!(usize::from(workspace.number()), index + 1);
        assert_eq!(Workspace::of_number(workspace.number()), Some(*workspace));
    }
    assert_eq!(Workspace::of_number(0), None);
    assert_eq!(Workspace::of_number(10), None);
    assert!(!Workspace::Reader.available(), "追加仕様 #27 で ②");
    assert!(!Workspace::Agent.available(), "M10 まで欠番");
    assert_eq!(Workspace::available_all().count(), 7);
}

#[test]
fn the_switcher_remembers_the_previous_workspace_and_first_visits() {
    let mut switcher = Switcher::default();
    assert_eq!(switcher.current(), Workspace::Write);
    assert_eq!(switcher.previous(), None);
    assert!(!switcher.back(), "直前が無い");
    assert!(
        !switcher.switch(Workspace::Write),
        "同じものへは切り替えない"
    );
    assert!(!switcher.switch(Workspace::Reader), "欠番");
    assert!(switcher.first_visit(Workspace::PlotEpisode));
    assert!(switcher.switch(Workspace::PlotEpisode));
    assert!(
        !switcher.first_visit(Workspace::Write),
        "離れたものは訪問済み"
    );
    switcher.mark_visited(Workspace::PlotEpisode);
    assert!(switcher.switch(Workspace::Review));
    assert_eq!(switcher.previous(), Some(Workspace::PlotEpisode));
    assert!(switcher.back());
    assert_eq!(switcher.current(), Workspace::PlotEpisode);
    assert_eq!(switcher.previous(), Some(Workspace::Review));
    assert!(switcher.back(), "Alt+0 を続けると 2 つを行き来する");
    assert_eq!(switcher.current(), Workspace::Review);
}

#[test]
fn workspace_keys_follow_the_v1_table() {
    let of = |key, modifiers| shell::command_of(key, modifiers, false, true);
    assert_eq!(of(Key::B, CTRL), Some(Command::Up));
    assert_eq!(of(Key::Num1, ALT), Some(Command::Workspace(1)));
    assert_eq!(of(Key::Num9, ALT), Some(Command::Workspace(9)));
    assert_eq!(of(Key::Num0, ALT), Some(Command::PreviousWorkspace));
    assert_eq!(
        of(Key::B, CTRL_SHIFT),
        Some(Command::Up),
        "Ctrl+Shift+B も上へ"
    );
    assert_eq!(of(Key::N, CTRL_SHIFT), Some(Command::InsertSection));
    assert_eq!(of(Key::I, CTRL_SHIFT), Some(Command::Capture));
    assert_eq!(of(Key::E, CTRL_SHIFT), Some(Command::Review));
    // macOS: Option+数字は文字を打つので Ctrl+Option（§9.8 の注）
    let mac_alt = Modifiers {
        alt: true,
        ..Default::default()
    };
    let mac_ctrl_alt = Modifiers {
        alt: true,
        ctrl: true,
        ..Default::default()
    };
    assert_eq!(shell::command_of(Key::Num2, mac_alt, true, true), None);
    assert_eq!(
        shell::command_of(Key::Num2, mac_ctrl_alt, true, true),
        Some(Command::Workspace(2))
    );
    // Windows の AltGr（Ctrl+Alt）+ 数字は文字なので取らない
    assert_eq!(
        shell::command_of(Key::Num7, mac_ctrl_alt, false, true),
        None
    );
    // ワークスペースの中だけのキー（編集欄の単語削除・単語選択と重なる）
    let ctrl = CTRL;
    assert_eq!(
        shell::local_command_of(Key::Delete, ctrl, false, Workspace::Idea),
        Some(Command::Drop)
    );
    assert_eq!(
        shell::local_command_of(Key::Delete, ctrl, false, Workspace::Write),
        None
    );
    assert_eq!(
        shell::local_command_of(Key::ArrowRight, CTRL_SHIFT, false, Workspace::Review),
        Some(Command::Tab(true))
    );
    assert_eq!(
        shell::local_command_of(Key::ArrowRight, CTRL_SHIFT, false, Workspace::Write),
        None
    );
}

// ============================================================ Workbench の Slice（B-126）

#[test]
fn a_slice_edits_part_of_another_document_and_is_written_with_the_rest() {
    let mut bench = Workbench::new(novel::novel(), None, 400);
    let path = "plots/01-01.txt";
    bench.open_slice(path, 1..3, "test:1");
    assert_eq!(bench.slice_buffer(), "二人が図書室で話す。\nstate: fixed");
    bench.slice_buffer_mut().push_str("\nmode: 'アルス:A");
    bench.slice_edited(1.0);
    assert!(bench.unsaved().any(|found| found == path));
    // 同じ鍵なら開き直さない（中身は捨てない）
    bench.open_slice(path, 1..3, "test:1");
    assert!(bench.slice_buffer().ends_with("mode: 'アルス:A"));
    let writes = bench.take_writes();
    let (written_path, text) = writes
        .iter()
        .find(|(found, _)| found == path)
        .expect("話プロットを書く");
    assert_eq!(written_path, path);
    assert!(text
        .starts_with("@01-01-01 放課後\n二人が図書室で話す。\nstate: fixed\nmode: 'アルス:A\n;\n"));
    assert_eq!(bench.slice().map(|slice| slice.lines.clone()), Some(1..4));
    bench.write_finished(2.0, &[path.to_owned()], &[]);
    assert!(!bench.unsaved().any(|found| found == path));
}

#[test]
fn edits_made_while_writing_keep_the_slice_unsaved() {
    let mut bench = Workbench::new(novel::novel(), None, 400);
    let path = "plots/01-01.txt";
    bench.open_slice(path, 4..5, "test:2");
    bench.slice_buffer_mut().push('！');
    bench.slice_edited(1.0);
    let _ = bench.take_writes();
    bench.slice_buffer_mut().push('？');
    bench.write_finished(2.0, &[path.to_owned()], &[]);
    assert!(
        bench.unsaved().any(|found| found == path),
        "書いている間の打鍵はまだ書かれていない"
    );
}

#[test]
fn an_external_change_to_a_clean_slice_closes_it_and_a_dirty_one_conflicts() {
    let mut bench = Workbench::new(novel::novel(), None, 400);
    let path = "plots/01-01.txt";
    bench.open_slice(path, 4..5, "test:3");
    let reaction = bench.external_change(path, Some("@01-01-01 放課後\n書き換えた\n".into()));
    assert_eq!(reaction, Reaction::Reloaded);
    assert!(bench.slice().is_none(), "開いた側が鍵から開き直す");
    bench.open_slice(path, 1..2, "test:4");
    bench.slice_buffer_mut().push('！');
    bench.slice_edited(2.0);
    let reaction = bench.external_change(path, Some("外の版\n".into()));
    assert_eq!(reaction, Reaction::Conflict);
    assert!(bench.slice().is_some(), "衝突中は編集を捨てない");
}

#[test]
fn closing_or_switching_slices_commits_and_a_new_file_is_created_only_when_written() {
    let mut bench = Workbench::new(novel::novel(), None, 400);
    let path = "plots/01-02.txt";
    bench.open_slice(path, 0..0, "test:new");
    bench.close_slice();
    assert!(bench.project().document(path).is_none(), "空なら作らない");
    bench.open_slice(path, 0..0, "test:new");
    bench.slice_buffer_mut().push_str("@01-02-01 旧校舎");
    bench.slice_edited(1.0);
    bench.open_slice("plots/01-01.txt", 0..1, "test:other");
    assert_eq!(bench.project().text(path), "@01-02-01 旧校舎\n");
}

#[test]
fn applying_an_edit_to_another_file_keeps_unsaved_typing_in_the_section() {
    let mut bench = Workbench::new(novel::novel(), None, 400);
    bench.buffer_mut().push_str("（打鍵）");
    bench.edited(1.0);
    let edit = novnow::core::features::WorkspaceEdit::single(
        "words.md",
        vec![novnow::core::features::TextEdit {
            range: 0..0,
            new_text: "<!-- x -->\n".into(),
        }],
    );
    assert!(bench.apply_edit(&edit, 2.0));
    assert!(
        bench.buffer().ends_with("（打鍵）"),
        "他のファイルを変えても、書き戻していない打鍵は消えない"
    );
    assert!(bench.project().text("words.md").starts_with("<!-- x -->\n"));
}

#[test]
fn a_missing_prose_file_opens_as_a_new_section_and_is_written_with_its_header() {
    let mut bench = Workbench::new(novel::novel(), None, 400);
    let path = "contents/03-01.txt";
    assert!(bench.start_file(path, id(3, 1, 1, 0), "@03-01-01 新章"));
    assert_eq!(bench.current_id(), Some(id(3, 1, 1, 1)));
    bench.commit();
    assert!(
        bench.project().document(path).is_none(),
        "書き始めるまで作らない"
    );
    bench.buffer_mut().push_str("新しい章の始まり。");
    bench.edited(1.0);
    bench.commit();
    assert_eq!(
        bench.project().text(path),
        "@03-01-01 新章\n新しい章の始まり。\n"
    );
    assert_eq!(bench.current_id(), Some(id(3, 1, 1, 1)));
    assert!(bench.go(Nav::PrevFile));
    assert_eq!(bench.current_id().map(|found| found.chapter), Some(2));
}

// ============================================================ 画面（plot-episode）

fn novel_harness() -> Harness {
    Harness::start_with(novel::novel(), true).0
}

fn selected(harness: &Harness) -> Option<SceneId> {
    harness
        .app
        .spaces()
        .and_then(|spaces| spaces.episode.selected())
}

#[test]
fn alt_2_opens_the_episode_plot_at_the_same_scene_id_with_its_parent_visible() {
    let mut harness = novel_harness();
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 1))
    );
    harness.frame(vec![key(Key::Num2, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));
    assert_eq!(selected(&harness), Some(id(1, 1, 1, 1)), "SceneId を維持");
    let text = painted_text(&output);
    // 上段: 章プロットの該当話ブロック（親が常に見える、§19.5・§19.11）
    assert!(text.contains("@01-01 図書室の密談"), "{text}");
    assert!(
        text.contains("summary: 二人が図書室で出会い直す。"),
        "{text}"
    );
    // 中段: 節リスト
    assert!(text.contains("@01-01-01 放課後"), "{text}");
    assert!(text.contains("'アルス が資料を見せる。"), "{text}");
    assert!(text.contains("@01-01-02 帰り道"), "{text}");
    // 右: 本文の有無
    assert!(text.contains("本文"), "{text}");
    // 書いている節は write のまま
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 1))
    );
}

#[test]
fn typing_in_the_episode_editor_is_saved_and_alt_arrows_move_between_rows() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::End, Modifiers::NONE)]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("senses: 視覚".into())]);
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get("plots/01-01.txt")
        .cloned()
        .unwrap_or_default();
    assert!(
        written.contains("二人が図書室で話す。\nstate: fixed\nsenses: 視覚\n;\n"),
        "{written}"
    );
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    harness.frame(Vec::new());
    assert_eq!(selected(&harness), Some(id(1, 1, 1, 2)));
    harness.frame(vec![key(Key::ArrowRight, ALT)]);
    harness.frame(Vec::new());
    assert_eq!(selected(&harness), Some(id(1, 1, 2, 0)), "次の場面の見出し");
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    assert_eq!(selected(&harness), Some(id(1, 1, 1, 3)));
}

#[test]
fn ctrl_enter_goes_down_to_write_and_ctrl_b_comes_back_up() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    harness.frame(vec![key(Key::Enter, CTRL)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 2)),
        "同じ SceneId の節"
    );
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    harness.frame(vec![key(Key::B, CTRL)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));
    assert_eq!(
        selected(&harness),
        Some(id(1, 1, 1, 3)),
        "Ctrl+B は今の節へ"
    );
    // Alt+0 で直前（write）へ、もう一度で plot-episode へ
    harness.frame(vec![key(Key::Num0, ALT)]);
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    harness.frame(vec![key(Key::Num0, ALT)]);
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));
}

#[test]
fn each_workspace_keeps_its_own_position_after_switching_back() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    assert_eq!(selected(&harness), Some(id(1, 1, 1, 3)));
    harness.frame(vec![key(Key::Num1, ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_ALT)]);
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 2, 1, 1)),
        "write は次のファイルへ"
    );
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(Vec::new());
    assert_eq!(
        selected(&harness),
        Some(id(1, 1, 1, 3)),
        "2 回目からは自分の最後の位置（持ち越しは Ctrl+B だけ）"
    );
}

#[test]
fn reserved_workspaces_show_a_notice_and_do_not_switch() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num7, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert!(painted_text(&output).contains("読者 はこの版ではまだ使えません"));
    harness.frame(vec![key(Key::Num8, ALT)]);
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
}

#[test]
fn switching_workspaces_saves_without_asking() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![key(Key::End, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("あ".into())]);
    let before = harness.host.files.borrow().writes;
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(Vec::new());
    assert!(
        harness.host.files.borrow().writes > before,
        "切替は保存のきっかけ（§8.2）"
    );
    assert!(!harness.app.overlay_open(), "確認ダイアログは出さない");
}

#[test]
fn ctrl_shift_n_creates_the_missing_episode_plot_with_a_scene_header() {
    let mut harness = novel_harness();
    // contents/01-02.txt（話プロットが無い話）へ
    harness.frame(vec![key(Key::ArrowRight, CTRL_ALT)]);
    harness.frame(vec![key(Key::Num2, ALT)]);
    let output = harness.frame(Vec::new());
    assert!(painted_text(&output).contains("話プロット plots/01-02.txt はまだありません"));
    harness.frame(vec![key(Key::N, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text("扉を開ける".into())]);
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get("plots/01-02.txt")
        .cloned()
        .unwrap_or_default();
    assert_eq!(written, "@01-02-01\n扉を開ける\n");
    assert_eq!(selected(&harness), Some(id(1, 2, 1, 1)));
}

#[test]
fn going_down_to_a_missing_prose_file_does_not_create_it_until_typing() {
    let mut project = novel::novel();
    project.insert(Document::new(
        "plots/03-01.txt",
        "@03-01-01 新章\n始まり。\n;\n",
    ));
    let mut harness = Harness::start_with(project, true).0;
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(vec![key(Key::G, CTRL)]);
    harness.frame(vec![Event::Text("03-01-01-01".into())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(selected(&harness), Some(id(3, 1, 1, 1)));
    harness.frame(vec![key(Key::Enter, CTRL)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(3, 1, 1, 1))
    );
    harness.idle(0.6);
    assert!(!harness
        .host
        .files
        .borrow()
        .written
        .contains_key("contents/03-01.txt"));
    harness.frame(vec![Event::Text("新しい章。".into())]);
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get("contents/03-01.txt")
        .cloned()
        .unwrap_or_default();
    assert_eq!(written, "@03-01-01 新章\n新しい章。\n");
}

// ============================================================ 画面（plot-outline）

fn outline_selected(harness: &Harness) -> Option<SceneId> {
    harness
        .app
        .spaces()
        .and_then(|spaces| spaces.outline.selected())
}

#[test]
fn alt_3_opens_the_chapter_plot_with_the_parent_block_and_foreshadows() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num3, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotOutline));
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.outline.level()),
        Some(Some(1)),
        "write の章"
    );
    assert_eq!(
        outline_selected(&harness),
        Some(id(1, 1, 0, 0)),
        "write の話"
    );
    let text = painted_text(&output);
    // 親（全体プロットの章のブロック）が常に見える（§19.11）
    assert!(text.contains("@01 失われた刻印"), "{text}");
    assert!(text.contains("図書室の密談"), "{text}");
    assert!(text.contains("旧校舎へ"), "{text}");
    assert!(text.contains("伏線"), "{text}");
    assert!(text.contains("全体"), "{text}");
}

#[test]
fn the_four_levels_go_up_with_ctrl_b_and_down_with_ctrl_enter() {
    let mut harness = novel_harness();
    // write → plot-episode → plot-outline（章）→ 全体
    harness.frame(vec![key(Key::B, CTRL)]);
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));
    harness.frame(vec![key(Key::B, CTRL_SHIFT)]);
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotOutline));
    assert_eq!(outline_selected(&harness), Some(id(1, 1, 0, 0)));
    harness.frame(vec![key(Key::B, CTRL)]);
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.outline.level()),
        Some(None)
    );
    assert_eq!(outline_selected(&harness), Some(id(1, 0, 0, 0)));
    harness.frame(vec![key(Key::B, CTRL)]);
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.outline.level()),
        Some(None),
        "全体より上は無い"
    );
    // 全体 → 章 → 話（plot-episode）→ 本文（write）
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    assert_eq!(outline_selected(&harness), Some(id(2, 0, 0, 0)));
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    harness.frame(vec![key(Key::Enter, CTRL)]);
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.outline.level()),
        Some(Some(1))
    );
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    assert_eq!(outline_selected(&harness), Some(id(1, 2, 0, 0)));
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    harness.frame(vec![key(Key::Enter, CTRL)]);
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.episode.episode()),
        Some((1, 1))
    );
    assert_eq!(selected(&harness), Some(id(1, 1, 1, 0)), "話の最初の行");
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    harness.frame(vec![key(Key::Enter, CTRL)]);
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 1))
    );
}

#[test]
fn editing_a_block_in_the_outline_is_saved_to_the_chapter_plot() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num3, ALT)]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::End, Modifiers::NONE)]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("when: 春".into())]);
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get("plots/01.txt")
        .cloned()
        .unwrap_or_default();
    assert!(
        written.starts_with("// 章プロット\n@01-01 図書室の密談\nsummary: 二人が図書室で出会い直す。\nwhen: 春\n;\n"),
        "{written}"
    );
}

#[test]
fn ctrl_shift_n_in_the_outline_adds_a_block_and_ctrl_alt_arrows_change_the_level() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num3, ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_ALT)]);
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.outline.level()),
        Some(Some(2))
    );
    let output = harness.frame(Vec::new());
    assert!(painted_text(&output).contains("plots/02.txt にブロックはまだありません"));
    harness.frame(vec![key(Key::N, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(outline_selected(&harness), Some(id(2, 1, 0, 0)));
    harness.frame(vec![key(Key::End, Modifiers::NONE)]);
    harness.frame(vec![Event::Text(" 補習".into())]);
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get("plots/02.txt")
        .cloned()
        .unwrap_or_default();
    assert_eq!(written, "@02-01 補習\n;\n");
    harness.frame(vec![key(Key::ArrowLeft, CTRL_ALT)]);
    harness.frame(vec![key(Key::ArrowLeft, CTRL_ALT)]);
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.outline.level()),
        Some(None)
    );
}

// ============================================================ 画面（idea・即時入力）

const IDEAS: &str = "# I0007 学院長の動機
status: keep
tags: 学院長
学院長は何を守ろうとしているのか。

# I0008 時計塔の鐘
status: inbox
";

fn idea_harness() -> Harness {
    Harness::start_with(novel::novel_with("ideas.md", IDEAS), true).0
}

fn written(harness: &Harness, path: &str) -> String {
    harness
        .host
        .files
        .borrow()
        .written
        .get(path)
        .cloned()
        .unwrap_or_default()
}

#[test]
fn ctrl_shift_i_captures_an_idea_from_anywhere_without_changing_the_screen() {
    let mut harness = idea_harness();
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(vec![key(Key::I, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert!(harness.app.overlay_open());
    harness.frame(vec![Event::Text("鐘が 13 回鳴る".into())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    let output = harness.idle(0.6);
    assert!(!harness.app.overlay_open());
    assert_eq!(
        harness.app.workspace(),
        Some(Workspace::PlotEpisode),
        "画面遷移なし（§10.4 MUST）"
    );
    let ideas = written(&harness, "ideas.md");
    assert!(
        ideas.ends_with(
            "# I0009 鐘が 13 回鳴る\nstatus: inbox\ncreated: 2026-10-13T21:40:00+09:00\n"
        ),
        "{ideas}"
    );
    assert!(painted_text(&output).contains("I0009 を ideas.md に積みました"));
}

#[test]
fn the_idea_workspace_lists_ideas_drops_without_deleting_and_promotes_to_a_term() {
    let mut harness = idea_harness();
    harness.frame(vec![key(Key::Num4, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Idea));
    let text = painted_text(&output);
    assert!(text.contains("学院長の動機"), "{text}");
    assert!(text.contains("時計塔の鐘"), "{text}");
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.idea.selected().map(str::to_owned)),
        Some("I0007".to_owned())
    );
    // 昇格: 用語（2 番目）
    harness.frame(vec![key(Key::Enter, CTRL)]);
    harness.frame(Vec::new());
    assert!(harness.app.overlay_open(), "行き先を選ぶ");
    harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.idle(0.6);
    let words = written(&harness, "words.md");
    assert!(
        words.ends_with("# 学院長の動機\ndefinition: 学院長は何を守ろうとしているのか。\n"),
        "{words}"
    );
    let ideas = written(&harness, "ideas.md");
    assert!(
        ideas.starts_with("# I0007 学院長の動機\nstatus: promoted\ntags: 学院長\npromoted-to: words.md#学院長の動機\n"),
        "{ideas}"
    );
    // 捨てる: 次のアイデアへ。行は残る
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.idea.selected().map(str::to_owned)),
        Some("I0008".to_owned())
    );
    harness.frame(vec![key(Key::Delete, CTRL)]);
    harness.idle(0.6);
    let ideas = written(&harness, "ideas.md");
    assert!(
        ideas.contains("# I0008 時計塔の鐘\nstatus: dropped\n"),
        "{ideas}"
    );
}

#[test]
fn ctrl_delete_still_deletes_a_word_in_write() {
    let mut harness = idea_harness();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![key(Key::Delete, CTRL)]);
    harness.idle(0.6);
    assert!(
        !written(&harness, "ideas.md").contains("dropped"),
        "write では Ctrl+Delete を取らない"
    );
}

// ============================================================ 画面（review）

#[test]
fn ctrl_shift_e_opens_the_project_diagnostics_and_enter_jumps_to_the_section() {
    let mut project = novel::novel();
    // 本文 @01-02-01-03 に辞書に無い参照（E001）
    let text = project
        .text("contents/01-02.txt")
        .replace("扉の向こうは暗かった。", "扉の向こうは'未登録の闇 だった。");
    project.insert(Document::new("contents/01-02.txt", text));
    let mut harness = Harness::start_with(project, true).0;
    harness.frame(vec![key(Key::E, CTRL_SHIFT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Review));
    let text = painted_text(&output);
    assert!(text.contains("診断"), "{text}");
    assert!(text.contains("進捗"), "{text}");
    assert!(text.contains("contents/01-02.txt:"), "{text}");
    assert!(
        text.contains("[本文 @01-02-01-03]"),
        "作家の言葉での「どこ」: {text}"
    );
    // E001 の行まで ↓ で進めて Enter
    let index = harness
        .app
        .spaces()
        .and_then(|spaces| spaces.review.cached())
        .and_then(|rows| {
            rows.iter()
                .position(|row| row.code == "E001" && row.path == "contents/01-02.txt")
        })
        .expect("E001 の行");
    for _ in 0..index {
        harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    }
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 2, 1, 3))
    );
}

#[test]
fn the_progress_tab_lists_the_tree_and_opens_a_section() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num9, ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(text.contains("失われた刻印"), "{text}");
    assert!(text.contains("図書室の密談"), "{text}");
    assert!(text.contains("放課後"), "{text}");
    // 章 → 話 → 場面 → 節（@01-01-01-01）
    for _ in 0..3 {
        harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    }
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 1))
    );
}

// ============================================================ 画面（cast）

#[test]
fn the_cast_workspace_shows_the_timeline_and_moves_a_band_with_shift_arrows() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num6, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Cast));
    let text = painted_text(&output);
    assert!(text.contains("アルス"), "{text}");
    assert!(text.contains("リリア"), "{text}");
    assert!(text.contains("@01"), "{text}");
    assert!(text.contains("A 探究"), "{text}");
    assert!(text.contains("B 補習"), "{text}");
    // タブを 1 周してモードへ戻る（キーで切り替えるとタブの中身にフォーカスが移る）
    for _ in 0..3 {
        harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]);
    }
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    harness.frame(vec![key(Key::ArrowLeft, Modifiers::SHIFT)]);
    harness.idle(0.6);
    let characters = written(&harness, "characters.md");
    assert!(
        characters.contains("### B 補習\nrange: @01 ..\n"),
        "{characters}"
    );
}

#[test]
fn the_role_language_tester_and_the_unmatched_dialogue_list() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num6, ALT)]);
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text("そうなのね".into())]);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(text.contains("✓"), "{text}");
    assert!(text.contains("「*のね」"), "{text}");
    assert!(text.contains("「*わよ」"), "{text}");
    harness.frame(vec![key(Key::V, CTRL_SHIFT)]);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(text.contains("明日も来るの？"), "{text}");
}

// ============================================================ 画面（material）

const MATERIALS: &str = r#"{
  "schema_version": 1,
  "next_id": 140,
  "items": [
    {
      "id": "M0137",
      "title": "科学の甲子園 本戦の運営と実技課題",
      "path": "materials/docs/M0137-kagaku-koshien.md",
      "source": { "kind": "url", "url": "https://example.org/report/2026" },
      "category": "取材",
      "conditions": [ { "kind": "ref", "value": "'科学の甲子園" } ],
      "saved_at": "2026-10-01T09:20:33+09:00",
      "tags": ["大会"],
      "status": "approved",
      "added_by": "human",
      "summary": "本戦の 1 日のタイムテーブル。"
    },
    {
      "id": "M0138",
      "title": "旧校舎の歴史",
      "path": "materials/inbox/M0138.md",
      "source": { "kind": "custom", "text": "" },
      "category": "文献",
      "conditions": [],
      "saved_at": "2026-10-12T10:00:00+09:00",
      "tags": [],
      "status": "proposed",
      "added_by": "human"
    },
    {
      "id": "M0139",
      "title": "図書室の書架",
      "path": "materials/inbox/M0139.md",
      "source": { "kind": "custom", "text": "学校の案内" },
      "category": "取材",
      "conditions": [ { "kind": "scope", "value": "@01-01" } ],
      "saved_at": "2026-10-11T10:00:00+09:00",
      "tags": [],
      "status": "approved",
      "added_by": "human"
    }
  ]
}
"#;

const M0138: &str = "# 旧校舎の歴史\n<!-- novnow:M0138 -->\n\n明治の木造校舎。\n";

fn material_project() -> novnow::core::features::Project {
    let mut project = novel::novel_with("materials/index.json", MATERIALS);
    project.insert(Document::new("materials/inbox/M0138.md", M0138));
    project.insert(Document::new(
        "materials/inbox/M0139.md",
        "# 図書室の書架\n<!-- novnow:M0139 -->\n",
    ));
    project.insert(Document::new("materials/inbox/memo.md", "# 取材メモ\n"));
    project
}

fn material_harness() -> Harness {
    Harness::start_with(material_project(), true).0
}

fn material_selected(harness: &Harness) -> Option<String> {
    harness
        .app
        .spaces()
        .and_then(|spaces| spaces.material.selected().map(str::to_owned))
}

fn removed(harness: &Harness) -> Vec<String> {
    harness.host.files.borrow().removed.clone()
}

#[test]
fn material_keys_follow_the_v1_table() {
    let of = |key, modifiers| shell::command_of(key, modifiers, false, true);
    assert_eq!(of(Key::M, CTRL), Some(Command::MaterialSearch));
    assert_eq!(
        of(Key::M, CTRL_SHIFT),
        Some(Command::Modes),
        "Ctrl+Shift+M は全モード"
    );
    assert_eq!(
        shell::local_command_of(Key::Delete, CTRL, false, Workspace::Material),
        Some(Command::Drop)
    );
}

#[test]
fn alt_5_lists_materials_and_ctrl_enter_approves_after_confirming_a_missing_source() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num5, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Material));
    let text = painted_text(&output);
    assert!(text.contains("科学の甲子園 本戦の運営と実技課題"), "{text}");
    assert!(text.contains("旧校舎の歴史"), "{text}");
    assert!(
        text.contains("inbox/memo.md"),
        "索引に無い資料 MD も並ぶ: {text}"
    );
    // 着地: 今の節（@01-01-01-01）に明示参照は無いので、条件一致の候補（M0139 の scope @01-01）
    assert_eq!(material_selected(&harness).as_deref(), Some("M0139"));
    // 並びは索引に無い資料 MD → saved_at の新しい順（memo.md・M0138・M0139・M0137）
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(material_selected(&harness).as_deref(), Some("M0138"));
    let text = painted_text(&output);
    assert!(text.contains("明治の木造校舎。"), "中央は資料 MD: {text}");
    assert!(
        text.contains("出典が設定されていません: M0138"),
        "審査フォームに W065: {text}"
    );
    // 出典が無いので確認を挟む（§16.7）
    harness.frame(vec![key(Key::Enter, CTRL)]);
    harness.frame(Vec::new());
    assert!(harness.app.overlay_open(), "確認");
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    let output = harness.idle(0.6);
    assert!(!harness.app.overlay_open());
    assert_eq!(written(&harness, "materials/docs/M0138.md"), M0138);
    assert_eq!(
        removed(&harness),
        vec!["materials/inbox/M0138.md".to_owned()]
    );
    let index = written(&harness, "materials/index.json");
    assert!(
        index.contains("\"path\": \"materials/docs/M0138.md\""),
        "{index}"
    );
    assert!(index.contains("\"status\": \"approved\""), "{index}");
    assert!(
        painted_text(&output).contains("M0138 を採用しました"),
        "通知"
    );
    // 保存した資料 MD の checksum が索引に入る（§16.6）
    let checksum = novnow::core::features::materials_ops::checksum(M0138);
    assert!(index.contains(&checksum), "{index}");
    assert_eq!(material_selected(&harness).as_deref(), Some("M0138"));
}

#[test]
fn ctrl_enter_registers_an_unregistered_file_and_ctrl_delete_rejects_without_deleting() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num5, ALT)]);
    harness.frame(Vec::new());
    // 先頭の索引に無い資料 MD を登録する（W064 の「登録する」、§16.2）
    harness.frame(vec![key(Key::Home, CTRL)]);
    assert_eq!(
        material_selected(&harness).as_deref(),
        Some("materials/inbox/memo.md")
    );
    harness.frame(vec![key(Key::Enter, CTRL)]);
    harness.idle(0.6);
    assert_eq!(material_selected(&harness).as_deref(), Some("M0140"));
    assert_eq!(
        written(&harness, "materials/inbox/memo.md"),
        "# 取材メモ\n<!-- novnow:M0140 -->\n"
    );
    let index = written(&harness, "materials/index.json");
    assert!(index.contains("\"id\": \"M0140\""), "{index}");
    assert!(index.contains("\"next_id\": 141"), "{index}");
    // 却下: ファイルは残し、既定の一覧からは消える
    let entries = harness
        .app
        .spaces()
        .map(|spaces| spaces.material.selected().map(str::to_owned))
        .unwrap_or_default();
    assert_eq!(entries.as_deref(), Some("M0140"));
    harness.frame(vec![key(Key::Delete, CTRL)]);
    let output = harness.idle(0.6);
    let index = written(&harness, "materials/index.json");
    assert!(index.contains("\"status\": \"rejected\""), "{index}");
    assert!(removed(&harness).is_empty(), "却下はファイルを消さない");
    assert_ne!(material_selected(&harness).as_deref(), Some("M0140"));
    assert!(painted_text(&output).contains("M0140 を却下しました"));
}

#[test]
fn the_review_form_writes_the_index_and_keeps_the_h1_in_step() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num5, ALT)]);
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    assert_eq!(material_selected(&harness).as_deref(), Some("M0138"));
    // Ctrl+3 で審査フォームのタイトルへ
    harness.frame(vec![key(Key::Num3, CTRL)]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::End, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("（増補）".into())]);
    harness.idle(1.0);
    let index = written(&harness, "materials/index.json");
    assert!(
        index.contains("\"title\": \"旧校舎の歴史（増補）\""),
        "{index}"
    );
    assert!(
        index.contains("\"updated_at\": \"2026-10-13T21:40:00+09:00\""),
        "{index}"
    );
    let body = written(&harness, "materials/inbox/M0138.md");
    assert!(
        body.starts_with("# 旧校舎の歴史（増補）\n"),
        "H1 も揃える: {body}"
    );
}

#[test]
fn ctrl_m_searches_materials_and_inserts_a_reference_at_the_caret_in_write() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num2, Modifiers::CTRL)]);
    harness.frame(vec![key(Key::M, CTRL)]);
    harness.frame(Vec::new());
    assert!(harness.app.overlay_open());
    harness.frame(vec![Event::Text("甲子園".into())]);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(
        text.contains("M0137  科学の甲子園 本戦の運営と実技課題  #取材  2026-10-01"),
        "{text}"
    );
    assert!(
        text.contains("本戦の 1 日のタイムテーブル。"),
        "選んだ行の要約: {text}"
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.idle(0.6);
    assert!(!harness.app.overlay_open());
    let prose = written(&harness, "contents/01-01.txt");
    assert!(prose.contains("'M0137"), "{prose}");
    // Ctrl+Enter: 検索語から新しい資料（提案）を作って挿入する
    harness.frame(vec![key(Key::M, CTRL)]);
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text("時計塔の構造 #文献".into())]);
    harness.frame(vec![key(Key::Enter, CTRL)]);
    harness.idle(0.6);
    let index = written(&harness, "materials/index.json");
    assert!(index.contains("\"title\": \"時計塔の構造\""), "{index}");
    assert!(index.contains("\"category\": \"文献\""), "{index}");
    assert!(
        written(&harness, "materials/inbox/M0140.md").starts_with("# 時計塔の構造\n"),
        "提案の MD"
    );
    let prose = written(&harness, "contents/01-01.txt");
    assert!(prose.contains("'M0140"), "{prose}");
}

#[test]
fn ctrl_m_in_the_episode_plot_appends_a_material_line_to_the_section() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(vec![key(Key::M, CTRL)]);
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text("M0137".into())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.idle(0.6);
    let plot = written(&harness, "plots/01-01.txt");
    assert!(
        plot.starts_with(
            "@01-01-01 放課後\n二人が図書室で話す。\nstate: fixed\nmaterial: 'M0137\n;\n"
        ),
        "{plot}"
    );
}

#[test]
fn a_condition_match_in_the_reference_pane_is_added_to_the_plot_with_enter() {
    let mut harness = material_harness();
    // @01-01-01-01: M0139 の scope（@01-01）に入る
    harness.idle(0.3);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(text.contains("図書室の書架"), "条件一致の候補: {text}");
    harness.frame(vec![key(Key::Num3, CTRL)]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.idle(0.6);
    let plot = written(&harness, "plots/01-01.txt");
    assert!(
        plot.starts_with(
            "@01-01-01 放課後\n二人が図書室で話す。\nstate: fixed\nmaterial: 'M0139\n;\n"
        ),
        "{plot}"
    );
}

#[test]
fn review_rows_for_materials_open_the_material_workspace() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::E, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    let rows = harness
        .app
        .spaces()
        .and_then(|spaces| spaces.review.cached())
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    let w065 = rows
        .iter()
        .position(|row| row.code == "W065")
        .expect("W065 の行");
    for _ in 0..w065 {
        harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    }
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Material));
    assert_eq!(material_selected(&harness).as_deref(), Some("M0138"));
    // W064（索引に無い資料 MD）も開ける
    let mut harness = material_harness();
    harness.frame(vec![key(Key::E, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    let rows = harness
        .app
        .spaces()
        .and_then(|spaces| spaces.review.cached())
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    let w064 = rows
        .iter()
        .position(|row| row.code == "W064")
        .expect("W064 の行");
    for _ in 0..w064 {
        harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    }
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Material));
    assert_eq!(
        material_selected(&harness).as_deref(),
        Some("materials/inbox/memo.md")
    );
}

#[test]
fn the_palette_searches_materials_with_hash_and_the_dictionary_with_a_quote() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::P, CTRL)]);
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text("#取材 書架".into())]);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(
        text.contains("M0139  図書室の書架  #取材  2026-10-11"),
        "{text}"
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Material));
    assert_eq!(material_selected(&harness).as_deref(), Some("M0139"));
    // `'` は辞書（キャラ → cast で開く）
    harness.frame(vec![key(Key::P, CTRL)]);
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text("'りりあ".into())]);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(text.contains("'リリア  キャラ"), "読みでも引ける: {text}");
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Cast));
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.cast.selected()),
        Some(1)
    );
}
