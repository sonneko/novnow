//! ContextPacket の構築（§15.4）。**LLM に何を見せるかは novnow が決め、人間が全文を確認できる**（A4）。
//!
//! 送る本文は Markdown で、ヘッダ（`scene` / `role` / `delegation` / `generated_at`）の後に 9 ブロックを
//! **固定の順と見出し**で並べる（中身が無いブロックも見出しは残す。§15.4.1「LLM 側のパース安定性」）。
//! 中身は全部プロジェクトの文書から作る（同じ文書なら同じパケット＝決定論。`generated_at` だけは
//! 呼び出し側が渡す。core は時計を持たない）。
//!
//! | ブロック | 中身（`docs/history.md` B-140） |
//! |---|---|
//! | 1 project | `title` と `style`（`prompts/_shared/style.md` の本文） |
//! | 2 outline | `all`（当該章の summary 1 行）・`chapter`（当該話の summary 1 行）・`goal` |
//! | 3 plot | `scene`（当該場面の B 形式の全文）・`section`（当該節の行。`check:` を含む） |
//! | 4 target | `current`（当該節の本文。空なら `(未執筆)`）・`before`（直前 2 節）・`after`（直後 1 節） |
//! | 5 cast | 節の本文と場面のプロットに出るキャラの、**有効モードだけ**の性格・感情・役割・役割語（§15.4.4） |
//! | 6 lexicon | 節の本文と節のプロットに出る用語（出現回数の多い順）と定義の 1 行目 |
//! | 7 materials | 明示参照された資料（本文と話プロットの `material:`）の id / title / summary / 本文。ideator は既存のアイデアの一覧も（付録 A.4） |
//! | 8 constraints | `prompts/<role>.md` の `## output`・共有の出力契約・機械的な制約（字数・記法・確定済み） |
//! | 9 history | 同じ節の直近 3 タスクの採否と人間のコメント・届いた伝言。integrator は直近のタスクの状態 |
//!
//! 文字数の予算と切り詰めは [`super::budget`]。

use std::collections::BTreeMap;

use crate::core::agents::files::{self, Board};
use crate::core::agents::prompts::{self, Rendered};
use crate::core::aicontext::budget::{self, Block, Part, Style};
use crate::core::export::State;
use crate::core::features::project::line_starts_of;
use crate::core::features::reference::{self, Cast};
use crate::core::features::{ideas, sections, Project};
use crate::core::id::{PartialSceneId, RangeExpr, SceneId};
use crate::core::model::delegation::Delegation;
use crate::core::model::document::Document;
use crate::core::resolve::context_rules::PacketBlock;
use crate::core::resolve::dictionary::Target;
use crate::core::text::lexer::FieldKey;

/// 当該節が空のときの `current`（§15.4.1 の例）。
/// i18n: format-vocabulary
pub const UNWRITTEN: &str = "(未執筆)";

/// パケットの題（§15.4.1）。
pub const TITLE: &str = "# novnow context";

/// integrator の `9 history` に載せるタスクの数。
pub const INTEGRATOR_HISTORY: usize = 10;

/// 組み立ての入力。
#[derive(Debug, Clone, Copy)]
pub struct Inputs<'a> {
    /// プロジェクト。
    pub project: &'a Project,
    /// 役割。
    pub role: &'a str,
    /// 対象の節（4 階層）。
    pub scene: SceneId,
    /// その役割の委譲レベル。
    pub delegation: Delegation,
    /// 生成日時（RFC 3339）。
    pub generated_at: &'a str,
    /// `project.toml` の `title` が空のときの題（プロジェクトの名前）。
    pub title: &'a str,
    /// 人間が外したブロック。
    pub excluded: &'a [PacketBlock],
    /// このパケットのタスク（自分の履歴に自分を入れない）。
    pub task: Option<&'a str>,
    /// 人間のコメント（`{{user_comment}}`）。
    pub comment: &'a str,
    /// Reference examples requested by the task.
    pub examples: &'a [String],
}

/// 送るもの一式。
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    /// `scene:`。
    pub scene: String,
    /// `role:`。
    pub role: String,
    /// `delegation:`。
    pub delegation: Delegation,
    /// `generated_at:`。
    pub generated_at: String,
    /// 9 ブロック（固定順）。
    pub blocks: Vec<Block>,
    /// 送れる上限。
    pub cap: usize,
    /// 展開したプロンプト。
    pub prompt: Rendered,
    /// `prompts/<role>.md` があったか。
    pub prompt_found: bool,
    /// Selected reference examples, capped separately from the fixed nine blocks.
    pub examples: String,
    /// 当該節が確定済み（`state: fixed`）か。
    pub fixed: bool,
    /// 本文の目標字数（writer の字数の制約）。
    pub target_chars: Option<usize>,
}

/// LLM への要求（MCP の `sampling/createMessage` の中身の元）。
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// `systemPrompt`。
    pub system: String,
    /// 利用者のメッセージ（`## task` → パケット → `## examples`）。
    pub user: String,
    /// `temperature`。
    pub temperature: Option<f64>,
    /// `model-hint`。
    pub model_hint: String,
}

impl Packet {
    /// 送る本文（Markdown）。
    pub fn text(&self) -> String {
        let mut out = format!(
            "{TITLE}\nscene: {}\nrole: {}\ndelegation: {}\ngenerated_at: {}\n",
            self.scene,
            self.role,
            self.delegation.as_str(),
            self.generated_at
        );
        for block in &self.blocks {
            out.push('\n');
            out.push_str(block.kind.heading());
            out.push('\n');
            let body = block.body();
            if !body.is_empty() {
                out.push_str(&body);
                out.push('\n');
            }
        }
        if !self.examples.is_empty() {
            out.push_str("\n## examples\n");
            out.push_str(&self.examples);
            out.push('\n');
        }
        out
    }

    /// `context-hash`（§5.10.1）: `generated_at` の行を除いた本文の SHA-256（`sha256:` + 16 進）。
    pub fn hash(&self) -> String {
        hash_of(&self.text())
    }

    /// 9 ブロックの合計の文字数。
    pub fn size(&self) -> usize {
        self.blocks.iter().map(Block::size).sum::<usize>() + self.examples.chars().count()
    }

    /// 上限を超えたまま（送れない、§15.9）。
    pub fn over(&self) -> bool {
        self.size() > self.cap
    }

    /// ブロック。
    pub fn block(&self, kind: PacketBlock) -> Option<&Block> {
        self.blocks.iter().find(|block| block.kind == kind)
    }

    /// LLM への要求。
    pub fn request(&self) -> Request {
        let mut user = String::new();
        if !self.prompt.task.is_empty() {
            user.push_str(&self.prompt.task);
            user.push_str("\n\n");
        }
        user.push_str(&self.text());
        if !self.prompt.examples.is_empty() {
            user.push_str("\n## examples\n");
            user.push_str(&self.prompt.examples);
            user.push('\n');
        }
        Request {
            system: self.prompt.system.clone(),
            user,
            temperature: self.prompt.temperature,
            model_hint: self.prompt.model_hint.clone(),
        }
    }
}

/// パケットの本文から `context-hash` を作る（`generated_at:` の行は除く）。
pub fn hash_of(text: &str) -> String {
    let stable: Vec<&str> = text
        .split('\n')
        .filter(|line| !line.starts_with("generated_at:"))
        .collect();
    format!(
        "sha256:{}",
        crate::core::text::sha256::hex(stable.join("\n").as_bytes())
    )
}

// ============================================================ 材料

#[derive(Default)]
struct Sources {
    title: String,
    genre: String,
    style: String,
    outline_all: String,
    outline_chapter: String,
    goal: String,
    plot_scene: String,
    plot_section: String,
    check: String,
    current: String,
    before: Vec<(String, String)>,
    after: Option<(String, String)>,
    cast: Vec<Cast>,
    lexicon: Vec<(String, String)>,
    materials: Vec<MaterialSource>,
    ideas: Vec<String>,
    history: Vec<String>,
    contract: String,
    target_chars: Option<usize>,
    fixed: bool,
}

struct MaterialSource {
    head: String,
    id: String,
    body: String,
}

/// パケットを組み立てる。
pub fn build(inputs: &Inputs<'_>) -> Packet {
    build_with_examples(inputs, example_text(inputs.project, inputs.examples), true)
}

fn build_with_examples(inputs: &Inputs<'_>, examples: String, retry: bool) -> Packet {
    let project = inputs.project;
    let digits = project.digits();
    let sources = gather(inputs);
    let prompt_path = files::prompt_path(inputs.role);
    let prompt_text = project
        .document(&prompt_path)
        .map(|found| found.text.clone());
    let variables = variables(&sources, inputs, digits, &examples);
    let lookup = |name: &str| variables.get(name).cloned();
    let prompt = prompt_text
        .as_deref()
        .map(|text| prompts::render(text, &lookup))
        .unwrap_or_default();
    let mut unresolved = prompt.unresolved.clone();
    let contract = prompts::expand(&sources.contract, &lookup, &mut unresolved);
    let budgets = budget::budgets(inputs.role, prompt.max_chars);
    let cap = budget::cap(
        inputs.role,
        prompt.max_chars,
        project.settings().agents.max_chars_per_request as usize,
    );
    let mut blocks: Vec<Block> = PacketBlock::ALL
        .iter()
        .zip(budgets)
        .map(|(kind, budget)| Block {
            kind: *kind,
            budget,
            excluded: inputs.excluded.contains(kind) && !kind.is_mandatory(),
            parts: parts_of(*kind, &sources, &prompt, &contract, inputs),
        })
        .collect();
    if !crate::core::agents::is_role(inputs.role) {
        if let Some(text) = prompt_text.as_deref() {
            let parsed = crate::core::syntax::prompt_md::parse(text);
            if let Some(field) = parsed
                .fields
                .iter()
                .filter_map(|n| parsed.field(*n))
                .find(|field| field.key == crate::core::text::lexer::FieldKey::Context)
            {
                let allowed: Vec<&str> = field
                    .value
                    .split([',', ' '])
                    .filter(|s| !s.is_empty())
                    .collect();
                for block in &mut blocks {
                    if !block.kind.is_mandatory()
                        && !allowed.iter().any(|s| *s == block.kind.name())
                    {
                        block.excluded = true;
                    }
                }
            }
        }
    }
    for block in &mut blocks {
        block.fit();
    }
    budget::fit_total(&mut blocks, cap.saturating_sub(examples.chars().count()));
    // Examples cannot silently push a valid packet past its request limit.
    let remaining = cap.saturating_sub(blocks.iter().map(Block::size).sum());
    if retry && examples.chars().count() > remaining {
        // Re-render {{examples}} as well as the packet after clipping: the
        // preview, hash, and actual LLM request must agree byte for byte.
        return build_with_examples(inputs, examples.chars().take(remaining).collect(), false);
    }
    let examples: String = examples.chars().take(remaining).collect();
    Packet {
        scene: inputs.scene.display(digits),
        role: inputs.role.to_owned(),
        delegation: inputs.delegation,
        generated_at: inputs.generated_at.to_owned(),
        blocks,
        cap,
        prompt: Rendered {
            unresolved,
            ..prompt
        },
        prompt_found: prompt_text.is_some(),
        examples,
        fixed: sources.fixed,
        target_chars: sources.target_chars,
    }
}

/// Expand a task's selected reference ranges into a bounded, deterministic sample.
/// Invalid expressions and missing files yield no sample instead of leaking unrelated content.
fn example_text(project: &Project, specs: &[String]) -> String {
    let mut output = String::new();
    let digits = project.digits();
    for spec in specs {
        let words: Vec<&str> = spec.split_whitespace().collect();
        let (range_text, kind) = match words.last().copied() {
            Some("contents" | "plot" | "both") => {
                (words[..words.len() - 1].join(" "), words[words.len() - 1])
            }
            _ => (spec.clone(), "both"),
        };
        let Ok(range) = RangeExpr::parse(&range_text) else {
            continue;
        };
        if kind != "plot" {
            for section in sections::all(project) {
                if !range.contains(section.id) {
                    continue;
                }
                let body = sections::text_of(project.text(&section.path), &section);
                output.push_str(&format!(
                    "\n### @{} contents\n{}\n",
                    section.id.display(digits),
                    body
                ));
                if output.chars().count() >= 8000 {
                    break;
                }
            }
        }
        if kind != "contents" && output.chars().count() < 8000 {
            // A-format outlines are the plot examples for chapter/episode
            // ranges. Narrower scene/section ranges use B-format below.
            if let Some(outline) = project.all_plot() {
                for block in &outline.blocks {
                    if let Some(id) = block.id.filter(|id| range.contains(*id)) {
                        let text = block
                            .span
                            .clone()
                            .filter_map(|line| outline.lines.get(line))
                            .map(|line| line.raw())
                            .collect::<Vec<_>>()
                            .join("\n");
                        output.push_str(&format!("\n### @{} plot\n{}\n", id.display(digits), text));
                        if output.chars().count() >= 8000 {
                            break;
                        }
                    }
                }
            }
            let chapters: std::collections::BTreeSet<_> = project
                .documents()
                .filter_map(|doc| {
                    doc.path
                        .strip_prefix("plots/")?
                        .strip_suffix(".cnov")?
                        .parse::<u16>()
                        .ok()
                })
                .collect();
            for chapter in chapters {
                if let Some(outline) = project.chapter_plot(chapter) {
                    for block in &outline.blocks {
                        if let Some(id) = block.id.filter(|id| range.contains(*id)) {
                            let text = block
                                .span
                                .clone()
                                .filter_map(|line| outline.lines.get(line))
                                .map(|line| line.raw())
                                .collect::<Vec<_>>()
                                .join("\n");
                            output.push_str(&format!(
                                "\n### @{} plot\n{}\n",
                                id.display(digits),
                                text
                            ));
                            if output.chars().count() >= 8000 {
                                break;
                            }
                        }
                    }
                }
                if output.chars().count() >= 8000 {
                    break;
                }
            }
            let episodes: std::collections::BTreeSet<_> = project
                .documents()
                .filter_map(|document| {
                    let stem = document
                        .path
                        .strip_prefix("plots/")?
                        .strip_suffix(".cnov")?;
                    let (chapter, episode) = stem.split_once('-')?;
                    Some((chapter.parse::<u16>().ok()?, episode.parse::<u16>().ok()?))
                })
                .collect();
            for (chapter, episode) in episodes {
                if let Some(plot) = project.episode_plot(chapter, episode) {
                    for scene in &plot.scenes {
                        let Some(id) = scene.id else { continue };
                        let matching_sections: Vec<_> = scene
                            .sections
                            .iter()
                            .filter(|section| {
                                range.contains(SceneId {
                                    section: section.number,
                                    ..id
                                })
                            })
                            .collect();
                        if range.contains(id) && matching_sections.is_empty() {
                            let text = scene
                                .span
                                .clone()
                                .filter_map(|line| plot.lines.get(line))
                                .map(|line| line.raw())
                                .collect::<Vec<_>>()
                                .join("\n");
                            output.push_str(&format!(
                                "\n### @{} plot\n{}\n",
                                id.display(digits),
                                text
                            ));
                        } else {
                            for section in matching_sections {
                                let section_id = SceneId {
                                    section: section.number,
                                    ..id
                                };
                                let text = section
                                    .span
                                    .clone()
                                    .filter_map(|line| plot.lines.get(line))
                                    .map(|line| line.raw())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                output.push_str(&format!(
                                    "\n### @{} plot\n{}\n",
                                    section_id.display(digits),
                                    text
                                ));
                            }
                        }
                        if output.chars().count() >= 8000 {
                            break;
                        }
                    }
                }
                if output.chars().count() >= 8000 {
                    break;
                }
            }
        }
        if output.chars().count() >= 8000 {
            break;
        }
    }
    output.chars().take(8000).collect()
}

fn variables(
    sources: &Sources,
    inputs: &Inputs<'_>,
    digits: crate::core::id::Digits,
    examples: &str,
) -> BTreeMap<&'static str, String> {
    let mut out = BTreeMap::new();
    let mut put = |name: &'static str, value: String| {
        out.insert(name, value);
    };
    put("scene_id", inputs.scene.display(digits));
    put("project_title", sources.title.clone());
    put("project_style", sources.style.clone());
    put("outline_all", sources.outline_all.clone());
    let mut chapter = sources.outline_chapter.clone();
    if !sources.goal.is_empty() {
        if !chapter.is_empty() {
            chapter.push('\n');
        }
        chapter.push_str("goal: ");
        chapter.push_str(&sources.goal);
    }
    put("outline_chapter", chapter);
    put("plot_scene", sources.plot_scene.clone());
    put("plot_section", sources.plot_section.clone());
    put("check", sources.check.clone());
    put("text_current", sources.current.clone());
    put(
        "text_before",
        sources
            .before
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n"),
    );
    put(
        "text_after",
        sources
            .after
            .as_ref()
            .map(|(_, text)| text.clone())
            .unwrap_or_default(),
    );
    put(
        "cast",
        sources
            .cast
            .iter()
            .map(cast_entry)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    put(
        "role_language",
        sources
            .cast
            .iter()
            .filter(|found| !found.tone.is_empty())
            .map(|found| format!("{}: {}", found.name, tones(found)))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    put(
        "lexicon",
        sources
            .lexicon
            .iter()
            .map(|(term, definition)| lexicon_entry(term, definition))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    put(
        "materials",
        sources
            .materials
            .iter()
            .map(|found| found.head.clone())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    put(
        "target_chars",
        sources
            .target_chars
            .map(|found| found.to_string())
            .unwrap_or_default(),
    );
    put("history", sources.history.join("\n"));
    put("delegation", inputs.delegation.as_str().to_owned());
    put("user_comment", inputs.comment.trim().to_owned());
    put("examples", examples.to_owned());
    out
}

fn gather(inputs: &Inputs<'_>) -> Sources {
    let project = inputs.project;
    let scene = inputs.scene;
    let digits = project.digits();
    let mut out = Sources {
        title: match project.settings().title.trim() {
            "" => inputs.title.trim().to_owned(),
            title => title.to_owned(),
        },
        genre: project
            .settings()
            .unknown
            .get("genre")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_owned())
            .unwrap_or_default(),
        style: project
            .document(prompts::STYLE_PATH)
            .map(|found| prompts::shared_body(&found.text))
            .unwrap_or_default(),
        contract: project
            .document(prompts::CONTRACT_PATH)
            .map(|found| prompts::shared_body(&found.text))
            .unwrap_or_default(),
        fixed: crate::core::export::effective_state(project, scene) == State::Fixed,
        ..Sources::default()
    };
    outline(project, scene, &mut out);
    let plot_ranges = plot(project, scene, &mut out);
    // 4 target: 節の並び（ファイル名の順 → ファイルの中の順）で前後を決める
    let list = sections::all(project);
    let contents_path = project.contents_path(scene.chapter, scene.episode);
    let here = list
        .iter()
        .position(|found| found.path == contents_path && found.id == scene);
    let text_of = |index: usize| {
        let found = &list[index];
        project
            .document(&found.path)
            .map(|document| sections::text_of(&document.text, found))
            .unwrap_or_default()
    };
    let (before, after): (Vec<usize>, Option<usize>) = match here {
        Some(index) => (
            (index.saturating_sub(2)..index).collect(),
            (index + 1 < list.len()).then_some(index + 1),
        ),
        None => {
            let earlier: Vec<usize> = (0..list.len()).filter(|&i| list[i].id < scene).collect();
            let start = earlier.len().saturating_sub(2);
            (
                earlier[start..].to_vec(),
                (0..list.len()).find(|&i| list[i].id > scene),
            )
        }
    };
    out.current = here.map(text_of).unwrap_or_default();
    out.before = before
        .into_iter()
        .map(|index| (list[index].id.display(digits), text_of(index)))
        .collect();
    out.after = after.map(|index| (list[index].id.display(digits), text_of(index)));
    // 5 cast / 6 lexicon / 7 materials: 節の本文の行と、話プロットの場面・節の行
    let contents = project.document(&contents_path);
    let section_lines = here.map(|index| list[index].lines.clone());
    let plot_path = project.episode_plot_path(scene.chapter, scene.episode);
    let plot_document = project.document(&plot_path);
    let mut order: Vec<usize> = Vec::new();
    let mut line_start = 0;
    if let (Some(document), Some(lines)) = (contents, section_lines.clone()) {
        let parsed =
            crate::core::syntax::contents::parse_with(&document.text, &project.contents_options());
        reference::characters_in(
            document,
            Some(&parsed),
            std::slice::from_ref(&lines),
            project,
            &mut order,
        );
        line_start = line_starts_of(&document.text)
            .get(lines.start)
            .copied()
            .unwrap_or(0);
    }
    if let (Some(document), Some((scene_span, _))) = (plot_document, plot_ranges.as_ref()) {
        reference::characters_in(
            document,
            None,
            std::slice::from_ref(scene_span),
            project,
            &mut order,
        );
    }
    out.cast = reference::cast_of(project, &contents_path, scene, line_start, order);
    out.lexicon = lexicon(project, &out.current, &out.plot_section);
    out.materials = materials(project, scene, contents.zip(section_lines), plot_document);
    if inputs.role == "ideator" {
        out.ideas = ideas::rows(project)
            .into_iter()
            .map(|row| format!("- {} {}", row.id, row.title))
            .collect();
    }
    out.history = history(inputs);
    out.target_chars = reference::target_length(project, scene)
        .map(|found| found as usize)
        .or_else(|| {
            let target = project.settings().writing.section_target_chars as usize;
            (target > 0).then_some(target)
        });
    out
}

fn outline(project: &Project, scene: SceneId, out: &mut Sources) {
    let summary_line = |outline: &crate::core::syntax::plot_outline::Outline<'_>, want: SceneId| {
        let index = outline
            .blocks
            .iter()
            .position(|block| block.id == Some(want))?;
        let first = |key: FieldKey| {
            outline
                .fields_of(index, key)
                .first()
                .map(|field| field.value.trim().to_owned())
                .unwrap_or_default()
        };
        Some((first(FieldKey::Summary), first(FieldKey::Goal)))
    };
    if let Some(all) = project.all_plot() {
        let want = SceneId {
            episode: 0,
            scene: 0,
            section: 0,
            ..scene
        };
        if let Some((summary, _)) = summary_line(&all, want) {
            out.outline_all = summary;
        }
    }
    if let Some(chapter) = project.chapter_plot(scene.chapter) {
        let want = SceneId {
            scene: 0,
            section: 0,
            ..scene
        };
        if let Some((summary, goal)) = summary_line(&chapter, want) {
            out.outline_chapter = summary;
            out.goal = goal;
        }
    }
}

/// 3 plot。`(場面の行, 節の行)` を返す（キャラと用語を拾う範囲）。
fn plot(
    project: &Project,
    scene: SceneId,
    out: &mut Sources,
) -> Option<(std::ops::Range<usize>, Option<std::ops::Range<usize>>)> {
    let plot = project.episode_plot(scene.chapter, scene.episode)?;
    let whole = SceneId {
        section: 0,
        ..scene
    };
    let (index, found) = plot
        .scenes
        .iter()
        .enumerate()
        .find(|(_, found)| found.id.map(|id| SceneId { section: 0, ..id }) == Some(whole))?;
    let raw = |range: std::ops::Range<usize>| {
        let lines: Vec<&str> = range
            .filter_map(|line| plot.lines.get(line))
            .map(|line| line.raw())
            .collect();
        trim_tail(&lines)
    };
    out.plot_scene = raw(found.span.clone());
    let at = found
        .sections
        .iter()
        .position(|section| section.number == scene.section);
    let section_span = at.map(|at| found.sections[at].span.clone());
    if let Some(at) = at {
        out.plot_section = raw(found.sections[at].span.clone());
        out.check = plot
            .section_fields(index, at, FieldKey::Check)
            .iter()
            .map(|field| field.full_value())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
    }
    Some((found.span.clone(), section_span))
}

fn trim_tail(lines: &[&str]) -> String {
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |at| at + 1);
    lines[..end].join("\n")
}

fn lexicon(project: &Project, current: &str, plot_section: &str) -> Vec<(String, String)> {
    let dictionary = project.dictionary();
    let words = project.words();
    let mut text = current.to_owned();
    text.push('\n');
    text.push_str(plot_section);
    // (語の index) → (回数, 最初の位置, 語)
    let mut seen: BTreeMap<usize, (usize, usize, String)> = BTreeMap::new();
    for found in dictionary.occurrences(&text) {
        let Some(entry) = dictionary.entry(found.entry) else {
            continue;
        };
        let Target::Word { index, term } = &entry.target else {
            continue;
        };
        let slot = seen
            .entry(*index)
            .or_insert_with(|| (0, found.range.start, term.clone()));
        slot.0 += 1;
    }
    let mut rows: Vec<(usize, (usize, usize, String))> = seen.into_iter().collect();
    rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.1 .1.cmp(&b.1 .1)));
    rows.into_iter()
        .map(|(index, (_, _, term))| {
            let definition = words
                .word_fields(index, FieldKey::Definition)
                .first()
                .map(|field| field.value.trim().to_owned())
                .unwrap_or_default();
            (term, definition)
        })
        .collect()
}

fn materials(
    project: &Project,
    scene: SceneId,
    contents: Option<(&Document, std::ops::Range<usize>)>,
    plot: Option<&Document>,
) -> Vec<MaterialSource> {
    let mut ids: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    if let Some((document, lines)) = contents {
        reference::collect_refs(
            document,
            std::slice::from_ref(&lines),
            project,
            &mut ids,
            &mut names,
        );
    }
    if let (Some(document), Some((_, ranges, _))) = (plot, reference::plot_scope(project, scene)) {
        reference::collect_refs(document, &ranges, project, &mut ids, &mut names);
    }
    let index = project.materials();
    ids.into_iter()
        .map(|id| {
            let item = index
                .index
                .as_ref()
                .and_then(|parsed| parsed.items.iter().find(|item| item.id == id));
            let title = item.map(|item| item.title.clone()).unwrap_or_default();
            let summary = item
                .and_then(|item| item.summary.clone())
                .unwrap_or_default();
            let body = item
                .and_then(|item| project.document(item.path.trim_start_matches("./")))
                .map(|document| material_body(&document.text))
                .unwrap_or_default();
            let mut head = format!("- {id} {title}").trim_end().to_owned();
            if !summary.is_empty() {
                head.push_str("\n  summary: ");
                head.push_str(&crate::core::agents::one_line(&summary));
            }
            MaterialSource { head, id, body }
        })
        .collect()
}

/// 資料 MD の本文（最初の H1 と ID のコメント行を除き、前後の空行を落とす）。
fn material_body(text: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut heading_seen = false;
    for line in text.lines() {
        if !heading_seen && line.starts_with("# ") {
            heading_seen = true;
            continue;
        }
        if line.trim_start().starts_with("<!--") && line.trim_end().ends_with("-->") {
            continue;
        }
        lines.push(line);
    }
    let start = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    trim_tail(&lines[start..])
}

fn history(inputs: &Inputs<'_>) -> Vec<String> {
    let board = Board::of(inputs.project);
    let mut out = Vec::new();
    let others = board
        .tasks
        .iter()
        .rev()
        .filter(|task| Some(task.id.as_str()) != inputs.task);
    if inputs.role == "integrator" {
        for task in others.take(INTEGRATOR_HISTORY) {
            out.push(
                format!(
                    "- {} {} {} {} ({})",
                    task.id, task.role, task.scope, task.status, task.initiator
                )
                .trim_end()
                .to_owned(),
            );
        }
    } else {
        for task in others
            .filter(|task| files::task_scene(task) == Some(inputs.scene))
            .take(crate::core::agents::HISTORY_TASKS)
        {
            let mut line = format!("- {} {} {}", task.id, task.role, task.status);
            let comment = board
                .proposal_of(&task.id)
                .map(|proposal| proposal.comment.clone())
                .unwrap_or_default();
            if !comment.is_empty() {
                line.push_str(": ");
                line.push_str(&comment);
            }
            out.push(line);
        }
    }
    // 届いた伝言（§15.6: 人間が承認したか、自律の自動受け渡し）
    for thread in &board.threads {
        let covers = PartialSceneId::parse_with_sigil(thread.scope.trim())
            .map(|scope| scope.expand_low() <= inputs.scene && inputs.scene <= scope.expand_high())
            .unwrap_or(false);
        if !covers || thread.status == "closed" {
            continue;
        }
        for message in &thread.messages {
            if message.to == inputs.role && message.delivered() {
                out.push(format!(
                    "- msg {}#{:03} {} -> {} ({}): {}",
                    thread.id,
                    message.number,
                    message.from,
                    message.to,
                    message.kind,
                    crate::core::agents::one_line(&message.body)
                ));
            }
        }
    }
    out
}

fn tones(cast: &Cast) -> String {
    cast.tone
        .iter()
        .map(|pattern| format!("「{pattern}」"))
        .collect::<Vec<_>>()
        .join("")
}

fn cast_entry(cast: &Cast) -> String {
    let mut out = format!("- name: {}", cast.name);
    let mut push = |key: &str, value: &str| {
        if !value.trim().is_empty() {
            out.push_str(&format!(
                "\n  {key}: {}",
                crate::core::agents::one_line(value)
            ));
        }
    };
    push("mode", &cast.modes.join(" / "));
    push("personality", cast.personality.as_deref().unwrap_or(""));
    push("emotion", cast.emotion.as_deref().unwrap_or(""));
    push("role", cast.role.as_deref().unwrap_or(""));
    push("role-language", &tones(cast));
    out
}

fn lexicon_entry(term: &str, definition: &str) -> String {
    if definition.is_empty() {
        format!("- {term}")
    } else {
        format!("- {term}: {definition}")
    }
}

fn parts_of(
    kind: PacketBlock,
    sources: &Sources,
    prompt: &Rendered,
    contract: &str,
    inputs: &Inputs<'_>,
) -> Vec<Part> {
    match kind {
        PacketBlock::Project => vec![
            Part::new("title", sources.title.clone(), Style::Inline),
            Part::new("genre", sources.genre.clone(), Style::Inline),
            Part::new("style", sources.style.clone(), Style::Block),
        ],
        PacketBlock::Outline => vec![
            Part::new("all", sources.outline_all.clone(), Style::Inline).trimmable(4, 0),
            Part::new("chapter", sources.outline_chapter.clone(), Style::Inline).trimmable(6, 0),
            Part::new("goal", sources.goal.clone(), Style::Inline).trimmable(6, 1),
        ],
        PacketBlock::Plot => vec![
            Part::new("scene", sources.plot_scene.clone(), Style::Block).trimmable(7, 0),
            Part::new("section", sources.plot_section.clone(), Style::Block),
        ],
        PacketBlock::Target => {
            let current = if sources.current.trim().is_empty() {
                UNWRITTEN.to_owned()
            } else {
                sources.current.clone()
            };
            let mut out = vec![Part::new("current", current, Style::Block)];
            // 遠い節から削る（before の先頭が遠い）。after はその前に削る
            for (at, (id, text)) in sources.before.iter().enumerate() {
                out.push(
                    Part::new(format!("before {id}"), text.clone(), Style::Block)
                        .trimmable(3, 1 + at as u32),
                );
            }
            if let Some((id, text)) = &sources.after {
                out.push(
                    Part::new(format!("after {id}"), text.clone(), Style::Block).trimmable(3, 0),
                );
            }
            out
        }
        PacketBlock::Cast => sources
            .cast
            .iter()
            .map(|found| Part::new(found.name.clone(), cast_entry(found), Style::Bare))
            .collect(),
        PacketBlock::Lexicon => {
            let count = sources.lexicon.len();
            sources
                .lexicon
                .iter()
                .enumerate()
                .map(|(at, (term, definition))| {
                    // 出現回数の少ない語（並びの後ろ）から削る
                    Part::new(term.clone(), lexicon_entry(term, definition), Style::Bare)
                        .trimmable(5, (count - at) as u32)
                })
                .collect()
        }
        PacketBlock::Materials => {
            let count = sources.materials.len();
            let mut out = Vec::new();
            for (at, found) in sources.materials.iter().enumerate() {
                out.push(Part::new(found.id.clone(), found.head.clone(), Style::Bare));
                if !found.body.is_empty() {
                    // 本文 → summary のみに縮退（後ろの資料から）
                    out.push(
                        Part::new(
                            format!("body {}", found.id),
                            found.body.clone(),
                            Style::Block,
                        )
                        .trimmable(1, (count - at) as u32),
                    );
                }
            }
            if !sources.ideas.is_empty() {
                out.push(
                    Part::new("ideas", sources.ideas.join("\n"), Style::Block)
                        .trimmable(1, count as u32 + 1),
                );
            }
            out
        }
        PacketBlock::Constraints => {
            let mut out = vec![
                Part::new("output", prompt.output.clone(), Style::Block),
                Part::new("contract", contract.to_owned(), Style::Block),
            ];
            if inputs.role == "writer" {
                if let Some(target) = sources.target_chars {
                    out.push(Part::new(
                        "chars",
                        format!("{target} (+-20%)"),
                        Style::Inline,
                    ));
                }
            }
            out.push(Part::new(
                "annotations",
                "do not use novnow annotations (@ ' ;) unless instructed",
                Style::Inline,
            ));
            if sources.fixed {
                out.push(Part::new(
                    "state",
                    "fixed (quote only; do not propose changes)",
                    Style::Inline,
                ));
            }
            out
        }
        PacketBlock::History => {
            let count = sources.history.len();
            sources
                .history
                .iter()
                .enumerate()
                .map(|(at, line)| {
                    // 古いもの（並びの後ろ）から削る
                    Part::new(format!("history {at}"), line.clone(), Style::Bare)
                        .trimmable(2, (count - at) as u32)
                })
                .collect()
        }
    }
}
