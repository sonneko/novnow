//! `material` — 資料（§10.5 / §16）。
//!
//! - 3 カラム: 左 = 一覧（状態の絞り込みと検索）、中央 = 資料 MD（`Workbench` の Slice でファイル全体を
//!   生のまま編集する、§16.6）、右 = 審査フォーム（参照ペインの位置。`Ctrl+\` で畳める）。
//! - 一覧の検索は `Ctrl+M` と同じ語（§16.3: `#カテゴリ` `@範囲` `'参照` `!status` `~日付` `M数字`）。
//!   却下は既定で隠し、絞り込みの「却下」で出す。索引に無い資料 MD（W064）も一覧に並び、
//!   `Ctrl+Enter` で登録する。
//! - `Ctrl+Enter` = 採用（`inbox/` → `docs/`。出典が無ければ確認を挟む、§16.7）、`Ctrl+Delete` =
//!   却下（ファイルは inbox に残し、ID は回収しない、§5.8.2）、`Ctrl+Shift+N` = 新しい資料（提案）。
//! - 審査フォームの変更は打鍵ごとに索引へ書き戻す（保存は自動保存の待ち時間の後、§8.2）。資料 MD の
//!   保存のたびに索引の `updated_at` と `checksum` を書き換える（§16.6、`Workbench::touch_materials`）。
//!
//! 規則は `PLANS.md` §9.2 B-131・B-134〜B-136。

use egui::{Context, FontFamily, FontId, Frame, Id, Margin, RichText, Sense, Stroke, Ui, Vec2};

use crate::core::features::materials_ops::{self, Review, INDEX_PATH};
use crate::core::model::material::MaterialStatus;
use crate::ui::i18n;
use crate::ui::settings::PaneSide;
use crate::ui::theme::{self, Mode};
use crate::ui::widgets::{reference_pane, slice_editor};
use crate::ui::workbench::{Nav, Workbench};

use super::{list_row, Request, Row, View};

/// 一覧の絞り込み。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// 却下以外と、索引に無い資料 MD（既定。§16.3「rejected は既定で出さない」、B-134）。
    #[default]
    Active,
    /// その状態だけ。
    Only(MaterialStatus),
}

impl Filter {
    /// 並びの順（クリックで選ぶ）。
    pub const ALL: [Filter; 4] = [
        Filter::Active,
        Filter::Only(MaterialStatus::Proposed),
        Filter::Only(MaterialStatus::Approved),
        Filter::Only(MaterialStatus::Rejected),
    ];

    fn label_key(self) -> &'static str {
        match self {
            Filter::Active => "material.filter_active",
            Filter::Only(MaterialStatus::Proposed) => "material.status_proposed",
            Filter::Only(MaterialStatus::Approved) => "material.status_approved",
            Filter::Only(MaterialStatus::Rejected) => "material.status_rejected",
        }
    }

    /// 検索（[`materials_ops::search`]）に足す語。
    fn token(self) -> &'static str {
        match self {
            Filter::Active => "",
            Filter::Only(MaterialStatus::Proposed) => "!proposed",
            Filter::Only(MaterialStatus::Approved) => "!approved",
            Filter::Only(MaterialStatus::Rejected) => "!rejected",
        }
    }
}

/// 一覧の 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 資料 ID（索引に無い資料 MD なら `None`）。
    pub id: Option<String>,
    /// 一覧に出す名前（タイトル。索引に無ければ `materials/` の下のパス）。
    pub title: String,
    /// 資料 MD のパス。
    pub path: String,
    /// 状態（索引に無い・表に無い値なら `None`）。
    pub status: Option<MaterialStatus>,
}

impl Entry {
    /// 選択の鍵（資料 ID、索引に無ければパス）。
    pub fn key(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.path)
    }
}

/// 審査フォーム（打鍵ごとに索引へ書き戻す。索引が外から変わったら作り直す）。
#[derive(Debug, Clone)]
struct Form {
    /// 資料 ID。
    id: String,
    /// 入力中の値。
    draft: Review,
    /// 最後に索引へ書いた入力（同じなら書き直さない）。
    sent: Review,
    /// 索引から読んだ値（これが変われば、フォームの外で変わったので作り直す）。
    applied: Review,
}

/// `Ctrl+Enter` / `Ctrl+Delete` の結果（`NovnowApp` が通知・確認に変える）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// 採用した（資料 ID）。
    Approved(String),
    /// 却下した（資料 ID）。
    Rejected(String),
    /// 索引に無い資料 MD を登録した（資料 ID, パス）。
    Registered(String, String),
    /// 出典が無い（W065）。確認してから `confirmed = true` でもう一度呼ぶ。
    NeedsConfirm(String),
    /// もうその状態（資料 ID）。
    Already(String),
    /// 動かす先に同じ名前の文書がある（そのパス）。
    Occupied(String),
}

/// 一覧の鍵（文書の世代・絞り込み・検索語）。
type ListKey = (u64, Filter, String);

/// `material` の状態（切替で破棄しない、§10.0）。
#[derive(Debug, Default)]
pub struct State {
    /// 選んでいる行（資料 ID、索引に無い資料 MD ならパス）。
    selected: Option<String>,
    /// 絞り込み。
    pub filter: Filter,
    /// 検索語（§16.3 と同じ語）。
    pub query: String,
    entries: Option<(ListKey, Vec<Entry>)>,
    editor: slice_editor::Cache,
    form: Option<Form>,
    reveal: bool,
    /// List highlight moved without opening/committing its text slice.
    preview: bool,
    /// Scroll the list once after a keyboard-only selection change.
    scroll_list: bool,
}

impl State {
    /// 選んでいる行の鍵（資料 ID、索引に無い資料 MD ならパス）。
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// 一覧（必要なら作り直す）。
    pub fn entries(&mut self, bench: &Workbench) -> &[Entry] {
        let key: ListKey = (
            bench.generation(),
            self.filter,
            self.query.trim().to_owned(),
        );
        let fresh = matches!(&self.entries, Some((cached, _)) if *cached == key);
        if !fresh {
            let entries = build(bench, key.1, &key.2);
            self.entries = Some((key, entries));
        }
        self.entries
            .as_ref()
            .map_or(&[], |(_, entries)| entries.as_slice())
    }

    /// 選んでいる行（一覧に無ければ、一覧の最初の行）。
    pub fn current(&mut self, bench: &Workbench) -> Option<Entry> {
        let selected = self.selected.clone();
        let entries = self.entries(bench);
        selected
            .and_then(|key| entries.iter().find(|entry| entry.key() == key))
            .or(entries.first())
            .cloned()
    }

    /// 資料 ID かパス `key` の行を選ぶ。一覧に無ければ検索語を消し、それでも無ければ絞り込みを
    /// その状態にする。選べたら `true`。
    pub fn select(&mut self, bench: &Workbench, key: &str) -> bool {
        let visible =
            |state: &mut Self| state.entries(bench).iter().any(|entry| entry.key() == key);
        if !visible(self) {
            self.query.clear();
            self.filter = Filter::Active;
        }
        if !visible(self) {
            let status = materials_ops::items(bench.project().text(INDEX_PATH))
                .iter()
                .find(|item| item.id == key)
                .and_then(|item| item.status_kind());
            if let Some(status) = status {
                self.filter = Filter::Only(status);
            }
        }
        if !visible(self) {
            return false;
        }
        self.selected = Some(key.to_owned());
        self.preview = false;
        self.reveal = true;
        true
    }

    /// 資料 MD `path` の行を選ぶ（点検からのジャンプ）。
    pub fn select_path(&mut self, bench: &Workbench, path: &str) -> bool {
        let id = materials_ops::items(bench.project().text(INDEX_PATH))
            .into_iter()
            .find(|item| item.path.trim_start_matches("./") == path)
            .map(|item| item.id);
        match id {
            Some(id) => self.select(bench, &id),
            None => self.select(bench, path),
        }
    }

    /// 初めて開いたときの着地（§10.0）: 今の節が明示的に参照している資料、無ければ条件一致の候補、
    /// どちらも無ければ一覧の最初の行（B-134）。
    pub fn land(&mut self, bench: &Workbench, explicit: &[String], candidates: &[String]) {
        for id in explicit.iter().chain(candidates) {
            if self.select(bench, id) {
                return;
            }
        }
    }

    /// 資料 MD のエディタにフォーカスを移す（`Ctrl+2`）。
    pub fn focus_editor(&mut self) {
        self.preview = false;
        self.reveal = true;
    }

    /// 選んだ行の資料 MD をエディタ（Slice）に開く（ファイル全体）。
    pub fn sync(&mut self, bench: &mut Workbench) {
        if self.preview {
            return;
        }
        match self.current(bench) {
            Some(entry) => {
                let count = bench.project().text(&entry.path).split('\n').count();
                let tag = format!("material:{}", entry.path);
                bench.open_slice(&entry.path, 0..count, &tag);
                self.selected = Some(entry.key().to_owned());
            }
            None => bench.close_slice(),
        }
    }

    /// Only move the list highlight; keep the current MD slice and unsaved buffer open.
    pub fn preview_row(&mut self, bench: &Workbench, forward: bool) -> bool {
        let selected = self.selected.clone();
        let entries = self.entries(bench);
        let at = selected.and_then(|key| entries.iter().position(|entry| entry.key() == key));
        let next = if forward {
            at.map_or(Some(0), |index| {
                (index + 1 < entries.len()).then_some(index + 1)
            })
        } else {
            at.and_then(|index| index.checked_sub(1))
        };
        let Some(key) = next
            .and_then(|index| entries.get(index))
            .map(|entry| entry.key().to_owned())
        else {
            return false;
        };
        self.selected = Some(key);
        self.preview = true;
        self.scroll_list = true;
        self.reveal = false;
        true
    }

    /// 前 / 次の資料（`Alt+↑/↓`・`Alt+←/→`・`Ctrl+Home/End`）。
    pub fn nav(&mut self, bench: &mut Workbench, nav: Nav) -> bool {
        self.preview = false;
        bench.commit();
        let selected = self.selected.clone();
        let entries = self.entries(bench).to_vec();
        let index = selected.and_then(|key| entries.iter().position(|entry| entry.key() == key));
        let target = match nav {
            Nav::PrevSection | Nav::PrevScene => index.and_then(|at| at.checked_sub(1)),
            Nav::NextSection | Nav::NextScene => match index {
                Some(at) => (at + 1 < entries.len()).then_some(at + 1),
                None => (!entries.is_empty()).then_some(0),
            },
            Nav::FileStart => (!entries.is_empty()).then_some(0),
            Nav::FileEnd => entries.len().checked_sub(1),
            Nav::PrevFile | Nav::NextFile => None,
        };
        match target {
            Some(found) if Some(found) != index => {
                self.selected = entries.get(found).map(|entry| entry.key().to_owned());
                self.reveal = true;
                bench.save.request_now();
                true
            }
            _ => false,
        }
    }

    /// 選んでいる資料を採用する（§16.1: `inbox/` → `docs/` へ移して `approved`）。索引に無い資料 MD
    /// なら登録する（W064）。出典が無く `confirmed` でなければ [`Outcome::NeedsConfirm`]（§16.7）。
    pub fn approve(
        &mut self,
        bench: &mut Workbench,
        now: f64,
        stamp: &str,
        confirmed: bool,
    ) -> Option<Outcome> {
        bench.commit();
        let entry = self.current(bench)?;
        let index_text = bench.project().text(INDEX_PATH).to_owned();
        let Some(id) = entry.id.clone() else {
            let body = bench.project().text(&entry.path).to_owned();
            let created = materials_ops::register(&index_text, &entry.path, &body, stamp)?;
            bench.replace_document(&entry.path, created.body, now);
            bench.replace_document(INDEX_PATH, created.index, now);
            self.select(bench, &created.id);
            return Some(Outcome::Registered(created.id, entry.path));
        };
        let item = materials_ops::items(&index_text)
            .into_iter()
            .find(|item| item.id == id)?;
        if item.status_kind() == Some(MaterialStatus::Approved) {
            return Some(Outcome::Already(id));
        }
        if !confirmed && materials_ops::has_no_source(&item) {
            return Some(Outcome::NeedsConfirm(id));
        }
        let moved = materials_ops::set_status(&index_text, &id, MaterialStatus::Approved, stamp)?;
        if let Some((from, to)) = &moved.rename {
            if bench.project().document(to).is_some() {
                return Some(Outcome::Occupied(to.clone()));
            }
            if bench.project().document(from).is_some() {
                bench.rename_document(from, to, now);
            }
        }
        bench.replace_document(INDEX_PATH, moved.index, now);
        self.select(bench, &id);
        Some(Outcome::Approved(id))
    }

    /// 選んでいる資料を却下する（`status: rejected`。ファイルは inbox に残す。ID は回収しない）。
    /// 却下したものは既定で見えなくなるので、次の行を選ぶ。索引に無い資料 MD には何もしない。
    pub fn reject(&mut self, bench: &mut Workbench, now: f64, stamp: &str) -> Option<Outcome> {
        bench.commit();
        let entry = self.current(bench)?;
        let id = entry.id.clone()?;
        if entry.status == Some(MaterialStatus::Rejected) {
            return Some(Outcome::Already(id));
        }
        let position = self
            .entries(bench)
            .iter()
            .position(|found| found.key() == id)
            .unwrap_or(0);
        let index_text = bench.project().text(INDEX_PATH).to_owned();
        let moved = materials_ops::set_status(&index_text, &id, MaterialStatus::Rejected, stamp)?;
        bench.replace_document(INDEX_PATH, moved.index, now);
        let entries = self.entries(bench);
        let next = entries
            .iter()
            .position(|found| found.key() == id)
            .map_or_else(
                || entries.get(position).or(entries.last()),
                |at| entries.get(at),
            )
            .map(|found| found.key().to_owned());
        self.selected = next;
        self.reveal = true;
        Some(Outcome::Rejected(id))
    }

    /// 新しい資料を提案として作る（`Ctrl+Shift+N`・`Ctrl+M` の `Ctrl+Enter`）。作った資料を選ぶ。
    pub fn create(
        &mut self,
        bench: &mut Workbench,
        title: &str,
        category: &str,
        now: f64,
        stamp: &str,
    ) -> Option<String> {
        bench.commit();
        let index_text = bench.project().text(INDEX_PATH).to_owned();
        let created = materials_ops::propose_with(&index_text, title, category, "", "", stamp)?;
        bench.replace_document(&created.path, created.body, now);
        bench.replace_document(INDEX_PATH, created.index, now);
        self.select(bench, &created.id);
        Some(created.id)
    }
}

/// 一覧を作る: 索引に無い資料 MD（既定の絞り込みのとき）→ 検索の結果（§16.3 の並び）。
fn build(bench: &Workbench, filter: Filter, query: &str) -> Vec<Entry> {
    let project = bench.project();
    let mut out = Vec::new();
    if filter == Filter::Active {
        let wanted = query.to_lowercase();
        for path in materials_ops::unregistered(project) {
            if !wanted.is_empty() && !path.to_lowercase().contains(&wanted) {
                continue;
            }
            out.push(Entry {
                id: None,
                title: path.trim_start_matches("materials/").to_owned(),
                path,
                status: None,
            });
        }
    }
    let text = format!("{query} {}", filter.token());
    for hit in materials_ops::search(project, text.trim()) {
        out.push(Entry {
            status: MaterialStatus::parse(&hit.status),
            id: Some(hit.id),
            title: hit.title,
            path: hit.path,
        });
    }
    out
}

/// 状態の縦線の色（提案 = 警告色・採用 = 緑・却下 = 淡色・索引に無い = 誤り色）。
fn status_color(mode: Mode, entry: &Entry) -> egui::Color32 {
    let colors = theme::palette(mode);
    match (&entry.id, entry.status) {
        (None, _) => colors.error,
        (_, Some(MaterialStatus::Proposed)) => colors.warn,
        (_, Some(MaterialStatus::Approved)) => colors.ok,
        _ => colors.fg_faint,
    }
}

fn status_key(status: Option<MaterialStatus>) -> &'static str {
    match status {
        Some(MaterialStatus::Proposed) => "material.status_proposed",
        Some(MaterialStatus::Approved) => "material.status_approved",
        Some(MaterialStatus::Rejected) => "material.status_rejected",
        None => "material.status_unknown",
    }
}

/// 1 フレーム。`stamp` は今の時刻（RFC 3339。審査フォームの `updated_at`）。
pub fn show(
    ctx: &Context,
    state: &mut State,
    bench: &mut Workbench,
    view: &View,
    stamp: &str,
) -> Option<Request> {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let locale = view.locale;
    state.sync(bench);
    if let Some(forward) = super::browse_keys(ctx, view) {
        state.preview_row(bench, forward);
    }
    let width = ctx.screen_rect().width();
    super::fixed_width(
        egui::SidePanel::left("material-list"),
        (width * 0.28).clamp(220.0, 360.0),
        8.0,
    )
    .frame(
        Frame::none()
            .fill(colors.bg_base)
            .inner_margin(Margin::symmetric(8.0, 8.0)),
    )
    .show(ctx, |ui| list(ui, state, bench, view));
    let mut request = None;
    if let Some((side, pane_width)) = view.reference {
        let panel = match side {
            PaneSide::Right => egui::SidePanel::right("reference"),
            PaneSide::Left => egui::SidePanel::left("reference"),
        };
        super::fixed_width(panel, pane_width, 14.0)
            .frame(
                Frame::none()
                    .fill(colors.bg_base)
                    .inner_margin(Margin::symmetric(14.0, 8.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_source("material-form-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        request = form(ui, state, bench, view, stamp);
                    });
            });
    }
    egui::CentralPanel::default()
        .frame(
            Frame::none()
                .fill(colors.bg_base)
                .inner_margin(Margin::symmetric(16.0, 8.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if super::action_button(ui, i18n::text(locale, "material.new_button")).clicked() {
                    request = Some(Request::Shortcut(crate::ui::shell::Command::InsertSection));
                }
                // Do not let the long keyboard guide expand the central panel
                // and steal the width reserved for the material review form.
                ui.add(
                    egui::Label::new(
                        RichText::new(i18n::text(locale, "material.keys"))
                            .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                            .color(colors.fg_faint),
                    )
                    .truncate(true),
                );
            });
            super::separator(ui, view.mode);
            match bench.slice().map(|slice| slice.path.clone()) {
                Some(path) => {
                    let missing = bench.project().document(&path).is_none();
                    let (text, color) = if missing {
                        (
                            i18n::format(locale, "material.missing_file", &[&path]),
                            colors.warn,
                        )
                    } else {
                        (path.clone(), colors.fg_faint)
                    };
                    ui.label(
                        RichText::new(text)
                            .font(FontId::new(11.0 * scale, FontFamily::Monospace))
                            .color(color),
                    );
                    ui.add_space(4.0);
                    if state.reveal {
                        let id = slice_editor::editor_id(bench);
                        ctx.memory_mut(|memory| memory.request_focus(id));
                    }
                    let output = slice_editor::show(ui, bench, &mut state.editor, view.look());
                    if output.changed {
                        bench.slice_edited(view.now);
                    }
                }
                None => {
                    super::hint(ui, view.mode, scale, i18n::text(locale, "material.empty"));
                }
            }
            state.reveal = false;
        });
    request
}

/// 左: 絞り込み・検索・一覧。
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
            if super::selectable_action(ui, selected, text).clicked() {
                state.filter = filter;
                state.reveal = true;
            }
        }
    });
    ui.add(
        egui::TextEdit::singleline(&mut state.query)
            .id(Id::new("material-query"))
            .hint_text(i18n::text(locale, "material.query_hint"))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(6.0);
    let entries = state.entries(bench).to_vec();
    if entries.is_empty() {
        super::hint(ui, view.mode, scale, i18n::text(locale, "material.none"));
        return;
    }
    let selected = state
        .selected
        .clone()
        .or_else(|| entries.first().map(|entry| entry.key().to_owned()));
    let mut clicked = None;
    let scroll_list = std::mem::take(&mut state.scroll_list);
    egui::ScrollArea::vertical()
        .id_source("material-list-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for entry in &entries {
                let is_selected = selected.as_deref() == Some(entry.key());
                let response = list_row(
                    ui,
                    view.mode,
                    scale,
                    Row {
                        selected: is_selected,
                        bar: Some(status_color(view.mode, entry)),
                        label: entry.id.as_deref().unwrap_or("—"),
                        text: &entry.title,
                        muted: entry.status == Some(MaterialStatus::Rejected),
                    },
                );
                if is_selected && (state.reveal || scroll_list) {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.clicked() {
                    clicked = Some(entry.key().to_owned());
                }
            }
        });
    if let Some(key) = clicked {
        state.selected = Some(key);
        state.preview = false;
        state.reveal = true;
    }
}

/// 右: 審査フォーム（§16.1「タイトル / カテゴリ / conditions / summary を整える」と出典・タグ）。
fn form(
    ui: &mut Ui,
    state: &mut State,
    bench: &mut Workbench,
    view: &View,
    stamp: &str,
) -> Option<Request> {
    let colors = theme::palette(view.mode);
    let scale = view.scale;
    let locale = view.locale;
    let Some(entry) = state.current(bench) else {
        super::hint(
            ui,
            view.mode,
            scale,
            i18n::text(locale, "material.form_empty"),
        );
        return None;
    };
    let Some(id) = entry.id.clone() else {
        // 索引に無い資料 MD（W064）: 登録のボタンだけ
        ui.label(
            RichText::new(i18n::format(locale, "diag.W064", &[&entry.path]))
                .font(FontId::new(13.0 * scale, FontFamily::Proportional))
                .color(colors.warn),
        );
        ui.add_space(8.0);
        let mut request = None;
        ui.horizontal(|ui| {
            if icon_button(
                ui,
                view.mode,
                Icon::Add,
                i18n::text(locale, "material.register"),
            )
            .clicked()
            {
                request = Some(Request::Down);
            }
        });
        return request;
    };
    let index_text = bench.project().text(INDEX_PATH).to_owned();
    let item = materials_ops::items(&index_text)
        .into_iter()
        .find(|item| item.id == id)?;
    let fresh = Review::of(&item);
    let reset = !matches!(&state.form, Some(form) if form.id == id && form.applied == fresh);
    if reset {
        state.form = Some(Form {
            id: id.clone(),
            draft: fresh.clone(),
            sent: fresh.clone(),
            applied: fresh,
        });
    }
    let form = state.form.as_mut()?;
    // 見出し: ID・状態・日付・追加者
    let status = item.status_kind();
    let status_color = match status {
        Some(MaterialStatus::Proposed) => colors.warn,
        Some(MaterialStatus::Approved) => colors.ok,
        _ => colors.fg_faint,
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(&id)
                .font(FontId::new(13.0 * scale, FontFamily::Monospace))
                .color(colors.fg_primary),
        );
        ui.label(
            RichText::new(i18n::text(locale, status_key(status)))
                .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                .color(status_color),
        );
        let date: String = item.saved_at.chars().take(10).collect();
        ui.label(
            RichText::new(format!("{date}  {}", item.added_by))
                .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                .color(colors.fg_faint),
        );
    });
    if status == Some(MaterialStatus::Proposed) && materials_ops::has_no_source(&item) {
        ui.label(
            RichText::new(i18n::format(locale, "diag.W065", &[&id]))
                .font(FontId::new(12.0 * scale, FontFamily::Proportional))
                .color(colors.warn),
        );
    }
    let font = FontId::new(13.0 * scale, FontFamily::Proportional);
    // フィールド（役割が中身から分からないので見出しを置く。§9.2.1 の 1）
    reference_pane::heading(
        ui,
        i18n::text(locale, "material.field_title"),
        view.mode,
        scale,
    );
    ui.add(
        egui::TextEdit::singleline(&mut form.draft.title)
            .id(Id::new(("material-title", &id)))
            .font(font.clone())
            .desired_width(f32::INFINITY),
    );
    reference_pane::heading(
        ui,
        i18n::text(locale, "material.field_category"),
        view.mode,
        scale,
    );
    ui.horizontal_wrapped(|ui| {
        for category in i18n::text(locale, "material.categories").split(',') {
            let category = category.trim();
            let chosen = form.draft.category == category;
            let text = RichText::new(category)
                .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                .color(if chosen {
                    colors.accent
                } else {
                    colors.fg_faint
                });
            let response = ui.add(
                egui::Button::new(text.underline())
                    .stroke(Stroke::new(
                        if chosen { 1.5_f32 } else { 0.5_f32 },
                        if chosen { colors.accent } else { colors.border },
                    ))
                    .min_size(Vec2::splat(24.0)),
            );
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                form.draft.category = category.to_owned();
            }
        }
    });
    ui.add(
        egui::TextEdit::singleline(&mut form.draft.category)
            .id(Id::new(("material-category", &id)))
            .font(font.clone())
            .desired_width(f32::INFINITY),
    );
    reference_pane::heading(
        ui,
        i18n::text(locale, "material.field_summary"),
        view.mode,
        scale,
    );
    ui.add(
        egui::TextEdit::multiline(&mut form.draft.summary)
            .id(Id::new(("material-summary", &id)))
            .font(font.clone())
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    reference_pane::heading(
        ui,
        i18n::text(locale, "material.field_tags"),
        view.mode,
        scale,
    );
    ui.add(
        egui::TextEdit::singleline(&mut form.draft.tags)
            .id(Id::new(("material-tags", &id)))
            .font(font.clone())
            .hint_text(i18n::text(locale, "material.tags_hint"))
            .desired_width(f32::INFINITY),
    );
    reference_pane::heading(
        ui,
        i18n::text(locale, "material.field_conditions"),
        view.mode,
        scale,
    );
    ui.add(
        egui::TextEdit::multiline(&mut form.draft.conditions)
            .id(Id::new(("material-conditions", &id)))
            .font(FontId::new(12.0 * scale, FontFamily::Monospace))
            .hint_text(i18n::text(locale, "material.conditions_hint"))
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
    reference_pane::heading(
        ui,
        i18n::text(locale, "material.field_source"),
        view.mode,
        scale,
    );
    ui.add(
        egui::TextEdit::singleline(&mut form.draft.source)
            .id(Id::new(("material-source", &id)))
            .font(font)
            .hint_text(i18n::text(locale, "material.source_hint"))
            .desired_width(f32::INFINITY),
    );
    // Open only explicit web URLs. The source field itself remains editable.
    let source = form.draft.source.trim();
    if source.starts_with("https://") || source.starts_with("http://") {
        crate::ui::widgets::external_link(ui, &format!("↗ {source}"), source, &colors);
    }
    ui.add_space(12.0);
    let mut request = None;
    ui.horizontal(|ui| {
        if status != Some(MaterialStatus::Approved)
            && icon_button(
                ui,
                view.mode,
                Icon::Approve,
                i18n::text(locale, "material.approve"),
            )
            .clicked()
        {
            request = Some(Request::Down);
        }
        ui.add_space(8.0);
        if status != Some(MaterialStatus::Rejected)
            && icon_button(
                ui,
                view.mode,
                Icon::Reject,
                i18n::text(locale, "material.reject"),
            )
            .clicked()
        {
            request = Some(Request::Drop);
        }
    });
    ui.add_space(8.0);
    ui.label(
        RichText::new(&item.path)
            .font(FontId::new(11.0 * scale, FontFamily::Monospace))
            .color(colors.fg_faint),
    );
    // 変わった入力を索引へ書き戻す（タイトルを変えたら、資料 MD の H1 も揃っていれば揃える、§5.8.3）
    if form.draft != form.sent {
        let draft = form.draft.clone();
        let old_title = form.sent.title.clone();
        form.sent = draft.clone();
        if let Some(updated) = materials_ops::update(&index_text, &id, &draft, stamp) {
            if let Some(found) = materials_ops::items(&updated)
                .into_iter()
                .find(|item| item.id == id)
            {
                form.applied = Review::of(&found);
            }
            bench.update_document(INDEX_PATH, updated, view.now);
            if draft.title.trim() != old_title.trim() {
                retitle(bench, &item.path, &old_title, &draft.title, view.now);
            }
        }
    }
    request
}

/// 資料 MD の 1 行目の H1 が `old` なら `new` に書き換える（審査フォームでタイトルを変えたとき。
/// 揃っていなければ触らない = W025 のまま）。
fn retitle(bench: &mut Workbench, path: &str, old: &str, new: &str, now: f64) {
    let text = bench.project().text(path);
    let mut lines: Vec<&str> = text.split('\n').collect();
    let Some(first) = lines.first() else {
        return;
    };
    let matches_old = first
        .strip_prefix("# ")
        .is_some_and(|heading| heading.trim() == old.trim());
    if !matches_old || new.trim().is_empty() {
        return;
    }
    let heading = format!("# {}", new.trim().replace('\n', " "));
    lines[0] = &heading;
    let updated = lines.join("\n");
    bench.update_document(path, updated, now);
}

/// 図形のボタン（§9.2.1: ラベルは置かず、ツールチップとアクセシブル名だけ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Icon {
    Approve,
    Reject,
    Add,
}

fn icon_button(ui: &mut Ui, mode: Mode, icon: Icon, name: &str) -> egui::Response {
    let colors = theme::palette(mode);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, name));
    let hovered = response.hovered();
    let response = response.on_hover_text(name);
    let color = match (icon, hovered) {
        (Icon::Approve, _) => colors.ok,
        (Icon::Reject, true) => colors.error,
        (Icon::Reject, false) => colors.fg_muted,
        (Icon::Add, _) => colors.accent,
    };
    let painter = ui.painter_at(rect.expand(2.0));
    if hovered {
        painter.rect_filled(rect, 4.0, colors.bg_selection.gamma_multiply(0.6));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(0.6_f32, colors.border),
    );
    painter.rect_stroke(
        rect.shrink(1.0),
        4.0,
        Stroke::new(1.0_f32, if hovered { color } else { colors.border }),
    );
    let stroke = Stroke::new(1.8_f32, color);
    let c = rect.center();
    let at = |x: f32, y: f32| egui::pos2(c.x + x, c.y + y);
    match icon {
        Icon::Approve => {
            painter.line_segment([at(-6.0, 0.0), at(-2.0, 4.5)], stroke);
            painter.line_segment([at(-2.0, 4.5), at(6.0, -5.0)], stroke);
        }
        Icon::Reject => {
            painter.line_segment([at(-5.0, -5.0), at(5.0, 5.0)], stroke);
            painter.line_segment([at(-5.0, 5.0), at(5.0, -5.0)], stroke);
        }
        Icon::Add => {
            painter.line_segment([at(0.0, -6.0), at(0.0, 6.0)], stroke);
            painter.line_segment([at(-6.0, 0.0), at(6.0, 0.0)], stroke);
        }
    }
    response
}
