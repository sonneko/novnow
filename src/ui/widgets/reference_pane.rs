//! 参照ペイン（§9.5。① の 6 セクション: プロット / 上位プロット / 配役 / 用語 / 資料 / 診断）。
//!
//! - **`egui::Frame::group`（枠線付きカード）を使わない**（§9.5.1 MUST。`tests/layering.rs` が
//!   `src/ui/` に 0 件であることを検査する）。区切りは余白（12px）と見出しの字送り。
//! - 見出し 11px / `fg.faint`、本文 13px / `fg.primary` / 行間 1.6。
//! - 更新はカーソル移動から 80ms のデバウンス。待つ間も前の内容をそのまま出す（スピナーを出さない、§9.5.4 MUST）。

use egui::text::{LayoutJob, TextFormat};
use egui::{FontFamily, FontId, RichText, Sense, Stroke, Ui};

use crate::core::features::reference::{self, Reference};
use crate::core::resolve::context_rules::PaneSection;
use crate::ui::i18n::{self, Locale};
use crate::ui::theme::{self, Mode};
use crate::ui::workbench::Workbench;

/// デバウンス（§9.5.4）。
pub const DEBOUNCE_SECS: f64 = 0.08;

/// 参照ペインの中身と、更新待ち。
#[derive(Debug, Default)]
pub struct Cache {
    shown_key: Option<(u64, String, usize)>,
    pending: Option<((u64, String, usize), f64)>,
    content: Reference,
}

impl Cache {
    /// 今の節に合わせて中身を更新する（80ms 待ってから）。次に確かめる時刻を返す。
    pub fn update(&mut self, bench: &Workbench, now: f64) -> Option<f64> {
        let current = bench.current()?;
        let key = (
            bench.generation(),
            current.path.clone(),
            current.lines.start,
        );
        if self.shown_key.as_ref() == Some(&key) {
            self.pending = None;
            return None;
        }
        let since = match &self.pending {
            Some((pending, since)) if pending == &key => *since,
            _ => {
                self.pending = Some((key.clone(), now));
                now
            }
        };
        // 初回（何も出していない）は待たない
        if self.shown_key.is_some() && now - since < DEBOUNCE_SECS {
            return Some(since + DEBOUNCE_SECS);
        }
        if let Some(document) = bench.project().document(&current.path) {
            self.content =
                reference::at(document, current.lines.clone(), current.id, bench.project());
        }
        self.shown_key = Some(key);
        self.pending = None;
        None
    }

    /// 出している中身。
    pub fn content(&self) -> &Reference {
        &self.content
    }
}

/// 参照ペインを描く。資料の候補が選ばれたら（ダブルクリック・フォーカスして `Enter`）その ID。
pub fn show(ui: &mut Ui, cache: &Cache, locale: Locale, mode: Mode, scale: f32) -> Option<String> {
    let mut chosen = None;
    egui::ScrollArea::vertical()
        .id_source("reference-pane")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            chosen = sections(ui, cache.content(), locale, mode, scale);
        });
    chosen
}

/// 資料の候補の行の egui の ID（`Ctrl+3` でフォーカスを移す先）。
pub fn candidate_id(id: &str) -> egui::Id {
    egui::Id::new(("material-candidate", id))
}

/// 参照ペインの見出し（11px・`fg.faint`、上に 12px の余白）。ワークスペースが足す行にも使う。
pub fn heading(ui: &mut Ui, text: &str, mode: Mode, scale: f32) {
    let colors = theme::palette(mode);
    ui.add_space(12.0);
    ui.label(
        RichText::new(text)
            .font(FontId::new(11.0 * scale, FontFamily::Proportional))
            .color(colors.fg_faint),
    );
    ui.add_space(2.0);
}

/// 参照ペインの中身（スクロールの中に置く。M8 のワークスペースも同じ見た目で使う）。
///
/// 資料は明示参照の後に、条件一致の候補を薄い色で並べる（§16.4）。候補はダブルクリックか、
/// フォーカスして `Enter` で選べ、選ばれた候補の ID を返す（呼び出し側が今の節のプロットの
/// `material:` に足す、`PLANS.md` §9.2 B-136）。
pub fn sections(
    ui: &mut Ui,
    content: &Reference,
    locale: Locale,
    mode: Mode,
    scale: f32,
) -> Option<String> {
    let mut chosen = None;
    let colors = theme::palette(mode);
    let heading = |ui: &mut Ui, key: &str| {
        ui.add_space(12.0);
        ui.label(
            RichText::new(i18n::text(locale, key))
                .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                .color(colors.fg_faint),
        );
        ui.add_space(2.0);
    };
    let body_font = FontId::new(13.0 * scale, FontFamily::Proportional);
    let body = |ui: &mut Ui, text: &str| {
        let job = LayoutJob::single_section(
            text.to_owned(),
            TextFormat {
                font_id: body_font.clone(),
                color: colors.fg_primary,
                line_height: Some(13.0 * scale * 1.6),
                ..Default::default()
            },
        );
        ui.label(job);
    };
    let muted = |ui: &mut Ui, label: &str, text: &str| {
        let mut job = LayoutJob::default();
        let format = TextFormat {
            font_id: body_font.clone(),
            color: colors.fg_muted,
            line_height: Some(13.0 * scale * 1.6),
            ..Default::default()
        };
        job.append(label, 0.0, format.clone());
        job.append(
            text,
            12.0,
            TextFormat {
                color: colors.fg_primary,
                ..format
            },
        );
        ui.label(job);
    };
    {
        {
            // 表示順は固定（§9.5.2。`context_rules::PaneSection` の順）
            let order = [
                PaneSection::Plot,
                PaneSection::UpperPlot,
                PaneSection::Cast,
                PaneSection::Words,
                PaneSection::Materials,
                PaneSection::Diagnostics,
            ];
            for section in order {
                match section {
                    PaneSection::Plot if !content.plot.is_empty() => {
                        heading(ui, "pane.plot");
                        for line in &content.plot {
                            body(ui, line);
                        }
                    }
                    PaneSection::UpperPlot if !content.upper.is_empty() => {
                        heading(ui, "pane.upper");
                        for upper in &content.upper {
                            muted(ui, &upper.id, &upper.summary);
                        }
                    }
                    PaneSection::Cast if !content.cast.is_empty() => {
                        heading(ui, "pane.cast");
                        for cast in &content.cast {
                            muted(ui, &cast.name, &cast.modes.join(" + "));
                            if let Some(text) = &cast.personality {
                                muted(ui, i18n::text(locale, "pane.personality"), first_line(text));
                            }
                            if let Some(text) = &cast.emotion {
                                muted(ui, i18n::text(locale, "pane.emotion"), first_line(text));
                            }
                            if let Some(text) = &cast.role {
                                muted(ui, i18n::text(locale, "pane.role"), first_line(text));
                            }
                            if !cast.tone.is_empty() {
                                let tone: Vec<String> =
                                    cast.tone.iter().map(|tone| format!("「{tone}」")).collect();
                                muted(ui, i18n::text(locale, "pane.tone"), &tone.join(""));
                            }
                            ui.add_space(4.0);
                        }
                    }
                    PaneSection::Words if !content.words.is_empty() => {
                        heading(ui, "pane.words");
                        for word in &content.words {
                            muted(ui, &word.term, &word.definition);
                        }
                    }
                    PaneSection::Materials if !content.materials.is_empty() => {
                        heading(ui, "pane.materials");
                        for material in content.materials.iter().filter(|found| !found.candidate) {
                            muted(ui, &material.id, &material.title);
                            if !material.summary.is_empty() {
                                body(ui, &material.summary);
                            }
                        }
                        for material in content.materials.iter().filter(|found| found.candidate) {
                            if candidate(ui, material, locale, mode, scale) {
                                chosen = Some(material.id.clone());
                            }
                        }
                    }
                    PaneSection::Diagnostics if !content.diagnostics.is_empty() => {
                        heading(ui, "pane.diagnostics");
                        for found in &content.diagnostics {
                            let color = match found.severity() {
                                crate::core::diagnostics::Severity::Error => colors.error,
                                _ => colors.warn,
                            };
                            let mut job = LayoutJob::default();
                            job.append(
                                found.code.as_str(),
                                0.0,
                                TextFormat {
                                    font_id: FontId::new(11.0 * scale, FontFamily::Monospace),
                                    color,
                                    ..Default::default()
                                },
                            );
                            job.append(
                                &i18n::diagnostic(locale, found),
                                8.0,
                                TextFormat {
                                    font_id: body_font.clone(),
                                    color: colors.fg_primary,
                                    ..Default::default()
                                },
                            );
                            ui.label(job);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    chosen
}

/// 条件一致の候補の 1 行（`fg.faint`。ダブルクリック・フォーカスして `Enter` で `true`）。
fn candidate(
    ui: &mut Ui,
    material: &reference::MaterialRef,
    locale: Locale,
    mode: Mode,
    scale: f32,
) -> bool {
    let colors = theme::palette(mode);
    let font = FontId::new(13.0 * scale, FontFamily::Proportional);
    let mut job = LayoutJob::default();
    let format = TextFormat {
        font_id: font,
        color: colors.fg_faint,
        line_height: Some(13.0 * scale * 1.6),
        ..Default::default()
    };
    job.append(&material.id, 0.0, format.clone());
    job.append(&material.title, 12.0, format);
    let label = ui.label(job);
    let response = ui.interact(label.rect, candidate_id(&material.id), Sense::click());
    let name = format!("{} {}", material.id, material.title);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, &name));
    if response.has_focus() || response.hovered() {
        ui.painter().rect_stroke(
            label.rect.expand(2.0),
            2.0,
            Stroke::new(1.0_f32, colors.accent),
        );
    }
    let response = response.on_hover_text(i18n::text(locale, "pane.candidate_hint"));
    response.double_clicked() || (response.clicked() && response.fake_primary_click)
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}
