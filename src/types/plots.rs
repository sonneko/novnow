//! plots/*.txt の構造表現。シーン ID とプロットノード。

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// `A-B-C-D` 形式のシーン ID。
///
/// * contents / plots 内の見出しは `@01-04-02-03`（D 省略時は `@01-04-02`）で現れる。
/// * D が省略された場合は `section = 0`（場面全体）を意味する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SceneId {
    pub chapter: u16,
    pub episode: u16,
    pub scene: u16,
    pub section: u16,
}

impl SceneId {
    pub fn new(chapter: u16, episode: u16, scene: u16, section: u16) -> Self {
        Self { chapter, episode, scene, section }
    }

    /// 場面レベル（D なし）か。
    pub fn is_scene_level(&self) -> bool {
        self.section == 0
    }

    /// `01-04-02`（場面まで）。
    pub fn scene_key(&self) -> String {
        format!("{:02}-{:02}-{:02}", self.chapter, self.episode, self.scene)
    }

    /// `01-04`（話まで）。
    pub fn episode_key(&self) -> String {
        format!("{:02}-{:02}", self.chapter, self.episode)
    }

    /// `01`（章）。
    pub fn chapter_key(&self) -> String {
        format!("{:02}", self.chapter)
    }

    /// `01-04-02-03`（全 4 階層、末尾 0 埋めあり）。
    pub fn full(&self) -> String {
        format!("{:02}-{:02}-{:02}-{:02}", self.chapter, self.episode, self.scene, self.section)
    }

    /// `@` を除いた表示用 ID。D が 0 のときは 3 階層で表示。
    pub fn display(&self) -> String {
        if self.is_scene_level() {
            self.scene_key()
        } else {
            self.full()
        }
    }
}

impl fmt::Display for SceneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

impl FromStr for SceneId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cleaned = s.trim().trim_start_matches('@');
        let parts: Vec<&str> = cleaned.split('-').collect();
        if parts.len() < 3 || parts.len() > 4 {
            anyhow::bail!("SceneId は A-B-C または A-B-C-D の形式です: {s:?}");
        }
        let nums: Result<Vec<u16>, _> = parts.iter().map(|p| p.parse::<u16>()).collect();
        let nums = nums.map_err(|_| anyhow::anyhow!("SceneId に数値でない成分があります: {s:?}"))?;
        Ok(SceneId {
            chapter: nums[0],
            episode: nums[1],
            scene: nums[2],
            section: nums.get(3).copied().unwrap_or(0),
        })
    }
}

/// プロットファイル 1 つ分（= 1 話）の解析結果。
#[derive(Debug, Clone, Default)]
pub struct PlotFile {
    pub id: String,
    pub nodes: Vec<PlotNode>,
}

/// プロットの 1 ノード。見出し or `;` 区切りのブロック。
#[derive(Debug, Clone, Default)]
pub struct PlotNode {
    pub scene: Option<SceneId>,
    /// 見出し行の元テキスト（`@01-04-01 /* ... */` 等）。
    pub header: String,
    /// 本文（`;` で区切られた各ブロックのテキスト）。
    pub body: String,
    pub line: usize,
}

impl PlotNode {
    pub fn is_header(&self) -> bool {
        self.scene.is_some() && !self.header.is_empty()
    }

    /// 先頭行をタイトル的に使う。
    pub fn first_line(&self) -> String {
        self.body.lines().next().unwrap_or("").trim().to_string()
    }
}
