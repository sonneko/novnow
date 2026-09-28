//! Ropey を使ったテキストバッファとカーソル位置管理。

use crate::text::incremental::TokenIndex;
use crate::text::tokenizer::FileKind;
use ropey::Rope;
use std::path::PathBuf;
use std::time::SystemTime;

/// 1 ファイル分の編集バッファ。
///
/// * `rope`   — テキスト本体（Ropey）
/// * `tokens` — 行単位でインクリメンタル更新されるトークン索引
/// * `scratch`— egui の `TextEdit` が要求する `&mut String` を供給するための再利用バッファ
///
/// `scratch` は毎フレーム作り直さない。egui が書き込んだ内容をそのまま保持し、
/// Rope 側が外部要因で変わったときだけ `scratch_dirty` を立てて再生成する。
/// これにより「TextEdit の編集が Rope へ流れ、Rope の変更が TextEdit へ流れる」往復が成立する。
#[derive(Debug)]
pub struct TextBuffer {
    pub path: PathBuf,
    pub kind: FileKind,
    pub rope: Rope,
    pub tokens: TokenIndex,
    pub scratch: String,
    /// 直前フレームの TextEdit 内容と比較し、変更があれば Rope へ反映するための写し。
    pub last_seen: String,
    /// Rope → scratch の再生成が必要か。
    pub scratch_dirty: bool,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub dirty: bool,
    pub modified: Option<SystemTime>,
}

impl TextBuffer {
    pub fn new(path: PathBuf, kind: FileKind, text: String) -> Self {
        let rope = Rope::from_str(&text);
        let tokens = TokenIndex::build(kind, &rope);
        Self {
            path,
            kind,
            rope,
            tokens,
            scratch: text.clone(),
            last_seen: text,
            scratch_dirty: false,
            cursor_line: 0,
            cursor_col: 0,
            dirty: false,
            modified: None,
        }
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn char_count(&self) -> usize {
        self.rope.len_chars()
    }

    /// egui の `TextEdit` に渡す文字列参照（再利用バッファ）。
    ///
    /// Rope が外から更新された場合のみ scratch を再生成する。
    pub fn as_edit_str(&mut self) -> &mut String {
        if self.scratch_dirty {
            self.scratch = self.rope.to_string();
            self.last_seen = self.scratch.clone();
            self.scratch_dirty = false;
        }
        &mut self.scratch
    }

    /// TextEdit の編集結果（`scratch`）と Rope を差分同期する。
    ///
    /// 変更のあった行範囲だけトークンを再解析する。
    pub fn sync_from_scratch(&mut self) {
        if self.scratch == self.last_seen {
            return;
        }

        // 変更行レンジを比較で特定する。
        let (first, last) = diff_line_range(&self.last_seen, &self.scratch);

        self.rope = Rope::from_str(&self.scratch);
        self.last_seen.clear();
        self.last_seen.push_str(&self.scratch);

        self.tokens.retokenize_range(self.kind, &self.rope, first, last);
        self.dirty = true;
    }

    /// 数式バー（下部）での編集を Rope の該当行に反映する。
    ///
    /// Excel のセル編集と同じメンタルモデル: 1 行を確定編集する。
    pub fn apply_line_edit(&mut self, line_idx: usize, new_line: &str) {
        let line_idx = line_idx.min(self.rope.len_lines().saturating_sub(1));
        let start = self.rope.line_to_char(line_idx);
        let end = if line_idx + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line_idx + 1)
        } else {
            self.rope.len_chars()
        };

        let had_newline = self.rope.slice(start..end).to_string().ends_with('\n');
        let mut replacement = String::from(new_line);
        if had_newline && !replacement.ends_with('\n') {
            replacement.push('\n');
        }

        self.rope.remove(start..end);
        self.rope.insert(start, &replacement);
        self.tokens
            .retokenize_range(self.kind, &self.rope, line_idx, line_idx + 1);

        // TextEdit 側へも反映させる。
        self.scratch = self.rope.to_string();
        self.last_seen = self.scratch.clone();
        self.scratch_dirty = false;
        self.dirty = true;
    }

    /// 現在行のテキストを取り出す（末尾の改行は含まない）。
    pub fn line_text(&self, line_idx: usize) -> String {
        if line_idx >= self.rope.len_lines() {
            return String::new();
        }
        let start = self.rope.line_to_char(line_idx);
        let end = if line_idx + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line_idx + 1)
        } else {
            self.rope.len_chars()
        };
        self.rope
            .slice(start..end)
            .to_string()
            .trim_end_matches('\n')
            .to_string()
    }

    pub fn current_line_text(&self) -> String {
        self.line_text(self.cursor_line)
    }
}

/// 2 つのテキストの「変更された行範囲」を求める。
///
/// 前後の共通行を除いた内側を返す。複数行トークンが無いため、この範囲だけ再解析すれば十分。
fn diff_line_range(old: &str, new: &str) -> (usize, usize) {
    let old_lines: Vec<&str> = old.split('\n').collect();
    let new_lines: Vec<&str> = new.split('\n').collect();

    let mut start = 0usize;
    let max_prefix = old_lines.len().min(new_lines.len());
    while start < max_prefix && old_lines[start] == new_lines[start] {
        start += 1;
    }

    let mut end_old = old_lines.len();
    let mut end_new = new_lines.len();
    while end_old > start && end_new > start && old_lines[end_old - 1] == new_lines[end_new - 1] {
        end_old -= 1;
        end_new -= 1;
    }

    (start, end_new.max(start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_single_line() {
        let (a, b) = diff_line_range("one\ntwo\nthree", "one\nTWO\nthree");
        assert_eq!((a, b), (1, 2));
    }

    #[test]
    fn apply_line_edit_roundtrip() {
        let mut buf = TextBuffer::new(
            PathBuf::from("x.txt"),
            FileKind::Contents,
            "a\nb\nc\n".to_string(),
        );
        buf.apply_line_edit(1, "B");
        assert_eq!(buf.rope.to_string(), "a\nB\nc\n");
    }

    #[test]
    fn scratch_roundtrip() {
        let mut buf = TextBuffer::new(
            PathBuf::from("x.txt"),
            FileKind::Contents,
            "a\nb\n".to_string(),
        );
        // TextEdit が書き込んだ想定。
        buf.as_edit_str().push_str("c\n");
        buf.sync_from_scratch();
        assert_eq!(buf.rope.to_string(), "a\nb\nc\n");
    }
}
