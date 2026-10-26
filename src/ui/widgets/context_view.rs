//! 文脈ビュー（§9.3。メインカラム上段・読み取り専用）。
//!
//! - 今の節を中心に前後 `context_sections` 節（既定 3）。1 行の最大幅は全角 44 字。
//! - 節の境界には横線を引き、完全形の節番号を表示する。
//! - **今の節の位置には本文を一切描かず**、プレースホルダだけを置く（§9.3.2 MUST）。
//! - 今の節が変わったら、プレースホルダが画面の上から 38% に来るようにスクロールする。

use std::collections::HashMap;
use std::ops::Range;

use egui::{Align, Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::core::features::project::line_starts_of;
use crate::core::features::sections::SectionRef;
use crate::core::id::SceneId;
use crate::ui::i18n::{self, Locale};
use crate::ui::save_machine::Indicator;
use crate::ui::theme::{self, Mode};
use crate::ui::widgets::syntax_text::{self, Highlight, Metrics};
use crate::ui::workbench::Workbench;

/// 全角 1 字の幅（本文フォントの大きさとほぼ同じ）× 44 字（§9.3.1）。
pub const MAX_COLUMNS: f32 = 44.0;

/// ファイルごとのハイライト（文書が変わるたびに作り直す）。
#[derive(Debug, Default)]
pub struct Cache {
    generation: u64,
    files: HashMap<String, (Highlight, Vec<usize>)>,
    // Actual text heights for visible sections; off-screen sections use a cheap
    // character-count estimate until they first enter the scroll viewport.
    heights: HashMap<(String, usize, usize, u32, u32, u8), f32>,
    scrolled_to: Option<(String, usize)>,
}

impl Cache {
    fn file(&mut self, bench: &Workbench, path: &str) -> &(Highlight, Vec<usize>) {
        if self.generation != bench.generation() {
            self.files.clear();
            self.heights.clear();
            self.generation = bench.generation();
        }
        self.files
            .entry(path.to_owned())
            .or_insert_with(|| match bench.project().document(path) {
                Some(document) => (
                    Highlight::of_range(document, 0..document.text.len(), bench.project()),
                    line_starts_of(&document.text),
                ),
                None => (Highlight::default(), Vec::new()),
            })
    }
}

/// プレースホルダに出すもの（§9.3.2。これ以外を置かない）。
pub struct Placeholder<'a> {
    /// SceneId（`01-04-02-03`）。
    pub scene: &'a str,
    /// 現在の文字数（空白除外）。
    pub chars: usize,
    /// 目標字数（`length`）。
    pub target: Option<u32>,
    /// 保存状態のドット。
    pub indicator: Indicator,
    /// アクセシブル名（「編集中」）。
    pub accessible: &'a str,
}

/// 文脈ビューでのマウス操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 節の本文をクリック（その節へ移動）。並びの位置。
    Go(usize),
    /// 節の削除ボタン（§8.1.2。本文と対応プロットの一体削除は呼び出し側の単一経路）。
    Delete(SceneId),
}

/// 文脈ビューを描く。クリックされた節の並びの位置を返す。
#[allow(clippy::too_many_arguments)] // UI の描画設定と表示中の節をそれぞれ明示的に渡す
pub fn show(
    ui: &mut Ui,
    bench: &Workbench,
    cache: &mut Cache,
    mode: Mode,
    scale: f32,
    count: usize,
    max_columns: usize,
    locale: Locale,
    placeholder: &Placeholder<'_>,
) -> Option<Action> {
    let colors = theme::palette(mode);
    let metrics = Metrics {
        size: 15.0 * scale,
        line_height: 1.9,
    };
    let list = bench.sections();
    let split = bench.split_index();
    let editing = bench.current().cloned();
    let mut clicked: Option<Action> = None;
    let scroll_key = editing
        .as_ref()
        .map(|section| (section.path.clone(), section.lines.start));
    let need_scroll = cache.scrolled_to != scroll_key;
    egui::ScrollArea::vertical()
        .id_source("context-view")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = ui.available_width().min(metrics.size * max_columns as f32);
            let (before, after) = neighbours(list, split, editing.as_ref(), count);
            for index in before {
                match draw_section(ui, bench, cache, &list[index], mode, metrics, width, locale) {
                    Some(SectionHit::Go) => clicked = Some(Action::Go(index)),
                    Some(SectionHit::Delete(id)) => clicked = Some(Action::Delete(id)),
                    None => {}
                }
            }
            if editing.is_some() {
                let (rect, delete) = draw_placeholder(ui, placeholder, mode, width, scale, locale);
                if delete {
                    if let Some(section) = &editing {
                        clicked = Some(Action::Delete(section.id));
                    }
                }
                if need_scroll {
                    // 上から 38% に来るように（中央合わせの基準を 12% 上へずらす）
                    let height = ui.clip_rect().height();
                    let anchor = Rect::from_min_size(
                        Pos2::new(rect.min.x, rect.min.y + height * 0.12),
                        Vec2::new(1.0, 1.0),
                    );
                    ui.scroll_to_rect(anchor, Some(Align::Center));
                }
            } else {
                ui.label(egui::RichText::new("—").color(colors.fg_faint));
            }
            for index in after {
                match draw_section(ui, bench, cache, &list[index], mode, metrics, width, locale) {
                    Some(SectionHit::Go) => clicked = Some(Action::Go(index)),
                    Some(SectionHit::Delete(id)) => clicked = Some(Action::Delete(id)),
                    None => {}
                }
            }
        });
    if need_scroll {
        cache.scrolled_to = scroll_key;
    }
    clicked
}

/// 前後の節（編集中の行と重なる節は出さない。節エディタに `;` を書いた直後の分割を含む）。
/// `split` は並びの中で編集中の節より前にある節の数。
fn neighbours(
    list: &[SectionRef],
    split: usize,
    editing: Option<&SectionRef>,
    count: usize,
) -> (Vec<usize>, Vec<usize>) {
    if editing.is_none() {
        return (Vec::new(), Vec::new());
    }
    let overlaps = |item: &SectionRef| {
        editing.is_some_and(|editing| {
            editing.path == item.path
                && item.lines.start < editing.lines.end.max(editing.lines.start + 1)
                && editing.lines.start < item.lines.end.max(item.lines.start + 1)
        })
    };
    let before: Vec<usize> = (0..split.min(list.len()))
        .rev()
        .filter(|&index| !overlaps(&list[index]))
        .take(count)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let after: Vec<usize> = (split.min(list.len())..list.len())
        .filter(|&index| !overlaps(&list[index]))
        .take(count)
        .collect();
    (before, after)
}

/// [`draw_section`] でのクリック。
enum SectionHit {
    /// 本文（その節へ移動）。
    Go,
    /// 削除ボタン。
    Delete(SceneId),
}

/// 節 1 つ（番号 + 本文）。クリックされたら `Some`。
#[allow(clippy::too_many_arguments)]
fn draw_section(
    ui: &mut Ui,
    bench: &Workbench,
    cache: &mut Cache,
    section: &SectionRef,
    mode: Mode,
    metrics: Metrics,
    width: f32,
    locale: Locale,
) -> Option<SectionHit> {
    let colors = theme::palette(mode);
    let full = section.id.display(bench.project().digits());
    let levels: Vec<&str> = full.split('-').collect();
    let mut folded = ui
        .ctx()
        .data(|d| d.get_temp::<Vec<String>>(egui::Id::new("context-folded")))
        .unwrap_or_default();
    let current = bench
        .current_id()
        .map(|id| id.display(bench.project().digits()))
        .unwrap_or_default();
    let mut hidden = false;
    ui.horizontal(|ui| {
        for depth in 1..=3 {
            let key = format!(
                "@{}",
                levels
                    .iter()
                    .take(depth)
                    .copied()
                    .collect::<Vec<_>>()
                    .join("-")
            );
            let prefix = key.trim_start_matches('@');
            let active = current == prefix || current.starts_with(&format!("{prefix}-"));
            let collapsed = folded.contains(&key) && !active;
            if collapsed {
                hidden = true;
            }
            let response = ui.add_enabled(
                !active,
                egui::Button::new(format!("{} {}", if collapsed { "▸" } else { "▾" }, prefix))
                    .small(),
            );
            if response.hovered() && !active {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::new("context-fold-hover"), key.clone()));
            }
            if response.clicked() {
                if collapsed {
                    folded.retain(|k| k != &key);
                } else {
                    folded.push(key);
                }
                ui.ctx().data_mut(|d| {
                    d.insert_temp(egui::Id::new("context-folded"), folded.clone());
                    d.insert_temp(egui::Id::new("context-folded-changed"), folded.clone());
                });
            }
        }
    });
    if hidden {
        return None;
    }
    let document = bench.project().document(&section.path)?;
    let range = {
        let (_, starts) = cache.file(bench, &section.path);
        byte_range(starts, document.text.len(), &section.lines)
    };
    let text = &document.text[range.clone()];
    let height_key = (
        section.path.clone(),
        section.lines.start,
        section.lines.end,
        width.to_bits(),
        metrics.size.to_bits(),
        mode as u8,
    );
    let text_height = cache.heights.get(&height_key).copied().unwrap_or_else(|| {
        let columns = (width / metrics.size).floor().max(1.0) as usize;
        let lines: usize = text
            .split('\n')
            .map(|line| line.chars().count().div_ceil(columns).max(1))
            .sum();
        lines as f32 * metrics.size * metrics.line_height
    });
    // Keep an estimated slot in the scroll layout, without shaping or painting
    // glyphs in sections outside the visible viewport (§9.3.4 / S-12).
    let block_height = 29.0 + text_height.max(metrics.size) + 4.0 * ui.spacing().item_spacing.y;
    let probe = Rect::from_min_size(ui.cursor().min, Vec2::new(width, block_height));
    if !ui.is_rect_visible(probe) {
        ui.allocate_exact_size(Vec2::new(width, block_height), Sense::hover());
        return None;
    }
    // Extract token/squiggle spans only for sections entering the viewport.
    let (file_highlight, _) = cache.file(bench, &section.path);
    let highlight = slice(file_highlight, &range);
    ui.add_space(8.0);
    let (rule, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rule.left_top(), rule.right_top()],
        Stroke::new(1.0_f32, colors.border),
    );
    ui.add_space(8.0);
    // 節番号（右寄せ・10px・fg.faint）と削除ボタン（§8.1.2）
    let digits = bench.project().digits();
    let label = format!("@{}", section.id.display(digits));
    let mut delete = false;
    ui.allocate_ui_with_layout(
        Vec2::new(width, 12.0),
        egui::Layout::right_to_left(Align::Center),
        |ui| {
            ui.label(
                egui::RichText::new(label)
                    .font(FontId::new(10.0, FontFamily::Monospace))
                    .color(colors.fg_faint),
            );
            // 右から左のレイアウトでは `Label` の割当矩形と描画位置がずれるため、
            // ボタン系の共通部品で描く（当たり判定と見た目が一致する）
            let button = crate::ui::widgets::selectable_action(
                ui,
                false,
                egui::RichText::new(i18n::text(locale, "delete.button"))
                    .font(FontId::new(10.0, FontFamily::Proportional))
                    .color(colors.fg_faint),
            );
            delete = button.clicked();
        },
    );
    if delete {
        return Some(SectionHit::Delete(section.id));
    }
    let job = syntax_text::job(text, &highlight, mode, metrics, width);
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    cache
        .heights
        .insert(height_key, galley.size().y.max(metrics.size));
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(width.max(24.0), galley.size().y.max(metrics.size).max(24.0)),
        Sense::click(),
    );
    let painter = ui.painter_at(rect.expand(4.0));
    if response.hovered() {
        painter.rect_filled(rect, 2.0, colors.bg_selection.gamma_multiply(0.6));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.galley(rect.min, galley.clone(), colors.fg_primary);
    painter.line_segment(
        [
            rect.left_bottom(),
            egui::pos2((rect.min.x + galley.size().x).min(rect.max.x), rect.max.y),
        ],
        egui::Stroke::new(0.6_f32, colors.border),
    );
    syntax_text::paint_squiggles(&painter, rect.min, &galley, text, &highlight, mode);
    response.clicked().then_some(SectionHit::Go)
}

/// プレースホルダ（§9.3.2。高さ 56px・背景 `bg.inset`・左端に 3px の `accent` 縦線）。
fn draw_placeholder(
    ui: &mut Ui,
    placeholder: &Placeholder<'_>,
    mode: Mode,
    width: f32,
    scale: f32,
    locale: Locale,
) -> (Rect, bool) {
    let colors = theme::palette(mode);
    ui.add_space(8.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 56.0), Sense::hover());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, placeholder.accessible));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, colors.bg_inset);
    painter.rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
        0.0,
        colors.accent,
    );
    let center_y = rect.center().y;
    // 編集中アイコン（✎ 相当のベクタ図形: 斜めの鉛筆）
    let icon = Pos2::new(rect.min.x + 24.0, center_y);
    pencil(&painter, icon, colors.fg_muted);
    let font = FontId::new(13.0 * scale, FontFamily::Monospace);
    painter.text(
        Pos2::new(rect.min.x + 48.0, center_y),
        egui::Align2::LEFT_CENTER,
        placeholder.scene,
        font.clone(),
        colors.fg_muted,
    );
    let count = match placeholder.target {
        Some(target) => format!("{} / {}", group(placeholder.chars), group(target as usize)),
        None => group(placeholder.chars),
    };
    let dot_x = rect.max.x - 24.0;
    painter.text(
        Pos2::new(dot_x - 20.0, center_y),
        egui::Align2::RIGHT_CENTER,
        count.clone(),
        font.clone(),
        colors.fg_muted,
    );
    if placeholder.chars == 0 {
        ui.add_space(6.0);
        for key in [
            "context.guide_workspace",
            "context.guide_section",
            "context.guide_palette",
        ] {
            ui.label(egui::RichText::new(i18n::text(locale, key)).color(colors.fg_faint));
        }
    }
    match placeholder.indicator {
        Indicator::Unsaved => {
            painter.circle_filled(Pos2::new(dot_x, center_y), 5.0, colors.unsaved);
        }
        Indicator::Failed => {
            painter.circle_filled(Pos2::new(dot_x, center_y), 4.0, colors.error);
        }
        Indicator::Spinner | Indicator::Nothing => {}
    }
    // 編集中の節の削除ボタン（§8.1.2。文字数の左に置く）
    let delete_font = FontId::new(11.0 * scale, FontFamily::Proportional);
    let delete_text = i18n::text(locale, "delete.button");
    let (delete_width, count_width) = ui.fonts(|fonts| {
        (
            fonts
                .layout_no_wrap(delete_text.to_owned(), delete_font.clone(), colors.fg_faint)
                .size()
                .x,
            fonts
                .layout_no_wrap(count.clone(), font.clone(), colors.fg_muted)
                .size()
                .x,
        )
    });
    let button_min = Pos2::new(
        dot_x - 20.0 - count_width - 14.0 - delete_width,
        center_y - 12.0,
    );
    let button_rect = Rect::from_min_size(button_min, Vec2::new(delete_width.max(24.0), 24.0));
    let button = ui.interact(
        button_rect,
        egui::Id::new("context-delete-current"),
        Sense::click(),
    );
    if button.hovered() {
        painter.rect_filled(button_rect, 2.0, colors.bg_selection.gamma_multiply(0.6));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.text(
        Pos2::new(button_min.x, center_y),
        egui::Align2::LEFT_CENTER,
        delete_text,
        delete_font,
        colors.fg_faint,
    );
    painter.line_segment(
        [
            Pos2::new(button_min.x, center_y + 7.0 * scale),
            Pos2::new(button_min.x + delete_width, center_y + 7.0 * scale),
        ],
        Stroke::new(0.6_f32, colors.border),
    );
    (rect, button.clicked())
}

fn pencil(painter: &egui::Painter, at: Pos2, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    let tip = at + Vec2::new(-6.0, 6.0);
    let tail = at + Vec2::new(6.0, -6.0);
    painter.line_segment([tip, tail], stroke);
    painter.line_segment(
        [tail + Vec2::new(-2.0, -2.0), tail + Vec2::new(2.0, 2.0)],
        stroke,
    );
    painter.line_segment([tip, tip + Vec2::new(3.0, 0.0)], stroke);
}

/// 3 桁ごとのカンマ（`5,842`）。
pub fn group(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, c) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 行の範囲 → byte の範囲（最後の行の改行は含めない）。
pub fn byte_range(starts: &[usize], len: usize, lines: &Range<usize>) -> Range<usize> {
    let start = starts.get(lines.start).copied().unwrap_or(len);
    if lines.is_empty() {
        return start..start;
    }
    let end = starts
        .get(lines.end)
        .map(|next| next.saturating_sub(1))
        .unwrap_or(len);
    start..end.max(start)
}

/// ファイル単位のハイライトから、`range` の部分を `range.start` 起点で取り出す。
pub fn slice(highlight: &Highlight, range: &Range<usize>) -> Highlight {
    let cut = |inner: &Range<usize>| {
        let start = inner.start.max(range.start);
        let end = inner.end.min(range.end);
        (start < end).then(|| start - range.start..end - range.start)
    };
    Highlight {
        tokens: highlight
            .tokens
            .iter()
            .filter_map(|(at, kind, role)| cut(at).map(|clipped| (clipped, *kind, *role)))
            .collect(),
        squiggles: highlight
            .squiggles
            .iter()
            .filter_map(|(at, severity)| cut(at).map(|clipped| (clipped, *severity)))
            .collect(),
        hints: Vec::new(),
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;
    use crate::core::features::Project;
    use crate::core::model::document::Document;
    use crate::core::model::project::Project as Settings;

    #[test]
    fn only_visible_sections_are_shaped_in_a_small_scroll_viewport() {
        let mut prose = String::from("@01-01-01 First scene\nfirst\n");
        for _ in 0..12 {
            prose.push_str(";\n");
            prose.push_str(&"文".repeat(400));
            prose.push('\n');
        }
        let project = Project::with_documents(
            Settings::default(),
            [Document::new("contents/01-01.txt", prose)],
        );
        let bench = Workbench::new(project, None, 400);
        assert!(bench.sections().len() > 9);
        let mut cache = Cache::default();
        let ctx = egui::Context::default();
        let placeholder = Placeholder {
            scene: "01-01-01-01",
            chars: 5,
            target: None,
            indicator: Indicator::Nothing,
            accessible: "編集中",
        };
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 180.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(
                        ui,
                        &bench,
                        &mut cache,
                        Mode::Dark,
                        1.0,
                        9,
                        44,
                        Locale::Ja,
                        &placeholder,
                    );
                });
            },
        );
        assert!(
            cache.heights.len() < 9,
            "invisible sections should reserve height without producing galleys"
        );
    }
}
