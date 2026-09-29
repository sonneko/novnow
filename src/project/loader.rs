//! JSON / テキストファイルの読み書き（tokio による非同期 I/O）。

#[cfg(not(target_arch = "wasm32"))]
use crate::project::resolver::parse_file_stem;
use crate::project::state::{PlotNodeRef, ProjectState};
#[cfg(not(target_arch = "wasm32"))]
use crate::types::characters::parse_characters;
#[cfg(not(target_arch = "wasm32"))]
use crate::types::materials::parse_materials;
use crate::types::plots::{PlotFile, PlotNode, SceneId};
#[cfg(not(target_arch = "wasm32"))]
use crate::types::words::parse_words;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 非同期でテキストファイルを読む。
#[cfg(not(target_arch = "wasm32"))]
pub async fn read_to_string_async(path: &Path) -> Result<String> {
    let s = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("読み込みに失敗: {}", path.display()))?;
    Ok(s)
}

#[cfg(target_arch = "wasm32")]
pub async fn read_to_string_async(path: &Path) -> Result<String> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("読み込みに失敗: {}", path.display()))?;
    Ok(s)
}

/// 非同期で書き込む（一時ファイル + rename による安全な保存は将来拡張）。
#[cfg(not(target_arch = "wasm32"))]
pub async fn write_string_async(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    tokio::fs::write(path, contents)
        .await
        .with_context(|| format!("書き込みに失敗: {}", path.display()))?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub async fn write_string_async(_path: &Path, _contents: &str) -> Result<()> {
    Ok(())
}

/// プロジェクトルート必須ファイル。
const REQUIRED: &[&str] = &[
    "contents",
    "materials/index.json",
    "plots/index.json",
    "plots/all.txt",
    "words.json",
    "characters.json",
    "ideas.json",
];

/// プロジェクトを読み込む。必須ファイルが無ければエラー。
#[cfg(not(target_arch = "wasm32"))]
pub async fn load_project(root: PathBuf) -> Result<ProjectState> {
    validate_layout(&root)?;

    let title = root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "novnow".to_string());

    let mut state = ProjectState {
        title,
        root: root.clone(),
        ..Default::default()
    };

    // characters.json
    let chars_text = read_to_string_async(&root.join("characters.json")).await?;
    let (chars, mut warns) = parse_characters(&chars_text).context("characters.json")?;
    state.characters = chars;

    // words.json
    let words_text = read_to_string_async(&root.join("words.json")).await?;
    let (words, w2) = parse_words(&words_text).context("words.json")?;
    state.words = words;
    warns.extend(w2);

    // materials/index.json
    let mat_text = read_to_string_async(&root.join("materials/index.json")).await?;
    let (mats, w3) = parse_materials(&mat_text).context("materials/index.json")?;
    warns.extend(w3);
    for m in mats {
        state.materials.insert(m.path.clone(), m);
    }

    // plots/*.txt（章プロット・話プロット・all.txt を走査）
    let plots_dir = root.join("plots");
    let mut rd = tokio::fs::read_dir(&plots_dir).await.context("plots/ を読めません")?;
    let mut plot_paths: Vec<PathBuf> = Vec::new();
    while let Some(entry) = rd.next_entry().await? {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("txt") {
            plot_paths.push(p);
        }
    }
    plot_paths.sort();

    let mut chapters: BTreeMap<u16, Vec<u16>> = BTreeMap::new();
    for p in &plot_paths {
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem == "all" {
            continue;
        }
        let text = read_to_string_async(p).await?;
        if let Some((chapter, episode)) = parse_file_stem(stem) {
            // 話プロット（`01-04.txt`）
            state.plots_files.push(p.clone());
            let file = parse_plot_file(stem, &text);
            for node in &file.nodes {
                if let Some(id) = node.scene {
                    state.plots.insert(
                        id,
                        PlotNodeRef {
                            scene: id,
                            source_file: stem.to_string(),
                            line: node.line,
                            text: node.body.clone(),
                        },
                    );
                    // 場面レベル ID でも引けるようにしておく。
                    let scene_only = SceneId::new(id.chapter, id.episode, id.scene, 0);
                    state.plots.entry(scene_only).or_insert_with(|| PlotNodeRef {
                        scene: scene_only,
                        source_file: stem.to_string(),
                        line: node.line,
                        text: node.body.clone(),
                    });
                }
            }
            state.plot_files.insert(stem.to_string(), file);
            chapters.entry(chapter).or_default().push(episode);
        }
        // 章プロット（`01.txt`）は plot_files には入れず chapters のみ。
        else if let Ok(ch) = stem.parse::<u16>() {
            chapters.entry(ch).or_default();
        }
    }
    for v in chapters.values_mut() {
        v.sort();
        v.dedup();
    }
    state.chapters = chapters.into_iter().collect();

    // contents/*.txt（ファイルツリー用）
    let contents_dir = root.join("contents");
    if let Ok(mut rd) = tokio::fs::read_dir(&contents_dir).await {
        while let Some(entry) = rd.next_entry().await? {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("txt") {
                state.contents_files.push(p);
            }
        }
    }
    state.contents_files.sort();

    state.warnings = warns;
    Ok(state)
}

#[cfg(target_arch = "wasm32")]
pub async fn load_project(_root: PathBuf) -> Result<ProjectState> {
    Ok(scaffold_in_memory_state())
}

/// レイアウト検証。
fn validate_layout(root: &Path) -> Result<()> {
    if !root.exists() {
        anyhow::bail!("プロジェクトディレクトリが存在しません: {}", root.display());
    }
    let mut missing: Vec<&str> = Vec::new();
    for req in REQUIRED {
        if !root.join(req).exists() {
            missing.push(req);
        }
    }
    if !missing.is_empty() {
        anyhow::bail!(
            "必須ファイル/ディレクトリが不足しています: {}",
            missing.join(", ")
        );
    }
    Ok(())
}

/// plots/*.txt の 1 ファイルを解析する。
///
/// 見出し `@A-B-C[-D]` ごとにブロックを切り、`;` で区切られた各ブロックの本文を保持する。
pub fn parse_plot_file(stem: &str, text: &str) -> PlotFile {
    let mut file = PlotFile { id: stem.to_string(), nodes: Vec::new() };
    let mut cur: Option<PlotNode> = None;

    for (i, raw) in text.lines().enumerate() {
        if let Some((id, _end)) = crate::text::tokenizer::parse_header(raw) {
            if let Some(node) = cur.take() {
                file.nodes.push(node);
            }
            cur = Some(PlotNode {
                scene: Some(id),
                header: raw.trim().to_string(),
                body: String::new(),
                line: i,
            });
            continue;
        }
        let trimmed = raw.trim();
        if trimmed.starts_with(';') {
            // ノード区切り（`;` または `; /* ... */`）: 直前ノードの本文を伸ばすだけで、`;` 行は本文に含めない。
            continue;
        }
        if let Some(node) = cur.as_mut() {
            if !node.body.is_empty() {
                node.body.push('\n');
            }
            node.body.push_str(raw);
        }
    }
    if let Some(node) = cur.take() {
        file.nodes.push(node);
    }
    file
}

pub fn scaffold_in_memory_state() -> ProjectState {
    let root = PathBuf::from("my-novel");
    let mut state = ProjectState {
        title: "my-novel".to_string(),
        root: root.clone(),
        ..Default::default()
    };

    let mut char_map = crate::types::characters::CharacterMap::new();
    char_map.insert(
        "アルス".to_string(),
        crate::types::characters::Character {
            name: "アルス".to_string(),
            role: "主人公".to_string(),
            personality: "正義感が強い魔法使い".to_string(),
            speech: "僕、～だよ".to_string(),
            notes: "デモプロジェクト".to_string(),
            ..Default::default()
        },
    );
    char_map.insert(
        "リリア".to_string(),
        crate::types::characters::Character {
            name: "リリア".to_string(),
            role: "ヒロイン".to_string(),
            personality: "冷静沈着な剣士".to_string(),
            speech: "私、～ね".to_string(),
            notes: "デモプロジェクト".to_string(),
            ..Default::default()
        },
    );
    state.characters = char_map;

    let mut word_map = crate::types::words::WordMap::new();
    word_map.insert(
        "魔導書".to_string(),
        crate::types::words::Word {
            term: "魔導書".to_string(),
            reading: "まどうしょ".to_string(),
            definition: "古代の魔法が記された書物".to_string(),
            category: "用語".to_string(),
            ..Default::default()
        },
    );
    state.words = word_map;

    let contents_file = root.join("contents/01-01.txt");
    let plots_file = root.join("plots/01-01.txt");

    state.contents_files = vec![contents_file];
    state.plots_files = vec![plots_file];

    let plot_text = "@01-01-01\nアルスとリリアが出会う場面\n;\n";
    let file = parse_plot_file("01-01", plot_text);
    let sid = SceneId::new(1, 1, 1, 0);
    state.plots.insert(
        sid,
        PlotNodeRef {
            scene: sid,
            source_file: "01-01".to_string(),
            line: 0,
            text: "アルスとリリアが出会う場面".to_string(),
        },
    );
    state.plot_files.insert("01-01".to_string(), file);

    let mut chapters = BTreeMap::new();
    chapters.insert(1, vec![1]);
    state.chapters = chapters.into_iter().collect();

    state
}

/// 新規プロジェクトの雛形を作成する。
#[cfg(not(target_arch = "wasm32"))]
pub async fn scaffold_project(root: &Path) -> Result<()> {
    let dirs = ["contents", "materials", "plots", "skills", "agents", "log"];
    for d in dirs {
        tokio::fs::create_dir_all(root.join(d)).await.ok();
    }

    write_if_absent(&root.join("characters.json"), "[]\n").await?;
    write_if_absent(&root.join("words.json"), "[]\n").await?;
    write_if_absent(&root.join("ideas.json"), "[]\n").await?;
    write_if_absent(
        &root.join("materials/index.json"),
        "[]\n",
    )
    .await?;
    write_if_absent(&root.join("plots/index.json"), "[]\n").await?;
    write_if_absent(&root.join("plots/all.txt"), "@00-00-00\n;\n").await?;
    write_if_absent(&root.join("plots/preparations.json"), "[]\n").await?;
    write_if_absent(&root.join("contents/01-01.txt"), "@01-01-01\n;\n").await?;
    write_if_absent(&root.join("plots/01-01.txt"), "@01-01-01\n;\n").await?;
    write_if_absent(
        &root.join("project.json"),
        "{\n  \"scene_id_format\": \"A-B-C-D\",\n  \"content_encoding\": \"utf-8\",\n  \"newline\": \"lf\",\n  \"plot_file_extension\": \"txt\"\n}\n",
    )
    .await?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
async fn write_if_absent(path: &Path, contents: &str) -> Result<()> {
    if !path.exists() {
        write_string_async(path, contents).await?;
    }
    Ok(())
}
