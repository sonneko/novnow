//! 行単位のインクリメンタル再トークン化。
//!
//! 複数行トークンが無い前提のため、行 → トークンの対応は完全に閉じている。
//! 編集があった行レンジだけを差し替えれば、それ以外のトークンはそのまま維持できる。

use crate::text::tokenizer::{tokenize_line, FileKind, Token};
use ropey::Rope;

/// 行ごとのトークンと、全トークンのフラットな列を保持する索引。
#[derive(Debug, Clone, Default)]
pub struct TokenIndex {
    /// 行番号 → その行のトークン（通常 1 個）。
    pub by_line: Vec<Vec<Token>>,
    /// 全トークンのフラット列（情報パネル等での走査用）。
    pub flat: Vec<Token>,
}

impl TokenIndex {
    /// 全行を新規にトークン化する（初回ロード用）。
    pub fn build(kind: FileKind, rope: &Rope) -> Self {
        let mut idx = TokenIndex::default();
        let n = rope.len_lines();
        idx.by_line.reserve(n);
        for i in 0..n {
            let line = rope.line(i).to_string();
            let line = line.trim_end_matches('\n').to_string();
            let tok = tokenize_line(kind, i, &line);
            idx.flat.push(tok.clone());
            idx.by_line.push(vec![tok]);
        }
        idx
    }

    /// `[first_line, last_line)` の範囲だけ再トークン化して差し替える。
    ///
    /// 行数が変化した場合は後続の行番号がずれるため、`by_line` を組み直す。
    pub fn retokenize_range(&mut self, kind: FileKind, rope: &Rope, first_line: usize, last_line: usize) {
        let total = rope.len_lines();
        // 行数が変わった（行の挿入/削除）場合は全行を再構築する。
        if self.by_line.len() != total {
            *self = TokenIndex::build(kind, rope);
            return;
        }

        let first = first_line.min(total);
        let last = last_line.min(total).max(first);

        for i in first..last {
            let line = rope.line(i).to_string();
            let line = line.trim_end_matches('\n').to_string();
            let tok = tokenize_line(kind, i, &line);
            self.by_line[i] = vec![tok];
        }

        // flat を作り直す（トークン数は行数とほぼ同数なので軽い）。
        self.flat.clear();
        for line_tokens in &self.by_line {
            self.flat.extend(line_tokens.iter().cloned());
        }
    }

    pub fn tokens_at_line(&self, line: usize) -> &[Token] {
        self.by_line.get(line).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_retokenize() {
        let rope = Rope::from_str("@01-01-01\n地の文\n;\n");
        let mut idx = TokenIndex::build(FileKind::Contents, &rope);
        assert_eq!(idx.by_line.len(), 4);
        idx.retokenize_range(FileKind::Contents, &rope, 0, 2);
        assert_eq!(idx.flat.len(), 4);
    }
}
