//! Regex search tab of the write workspace (design doc `workflows` §16.3).
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
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.query.text);
        ui.checkbox(&mut state.query.regex, label(locale, "stage.regex"));
        ui.checkbox(&mut state.query.case_sensitive, label(locale, "stage.case"));
        if ui.button(label(locale, "stage.search")).clicked() {
            bench.commit();
            let start = web_time::Instant::now();
            state.results = Some(search::run(bench.project(), &state.query, || {
                start.elapsed().as_millis() as u64
            }));
            state.confirmed = false;
        }
    });
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.replacement);
        if ui.button(label(locale, "stage.replace_all")).clicked() {
            state.confirmed = true;
        }
    });
    if state.confirmed {
        if let Some(Ok(results)) = &state.results {
            match search::replacement(bench.project(), &state.query, &state.replacement, results) {
                Ok(edit) => {
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .id_source("replacement-preview")
                        .show(ui, |ui| {
                            for file in edit.changes {
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
                        });
                }
                Err(error) => {
                    ui.label(error);
                    state.confirmed = false;
                }
            }
        }
        ui.horizontal(|ui| {
            ui.label(label(locale, "stage.confirm_replace"));
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
    if let Some(Ok(results)) = &state.results {
        if ui.button(label(locale, "stage.save_search")).clicked() {
            if let Ok(created) =
                search::snapshot(bench.project(), &state.query, results, &state.stamp)
            {
                bench.replace_document(&created.path, created.body, now);
                bench.replace_document("materials/index.json", created.index, now);
            }
        }
    }
    let saved: Vec<_> =
        crate::core::features::materials_ops::items(bench.project().text("materials/index.json"))
            .into_iter()
            .filter(|m| m.source.kind == "search")
            .collect();
    for material in saved {
        if ui
            .button(format!("{} {}", label(locale, "stage.rerun"), material.id))
            .clicked()
        {
            let query = search::Query {
                text: material.source.query.clone().unwrap_or_default(),
                regex: material
                    .source
                    .unknown
                    .get("regex")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                case_sensitive: material
                    .source
                    .unknown
                    .get("case_sensitive")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            };
            let clock = web_time::Instant::now();
            if let Ok(results) = search::run(bench.project(), &query, || {
                clock.elapsed().as_millis() as u64
            }) {
                if let Ok(snapshot) =
                    search::snapshot(bench.project(), &query, &results, &state.stamp)
                {
                    state.local_result = format!(
                        "− {}\n+ {}",
                        bench.project().text(&material.path),
                        snapshot.body
                    );
                }
            }
        }
    }
    if !state.local_result.is_empty() {
        ui.collapsing(label(locale, "stage.diff"), |ui| {
            ui.monospace(&state.local_result);
        });
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
                for hit in &results.hits {
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
                    if crate::ui::widgets::selectable_job(ui, false, job).clicked() {
                        *jump = Some((hit.path.clone(), hit.range.start));
                    }
                }
            });
        }
        None => {}
    }
}
