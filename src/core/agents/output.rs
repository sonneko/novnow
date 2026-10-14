//! LLM の出力の読み方（§15.5.2・§15.6・§15.9）と、レビューの行差分（§15.7.3）。
//!
//! 出力は**加工せずに**結果ファイルへ残す（§5.10.2）。ここでするのは「どこが提案の本体か」を
//! 読むことだけ:
//!
//! - 最初の `## question` / `## message` / `## task` の見出しより前が本体（提案の `after`）。
//! - `## question`: 人間への質問（§15.5.2-6）。提案と一緒にレビューに出す。
//! - `## message`: 他の役割への伝言（§15.6）。`to:` / `kind:` / `refs:` の行と本文。未承認で積む。
//! - `## task`: 次のタスクの起票（`role:` / `scope:` / `intent:` の行）。**integrator が `autonomous`
//!   のときだけ**読む（§15.1.1・B-6。他の役割の `## task` は無視する）。
//!
//! 契約違反（前置き・見出し・コードブロック・記法・字数）は警告として見せるだけで、整形はしない
//! （§15.9「採用時に自動整形しない」）。

use crate::core::agents::Kind;

/// 伝言（`## message`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutMessage {
    /// 宛先の役割。
    pub to: String,
    /// `request | deliver | question | reject | note`（無ければ `note`）。
    pub kind: String,
    /// 参照。
    pub refs: String,
    /// 本文。
    pub body: String,
}

/// 起票（`## task`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutTask {
    /// 役割。
    pub role: String,
    /// 対象（`@01-04-02-03`）。
    pub scope: String,
    /// 目的。
    pub intent: String,
}

/// 読んだ出力。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// 本体（前後の空行を落としたもの）。
    pub main: String,
    /// 質問。
    pub questions: Vec<String>,
    /// 伝言。
    pub messages: Vec<OutMessage>,
    /// 起票。
    pub tasks: Vec<OutTask>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Main,
    Question,
    Message,
    Task,
}

fn heading(line: &str) -> Option<Part> {
    match line.trim() {
        "## question" => Some(Part::Question),
        "## message" => Some(Part::Message),
        "## task" => Some(Part::Task),
        _ => None,
    }
}

/// 出力を読む。
pub fn parse(output: &str) -> Parsed {
    let mut parts: Vec<(Part, Vec<&str>)> = vec![(Part::Main, Vec::new())];
    for line in output.lines() {
        match heading(line) {
            Some(part) => parts.push((part, Vec::new())),
            None => {
                if let Some((_, lines)) = parts.last_mut() {
                    lines.push(line);
                }
            }
        }
    }
    let mut out = Parsed::default();
    for (part, lines) in parts {
        let text = trim_lines(&lines);
        match part {
            Part::Main => out.main = text,
            Part::Question => {
                if !text.is_empty() {
                    out.questions.push(text);
                }
            }
            Part::Message => {
                let (fields, body) = fields_and_body(&lines);
                let get = |key: &str| {
                    fields
                        .iter()
                        .find(|(found, _)| *found == key)
                        .map(|(_, value)| value.to_string())
                        .unwrap_or_default()
                };
                let message = OutMessage {
                    to: get("to"),
                    kind: match get("kind").as_str() {
                        "" => "note".to_owned(),
                        other => other.to_owned(),
                    },
                    refs: get("refs"),
                    body,
                };
                if !message.to.is_empty() && !message.body.is_empty() {
                    out.messages.push(message);
                }
            }
            Part::Task => {
                let (fields, _) = fields_and_body(&lines);
                let get = |key: &str| {
                    fields
                        .iter()
                        .find(|(found, _)| *found == key)
                        .map(|(_, value)| value.to_string())
                        .unwrap_or_default()
                };
                let task = OutTask {
                    role: get("role"),
                    scope: get("scope"),
                    intent: get("intent"),
                };
                if !task.role.is_empty() && !task.scope.is_empty() {
                    out.tasks.push(task);
                }
            }
        }
    }
    out
}

/// 先頭の `key: value` の行（ASCII のキー）と、残りの本文。
fn fields_and_body<'a>(lines: &[&'a str]) -> (Vec<(&'a str, &'a str)>, String) {
    let mut fields = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if line.is_empty() && fields.is_empty() {
            index += 1;
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            break;
        };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            break;
        }
        fields.push((key, value.trim()));
        index += 1;
    }
    (fields, trim_lines(&lines[index..]))
}

fn trim_lines(lines: &[&str]) -> String {
    let start = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(start, |at| at + 1);
    lines[start..end.max(start)].join("\n")
}

// ============================================================ 契約違反（§15.9）

/// 契約違反の種類（表示の文言は UI のカタログ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Violation {
    /// 本体が空（質問だけ・何も無い）。
    Empty,
    /// 前置き（最初の行が `：` で終わる短い文）。
    Preamble,
    /// Markdown の見出し。
    Heading,
    /// コードブロック。
    CodeFence,
    /// novnow の記法（行頭の `@` / `;`）。
    Annotation,
    /// 字数が指定の ±20% を外れた。
    Length,
}

impl Violation {
    /// UI のカタログの鍵（`agent.violation.<name>`）。
    pub fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Preamble => "preamble",
            Self::Heading => "heading",
            Self::CodeFence => "code-fence",
            Self::Annotation => "annotation",
            Self::Length => "length",
        }
    }
}

/// 契約違反を探す（`target_chars` は本文の目標字数、§15.5.2-7 の ±20%）。
pub fn violations(kind: Kind, parsed: &Parsed, target_chars: Option<usize>) -> Vec<Violation> {
    let mut out = Vec::new();
    let main = parsed.main.as_str();
    if main.trim().is_empty() {
        if kind != Kind::Note {
            out.push(Violation::Empty);
        }
        return out;
    }
    let content_kind = matches!(kind, Kind::SectionText | Kind::PlotField | Kind::Idea);
    let first = main
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    if content_kind {
        let trimmed = first.trim();
        if (trimmed.ends_with('：') || trimmed.ends_with(':')) && trimmed.chars().count() <= 40 {
            out.push(Violation::Preamble);
        }
        if main.lines().any(|line| {
            line.starts_with("# ") || line.starts_with("## ") || line.starts_with("### ")
        }) {
            out.push(Violation::Heading);
        }
    }
    if kind != Kind::Note
        && main
            .lines()
            .any(|line| line.trim_start().starts_with("```"))
    {
        out.push(Violation::CodeFence);
    }
    if kind == Kind::SectionText
        && main
            .lines()
            .any(|line| line.starts_with('@') || line.trim() == ";" || line.starts_with("; "))
    {
        out.push(Violation::Annotation);
    }
    if kind == Kind::SectionText {
        if let Some(target) = target_chars.filter(|found| *found > 0) {
            let actual = crate::core::features::sections::count_chars(main);
            let low = target * 8 / 10;
            let high = target * 12 / 10;
            if actual < low || actual > high {
                out.push(Violation::Length);
            }
        }
    }
    out
}

// ============================================================ 行差分（§15.7.3）

/// 差分の 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// 両方にある。
    Same(String),
    /// `before` にだけある。
    Removed(String),
    /// `after` にだけある。
    Added(String),
}

impl DiffLine {
    /// 行の中身。
    pub fn text(&self) -> &str {
        match self {
            Self::Same(text) | Self::Removed(text) | Self::Added(text) => text,
        }
    }

    /// 変更の行か（一部採用で選べる行）。
    pub fn changed(&self) -> bool {
        !matches!(self, Self::Same(_))
    }
}

/// 表の大きさの上限（これを超える組み合わせは「全部消して全部足す」にする）。
const LCS_LIMIT: usize = 4_000_000;

/// 行単位の差分（最長共通部分列。同じ長さなら `before` 側を先に出す＝決定論）。
pub fn diff(before: &str, after: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = if before.is_empty() {
        Vec::new()
    } else {
        before.split('\n').collect()
    };
    let b: Vec<&str> = if after.is_empty() {
        Vec::new()
    } else {
        after.split('\n').collect()
    };
    let (n, m) = (a.len(), b.len());
    if n.saturating_mul(m) > LCS_LIMIT {
        let mut out: Vec<DiffLine> = a
            .iter()
            .map(|line| DiffLine::Removed((*line).to_owned()))
            .collect();
        out.extend(b.iter().map(|line| DiffLine::Added((*line).to_owned())));
        return out;
    }
    // table[i][j] = a[i..] と b[j..] の LCS の長さ
    let width = m + 1;
    let mut table = vec![0u32; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i * width + j] = if a[i] == b[j] {
                table[(i + 1) * width + j + 1] + 1
            } else {
                table[(i + 1) * width + j].max(table[i * width + j + 1])
            };
        }
    }
    let mut out = Vec::with_capacity(n + m);
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(DiffLine::Same(a[i].to_owned()));
            i += 1;
            j += 1;
        } else if table[(i + 1) * width + j] >= table[i * width + j + 1] {
            out.push(DiffLine::Removed(a[i].to_owned()));
            i += 1;
        } else {
            out.push(DiffLine::Added(b[j].to_owned()));
            j += 1;
        }
    }
    out.extend(
        a[i..]
            .iter()
            .map(|line| DiffLine::Removed((*line).to_owned())),
    );
    out.extend(
        b[j..]
            .iter()
            .map(|line| DiffLine::Added((*line).to_owned())),
    );
    out
}

/// 一部採用（§15.7.3「行選択」）: 選んだ変更だけを当てた文字列。`chosen[i]` は `lines[i]` を選んだか
/// （消える行は選べば消え、足す行は選べば入る。変わらない行は常に残る）。
pub fn merge(lines: &[DiffLine], chosen: &[bool]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let pick = chosen.get(index).copied().unwrap_or(false);
        match line {
            DiffLine::Same(text) => out.push(text),
            DiffLine::Removed(text) => {
                if !pick {
                    out.push(text);
                }
            }
            DiffLine::Added(text) => {
                if pick {
                    out.push(text);
                }
            }
        }
    }
    out.join("\n")
}
