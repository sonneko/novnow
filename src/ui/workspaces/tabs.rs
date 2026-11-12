//! 段階 ② のワークスペースのタブ。共有の `State`、タブの並び、振り分けを置く。
//! 画面ごとの中身は隣のモジュールにある
//! （`reader` / `stats` / `regex_search` / `addresses` / `local_llm` / `proposals`）。
use super::Workspace;
use crate::{
    core::{
        features::{metadata, metrics, search, Project},
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
    pub reader_section: Option<crate::core::id::SceneId>,
    pub(super) reader_pinned_section: Option<(usize, usize, crate::core::id::SceneId)>,
    pub reader_auto_columns: bool,
    pub(super) address_cells: std::collections::BTreeMap<(String, String), String>,
    observation_filters: [bool; 18],
    observation_loaded: bool,
    pub tabs: [usize; 9],
    pub tab_selected: usize,
    last_active_tab: Option<(Workspace, usize)>,
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

/// [`show`] の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabOutcome {
    /// 基本ビュー（`tab == 0` かつ `Reader` でない）なので通常のワークスペースを描画する。
    Inactive,
    /// 追加タブまたは読者モードを描画した。
    Active {
        jump: Option<(String, usize)>,
        exit_reader: bool,
    },
}

pub(super) fn label(locale: Locale, key: &str) -> String {
    i18n::text(locale, key).to_owned()
}

/// ワークスペースごとのタブ定義。
pub fn tab_keys(workspace: Workspace) -> &'static [&'static str] {
    match workspace {
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
        _ => &[],
    }
}

/// 現在プロジェクトで表示対象となるタブ添字の列（`when:` が無ければ `stage.timeline` を省く、§10.3.2・§10.9.1）。
pub fn visible_tab_indices(project: &Project, workspace: Workspace) -> Vec<usize> {
    let keys = tab_keys(workspace);
    if keys.is_empty() {
        return Vec::new();
    }
    let has_timeline = if keys.contains(&"stage.timeline") {
        !metadata::timeline(&metadata::records(project)).is_empty()
    } else {
        false
    };
    keys.iter()
        .enumerate()
        .filter_map(|(i, key)| {
            if *key == "stage.timeline" && !has_timeline {
                None
            } else {
                Some(i)
            }
        })
        .collect()
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

    /// ワークスペースのタブを基本ビュー（`0`）へ戻す。
    pub fn reset_tab(&mut self, workspace: Workspace) {
        let index = workspace.number() as usize - 1;
        if index < self.tabs.len() {
            self.tabs[index] = 0;
            self.tab_selected = 0;
        }
    }

    /// 表示対象タブ列の中で前後に巡回する（`PlotOutline` / `Material` / `Agent` / `Write`）。
    pub fn step_tab(&mut self, project: &Project, workspace: Workspace, forward: bool) -> bool {
        let visible = visible_tab_indices(project, workspace);
        if visible.len() <= 1 {
            return false;
        }
        let index = workspace.number() as usize - 1;
        let current = self.tabs[index];
        let pos = visible.iter().position(|&i| i == current).unwrap_or(0);
        let next = if forward {
            (pos + 1) % visible.len()
        } else {
            (pos + visible.len() - 1) % visible.len()
        };
        self.tabs[index] = visible[next];
        self.tab_selected = 0;
        true
    }
}

/// タブ内リンク一覧の 1 行を描画し、クリックまたはフォーカス中 `Enter` を判定する。
pub(super) fn nav_link(
    ui: &mut egui::Ui,
    row_index: usize,
    selected_index: usize,
    text: impl Into<String>,
) -> bool {
    let label_str = text.into();
    let is_selected = row_index == selected_index;
    let rich = if is_selected {
        egui::RichText::new(label_str)
            .strong()
            .background_color(ui.visuals().selection.bg_fill)
    } else {
        egui::RichText::new(label_str)
    };
    let resp = ui.link(rich);
    let enter = resp.has_focus()
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    resp.clicked() || enter
}

/// テキスト編集欄にフォーカスが無いとき、`↑/↓`（または `Alt+↑/↓`）で選択を動かし、`Enter` で確定する。
pub(super) fn handle_list_keys(
    ui: &mut egui::Ui,
    selected: &mut usize,
    targets: &[(String, usize)],
    jump: &mut Option<(String, usize)>,
) {
    if targets.is_empty() {
        *selected = 0;
        return;
    }
    *selected = (*selected).min(targets.len() - 1);
    let editing_text = ui.ctx().wants_keyboard_input();
    let up = ui.input_mut(|i| {
        i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp)
            || (!editing_text && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
    });
    let down = ui.input_mut(|i| {
        i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown)
            || (!editing_text && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
    });
    if up {
        *selected = selected.saturating_sub(1);
    }
    if down {
        *selected = (*selected + 1).min(targets.len() - 1);
    }
    let enter =
        ui.input_mut(|i| !editing_text && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    if enter && jump.is_none() {
        if let Some(found) = targets.get(*selected) {
            *jump = Some(found.clone());
        }
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
) -> TabOutcome {
    if workspace != Workspace::Reader && tab_keys(workspace).is_empty() {
        return TabOutcome::Inactive;
    }
    let keys = tab_keys(workspace);
    let visible = visible_tab_indices(bench.project(), workspace);
    let index = workspace.number() as usize - 1;
    if !visible.is_empty() && !visible.contains(&state.tabs[index]) {
        state.tabs[index] = 0;
    }
    if !keys.is_empty() {
        egui::TopBottomPanel::top("stage-two-tabs").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for &i in &visible {
                    let key = keys[i];
                    if ui
                        .selectable_value(&mut state.tabs[index], i, label(locale, key))
                        .clicked()
                    {
                        state.tab_selected = 0;
                    }
                }
            });
        });
    }
    let tab = state.tabs[index].min(keys.len().saturating_sub(1));
    if state.last_active_tab != Some((workspace, tab)) {
        state.last_active_tab = Some((workspace, tab));
        state.tab_selected = 0;
        if tab > 0 {
            if let Some(focused) = ctx.memory(|m| m.focused()) {
                ctx.memory_mut(|m| m.surrender_focus(focused));
            }
        }
    }
    if workspace != Workspace::Reader && tab == 0 {
        return TabOutcome::Inactive;
    }
    let key = keys.get(tab).copied().unwrap_or("stage.reader");
    let mut jump = None;
    egui::CentralPanel::default().show(ctx, |ui| {
        if workspace != Workspace::Reader {
            ui.heading(label(locale, key));
        }
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
                let items: Vec<_> = bench
                    .project()
                    .sections()
                    .iter()
                    .filter(|s| s.id.chapter > 0 && s.id.episode > 0)
                    .cloned()
                    .collect();
                let mut targets = Vec::with_capacity(items.len());
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (row_idx, ((item, n), score)) in
                        items.iter().zip(m.lengths).zip(scores).enumerate()
                    {
                        let target = (
                            item.path.clone(),
                            crate::core::features::project::line_starts_of(
                                bench.project().text(&item.path),
                            )
                            .get(item.lines.start)
                            .copied()
                            .unwrap_or(0),
                        );
                        let score_str =
                            score.map_or_else(|| "—".to_owned(), |val| format!("{val:.1}"));
                        if nav_link(
                            ui,
                            row_idx,
                            state.tab_selected,
                            format!(
                                "{}  {n}  {score_str}",
                                item.id.display(bench.project().digits())
                            ),
                        ) {
                            state.tab_selected = row_idx;
                            jump = Some(target.clone());
                        }
                        targets.push(target);
                    }
                });
                handle_list_keys(ui, &mut state.tab_selected, &targets, &mut jump);
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
                    for (idx, code) in codes.iter().enumerate() {
                        ui.checkbox(&mut state.observation_filters[idx], code.as_str());
                    }
                    ui.checkbox(
                        &mut state.observation_unmatched,
                        i18n::text(locale, "cast.tab_unmatched"),
                    );
                });
                let mut targets = Vec::new();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if state.observation_unmatched {
                        for found in
                            crate::core::features::cast::unmatched_dialogue(bench.project())
                        {
                            let row_idx = targets.len();
                            let target = (found.path, found.byte);
                            if nav_link(
                                ui,
                                row_idx,
                                state.tab_selected,
                                format!("{}: {}", found.speaker, found.text),
                            ) {
                                state.tab_selected = row_idx;
                                jump = Some(target.clone());
                            }
                            targets.push(target);
                        }
                    }
                    for doc in bench.project().documents() {
                        for d in
                            crate::core::features::diagnose::document(doc, bench.project(), true)
                        {
                            if codes
                                .iter()
                                .position(|code| *code == d.code)
                                .is_some_and(|idx| state.observation_filters[idx])
                            {
                                let row_idx = targets.len();
                                let target =
                                    (doc.path.clone(), d.range.as_ref().map_or(0, |r| r.start));
                                if nav_link(
                                    ui,
                                    row_idx,
                                    state.tab_selected,
                                    i18n::diagnostic(locale, &d),
                                ) {
                                    state.tab_selected = row_idx;
                                    jump = Some(target.clone());
                                }
                                targets.push(target);
                            }
                        }
                    }
                });
                handle_list_keys(ui, &mut state.tab_selected, &targets, &mut jump);
            }
            "stage.addresses" => {
                super::addresses::addresses_ui(ui, state, bench, locale, now);
            }
            "stage.illustrations" => {
                let mut targets = Vec::new();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("illustrations-unified")
                        .striped(true)
                        .show(ui, |ui| {
                            for (row_idx, item) in metadata::illustrations(bench.project())
                                .into_iter()
                                .enumerate()
                            {
                                let source = match item.source {
                                    metadata::IllustrationSource::Plot => "stage.illustration_plot",
                                    metadata::IllustrationSource::Contents => {
                                        "stage.illustration_contents"
                                    }
                                };
                                ui.label(label(locale, source));
                                let target = (item.path.clone(), item.at);
                                let link_text = item.id.map_or_else(
                                    || item.path.clone(),
                                    |id| id.display(bench.project().digits()),
                                );
                                if nav_link(ui, row_idx, state.tab_selected, link_text) {
                                    state.tab_selected = row_idx;
                                    jump = Some(target.clone());
                                }
                                targets.push(target);
                                ui.label(item.memo);
                                ui.end_row();
                            }
                        });
                });
                handle_list_keys(ui, &mut state.tab_selected, &targets, &mut jump);
            }
            "stage.matrix" => {
                let rows = metadata::records(bench.project());
                let mut targets = Vec::with_capacity(rows.len());
                egui::ScrollArea::both().show(ui, |ui| {
                    egui::Grid::new("scene-matrix")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(label(locale, "stage.scene"));
                            for field in ["summary", "when", "cast", "place", "senses", "space"] {
                                ui.label(field);
                            }
                            ui.end_row();
                            for (row_idx, row) in rows.iter().enumerate() {
                                let target = (row.path.clone(), row.range.start);
                                if nav_link(
                                    ui,
                                    row_idx,
                                    state.tab_selected,
                                    row.id.display(bench.project().digits()),
                                ) {
                                    state.tab_selected = row_idx;
                                    jump = Some(target.clone());
                                }
                                targets.push(target);
                                for field in ["summary", "when", "cast", "place", "senses", "space"]
                                {
                                    ui.label(row.fields.get(field).map_or("—", String::as_str));
                                }
                                ui.end_row();
                            }
                        });
                });
                handle_list_keys(ui, &mut state.tab_selected, &targets, &mut jump);
            }
            "stage.citations" => {
                let mut targets = Vec::new();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for doc in bench
                        .project()
                        .documents()
                        .filter(|d| d.format == DocumentFormat::Contents)
                    {
                        let mut offset = 0;
                        for raw in doc.text.split_inclusive('\n') {
                            for (_, id) in crate::core::annotations::citations(raw.trim_end()) {
                                let row_idx = targets.len();
                                let target = (doc.path.clone(), offset);
                                if nav_link(
                                    ui,
                                    row_idx,
                                    state.tab_selected,
                                    format!("{id} {}  {}", doc.path, raw.trim()),
                                ) {
                                    state.tab_selected = row_idx;
                                    jump = Some(target.clone());
                                }
                                targets.push(target);
                            }
                            offset += raw.len();
                        }
                    }
                });
                handle_list_keys(ui, &mut state.tab_selected, &targets, &mut jump);
            }
            _ => {
                let rows = metadata::records(bench.project());
                let mut targets = Vec::new();
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
                        for (row_idx, row) in timeline.into_iter().enumerate() {
                            let target = (row.record.path.clone(), row.record.range.start);
                            if nav_link(
                                ui,
                                row_idx,
                                state.tab_selected,
                                format!(
                                    "{}  {}",
                                    row.record.id.display(bench.project().digits()),
                                    row.record.fields.get("when").cloned().unwrap_or_default()
                                ),
                            ) {
                                state.tab_selected = row_idx;
                                jump = Some(target.clone());
                            }
                            targets.push(target);
                        }
                    } else {
                        for row in rows.iter() {
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
                            let row_idx = targets.len();
                            let target = (row.path.clone(), row.range.start);
                            if nav_link(
                                ui,
                                row_idx,
                                state.tab_selected,
                                row.id.display(bench.project().digits()),
                            ) {
                                state.tab_selected = row_idx;
                                jump = Some(target.clone());
                            }
                            targets.push(target);
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
                handle_list_keys(ui, &mut state.tab_selected, &targets, &mut jump);
            }
        }
    });
    let exit_reader = ctx
        .data_mut(|d| d.remove_temp::<bool>(egui::Id::new("reader-exit")))
        .unwrap_or(false);
    if exit_reader || jump.is_some() {
        if workspace == Workspace::Reader {
            bench.save.request_now();
        }
    }
    TabOutcome::Active { jump, exit_reader }
}
