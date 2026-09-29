//! 右: 情報パネル（カーソル位置からのコンテキスト推論）。
//!
//! 優先順位: カーソル行に直接含まれる要素 > 同一場面の要素 > 同一話の要素。

use crate::app::NovnowApp;
use egui::{Color32, RichText};

pub fn show(app: &mut NovnowApp, ui: &mut egui::Ui) {
    ui.heading("情報");
    ui.separator();

    if let Some(path) = app.hovered_path.clone() {
        ui.label(
            RichText::new(format!("ホバー: {}", short(&path)))
                .small()
                .weak(),
        );
        ui.separator();
    }

    if app.context_stale {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("更新待ち…").weak());
        });
    }

    let Some(info) = app.context.clone() else {
        ui.label("カーソルを本文へ移動してください。");
        return;
    };

    egui::ScrollArea::vertical()
        .id_source("info-panel")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ---- SceneId --------------------------------------------------
            if let Some(sid) = &info.scene_id {
                ui.label(RichText::new("現在の節").strong());
                ui.label(
                    RichText::new(sid.display())
                        .monospace()
                        .color(Color32::from_rgb(120, 200, 255)),
                );
                ui.separator();
            }

            // ---- 対応プロット（超詳細版） ---------------------------------
            if !info.plot_text.is_empty() {
                ui.label(RichText::new("対応プロット").strong());
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new(&info.plot_text).small());
                });
                ui.separator();
            }

            // ---- 登場キャラ -----------------------------------------------
            if !info.characters.is_empty() {
                ui.label(RichText::new("登場人物").strong());
                for c in &info.characters {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        let role = if c.role.is_empty() {
                            String::new()
                        } else {
                            format!("（{}）", c.role)
                        };
                        ui.label(
                            RichText::new(format!("{}{}", c.name, role))
                                .strong()
                                .color(Color32::from_rgb(200, 200, 130)),
                        );
                        if !c.speech.is_empty() {
                            ui.label(RichText::new(format!("口調: {}", c.speech)).small());
                        }
                        if !c.personality.is_empty() {
                            ui.label(RichText::new(format!("性格: {}", c.personality)).small());
                        }
                        if !c.notes.is_empty() {
                            ui.label(RichText::new(&c.notes).small().weak());
                        }
                        for (k, v) in &c.extra {
                            ui.label(RichText::new(format!("{k}: {v}")).small().weak());
                        }
                    });
                }
                ui.separator();
            }

            // ---- 用語 -----------------------------------------------------
            if !info.words.is_empty() {
                ui.label(RichText::new("用語").strong());
                for w in &info.words {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        let head = if w.reading.is_empty() {
                            w.term.clone()
                        } else {
                            format!("{}（{}）", w.term, w.reading)
                        };
                        ui.label(
                            RichText::new(head)
                                .strong()
                                .color(Color32::from_rgb(140, 200, 160)),
                        );
                        if !w.definition.is_empty() {
                            ui.label(RichText::new(&w.definition).small());
                        }
                        if !w.category.is_empty() {
                            ui.label(RichText::new(format!("#{}", w.category)).small().weak());
                        }
                    });
                }
                ui.separator();
            }

            // ---- 資料 -----------------------------------------------------
            if !info.materials.is_empty() {
                ui.label(RichText::new("関連資料").strong());
                for m in &info.materials {
                    ui.label(
                        RichText::new(format!("[{}] {}", m.status_kind().label(), m.path)).small(),
                    );
                    if !m.abs.is_empty() {
                        ui.label(RichText::new(&m.abs).small().weak());
                    }
                    if !m.url.is_empty() {
                        ui.hyperlink_to(m.url.clone(), m.url.clone());
                    }
                }
                ui.separator();
            }

            // ---- 起動時警告 ------------------------------------------------
            if !info.warnings.is_empty() {
                let header = RichText::new(format!("警告 ({})", info.warnings.len()))
                    .color(Color32::from_rgb(220, 170, 90));
                ui.collapsing(header, |ui| {
                    for wmsg in &info.warnings {
                        ui.label(
                            RichText::new(wmsg).small().color(Color32::from_rgb(220, 170, 90)),
                        );
                    }
                });
            }
        });
}

fn short(p: &std::path::Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}
