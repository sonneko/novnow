//! 配役ワークスペースの「呼称表」タブ（§10.6.1・§5.5.6・§6.5）。
use super::tabs::{handle_list_keys, label, nav_link, State};
use crate::core::features::{cast, metadata};
use crate::ui::{
    i18n::{self, Locale},
    workbench::Workbench,
};
use egui::{Key, Modifiers, RichText, Ui};

/// `characters.md` 内に登場する全モード ID を順序を保って集める。
fn all_modes(bench: &Workbench) -> Vec<String> {
    let mut out = Vec::new();
    for line in bench.project().text("characters.md").lines() {
        if let Some(rest) = line.strip_prefix("### ") {
            if let Some(id) = rest.split_whitespace().next() {
                if !id.is_empty() && !out.iter().any(|m| m == id) {
                    out.push(id.to_owned());
                }
            }
        }
    }
    out
}

/// セルに表示する代表呼称（複数あるときは `めぐみ +1`、`* =>` 既定は淡色フラグ `true`）。
fn cell_preview(
    addresses: &[metadata::Address],
    speaker: &str,
    mode: Option<&str>,
    target: &str,
) -> (String, bool, Vec<String>) {
    if let Some(exact) = addresses
        .iter()
        .find(|a| a.speaker == speaker && a.target == target && a.mode.as_deref() == mode)
    {
        let disp = match exact.names.as_slice() {
            [] => String::new(),
            [first] => first.clone(),
            [first, rest @ ..] => format!("{first} +{}", rest.len()),
        };
        return (disp, false, exact.names.clone());
    }
    let wildcard = addresses
        .iter()
        .filter(|a| {
            a.speaker == speaker
                && a.target == "*"
                && (a.mode.as_deref() == mode || a.mode.is_none())
        })
        .max_by_key(|a| a.mode.is_some());
    if let Some(wild) = wildcard {
        let disp = wild.names.first().cloned().unwrap_or_default();
        return (disp, true, Vec::new());
    }
    (String::new(), false, Vec::new())
}

pub(super) fn addresses_ui(
    ui: &mut Ui,
    state: &mut State,
    bench: &mut Workbench,
    locale: Locale,
    now: f64,
    jump: &mut Option<(String, usize)>,
) {
    let cast_rows = cast::characters(bench.project());
    let names: Vec<String> = cast_rows.into_iter().map(|c| c.name).collect();
    if names.is_empty() {
        ui.label(i18n::text(locale, "cast.none"));
        return;
    }

    // 初期選択ペアの補完
    let valid_pair = state
        .address_selected_pair
        .as_ref()
        .is_some_and(|(s, t)| s != t && names.contains(s) && names.contains(t));
    if !valid_pair && names.len() >= 2 {
        state.address_selected_pair = Some((names[0].clone(), names[1].clone()));
    }
    if !names.contains(&state.address_speaker_filter) {
        if let Some(first) = names.first() {
            state.address_speaker_filter = first.clone();
        }
    }

    let modes = all_modes(bench);
    if let Some(cur_mode) = &state.address_mode {
        if !modes.contains(cur_mode) {
            state.address_mode = None;
        }
    }

    // 上部バー: モードセレクタ + Ctrl+Shift+A 候補抽出ボタン
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(
                state.address_mode.is_none(),
                label(locale, "stage.addr_mode_base"),
            )
            .clicked()
        {
            state.address_mode = None;
            state.address_editing = None;
        }
        for m in &modes {
            let selected = state.address_mode.as_deref() == Some(m.as_str());
            if ui.selectable_label(selected, format!(":{m}")).clicked() {
                state.address_mode = Some(m.clone());
                state.address_editing = None;
            }
        }
        ui.separator();
        if ui
            .button(label(locale, "stage.addr_extract_button"))
            .clicked()
        {
            state.extract_address_candidates(bench.project());
        }
    });
    ui.label(
        RichText::new(label(locale, "stage.addr_edit_hint"))
            .small()
            .color(ui.visuals().weak_text_color()),
    );

    // 候補抽出パネル（自動書き込みはせず、人間が選んで登録する MUST）
    if state.address_candidates_open {
        ui.separator();
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.strong(label(locale, "stage.addr_candidates_heading"));
                if ui.button(label(locale, "stage.cancel")).clicked() {
                    state.address_candidates_open = false;
                }
            });
            if state.address_candidates.is_empty() {
                ui.label(label(locale, "stage.addr_candidates_empty"));
            } else {
                let mut register_indices = Vec::new();
                for (idx, (cand, checked)) in state.address_candidates.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(checked, "");
                        let mode_tag = cand
                            .mode
                            .as_deref()
                            .map_or_else(String::new, |m| format!(":{m}"));
                        ui.label(format!(
                            "{}{mode_tag} → {}: {}  (×{})",
                            cand.speaker, cand.target, cand.surface, cand.count
                        ));
                        if ui
                            .small_button(label(locale, "stage.addr_register_one"))
                            .clicked()
                        {
                            register_indices.push(idx);
                        }
                    });
                }
                if ui
                    .button(label(locale, "stage.addr_register_selected"))
                    .clicked()
                {
                    for (idx, (_, checked)) in state.address_candidates.iter().enumerate() {
                        if *checked {
                            register_indices.push(idx);
                        }
                    }
                }
                if !register_indices.is_empty() {
                    register_indices.sort_unstable();
                    register_indices.dedup();
                    let mut text = bench.project().text("characters.md").to_owned();
                    let mut changed = false;
                    for &idx in &register_indices {
                        if let Some((cand, _)) = state.address_candidates.get(idx) {
                            if let Some(updated) = metadata::add_address_candidate(
                                &text,
                                &cand.speaker,
                                cand.mode.as_deref(),
                                &cand.target,
                                &cand.surface,
                            ) {
                                text = updated;
                                changed = true;
                            }
                        }
                    }
                    if changed {
                        bench.replace_document("characters.md", text, now);
                        state.extract_address_candidates(bench.project());
                    }
                }
            }
        });
    }

    let addresses = metadata::addresses(bench.project().text("characters.md"));
    let active_mode = state.address_mode.clone();

    // 非編集中かつテキスト入力中でないとき、違反行フォーカス中でなければ Enter でセルのインライン編集を開始する
    let start_edit_by_key = state.address_editing.is_none()
        && !state.address_focus_violations
        && !ui.ctx().wants_keyboard_input()
        && state.address_selected_pair.is_some()
        && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
    if start_edit_by_key {
        if let Some((sp, tg)) = state.address_selected_pair.clone() {
            let (_, _, exact_names) = cell_preview(&addresses, &sp, active_mode.as_deref(), &tg);
            state.address_editing = Some((sp, active_mode.clone(), tg));
            state.address_edit_buffer = exact_names.join(", ");
            state.address_edit_fresh = true;
        }
    }

    let mut commit_edit: Option<(String, Option<String>, String, String)> = None;
    let mut cancel_edit = false;

    egui::ScrollArea::both()
        .id_source("address-matrix-scroll")
        .max_height(280.0)
        .show(ui, |ui| {
            if names.len() > 24 {
                // n > 24: 呼ぶ人を 1 人選んで縦に並べる（§10.6.1 MUST）
                ui.horizontal_wrapped(|ui| {
                    ui.label(label(locale, "stage.addr_speaker_select"));
                    for sp in &names {
                        if ui
                            .selectable_label(&state.address_speaker_filter == sp, sp)
                            .clicked()
                        {
                            state.address_speaker_filter = sp.clone();
                            state.address_editing = None;
                        }
                    }
                });
                let speaker = state.address_speaker_filter.clone();
                egui::Grid::new("address-single-speaker")
                    .striped(true)
                    .show(ui, |ui| {
                        for target in &names {
                            if &speaker == target {
                                continue;
                            }
                            ui.label(target);
                            render_cell(
                                ui,
                                state,
                                &addresses,
                                &speaker,
                                active_mode.as_deref(),
                                target,
                                &mut commit_edit,
                                &mut cancel_edit,
                            );
                            ui.end_row();
                        }
                    });
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
                                render_cell(
                                    ui,
                                    state,
                                    &addresses,
                                    speaker,
                                    active_mode.as_deref(),
                                    target,
                                    &mut commit_edit,
                                    &mut cancel_edit,
                                );
                            }
                            ui.end_row();
                        }
                    });
            }
        });

    if cancel_edit {
        state.address_editing = None;
        state.address_edit_fresh = false;
    } else if let Some((speaker, mode, target, new_names)) = commit_edit {
        if let Some(text) = metadata::set_address_with_mode(
            bench.project().text("characters.md"),
            &speaker,
            mode.as_deref(),
            &target,
            &new_names,
        ) {
            bench.replace_document("characters.md", text, now);
        }
        state.address_editing = None;
        state.address_edit_fresh = false;
    }

    ui.separator();

    // 選択中ペアの W084 違反箇所一覧（クリック / Alt+↑↓ + Enter で該当節へ、§10.6.1）
    if let Some((sel_sp, sel_tg)) = state.address_selected_pair.clone() {
        ui.strong(i18n::format(
            locale,
            "stage.addr_violations",
            &[&sel_sp, &sel_tg],
        ));
        let violations: Vec<_> = metadata::address_violations(bench.project())
            .into_iter()
            .filter(|v| {
                v.speaker == sel_sp
                    && v.target == sel_tg
                    && (active_mode.is_none() || v.mode == active_mode)
            })
            .collect();
        if violations.is_empty() {
            ui.label(
                RichText::new(label(locale, "stage.addr_no_violations"))
                    .color(ui.visuals().weak_text_color()),
            );
        } else {
            let mut targets = Vec::with_capacity(violations.len());
            egui::ScrollArea::vertical()
                .id_source("address-violations-scroll")
                .max_height(150.0)
                .show(ui, |ui| {
                    for (idx, v) in violations.iter().enumerate() {
                        let target = (v.path.clone(), v.byte);
                        let place = v.scene.map_or_else(
                            || v.path.clone(),
                            |id| format!("@{}", id.display(bench.project().digits())),
                        );
                        if nav_link(
                            ui,
                            idx,
                            state.address_violation_selected,
                            format!("{place}  {}", v.line_text),
                        ) {
                            state.address_violation_selected = idx;
                            *jump = Some(target.clone());
                        }
                        targets.push(target);
                    }
                });
            if state.address_editing.is_none() && !start_edit_by_key {
                let prev_sel = state.address_violation_selected;
                let alt_moved = ui.input(|i| {
                    i.modifiers.alt
                        && (i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowDown))
                });
                handle_list_keys(ui, &mut state.address_violation_selected, &targets, jump);
                if alt_moved || state.address_violation_selected != prev_sel {
                    state.address_focus_violations = true;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_cell(
    ui: &mut Ui,
    state: &mut State,
    addresses: &[metadata::Address],
    speaker: &str,
    mode: Option<&str>,
    target: &str,
    commit_edit: &mut Option<(String, Option<String>, String, String)>,
    cancel_edit: &mut bool,
) {
    let is_editing = state
        .address_editing
        .as_ref()
        .is_some_and(|(s, m, t)| s == speaker && m.as_deref() == mode && t == target);
    if is_editing {
        let edit_id = egui::Id::new(("addr-cell-edit", speaker, mode.unwrap_or(""), target));
        let resp = ui.add(
            egui::TextEdit::singleline(&mut state.address_edit_buffer)
                .id(edit_id)
                .desired_width(92.0),
        );
        if state.address_edit_fresh {
            resp.request_focus();
            state.address_edit_fresh = false;
        } else {
            let esc = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
                || (resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
            if esc {
                *cancel_edit = true;
            } else if enter {
                *commit_edit = Some((
                    speaker.to_owned(),
                    mode.map(str::to_owned),
                    target.to_owned(),
                    state.address_edit_buffer.clone(),
                ));
            }
        }
        return;
    }

    let (disp, is_wildcard, exact_names) = cell_preview(addresses, speaker, mode, target);
    let has_mode_specific = addresses
        .iter()
        .any(|a| a.speaker == speaker && a.target == target && a.mode.is_some());
    let is_selected = state
        .address_selected_pair
        .as_ref()
        .is_some_and(|(s, t)| s == speaker && t == target);

    let base_label = if disp.is_empty() {
        "   ".to_owned()
    } else {
        disp
    };
    let shown = if has_mode_specific {
        format!("{base_label} •")
    } else {
        base_label
    };
    let mut rich = RichText::new(shown);
    if is_wildcard {
        rich = rich.color(ui.visuals().weak_text_color());
    }
    let resp = ui.selectable_label(is_selected, rich);
    if resp.double_clicked() {
        state.address_selected_pair = Some((speaker.to_owned(), target.to_owned()));
        state.address_violation_selected = 0;
        state.address_focus_violations = false;
        state.address_editing = Some((
            speaker.to_owned(),
            mode.map(str::to_owned),
            target.to_owned(),
        ));
        state.address_edit_buffer = exact_names.join(", ");
        state.address_edit_fresh = true;
    } else if resp.clicked() {
        state.address_selected_pair = Some((speaker.to_owned(), target.to_owned()));
        state.address_violation_selected = 0;
        state.address_focus_violations = false;
    }
}
