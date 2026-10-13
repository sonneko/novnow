//! インレイヒントの描画（§6.6.2。core の [`inlay_hint::range`] を、エディタの `TextEdit` の上に描く）。
//!
//! - egui 0.27 の `TextEdit` には「文字ではない表示」を挟めない。そこで、ヒントの位置から始まる
//!   レイアウトのセクションに `leading_space`（ヒントの幅）を付けて**隙間を空け**、その隙間に
//!   ヒントを上から描く（`PLANS.md` §9.2 B-137）。文字と `Galley` の対応は変わらないので、カーソル・
//!   選択・IME はそのまま動く。ヒントは文字ではないので、選択・コピー・保存・エクスポートには
//!   入らない（MUST）。
//! - 行末（改行の直前）とテキストの末尾のヒントは、後ろに文字が無いので隙間を空けずに描く。
//! - 文字色は `fg.faint`、大きさは本文の 0.85 倍、ベースラインを本文に揃える（§6.6.2）。
//! - 括弧と「提案中」「却下」の添え文言はカタログ（`editor.inlay_*`、§12.2）。モードの表示名は
//!   ソースの `B+C` と同じ順に `+` でつなぐ。
//! - 出すのはエディタ（節エディタと M8 のワークスペースのエディタ）だけ。文脈ビューなど読むだけの
//!   表示・エクスポート・読者モードには出さない。`Alt+H` で出し入れする（`ui.json.inlay_hints`）。

use std::ops::Range;
use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{FontFamily, FontId, Galley, Painter, Pos2, Ui, Vec2};

use crate::core::features::inlay_hint::{self, InlayKind};
use crate::core::features::Project;
use crate::core::model::document::Document;
use crate::core::model::material::MaterialStatus;
use crate::ui::i18n::{self, Locale};
use crate::ui::theme::{self, Mode};

/// ヒントと前後の文字の間の余白（px、片側）。
pub const PAD: f32 = 3.0;

/// 本文に対するヒントの文字の大きさ（§6.6.2）。
pub const SCALE: f32 = 0.85;

/// 文書 `document` の `range`（byte）の中のヒントを、`range.start` 起点の位置に直して返す。
pub fn of_range(
    document: &Document,
    range: Range<usize>,
    project: &Project,
) -> Vec<(usize, InlayKind)> {
    inlay_hint::range(document, range.clone(), project)
        .into_iter()
        .filter_map(|hint| Some((hint.at.checked_sub(range.start)?, hint.kind)))
        .collect()
}

/// ヒントの文言（§6.6.2: `⟨タイトル⟩`・`⟨タイトル・提案中⟩`・`⟨タイトル・却下⟩`、モードは表示名）。
pub fn label(locale: Locale, kind: &InlayKind) -> String {
    match kind {
        InlayKind::MaterialTitle { title, status, .. } => match status {
            Some(MaterialStatus::Proposed) => {
                i18n::format(locale, "editor.inlay_proposed", &[title.as_str()])
            }
            Some(MaterialStatus::Rejected) => {
                i18n::format(locale, "editor.inlay_rejected", &[title.as_str()])
            }
            _ => i18n::format(locale, "editor.inlay_title", &[title.as_str()]),
        },
        InlayKind::ModeLabels { labels } => {
            i18n::format(locale, "editor.inlay_modes", &[labels.join("+").as_str()])
        }
    }
}

/// ヒントの文言をレイアウトする（位置はテキストの中の byte のまま）。`size` は本文の文字の大きさ。
pub fn lay_out(
    ui: &Ui,
    hints: &[(usize, InlayKind)],
    locale: Locale,
    mode: Mode,
    size: f32,
) -> Vec<(usize, Arc<Galley>)> {
    if hints.is_empty() {
        return Vec::new();
    }
    let color = theme::palette(mode).fg_faint;
    let font = FontId::new(size * SCALE, FontFamily::Proportional);
    ui.fonts(|fonts| {
        hints
            .iter()
            .map(|(at, kind)| {
                let galley = fonts.layout_no_wrap(label(locale, kind), font.clone(), color);
                (*at, galley)
            })
            .collect()
    })
}

/// 隙間の幅（ヒントの幅 + 両側の余白）。
pub fn gap_of(galley: &Galley) -> f32 {
    galley.size().x + 2.0 * PAD
}

/// `job` の、位置 `at`（`job.text` の中の byte）から始まるセクションに `leading_space` を足して、
/// 幅 `width` の隙間を空ける（セクションの途中なら 2 つに割る）。行末・末尾・文字の境界でない位置は
/// 何もしない。
pub fn open_gaps(job: &mut LayoutJob, gaps: &[(usize, f32)]) {
    for &(at, width) in gaps {
        if at == 0
            || at >= job.text.len()
            || !job.text.is_char_boundary(at)
            || job.text[at..].starts_with('\n')
        {
            continue;
        }
        let Some(index) = job
            .sections
            .iter()
            .position(|section| section.byte_range.contains(&at))
        else {
            continue;
        };
        let section = &mut job.sections[index];
        if section.byte_range.start == at {
            section.leading_space += width;
        } else {
            let mut tail = section.clone();
            tail.byte_range = at..section.byte_range.end;
            tail.leading_space = width;
            section.byte_range.end = at;
            job.sections.insert(index + 1, tail);
        }
    }
}

/// ヒントを描く（`galley` を `origin` に置いたとき。`text` はその中身）。位置はヒントの直前の文字
/// （トークンの最後の文字）の右端 + 余白で、ベースラインを本文に揃える。
pub fn paint(
    painter: &Painter,
    origin: Pos2,
    galley: &Galley,
    text: &str,
    laid: &[(usize, Arc<Galley>)],
    mode: Mode,
) {
    let color = theme::palette(mode).fg_faint;
    for (at, label) in laid {
        let Some(before) = char_before(text, *at) else {
            continue;
        };
        let Some(glyph) = glyph_of(galley, before) else {
            continue;
        };
        let ascent = label
            .rows
            .first()
            .and_then(|row| row.glyphs.first())
            .map_or(label.size().y * 0.8, |first| first.pos.y);
        let pos = origin + Vec2::new(glyph.max_x() + PAD, glyph.pos.y - ascent);
        painter.galley(pos, label.clone(), color);
    }
}

/// byte 位置 `at` の直前の文字の番号（char）。`at` が先頭・境界でない位置なら `None`。
fn char_before(text: &str, at: usize) -> Option<usize> {
    if at == 0 || at > text.len() || !text.is_char_boundary(at) {
        return None;
    }
    Some(text[..at].chars().count() - 1)
}

/// 番号 `index`（char）の文字のグリフ（改行は除く）。
fn glyph_of(galley: &Galley, index: usize) -> Option<&egui::epaint::text::Glyph> {
    let mut offset = 0;
    for row in &galley.rows {
        let visible = row.char_count_excluding_newline();
        if index < offset + visible {
            return row.glyphs.get(index - offset);
        }
        offset += row.char_count_including_newline();
    }
    None
}
