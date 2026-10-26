//! `novnow mcp <dir>` のツール（§15.8.3）。**core の純粋関数だけを公開する**（UI を経由しない）。
//!
//! - 読むツールはプロジェクトの文書から JSON を作るだけ。
//! - 書くツールは [`Plan`] を返し、`cli::mcp` がプロジェクトの中にだけ書く（規則 10・FsGate）。
//! - `novnow.section.write` / `novnow.plot.write` は `role` 引数が必須で、その役割の委譲レベルが
//!   `apply` / `autonomous` のときだけ書ける（無い・それ未満は **E036**）。書いた前の状態は
//!   `agents/proposals/` に `applied` の提案として残し、監査ログに `direct_write` を書く（A2）。
//!   自動適用と同じ検証を通し、E 系の診断が増えるなら書かずに `open` の提案として残す（B-11）。
//! - `novnow.mail.send` は未承認（`approved_by: pending`）で積む。送り手は `mcp`。

use serde_json::{json, Map, Value};

use crate::core::agents::files::{
    self, Board, LogEvent, Message, Proposal, ProposalStatus, Thread,
};
use crate::core::agents::{apply, level_of, Kind, Plan};
use crate::core::aicontext::packet::{self, Inputs};
use crate::core::diagnostics::{self, Arg, Diagnostic, DiagnosticCode};
use crate::core::features::{materials_ops, sections, Project};
use crate::core::id::{PartialSceneId, SceneId};
use crate::core::model::delegation::Delegation;
use crate::core::resolve::context_rules::PacketBlock;
use crate::core::resolve::dictionary;
use crate::core::text::lexer::FieldKey;

/// 公開するツール（§15.8.3 の表の順）: `(名前, 説明, 引数)`。
pub const TOOLS: &[(&str, &str, &[&str])] = &[
    (
        "novnow.project.info",
        "Title, structure and statistics of the project",
        &[],
    ),
    (
        "novnow.structure.tree",
        "Chapter / episode / scene / section tree with summaries",
        &["level"],
    ),
    (
        "novnow.plot.get",
        "Plot at a level (all | chapter | episode) for a scene",
        &["scene", "level"],
    ),
    (
        "novnow.section.get",
        "Text of a section and its neighbors",
        &["scene", "neighbors"],
    ),
    (
        "novnow.cast.get",
        "Cast of a section (active modes only)",
        &["scene"],
    ),
    (
        "novnow.lexicon.search",
        "Search the lexicon (words.md)",
        &["query"],
    ),
    (
        "novnow.materials.search",
        "Search materials",
        &["query", "category", "scope"],
    ),
    (
        "novnow.context.build",
        "The ContextPacket for a role and a section",
        &["role", "scene"],
    ),
    (
        "novnow.diagnostics.run",
        "Diagnostics (all files, a path prefix or a scene)",
        &["scope"],
    ),
    (
        "novnow.section.propose",
        "Create a proposal for a section (does not change the text)",
        &["scene", "text", "note"],
    ),
    (
        "novnow.section.write",
        "Write a section directly (role with apply / autonomous only)",
        &["role", "scene", "text", "note"],
    ),
    (
        "novnow.plot.write",
        "Write a plot section directly (role with apply / autonomous only)",
        &["role", "scene", "level", "text"],
    ),
    (
        "novnow.materials.propose",
        "Add a material with status proposed",
        &["title", "source", "markdown", "category"],
    ),
    (
        "novnow.mail.send",
        "Queue an unapproved message to a role",
        &["to", "kind", "scope", "body"],
    ),
];

/// 凍結された 14 ツールの 1 つか（§19.8.3）。呼び出し側が既存分と新規分（§19.5.1 の
/// `novnow.<正規操作 ID>`）を振り分けるときに使う。
pub fn is_tool(name: &str) -> bool {
    TOOLS.iter().any(|(tool, _, _)| *tool == name)
}

/// `tools/list` の `tools`。
pub fn tools() -> Value {
    Value::Array(
        TOOLS
            .iter()
            .map(|(name, description, args)| {
                let mut properties = Map::new();
                for arg in *args {
                    let kind = if *arg == "neighbors" {
                        "integer"
                    } else {
                        "string"
                    };
                    properties.insert((*arg).to_owned(), json!({ "type": kind }));
                }
                json!({
                    "name": name,
                    "description": description,
                    "inputSchema": { "type": "object", "properties": properties }
                })
            })
            .collect(),
    )
}

/// ツールの失敗（文言は cli のカタログ）。
#[derive(Debug, Clone, PartialEq)]
pub enum ToolError {
    /// 知らないツール。
    Unknown(String),
    /// 引数が無い・読めない（引数の名前）。
    Argument(&'static str),
    /// 診断として伝えるもの（E036 など）。
    Diagnostic(Diagnostic),
    /// 対象が無い。
    Missing(String),
    /// 書くと E 系の診断が増えるので提案として残した（提案の ID と診断のコード）。
    Gate {
        /// 残した提案。
        proposal: String,
        /// 診断のコード。
        codes: String,
    },
}

/// 呼び出しの結果。書き込みは `plan`（失敗でも提案を残すことがある）。
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// 返す JSON か失敗。
    pub result: Result<Value, ToolError>,
    /// 書くもの。
    pub plan: Plan,
}

fn ok(value: Value) -> Outcome {
    Outcome {
        result: Ok(value),
        plan: Plan::default(),
    }
}

fn err(error: ToolError) -> Outcome {
    Outcome {
        result: Err(error),
        plan: Plan::default(),
    }
}

fn text_arg<'a>(args: &'a Value, name: &str) -> Option<&'a str> {
    args.get(name).and_then(Value::as_str)
}

fn scene_arg(args: &Value) -> Result<SceneId, ToolError> {
    let text = text_arg(args, "scene").ok_or(ToolError::Argument("scene"))?;
    let text = text.trim();
    let parsed = PartialSceneId::parse_with_sigil(text)
        .or_else(|_| PartialSceneId::parse(text))
        .map_err(|_| ToolError::Argument("scene"))?;
    if parsed.depth() != 4 {
        return Err(ToolError::Argument("scene"));
    }
    Ok(parsed.expand_low())
}

/// ツールを呼ぶ。`title` は `project.toml` の `title` が空のときの題、`now` は RFC 3339 の時刻。
pub fn call(project: &Project, name: &str, args: &Value, now: &str, title: &str) -> Outcome {
    match name {
        "novnow.project.info" => ok(info(project, title)),
        "novnow.structure.tree" => ok(tree(project, text_arg(args, "level").unwrap_or("section"))),
        "novnow.plot.get" => match scene_arg(args) {
            Ok(scene) => plot_get(project, scene, text_arg(args, "level").unwrap_or("episode")),
            Err(error) => err(error),
        },
        "novnow.section.get" => match scene_arg(args) {
            Ok(scene) => {
                let neighbors = args.get("neighbors").and_then(Value::as_u64).unwrap_or(1) as usize;
                ok(section_get(project, scene, neighbors.min(5)))
            }
            Err(error) => err(error),
        },
        "novnow.cast.get" => match scene_arg(args) {
            Ok(scene) => ok(
                json!({ "cast": block_of(project, "writer", scene, now, title, PacketBlock::Cast) }),
            ),
            Err(error) => err(error),
        },
        "novnow.lexicon.search" => ok(lexicon_search(
            project,
            text_arg(args, "query").unwrap_or(""),
        )),
        "novnow.materials.search" => ok(materials_search(project, args)),
        "novnow.context.build" => {
            let role = text_arg(args, "role").unwrap_or("writer");
            if !crate::core::agents::roles(project)
                .iter()
                .any(|r| r == role)
            {
                return err(ToolError::Argument("role"));
            }
            match scene_arg(args) {
                Ok(scene) => {
                    let built = packet::build(&Inputs {
                        project,
                        role,
                        scene,
                        delegation: level_of(&project.settings().agents, role),
                        generated_at: now,
                        title,
                        excluded: &[],
                        task: None,
                        comment: "",
                        examples: &[],
                    });
                    ok(json!({ "packet": built.text(), "chars": built.size(), "cap": built.cap }))
                }
                Err(error) => err(error),
            }
        }
        "novnow.diagnostics.run" => ok(diagnostics_run(
            project,
            text_arg(args, "scope").unwrap_or(""),
        )),
        "novnow.section.propose" => section_propose(project, args, now),
        "novnow.section.write" => direct_write(project, args, now, Kind::SectionText),
        "novnow.plot.write" => {
            let level = text_arg(args, "level").unwrap_or("episode");
            if level != "episode" {
                return err(ToolError::Argument("level"));
            }
            direct_write(project, args, now, Kind::PlotField)
        }
        "novnow.materials.propose" => materials_propose(project, args, now),
        "novnow.mail.send" => mail_send(project, args, now),
        other => err(ToolError::Unknown(other.to_owned())),
    }
}

fn info(project: &Project, title: &str) -> Value {
    let list = sections::all(project);
    let mut chapters: Vec<u16> = list.iter().map(|found| found.id.chapter).collect();
    chapters.dedup();
    let mut episodes: Vec<(u16, u16)> = list
        .iter()
        .map(|found| (found.id.chapter, found.id.episode))
        .collect();
    episodes.dedup();
    let mut scenes: Vec<SceneId> = list
        .iter()
        .map(|found| SceneId {
            section: 0,
            ..found.id
        })
        .collect();
    scenes.dedup();
    let chars: usize = list
        .iter()
        .map(|found| sections::count_chars(&sections::text_of(project.text(&found.path), found)))
        .sum();
    let settings = project.settings();
    json!({
        "title": if settings.title.trim().is_empty() { title } else { settings.title.trim() },
        "chapters": chapters.len(),
        "episodes": episodes.len(),
        "scenes": scenes.len(),
        "sections": list.len(),
        "chars": chars,
    })
}

fn tree(project: &Project, level: &str) -> Value {
    let digits = project.digits();
    let depth = match level {
        "chapter" => 1,
        "episode" => 2,
        "scene" => 3,
        _ => 4,
    };
    let mut out: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for found in sections::all(project) {
        let id = match depth {
            1 => SceneId {
                episode: 0,
                scene: 0,
                section: 0,
                ..found.id
            },
            2 => SceneId {
                scene: 0,
                section: 0,
                ..found.id
            },
            3 => SceneId {
                section: 0,
                ..found.id
            },
            _ => found.id,
        };
        let shown = match depth {
            1 => format!("{:0width$}", id.chapter, width = digits.chapter as usize),
            2 => format!(
                "{:0w1$}-{:0w2$}",
                id.chapter,
                id.episode,
                w1 = digits.chapter as usize,
                w2 = digits.episode as usize
            ),
            _ => id.display(digits),
        };
        if seen.contains(&shown) {
            continue;
        }
        seen.push(shown.clone());
        let summary = summary_of(project, id, depth);
        out.push(json!({ "id": shown, "summary": summary, "path": found.path }));
    }
    json!({ "level": level, "nodes": out })
}

fn summary_of(project: &Project, id: SceneId, depth: usize) -> String {
    let first = |outline: Option<crate::core::syntax::plot_outline::Outline<'_>>, want: SceneId| {
        outline.and_then(|outline| {
            let index = outline
                .blocks
                .iter()
                .position(|block| block.id == Some(want))?;
            outline
                .fields_of(index, FieldKey::Summary)
                .first()
                .map(|field| field.value.trim().to_owned())
        })
    };
    match depth {
        1 => first(project.all_plot(), id),
        2 => first(project.chapter_plot(id.chapter), id),
        _ => project
            .episode_plot(id.chapter, id.episode)
            .and_then(|plot| {
                let whole = SceneId { section: 0, ..id };
                let (index, _) = plot.scenes.iter().enumerate().find(|(_, found)| {
                    found.id.map(|found| SceneId {
                        section: 0,
                        ..found
                    }) == Some(whole)
                })?;
                plot.scene_fields(index, FieldKey::Summary)
                    .first()
                    .map(|field| field.value.trim().to_owned())
            }),
    }
    .unwrap_or_default()
}

fn plot_get(project: &Project, scene: SceneId, level: &str) -> Outcome {
    let (path, want) = match level {
        "all" => (
            "plots/all.cnov".to_owned(),
            SceneId {
                episode: 0,
                scene: 0,
                section: 0,
                ..scene
            },
        ),
        "chapter" => (
            project.chapter_plot_path(scene.chapter),
            SceneId {
                scene: 0,
                section: 0,
                ..scene
            },
        ),
        "episode" => (
            project.episode_plot_path(scene.chapter, scene.episode),
            scene,
        ),
        _ => return err(ToolError::Argument("level")),
    };
    let Some(document) = project.document(&path) else {
        return err(ToolError::Missing(path));
    };
    let text = if level == "episode" {
        let plot = crate::core::syntax::plot_episode::parse_with(
            &document.text,
            &project.contents_options(),
        );
        let whole = SceneId { section: 0, ..want };
        plot.scenes
            .iter()
            .find(|found| found.id.map(|id| SceneId { section: 0, ..id }) == Some(whole))
            .map(|found| sections::text_of_lines(&document.text, &found.span))
    } else {
        let options = project.outline_options(&path);
        let outline = crate::core::syntax::plot_outline::parse_with(&document.text, &options);
        outline
            .blocks
            .iter()
            .find(|block| block.id == Some(want))
            .map(|block| sections::text_of_lines(&document.text, &block.span))
    };
    match text {
        Some(text) => ok(json!({ "path": path, "level": level, "text": text.trim_end() })),
        None => err(ToolError::Missing(want.display(project.digits()))),
    }
}

fn section_get(project: &Project, scene: SceneId, neighbors: usize) -> Value {
    let list = sections::all(project);
    let digits = project.digits();
    let path = project.contents_path(scene.chapter, scene.episode);
    let here = list
        .iter()
        .position(|found| found.path == path && found.id == scene);
    let entry = |index: usize| {
        let found = &list[index];
        json!({
            "scene": found.id.display(digits),
            "text": sections::text_of(project.text(&found.path), found),
        })
    };
    match here {
        Some(index) => json!({
            "scene": scene.display(digits),
            "path": path,
            "text": sections::text_of(project.text(&path), &list[index]),
            "before": (index.saturating_sub(neighbors)..index).map(entry).collect::<Vec<_>>(),
            "after": (index + 1..(index + 1 + neighbors).min(list.len())).map(entry).collect::<Vec<_>>(),
        }),
        None => {
            json!({ "scene": scene.display(digits), "path": path, "text": "", "before": [], "after": [] })
        }
    }
}

fn block_of(
    project: &Project,
    role: &str,
    scene: SceneId,
    now: &str,
    title: &str,
    kind: PacketBlock,
) -> String {
    let built = packet::build(&Inputs {
        project,
        role,
        scene,
        delegation: Delegation::Propose,
        generated_at: now,
        title,
        excluded: &[],
        task: None,
        comment: "",
        examples: &[],
    });
    built
        .block(kind)
        .map(|block| {
            let mut block = block.clone();
            for part in &mut block.parts {
                part.keep = part.total();
            }
            block.body()
        })
        .unwrap_or_default()
}

fn lexicon_search(project: &Project, query: &str) -> Value {
    let words = project.words();
    let needle = dictionary::normalize(query.trim());
    let hits: Vec<Value> = words
        .words
        .iter()
        .enumerate()
        .filter(|(_, word)| needle.is_empty() || dictionary::normalize(word.term).contains(&needle))
        .map(|(index, word)| {
            let definition = words
                .word_fields(index, FieldKey::Definition)
                .first()
                .map(|field| field.value.trim().to_owned())
                .unwrap_or_default();
            json!({ "term": word.term, "definition": definition })
        })
        .collect();
    json!({ "words": hits })
}

fn materials_search(project: &Project, args: &Value) -> Value {
    let mut query = text_arg(args, "query").unwrap_or("").to_owned();
    if let Some(category) = text_arg(args, "category").filter(|found| !found.trim().is_empty()) {
        query.push_str(" #");
        query.push_str(category.trim());
    }
    let scope = text_arg(args, "scope").unwrap_or("").trim().to_owned();
    let allowed: Option<Vec<String>> = PartialSceneId::parse_with_sigil(&scope)
        .ok()
        .filter(|found| found.depth() == 4)
        .map(|found| materials_ops::matching(project, found.expand_low(), &[], &[]));
    let hits: Vec<Value> = materials_ops::search(project, &query)
        .into_iter()
        .filter(|hit| allowed.as_ref().is_none_or(|ids| ids.contains(&hit.id)))
        .map(|hit| {
            json!({
                "id": hit.id,
                "title": hit.title,
                "category": hit.category,
                "summary": hit.summary,
                "status": hit.status,
                "path": hit.path,
            })
        })
        .collect();
    json!({ "materials": hits })
}

fn diagnostics_run(project: &Project, scope: &str) -> Value {
    let scope = scope.trim();
    let scene = PartialSceneId::parse_with_sigil(scope).ok();
    let mut out: Vec<Value> = Vec::new();
    let mut documents: Vec<_> = project.documents().collect();
    documents.sort_by(|a, b| a.path.cmp(&b.path));
    for document in documents {
        if scene.is_none() && !scope.is_empty() && !document.path.starts_with(scope) {
            continue;
        }
        let scenes = scene.map(|_| project.scenes_by_line(document));
        let starts = crate::core::features::project::line_starts_of(&document.text);
        for found in diagnostics::document(document, project) {
            let line = found
                .range
                .as_ref()
                .map(|range| crate::core::features::project::line_of(&document.text, range.start))
                .unwrap_or(0);
            if let (Some(partial), Some(scenes)) = (scene.as_ref(), scenes.as_ref()) {
                let inside =
                    scenes.get(line).copied().flatten().is_some_and(|id| {
                        partial.expand_low() <= id && id <= partial.expand_high()
                    });
                if !inside {
                    continue;
                }
            }
            let column = found
                .range
                .as_ref()
                .map(|range| {
                    let start = starts.get(line).copied().unwrap_or(0);
                    document.text[start..range.start.min(document.text.len())]
                        .chars()
                        .count()
                        + 1
                })
                .unwrap_or(1);
            let args: Vec<Value> = found
                .args
                .iter()
                .map(|arg| match arg {
                    Arg::Text(text) => json!(text),
                    Arg::Number(number) => json!(number),
                })
                .collect();
            out.push(json!({
                "path": document.path,
                "line": line + 1,
                "column": column,
                "code": found.code.as_str(),
                "args": args,
            }));
        }
    }
    json!({ "diagnostics": out })
}

/// 本文の節の提案（`origin: mcp`）。
fn section_proposal(
    project: &Project,
    scene: SceneId,
    text: &str,
    note: &str,
    now: &str,
    kind: Kind,
) -> Proposal {
    let paths = files::paths(project);
    let id = files::next_proposal_id(paths.iter().map(String::as_str));
    let digits = project.digits();
    let path = if kind == Kind::PlotField {
        project.episode_plot_path(scene.chapter, scene.episode)
    } else {
        project.contents_path(scene.chapter, scene.episode)
    };
    Proposal {
        id,
        kind: kind.as_str().to_owned(),
        target: files::target_of(&path, Some(scene), digits),
        origin: "mcp".to_owned(),
        status: ProposalStatus::Open.as_str().to_owned(),
        created_at: now.to_owned(),
        note: crate::core::agents::one_line(note),
        before: apply::current_text(project, kind, scene),
        after: text.to_owned(),
        ..Proposal::default()
    }
}

fn section_propose(project: &Project, args: &Value, now: &str) -> Outcome {
    let scene = match scene_arg(args) {
        Ok(scene) => scene,
        Err(error) => return err(error),
    };
    let Some(text) = text_arg(args, "text") else {
        return err(ToolError::Argument("text"));
    };
    let proposal = section_proposal(
        project,
        scene,
        text,
        text_arg(args, "note").unwrap_or(""),
        now,
        Kind::SectionText,
    );
    let mut plan = Plan::default();
    plan.write(
        files::proposal_path(&proposal.id),
        files::write_proposal(&proposal),
    );
    Outcome {
        result: Ok(json!({ "proposal": proposal.id, "status": proposal.status })),
        plan,
    }
}

fn direct_write(project: &Project, args: &Value, now: &str, kind: Kind) -> Outcome {
    let role = text_arg(args, "role").unwrap_or("").trim();
    let level = if crate::core::agents::is_role(role) {
        level_of(&project.settings().agents, role)
    } else {
        Delegation::Off
    };
    if !level.can_write() {
        let shown = if role.is_empty() { "-" } else { role };
        return err(ToolError::Diagnostic(Diagnostic::new(
            DiagnosticCode::E036,
            None,
            vec![Arg::text(shown)],
        )));
    }
    let scene = match scene_arg(args) {
        Ok(scene) => scene,
        Err(error) => return err(error),
    };
    let Some(text) = text_arg(args, "text") else {
        return err(ToolError::Argument("text"));
    };
    let mut proposal = section_proposal(
        project,
        scene,
        text,
        text_arg(args, "note").unwrap_or(""),
        now,
        kind,
    );
    proposal.origin = format!("mcp:{role}");
    let changes = match apply::changes(project, &proposal, text, now, "", role) {
        Ok(changes) => changes,
        Err(_) => return err(ToolError::Missing(scene.display(project.digits()))),
    };
    let refused = apply::gate(project, &changes);
    let mut plan = Plan::default();
    if !refused.is_empty() {
        plan.write(
            files::proposal_path(&proposal.id),
            files::write_proposal(&proposal),
        );
        return Outcome {
            result: Err(ToolError::Gate {
                proposal: proposal.id.clone(),
                codes: apply::codes(&refused),
            }),
            plan,
        };
    }
    for change in changes {
        plan.write(change.path, change.text);
    }
    proposal.status = ProposalStatus::Applied.as_str().to_owned();
    proposal.updated_at = now.to_owned();
    plan.write(
        files::proposal_path(&proposal.id),
        files::write_proposal(&proposal),
    );
    let mut event = LogEvent::new(now, "direct_write");
    event.role = Some(role.to_owned());
    event.server = Some("mcp".to_owned());
    event.proposal = Some(proposal.id.clone());
    plan.log.push(event);
    if apply::is_fixed(project, &proposal) {
        let mut event = LogEvent::new(now, "fixed_overwrite");
        event.role = Some(role.to_owned());
        event.proposal = Some(proposal.id.clone());
        plan.log.push(event);
    }
    Outcome {
        result: Ok(json!({ "proposal": proposal.id, "status": proposal.status })),
        plan,
    }
}

fn materials_propose(project: &Project, args: &Value, now: &str) -> Outcome {
    let Some(markdown) = text_arg(args, "markdown").filter(|found| !found.trim().is_empty()) else {
        return err(ToolError::Argument("markdown"));
    };
    let title = text_arg(args, "title").unwrap_or("");
    let role = text_arg(args, "role")
        .filter(|found| crate::core::agents::is_role(found))
        .unwrap_or("mcp");
    let Some(created) = materials_ops::propose_as(
        project.text(materials_ops::INDEX_PATH),
        if title.trim().is_empty() {
            markdown.lines().next().unwrap_or("")
        } else {
            title
        },
        text_arg(args, "category").unwrap_or(""),
        markdown,
        text_arg(args, "source").unwrap_or(""),
        now,
        &format!("agent:{role}"),
    ) else {
        return err(ToolError::Missing(materials_ops::INDEX_PATH.to_owned()));
    };
    let mut plan = Plan::default();
    plan.write(materials_ops::INDEX_PATH, created.index);
    plan.write(created.path.clone(), created.body);
    Outcome {
        result: Ok(json!({ "id": created.id, "path": created.path, "status": "proposed" })),
        plan,
    }
}

fn mail_send(project: &Project, args: &Value, now: &str) -> Outcome {
    let Some(to) = text_arg(args, "to")
        .map(str::trim)
        .filter(|found| crate::core::agents::is_role(found))
    else {
        return err(ToolError::Argument("to"));
    };
    let Some(body) = text_arg(args, "body").filter(|found| !found.trim().is_empty()) else {
        return err(ToolError::Argument("body"));
    };
    let kind = match text_arg(args, "kind").unwrap_or("note").trim() {
        found @ ("request" | "deliver" | "question" | "reject" | "note") => found.to_owned(),
        _ => return err(ToolError::Argument("kind")),
    };
    let scope = text_arg(args, "scope").unwrap_or("").trim().to_owned();
    if !scope.is_empty() && PartialSceneId::parse_with_sigil(&scope).is_err() {
        return err(ToolError::Argument("scope"));
    }
    let board = Board::of(project);
    let paths = files::paths(project);
    let mut thread = board
        .threads
        .iter()
        .rev()
        .find(|thread| thread.scope == scope && thread.status != "closed")
        .cloned()
        .unwrap_or_else(|| Thread {
            id: files::next_thread_id(paths.iter().map(String::as_str)),
            title: String::new(),
            scope: scope.clone(),
            status: "open".to_owned(),
            messages: Vec::new(),
        });
    let number = thread
        .messages
        .iter()
        .map(|found| found.number)
        .max()
        .unwrap_or(0)
        + 1;
    thread.messages.push(Message {
        number,
        from: "mcp".to_owned(),
        to: to.to_owned(),
        kind,
        created_at: now.to_owned(),
        approved_by: files::PENDING.to_owned(),
        refs: String::new(),
        body: body.trim().to_owned(),
    });
    let mut plan = Plan::default();
    plan.write(files::thread_path(&thread.id), files::write_thread(&thread));
    Outcome {
        result: Ok(
            json!({ "thread": thread.id, "message": number, "approved_by": files::PENDING }),
        ),
        plan,
    }
}
