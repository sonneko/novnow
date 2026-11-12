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

use harness::{key, painted_text, painted_texts, Harness};

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
    assert!(Workspace::Reader.available(), "追加仕様 #27");
    assert!(Workspace::Agent.available(), "M10 で入った");
    assert_eq!(Workspace::available_all().count(), 9);
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
    assert!(switcher.switch(Workspace::Reader));
    assert!(switcher.switch(Workspace::Write));
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
    let path = "plots/01-01.cnov";
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
    let path = "plots/01-01.cnov";
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
    let path = "plots/01-01.cnov";
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
    let path = "plots/01-02.cnov";
    bench.open_slice(path, 0..0, "test:new");
    bench.close_slice();
    assert!(bench.project().document(path).is_none(), "空なら作らない");
    bench.open_slice(path, 0..0, "test:new");
    bench.slice_buffer_mut().push_str("@01-02-01 旧校舎");
    bench.slice_edited(1.0);
    bench.open_slice("plots/01-01.cnov", 0..1, "test:other");
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
    let path = "contents/03-01.cnov";
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
    // 上段: 章プロットの該当話ブロック（親が常に見える、§10.2）
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
        .get("plots/01-01.cnov")
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
fn the_reader_opens_a_clean_preview() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num7, ALT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Reader));
    assert!(painted_text(&output).contains("読者モード"));
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
    // contents/01-02.cnov（話プロットが無い話）へ
    harness.frame(vec![key(Key::ArrowRight, CTRL_ALT)]);
    harness.frame(vec![key(Key::Num2, ALT)]);
    let output = harness.frame(Vec::new());
    assert!(painted_text(&output).contains("話プロット plots/01-02.cnovはまだありません"));
    harness.frame(vec![key(Key::N, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text("扉を開ける".into())]);
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get("plots/01-02.cnov")
        .cloned()
        .unwrap_or_default();
    assert_eq!(written, "@01-02-01\n扉を開ける\n");
    assert_eq!(selected(&harness), Some(id(1, 2, 1, 1)));
}

#[test]
fn going_down_to_a_missing_prose_file_does_not_create_it_until_typing() {
    let mut project = novel::novel();
    project.insert(Document::new(
        "plots/03-01.cnov",
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
        .contains_key("contents/03-01.cnov"));
    harness.frame(vec![Event::Text("新しい章。".into())]);
    harness.idle(0.6);
    let written = harness
        .host
        .files
        .borrow()
        .written
        .get("contents/03-01.cnov")
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
    // 親（全体プロットの章のブロック）が常に見える（§10.3.1）
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
        .get("plots/01.cnov")
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
    assert!(painted_text(&output).contains("plots/02.cnovにブロックはまだありません"));
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
        .get("plots/02.cnov")
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
    assert!(painted_text(&output).contains("I0009を ideas.md に積みました"));
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
        .text("contents/01-02.cnov")
        .replace("扉の向こうは暗かった。", "扉の向こうは'未登録の闇 だった。");
    project.insert(Document::new("contents/01-02.cnov", text));
    let mut harness = Harness::start_with(project, true).0;
    harness.frame(vec![key(Key::E, CTRL_SHIFT)]);
    let output = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Review));
    let text = painted_text(&output);
    assert!(text.contains("診断"), "{text}");
    assert!(text.contains("進捗"), "{text}");
    assert!(text.contains("contents/01-02.cnov:"), "{text}");
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
                .position(|row| row.code == "E001" && row.path == "contents/01-02.cnov")
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
fn the_review_screen_saves_a_diagnostic_level_to_project_toml() {
    // §10.9.6: 点検画面の「出さない」が [features.diagnostics] へ保存され、
    // そのコードは診断から消える（表示フィルタではなく有効性そのもの）
    let mut project = novel::novel();
    let text = project
        .text("contents/01-02.cnov")
        .replace("扉の向こうは暗かった。", "扉の向こうは'未登録の闇 だった。");
    project.insert(Document::new("contents/01-02.cnov", text));
    let mut harness = Harness::start_with(project, true).0;
    harness.frame(vec![key(Key::E, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    let index = harness
        .app
        .spaces()
        .and_then(|spaces| spaces.review.cached())
        .and_then(|rows| rows.iter().position(|row| row.code == "E001"))
        .expect("E001 の行");
    for _ in 0..index {
        harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    }
    let rect = harness.rect_of("出さない").expect("有効設定の選択肢");
    harness.click_at(rect.center());
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let rows = harness
        .app
        .spaces()
        .and_then(|spaces| spaces.review.cached())
        .expect("数え直した診断");
    assert!(
        rows.iter().all(|row| row.code != "E001"),
        "E001 = off が効いている: {rows:?}"
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
        painted_text(&output).contains("M0138を採用しました"),
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
    assert!(painted_text(&output).contains("M0140を却下しました"));
}

#[test]
fn material_tab_traverses_to_the_review_form_after_ctrl_three() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num5, ALT)]);
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    let output = harness.frame(vec![key(Key::Num3, CTRL)]);
    // The shortcut guide in the central pane must not force the right form
    // offscreen: the title label and source label remain within the viewport.
    for label in ["タイトル", "出典"] {
        let rect = painted_texts(&output)
            .into_iter()
            .find(|(text, _)| text == label)
            .map(|(_, rect)| rect)
            .expect("material review form label");
        assert!(rect.right() <= 1280.0, "{label} is clipped: {rect:?}");
    }
    let title = egui::Id::new(("material-title", "M0138"));
    let mut reached = false;
    for _ in 0..40 {
        harness.frame(vec![key(Key::Tab, Modifiers::NONE)]);
        if harness.ctx.memory(|memory| memory.has_focus(title)) {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab must traverse to the material title field");
}

#[test]
fn the_review_form_writes_the_index_and_keeps_the_h1_in_step() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num5, ALT)]);
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    assert_eq!(material_selected(&harness).as_deref(), Some("M0138"));
    // Ctrl+3 reveals the shared reference pane, without taking a
    // material-specific form shortcut. The form uses ordinary focus traversal.
    harness.frame(vec![key(Key::Num3, CTRL)]);
    let title = egui::Id::new(("material-title", "M0138"));
    for _ in 0..40 {
        if harness.ctx.memory(|memory| memory.has_focus(title)) {
            break;
        }
        harness.frame(vec![key(Key::Tab, Modifiers::NONE)]);
    }
    assert!(harness.ctx.memory(|memory| memory.has_focus(title)));
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
    let prose = written(&harness, "contents/01-01.cnov");
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
    let prose = written(&harness, "contents/01-01.cnov");
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
    let plot = written(&harness, "plots/01-01.cnov");
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
    // Ctrl+3 では最初の行にフォーカスするので、候補の行まで ↓ で移る（§9.5.5）
    let candidate = novnow::ui::widgets::reference_pane::candidate_id("M0139");
    for _ in 0..30 {
        if harness.ctx.memory(|memory| memory.focused()) == Some(candidate) {
            break;
        }
        harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    }
    assert_eq!(
        harness.ctx.memory(|memory| memory.focused()),
        Some(candidate),
        "the candidate row must be reachable with ArrowDown"
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.idle(0.6);
    let plot = written(&harness, "plots/01-01.cnov");
    assert!(
        plot.starts_with(
            "@01-01-01 放課後\n二人が図書室で話す。\nstate: fixed\nmaterial: 'M0139\n;\n"
        ),
        "{plot}"
    );
}

/// 本文の先頭の節に辞書にない参照（`'未登録の紋様`）が 1 つだけあるプロジェクト。
fn unresolved_harness() -> Harness {
    let mut project = novel::novel();
    project.insert(Document::new(
        "contents/01-01.cnov",
        "@01-01-01 放課後\n'未登録の紋様 が描かれていた。\n",
    ));
    Harness::start_with(project, true).0
}

#[test]
fn a_diagnostic_in_the_reference_pane_registers_the_first_morpheme() {
    // §6.2.6 / §9.5.4（F-23）: 参照ペインの診断から、形態素解析で切った 1 形態素を
    // その場のボタンで `words.md` へ登録して自動回復できる
    let mut harness = unresolved_harness();
    harness.idle(0.5);
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    // `'未登録の紋様` の赤波線は先頭 1 形態素（`未登録`）まで
    assert!(text.contains("辞書にない参照です: 未登録"), "{text}");
    let at = {
        let found = painted_texts(&output)
            .into_iter()
            .find(|(painted, _)| painted == "words.md に追加")
            .expect("words.md に追加 ボタン");
        found.1.center()
    };
    harness.click_at(at);
    harness.idle(0.6);
    let words = written(&harness, "words.md");
    assert!(words.ends_with("# 未登録\ndefinition: \n"), "{words}");
    // `characters.md` へも同じ 1 形態素を登録できる
    let mut harness = unresolved_harness();
    harness.idle(0.5);
    let output = harness.frame(Vec::new());
    let at = {
        let found = painted_texts(&output)
            .into_iter()
            .find(|(painted, _)| painted == "characters.md に追加")
            .expect("characters.md に追加 ボタン");
        found.1.center()
    };
    harness.click_at(at);
    harness.idle(0.6);
    let characters = written(&harness, "characters.md");
    assert!(characters.ends_with("# 未登録\n"), "{characters}");
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
    let plot = written(&harness, "plots/01-01.cnov");
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
    let plot = written(&harness, "plots/01-01.cnov");
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

/// 仕様書の例 + `project.toml`（`agents` の中身を渡す）+ サーバ 1 つ + 先に作ったタスク。
/// プロンプトは開くときに書かれる（`Session::prepare_agents`）。
fn agent_project(agents: &str, tasks: &[&str]) -> novnow::core::features::Project {
    let mut project = world::world();
    // 仕様書の例の全体プロットは第 1 章を確定済みにしている。節 4（話プロットの state: fixed）だけにする
    project.insert(Document::new(
        world::ALL_PATH,
        world::ALL.replacen("state: fixed", "state: draft", 1),
    ));
    project.insert(Document::new(
        "project.toml",
        format!("schema_version = 2\ntitle = \"星屑\"\n\n[agents]\n{agents}\n"),
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
                examples: "",
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
    let (mut harness, opened) = agent_harness(agent_project("", &[]), |_| {});
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
    let (mut harness, opened) = agent_harness(agent_project("", &["@01-04-02-02"]), |files| {
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
    let (mut harness, _) = agent_harness(agent_project("", &["@01-04-02-02"]), |files| {
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
    let (mut harness, _) = agent_harness(agent_project("", &["@01-04-02-02"]), |files| {
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
    let (mut harness, _) = agent_harness(agent_project("", &["@01-04-02-02"]), |files| {
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
    let (mut harness, _) = agent_harness(agent_project("", &["@01-04-02-02"]), |_| {});
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
    let (mut harness, _) = agent_harness(agent_project("", &[]), |_| {});
    for _ in 0..3 {
        press(&mut harness, Key::ArrowRight, Modifiers::NONE);
    }
    let text = press(&mut harness, Key::Num3, Modifiers::NONE);
    assert!(
        text.contains("writerの委譲レベルを applyにしました"),
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
    assert_eq!(
        novnow::core::syntax::project_toml::settings(&project_text(&harness, "project.toml"))
            .agents
            .roles
            .get("writer")
            .map(String::as_str),
        Some("apply"),
        "project.toml に書く"
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
        agent_project("", &["@01-04-02-01", "@01-04-02-02", "@01-04-02-03"]),
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
    let (mut harness, _) = agent_harness(agent_project("", &["@01-04-02-02"]), |files| {
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
        agent_project("roles = { writer = \"autonomous\" }", &["@01-04-02-02"]),
        |files| files.hold = true,
    );
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    let text = settle(&mut harness);
    assert!(text.contains("\u{27f3} T0001 writer (自律)"), "{text}");
}

#[test]
fn with_agents_disabled_nothing_is_written_and_the_other_workspaces_still_work() {
    let (mut harness, opened) = agent_harness(agent_project("enabled = false", &[]), |_| {});
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
    // A6: ほかのワークスペースはすべてそのまま（読者モードの Alt+7 は ② の欠番）
    for (number, workspace) in [
        (Key::Num1, Workspace::Write),
        (Key::Num2, Workspace::PlotEpisode),
        (Key::Num3, Workspace::PlotOutline),
        (Key::Num4, Workspace::Idea),
        (Key::Num5, Workspace::Material),
        (Key::Num6, Workspace::Cast),
        (Key::Num9, Workspace::Review),
    ] {
        harness.frame(vec![key(number, ALT)]);
        let text = painted_text(&harness.frame(Vec::new()));
        assert_eq!(harness.app.workspace(), Some(workspace));
        assert!(
            !text.contains("エージェントは無効です"),
            "{workspace:?}: {text}"
        );
    }
}

#[test]
fn without_an_llm_r_explains_why_and_d_still_saves_a_dry_run() {
    let (mut harness, _) = agent_harness(agent_project("", &["@01-04-02-02"]), |files| {
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
    let (mut harness, _) = agent_harness(agent_project("", &["@01-04-02-02"]), |files| {
        files.env.clear();
    });
    let text = painted_text(&harness.frame(Vec::new()));
    assert!(
        text.contains("NOVNOW_LLM_KEY"),
        "④ に解決できない変数: {text}"
    );
    let text = press(&mut harness, Key::R, Modifiers::NONE);
    assert!(
        text.contains("環境変数 NOVNOW_LLM_KEYが設定されていません"),
        "{text}"
    );
    assert!(harness.host.files.borrow().jobs.is_empty());
    assert_eq!(board(&harness).tasks[0].status, "draft");
}

#[test]
fn a_failed_call_returns_the_task_to_ready_without_retrying() {
    let (mut harness, _) = agent_harness(agent_project("", &["@01-04-02-02"]), |files| {
        files.replies.push_back(Err(McpError::Timeout));
    });
    harness.frame(vec![key(Key::R, Modifiers::NONE)]);
    let text = settle(&mut harness);
    assert!(
        text.contains("T0001がタイムアウトしました（ready に戻しました）"),
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
        agent_project("roles = { writer = \"apply\" }", &["@01-04-02-02"]),
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
        agent_project("roles = { writer = \"apply\" }", &["@01-04-02-04"]),
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
    let mut project = agent_project("", &[]);
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
        text.contains("writerの委譲レベルを applyにしました"),
        "{text}"
    );
    let (_, text) = palette(&mut harness, ">agent stop-all");
    assert!(text.contains("総停止しました"), "{text}");
    assert_eq!(
        novnow::core::syntax::project_toml::settings(&project_text(&harness, "project.toml"))
            .agents
            .roles
            .get("writer")
            .map(String::as_str),
        Some("propose")
    );
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

#[test]
fn palette_format_and_dictionary_references_and_rename_and_outline_folding_work_in_ui() {
    let mut harness = novel_harness();

    // 1. `'` パレットの Shift+Enter で使用箇所一覧（`references::at`）が開き、Enter で該当箇所へ移動
    harness.frame(vec![key(Key::P, CTRL)]);
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text("'リリア".into())]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::Enter, Modifiers::SHIFT)]);
    harness.frame(Vec::new());
    let text = painted_text(&harness.frame(Vec::new()));
    assert!(
        text.contains("「リリア」の使用箇所"),
        "references overlay: {text}"
    );
    assert!(text.contains("plots/01-01.cnov"), "references list: {text}");
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    harness.frame(Vec::new());

    // 2. `>format`（`formatting::document`）で今の文書を正規化
    harness.frame(vec![key(Key::End, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("   ".into())]);
    harness.frame(vec![key(Key::P, CTRL)]);
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text(">format".into())]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    let text = painted_text(&harness.frame(Vec::new()));
    assert!(
        text.contains("contents/01-01.cnovを正規化しました"),
        "format notice: {text}"
    );

    // 3. `F2`（`rename::prepare` + `rename::apply`）でキャラ名を一括改名
    harness.frame(vec![key(Key::Num6, ALT)]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::F2, Modifiers::NONE)]);
    harness.frame(Vec::new());
    let text = painted_text(&harness.frame(Vec::new()));
    assert!(text.contains("アルス"), "rename modal placeholder: {text}");
    harness.frame(vec![key(Key::End, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("太郎".into())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    let text = painted_text(&harness.frame(Vec::new()));
    assert!(
        text.contains("「アルス太郎」へ変更しました"),
        "rename notice: {text}"
    );
    assert!(
        project_text(&harness, "characters.md").contains("# アルス太郎"),
        "characters.md updated"
    );
    assert!(
        project_text(&harness, "contents/01-01.cnov").contains("'{アルス太郎}"),
        "contents/01-01.cnov references updated"
    );

    // 4. `plot-outline`（Alt+3）で `Ctrl+Shift+]` / `Ctrl+Shift+[` による章→話の展開・折り畳み（`folding::ranges`）
    harness.frame(vec![key(Key::Num3, ALT)]);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .app
            .spaces()
            .map(|spaces| spaces.outline.is_expanded(1)),
        Some(false)
    );
    harness.frame(vec![key(Key::CloseBracket, CTRL_SHIFT)]);
    let text = painted_text(&harness.frame(Vec::new()));
    assert_eq!(
        harness
            .app
            .spaces()
            .map(|spaces| spaces.outline.is_expanded(1)),
        Some(true)
    );
    assert!(
        text.contains("▾ 01") && text.contains("01-01") && text.contains("01-02"),
        "expanded episodes in outline tree: {text}"
    );
    harness.frame(vec![key(Key::OpenBracket, CTRL_SHIFT)]);
    let text = painted_text(&harness.frame(Vec::new()));
    assert_eq!(
        harness
            .app
            .spaces()
            .map(|spaces| spaces.outline.is_expanded(1)),
        Some(false)
    );
    assert!(
        text.contains("▸ 01"),
        "folded chapter in outline tree: {text}"
    );
}

#[test]
fn bare_arrow_selection_does_not_commit_or_replace_an_open_slice() {
    use novnow::ui::workspaces::{idea, material, plot_episode, plot_outline};

    let mut bench = Workbench::new(material_project(), None, 400);
    let mut materials = material::State::default();
    assert!(materials.select(&bench, "M0138"));
    materials.sync(&mut bench);
    let slice = bench.slice().cloned().expect("open material");
    bench.slice_buffer_mut().push_str(" 未確定");
    assert!(materials.preview_row(&bench, true));
    materials.sync(&mut bench);
    assert_eq!(bench.slice(), Some(&slice));
    assert!(bench.slice_buffer().ends_with(" 未確定"));
    assert!(!bench.project().text(&slice.path).contains(" 未確定"));
    materials.nav(&mut bench, Nav::PrevSection);
    assert!(bench.project().text(&slice.path).contains(" 未確定"));

    let mut bench = Workbench::new(novel::novel_with("ideas.md", IDEAS), None, 400);
    let mut ideas = idea::State::default();
    ideas.sync(&mut bench);
    let slice = bench.slice().cloned().expect("open idea");
    assert!(ideas.preview_row(&bench, true));
    ideas.sync(&mut bench);
    assert_eq!(bench.slice(), Some(&slice));

    let mut bench = Workbench::new(world::world(), None, 400);
    let mut episode = plot_episode::State::default();
    episode.land(&bench, id(1, 4, 2, 1));
    episode.sync(&mut bench);
    let slice = bench.slice().cloned().expect("open plot episode");
    assert!(episode.preview_row(&bench, true));
    episode.sync(&mut bench);
    assert_eq!(bench.slice(), Some(&slice));

    let mut outline = plot_outline::State::default();
    outline.land_all(&bench, 1);
    outline.sync(&mut bench);
    let slice = bench.slice().cloned().expect("open plot outline");
    if outline.preview_row(&bench, true) {
        outline.sync(&mut bench);
        assert_eq!(bench.slice(), Some(&slice));
    }
}

#[test]
fn bare_arrow_browses_material_list_without_leaving_its_editor() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num5, ALT)]);
    harness.frame(Vec::new());
    let before = material_selected(&harness);
    let slice = harness.app.workbench().and_then(Workbench::slice).cloned();
    if let Some(focused) = harness.ctx.memory(|memory| memory.focused()) {
        harness
            .ctx
            .memory_mut(|memory| memory.surrender_focus(focused));
    }
    harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    assert_ne!(material_selected(&harness), before);
    assert_eq!(
        harness.app.workbench().and_then(Workbench::slice),
        slice.as_ref()
    );
}

#[test]
fn bare_arrow_browses_cast_and_plot_lists_without_committing_slices() {
    for (workspace, selected) in [
        (Key::Num6, Workspace::Cast),
        (Key::Num2, Workspace::PlotEpisode),
        (Key::Num3, Workspace::PlotOutline),
    ] {
        let mut harness = Harness::start_with(world::world(), true).0;
        harness.frame(vec![key(workspace, ALT)]);
        harness.frame(Vec::new());
        let previous = harness.app.workbench().and_then(Workbench::slice).cloned();
        let before = match selected {
            Workspace::Cast => {
                format!("{:?}", harness.app.spaces().and_then(|s| s.cast.selected()))
            }
            Workspace::PlotEpisode => format!(
                "{:?}",
                harness.app.spaces().and_then(|s| s.episode.selected())
            ),
            Workspace::PlotOutline => format!(
                "{:?}",
                harness.app.spaces().and_then(|s| s.outline.selected())
            ),
            _ => unreachable!(),
        };
        if let Some(id) = harness.ctx.memory(|memory| memory.focused()) {
            harness.ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
        harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
        let spaces = harness.app.spaces().expect("open");
        assert_eq!(spaces.switcher.current(), selected);
        let after = match selected {
            Workspace::Cast => format!("{:?}", spaces.cast.selected()),
            Workspace::PlotEpisode => format!("{:?}", spaces.episode.selected()),
            Workspace::PlotOutline => format!("{:?}", spaces.outline.selected()),
            _ => unreachable!(),
        };
        if selected != Workspace::PlotOutline {
            assert_ne!(before, after, "bare arrow must browse {selected:?}");
        }
        assert_eq!(
            harness.app.workbench().and_then(Workbench::slice),
            previous.as_ref()
        );
    }
}

#[test]
fn empty_episode_plot_can_be_created_with_the_visible_button() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::ArrowRight, CTRL_ALT)]);
    harness.frame(vec![key(Key::Num2, ALT)]);
    let rect = harness
        .rect_of("Ctrl+Shift+N 最初の節を作る")
        .expect("visible insertion button");
    harness.click_at(rect.center());
    harness.frame(Vec::new());
    harness.frame(vec![Event::Text("扉を開ける".into())]);
    harness.idle(0.6);
    assert_eq!(
        written(&harness, "plots/01-02.cnov"),
        "@01-02-01\n扉を開ける\n"
    );
}

#[test]
fn review_and_cast_shortcut_buttons_execute_their_actions() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::Num9, ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]);
    assert_eq!(
        harness.app.spaces().unwrap().review.tab,
        novnow::ui::workspaces::review::Tab::Progress
    );
    // キー案内は折り返しレイアウトなので、安定したフレームで位置を取る
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let rect = harness
        .rect_of("Ctrl+Shift+E 数え直す")
        .expect("review rerun button");
    harness.click_at(rect.center());
    assert_eq!(
        harness.app.spaces().unwrap().review.tab,
        novnow::ui::workspaces::review::Tab::Diagnostics
    );

    harness.frame(vec![key(Key::Num6, ALT)]);
    let rect = harness
        .rect_of("Ctrl+Shift+V 未一致の台詞を走査")
        .expect("cast scan button");
    harness.click_at(rect.center());
    assert_eq!(
        harness.app.spaces().unwrap().cast.tab,
        novnow::ui::workspaces::cast::Tab::Unmatched
    );
}

#[test]
fn empty_chapter_plot_can_be_created_with_the_visible_button() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num3, ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_ALT)]);
    let rect = harness
        .rect_of("Ctrl+Shift+N ブロックを作る")
        .expect("visible outline insertion button");
    harness.click_at(rect.center());
    harness.frame(Vec::new());
    assert_eq!(outline_selected(&harness), Some(id(2, 1, 0, 0)));
    harness.idle(0.6);
    assert_eq!(written(&harness, "plots/02.cnov"), "@02-01\n;\n");
}

#[test]
fn agent_stop_all_button_downgrades_delegation() {
    let (mut harness, _) = agent_harness(
        agent_project("enabled = true\nroles = { writer = \"apply\" }", &[]),
        |_| {},
    );
    let rect = harness
        .rect_of("Ctrl+Shift+X 総停止")
        .expect("visible stop-all button");
    harness.click_at(rect.center());
    assert_eq!(
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .settings()
            .agents
            .roles
            .get("writer")
            .map(String::as_str),
        Some("propose")
    );
}

#[test]
fn thousand_idea_rows_scroll_to_the_keyboard_selection_without_rendering_every_row() {
    let mut project = world::world();
    let ideas = (1..=1000)
        .map(|number| format!("# I{number:04} Idea {number:04}\nstatus: active\n\n"))
        .collect::<String>();
    project.insert(Document::new("ideas.md", ideas));
    let mut harness = Harness::start_with(project, true).0;
    harness.frame(vec![key(Key::Num4, ALT)]);
    for _ in 0..65 {
        harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    }
    let selected = harness.app.spaces().unwrap().idea.selected();
    assert_eq!(selected, Some("I0066"));
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(
        text.contains("Idea 0066"),
        "selected row should be in the viewport: {text}"
    );
    assert!(
        !text.contains("Idea 0900"),
        "offscreen rows must not be laid out: {text}"
    );
}

#[test]
fn focused_cast_timeline_arrows_do_not_change_the_character_list_selection() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num6, ALT)]);
    let first = harness.app.spaces().unwrap().cast.selected();
    for _ in 0..3 {
        harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]);
    }
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    assert_eq!(harness.app.spaces().unwrap().cast.selected(), first);
}

#[test]
fn agent_task_examples_entered_in_the_modal_reach_the_dry_run_packet() {
    let (mut harness, _) = agent_harness(agent_project("", &[]), |_| {});
    press(&mut harness, Key::N, Modifiers::NONE);
    let rect = harness
        .rect_of("@01-02 both")
        .expect("examples field hint is visible");
    harness.click_at(rect.center());
    harness.frame(vec![Event::Text("@01-04 plot".into())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    let task = board(&harness).tasks[0].clone();
    assert_eq!(task.examples, ["@01-04 plot"]);
    press(&mut harness, Key::D, Modifiers::NONE);
    let packet = project_text(&harness, "agents/results/T0001.dryrun.md");
    assert!(packet.contains("## examples"), "{packet}");
    assert!(packet.contains("@01-04"), "{packet}");
}

#[test]
fn agent_delegation_level_buttons_apply_to_the_selected_role() {
    let (mut harness, _) = agent_harness(agent_project("", &[]), |_| {});
    let rect = harness
        .rect_of("3 apply")
        .expect("visible delegation button");
    harness.click_at(rect.center());
    assert_eq!(
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .settings()
            .agents
            .roles
            .get("writer")
            .map(String::as_str),
        Some("apply")
    );
}

#[test]
fn agent_dry_run_button_generates_the_selected_task_packet() {
    let (mut harness, _) = agent_harness(agent_project("", &["@01-04-02-01"]), |_| {});
    let rect = harness.rect_of("D ドライラン").expect("dry run button");
    harness.click_at(rect.center());
    assert!(
        project_text(&harness, "agents/results/T0001.dryrun.md").contains("## 1 project"),
        "the button must perform the same action as the D key"
    );
}

#[test]
fn material_creation_shortcut_works_while_the_review_form_is_focused() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num5, ALT)]);
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    harness.frame(vec![key(Key::Num3, CTRL)]);
    let title = egui::Id::new(("material-title", "M0138"));
    for _ in 0..40 {
        if harness.ctx.memory(|memory| memory.has_focus(title)) {
            break;
        }
        harness.frame(vec![key(Key::Tab, Modifiers::NONE)]);
    }
    assert!(harness.ctx.memory(|memory| memory.has_focus(title)));
    harness.frame(vec![key(Key::N, CTRL_SHIFT)]);
    assert!(
        harness.app.overlay_open(),
        "Ctrl+Shift+N is global in material"
    );
    harness.frame(vec![Event::Text("新しい取材".into())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.idle(0.6);
    let index = written(&harness, "materials/index.json");
    assert!(index.contains("\"title\": \"新しい取材\""), "{index}");
}

#[test]
fn material_creation_button_opens_the_same_dialog() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num5, ALT)]);
    let rect = harness
        .rect_of("Ctrl+Shift+N 新しい資料")
        .expect("the visible material new button");
    harness.click_at(rect.center());
    assert!(harness.app.overlay_open());
}

#[test]
fn material_category_chips_and_condition_hint_match_the_metadata_form() {
    let mut harness = material_harness();
    harness.frame(vec![key(Key::Num5, ALT)]);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    let text = painted_text(&output);
    assert!(text.contains("この資料のカテゴリ"), "{text}");
    for chip in [
        "取材",
        "文献",
        "Web",
        "画像",
        "体験",
        "一次資料",
        "二次資料",
        "その他",
    ] {
        assert!(text.contains(chip), "chip {chip}: {text}");
    }
    assert!(
        output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Rect(rect) if rect.stroke.width == 1.5
        )),
        "selected category has a visible outline"
    );
    // M0138 has an empty conditions list, so the examples belong to the
    // TextEdit hint rather than a permanent explanatory paragraph.
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    let empty = painted_text(&harness.frame(Vec::new()));
    assert_eq!(material_selected(&harness).as_deref(), Some("M0138"));
    assert!(empty.contains("scope: @02-03"), "condition hint: {empty}");
}

#[test]
fn review_and_agent_bare_arrows_work_with_a_focused_header_button() {
    let mut review = Harness::new();
    review.frame(vec![key(Key::Num9, ALT)]);
    let header = review.rect_of("あ").expect("language button");
    review.click_at(header.center());
    review.frame(vec![key(Key::Tab, Modifiers::NONE)]);
    assert!(review.ctx.memory(|memory| memory.focused().is_some()));
    let before = review.app.spaces().unwrap().review.selected();
    review.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    let after = review.app.spaces().unwrap().review.selected();
    assert!(
        after > before,
        "review selection must move with header focus"
    );

    let (mut agent, _) =
        agent_harness(agent_project("", &["@01-04-02-01", "@01-04-02-02"]), |_| {});
    let header = agent.rect_of("あ").expect("language button");
    agent.click_at(header.center());
    agent.frame(vec![key(Key::Tab, Modifiers::NONE)]);
    assert!(agent.ctx.memory(|memory| memory.focused().is_some()));
    let before = agent.app.spaces().unwrap().agent.task.clone();
    agent.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    let after = agent.app.spaces().unwrap().agent.task.clone();
    assert_ne!(
        before, after,
        "agent task list must browse with header focus"
    );
}

#[test]
fn idea_capture_guide_is_a_clickable_shortcut_button() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num4, ALT)]);
    // キー案内は折り返しレイアウトなので、安定したフレームで位置を取る
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let button = harness
        .rect_of("Ctrl+Shift+I 即時入力")
        .expect("visible capture button");
    harness.click_at(button.center());
    assert!(
        harness.app.overlay_open(),
        "opens the same capture as the keyboard shortcut"
    );
}

#[test]
fn cast_timeline_hover_covers_a_full_minimum_height_row() {
    let mut harness = novel_harness();
    harness.frame(vec![key(Key::Num6, ALT)]);
    let output = harness.frame(Vec::new());
    let label = painted_texts(&output)
        .into_iter()
        .find(|(text, rect)| text == "A 探究" && rect.min.x > 700.0)
        .expect("timeline band")
        .1;
    assert!(
        output.shapes.iter().any(|item| matches!(&item.shape,
            egui::Shape::LineSegment { points, stroke }
                if stroke.width == 0.6
                    && (points[0].x - label.min.x).abs() < 1.0
                    && (points[1].x - label.max.x).abs() < 1.0
                    && (points[0].y - label.max.y - 1.0).abs() < 1.0
        )),
        "clickable timeline band has a thin underline"
    );
    harness.frame(vec![Event::PointerMoved(label.center())]);
    let hovered = harness.frame(vec![Event::PointerMoved(label.center())]);
    assert_eq!(
        hovered.platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    let fill = novnow::ui::theme::palette(novnow::ui::theme::Mode::Dark)
        .bg_selection
        .gamma_multiply(0.6);
    assert!(hovered.shapes.iter().any(|item| matches!(&item.shape,
        egui::Shape::Rect(shape) if shape.fill == fill && shape.rect.height() >= 24.0 && shape.rect.contains(label.center())
    )), "timeline hover must cover a 24px band");
}

#[test]
fn plot_list_delete_button_uses_the_unified_delete() {
    let mut harness = novel_harness();
    let before = harness
        .app
        .workbench()
        .map(|bench| bench.sections().len())
        .unwrap();
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));
    let output = harness.frame(Vec::new());
    // 節リストの各行の右端に削除ボタンがある（§8.1.2）
    let mut buttons: Vec<egui::Rect> = harness::painted_texts(&output)
        .into_iter()
        .filter(|(text, _)| text == "削除")
        .map(|(_, rect)| rect)
        .collect();
    assert!(!buttons.is_empty(), "プロット一覧の削除ボタン");
    buttons.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
    harness.click_at(buttons[0].center());
    // 空でない節は既定で確認する（文脈ビューと同じ入口）
    assert_eq!(
        harness.app.overlay_kind(),
        Some(novnow::ui::app::OverlayKind::ConfirmDelete)
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    let bench = harness.app.workbench().expect("bench");
    assert_eq!(
        bench.sections().len(),
        before - 1,
        "本文の節も一緒に消える（§8.1.2）"
    );
    assert_eq!(harness.app.overlay_kind(), None);
}

#[test]
fn ctrl_shift_carryover_lands_on_every_spec_counterpart() {
    let mut project = novel::novel();
    project.insert(Document::new(
        "project.toml",
        format!("{}\n[agents]\nenabled = true\n", novel::PROJECT_TOML),
    ));
    project.insert(Document::new(
        "ideas.md",
        "# I0001 第一の着想\nstatus: inbox\n最初のメモ。\n\n# I0002 第二の着想\nstatus: keep\n二番目のメモ。\n",
    ));
    project.insert(Document::new(
        "materials/index.json",
        r#"{
  "version": 1,
  "items": [
    {
      "id": "M0100",
      "title": "別の資料",
      "category": "setting",
      "status": "approved",
      "path": "materials/docs/M0100.md",
      "source": { "kind": "manual", "origin": "著者メモ" }
    },
    {
      "id": "M0137",
      "title": "科学の甲子園",
      "category": "fact",
      "status": "approved",
      "path": "materials/docs/M0137.md",
      "source": { "kind": "manual", "origin": "大会記録" }
    }
  ]
}"#,
    ));
    project.insert(Document::new(
        "materials/docs/M0100.md",
        "# 別の資料\n内容A。\n",
    ));
    project.insert(Document::new(
        "materials/docs/M0137.md",
        "# 科学の甲子園\n内容B。\n",
    ));
    project.insert(Document::new(
        "plots/01-01.cnov",
        "@01-01-01 放課後\nmode: 'リリア:C\n二人が図書室で話す。\nstate: fixed\n;\n'アルス が資料を見せる。\n;\n'リリア が設計図を覗き込む。\nstate: draft\n;\n\n@01-01-02 帰り道\n二人で帰る。\n",
    ));
    project.insert(Document::new(
        "plots/01-03.cnov",
        "@01-03-01 屋上\nまだ本文の無いプロット節。\n",
    ));
    for scope in ["@01-01-01-01", "@01-01-01-02"] {
        let plan = runner::new_tasks(
            &project,
            &runner::Draft {
                role: "writer",
                scope,
                intent: "テンポ調整",
                comment: "",
                examples: "",
                initiator: "human",
                status: "draft",
            },
            "2026-10-13T21:00:00+09:00",
        );
        for change in &plan.writes {
            project.insert(Document::new(change.path.clone(), change.text.clone()));
        }
    }

    let (mut harness, _) = Harness::start_agents(project, |_| {});

    // 事前に各ワークスペースを先頭以外の場所へ動かしておく
    harness.frame(vec![key(Key::Num4, ALT)]);
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    assert_eq!(
        harness.app.spaces().and_then(|s| s.idea.selected()),
        Some("I0002")
    );
    harness.frame(vec![key(Key::Num5, ALT)]);
    assert_eq!(
        harness.app.spaces().and_then(|s| s.material.selected()),
        Some("M0100")
    );
    // agent 画面の一覧は新しい順（board.tasks.iter().rev()）
    let task_ids: Vec<String> = board(&harness)
        .tasks
        .into_iter()
        .rev()
        .map(|t| t.id)
        .collect();
    assert_eq!(task_ids.len(), 2);
    harness.frame(vec![key(Key::Num8, ALT)]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    assert_eq!(
        harness.app.spaces().and_then(|s| s.agent.selected_task()),
        Some(task_ids[1].as_str())
    );

    // 1. write(01-01-01-01, mode: 'リリア:C) → Ctrl+Shift+6 (cast): 先頭キャラ「アルス」(0) ではなく mode: の先頭「リリア」(1)
    harness.frame(vec![key(Key::Num1, ALT)]);
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 1))
    );
    harness.frame(vec![key(Key::Num6, CTRL_SHIFT)]);
    let out_cast = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Cast));
    assert_eq!(
        harness.app.spaces().and_then(|s| s.cast.selected()),
        Some(1)
    );
    assert!(painted_text(&out_cast).contains("# リリア"));

    // 2. write を @01-01-01-02 ('M0137 参照 + @アルス:B の E004 診断あり) に進める
    harness.frame(vec![key(Key::Num1, ALT)]);
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 2))
    );

    // write → Ctrl+Shift+2 (plot-episode): 同じ SceneId の節ブロック
    harness.frame(vec![key(Key::Num2, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));
    assert_eq!(selected(&harness), Some(id(1, 1, 1, 2)));

    // write → Ctrl+Shift+3 (plot-outline): 属する話 @01-01 のブロック
    harness.frame(vec![key(Key::Num1, ALT)]);
    harness.frame(vec![key(Key::Num3, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotOutline));
    assert_eq!(
        harness.app.spaces().and_then(|s| s.outline.selected()),
        Some(id(1, 1, 0, 0))
    );

    // 3. plot-outline (@01-01: 01-01-01-01 は fixed、01-01-01-02 が最初の state != fixed) → Ctrl+Shift+1 (write)
    harness.frame(vec![key(Key::Num1, ALT)]);
    harness.frame(vec![key(Key::ArrowUp, ALT)]);
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 1))
    );
    harness.frame(vec![key(Key::Num3, ALT)]);
    harness.frame(vec![key(Key::Num1, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 2)),
        "plot-outline → write は最初の state != fixed の節に着地する"
    );

    // 4. 任意 (01-01-01-02) → Ctrl+Shift+5 (material): 現在の節が参照している M0137 に着地
    harness.frame(vec![key(Key::Num5, CTRL_SHIFT)]);
    let out_mat = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Material));
    assert_eq!(
        harness.app.spaces().and_then(|s| s.material.selected()),
        Some("M0137")
    );
    assert!(painted_text(&out_mat).contains("# 科学の甲子園"));

    // 5. 任意 (01-01-01-02) → Ctrl+Shift+9 (review): 01-01-01-02 の診断行に着地
    harness.frame(vec![key(Key::Num1, ALT)]);
    harness.frame(vec![key(Key::Num9, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Review));
    let review_idx = harness.app.spaces().map(|s| s.review.selected()).unwrap();
    let cached = harness
        .app
        .spaces()
        .and_then(|s| s.review.cached())
        .expect("diagnostics cached");
    assert!(
        cached[review_idx]
            .place
            .as_deref()
            .is_some_and(|p| p.contains("01-01-01-02")),
        "現在の節の診断に着地する: {:?}",
        cached[review_idx]
    );

    // 6. 任意 → Ctrl+Shift+4 (idea) / Ctrl+Shift+8 (agent): 一覧の先頭に着地
    harness.frame(vec![key(Key::Num4, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(
        harness.app.spaces().and_then(|s| s.idea.selected()),
        Some("I0001")
    );
    harness.frame(vec![key(Key::Num8, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(
        harness.app.spaces().and_then(|s| s.agent.selected_task()),
        Some(task_ids[0].as_str())
    );

    // 7. plot-episode (本文ファイルが無い 01-03-01-01) → Ctrl+Shift+1 (write): 空の節ファイルを作らずカーソルだけ置く
    harness.frame(vec![key(Key::Num2, ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_ALT)]);
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    assert_eq!(selected(&harness), Some(id(1, 3, 1, 1)));
    harness.frame(vec![key(Key::Num1, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 3, 1, 1))
    );
    assert!(
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .document("contents/01-03.cnov")
            .is_none(),
        "文字を打つまで空の本文ファイルは作らない"
    );
}

#[test]
fn ctrl_shift_arrows_cycle_tabs_and_links_open_their_target_workspaces() {
    let mut base = novel::novel();
    // 引用注釈 ^M0137 を第2話に入れて stage.citations からの遷移も検証する
    base.insert(Document::new("materials/index.json", world::INDEX));
    base.insert(Document::new(
        "contents/01-02.cnov",
        "@01-02-01 旧校舎\n旧校舎の扉は錆びついていた。^M0137\n",
    ));
    let (mut harness, _) = Harness::start_with(base, true);

    // when: が 1 件も無いプロジェクトでは PlotOutline と Review に「時系列」タブを出さない
    harness.frame(vec![key(Key::Num3, ALT)]);
    let out_outline = harness.frame(Vec::new());
    assert!(
        !painted_text(&out_outline).contains("時系列"),
        "when: が無ければ時系列タブは出さない"
    );
    // Ctrl+Shift+→ でマトリクスへ進み、Ctrl+Shift+← で基本に戻る
    harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]);
    let out_matrix = harness.frame(Vec::new());
    assert!(painted_text(&out_matrix).contains("summary"));
    // マトリクスの先頭行で Enter を押すと該当プロットのワークスペースへ遷移して表示される
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(
        harness.app.tabs_state().map(|t| t.tabs[2]),
        Some(0),
        "リンク遷移で基本ビューが開く"
    );

    // Cast: モード → 役割語 → 呼称表 → モード（逆方向も）
    harness.frame(vec![key(Key::Num6, ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]); // Tester
    harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]); // Addresses
    let out_addr = harness.frame(Vec::new());
    assert_eq!(harness.app.tabs_state().map(|t| t.tabs[5]), Some(1));
    assert!(painted_text(&out_addr).contains("呼称表"));
    harness.frame(vec![key(Key::ArrowLeft, CTRL_SHIFT)]); // Tester へ戻る
    assert_eq!(harness.app.tabs_state().map(|t| t.tabs[5]), Some(0));
    assert_eq!(
        harness.app.spaces().map(|s| s.cast.tab),
        Some(novnow::ui::workspaces::cast::Tab::Tester)
    );

    // Material: Ctrl+Shift+→ で「引用元から本文へ」タブへ移り、Enter で Write の 01-02-01-01 が開く
    harness.frame(vec![key(Key::Num5, ALT)]);
    harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]);
    let out_cit = harness.frame(Vec::new());
    assert!(painted_text(&out_cit).contains("contents/01-02.cnov"));
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 2, 1, 1))
    );

    // Review: Ctrl+Shift+← で末尾の「提案」タブへ逆巡回し、Ctrl+Shift+→ で「診断」へ戻る
    harness.frame(vec![key(Key::Num9, ALT)]);
    harness.frame(vec![key(Key::ArrowLeft, CTRL_SHIFT)]);
    let out_prop = harness.frame(Vec::new());
    assert_eq!(harness.app.tabs_state().map(|t| t.tabs[8]), Some(8));
    assert!(painted_text(&out_prop).contains("提案"));
    harness.frame(vec![key(Key::ArrowRight, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.tabs_state().map(|t| t.tabs[8]), Some(0));
    assert_eq!(
        harness.app.spaces().map(|s| s.review.tab),
        Some(novnow::ui::workspaces::review::Tab::Diagnostics)
    );
}

#[test]
fn reader_toc_cover_folio_clean_chrome_and_escape_return_stay_aligned() {
    let mut project = novel::novel();
    // 長大な blurb を設定しても表紙は必ず 1 ページに収まること、複数ページの本文で目次と Esc 復帰が一致すること
    let long_blurb = "あらすじの文。".repeat(300);
    project.insert(Document::new(
        "plots/all.cnov",
        format!(
            "title: 星屑の時計\n\n@01 失われた刻印\nsummary: 導入章。\nblurb: {long_blurb}\n;\n"
        ),
    ));
    let sec1 = "春の風が吹く図書室で｜古文書《こもんじょ》をめくる。\n".repeat(35);
    let sec2 =
        "// 先頭コメント\n「見つけたぞ」'アルス @アルス:A\n星の地図が光り始める場面の本文。\n"
            .repeat(25);
    let sec3 = "夕暮れの帰り道で二人が約束を交わす結びの場面。\n".repeat(20);
    project.insert(Document::new(
        "contents/01-01.cnov",
        format!("@01-01-01 放課後\n{sec1};\n{sec2};\n{sec3}"),
    ));

    let (mut harness, _) = Harness::start_with(project, true);
    // 第2節 (01-01-01-02) から Ctrl+Shift+7 で読者モードへ強制持ち越し
    harness.frame(vec![key(Key::ArrowDown, ALT)]);
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 2))
    );
    harness.frame(vec![key(Key::Num7, CTRL_SHIFT)]);
    let out_sec2 = harness.frame(Vec::new());
    let text_sec2 = painted_text(&out_sec2);

    // §17.3.5: パンくず・保存状態・診断バッジは読者モードで一切表示しない
    assert!(
        !text_sec2.contains("01›01"),
        "パンくずは出さない: {text_sec2}"
    );
    assert!(
        !text_sec2.contains('⚑'),
        "診断件数バッジは出さない: {text_sec2}"
    );
    // §17.3.2: 柱（章題）とノンブルが描かれている
    assert!(
        text_sec2.contains("失われた刻印"),
        "柱に章題が出る: {text_sec2}"
    );
    let sec2_page = harness.app.tabs_state().map(|t| t.reader_page).unwrap();
    assert!(
        sec2_page >= 2,
        "表紙(0) + 第1節(1..) の後ろのページに着地する: page={sec2_page}"
    );
    assert_eq!(
        harness.app.tabs_state().and_then(|t| t.reader_section),
        Some(id(1, 1, 1, 2))
    );

    // Home で先頭（表紙 = page 0）へ行き、1回 ← で第1節の先頭（page 1 = p.2）になる（＝長大 blurb でも表紙は 1 枚！）
    harness.frame(vec![key(Key::Home, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.tabs_state().map(|t| t.reader_page), Some(0));
    harness.frame(vec![key(Key::ArrowLeft, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.tabs_state().map(|t| t.reader_page), Some(1));
    assert_eq!(
        harness.app.tabs_state().and_then(|t| t.reader_section),
        Some(id(1, 1, 1, 1)),
        "2ページ目（index 1）が第1節の先頭＝表紙は厳密に1枚"
    );

    // 目次を開いて第3節 01-01-01-03 のリンクをクリックすると、表示されたページ番号どおりのページへ飛び、Esc で 01-01-01-03 の節エディタに戻る
    let toc_out = harness.frame(Vec::new());
    let toc_header = painted_texts(&toc_out)
        .into_iter()
        .find(|(t, _)| t == "目次")
        .map(|(_, r)| r)
        .expect("目次ヘッダ");
    harness.click_at(toc_header.center());
    let opened_toc = harness.frame(Vec::new());
    let (toc_label, toc_rect) = painted_texts(&opened_toc)
        .into_iter()
        .find(|(t, _)| t.starts_with("01-01-01-03"))
        .expect("第3節の目次リンク");
    let expected_page_1based: usize = toc_label
        .rsplit("p.")
        .next()
        .and_then(|n| n.trim().parse().ok())
        .expect("目次ページ番号");
    harness.click_at(toc_rect.center());
    let after_click = harness.frame(Vec::new());
    let actual_page_0based = harness.app.tabs_state().map(|t| t.reader_page).unwrap();
    assert_eq!(
        actual_page_0based + 1,
        expected_page_1based,
        "目次のページ表記とジャンプ後の実ページが完全一致する"
    );
    assert!(
        painted_text(&after_click).contains(&format!("{expected_page_1based} / ")),
        "ノンブルも一致する"
    );

    // Esc で編集に戻ると、注釈やルビがあっても正確に第3節 01-01-01-03 に着地する
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Write));
    assert_eq!(
        harness.app.workbench().and_then(Workbench::current_id),
        Some(id(1, 1, 1, 3)),
        "Esc で直前に読んでいた節の節エディタに正確に戻る"
    );
}

#[test]
fn plot_episode_when_label_insert_mode_space_popup_and_section_swap_work() {
    let mut project = novel::novel();
    project.insert(Document::new(
        "plots/01-01.cnov",
        "@01-01-01 広場\nspace: 石畳の中央広場\n第一節の要約\nwhen: 第三日 夕刻\n;\n第二節の要約\nwhen: 第三日 夜\n",
    ));
    let mut harness = Harness::start_with(project, true).0;
    harness.frame(vec![key(Key::Num2, ALT)]);
    let out = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));

    // 1. 右端に when: の値が表示されている
    let shown = painted_text(&out);
    assert!(
        shown.contains("第三日 夕刻"),
        "when: が節リスト右端に表示される: {shown}"
    );

    // 2. Ctrl+Shift+Space で空間設計メモの全文ポップアップが開く
    harness.frame(vec![key(Key::Space, CTRL_SHIFT)]);
    let popup_out = harness.frame(Vec::new());
    assert_eq!(
        harness.app.overlay_kind(),
        Some(novnow::ui::app::OverlayKind::Info)
    );
    assert!(painted_text(&popup_out).contains("石畳の中央広場"));
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    harness.frame(Vec::new());

    // 3. Ctrl+D で mode: 行を補完から挿入できる（着地時は第1節が選択されている）
    harness.frame(vec![key(Key::D, CTRL)]);
    harness.frame(Vec::new());
    assert_eq!(
        harness.app.overlay_kind(),
        Some(novnow::ui::app::OverlayKind::InsertMode)
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    let plot_after_mode = harness
        .app
        .workbench()
        .unwrap()
        .project()
        .text("plots/01-01.cnov")
        .to_owned();
    assert!(
        plot_after_mode.contains("mode: '"),
        "Ctrl+D → Enter で mode: 行が挿入される: {plot_after_mode}"
    );

    // 4. Alt+Shift+Down で本文あり節の入替確認モーダルが開き、Enter で連動入替される
    harness.frame(vec![key(Key::ArrowDown, ALT | Modifiers::SHIFT)]);
    harness.frame(Vec::new());
    assert_eq!(
        harness.app.overlay_kind(),
        Some(novnow::ui::app::OverlayKind::ConfirmSwapSections)
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    let plot_after_swap = harness
        .app
        .workbench()
        .unwrap()
        .project()
        .text("plots/01-01.cnov")
        .to_owned();
    let idx_second = plot_after_swap.find("第二節の要約").unwrap();
    let idx_first = plot_after_swap.find("第一節の要約").unwrap();
    assert!(
        idx_second < idx_first,
        "節プロットが入れ替わる: {plot_after_swap}"
    );
}

#[test]
fn outline_tree_keeps_stock_last_and_search_auto_expands_folded_units() {
    let mut project = novel::novel();
    project.insert(Document::new(
        "project.toml",
        "[project]\ntitle = \"t\"\n[structure]\nallow_chapter_zero = true\n",
    ));
    project.insert(Document::new(
        "plots/all.cnov",
        "@00\n;\n@01 序章\n;\n@02 転章\n",
    ));
    project.insert(Document::new(
        "contents/00-01.cnov",
        "@00-01-01\n秘蔵ストック本文\n",
    ));
    let mut harness = Harness::start_with(project, true).0;

    // PlotOutline を開く: ストック章 00（「未分類ストック」）は 01・02 より下（最後尾）に描かれる
    harness.frame(vec![key(Key::Num3, ALT)]);
    let out = harness.frame(Vec::new());
    let texts = painted_texts(&out);
    let y_ch1 = texts
        .iter()
        .find(|(t, _)| t.contains("序章"))
        .map(|(_, r)| r.center().y)
        .expect("序章");
    let y_ch2 = texts
        .iter()
        .find(|(t, _)| t.contains("転章"))
        .map(|(_, r)| r.center().y)
        .expect("転章");
    let y_stock = texts
        .iter()
        .find(|(t, _)| t.contains("未分類ストック"))
        .map(|(_, r)| r.center().y)
        .expect("未分類ストック");
    assert!(
        y_ch1 < y_ch2 && y_ch2 < y_stock,
        "ストック章は常に最後尾に並ぶ"
    );

    // すべて折りたたんだ状態から Ctrl+Shift+F で検索すると該当単位が自動展開される
    harness.frame(vec![key(Key::K, CTRL)]);
    harness.frame(vec![key(Key::Num0, CTRL)]);
    harness.frame(vec![key(Key::F, CTRL_SHIFT)]);
    harness.frame(vec![egui::Event::Text("秘蔵ストック".into())]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    let expanded_out = harness.frame(Vec::new());
    assert!(
        painted_text(&expanded_out).contains("00-01"),
        "検索ヒットにより折りたたまれたストック章が自動展開される"
    );
}

#[test]
fn save_search_with_ctrl_shift_s_and_material_rerun_diff_preview() {
    let mut harness = novel_harness();
    // Write で Ctrl+Shift+F → 検索タブが開いて入力欄にフォーカス → "アルス" + Enter で検索 → Ctrl+Shift+S
    harness.frame(vec![key(Key::F, CTRL_SHIFT)]);
    harness.frame(Vec::new());
    harness.frame(vec![
        egui::Event::Text("アルス".into()),
        key(Key::Enter, Modifiers::NONE),
    ]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::S, CTRL_SHIFT)]);
    let modal_out = harness.frame(Vec::new());
    assert!(
        painted_text(&modal_out).contains("この検索結果を資料にする"),
        "Ctrl+Shift+S で検索資料化モーダルが開く: open={:?} text={}",
        harness.app.tabs_state().map(|t| t.save_search_open),
        painted_text(&modal_out)
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());

    let saved_path = harness
        .app
        .workbench()
        .unwrap()
        .project()
        .documents()
        .find(|d| d.path.starts_with("materials/docs/S") && d.path.ends_with("-アルス.md"))
        .map(|d| d.path.clone())
        .expect("S####-<slug>.md として保存される");
    assert_eq!(saved_path, "materials/docs/S0138-アルス.md");

    // Write 節エディタへ戻って本文に新しいヒットを追記してから Material ワークスペースで [再検索]
    let (_, basic_tab) = painted_texts(&harness.frame(Vec::new()))
        .into_iter()
        .find(|(t, _)| t == "基本")
        .expect("基本タブ");
    harness.click_at(basic_tab.center());
    harness.frame(vec![key(Key::Num2, CTRL)]);
    harness.frame(Vec::new());
    harness.frame(vec![egui::Event::Text("\nアルスが新しく頷いた。\n".into())]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::Num5, ALT)]);
    let detail_out = harness.frame(Vec::new());
    let (_, rerun_btn) = painted_texts(&detail_out)
        .into_iter()
        .find(|(t, _)| t == "再検索")
        .expect("再検索ボタン");
    harness.click_at(rerun_btn.center());
    let diff_out = harness.frame(Vec::new());
    assert!(
        painted_text(&diff_out).contains("新規ヒット"),
        "再検索で未適用の差分プレビューが表示される: {}",
        painted_text(&diff_out)
    );
    // まだ保存済み本文は書き換わっていない
    assert!(
        !harness
            .app
            .workbench()
            .unwrap()
            .project()
            .text(&saved_path)
            .contains("新しく頷いた"),
        "人間が採用するまで本文は自動上書きされない"
    );
    let (_, apply_btn) = painted_texts(&diff_out)
        .into_iter()
        .find(|(t, _)| t == "この検索結果で更新を適用")
        .expect("適用ボタン");
    harness.click_at(apply_btn.center());
    harness.frame(Vec::new());
    assert!(
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .text(&saved_path)
            .contains("新しく頷いた"),
        "適用ボタンで本文が更新される"
    );
}

#[test]
fn stock_tab_bidirectional_assign_and_return_work_in_gui() {
    let mut project = novel::novel();
    project.insert(Document::new(
        "project.toml",
        "[project]\ntitle = \"t\"\n[structure]\nallow_chapter_zero = true\n",
    ));
    project.insert(Document::new(
        "contents/00-01.cnov",
        "@00-01-01\nストックの挿話。\n",
    ));
    project.insert(Document::new(
        "plots/00.cnov",
        "@00-01 秘蔵挿話\nsummary: 幕間の構想\n",
    ));
    let mut harness = Harness::start_with(project, true).0;

    // PlotOutline → ストック配属タブ（末尾タブなので Ctrl+Shift+Left 1 回で到達）
    harness.frame(vec![key(Key::Num3, ALT)]);
    harness.frame(vec![key(Key::ArrowLeft, CTRL_SHIFT)]);
    let stock_out = harness.frame(Vec::new());
    let text = painted_text(&stock_out);
    assert!(
        text.contains("00-01") && text.contains("幕間の構想"),
        "ストック一覧に話番号と要約が表示される: {text}"
    );

    // Ctrl+Enter で章・話番号指定モーダルを開き、Enter で既定の空き話番号へ配属
    harness.frame(vec![key(Key::Enter, CTRL)]);
    let modal_out = harness.frame(Vec::new());
    assert!(
        painted_text(&modal_out).contains("移動先の章番号"),
        "stock modal open={:?} text={}",
        harness
            .app
            .tabs_state()
            .map(|t| (t.tabs[2], t.stock_modal_open)),
        painted_text(&modal_out)
    );
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert!(
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .document("contents/00-01.cnov")
            .is_none(),
        "ストックから正式な章へファイルが移動した"
    );
}

fn text_rect(output: &egui::FullOutput, needle: &str) -> Option<egui::Rect> {
    painted_texts(output)
        .into_iter()
        .find(|(t, _)| t == needle)
        .or_else(|| {
            painted_texts(output)
                .into_iter()
                .find(|(t, _)| t.contains(needle))
        })
        .map(|(_, r)| r)
}

#[test]
fn address_matrix_enter_commit_escape_cancel_candidate_extract_and_w084_pair_jump_work() {
    let mut project = novel::novel();
    project.insert(Document::new(
        "characters.md",
        "# アルス\nreading: あるす\naddress: リリア => リリア嬢\n## modes\n### B\naddress: リリア => お前\n\n# リリア\nreading: りりあ\n\n# 佐倉\nreading: さくら\n",
    ));
    project.insert(Document::new(
        "contents/01-01.cnov",
        "@01-01-01\n「リリア、佐倉先生を呼んでくれ」@アルス\n",
    ));
    let mut harness = Harness::start_with(project, true).0;

    // 1. Review の診断一覧で W084 を選び、「呼称ペアを開く」ボタンで Cast の呼称タブの該当ペアへ移動する
    harness.frame(vec![key(Key::Num9, ALT)]);
    let rev_out = harness.frame(Vec::new());
    // W084 の行を選択する
    let w084_idx = harness
        .app
        .spaces()
        .and_then(|s| s.review.cached())
        .and_then(|rows| rows.iter().position(|r| r.code == "W084"))
        .expect("W084 diagnostic must exist");
    for _ in 0..w084_idx {
        harness.frame(vec![key(Key::ArrowDown, Modifiers::NONE)]);
    }
    let w084_out = harness.frame(Vec::new());
    let _ = rev_out;
    let open_pair_btn = text_rect(&w084_out, "呼称ペアを開く")
        .expect("W084 選択時は呼称ペアを開くボタンが表示される");
    harness.click_at(open_pair_btn.center());
    let addr_out = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Cast));
    assert_eq!(harness.app.tabs_state().map(|t| t.tabs[5]), Some(1));
    assert_eq!(
        harness
            .app
            .tabs_state()
            .and_then(|t| t.address_selected_pair.clone()),
        Some(("アルス".to_owned(), "リリア".to_owned()))
    );
    assert!(
        painted_text(&addr_out).contains("W084 違反箇所"),
        "選択ペアの W084 セクションが表示される"
    );

    // 2. Enter でインライン編集を開始し、書き換えてから Esc でキャンセルすると characters.md は変更されない
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(vec![Event::Text("キャンセル呼称".to_owned())]);
    harness.frame(vec![key(Key::Escape, Modifiers::NONE)]);
    harness.frame(Vec::new());
    let after_cancel = harness
        .app
        .workbench()
        .unwrap()
        .project()
        .text("characters.md")
        .to_owned();
    assert!(
        !after_cancel.contains("キャンセル呼称") && after_cancel.contains("リリア嬢"),
        "Esc キャンセル時は characters.md を変更しない: {after_cancel}"
    );

    // 3. モード `:B` に切り替え、Enter で編集開始 → 入力 → Enter で確定するとモード別ペアに保存される
    let mode_b = text_rect(&addr_out, ":B").expect(":B mode selector");
    harness.click_at(mode_b.center());
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(vec![key(Key::A, CTRL), Event::Text("貴様".to_owned())]);
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    let after_commit = harness
        .app
        .workbench()
        .unwrap()
        .project()
        .text("characters.md")
        .to_owned();
    assert!(
        after_commit.contains("### B\naddress: リリア => 貴様"),
        "Enter 確定でモード別 address: が書き戻される: {after_commit}"
    );
    assert!(
        after_commit.contains("address: リリア => リリア嬢"),
        "基底モードの定義は保たれる: {after_commit}"
    );

    // 4. Ctrl+Shift+A で候補抽出: 自動では書き込まず、登録ボタンを押した後に書き込まれる
    harness.frame(vec![key(Key::A, CTRL_SHIFT)]);
    let cand_out = harness.frame(Vec::new());
    assert!(
        painted_text(&cand_out).contains("佐倉先生"),
        "会話内の呼称候補『佐倉先生』が提示される: {}",
        painted_text(&cand_out)
    );
    assert!(
        !harness
            .app
            .workbench()
            .unwrap()
            .project()
            .text("characters.md")
            .contains("佐倉先生"),
        "自動では characters.md に書き込まない"
    );
    let reg_btn = text_rect(&cand_out, "選択した候補を登録").expect("register selected button");
    harness.click_at(reg_btn.center());
    harness.frame(Vec::new());
    assert!(
        harness
            .app
            .workbench()
            .unwrap()
            .project()
            .text("characters.md")
            .contains("佐倉 => 佐倉先生"),
        "人間が登録を実行した後に characters.md へ反映される"
    );
}

#[test]
fn review_tabs_metrics_scopes_senses_jump_and_diagnostic_level_persistence_work() {
    let mut project = novel::novel();
    project.insert(Document::new(
        "plots/all.cnov",
        "title: テスト作品\n\n@01 第一章\nsummary: 導入\nwhen: 405\n;\n@02 第二章\nsummary: 展開\n;\n",
    ));
    project.insert(Document::new(
        "plots/01.cnov",
        "@01-01 第一話\nsummary: 発端\n;\n@01-02 第二話\nsummary: 事件\nwhen: 410\n;\n@01-03 第三話\nsummary: 逆転\nwhen: 408\n;\n@01-04 第四話\nsummary: 過去編\nwhen: 200 回想\n;\n",
    ));
    project.insert(Document::new(
        "plots/01-01.cnov",
        "@01-01-01 図書室\ncast: 'アルス 'リリア\npov: 'アルス\nsenses: 視覚\nlength: 20\n;\nsenses: 視覚\n;\nsenses: 視覚\n;\nsenses: 視覚\n;\nsenses: 視覚\n;\nsenses: 聴覚\n;\n",
    ));
    project.insert(Document::new(
        "plots/02-01.cnov",
        "@02-01-01 中庭\npov: 'リリア\n;\n",
    ));
    project.insert(Document::new(
        "contents/01-01.cnov",
        "@01-01-01\n「おはよう」@アルス\n光が差す。\n;\n二節。\n;\n三節。\n;\n四節。\n;\n五節。\n;\n「聞こえる？」\n",
    ));
    project.insert(Document::new(
        "contents/02-01.cnov",
        "@02-01-01\n「二章よ」@リリア\n風が吹く。\n",
    ));
    project.insert(Document::new(
        ".novnow/stats.json",
        r#"{"schema_version":1,"days":{"2026-09-01":{"chars":100,"hours":[9]},"2026-09-02":{"chars":120,"hours":[10]},"2026-09-03":{"chars":140,"hours":[11]},"2026-09-04":{"chars":90,"hours":[12]}}}"#,
    ));
    let mut harness = Harness::start_with(project, true).0;

    // 1. Review 診断設定パネルから W097 を error に変更し project.toml へ永続化されることを確認
    harness.frame(vec![key(Key::Num9, ALT)]);
    let diag_out = harness.frame(Vec::new());
    let settings_btn = text_rect(&diag_out, "診断設定").expect("診断設定ボタン");
    harness.click_at(settings_btn.center());
    let panel_out = harness.frame(Vec::new());
    let filter_box = text_rect(&panel_out, "コードで絞り込み").expect("絞り込み欄");
    harness.click_at(filter_box.center());
    harness.frame(vec![Event::Text("W097".to_owned())]);
    let filtered_out = harness.frame(Vec::new());
    let as_err_btn = text_rect(&filtered_out, "エラー扱い").expect("エラー扱いボタン");
    harness.click_at(as_err_btn.center());
    harness.frame(Vec::new());
    let saved_toml = harness
        .app
        .workbench()
        .unwrap()
        .project()
        .text("project.toml")
        .to_owned();
    assert!(
        saved_toml.contains("[features.diagnostics]") && saved_toml.contains("W097 = \"error\""),
        "診断設定が project.toml に永続化される: {saved_toml}"
    );

    // 2. 時系列タブ（tabs[8] = 1）: 継承・回想・巻き戻り警告 ⚠ が表示される
    let tl_tab = text_rect(&filtered_out, "時系列").expect("時系列タブ");
    harness.click_at(tl_tab.center());
    let tl_out = harness.frame(Vec::new());
    let tl_text = painted_text(&tl_out);
    assert!(
        tl_text.contains("（継承）") && tl_text.contains("（回想）") && tl_text.contains("⚠"),
        "時系列タブに継承・回想・矛盾警告が表示される: {tl_text}"
    );

    // 3. 話者別タブ（tabs[8] = 2）: 登場節数・視点節数・（不明）・章範囲切替
    let sp_tab = text_rect(&tl_out, "話者別").expect("話者別タブ");
    harness.click_at(sp_tab.center());
    let sp_out = harness.frame(Vec::new());
    let sp_text = painted_text(&sp_out);
    assert!(
        sp_text.contains("（不明）") && sp_text.contains("登場節"),
        "話者別タブに話者不明と登場節・視点節列が表示される: {sp_text}"
    );
    let ch2_btn = text_rect(&sp_out, "章 02").expect("章 02 スコープボタン");
    harness.click_at(ch2_btn.center());
    let sp_ch2_out = harness.frame(Vec::new());
    assert!(
        !painted_text(&sp_ch2_out).contains("（不明）"),
        "章 02 に切り替えると 1 章のみの話者不明行は集計外になる"
    );
    let all_btn = text_rect(&sp_ch2_out, "全体").expect("全体ボタン");
    harness.click_at(all_btn.center());
    harness.frame(Vec::new());

    // 4. 字数分布タブ: 中央値・平均・最長・最短と目標比が表示される
    let hist_tab = text_rect(&sp_ch2_out, "字数分布").expect("字数分布タブ");
    harness.click_at(hist_tab.center());
    let hist_out = harness.frame(Vec::new());
    let hist_text = painted_text(&hist_out);
    assert!(
        hist_text.contains("中央値") && hist_text.contains("目標比"),
        "字数分布に代表値と目標比が表示される: {hist_text}"
    );

    // 4.5. 文体観察タブ: 章ごと・話ごとの台詞比率バーと 1 文長分布が表示される
    let style_tab = text_rect(&hist_out, "文体観察").expect("文体観察タブ");
    harness.click_at(style_tab.center());
    let style_out = harness.frame(Vec::new());
    let style_text = painted_text(&style_out);
    assert!(
        style_text.contains("台詞比率") && style_text.contains("1文の長さ分布"),
        "文体観察タブに台詞比率バーと 1 文長分布が表示される: {style_text}"
    );

    // 5. 執筆統計タブ: 最長連続記録 4日 が表示される
    let stats_tab = text_rect(&style_out, "執筆統計").expect("執筆統計タブ");
    harness.click_at(stats_tab.center());
    let stats_out = harness.frame(Vec::new());
    assert!(
        painted_text(&stats_out).contains("最長連続記録: 4日"),
        "最長連続記録が表示される: {}",
        painted_text(&stats_out)
    );

    // 6. 五感タブ: 宣言節数・未宣言節数・5節連続視覚区間が表示され、Enter で節一覧 → Enter でプロットへ遷移
    let senses_tab = text_rect(&stats_out, "五感").expect("五感タブ");
    harness.click_at(senses_tab.center());
    let senses_out = harness.frame(Vec::new());
    let senses_text = painted_text(&senses_out);
    assert!(
        senses_text.contains("未宣言 4節") && senses_text.contains("視覚のみが 5節連続"),
        "五感タブに未宣言数と 5節連続視覚区間が表示される: {senses_text}"
    );
    // 1 回目の Enter で視覚の節一覧を展開し、下へ動かして Enter で話プロットへジャンプ
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    harness.frame(vec![key(Key::Enter, Modifiers::NONE)]);
    harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::PlotEpisode));
}

#[test]
fn agent_threads_are_navigable_and_consistent_between_review_and_agent_workspaces() {
    let mut project = world::world();
    project.insert(Document::new(
        "project.toml",
        "[project]\ntitle = \"t\"\n[agents]\nenabled = true\n",
    ));
    project.insert(Document::new(
        "agents/threads/TH0001.md",
        "# TH0001 伏線照会の連絡\nscope: @01-01-01-01\nstatus: open\n\n## msg 001\nfrom: writer\nto: continuity\nkind: question\ncreated_at: 2026-11-12T09:00:00Z\napproved_by: pending\nrefs: 'M0137\nbody:\n  時計の針の向きは右回りで整合しますか？\n",
    ));
    let mut harness = Harness::start_with(project, true).0;

    // 1. Review ワークスペースで「スレッド」タブを開き、TH0001・本文・pending を確認
    harness.frame(vec![key(Key::Num9, ALT)]);
    let rev_out = harness.frame(Vec::new());
    let threads_btn = text_rect(&rev_out, "スレッド").expect("Review スレッドタブ");
    harness.click_at(threads_btn.center());
    let rev_th_out = harness.frame(Vec::new());
    let rev_text = painted_text(&rev_th_out);
    assert!(
        rev_text.contains("TH0001")
            && rev_text.contains("時計の針の向きは右回りで整合しますか？")
            && rev_text.contains("承認: pending"),
        "Review でスレッド ID・メッセージ本文・未承認状態が確認できる: {rev_text}"
    );

    // 2. 「エージェント管制で開く」で Agent ワークスペースへ遷移し、同一スレッドを確認して承認する
    let to_agent_btn =
        text_rect(&rev_th_out, "エージェント管制で開く").expect("エージェント管制で開くボタン");
    harness.click_at(to_agent_btn.center());
    let agent_out = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Agent));
    let agent_text = painted_text(&agent_out);
    assert!(
        agent_text.contains("TH0001")
            && agent_text.contains("時計の針の向きは右回りで整合しますか？")
            && agent_text.contains("承認: pending"),
        "Agent でも同一スレッドの本文と承認状態が確認できる: {agent_text}"
    );

    let approve_btn = text_rect(&agent_out, "未承認を承認").expect("未承認を承認ボタン");
    harness.click_at(approve_btn.center());
    let approved_out = harness.frame(Vec::new());
    assert!(
        painted_text(&approved_out).contains("承認: human"),
        "承認後に human へ更新される: {}",
        painted_text(&approved_out)
    );

    // 3. 「点検で開く」で Review に戻っても同じ TH0001 が human 承認済みで表示される
    let to_review_btn = text_rect(&approved_out, "点検で開く").expect("点検で開くボタン");
    harness.click_at(to_review_btn.center());
    let back_out = harness.frame(Vec::new());
    assert_eq!(harness.app.workspace(), Some(Workspace::Review));
    assert!(
        painted_text(&back_out).contains("TH0001")
            && painted_text(&back_out).contains("承認: human"),
        "Review に戻っても承認済み状態が一致する: {}",
        painted_text(&back_out)
    );
}

#[test]
fn human_proposal_discard_and_external_stale_keep_history_and_protect_manuscript_in_gui() {
    let mut project = novel::novel();
    project.insert(Document::new(
        "agents/proposals/H0001.md",
        "# H0001\nkind: human\ntarget: contents/01-01.cnov @01-01-01-01\norigin: human\nstatus: open\ncreated_at: 2026-11-12T10:00:00Z\nupdated_at: 2026-11-12T10:00:00Z\n\n## before\n春の風が吹いていた。\n\n## after\n冷たい北風が吹いていた。\n",
    ));
    project.insert(Document::new(
        "agents/proposals/H0002.md",
        "# H0002\nkind: human\ntarget: contents/01-01.cnov @01-01-01-01\norigin: human\nstatus: open\ncreated_at: 2026-11-12T10:01:00Z\nupdated_at: 2026-11-12T10:01:00Z\n\n## before\n古い版のスナップショット。\n\n## after\n採用されてはいけない提案文。\n",
    ));
    let mut harness = Harness::start_with(project, true).0;

    // 1. Review → 人間の提案タブ（Ctrl+Shift+Left で末尾タブへ）で H0002 は即座に stale + W093 になり、H0001 は [破棄] で discarded になる
    harness.frame(vec![key(Key::Num9, ALT)]);
    harness.frame(vec![key(Key::ArrowLeft, CTRL_SHIFT)]);
    let prop_out = harness.frame(Vec::new());
    let prop_text = painted_text(&prop_out);
    assert!(
        prop_text.contains("H0002 stale"),
        "不一致の提案は status: stale として表示される: {prop_text}"
    );
    let h2_saved = harness
        .app
        .workbench()
        .unwrap()
        .project()
        .text("agents/proposals/H0002.md")
        .to_owned();
    assert!(
        h2_saved.contains("status: stale"),
        "H0002 のファイル自体も status: stale に更新される: {h2_saved}"
    );

    let discard_btn = text_rect(&prop_out, "破棄").expect("破棄ボタン");
    harness.click_at(discard_btn.center());
    let after_discard = harness.frame(Vec::new());
    assert!(
        painted_text(&after_discard).contains("H0001 discarded"),
        "破棄後も一覧に discarded として残る: {}",
        painted_text(&after_discard)
    );
    let kept_text = harness
        .app
        .workbench()
        .unwrap()
        .project()
        .text("agents/proposals/H0001.md")
        .to_owned();
    assert!(
        kept_text.contains("status: discarded") && kept_text.contains("冷たい北風が吹いていた。"),
        "ファイルと履歴は削除されない: {kept_text}"
    );
    assert!(
        !harness
            .app
            .workbench()
            .unwrap()
            .project()
            .text("contents/01-01.cnov")
            .contains("採用されてはいけない提案文。"),
        "stale 提案は本文を上書きしない"
    );
}
