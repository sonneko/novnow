//! 開いている文書 1 つ（§3.3 `Document`、§3.4 の `(&Project, &Document, Position)`）。
//!
//! **core は I/O をしない**（§3.2）ので、パスは文字列で持ち、中身は呼び出し側
//! （`adapters/storage_*`）が読んだものを渡す。改行は LF に正規化済みであること（§5.0.4）。

use serde::{Deserialize, Serialize};

use crate::core::text::lexer::TextFormat;

/// 文書の形式（§4.2 のファイル一覧）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentFormat {
    /// `contents/AA-BB.txt`（C 形式、§5.2）。
    Contents,
    /// `plots/all.txt` / `plots/AA.txt`（A 形式、§5.3）。
    PlotOutline,
    /// `plots/AA-BB.txt`（B 形式、§5.4）。
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
    /// `project.json`（§4.5）。
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
        let stem = trimmed
            .rsplit('/')
            .next()
            .unwrap_or(trimmed)
            .trim_end_matches(".md")
            .trim_end_matches(".txt")
            .trim_end_matches(".json");
        match trimmed {
            "project.json" => Self::ProjectFile,
            "characters.md" => Self::Characters,
            "words.md" => Self::Words,
            "ideas.md" => Self::Ideas,
            "materials/index.json" => Self::MaterialsIndex,
            "agents/mcp.json" => Self::McpConfig,
            ".novnow/ui.json" => Self::UiState,
            _ => Self::from_dir(trimmed, stem),
        }
    }

    /// ディレクトリで決まるもの（§4.2）。
    fn from_dir(path: &str, stem: &str) -> Self {
        if let Some(rest) = path.strip_prefix("contents/") {
            return if rest.ends_with(".txt") {
                Self::Contents
            } else {
                Self::Unknown
            };
        }
        if let Some(rest) = path.strip_prefix("plots/") {
            // `plots/all.txt` と `plots/AA.txt` は A 形式、`plots/AA-BB.txt` は B 形式（§5.3 指示 11）
            return if rest.ends_with(".txt") && stem.contains('-') {
                Self::PlotEpisode
            } else if rest.ends_with(".txt") {
                Self::PlotOutline
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

    /// §5.0 の字句形式（`.txt` 3 形式と `*.md`）。JSON やログは `None`。
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
    /// プロジェクトルートからの相対パス（`contents/01-04.txt`）。
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
