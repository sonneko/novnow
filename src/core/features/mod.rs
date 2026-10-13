//! LSP のメソッドと 1:1 で対応する純粋関数（§3.4）。
//!
//! 引数は `(&Document, 位置, &Project)` の組み合わせだけ、戻り値は LSP の構造体に機械的に
//! 写せるプレーンな型（§3.4 MUST）。位置は**ファイル内の byte**（§3.4「core は内部的に
//! byte offset を使う」、`PLANS.md` §9.2 B-1）で、UTF-16 との変換は `text/position.rs` の
//! [`crate::core::text::position::LineTable`] が受け持つ（将来の LSP アダプタと UI の仕事）。
//!
//! **文言は返さない**（§12.2）。表示名が要るものは種別の enum を返し、UI が `ui/i18n/` で引く。
//! 引数の順は §3.4 の表のとおり（`semantic_tokens::full(doc, project)` など。`PLANS.md` §9.2 B-78）。

pub mod code_action;
pub mod completion;
pub mod definition;
pub mod document_symbol;
pub mod folding;
pub mod formatting;
pub mod hover;
pub mod inlay_hint;
/// LSP に対応しない補助（診断の「どこ」を作家の言葉で。§14.2.5・§10.9.2、第 2.11 版）。
pub mod place;
pub mod project;
/// LSP に対応しない補助（参照ペインの中身を集める。§9.5.2、`PLANS.md` §9.2 B-113）。
pub mod reference;
pub mod references;
pub mod rename;
/// LSP に対応しない補助（節単位の編集と節の並び。§8.1・§9.3・§9.8.2、`PLANS.md` §9.2 B-112）。
pub mod sections;
pub mod semantic_tokens;

pub use self::project::Project;

use std::ops::Range;

use serde::Serialize;

/// 文書の中の位置（LSP の `Location`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Location {
    /// プロジェクトルートからの相対パス。
    pub path: String,
    /// その文書の中の byte 範囲。
    pub range: Range<usize>,
}

/// テキスト編集 1 つ（LSP の `TextEdit`）。`range` を `new_text` で置き換える。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct TextEdit {
    /// 置き換える byte 範囲（空なら挿入）。
    pub range: Range<usize>,
    /// 新しい文字列。
    pub new_text: String,
}

/// 1 文書ぶんの編集（位置の昇順・重なりなし）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct FileEdit {
    /// プロジェクトルートからの相対パス。プロジェクトに無いパスなら**新しいファイル**を作る。
    pub path: String,
    /// 編集。
    pub edits: Vec<TextEdit>,
}

/// 複数文書にまたがる編集（LSP の `WorkspaceEdit`）。パスの昇順。
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize)]
pub struct WorkspaceEdit {
    /// 文書ごとの編集。
    pub changes: Vec<FileEdit>,
}

impl WorkspaceEdit {
    /// 1 文書だけの編集。
    pub fn single(path: impl Into<String>, edits: Vec<TextEdit>) -> Self {
        Self {
            changes: vec![FileEdit {
                path: path.into(),
                edits,
            }],
        }
    }

    /// 編集を足す（同じ文書ならまとめ、位置の昇順に並べ直す）。
    pub fn push(&mut self, path: &str, edit: TextEdit) {
        match self.changes.iter_mut().find(|found| found.path == path) {
            Some(found) => found.edits.push(edit),
            None => self.changes.push(FileEdit {
                path: path.to_string(),
                edits: vec![edit],
            }),
        }
        for change in &mut self.changes {
            change
                .edits
                .sort_by_key(|found| (found.range.start, found.range.end));
        }
        self.changes
            .sort_by(|left, right| left.path.cmp(&right.path));
    }

    /// 編集が 1 つも無いか。
    pub fn is_empty(&self) -> bool {
        self.changes.iter().all(|found| found.edits.is_empty())
    }
}

/// `text` に編集を当てる（テストと UI のプレビュー用）。`edits` は重ならないこと。
pub fn apply_edits(text: &str, edits: &[TextEdit]) -> String {
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by_key(|found| (found.range.start, found.range.end));
    let mut out = String::with_capacity(text.len());
    let mut at = 0usize;
    for edit in sorted {
        let start = edit.range.start.clamp(at, text.len());
        out.push_str(text.get(at..start).unwrap_or_default());
        out.push_str(&edit.new_text);
        at = edit.range.end.clamp(start, text.len());
    }
    out.push_str(text.get(at..).unwrap_or_default());
    out
}
