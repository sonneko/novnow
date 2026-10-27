//! ステータスバーと省略判定（§9.6.1）・ミニヘッダー（§9.4.2）。幅は実測ベースで、
//! 画面幅が狭い順に要素を省略する。

use egui::{
    Align, Align2, FontFamily, FontId, Layout, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};

use crate::ui::app::{diagnostic_count, Session};
use crate::ui::i18n::{self, Locale};
use crate::ui::palette;
use crate::ui::save_machine::Indicator;
use crate::ui::theme::{self, Mode};
use crate::ui::widgets::context_view;

/// ステータスバー（§9.6.1）。保存インジケータが押されたら `true`。
/// ヘッダ左側（パンくず・プロジェクト名・画面名の一時表示）の省略判定。
/// 幅は各要素の実測（先行する余白込み）。省略の順は、一時表示 → プロジェクト名 →
/// パンくず（返す順は、パンくず・プロジェクト名・一時表示の表示可否）。
pub(crate) fn header_left_visibility(
    room: f32,
    crumb: f32,
    name: f32,
    transient: f32,
) -> (bool, bool, bool) {
    // 上位が収まらない時点でそれより下位は出さない（省略の順を守る）
    let mut rest = room.max(0.0);
    let mut take = |width: f32| -> bool {
        if width <= 0.0 {
            return false;
        }
        if width <= rest {
            rest -= width;
            true
        } else {
            rest = 0.0;
            false
        }
    };
    let show_crumb = take(crumb);
    let show_name = take(name);
    let show_transient = take(transient);
    (show_crumb, show_name, show_transient)
}

/// ステータスバーの省略判定（優先順位の高いものから実測幅で収まるだけ出す）。
/// 並びは、パンくず・文字数・原稿用紙枚数・セッション情報・通知。
pub(crate) fn status_visibility(room: f32, widths: &[f32; 5], present: &[bool; 5]) -> [bool; 5] {
    // 上位が収まらない時点でそれより下位は出さない（畳む順を守る）
    let mut rest = room.max(0.0);
    let mut shown = [false; 5];
    for ((&width, &exists), visible) in widths.iter().zip(present.iter()).zip(shown.iter_mut()) {
        if !exists {
            continue;
        }
        if width > rest {
            break;
        }
        *visible = true;
        rest -= width;
    }
    shown
}

#[allow(clippy::too_many_arguments)] // Explicit display settings and the retained session match other UI entry points.
pub(crate) fn status_bar(
    ui: &mut egui::Ui,
    session: &mut Session,
    locale: Locale,
    mode: Mode,
    scale: f32,
    indicator: Indicator,
    now: f64,
    stats_mode: bool,
) -> bool {
    let colors = theme::palette(mode);
    let font = FontId::new(11.0 * scale, FontFamily::Proportional);
    let mono = FontId::new(11.0 * scale, FontFamily::Monospace);
    let mut clicked = false;
    ui.horizontal_centered(|ui| {
        let width = ui.available_width();
        // Reserve the rightmost save state before placing lower-priority labels.
        // A clipped long notice or diagnostic count must never push it off-screen.
        let reserved = if indicator == Indicator::Failed {
            170.0
        } else if session.bench.save.guards_close() {
            46.0
        } else {
            23.0
        };
        ui.allocate_ui_with_layout(
            Vec2::new((width - reserved).max(0.0), ui.available_height()),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                let dot = || {
                    RichText::new("  ·  ")
                        .font(font.clone())
                        .color(colors.fg_faint)
                };
                // 省略はウィンドウ幅の固定しきい値ではなく各要素の実測幅で決める。
                // 優先順位は、パンくず > 文字数 > 原稿用紙枚数 > セッション情報 > 通知
                // （畳む順はその逆。保存表示は右端に予約済みで常に残る）
                let fonts_ctx = ui.ctx().clone();
                let measure = |text: &str, font: &FontId| -> f32 {
                    fonts_ctx.fonts(|fonts| {
                        fonts
                            .layout_no_wrap(text.to_owned(), font.clone(), colors.fg_primary)
                            .rect
                            .width()
                    })
                };
                let dot_width = measure("  ·  ", &font);
                let proposal = session
                    .bench
                    .human_active()
                    .then(|| i18n::text(locale, "stage.proposal").to_owned());
                let crumb = session.bench.breadcrumb();
                let (section, episode, project) = session.bench.counts();
                let unit = i18n::text(locale, "status.chars_unit");
                let chars = format!(
                    "{}{unit} / {}{unit} / {}{unit}",
                    context_view::group(section),
                    context_view::group(episode),
                    context_view::group(project)
                );
                let chars = if stats_mode {
                    let minutes = session.tabs.session.minutes(now);
                    let date = session.tabs.stamp.get(..10).unwrap_or("");
                    let recent = crate::core::features::metrics::recent_dates(date, 400);
                    let stats = &session.tabs.stats;
                    let today = stats.days.get(date).map_or(0, |d| d.chars);
                    let speed = if minutes > 0 {
                        format!("{:.1}", session.tabs.session.delta as f64 / minutes as f64)
                    } else {
                        "—".to_owned()
                    };
                    let average =
                        stats.average(&recent.iter().take(7).cloned().collect::<Vec<_>>());
                    i18n::format(
                        locale,
                        "stage.stats_status",
                        &[
                            &today.to_string(),
                            &speed,
                            &format!("{average:.0}"),
                            &stats.streak(&recent).to_string(),
                        ],
                    )
                } else {
                    chars
                };
                let page = session
                    .bench
                    .project()
                    .settings()
                    .writing
                    .manuscript_page_chars
                    .max(1) as f64;
                let pages = format!(
                    "{:.1}{}",
                    episode as f64 / page,
                    i18n::text(locale, "status.pages_unit")
                );
                let session_text = (!stats_mode).then(|| {
                    format!(
                        "{:+}{unit} {}{}",
                        session.tabs.session.delta,
                        session.tabs.session.minutes(now),
                        i18n::text(locale, "stage.minutes")
                    )
                });
                let notice = session
                    .notice
                    .as_ref()
                    .filter(|(_, at)| now - at < 4.0)
                    .map(|(message, _)| message.clone());
                let count = diagnostic_count(session);
                let conflict = session.bench.conflicts().next().is_some();
                let agent_label = crate::ui::agents::status_label(&session.agents, locale);
                let mut mandatory = proposal.as_deref().map_or(0.0, |text| measure(text, &font));
                if count > 0 {
                    mandatory += dot_width + measure(&format!("⚑{count}"), &mono);
                }
                if conflict {
                    mandatory += dot_width + measure("⇄", &mono);
                }
                if let Some(label) = &agent_label {
                    mandatory += dot_width + measure(label, &mono);
                }
                let widths = [
                    crumb
                        .as_deref()
                        .map_or(0.0, |text| measure(text, &mono) + dot_width),
                    measure(&chars, &mono) + dot_width,
                    measure(&pages, &mono),
                    session_text
                        .as_deref()
                        .map_or(0.0, |text| measure(text, &mono)),
                    notice
                        .as_deref()
                        .map_or(0.0, |text| dot_width + measure(text, &font)),
                ];
                let present = [
                    crumb.is_some(),
                    true,
                    true,
                    session_text.is_some(),
                    notice.is_some(),
                ];
                let room = (width - reserved - mandatory).max(0.0);
                let mut shown = status_visibility(room, &widths, &present);
                let any_hidden = present
                    .iter()
                    .zip(&shown)
                    .any(|(exists, visible)| *exists && !visible);
                if any_hidden {
                    // `…` 自体の幅を引いてもう一度判定する
                    shown =
                        status_visibility((room - measure("…", &font)).max(0.0), &widths, &present);
                }
                let mut hidden: Vec<String> = Vec::new();
                if let Some(text) = &proposal {
                    ui.colored_label(colors.warn, text.clone());
                }
                if let Some(text) = &crumb {
                    if shown[0] {
                        ui.label(
                            RichText::new(text.clone())
                                .font(mono.clone())
                                .color(colors.fg_muted),
                        );
                        ui.label(dot());
                    } else {
                        hidden.push(text.clone());
                    }
                }
                if shown[1] {
                    ui.label(
                        RichText::new(&chars)
                            .font(mono.clone())
                            .color(colors.fg_muted),
                    );
                    ui.label(dot());
                } else {
                    hidden.push(chars.clone());
                }
                if shown[2] {
                    ui.label(
                        RichText::new(&pages)
                            .font(mono.clone())
                            .color(colors.fg_muted),
                    );
                } else {
                    hidden.push(pages.clone());
                }
                if let Some(text) = &session_text {
                    if shown[3] {
                        ui.label(
                            RichText::new(text.clone())
                                .font(mono.clone())
                                .color(colors.fg_muted),
                        );
                    }
                }
                if !shown[4] {
                    if let Some(text) = &notice {
                        hidden.push(text.clone());
                    }
                }
                if !hidden.is_empty() {
                    ui.label(RichText::new("…").color(colors.fg_faint))
                        .on_hover_text(hidden.join(" · "));
                }
                // 診断件数（E + W、今のファイル。0 件なら出さない）
                if count > 0 {
                    ui.label(dot());
                    let flag = crate::ui::widgets::clickable_label(
                        ui,
                        RichText::new(format!("⚑{count}"))
                            .font(mono.clone())
                            .color(colors.warn),
                        egui::Id::new("status-open-review"),
                    )
                    .on_hover_text(i18n::text(locale, "status.open_review"));
                    if flag.clicked() {
                        session.open_review = true;
                    }
                }
                if conflict {
                    ui.label(dot());
                    ui.label(RichText::new("⇄").font(mono.clone()).color(colors.error))
                        .on_hover_text(i18n::text(locale, "status.conflict"));
                }
                // 稼働表示（§15.7.4: `⟳ T0042 writer` / `⟳ 3件`、自律なら `(自律)`。クリックで一覧）
                if let Some(label) = &agent_label {
                    ui.label(dot());
                    let running = crate::ui::widgets::clickable_label(
                        ui,
                        RichText::new(label).font(mono.clone()).color(colors.accent),
                        egui::Id::new("status-open-agent"),
                    )
                    .on_hover_text(i18n::text(locale, "status.open_agent"));
                    if running.clicked() {
                        session.open_agent = true;
                    }
                }
                if shown[4] {
                    if let Some(message) = &notice {
                        ui.label(dot());
                        ui.label(
                            RichText::new(message)
                                .font(font.clone())
                                .color(colors.fg_faint),
                        );
                    }
                }
                // F-05: live modifier-prefix completion, without displacing the save indicator.
                let mods = ui.ctx().input(|input| input.modifiers);
                let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
                if let Some(key) = modifier_hint_key(mods, mac) {
                    let detail = i18n::text(locale, key);
                    let short = palette::ellipsize(detail, 23);
                    if measure(&short, &font) + dot_width < ui.available_width() {
                        ui.label(dot());
                        ui.label(
                            RichText::new(short)
                                .font(font.clone())
                                .color(colors.fg_faint),
                        )
                        .on_hover_text(detail);
                    }
                }
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (rect, response) = ui.allocate_exact_size(
                Vec2::splat(24.0),
                if indicator == Indicator::Failed {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            let painter = ui.painter_at(rect);
            let center = rect.center();
            if indicator == Indicator::Failed {
                painter.line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(0.6_f32, colors.border),
                );
                if response.hovered() {
                    painter.rect_filled(rect, 2.0, colors.bg_selection.gamma_multiply(0.6));
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
            match indicator {
                Indicator::Nothing => {}
                Indicator::Unsaved => {
                    painter.circle_filled(center, 4.0, colors.unsaved);
                }
                Indicator::Spinner => {
                    let angle = (now * 6.0) as f32;
                    let points: Vec<Pos2> = (0..12)
                        .map(|step| {
                            let t = angle + step as f32 * 0.4;
                            center + Vec2::new(t.cos(), t.sin()) * 5.0
                        })
                        .collect();
                    painter.add(egui::Shape::line(
                        points,
                        Stroke::new(1.5_f32, colors.fg_muted),
                    ));
                    ui.ctx().request_repaint();
                }
                Indicator::Failed => {
                    let top = center + Vec2::new(0.0, -6.0);
                    let left = center + Vec2::new(-6.0, 5.0);
                    let right = center + Vec2::new(6.0, 5.0);
                    painter.add(egui::Shape::convex_polygon(
                        vec![top, right, left],
                        colors.error,
                        Stroke::NONE,
                    ));
                    painter.text(
                        center + Vec2::new(0.0, 1.0),
                        Align2::CENTER_CENTER,
                        "!",
                        FontId::new(9.0, FontFamily::Proportional),
                        colors.bg_base,
                    );
                }
            }
            let tooltip = match indicator {
                Indicator::Unsaved => Some("status.unsaved"),
                Indicator::Spinner => Some("status.saving"),
                Indicator::Failed => Some("status.save_failed"),
                Indicator::Nothing => None,
            };
            let response = match tooltip {
                Some(key) => {
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, i18n::text(locale, key))
                    });
                    response.on_hover_text(i18n::text(locale, key))
                }
                None => response,
            };
            if response.clicked() && indicator == Indicator::Failed {
                clicked = true;
            }
            // 「閉じないで」（錠前の開いた形）。Dirty / Saving / SaveFailed のときだけ（§8.4）
            if session.bench.save.guards_close() {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Label,
                        i18n::text(locale, "status.dont_close"),
                    )
                });
                let painter = ui.painter_at(rect.expand(2.0));
                let color = if indicator == Indicator::Failed {
                    colors.error
                } else {
                    colors.fg_muted
                };
                let body = Rect::from_center_size(
                    rect.center() + Vec2::new(0.0, 2.5),
                    Vec2::new(9.0, 6.0),
                );
                painter.rect_stroke(body, 1.0, Stroke::new(1.2_f32, color));
                // 開いた掛け金（左だけ下りている）
                let top = body.min.y - 4.0;
                painter.line_segment(
                    [
                        Pos2::new(body.min.x + 2.0, body.min.y),
                        Pos2::new(body.min.x + 2.0, top),
                    ],
                    Stroke::new(1.2_f32, color),
                );
                painter.line_segment(
                    [
                        Pos2::new(body.min.x + 2.0, top),
                        Pos2::new(body.max.x - 2.0, top),
                    ],
                    Stroke::new(1.2_f32, color),
                );
                painter.line_segment(
                    [
                        Pos2::new(body.max.x - 2.0, top),
                        Pos2::new(body.max.x - 2.0, top + 2.0),
                    ],
                    Stroke::new(1.2_f32, color),
                );
                if indicator == Indicator::Failed {
                    ui.label(
                        RichText::new(i18n::text(locale, "status.dont_close"))
                            .font(font.clone())
                            .color(colors.error),
                    );
                }
            }
        });
    });
    clicked
}

/// The platform's primary modifier is Command on macOS, Control elsewhere.
pub(crate) fn modifier_hint_key(mods: Modifiers, mac: bool) -> Option<&'static str> {
    if !mods.command && !(mac && mods.mac_cmd) {
        return None;
    }
    Some(match (mac, mods.shift, mods.alt) {
        (true, true, _) => "status.shortcut_cmd_shift",
        (true, false, true) => "status.shortcut_cmd_alt",
        (true, false, false) => "status.shortcut_cmd",
        (false, true, _) => "status.shortcut_shift",
        (false, false, true) => "status.shortcut_alt",
        (false, false, false) => "status.shortcut_ctrl",
    })
}

/// 節エディタのミニヘッダー（§9.4.2 の ① の部分: パンくず ＋ 有効モードのバッジ。比率バーは ②）。
pub(crate) fn mini_header(ui: &mut egui::Ui, session: &Session, mode: Mode, scale: f32) {
    let colors = theme::palette(mode);
    let mode_color = theme::token_style(
        mode,
        crate::core::features::semantic_tokens::TokenType::ModifierMode,
    )
    .color;
    let font = FontId::new(11.0 * scale, FontFamily::Monospace);
    ui.horizontal(|ui| {
        ui.set_height(18.0);
        if let Some(crumb) = session.bench.breadcrumb() {
            ui.label(
                RichText::new(crumb)
                    .font(font.clone())
                    .color(colors.fg_faint),
            );
        }
        for cast in &session.reference_cache.content().cast {
            if cast.modes.is_empty() {
                continue;
            }
            let ids: Vec<&str> = cast
                .modes
                .iter()
                .map(|heading| heading.split(' ').next().unwrap_or(""))
                .collect();
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("{} {}", cast.name, ids.join("+")))
                    .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                    .color(mode_color),
            );
        }
    });
}

#[cfg(test)]
mod status_tests {
    use super::*;
    use crate::ui::app::validate_mcp_editor;

    #[test]
    fn mcp_editor_rejects_unknown_transport_before_save() {
        let input = r#"{"servers":{"remote":{"transport":"ftp","command":"run"}}}"#;
        assert!(validate_mcp_editor(input, Locale::Ja)
            .unwrap_err()
            .contains("ftp"));
        assert!(validate_mcp_editor(r#"{"servers":{}}"#, Locale::Ja).is_ok());
    }

    #[test]
    fn mac_modifiers_use_command_not_control_in_the_status_bar() {
        let command = Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(
            modifier_hint_key(command, true),
            Some("status.shortcut_cmd")
        );
        assert_eq!(
            modifier_hint_key(command | Modifiers::SHIFT, true),
            Some("status.shortcut_cmd_shift")
        );
        assert_eq!(
            modifier_hint_key(command | Modifiers::ALT, true),
            Some("status.shortcut_cmd_alt")
        );
        assert_eq!(modifier_hint_key(Modifiers::NONE, true), None);
    }
}
