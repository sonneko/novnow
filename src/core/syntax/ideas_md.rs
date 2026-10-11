//! `ideas.md`（§5.7）のパーサ。
//!
//! ```md
//! # I0007 学院長の動機
//! status: inbox            # inbox | keep | promoted | dropped
//! created: 2026-10-09T12:04:11+09:00
//! tags: 学院長, 動機
//! relates: @02, 'アルス
//! promoted-to: plots/02.txt#@02-03      # 昇格時のみ
//! 本文は自由記述。改行もそのまま保持する。
//! ```
//!
//! §5.6 と同じく「`characters.md` と同一の MD 方言」なので、行の分類は
//! [`super::characters_md`] のものをそのまま使う。木は H1 = 1 アイデアで、直下は
//! フィールド行と自由記述（本文）だけ。
//!
//! # ID（§5.7）
//!
//! H1 は `I` + 4 桁以上の ID + 空白 + タイトル。ID は `ideas.md` 内で最大値 +1 を採番し、
//! **再利用は禁止**（MUST）。採番そのものは編集操作（M8 の `idea` ワークスペース）の仕事で、
//! パーサは**形を読むだけ**と、同じ ID が 2 つあったら **E008**（§7「同一ファイル内の ID 重複」）
//! を出す。形が合わない ID（`# 7 動機` 等）は §7 に対応コードが無いので診断せず
//! [`Idea::valid_id`] が `false` になる（A 形式の B-39 と同じ流儀）。
//!
//! # 診断（M2 で出すもの）
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **W011** | §5.0.6 の表に無いフィールドキー（`status` `created` `relates` `promoted-to` は表に無い。`PLANS.md` §9.1 A-15） | キー |
//! | **E008** | 同一ファイル内の ID 重複（§5.7「再利用禁止」） | 2 件目の ID |
//!
//! 並びは**行順の W011** のあとに**アイデア順の E008**。
//!
//! # ① で読まないもの
//!
//! - `status: dropped` を既定で隠すのは UI（§10.4）の仕事。パーサは値を
//!   [`Ideas::status`] で返すだけにする。
//! - 「昇格」操作（`promoted-to` の書き戻し、§10.4）は M8。
//! - `# inbox | keep | promoted | dropped` のような**行末の `# ` コメントは ②**（§5.0.8）なので、
//!   ① では値の一部としてそのまま保持される（B 形式の B-36 と同じ扱い）。

use std::ops::Range;

use serde::Serialize;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::text::lexer::FieldKey;

use super::characters_md;
use super::plot_outline::{self, Field};

// MD 方言の行は `characters_md` のものが正（§5.6「同一の MD 方言」）。
// このモジュールだけで木を辿れるよう、同じ名前で公開し直す（B 形式が A 形式の
// `Field` 等を `pub use` したのと同じ流儀。PLANS.md §9.2 B-40）。
pub use super::characters_md::{Bullet, Heading, Line, Pattern, PatternLine};

/// ID のシジル（§5.7「H1 は `I` + 4 桁 ID + 空白 + タイトル」）。
pub const ID_SIGIL: char = 'I';

/// ID の最少桁数（§5.7）。`next_id` が 9999 を超えれば桁は増えるので**以上**。
pub const ID_DIGITS: usize = 4;

/// `ideas.md` 1 ファイル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ideas<'a> {
    /// 全行（書かれたまま）。[`Ideas::to_text`] はこれを LF で連結する。
    pub lines: Vec<Line<'a>>,
    /// 最初の H1 より前の行。
    pub preamble: Preamble,
    /// アイデア（H1 の順）。
    pub ideas: Vec<Idea<'a>>,
    /// 診断（§7）。位置は `text` の中の byte range。
    pub diagnostics: Vec<Diagnostic>,
}

/// `ideas.md` のテキストをパースする。
///
/// `text` は **LF 改行**であること（§5.0.4）。壊れた入力でも部分木と診断を返す
/// （`PLANS.md` §5.3 完了条件 3）。
pub fn parse(text: &str) -> Ideas<'_> {
    let mut lines: Vec<Line<'_>> = text.split('\n').map(characters_md::parse_line).collect();
    characters_md::attach_continuations(&mut lines);
    let mut diagnostics = Vec::new();
    let (preamble, ideas) = collect(&lines, &mut diagnostics);
    warn_duplicates(
        &ideas,
        &characters_md::line_offsets(&lines),
        &mut diagnostics,
    );
    Ideas {
        lines,
        preamble,
        ideas,
        diagnostics,
    }
}

impl<'a> Ideas<'a> {
    /// 入力と**バイト単位で一致する**テキストに戻す（`PLANS.md` §5.3 完了条件 2）。
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(line.raw());
        }
        out
    }

    /// `lines[index]` を含むアイデア。前文の行は `None`。
    pub fn idea_of(&self, index: usize) -> Option<&Idea<'a>> {
        self.ideas.iter().find(|found| found.span.contains(&index))
    }

    /// `lines[index]` が見出し行ならその中身。
    pub fn heading(&self, index: usize) -> Option<&Heading<'a>> {
        match self.lines.get(index) {
            Some(Line::Heading(found)) => Some(found),
            _ => None,
        }
    }

    /// `lines[index]` がフィールド行ならその中身。
    pub fn field(&self, index: usize) -> Option<&Field<'a>> {
        match self.lines.get(index) {
            Some(Line::Field(found)) => Some(found),
            _ => None,
        }
    }

    /// `index` 番目のアイデアの `key` のフィールド（出現順）。
    pub fn idea_fields(&self, index: usize, key: FieldKey) -> Vec<&Field<'a>> {
        match self.ideas.get(index) {
            Some(found) => pick(&self.lines, &found.fields, key),
            None => Vec::new(),
        }
    }

    /// `index` 番目のアイデアの `status`（§5.7 `inbox | keep | promoted | dropped`）。
    ///
    /// `status` は §5.0.6 の表に無いキーなので **W011** が出るが、値はそのまま読める
    /// （`PLANS.md` §9.1 A-15）。② の `# ` 行コメントが付いていれば値に含まれる（§5.0.8）。
    pub fn status(&self, index: usize) -> Option<&Field<'a>> {
        self.unknown_field(index, "status")
    }

    /// `index` 番目のアイデアの `promoted-to`（§5.7「昇格時のみ」）。
    pub fn promoted_to(&self, index: usize) -> Option<&Field<'a>> {
        self.unknown_field(index, "promoted-to")
    }

    /// 表に無いキー（[`FieldKey::Unknown`]）を**書かれたままのキー**で引く。
    ///
    /// `status` `created` `relates` `promoted-to` のように §5.0.6 の表に無いキーも値を
    /// 改変せずに保持する（縫い代 2）ので、読む手段が要る。
    pub fn unknown_field(&self, index: usize, key_text: &str) -> Option<&Field<'a>> {
        let found = self.ideas.get(index)?;
        characters_md::unknown_field(&self.lines, &found.fields, key_text)
    }

    /// `index` 番目のアイデアの本文（自由記述を LF で連結したもの、§5.7「改行もそのまま保持」）。
    pub fn body(&self, index: usize) -> String {
        let Some(found) = self.ideas.get(index) else {
            return String::new();
        };
        let parts: Vec<&str> = found
            .free
            .iter()
            .filter_map(|&line| match self.lines.get(line) {
                Some(Line::Text(text)) => Some(*text),
                _ => None,
            })
            .collect();
        parts.join("\n")
    }
}

/// 最初の H1 より前の行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preamble {
    /// 行範囲（`Ideas::lines` の中）。H1 が無ければ全行。
    pub span: Range<usize>,
    /// フィールド行の index（出現順）。
    pub fields: Vec<usize>,
}

/// 1 アイデア（§5.7）。`# I0007 タイトル` から次の `# ` まで。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Idea<'a> {
    /// H1 の行 index。
    pub heading: usize,
    /// ID（`I0007`）。空白で区切った 1 トークン目。
    pub id: &'a str,
    /// `id` の行内 byte 位置（**E008** の範囲に使う）。
    pub id_at: usize,
    /// ID の数字部分（`I0007` なら 7）。形が合わなければ `None`。
    pub number: Option<u32>,
    /// `id` が §5.7 の形（`I` + 4 桁以上）に合うか。
    pub valid_id: bool,
    /// タイトル（ID の後ろ）。無ければ空。
    pub title: &'a str,
    /// 行範囲。H1 を含み、次の H1 を含まない。
    pub span: Range<usize>,
    /// フィールド行の index（出現順）。
    pub fields: Vec<usize>,
    /// 自由記述（本文）の index（出現順）。
    pub free: Vec<usize>,
}

// ============================================================ 木と診断

/// 解析中のアイデア。
struct OpenIdea<'a> {
    /// H1 の行 index。
    heading: usize,
    /// ID。
    id: &'a str,
    /// `id` の行内 byte 位置。
    id_at: usize,
    /// タイトル。
    title: &'a str,
    /// フィールド行の index。
    fields: Vec<usize>,
    /// 自由記述の index。
    free: Vec<usize>,
}

/// 行の並びから前文とアイデアを作り、行順の診断（W011）を出す。
fn collect<'a>(lines: &[Line<'a>], out: &mut Vec<Diagnostic>) -> (Preamble, Vec<Idea<'a>>) {
    let head = lines
        .iter()
        .position(|line| matches!(line, Line::Heading(found) if found.level == 1));
    let mut preamble = Preamble {
        span: 0..head.unwrap_or(lines.len()),
        fields: Vec::new(),
    };
    let mut ideas: Vec<Idea<'a>> = Vec::new();
    let mut open: Option<OpenIdea<'a>> = None;
    let mut at = 0;
    for (index, line) in lines.iter().enumerate() {
        match line {
            // §5.7 に H2 / H3 は無いので、`## ` も `### ` もアイデアを閉じない
            // （行は [`Line::Heading`] のまま残るので round-trip は保たれる）
            Line::Heading(found) if found.level == 1 => {
                if let Some(closed) = open.take() {
                    ideas.push(finish_idea(closed, index));
                }
                let (id, title) = found.split_label();
                open = Some(OpenIdea {
                    heading: index,
                    id,
                    id_at: found.text_at,
                    title,
                    fields: Vec::new(),
                    free: Vec::new(),
                });
            }
            Line::Heading(_) => {}
            Line::Field(found) => {
                // §5.0.6「未知のキーは W011 とし、値は改変せず保持する」（行順）
                plot_outline::warn_unknown_key(found, at, out);
                match open.as_mut() {
                    Some(found) => found.fields.push(index),
                    None => preamble.fields.push(index),
                }
            }
            Line::Text(_) => {
                if let Some(found) = open.as_mut() {
                    found.free.push(index);
                }
            }
            Line::Bullet(_)
            | Line::Pattern(_)
            | Line::Continuation(_)
            | Line::Comment(_)
            | Line::Blank(_) => {}
        }
        at += line.raw().len() + 1;
    }
    if let Some(closed) = open.take() {
        ideas.push(finish_idea(closed, lines.len()));
    }
    (preamble, ideas)
}

/// アイデアを確定する。
fn finish_idea<'a>(open: OpenIdea<'a>, stop: usize) -> Idea<'a> {
    let OpenIdea {
        heading,
        id,
        id_at,
        title,
        fields,
        free,
    } = open;
    Idea {
        heading,
        id,
        id_at,
        number: id_number(id),
        valid_id: is_idea_id(id),
        title,
        span: heading..stop,
        fields,
        free,
    }
}

/// §5.7 の ID の形（`I` + 4 桁以上の数字）か。
fn is_idea_id(text: &str) -> bool {
    let Some(digits) = text.strip_prefix(ID_SIGIL) else {
        return false;
    };
    digits.len() >= ID_DIGITS && digits.bytes().all(|b| b.is_ascii_digit())
}

/// ID の数字部分（§5.7「最大値 +1 を採番」に使う）。形が合わなければ `None`。
fn id_number(text: &str) -> Option<u32> {
    if !is_idea_id(text) {
        return None;
    }
    text.strip_prefix(ID_SIGIL)?.parse().ok()
}

/// 同一 ID の 2 件目以降に **E008** を出す（§5.7「再利用禁止」、§7「同一ファイル内の ID 重複」）。
fn warn_duplicates(ideas: &[Idea<'_>], offsets: &[usize], out: &mut Vec<Diagnostic>) {
    for (index, found) in ideas.iter().enumerate() {
        let duplicate = ideas[..index].iter().any(|other| other.id == found.id);
        if duplicate {
            let start = offsets[found.heading] + found.id_at;
            out.push(Diagnostic::new(
                DiagnosticCode::E008,
                Some(start..start + found.id.len()),
                vec![Arg::text(found.id)],
            ));
        }
    }
}

/// `indices` の中から `key` のフィールド行を集める（A / B 形式の `pick` と同じ）。
fn pick<'s, 'a>(lines: &'s [Line<'a>], indices: &[usize], key: FieldKey) -> Vec<&'s Field<'a>> {
    indices
        .iter()
        .filter_map(|&index| match lines.get(index) {
            Some(Line::Field(found)) if found.key == key => Some(found),
            _ => None,
        })
        .collect()
}
