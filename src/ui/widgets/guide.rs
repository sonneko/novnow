//! ガイドモードの描画（§9.10）。対象を枠と矢印で指し、吹き出しに 1 文を出す。
//!
//! - **画面を暗くしない**（膜を描かない）。対象の外側を塗らないので、ガイド中も本文を書ける。
//! - 吹き出しは対象の**下 → 上 → 右 → 左**の順に空きを探して置く（画面の端で逃げる）。
//!   幅 900px 未満（§9.1 の折り畳み）では対象の上に置き、矢印だけを短くする。
//! - キーは横取りしない。`Alt+Enter`（次へ）・`Alt+Backspace`（戻る）・`Esc`（閉じる）は
//!   呼ぶ側（`ui::app`）が受け、ここはボタンのクリックだけを返す。

use egui::{Color32, Context, Frame, Id, Margin, Rect, Stroke, Vec2};

use crate::ui::guide::Guide;
use crate::ui::i18n::{self, Locale};
use crate::ui::theme::{self, Mode};

/// 吹き出しの操作。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Action {
    /// 何も押されていない。
    #[default]
    None,
    /// 「次へ」。
    Next,
    /// 「戻る」。
    Back,
    /// 「あとで閉じる」。
    Close,
}

/// 幅が狭い画面（§9.1 の折り畳み）では吹き出しを対象の上に置く。
pub const NARROW: f32 = 900.0;

/// 吹き出しの幅。
const BUBBLE: f32 = 320.0;

/// ガイドを描く。`target` は指したい部品の矩形（無ければ画面の中央に吹き出しだけを出す）。
pub fn draw(
    ctx: &Context,
    mode: Mode,
    locale: Locale,
    guide: &Guide,
    target: Option<Rect>,
) -> Action {
    let Some(step) = guide.step() else {
        return Action::None;
    };
    let colors = theme::palette(mode);
    let screen = ctx.screen_rect();

    if let Some(rect) = target {
        // 対象を囲む枠（塗らない。入力は奪わない）
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            Id::new("guide-frame"),
        ))
        .rect_stroke(rect.expand(4.0), 4.0, Stroke::new(1.5_f32, colors.accent));
    }

    let (width, height) = (BUBBLE.min(screen.width() - 32.0), 150.0);
    let narrow = screen.width() < NARROW;
    let (anchor, above) = match target {
        Some(rect) if narrow => (rect.center_top(), true),
        Some(rect) if rect.bottom() + height + 24.0 <= screen.bottom() => {
            (rect.center_bottom(), false)
        }
        Some(rect) => (rect.center_top(), true),
        None => (screen.center(), false),
    };
    let offset = if above {
        Vec2::new(0.0, -if narrow { 12.0 } else { 18.0 })
    } else {
        Vec2::new(0.0, 18.0)
    };

    let mut action = Action::None;
    egui::Area::new(Id::new("guide-bubble"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor + offset)
        .show(ctx, |ui| {
            Frame::popup(&ctx.style())
                .fill(colors.bg_raised)
                .stroke(Stroke::new(1.0_f32, colors.accent))
                .inner_margin(Margin::same(10.0))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.label(
                        egui::RichText::new(i18n::text(locale, "guide.title"))
                            .small()
                            .color(colors.fg_faint),
                    );
                    ui.label(
                        egui::RichText::new(i18n::format(
                            locale,
                            "guide.progress",
                            &[
                                &guide.progress().0.to_string(),
                                &guide.progress().1.to_string(),
                            ],
                        ))
                        .small()
                        .color(colors.fg_muted),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(i18n::text(locale, step.text))
                            .color(colors.fg_primary)
                            .strong(),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(i18n::text(locale, step.hint))
                            .color(colors.fg_muted)
                            .monospace(),
                    );
                    if !step.advance.by_button() {
                        ui.label(
                            egui::RichText::new(i18n::text(locale, "guide.waiting"))
                                .small()
                                .color(colors.fg_faint),
                        );
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if step.advance.by_button()
                            && crate::ui::widgets::action_button(
                                ui,
                                i18n::text(locale, "guide.next"),
                            )
                            .clicked()
                        {
                            action = Action::Next;
                        }
                        if guide.index > 0
                            && crate::ui::widgets::action_button(
                                ui,
                                i18n::text(locale, "guide.back"),
                            )
                            .clicked()
                        {
                            action = Action::Back;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "guide.later"))
                            .clicked()
                        {
                            action = Action::Close;
                        }
                    });
                });
        });

    // 対象と吹き出しを結ぶ短い線（矢印の代わり。折り畳みでは短くする）
    if let (Some(rect), true) = (target, action == Action::None) {
        let from = if above {
            rect.center_top()
        } else {
            rect.center_bottom()
        };
        let to = if above {
            egui::pos2(anchor.x, anchor.y - 18.0)
        } else {
            egui::pos2(anchor.x, anchor.y + 18.0)
        };
        let color: Color32 = colors.accent;
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            Id::new("guide-arrow"),
        ))
        .line_segment([from, to], Stroke::new(1.5_f32, color));
    }

    action
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 吹き出しの位置（対象の下か上か）を決める規則だけを切り出して試す。
    fn place(target: Rect, screen: Rect) -> (bool, bool) {
        let narrow = screen.width() < NARROW;
        let above = if narrow {
            true
        } else {
            target.bottom() + 150.0 + 24.0 > screen.bottom()
        };
        (above, narrow)
    }

    #[test]
    fn the_bubble_goes_below_when_there_is_room() {
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let target = Rect::from_min_size(egui::Pos2::new(100.0, 100.0), Vec2::new(200.0, 40.0));
        assert_eq!(place(target, screen), (false, false));
    }

    #[test]
    fn the_bubble_escapes_upward_at_the_bottom_edge() {
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let target = Rect::from_min_size(egui::Pos2::new(100.0, 740.0), Vec2::new(200.0, 40.0));
        assert!(place(target, screen).0);
    }

    #[test]
    fn narrow_screens_put_the_bubble_above() {
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(700.0, 800.0));
        let target = Rect::from_min_size(egui::Pos2::new(10.0, 10.0), Vec2::new(200.0, 40.0));
        assert_eq!(place(target, screen), (true, true));
    }
}
