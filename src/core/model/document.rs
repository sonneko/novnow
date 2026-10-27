//! 開いている文書 1 つ（§3.3 `Document`、§3.4 の `(&Project, &Document, Position)`）。
//!
//! **core は I/O をしない**（§3.2）ので、パスは文字列で持ち、中身は呼び出し側
//! （`adapters/storage_*`）が読んだものを渡す。改行は LF に正規化済みであること（§5.0.4）。

use serde::{Deserialize, Serialize};

use crate::core::text::lexer::TextFormat;

/// 文書の形式（§4.2 のファイル一覧）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentFormat {
    /// `contents/AA-BB.cnov`（C 形式、§5.2）。
    Contents,
    /// `plots/all.cnov` / `plots/AA.cnov`（A 形式、§5.3）。
    PlotOutline,
    /// `plots/AA-BB.cnov`（B 形式、§5.4）。
    PlotEpisode,
    /// `characters.md`（§5.5）。
    Characters,
    /// `words.md`（§5.6）。
    Words,
    /// `ideas.md`（§5.7）。
    Ideas,
    /// `materials/index.json`（§5.8.2）。
    MaterialsIndex,
    /// `materials/docs/*.md` / `materials/inbox/*.md`（§5.8.3）。
    MaterialDoc,
    /// `prompts/<role>.md`（§5.9.1）。
    Prompt,
    /// `prompts/custom/<role>.md`（§5.9.2）。
    CustomPrompt,
    /// `prompts/_shared/*.md`（§5.9.1 / §15.5.3 の `{{project_style}}`）。
    SharedPrompt,
    /// `agents/tasks/T####.md`（§5.10.1）。
    Task,
    /// `agents/results/T####.md`（§5.10.2）。
    TaskResult,
    /// `agents/proposals/P####.md`（§5.10.3 / §8.7）。
    Proposal,
    /// `agents/threads/TH####.md`（§5.10.4）。
    Thread,
    /// `agents/log/YYYY-MM-DD.jsonl`（§5.10.5）。
    AgentLog,
    /// `agents/mcp.json`（§5.11）。
    McpConfig,
    /// `project.toml`（§4.5）。
    ProjectFile,
    /// `.novnow/ui.json`（§4.4.1）。
    UiState,
    /// どれでもない（`exports/` の出力や `assets/` のバイナリ等）。
    Unknown,
}

impl DocumentFormat {
    /// プロジェクトルートからの相対パスから形式を決める（§4.2）。
    ///
    /// `std::path` は使わない（§3.2 は core での I/O を禁じる。区切りは `/` 固定で読む）。
    pub fn of_path(path: &str) -> Self {
        let trimmed = path.trim_start_matches('/').trim_start_matches("./");
        match trimmed {
            "project.toml" => Self::ProjectFile,
            "characters.md" => Self::Characters,
            "words.md" => Self::Words,
            "ideas.md" => Self::Ideas,
            "materials/index.json" => Self::MaterialsIndex,
            "agents/mcp.json" => Self::McpConfig,
            ".novnow/ui.json" => Self::UiState,
            _ => Self::from_dir(trimmed),
        }
    }

    /// ディレクトリで決まるもの（§4.2）。本文・プロットは §5.0.9 の厳密一致
    /// （置き場所とファイル名の形と `.cnov` の組）でのみ文書として認識する。
    /// 形が合わないものは未知のファイルとして触れない（§4.3-5）。
    fn from_dir(path: &str) -> Self {
        if let Some(rest) = path.strip_prefix("contents/") {
            // `contents/<章>-<話>.cnov` は本文（C 形式、§5.2）
            return if numbered_stem(rest) == Some(2) {
                Self::Contents
            } else {
                Self::Unknown
            };
        }
        if let Some(rest) = path.strip_prefix("plots/") {
            // `plots/all.cnov` と `plots/<章>.cnov` は A 形式、`plots/<章>-<話>.cnov` は B 形式（§5.3・§5.4）
            return if rest == "all.cnov" || numbered_stem(rest) == Some(1) {
                Self::PlotOutline
            } else if numbered_stem(rest) == Some(2) {
                Self::PlotEpisode
            } else {
                Self::Unknown
            };
        }
        if path.starts_with("materials/docs/") || path.starts_with("materials/inbox/") {
            return Self::MaterialDoc;
        }
        if path.starts_with("prompts/custom/") {
            return Self::CustomPrompt;
        }
        if path.starts_with("prompts/_shared/") {
            return Self::SharedPrompt;
        }
        if path.starts_with("prompts/") && path.ends_with(".md") {
            return Self::Prompt;
        }
        if path.starts_with("agents/tasks/") {
            return Self::Task;
        }
        if path.starts_with("agents/results/") {
            return Self::TaskResult;
        }
        if path.starts_with("agents/proposals/") {
            return Self::Proposal;
        }
        if path.starts_with("agents/threads/") {
            return Self::Thread;
        }
        if path.starts_with("agents/log/") {
            return Self::AgentLog;
        }
        Self::Unknown
    }

    /// §5.0 の字句形式（`.cnov` 3 形式と `*.md`）。JSON やログは `None`。
    pub fn text_format(self) -> Option<TextFormat> {
        match self {
            Self::Contents => Some(TextFormat::Contents),
            Self::PlotOutline => Some(TextFormat::PlotOutline),
            Self::PlotEpisode => Some(TextFormat::PlotEpisode),
            Self::Characters
            | Self::Words
            | Self::Ideas
            | Self::MaterialDoc
            | Self::Prompt
            | Self::CustomPrompt
            | Self::SharedPrompt
            | Self::Task
            | Self::TaskResult
            | Self::Proposal
            | Self::Thread => Some(TextFormat::Markdown),
            Self::MaterialsIndex
            | Self::AgentLog
            | Self::McpConfig
            | Self::ProjectFile
            | Self::UiState
            | Self::Unknown => None,
        }
    }
}

/// 開いている文書 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    /// プロジェクトルートからの相対パス（`contents/01-04.cnov`）。
    pub path: String,
    /// 形式。
    pub format: DocumentFormat,
    /// 中身（**LF 正規化済み**、§5.0.4）。
    pub text: String,
}

impl Document {
    /// パスと中身から作る（形式はパスから決める）。
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> Self {
        let path = path.into();
        let format = DocumentFormat::of_path(&path);
        Self {
            path,
            format,
            text: text.into(),
        }
    }
}

/// `.cnov` のファイル名が「数字（`-` 数字）…」の形なら数字の個数を返す（§5.0.9）。
/// サブディレクトリ・数字以外の文字・空の要素は形に合わない。
fn numbered_stem(name: &str) -> Option<usize> {
    let stem = name.strip_suffix(".cnov")?;
    let mut count = 0;
    for part in stem.split('-') {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        count += 1;
    }
    Some(count)
}
