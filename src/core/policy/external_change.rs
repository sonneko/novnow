//! 外部変更の検出と反応（§8.5、`docs/history.md` の作業項目 5・B-95）。
//!
//! §8.5: プロジェクトは **1,000ms 間隔のポーリング**で見る（v0 踏襲。`inotify` 等は使わない＝
//! Web と実装を揃える）。分担は次のとおり。
//!
//! | 誰が | 何を |
//! |---|---|
//! | UI（M7） | [`POLL_INTERVAL_MS`] ごとにストレージの一覧（`Storage::list`）を取る |
//! | ここ | 前回の一覧（[`Snapshot`]）と比べて**候補**を出す（[`Snapshot::diff`]）。候補の中身を読んだら、知っている版と比べて**反応**を決める（[`react`]） |
//! | UI | 反応に従って再読込・衝突表示・I052 の通知をする |
//!
//! スタンプ（更新時刻と長さ）だけで判断せず、**中身を比べてから**反応を決める。自分の保存も
//! スタンプを変えるが、中身は知っている版と同じなので [`Reaction::Ignore`] になる（`touch` だけの
//! 変更も同じ）。比べる前に §5.0.4 の読み込み時の正規化（BOM・改行）を両方にかける。

use std::collections::BTreeMap;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::features::project::is_dictionary_source;
use crate::core::text::lexer;

use super::write_policy::ProjectPath;

/// ポーリングの間隔（§8.5「1,000ms 間隔」）。
pub const POLL_INTERVAL_MS: u64 = 1_000;

/// ファイルのスタンプ（変わったかどうかの候補を見つけるためだけの値）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Stamp {
    /// 最終更新（UNIX 時刻のミリ秒）。分からなければ `None`。
    pub modified_ms: Option<i64>,
    /// 長さ（byte）。
    pub len: u64,
}

/// ストレージの一覧の 1 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// プロジェクトルートからの相対パス。
    pub path: ProjectPath,
    /// スタンプ。
    pub stamp: Stamp,
}

/// ポーリングで見るか（§8.5）。`.novnow/`（ローカル状態）・`exports/`（出力先）・
/// ドットで始まる名前（一時ファイル・`.git/` 等）は見ない。
pub fn is_watched(path: &ProjectPath) -> bool {
    !path.is_local_state() && !path.is_under("exports") && !path.is_hidden()
}

/// 前回の一覧（パス → スタンプ）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    stamps: BTreeMap<ProjectPath, Stamp>,
}

/// 候補の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// 新しく現れた。
    Created,
    /// スタンプが変わった。
    Modified,
    /// 消えた。
    Removed,
}

/// 外部変更の候補。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// パス。
    pub path: ProjectPath,
    /// 種類。
    pub kind: ChangeKind,
}

impl Snapshot {
    /// 一覧から作る（[`is_watched`] でないものは落とす）。
    pub fn from_entries(entries: impl IntoIterator<Item = FileEntry>) -> Self {
        Self {
            stamps: entries
                .into_iter()
                .filter(|entry| is_watched(&entry.path))
                .map(|entry| (entry.path, entry.stamp))
                .collect(),
        }
    }

    /// 自分で書いたファイルのスタンプを覚える（次の比較で候補にしない）。
    pub fn acknowledge(&mut self, path: ProjectPath, stamp: Stamp) {
        if is_watched(&path) {
            self.stamps.insert(path, stamp);
        }
    }

    /// 自分で消したファイルを忘れる。
    pub fn forget(&mut self, path: &ProjectPath) {
        self.stamps.remove(path);
    }

    /// スタンプ。
    pub fn stamp(&self, path: &ProjectPath) -> Option<Stamp> {
        self.stamps.get(path).copied()
    }

    /// 件数。
    pub fn len(&self) -> usize {
        self.stamps.len()
    }

    /// 空か。
    pub fn is_empty(&self) -> bool {
        self.stamps.is_empty()
    }

    /// 前回（`self`）から今回（`now`）への候補（パスの昇順）。
    pub fn diff(&self, now: &Snapshot) -> Vec<Change> {
        let mut out = Vec::new();
        for (path, stamp) in &now.stamps {
            let kind = match self.stamps.get(path) {
                None => ChangeKind::Created,
                Some(before) if before != stamp => ChangeKind::Modified,
                Some(_) => continue,
            };
            out.push(Change {
                path: path.clone(),
                kind,
            });
        }
        for path in self.stamps.keys() {
            if !now.stamps.contains_key(path) {
                out.push(Change {
                    path: path.clone(),
                    kind: ChangeKind::Removed,
                });
            }
        }
        out.sort_by(|left, right| left.path.cmp(&right.path));
        out
    }
}

/// 開いている文書のバッファの状態（§8.2 の保存ステートマシンを 2 つに畳んだもの）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Buffer {
    /// 未保存の変更が無い（`Clean`）。読み込んでいないファイルもこちら。
    Clean,
    /// 未保存の変更がある（`Dirty` / `Saving` / `SaveFailed`）。
    Dirty,
}

/// 候補に対する反応（§8.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reaction {
    /// 何もしない（中身が知っている版と同じ。自分の保存や `touch`）。
    Ignore,
    /// 読み直す（消えたなら文書を外す）。カーソル位置は保ち、**I052** を出す（[`notice`]）。
    Reload,
    /// 自動では反映せず、ステータスバーに衝突を出す（`Ctrl+Shift+D` で差分、§8.5）。
    Conflict,
}

/// 候補 1 件への反応を決める（§8.5）。
///
/// - `known`: 今持っている版（読み込んでいなければ `None`）。
/// - `disk`: 今ディスク（ストレージ）にある版（消えていれば `None`）。
///
/// 中身が同じなら [`Reaction::Ignore`]。違えば、辞書の 3 ファイル（`characters.md` / `words.md` /
/// `materials/index.json`）は**常に** [`Reaction::Reload`]（§8.5「辞書は編集中バッファを持たない」）、
/// それ以外は Clean なら再読込・Dirty なら衝突。
pub fn react(
    path: &ProjectPath,
    buffer: Buffer,
    known: Option<&str>,
    disk: Option<&str>,
) -> Reaction {
    let same = match (known, disk) {
        (Some(known), Some(disk)) => {
            lexer::normalize_on_load(known) == lexer::normalize_on_load(disk)
        }
        (None, None) => true,
        _ => false,
    };
    if same {
        return Reaction::Ignore;
    }
    if is_dictionary_source(path.as_str()) || buffer == Buffer::Clean {
        Reaction::Reload
    } else {
        Reaction::Conflict
    }
}

/// I052「外部の変更を読み込みました: {0}」（§7.1。文書位置を持たない通知）。
pub fn notice(path: &ProjectPath) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::I052, None, vec![Arg::text(path.as_str())])
}
