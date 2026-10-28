//! エージェント（§15）の純粋な部品。I/O もスレッドも持たない（§3.2）。
//!
//! | モジュール | 中身 |
//! |---|---|
//! | [`files`] | `agents/` のファイル形式（§5.10: タスク・結果・提案・スレッド・監査ログ）の読み書きと ID の採番 |
//! | [`mcp_config`] | `agents/mcp.json`（§5.11）。定義順を保って読み、`${ENV}` を解決する（W033 / E030） |
//! | [`prompts`] | `prompts/`（§15.5）: 既定の書き出し（U-7・I050）・変数の展開・壊れたプロンプト（E029） |
//! | [`output`] | LLM の出力の読み方（`## question` / `## message` / `## task`）・契約違反・行差分 |
//! | [`apply`] | 提案の適用（採用・自動適用・差し戻し）と、自動適用の前の検証ゲート（B-11: E 系のみ） |
//! | [`runner`] | 委譲レベルの解決・実行の順番（U-8: 並列度・サーバ別上限・同一節の直列化）・自律の上限（A8） |
//! | [`server`] | `novnow mcp`（§15.8.3）のツール。外部のエージェントに core の純粋関数だけを公開する |
//!
//! ContextPacket（§15.4）は `core::aicontext`。

pub mod apply;
pub mod files;
pub mod mcp_config;
pub mod output;
pub mod prompts;
pub mod runner;
pub mod server;

use crate::core::model::delegation::Delegation;
use crate::core::model::project::Agents;
pub use crate::core::syntax::prompt_md::BUILTIN_ROLES;

/// 出力の上限（§15.9「出力 20,000 字で打ち切り、`truncated: true` を記録」）。
pub const OUTPUT_LIMIT_CHARS: usize = 20_000;

/// サーバごとの同時接続の上限（§15.7.5 (2)「同一 MCP サーバへの同時接続は最大 2」）。
pub const PER_SERVER_LIMIT: usize = 2;

/// `agents.max_parallel` の範囲（§15.7.5 (2)「既定 2、範囲 1–8」）。
pub const MAX_PARALLEL_RANGE: (u32, u32) = (1, 8);

/// `9 history` に載せるタスクの数（§15.4.1「同一節に対する直近 3 件」）。
pub const HISTORY_TASKS: usize = 3;

/// 役割の委譲レベル（§15.1.1）。`agents.enabled = false` なら全部 `off`、`integrator` は
/// `agents.integrator = "human"` なら `off`（統合を人間が握る、§4.5.1）。
pub fn level_of(settings: &Agents, role: &str) -> Delegation {
    if !settings.enabled {
        return Delegation::Off;
    }
    if role == "integrator" && settings.integrator_is_human() {
        return Delegation::Off;
    }
    settings.delegation_of(role)
}

/// 既定の役割か（カスタムロールは `agents.custom_roles` と `prompts/custom/`、§5.9.2）。
pub fn is_role(role: &str) -> bool {
    BUILTIN_ROLES.contains(&role)
}

/// 役割の出力が何になるか（§15.2 の「出力」列。§5.10.3 の `kind`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// writer: 節の本文（`contents/`）。
    SectionText,
    /// plotter: 話プロットの節（`plots/AA-BB.cnov`）。
    PlotField,
    /// ideator: `ideas.md` への追記（1 行 1 件）。
    Idea,
    /// researcher: 資料（`materials/inbox/`、`status: proposed`）。
    Material,
    /// stylist / logician / integrator: 指摘・提案の文章。ファイルには適用しない（`docs/history.md` B-141）。
    Note,
}

impl Kind {
    /// 役割の出力の種類。
    pub fn of_role(role: &str) -> Self {
        match role {
            "writer" => Self::SectionText,
            "plotter" => Self::PlotField,
            "ideator" => Self::Idea,
            "researcher" => Self::Material,
            _ => Self::Note,
        }
    }

    /// §5.10.3 の `kind:` の値。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SectionText => "section-text",
            Self::PlotField => "plot-field",
            Self::Idea => "idea",
            Self::Material => "material",
            Self::Note => "note",
        }
    }

    /// `kind:` の値から。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "section-text" | "human" => Some(Self::SectionText),
            "plot-field" => Some(Self::PlotField),
            "idea" => Some(Self::Idea),
            "material" => Some(Self::Material),
            "note" => Some(Self::Note),
            _ => None,
        }
    }

    /// ファイルに適用できるか（`note` だけは読むだけ）。
    pub fn applies(self) -> bool {
        self != Self::Note
    }
}

/// 出力を上限で切る（§15.9）。`(切った後, 切ったか)`。
pub fn cap_output(text: &str) -> (String, bool) {
    match text.char_indices().nth(OUTPUT_LIMIT_CHARS) {
        Some((at, _)) => (text[..at].to_owned(), true),
        None => (text.to_owned(), false),
    }
}

/// 文字数（Unicode のスカラー値の数。予算・`input_chars` / `output_chars` の単位）。
pub fn chars(text: &str) -> usize {
    text.chars().count()
}

/// 1 行に収める（フィールドの値に改行を入れない。§5.10 は 1 行 1 フィールド）。
pub fn one_line(text: &str) -> String {
    text.split(['\n', '\r'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// ファイル 1 つの書き換え（全体）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// プロジェクト相対のパス。
    pub path: String,
    /// 新しい中身（全体）。
    pub text: String,
}

/// 人間に知らせること（文言は UI のカタログ）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// 自動適用した（§15.1.1）。
    AutoApplied {
        /// タスク ID。
        task: String,
        /// 対象（表示用の ID かパス）。
        target: String,
    },
    /// 自動適用が確定済みの節を書き換えた（§15.9「ステータスバーに 1 行通知」MUST）。
    FixedOverwrite {
        /// タスク ID。
        task: String,
        /// 節の ID。
        target: String,
    },
    /// 自動適用が E 系の診断を生むので見送った（§15.9・B-11）。
    GateRefused {
        /// タスク ID。
        task: String,
        /// 生まれる診断のコード（`E001, E013`）。
        codes: String,
    },
    /// 適用先（節・プロットの節）が見つからないので提案のまま残した。
    Missing {
        /// タスク ID。
        task: String,
    },
    /// 出力を 20,000 字で切った（§15.9）。
    Truncated {
        /// タスク ID。
        task: String,
    },
    /// 自律の上限で止めた（**W095**、§15.9）。
    Limit {
        /// 上限の名前（`max_chain_hops` など）。
        limit: String,
    },
    /// 同じ役割・同じ節のタスクが既にある（**E035**、§15.7.5）。
    Duplicate {
        /// 役割。
        role: String,
        /// 節の ID。
        scene: String,
    },
    /// LLM が質問を返した（§15.5.2-6）。
    Questions {
        /// タスク ID。
        task: String,
        /// 件数。
        count: usize,
    },
}

/// 実行・採用・却下の結果として行うこと（書き込みは順に、ログは追記）。UI と `novnow mcp` が実行する。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// 書くファイル（順に。同じパスが 2 度あれば後が勝つ）。
    pub writes: Vec<Change>,
    /// 監査ログ（§5.10.5）。
    pub log: Vec<files::LogEvent>,
    /// 人間に知らせること。
    pub notices: Vec<Notice>,
    /// 続けて走らせるタスク（自律の連鎖で起こしたもの。ID）。
    pub follow: Vec<String>,
}

impl Plan {
    /// 書き込みを足す（同じパスは置き換える）。
    pub fn write(&mut self, path: impl Into<String>, text: impl Into<String>) {
        let path = path.into();
        let text = text.into();
        if let Some(found) = self.writes.iter_mut().find(|change| change.path == path) {
            found.text = text;
        } else {
            self.writes.push(Change { path, text });
        }
    }
}

/// Built-in roles plus explicitly registered, well-formed custom roles, in stable order.
pub fn roles(project: &crate::core::features::Project) -> Vec<String> {
    let mut roles: Vec<String> = BUILTIN_ROLES.iter().map(|r| r.to_string()).collect();
    for role in project.settings().agents.custom_roles.iter().take(16) {
        if crate::core::syntax::prompt_md::is_role_name(role) && !roles.contains(role) {
            roles.push(role.clone());
        }
    }
    roles
}
