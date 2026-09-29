//! 上左: ファイルリーダー（ファイルツリー + 読み取り専用ビューア）。

use crate::app::NovnowApp;
use egui::{Color32, RichText};

/// ファイルツリーの UI。
///
/// * 上下キーで行移動、Enter で開く
/// * 選択中ファイルはハイライト
/// * ホバー行は `hovered_tree_row` に記録し、右パネルに情報表示
pub fn show(app: &mut NovnowApp, ui: &mut egui::Ui) {
    ui.heading("ファイル");
    ui.separator();

    let rows = std::mem::take(&mut app.tree_rows);
    let mut open_request: Option<std::path::PathBuf> = None;
    let mut move_delta: i32 = 0;

    egui::ScrollArea::vertical()
        .id_source("file-tree")
        .show(ui, |ui| {
            for (i, row) in rows.iter().enumerate() {
                let selected = app
                    .active_path
                    .as_ref()
                    .map(|p| row.path.as_ref() == Some(p))
                    .unwrap_or(false);
                let hovered_now = app.hovered_tree_row == Some(i);

                let label = if row.is_dir {
                    RichText::new(format!("{} {}", "=".repeat(row.depth + 1), row.label))
                        .strong()
                        .color(Color32::from_rgb(160, 160, 200))
                } else if selected {
                    RichText::new(format!("> {}", row.label))
                        .strong()
                        .color(Color32::from_rgb(120, 200, 255))
                } else if hovered_now {
                    RichText::new(format!("  {}", row.label))
                        .color(Color32::from_rgb(200, 200, 220))
                } else {
                    RichText::new(format!("  {}", row.label))
                        .color(Color32::from_rgb(150, 150, 160))
                };

                let response = if row.is_dir {
                    ui.label(label)
                } else {
                    ui.selectable_label(selected, label)
                };

                if response.hovered() {
                    app.hovered_tree_row = Some(i);
                    app.hovered_path = row.path.clone();
                }

                if response.clicked() && row.path.is_some() {
                    open_request = row.path.clone();
                }
            }
        });

    // ツリーペインがフォーカス中のときのキーボード操作。
    if app.focus == super::FocusPane::Tree {
        let (down, up, enter) = ui.ctx().input(|i| {
            (
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::Enter),
            )
        });
        if down {
            move_delta = 1;
        } else if up {
            move_delta = -1;
        } else if enter {
            if let Some(sel) = app.tree_selection {
                if let Some(p) = rows.get(sel).and_then(|r| r.path.clone()) {
                    open_request = Some(p);
                }
            }
        }
    }

    if move_delta != 0 && !rows.is_empty() {
        let len = rows.len() as i32;
        let cur = app.tree_selection.map(|v| v as i32).unwrap_or(-1);
        let next = (cur + move_delta).clamp(0, len - 1) as usize;
        app.tree_selection = Some(next);
    }

    app.tree_rows = rows;

    if let Some(p) = open_request {
        app.open_file(&p);
    }

    // 選択行の追従（アクティブファイルが変わったら選択を合わせる）。
    if app.tree_selection.is_none() {
        if let Some(active) = app.active_path.clone() {
            if let Some(idx) = app
                .tree_rows
                .iter()
                .position(|r| r.path.as_ref() == Some(&active))
            {
                app.tree_selection = Some(idx);
            }
        }
    }
}

/// ツリー下部の「選択中ファイルの内容ビューア（薄くハイライト付き）」。
///
/// 節スクロールでカーソル行が移動し、その行が薄くハイライトされる。
pub fn show_viewer(app: &mut NovnowApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.label(
        RichText::new("プレビュー（節スクロール: Alt+↑ / Alt+↓）")
            .small()
            .weak(),
    );

    let (text, cursor_line) = match app.active_buffer() {
        Some(buf) => (buf.rope.to_string(), buf.cursor_line),
        None => {
            ui.label("(ファイル未選択)");
            return;
        }
    };

    let lines: Vec<&str> = text.lines().collect();
    let row_height = ui.text_style_height(&egui::TextStyle::Body);

    egui::ScrollArea::vertical()
        .id_source("file-viewer")
        .show_rows(ui, row_height, lines.len(), |ui, range| {
            for i in range {
                let line = lines[i];
                let is_cursor = i == cursor_line;
                let is_header = line.trim_start().starts_with('@');
                let txt = if is_header {
                    RichText::new(line)
                        .strong()
                        .color(Color32::from_rgb(140, 200, 255))
                } else if is_cursor {
                    RichText::new(line).background_color(Color32::from_rgb(50, 60, 80))
                } else {
                    RichText::new(line)
                };
                ui.label(txt);
            }
        });
}
