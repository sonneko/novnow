//! パス解決・参照追跡。
#![allow(dead_code)]

use crate::types::plots::SceneId;
use std::path::{Path, PathBuf};

/// `01-04.txt` のような話ファイル名から SceneId の章・話を取り出す。
pub fn parse_file_stem(stem: &str) -> Option<(u16, u16)> {
    let mut it = stem.split('-');
    let a = it.next()?.parse::<u16>().ok()?;
    let b = it.next()?.parse::<u16>().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b))
}

/// SceneId から contents 側の話ファイルパスを作る。
pub fn contents_path(root: &Path, id: &SceneId) -> PathBuf {
    root.join("contents").join(format!("{}-{}.txt", pad(id.chapter), pad(id.episode)))
}

/// SceneId から plots 側の話ファイルパスを作る。
pub fn plots_path(root: &Path, id: &SceneId) -> PathBuf {
    root.join("plots").join(format!("{}-{}.txt", pad(id.chapter), pad(id.episode)))
}

/// 章プロット `01.txt`。
pub fn chapter_plot_path(root: &Path, chapter: u16) -> PathBuf {
    root.join("plots").join(format!("{}.txt", pad(chapter)))
}

fn pad(v: u16) -> String {
    format!("{v:02}")
}

/// 2 つの SceneId が同一場面か（D は無視）。
pub fn same_scene(a: &SceneId, b: &SceneId) -> bool {
    a.chapter == b.chapter && a.episode == b.episode && a.scene == b.scene
}

/// 2 つの SceneId が同一話か。
pub fn same_episode(a: &SceneId, b: &SceneId) -> bool {
    a.chapter == b.chapter && a.episode == b.episode
}
