//! 本文以外の文書の一部を編集するエディタ（M8 のワークスペース。`Workbench` の `Slice`、
//! `docs/history.md` B-126）。
//!
//! 節エディタ（[`super::section_editor`]）と同じ `TextEdit` ＋ layouter で、色分けは文書全体に
//! 書き戻した形で解析して、開いている行の部分だけを取る（話プロットの `mode:` の E003 / E004 など、
//! ファイル全体の文脈が要る診断も波線になる）。undo の履歴は鍵（`Slice::tag`）ごとに残る。

use std::ops::Range;

use egui::{Id, Ui};

use crate::ui::widgets::inlay;
use crate::ui::widgets::section_editor::{self, EditorOutput, Look};
use crate::ui::widgets::syntax_text::Highlight;
use crate::ui::workbench::Workbench;

/// ハイライトのキャッシュ（中身が変わったときだけ作り直す）。
#[derive(Debug, Default)]
pub struct Cache {
    key: Option<(u64, String, usize, bool)>,
    text: String,
    highlight: Highlight,
}

impl Cache {
    fn get(&mut self, bench: &Workbench, text: &str, hints: bool) -> Highlight {
        let Some(slice) = bench.slice() else {
            return Highlight::default();
        };
        let key = (
            bench.generation(),
            slice.tag.clone(),
            slice.lines.start,
            hints,
        );
        if self.key.as_ref() == Some(&key) && self.text == text {
            return self.highlight.clone();
        }
        self.highlight = highlight_of(bench, text, hints);
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
pub fn show(ui: &mut Ui, bench: &mut Workbench, cache: &mut Cache, look: Look) -> EditorOutput {
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
    let hints = look.inlay.is_some();
    let mut highlight = |text: &str| cache.get(bench_ref, text, hints);
    let result = section_editor::area(ui, id, &scroll, &mut buffer, &mut highlight, look);
    *bench.slice_buffer_mut() = buffer;
    if let Some(selection) = result.selection.clone() {
        bench.set_slice_caret(selection);
    }
    if let Some(rect) = result.caret {
        ui.ctx()
            .data_mut(|data| data.insert_temp(Id::new(CARET_RECT), rect));
    }
    result
}

/// [`caret_rect`] の置き場所（egui の一時データの鍵）。
const CARET_RECT: &str = "slice-editor-caret-rect";

/// エディタのキャレットの最後の矩形（画面の座標。`Ctrl+.` のポップアップの位置、B-137）。
pub fn caret_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    ctx.data(|data| data.get_temp::<egui::Rect>(Id::new(CARET_RECT)))
}

/// エディタの選択範囲を byte で指定してフォーカスを移す（次のフレームから効く。`Ctrl+M` の挿入の後）。
pub fn select(ctx: &egui::Context, bench: &Workbench, range: Range<usize>) {
    let id = editor_id(bench);
    let text = bench.slice_buffer();
    let start = section_editor::char_of(text, range.start);
    let end = section_editor::char_of(text, range.end);
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    state.store(ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
}

/// Slice の中身 `text` のハイライト（文書全体に書き戻した形で解析し、開いている行の部分だけを取る）。
/// `hints` ならインレイヒント（§6.6.2）も入れる。
pub fn highlight_of(bench: &Workbench, text: &str, hints: bool) -> Highlight {
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
    let mut out = Highlight::of_range(&whole, range.clone(), bench.project());
    if hints {
        out.hints = inlay::of_range(&whole, range, bench.project());
    }
    out
}
