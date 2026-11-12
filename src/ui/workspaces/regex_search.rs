//! 執筆画面の「正規表現検索」タブ（§9.7.4・§16.3.1）。
use super::tabs::{label, State};
use crate::core::features::search;
use crate::ui::{
    i18n::{self, Locale},
    workbench::Workbench,
};
use egui::Ui;

pub(super) fn search_ui(
    ui: &mut Ui,
    state: &mut State,
    bench: &mut Workbench,
    locale: Locale,
    now: f64,
    jump: &mut Option<(String, usize)>,
) {
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::R)) {
        state.query.regex = !state.query.regex;
    }
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::C)) {
        state.query.case_sensitive = !state.query.case_sensitive;
    }
    let mut just_searched = false;
    ui.horizontal(|ui| {
        let query_edit = ui.text_edit_singleline(&mut state.query.text);
        if state.focus_search_input {
            query_edit.request_focus();
            state.focus_search_input = false;
        }
        let run_enter = query_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.checkbox(&mut state.query.regex, label(locale, "stage.regex"));
        ui.checkbox(&mut state.query.case_sensitive, label(locale, "stage.case"));
        if ui.button(label(locale, "stage.search")).clicked() || run_enter {
            let _ = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            just_searched = true;
            bench.commit();
            let start = web_time::Instant::now();
            state.results = Some(search::run(bench.project(), &state.query, || {
                start.elapsed().as_millis() as u64
            }));
            state.confirmed = false;
            state.save_search_open = false;
            state.tab_selected = 0;
        }
    });
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.replacement);
        if ui.button(label(locale, "stage.replace_all")).clicked() {
            state.confirmed = true;
        }
    });
    if state.confirmed {
        let mut counts: Option<(usize, usize)> = None;
        if let Some(Ok(results)) = &state.results {
            match search::replacement(bench.project(), &state.query, &state.replacement, results) {
                Ok(edit) => {
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .id_source("replacement-preview")
                        .show(ui, |ui| {
                            preview(ui, locale, bench, &edit.changes);
                        });
                    let files = edit.changes.len();
                    let places: usize = edit.changes.iter().map(|file| file.edits.len()).sum();
                    counts = Some((files, places));
                }
                Err(error) => {
                    ui.label(error);
                    state.confirmed = false;
                }
            }
        }
        ui.horizontal(|ui| {
            match counts {
                Some((files, places)) => {
                    ui.label(i18n::format(
                        locale,
                        "stage.confirm_replace",
                        &[&files.to_string(), &places.to_string()],
                    ));
                }
                None => {
                    ui.label(label(locale, "stage.confirm_replace"));
                }
            }
            if ui.button(label(locale, "stage.confirm")).clicked() {
                if let Some(Ok(results)) = &state.results {
                    if let Ok(edit) = search::replacement(
                        bench.project(),
                        &state.query,
                        &state.replacement,
                        results,
                    ) {
                        bench.apply_edit(&edit, now);
                    }
                }
                state.results = None;
                state.confirmed = false;
            }
            if ui.button(label(locale, "stage.cancel")).clicked() {
                state.confirmed = false;
            }
        });
    }

    let modal_was_open = state.save_search_open;
    if let Some(Ok(results)) = &state.results {
        if ui.button(label(locale, "stage.save_search")).clicked() && !state.save_search_open {
            state.save_search_open = true;
            state.save_search_title = state.query.text.clone();
            state.save_search_summary.clear();
            state.save_search_fresh = true;
        }
        if state.save_search_open {
            let mut commit_save = false;
            let mut cancel_save =
                ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            egui::Window::new(label(locale, "stage.save_search_title"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    ui.label(label(locale, "stage.save_search_name"));
                    let title_resp = ui.add(
                        egui::TextEdit::singleline(&mut state.save_search_title)
                            .id(egui::Id::new("save-search-title"))
                            .desired_width(320.0),
                    );
                    if state.save_search_fresh {
                        title_resp.request_focus();
                        state.save_search_fresh = false;
                    }
                    ui.label(label(locale, "stage.save_search_summary"));
                    let sum_resp = ui.add(
                        egui::TextEdit::singleline(&mut state.save_search_summary)
                            .id(egui::Id::new("save-search-summary"))
                            .desired_width(320.0),
                    );
                    let enter_pressed = (title_resp.lost_focus() || sum_resp.lost_focus())
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if ui
                            .button(label(locale, "stage.save_search_confirm"))
                            .clicked()
                            || enter_pressed
                        {
                            commit_save = true;
                        }
                        if ui.button(label(locale, "stage.cancel")).clicked() {
                            cancel_save = true;
                        }
                    });
                });
            if cancel_save {
                state.save_search_open = false;
                state.save_search_summary.clear();
            } else if commit_save {
                if let Ok(created) = search::snapshot_with(
                    bench.project(),
                    &state.query,
                    results,
                    &state.stamp,
                    &state.save_search_title,
                    &state.save_search_summary,
                ) {
                    bench.replace_document(&created.path, created.body, now);
                    bench.replace_document("materials/index.json", created.index, now);
                }
                state.save_search_open = false;
                state.save_search_summary.clear();
            }
        }
    }

    match &state.results {
        Some(Err(error)) => {
            ui.label(error);
        }
        Some(Ok(results)) => {
            if results.timed_out {
                ui.label(i18n::diagnostic(
                    locale,
                    &crate::core::diagnostics::Diagnostic::new(
                        crate::core::diagnostics::DiagnosticCode::W094,
                        None,
                        vec![],
                    ),
                ));
            }
            let mut targets = Vec::with_capacity(results.hits.len());
            egui::ScrollArea::vertical().show(ui, |ui| {
                let font = ui
                    .style()
                    .text_styles
                    .get(&egui::TextStyle::Button)
                    .cloned()
                    .unwrap_or_else(|| egui::FontId::proportional(13.0));
                let link = ui.visuals().hyperlink_color;
                let color = ui.visuals().text_color();
                let match_bg = ui.visuals().selection.bg_fill;
                for (row_idx, hit) in results.hits.iter().enumerate() {
                    // 抜粋は前文・一致・後文の構造で受け、画面では一致だけを強調する。
                    let mut job = egui::text::LayoutJob::default();
                    crate::ui::widgets::append_text(
                        &mut job,
                        &format!("{}  ", hit.path),
                        &font,
                        link,
                    );
                    crate::ui::widgets::append_excerpt(
                        &mut job,
                        &hit.before,
                        &hit.matched,
                        &hit.after,
                        &font,
                        color,
                        match_bg,
                    );
                    let target = (hit.path.clone(), hit.range.start);
                    let selected = row_idx == state.tab_selected;
                    let resp = crate::ui::widgets::selectable_job(ui, selected, job);
                    let enter = resp.has_focus()
                        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                    if resp.clicked() || enter {
                        state.tab_selected = row_idx;
                        *jump = Some(target.clone());
                    }
                    targets.push(target);
                }
            });
            if !state.save_search_open && !modal_was_open && !just_searched {
                super::tabs::handle_list_keys(ui, &mut state.tab_selected, &targets, jump);
            }
        }
        None => {}
    }
}

/// 置換のプレビュー（ファイルごとに変更前・変更後）を畳んで出す（§9.7.4）。
fn preview(
    ui: &mut Ui,
    locale: Locale,
    bench: &Workbench,
    changes: &[crate::core::features::FileEdit],
) {
    for file in changes {
        ui.collapsing(&file.path, |ui| {
            ui.label(label(locale, "stage.before"));
            ui.monospace(bench.project().text(&file.path));
            ui.label(label(locale, "stage.after"));
            ui.monospace(crate::core::features::apply_edits(
                bench.project().text(&file.path),
                &file.edits,
            ));
        });
    }
}
