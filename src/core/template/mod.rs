//! `novnow new` の雛形（§14.5）と既定プロンプト（§15.5.1、埋め込み既定 §15.5.4）。
//!
//! 中身は**プロジェクトの内容**（サンプルの原稿・辞書・LLM へのプロンプト）であって UI の文言では
//! ない（§12.6「プロジェクトの中身は翻訳しない」）。UI 言語に関わらず日本語で、`project/` の下の
//! ファイルを `include_str!` で埋め込む（`docs/history.md` B-104）。core なので I/O は持たない
//! （書くのは CLI の `novnow new`、M7 以降は GUI の「新規作成」と `prompts/` 欠落時の I050）。
//!
//! 雛形は `novnow check` で **E も W も出ない**ことを `tests/template.rs` が固定している
//! （作家が最初に見る診断がサンプル由来の警告にならないように）。

/// 雛形の 1 ファイル。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateFile {
    /// プロジェクトルートからの相対パス（`/` 区切り。`ProjectPath` にそのまま使える）。
    pub path: &'static str,
    /// 中身（UTF-8・改行は LF・末尾に改行 1 個）。
    pub text: String,
}

/// 既定プロンプト（§15.5.1 の表の順）。`prompts/` が無いときの生成元（§4.2・§15.5.4）。
pub const DEFAULT_PROMPTS: &[(&str, &str)] = &[
    (
        "prompts/_shared/style.md",
        include_str!("project/prompts/_shared/style.md"),
    ),
    (
        "prompts/_shared/output-contract.md",
        include_str!("project/prompts/_shared/output-contract.md"),
    ),
    (
        "prompts/writer.md",
        include_str!("project/prompts/writer.md"),
    ),
    (
        "prompts/plotter.md",
        include_str!("project/prompts/plotter.md"),
    ),
    (
        "prompts/ideator.md",
        include_str!("project/prompts/ideator.md"),
    ),
    (
        "prompts/researcher.md",
        include_str!("project/prompts/researcher.md"),
    ),
    (
        "prompts/stylist.md",
        include_str!("project/prompts/stylist.md"),
    ),
    (
        "prompts/logician.md",
        include_str!("project/prompts/logician.md"),
    ),
    (
        "prompts/integrator.md",
        include_str!("project/prompts/integrator.md"),
    ),
];

/// サンプルの原稿と辞書（§14.5 の表の順。`project.toml` と JSON は [`new_project`] が作る）。
const SAMPLES: &[(&str, &str)] = &[
    ("characters.md", include_str!("project/characters.md")),
    ("words.md", include_str!("project/words.md")),
    ("ideas.md", include_str!("project/ideas.md")),
    (
        "contents/01-01.cnov",
        include_str!("project/contents/01-01.cnov"),
    ),
    ("plots/all.cnov", include_str!("project/plots/all.cnov")),
    ("plots/01.cnov", include_str!("project/plots/01.cnov")),
    ("plots/01-01.cnov", include_str!("project/plots/01-01.cnov")),
];

/// `materials/index.json`（§5.8.2 の空の索引）。
pub const MATERIALS_INDEX: &str =
    "{\n  \"schema_version\": 1,\n  \"next_id\": 1,\n  \"items\": []\n}\n";

/// `agents/mcp.json`（§5.11 の空の設定。§4.2 の「無言で作成」と同じ中身）。
pub const MCP_CONFIG: &str = "{\n  \"schema_version\": 1,\n  \"servers\": {}\n}\n";

/// `.gitignore`（§14.5: `.novnow/` と `exports/` を無視する）。
pub const GITIGNORE: &str = ".novnow/\nexports/\n";

/// 空のまま Git に残すディレクトリの印（§14.5。中身は空）。
pub const GITKEEPS: &[&str] = &[
    "materials/docs/.gitkeep",
    "agents/tasks/.gitkeep",
    "agents/results/.gitkeep",
    "agents/threads/.gitkeep",
    "agents/proposals/.gitkeep",
    "agents/log/.gitkeep",
];

/// `project.toml`（§4.5。`schema_version` と `title` だけ。他は既定値＝書かない）。
pub fn project_toml(title: &str) -> String {
    let escaped = title.replace('\\', "\\\\").replace('"', "\\\"");
    format!("schema_version = 2\ntitle = \"{escaped}\"\n")
}

/// 既定プロンプト（パスで引く）。
pub fn default_prompt(path: &str) -> Option<&'static str> {
    DEFAULT_PROMPTS
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, text)| *text)
}

/// 付録 C の `sample` プロジェクト（Web 版の初期投入 / `examples/sample-novel` の v1 版）。
const SAMPLE_NOVEL: &[(&str, &str)] = &[
    (
        "project.toml",
        include_str!("../../../examples/sample-novel/project.toml"),
    ),
    (
        "characters.md",
        include_str!("../../../examples/sample-novel/characters.md"),
    ),
    (
        "words.md",
        include_str!("../../../examples/sample-novel/words.md"),
    ),
    (
        "ideas.md",
        include_str!("../../../examples/sample-novel/ideas.md"),
    ),
    (
        "contents/01-04.cnov",
        include_str!("../../../examples/sample-novel/contents/01-04.cnov"),
    ),
    (
        "plots/all.cnov",
        include_str!("../../../examples/sample-novel/plots/all.cnov"),
    ),
    (
        "plots/01.cnov",
        include_str!("../../../examples/sample-novel/plots/01.cnov"),
    ),
    (
        "plots/01-04.cnov",
        include_str!("../../../examples/sample-novel/plots/01-04.cnov"),
    ),
    (
        "materials/index.json",
        include_str!("../../../examples/sample-novel/materials/index.json"),
    ),
    (
        "materials/docs/M0001-kyukousha.md",
        include_str!("../../../examples/sample-novel/materials/docs/M0001-kyukousha.md"),
    ),
];

/// §14.5 の生成物一式（表の順）。`title` は `project.toml` の題（`novnow new` はディレクトリ名）。
pub fn new_project(title: &str) -> Vec<TemplateFile> {
    let file = |path: &'static str, text: &str| TemplateFile {
        path,
        text: text.to_owned(),
    };
    let mut files = vec![TemplateFile {
        path: "project.toml",
        text: project_toml(title),
    }];
    files.extend(SAMPLES.iter().map(|(path, text)| file(path, text)));
    files.push(file("materials/index.json", MATERIALS_INDEX));
    let (materials, agents): (Vec<&str>, Vec<&str>) = GITKEEPS
        .iter()
        .partition(|path| path.starts_with("materials/"));
    files.extend(materials.into_iter().map(|path| file(path, "")));
    files.extend(DEFAULT_PROMPTS.iter().map(|(path, text)| file(path, text)));
    files.push(file("agents/mcp.json", MCP_CONFIG));
    files.extend(agents.into_iter().map(|path| file(path, "")));
    files.push(file(".gitignore", GITIGNORE));
    files
}

/// 付録 C の `sample` プロジェクト一式（Web 版の初期投入 §13.3.5 / `examples/sample-novel`）。
pub fn sample_project() -> Vec<TemplateFile> {
    let file = |path: &'static str, text: &str| TemplateFile {
        path,
        text: text.to_owned(),
    };
    let mut files: Vec<TemplateFile> = SAMPLE_NOVEL
        .iter()
        .map(|(path, text)| file(path, text))
        .collect();
    files.extend(DEFAULT_PROMPTS.iter().map(|(path, text)| file(path, text)));
    files.push(file("agents/mcp.json", MCP_CONFIG));
    files.extend(
        GITKEEPS
            .iter()
            .filter(|path| path.starts_with("agents/"))
            .map(|path| file(path, "")),
    );
    files.push(file(".gitignore", GITIGNORE));
    files
}
