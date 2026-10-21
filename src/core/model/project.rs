//! `project.json`（§4.5 スキーマ v1）。
//!
//! **未知キーは保存時に温存する**（§4.5 MUST）。`#[serde(flatten)] unknown` が受けるので、
//! §4.5.1 が「**v1 本体は読まない**。未知キーとして温存する」と明記している `reader` も
//! ここに入る（② の追加仕様 27）。
//!
//! 既定値は §4.5 の例と §4.5.1 の表の「既定」列をそのまま `Default` にした。
//! `#[serde(default)]`（コンテナ単位）を付けてあるので、キーが欠けた `project.json` でも
//! 既定値で読み込める（§4.3-4「パース失敗を起動失敗にしない」）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::delegation::Delegation;

/// §5.9.2「1 プロジェクトあたり 16 ロール。超過は **E034**」。
pub const CUSTOM_ROLES_MAX: usize = 16;

/// `agents.integrator` の人間（§4.5.1「`roles.integrator = "off"` と同義」）。
pub const INTEGRATOR_HUMAN: &str = "human";

/// `agents.integrator` のエージェント（§15.2）。
pub const INTEGRATOR_AGENT: &str = "agent";

/// 保存時の改行コード（§4.5「`"lf" | "crlf"`。既定 `lf`」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum Newline {
    /// LF。
    #[default]
    #[serde(rename = "lf")]
    Lf,
    /// CRLF。
    #[serde(rename = "crlf")]
    Crlf,
}

impl Newline {
    /// 文字列から引く。表に無ければ `None`（呼び出し側は既定値に落とす）。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "lf" => Some(Self::Lf),
            "crlf" => Some(Self::Crlf),
            _ => None,
        }
    }

    /// `project.json` に書く文字列。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::Crlf => "crlf",
        }
    }

    /// 改行文字。
    pub fn text(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

/// §4.3-2 の `schema_version` の判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaState {
    /// `1`（今の版）。
    Current,
    /// `> 1`（novnow 本体の更新が必要）。
    TooNew {
        /// 書かれていた値。
        found: u32,
    },
}

/// `project.json`（§4.5）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    /// スキーマの版（§4.3-2、既定 `1`）。
    pub schema_version: u32,
    /// 作品名（§4.5「空文字のときは表示上のタイトルにディレクトリ名を使う」）。
    pub title: String,
    /// 著者名。
    pub author: String,
    /// 文字コード（§4.3-3。v1 は UTF-8 固定）。
    pub content_encoding: String,
    /// 保存時の改行（§4.5）。文字列で持つのは未知の値で道連れにしないため（§4.3-4）。
    pub newline: String,
    /// 構造（§4.5 `structure`）。
    pub structure: Structure,
    /// 執筆の目標値（§4.5 `writing`）。
    pub writing: Writing,
    /// 辞書の設定（§4.5 `lexicon`）。
    pub lexicon: Lexicon,
    /// エクスポート（§4.5 `export`、§17）。
    pub export: Export,
    /// AI（§4.5 `agents`、§15）。
    pub agents: Agents,
    /// §4.5 に無いキー（`reader` を含む）。**削除してはならない**（縫い代 1）。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: 1,
            title: String::new(),
            author: String::new(),
            content_encoding: "utf-8".to_owned(),
            newline: Newline::Lf.as_str().to_owned(),
            structure: Structure::default(),
            writing: Writing::default(),
            lexicon: Lexicon::default(),
            export: Export::default(),
            agents: Agents::default(),
            unknown: BTreeMap::new(),
        }
    }
}

impl Project {
    /// §4.3-2 の `schema_version` 検査。
    pub fn schema_state(&self) -> SchemaState {
        match self.schema_version {
            0 | 1 => SchemaState::Current,
            found => SchemaState::TooNew { found },
        }
    }

    /// 保存時の改行（§4.5）。読めなければ既定の LF。
    pub fn newline_value(&self) -> Newline {
        Newline::parse(&self.newline).unwrap_or_default()
    }
}

/// `structure`（§4.5）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Structure {
    /// SceneId の形（§5.1）。
    pub scene_id_format: String,
    /// 章の桁数。
    pub chapter_digits: u8,
    /// 話の桁数。
    pub episode_digits: u8,
    /// 場面の桁数。
    pub scene_digits: u8,
    /// 節の桁数。
    pub section_digits: u8,
    /// `00` 章（ストック）を許すか（§5.1.2・追加仕様 22）。
    ///
    /// `false` のとき 0 章を指す見出しは **E022**（`syntax` の `Options::allow_chapter_zero`）。
    pub allow_chapter_zero: bool,
    /// §4.5 に無いキー。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for Structure {
    fn default() -> Self {
        Self {
            scene_id_format: "A-B-C-D".to_owned(),
            chapter_digits: 2,
            episode_digits: 2,
            scene_digits: 2,
            section_digits: 2,
            allow_chapter_zero: true,
            unknown: BTreeMap::new(),
        }
    }
}

/// `writing`（§4.5 / §4.5.1）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Writing {
    /// 1 節の目標字数（§9.4.2）。
    pub section_target_chars: u32,
    /// 1 節の警告字数。
    pub section_warn_chars: u32,
    /// 1 話の目標字数。
    pub episode_target_chars: u32,
    /// 原稿用紙 1 枚の字数（§17.1）。
    pub manuscript_page_chars: u32,
    /// 同一話者の台詞の連続上限（§4.5.1 範囲 1–9、**W082**）。
    pub max_consecutive_speeches: u32,
    /// 台詞比率の上限目安（§4.5.1 範囲 0.0–1.0、§9.4.2）。
    pub dialogue_ratio_target: f64,
    /// 長い文の字数（§4.5.1 範囲 40–400、§9.4.3・**W089**）。
    pub long_sentence_chars: u32,
    /// 連続助詞の閾値（§4.5.1 範囲 2–10、§9.4.3）。
    pub particle_run_threshold: u32,
    /// §4.5 に無いキー。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for Writing {
    fn default() -> Self {
        Self {
            section_target_chars: 400,
            section_warn_chars: 1200,
            episode_target_chars: 6000,
            manuscript_page_chars: 400,
            max_consecutive_speeches: 3,
            dialogue_ratio_target: 0.3,
            long_sentence_chars: 120,
            particle_run_threshold: 4,
            unknown: BTreeMap::new(),
        }
    }
}

/// `lexicon`（§4.5）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Lexicon {
    /// 語クラスの定義（§6.5.1 `(体言)` 等）。**未指定時の既定クラスは §6.5.1 の表**で、
    /// マッチャ（M3 `resolve/role_language.rs`）が持つ。ここは書かれたものだけ。
    pub word_classes: BTreeMap<String, String>,
    /// §4.5 に無いキー。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Lexicon {
    /// 正規表現として**不正な**語クラスの名前（キーの昇順）。
    ///
    /// §6.5.1（第 2.9 版）: 不正かどうかは値を**単独で**コンパイルできるかで決める。
    /// 報告は `project.json` 側の **E027**（`syntax::project_json`、`PLANS.md` §9.1 A-24）で、
    /// マッチャ（`resolve::role_language::Matcher::new`）はそのクラスを使うパターンだけを捨てる。
    pub fn invalid_word_classes(&self) -> Vec<&str> {
        self.word_classes
            .iter()
            .filter(|(_, pattern)| regex::Regex::new(pattern).is_err())
            .map(|(name, _)| name.as_str())
            .collect()
    }
}

/// `export`（§4.5、§17）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Export {
    /// 段落の区切り（§17.1）。
    pub paragraph_break: String,
    /// 行頭字下げをするか。
    pub indent_first_line: bool,
    /// ルビを残すか（§17.1）。
    pub keep_ruby: bool,
    /// 見出しを挿入するか。
    pub insert_headings: bool,
    /// 章あらすじを挿入するか（§4.5.1、§17.2）。
    pub include_blurb: bool,
    /// エクスポートの改行（§4.5 の例は `crlf`）。
    pub newline: Newline,
    /// §4.5 に無いキー。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for Export {
    fn default() -> Self {
        Self {
            paragraph_break: "blank-line".to_owned(),
            indent_first_line: false,
            keep_ruby: true,
            insert_headings: false,
            include_blurb: false,
            newline: Newline::Crlf,
            unknown: BTreeMap::new(),
        }
    }
}

/// `agents`（§4.5 / §4.5.1 / §15）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Agents {
    /// AI を使うか（§4.5.1「全く任せない」= `delegation: "off"` または `enabled: false`）。
    pub enabled: bool,
    /// 全役割の既定の委譲レベル（§15.1.1）。**書かれたままの文字列**で持つ。
    pub delegation: String,
    /// 役割ごとの上書き（§4.5.1 `agents.roles`）。値は書かれたままの文字列。
    pub roles: BTreeMap<String, String>,
    /// 自律の上限（§15.1.1 A8）。
    pub limits: AgentLimits,
    /// ContextPacket の上限字数（§15.4）。
    pub max_chars_per_request: u32,
    /// 統合役を人間が担うか（§4.5.1 / §15.2）。書かれたままの文字列。
    pub integrator: String,
    /// 並列度（§4.5.1 範囲 1–8、§15.7.5）。
    pub max_parallel: u32,
    /// 登録されたカスタムロール（§5.9.2 / §15.2.1）。
    ///
    /// **ファイルがあっても未登録なら UI に出ない**（未登録は **I056**、M10）。
    pub custom_roles: Vec<String>,
    /// ローカル LLM（§4.5.1 / §15.8.4）。
    pub local_llm: LocalLlm,
    /// §4.5 に無いキー。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for Agents {
    fn default() -> Self {
        Self {
            enabled: true,
            delegation: Delegation::Propose.as_str().to_owned(),
            roles: BTreeMap::new(),
            limits: AgentLimits::default(),
            max_chars_per_request: 12000,
            integrator: INTEGRATOR_HUMAN.to_owned(),
            max_parallel: 2,
            custom_roles: Vec::new(),
            local_llm: LocalLlm::default(),
            unknown: BTreeMap::new(),
        }
    }
}

impl Agents {
    /// 全体の既定の委譲レベル（§15.1.1）。値が壊れていれば既定の `propose`。
    pub fn delegation_level(&self) -> Delegation {
        Delegation::parse(&self.delegation).unwrap_or_default()
    }

    /// 役割の委譲レベル（§15.1.1「`agents.roles.<role>` が上書き」）。
    ///
    /// 上書きが無ければ全体の既定。上書きの値が壊れていても全体の既定に落とす。
    pub fn delegation_of(&self, role: &str) -> Delegation {
        match self.roles.get(role) {
            Some(text) => Delegation::parse(text).unwrap_or_else(|| self.delegation_level()),
            None => self.delegation_level(),
        }
    }

    /// 統合役が人間か（§4.5.1「`"human"` は `roles.integrator = "off"` と同義」）。
    pub fn integrator_is_human(&self) -> bool {
        self.integrator == INTEGRATOR_HUMAN
    }

    /// カスタムロールが上限を超えているか（§5.9.2 **E034**）。
    pub fn exceeds_custom_roles(&self) -> bool {
        self.custom_roles.len() > CUSTOM_ROLES_MAX
    }
}

/// `agents.limits`（§4.5.1。「超過したら自律タスクは自動停止し **W095**」、A8）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentLimits {
    /// 1 セッションに起動できるタスク数。
    pub max_tasks_per_session: u32,
    /// エージェント間連鎖のホップ数上限（§15.6）。
    pub max_chain_hops: u32,
    /// 1 セッションの入力文字数上限（コストの暴走止め）。
    pub max_input_chars_per_session: u64,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            max_tasks_per_session: 100,
            max_chain_hops: 10,
            max_input_chars_per_session: 2_000_000,
        }
    }
}

/// `agents.local_llm`（§4.5.1 / §15.8.4）。既定は無効。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalLlm {
    /// 使うか。
    pub enabled: bool,
    /// URL。
    pub url: String,
    /// モデル名。
    pub model: String,
    /// §4.5 に無いキー。
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for LocalLlm {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "http://localhost:11434".to_owned(),
            model: String::new(),
            unknown: BTreeMap::new(),
        }
    }
}

// ============================================================ §4.3 プロジェクト妥当性の判定

/// §4.2 の必須ファイル（表の順）。
pub const REQUIRED_FILES: &[&str] = &[
    "project.json",
    "characters.md",
    "words.md",
    "materials/index.json",
    "plots/all.txt",
];

/// §4.2 の必須ディレクトリ（表の順）。
pub const REQUIRED_DIRS: &[&str] = &["contents", "plots"];

/// プロジェクト構造の解説ページ（§14.2.1、指示 2）。URL は**この定数だけ**を参照する（MUST。
/// `tests/cli.rs` がソース中の出現を 1 か所に固定している）。
pub const PROJECT_STRUCTURE_DOC_URL: &str =
    "https://sonneko.github.io/novnow/guide/project_structure.html";

pub const GIT_GUIDE_DOC_URL: &str = "https://sonneko.github.io/novnow/guide/git.html";
pub const CICD_GUIDE_DOC_URL: &str = "https://sonneko.github.io/novnow/guide/publish.html";

/// `project.json`（§4.1）。
pub const PROJECT_FILE: &str = "project.json";

/// §4.3 の判定に要るもの。**集めるのはストレージ層**（adapters。core は I/O を持たない、§3.2）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Listing {
    /// 存在するファイル（ルートからの相対・`/` 区切り）。
    pub files: BTreeSet<String>,
    /// 存在するディレクトリ（同。末尾の `/` は付けない）。
    pub dirs: BTreeSet<String>,
    /// ディレクトリを持たないストレージか（Web の IndexedDB、§13.3）。`true` なら必須ディレクトリは
    /// 常にあるものとして扱う（§4.3 第 2.9 版）。
    pub dirs_implicit: bool,
    /// `project.json` の中身（読めたとき）。
    pub project_json: Option<String>,
    /// UTF-8 として不正な `.md` / `.txt` / `.json`（§4.3-3）。調べていなければ空。
    pub invalid_utf8: Vec<String>,
}

/// §4.3 の判定の失敗（起動失敗にするもの。構文エラーは含めない、§4.3-4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    /// 必須パスが欠けている（§4.2 の表の順に**全部**、§14.2-2）。ディレクトリは末尾に `/` を付ける。
    Incomplete {
        /// 欠けているもの。
        missing: Vec<PathBuf>,
    },
    /// `schema_version` が新しすぎる（novnow 本体の更新が必要）。
    TooNew {
        /// 書かれていた値。
        found: u32,
    },
    /// UTF-8 として不正なファイルがある（§4.3-3）。
    Encoding {
        /// 該当ファイル（ルートからの相対）。
        files: Vec<PathBuf>,
    },
}

/// §4.3 のプロジェクト妥当性の判定（`core::model::project::validate`）。
///
/// 順は §4.3 のとおり: (1) 必須パス → (2) `schema_version` → (3) 文字コード。
/// 最初に失敗した段の理由を返す。(2) は `project.json` が JSON として読めない・`schema_version` が
/// 数でないときは飛ばす（版が読めないことは起動失敗にしない。E025 を出して既定値で開く、§4.3-4）。
pub fn validate(listing: &Listing) -> Result<(), ProjectError> {
    let mut missing: Vec<PathBuf> = REQUIRED_FILES
        .iter()
        .filter(|file| !listing.files.contains(**file))
        .map(PathBuf::from)
        .collect();
    if !listing.dirs_implicit {
        missing.extend(
            REQUIRED_DIRS
                .iter()
                .filter(|dir| !listing.dirs.contains(**dir))
                .map(|dir| PathBuf::from(format!("{dir}/"))),
        );
    }
    if !missing.is_empty() {
        return Err(ProjectError::Incomplete { missing });
    }
    if let Some(SchemaState::TooNew { found }) = listing.project_json.as_deref().and_then(schema_of)
    {
        return Err(ProjectError::TooNew { found });
    }
    if !listing.invalid_utf8.is_empty() {
        return Err(ProjectError::Encoding {
            files: listing.invalid_utf8.iter().map(PathBuf::from).collect(),
        });
    }
    Ok(())
}

/// `project.json` の `schema_version` だけを読む（§4.3-2）。
///
/// `schema_version` が省略されているときは [`SchemaState::Current`]。
/// JSON として読めない・オブジェクトでない・`schema_version` が 0 以上の整数でないときは `None`
/// （§4.3 第 2.9 版: 2. を飛ばす）。設定全体の読み込み（型の合わない値で E025）とは別に、
/// 版の数だけを見る。
pub fn schema_of(text: &str) -> Option<SchemaState> {
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: Value = serde_json::from_str(body).ok()?;
    let object = value.as_object()?;
    let Some(found) = object.get("schema_version") else {
        return Some(SchemaState::Current);
    };
    Some(match found.as_u64()? {
        0 | 1 => SchemaState::Current,
        other => SchemaState::TooNew {
            found: u32::try_from(other).unwrap_or(u32::MAX),
        },
    })
}
