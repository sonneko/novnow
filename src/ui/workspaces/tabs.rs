//! Stage-two workspace tabs: the shared State, the tab bar and dispatch.
//! Screen-specific UI lives in the sibling modules
//! (`reader` / `stats` / `regex_search` / `addresses` / `local_llm` / `proposals`).
use super::Workspace;
use crate::{
    core::{
        features::{metadata, metrics, search, sections},
        model::document::DocumentFormat,
    },
    ui::{
        i18n::{self, Locale},
        workbench::Workbench,
    },
};
use egui::Context;

#[derive(Default)]
pub struct State {
    pub stamp: String,
    pub(super) address_generation: Option<u64>,
    observation_unmatched: bool,
    pub reader_position: Option<(String, usize)>,
    pub(super) address_cells: std::collections::BTreeMap<(String, String), String>,
    observation_filters: [bool; 18],
    observation_loaded: bool,
    pub tabs: [usize; 9],
    pub(super) query: search::Query,
    pub(super) replacement: String,
    pub(super) results: Option<Result<search::Results, String>>,
    pub(super) confirmed: bool,
    pub stats: metrics::Stats,
    pub session: metrics::Session,
    pub generation: Option<u64>,
    pub stats_dirty: bool,
    pub stats_flush_requested: bool,
    pub stats_last_flush: f64,
    stats_idle_flushed: bool,
    pub reader_loaded: bool,
    pub reader_jump: Option<crate::core::id::SceneId>,
    pub reader_page: usize,
    pub reader_columns: usize,
    pub reader_rows: usize,
    pub reader_dirty: bool,
    pub reader_marks: Vec<(String, usize)>,
    pub(super) reader_ruby: bool,
    pub(super) reader_dark: bool,
    pub(super) reader_cache: Option<super::reader::ReaderCache>,
    pub reader_raw: serde_json::Value,
    pub stock_episode: u16,
    pub stock_chapter: u16,
    pub local_enabled: bool,
    pub local_url: String,
    pub local_model: String,
    pub local_loaded: bool,
    pub local_job: Option<u64>,
    pub local_result: String,
    pub custom_roles: String,
    pub reader_episode: usize,
    pub(super) reader_font: f32,
    pub(super) reader_vertical: bool,
}

pub(super) fn label(locale: Locale, key: &str) -> String {
    i18n::text(locale, key).to_owned()
}
impl State {
    pub fn tick(&mut self, bench: &Workbench, now: f64, date: &str, hour: usize, activity: bool) {
        if self.generation.is_none() {
            self.stats = serde_json::from_str(bench.project().text(".novnow/stats.json"))
                .unwrap_or_default();
            self.stats_last_flush = now;
            self.session.begin(now, bench.counts().2);
            self.stats.record(date, hour, 0, 0, true);
            self.stats_dirty = true;
        }
        if self.generation.is_some()
            && (self.generation != Some(bench.generation())
                || (activity && self.session.last_count != bench.counts().2))
        {
            let (delta, seconds, reset) = self.session.edit(now, bench.counts().2);
            self.stats.record(date, hour, delta, seconds, reset);
            self.stats_dirty = true;
        } else if activity && self.generation.is_some() {
            let count = self.session.last_count;
            let (_, seconds, reset) = self.session.edit(now, count);
            self.stats.record(date, hour, 0, seconds, reset);
            self.stats_dirty = true;
        }
        if now - self.session.last_activity >= 1800.0 && !self.stats_idle_flushed {
            self.stats_flush_requested = true;
            self.stats_idle_flushed = true;
        } else if activity {
            self.stats_idle_flushed = false;
        }
        self.generation = Some(bench.generation());
    }
}

pub fn show(
    ctx: &Context,
    state: &mut State,
    bench: &mut Workbench,
    workspace: Workspace,
    locale: Locale,
    now: f64,
    host: &mut dyn crate::ui::app::Host,
) -> bool {
    let keys: &[&str] = match workspace {
        Workspace::Review => &[
            "stage.base",
            "stage.timeline",
            "stage.speakers",
            "stage.histogram",
            "stage.observations",
            "stage.stats",
            "stage.senses",
            "stage.illustrations",
            "stage.proposals",
        ],
        Workspace::PlotOutline => &[
            "stage.base",
            "stage.timeline",
            "stage.matrix",
            "stage.illustrations",
            "stage.senses",
            "stage.space",
            "stage.blurb",
            "stage.histogram",
            "stage.stock",
        ],
        Workspace::Cast => &["stage.base", "stage.addresses"],
        Workspace::Material => &["stage.base", "stage.citations"],
        Workspace::Agent => &["stage.base", "stage.local_llm"],
        Workspace::Reader => &[],
        Workspace::Write => &["stage.base", "stage.search"],
        _ => return false,
    };
    let index = workspace.number() as usize - 1;
    if !keys.is_empty() {
        egui::TopBottomPanel::top("stage-two-tabs").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, key) in keys.iter().enumerate() {
                    if *key == "stage.timeline"
                        && metadata::timeline(&metadata::records(bench.project())).is_empty()
                    {
                        continue;
                    }
                    ui.selectable_value(&mut state.tabs[index], i, label(locale, key));
                }
            });
        });
    }
    let tab = state.tabs[index].min(keys.len().saturating_sub(1));
    if workspace != Workspace::Reader && tab == 0 {
        return false;
    }
    let key = keys.get(tab).copied().unwrap_or("stage.reader");
    let mut jump = None;
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading(label(locale, key));
        match key {
            "stage.local_llm" => {
                super::local_llm::local_ui(ui, state, bench, locale, now, host);
            }
            "stage.stock" => {
                ui.add(egui::DragValue::new(&mut state.stock_episode).clamp_range(1..=999));
                ui.add(egui::DragValue::new(&mut state.stock_chapter).clamp_range(1..=999));
                if ui.button(label(locale, "stage.assign")).clicked() {
                    bench.commit();
                    if let Ok(plan) = crate::core::features::stock::assign(
                        bench.project(),
                        state.stock_episode,
                        state.stock_chapter,
                    ) {
                        bench.apply_edit(&plan.edit, now);
                        for (from, to) in plan.renames {
                            bench.rename_document(&from, &to, now);
                        }
                    }
                }
            }
            "stage.proposals" => {
                super::proposals::proposals_ui(ui, bench, locale, now, &state.stamp);
            }
            "stage.search" => {
                super::regex_search::search_ui(ui, state, bench, locale, now, &mut jump);
            }
            "stage.reader" => {
                super::reader::reader(ui, state, bench, locale, &mut jump);
            }
            "stage.stats" => {
                super::stats::stats_ui(ui, state, locale);
            }
            "stage.speakers" => {
                let m = metrics::project(bench.project());
                ui.label(format!(
                    "{}: {:.1}%",
                    label(locale, "stage.dialogue_ratio"),
                    m.ratio() * 100.0
                ));
                for (name, speech) in m.speakers {
                    let name = if name.is_empty() {
                        label(locale, "stage.unknown")
                    } else {
                        name
                    };
                    ui.label(format!("{name}  {} / {}", speech.lines, speech.chars));
                }
            }
            "stage.histogram" => {
                let m = metrics::project(bench.project());
                for (bin, n) in metrics::histogram(&m.lengths, 100) {
                    ui.label(format!("{bin}–{} {}", bin + 99, "▮".repeat(n.min(100))));
                }
                let scores = metrics::t_scores(&m.lengths);
                let items: Vec<_> = sections::all(bench.project())
                    .into_iter()
                    .filter(|s| s.id.chapter > 0 && s.id.episode > 0)
                    .collect();
                for ((item, n), score) in items.iter().zip(m.lengths).zip(scores) {
                    let score = score.map_or_else(|| "—".to_owned(), |n| format!("{n:.1}"));
                    if ui
                        .link(format!(
                            "{}  {n}  {score}",
                            item.id.display(bench.project().digits())
                        ))
                        .clicked()
                    {
                        jump = Some((
                            item.path.clone(),
                            crate::core::features::project::line_starts_of(
                                bench.project().text(&item.path),
                            )
                            .get(item.lines.start)
                            .copied()
                            .unwrap_or(0),
                        ));
                    }
                }
            }
            "stage.observations" => {
                use crate::core::diagnostics::DiagnosticCode as C;
                if !state.observation_loaded {
                    state.observation_filters = [
                        false, false, false, false, false, true, false, false, false, false, false,
                        false, false, false, false, false, false, false,
                    ];
                    state.observation_unmatched = true;
                    state.observation_loaded = true;
                }
                ui.label(label(locale, "stage.advisory"));
                let codes = [
                    C::W081,
                    C::W087,
                    C::W088,
                    C::W089,
                    C::W090,
                    C::W082,
                    C::W083,
                    C::W096,
                    C::W097,
                    C::W098,
                    C::W099,
                    C::W100,
                    C::W101,
                    C::W102,
                    C::W103,
                    C::W104,
                    C::W105,
                    C::W106,
                ];
                ui.horizontal_wrapped(|ui| {
                    for (index, code) in codes.iter().enumerate() {
                        ui.checkbox(&mut state.observation_filters[index], code.as_str());
                    }
                    ui.checkbox(
                        &mut state.observation_unmatched,
                        i18n::text(locale, "cast.tab_unmatched"),
                    );
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if state.observation_unmatched {
                        for found in
                            crate::core::features::cast::unmatched_dialogue(bench.project())
                        {
                            if ui
                                .link(format!("{}: {}", found.speaker, found.text))
                                .clicked()
                            {
                                jump = Some((found.path, found.byte));
                            }
                        }
                    }
                    for doc in bench.project().documents() {
                        for d in
                            crate::core::features::diagnose::document(doc, bench.project(), true)
                        {
                            if codes
                                .iter()
                                .position(|code| *code == d.code)
                                .is_some_and(|index| state.observation_filters[index])
                                && ui.link(i18n::diagnostic(locale, &d)).clicked()
                            {
                                jump = Some((
                                    doc.path.clone(),
                                    d.range.as_ref().map_or(0, |r| r.start),
                                ));
                            }
                        }
                    }
                });
            }
            "stage.addresses" => {
                super::addresses::addresses_ui(ui, state, bench, locale, now);
            }
            "stage.illustrations" => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("illustrations-unified")
                        .striped(true)
                        .show(ui, |ui| {
                            for item in metadata::illustrations(bench.project()) {
                                let source = match item.source {
                                    metadata::IllustrationSource::Plot => "stage.illustration_plot",
                                    metadata::IllustrationSource::Contents => {
                                        "stage.illustration_contents"
                                    }
                                };
                                ui.label(label(locale, source));
                                if ui
                                    .link(item.id.map_or_else(
                                        || item.path.clone(),
                                        |id| id.display(bench.project().digits()),
                                    ))
                                    .clicked()
                                {
                                    jump = Some((item.path, item.at));
                                }
                                ui.label(item.memo);
                                ui.end_row();
                            }
                        });
                });
            }
            "stage.matrix" => {
                let rows = metadata::records(bench.project());
                egui::ScrollArea::both().show(ui, |ui| {
                    egui::Grid::new("scene-matrix")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(label(locale, "stage.scene"));
                            for field in ["summary", "when", "cast", "place", "senses", "space"] {
                                ui.label(field);
                            }
                            ui.end_row();
                            for row in rows {
                                if ui.link(row.id.display(bench.project().digits())).clicked() {
                                    jump = Some((row.path.clone(), row.range.start));
                                }
                                for field in ["summary", "when", "cast", "place", "senses", "space"]
                                {
                                    ui.label(row.fields.get(field).map_or("—", String::as_str));
                                }
                                ui.end_row();
                            }
                        });
                });
            }
            "stage.citations" => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for doc in bench
                        .project()
                        .documents()
                        .filter(|d| d.format == DocumentFormat::Contents)
                    {
                        let mut offset = 0;
                        for raw in doc.text.split_inclusive('\n') {
                            for (_, id) in crate::core::annotations::citations(raw.trim_end()) {
                                if ui
                                    .link(format!("{id} {}  {}", doc.path, raw.trim()))
                                    .clicked()
                                {
                                    jump = Some((doc.path.clone(), offset));
                                }
                            }
                            offset += raw.len();
                        }
                    }
                });
            }
            _ => {
                let rows = metadata::records(bench.project());
                egui::ScrollArea::both().show(ui, |ui| {
                    if key == "stage.timeline" {
                        let mut timeline = metadata::timeline(&rows);
                        timeline.sort_by(|a, b| {
                            match (a.time.start, b.time.start) {
                                (Some(a), Some(b)) => a.total_cmp(&b),
                                (Some(_), None) => std::cmp::Ordering::Less,
                                (None, Some(_)) => std::cmp::Ordering::Greater,
                                _ => std::cmp::Ordering::Equal,
                            }
                            .then(a.time.clock.cmp(&b.time.clock))
                        });
                        for row in timeline {
                            if ui
                                .link(format!(
                                    "{}  {}",
                                    row.record.id.display(bench.project().digits()),
                                    row.record.fields.get("when").cloned().unwrap_or_default()
                                ))
                                .clicked()
                            {
                                jump = Some((row.record.path, row.record.range.start));
                            }
                        }
                    } else {
                        for row in rows {
                            let field = match key {
                                "stage.senses" => "senses",
                                "stage.space" => "space",
                                "stage.blurb" => "blurb",
                                "stage.illustrations" => "illustration",
                                _ => "summary",
                            };
                            let value = row.fields.get(field).cloned().unwrap_or_default();
                            if value.is_empty() && key != "stage.matrix" {
                                continue;
                            }
                            if ui.link(row.id.display(bench.project().digits())).clicked() {
                                jump = Some((row.path.clone(), row.range.start));
                            }
                            if key == "stage.space" {
                                let path = metadata::asset_path(&value, false);
                                if let Some(doc) = path.and_then(|p| bench.project().document(&p)) {
                                    ui.monospace(&doc.text);
                                } else {
                                    ui.monospace(value);
                                }
                            } else {
                                ui.label(value);
                            }
                        }
                    }
                });
            }
        }
    });
    if let Some((path, at)) = jump {
        bench.reveal(&path, at);
        if workspace == Workspace::Reader {
            bench.save.request_now();
        }
    }
    true
}
