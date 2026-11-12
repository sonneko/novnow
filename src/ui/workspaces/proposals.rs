//! 点検ワークスペースの「人間の提案」タブ（§8.7・§10.9.1）。
use super::tabs::label;
use crate::core::model::document::DocumentFormat;
use crate::ui::{
    i18n::{self, Locale},
    workbench::Workbench,
};
use egui::Ui;

pub(super) fn proposals_ui(
    ui: &mut Ui,
    bench: &mut Workbench,
    locale: Locale,
    now: f64,
    stamp: &str,
) {
    // 外部変更や本文更新で before と一致しなくなった open 提案は status: stale に遷移させる（§8.7.4）
    bench.sync_stale_human_proposals(stamp, now);

    let proposals: Vec<_> = bench
        .project()
        .documents()
        .filter(|d| d.format == DocumentFormat::Proposal)
        .filter_map(|d| crate::core::agents::files::parse_proposal(&d.text))
        .filter(|p| p.kind == "human")
        .collect();
    for proposal in proposals {
        egui::CollapsingHeader::new(format!("{} {}", proposal.id, proposal.status))
            .default_open(proposal.status == "open" || proposal.status == "stale")
            .show(ui, |ui| {
                ui.columns(2, |c| {
                    c[0].label(label(locale, "stage.before"));
                    c[0].monospace(&proposal.before);
                    c[1].label(label(locale, "stage.after"));
                    c[1].monospace(&proposal.after);
                });
                if proposal.status == "stale" {
                    ui.label(i18n::diagnostic(
                        locale,
                        &crate::core::diagnostics::Diagnostic::new(
                            crate::core::diagnostics::DiagnosticCode::W093,
                            None,
                            vec![crate::core::diagnostics::Arg::text(&proposal.id)],
                        ),
                    ));
                    ui.label(label(locale, "stage.proposal_stale_note"));
                    if ui.button(label(locale, "stage.discard")).clicked() {
                        if let Ok(plan) = crate::core::agents::apply::discard(
                            bench.project(),
                            &proposal.id,
                            stamp,
                        ) {
                            for write in plan.writes {
                                bench.replace_document(&write.path, write.text, now);
                            }
                        }
                    }
                } else if proposal.status == "open" {
                    ui.horizontal(|ui| {
                        if ui.button(label(locale, "stage.accept")).clicked() {
                            if let Ok(plan) = crate::core::agents::apply::accept(
                                bench.project(),
                                &proposal.id,
                                None,
                                stamp,
                            ) {
                                for write in plan.writes {
                                    bench.replace_document(&write.path, write.text, now);
                                }
                            }
                        }
                        if ui.button(label(locale, "stage.discard")).clicked() {
                            if let Ok(plan) = crate::core::agents::apply::discard(
                                bench.project(),
                                &proposal.id,
                                stamp,
                            ) {
                                for write in plan.writes {
                                    bench.replace_document(&write.path, write.text, now);
                                }
                            }
                        }
                    });
                }
            });
    }
}
