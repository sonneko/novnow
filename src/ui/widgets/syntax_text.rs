//! 構文ハイライト付きのテキスト（§11.2）と下線（§11.3）。
//!
//! - 色はトークン（`core::features::semantic_tokens`）→ [`theme::token_style`]。
//! - 役割語の一致は **緑の直線**（`state.ok`、1.5px、ベースライン + 2px、§6.5.4・§11.3）。
//! - 診断は **波線**（エラー `state.error` / 警告 `state.warn`、周期 4px・振幅 1.5px、ベースライン + 2px）。
//! - どちらもレイアウト後の `Galley` のグリフ（`Glyph::pos.y` がベースライン）から位置を求めて上から描く
//!   （`TextFormat::underline` は行の下端に引くので、行間 1.8〜1.9 では文字から離れすぎる）。
//!   役割語の緑を先に、波線を後に描く = エラーが上（§11.3）。

use std::ops::Range;
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, Galley, Painter, Pos2, Stroke, Vec2};

use crate::core::diagnostics::{self, Severity};
use crate::core::features::inlay_hint::InlayKind;
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
    /// インレイヒント（位置・中身。§6.6.2）。エディタ（節エディタと M8 のワークスペースのエディタ）
    /// だけが入れる（[`crate::ui::widgets::inlay`]、`docs/history.md` B-137）。
    pub hints: Vec<(usize, InlayKind)>,
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
        Self {
            tokens,
            squiggles,
            hints: Vec::new(),
        }
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
    for (range, kind, _) in tokens {
        let start = floor(text, range.start.max(at));
        let end = floor(text, range.end.min(text.len()));
        if start >= end {
            continue;
        }
        if at < start {
            job.append(&text[at..start], 0.0, base.clone());
        }
        let style = theme::token_style(mode, *kind);
        let format = TextFormat {
            font_id: FontId::new(metrics.size * style.scale, FontFamily::Proportional),
            color: style.color,
            italics: style.italic,
            ..base.clone()
        };
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

/// 役割語の緑の直線と、診断の波線を描く（`galley` を `origin` に置いたとき）。緑が下、波線が上。
pub fn paint_squiggles(
    painter: &Painter,
    origin: Pos2,
    galley: &Arc<Galley>,
    text: &str,
    highlight: &Highlight,
    mode: Mode,
) {
    let colors = theme::palette(mode);
    for (range, _, role) in &highlight.tokens {
        if !role {
            continue;
        }
        for (left, right, y) in row_spans(galley, text, range) {
            painter.line_segment(
                [
                    origin + Vec2::new(left, y + 2.0),
                    origin + Vec2::new(right, y + 2.0),
                ],
                Stroke::new(1.5_f32, colors.ok),
            );
        }
    }
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
/// 線はベースライン + 2px に引く（§11.3）。
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
            // 右端は範囲の最後の字の右端（次の字の左端ではない。インレイヒントの隙間に線を引かない、B-137）
            let right = row
                .glyphs
                .get(to - offset - 1)
                .map_or_else(|| row.x_offset(to - offset), |last| last.max_x());
            // ベースライン（グリフの `pos.y`）。グリフが無ければ行の下端の少し上
            let baseline = row
                .glyphs
                .get(from - offset)
                .map(|glyph| glyph.pos.y)
                .unwrap_or(row.rect.max.y - 4.0);
            out.push((left, right.max(left + 2.0), baseline));
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

/// 行のベースラインと最大フォントのアセント（Galley 相対）。行番号・キャレットの
/// 縦位置の基準（行矩形の中心だとフォントや行間を変えたときに本文とずれる）。
/// 空行は、同じ Galley の他の行の「行上端→ベースライン」のオフセットを流用する。
pub fn row_baseline(galley: &Galley, row: &egui::epaint::text::Row) -> (f32, f32) {
    fn tallest(row: &egui::epaint::text::Row) -> Option<&egui::epaint::text::Glyph> {
        row.glyphs
            .iter()
            .max_by(|a, b| a.size.y.total_cmp(&b.size.y))
    }
    if let Some(glyph) = tallest(row) {
        return (glyph.pos.y, glyph.ascent);
    }
    for other in &galley.rows {
        if let Some(glyph) = tallest(other) {
            return (
                row.rect.min.y + (glyph.pos.y - other.rect.min.y),
                glyph.ascent,
            );
        }
    }
    (row.rect.max.y, row.rect.height())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn galley_of(text: &str) -> Arc<Galley> {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::ui::theme::fonts().0);
        let metrics = Metrics {
            size: 15.0,
            line_height: 1.8,
        };
        let mut laid = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            laid = Some(ctx.fonts(|fonts| {
                fonts.layout_job(job(text, &Highlight::default(), Mode::Dark, metrics, 660.0))
            }));
        });
        laid.expect("galley")
    }

    /// 行番号・キャレットの基準: グリフのある行はグリフのベースラインそのもの、
    /// 空行は隣の行と同じオフセット（行の中心ではない）。
    #[test]
    fn row_baseline_uses_glyph_baselines_even_for_empty_rows() {
        let galley = galley_of("あ\n\nい");
        assert_eq!(galley.rows.len(), 3);
        let (first, ascent) = row_baseline(&galley, &galley.rows[0]);
        assert_eq!(first, galley.rows[0].glyphs[0].pos.y);
        assert!(ascent > 0.0);
        let (empty, _) = row_baseline(&galley, &galley.rows[1]);
        let (third, _) = row_baseline(&galley, &galley.rows[2]);
        assert_eq!(third, galley.rows[2].glyphs[0].pos.y);
        // 空行のベースラインは上下の行のほぼ真ん中（等間隔）にある
        assert!(first < empty && empty < third, "{first} {empty} {third}");
        let upper = empty - first;
        let lower = third - empty;
        assert!((upper - lower).abs() <= 1.5, "{upper} vs {lower}");
        // 0 文字だけの Galley でも有限の値を返す
        let blank = galley_of("");
        let (base, _) = row_baseline(&blank, &blank.rows[0]);
        assert!(base.is_finite());
    }
}
