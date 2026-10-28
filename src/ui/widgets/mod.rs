//! 画面をまたいで使う小部品（§9.1「全体レイアウト」・§9.9「押せる要素」）。
//!
//! 押せる要素の見た目と当たり判定をここに集める。
//!
//! - 薄い下線を引き、ホバーで背景（`bg.selection` を 60% で塗る）とポインタを出す（F-30 / F-32）。
//! - `interact` の矩形は**最低 24×24px**（見た目の寸法は変えない。F-32）。

pub mod context_view;
pub mod guide;
pub mod inlay;
pub mod portrait;
pub mod reference_pane;
pub mod section_editor;
pub mod slice_editor;
pub mod syntax_text;

use egui::{Color32, FontId, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};

use crate::ui::theme::Palette;

/// 押せる要素の当たり判定の下限（§9.9、F-32。「24×24px 以上」）。
pub const MIN_HIT: f32 = 24.0;

/// 描く大きさは変えずに、入力の当たり判定だけを 24×24 以上へ広げる（§9.9、F-32）。
pub fn hit_rect(rect: Rect) -> Rect {
    rect.expand2(Vec2::new(
        (MIN_HIT - rect.width()).max(0.0) * 0.5,
        (MIN_HIT - rect.height()).max(0.0) * 0.5,
    ))
}

/// 標準の文字ボタン。キーボードで見えるラベルはそのままに、押せる範囲へ下線を引き、
/// どの画面でも同じ最小の当たり判定とポインタを使う（F-32）。
pub fn action_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    let response = ui.add(
        egui::Button::new(RichText::new(text.into()).underline()).min_size(Vec2::splat(MIN_HIT)),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// 押せる絞り込み・タブ・選択肢。操作ボタンと同じ下線の見た目にする（F-32）。
pub fn selectable_action(ui: &mut Ui, selected: bool, text: RichText) -> Response {
    let background = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(
        egui::Button::new(text.underline())
            .selected(selected)
            .frame(false)
            .min_size(Vec2::splat(MIN_HIT)),
    );
    if response.hovered() {
        ui.painter().set(
            background,
            egui::Shape::rect_filled(
                response.rect,
                2.0,
                ui.visuals().selection.bg_fill.gamma_multiply(0.6),
            ),
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// [`selectable_action`] の `LayoutJob` 版。検索一致の背景塗り（§11.3）のように
/// 行の途中で書式が変わる候補行に使う。見た目と当たり判定は `selectable_action` と同じ。
pub fn selectable_job(ui: &mut Ui, selected: bool, job: egui::text::LayoutJob) -> Response {
    let background = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(
        egui::Button::new(egui::WidgetText::LayoutJob(job))
            .selected(selected)
            .frame(false)
            .min_size(Vec2::splat(MIN_HIT)),
    );
    if response.hovered() {
        ui.painter().set(
            background,
            egui::Shape::rect_filled(
                response.rect,
                2.0,
                ui.visuals().selection.bg_fill.gamma_multiply(0.6),
            ),
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn row_format(font: &FontId, color: Color32) -> egui::text::TextFormat {
    egui::text::TextFormat {
        font_id: font.clone(),
        color,
        underline: Stroke::new(1.0_f32, color),
        ..Default::default()
    }
}

/// 下線つきの素の断片を足す（検索結果行の接頭辞など）。
pub fn append_text(job: &mut egui::text::LayoutJob, text: &str, font: &FontId, color: Color32) {
    if !text.is_empty() {
        job.append(text, 0.0, row_format(font, color));
    }
}

/// `text` の中の `query` の一致（文字どおり）へ §11.3 の検索一致の背景塗りを付けた断片列を足す。
pub fn append_match(
    job: &mut egui::text::LayoutJob,
    text: &str,
    query: &str,
    font: &FontId,
    color: Color32,
    match_bg: Color32,
) {
    if query.is_empty() {
        append_text(job, text, font, color);
        return;
    }
    let mut at = 0;
    for (start, found) in text.match_indices(query) {
        append_text(job, &text[at..start], font, color);
        let mut matched = row_format(font, color);
        matched.background = match_bg;
        job.append(found, 0.0, matched);
        at = start + found.len();
    }
    append_text(job, &text[at..], font, color);
}

/// 構造化された抜粋（前文・一致・後文）を、一致だけ背景塗りにして足す（§9.7.4 の全文検索タブ）。
pub fn append_excerpt(
    job: &mut egui::text::LayoutJob,
    before: &str,
    matched: &str,
    after: &str,
    font: &FontId,
    color: Color32,
    match_bg: Color32,
) {
    append_text(job, before, font, color);
    let mut format = row_format(font, color);
    format.background = match_bg;
    if !matched.is_empty() {
        job.append(matched, 0.0, format);
    }
    append_text(job, after, font, color);
}

/// 押せる状態表示のラベル。描く文字の大きさはそのままに、入力の範囲だけを広げる
/// （`Label::sense` が字形の範囲だけを使うのとは違う）。
pub fn clickable_label(ui: &mut Ui, text: RichText, id: egui::Id) -> Response {
    let background = ui.painter().add(egui::Shape::Noop);
    let painted = ui.label(text.underline());
    let response = ui.interact(hit_rect(painted.rect), id, Sense::click());
    if response.hovered() {
        ui.painter().set(
            background,
            egui::Shape::rect_filled(
                response.rect,
                2.0,
                ui.visuals().selection.bg_fill.gamma_multiply(0.6),
            ),
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// 折り返して描く外部 URL。押せる範囲を明示的に持つ。http(s) の URL を出してよいと
/// 分かっている呼び出し側だけが使い、ふつうの本文は静的なままにする（§10.5.1）。
pub fn external_link(ui: &mut Ui, label: &str, url: &str, colors: &Palette) -> Response {
    use egui::text::{LayoutJob, TextFormat};
    let format = |color| TextFormat {
        font_id: FontId::proportional(13.0),
        color,
        underline: Stroke::new(0.6_f32, colors.border),
        ..Default::default()
    };
    let mut job = LayoutJob::single_section(label.to_owned(), format(colors.fg_muted));
    job.wrap.max_width = ui.available_width().max(MIN_HIT);
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    let (rect, response) =
        ui.allocate_exact_size(galley.size().max(Vec2::splat(MIN_HIT)), Sense::click());
    let painter = ui.painter_at(rect);
    if response.hovered() {
        painter.rect_filled(rect, 2.0, colors.bg_selection.gamma_multiply(0.6));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        let mut job = LayoutJob::single_section(label.to_owned(), format(colors.accent));
        job.wrap.max_width = ui.available_width().max(MIN_HIT);
        let highlighted = ui.fonts(|fonts| fonts.layout_job(job));
        painter.galley(rect.min, highlighted, colors.accent);
    } else {
        painter.galley(rect.min, galley, colors.fg_muted);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, label));
    if response.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
    response
}

/// クリックできるテキスト（下線つきリンク風）。
///
/// 見た目は文字と下線だけ（枠線は引かない、§9.5.1）。当たり判定は [`MIN_HIT`] まで広げる。
/// 文字色は `fg.primary`、ホバー中の背景は `bg.selection` の 60%。
pub fn link_button(ui: &mut Ui, text: &str, font: FontId, colors: &Palette) -> Response {
    let galley = ui.fonts(|fonts| fonts.layout_no_wrap(text.to_owned(), font, colors.fg_primary));
    let size = galley.size();
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(size.x.max(MIN_HIT), size.y.max(MIN_HIT)),
        Sense::click(),
    );
    let at = egui::pos2(rect.min.x, rect.center().y - size.y / 2.0);
    let text_rect = egui::Rect::from_min_size(at, size);
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, 2.0, colors.bg_selection.gamma_multiply(0.6));
    }
    painter.galley(at, galley, colors.fg_primary);
    painter.line_segment(
        [
            text_rect.left_bottom() + Vec2::new(0.0, 1.0),
            text_rect.right_bottom() + Vec2::new(0.0, 1.0),
        ],
        Stroke::new(1.0_f32, colors.border),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// 候補一覧のスクロール領域（候補一覧を持つ重ね物で共有する）。
/// 枠の右端に寄せた細いスクロールバーにし、選択行への追従スは各行が
/// [`follow_selection`] で行う。
pub fn candidate_scroll<R>(
    ui: &mut Ui,
    id: &str,
    max_height: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    ui.spacing_mut().scroll = egui::style::ScrollStyle::thin();
    egui::ScrollArea::vertical()
        .id_source(id)
        .max_height(max_height)
        .show(ui, add)
        .inner
}

/// 選択行が見えるところまで最小移動で追従する（キーボード移動だけで選択行が常に見える）。
pub fn follow_selection(response: &Response, selected: bool) {
    if selected {
        response.scroll_to_me(None);
    }
}

/// キー案内の項目列（各項目がボタン。押された項目の番号を返す）。
/// 項目は「キー＋動作」の文言キー単位で持ち、実行は呼び出し側がキーの経路と同じ
/// 処理へつなぐ（案内と実行がずれないように）。
pub fn key_guide(ui: &mut Ui, items: &[&str], font: FontId, colors: &Palette) -> Option<usize> {
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                ui.label(RichText::new("·").font(font.clone()).color(colors.fg_faint));
            }
            // `link_button` と同じ自前描画（折り返しレイアウトの中で `Label` の
            // 矩形が行幅いっぱいに広がり、当たり判定と見た目がずれるのを避ける）
            let galley = ui.fonts(|fonts| {
                fonts.layout_no_wrap((*item).to_owned(), font.clone(), colors.fg_faint)
            });
            let size = galley.size();
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(size.x.max(MIN_HIT), size.y.max(MIN_HIT)),
                Sense::click(),
            );
            let at = egui::pos2(rect.min.x, rect.center().y - size.y / 2.0);
            let text_rect = egui::Rect::from_min_size(at, size);
            let painter = ui.painter();
            if response.hovered() {
                painter.rect_filled(rect, 2.0, colors.bg_selection.gamma_multiply(0.6));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            painter.galley(at, galley, colors.fg_faint);
            painter.line_segment(
                [
                    text_rect.left_bottom() + Vec2::new(0.0, 1.0),
                    text_rect.right_bottom() + Vec2::new(0.0, 1.0),
                ],
                Stroke::new(1.0_f32, colors.border),
            );
            if response.clicked() {
                chosen = Some(index);
            }
        }
    });
    chosen
}

/// 編集面の左線の状態（§9.2.4）。青 = 編集面とフォーカス、橙・緑・灰 = 状態で分け、
/// 同じ左端で重なる場合は状態表示を優先する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldState {
    /// 通常（状態線なし）。
    Normal,
    /// 警告（提案モード中の節エディタ）。
    Warn,
}

/// 編集面の左線の色（§9.2.4 の二段表示）: 普段は `accent.faint`、フォーカス中は
/// `accent`。状態表示があるときはそちらを優先する。
pub fn field_accent(state: FieldState, focused: bool, colors: &Palette) -> egui::Color32 {
    match state {
        FieldState::Warn => colors.warn,
        FieldState::Normal if focused => colors.accent,
        FieldState::Normal => colors.accent_faint,
    }
}

/// 編集面の左端へ重ねる 3px の縦線（§9.2.4）。高さは `rect` いっぱい。
pub fn paint_field_accent(ui: &Ui, rect: Rect, color: egui::Color32) {
    let line = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
    ui.painter().rect_filled(line, 0.0, color);
}

/// 台詞比率バー（§9.4.2）: 幅 `width`・高さ 4px の横棒（塗りが台詞割合、色は常に
/// `fg.faint` 単色）＋右に 10px の `%`。比率で色は変えない（段階2の目標は「測って
/// 見える化して自覚を促す」ことで、達成・未達で鼓舞・𠮟責しない）。分母 0 の節では
/// 呼び出し側が描かない。節エディタは 64px、話プロットは 48px（いずれも倍率込み）。
pub fn ratio_bar(ui: &mut Ui, ratio: f64, width: f32, scale: f32, colors: &Palette) -> Response {
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 4.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, colors.bg_inset);
        let filled = Rect::from_min_size(
            rect.min,
            Vec2::new(width * ratio.clamp(0.0, 1.0) as f32, 4.0),
        );
        painter.rect_filled(filled, 0.0, colors.fg_faint);
        ui.label(
            RichText::new(format!("{:.0}%", ratio * 100.0))
                .font(FontId::new(10.0 * scale, egui::FontFamily::Proportional))
                .color(colors.fg_faint),
        );
        response
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{palette, Mode};

    /// §9.2.4: 普段は淡色、フォーカス中は accent、状態表示があればそちら優先。
    #[test]
    fn field_accent_follows_the_two_tier_rule() {
        for mode in [Mode::Dark, Mode::Light] {
            let colors = palette(mode);
            assert_eq!(
                field_accent(FieldState::Normal, false, &colors),
                colors.accent_faint
            );
            assert_eq!(
                field_accent(FieldState::Normal, true, &colors),
                colors.accent
            );
            assert_eq!(field_accent(FieldState::Warn, false, &colors), colors.warn);
            assert_eq!(field_accent(FieldState::Warn, true, &colors), colors.warn);
        }
    }

    /// §11.3: 検索一致の範囲だけに背景塗りが付き、行の文字列そのものは変わらない。
    #[test]
    fn search_match_paints_only_the_matched_ranges() {
        let colors = palette(Mode::Dark);
        let font = FontId::proportional(13.0);
        let mut job = egui::text::LayoutJob::default();
        append_text(&mut job, "p:1  ", &font, colors.fg_primary);
        append_match(
            &mut job,
            "銀の指輪と指輪",
            "指輪",
            &font,
            colors.fg_primary,
            colors.bg_selection,
        );
        assert_eq!(job.text, "p:1  銀の指輪と指輪");
        let painted: Vec<&str> = job
            .sections
            .iter()
            .filter(|section| section.format.background == colors.bg_selection)
            .map(|section| &job.text[section.byte_range.clone()])
            .collect();
        assert_eq!(painted, ["指輪", "指輪"]);
        let mut excerpt = egui::text::LayoutJob::default();
        append_excerpt(
            &mut excerpt,
            "銀の",
            "指輪",
            "を外した",
            &font,
            colors.fg_primary,
            colors.bg_selection,
        );
        let painted: Vec<&str> = excerpt
            .sections
            .iter()
            .filter(|section| section.format.background == colors.bg_selection)
            .map(|section| &excerpt.text[section.byte_range.clone()])
            .collect();
        assert_eq!(painted, ["指輪"]);
    }
}
