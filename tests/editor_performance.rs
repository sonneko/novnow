#![cfg(feature = "ui")]

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
