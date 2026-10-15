#![cfg(feature = "ui")]

mod harness;
mod world;

use novnow::core::model::document::Document;
use novnow::ui::widgets::section_editor;
use novnow::ui::workbench::Workbench;
use std::time::{Duration, Instant};
use world::*;

#[test]
fn typing_scans_only_the_active_section_of_a_100k_character_file() {
    let mut project = world_with(CONTENTS_PATH, "@01-04-02 x\n短い節。\n;\n");
    let mut text = "@01-04-02 x\n短い節。\n;\n".to_owned();
    while text.chars().count() < 100_000 {
        text.push_str(
            "これは前後の節にある文章です。繰り返しても編集中の節の同期解析には含めません。\n",
        );
    }
    project.insert(Document::new(CONTENTS_PATH, text));
    let mut bench = Workbench::new(project, Some("01-04-02-01"), 400);
    let mut worst = Duration::ZERO;
    for index in 0..12 {
        bench
            .buffer_mut()
            .push_str(if index % 2 == 0 { "あ" } else { "い" });
        bench.edited(index as f64);
        let started = Instant::now();
        let result = section_editor::highlight_of(&bench, bench.buffer(), true);
        worst = worst.max(started.elapsed());
        std::hint::black_box(result);
    }
    // 16 ms frame budget for a 100,000-character file.
    assert!(
        worst < Duration::from_millis(16),
        "active-section scan took {worst:?}"
    );
}

/// F-43: measure the *whole* egui frame after a real Text event, not just highlighting.
/// The large file is divided into sections so the active section is short, as required
/// by the per-section synchronous analysis contract. Run in --release for the 16ms target.
#[test]
fn a_keystroke_in_a_100k_character_file_fits_the_frame_budget() {
    let mut text = "@01-04-02 x\n短い節。\n;\n".to_owned();
    let other = "隣の節は編集中ではありません。描画と診断を打鍵と同期させません。\n";
    while text.chars().count() < 100_000 {
        text.push_str(other);
        text.push_str(";\n");
    }
    let project = world_with(CONTENTS_PATH, &text);
    let mut ui = harness::Harness::start_with(project, true).0;
    ui.frame(vec![harness::key(egui::Key::Num2, egui::Modifiers::CTRL)]);
    // Allow font atlas and initial caches to settle before measuring the typing path.
    for _ in 0..3 {
        ui.frame(Vec::new());
    }
    let mut worst = Duration::ZERO;
    for _ in 0..8 {
        let started = Instant::now();
        ui.frame(vec![egui::Event::Text("あ".to_owned())]);
        worst = worst.max(started.elapsed());
    }
    assert!(
        ui.app.workbench().unwrap().buffer().matches("あ").count() == 8,
        "text events must reach the editor"
    );
    eprintln!("100k-character whole-frame worst keystroke: {worst:?}");
    let limit = if cfg!(debug_assertions) { 250 } else { 16 };
    assert!(
        worst < Duration::from_millis(limit),
        "whole egui typing frame took {worst:?} (limit {limit}ms)"
    );
}

/// F-38: a 1,000-row idea list must scroll within one frame for both browsing
/// and committing navigation. Measure in --release to enforce the 60fps budget.
#[test]
fn thousand_ideas_scroll_within_the_frame_budget() {
    let ideas = (1..=1000)
        .map(|number| format!("# I{number:04} Idea {number:04}\nstatus: active\n\n"))
        .collect::<String>();
    let mut worst = Duration::ZERO;
    for modifiers in [egui::Modifiers::NONE, egui::Modifiers::ALT] {
        let mut project = world();
        project.insert(Document::new("ideas.md", ideas.clone()));
        let mut ui = harness::Harness::start_with(project, true).0;
        ui.frame(vec![harness::key(egui::Key::Num4, egui::Modifiers::ALT)]);
        for _ in 0..3 {
            ui.frame(Vec::new());
        }
        for _ in 0..80 {
            let started = Instant::now();
            ui.frame(vec![harness::key(egui::Key::ArrowDown, modifiers)]);
            worst = worst.max(started.elapsed());
        }
        assert_eq!(ui.app.spaces().unwrap().idea.selected(), Some("I0081"));
    }
    eprintln!("1,000-idea worst navigation frame: {worst:?}");
    let limit = if cfg!(debug_assertions) { 250 } else { 16 };
    assert!(
        worst < Duration::from_millis(limit),
        "1,000-idea navigation frame took {worst:?} (limit {limit}ms)"
    );
}
