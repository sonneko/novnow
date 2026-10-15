//! `agent` ワークスペース（§15.7、`Alt+8`）: ① タスク一覧 / ② コンテキスト検査 / ③ 結果レビュー /
//! ④ 委譲レベル。
//!
//! 操作（第 2.16 版、`PLANS.md` §9.2 B-150）: テキストの入力欄にフォーカスが無いとき、`←` / `→` で列を
//! 移り、`↑` / `↓` で列の中を選ぶ。修飾キーなしの文字キーはフォーカスのある列のものだけが効く。
//!
//! | 列 | キー |
//! |---|---|
//! | ① | `N` 新規・`R` 実行・`X` 中止・`D` ドライラン・`Space` 選択・`Ctrl+A` 表示中の `ready` を全選択・`Shift+R` 選択をまとめて実行（確認）・`Enter` ③ へ |
//! | ② | `Space` ブロックを外す / 戻す・`D` ドライラン |
//! | ③ | `A` 採用・`P` 一部採用（`↑` / `↓`・`Space`・`Enter`・`Esc`）・`X` 却下 / 差し戻し・`M` メッセージの承認 |
//! | ④ | `1`〜`4` 委譲レベル |
//!
//! 書き込み・実行は `NovnowApp` が [`Action`] を受けて行う（ここは表示と選択だけ）。

use egui::{
    Align, Color32, Context, FontFamily, FontId, Frame, Key, Layout, Margin, Modifiers, RichText,
    ScrollArea, Sense, Stroke, Ui, Vec2,
};

use crate::core::agents::files::{self, Board, Proposal, ProposalStatus};
use crate::core::agents::output::{self, DiffLine};
use crate::core::agents::{prompts, runner, Kind, BUILTIN_ROLES};
use crate::core::aicontext::packet::Packet;
use crate::core::model::delegation::Delegation;
use crate::core::model::task::Task;
use crate::core::resolve::context_rules::PacketBlock;
use crate::ui::agents::Runtime;
use crate::ui::i18n;
use crate::ui::theme;
use crate::ui::workbench::Workbench;

use super::View;

/// 列。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Column {
    /// ① タスク一覧。
    #[default]
    Tasks,
    /// ② コンテキスト検査。
    Context,
    /// ③ 結果レビュー。
    Review,
    /// ④ 委譲レベル。
    Delegation,
}

impl Column {
    fn step(self, forward: bool) -> Self {
        let all = [Self::Tasks, Self::Context, Self::Review, Self::Delegation];
        let at = all.iter().position(|found| *found == self).unwrap_or(0);
        let next = if forward {
            (at + 1).min(all.len() - 1)
        } else {
            at.saturating_sub(1)
        };
        all[next]
    }
}

/// `NovnowApp` に頼むこと。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// `N`: 新しいタスク（重ね物）。
    New,
    /// MCP configuration editor.
    EditMcp,
    /// Stop every active agent task (same as Ctrl+Shift+X).
    StopAll,
    /// `R`: 実行。
    Run(Vec<String>),
    /// `Shift+R`: まとめて実行（確認の重ね物）。
    Confirm(Vec<String>),
    /// `X`（①）: 中止。
    Cancel(String),
    /// `D`: ドライラン。
    DryRun(Vec<String>),
    /// `Space`（②）: ブロックを外す / 戻す。
    Exclude {
        /// タスク。
        task: String,
        /// ブロック。
        block: PacketBlock,
    },
    /// `A` / 一部採用の `Enter`。
    Accept {
        /// 提案。
        proposal: String,
        /// 一部採用のテキスト。
        text: Option<String>,
    },
    /// `X`（③）: 却下か差し戻し（コメントの重ね物）。
    Reject {
        /// 提案。
        proposal: String,
        /// 差し戻しか。
        revert: bool,
    },
    /// `M`: 未承認メッセージの承認。
    Approve(String),
    /// `1`〜`4`: 委譲レベル。
    Level {
        /// 役割。
        role: String,
        /// レベル。
        level: Delegation,
    },
}

/// 描画に要る、`NovnowApp` が持っているもの。
#[derive(Debug, Clone, Copy)]
pub struct Info<'a> {
    /// 実行の状態。
    pub runtime: &'a Runtime,
    /// LLM を呼べる環境か（Web は呼べない、§15.8.2）。
    pub available: bool,
    /// サーバと解決できない環境変数。
    pub servers: &'a [(String, Vec<String>)],
    /// 今の日時（RFC 3339。パケットの `generated_at`）。
    pub stamp: &'a str,
    /// 題（プロジェクトの名前）。
    pub title: &'a str,
    /// 重ね物（確認・入力）が出ている（キーを取らない）。
    pub overlay: bool,
}

/// 一部採用の状態。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Partial {
    proposal: String,
    chosen: Vec<bool>,
    line: usize,
}

/// ワークスペースの状態（`Spaces` が持ち続ける）。
#[derive(Debug, Default)]
pub struct State {
    /// フォーカスのある列。
    pub column: Column,
    /// 選んでいるタスク。
    pub task: Option<String>,
    /// `Space` で選んだタスク。
    pub marked: Vec<String>,
    /// ② で選んでいるブロック（0〜8）。
    pub block: usize,
    /// ④ で選んでいる役割。
    pub role: usize,
    partial: Option<Partial>,
    board: Option<(u64, Board)>,
    packet: Option<((u64, String, String), Option<Packet>)>,
    stale: Option<((u64, String), bool)>,
}

impl State {
    /// `Esc`: 一部採用をやめる。やめたら `true`。
    pub fn escape(&mut self) -> bool {
        self.partial.take().is_some()
    }

    /// 一部採用の最中か。
    pub fn partial(&self) -> bool {
        self.partial.is_some()
    }

    /// タスクを選ぶ（新しく作ったとき・ステータスバーから来たとき）。
    pub fn select(&mut self, task: &str) {
        self.task = Some(task.to_owned());
        self.partial = None;
    }

    fn board(&mut self, bench: &Workbench) -> &Board {
        let generation = bench.generation();
        if self.board.as_ref().is_none_or(|(at, _)| *at != generation) {
            self.board = Some((generation, Board::of(bench.project())));
        }
        &self.board.as_ref().expect("built").1
    }

    fn packet(&mut self, bench: &Workbench, task: &Task, info: &Info<'_>) -> Option<&Packet> {
        let key = (bench.generation(), task.id.clone(), task.exclude.clone());
        if self.packet.as_ref().is_none_or(|(at, _)| *at != key) {
            let built = runner::packet_for(bench.project(), task, info.stamp, info.title);
            self.packet = Some((key, built));
        }
        self.packet.as_ref().and_then(|(_, packet)| packet.as_ref())
    }

    fn stale(&mut self, bench: &Workbench, task: &Task, info: &Info<'_>) -> bool {
        if task.context_hash.is_empty() {
            return false;
        }
        let key = (bench.generation(), task.id.clone());
        if self.stale.as_ref().is_none_or(|(at, _)| *at != key) {
            let now = runner::packet_for(bench.project(), task, info.stamp, info.title)
                .map(|packet| packet.hash());
            let stale = now.is_some_and(|hash| hash != task.context_hash);
            self.stale = Some((key, stale));
        }
        self.stale.as_ref().is_some_and(|(_, stale)| *stale)
    }
}

/// タスクの状態の色（§15.7.1「state 色分け」）。
fn status_color(colors: &theme::Palette, status: &str) -> Color32 {
    match status {
        "ready" | "running" => colors.accent,
        "review" => colors.warn,
        "applied" | "accepted" => colors.ok,
        "rejected" => colors.error,
        _ => colors.fg_faint,
    }
}

/// 1 フレーム。
pub fn show(
    ctx: &Context,
    state: &mut State,
    bench: &mut Workbench,
    view: &View,
    info: &Info<'_>,
) -> Vec<Action> {
    let colors = theme::palette(view.mode);
    let locale = view.locale;
    let mut actions = Vec::new();
    if !bench.project().settings().agents.enabled {
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(colors.bg_base)
                    .inner_margin(Margin::same(32.0)),
            )
            .show(ctx, |ui| {
                // §15.7「エージェントは無効です」とだけ出す（他のワークスペースはそのまま、A6）
                super::hint(
                    ui,
                    view.mode,
                    view.scale,
                    i18n::text(locale, "agent.disabled"),
                );
            });
        return actions;
    }
    let tasks: Vec<Task> = state.board(bench).tasks.iter().rev().cloned().collect();
    if state
        .task
        .as_ref()
        .is_none_or(|id| !tasks.iter().any(|task| &task.id == id))
    {
        state.task = tasks.first().map(|task| task.id.clone());
    }
    let selected = state
        .task
        .as_ref()
        .and_then(|id| tasks.iter().find(|task| &task.id == id))
        .cloned();
    let proposal = selected
        .as_ref()
        .and_then(|task| state.board(bench).proposal_of(&task.id).cloned());
    if !info.overlay {
        keys(
            ctx,
            state,
            &tasks,
            selected.as_ref(),
            proposal.as_ref(),
            &mut actions,
        );
    }
    let margin = Margin::symmetric(10.0, 8.0);
    super::fixed_width(
        egui::SidePanel::left("agent-tasks"),
        290.0 * view.scale,
        margin.left,
    )
    .frame(Frame::none().fill(colors.bg_raised).inner_margin(margin))
    .show(ctx, |ui| {
        tasks_column(ui, state, &tasks, view, info, &mut actions)
    });
    super::fixed_width(
        egui::SidePanel::right("agent-delegation"),
        240.0 * view.scale,
        margin.left,
    )
    .frame(Frame::none().fill(colors.bg_raised).inner_margin(margin))
    .show(ctx, |ui| {
        delegation_column(ui, state, bench, view, info, &mut actions)
    });
    egui::CentralPanel::default()
        .frame(Frame::none().fill(colors.bg_base).inner_margin(margin))
        .show(ctx, |ui| {
            ui.columns(2, |columns| {
                context_column(
                    &mut columns[0],
                    state,
                    bench,
                    selected.as_ref(),
                    view,
                    info,
                    &mut actions,
                );
                review_column(
                    &mut columns[1],
                    state,
                    bench,
                    selected.as_ref(),
                    proposal.as_ref(),
                    view,
                    info,
                );
            });
        });
    if info.runtime.busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
    actions
}

/// キー（テキストの入力欄にフォーカスが無いときだけ）。
fn keys(
    ctx: &Context,
    state: &mut State,
    tasks: &[Task],
    selected: Option<&Task>,
    proposal: Option<&Proposal>,
    actions: &mut Vec<Action>,
) {
    if ctx.memory(|memory| memory.focused().is_some()) {
        return;
    }
    let pressed =
        |key: Key, modifiers: Modifiers| ctx.input_mut(|input| input.consume_key(modifiers, key));
    let ctrl = if ctx.os() == egui::os::OperatingSystem::Mac {
        Modifiers::MAC_CMD
    } else {
        Modifiers::CTRL
    };
    if state.partial.is_none() {
        if pressed(Key::ArrowLeft, Modifiers::NONE) {
            state.column = state.column.step(false);
        }
        if pressed(Key::ArrowRight, Modifiers::NONE) {
            state.column = state.column.step(true);
        }
    }
    let down = pressed(Key::ArrowDown, Modifiers::NONE);
    let up = pressed(Key::ArrowUp, Modifiers::NONE);
    let selected_id = selected.map(|task| task.id.clone());
    let targets = |state: &State| -> Vec<String> {
        if state.marked.is_empty() {
            selected_id.clone().into_iter().collect()
        } else {
            state.marked.clone()
        }
    };
    match state.column {
        Column::Tasks => {
            let at = selected
                .and_then(|task| tasks.iter().position(|found| found.id == task.id))
                .unwrap_or(0);
            if down && at + 1 < tasks.len() {
                state.select(&tasks[at + 1].id);
            }
            if up && at > 0 {
                state.select(&tasks[at - 1].id);
            }
            if pressed(Key::N, Modifiers::NONE) {
                actions.push(Action::New);
            }
            if pressed(Key::R, Modifiers::SHIFT) {
                let ids = targets(state);
                if !ids.is_empty() {
                    actions.push(Action::Confirm(ids));
                }
            }
            if pressed(Key::R, Modifiers::NONE) {
                if let Some(id) = &selected_id {
                    actions.push(Action::Run(vec![id.clone()]));
                }
            }
            if pressed(Key::X, Modifiers::NONE) {
                if let Some(id) = &selected_id {
                    actions.push(Action::Cancel(id.clone()));
                }
            }
            if pressed(Key::D, Modifiers::NONE) {
                let ids = targets(state);
                if !ids.is_empty() {
                    actions.push(Action::DryRun(ids));
                }
            }
            if pressed(Key::Space, Modifiers::NONE) {
                if let Some(id) = &selected_id {
                    if let Some(at) = state.marked.iter().position(|found| found == id) {
                        state.marked.remove(at);
                    } else {
                        state.marked.push(id.clone());
                    }
                }
            }
            if pressed(Key::A, ctrl) {
                state.marked = tasks
                    .iter()
                    .filter(|task| task.status == "ready")
                    .map(|task| task.id.clone())
                    .collect();
            }
            if pressed(Key::Enter, Modifiers::NONE) {
                state.column = Column::Review;
            }
        }
        Column::Context => {
            if down {
                state.block = (state.block + 1).min(PacketBlock::ALL.len() - 1);
            }
            if up {
                state.block = state.block.saturating_sub(1);
            }
            if pressed(Key::Space, Modifiers::NONE) {
                if let Some(id) = &selected_id {
                    actions.push(Action::Exclude {
                        task: id.clone(),
                        block: PacketBlock::ALL[state.block],
                    });
                }
            }
            if pressed(Key::D, Modifiers::NONE) {
                let ids = targets(state);
                if !ids.is_empty() {
                    actions.push(Action::DryRun(ids));
                }
            }
        }
        Column::Review => review_keys(
            ctx,
            state,
            proposal,
            selected_id.as_deref(),
            down,
            up,
            actions,
        ),
        Column::Delegation => {
            if down {
                state.role = (state.role + 1).min(BUILTIN_ROLES.len() - 1);
            }
            if up {
                state.role = state.role.saturating_sub(1);
            }
            for (key, level) in [
                (Key::Num1, Delegation::Off),
                (Key::Num2, Delegation::Propose),
                (Key::Num3, Delegation::Apply),
                (Key::Num4, Delegation::Autonomous),
            ] {
                if pressed(key, Modifiers::NONE) {
                    actions.push(Action::Level {
                        role: BUILTIN_ROLES[state.role].to_owned(),
                        level,
                    });
                }
            }
        }
    }
}

fn review_keys(
    ctx: &Context,
    state: &mut State,
    proposal: Option<&Proposal>,
    task: Option<&str>,
    down: bool,
    up: bool,
    actions: &mut Vec<Action>,
) {
    let pressed =
        |key: Key, modifiers: Modifiers| ctx.input_mut(|input| input.consume_key(modifiers, key));
    let Some(proposal) = proposal else {
        return;
    };
    let status = proposal.status_kind();
    if let Some(partial) = state.partial.as_mut() {
        if partial.proposal != proposal.id {
            state.partial = None;
            return;
        }
        let lines = output::diff(&proposal.before, &proposal.after);
        if down && partial.line + 1 < lines.len() {
            partial.line += 1;
        }
        if up {
            partial.line = partial.line.saturating_sub(1);
        }
        if pressed(Key::Space, Modifiers::NONE) {
            if let Some(line) = lines.get(partial.line) {
                if line.changed() {
                    partial.chosen[partial.line] = !partial.chosen[partial.line];
                }
            }
        }
        if pressed(Key::Enter, Modifiers::NONE) {
            let text = output::merge(&lines, &partial.chosen);
            actions.push(Action::Accept {
                proposal: proposal.id.clone(),
                text: Some(text),
            });
            state.partial = None;
        }
        return;
    }
    if status == Some(ProposalStatus::Open) {
        if pressed(Key::A, Modifiers::NONE) {
            actions.push(Action::Accept {
                proposal: proposal.id.clone(),
                text: None,
            });
        }
        let kind = Kind::parse(&proposal.kind).unwrap_or(Kind::Note);
        if pressed(Key::P, Modifiers::NONE) && matches!(kind, Kind::SectionText | Kind::PlotField) {
            let lines = output::diff(&proposal.before, &proposal.after);
            state.partial = Some(Partial {
                proposal: proposal.id.clone(),
                chosen: vec![false; lines.len()],
                line: lines.iter().position(DiffLine::changed).unwrap_or(0),
            });
        }
        if pressed(Key::X, Modifiers::NONE) {
            actions.push(Action::Reject {
                proposal: proposal.id.clone(),
                revert: false,
            });
        }
    } else if matches!(
        status,
        Some(ProposalStatus::Applied | ProposalStatus::Accepted)
    ) && pressed(Key::X, Modifiers::NONE)
    {
        actions.push(Action::Reject {
            proposal: proposal.id.clone(),
            revert: true,
        });
    }
    if pressed(Key::M, Modifiers::NONE) {
        if let Some(task) = task {
            actions.push(Action::Approve(task.to_owned()));
        }
    }
}

fn heading(ui: &mut Ui, view: &View, text: &str, focused: bool) {
    let colors = theme::palette(view.mode);
    ui.label(
        RichText::new(text)
            .font(FontId::new(12.0 * view.scale, FontFamily::Proportional))
            .color(if focused {
                colors.accent
            } else {
                colors.fg_faint
            }),
    );
}

fn small(view: &View) -> FontId {
    FontId::new(11.0 * view.scale, FontFamily::Proportional)
}

fn mono(view: &View) -> FontId {
    FontId::new(11.0 * view.scale, FontFamily::Monospace)
}

fn tasks_column(
    ui: &mut Ui,
    state: &mut State,
    tasks: &[Task],
    view: &View,
    info: &Info<'_>,
    actions: &mut Vec<Action>,
) {
    let colors = theme::palette(view.mode);
    let locale = view.locale;
    heading(
        ui,
        view,
        i18n::text(locale, "agent.column.tasks"),
        state.column == Column::Tasks,
    );
    let runtime = info.runtime;
    let waiting = runtime.queue.len();
    ui.label(
        RichText::new(i18n::format(
            locale,
            "agent.progress",
            &[
                &runtime.running.len().to_string(),
                &waiting.to_string(),
                &runtime.finished.to_string(),
            ],
        ))
        .font(small(view))
        .color(colors.fg_muted),
    );
    if !info.available {
        ui.label(
            RichText::new(i18n::text(locale, "agent.llm_unavailable"))
                .font(small(view))
                .color(colors.warn),
        );
    }
    if ui.button(i18n::text(locale, "help.edit_mcp")).clicked() {
        actions.push(Action::EditMcp);
    }
    if ui
        .button(i18n::text(locale, "agent.stop_all_button"))
        .clicked()
    {
        actions.push(Action::StopAll);
    }
    super::separator(ui, view.mode);
    if tasks.is_empty() {
        if ui.button(i18n::text(locale, "agent.new_button")).clicked() {
            actions.push(Action::New);
        }
        super::hint(
            ui,
            view.mode,
            view.scale,
            i18n::text(locale, "agent.no_tasks"),
        );
        return;
    }
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 40.0 * view.scale)
        .show(ui, |ui| {
            for task in tasks {
                let selected = state.task.as_deref() == Some(task.id.as_str());
                let running = runtime.running.iter().find(|found| found.task == task.id);
                let queued = runtime.queue.iter().any(|(id, _, _)| id == &task.id);
                let status = if let Some(found) = running {
                    let seconds = (view.now - found.started).max(0.0) as u64;
                    format!(
                        "\u{27f3} {}",
                        i18n::format(locale, "agent.seconds", &[&seconds.to_string()])
                    )
                } else if queued {
                    i18n::text(locale, "agent.queued").to_owned()
                } else {
                    task.status.clone()
                };
                let mark = if state.marked.contains(&task.id) {
                    "\u{25cf} "
                } else {
                    ""
                };
                let scope = task.scope.trim_start_matches('@');
                let mut text = format!("{mark}{}  {}  {scope}  {status}", task.id, task.role);
                if task.initiator == "agent" {
                    text.push_str("  ");
                    text.push_str(i18n::text(locale, "agent.initiator_agent"));
                }
                let response = super::list_row(
                    ui,
                    view.mode,
                    view.scale,
                    super::Row {
                        selected,
                        bar: Some(status_color(&colors, &task.status)),
                        label: "",
                        text: &text,
                        muted: task.status_kind().is_some_and(|found| found.is_final()),
                    },
                );
                if response.clicked() {
                    state.select(&task.id);
                    state.column = Column::Tasks;
                }
                if response.double_clicked() {
                    state.column = Column::Review;
                }
            }
        });
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        ui.label(
            RichText::new(i18n::text(locale, "agent.keys.tasks"))
                .font(small(view))
                .color(colors.fg_faint),
        );
        if ui
            .add(egui::Button::new(i18n::text(locale, "agent.new")).small())
            .clicked()
        {
            actions.push(Action::New);
        }
        if let Some(id) = state.task.as_ref() {
            let targets = if state.marked.is_empty() {
                vec![id.clone()]
            } else {
                state.marked.clone()
            };
            ui.horizontal_wrapped(|ui| {
                if ui.button(i18n::text(locale, "agent.run_button")).clicked() {
                    actions.push(Action::Run(vec![id.clone()]));
                }
                if ui
                    .button(i18n::text(locale, "agent.confirm_button"))
                    .clicked()
                {
                    actions.push(Action::Confirm(targets.clone()));
                }
                if ui
                    .button(i18n::text(locale, "agent.dry_run_button"))
                    .clicked()
                {
                    actions.push(Action::DryRun(targets));
                }
                if ui
                    .button(i18n::text(locale, "agent.cancel_button"))
                    .clicked()
                {
                    actions.push(Action::Cancel(id.clone()));
                }
            });
        }
    });
}

fn context_column(
    ui: &mut Ui,
    state: &mut State,
    bench: &Workbench,
    task: Option<&Task>,
    view: &View,
    info: &Info<'_>,
    actions: &mut Vec<Action>,
) {
    let colors = theme::palette(view.mode);
    let locale = view.locale;
    heading(
        ui,
        view,
        i18n::text(locale, "agent.column.context"),
        state.column == Column::Context,
    );
    let Some(task) = task else {
        super::hint(
            ui,
            view.mode,
            view.scale,
            i18n::text(locale, "agent.no_selection"),
        );
        return;
    };
    let column_focused = state.column == Column::Context;
    let selected_block = state.block;
    let Some(packet) = state.packet(bench, task, info).cloned() else {
        super::hint(
            ui,
            view.mode,
            view.scale,
            i18n::text(locale, "agent.no_packet"),
        );
        return;
    };
    let over = packet.over();
    ui.label(
        RichText::new(i18n::format(
            locale,
            "agent.packet_size",
            &[&packet.size().to_string(), &packet.cap.to_string()],
        ))
        .font(mono(view))
        .color(if over { colors.error } else { colors.fg_muted }),
    );
    if over {
        ui.label(
            RichText::new(i18n::text(locale, "agent.packet_over"))
                .font(small(view))
                .color(colors.error),
        );
    }
    if !packet.prompt.unresolved.is_empty() {
        ui.label(
            RichText::new(i18n::format(
                locale,
                "agent.unresolved",
                &[&packet.prompt.unresolved.join(", ")],
            ))
            .font(small(view))
            .color(colors.fg_faint),
        );
    }
    ui.add_space(4.0);
    for (index, block) in packet.blocks.iter().enumerate() {
        let size = block.size();
        let ratio = if block.budget == 0 {
            if size == 0 {
                0.0
            } else {
                1.5
            }
        } else {
            size as f32 / block.budget as f32
        };
        let selected = column_focused && index == selected_block;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 18.0 * view.scale),
            Sense::click(),
        );
        if response.clicked() {
            state.block = index;
            state.column = Column::Context;
        }
        if response.double_clicked() {
            actions.push(Action::Exclude {
                task: task.id.clone(),
                block: block.kind,
            });
        }
        let painter = ui.painter_at(rect);
        if selected {
            painter.rect_filled(rect, 0.0, colors.bg_selection);
        }
        let label = block.kind.heading().trim_start_matches("## ");
        let mut text = format!("{label}  {size} / {}", block.budget);
        if block.kind.is_mandatory() {
            text.push_str("  ");
            text.push_str(i18n::text(locale, "agent.block.required"));
        }
        if block.excluded {
            text.push_str("  ");
            text.push_str(i18n::text(locale, "agent.block.excluded"));
        } else if block.trimmed() {
            text.push_str("  ");
            text.push_str(i18n::text(locale, "agent.block.trimmed"));
        }
        painter.text(
            rect.left_center() + Vec2::new(4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            mono(view),
            if block.excluded {
                colors.fg_faint
            } else {
                colors.fg_primary
            },
        );
        // 予算のバー（右の 30%）
        let bar = egui::Rect::from_min_max(
            egui::pos2(rect.max.x - rect.width() * 0.3, rect.center().y - 3.0),
            egui::pos2(rect.max.x - 4.0, rect.center().y + 3.0),
        );
        painter.rect_stroke(bar, 0.0, Stroke::new(1.0_f32, colors.border));
        let filled = egui::Rect::from_min_max(
            bar.min,
            egui::pos2(bar.min.x + bar.width() * ratio.min(1.0), bar.max.y),
        );
        let fill = if ratio > 1.0 {
            colors.error
        } else if block.trimmed() {
            colors.warn
        } else {
            colors.accent
        };
        painter.rect_filled(filled, 0.0, fill);
        if block.trimmed() {
            egui::CollapsingHeader::new(
                RichText::new(i18n::text(locale, "agent.block.dropped"))
                    .font(small(view))
                    .color(colors.fg_faint),
            )
            .id_source(("agent-dropped", index))
            .show(ui, |ui| {
                for part in block.parts.iter().filter(|part| part.omitted() > 0) {
                    let dropped: String = part.dropped().chars().take(160).collect();
                    ui.label(
                        RichText::new(format!("{}  -{}  {dropped}", part.label, part.omitted()))
                            .font(small(view))
                            .color(colors.fg_muted),
                    );
                }
            });
        }
    }
    ui.add_space(4.0);
    ui.label(
        RichText::new(i18n::text(locale, "agent.keys.context"))
            .font(small(view))
            .color(colors.fg_faint),
    );
    super::separator(ui, view.mode);
    ScrollArea::vertical()
        .id_source("agent-packet")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label(
                RichText::new(packet.text())
                    .font(mono(view))
                    .color(colors.fg_muted),
            );
        });
}

fn review_column(
    ui: &mut Ui,
    state: &mut State,
    bench: &Workbench,
    task: Option<&Task>,
    proposal: Option<&Proposal>,
    view: &View,
    info: &Info<'_>,
) {
    let colors = theme::palette(view.mode);
    let locale = view.locale;
    heading(
        ui,
        view,
        i18n::text(locale, "agent.column.review"),
        state.column == Column::Review,
    );
    let Some(task) = task else {
        super::hint(
            ui,
            view.mode,
            view.scale,
            i18n::text(locale, "agent.no_selection"),
        );
        return;
    };
    let Some(proposal) = proposal else {
        let key = match task.status.as_str() {
            "running" => "agent.review.running",
            "ready" if info.runtime.holds(&task.id) => "agent.review.queued",
            "canceled" => "agent.review.canceled",
            _ => "agent.review.none",
        };
        super::hint(ui, view.mode, view.scale, i18n::text(locale, key));
        if !task.intent.is_empty() {
            super::hint(ui, view.mode, view.scale, &task.intent);
        }
        return;
    };
    ui.label(
        RichText::new(format!(
            "{}  {}  {}",
            proposal.id, proposal.kind, proposal.status
        ))
        .font(mono(view))
        .color(status_color(
            &colors,
            match proposal.status.as_str() {
                "open" => "review",
                "superseded" => "canceled",
                other => other,
            },
        )),
    );
    ui.label(
        RichText::new(&proposal.target)
            .font(small(view))
            .color(colors.fg_faint),
    );
    let mut warnings: Vec<String> = Vec::new();
    if proposal.status == "open" && state.stale(bench, task, info) {
        warnings.push(i18n::text(locale, "agent.review.stale").to_owned());
    }
    if proposal.status == "open" && crate::core::agents::apply::drifted(bench.project(), proposal) {
        warnings.push(i18n::text(locale, "agent.review.drifted").to_owned());
    }
    if crate::core::agents::apply::is_fixed(bench.project(), proposal) {
        warnings.push(i18n::text(locale, "agent.review.fixed").to_owned());
    }
    let result = bench
        .project()
        .document(&files::result_path(&task.id))
        .and_then(|document| files::parse_result(&document.text));
    let parsed = result.as_ref().map(|found| output::parse(&found.output));
    if let (Some(result), Some(parsed)) = (result.as_ref(), parsed.as_ref()) {
        if result.truncated {
            warnings.push(i18n::text(locale, "agent.review.truncated").to_owned());
        }
        let kind = Kind::of_role(&task.role);
        let packet_target = state
            .packet(bench, task, info)
            .and_then(|packet| packet.target_chars);
        for violation in output::violations(kind, parsed, packet_target) {
            warnings.push(i18n::text(locale, violation_key(violation)).to_owned());
        }
    }
    for warning in &warnings {
        ui.label(RichText::new(warning).font(small(view)).color(colors.warn));
    }
    if let Some(parsed) = &parsed {
        for question in &parsed.questions {
            ui.label(
                RichText::new(format!("? {question}"))
                    .font(small(view))
                    .color(colors.accent),
            );
        }
    }
    let pending = state
        .board(bench)
        .threads
        .iter()
        .filter(|thread| thread.scope == task.scope)
        .flat_map(|thread| thread.messages.iter())
        .filter(|message| message.from == task.role && message.approved_by == files::PENDING)
        .count();
    if pending > 0 {
        ui.label(
            RichText::new(i18n::format(
                locale,
                "agent.review.pending",
                &[&pending.to_string()],
            ))
            .font(small(view))
            .color(colors.accent),
        );
    }
    if !proposal.comment.is_empty() {
        ui.label(
            RichText::new(format!("// {}", proposal.comment))
                .font(small(view))
                .color(colors.fg_muted),
        );
    }
    let others: Vec<String> = state
        .board(bench)
        .proposals
        .iter()
        .filter(|other| {
            other.id != proposal.id && other.target == proposal.target && other.status == "open"
        })
        .map(|other| other.id.clone())
        .collect();
    if !others.is_empty() {
        ui.label(
            RichText::new(i18n::format(
                locale,
                "agent.review.others",
                &[&others.join(", ")],
            ))
            .font(small(view))
            .color(colors.fg_faint),
        );
    }
    let keys = match proposal.status_kind() {
        Some(ProposalStatus::Open) => "agent.keys.review",
        Some(ProposalStatus::Applied | ProposalStatus::Accepted) => "agent.keys.revert",
        _ => "agent.keys.settled",
    };
    ui.label(
        RichText::new(i18n::text(locale, keys))
            .font(small(view))
            .color(colors.fg_faint),
    );
    super::separator(ui, view.mode);
    let kind = Kind::parse(&proposal.kind).unwrap_or(Kind::Note);
    let partial = state
        .partial
        .clone()
        .filter(|found| found.proposal == proposal.id);
    ScrollArea::vertical()
        .id_source("agent-diff")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if !matches!(kind, Kind::SectionText | Kind::PlotField) {
                ui.label(
                    RichText::new(&proposal.after)
                        .font(FontId::new(13.0 * view.scale, FontFamily::Proportional))
                        .color(colors.fg_primary),
                );
                return;
            }
            let lines = output::diff(&proposal.before, &proposal.after);
            for (index, line) in lines.iter().enumerate() {
                let (sign, color, fill) = match line {
                    DiffLine::Same(_) => (" ", colors.fg_muted, None),
                    DiffLine::Removed(_) => {
                        ("-", colors.error, Some(colors.error.gamma_multiply(0.12)))
                    }
                    DiffLine::Added(_) => ("+", colors.ok, Some(colors.ok.gamma_multiply(0.12))),
                };
                let check = match &partial {
                    Some(found) if line.changed() => {
                        if found.chosen.get(index).copied().unwrap_or(false) {
                            "\u{2611} "
                        } else {
                            "\u{2610} "
                        }
                    }
                    _ => "",
                };
                let cursor = partial.as_ref().is_some_and(|found| found.line == index);
                let text = RichText::new(format!("{check}{sign} {}", line.text()))
                    .font(FontId::new(12.0 * view.scale, FontFamily::Proportional))
                    .color(color);
                let frame = Frame::none()
                    .fill(if cursor {
                        colors.bg_selection
                    } else {
                        fill.unwrap_or(Color32::TRANSPARENT)
                    })
                    .inner_margin(Margin::symmetric(4.0, 1.0));
                frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(text);
                });
            }
        });
}

fn violation_key(violation: output::Violation) -> &'static str {
    match violation {
        output::Violation::Empty => "agent.violation.empty",
        output::Violation::Preamble => "agent.violation.preamble",
        output::Violation::Heading => "agent.violation.heading",
        output::Violation::CodeFence => "agent.violation.code_fence",
        output::Violation::Annotation => "agent.violation.annotation",
        output::Violation::Length => "agent.violation.length",
    }
}

fn delegation_column(
    ui: &mut Ui,
    state: &mut State,
    bench: &Workbench,
    view: &View,
    info: &Info<'_>,
    actions: &mut Vec<Action>,
) {
    let colors = theme::palette(view.mode);
    let locale = view.locale;
    heading(
        ui,
        view,
        i18n::text(locale, "agent.column.delegation"),
        state.column == Column::Delegation,
    );
    let settings = &bench.project().settings().agents;
    for (index, role) in BUILTIN_ROLES.iter().enumerate() {
        let level = crate::core::agents::level_of(settings, role);
        let path = files::prompt_path(role);
        let mut text = format!("{role}  {}", level.as_str());
        if prompts::edited(bench.project(), &path) {
            text.push_str("  ");
            text.push_str(i18n::text(locale, "agent.prompt_edited"));
        }
        if !prompts::blockers(bench.project(), role).is_empty() {
            text.push_str("  \u{26a0}");
        }
        let bar = match level {
            Delegation::Off => colors.fg_faint,
            Delegation::Propose => colors.accent,
            Delegation::Apply => colors.warn,
            Delegation::Autonomous => colors.error,
        };
        let response = super::list_row(
            ui,
            view.mode,
            view.scale,
            super::Row {
                selected: state.column == Column::Delegation && state.role == index,
                bar: Some(bar),
                label: "",
                text: &text,
                muted: level == Delegation::Off,
            },
        );
        if response.clicked() {
            state.role = index;
            state.column = Column::Delegation;
        }
    }
    ui.horizontal_wrapped(|ui| {
        for (label, level) in [
            ("1 off", Delegation::Off),
            ("2 propose", Delegation::Propose),
            ("3 apply", Delegation::Apply),
            ("4 autonomous", Delegation::Autonomous),
        ] {
            if ui.button(label).clicked() {
                actions.push(Action::Level {
                    role: BUILTIN_ROLES[state.role].to_owned(),
                    level,
                });
            }
        }
    });
    super::separator(ui, view.mode);
    let counters = &info.runtime.counters;
    ui.label(
        RichText::new(i18n::format(
            locale,
            "agent.totals",
            &[
                &counters.calls.to_string(),
                &counters.input_chars.to_string(),
                &counters.output_chars.to_string(),
            ],
        ))
        .font(small(view))
        .color(colors.fg_muted),
    );
    super::separator(ui, view.mode);
    heading(ui, view, i18n::text(locale, "agent.servers"), false);
    if info.servers.is_empty() {
        ui.label(
            RichText::new(i18n::text(locale, "agent.no_servers"))
                .font(small(view))
                .color(colors.fg_faint),
        );
    }
    for (name, missing) in info.servers {
        let (text, color) = if missing.is_empty() {
            (name.clone(), colors.fg_primary)
        } else {
            (
                format!(
                    "{name}  {}",
                    i18n::diagnostic(
                        locale,
                        &crate::core::agents::mcp_config::unresolved(missing)
                    )
                ),
                colors.error,
            )
        };
        ui.label(RichText::new(text).font(small(view)).color(color));
    }
    let _ = actions;
}
