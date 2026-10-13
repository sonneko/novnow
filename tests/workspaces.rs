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
