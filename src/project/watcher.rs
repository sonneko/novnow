//! ファイル監視と自動リロード。
//!
//! `notify` crate の依存を避け、クロスプラットフォームで完結する自前ポーリング方式を採る。
//! ワーカースレッドから定期的に mtime を取得し、変化があれば UI へ通知する。

use std::path::PathBuf;
use std::sync::mpsc::Sender;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, SystemTime};

/// 監視対象の変更通知。
#[derive(Debug, Clone)]
pub struct FileChanged {
    pub path: PathBuf,
}

/// ポーリング監視スレッドを起動する。
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn(root: PathBuf, tx: Sender<FileChanged>) {
    std::thread::Builder::new()
        .name("novnow-watch".into())
        .spawn(move || {
            let targets: Vec<PathBuf> = vec![
                root.join("characters.json"),
                root.join("words.json"),
                root.join("materials/index.json"),
                root.join("plots/index.json"),
                root.join("ideas.json"),
            ];
            let mut snapshots: Vec<(PathBuf, Option<SystemTime>)> = targets
                .iter()
                .map(|p| (p.clone(), mtime(p)))
                .collect();

            loop {
                std::thread::sleep(Duration::from_millis(750));
                for (path, last) in snapshots.iter_mut() {
                    let now = mtime(path);
                    if now != *last {
                        *last = now;
                        let _ = tx.send(FileChanged { path: path.clone() });
                    }
                }
            }
        })
        .ok();
}

#[cfg(target_arch = "wasm32")]
pub fn spawn(_root: PathBuf, _tx: Sender<FileChanged>) {}

#[cfg(not(target_arch = "wasm32"))]
fn mtime(p: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(p).ok().and_then(|m| m.modified().ok())
}
