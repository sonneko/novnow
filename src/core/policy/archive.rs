//! Zip に入れるもの・取り込むもの（§13.5、仕様 第 2.9 版。`PLANS.md` §9.2 B-93）。
//!
//! Zip の読み書きそのもの（`zip` クレート）は core で使えない（§3.2）ので `adapters/zip.rs` が持ち、
//! ここは**どのエントリを扱うか**だけを決める。
//!
//! | 方向 | 規則 |
//! |---|---|
//! | エクスポート | `.novnow/` を含めない。名前がドットで始まるもの（`.git/`・`.DS_Store`・一時ファイル）も含めない。ただし `.gitignore` / `.gitkeep` は含める（§14.5 の `novnow new` が作る）。`exports/` は含める（書き出し先の Zip 自身は呼び出し側が除く） |
//! | インポート | 区切りの `\` は `/` に読み替える。ディレクトリのエントリと `__MACOSX/` は無視。**単一のルートディレクトリ**があれば剥がす。ドットファイル（上の 2 つを除く）は無視。絶対パス・`..`・`:` を含む名前は**取り込まずに一覧で返す**（プロジェクト外への書き込みになるため、§13.1）。同じパスの 2 件目も返す |

use std::collections::BTreeSet;

use super::write_policy::{PathError, ProjectPath};

/// ドットで始まっても Zip に入れる・取り込む名前（§13.5 第 2.9 版）。
pub const KEPT_DOTFILES: &[&str] = &[".gitignore", ".gitkeep"];

/// macOS が Zip に足すメタデータのディレクトリ（§13.5）。
pub const MACOS_METADATA_DIR: &str = "__MACOSX";

/// プロジェクトの構成で使うディレクトリ名（§4.1）。Zip の単一ルートがこの名前で、しかも
/// その下に `project.json` が無ければ「ルートディレクトリ」ではなく中身とみなして剥がさない。
pub const LAYOUT_DIRS: &[&str] = &[
    "contents",
    "plots",
    "materials",
    "prompts",
    "agents",
    "assets",
    "exports",
];

/// Zip のエクスポートに含めるか（§13.5）。
pub fn is_exported(path: &ProjectPath) -> bool {
    !path.is_local_state() && !has_hidden_segment(path.as_str())
}

/// ドットで始まる要素（[`KEPT_DOTFILES`] を除く）を含むか。
fn has_hidden_segment(path: &str) -> bool {
    path.split('/')
        .any(|segment| segment.starts_with('.') && !KEPT_DOTFILES.contains(&segment))
}

/// Zip のエントリ 1 件（名前とディレクトリかどうか）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    /// Zip 内の名前（書かれたまま）。
    pub name: String,
    /// ディレクトリのエントリか。
    pub is_dir: bool,
}

impl ArchiveEntry {
    /// ファイルのエントリ。
    pub fn file(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_dir: false,
        }
    }

    /// ディレクトリのエントリ。
    pub fn dir(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_dir: true,
        }
    }
}

/// 取り込む 1 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    /// Zip 内の番号（渡した一覧の添字）。
    pub index: usize,
    /// 取り込み先（プロジェクトルートからの相対）。
    pub path: ProjectPath,
}

/// 取り込まなかった理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// `__MACOSX/` の下。
    MacMetadata,
    /// ドットファイル（`.gitignore` / `.gitkeep` 以外）。`.novnow/` もここ。
    Hidden,
    /// プロジェクト内の相対パスにできない（絶対パス・`..`・`:` 等）。
    Unsafe(PathError),
    /// 同じパスが先に出ている。
    Duplicate,
    /// UTF-8 として読めない（Web はテキストのみ、§13.3.2。判定は adapters）。
    NotText,
}

/// 取り込まなかったエントリ（UI が一覧で示す）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// Zip 内の名前（書かれたまま）。
    pub name: String,
    /// 理由。
    pub reason: SkipReason,
}

/// 取り込みの計画。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportPlan {
    /// 取り込むもの（Zip の順）。
    pub entries: Vec<Planned>,
    /// 取り込まないもの（ディレクトリのエントリは含めない）。
    pub skipped: Vec<Skipped>,
    /// 剥がした単一のルートディレクトリ。
    pub stripped_root: Option<String>,
}

/// Zip のエントリ一覧から取り込みの計画を立てる（§13.5）。
pub fn plan_import(entries: &[ArchiveEntry]) -> ImportPlan {
    let mut plan = ImportPlan::default();
    // (添字, 書かれた名前, `/` 区切りに直した名前)
    let mut candidates: Vec<(usize, &str, String)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let name = entry.name.replace('\\', "/");
        let trimmed = trim_current_dir(&name);
        if entry.is_dir || trimmed.is_empty() || trimmed.ends_with('/') {
            continue;
        }
        if trimmed
            .split('/')
            .any(|segment| segment == MACOS_METADATA_DIR)
        {
            plan.skipped.push(Skipped {
                name: entry.name.clone(),
                reason: SkipReason::MacMetadata,
            });
            continue;
        }
        candidates.push((index, entry.name.as_str(), trimmed.to_owned()));
    }
    let names: Vec<&str> = candidates
        .iter()
        .map(|(_, _, name)| name.as_str())
        .collect();
    plan.stripped_root = single_root(&names);
    let prefix = plan.stripped_root.as_ref().map(|root| format!("{root}/"));
    let mut seen = BTreeSet::new();
    for (index, original, name) in candidates {
        let relative = match &prefix {
            Some(prefix) => name.strip_prefix(prefix.as_str()).unwrap_or(&name),
            None => &name,
        };
        let reason = match ProjectPath::new(relative) {
            Err(error) => Some(SkipReason::Unsafe(error)),
            Ok(path) if has_hidden_segment(path.as_str()) => Some(SkipReason::Hidden),
            Ok(path) if !seen.insert(path.clone()) => Some(SkipReason::Duplicate),
            Ok(path) => {
                plan.entries.push(Planned { index, path });
                None
            }
        };
        if let Some(reason) = reason {
            plan.skipped.push(Skipped {
                name: original.to_owned(),
                reason,
            });
        }
    }
    plan
}

/// 先頭の `./` を外す（`./contents/01-01.txt` のような書き方の Zip がある）。
fn trim_current_dir(name: &str) -> &str {
    let mut rest = name;
    while let Some(after) = rest.strip_prefix("./") {
        rest = after;
    }
    rest
}

/// 全エントリが同じ 1 つのディレクトリの下にあれば、その名前（§13.5「単一のルートディレクトリ
/// があれば剥がす」）。
///
/// 剥がさないのは: エントリが無い・ルート直下にファイルがある・先頭の要素が 2 種類以上・
/// 先頭の要素が普通の名前でない（`..` 等。剥がして安全にすることはしない）・先頭の要素が
/// プロジェクトの構成のディレクトリ名（[`LAYOUT_DIRS`]）でその下に `project.json` が無い。
fn single_root(names: &[&str]) -> Option<String> {
    let mut root: Option<&str> = None;
    for name in names {
        let (first, _) = name.split_once('/')?;
        match root {
            None => root = Some(first),
            Some(found) if found == first => {}
            Some(_) => return None,
        }
    }
    let root = root?;
    let safe = ProjectPath::new(root).is_ok_and(|path| !path.is_hidden());
    if !safe {
        return None;
    }
    let has_project_file = names
        .iter()
        .any(|name| name.strip_prefix(root) == Some("/project.json"));
    if LAYOUT_DIRS.contains(&root) && !has_project_file {
        return None;
    }
    Some(root.to_owned())
}
