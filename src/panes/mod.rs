//! 3 ペインの UI 実装。

pub mod editor;
pub mod file_tree;
pub mod info_panel;

use std::collections::BTreeMap;

/// ファイルツリーに表示する 1 行。
#[derive(Debug, Clone)]
pub struct TreeRow {
    pub label: String,
    pub path: Option<std::path::PathBuf>,
    pub depth: usize,
    pub is_dir: bool,
    pub scene_id: Option<String>,
}

/// contents / plots ファイルから `章 → 話` のツリー構造を作る。
pub fn build_rows(
    contents: &[std::path::PathBuf],
    plots: &[std::path::PathBuf],
) -> Vec<TreeRow> {
    let mut rows: Vec<TreeRow> = Vec::new();

    rows.extend(build_section(
        "contents",
        contents,
        "contents 配下に *.txt がありません",
    ));
    rows.extend(build_section(
        "plots",
        plots,
        "plots 配下に話プロット (*.txt) がありません",
    ));

    rows
}

fn build_section(
    title: &str,
    files: &[std::path::PathBuf],
    empty_hint: &str,
) -> Vec<TreeRow> {
    let mut rows = vec![TreeRow {
        label: title.to_string(),
        path: None,
        depth: 0,
        is_dir: true,
        scene_id: None,
    }];

    let mut by_chapter: BTreeMap<String, Vec<(String, std::path::PathBuf)>> = BTreeMap::new();
    for p in files {
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if let Some((ch, ep)) = crate::project::resolver::parse_file_stem(stem) {
            by_chapter
                .entry(format!("{ch:02}"))
                .or_default()
                .push((format!("{ch:02}-{ep:02}"), p.clone()));
        } else {
            by_chapter
                .entry("(その他)".to_string())
                .or_default()
                .push((stem.to_string(), p.clone()));
        }
    }

    if by_chapter.is_empty() {
        rows.push(TreeRow {
            label: format!("({empty_hint})"),
            path: None,
            depth: 1,
            is_dir: true,
            scene_id: None,
        });
    }

    for (ch, eps) in by_chapter {
        rows.push(TreeRow {
            label: format!("{ch} 章"),
            path: None,
            depth: 1,
            is_dir: true,
            scene_id: None,
        });
        for (ep, p) in eps {
            rows.push(TreeRow {
                label: ep.clone(),
                path: Some(p),
                depth: 2,
                is_dir: false,
                scene_id: Some(ep),
            });
        }
    }
    rows
}

/// フォーカス先ペイン。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Tree,
    Editor,
    Info,
}

impl FocusPane {
    /// Ctrl+↑ / Ctrl+↓ で巡回させる。
    pub fn next(self) -> Self {
        match self {
            FocusPane::Tree => FocusPane::Editor,
            FocusPane::Editor => FocusPane::Info,
            FocusPane::Info => FocusPane::Tree,
        }
    }
}
