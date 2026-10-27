//! Stock-proposal tab of the plot outline workspace.
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
    let proposals: Vec<_> = bench
        .project()
        .documents()
        .filter(|d| d.format == DocumentFormat::Proposal)
        .filter_map(|d| crate::core::agents::files::parse_proposal(&d.text))
        .filter(|p| p.kind == "human")
        .collect();
    for proposal in proposals {
        ui.collapsing(format!("{} {}", proposal.id, proposal.status), |ui| {
            ui.columns(2, |c| {
                c[0].monospace(&proposal.before);
                c[1].monospace(&proposal.after);
            });
            if proposal.status == "open" {
                let current = proposal
                    .target_parts()
                    .1
                    .map(|id| {
                        crate::core::agents::apply::current_text(
                            bench.project(),
                            crate::core::agents::Kind::SectionText,
                            id,
                        )
                    })
                    .unwrap_or_default();
                if current != proposal.before {
                    ui.label(i18n::diagnostic(
                        locale,
                        &crate::core::diagnostics::Diagnostic::new(
                            crate::core::diagnostics::DiagnosticCode::W093,
                            None,
                            vec![crate::core::diagnostics::Arg::text(&proposal.id)],
                        ),
                    ));
                } else if ui.button(label(locale, "stage.accept")).clicked() {
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
                    bench.remove_document(
                        &crate::core::agents::files::proposal_path(&proposal.id),
                        now,
                    );
                }
            }
        });
    }
}
