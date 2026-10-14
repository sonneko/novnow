//! エージェントの実行の状態（§15.7.4・§15.7.5）: 実行待ちの列・実行中の呼び出し・セッションの累計。
//!
//! 何を書くか・どう決めるかは `core::agents`（純粋）にあり、ここは**いつ**それを呼ぶかと、結果
//! （[`Plan`]）を実行することだけを持つ（egui に依存しない）。
//!
//! - 結果は UI のスレッドで 1 件ずつ処理する（§15.7.5 R3 / R4 / R6: 提案・資料の採番と本文への適用は
//!   逐次。タスクの ID も結果を処理するときに最大値 + 1 で決まる）。
//! - 書き込みは `Workbench::replace_document`（§8 の保存に乗る）、監査ログは `Host::append`（追記だけ）。
//! - 失敗したタスクは `ready` に戻し、自動再試行しない。完了でポップアップを出さない（§15.7.5 (4)）。

use crate::core::agents::files::{self, Board};
use crate::core::agents::mcp_config::{McpConfig, ServerConfig};
use crate::core::agents::runner::{self, Blocker, Reply};
use crate::core::agents::{prompts, Notice, Plan};
use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::id::SceneId;
use crate::core::model::delegation::Delegation;
use crate::core::model::project::PROJECT_FILE;
use crate::ports::mcp::{McpCall, McpError, McpReply, McpServer, DEFAULT_TIMEOUT_MS};
use crate::ui::app::Host;
use crate::ui::i18n::{self, Locale};
use crate::ui::workbench::Workbench;

/// MCP の呼び出し 1 回（`Host::mcp_start` に渡す）。
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    /// サーバ（環境変数は解決済み）。
    pub server: McpServer,
    /// 呼び出し。
    pub call: McpCall,
}

/// 終わった呼び出し: `(呼び出しの番号, 結果, かかったミリ秒)`。
pub type Finished = (u64, Result<McpReply, McpError>, u64);

/// 実行中のタスク。
#[derive(Debug, Clone, PartialEq)]
pub struct Running {
    /// タスク ID。
    pub task: String,
    /// 役割。
    pub role: String,
    /// サーバ名。
    pub server: String,
    /// 対象の節。
    pub scene: Option<SceneId>,
    /// 呼び出しの番号。
    pub job: u64,
    /// 始めた時刻（秒、egui の時計）。
    pub started: f64,
    /// 送った文字数。
    pub input_chars: usize,
    /// 自律（`autonomous` のレベルか、エージェントが起こしたタスク）。
    pub autonomous: bool,
}

/// 実行待ちのタスク: `(タスク, サーバ, 節)`。
pub type Queued = (String, String, Option<SceneId>);

/// エージェントの実行の状態（`Session` が持つ）。
#[derive(Debug, Default)]
pub struct Runtime {
    /// 実行待ち（FIFO）。
    pub queue: Vec<Queued>,
    /// 実行中。
    pub running: Vec<Running>,
    /// セッションの累計と自律の数字（A8）。
    pub counters: runner::Session,
    /// このセッションで終わった数。
    pub finished: usize,
}

impl Runtime {
    /// 走っているか待っているか。
    pub fn busy(&self) -> bool {
        !self.running.is_empty() || !self.queue.is_empty()
    }

    /// タスクが待ち・実行中か。
    pub fn holds(&self, task: &str) -> bool {
        self.queue.iter().any(|(id, _, _)| id == task)
            || self.running.iter().any(|found| found.task == task)
    }
}

/// `agents/mcp.json`。
pub fn config(bench: &Workbench) -> McpConfig {
    runner::config(bench.project())
}

/// サーバの状態（④ の一覧）: `(名前, 解決できない環境変数)`。
pub fn servers(bench: &Workbench, host: &dyn Host) -> Vec<(String, Vec<String>)> {
    config(bench)
        .servers
        .iter()
        .filter(|server| server.is_stdio())
        .map(|server| {
            let missing = server
                .resolve_env(|name| host.env_var(name))
                .err()
                .unwrap_or_default();
            (server.name.clone(), missing)
        })
        .collect()
}

/// 計画を実行する（書き込み → ログ）。
pub fn execute(plan: &Plan, bench: &mut Workbench, host: &mut dyn Host, now: f64) {
    for change in &plan.writes {
        bench.replace_document(&change.path, change.text.clone(), now);
    }
    for event in &plan.log {
        // ログが書けなくても作業は止めない（次の保存で気づける。ログは追記だけ）
        let _ = host.append(&event.path(), &event.line());
    }
}

/// 知らせることの文言。
pub fn notice_text(locale: Locale, notice: &Notice) -> String {
    match notice {
        Notice::AutoApplied { task, target } => {
            i18n::format(locale, "agent.notice.auto_applied", &[task, target])
        }
        Notice::FixedOverwrite { task, target } => {
            i18n::format(locale, "agent.notice.fixed_overwrite", &[task, target])
        }
        Notice::GateRefused { task, codes } => {
            i18n::format(locale, "agent.notice.gate_refused", &[task, codes])
        }
        Notice::Missing { task } => i18n::format(locale, "agent.notice.missing", &[task]),
        Notice::Truncated { task } => i18n::format(locale, "agent.notice.truncated", &[task]),
        Notice::Limit { limit } => i18n::diagnostic(
            locale,
            &Diagnostic::new(DiagnosticCode::W095, None, vec![Arg::text(limit.clone())]),
        ),
        Notice::Duplicate { role, scene } => i18n::diagnostic(
            locale,
            &Diagnostic::new(
                DiagnosticCode::E035,
                None,
                vec![Arg::text(scene.clone()), Arg::text(role.clone())],
            ),
        ),
        Notice::Questions { task, count } => i18n::format(
            locale,
            "agent.notice.questions",
            &[task, &count.to_string()],
        ),
    }
}

/// 実行できない理由の文言。
pub fn blocker_text(locale: Locale, task: &str, blocker: &Blocker) -> String {
    match blocker {
        Blocker::Disabled => i18n::text(locale, "agent.disabled").to_owned(),
        Blocker::Unknown => i18n::format(locale, "agent.blocked.unknown", &[task]),
        Blocker::Off => i18n::format(locale, "agent.blocked.off", &[task]),
        Blocker::Role => i18n::format(locale, "agent.blocked.role", &[task]),
        Blocker::Scope => i18n::format(locale, "agent.blocked.scope", &[task]),
        Blocker::Prompt(found) => found
            .first()
            .map(|diagnostic| i18n::diagnostic(locale, diagnostic))
            .unwrap_or_default(),
        Blocker::NoServer => i18n::format(locale, "agent.blocked.no_server", &[task]),
        Blocker::Env(names) => i18n::diagnostic(
            locale,
            &Diagnostic::new(
                DiagnosticCode::E030,
                None,
                vec![Arg::text(names.join(", "))],
            ),
        ),
        Blocker::Fixed => i18n::format(locale, "agent.blocked.fixed", &[task]),
        Blocker::Over => i18n::format(locale, "agent.blocked.over", &[task]),
    }
}

fn failure_text(locale: Locale, task: &str, error: &McpError) -> String {
    match error {
        McpError::Connect { server, .. } => i18n::diagnostic(
            locale,
            &Diagnostic::new(DiagnosticCode::E063, None, vec![Arg::text(server.clone())]),
        ),
        McpError::Timeout => i18n::format(locale, "agent.failed.timeout", &[task]),
        McpError::Cancelled => i18n::format(locale, "agent.failed.cancelled", &[task]),
        McpError::ToolNotAllowed(name) | McpError::Protocol(name) => {
            i18n::format(locale, "agent.failed.protocol", &[task, name])
        }
    }
}

/// 呼び出しを作る（`sampling` が無く `tools` だけのサーバは `allow_tools` の先頭のツール）。
fn job_of(start: &runner::Start) -> Job {
    let server: &ServerConfig = &start.server;
    let call = if server.samples() || server.allow_tools.is_empty() {
        McpCall::Sampling {
            params: runner::sampling_params(&start.request, &start.task),
        }
    } else {
        McpCall::Tool {
            name: server.allow_tools[0].clone(),
            arguments: runner::tool_arguments(&start.request, &start.task),
        }
    };
    Job {
        server: McpServer {
            name: server.name.clone(),
            command: server.command.clone(),
            args: server.args.clone(),
            env: start.env.clone(),
            timeout_ms: server.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
        },
        call,
    }
}

/// 実行待ちに入れる（`R`・`Shift+R`）。走れないものは理由を返して入れない。`draft` は `ready` にする。
pub fn enqueue(
    ids: &[String],
    bench: &mut Workbench,
    host: &mut dyn Host,
    runtime: &mut Runtime,
    now: f64,
    locale: Locale,
) -> Vec<String> {
    let mut notices = Vec::new();
    if !host.mcp_available() {
        notices.push(i18n::text(locale, "agent.llm_unavailable").to_owned());
        return notices;
    }
    bench.commit();
    let stamp = host.now();
    let title = host.name();
    for id in ids {
        if runtime.holds(id) {
            continue;
        }
        let checked = {
            let env = |name: &str| host.env_var(name);
            runner::prepare(bench.project(), id, &stamp, &title, &env)
        };
        match checked {
            Ok(start) => {
                execute(
                    &runner::ready(bench.project(), id, &stamp),
                    bench,
                    host,
                    now,
                );
                runtime.queue.push((
                    id.clone(),
                    start.server.name.clone(),
                    files::task_scene(&start.task),
                ));
            }
            Err(blocker) => notices.push(blocker_text(locale, id, &blocker)),
        }
    }
    notices
}

/// 1 フレーム分: 終わった呼び出しを処理し、待っているものを走らせる。知らせることを返す。
pub fn tick(
    bench: &mut Workbench,
    host: &mut dyn Host,
    runtime: &mut Runtime,
    now: f64,
    locale: Locale,
) -> Vec<String> {
    let mut notices = Vec::new();
    if !runtime.running.is_empty() {
        for (job, result, duration_ms) in host.mcp_poll() {
            let Some(at) = runtime.running.iter().position(|found| found.job == job) else {
                continue;
            };
            let running = runtime.running.remove(at);
            // 編集中の節を先に書き戻してから、今の中身で結果を処理する（人の打鍵を上書きしない）
            bench.commit();
            let stamp = host.now();
            runtime.finished += 1;
            match result {
                Ok(reply) => {
                    let reply = Reply {
                        text: reply.text,
                        model: reply.model,
                        truncated: reply.truncated,
                        duration_ms,
                        input_chars: running.input_chars,
                    };
                    let plan = runner::finish(
                        bench.project(),
                        &running.task,
                        &reply,
                        &stamp,
                        &mut runtime.counters,
                    );
                    execute(&plan, bench, host, now);
                    notices.extend(
                        plan.notices
                            .iter()
                            .map(|notice| notice_text(locale, notice)),
                    );
                    follow(&plan.follow, bench, runtime);
                }
                Err(McpError::Cancelled) => {}
                Err(error) => {
                    let plan = runner::fail(
                        bench.project(),
                        &running.task,
                        &error.log_text(),
                        duration_ms,
                        &stamp,
                    );
                    execute(&plan, bench, host, now);
                    notices.push(failure_text(locale, &running.task, &error));
                }
            }
        }
    }
    notices.extend(pump(bench, host, runtime, now, locale));
    notices
}

/// 自律の連鎖で起こしたタスクを実行待ちに入れる（`ready` で作られている）。
fn follow(ids: &[String], bench: &Workbench, runtime: &mut Runtime) {
    let board = Board::of(bench.project());
    let config = config(bench);
    for id in ids {
        let Some(task) = board.task(id) else {
            continue;
        };
        let server = config
            .server_for(&task.role)
            .map(|found| found.name.clone())
            .unwrap_or_default();
        runtime
            .queue
            .push((id.clone(), server, files::task_scene(task)));
    }
}

/// 待っているものを、並列度・サーバごとの上限・同じ節の直列化（U-8）の範囲で走らせる。
fn pump(
    bench: &mut Workbench,
    host: &mut dyn Host,
    runtime: &mut Runtime,
    now: f64,
    locale: Locale,
) -> Vec<String> {
    let mut notices = Vec::new();
    let max_parallel = runner::max_parallel(&bench.project().settings().agents);
    loop {
        let slots: Vec<Queued> = runtime
            .running
            .iter()
            .map(|found| (found.task.clone(), found.server.clone(), found.scene))
            .collect();
        let Some(index) = runner::pick(&runtime.queue, &slots, max_parallel) else {
            break;
        };
        let (id, _, _) = runtime.queue.remove(index);
        let stamp = host.now();
        let title = host.name();
        let prepared = {
            let env = |name: &str| host.env_var(name);
            runner::prepare(bench.project(), &id, &stamp, &title, &env)
        };
        let start = match prepared {
            Ok(start) => start,
            Err(blocker) => {
                notices.push(blocker_text(locale, &id, &blocker));
                continue;
            }
        };
        let job = job_of(&start);
        let Some(number) = host.mcp_start(job) else {
            notices.push(i18n::text(locale, "agent.llm_unavailable").to_owned());
            continue;
        };
        execute(&runner::started(&start), bench, host, now);
        runtime.counters.input_chars += start.input_chars as u64;
        runtime.running.push(Running {
            task: id,
            role: start.task.role.clone(),
            server: start.server.name.clone(),
            scene: files::task_scene(&start.task),
            job: number,
            started: now,
            input_chars: start.input_chars,
            autonomous: start.task.delegation == Delegation::Autonomous
                || start.task.initiator == "agent",
        });
    }
    notices
}

/// 中止（`X`）: 走っていれば呼び出しを取り消し、待っていれば列から外して、`canceled` にする。
pub fn cancel(
    id: &str,
    bench: &mut Workbench,
    host: &mut dyn Host,
    runtime: &mut Runtime,
    now: f64,
) {
    bench.commit();
    let mut was_running = false;
    if let Some(at) = runtime.running.iter().position(|found| found.task == id) {
        let running = runtime.running.remove(at);
        host.mcp_cancel(running.job);
        was_running = true;
    }
    runtime.queue.retain(|(task, _, _)| task != id);
    let stamp = host.now();
    execute(
        &runner::cancel(bench.project(), id, was_running, &stamp),
        bench,
        host,
        now,
    );
}

/// 総停止（§15.7.4 `Ctrl+Shift+X` / `>agent stop-all`）: 自律のタスク（`autonomous` のレベルか
/// エージェントが起こしたもの）を止め、書き込めるレベルを `propose` に下げる。
pub fn stop_all(
    bench: &mut Workbench,
    host: &mut dyn Host,
    runtime: &mut Runtime,
    now: f64,
    locale: Locale,
) -> String {
    bench.commit();
    let board = Board::of(bench.project());
    let autonomous = |task: &str| {
        board.task(task).is_some_and(|found| {
            found.delegation == Delegation::Autonomous || found.initiator == "agent"
        })
    };
    let mut stopped: Vec<String> = runtime
        .running
        .iter()
        .filter(|found| found.autonomous || autonomous(&found.task))
        .map(|found| found.task.clone())
        .collect();
    stopped.extend(
        runtime
            .queue
            .iter()
            .filter(|(task, _, _)| autonomous(task))
            .map(|(task, _, _)| task.clone()),
    );
    for id in &stopped {
        cancel(id, bench, host, runtime, now);
    }
    let stamp = host.now();
    let text = bench.project().text(PROJECT_FILE).to_owned();
    let settings = bench.project().settings().agents.clone();
    let mut changed = 0;
    if let Some((json, events)) = runner::stop_all(&text, &settings, &stamp) {
        changed = events.len();
        let mut plan = Plan::default();
        if changed > 0 {
            plan.write(PROJECT_FILE, json);
        }
        plan.log = events;
        execute(&plan, bench, host, now);
    }
    i18n::format(
        locale,
        "agent.stop_all_done",
        &[&stopped.len().to_string(), &changed.to_string()],
    )
}

/// 委譲レベルを変える（④ の `1`〜`4`・`>agent delegation`）。実行中のタスクには影響しない。
pub fn set_level(
    role: &str,
    level: Delegation,
    bench: &mut Workbench,
    host: &mut dyn Host,
    now: f64,
    locale: Locale,
) -> Option<String> {
    bench.commit();
    let settings = bench.project().settings().agents.clone();
    if crate::core::agents::level_of(&settings, role) == level && settings.enabled {
        return None;
    }
    let stamp = host.now();
    let text = bench.project().text(PROJECT_FILE).to_owned();
    let (json, event) = runner::set_level(&text, &settings, role, level, &stamp)?;
    let mut plan = Plan::default();
    plan.write(PROJECT_FILE, json);
    plan.log.push(event);
    execute(&plan, bench, host, now);
    Some(i18n::format(
        locale,
        "agent.level_changed",
        &[role, level.as_str()],
    ))
}

/// 開いたときに欠けた既定のプロンプトを書き出す（U-7・§15.5.4）。`prompts/` が丸ごと無かったら
/// I050 の文言を返す。エージェントが無効なら何もしない。
pub fn write_prompts(
    bench: &mut Workbench,
    host: &dyn Host,
    now: f64,
    locale: Locale,
) -> Option<String> {
    if !bench.project().settings().agents.enabled {
        return None;
    }
    let (missing, absent) = prompts::missing(bench.project());
    let mut written = false;
    for (path, text) in missing {
        // 一覧に無くてもディスクに何かある（外を指すシンボリックリンク）なら触れない（B-155）
        if !host.occupied(path) {
            written |= bench.replace_document(path, text.to_owned(), now);
        }
    }
    (written && absent)
        .then(|| i18n::diagnostic(locale, &Diagnostic::simple(DiagnosticCode::I050, None)))
}

/// プロンプトを既定に戻す（`>prompt reset`。退避しない・undo できない、§15.5.4 (3)）。
pub fn reset_prompts(name: &str, bench: &mut Workbench, host: &dyn Host, now: f64) -> usize {
    let mut count = 0;
    for (path, text) in prompts::reset_targets(name) {
        // 一覧に無いのにディスクにあるもの（外を指すシンボリックリンク）は書かない（B-155）
        if bench.project().document(path).is_none() && host.occupied(path) {
            continue;
        }
        if bench.replace_document(path, text.to_owned(), now) {
            count += 1;
        }
    }
    count
}

/// タスクの未承認メッセージを承認する（`M`、§15.6）。承認した数。
pub fn approve(task: &str, bench: &mut Workbench, now: f64) -> usize {
    bench.commit();
    let board = Board::of(bench.project());
    let Some(found) = board.task(task) else {
        return 0;
    };
    let mut count = 0;
    let mut plan = Plan::default();
    for thread in &board.threads {
        if thread.scope != found.scope {
            continue;
        }
        let mut updated = thread.clone();
        let mut changed = false;
        for message in &mut updated.messages {
            if message.from == found.role && message.approved_by == files::PENDING {
                message.approved_by = "human".to_owned();
                changed = true;
                count += 1;
            }
        }
        if changed {
            plan.write(
                files::thread_path(&updated.id),
                files::write_thread(&updated),
            );
        }
    }
    for change in &plan.writes {
        bench.replace_document(&change.path, change.text.clone(), now);
    }
    count
}

/// ステータスバーの表示（§15.7.4）: `⟳ T0042 writer`・`⟳ 3件`・自律なら `(自律)`。走っていなければ `None`。
pub fn status_label(runtime: &Runtime, locale: Locale) -> Option<String> {
    match runtime.running.as_slice() {
        [] => None,
        [one] => {
            let mut out = format!("\u{27f3} {} {}", one.task, one.role);
            if one.autonomous {
                out.push(' ');
                out.push_str(i18n::text(locale, "agent.status.autonomous"));
            }
            Some(out)
        }
        many => {
            let mut out = format!(
                "\u{27f3} {}",
                i18n::format(locale, "agent.status.count", &[&many.len().to_string()])
            );
            if many.iter().any(|found| found.autonomous) {
                out.push(' ');
                out.push_str(i18n::text(locale, "agent.status.autonomous"));
            }
            Some(out)
        }
    }
}
