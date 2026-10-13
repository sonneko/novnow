//! 構文ハイライト付きのテキスト（§11.2）と下線（§11.3）。
//!
//! - 色はトークン（`core::features::semantic_tokens`）→ [`theme::token_style`]。
//! - 役割語の一致は **緑の直線**（`state.ok`、1.5px、§6.5.4・§11.3）。`TextFormat::underline` で描く。
//! - 診断は **波線**（エラー `state.error` / 警告 `state.warn`、周期 4px・振幅 1.5px）。egui の
//!   `TextFormat` に波線は無いので、レイアウト後の `Galley` の行から位置を求めて上から描く
//!   （役割語の緑より後に描く = エラーが上、§11.3）。

use std::ops::Range;
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, Galley, Painter, Pos2, Stroke, Vec2};

use crate::core::diagnostics::{self, Severity};
use crate::core::features::semantic_tokens::{self, TokenType};
use crate::core::features::Project;
use crate::core::model::document::Document;
use crate::ui::theme::{self, Mode};

/// テキスト 1 つ分のハイライト（範囲は**そのテキストの中の byte**）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Highlight {
    /// トークン（範囲・種別・役割語の一致か）。
    pub tokens: Vec<(Range<usize>, TokenType, bool)>,
    /// 診断の波線（範囲・重さ）。I は出さない。
    pub squiggles: Vec<(Range<usize>, Severity)>,
}

impl Highlight {
    /// 文書 `document` を解析し、その中の `range`（byte）の部分だけを `range.start` 起点に直して返す。
    pub fn of_range(document: &Document, range: Range<usize>, project: &Project) -> Self {
        let tokens = semantic_tokens::full(document, project)
            .into_iter()
            .filter_map(|token| {
                clip(&token.range, &range).map(|clipped| (clipped, token.kind, token.role_language))
            })
            .collect();
        let squiggles = diagnostics::document(document, project)
            .into_iter()
            .filter(|found| matches!(found.severity(), Severity::Error | Severity::Warning))
            .filter_map(|found| {
                let at = found.range.clone()?;
                // 長さ 0 の診断（位置だけ）は 1 文字ぶんに広げる
                let at = if at.is_empty() {
                    at.start..at.start + 1
                } else {
                    at
                };
                clip(&at, &range).map(|clipped| (clipped, found.severity()))
            })
            .collect();
        Self { tokens, squiggles }
    }
}

/// `inner` を `outer` に切り詰め、`outer.start` 起点に直す。重ならなければ `None`。
fn clip(inner: &Range<usize>, outer: &Range<usize>) -> Option<Range<usize>> {
    let start = inner.start.max(outer.start);
    let end = inner.end.min(outer.end);
    (start < end).then(|| start - outer.start..end - outer.start)
}

/// 描き方の大きさ（§11.5）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// 文字の大きさ（px）。
    pub size: f32,
    /// 行間（倍率）。
    pub line_height: f32,
}

/// ハイライトを掛けた `LayoutJob`。`text` は `highlight` を作ったときと同じ文字列であること
/// （違えば範囲を文字の境界に丸めて使う）。
pub fn job(
    text: &str,
    highlight: &Highlight,
    mode: Mode,
    metrics: Metrics,
    wrap: f32,
) -> LayoutJob {
    let colors = theme::palette(mode);
    let mut job = LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: wrap,
            ..Default::default()
        },
        break_on_newline: true,
        ..Default::default()
    };
    let base = TextFormat {
        font_id: FontId::new(metrics.size, FontFamily::Proportional),
        color: colors.fg_primary,
        line_height: Some(metrics.size * metrics.line_height),
        ..Default::default()
    };
    let mut at = 0;
    let mut tokens: Vec<&(Range<usize>, TokenType, bool)> = highlight.tokens.iter().collect();
    tokens.sort_by_key(|token| token.0.start);
    for (range, kind, role) in tokens {
        let start = floor(text, range.start.max(at));
        let end = floor(text, range.end.min(text.len()));
        if start >= end {
            continue;
        }
        if at < start {
            job.append(&text[at..start], 0.0, base.clone());
        }
        let style = theme::token_style(mode, *kind);
        let mut format = TextFormat {
            font_id: FontId::new(metrics.size * style.scale, FontFamily::Proportional),
            color: style.color,
            italics: style.italic,
            ..base.clone()
        };
        if *role {
            format.underline = Stroke::new(1.5_f32, colors.ok);
        }
        job.append(&text[start..end], 0.0, format);
        at = end;
    }
    if at < text.len() {
        job.append(&text[at..], 0.0, base);
    }
    job
}

fn floor(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// 波線を描く（`galley` を `origin` に置いたとき）。
pub fn paint_squiggles(
    painter: &Painter,
    origin: Pos2,
    galley: &Arc<Galley>,
    text: &str,
    highlight: &Highlight,
    mode: Mode,
) {
    let colors = theme::palette(mode);
    for (range, severity) in &highlight.squiggles {
        let color = match severity {
            Severity::Error => colors.error,
            _ => colors.warn,
        };
        for (left, right, y) in row_spans(galley, text, range) {
            zigzag(
                painter,
                origin + Vec2::new(left, y + 2.0),
                origin + Vec2::new(right, y + 2.0),
                color,
            );
        }
    }
}

/// byte 範囲が占める行ごとの `(左端 x, 右端 x, ベースライン y)`（`galley` の座標）。
pub fn row_spans(galley: &Galley, text: &str, range: &Range<usize>) -> Vec<(f32, f32, f32)> {
    let start = char_index(text, range.start);
    let end = char_index(text, range.end).max(start + 1);
    let mut out = Vec::new();
    let mut offset = 0;
    for row in &galley.rows {
        let count = row.char_count_including_newline();
        let row_end = offset + row.char_count_excluding_newline();
        let from = start.max(offset);
        let to = end.min(row_end);
        if from < to {
            let left = row.x_offset(from - offset);
            let right = row.x_offset(to - offset);
            out.push((left, right.max(left + 2.0), row.rect.max.y - 2.0));
        }
        offset += count;
        if offset >= end {
            break;
        }
    }
    out
}

/// byte 位置 → char の番号。
pub fn char_index(text: &str, byte: usize) -> usize {
    text[..floor(text, byte)].chars().count()
}

/// 周期 4px・振幅 1.5px の波線（§11.3）。
fn zigzag(painter: &Painter, from: Pos2, to: Pos2, color: Color32) {
    let mut points = Vec::new();
    let mut x = from.x;
    let mut up = true;
    while x < to.x {
        points.push(Pos2::new(x, from.y + if up { -1.5 } else { 1.5 }));
        x += 2.0;
        up = !up;
    }
    points.push(Pos2::new(to.x, from.y));
    painter.add(egui::Shape::line(points, Stroke::new(1.0_f32, color)));
}
