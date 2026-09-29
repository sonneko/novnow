//! 下左: 編集領域（単一の TextEdit + 行番号 + 下部の節ピンポイント編集バー）。
//!
//! 対象 API: egui 0.27 系。
//! * `TextEditOutput { response: Response, cursor_range: Option<CursorRange>, .. }`
//! * `CursorRange { primary: Cursor, .. }` / `Cursor { ccursor: CCursor, .. }`
//! * `CCursor { index: usize, .. }`

use crate::app::NovnowApp;
use crate::text::buffer::TextBuffer;
use egui::{Color32, RichText};

/// エディタ本体 + 下部の「数式バー」（Excel のセル編集に相当）。
pub fn show(app: &mut NovnowApp, ui: &mut egui::Ui) {
    ui.heading("エディタ");
    ui.separator();

    // 下部バー用の状態を先に取り出しておく。
    let focus_line = app.editor_focus_line;
    let mut bar_edit = app.editor_bar_text.clone();

    // 利用可能領域から下部バーの高さを引く。
    let total_h = ui.available_height();
    let bar_h = 64.0;
    let editor_h = (total_h - bar_h).max(80.0);

    // ---- エディタ本体 -----------------------------------------------------
    egui::Frame::none()
        .inner_margin(egui::Margin::same(4.0))
        .show(ui, |ui| {
            ui.set_height(editor_h);
            egui::ScrollArea::both()
                .id_source("editor-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        // 行番号（Rope の行インデックスをそのまま利用）。
                        let (line_count, cursor_line) = app
                            .active_buffer()
                            .map(|b| (b.line_count(), b.cursor_line))
                            .unwrap_or((0, 0));
                        let mut gutter = String::new();
                        for i in 0..line_count.max(1) {
                            let marker = if i == cursor_line { '>' } else { ' ' };
                            gutter.push_str(&format!("{marker}{:>5}\n", i + 1));
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new(gutter)
                                    .monospace()
                                    .color(Color32::from_gray(110)),
                            ),
                        );

                        // 本体の TextEdit。
                        let Some(path) = app.active_path.clone() else {
                            ui.label("ファイルを選択してください（ファイルツリーで上下 + Enter）。");
                            return;
                        };

                        let mut new_cursor: Option<(usize, usize)> = None;
                        let mut new_selection: Option<(usize, usize)> = None;
                        let mut cursor_moved = false;
                        let mut changed = false;

                        // このブロック内でのみ `app.buffers` を可変借用する。
                        {
                            let Some(buffer) = app.buffers.get_mut(&path) else {
                                return;
                            };
                            // TextEdit が要求する &mut String を再利用バッファから供給する。
                            let text_ref = buffer.as_edit_str();

                            let output = egui::TextEdit::multiline(text_ref)
                                .font(egui::TextStyle::Monospace)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(30)
                                .id_source(("editor-main", path.to_string_lossy().to_string()))
                                .show(ui);

                            if output.response.changed() {
                                changed = true;
                            }
                            if let Some(range) = output.cursor_range {
                                let p_idx = range.primary.ccursor.index;
                                let s_idx = range.secondary.ccursor.index;
                                let (line, col) = char_to_line_col(buffer, p_idx);
                                if line != buffer.cursor_line || col != buffer.cursor_col {
                                    cursor_moved = true;
                                }
                                new_cursor = Some((line, col));

                                if p_idx != s_idx {
                                    let min = p_idx.min(s_idx);
                                    let max = p_idx.max(s_idx);
                                    new_selection = Some((min, max));
                                } else {
                                    new_selection = None;
                                }
                            }
                        }

                        // 借用が切れたので、状態を反映する。
                        if let Some(buf) = app.buffers.get_mut(&path) {
                            if let Some((line, col)) = new_cursor {
                                buf.cursor_line = line;
                                buf.cursor_col = col;
                            }
                            buf.selection_range = new_selection;
                            if changed {
                                // 変更行のみ再トークン化する。
                                buf.sync_from_scratch();
                            }
                        }

                        if cursor_moved {
                            app.sync_editor_focus();
                            app.mark_context_dirty();
                        } else if changed {
                            app.mark_context_dirty();
                        }
                    });
                });
        });

    // 節単位スクロール（Alt+↑ / Alt+↓）。
    // フォーカス移動の Ctrl+↑↓ や入力と衝突しないよう Alt を要求する。
    if app.focus == super::FocusPane::Editor {
        let (up, down) = ui.ctx().input(|i| {
            let alt = i.modifiers.alt;
            (
                alt && i.key_pressed(egui::Key::ArrowUp),
                alt && i.key_pressed(egui::Key::ArrowDown),
            )
        });
        if up {
            app.jump_section(-1);
        } else if down {
            app.jump_section(1);
        }
    }

    // ---- 下部の節ピンポイント編集バー -------------------------------------
    ui.separator();
    ui.horizontal(|ui| {
        let sid_label = app
            .current_scene_id()
            .map(|s| s.display())
            .unwrap_or_else(|| "—".to_string());
        ui.label(RichText::new(format!("節 {sid_label}")).strong());
        ui.label(RichText::new("(確定: Enter)").small().weak());
        ui.label(
            RichText::new(format!("対象行: {}", focus_line + 1))
                .small()
                .weak(),
        );

    });

    let response = ui.add(
        egui::TextEdit::singleline(&mut bar_edit)
            .font(egui::TextStyle::Monospace)
            .desired_width(f32::INFINITY)
            .hint_text("選択行をここで直接編集"),
    );
    app.editor_bar_text = bar_edit;

    // Enter で確定する（Excel のセル編集と同じメンタルモデル）。
    let commit = response.has_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter));
    if commit {
        app.commit_bar_edit();
        ui.ctx().memory_mut(|m| m.surrender_focus(response.id));
    }
}

/// char index → (line, col)。
fn char_to_line_col(buf: &TextBuffer, idx: usize) -> (usize, usize) {
    let rope = &buf.rope;
    let idx = idx.min(rope.len_chars());
    let line = rope.char_to_line(idx);
    let line_start = rope.line_to_char(line);
    (line, idx - line_start)
}
