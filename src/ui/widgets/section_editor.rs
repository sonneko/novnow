//! 節エディタ（§9.4.1。メインカラム下段）。
//!
//! - 編集対象は今の節の本文だけ（`Workbench::buffer`）。複数行の `egui::TextEdit` に、§11.2 の
//!   色分けをする layouter を渡して使う（§8.1。`PLANS.md` §9.2 B-111）。
//! - IME: `TextEdit` は編集可能でフォーカスがあるフレームで `PlatformOutput.ime`（`IMEOutput`）を
//!   出す（egui 0.27.2）。これが §9.8.5 (5) の「毎フレームの出力」になる。
//! - 行番号は**節内のローカル行番号**（1 始まり、§9.4.1）。折り返した行には番号を振らない。
//! - ソフトラップ有り・横スクロール無し。
//! - 節ごとに別の `Id` を使うので、undo の履歴は節ごとに残る（§8.6。戻ってくると復元される）。

use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange};
use egui::{FontFamily, FontId, Galley, Id, Pos2, Rect, TextEdit, Ui, Vec2};

use crate::core::features::project::line_starts_of;
use crate::core::features::sections;
use crate::core::model::document::Document;
use crate::ui::theme::{self, Mode};
use crate::ui::widgets::syntax_text::{self, Highlight, Metrics};
use crate::ui::workbench::Workbench;

/// 節エディタのハイライトのキャッシュ（中身が変わったときだけ作り直す）。
#[derive(Debug, Default)]
pub struct Cache {
    key: Option<(u64, String, usize)>,
    text: String,
    highlight: Highlight,
}

impl Cache {
    fn get(&mut self, bench: &Workbench, text: &str) -> Highlight {
        let Some(current) = bench.current() else {
            return Highlight::default();
        };
        let key = (
            bench.generation(),
            current.path.clone(),
            current.lines.start,
        );
        if self.key.as_ref() == Some(&key) && self.text == text {
            return self.highlight.clone();
        }
        self.highlight = highlight_of(bench, text);
        self.text = text.to_owned();
        self.key = Some(key);
        self.highlight.clone()
    }
}

/// 節エディタの `TextEdit` の Id（節ごと）。
pub fn editor_id(bench: &Workbench) -> Id {
    match bench.current() {
        Some(current) => Id::new(("section-editor", current.path.as_str(), current.id)),
        None => Id::new("section-editor-empty"),
    }
}

/// 節エディタの 1 フレームの結果。
#[derive(Debug, Clone, Default)]
pub struct EditorOutput {
    /// 中身が変わった。
    pub changed: bool,
    /// 選択範囲（節の本文の中の byte。キャレットだけなら空範囲）。フォーカスが無ければ `None`。
    pub selection: Option<Range<usize>>,
    /// キャレットの矩形（画面の座標。補完のポップアップの位置）。
    pub caret: Option<Rect>,
}

/// 節エディタの選択範囲を byte で指定する（次のフレームから効く。検索の一致・補完の確定の後）。
pub fn select(ui_ctx: &egui::Context, bench: &Workbench, range: Range<usize>) {
    let id = editor_id(bench);
    let text = bench.buffer();
    let start = char_of(text, range.start);
    let end = char_of(text, range.end);
    let mut state = egui::text_edit::TextEditState::load(ui_ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::two(
        CCursor::new(start),
        CCursor::new(end),
    )));
    state.store(ui_ctx, id);
    ui_ctx.memory_mut(|memory| memory.request_focus(id));
}

fn char_of(text: &str, byte: usize) -> usize {
    let mut at = byte.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    text[..at].chars().count()
}

fn byte_of(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(at, _)| at)
}

/// 節エディタを描く。
pub fn show(
    ui: &mut Ui,
    bench: &mut Workbench,
    cache: &mut Cache,
    mode: Mode,
    scale: f32,
) -> EditorOutput {
    let colors = theme::palette(mode);
    let metrics = Metrics {
        size: 15.0 * scale,
        line_height: 1.8,
    };
    let id = editor_id(bench);
    let gutter = 36.0 * scale;
    let mut buffer = std::mem::take(bench.buffer_mut());
    let mut result = EditorOutput::default();
    egui::ScrollArea::vertical()
        .id_source("section-editor-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let (gutter_rect, _) = ui.allocate_exact_size(
                    Vec2::new(gutter, ui.available_height()),
                    egui::Sense::hover(),
                );
                let width = ui.available_width();
                let bench_ref: &Workbench = bench;
                let mut layouter = |ui: &Ui, text: &str, wrap: f32| -> Arc<Galley> {
                    let highlight = cache.get(bench_ref, text);
                    let job = syntax_text::job(text, &highlight, mode, metrics, wrap);
                    ui.fonts(|fonts| fonts.layout_job(job))
                };
                let output = TextEdit::multiline(&mut buffer)
                    .id(id)
                    .frame(false)
                    .desired_width(width)
                    .desired_rows(8)
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui);
                result.changed = output.response.changed();
                if output.response.has_focus() {
                    if let Some(range) = output.cursor_range {
                        let [low, high] = range.sorted_cursors();
                        result.selection = Some(
                            byte_of(&buffer, low.ccursor.index)
                                ..byte_of(&buffer, high.ccursor.index),
                        );
                        let caret = output.galley.pos_from_cursor(&range.primary);
                        result.caret = Some(caret.translate(output.galley_pos.to_vec2()));
                    }
                }
                let painter = ui.painter().clone();
                let highlight = cache.highlight.clone();
                syntax_text::paint_squiggles(
                    &painter,
                    output.galley_pos,
                    &output.galley,
                    &buffer,
                    &highlight,
                    mode,
                );
                paint_line_numbers(
                    &painter,
                    gutter_rect.min.x + gutter - 8.0,
                    output.galley_pos,
                    &output.galley,
                    FontId::new(11.0 * scale, FontFamily::Monospace),
                    colors.fg_faint,
                );
            });
        });
    *bench.buffer_mut() = buffer;
    result
}

/// 論理行の 1 行目にだけ番号を振る（右寄せ）。
fn paint_line_numbers(
    painter: &egui::Painter,
    right: f32,
    origin: Pos2,
    galley: &Galley,
    font: FontId,
    color: egui::Color32,
) {
    let mut number = 1;
    let mut starts_line = true;
    for row in &galley.rows {
        if starts_line {
            painter.text(
                Pos2::new(right, origin.y + row.rect.center().y),
                egui::Align2::RIGHT_CENTER,
                number.to_string(),
                font.clone(),
                color,
            );
            number += 1;
        }
        starts_line = row.ends_with_newline;
    }
}

/// 今の節の本文 `text` のハイライト（ファイル全体に書き戻した形で解析し、節の部分だけを取る。
/// 役割語・モードの解決には場面の文脈が要るため）。
pub fn highlight_of(bench: &Workbench, text: &str) -> Highlight {
    let Some(current) = bench.current() else {
        return Highlight::default();
    };
    let Some(document) = bench.project().document(&current.path) else {
        return Highlight::default();
    };
    let spliced = sections::splice(&document.text, current, text);
    let starts = line_starts_of(&spliced);
    let start = starts
        .get(current.lines.start)
        .copied()
        .unwrap_or(spliced.len());
    let range: Range<usize> = start..(start + text.len()).min(spliced.len());
    let whole = Document::new(current.path.clone(), spliced);
    Highlight::of_range(&whole, range, bench.project())
}
