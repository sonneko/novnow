//! 本文以外の文書の一部を編集するエディタ（M8 のワークスペース。`Workbench` の `Slice`、
//! `PLANS.md` §9.2 B-126）。
//!
//! 節エディタ（[`super::section_editor`]）と同じ `TextEdit` ＋ layouter で、色分けは文書全体に
//! 書き戻した形で解析して、開いている行の部分だけを取る（話プロットの `mode:` の E003 / E004 など、
//! ファイル全体の文脈が要る診断も波線になる）。undo の履歴は鍵（`Slice::tag`）ごとに残る。

use std::ops::Range;

use egui::{Id, Ui};

use crate::ui::theme::Mode;
use crate::ui::widgets::section_editor::{self, EditorOutput};
use crate::ui::widgets::syntax_text::Highlight;
use crate::ui::workbench::Workbench;

/// ハイライトのキャッシュ（中身が変わったときだけ作り直す）。
#[derive(Debug, Default)]
pub struct Cache {
    key: Option<(u64, String, usize)>,
    text: String,
    highlight: Highlight,
}

impl Cache {
    fn get(&mut self, bench: &Workbench, text: &str) -> Highlight {
        let Some(slice) = bench.slice() else {
            return Highlight::default();
        };
        let key = (bench.generation(), slice.tag.clone(), slice.lines.start);
        if self.key.as_ref() == Some(&key) && self.text == text {
            return self.highlight.clone();
        }
        self.highlight = highlight_of(bench, text);
        self.text = text.to_owned();
        self.key = Some(key);
        self.highlight.clone()
    }
}

/// エディタの `TextEdit` の Id（開いている Slice ごと）。
pub fn editor_id(bench: &Workbench) -> Id {
    match bench.slice() {
        Some(slice) => Id::new(("slice-editor", slice.tag.as_str())),
        None => Id::new("slice-editor-empty"),
    }
}

/// エディタを描く。Slice を開いていなければ何もしない。
pub fn show(
    ui: &mut Ui,
    bench: &mut Workbench,
    cache: &mut Cache,
    mode: Mode,
    scale: f32,
) -> EditorOutput {
    if bench.slice().is_none() {
        return EditorOutput::default();
    }
    let id = editor_id(bench);
    let scroll = format!(
        "slice-editor-scroll-{}",
        bench.slice().map_or("", |slice| slice.tag.as_str())
    );
    let mut buffer = std::mem::take(bench.slice_buffer_mut());
    let bench_ref: &Workbench = bench;
    let mut highlight = |text: &str| cache.get(bench_ref, text);
    let result = section_editor::area(ui, id, &scroll, &mut buffer, &mut highlight, mode, scale);
    *bench.slice_buffer_mut() = buffer;
    result
}

/// Slice の中身 `text` のハイライト（文書全体に書き戻した形で解析し、開いている行の部分だけを取る）。
pub fn highlight_of(bench: &Workbench, text: &str) -> Highlight {
    let Some(slice) = bench.slice() else {
        return Highlight::default();
    };
    let spliced = crate::core::features::sections::splice_lines(
        bench.project().text(&slice.path),
        &slice.lines,
        text,
    );
    let starts = crate::core::features::project::line_starts_of(&spliced);
    let start = starts
        .get(slice.lines.start)
        .copied()
        .unwrap_or(spliced.len());
    let range: Range<usize> = start..(start + text.len()).min(spliced.len());
    let whole = crate::core::model::document::Document::new(slice.path.clone(), spliced);
    Highlight::of_range(&whole, range, bench.project())
}
