//! `idea` — アイデア（§10.4）。
//!
//! - 2 カラム: 左 = 一覧（`status` の絞り込み・タグの絞り込み）、右 = 選んだアイデアのエディタ
//!   （`Workbench` の Slice、見出しから次の見出しの前まで）。
//! - **捕捉が最優先**: `Ctrl+Shift+I` の即時入力はどのワークスペースからでも開く（`ui::app` の重ね物）。
//! - `Ctrl+Enter` で昇格（行き先: プロット節 / 用語 / キャラのメモ / 資料）。転記の後 `status: promoted`
//!   と `promoted-to` を書く。`Ctrl+Delete` は `status: dropped` にするだけで行を消さない。
//! - `status: dropped` は既定で隠す（§5.7）。絞り込みで出せる。

use egui::{Context, FontFamily, FontId, Frame, Margin, RichText, Ui};

use crate::core::features::ideas::{self, IdeaRow};
use crate::core::features::plots;
use crate::core::id::SceneId;
use crate::core::model::idea::IdeaStatus;
use crate::ui::i18n;
use crate::ui::theme::{self, Mode};
use crate::ui::widgets::slice_editor;
use crate::ui::workbench::{Nav, Workbench};

use super::{list_row, Request, Row, View};

/// 一覧の絞り込み（§10.4「`status` フィルタ」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// `dropped` 以外（既定。§5.7「dropped は既定で非表示」）。
    #[default]
    Active,
    /// その状態だけ。
    Only(IdeaStatus),
}

impl Filter {
    /// 並びの順（クリックで選ぶ）。
    pub const ALL: [Filter; 5] = [
        Filter::Active,
        Filter::Only(IdeaStatus::Inbox),
        Filter::Only(IdeaStatus::Keep),
        Filter::Only(IdeaStatus::Promoted),
        Filter::Only(IdeaStatus::Dropped),
    ];

    fn admits(self, status: IdeaStatus) -> bool {
        match self {
            Filter::Active => !status.is_hidden_by_default(),
            Filter::Only(wanted) => status == wanted,
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Filter::Active => "idea.filter_active",
            Filter::Only(IdeaStatus::Inbox) => "idea.status_inbox",
            Filter::Only(IdeaStatus::Keep) => "idea.status_keep",
            Filter::Only(IdeaStatus::Promoted) => "idea.status_promoted",
            Filter::Only(IdeaStatus::Dropped) => "idea.status_dropped",
        }
    }
}

/// 昇格の行き先（§10.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// 話プロットの節（write の今の場面の最後に足す）。
    Plot(SceneId),
    /// 用語（`words.md` の末尾）。
    Term,
    /// キャラのメモ（`characters.md` のそのキャラの最後）。
    Character(usize),
    /// 資料（`materials/inbox/` に提案として作る）。
    Material,
}

/// `idea` の状態（切替で破棄しない、§10.0）。
#[derive(Debug, Default)]
pub struct State {
    /// 選んでいるアイデアの ID。
    selected: Option<String>,
    /// 絞り込み。
    pub filter: Filter,
    /// タグの絞り込み（部分一致。空なら絞らない）。
    pub tag: String,
    rows: Option<(u64, Vec<IdeaRow>)>,
    editor: slice_editor::Cache,
    reveal: bool,
}

impl State {
    /// 選んでいるアイデアの ID。
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// すべてのアイデア（必要なら作り直す）。
    pub fn rows(&mut self, bench: &Workbench) -> &[IdeaRow] {
        let fresh =
            matches!(&self.rows, Some((generation, _)) if *generation == bench.generation());
        if !fresh {
            self.rows = Some((bench.generation(), ideas::rows(bench.project())));
        }
        self.rows.as_ref().map_or(&[], |(_, rows)| rows.as_slice())
    }

    /// 絞り込んだアイデア。
    pub fn visible(&mut self, bench: &Workbench) -> Vec<IdeaRow> {
        let filter = self.filter;
        let tag = self.tag.trim().to_owned();
        self.rows(bench)
            .iter()
            .filter(|row| filter.admits(row.status))
            .filter(|row| tag.is_empty() || row.tags.iter().any(|found| found.contains(&tag)))
            .cloned()
            .collect()
    }

    /// 選んでいるアイデア（絞り込みで見えなければ、見えている最初のもの）。
    pub fn current(&mut self, bench: &Workbench) -> Option<IdeaRow> {
        let visible = self.visible(bench);
        let found = self
            .selected
            .as_ref()
            .and_then(|id| visible.iter().find(|row| &row.id == id))
            .or(visible.first())
            .cloned();
        found
    }

    /// `ideas.md` の行 `line` を含むアイデアを選ぶ（点検からのジャンプ）。見えなければ絞り込みを外す。
    pub fn select_line(&mut self, bench: &Workbench, line: usize) {
        let found = self
            .rows(bench)
            .iter()
            .rev()
            .find(|row| row.lines.start <= line)
            .cloned();
        if let Some(row) = found {
            if !self.filter.admits(row.status) {
                self.filter = Filter::Only(row.status);
            }
            if !self.tag.is_empty() && !row.tags.iter().any(|tag| tag.contains(self.tag.trim())) {
                self.tag.clear();
            }
            self.selected = Some(row.id);
            self.reveal = true;
        }
    }

    /// 選んだアイデアをエディタ（Slice）に開く。
    pub fn sync(&mut self, bench: &mut Workbench) {
        match self.current(bench) {
            Some(row) => {
                let tag = format!("idea:{}:{}", row.id, row.index);
                bench.open_slice(ideas::PATH, row.lines.clone(), &tag);
                self.selected = Some(row.id);
            }
            None => bench.close_slice(),
        }
    }

    /// 前 / 次のアイデア（`Alt+↑/↓`・`Alt+←/→`・`Ctrl+Home/End`）。
    pub fn nav(&mut self, bench: &mut Workbench, nav: Nav) -> bool {
        bench.commit();
        let visible = self.visible(bench);
        let index = self
            .selected
            .as_ref()
            .and_then(|id| visible.iter().position(|row| &row.id == id));
        let target = match nav {
            Nav::PrevSection | Nav::PrevScene => index.and_then(|at| at.checked_sub(1)),
            Nav::NextSection | Nav::NextScene => match index {
                Some(at) => (at + 1 < visible.len()).then_some(at + 1),
                None => (!visible.is_empty()).then_some(0),
            },
            Nav::FileStart => (!visible.is_empty()).then_some(0),
            Nav::FileEnd => visible.len().checked_sub(1),
            Nav::PrevFile | Nav::NextFile => None,
        };
        match target {
            Some(found) if Some(found) != index => {
                self.selected = visible.get(found).map(|row| row.id.clone());
                self.reveal = true;
                bench.save.request_now();
                true
            }
            _ => false,
        }
    }

    /// 選んでいるアイデアを捨てる（`status: dropped`。行は消さない）。捨てたアイデアの ID。
    pub fn drop_selected(&mut self, bench: &mut Workbench, now: f64) -> Option<String> {
        bench.commit();
        let row = self.current(bench)?;
        let text = bench.project().text(ideas::PATH).to_owned();
        let updated = ideas::set_field(&text, row.index, "status", "dropped");
        bench.replace_document(ideas::PATH, updated, now);
        // 捨てたものは既定で見えなくなるので、次のものを選ぶ
        let visible = self.visible(bench);
        self.selected = visible
            .iter()
            .find(|found| found.index > row.index)
            .or(visible.last())
            .map(|found| found.id.clone());
        self.reveal = true;
        Some(row.id)
    }

    /// 選んでいるアイデアを `target` へ昇格する（転記して `status: promoted` と `promoted-to` を書く）。
    /// アイデアの ID と、転記先（`promoted-to` の値）を返す。
    pub fn promote(
        &mut self,
        bench: &mut Workbench,
        target: Target,
        now: f64,
        stamp: &str,
    ) -> Option<(String, String)> {
        bench.commit();
        let row = self.current(bench)?;
        let source = bench.project().text(ideas::PATH).to_owned();
        let (title, body) = ideas::content(&source, row.index)?;
        let digits = bench.project().digits();
        let promoted_to = match target {
            Target::Plot(scene) => {
                let path = bench
                    .project()
                    .episode_plot_path(scene.chapter, scene.episode);
                let mut points = vec![title.clone()];
                if !body.is_empty() {
                    points.push(body.clone());
                }
                let text = bench.project().text(&path).to_owned();
                let (updated, id) =
                    ideas::append_plot_section(&text, scene, &points, digits, bench.project());
                bench.replace_document(&path, updated, now);
                format!("{path}#@{}", plots::short_id(id, digits))
            }
            Target::Term => {
                let text = bench.project().text("words.md").to_owned();
                let updated = ideas::append_term(&text, &title, &body);
                bench.replace_document("words.md", updated, now);
                format!("words.md#{title}")
            }
            Target::Character(index) => {
                let text = bench.project().text("characters.md").to_owned();
                let name = crate::core::syntax::characters_md::parse(&text)
                    .characters
                    .get(index)
                    .map(|found| found.name.trim().to_owned())?;
                let memo = if body.is_empty() {
                    title.clone()
                } else {
                    format!("{title}\n{body}")
                };
                let updated = ideas::append_character_memo(&text, index, &memo);
                bench.replace_document("characters.md", updated, now);
                format!("characters.md#{name}")
            }
            Target::Material => {
                let index_text = bench.project().text("materials/index.json").to_owned();
                let created = crate::core::features::materials_ops::propose(
                    &index_text,
                    &title,
                    &body,
                    stamp,
                )?;
                bench.replace_document(&created.path, created.body.clone(), now);
                bench.replace_document("materials/index.json", created.index.clone(), now);
                format!("{}#{}", created.path, created.id)
            }
        };
        let text = bench.project().text(ideas::PATH).to_owned();
        let text = ideas::set_field(&text, row.index, "status", "promoted");
        let text = ideas::set_field(&text, row.index, "promoted-to", &promoted_to);
        bench.replace_document(ideas::PATH, text, now);
        self.reveal = true;
        Some((row.id, promoted_to))
    }
}

/// 状態の縦線の色（inbox = 警告色・keep = accent・promoted = 緑・dropped = 淡色）。
fn status_color(mode: Mode, status: IdeaStatus) -> egui::Color32 {
    let colors = theme::palette(mode);
    match status {
        IdeaStatus::Inbox => colors.warn,
        IdeaStatus::Keep => colors.accent,
        IdeaStatus::Promoted => colors.ok,
        IdeaStatus::Dropped => colors.fg_faint,
    }
}

/// 1 フレーム。
pub fn show(
    ctx: &Context,
    state: &mut State,
    bench: &mut Workbench,
    view: &View,
) -> Option<Request> {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let locale = view.locale;
    state.sync(bench);
    let width = ctx.screen_rect().width();
    super::fixed_width(
        egui::SidePanel::left("idea-list"),
        (width * 0.36).clamp(240.0, 420.0),
        8.0,
    )
    .frame(
        Frame::none()
            .fill(colors.bg_base)
            .inner_margin(Margin::symmetric(8.0, 8.0)),
    )
    .show(ctx, |ui| list(ui, state, bench, view));
    egui::CentralPanel::default()
        .frame(
            Frame::none()
                .fill(colors.bg_base)
                .inner_margin(Margin::symmetric(16.0, 8.0)),
        )
        .show(ctx, |ui| {
            ui.label(
                RichText::new(i18n::text(locale, "idea.keys"))
                    .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                    .color(colors.fg_faint),
            );
            super::separator(ui, view.mode);
            if bench.slice().is_some() {
                if state.reveal {
                    let id = slice_editor::editor_id(bench);
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
                let output = slice_editor::show(ui, bench, &mut state.editor, view.look());
                if output.changed {
                    bench.slice_edited(view.now);
                }
            } else {
                super::hint(ui, view.mode, scale, i18n::text(locale, "idea.empty"));
            }
            state.reveal = false;
        });
    None
}

/// 左: 絞り込みと一覧。
fn list(ui: &mut Ui, state: &mut State, bench: &Workbench, view: &View) {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let locale = view.locale;
    ui.horizontal_wrapped(|ui| {
        for filter in Filter::ALL {
            let selected = state.filter == filter;
            let text = RichText::new(i18n::text(locale, filter.label_key()))
                .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                .color(if selected {
                    colors.accent
                } else {
                    colors.fg_muted
                });
            if ui.selectable_label(selected, text).clicked() {
                state.filter = filter;
                state.reveal = true;
            }
        }
    });
    ui.add(
        egui::TextEdit::singleline(&mut state.tag)
            .hint_text(i18n::text(locale, "idea.tag_filter"))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);
    let visible = state.visible(bench);
    let selected = state.selected.clone();
    if visible.is_empty() {
        super::hint(ui, view.mode, scale, i18n::text(locale, "idea.none"));
        return;
    }
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_source("idea-list-scroll")
        .auto_shrink([false, false])
        .show_rows(ui, 24.0 * scale, visible.len(), |ui, rows| {
            for row in &visible[rows] {
                let is_selected = selected.as_deref() == Some(row.id.as_str());
                let response = list_row(
                    ui,
                    view.mode,
                    scale,
                    Row {
                        selected: is_selected,
                        bar: Some(status_color(view.mode, row.status)),
                        label: &row.id,
                        text: &row.title,
                        muted: row.status == IdeaStatus::Dropped,
                    },
                );
                if is_selected && state.reveal {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.clicked() {
                    clicked = Some(row.id.clone());
                }
            }
        });
    if let Some(id) = clicked {
        state.selected = Some(id);
        state.reveal = true;
    }
}
