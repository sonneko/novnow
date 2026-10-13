//! 節エディタ（§9.4.1。メインカラム下段）。
//!
//! - 編集対象は今の節の本文だけ（`Workbench::buffer`）。複数行の `egui::TextEdit` に、§11.2 の
//!   色分けをする layouter を渡して使う（§8.1。`PLANS.md` §9.2 B-111）。
//! - IME: `TextEdit` は編集可能でフォーカスがあるフレームで `PlatformOutput.ime`（`IMEOutput`）を
//!   出す（egui 0.27.2）。これが §9.8.5 (5) の「毎フレームの出力」になる。
//! - 行番号は**節内のローカル行番号**（1 始まり、§9.4.1）。折り返した行には番号を振らない。
//! - ソフトラップ有り・横スクロール無し。
//! - 節ごとに別の `Id` を使うので、undo の履歴は節ごとに残る（§8.6。戻ってくると復元される）。
//! - インレイヒント（§6.6.2）は [`crate::ui::widgets::inlay`] が隙間を空けて上から描く（B-137）。

use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange};
use egui::{FontFamily, FontId, Galley, Id, Pos2, Rect, TextEdit, Ui, Vec2};

use crate::core::features::project::line_starts_of;
use crate::core::features::sections;
use crate::core::model::document::Document;
use crate::ui::i18n::Locale;
use crate::ui::theme::{self, Mode};
use crate::ui::widgets::inlay;
use crate::ui::widgets::syntax_text::{self, Highlight, Metrics};
use crate::ui::workbench::Workbench;

/// 節エディタのハイライトのキャッシュ（中身が変わったときだけ作り直す）。
#[derive(Debug, Default)]
pub struct Cache {
    key: Option<(u64, String, usize, bool)>,
    text: String,
    highlight: Highlight,
}

impl Cache {
    fn get(&mut self, bench: &Workbench, text: &str, hints: bool) -> Highlight {
        let Some(current) = bench.current() else {
            return Highlight::default();
        };
        let key = (
            bench.generation(),
            current.path.clone(),
            current.lines.start,
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

pub(crate) fn char_of(text: &str, byte: usize) -> usize {
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

/// エディタの見た目（配色・文字の倍率・インレイヒント）。
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// 配色。
    pub mode: Mode,
    /// 文字の倍率（`ui.json.font_scale`）。
    pub scale: f32,
    /// インレイヒントを出すなら、その文言の言語（`None` = 出さない。`Alt+H`、§6.6.2）。
    pub inlay: Option<Locale>,
}

/// 節エディタを描く。
pub fn show(ui: &mut Ui, bench: &mut Workbench, cache: &mut Cache, look: Look) -> EditorOutput {
    let id = editor_id(bench);
    let mut buffer = std::mem::take(bench.buffer_mut());
    let bench_ref: &Workbench = bench;
    let hints = look.inlay.is_some();
    let mut highlight = |text: &str| cache.get(bench_ref, text, hints);
    let result = area(
        ui,
        id,
        "section-editor-scroll",
        &mut buffer,
        &mut highlight,
        look,
    );
    *bench.buffer_mut() = buffer;
    result
}

/// 色分け付きの複数行エディタ（節エディタと、M8 のワークスペースの [`crate::ui::widgets::slice_editor`]
/// が共有する）。行番号はエディタの中のローカル行番号（1 始まり）。
pub(crate) fn area(
    ui: &mut Ui,
    id: Id,
    scroll_id: &str,
    buffer: &mut String,
    highlight: &mut dyn FnMut(&str) -> Highlight,
    look: Look,
) -> EditorOutput {
    let Look { mode, scale, inlay } = look;
    let colors = theme::palette(mode);
    let metrics = Metrics {
        size: 15.0 * scale,
        line_height: 1.8,
    };
    let gutter = 36.0 * scale;
    let mut result = EditorOutput::default();
    egui::ScrollArea::vertical()
        .id_source(scroll_id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let (gutter_rect, _) = ui.allocate_exact_size(
                    Vec2::new(gutter, ui.available_height()),
                    egui::Sense::hover(),
                );
                let width = ui.available_width();
                let mut layouter = |ui: &Ui, text: &str, wrap: f32| -> Arc<Galley> {
                    let found = highlight(text);
                    let mut job = syntax_text::job(text, &found, mode, metrics, wrap);
                    if let Some(locale) = inlay {
                        // ヒントの幅の隙間を空ける（描くのは `TextEdit` の後）
                        let gaps: Vec<(usize, f32)> =
                            inlay::lay_out(ui, &found.hints, locale, mode, metrics.size)
                                .iter()
                                .map(|(at, label)| (*at, inlay::gap_of(label)))
                                .collect();
                        inlay::open_gaps(&mut job, &gaps);
                    }
                    ui.fonts(|fonts| fonts.layout_job(job))
                };
                let output = TextEdit::multiline(buffer)
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
                            byte_of(buffer, low.ccursor.index)..byte_of(buffer, high.ccursor.index),
                        );
                        let caret = output.galley.pos_from_cursor(&range.primary);
                        result.caret = Some(caret.translate(output.galley_pos.to_vec2()));
                    }
                }
                let painter = ui.painter().clone();
                let found = highlight(buffer);
                syntax_text::paint_squiggles(
                    &painter,
                    output.galley_pos,
                    &output.galley,
                    buffer,
                    &found,
                    mode,
                );
                if let Some(locale) = inlay {
                    let laid = inlay::lay_out(ui, &found.hints, locale, mode, metrics.size);
                    inlay::paint(
                        &painter,
                        output.galley_pos,
                        &output.galley,
                        buffer,
                        &laid,
                        mode,
                    );
                }
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
/// 役割語・モードの解決には場面の文脈が要るため）。`hints` ならインレイヒントも入れる。
pub fn highlight_of(bench: &Workbench, text: &str, hints: bool) -> Highlight {
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
    let mut out = Highlight::of_range(&whole, range.clone(), bench.project());
    if hints {
        out.hints = inlay::of_range(&whole, range, bench.project());
    }
    out
}
