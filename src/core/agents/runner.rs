//! タスクの実行（§15.3・§15.7.5・§15.8.1）の純粋な部分。
//!
//! - [`new_tasks`] — 起票（`N`。場面・話のスコープは「節タスクの束」に展開する。E035 の重複は作らない）。
//! - [`prepare`] — 実行の前の検査と、送るもの（パケット・要求・サーバ・解決した環境変数）。
//! - [`finish`] / [`fail`] / [`cancel`] — 実行の後に書くもの（[`Plan`]）。自動適用（§15.1.1）と
//!   自律の連鎖（integrator が `autonomous` のときだけ、上限つき）もここで決める。
//! - [`pick`] — 次に走らせるタスク（U-8: `max_parallel`・サーバごとに最大 2・同じ節は直列）。
//! - [`set_level`] / [`stop_all`] — 委譲レベルの変更（`project.json` の書き換えと `delegation_changed`）。
//!
//! MCP の呼び出しそのもの（プロセス・スレッド・タイムアウト）は adapters が持つ。ここは要求の JSON を
//! 作り、応答の JSON を読むだけ。

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::core::agents::files::{self, Board, LogEvent, Proposal, ProposalStatus, RunResult};
use crate::core::agents::mcp_config::{McpConfig, ServerConfig};
use crate::core::agents::output::{self, Parsed};
use crate::core::agents::{apply, level_of, prompts, Kind, Notice, Plan};
use crate::core::aicontext::packet::{self, Inputs, Packet, Request};
use crate::core::diagnostics::Diagnostic;
use crate::core::features::{sections, Project};
use crate::core::id::{PartialSceneId, SceneId};
use crate::core::model::delegation::Delegation;
use crate::core::model::project::{AgentLimits, Agents};
use crate::core::model::task::Task;
use crate::core::syntax::plot_episode;

/// 自律の連鎖で起こす integrator のタスクの `intent`（タスクのファイルに書く語）。
/// i18n: format-vocabulary
pub const CHAIN_INTENT: &str = "次の 1 手";

/// MCP の `maxTokens`（20,000 字の出力に足りる量）。
pub const MAX_TOKENS: u64 = 16_000;

/// `agents.max_parallel` を 1〜8 に収める（§15.7.5 (2)）。
pub fn max_parallel(settings: &Agents) -> usize {
    let (low, high) = crate::core::agents::MAX_PARALLEL_RANGE;
    settings.max_parallel.clamp(low, high) as usize
}

/// 「生きている」タスク（E035 の重複の判定に数える。`review` は人間の番なので数えない）。
pub fn is_active(task: &Task) -> bool {
    matches!(task.status.as_str(), "draft" | "ready" | "running")
}

/// 同じ役割・同じ節の生きているタスク（E035）。
pub fn duplicate<'a>(board: &'a Board, role: &str, scene: SceneId) -> Option<&'a Task> {
    board
        .tasks
        .iter()
        .find(|task| task.role == role && is_active(task) && files::task_scene(task) == Some(scene))
}

/// スコープ（`@01-04` など）を節に展開する（本文と話プロットにある節、ID 順）。
pub fn expand_scope(project: &Project, scope: &str) -> Vec<SceneId> {
    let Ok(partial) = PartialSceneId::parse_with_sigil(scope.trim()) else {
        return Vec::new();
    };
    if partial.depth() == 4 {
        return vec![partial.expand_low()];
    }
    let (low, high) = (partial.expand_low(), partial.expand_high());
    let mut out: Vec<SceneId> = sections::all(project)
        .into_iter()
        .map(|found| found.id)
        .filter(|id| low <= *id && *id <= high)
        .collect();
    for document in project.documents() {
        if document.format != crate::core::model::document::DocumentFormat::PlotEpisode {
            continue;
        }
        let plot = plot_episode::parse_with(&document.text, &project.contents_options());
        for found in &plot.scenes {
            let Some(id) = found.id else {
                continue;
            };
            for section in &found.sections {
                let id = SceneId {
                    section: section.number,
                    ..id
                };
                if low <= id && id <= high {
                    out.push(id);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// 起票の中身。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft<'a> {
    /// 役割。
    pub role: &'a str,
    /// スコープ（`@01-04-02-03`、場面・話も可）。
    pub scope: &'a str,
    /// 目的。
    pub intent: &'a str,
    /// 人間のコメント。
    pub comment: &'a str,
    /// Optional reference examples, one per line.
    pub examples: &'a str,
    /// 起動者（`human` / `agent`）。
    pub initiator: &'a str,
    /// 状態（人間の `N` は `draft`、連鎖は `ready`）。
    pub status: &'a str,
}

/// 起票する（節ごとに 1 件。E035 の重複は作らずに知らせる）。
pub fn new_tasks(project: &Project, draft: &Draft<'_>, now: &str) -> Plan {
    let board = Board::of(project);
    let mut paths = files::paths(project);
    let digits = project.digits();
    let mut plan = Plan::default();
    for scene in expand_scope(project, draft.scope) {
        let scope = format!("@{}", scene.display(digits));
        if duplicate(&board, draft.role, scene).is_some() {
            plan.notices.push(Notice::Duplicate {
                role: draft.role.to_owned(),
                scene: scene.display(digits),
            });
            continue;
        }
        let id = files::next_task_id(paths.iter().map(String::as_str));
        let task = Task {
            id: id.clone(),
            role: draft.role.to_owned(),
            scope,
            intent: crate::core::agents::one_line(draft.intent),
            status: draft.status.to_owned(),
            initiator: draft.initiator.to_owned(),
            delegation: level_of(&project.settings().agents, draft.role),
            prompt: files::prompt_path(draft.role),
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
            human_comment: crate::core::agents::one_line(draft.comment),
            examples: draft
                .examples
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
            ..Task::default()
        };
        let path = files::task_path(&id);
        plan.write(path.clone(), files::write_task(&task));
        paths.push(path);
    }
    plan
}

// ============================================================ 実行の前

/// 実行できない理由（文言は UI）。
#[derive(Debug, Clone, PartialEq)]
pub enum Blocker {
    /// `agents.enabled = false`。
    Disabled,
    /// タスクが無い・もう終わっている。
    Unknown,
    /// 役割の委譲レベルが `off`。
    Off,
    /// 既定の役割でない（カスタムロールは ②）。
    Role,
    /// 対象が 1 節でない（P1）。
    Scope,
    /// プロンプトが壊れている（E029 / E031）。
    Prompt(Vec<Diagnostic>),
    /// 役割を受け持つサーバが無い。
    NoServer,
    /// 環境変数が無い（E030）。
    Env(Vec<String>),
    /// `propose` の役割が確定済みの節を対象にした（A7）。
    Fixed,
    /// パケットが上限を超えたまま（§15.9）。
    Over,
}

/// 送るもの一式。
#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    /// `running` にしたタスク。
    pub task: Task,
    /// パケット。
    pub packet: Packet,
    /// 要求。
    pub request: Request,
    /// サーバ。
    pub server: ServerConfig,
    /// 解決した環境変数。
    pub env: Vec<(String, String)>,
    /// 送る文字数（system + user）。
    pub input_chars: usize,
}

/// パケットを組み立てる（ドライランと実行で同じ）。
pub fn packet_for(project: &Project, task: &Task, now: &str, title: &str) -> Option<Packet> {
    let scene = files::task_scene(task)?;
    let excluded = files::excluded_blocks(task);
    Some(packet::build(&Inputs {
        project,
        role: &task.role,
        scene,
        delegation: level_of(&project.settings().agents, &task.role),
        generated_at: now,
        title,
        excluded: &excluded,
        task: Some(&task.id),
        comment: &task.human_comment,
        examples: &task.examples,
    }))
}

/// ドライランのファイル（§15.7.2: パケットだけ）。
pub fn dry_run(project: &Project, id: &str, now: &str, title: &str) -> Option<(String, String)> {
    let board = Board::of(project);
    let task = board.task(id)?;
    let packet = packet_for(project, task, now, title)?;
    Some((files::dryrun_path(id), packet.text()))
}

/// `agents/mcp.json` を読む（無い・壊れていれば空）。
pub fn config(project: &Project) -> McpConfig {
    crate::core::agents::mcp_config::parse(project.text(crate::core::agents::mcp_config::PATH))
        .unwrap_or_default()
}

/// 実行の前の検査と、送るもの。
pub fn prepare(
    project: &Project,
    id: &str,
    now: &str,
    title: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Start, Blocker> {
    let settings = &project.settings().agents;
    if !settings.enabled {
        return Err(Blocker::Disabled);
    }
    let board = Board::of(project);
    let task = board.task(id).ok_or(Blocker::Unknown)?;
    if !matches!(task.status.as_str(), "draft" | "ready") {
        return Err(Blocker::Unknown);
    }
    if !super::roles(project).contains(&task.role) {
        return Err(Blocker::Role);
    }
    let level = level_of(settings, &task.role);
    if level == Delegation::Off {
        return Err(Blocker::Off);
    }
    let scene = files::task_scene(task).ok_or(Blocker::Scope)?;
    let blockers = prompts::blockers(project, &task.role);
    if !blockers.is_empty() {
        return Err(Blocker::Prompt(blockers));
    }
    if level == Delegation::Propose
        && Kind::of_role(&task.role) != Kind::Note
        && crate::core::export::effective_state(project, scene) == crate::core::export::State::Fixed
    {
        return Err(Blocker::Fixed);
    }
    let config = config(project);
    let server = config.server_for(&task.role).ok_or(Blocker::NoServer)?;
    let resolved = server.resolve_env(env).map_err(Blocker::Env)?;
    let packet = packet_for(project, task, now, title).ok_or(Blocker::Scope)?;
    if packet.over() {
        return Err(Blocker::Over);
    }
    let request = packet.request();
    let input_chars =
        crate::core::agents::chars(&request.system) + crate::core::agents::chars(&request.user);
    let mut running = task.clone();
    running.status = "running".to_owned();
    running.delegation = level;
    running.prompt = files::prompt_path(&task.role);
    running.updated_at = now.to_owned();
    running.context_hash = packet.hash();
    running.server = server.name.clone();
    Ok(Start {
        task: running,
        packet,
        request,
        server: server.clone(),
        env: resolved,
        input_chars,
    })
}

/// MCP の `sampling/createMessage` の `params`（§15.8.1）。
pub fn sampling_params(request: &Request, task: &Task) -> Value {
    let (speed, intelligence) = match request.model_hint.as_str() {
        "fast" => (1.0, 0.2),
        "careful" => (0.2, 1.0),
        _ => (0.5, 0.5),
    };
    let mut params = json!({
        "messages": [{
            "role": "user",
            "content": { "type": "text", "text": request.user }
        }],
        "systemPrompt": request.system,
        "includeContext": "none",
        "maxTokens": MAX_TOKENS,
        "modelPreferences": {
            "speedPriority": speed,
            "intelligencePriority": intelligence
        },
        "metadata": {
            "novnow": { "task": task.id, "role": task.role, "scope": task.scope }
        }
    });
    if let Some(temperature) = request.temperature {
        params["temperature"] = json!(temperature);
    }
    params
}

/// `tools` だけのサーバに送る `tools/call` の `arguments`（`allow_tools` の先頭のツール。
/// `PLANS.md` §9.2 B-147）。
pub fn tool_arguments(request: &Request, task: &Task) -> Value {
    json!({
        "system": request.system,
        "prompt": request.user,
        "role": task.role,
        "scope": task.scope,
    })
}

/// 応答の本文とモデル名（`sampling/createMessage` の結果か `tools/call` の結果）。読めなければ `None`。
pub fn reply_of(result: &Value) -> Option<(String, String)> {
    let model = result
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let content = result.get("content")?;
    let texts: Vec<&str> = match content {
        Value::Array(items) => items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect(),
        Value::Object(_) => content
            .get("text")
            .and_then(Value::as_str)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    };
    if texts.is_empty() {
        return None;
    }
    Some((texts.join("\n"), model))
}

// ============================================================ セッション（A8）

/// 自律の上限（§4.5.1 `agents.limits`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Limit {
    /// `max_tasks_per_session`（自動で起こしたタスクの数）。
    Tasks,
    /// `max_chain_hops`（1 つの連鎖のホップ数）。
    Hops,
    /// `max_input_chars_per_session`（送った文字数の合計）。
    InputChars,
}

impl Limit {
    /// `project.json` のキー（W095 の `{0}`）。
    pub fn key(self) -> &'static str {
        match self {
            Self::Tasks => "max_tasks_per_session",
            Self::Hops => "max_chain_hops",
            Self::InputChars => "max_input_chars_per_session",
        }
    }
}

/// セッションの数字（アプリを開いている間だけ。§15.7.4 の累計と A8 の上限）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    /// 呼び出しの回数。
    pub calls: u64,
    /// 送った文字数の合計。
    pub input_chars: u64,
    /// 受け取った文字数の合計。
    pub output_chars: u64,
    /// 自動で起こしたタスクの数。
    pub auto_started: u32,
    /// タスク → 連鎖のホップ数（人間が起こしたタスクは 0）。
    pub hops: BTreeMap<String, u32>,
}

/// 自動で起こす前の検査（0 は無制限、`PLANS.md` §9.2 B-143）。
pub fn limit(limits: &AgentLimits, session: &Session, hop: u32) -> Option<Limit> {
    if limits.max_tasks_per_session > 0 && session.auto_started >= limits.max_tasks_per_session {
        return Some(Limit::Tasks);
    }
    if limits.max_chain_hops > 0 && hop > limits.max_chain_hops {
        return Some(Limit::Hops);
    }
    if limits.max_input_chars_per_session > 0
        && session.input_chars >= limits.max_input_chars_per_session
    {
        return Some(Limit::InputChars);
    }
    None
}

// ============================================================ 実行の後

/// MCP の応答（adapters が読んだもの）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reply {
    /// 本文（20,000 字で切った後）。
    pub text: String,
    /// モデル名。
    pub model: String,
    /// 切ったか。
    pub truncated: bool,
    /// かかった時間。
    pub duration_ms: u64,
    /// 送った文字数。
    pub input_chars: usize,
}

/// 実行が終わったときに書くもの。`session` は累計と連鎖の数字を進める。
pub fn finish(
    project: &Project,
    id: &str,
    reply: &Reply,
    now: &str,
    session: &mut Session,
) -> Plan {
    let mut plan = Plan::default();
    let board = Board::of(project);
    let Some(task) = board.task(id) else {
        return plan;
    };
    let mut task = task.clone();
    let digits = project.digits();
    let mut paths = files::paths(project);
    let parsed = output::parse(&reply.text);
    let scene = files::task_scene(&task);
    let mut kind = Kind::of_role(&task.role);
    if parsed.main.trim().is_empty() {
        kind = Kind::Note;
    }
    // 提案
    let proposal_id = files::next_proposal_id(paths.iter().map(String::as_str));
    paths.push(files::proposal_path(&proposal_id));
    let target = match (kind, scene) {
        (Kind::SectionText | Kind::Note, Some(scene)) => files::target_of(
            &project.contents_path(scene.chapter, scene.episode),
            Some(scene),
            digits,
        ),
        (Kind::PlotField, Some(scene)) => files::target_of(
            &project.episode_plot_path(scene.chapter, scene.episode),
            Some(scene),
            digits,
        ),
        (Kind::Idea, _) => crate::core::features::ideas::PATH.to_owned(),
        (Kind::Material, _) => crate::core::features::materials_ops::INDEX_PATH.to_owned(),
        _ => task.scope.clone(),
    };
    let before = match (kind, scene) {
        (Kind::SectionText | Kind::PlotField, Some(scene)) => {
            apply::current_text(project, kind, scene)
        }
        _ => String::new(),
    };
    let mut proposal = Proposal {
        id: proposal_id.clone(),
        kind: kind.as_str().to_owned(),
        target,
        origin: task.id.clone(),
        status: ProposalStatus::Open.as_str().to_owned(),
        created_at: now.to_owned(),
        before,
        after: parsed.main.clone(),
        ..Proposal::default()
    };
    task.status = "review".to_owned();
    // 自動適用（§15.1.1）: 実行の時点のレベルで決める
    if task.delegation.can_write() && kind.applies() {
        match apply::changes(
            project,
            &proposal,
            &parsed.main,
            now,
            &task.intent,
            &task.role,
        ) {
            Ok(changes) => {
                let refused = apply::gate(project, &changes);
                if refused.is_empty() {
                    for change in changes {
                        plan.write(change.path, change.text);
                    }
                    proposal.status = ProposalStatus::Applied.as_str().to_owned();
                    proposal.updated_at = now.to_owned();
                    task.status = "applied".to_owned();
                    let shown =
                        scene.map_or_else(|| proposal.target.clone(), |id| id.display(digits));
                    let mut event = LogEvent::new(now, "auto_applied");
                    event.task = Some(task.id.clone());
                    event.role = Some(task.role.clone());
                    event.proposal = Some(proposal_id.clone());
                    plan.log.push(event);
                    if apply::is_fixed(project, &proposal) {
                        let mut event = LogEvent::new(now, "fixed_overwrite");
                        event.task = Some(task.id.clone());
                        event.role = Some(task.role.clone());
                        event.proposal = Some(proposal_id.clone());
                        plan.log.push(event);
                        plan.notices.push(Notice::FixedOverwrite {
                            task: task.id.clone(),
                            target: shown.clone(),
                        });
                    }
                    plan.notices.push(Notice::AutoApplied {
                        task: task.id.clone(),
                        target: shown,
                    });
                } else {
                    plan.notices.push(Notice::GateRefused {
                        task: task.id.clone(),
                        codes: apply::codes(&refused),
                    });
                }
            }
            Err(_) => plan.notices.push(Notice::Missing {
                task: task.id.clone(),
            }),
        }
    }
    plan.write(
        files::proposal_path(&proposal_id),
        files::write_proposal(&proposal),
    );
    // 結果
    let result = RunResult {
        task: task.id.clone(),
        finished_at: now.to_owned(),
        model: reply.model.clone(),
        input_chars: reply.input_chars,
        output_chars: crate::core::agents::chars(&reply.text),
        duration_ms: reply.duration_ms,
        truncated: reply.truncated,
        output: reply.text.clone(),
        proposal: files::proposal_path(&proposal_id),
    };
    let result_path = files::result_path(&task.id);
    plan.write(result_path.clone(), files::write_result(&result));
    task.result = result_path.clone();
    task.updated_at = now.to_owned();
    plan.write(files::task_path(&task.id), files::write_task(&task));
    if reply.truncated {
        plan.notices.push(Notice::Truncated {
            task: task.id.clone(),
        });
    }
    if !parsed.questions.is_empty() {
        plan.notices.push(Notice::Questions {
            task: task.id.clone(),
            count: parsed.questions.len(),
        });
    }
    // 伝言（§15.6）
    deliver(project, &board, &task, &parsed, now, &mut paths, &mut plan);
    // 監査ログ
    let mut event = LogEvent::new(now, "run");
    event.task = Some(task.id.clone());
    event.role = Some(task.role.clone());
    event.server = Some(task.server.clone());
    event.input_chars = Some(reply.input_chars);
    event.output_chars = Some(result.output_chars);
    event.duration_ms = Some(reply.duration_ms);
    event.result_path = Some(result_path);
    plan.log.push(event);
    session.calls += 1;
    session.output_chars += result.output_chars as u64;
    // 連鎖（§15.1.1・§15.3: 起こせるのは integrator が autonomous のときだけ）
    chain(
        project, &board, &task, &parsed, now, session, &mut paths, &mut plan,
    );
    plan
}

/// 伝言をスレッドに積む（§15.6: `autonomous` の役割どうしだけ承認なしに渡る。それ以外は未承認）。
fn deliver(
    project: &Project,
    board: &Board,
    task: &Task,
    parsed: &Parsed,
    now: &str,
    paths: &mut Vec<String>,
    plan: &mut Plan,
) {
    if parsed.messages.is_empty() {
        return;
    }
    let settings = &project.settings().agents;
    let mut thread = board
        .threads
        .iter()
        .rev()
        .find(|thread| thread.scope == task.scope && thread.status != "closed")
        .cloned()
        .unwrap_or_else(|| {
            let id = files::next_thread_id(paths.iter().map(String::as_str));
            paths.push(files::thread_path(&id));
            files::Thread {
                id,
                title: task.intent.clone(),
                scope: task.scope.clone(),
                status: "open".to_owned(),
                messages: Vec::new(),
            }
        });
    for message in &parsed.messages {
        let number = thread
            .messages
            .iter()
            .map(|found| found.number)
            .max()
            .unwrap_or(0)
            + 1;
        let approved = if task.delegation == Delegation::Autonomous
            && level_of(settings, message.to.trim()) == Delegation::Autonomous
        {
            "auto"
        } else {
            files::PENDING
        };
        thread.messages.push(files::Message {
            number,
            from: task.role.clone(),
            to: message.to.clone(),
            kind: message.kind.clone(),
            created_at: now.to_owned(),
            approved_by: approved.to_owned(),
            refs: message.refs.clone(),
            body: message.body.clone(),
        });
    }
    plan.write(files::thread_path(&thread.id), files::write_thread(&thread));
}

#[allow(clippy::too_many_arguments)]
fn chain(
    project: &Project,
    board: &Board,
    task: &Task,
    parsed: &Parsed,
    now: &str,
    session: &mut Session,
    paths: &mut Vec<String>,
    plan: &mut Plan,
) {
    let settings = &project.settings().agents;
    if level_of(settings, "integrator") != Delegation::Autonomous {
        return;
    }
    let hop = session.hops.get(&task.id).copied().unwrap_or(0) + 1;
    let digits = project.digits();
    let mut wanted: Vec<(String, SceneId, String)> = Vec::new();
    if task.role == "integrator" {
        for found in &parsed.tasks {
            let role = found.role.trim();
            if !super::roles(project).iter().any(|r| r == role)
                || role == "integrator"
                || level_of(settings, role) == Delegation::Off
            {
                continue;
            }
            let Ok(partial) = PartialSceneId::parse_with_sigil(found.scope.trim()) else {
                continue;
            };
            if partial.depth() != 4 {
                continue;
            }
            wanted.push((role.to_owned(), partial.expand_low(), found.intent.clone()));
        }
    } else if let Some(scene) = files::task_scene(task) {
        wanted.push(("integrator".to_owned(), scene, CHAIN_INTENT.to_owned()));
    }
    for (role, scene, intent) in wanted {
        if duplicate(board, &role, scene).is_some()
            || plan.writes.iter().any(|change| {
                change.path.starts_with(files::TASKS_DIR)
                    && files::parse_task(&change.path, &change.text).is_some_and(|other| {
                        other.role == role
                            && files::task_scene(&other) == Some(scene)
                            && is_active(&other)
                    })
            })
        {
            continue;
        }
        if let Some(found) = limit(&settings.limits, session, hop) {
            let mut event = LogEvent::new(now, "limit_stop");
            event.task = Some(task.id.clone());
            event.role = Some(task.role.clone());
            event.detail = Some(found.key().to_owned());
            plan.log.push(event);
            plan.notices.push(Notice::Limit {
                limit: found.key().to_owned(),
            });
            return;
        }
        let id = files::next_task_id(paths.iter().map(String::as_str));
        let path = files::task_path(&id);
        paths.push(path.clone());
        let created = Task {
            id: id.clone(),
            role: role.clone(),
            scope: format!("@{}", scene.display(digits)),
            intent: crate::core::agents::one_line(&intent),
            status: "ready".to_owned(),
            initiator: "agent".to_owned(),
            delegation: level_of(settings, &role),
            prompt: files::prompt_path(&role),
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
            ..Task::default()
        };
        plan.write(path, files::write_task(&created));
        let mut event = LogEvent::new(now, "chain_hop");
        event.task = Some(id.clone());
        event.role = Some(role);
        event.detail = Some(format!("{} -> {id}", task.id));
        plan.log.push(event);
        session.auto_started += 1;
        session.hops.insert(id.clone(), hop);
        plan.follow.push(id);
    }
}

/// 呼び出しが失敗したとき（E063・タイムアウト・応答が読めない）: タスクは `ready` に戻す
/// （部分出力は捨てる。§15.9）。`error` はログに書く短い理由。
pub fn fail(project: &Project, id: &str, error: &str, duration_ms: u64, now: &str) -> Plan {
    let mut plan = Plan::default();
    let board = Board::of(project);
    let Some(task) = board.task(id) else {
        return plan;
    };
    let mut task = task.clone();
    task.status = "ready".to_owned();
    task.updated_at = now.to_owned();
    plan.write(files::task_path(&task.id), files::write_task(&task));
    let mut event = LogEvent::new(now, "run");
    event.task = Some(task.id.clone());
    event.role = Some(task.role.clone());
    event.server = Some(task.server.clone());
    event.duration_ms = Some(duration_ms);
    event.error = Some(error.to_owned());
    plan.log.push(event);
    plan
}

/// 中止（`X`・総停止）: `canceled` にする。走っていたなら `run` に `canceled` を書く。
pub fn cancel(project: &Project, id: &str, was_running: bool, now: &str) -> Plan {
    let mut plan = Plan::default();
    let board = Board::of(project);
    let Some(task) = board.task(id) else {
        return plan;
    };
    if task.status_kind().is_some_and(|status| status.is_final()) {
        return plan;
    }
    let mut task = task.clone();
    task.status = "canceled".to_owned();
    task.updated_at = now.to_owned();
    plan.write(files::task_path(&task.id), files::write_task(&task));
    if was_running {
        let mut event = LogEvent::new(now, "run");
        event.task = Some(task.id.clone());
        event.role = Some(task.role.clone());
        event.server = Some(task.server.clone());
        event.error = Some("canceled".to_owned());
        plan.log.push(event);
    }
    plan
}

/// `ready` にする（`R` で実行待ちに入れる・失敗の後）。
pub fn ready(project: &Project, id: &str, now: &str) -> Plan {
    let mut plan = Plan::default();
    let board = Board::of(project);
    if let Some(task) = board.task(id) {
        if task.status == "draft" {
            let mut task = task.clone();
            task.status = "ready".to_owned();
            task.updated_at = now.to_owned();
            plan.write(files::task_path(&task.id), files::write_task(&task));
        }
    }
    plan
}

/// `running` にする（[`Start::task`] を書く）。
pub fn started(start: &Start) -> Plan {
    let mut plan = Plan::default();
    plan.write(
        files::task_path(&start.task.id),
        files::write_task(&start.task),
    );
    plan
}

// ============================================================ 委譲レベル（§15.7.6）

/// 役割のレベルを変えた `project.json`（`serde_json::Value` で書き換える）と、`delegation_changed`。
/// integrator の `off` は `agents.integrator = "human"`、それ以外のレベルは `"agent"` + `roles.integrator`。
pub fn set_level(
    project_json: &str,
    settings: &Agents,
    role: &str,
    level: Delegation,
    now: &str,
) -> Option<(String, LogEvent)> {
    let mut root: Value = serde_json::from_str(project_json).ok()?;
    let object = root.as_object_mut()?;
    let agents = object
        .entry("agents")
        .or_insert_with(|| json!({}))
        .as_object_mut()?;
    let from = level_of(settings, role);
    if role == "integrator" {
        if level == Delegation::Off {
            agents.insert("integrator".to_owned(), json!("human"));
        } else {
            agents.insert("integrator".to_owned(), json!("agent"));
            let roles = agents
                .entry("roles")
                .or_insert_with(|| json!({}))
                .as_object_mut()?;
            roles.insert(role.to_owned(), json!(level.as_str()));
        }
    } else {
        let roles = agents
            .entry("roles")
            .or_insert_with(|| json!({}))
            .as_object_mut()?;
        roles.insert(role.to_owned(), json!(level.as_str()));
    }
    let mut text = serde_json::to_string_pretty(&root).ok()?;
    text.push('\n');
    let mut event = LogEvent::new(now, "delegation_changed");
    event.role = Some(role.to_owned());
    event.detail = Some(format!("{role}: {} -> {}", from.as_str(), level.as_str()));
    Some((text, event))
}

/// 総停止（§15.7.4 `Ctrl+Shift+X`）: `apply` / `autonomous` の役割（既定のレベルを含む）を `propose`
/// に下げた `project.json` と、変えた役割ごとの `delegation_changed`。
pub fn stop_all(
    project_json: &str,
    settings: &Agents,
    now: &str,
) -> Option<(String, Vec<LogEvent>)> {
    let mut root: Value = serde_json::from_str(project_json).ok()?;
    let object = root.as_object_mut()?;
    let agents = object
        .entry("agents")
        .or_insert_with(|| json!({}))
        .as_object_mut()?;
    let mut events = Vec::new();
    let before: Vec<(&str, Delegation)> = crate::core::agents::BUILTIN_ROLES
        .iter()
        .map(|role| (*role, level_of(settings, role)))
        .collect();
    if settings.delegation_level().can_write() {
        agents.insert("delegation".to_owned(), json!(Delegation::Propose.as_str()));
    }
    let roles = agents
        .entry("roles")
        .or_insert_with(|| json!({}))
        .as_object_mut()?;
    for (role, level) in &before {
        if !level.can_write() {
            continue;
        }
        roles.insert((*role).to_owned(), json!(Delegation::Propose.as_str()));
        let mut event = LogEvent::new(now, "delegation_changed");
        event.role = Some((*role).to_owned());
        event.detail = Some(format!("{role}: {} -> propose", level.as_str()));
        events.push(event);
    }
    // 明示の値が無く既定に従っていた役割で、既定を下げたことで変わるものはもう events に入っている
    let mut text = serde_json::to_string_pretty(&root).ok()?;
    text.push('\n');
    Some((text, events))
}

/// 次に走らせるタスク（U-8）。`queue` と `running` は `(タスク, サーバ, 節)`。
pub fn pick(
    queue: &[(String, String, Option<SceneId>)],
    running: &[(String, String, Option<SceneId>)],
    max_parallel: usize,
) -> Option<usize> {
    if running.len() >= max_parallel {
        return None;
    }
    queue.iter().position(|(_, server, scene)| {
        let busy = running
            .iter()
            .filter(|(_, other, _)| other == server)
            .count();
        busy < crate::core::agents::PER_SERVER_LIMIT
            && !(scene.is_some() && running.iter().any(|(_, _, other)| other == scene))
    })
}
