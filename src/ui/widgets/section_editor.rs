//! 節エディタ（§9.4.1。メインカラム下段）。
//!
//! - 編集対象は今の節の本文だけ（`Workbench::buffer`）。複数行の `egui::TextEdit` に、§11.2 の
//!   色分けをする layouter を渡して使う（§8.1。`PLANS.md` §9.2 B-111）。
//! - IME: `TextEdit` は編集可能でフォーカスがあるフレームで `PlatformOutput.ime`（`IMEOutput`）を
//!   出す（egui 0.27.2）。これが §9.8.5 (5) の「毎フレームの出力」になる。
//! - 行番号は**節内のローカル行番号**（1 始まり、§9.4.1）。折り返した行には番号を振らない。
//! - ソフトラップ有り・横スクロール無し。
//! - 節ごとに別の `Id` を使うので、undo の履歴は節ごとに残る（§8.6。戻ってくると復元される）。
//! - 貼り付け（`Event::Paste`）は改行をそのまま入れる。単独 `;` 行・場面見出し行（`@A-B-C`）が
//!   含まれていれば [`apply_paste`] が節に分けて文書へ一括で反映する（§8.1.3）。
//! - インレイヒント（§6.6.2）は [`crate::ui::widgets::inlay`] が隙間を空けて上から描く（B-137）。

use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange};
use egui::{Event, FontFamily, FontId, Galley, Id, Pos2, Rect, TextEdit, Ui, Vec2};

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

/// 貼り付けを処理する（§8.1.3。`TextEdit` を描く**前**に呼ぶ）。
///
/// - 改行は内部表現の LF に直す（§5.0.4 MUST。貼り付けはファイル読み込みを通らない）。
/// - 選択（無ければキャレット）に貼って節区切り行・場面見出し行が含まれるときだけ、こちらで節に分けて
///   文書へ入れる。そのときはイベントを消費して `TextEdit` に二重に入れさせない。
/// - 分ける必要が無ければ、直した LF の `Event::Paste` をそのまま残す（[`egui::TextEdit`] が入れる）。
pub(crate) fn apply_paste(ui: &Ui, bench: &mut Workbench, id: Id) {
    if !ui.memory(|memory| memory.has_focus(id)) {
        return;
    }
    let now = ui.input(|input| input.time);
    // 選択（無ければキャレット。`TextEdit` はカーソルをまだ持たない間、末尾に入れる。egui の
    // `cursor_at_end` の既定と同じ）
    let selection = egui::text_edit::TextEditState::load(ui.ctx(), id)
        .and_then(|state| state.cursor.char_range())
        .map(|range| {
            let (low, high) = if range.primary.index <= range.secondary.index {
                (range.primary.index, range.secondary.index)
            } else {
                (range.secondary.index, range.primary.index)
            };
            let text = bench.buffer();
            byte_of(text, low)..byte_of(text, high)
        })
        .unwrap_or_else(|| {
            let end = bench.buffer().len();
            end..end
        });
    let mut done = false;
    ui.input_mut(|input| {
        let mut kept = Vec::with_capacity(input.events.len());
        for event in input.events.drain(..) {
            match event {
                // CRLF・CR は LF に直す（§5.0.4 MUST）
                Event::Paste(text) => {
                    let text = text.replace("\r\n", "\n").replace('\r', "\n");
                    if !done && bench.paste(selection.clone(), &text, now) {
                        done = true;
                        continue;
                    }
                    kept.push(Event::Paste(text));
                }
                other => kept.push(other),
            }
        }
        input.events = kept;
    });
    if done {
        // キャレットは先頭の節（エディタに残る本文）の末尾へ
        let end = bench.buffer().len();
        select(ui.ctx(), bench, end..end);
    }
}

/// エディタの見た目（配色・文字の倍率・インレイヒント）。
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// 配色。
    pub mode: Mode,
    /// 文字の倍率（`ui.json.font_scale`）。
    pub scale: f32,
    /// 本文の折り返し幅（全角相当）。
    pub max_line_chars: usize,
    /// インレイヒントを出すなら、その文言の言語（`None` = 出さない。`Alt+H`、§6.6.2）。
    pub inlay: Option<Locale>,
}

/// 節エディタを描く。
pub fn show(ui: &mut Ui, bench: &mut Workbench, cache: &mut Cache, look: Look) -> EditorOutput {
    let ratio = crate::core::stage_two::metrics::section(bench.buffer()).ratio();
    ui.add(
        egui::ProgressBar::new(ratio as f32)
            .show_percentage()
            .desired_width(ui.available_width())
            .fill(theme::palette(look.mode).fg_muted),
    )
    .on_hover_text(crate::ui::i18n::text(
        look.inlay.unwrap_or(Locale::Ja),
        "stage.dialogue_ratio",
    ));
    if bench.human_active() {
        let rect = ui.max_rect();
        ui.painter().line_segment(
            [rect.left_top(), rect.left_bottom()],
            egui::Stroke::new(3.0_f32, theme::palette(look.mode).warn),
        );
    }
    let id = editor_id(bench);
    apply_paste(ui, bench, id);
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
    let Look {
        mode,
        scale,
        inlay,
        max_line_chars,
    } = look;
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
                let width = ui
                    .available_width()
                    .min(metrics.size * max_line_chars as f32);
                let mut layouter = |ui: &Ui, text: &str, wrap: f32| -> Arc<Galley> {
                    let found = highlight(text);
                    let mut job = syntax_text::job(text, &found, mode, metrics, wrap.min(width));
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
                // egui's built-in cursor spans the full 1.8× line-height.
                // Keep its layout/IME geometry, but paint a font-sized caret.
                let cursor_stroke = ui.visuals().text_cursor;
                ui.visuals_mut().text_cursor.width = 0.0;
                let output = TextEdit::multiline(buffer)
                    .id(id)
                    .frame(false)
                    .font(FontId::new(metrics.size, FontFamily::Proportional))
                    .desired_width(width)
                    .desired_rows(8)
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui);
                ui.visuals_mut().text_cursor = cursor_stroke;
                if output.response.has_focus() {
                    if let Some(range) = &output.cursor_range {
                        let cursor = output.galley.pos_from_cursor(&range.primary);
                        let center = cursor.center().y + output.galley_pos.y;
                        let x = cursor.min.x + output.galley_pos.x;
                        let half = metrics.size * 0.57;
                        ui.painter()
                            .with_clip_rect(output.response.rect.intersect(ui.clip_rect()))
                            .line_segment(
                                [Pos2::new(x, center - half), Pos2::new(x, center + half)],
                                egui::Stroke::new(cursor_stroke.width, cursor_stroke.color),
                            );
                    }
                }
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
                // Clip decorations to the TextEdit and the scroll viewport,
                // independently of the gutter and the surrounding editor frame.
                let text_clip = output.response.rect.intersect(ui.clip_rect());
                let painter = ui.painter().with_clip_rect(text_clip);
                if ui.ctx().data(|d| {
                    d.get_temp::<bool>(Id::new("suggest-comma"))
                        .unwrap_or(false)
                }) {
                    for at in crate::core::stage_two::metrics::comma_candidates(buffer, 40) {
                        let cursor = output
                            .galley
                            .from_ccursor(CCursor::new(buffer[..at].chars().count()));
                        let rect = output
                            .galley
                            .pos_from_cursor(&cursor)
                            .translate(output.galley_pos.to_vec2());
                        painter.line_segment(
                            [rect.left_top(), rect.left_bottom()],
                            egui::Stroke::new(1.0_f32, colors.fg_faint),
                        );
                    }
                }
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
                let gutter_painter = ui
                    .painter()
                    .with_clip_rect(gutter_rect.intersect(ui.clip_rect()));
                paint_line_numbers(
                    &gutter_painter,
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

/// Only the active section is analyzed synchronously while typing. Other sections
/// retain their cached rendering until the workbench commits after the debounce.
pub fn highlight_of(bench: &Workbench, text: &str, hints: bool) -> Highlight {
    let Some(current) = bench.current() else {
        return Highlight::default();
    };
    // A standalone scene with the same structural id provides the resolver with the
    // correct plot context without rescanning a 100,000-character neighboring section.
    // Empty preceding sections preserve the section number for mode/range resolution.
    let displayed = current.id.display(bench.project().digits());
    let scene = displayed
        .rsplit_once('-')
        .map_or(displayed.as_str(), |(scene, _)| scene);
    let mut body = format!("@{scene}\n");
    for _ in 1..current.id.section {
        body.push_str(";\n");
    }
    let start = body.len();
    body.push_str(text);
    let range = start..body.len();
    let scoped = Document::new(current.path.clone(), body);
    let mut out = Highlight::of_range(&scoped, range.clone(), bench.project());
    if hints {
        out.hints = inlay::of_range(&scoped, range, bench.project());
    }
    out
}
