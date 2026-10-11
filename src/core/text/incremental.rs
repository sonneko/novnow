//! §6.7 のインクリメンタル再解析のフレーム（`PLANS.md` §5.2）。
//!
//! 役割は 1 点だけ: **1 行の編集でファイル全体を字句解析し直さない**。
//!
//! - 1 行編集 → その行だけ再字句解析（[`LineIndex::relex_line`] / [`LineIndex::relex_text`]）。
//! - 行の種別が変わり、**かつ**その種別が節番号（D）に影響するときだけ、その行以降の付番を
//!   再計算する（[`Renumber::From`]）。呼び出し側は O(ファイル行数) の単純ループを回すだけで、
//!   仕様のコスト目標は「10 万行で 2ms 以内」（§6.7）。
//! - 改行の挿入・削除（行数が変わる編集）は [`LineIndex::replace_lines`]。行番号と節番号の対応が
//!   ずれるので、種別が変わらなくても `at` 以降の再計算が要る。
//!
//! **core は時刻を持たない**（§3.2 で global clock が禁止）ので、「300ms アイドルで全体」
//! 「5 秒でプロジェクト全体」という**タイミングの判断はこのファイルに無い**。core は
//! 「何を再計算すべきか」を [`Renumber`] で返し、いつ回すかは UI / アダプタが決める。
//! 参照解決（M3）と診断（M4）も同じ形（「範囲を返す・回すのは外」）で足す。
//!
//! 節番号（D）そのものの**意味づけ**（`@A-B-C-D` の D が何を指すか、`;` との対応）は
//! M2 のパーサ（`core/syntax/*`）が持つ。ここにあるのは「どの行から付け直しが必要か」という
//! 無効化の判定だけで、§6.7 が M1 に求めているのもそこ（`PLANS.md` §5.2「行単位再解析の
//! フレーム」）。

use super::lexer::{classify, LineKind, TextFormat};
use super::rope::TextBuffer;

/// 1 行の字句的な種別（所有形）。
///
/// [`LineKind`] は行スライスへの借用なので行をまたいで保持できない。本文は rope
/// （[`TextBuffer`]）が持っているので複製せず、ここでは**種別だけ**を保持する。
/// §6.7 の「行の種別が変わった場合のみ再計算」の判定に要るのは種別だけで十分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineTag {
    /// `@` 構造見出し。
    Structure,
    /// `;` 節区切り。
    SectionBreak,
    /// 行頭コメント（`//`、② では `# ` も）。
    Comment,
    /// フィールド行（§5.0.5）。
    Field,
    /// 継続行（2 スペース以上のインデント）。
    Continuation,
    /// `! ` 挿絵メモ（**②**。[`super::lexer::Stage::One`] では作られない）。
    IllustrationMemo,
    /// 通常のテキスト行。
    Text,
}

impl LineTag {
    /// [`LineKind`] を種別だけに畳む。
    pub fn of(kind: &LineKind<'_>) -> Self {
        match kind {
            LineKind::Structure(_) => Self::Structure,
            LineKind::SectionBreak => Self::SectionBreak,
            LineKind::Comment { .. } => Self::Comment,
            LineKind::Field(_) => Self::Field,
            LineKind::Continuation { .. } => Self::Continuation,
            LineKind::IllustrationMemo(_) => Self::IllustrationMemo,
            LineKind::Text(_) => Self::Text,
        }
    }

    /// 節番号（D）の付番に影響するか。
    ///
    /// 影響するのは構造見出し（`@`）と節区切り（`;`）だけ。コメントやフィールド行の増減は
    /// 節番号を変えないので再計算を**起こさない**。§6.7 の「行の種別が変わった場合のみ」を
    /// 一歩詰めた形（`PLANS.md` §9.2 B-20）。
    pub fn affects_numbering(self) -> bool {
        matches!(self, Self::Structure | Self::SectionBreak)
    }
}

/// 節番号（D）の付番を再計算する必要がある範囲（§6.7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Renumber {
    /// 再計算は不要。
    #[default]
    NotNeeded,
    /// この行以降を再計算する。
    From(usize),
}

impl Renumber {
    /// 再計算が要るか。
    pub fn is_needed(self) -> bool {
        !matches!(self, Self::NotNeeded)
    }

    /// 再計算を始める行。不要なら `None`。
    pub fn from(self) -> Option<usize> {
        match self {
            Self::From(line) => Some(line),
            Self::NotNeeded => None,
        }
    }

    /// 2 つの結果をまとめる（先頭の行番号＝早い方を優先する）。
    ///
    /// 1 フレームに複数の編集が入ったとき、呼び出し側はこれで 1 本に束ねられる。
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::NotNeeded, other) | (other, Self::NotNeeded) => other,
            (Self::From(a), Self::From(b)) => Self::From(a.min(b)),
        }
    }
}

/// ファイル 1 本の字句状態を**行単位**で持つ（§6.7 のフレーム）。
///
/// 行数は rope の行数（LF のみで数える、§5.0.4）と一致させる。ずれた場合は
/// [`LineIndex::relex_text`] が安全側に倒して全文の再計算を要求する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    format: TextFormat,
    tags: Vec<LineTag>,
}

impl LineIndex {
    /// `text`（LF 区切り）の全文を字句解析する。
    pub fn new(text: &str, format: TextFormat) -> Self {
        let tags = text
            .split('\n')
            .map(|line| LineTag::of(&classify(line, format)))
            .collect();
        Self { format, tags }
    }

    /// [`TextBuffer`] から作る。全文を 1 つの `String` に複製せず、行ごとに取り出す。
    pub fn from_buffer(buffer: &TextBuffer, format: TextFormat) -> Self {
        let mut tags = Vec::with_capacity(buffer.len_lines());
        for line in 0..buffer.len_lines() {
            let text = buffer.line_content(line).unwrap_or_default();
            tags.push(LineTag::of(&classify(&text, format)));
        }
        Self { format, tags }
    }

    /// 字句解析の対象にした形式。
    pub fn format(&self) -> TextFormat {
        self.format
    }

    /// 行数（rope の行数と同じ）。
    pub fn len(&self) -> usize {
        self.tags.len()
    }

    /// 行が無いか。
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// `line` 行目の種別。範囲外なら `None`。
    pub fn tag(&self, line: usize) -> Option<LineTag> {
        self.tags.get(line).copied()
    }

    /// 全行の種別。
    pub fn tags(&self) -> &[LineTag] {
        &self.tags
    }

    /// 節番号の付番に関わる行（`@` と `;`）を `(行番号, 種別)` で先頭から列挙する。
    ///
    /// M2 のパーサはこれを使って D を付ける（§6.7）。全行を走査しないので、
    /// 見出しの少ない本文では O(節数) で済む。
    pub fn numbering_lines(&self) -> impl Iterator<Item = (usize, LineTag)> + '_ {
        self.tags
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, tag)| tag.affects_numbering())
    }

    /// `line` 行目を `text` で再字句解析する（§6.7 の「その行のみ再字句解析」）。
    ///
    /// 返り値が「その行以降の節番号を付け直すか」を表す。行数がずれていて `line` が範囲外の
    /// ときは、安全側に倒して [`Renumber::From(0)`]（全文の再計算）を返す。
    pub fn relex_text(&mut self, line: usize, text: &str) -> Renumber {
        let Some(slot) = self.tags.get_mut(line) else {
            return Renumber::From(0);
        };
        let previous = *slot;
        let next = LineTag::of(&classify(text, self.format));
        if previous == next {
            return Renumber::NotNeeded;
        }
        *slot = next;
        if previous.affects_numbering() || next.affects_numbering() {
            Renumber::From(line)
        } else {
            Renumber::NotNeeded
        }
    }

    /// [`TextBuffer`] の `line` 行目を取り直して再字句解析する（編集直後の通常経路）。
    pub fn relex_line(&mut self, buffer: &TextBuffer, line: usize) -> Renumber {
        let text = buffer.line_content(line).unwrap_or_default();
        self.relex_text(line, &text)
    }

    /// `at` 行目から `old_count` 行を `new_lines` で置き換える（改行の挿入・削除、貼り付け）。
    ///
    /// 行数が変わると行番号と節番号の対応がずれるので、種別が同じでも `at` 以降の再計算が要る。
    /// `at` が行数を超えていれば末尾に追加する扱いにする（パニックしない）。
    pub fn replace_lines(&mut self, at: usize, old_count: usize, new_lines: &[&str]) -> Renumber {
        let start = at.min(self.tags.len());
        let end = start.saturating_add(old_count).min(self.tags.len());
        let fresh = new_lines
            .iter()
            .map(|line| LineTag::of(&classify(line, self.format)));
        self.tags.splice(start..end, fresh);
        Renumber::From(start)
    }

    /// `at` 行目に 1 行挿入する（改行を打ったとき）。
    pub fn insert_line(&mut self, at: usize, text: &str) -> Renumber {
        self.replace_lines(at, 0, &[text])
    }

    /// `at` 行目を削除する（改行を消したとき）。
    pub fn remove_line(&mut self, at: usize) -> Renumber {
        self.replace_lines(at, 1, &[])
    }
}
