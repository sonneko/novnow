//! `prompts/`（§5.9・§15.5）。
//!
//! - **使うのは常に `prompts/` の下のファイルだけ**（§15.5.4 (1) MUST）。埋め込みの既定
//!   （`core::template::DEFAULT_PROMPTS`）は「生成元」で、読み込み元ではない。ここにある関数も、
//!   既定の文面を返すのは「書き出す・並べて見せる・同じか比べる」ためだけ。
//! - 開いたときに無いファイルだけを書き出す（[`missing`]。`prompts/` 自体が無かったときだけ I050）。
//!   あるファイルは**何があっても上書きしない**。
//! - `## system` と `## task` が無いプロンプトは壊れている（**E029**、第 2.16 版。§15.5.4 (2) の表の
//!   「E030」は環境変数の未解決のコードと重なっていたので新しいコードにした。`PLANS.md` §9.2 B-139）。
//!   その役割だけ実行できない。
//! - 変数（§15.5.3）は閉じた集合で、知らない変数は **E031**（構文の段階で出ている）。値が無い変数は空に
//!   なり、ドライランで「未解決」として見える（[`Rendered::unresolved`]）。

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::features::Project;
use crate::core::syntax::characters_md::Line;
use crate::core::syntax::prompt_md::{self, PromptSectionKey, VAR_CLOSE, VAR_OPEN};
use crate::core::template::DEFAULT_PROMPTS;

/// 共有の文体規約。
pub const STYLE_PATH: &str = "prompts/_shared/style.md";
/// 共有の出力契約。
pub const CONTRACT_PATH: &str = "prompts/_shared/output-contract.md";
/// 置き場所。
pub const DIR: &str = "prompts/";

/// 必須のセクション（壊れているかの判定）。
pub const REQUIRED: &[(PromptSectionKey, &str)] = &[
    (PromptSectionKey::System, prompt_md::SECTION_SYSTEM),
    (PromptSectionKey::Task, prompt_md::SECTION_TASK),
];

/// 開いたときに書き出すもの: `(パス, 既定の文面)` と、`prompts/` が丸ごと無かったか（I050）。
pub fn missing(project: &Project) -> (Vec<(&'static str, &'static str)>, bool) {
    let absent = !project
        .documents()
        .any(|document| document.path.starts_with(DIR));
    let out = DEFAULT_PROMPTS
        .iter()
        .filter(|(path, _)| project.document(path).is_none())
        .copied()
        .collect();
    (out, absent)
}

/// `>prompt reset` の対象（`role` は役割名・`style`・`output-contract`・`all`）。`(パス, 既定)`。
pub fn reset_targets(name: &str) -> Vec<(&'static str, &'static str)> {
    DEFAULT_PROMPTS
        .iter()
        .filter(|(path, _)| name == "all" || file_name(path) == name)
        .copied()
        .collect()
}

/// `prompts/writer.md` → `writer`（`_shared/style.md` → `style`）。
pub fn file_name(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name)
}

/// 既定と違うか（`編集済`、§15.5.4 (4)）。ファイルが無ければ `false`（開くと書き出される）。
pub fn edited(project: &Project, path: &str) -> bool {
    let Some(default) = crate::core::template::default_prompt(path) else {
        return false;
    };
    project
        .document(path)
        .is_some_and(|document| document.text != default)
}

/// 必須のセクションが無ければその名前（E029）。
pub fn missing_sections(text: &str) -> Vec<&'static str> {
    let parsed = prompt_md::parse(text);
    REQUIRED
        .iter()
        .filter(|(key, _)| parsed.section(*key).is_none())
        .map(|(_, name)| *name)
        .collect()
}

/// 壊れたプロンプトの診断（E029。位置は 1 行目）。
pub fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let missing = missing_sections(text);
    if missing.is_empty() {
        return Vec::new();
    }
    let first = text.find('\n').unwrap_or(text.len());
    vec![Diagnostic::new(
        DiagnosticCode::E029,
        Some(0..first),
        vec![Arg::text(missing.join(", "))],
    )]
}

/// 実行できない理由（E029 / E031）。ファイルが無いときは `## system` と `## task` の両方が無い E029
/// （埋め込みの既定は読み込み元ではない、§15.5.4 (1)。開いたときに書き出されるので、ふつうは起きない）。
pub fn blockers(project: &Project, role: &str) -> Vec<Diagnostic> {
    let path = crate::core::agents::files::prompt_path(role);
    let Some(document) = project.document(&path) else {
        return diagnostics("");
    };
    let mut out = diagnostics(&document.text);
    out.extend(
        prompt_md::parse(&document.text)
            .diagnostics
            .into_iter()
            .filter(|found| found.code == DiagnosticCode::E031),
    );
    out
}

/// 共有ファイル（`style.md` / `output-contract.md`）の中身: H1 とコメントを除いて前後の空行を落とす。
pub fn shared_body(text: &str) -> String {
    let parsed = prompt_md::parse(text);
    let lines: Vec<&str> = parsed
        .lines
        .iter()
        .enumerate()
        .filter(|(index, line)| {
            !matches!(line, Line::Comment(_))
                && parsed
                    .role
                    .as_ref()
                    .is_none_or(|role| role.heading != *index)
        })
        .map(|(_, line)| line.raw())
        .collect();
    trim_blank(&lines)
}

fn trim_blank(lines: &[&str]) -> String {
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

/// 展開したプロンプト。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rendered {
    /// `## system`（＋ `## refuse`）。
    pub system: String,
    /// `## task`。
    pub task: String,
    /// `## output`（`8 constraints` に入る）。
    pub output: String,
    /// `## examples`。
    pub examples: String,
    /// `model-hint`（`fast | balanced | careful`）。
    pub model_hint: String,
    /// `max-chars`（ContextPacket の上限、§5.9.1）。
    pub max_chars: Option<usize>,
    /// `temperature`。
    pub temperature: Option<f64>,
    /// 値が無かった変数（重複なし、出た順）。
    pub unresolved: Vec<String>,
}

/// セクションの中身（見出し行とコメントを除き、前後の空行を落とす）。
fn section_text(parsed: &prompt_md::Prompt<'_>, key: PromptSectionKey) -> String {
    let Some(found) = parsed.section(key) else {
        return String::new();
    };
    let lines: Vec<&str> = (found.span.start + 1..found.span.end)
        .filter_map(|index| parsed.lines.get(index))
        .filter(|line| !matches!(line, Line::Comment(_)))
        .map(|line| line.raw())
        .collect();
    trim_blank(&lines)
}

/// `{{name}}` を値で置き換える（`value` が `None` なら空にして `unresolved` に積む）。
pub fn expand(
    text: &str,
    value: &dyn Fn(&str) -> Option<String>,
    unresolved: &mut Vec<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(VAR_OPEN) {
        out.push_str(&rest[..start]);
        let after = &rest[start + VAR_OPEN.len()..];
        let Some(end) = after.find(VAR_CLOSE) else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = after[..end].trim();
        match value(name) {
            Some(found) if !found.is_empty() => out.push_str(&found),
            _ => {
                if !unresolved.iter().any(|seen| seen == name) {
                    unresolved.push(name.to_owned());
                }
            }
        }
        rest = &after[end + VAR_CLOSE.len()..];
    }
    out.push_str(rest);
    out
}

/// プロンプトを展開する。
pub fn render(text: &str, value: &dyn Fn(&str) -> Option<String>) -> Rendered {
    let parsed = prompt_md::parse(text);
    let mut unresolved = Vec::new();
    let mut take = |key| expand(&section_text(&parsed, key), value, &mut unresolved);
    let mut system = take(PromptSectionKey::System);
    let refuse = take(PromptSectionKey::Refuse);
    if !refuse.is_empty() {
        system.push_str("\n\n");
        system.push_str(&refuse);
    }
    let task = take(PromptSectionKey::Task);
    let output = take(PromptSectionKey::Output);
    let examples = take(PromptSectionKey::Examples);
    let meta = |key: &str| {
        parsed
            .meta_field(key)
            .map(|found| found.value.trim().to_owned())
            .unwrap_or_default()
    };
    Rendered {
        system,
        task,
        output,
        examples,
        model_hint: meta("model-hint"),
        max_chars: meta("max-chars").parse().ok().filter(|found| *found > 0),
        temperature: meta("temperature").parse().ok(),
        unresolved,
    }
}
