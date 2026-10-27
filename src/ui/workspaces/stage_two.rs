//! Retained stage-two views. They share the normal Workbench save/undo pipeline.
use super::Workspace;
use crate::{
    core::{
        features::sections,
        model::document::DocumentFormat,
        stage_two::{metadata, metrics, search},
    },
    ui::{
        i18n::{self, Locale},
        workbench::Workbench,
    },
};
use egui::{Context, Ui};
struct ReaderCache {
    key: (String, u64, usize, usize, bool, u32),
    pages: Vec<crate::core::reader::Page>,
}
#[derive(Default)]
pub struct State {
    pub stamp: String,
    address_generation: Option<u64>,
    observation_unmatched: bool,
    pub reader_position: Option<(String, usize)>,
    address_cells: std::collections::BTreeMap<(String, String), String>,
    observation_filters: [bool; 8],
    observation_loaded: bool,
    pub tabs: [usize; 9],
    query: search::Query,
    replacement: String,
    results: Option<Result<search::Results, String>>,
    confirmed: bool,
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
    reader_ruby: bool,
    reader_dark: bool,
    reader_cache: Option<ReaderCache>,
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
    reader_font: f32,
    reader_vertical: bool,
}
fn label(locale: Locale, key: &str) -> String {
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
                local_ui(ui, state, bench, locale, now, host);
            }
            "stage.stock" => {
                ui.add(egui::DragValue::new(&mut state.stock_episode).clamp_range(1..=999));
                ui.add(egui::DragValue::new(&mut state.stock_chapter).clamp_range(1..=999));
                if ui.button(label(locale, "stage.assign")).clicked() {
                    bench.commit();
                    if let Ok(plan) = crate::core::stage_two::stock::assign(
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
                proposals_ui(ui, bench, locale, now, &state.stamp);
            }
            "stage.search" => {
                search_ui(ui, state, bench, locale, now, &mut jump);
            }
            "stage.reader" => {
                reader(ui, state, bench, locale, &mut jump);
            }
            "stage.stats" => {
                stats_ui(ui, state, locale);
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
                    state.observation_filters =
                        [false, false, false, false, false, true, false, false];
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
                            crate::core::stage_two::diagnose::document(doc, bench.project(), true)
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
                addresses_ui(ui, state, bench, locale, now);
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
fn bars(ui: &mut Ui, values: &[(String, f64)]) {
    let max = values.iter().map(|(_, n)| n.abs()).fold(1.0, f64::max);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(100.0), 100.0),
        egui::Sense::hover(),
    );
    let width = rect.width() / values.len().max(1) as f32;
    for (index, (label, value)) in values.iter().enumerate() {
        let height = (value.abs() / max) as f32 * 90.0;
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left() + index as f32 * width, rect.bottom() - height),
            egui::vec2((width - 1.0).max(1.0), height),
        );
        ui.painter()
            .rect_filled(bar, 0.0, ui.visuals().selection.bg_fill);
        ui.interact(bar, ui.id().with(index), egui::Sense::hover())
            .on_hover_text(format!("{label}: {value:.1}"));
    }
}
fn stats_ui(ui: &mut Ui, state: &State, locale: Locale) {
    let dates = metrics::recent_dates(state.stamp.get(..10).unwrap_or(""), 400);
    let stats = &state.stats;
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.label(i18n::format(
            locale,
            "stage.stats_averages",
            &[
                &format!(
                    "{:.0}",
                    stats.average(&dates.iter().take(7).cloned().collect::<Vec<_>>())
                ),
                &format!(
                    "{:.0}",
                    stats.average(&dates.iter().take(30).cloned().collect::<Vec<_>>())
                ),
                &stats.streak(&dates).to_string(),
            ],
        ));
        ui.heading(label(locale, "stage.daily"));
        let daily = dates
            .iter()
            .take(90)
            .rev()
            .map(|d| (d.clone(), stats.days.get(d).map_or(0, |d| d.chars) as f64))
            .collect::<Vec<_>>();
        bars(ui, &daily);
        ui.heading(label(locale, "stage.weekdays"));
        let mut sums = [0i64; 7];
        let mut counts = [0usize; 7];
        for date in dates.iter().take(90) {
            if let Some(day) = metrics::weekday(date) {
                sums[day] += stats.days.get(date).map_or(0, |d| d.chars);
                counts[day] += 1;
            }
        }
        bars(
            ui,
            &(0..7)
                .map(|i| (i.to_string(), sums[i] as f64 / counts[i].max(1) as f64))
                .collect::<Vec<_>>(),
        );
        ui.heading(label(locale, "stage.hours"));
        let mut hours = [0; 24];
        for day in stats.days.values() {
            for hour in &day.hours {
                if *hour < 24 {
                    hours[*hour] += 1;
                }
            }
        }
        bars(
            ui,
            &hours
                .iter()
                .enumerate()
                .map(|(h, n)| (format!("{h:02}:00"), *n as f64))
                .collect::<Vec<_>>(),
        );
        for (date, day) in stats.days.iter().rev() {
            ui.label(format!(
                "{date}   {:+}   {} {}   ×{}",
                day.chars,
                day.active_seconds / 60,
                label(locale, "stage.minutes"),
                day.sessions
            ));
        }
    });
}

fn addresses_ui(ui: &mut Ui, state: &mut State, bench: &mut Workbench, locale: Locale, now: f64) {
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

fn search_ui(
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
fn reader(
    ui: &mut Ui,
    state: &mut State,
    bench: &mut Workbench,
    locale: Locale,
    jump: &mut Option<(String, usize)>,
) {
    if !state.reader_loaded {
        state.reader_raw = serde_json::from_str(bench.project().text(".novnow/reader.json"))
            .unwrap_or_else(|_| serde_json::json!({}));
        let raw = &state.reader_raw;
        let defaults = bench.project().settings().unknown.get("reader");
        state.reader_columns = raw
            .get("columns")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| {
                defaults
                    .and_then(|d| d.get("columns"))
                    .and_then(serde_json::Value::as_u64)
            })
            .unwrap_or(18)
            .clamp(5, 40) as usize;
        state.reader_rows = defaults
            .and_then(|d| d.get("chars_per_column"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(42)
            .clamp(10, 80) as usize;
        state.reader_font = raw
            .get("font_size")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(20.0)
            .clamp(12.0, 36.0) as f32;
        state.reader_vertical = raw
            .get("vertical")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        state.reader_ruby = raw
            .get("ruby")
            .and_then(serde_json::Value::as_bool)
            .or_else(|| {
                defaults
                    .and_then(|d| d.get("ruby"))
                    .and_then(serde_json::Value::as_bool)
            })
            .unwrap_or(true);
        state.reader_dark = raw
            .get("dark")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        state.reader_episode = raw
            .get("episode")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        state.reader_page = raw
            .get("page")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        state.reader_marks = serde_json::from_value(
            raw.get("bookmarks")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )
        .unwrap_or_default();
        state.reader_loaded = true;
    }
    let all = sections::all(bench.project());
    let mut episodes = std::collections::BTreeMap::new();
    for section in all.iter().filter(|s| s.id.chapter > 0 && s.id.episode > 0) {
        episodes
            .entry((section.id.chapter, section.id.episode))
            .or_insert(section.path.clone());
    }
    let episodes: Vec<_> = episodes.into_iter().collect();
    if episodes.is_empty() {
        return;
    }
    if let Some(id) = state.reader_jump.take() {
        if let Some(index) = episodes
            .iter()
            .position(|((c, e), _)| *c == id.chapter && *e == id.episode)
        {
            state.reader_episode = index;
            state.reader_page = 0;
            state.reader_loaded = true;
        }
    }
    state.reader_episode = state.reader_episode.min(episodes.len() - 1);
    let old = (
        state.reader_episode,
        state.reader_page,
        state.reader_font.to_bits(),
        state.reader_columns,
        state.reader_vertical,
        state.reader_ruby,
        state.reader_dark,
    );
    let consume = |key| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
    if consume(egui::Key::ArrowLeft) || consume(egui::Key::Space) || consume(egui::Key::PageDown) {
        state.reader_page = state.reader_page.saturating_add(1);
    }
    if consume(egui::Key::ArrowRight) || consume(egui::Key::PageUp) {
        state.reader_page = state.reader_page.saturating_sub(1);
    }
    if consume(egui::Key::Home) {
        state.reader_episode = 0;
        state.reader_page = 0;
    }
    if consume(egui::Key::End) {
        state.reader_episode = episodes.len() - 1;
        state.reader_page = usize::MAX;
    }
    if consume(egui::Key::R) {
        state.reader_ruby = !state.reader_ruby;
    }
    if consume(egui::Key::T) {
        state.reader_dark = !state.reader_dark;
    }
    if consume(egui::Key::Plus) || consume(egui::Key::Equals) {
        state.reader_font = (state.reader_font + 1.0).min(36.0);
    }
    if consume(egui::Key::Minus) {
        state.reader_font = (state.reader_font - 1.0).max(12.0);
    }
    if consume(egui::Key::Num0) {
        state.reader_font = 20.0;
    }
    let exit = consume(egui::Key::Escape);
    let mut bookmark = consume(egui::Key::B);
    let mut edit = exit;
    ui.horizontal_wrapped(|ui| {
        if ui.button("←").clicked() {
            state.reader_page = state.reader_page.saturating_add(1);
        }
        if ui.button("→").clicked() {
            state.reader_page = state.reader_page.saturating_sub(1);
        }
        if ui.button(label(locale, "stage.previous_episode")).clicked() {
            state.reader_episode = state.reader_episode.saturating_sub(1);
            state.reader_page = 0;
        }
        if ui.button(label(locale, "stage.next_episode")).clicked() {
            state.reader_episode = (state.reader_episode + 1).min(episodes.len() - 1);
            state.reader_page = 0;
        }
        ui.add(
            egui::Slider::new(&mut state.reader_font, 12.0..=36.0)
                .text(label(locale, "stage.font")),
        );
        ui.add(
            egui::Slider::new(&mut state.reader_columns, 5..=40)
                .text(label(locale, "stage.columns")),
        );
        ui.checkbox(&mut state.reader_vertical, label(locale, "stage.vertical"));
        ui.checkbox(&mut state.reader_ruby, label(locale, "stage.ruby"));
        ui.checkbox(&mut state.reader_dark, label(locale, "stage.dark"));
        bookmark |= ui.button(label(locale, "stage.bookmark")).clicked();
        edit |= ui.button(label(locale, "stage.edit")).clicked();
    });
    let ((chapter, episode), path) = &episodes[state.reader_episode];
    ui.heading(format!("{chapter:02}–{episode:02}"));
    let size = state.reader_font;
    let font = egui::FontId::proportional(size);
    let measured = ui.fonts(|f| f.glyph_width(&font, '漢')).max(size);
    let metrics = crate::core::reader::Metrics {
        advance: measured * 1.15,
        line_advance: measured * 1.7,
        vertical_forms: Default::default(),
        glyph_advances: Default::default(),
    };
    let options = crate::core::reader::Options {
        rows: state.reader_rows,
        columns: state.reader_columns,
        vertical: state.reader_vertical,
    };
    use std::hash::{Hash, Hasher};
    let mut fingerprint = std::collections::hash_map::DefaultHasher::new();
    bench.project().text(path).hash(&mut fingerprint);
    bench
        .project()
        .text("plots/all.cnov")
        .hash(&mut fingerprint);
    bench
        .project()
        .text(&bench.project().chapter_plot_path(*chapter))
        .hash(&mut fingerprint);
    let cache_key = (
        path.clone(),
        fingerprint.finish(),
        state.reader_rows,
        state.reader_columns,
        state.reader_vertical,
        size.to_bits(),
    );
    if state
        .reader_cache
        .as_ref()
        .is_none_or(|cache| cache.key != cache_key)
    {
        let text = crate::core::export::fragment(bench.project().text(path), true);
        let mut measured_metrics = metrics.clone();
        ui.fonts(|fonts| {
            // 括弧の互換形は、使用フォントに収録されている場合だけ組版で使う（§17.3.3）。
            for &(_, form) in crate::core::reader::VERTICAL_FORMS {
                if fonts.has_glyph(&font, form) {
                    measured_metrics.vertical_forms.insert(form);
                }
            }
            for ch in text.chars() {
                measured_metrics
                    .glyph_advances
                    .entry(ch)
                    .or_insert_with(|| fonts.glyph_width(&font, ch).max(size * 0.5) * 1.15);
            }
        });
        let mut pages = crate::core::reader::layout(&text, options, &measured_metrics);
        if episodes.iter().position(|((c, _), _)| c == chapter) == Some(state.reader_episode) {
            let title = crate::core::features::plots::outline_blocks(bench.project(), None)
                .into_iter()
                .find(|b| b.id.chapter == *chapter && b.id.episode == 0)
                .map_or_else(|| format!("{chapter:02}"), |b| b.title);
            let blurb = metadata::chapter_blurb(bench.project(), *chapter);
            let cover = crate::core::reader::layout(
                &format!("{title}\n\n{blurb}"),
                options,
                &measured_metrics,
            );
            pages.splice(0..0, cover);
        }

        state.reader_cache = Some(ReaderCache {
            key: cache_key,
            pages,
        });
    }
    let pages = &state.reader_cache.as_ref().unwrap().pages;
    state.reader_page = state.reader_page.min(pages.len() - 1);
    ui.label(format!("{} / {}", state.reader_page + 1, pages.len()));
    let starts = crate::core::features::project::line_starts_of(bench.project().text(path));
    // The same prefix layout maps source sections to pages for the TOC and edit return.
    let source_sections: Vec<_> = all.iter().filter(|s| s.path == *path).collect();
    ui.collapsing(label(locale, "stage.toc"), |ui| {
        for section in &source_sections {
            if ui
                .link(section.id.display(bench.project().digits()))
                .clicked()
            {
                let at = starts.get(section.lines.start).copied().unwrap_or(0);
                let prefix = crate::core::export::fragment(&bench.project().text(path)[..at], true);
                state.reader_page = crate::core::reader::layout(&prefix, options, &metrics)
                    .len()
                    .saturating_sub(1);
            }
        }
    });
    if bookmark {
        let mark = (path.clone(), state.reader_page);
        if let Some(index) = state.reader_marks.iter().position(|m| *m == mark) {
            state.reader_marks.remove(index);
        } else {
            state.reader_marks.push(mark);
        }
        state.reader_dirty = true;
    }
    ui.horizontal_wrapped(|ui| {
        for (marked_path, page) in state.reader_marks.clone() {
            if ui.link(format!("🔖 {marked_path}:{}", page + 1)).clicked() {
                if let Some(index) = episodes.iter().position(|(_, path)| *path == marked_path) {
                    state.reader_episode = index;
                    state.reader_page = page;
                }
            }
        }
    });
    let page = state.reader_page.min(pages.len() - 1);
    let fg = if state.reader_dark {
        egui::Color32::from_rgb(225, 225, 220)
    } else {
        egui::Color32::from_rgb(35, 35, 30)
    };
    let bg = if state.reader_dark {
        egui::Color32::from_rgb(27, 29, 32)
    } else {
        egui::Color32::from_rgb(250, 248, 239)
    };
    egui::ScrollArea::both().show(ui, |ui| {
        let width = state.reader_columns as f32
            * if state.reader_vertical {
                metrics.line_advance
            } else {
                metrics.advance
            };
        let height = state.reader_rows as f32
            * if state.reader_vertical {
                metrics.advance
            } else {
                metrics.line_advance
            };
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(width + size, height + size),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, 0.0, bg);
        for glyph in &pages[page].glyphs {
            let pos = rect.min + egui::vec2(glyph.x + size * 0.25, glyph.y + size * 0.25);
            let glyph_size = if glyph.compact { size * 0.65 } else { size };
            let galley = ui.painter().layout_no_wrap(
                glyph.text.clone(),
                egui::FontId::proportional(glyph_size),
                fg,
            );
            let rotated = if glyph.rotate {
                pos + egui::vec2(size, 0.0)
            } else {
                pos
            };
            ui.painter().add(
                egui::epaint::TextShape::new(rotated, galley, fg).with_angle(if glyph.rotate {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                }),
            );
            if state.reader_ruby && !glyph.ruby.is_empty() {
                for (index, ch) in glyph.ruby.chars().enumerate() {
                    let offset = if state.reader_vertical {
                        egui::vec2(size, index as f32 * size * 0.5)
                    } else {
                        egui::vec2(index as f32 * size * 0.5, -size * 0.5)
                    };
                    ui.painter().text(
                        pos + offset,
                        egui::Align2::LEFT_TOP,
                        ch,
                        egui::FontId::proportional(size * 0.5),
                        fg,
                    );
                }
            }
        }
    });
    let first = pages[page]
        .glyphs
        .iter()
        .take(6)
        .map(|g| {
            if g.compact {
                g.text.clone()
            } else {
                g.ch.to_string()
            }
        })
        .collect::<String>();
    let at = bench.project().text(path).find(&first).unwrap_or(0);
    state.reader_position = Some((path.clone(), at));
    if edit {
        *jump = state.reader_position.clone();
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("reader-exit"), true));
    }
    if old
        != (
            state.reader_episode,
            state.reader_page,
            state.reader_font.to_bits(),
            state.reader_columns,
            state.reader_vertical,
            state.reader_ruby,
            state.reader_dark,
        )
    {
        state.reader_dirty = true;
    }
    if state.reader_dirty {
        if !state.reader_raw.is_object() {
            state.reader_raw = serde_json::json!({});
        }
        for (key, value) in [
            ("episode", serde_json::json!(state.reader_episode)),
            ("page", serde_json::json!(state.reader_page)),
            ("font_size", serde_json::json!(state.reader_font)),
            ("columns", serde_json::json!(state.reader_columns)),
            ("vertical", serde_json::json!(state.reader_vertical)),
            ("ruby", serde_json::json!(state.reader_ruby)),
            ("dark", serde_json::json!(state.reader_dark)),
            ("bookmarks", serde_json::json!(state.reader_marks)),
        ] {
            state.reader_raw[key] = value;
        }
        bench.replace_document(
            ".novnow/reader.json",
            serde_json::to_string_pretty(&state.reader_raw).unwrap() + "\n",
            0.0,
        );
        state.reader_dirty = false;
    }
}

fn local_ui(
    ui: &mut Ui,
    state: &mut State,
    bench: &mut Workbench,
    locale: Locale,
    now: f64,
    host: &mut dyn crate::ui::app::Host,
) {
    if !host.mcp_available() {
        ui.label(label(locale, "stage.local_web"));
        return;
    }
    if !state.local_loaded {
        let agents = &bench.project().settings().agents;
        state.local_enabled = agents.local_llm.enabled;
        state.local_url = agents.local_llm.url.clone();
        state.local_model = agents.local_llm.model.clone();
        state.custom_roles = agents.custom_roles.join(", ");
        state.local_loaded = true;
    }
    ui.label(label(locale, "stage.local_caution"));
    ui.checkbox(&mut state.local_enabled, label(locale, "stage.enabled"));
    ui.text_edit_singleline(&mut state.local_url);
    ui.text_edit_singleline(&mut state.local_model);
    ui.label(label(locale, "stage.custom_roles"));
    ui.text_edit_singleline(&mut state.custom_roles);
    if ui.button(label(locale, "stage.confirm")).clicked() {
        let config = crate::core::model::project::LocalLlm {
            enabled: state.local_enabled,
            url: state.local_url.clone(),
            model: state.local_model.clone(),
            ..Default::default()
        };
        match crate::core::stage_two::local_llm::configure(
            bench.project().text("agents/mcp.json"),
            &config,
        ) {
            Ok(mcp) => {
                // project.toml は無損失編集で書き戻し、壊れた TOML は上書きしない（§4.5）
                let roles: Vec<String> = state
                    .custom_roles
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
                if let Some(edited) = crate::core::syntax::project_toml::edit(
                    bench
                        .project()
                        .text(crate::core::model::project::PROJECT_FILE),
                    |document| {
                        use toml_edit::{Item, Table, Value};
                        let root = document.as_table_mut();
                        if root.get("agents").is_none() {
                            root.insert("agents", Item::Table(Table::new()));
                        }
                        let Some(agents) = root.get_mut("agents").and_then(Item::as_table_like_mut)
                        else {
                            return;
                        };
                        let mut array = toml_edit::Array::new();
                        for role in &roles {
                            array.push(role.as_str());
                        }
                        crate::core::syntax::project_toml::set_value(
                            agents,
                            "custom_roles",
                            Value::Array(array),
                        );
                        if agents.get("local_llm").is_none() {
                            agents.insert(
                                "local_llm",
                                Item::Value(Value::InlineTable(Default::default())),
                            );
                        }
                        let Some(local) = agents
                            .get_mut("local_llm")
                            .and_then(Item::as_table_like_mut)
                        else {
                            return;
                        };
                        crate::core::syntax::project_toml::set_value(
                            local,
                            "enabled",
                            toml_edit::Value::from(config.enabled),
                        );
                        crate::core::syntax::project_toml::set_string(local, "url", &config.url);
                        crate::core::syntax::project_toml::set_string(
                            local,
                            "model",
                            &config.model,
                        );
                    },
                ) {
                    bench.replace_document("agents/mcp.json", mcp, now);
                    bench.replace_document(crate::core::model::project::PROJECT_FILE, edited, now);
                }
            }
            Err(e) => state.local_result = e.to_owned(),
        }
    }
    if state.local_job.is_none() && ui.button(label(locale, "stage.connection_check")).clicked() {
        if crate::core::stage_two::local_llm::is_loopback(&state.local_url) {
            state.local_job = host.local_probe_start(crate::ui::agents::Job {
                server: crate::ports::mcp::McpServer {
                    name: "novnow-local-llm".to_owned(),
                    command: "__novnow_http_mcp".to_owned(),
                    args: vec![state.local_url.clone(), state.local_model.clone()],
                    env: Vec::new(),
                    timeout_ms: 3000,
                },
                call: crate::ports::mcp::McpCall::Tool {
                    name: "__novnow_probe".to_owned(),
                    arguments: serde_json::Value::Null,
                },
            });
        } else {
            state.local_result = "E064".to_owned();
        }
    }
    for (id, result, _) in host.local_probe_poll() {
        if state.local_job == Some(id) {
            state.local_job = None;
            let diagnostic = match result {
                Ok(reply) => crate::core::diagnostics::Diagnostic::new(
                    crate::core::diagnostics::DiagnosticCode::I057,
                    None,
                    vec![crate::core::diagnostics::Arg::text(reply.model)],
                ),
                Err(error) => crate::core::diagnostics::Diagnostic::new(
                    crate::core::diagnostics::DiagnosticCode::E064,
                    None,
                    vec![crate::core::diagnostics::Arg::text(error.log_text())],
                ),
            };
            state.local_result = i18n::diagnostic(locale, &diagnostic);
        }
    }
    if state.local_job.is_some() {
        ui.spinner();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    ui.label(&state.local_result);
}
fn proposals_ui(ui: &mut Ui, bench: &mut Workbench, locale: Locale, now: f64, stamp: &str) {
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
