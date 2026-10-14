//! ワークスペース（M8、§10）のテスト。切替の規則・キー・`Workbench` の Slice は egui の画面を使わずに、
//! 画面の MUST（SceneId の維持・親プロットが常に見える・切替で保存）は `NovnowApp` を 1 フレームずつ
//! 回して確かめる。

#![cfg(feature = "ui")]

mod harness;
mod novel;
mod world;

use egui::{Event, Key, Modifiers};

use novnow::core::agents::files::Board;
use novnow::core::agents::runner;
use novnow::core::id::SceneId;
use novnow::core::model::document::Document;
use novnow::ports::mcp::{McpError, McpReply};
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
fn workspaces_are_numbered_like_the_spec_table_and_reader_is_reserved() {
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
    assert!(Workspace::Agent.available(), "M10 で入った");
    assert_eq!(Workspace::available_all().count(), 8);
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
fn the_reserved_reader_shows_a_notice_and_does_not_switch() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num7, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert!(painted_text(&output).contains("読者 はこの版ではまだ使えません"));
    harness.frame(vec![key(Key::Num8, ALT)]);
    assert_eq!(
        harness.app.workspace(),
        Some(Workspace::Agent),
        "agent は M10 で入った"
    );
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

// ============================================================ インレイヒント（§6.6.2、B-137）

#[test]
fn a_material_id_in_the_episode_plot_shows_its_title_as_an_inlay_hint_and_the_file_keeps_the_id() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::End, Modifiers::NONE)]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("material: 'M0137".into())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("material: 'M0138".into())]);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(
        text.contains("⟨科学の甲子園 本戦の運営と実技課題⟩"),
        "{text}"
    );
    assert!(
        text.contains("⟨旧校舎の歴史・提案中⟩"),
        "提案中の資料には添える: {text}"
    );
    harness.idle(0.6);
    let plot = written(&harness, "plots/01-01.txt");
    assert!(
        plot.contains("state: fixed\nmaterial: 'M0137\nmaterial: 'M0138\n;\n"),
        "{plot}"
    );
    assert!(!plot.contains('⟨'), "ヒントはファイルに書かない: {plot}");
    // Alt+H で消え、ui.json に残り、もう一度で戻る
    harness.frame(vec![key(Key::H, ALT)]);
    let output = harness.frame(Vec::new());
    assert!(!painted_text(&output).contains("⟨科学の甲子園"));
    harness.idle(0.3);
    let ui_json = written(&harness, ".novnow/ui.json");
    assert!(ui_json.contains("\"inlay_hints\": false"), "{ui_json}");
    harness.frame(vec![key(Key::H, ALT)]);
    let output = harness.frame(Vec::new());
    assert!(painted_text(&output).contains("⟨科学の甲子園 本戦の運営と実技課題⟩"));
}

#[test]
fn the_section_editor_shows_a_material_title_and_a_mode_name_after_their_tokens() {
    let mut harness = novel_harness();
    // @01-01-01-02: 「これに載ってたんだ」'M0137 @アルス:B（B = 補習）
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 2))
    );
    let text = painted_text(&output);
    assert!(
        text.contains("⟨科学の甲子園 本戦の運営と実技課題⟩"),
        "{text}"
    );
    assert!(text.contains("⟨補習⟩"), "{text}");
    let buffer = harness.app.workbench().expect("open").buffer().to_owned();
    assert!(buffer.contains("'M0137 @アルス:B"), "{buffer}");
    assert!(!buffer.contains('⟨'));
}

#[test]
fn ctrl_period_in_the_episode_plot_offers_the_e006_fixes_and_enter_applies_the_first() {
    // 資料 ID はプロットに書く（§6.6.4）ので、E006 のクイックフィックス（§6.6.3）も話プロットで効く
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::End, Modifiers::NONE)]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("material: 'M0137:科学の甲子園".into())]);
    harness.frame(vec![key(Key::ArrowLeft, Modifiers::NONE)]);
    let period = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    harness.frame(vec![key(Key::Period, period)]);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(
        text.contains("E006  索引のタイトルに揃える: 科学の甲子園 本戦の運営と実技課題"),
        "{text}"
    );
    assert!(
        text.contains("E006  索引のタイトルを書き換える: 科学の甲子園"),
        "{text}"
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    harness.idle(0.6);
    let plot = written(&harness, "plots/01-01.txt");
    assert!(
        plot.contains("state: fixed\nmaterial: 'M0137:科学の甲子園 本戦の運営と実技課題\n;\n"),
        "{plot}"
    );
    // Enter は適用に使われ、改行は入らない。エディタは開き直して続きを書ける
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));
    let slice = harness
        .app
        .workbench()
        .map(|bench| bench.slice_buffer().to_owned())
        .unwrap_or_default();
    assert!(
        slice.ends_with("material: 'M0137:科学の甲子園 本戦の運営と実技課題"),
        "{slice}"
    );
}

// ============================================================ agent（§15.7、M10）

const AGENT_MCP: &str = r#"{
  "schema_version": 1,
  "servers": {
    "llm-main": {
      "transport": "stdio",
      "command": "my-llm-mcp",
      "env": { "API_KEY": "${NOVNOW_LLM_KEY}" },
      "capabilities": ["sampling"],
      "roles": ["*"]
    }
  }
}"#;

/// 監査ログ（`MemoryHost::now` の日）。
const AGENT_LOG: &str = "agents/log/2026-10-13.jsonl";

/// 仕様書の例 + `project.json`（`agents` の中身を渡す）+ サーバ 1 つ + 先に作ったタスク。
/// プロンプトは開くときに書かれる（`Session::prepare_agents`）。
fn agent_project(agents: &str, tasks: &[&str]) -> novnow::core::features::Project {
    let mut project = world::world();
    // 仕様書の例の全体プロットは第 1 章を確定済みにしている。節 4（話プロットの state: fixed）だけにする
    project.insert(Document::new(
        world::ALL_PATH,
        world::ALL.replacen("state: fixed", "state: draft", 1),
    ));
    project.insert(Document::new(
        "project.json",
        format!(
            "{{\n  \"schema_version\": 1,\n  \"title\": \"星屑\",\n  \"agents\": {agents}\n}}\n"
        ),
    ));
    project.insert(Document::new("agents/mcp.json", AGENT_MCP));
    for scope in tasks {
        let plan = runner::new_tasks(
            &project,
            &runner::Draft {
                role: "writer",
                scope,
                intent: "テンポを上げる",
                comment: "",
                initiator: "human",
                status: "draft",
            },
            "2026-10-13T21:00:00+09:00",
        );
        for change in &plan.writes {
            project.insert(Document::new(change.path.clone(), change.text.clone()));
        }
    }
    project
}

fn reply(text: &str) -> Result<McpReply, McpError> {
    Ok(McpReply {
        text: text.to_owned(),
        model: "model-x".to_owned(),
        truncated: false,
    })
}

/// LLM を呼べて鍵もある環境で開き（`setup` で台本を足す）、`Alt+8` で agent へ。開いた最初の画面の文字も返す。
fn agent_harness(
    project: novnow::core::features::Project,
    setup: impl FnOnce(&mut novnow::ui::app::MemoryFiles),
) -> (Harness, String) {
    let (mut harness, first) = Harness::start_agents(project, |files| {
        files.mcp = true;
        files
            .env
            .insert("NOVNOW_LLM_KEY".to_owned(), "secret".to_owned());
        setup(files);
    });
    let opened = painted_text(&first);
    harness.frame(vec![key(Key::Num8, ALT)]);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    (harness, opened)
}

fn board(harness: &Harness) -> Board {
    Board::of(harness.app.workbench().expect("open").project())
}

fn project_text(harness: &Harness, path: &str) -> String {
    harness
        .app
        .workbench()
        .expect("open")
        .project()
        .text(path)
        .to_owned()
}

fn agent_log(harness: &Harness) -> String {
    harness
        .host
        .files
        .borrow()
        .appended
        .get(AGENT_LOG)
        .cloned()
        .unwrap_or_default()
}

fn press(harness: &mut Harness, pressed: Key, modifiers: Modifiers) -> String {
    harness.frame(vec![key(pressed, modifiers)]);
    painted_text(&harness.frame(Vec::new()))
}

/// 実行待ち → 実行中 → 結果の処理まで回す。
fn settle(harness: &mut Harness) -> String {
    let mut last = String::new();
    for _ in 0..4 {
        last = painted_text(&harness.frame(Vec::new()));
    }
    last
}

#[test]
fn alt_8_opens_the_agent_workspace_and_n_creates_a_task_for_the_current_section() {
    let (mut harness, opened) = agent_harness(agent_project("{}", &[]), |_| {});
    assert!(
        opened.contains("prompts/ に既定プロンプトを作成しました"),
        "I050（U-7）: {opened}"
    );
    assert!(
        project_text(&harness, "prompts/writer.md").contains("## system"),
        "欠けた既定のプロンプトを開くときに書く"
    );
    assert_eq!(harness.app.workspace(), Some(Workspace::Agent));
    let text = painted_text(&harness.frame(Vec::new()));
    for heading in [
        "① タスク",
        "② コンテキスト",
        "③ 結果レビュー",
        "④ 委譲レベル",
    ] {
        assert!(text.contains(heading), "{heading}: {text}");
    }
    assert!(text.contains("タスクはまだありません"), "{text}");
    assert!(text.contains("llm-main"), "④ のサーバ: {text}");
    let text = press(&mut harness, Key::N, Modifiers::NONE);
    assert!(
        text.contains("新しいタスク") && text.contains("役割（↑↓ で選ぶ）"),
        "{text}"
    );
    harness.frame(vec![Event::Text("テンポを上げる".into())]);
    let text = press(&mut harness, Key::Enter, Modifiers::NONE);
    let board = board(&harness);
    assert_eq!(board.tasks.len(), 1);
    let task = &board.tasks[0];
    assert_eq!(
        (
            task.id.as_str(),
            task.role.as_str(),
            task.scope.as_str(),
            task.intent.as_str(),
            task.status.as_str(),
            task.initiator.as_str()
        ),
        (
            "T0001",
            "writer",
            "@01-04-02-01",
            "テンポを上げる",
            "draft",
            "human"
        ),
        "対象は今の節"
    );
    assert!(text.contains("タスクを作りました: T0001"), "{text}");
    assert!(text.contains("T0001  writer  01-04-02-01  draft"), "{text}");
    assert_eq!(
        harness
            .app
            .spaces()
            .and_then(|spaces| spaces.agent.task.clone())
            .as_deref(),
        Some("T0001"),
        "作ったタスクを選ぶ"
    );
}

#[test]
fn a_prompt_path_taken_on_disk_is_not_written_and_its_role_cannot_run() {
    // 外を指すシンボリックリンクの代わり（B-155）: 書かず、その役割は E029 で走らない
    let (mut harness, opened) = agent_harness(agent_project("{}", &["@01-04-02-02"]), |files| {
        files.occupied.push("prompts/writer.md".to_owned());
    });
    assert!(
        opened.contains("prompts/ に既定プロンプトを作成しました"),
        "{opened}"
    );
    let project = harness.app.workbench().unwrap().project();
    assert!(project.document("prompts/writer.md").is_none());
    assert!(project.document("prompts/plotter.md").is_some());
    let text = press(&mut harness, Key::R, Modifiers::NONE);
    assert!(
        text.contains("プロンプトに必須のセクションがありません"),
        "{text}"
    );
    assert!(harness.host.files.borrow().jobs.is_empty());
}

#[test]
fn r_runs_the_task_through_mcp_and_a_accepts_the_proposal_with_an_audit_log() {
    let (mut harness, _) = agent_harness(agent_project("{}", &["@01-04-02-02"]), |files| {
        files.replies.push_back(reply("書き直した本文。"));
    });
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    let text = settle(&mut harness);
    {
        let files = harness.host.files.borrow();
        assert_eq!(files.jobs.len(), 1);
        let job = &files.jobs[0];
        assert_eq!(job.server.name, "llm-main");
        assert_eq!(job.server.command, "my-llm-mcp");
        assert_eq!(
            job.server.env,
            vec![("API_KEY".to_owned(), "secret".to_owned())],
            "環境変数は送る直前に解決する"
        );
    }
    let board = board(&harness);
    assert_eq!(board.tasks[0].status, "review");
    let proposal = board.proposal_of("T0001").expect("a proposal");
    assert_eq!(
        (proposal.id.as_str(), proposal.status.as_str()),
        ("P0001", "open")
    );
    assert!(text.contains("書き直した本文。"), "③ に差分が出る: {text}");
    assert!(
        project_text(&harness, world::CONTENTS_PATH).contains("「旧校舎の地下遺跡"),
        "propose では本文は変わらない（A2）"
    );
    // ① の Enter で ③ へ、A で採用
    press(&mut harness, Key::Enter, Modifiers::NONE);
    let text = press(&mut harness, Key::A, Modifiers::NONE);
    assert!(text.contains("採用しました: P0001"), "{text}");
    let contents = project_text(&harness, world::CONTENTS_PATH);
    assert!(contents.contains("書き直した本文。"), "{contents}");
    assert!(!contents.contains("「旧校舎の地下遺跡"), "{contents}");
    let board = Board::of(harness.app.workbench().unwrap().project());
    assert_eq!(board.proposal("P0001").unwrap().status, "accepted");
    assert_eq!(board.tasks[0].status, "accepted");
    let log = agent_log(&harness);
    assert!(log.contains("\"event\":\"run\""), "{log}");
    assert!(log.contains("\"event\":\"accept\""), "{log}");
    assert!(
        !log.contains("書き直した本文") && !log.contains("secret"),
        "ログに本文と秘密を書かない: {log}"
    );
}

#[test]
fn p_accepts_only_the_chosen_lines_of_a_proposal() {
    let after = "「旧校舎の地下遺跡に眠っているっていう'『星屑の時計』の設計図だ」@アルス\n\n書き足した一行。";
    let (mut harness, _) = agent_harness(agent_project("{}", &["@01-04-02-02"]), |files| {
        files.replies.push_back(reply(after));
    });
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    settle(&mut harness);
    press(&mut harness, Key::Enter, Modifiers::NONE);
    // 変更の行は「消える 1 行」と「足す 1 行」。足す行だけを選ぶ
    press(&mut harness, Key::P, Modifiers::NONE);
    assert!(harness.app.spaces().unwrap().agent.partial());
    press(&mut harness, Key::ArrowDown, Modifiers::NONE);
    press(&mut harness, Key::Space, Modifiers::NONE);
    press(&mut harness, Key::Enter, Modifiers::NONE);
    let contents = project_text(&harness, world::CONTENTS_PATH);
    assert!(
        contents.contains("図面の精巧な書き込みに目を奪われた。\n書き足した一行。"),
        "消える行は残り、足す行だけが入る: {contents}"
    );
    let proposal = Board::of(harness.app.workbench().unwrap().project())
        .proposal("P0001")
        .cloned()
        .unwrap();
    assert_eq!(proposal.status, "accepted");
    assert!(proposal.partial, "一部採用の印");
}

#[test]
fn x_rejects_a_proposal_only_with_a_comment_for_the_next_history() {
    let (mut harness, _) = agent_harness(agent_project("{}", &["@01-04-02-02"]), |files| {
        files.replies.push_back(reply("案。"));
    });
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    settle(&mut harness);
    press(&mut harness, Key::Enter, Modifiers::NONE);
    let text = press(&mut harness, Key::X, Modifiers::NONE);
    assert!(text.contains("却下の理由（必須"), "{text}");
    let text = press(&mut harness, Key::Enter, Modifiers::NONE);
    assert!(
        text.contains("コメントが要ります"),
        "空では却下しない: {text}"
    );
    assert_eq!(board(&harness).proposal("P0001").unwrap().status, "open");
    harness.frame(vec![Event::Text("語尾が違う".into())]);
    let text = press(&mut harness, Key::Enter, Modifiers::NONE);
    assert!(text.contains("却下しました: P0001"), "{text}");
    let board = board(&harness);
    let proposal = board.proposal("P0001").unwrap();
    assert_eq!(
        (proposal.status.as_str(), proposal.comment.as_str()),
        ("rejected", "語尾が違う")
    );
    assert_eq!(board.tasks[0].status, "rejected");
    assert!(agent_log(&harness).contains("\"event\":\"reject\""));
}

#[test]
fn space_in_the_context_column_excludes_a_block_but_never_project_or_constraints() {
    let (mut harness, _) = agent_harness(agent_project("{}", &["@01-04-02-02"]), |_| {});
    let text = press(&mut harness, Key::ArrowRight, Modifiers::NONE);
    assert!(
        text.contains("1 project") && text.contains("必須"),
        "{text}"
    );
    // 1 project は外せない
    let text = press(&mut harness, Key::Space, Modifiers::NONE);
    assert!(text.contains("このブロックは外せません"), "{text}");
    assert_eq!(board(&harness).tasks[0].exclude, "");
    // 2 outline は外せて、もう一度で戻る
    press(&mut harness, Key::ArrowDown, Modifiers::NONE);
    let text = press(&mut harness, Key::Space, Modifiers::NONE);
    assert_eq!(board(&harness).tasks[0].exclude, "outline");
    assert!(
        text.lines()
            .any(|line| line.starts_with("2 outline  0 / ") && line.ends_with("外した")),
        "{text}"
    );
    press(&mut harness, Key::Space, Modifiers::NONE);
    assert_eq!(board(&harness).tasks[0].exclude, "");
}

#[test]
fn one_to_four_set_the_delegation_level_and_ctrl_shift_x_stops_all() {
    let (mut harness, _) = agent_harness(agent_project("{}", &[]), |_| {});
    for _ in 0..3 {
        press(&mut harness, Key::ArrowRight, Modifiers::NONE);
    }
    let text = press(&mut harness, Key::Num3, Modifiers::NONE);
    assert!(
        text.contains("writer の委譲レベルを apply にしました"),
        "{text}"
    );
    let level = |harness: &Harness, role: &str| {
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .settings()
            .agents
            .roles
            .get(role)
            .cloned()
    };
    assert_eq!(level(&harness, "writer").as_deref(), Some("apply"));
    assert!(
        project_text(&harness, "project.json").contains("\"writer\": \"apply\""),
        "project.json に書く"
    );
    press(&mut harness, Key::ArrowDown, Modifiers::NONE);
    press(&mut harness, Key::Num4, Modifiers::NONE);
    assert_eq!(level(&harness, "plotter").as_deref(), Some("autonomous"));
    let log = agent_log(&harness);
    assert!(
        log.contains("\"event\":\"delegation_changed\"")
            && log.contains("writer: propose -> apply"),
        "{log}"
    );
    // 総停止: 書き込めるレベルを propose に戻す
    let text = press(&mut harness, Key::X, CTRL_SHIFT);
    assert!(text.contains("総停止しました"), "{text}");
    assert_eq!(level(&harness, "writer").as_deref(), Some("propose"));
    assert_eq!(level(&harness, "plotter").as_deref(), Some("propose"));
    assert!(agent_log(&harness).contains("plotter: autonomous -> propose"));
}

#[test]
fn shift_r_confirms_runs_two_at_a_time_and_the_status_bar_shows_them() {
    let (mut harness, _) = agent_harness(
        agent_project("{}", &["@01-04-02-01", "@01-04-02-02", "@01-04-02-03"]),
        |files| files.hold = true,
    );
    // 新しい順に T0003 / T0002 / T0001。全部に印を付けて Shift+R
    for _ in 0..2 {
        press(&mut harness, Key::Space, Modifiers::NONE);
        press(&mut harness, Key::ArrowDown, Modifiers::NONE);
    }
    press(&mut harness, Key::Space, Modifiers::NONE);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    let text = press(&mut harness, Key::R, shift);
    assert!(text.contains("3 件のタスクを実行します"), "{text}");
    assert!(text.contains("使用サーバ: llm-main"), "{text}");
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    let text = settle(&mut harness);
    let runtime = harness.app.agents().unwrap();
    assert_eq!(runtime.running.len(), 2, "並列度の既定は 2（§15.7.5）");
    assert_eq!(runtime.queue.len(), 1);
    assert!(
        text.contains("\u{27f3} 2件"),
        "ステータスバー（§15.7.4）: {text}"
    );
    assert!(text.contains("実行中 2 / 待機 1 / 完了 0"), "{text}");
    // 待っている T0001 を中止
    let text = press(&mut harness, Key::X, Modifiers::NONE);
    assert_eq!(board(&harness).task("T0001").unwrap().status, "canceled");
    assert!(
        text.contains("T0001  writer  01-04-02-01  canceled"),
        "{text}"
    );
    // 呼び出しが返ると、提案の ID はぶつからない
    harness.host.files.borrow_mut().hold = false;
    harness
        .host
        .files
        .borrow_mut()
        .replies
        .extend([reply("案その一。"), reply("案その二。")]);
    settle(&mut harness);
    let board = board(&harness);
    let ids: Vec<&str> = board
        .proposals
        .iter()
        .map(|found| found.id.as_str())
        .collect();
    assert_eq!(ids, ["P0001", "P0002"]);
    assert!(harness.app.agents().unwrap().running.is_empty());
}

#[test]
fn the_status_bar_label_names_a_single_running_task_and_x_cancels_it() {
    let (mut harness, _) = agent_harness(agent_project("{}", &["@01-04-02-02"]), |files| {
        files.hold = true;
    });
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    let text = settle(&mut harness);
    assert!(text.contains("\u{27f3} T0001 writer"), "{text}");
    assert_eq!(board(&harness).tasks[0].status, "running");
    // 書いている間（write）も出る。クリックの代わりに Alt+8 で戻る
    harness.frame(vec![key(Key::Num1, ALT)]);
    let text = settle(&mut harness);
    assert!(text.contains("\u{27f3} T0001 writer"), "{text}");
    harness.frame(vec![key(Key::Num8, ALT)]);
    settle(&mut harness);
    press(&mut harness, Key::X, Modifiers::NONE);
    assert_eq!(harness.host.files.borrow().canceled, vec![1]);
    assert_eq!(board(&harness).tasks[0].status, "canceled");
    let text = settle(&mut harness);
    assert!(!text.contains("\u{27f3} T0001"), "{text}");
}

#[test]
fn autonomous_levels_show_in_the_status_bar() {
    let (mut harness, _) = agent_harness(
        agent_project(
            r#"{ "roles": { "writer": "autonomous" } }"#,
            &["@01-04-02-02"],
        ),
        |files| files.hold = true,
    );
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    let text = settle(&mut harness);
    assert!(text.contains("\u{27f3} T0001 writer (自律)"), "{text}");
}

#[test]
fn with_agents_disabled_nothing_is_written_and_the_other_workspaces_still_work() {
    let (mut harness, opened) =
        agent_harness(agent_project(r#"{ "enabled": false }"#, &[]), |_| {});
    assert!(!opened.contains("prompts/"), "{opened}");
    assert!(
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .document("prompts/writer.md")
            .is_none(),
        "無効ならプロンプトも書かない"
    );
    let text = painted_text(&harness.frame(Vec::new()));
    assert!(text.contains("エージェントは無効です"), "{text}");
    assert!(!text.contains("① タスク"), "「無効です」とだけ出す: {text}");
    // A6: ほかのワークスペースはそのまま
    for (number, workspace) in [
        (Key::Num1, Workspace::Write),
        (Key::Num2, Workspace::PlotEpisode),
        (Key::Num9, Workspace::Review),
    ] {
        harness.frame(vec![key(number, ALT)]);
        harness.frame(Vec::new());
        assert_eq!(harness.app.workspace(), Some(workspace));
    }
}

#[test]
fn without_an_llm_r_explains_why_and_d_still_saves_a_dry_run() {
    let (mut harness, _) = agent_harness(agent_project("{}", &["@01-04-02-02"]), |files| {
        files.mcp = false;
    });
    let text = painted_text(&harness.frame(Vec::new()));
    assert!(text.contains("この環境では LLM を呼び出せません"), "{text}");
    let text = press(&mut harness, Key::R, Modifiers::NONE);
    assert!(text.contains("この環境では LLM を呼び出せません"), "{text}");
    assert_eq!(board(&harness).tasks[0].status, "draft");
    assert!(harness.host.files.borrow().jobs.is_empty());
    let text = press(&mut harness, Key::D, Modifiers::NONE);
    assert!(
        text.contains("ドライランを保存しました: agents/results/T0001.dryrun.md"),
        "{text}"
    );
    let packet = project_text(&harness, "agents/results/T0001.dryrun.md");
    assert!(
        packet.contains("## 1 project") && packet.contains("## 9 history"),
        "{packet}"
    );
    assert!(
        harness.host.files.borrow().appended.is_empty(),
        "ドライランはログに書かない"
    );
}

#[test]
fn a_missing_key_blocks_the_run_with_e030() {
    let (mut harness, _) = agent_harness(agent_project("{}", &["@01-04-02-02"]), |files| {
        files.env.clear();
    });
    let text = painted_text(&harness.frame(Vec::new()));
    assert!(
        text.contains("NOVNOW_LLM_KEY"),
        "④ に解決できない変数: {text}"
    );
    let text = press(&mut harness, Key::R, Modifiers::NONE);
    assert!(
        text.contains("環境変数 NOVNOW_LLM_KEY が設定されていません"),
        "{text}"
    );
    assert!(harness.host.files.borrow().jobs.is_empty());
    assert_eq!(board(&harness).tasks[0].status, "draft");
}

#[test]
fn a_failed_call_returns_the_task_to_ready_without_retrying() {
    let (mut harness, _) = agent_harness(agent_project("{}", &["@01-04-02-02"]), |files| {
        files.replies.push_back(Err(McpError::Timeout));
    });
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    let text = settle(&mut harness);
    assert!(
        text.contains("T0001 がタイムアウトしました（ready に戻しました）"),
        "{text}"
    );
    assert_eq!(board(&harness).tasks[0].status, "ready");
    assert_eq!(
        harness.host.files.borrow().jobs.len(),
        1,
        "自動再試行しない"
    );
    assert!(agent_log(&harness).contains("\"error\""));
}

#[test]
fn apply_auto_applies_and_x_reverts_with_a_comment() {
    let (mut harness, _) = agent_harness(
        agent_project(r#"{ "roles": { "writer": "apply" } }"#, &["@01-04-02-02"]),
        |files| files.replies.push_back(reply("書き換えた本文。")),
    );
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    let text = settle(&mut harness);
    assert!(
        text.contains("自動適用しました: T0001 → 01-04-02-02"),
        "{text}"
    );
    assert!(project_text(&harness, world::CONTENTS_PATH).contains("書き換えた本文。"));
    assert_eq!(board(&harness).proposal("P0001").unwrap().status, "applied");
    assert!(agent_log(&harness).contains("\"event\":\"auto_applied\""));
    press(&mut harness, Key::Enter, Modifiers::NONE);
    let text = press(&mut harness, Key::X, Modifiers::NONE);
    assert!(text.contains("差し戻しの理由（必須"), "{text}");
    harness.frame(vec![Event::Text("語尾が違う".into())]);
    let text = press(&mut harness, Key::Enter, Modifiers::NONE);
    assert!(text.contains("差し戻しました: P0001"), "{text}");
    let contents = project_text(&harness, world::CONTENTS_PATH);
    assert!(
        contents.contains("「旧校舎の地下遺跡") && !contents.contains("書き換えた本文。"),
        "本文を before に戻す（§15.7.3 MUST）: {contents}"
    );
    assert_eq!(
        board(&harness).proposal("P0001").unwrap().status,
        "rejected"
    );
}

#[test]
fn auto_applying_a_fixed_section_says_so_in_the_status_bar() {
    // A7（§19.8）: 確定済みの節を自動で書き換えたらステータスバーに 1 行出し、監査ログに fixed_overwrite
    let (mut harness, _) = agent_harness(
        agent_project(r#"{ "roles": { "writer": "apply" } }"#, &["@01-04-02-04"]),
        |files| files.replies.push_back(reply("確定済みを書き換えた。")),
    );
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    let text = settle(&mut harness);
    assert!(
        text.contains("fixed を書き換えました: T0001 → 01-04-02-04"),
        "{text}"
    );
    assert!(project_text(&harness, world::CONTENTS_PATH).contains("確定済みを書き換えた。"));
    assert!(agent_log(&harness).contains("\"event\":\"fixed_overwrite\""));
}

#[test]
fn the_palette_sets_levels_and_resets_or_compares_prompts() {
    let mut project = agent_project("{}", &[]);
    project.insert(Document::new("prompts/writer.md", "# writer\n\n## system\n書き換えた。\n\n## task\n節 {{scene_id}} の本文を書いてください。\n"));
    let (mut harness, _) = agent_harness(project, |_| {});
    assert!(
        project_text(&harness, "prompts/writer.md").contains("書き換えた。"),
        "既存のプロンプトは開いても上書きしない（§19.8）"
    );
    let palette = |harness: &mut Harness, query: &str| {
        harness.frame(vec![key(Key::P, CTRL)]);
        harness.frame(Vec::new());
        harness.frame(vec![Event::Text(query.into())]);
        let listed = painted_text(&harness.frame(Vec::new()));
        harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
        // パレットから開いた重ね物は次のフレームで大きさを測り、その次から見える
        harness.frame(Vec::new());
        (listed, painted_text(&harness.frame(Vec::new())))
    };
    let (listed, text) = palette(&mut harness, ">agent delegation writer apply");
    assert!(
        listed.contains(">agent delegation writer apply"),
        "{listed}"
    );
    assert!(
        text.contains("writer の委譲レベルを apply にしました"),
        "{text}"
    );
    let (_, text) = palette(&mut harness, ">agent stop-all");
    assert!(text.contains("総停止しました"), "{text}");
    assert!(project_text(&harness, "project.json").contains("\"writer\": \"propose\""));
    // diff は並べるだけ
    let (listed, text) = palette(&mut harness, ">prompt diff writer");
    assert!(listed.contains(">prompt diff writer"), "{listed}");
    assert!(text.contains("既定との比較: writer"), "{text}");
    assert!(
        text.contains("書き換えた。") && text.contains("既定"),
        "{text}"
    );
    press(&mut harness, Key::Escape, Modifiers::NONE);
    // reset は確認してから既定に戻す（退避しない）
    let (_, text) = palette(&mut harness, ">prompt reset writer");
    assert!(text.contains("プロンプトを既定に戻す: writer"), "{text}");
    assert!(text.contains("元に戻せません"), "{text}");
    let text = press(&mut harness, Key::Enter, Modifiers::NONE);
    assert!(
        text.contains("1 個のプロンプトを既定に戻しました"),
        "{text}"
    );
    assert_eq!(
        project_text(&harness, "prompts/writer.md"),
        novnow::core::template::default_prompt("prompts/writer.md").unwrap()
    );
    assert!(
        !harness
            .app
            .workbench()
            .unwrap()
            .project()
            .documents()
            .any(|document| document.path.contains(".bak")),
        "`.bak-*` を作らない"
    );
}
