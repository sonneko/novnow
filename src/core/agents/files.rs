//! `agents/` のファイル形式（§5.10）の読み書きと ID の採番。
//!
//! どのファイルも「H1・`key: value` の行・`## ` のセクション」の Markdown で、novnow が書いたものを
//! novnow が読み戻す（人間が直しても読めるように、知らない行は飛ばし、タスクの知らないキーは残す）。
//!
//! - 結果の `## output` と提案の `## before` / `## after` は**書かれたまま**（LLM の出力・本文の一部
//!   なので `## ` の行を含みうる）。区切りは novnow が書いた位置で決める（[`parse_result`] / [`parse_proposal`]）。
//! - 監査ログ（§5.10.5）は 1 行 1 イベントの JSON で、**追記だけ**（プロンプト本文・出力本文・環境変数の値を
//!   書かない）。
//! - ID は `T` / `P` / `TH` + 4 桁の連番で、**既存の最大値 + 1**（§15.3「再利用禁止」。結果ファイルの
//!   番号も数えるので、タスクのファイルを消しても番号は戻らない）。

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::core::features::Project;
use crate::core::id::{PartialSceneId, SceneId};
use crate::core::model::delegation::Delegation;
use crate::core::model::task::Task;

/// タスクの置き場所。
pub const TASKS_DIR: &str = "agents/tasks/";
/// 結果（とドライラン）の置き場所。
pub const RESULTS_DIR: &str = "agents/results/";
/// 提案の置き場所。
pub const PROPOSALS_DIR: &str = "agents/proposals/";
/// スレッドの置き場所。
pub const THREADS_DIR: &str = "agents/threads/";
/// 監査ログの置き場所。
pub const LOG_DIR: &str = "agents/log/";

/// 監査ログのイベント種別（§5.10.5「固定」）。
pub const EVENTS: &[&str] = &[
    "run",
    "accept",
    "reject",
    "auto_applied",
    "direct_write",
    "fixed_overwrite",
    "delegation_changed",
    "chain_hop",
    "limit_stop",
];

// ============================================================ 読み方の共通部分

/// H1 とフィールド行（最初の `## ` まで）。
struct Sheet<'a> {
    title: &'a str,
    fields: Vec<(&'a str, &'a str)>,
    lines: Vec<&'a str>,
    /// 最初の `## ` の行（無ければ行数）。
    body: usize,
}

fn sheet(text: &str) -> Sheet<'_> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut title = "";
    let mut fields = Vec::new();
    let mut body = lines.len();
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("## ") {
            body = index;
            break;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            if title.is_empty() {
                title = rest.trim();
            }
            continue;
        }
        if let Some(found) = field(line) {
            fields.push(found);
        }
    }
    Sheet {
        title,
        fields,
        lines,
        body,
    }
}

/// `key: value`（キーは ASCII の英数字・`-`・`_`）。
fn field(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some((key, value.trim()))
}

impl<'a> Sheet<'a> {
    fn get(&self, key: &str) -> &'a str {
        self.fields
            .iter()
            .find(|(found, _)| *found == key)
            .map_or("", |(_, value)| value)
    }

    /// 見出し `heading` の行（`from` 以降で最初）。
    fn find(&self, heading: &str, from: usize) -> Option<usize> {
        (from..self.lines.len()).find(|&index| self.lines[index].trim_end() == heading)
    }
}

/// 行を LF でつなぐ。`drop_blank_tail` なら最後の 1 行が空のときだけ落とす（novnow が書いた区切りの空行）。
fn join(lines: &[&str], drop_blank_tail: bool) -> String {
    let mut lines = lines;
    if drop_blank_tail {
        if let Some((last, rest)) = lines.split_last() {
            if last.is_empty() {
                lines = rest;
            }
        }
    }
    lines.join("\n")
}

fn push_field(out: &mut String, key: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    out.push_str(key);
    out.push_str(": ");
    out.push_str(&crate::core::agents::one_line(value));
    out.push('\n');
}

// ============================================================ ID

/// `dir` の直下の `prefix` + 数字（+ `suffix`）の番号。
fn number_in(path: &str, dir: &str, prefix: &str) -> Option<u32> {
    let rest = path.strip_prefix(dir)?.strip_prefix(prefix)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || !rest[digits.len()..].starts_with('.') {
        return None;
    }
    digits.parse().ok()
}

fn next_number<'a>(paths: impl Iterator<Item = &'a str>, places: &[(&str, &str)]) -> u32 {
    paths
        .filter_map(|path| {
            places
                .iter()
                .find_map(|(dir, prefix)| number_in(path, dir, prefix))
        })
        .max()
        .map_or(1, |max| max.saturating_add(1))
}

/// 次のタスク ID（`agents/tasks/` と `agents/results/` の最大値 + 1）。
pub fn next_task_id<'a>(paths: impl Iterator<Item = &'a str>) -> String {
    let number = next_number(paths, &[(TASKS_DIR, "T"), (RESULTS_DIR, "T")]);
    format!("T{number:04}")
}

/// 次の提案 ID。
pub fn next_proposal_id<'a>(paths: impl Iterator<Item = &'a str>) -> String {
    let number = next_number(paths, &[(PROPOSALS_DIR, "P")]);
    format!("P{number:04}")
}

/// 次のスレッド ID。
pub fn next_thread_id<'a>(paths: impl Iterator<Item = &'a str>) -> String {
    let number = next_number(paths, &[(THREADS_DIR, "TH")]);
    format!("TH{number:04}")
}

/// タスクのパス。
pub fn task_path(id: &str) -> String {
    format!("{TASKS_DIR}{id}.md")
}

/// 結果のパス。
pub fn result_path(id: &str) -> String {
    format!("{RESULTS_DIR}{id}.md")
}

/// ドライランのパス（§15.7.2）。
pub fn dryrun_path(id: &str) -> String {
    format!("{RESULTS_DIR}{id}.dryrun.md")
}

/// 提案のパス。
pub fn proposal_path(id: &str) -> String {
    format!("{PROPOSALS_DIR}{id}.md")
}

/// スレッドのパス。
pub fn thread_path(id: &str) -> String {
    format!("{THREADS_DIR}{id}.md")
}

/// プロンプトのパス（既定の役割）。
pub fn prompt_path(role: &str) -> String {
    if super::is_role(role) {
        format!("prompts/{role}.md")
    } else {
        format!("prompts/custom/{role}.md")
    }
}

// ============================================================ タスク（§5.10.1）

/// §5.10.1 の順のキー（書き出しの順）。
const TASK_KEYS: &[&str] = &[
    "role",
    "scope",
    "examples",
    "intent",
    "status",
    "initiator",
    "delegation",
    "prompt",
    "created_at",
    "updated_at",
    "context-hash",
    "result",
    "server",
    "human-comment",
    "exclude",
];

/// タスクを読む。ID は H1（無ければファイル名）。
pub fn parse_task(path: &str, text: &str) -> Option<Task> {
    let found = sheet(text);
    let id = found
        .title
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .or_else(|| {
            path.rsplit('/')
                .next()
                .and_then(|name| name.strip_suffix(".md"))
                .map(str::to_owned)
        })?;
    let mut task = Task {
        id,
        role: found.get("role").to_owned(),
        scope: found.get("scope").to_owned(),
        examples: found
            .fields
            .iter()
            .filter(|(key, _)| *key == "examples")
            .map(|(_, value)| (*value).to_owned())
            .collect(),
        intent: found.get("intent").to_owned(),
        status: found.get("status").to_owned(),
        initiator: found.get("initiator").to_owned(),
        delegation: Delegation::parse(found.get("delegation")).unwrap_or_default(),
        prompt: found.get("prompt").to_owned(),
        created_at: found.get("created_at").to_owned(),
        updated_at: found.get("updated_at").to_owned(),
        context_hash: found.get("context-hash").to_owned(),
        result: found.get("result").to_owned(),
        server: found.get("server").to_owned(),
        human_comment: found.get("human-comment").to_owned(),
        exclude: found.get("exclude").to_owned(),
        unknown: BTreeMap::new(),
    };
    for (key, value) in &found.fields {
        if !TASK_KEYS.contains(key) && !task.unknown.contains_key(*key) {
            task.unknown
                .insert((*key).to_owned(), Value::String((*value).to_owned()));
        }
    }
    Some(task)
}

/// タスクを書く（§5.10.1 の順。空の値は書かない。知らないキーは最後に残す）。
pub fn write_task(task: &Task) -> String {
    let mut out = format!("# {}\n", task.id);
    let delegation = task.delegation.as_str();
    let values: [&str; 14] = [
        &task.role,
        &task.scope,
        &task.intent,
        &task.status,
        &task.initiator,
        delegation,
        &task.prompt,
        &task.created_at,
        &task.updated_at,
        &task.context_hash,
        &task.result,
        &task.server,
        &task.human_comment,
        &task.exclude,
    ];
    for (key, value) in TASK_KEYS
        .iter()
        .filter(|key| **key != "examples")
        .zip(values)
    {
        push_field(&mut out, key, value);
        if *key == "scope" {
            for example in &task.examples {
                push_field(&mut out, "examples", example);
            }
        }
    }
    for (key, value) in &task.unknown {
        let text = match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        push_field(&mut out, key, &text);
    }
    out
}

/// タスクの対象の節（4 階層。場面・話のスコープなら `None`）。
pub fn task_scene(task: &Task) -> Option<SceneId> {
    let partial = PartialSceneId::parse_with_sigil(task.scope.trim()).ok()?;
    (partial.depth() == 4).then(|| partial.expand_low())
}

/// 外したブロック（`exclude:` の名前）。
pub fn excluded_blocks(task: &Task) -> Vec<crate::core::resolve::context_rules::PacketBlock> {
    use crate::core::resolve::context_rules::PacketBlock;
    let mut out: Vec<PacketBlock> = task
        .exclude
        .split(',')
        .filter_map(PacketBlock::parse)
        .filter(|block| !block.is_mandatory())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// `exclude:` の値（固定順）。
pub fn exclude_value(blocks: &[crate::core::resolve::context_rules::PacketBlock]) -> String {
    let mut sorted = blocks.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted
        .iter()
        .map(|block| block.name())
        .collect::<Vec<_>>()
        .join(", ")
}

// ============================================================ 結果（§5.10.2）

/// 実行の結果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunResult {
    /// タスク ID。
    pub task: String,
    /// 終わった日時。
    pub finished_at: String,
    /// サーバが返したモデル識別子。
    pub model: String,
    /// 送った文字数。
    pub input_chars: usize,
    /// 受け取った文字数（切った後）。
    pub output_chars: usize,
    /// かかった時間。
    pub duration_ms: u64,
    /// 20,000 字で切ったか（§15.9）。
    pub truncated: bool,
    /// LLM の出力そのまま。
    pub output: String,
    /// 作った提案のパス（無ければ空）。
    pub proposal: String,
}

/// 結果を書く。
pub fn write_result(result: &RunResult) -> String {
    let mut out = format!("# {} result\n", result.task);
    push_field(&mut out, "finished_at", &result.finished_at);
    push_field(&mut out, "model", &result.model);
    push_field(&mut out, "input_chars", &result.input_chars.to_string());
    push_field(&mut out, "output_chars", &result.output_chars.to_string());
    push_field(&mut out, "duration_ms", &result.duration_ms.to_string());
    if result.truncated {
        push_field(&mut out, "truncated", "true");
    }
    out.push_str("\n## output\n");
    out.push_str(&result.output);
    out.push('\n');
    if !result.proposal.is_empty() {
        out.push_str("\n## proposal\n");
        out.push_str(&result.proposal);
        out.push('\n');
    }
    out
}

/// 結果を読む。
pub fn parse_result(text: &str) -> Option<RunResult> {
    let found = sheet(text);
    let task = found.title.split_whitespace().next()?.to_owned();
    let output_at = found.find("## output", found.body)?;
    // `## proposal` は novnow が最後に書く（出力の中の同じ見出しと区別するため、後ろから探す）
    let proposal_at = (output_at + 1..found.lines.len()).rev().find(|&index| {
        found.lines[index].trim_end() == "## proposal"
            && found
                .lines
                .get(index + 1)
                .is_some_and(|next| next.starts_with(PROPOSALS_DIR))
    });
    let (output, proposal) = match proposal_at {
        Some(at) => (
            join(&found.lines[output_at + 1..at], true),
            found.lines[at + 1].trim().to_owned(),
        ),
        None => (join(&found.lines[output_at + 1..], true), String::new()),
    };
    let number = |key: &str| found.get(key).parse().unwrap_or(0);
    Some(RunResult {
        task,
        finished_at: found.get("finished_at").to_owned(),
        model: found.get("model").to_owned(),
        input_chars: number("input_chars") as usize,
        output_chars: number("output_chars") as usize,
        duration_ms: number("duration_ms"),
        truncated: found.get("truncated") == "true",
        output,
        proposal,
    })
}

// ============================================================ 提案（§5.10.3）

/// 提案の状態（§5.10.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProposalStatus {
    /// レビュー待ち。
    Open,
    /// 採用。
    Accepted,
    /// 却下（差し戻しもこれ。コメントが要る）。
    Rejected,
    /// 同じ対象の別の提案が採用された。
    Superseded,
    /// 自動適用済み（§15.1.1）。
    Applied,
}

impl ProposalStatus {
    /// `status:` の値。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
            Self::Applied => "applied",
        }
    }

    /// `status:` の値から。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "open" => Some(Self::Open),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "superseded" => Some(Self::Superseded),
            "applied" => Some(Self::Applied),
            _ => None,
        }
    }
}

/// 提案。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proposal {
    /// `P0003`。
    pub id: String,
    /// `section-text` など（[`crate::core::agents::Kind`]）。
    pub kind: String,
    /// `contents/01-04.cnov @01-04-02-03`（パス + 空白 + 節）。
    pub target: String,
    /// 元のタスク（MCP のツールから作られたものは `mcp`）。
    pub origin: String,
    /// 状態。
    pub status: String,
    /// 作った日時。
    pub created_at: String,
    /// 状態を変えた日時。
    pub updated_at: String,
    /// 却下・差し戻しの理由（§15.7.3「却下時はコメント必須」）。
    pub comment: String,
    /// 作った側の添え書き（`novnow.section.propose` の `note`）。
    pub note: String,
    /// 一部採用（§15.7.3「行選択」）で採用したか（`after` は提案のまま残す）。
    pub partial: bool,
    /// 適用前のテキスト。
    pub before: String,
    /// 提案のテキスト。
    pub after: String,
}

impl Proposal {
    /// 状態。
    pub fn status_kind(&self) -> Option<ProposalStatus> {
        ProposalStatus::parse(&self.status)
    }

    /// 対象のパスと節。
    pub fn target_parts(&self) -> (String, Option<SceneId>) {
        split_target(&self.target)
    }
}

/// `target:` → (パス, 節)。
pub fn split_target(target: &str) -> (String, Option<SceneId>) {
    match target.split_once(" @") {
        Some((path, id)) => (
            path.trim().to_owned(),
            PartialSceneId::parse(id.trim())
                .ok()
                .filter(|found| found.depth() == 4)
                .map(|found| found.expand_low()),
        ),
        None => (target.trim().to_owned(), None),
    }
}

/// `target:` の値。
pub fn target_of(path: &str, scene: Option<SceneId>, digits: crate::core::id::Digits) -> String {
    match scene {
        Some(id) => format!("{path} @{}", id.display(digits)),
        None => path.to_owned(),
    }
}

/// 提案を書く。
pub fn write_proposal(proposal: &Proposal) -> String {
    let mut out = format!("# {}\n", proposal.id);
    push_field(&mut out, "kind", &proposal.kind);
    push_field(&mut out, "target", &proposal.target);
    push_field(&mut out, "origin", &proposal.origin);
    push_field(&mut out, "status", &proposal.status);
    push_field(&mut out, "created_at", &proposal.created_at);
    push_field(&mut out, "updated_at", &proposal.updated_at);
    push_field(&mut out, "comment", &proposal.comment);
    push_field(&mut out, "note", &proposal.note);
    if proposal.partial {
        push_field(&mut out, "partial", "true");
    }
    out.push_str("\n## before\n");
    out.push_str(&proposal.before);
    out.push_str("\n\n## after\n");
    out.push_str(&proposal.after);
    out.push('\n');
    out
}

/// 提案を読む。
pub fn parse_proposal(text: &str) -> Option<Proposal> {
    let found = sheet(text);
    let id = found.title.split_whitespace().next()?.to_owned();
    let before_at = found.find("## before", found.body)?;
    let after_at = found.find("## after", before_at + 1)?;
    Some(Proposal {
        id,
        kind: found.get("kind").to_owned(),
        target: found.get("target").to_owned(),
        origin: found.get("origin").to_owned(),
        status: found.get("status").to_owned(),
        created_at: found.get("created_at").to_owned(),
        updated_at: found.get("updated_at").to_owned(),
        comment: found.get("comment").to_owned(),
        note: found.get("note").to_owned(),
        partial: found.get("partial") == "true",
        before: join(&found.lines[before_at + 1..after_at], true),
        after: join(&found.lines[after_at + 1..], true),
    })
}

// ============================================================ スレッド（§5.10.4）

/// 未承認のメッセージの `approved_by:`（§15.6。人間が中継を承認するまで宛先には届かない。
/// 第 2.16 版、`docs/history.md` B-145）。
pub const PENDING: &str = "pending";

/// メッセージ 1 通。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    /// 通し番号（`## msg 001`）。
    pub number: u32,
    /// 送り手の役割（MCP のツールからなら `mcp`）。
    pub from: String,
    /// 宛先の役割。
    pub to: String,
    /// `request | deliver | question | reject | note`。
    pub kind: String,
    /// 作った日時。
    pub created_at: String,
    /// `pending | human | auto`。
    pub approved_by: String,
    /// 参照（`'M0141`）。
    pub refs: String,
    /// 本文。
    pub body: String,
}

impl Message {
    /// 宛先に届くか（人間の承認か自律の自動受け渡し）。
    pub fn delivered(&self) -> bool {
        self.approved_by == "human" || self.approved_by == "auto"
    }
}

/// スレッド。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Thread {
    /// `TH0007`。
    pub id: String,
    /// 題（H1 の ID の後ろ）。
    pub title: String,
    /// `@01-04-02`。
    pub scope: String,
    /// `open` など。
    pub status: String,
    /// メッセージ（番号順）。
    pub messages: Vec<Message>,
}

/// スレッドを書く。
pub fn write_thread(thread: &Thread) -> String {
    let mut out = format!("# {} {}\n", thread.id, thread.title)
        .trim_end()
        .to_owned();
    out.push('\n');
    push_field(&mut out, "scope", &thread.scope);
    push_field(&mut out, "status", &thread.status);
    for message in &thread.messages {
        out.push_str(&format!("\n## msg {:03}\n", message.number));
        push_field(&mut out, "from", &message.from);
        push_field(&mut out, "to", &message.to);
        push_field(&mut out, "kind", &message.kind);
        push_field(&mut out, "created_at", &message.created_at);
        push_field(&mut out, "approved_by", &message.approved_by);
        push_field(&mut out, "refs", &message.refs);
        out.push_str("body:\n");
        for line in message.body.split('\n') {
            if line.is_empty() {
                out.push('\n');
            } else {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

/// スレッドを読む。
pub fn parse_thread(text: &str) -> Option<Thread> {
    let found = sheet(text);
    let (id, title) = match found.title.split_once(' ') {
        Some((id, title)) => (id.to_owned(), title.trim().to_owned()),
        None => (found.title.to_owned(), String::new()),
    };
    if id.is_empty() {
        return None;
    }
    let mut messages = Vec::new();
    let mut index = found.body;
    while index < found.lines.len() {
        let line = found.lines[index];
        let Some(number) = line
            .strip_prefix("## msg ")
            .and_then(|rest| rest.trim().parse::<u32>().ok())
        else {
            index += 1;
            continue;
        };
        let mut message = Message {
            number,
            ..Message::default()
        };
        index += 1;
        let mut body: Vec<&str> = Vec::new();
        let mut in_body = false;
        while index < found.lines.len() && !found.lines[index].starts_with("## ") {
            let line = found.lines[index];
            if in_body {
                body.push(line.strip_prefix("  ").unwrap_or(line.trim_start()));
            } else if line.trim_end() == "body:" {
                in_body = true;
            } else if let Some((key, value)) = field(line) {
                let value = value.to_owned();
                match key {
                    "from" => message.from = value,
                    "to" => message.to = value,
                    "kind" => message.kind = value,
                    "created_at" => message.created_at = value,
                    "approved_by" => message.approved_by = value,
                    "refs" => message.refs = value,
                    _ => {}
                }
            }
            index += 1;
        }
        while body.last().is_some_and(|line| line.trim().is_empty()) {
            body.pop();
        }
        message.body = body.join("\n");
        messages.push(message);
    }
    Some(Thread {
        id,
        title,
        scope: found.get("scope").to_owned(),
        status: found.get("status").to_owned(),
        messages,
    })
}

// ============================================================ 監査ログ（§5.10.5）

/// 監査ログの 1 イベント。キーは §5.10.5 の順（`proposal` と `detail` は第 2.16 版の追加で、
/// 値があるときだけ書く。`docs/history.md` B-146）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LogEvent {
    /// 日時（RFC 3339）。
    pub ts: String,
    /// 種別（[`EVENTS`]）。
    pub event: String,
    /// タスク ID。
    pub task: Option<String>,
    /// 役割。
    pub role: Option<String>,
    /// サーバ名。
    pub server: Option<String>,
    /// 送った文字数。
    pub input_chars: Option<usize>,
    /// 受け取った文字数。
    pub output_chars: Option<usize>,
    /// かかった時間。
    pub duration_ms: Option<u64>,
    /// 結果ファイル。
    pub result_path: Option<String>,
    /// 失敗の理由（診断コードと短い説明。秘密情報を入れない）。
    pub error: Option<String>,
    /// 提案 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<String>,
    /// 補足（`delegation_changed` の `writer: propose -> apply`、`limit_stop` の上限の名前など）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl LogEvent {
    /// `event` の空のイベント。
    pub fn new(ts: &str, event: &str) -> Self {
        debug_assert!(EVENTS.contains(&event), "{event}");
        Self {
            ts: ts.to_owned(),
            event: event.to_owned(),
            ..Self::default()
        }
    }

    /// 1 行（改行込み）。
    pub fn line(&self) -> String {
        let mut out = serde_json::to_string(self).unwrap_or_default();
        out.push('\n');
        out
    }

    /// 書く先（`agents/log/YYYY-MM-DD.jsonl`。日付は `ts` の日付部分）。
    pub fn path(&self) -> String {
        log_path(&self.ts)
    }
}

/// `ts` の日のログ。
pub fn log_path(ts: &str) -> String {
    let date: String = ts.chars().take(10).collect();
    let date = if date.len() == 10 {
        date
    } else {
        "0000-00-00".to_owned()
    };
    format!("{LOG_DIR}{date}.jsonl")
}

// ============================================================ 全体

/// `agents/` の中身（プロジェクトの文書から読んだもの）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Board {
    /// タスク（ID 順）。
    pub tasks: Vec<Task>,
    /// 提案（ID 順）。
    pub proposals: Vec<Proposal>,
    /// スレッド（ID 順）。
    pub threads: Vec<Thread>,
}

impl Board {
    /// プロジェクトの文書から読む。
    pub fn of(project: &Project) -> Self {
        let mut out = Self::default();
        for document in project.documents() {
            let path = document.path.as_str();
            if number_in(path, TASKS_DIR, "T").is_some() {
                if let Some(task) = parse_task(path, &document.text) {
                    out.tasks.push(task);
                }
            } else if number_in(path, PROPOSALS_DIR, "P").is_some()
                || number_in(path, PROPOSALS_DIR, "H").is_some()
            {
                if let Some(proposal) = parse_proposal(&document.text) {
                    out.proposals.push(proposal);
                }
            } else if number_in(path, THREADS_DIR, "TH").is_some() {
                if let Some(thread) = parse_thread(&document.text) {
                    out.threads.push(thread);
                }
            }
        }
        out.tasks.sort_by(|a, b| a.id.cmp(&b.id));
        out.proposals.sort_by(|a, b| a.id.cmp(&b.id));
        out.threads.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// ID のタスク。
    pub fn task(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|task| task.id == id)
    }

    /// タスクから生まれた提案（新しい順に最初の 1 つ）。
    pub fn proposal_of(&self, task: &str) -> Option<&Proposal> {
        self.proposals
            .iter()
            .rev()
            .find(|proposal| proposal.origin == task)
    }

    /// ID の提案。
    pub fn proposal(&self, id: &str) -> Option<&Proposal> {
        self.proposals.iter().find(|proposal| proposal.id == id)
    }
}

/// プロジェクトの全パス（ID の採番用）。
pub fn paths(project: &Project) -> Vec<String> {
    project
        .documents()
        .map(|document| document.path.clone())
        .collect()
}
