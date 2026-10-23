//! `agent.*`・`context.build`・`prompt.*`・`mail.send` の実装（§19.7）。
//!
//! 実行（MCP への接続）は [`crate::app::agents`] の状態機に任せ、ここは引数の受け渡しと
//! ファイル（`agents/`）の読み書きだけをする（§19.1.1「5 つのインターフェースは 1 つの実装を共有」）。

use serde_json::{json, Value};

use crate::app::error::{Class, Error, Result};
use crate::app::ops::{apply_plan, audit, commit, gate, via_mcp, write_document, Args};
use crate::app::session::{scene_arg, str_arg, str_or, usize_opt, Session};
use crate::core::agents::{
    apply,
    files::{self, Board},
    mcp_config, prompts, runner,
};
use crate::core::model::delegation::Delegation;
use crate::core::model::task::Task;
use crate::ui::i18n::{self, Locale};

/// 監査ログのディレクトリ（§5.10.5）。
const LOG_DIR: &str = files::LOG_DIR;

/// 今（RFC 3339）。
fn stamp(session: &Session) -> String {
    session.host.now()
}

fn task_json(task: &Task) -> Value {
    json!({
        "created_at": task.created_at,
        "delegation": task.delegation.as_str(),
        "examples": task.examples,
        "id": task.id,
        "intent": task.intent,
        "prompt": task.prompt,
        "result": task.result,
        "role": task.role,
        "scope": task.scope,
        "server": task.server,
        "status": task.status,
        "updated_at": task.updated_at,
    })
}

fn proposal_json(proposal: &files::Proposal) -> Value {
    json!({
        "after_chars": proposal.after.chars().count(),
        "before_chars": proposal.before.chars().count(),
        "comment": proposal.comment,
        "created_at": proposal.created_at,
        "id": proposal.id,
        "kind": proposal.kind,
        "note": proposal.note,
        "origin": proposal.origin,
        "partial": proposal.partial,
        "status": proposal.status,
        "target": proposal.target,
        "updated_at": proposal.updated_at,
    })
}

fn board(session: &mut Session) -> Board {
    session.bench.commit();
    Board::of(session.aggregate())
}

// =========================================================================== agent.task_*

/// `agent.task_list`（§15.4.1 の一覧）。
pub fn task_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let board = board(session);
    let items: Vec<Value> = board.tasks.iter().map(task_json).collect();
    Ok(json!({ "count": items.len(), "tasks": items }))
}

/// `agent.task_create`（§15.3 の起票。`N`。E035 の重複は作らずに知らせる）。
pub fn task_create(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = str_arg(locale, args, "role")?;
    let scope = str_or(args, "scope", "");
    let intent = str_or(args, "intent", "");
    let comment = str_or(args, "comment", "");
    let examples = str_or(args, "examples", "");
    if !crate::core::agents::is_role(&role) {
        return Err(Error::usage(locale, "role"));
    }
    if scope.is_empty() {
        let Some(scene) = session.bench.current_id() else {
            return Err(Error::missing(locale, "scope"));
        };
        let digits = session.aggregate().digits();
        let scope = format!("@{}", scene.display(digits));
        return create_with(session, &role, &scope, intent, comment, examples);
    }
    create_with(session, &role, scope, intent, comment, examples)
}

fn create_with(
    session: &mut Session,
    role: &str,
    scope: &str,
    intent: &str,
    comment: &str,
    examples: &str,
) -> Result<Value> {
    let locale = session.locale;
    let now = stamp(session);
    let plan = runner::new_tasks(
        session.aggregate(),
        &runner::Draft {
            role,
            scope,
            intent,
            comment,
            examples,
            initiator: "human",
            status: "draft",
        },
        &now,
    );
    let created: Vec<String> = plan.writes.iter().map(|write| write.path.clone()).collect();
    if created.is_empty() {
        return Err(Error::new(
            Class::Validation,
            i18n::format(locale, "api.error.duplicate_task", &[role, scope]),
        )
        .with_kind("E035"));
    }
    apply_plan(session, &plan)?;
    audit(
        session,
        "run",
        &format!("op:agent.task_create {role} {scope}"),
    );
    let saved = commit(session)?;
    Ok(json!({
        "created": created,
        "role": role,
        "save": saved,
        "scope": scope,
    }))
}

/// `agent.task_get`（タスク 1 件と結果）。
pub fn task_get(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let id = str_arg(locale, args, "id")?;
    let board = board(session);
    let Some(task) = board.task(&id) else {
        return Err(Error::not_found(locale, &id, None));
    };
    let result = session
        .aggregate()
        .text(&files::result_path(&id))
        .to_owned();
    let proposal = board.proposal_of(&id).map(proposal_json);
    Ok(json!({
        "proposal": proposal.unwrap_or(Value::Null),
        "result": result,
        "task": task_json(task),
    }))
}

/// `agent.task_run`（§15.7 の実行。MCP サーバが要る）。
pub fn task_run(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let board = board(session);
    if board.task(&id).is_none() {
        return Err(Error::not_found(locale, &id, None));
    }
    let available = session.host.mcp_available();
    if !available {
        return Err(Error::new(
            Class::Policy,
            i18n::text(locale, "api.error.mcp_unavailable").to_owned(),
        ));
    }
    let notices = {
        let Session {
            bench,
            host,
            agents,
            clock,
            ..
        } = session;
        crate::app::agents::enqueue(
            std::slice::from_ref(&id),
            bench,
            host.as_mut(),
            agents,
            *clock,
            locale,
        )
    };
    if role.is_some() {
        audit(session, "run", &format!("op:agent.task_run {id}"));
    }
    Ok(json!({ "notices": notices, "queued": id }))
}

/// `agent.task_dry_run`（§15.7.2 の試し実行: ContextPacket だけ作って書く）。
pub fn task_dry_run(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let id = str_arg(locale, args, "id")?;
    let board = board(session);
    if board.task(&id).is_none() {
        return Err(Error::not_found(locale, &id, None));
    }
    let title = session.aggregate().settings().title.clone();
    let now = stamp(session);
    let Some((path, text)) = runner::dry_run(session.aggregate(), &id, &now, &title) else {
        return Err(Error::not_found(locale, &id, None));
    };
    write_document(session, &path, text.clone());
    let saved = commit(session)?;
    Ok(json!({ "chars": text.chars().count(), "path": path, "save": saved, "task": id }))
}

/// `agent.task_stop`（§15.7.4 の停止）。
pub fn task_stop(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    {
        let Session {
            bench,
            host,
            agents,
            clock,
            ..
        } = session;
        crate::app::agents::cancel(&id, bench, host.as_mut(), agents, *clock);
    }
    let saved = commit(session)?;
    Ok(json!({ "save": saved, "stopped": id }))
}

/// `agent.stop_all`（§15.7.4 `Ctrl+Shift+X`）。
pub fn stop_all(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    gate(session, args)?;
    let message = {
        let Session {
            bench,
            host,
            agents,
            clock,
            ..
        } = session;
        crate::app::agents::stop_all(bench, host.as_mut(), agents, *clock, locale)
    };
    let saved = commit(session)?;
    Ok(json!({ "message": message, "save": saved }))
}

// =========================================================================== agent.proposal_*

/// `agent.proposal_list`（§15.7.3 の一覧）。
pub fn proposal_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let board = board(session);
    let items: Vec<Value> = board.proposals.iter().map(proposal_json).collect();
    Ok(json!({ "count": items.len(), "proposals": items }))
}

/// `agent.proposal_get`（提案の全文: `before` / `after`）。
pub fn proposal_get(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let id = str_arg(locale, args, "id")?;
    let board = board(session);
    let Some(proposal) = board.proposal(&id) else {
        return Err(Error::not_found(locale, &id, None));
    };
    let drifted = apply::drifted(session.aggregate(), proposal);
    let mut out = proposal_json(proposal);
    if let Some(map) = out.as_object_mut() {
        map.insert("after".to_owned(), json!(proposal.after));
        map.insert("before".to_owned(), json!(proposal.before));
        map.insert("drifted".to_owned(), json!(drifted));
    }
    Ok(out)
}

/// `agent.proposal_apply`（§15.7.3 の採用。検証ゲート B-11 を通す）。
pub fn proposal_apply(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let text = args.get("text").and_then(Value::as_str).map(str::to_owned);
    let now = stamp(session);
    let board = board(session);
    if board.proposal(&id).is_none() {
        return Err(Error::not_found(locale, &id, None));
    }
    let plan = apply::accept(session.aggregate(), &id, text.as_deref(), &now)
        .map_err(|refusal| refusal_error(locale, refusal))?;
    apply_plan(session, &plan)?;
    audit(session, "accept", &format!("op:agent.proposal_apply {id}"));
    let saved = commit(session)?;
    Ok(json!({ "applied": id, "save": saved }))
}

/// `agent.proposal_reject`（§15.7.3 の却下。コメント必須）。
pub fn proposal_reject(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    gate(session, args)?;
    let id = str_arg(locale, args, "id")?;
    let comment = str_arg(locale, args, "comment")?;
    if comment.trim().is_empty() {
        return Err(Error::missing(locale, "comment"));
    }
    let now = stamp(session);
    let plan = apply::reject(session.aggregate(), &id, &comment, &now)
        .map_err(|refusal| refusal_error(locale, refusal))?;
    apply_plan(session, &plan)?;
    audit(session, "reject", &format!("op:agent.proposal_reject {id}"));
    let saved = commit(session)?;
    Ok(json!({ "comment": comment, "rejected": id, "save": saved }))
}

fn refusal_error(locale: Locale, refusal: apply::Refusal) -> Error {
    Error::new(
        Class::Validation,
        i18n::format(locale, "api.error.refusal", &[&format!("{refusal:?}")]),
    )
}

// =========================================================================== agent.delegation_* / mcp_config_*

/// `agent.delegation_get`（§15.1.1 の委譲レベル）。
pub fn delegation_get(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let settings = session.aggregate().settings().agents.clone();
    let levels: Vec<Value> = crate::core::syntax::prompt_md::BUILTIN_ROLES
        .iter()
        .map(|role| {
            json!({
                "level": crate::core::agents::level_of(&settings, role).as_str(),
                "role": role,
            })
        })
        .collect();
    Ok(json!({
        "enabled": settings.enabled,
        "levels": levels,
        "max_parallel": runner::max_parallel(&settings),
    }))
}

/// `agent.delegation_set`（§15.1.1。MCP には出さない安全上の除外、§19.4.3）。
pub fn delegation_set(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let role = str_or(args, "role", "writer").to_owned();
    let name = str_arg(locale, args, "level")?;
    let Some(level) = Delegation::parse(&name) else {
        return Err(Error::usage(locale, "level"));
    };
    let message = {
        let Session {
            bench, host, clock, ..
        } = session;
        crate::app::agents::set_level(&role, level, bench, host.as_mut(), *clock, locale)
    };
    let saved = commit(session)?;
    Ok(json!({
        "level": name,
        "message": message,
        "role": role,
        "save": saved,
    }))
}

/// `agent.mcp_config_get`（§5.11 の `agents/mcp.json`）。
pub fn mcp_config_get(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let text = session.aggregate().text(MCP_CONFIG).to_owned();
    let servers = {
        let Session { bench, host, .. } = session;
        crate::app::agents::servers(bench, host.as_ref())
    };
    Ok(json!({
        "path": MCP_CONFIG,
        "servers": servers,
        "text": text,
    }))
}

/// `agents/mcp.json` のパス（§5.11）。
const MCP_CONFIG: &str = "agents/mcp.json";

/// `agent.mcp_config_validate`（E030 / W033 / W034）。
pub fn mcp_config_validate(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let text = match args.get("text").and_then(Value::as_str) {
        Some(text) => text.to_owned(),
        None => session.aggregate().text(MCP_CONFIG).to_owned(),
    };
    let found = mcp_config::diagnostics(&text);
    let items: Vec<Value> = found
        .iter()
        .map(|diagnostic| {
            json!({
                "code": diagnostic.code.as_str(),
                "message": i18n::diagnostic(locale, diagnostic),
                "severity": crate::app::ops::severity_name(diagnostic.severity()),
            })
        })
        .collect();
    let valid = mcp_config::parse(&text).is_ok()
        && !found
            .iter()
            .any(|diagnostic| diagnostic.severity() == crate::core::diagnostics::Severity::Error);
    Ok(json!({ "count": items.len(), "diagnostics": items, "valid": valid }))
}

/// `agent.mcp_config_set`（§5.11 の書き換え。MCP には出さない、§19.4.3）。
pub fn mcp_config_set(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let text = str_arg(locale, args, "text")?;
    if mcp_config::parse(&text).is_err() {
        return Err(Error::validation(
            "E030",
            i18n::text(locale, "api.error.mcp_invalid").to_owned(),
        ));
    }
    write_document(session, MCP_CONFIG, text.clone());
    audit(session, "direct_write", "op:agent.mcp_config_set");
    let saved = commit(session)?;
    Ok(json!({ "chars": text.chars().count(), "path": MCP_CONFIG, "save": saved }))
}

// =========================================================================== agent.thread_* / audit_list

/// `agent.thread_list`（§15.6 のスレッド一覧）。
pub fn thread_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let board = board(session);
    let items: Vec<Value> = board
        .threads
        .iter()
        .map(|thread| {
            let messages: Vec<Value> = thread
                .messages
                .iter()
                .map(|message| {
                    json!({
                        "body": message.body,
                        "from": message.from,
                        "kind": message.kind,
                        "created_at": message.created_at,
                        "to": message.to,
                    })
                })
                .collect();
            json!({
                "id": thread.id,
                "messages": messages,
                "scope": thread.scope,
                "status": thread.status,
                "title": thread.title,
            })
        })
        .collect();
    Ok(json!({ "count": items.len(), "threads": items }))
}

/// `agent.thread_get`（スレッド 1 件）。
pub fn thread_get(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let id = str_arg(locale, args, "id")?;
    let board = board(session);
    let Some(thread) = board.threads.iter().find(|thread| thread.id == id) else {
        return Err(Error::not_found(locale, &id, None));
    };
    let items: Vec<Value> = thread
        .messages
        .iter()
        .map(|message| {
            json!({
                "body": message.body,
                "from": message.from,
                "kind": message.kind,
                "created_at": message.created_at,
                        "to": message.to,
            })
        })
        .collect();
    Ok(
        json!({ "id": id, "messages": items, "scope": thread.scope, "status": thread.status, "title": thread.title }),
    )
}

/// `agent.audit_list`（§5.10.5 の監査ログ）。
pub fn audit_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    let limit = usize_opt(args, "limit").unwrap_or(200);
    session.bench.commit();
    let mut paths: Vec<String> = session
        .aggregate()
        .documents()
        .map(|document| document.path.clone())
        .filter(|path| path.starts_with(LOG_DIR))
        .collect();
    paths.sort();
    let mut items = Vec::new();
    for path in paths.iter().rev() {
        for line in session.aggregate().text(path).lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                items.push(value);
            }
            if items.len() >= limit {
                break;
            }
        }
        if items.len() >= limit {
            break;
        }
    }
    Ok(json!({ "count": items.len(), "events": items }))
}

// =========================================================================== context / prompt / mail

/// `context.build`（既存の MCP ツールと同じ。§15.4）。
pub fn context_build(session: &mut Session, args: &Args) -> Result<Value> {
    via_mcp(session, "novnow.context.build", args)
}

/// `prompt.list`（§15.5 のプロンプト一覧）。
pub fn prompt_list(session: &mut Session, args: &Args) -> Result<Value> {
    let _ = args;
    session.bench.commit();
    let items: Vec<Value> = crate::core::syntax::prompt_md::BUILTIN_ROLES
        .iter()
        .map(|role| {
            let path = files::prompt_path(role);
            let text = session.aggregate().text(&path);
            json!({
                "blocks": prompts::missing_sections(text),
                "edited": prompts::edited(session.aggregate(), &path),
                "name": role,
                "path": path,
                "present": session.aggregate().document(&path).is_some(),
            })
        })
        .collect();
    Ok(json!({ "count": items.len(), "prompts": items }))
}

/// `prompt.get`（プロンプトの全文と既定からの差分）。
pub fn prompt_get(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let name = str_arg(locale, args, "name")?;
    let path = files::prompt_path(&name);
    let text = session.aggregate().text(&path).to_owned();
    let blockers: Vec<String> = prompts::blockers(session.aggregate(), &name)
        .iter()
        .map(|diagnostic| i18n::diagnostic(locale, diagnostic))
        .collect();
    Ok(json!({
        "blockers": blockers,
        "edited": prompts::edited(session.aggregate(), &path),
        "name": name,
        "path": path,
        "shared": prompts::shared_body(&text),
        "text": text,
    }))
}

/// `prompt.diff`（既定との差分。§15.5.4）。
pub fn prompt_diff(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let name = str_arg(locale, args, "name")?;
    let path = files::prompt_path(&name);
    let current = session.aggregate().text(&path).to_owned();
    let default = prompts::reset_targets(&name)
        .into_iter()
        .find(|(target, _)| *target == path)
        .map(|(_, text)| text.to_owned())
        .unwrap_or_default();
    let mut lines = Vec::new();
    for (index, line) in current.split('\n').enumerate() {
        if default.split('\n').nth(index) != Some(line) {
            lines.push(json!({ "line": index + 1, "side": "current", "text": line }));
        }
    }
    for (index, line) in default.split('\n').enumerate() {
        if current.split('\n').nth(index) != Some(line) {
            lines.push(json!({ "line": index + 1, "side": "default", "text": line }));
        }
    }
    Ok(json!({
        "count": lines.len(),
        "edited": prompts::edited(session.aggregate(), &path),
        "lines": lines,
        "name": name,
        "path": path,
    }))
}

/// `prompt.reset`（§15.5.4 (3): 退避しない・undo できない。MCP には出さない）。
pub fn prompt_reset(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let name = str_arg(locale, args, "name")?;
    let count = {
        let Session {
            bench, host, clock, ..
        } = session;
        crate::app::agents::reset_prompts(&name, bench, host.as_ref(), *clock)
    };
    audit(session, "direct_write", &format!("op:prompt.reset {name}"));
    let saved = commit(session)?;
    Ok(json!({ "name": name, "reset": count, "save": saved }))
}

/// `mail.send`（既存の MCP ツールと同じ。§15.6）。
pub fn mail_send(session: &mut Session, args: &Args) -> Result<Value> {
    let locale = session.locale;
    let _ = scene_arg(locale, args, "scene").ok();
    via_mcp(session, "novnow.mail.send", args)
}
