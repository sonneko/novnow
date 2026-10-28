//! 配役ワークスペースの「住所」タブ。登場人物ごとの住所を扱う（§10.6・§5.3.6）。
use super::tabs::{label, State};
use crate::core::features::metadata;
use crate::ui::{i18n::Locale, workbench::Workbench};
use egui::Ui;

pub(super) fn addresses_ui(
    ui: &mut Ui,
    state: &mut State,
    bench: &mut Workbench,
    locale: Locale,
    now: f64,
) {
    if state.address_generation != Some(bench.generation()) {
        state.address_cells.clear();
        state.address_generation = Some(bench.generation());
    }
    let parsed = bench.project().characters();
    let names: Vec<_> = parsed
        .characters
        .iter()
        .map(|c| c.name.to_owned())
        .collect();
    let addresses = metadata::addresses(bench.project().text("characters.md"));
    let mut changed = None;
    egui::ScrollArea::both().show(ui, |ui| {
        if names.len() > 24 {
            for address in &addresses {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} [{}] → {}",
                        address.speaker,
                        address.mode.as_deref().unwrap_or_default(),
                        address.target
                    ));
                    if address.mode.is_some() {
                        ui.label(address.names.join(", "));
                        return;
                    }
                    let key = (address.speaker.clone(), address.target.clone());
                    let cell = state
                        .address_cells
                        .entry(key.clone())
                        .or_insert_with(|| address.names.join(", "));
                    let response = ui.text_edit_singleline(cell);
                    if response.lost_focus() && *cell != address.names.join(", ") {
                        changed = Some((key, cell.clone()));
                    }
                });
            }
        } else {
            egui::Grid::new("address-matrix")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(label(locale, "stage.addresses"));
                    for target in &names {
                        ui.label(target);
                    }
                    ui.end_row();
                    for speaker in &names {
                        ui.label(speaker);
                        for target in &names {
                            if speaker == target {
                                ui.label("—");
                                continue;
                            }
                            let original = addresses
                                .iter()
                                .find(|a| {
                                    a.speaker == *speaker && a.target == *target && a.mode.is_none()
                                })
                                .map(|a| a.names.join(", "))
                                .unwrap_or_default();
                            let key = (speaker.clone(), target.clone());
                            let cell = state
                                .address_cells
                                .entry(key.clone())
                                .or_insert_with(|| original.clone());
                            let response =
                                ui.add(egui::TextEdit::singleline(cell).desired_width(85.0));
                            if response.lost_focus() && *cell != original {
                                changed = Some((key, cell.clone()));
                            }
                        }
                        ui.end_row();
                    }
                });
        }
    });
    if let Some(((speaker, target), names)) = changed {
        if let Some(text) = metadata::set_address(
            bench.project().text("characters.md"),
            &speaker,
            &target,
            &names,
        ) {
            bench.replace_document("characters.md", text, now);
        }
    }
}
