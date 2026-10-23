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

use egui::{FontId, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};

use crate::ui::theme::Palette;

/// 押せる要素の当たり判定の下限（§9.9、F-32。「24×24px 以上」）。
pub const MIN_HIT: f32 = 24.0;

/// Keep the painted size unchanged while expanding the input target to at least 24×24.
pub fn hit_rect(rect: Rect) -> Rect {
    rect.expand2(Vec2::new(
        (MIN_HIT - rect.width()).max(0.0) * 0.5,
        (MIN_HIT - rect.height()).max(0.0) * 0.5,
    ))
}

/// Standard textual action. Keep the keyboard-visible label, underline the
/// click target, and use the same minimum hit area and pointer on every screen.
pub fn action_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    let response = ui.add(
        egui::Button::new(RichText::new(text.into()).underline()).min_size(Vec2::splat(MIN_HIT)),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// Clickable filter/tab/choice with the same underlined affordance as actions.
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

/// Clickable status label: preserve its painted text size but expand the input
/// area around it (unlike `Label::sense`, which uses the glyph bounds only).
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

/// A wrapped external URL with an explicit click target. Only callers with an
/// approved http(s) URL should invoke this; ordinary prose remains static.
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
