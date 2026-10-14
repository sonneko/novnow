//! 提案の適用（§15.7.3・§15.1.1）。採用・自動適用・直接書き込み・差し戻しが同じ関数を通る。
//!
//! - 適用先: `section-text` → 本文の節（無ければ前の節の後ろ・新しい場面として足す）、`plot-field` →
//!   話プロットの節、`idea` → `ideas.md` に 1 行 1 件で追記（新しい `I####`）、`material` →
//!   `materials/inbox/` に `status: proposed` で追加（§16.1）、`note` → 適用しない（読むだけ）。
//! - 自動適用の前の検証（[`gate`]）: 適用で**新しく** E 系の診断が増えるなら適用しない（§15.9 MUST・B-11）。
//! - 採用すると、同じ対象の他の `open` の提案は `superseded`（§15.9）。
//! - 差し戻し（[`revert`]）は本文・プロットを `before` に戻し、提案を `rejected` にする（コメント必須）。
//!   「undo 可能」（§15.7.3）は、適用がエディタの編集ではなくファイルの書き換えなので、この差し戻しで
//!   実現する（`PLANS.md` §9.2 B-142）。

use crate::core::agents::files::{self, Board, LogEvent, Proposal, ProposalStatus};
use crate::core::agents::{Change, Kind, Plan};
use crate::core::diagnostics::{self, Diagnostic, Severity};
use crate::core::export::State;
use crate::core::features::{ideas, materials_ops, sections, Project};
use crate::core::id::SceneId;
use crate::core::model::document::Document;
use crate::core::syntax::plot_episode;

/// 適用できない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// 提案・タスクが無い。
    Unknown,
    /// 適用先（節・プロットの節）が見つからない。
    Missing,
    /// 提案が空（本文を消すことになる）。
    Empty,
    /// 適用すると E 系の診断が増える（自動適用・直接書き込みのとき）。
    Gate(Vec<Diagnostic>),
    /// もう決着した提案（`open` でない。差し戻しは `accepted` / `applied` だけ）。
    Settled,
}

/// 提案の対象の節が確定済みか（A7。採用に確認を 1 段挟む、§15.9）。
pub fn is_fixed(project: &Project, proposal: &Proposal) -> bool {
    match proposal.target_parts() {
        (_, Some(scene)) => crate::core::export::effective_state(project, scene) == State::Fixed,
        _ => false,
    }
}

/// 対象の今のテキスト（`section-text` / `plot-field` だけ。無ければ空）。
pub fn current_text(project: &Project, kind: Kind, scene: SceneId) -> String {
    match kind {
        Kind::SectionText => {
            let path = project.contents_path(scene.chapter, scene.episode);
            sections::all(project)
                .iter()
                .find(|found| found.path == path && found.id == scene)
                .map(|found| sections::text_of(project.text(&path), found))
                .unwrap_or_default()
        }
        Kind::PlotField => {
            let path = project.episode_plot_path(scene.chapter, scene.episode);
            let text = project.text(&path);
            plot_section(project, text, scene)
                .map(|range| sections::text_of_lines(text, &range))
                .unwrap_or_default()
        }
        _ => String::new(),
    }
}

/// 提案を出してから対象が変わったか（今のテキスト ≠ `before`）。
pub fn drifted(project: &Project, proposal: &Proposal) -> bool {
    let Some(kind) = Kind::parse(&proposal.kind) else {
        return false;
    };
    match proposal.target_parts() {
        (_, Some(scene)) if matches!(kind, Kind::SectionText | Kind::PlotField) => {
            current_text(project, kind, scene) != proposal.before
        }
        _ => false,
    }
}

/// 話プロットの節の内容の行（末尾の空行を除く）。
fn plot_section(project: &Project, text: &str, scene: SceneId) -> Option<std::ops::Range<usize>> {
    let plot = plot_episode::parse_with(text, &project.contents_options());
    let whole = SceneId {
        section: 0,
        ..scene
    };
    let found = plot
        .scenes
        .iter()
        .find(|found| found.id.map(|id| SceneId { section: 0, ..id }) == Some(whole))?;
    let section = found
        .sections
        .iter()
        .find(|section| section.number == scene.section)?;
    Some(if section.content.is_empty() {
        section.span.start..section.span.start
    } else {
        section.content.clone()
    })
}

/// 節の本文を `body` にする書き換え。節が無ければ、同じ場面の前の節の後ろに足す（節 1 で場面も
/// 無ければ、ファイルの末尾に場面ごと足す）。どちらでもなければ [`Refusal::Missing`]。
pub fn section_change(project: &Project, scene: SceneId, body: &str) -> Result<Change, Refusal> {
    let path = project.contents_path(scene.chapter, scene.episode);
    let text = project.text(&path);
    let list = sections::all(project);
    let in_file = |id: SceneId| {
        list.iter()
            .find(|found| found.path == path && found.id == id)
    };
    if let Some(found) = in_file(scene) {
        return Ok(Change {
            text: sections::splice(text, found, body),
            path,
        });
    }
    if scene.section > 1 {
        let previous = SceneId {
            section: scene.section - 1,
            ..scene
        };
        let Some(found) = in_file(previous) else {
            return Err(Refusal::Missing);
        };
        let (inserted, at) = sections::insert_after(text, found);
        return Ok(Change {
            text: sections::splice_lines(&inserted, &(at..at), body),
            path,
        });
    }
    let whole = SceneId {
        section: 0,
        ..scene
    };
    if list.iter().any(|found| {
        found.path == path
            && SceneId {
                section: 0,
                ..found.id
            } == whole
    }) {
        return Err(Refusal::Missing);
    }
    let mut out = text.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push('@');
    out.push_str(&whole.display(project.digits()));
    out.push('\n');
    out.push_str(body);
    out.push('\n');
    Ok(Change { path, text: out })
}

/// 話プロットの節の内容を `body` にする書き換え（節が無ければ [`Refusal::Missing`]）。
pub fn plot_change(project: &Project, scene: SceneId, body: &str) -> Result<Change, Refusal> {
    let path = project.episode_plot_path(scene.chapter, scene.episode);
    let text = project.text(&path);
    let range = plot_section(project, text, scene).ok_or(Refusal::Missing)?;
    Ok(Change {
        text: sections::splice_lines(text, &range, body),
        path,
    })
}

/// 案の行の頭に付きがちな印（落とす）。
/// i18n: format-vocabulary
const BULLETS: &[&str] = &["- ", "・", "* "];

/// 案の行（`- ` などの印を落とす。最大 20 件、付録 A の ideator の output）。
pub fn idea_lines(body: &str) -> Vec<String> {
    body.lines()
        .map(|line| {
            let line = line.trim();
            let line = BULLETS
                .iter()
                .find_map(|bullet| line.strip_prefix(bullet))
                .unwrap_or(line);
            line.trim().to_owned()
        })
        .filter(|line| !line.is_empty())
        .take(20)
        .collect()
}

/// `ideas.md` に案を足す書き換え。
pub fn ideas_change(project: &Project, body: &str, now: &str) -> Result<Change, Refusal> {
    let lines = idea_lines(body);
    if lines.is_empty() {
        return Err(Refusal::Empty);
    }
    let mut text = project.text(ideas::PATH).to_owned();
    for line in lines {
        text = ideas::capture(&text, &line, now).0;
    }
    Ok(Change {
        path: ideas::PATH.to_owned(),
        text,
    })
}

/// 資料を `status: proposed` で足す書き換え（`index.json` と `materials/inbox/M####.md`）。
pub fn material_change(
    project: &Project,
    title: &str,
    body: &str,
    now: &str,
    added_by: &str,
) -> Result<Vec<Change>, Refusal> {
    if body.trim().is_empty() {
        return Err(Refusal::Empty);
    }
    let title = if title.trim().is_empty() {
        body.lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .chars()
            .take(40)
            .collect::<String>()
    } else {
        title.trim().to_owned()
    };
    let created = materials_ops::propose_as(
        project.text(materials_ops::INDEX_PATH),
        &title,
        "",
        body,
        "",
        now,
        added_by,
    )
    .ok_or(Refusal::Missing)?;
    Ok(vec![
        Change {
            path: materials_ops::INDEX_PATH.to_owned(),
            text: created.index,
        },
        Change {
            path: created.path,
            text: created.body,
        },
    ])
}

/// 提案を `text` で適用する書き換え（`note` は空）。`title` は資料の題（タスクの intent）。
pub fn changes(
    project: &Project,
    proposal: &Proposal,
    text: &str,
    now: &str,
    title: &str,
    role: &str,
) -> Result<Vec<Change>, Refusal> {
    let kind = Kind::parse(&proposal.kind).unwrap_or(Kind::Note);
    let (_, scene) = proposal.target_parts();
    match kind {
        Kind::SectionText => {
            if text.trim().is_empty() {
                return Err(Refusal::Empty);
            }
            let scene = scene.ok_or(Refusal::Missing)?;
            Ok(vec![section_change(project, scene, text)?])
        }
        Kind::PlotField => {
            if text.trim().is_empty() {
                return Err(Refusal::Empty);
            }
            let scene = scene.ok_or(Refusal::Missing)?;
            Ok(vec![plot_change(project, scene, text)?])
        }
        Kind::Idea => Ok(vec![ideas_change(project, text, now)?]),
        Kind::Material => material_change(project, title, text, now, &format!("agent:{role}")),
        Kind::Note => Ok(Vec::new()),
    }
}

fn errors(found: Vec<Diagnostic>) -> Vec<Diagnostic> {
    found
        .into_iter()
        .filter(|diagnostic| diagnostic.code.severity() == Severity::Error)
        .collect()
}

/// 自動適用の前の検証（§15.9 MUST）: 書き換えで**増える** E 系の診断。空なら適用してよい。
pub fn gate(project: &Project, changes: &[Change]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for change in changes {
        let after = errors(diagnostics::document(
            &Document::new(change.path.clone(), change.text.clone()),
            project,
        ));
        let before = project
            .document(&change.path)
            .map(|document| errors(diagnostics::document(document, project)))
            .unwrap_or_default();
        if after.len() > before.len() {
            out.extend(
                after
                    .into_iter()
                    .filter(|found| !before.iter().any(|seen| seen.code == found.code)),
            );
        }
    }
    out
}

/// コードを並べた文字列（`E001, E013`）。
pub fn codes(found: &[Diagnostic]) -> String {
    let mut out: Vec<&str> = found.iter().map(|found| found.code.as_str()).collect();
    out.sort();
    out.dedup();
    out.join(", ")
}

fn origin_role(board: &Board, proposal: &Proposal) -> String {
    board
        .task(&proposal.origin)
        .map(|task| task.role.clone())
        .unwrap_or_else(|| proposal.origin.clone())
}

fn origin_intent(board: &Board, proposal: &Proposal) -> String {
    board
        .task(&proposal.origin)
        .map(|task| task.intent.clone())
        .unwrap_or_default()
}

/// 採用（§15.7.3 `A`。`text` があれば一部採用のテキスト）。
pub fn accept(project: &Project, id: &str, text: Option<&str>, now: &str) -> Result<Plan, Refusal> {
    let board = Board::of(project);
    let proposal = board.proposal(id).ok_or(Refusal::Unknown)?;
    if proposal.status_kind() != Some(ProposalStatus::Open) {
        return Err(Refusal::Settled);
    }
    let role = origin_role(&board, proposal);
    let applied_text = text.unwrap_or(&proposal.after);
    let mut plan = Plan::default();
    for change in changes(
        project,
        proposal,
        applied_text,
        now,
        &origin_intent(&board, proposal),
        &role,
    )? {
        plan.write(change.path, change.text);
    }
    let mut updated = proposal.clone();
    updated.status = ProposalStatus::Accepted.as_str().to_owned();
    updated.updated_at = now.to_owned();
    updated.partial = text.is_some_and(|found| found != proposal.after);
    plan.write(
        files::proposal_path(&updated.id),
        files::write_proposal(&updated),
    );
    settle_task(&board, proposal, "accepted", now, &mut plan);
    supersede(&board, proposal, now, &mut plan);
    let mut event = LogEvent::new(now, "accept");
    event.task = Some(proposal.origin.clone());
    event.role = Some(role);
    event.proposal = Some(proposal.id.clone());
    plan.log.push(event);
    Ok(plan)
}

/// 却下（§15.7.3 `X`。コメント必須）。
pub fn reject(project: &Project, id: &str, comment: &str, now: &str) -> Result<Plan, Refusal> {
    let board = Board::of(project);
    let proposal = board.proposal(id).ok_or(Refusal::Unknown)?;
    if proposal.status_kind() != Some(ProposalStatus::Open) {
        return Err(Refusal::Settled);
    }
    let mut plan = Plan::default();
    let mut updated = proposal.clone();
    updated.status = ProposalStatus::Rejected.as_str().to_owned();
    updated.updated_at = now.to_owned();
    updated.comment = crate::core::agents::one_line(comment);
    plan.write(
        files::proposal_path(&updated.id),
        files::write_proposal(&updated),
    );
    settle_task(&board, proposal, "rejected", now, &mut plan);
    let mut event = LogEvent::new(now, "reject");
    event.task = Some(proposal.origin.clone());
    event.role = Some(origin_role(&board, proposal));
    event.proposal = Some(proposal.id.clone());
    plan.log.push(event);
    Ok(plan)
}

/// 差し戻し（§15.7.3。`accepted` / `applied` の提案の対象を `before` に戻し、`rejected` にする）。
pub fn revert(project: &Project, id: &str, comment: &str, now: &str) -> Result<Plan, Refusal> {
    let board = Board::of(project);
    let proposal = board.proposal(id).ok_or(Refusal::Unknown)?;
    if !matches!(
        proposal.status_kind(),
        Some(ProposalStatus::Accepted | ProposalStatus::Applied)
    ) {
        return Err(Refusal::Settled);
    }
    let kind = Kind::parse(&proposal.kind).unwrap_or(Kind::Note);
    let mut plan = Plan::default();
    if let (Kind::SectionText | Kind::PlotField, (_, Some(scene))) = (kind, proposal.target_parts())
    {
        let change = if kind == Kind::SectionText {
            section_change(project, scene, &proposal.before)?
        } else {
            plot_change(project, scene, &proposal.before)?
        };
        plan.write(change.path, change.text);
    }
    let mut updated = proposal.clone();
    updated.status = ProposalStatus::Rejected.as_str().to_owned();
    updated.updated_at = now.to_owned();
    updated.comment = crate::core::agents::one_line(comment);
    plan.write(
        files::proposal_path(&updated.id),
        files::write_proposal(&updated),
    );
    settle_task(&board, proposal, "rejected", now, &mut plan);
    let mut event = LogEvent::new(now, "reject");
    event.task = Some(proposal.origin.clone());
    event.role = Some(origin_role(&board, proposal));
    event.proposal = Some(proposal.id.clone());
    event.detail = Some("revert".to_owned());
    plan.log.push(event);
    Ok(plan)
}

fn settle_task(board: &Board, proposal: &Proposal, status: &str, now: &str, plan: &mut Plan) {
    if let Some(task) = board.task(&proposal.origin) {
        let mut task = task.clone();
        task.status = status.to_owned();
        task.updated_at = now.to_owned();
        plan.write(files::task_path(&task.id), files::write_task(&task));
    }
}

/// 同じ対象の他の `open` の提案を `superseded` にする（§15.9）。
fn supersede(board: &Board, accepted: &Proposal, now: &str, plan: &mut Plan) {
    let kind = Kind::parse(&accepted.kind).unwrap_or(Kind::Note);
    if !matches!(kind, Kind::SectionText | Kind::PlotField) {
        return;
    }
    for other in &board.proposals {
        if other.id == accepted.id
            || other.status_kind() != Some(ProposalStatus::Open)
            || other.target != accepted.target
            || other.kind != accepted.kind
        {
            continue;
        }
        let mut updated = other.clone();
        updated.status = ProposalStatus::Superseded.as_str().to_owned();
        updated.updated_at = now.to_owned();
        plan.write(
            files::proposal_path(&updated.id),
            files::write_proposal(&updated),
        );
        if let Some(task) = board.task(&other.origin) {
            if task.status == "review" {
                let mut task = task.clone();
                task.status = "rejected".to_owned();
                task.updated_at = now.to_owned();
                plan.write(files::task_path(&task.id), files::write_task(&task));
            }
        }
    }
}
