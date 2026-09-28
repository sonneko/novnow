//! プロジェクト全体のインメモリキャッシュ。
//!
//! HashMap ベースで即時参照できるようにする。

use crate::types::characters::CharacterMap;
use crate::types::materials::Material;
use crate::types::plots::{PlotFile, SceneId};
use crate::types::words::WordMap;
use std::collections::HashMap;
use std::path::PathBuf;

/// プロットノードを ID で引くための束。
#[derive(Debug, Default)]
pub struct ProjectState {
    pub title: String,
    pub root: PathBuf,

    /// キャラ名 → キャラ。
    pub characters: CharacterMap,
    /// 用語 → 用語。
    pub words: WordMap,
    /// `01-04-02-03` / `01-04-02` → プロット本文。
    pub plots: HashMap<SceneId, PlotNodeRef>,
    /// 資料パス → 資料。
    pub materials: HashMap<String, Material>,
    /// 話ファイル（`01-04.txt`）単位のプロット。全ノード順序を保持。
    pub plot_files: HashMap<String, PlotFile>,
    /// 章 → 話番号リスト（ファイルツリー表示用）。
    pub chapters: HashMap<u16, Vec<u16>>,
    /// 起動時のバリデーション結果（右パネル / モーダルに出す）。
    pub warnings: Vec<String>,

    /// ディレクトリ走査結果。
    pub contents_files: Vec<PathBuf>,
    pub plots_files: Vec<PathBuf>,
}

/// プロット本文への参照（どのファイルのどのノードか）。
#[derive(Debug, Clone)]
pub struct PlotNodeRef {
    pub scene: SceneId,
    pub source_file: String,
    pub line: usize,
    pub text: String,
}

impl ProjectState {
    pub fn character(&self, name: &str) -> Option<&crate::types::characters::Character> {
        self.characters.get(name)
    }

    pub fn word(&self, term: &str) -> Option<&crate::types::words::Word> {
        self.words.get(term)
    }

    pub fn plot(&self, id: &SceneId) -> Option<&PlotNodeRef> {
        self.plots.get(id)
    }
}
