//! A 形式（アウトライン、§5.3）のパーサ。
//!
//! `plots/all.txt`（全体プロット）と `plots/AA.txt`（章プロット）は**同一形式**で、違いは
//! 見出しが指す階層だけである（§5.3 指示 11）。
//!
//! ```ebnf
//! outline_file   = [ preamble ] , { block } ;
//! preamble       = { field_line | comment_line | blank } ;
//! block          = outline_header , { field_line | note_line | comment_line | blank } , [ block_end ] ;
//! outline_header = ws? , "@" , partial_id , [ ws , title_text ] , [ comment ] , eol ;
//! block_end      = ws? , ";" , eol ;
//! note_line      = inline+ , eol ;
//! ```
//!
//! # 行とブロック
//!
//! [`Outline::lines`] は**書かれたままの 1 行ずつ**を持ち、[`Outline::to_text`] はそれを LF で
//! 連結するので入力とバイト単位で一致する（`docs/history.md` の完了条件 2）。[`Preamble`] と
//! [`Block`] は同じ行を **index で指すだけ**で、行の複製を持たない（C 形式の
//! [`super::contents::Section`] と同じ流儀）。
//!
//! # フィールドと未知キー
//!
//! 行がフィールド行かどうかの判定は字句器（§5.0.5 の 7 段の手順）に任せ、このパーサは
//! キーの表引き結果と値を木に載せる。§5.0.6 の表に無いキーは **W011** を出して**値を改変せずに
//! 保持する**（`docs/history.md` の作業項目 2「これが ② の生命線」）。② のキー（`when` `space`
//! `space-mode` `blurb` `senses` `illustration`）は ① の表に無いので、この経路でそのまま残る
//! （`tests/lexer.rs` の `aliases_of_stage_two_keys_are_not_in_the_table` が固定している）。
//!
//! 値が 2 行以上にわたる場合は継続行（§5.0.5）を使い、[`Field::extra`] に入る。
//! [`Field::full_value`] は 1 行目と継続行を LF で連結した値を返す。継続行のインデントは
//! **2 文字（[`crate::core::text::lexer::CONTINUATION_INDENT`]）だけ**落として相対的な字下げを
//! 残す（§5.3.6 の `space-mode: ascii` は「等幅・折り返しなし・整形なし」で表示する MUST なので
//! 形を保たねばならない。`docs/history.md` B-37）。
//!
//! # 診断（M2 で出すもの）
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **E002** | 見出しの ID が `partial_id` としてパースできない（§6.2.5） | `@id` |
//! | **E020** | 見出しの階層がこのファイルの許容と違う（§5.3.2） | `@id` |
//! | **E022** | `A = 00` を拒否する設定で 0 章の見出し（§5.1.2、第 2.8 版で E020 から分離） | `@id` |
//! | **E008** | 見出しの ID が同一ファイル内で重複（§5.3.2） | 2 件目の `@id` |
//! | **E018** | ブロックに `summary` が無い（§5.3.2 MUST） | 見出し行 |
//! | **W011** | 未知のフィールドキー（§5.0.6） | キー |
//! | **W019** | `summary` が 120 字超（§5.3.2） | 値 |
//!
//! 並びは**行の診断が行順、そのあとにブロック順の E018**（C 形式の W012 と同じ流儀）。
//!
//! # M2 で出さない診断
//!
//! - **W030**（プロットと本文の節数不一致、§5.4.2）: 2 ファイルを比べるので M4 の検査器。
//! - **W080**（`when` の時系列矛盾、§5.3.5）・**E062**（`space` の外部パス、§5.3.6）・
//!   **W083**（`senses` の偏り、§5.4.4）: いずれも ② のキーが絡むので ② の実装時。
//! - `state` / `length` の値が形に合わないとき: §7 に対応コードが無いので診断せず、
//!   [`Field::number`] が `None` を返す（`docs/history.md` B-39）。
//! - 参照（`'名前`）の実体が無い・モードが無い: 辞書が要るので M3（§6.2・§6.4）。
//!
//! # `.txt` 3 形式の共通部
//!
//! 行末コメント（§5.0.2）・行頭空白の byte 位置・`mode_id` / `char_name` の形（§5.5）は
//! A / B / C 形式で同じ規則なので、C 形式の [`super::contents`] のものをそのまま使う
//! （同じ規則を 2 箇所に書かないため）。共通部が増えたら `syntax` の直下に移す
//! （`docs/history.md` B-40）。

use std::ops::Range;

use serde::Serialize;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::id::{self, PartialSceneId, SceneId};
use crate::core::text::lexer::{self, FieldKey, LineKind, TextFormat};

use super::contents;

/// `summary` の上限字数（§5.3.2「1 行 ≤ 120 字」。超えると **W019**）。
const SUMMARY_MAX: usize = 120;

/// 参照シジル `'`（§6.2）。M2 は**形だけ**を読み、実体への解決は M3（§6.4）。
const REF_SIGIL: char = '\'';

/// 参照の修飾子を分けるコロン（§6.2.4 `modifier = ":" , ( mode_expr | material_title )`）。
const MODIFIER_SEP: char = ':';

/// §6.2.3 の明示形 `'{...}` の括弧。
const EXPLICIT_OPEN: char = '{';
/// §6.2.3 の明示形 `'{...}` の閉じ括弧。
const EXPLICIT_CLOSE: char = '}';

/// `seed:` / `payoff:` の矢印（§5.3.2 `F<id> 説明 -> @範囲`）。
const ARROW: &str = "->";

/// 解析の設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// このファイルの見出しの階層。`plots/all.txt` なら **1**、`plots/AA.txt` なら **2**。
    ///
    /// `None` なら**最初のブロックの見出し**から決める（§5.3.2「ファイル内の見出し階層は
    /// 揃っていること」）。どちらの場合も、揃わない見出しは **E020**。
    pub depth: Option<usize>,
    /// `A = 00`（0 章）を許すか（§5.1.2 `structure.allow_chapter_zero`）。
    ///
    /// `false` のとき、0 章を指す見出しは **E022**（予約番号、第 2.8 版。旧 E020）。
    pub allow_chapter_zero: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            depth: None,
            allow_chapter_zero: true,
        }
    }
}

/// A 形式 1 ファイル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outline<'a> {
    /// 全行（書かれたまま）。[`Outline::to_text`] はこれを LF で連結する。
    pub lines: Vec<Line<'a>>,
    /// 最初のブロック見出しより前の行（§5.3.1 `preamble`。ファイル全体のメタ）。
    pub preamble: Preamble,
    /// ブロック（§5.3.1 `block`）。見出しの順。
    pub blocks: Vec<Block>,
    /// 診断（§7）。位置は `text` の中の byte range。
    pub diagnostics: Vec<Diagnostic>,
}

/// A 形式のテキストをパースする（設定は [`Options::default`]）。
///
/// `text` は **LF 改行**であること（§5.0.4。読み込み時に
/// [`crate::core::text::lexer::normalize_on_load`] を通す）。
pub fn parse(text: &str) -> Outline<'_> {
    parse_with(text, &Options::default())
}

/// 設定を指定してパースする。
///
/// 壊れた入力でも部分木と診断を返す（§4.3-4、`docs/history.md` の完了条件 3）。ID が壊れた
/// 見出しもブロックを作り（[`Block::id`] が `None`）、`summary` の無いブロックは **E018**
/// になるだけで中身は保持する。
pub fn parse_with<'a>(text: &'a str, options: &Options) -> Outline<'a> {
    let mut lines: Vec<Line<'a>> = text.split('\n').map(parse_line).collect();
    attach_continuations(&mut lines);
    let mut diagnostics = Vec::new();
    let (preamble, blocks) = collect(&lines, options, &mut diagnostics);
    Outline {
        lines,
        preamble,
        blocks,
        diagnostics,
    }
}

impl<'a> Outline<'a> {
    /// 入力と**バイト単位で一致する**テキストに戻す（`docs/history.md` の完了条件 2）。
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

    /// `lines[index]` を含むブロック。前文の行とブロック外の行は `None`。
    pub fn block_of(&self, index: usize) -> Option<&Block> {
        self.blocks.iter().find(|block| block.span.contains(&index))
    }

    /// `lines[index]` が見出し行ならその中身。
    pub fn header(&self, index: usize) -> Option<&HeaderLine<'a>> {
        match self.lines.get(index) {
            Some(Line::Header(found)) => Some(found),
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

    /// `index` 番目のブロックの `key` のフィールド（出現順）。
    ///
    /// 0..1 のキー（`summary` `goal` `state` など）でも複数書かれていれば全部返す。
    /// 重複の検査は §5.3.2 の表の「回数」列のとおりに M4 の検査器が行う。
    pub fn fields_of(&self, index: usize, key: FieldKey) -> Vec<&Field<'a>> {
        let Some(block) = self.blocks.get(index) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &line in &block.fields {
            if let Some(found) = self.field(line) {
                if found.key == key {
                    out.push(found);
                }
            }
        }
        out
    }

    /// `index` 行目のフィールドが占める行範囲（1 行目 + 継続行）。
    pub fn field_span(&self, index: usize) -> Option<Range<usize>> {
        let found = self.field(index)?;
        Some(index..index + found.line_count())
    }
}

/// A 形式の 1 行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Line<'a> {
    /// 見出し行（§5.3.1 `outline_header`）。
    Header(HeaderLine<'a>),
    /// フィールド行（§5.0.5）。継続行は [`Field::extra`] に入る。
    Field(Field<'a>),
    /// 継続行（§5.0.5）。直前がフィールド行ならその値の続き。
    Continuation(Continuation<'a>),
    /// 自由記述（§5.3.1 `note_line`）。フィールドに当てはまらない行。
    Note(&'a str),
    /// 行コメント（`//`、§5.0.2）。`# ` は ② なので ① では [`Line::Note`] になる。
    Comment(contents::CommentLine<'a>),
    /// 空行（空白だけの行を含む。`docs/history.md` B-34）。
    Blank(&'a str),
    /// ブロックの終わり `;`（§5.3.1 `block_end`）。
    Break(contents::BreakLine<'a>),
}

impl<'a> Line<'a> {
    /// 書かれたままの 1 行。
    pub fn raw(&self) -> &'a str {
        match self {
            Self::Header(found) => found.raw,
            Self::Field(found) => found.raw,
            Self::Continuation(found) => found.raw,
            Self::Note(found) => found,
            Self::Comment(found) => found.raw,
            Self::Blank(found) => found,
            Self::Break(found) => found.raw,
        }
    }

    /// 空行か（空白だけの行を含む）。
    pub fn is_blank(&self) -> bool {
        matches!(self, Self::Blank(_))
    }
}

/// 最初のブロック見出しより前の行（§5.3.1 `preamble`）。ファイル全体のメタ。
///
/// §5.3.7 は `plots/all.txt` の前文の `blurb:` を**作品全体のあらすじ**とする。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preamble {
    /// 行範囲（`Outline::lines` の中）。見出しが無ければ全行。
    pub span: Range<usize>,
    /// フィールド行の index（出現順）。
    pub fields: Vec<usize>,
}

/// 1 ブロック（§5.3.1 `block`）。章（`all.txt`）または話（`AA.txt`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Block {
    /// 見出し行の index（`Outline::lines` の中）。
    pub header: usize,
    /// 見出しの ID。省略された階層は 0（[`PartialSceneId::expand_low`]）。
    /// 形が壊れていれば `None`（**E002**）。
    pub id: Option<SceneId>,
    /// 書かれた階層の数（`@01` = 1、`@01-04` = 2）。壊れていれば 0。
    pub depth: usize,
    /// ブロックの行範囲。見出し行を含み、`;` があればそれも含む。次の見出しは含まない。
    pub span: Range<usize>,
    /// `;` の行の index。閉じられていなければ `None`。
    pub end: Option<usize>,
    /// フィールド行の index（出現順）。
    pub fields: Vec<usize>,
    /// 自由記述（`note_line`）の index（出現順）。
    pub notes: Vec<usize>,
}

/// 見出し行（§5.3.1 `outline_header`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeaderLine<'a> {
    /// 書かれたままの 1 行。
    pub raw: &'a str,
    /// 先頭の空白文字数（`ws?`）。
    pub indent: usize,
    /// `@` の byte 位置。
    pub sigil_at: usize,
    /// `@` の後ろ、最初の空白まで（`partial_id`）。
    pub id_text: &'a str,
    /// パースできた ID。省略された階層は 0。形が壊れていれば `None`（**E002**）。
    pub id: Option<SceneId>,
    /// 書かれた階層の数。壊れていれば 0。
    pub depth: usize,
    /// タイトル（`title_text`）。無ければ空。
    pub title: &'a str,
    /// 行末コメント（§5.0.2）。
    pub comment: Option<contents::Comment<'a>>,
}

/// フィールド行（§5.0.5）と、その継続行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field<'a> {
    /// 書かれたままの 1 行目。
    pub raw: &'a str,
    /// 先頭の空白文字数（`ws?`）。
    pub indent: usize,
    /// 表引きしたキー（§5.0.6）。表に無ければ [`FieldKey::Unknown`]（**W011**）。
    pub key: FieldKey,
    /// **書かれたままの**キー（日本語別名と未知キーを保持するため。§5.0.6「保存時に
    /// 正準形へ正規化しない」）。
    pub key_text: &'a str,
    /// 1 行目の値（前後の空白を除く）。`key:` だけの行は空文字列。
    pub value: &'a str,
    /// `value` の**行内の byte 位置**（[`crate::core::text::lexer::FieldLine::value_at`]）。
    pub value_at: usize,
    /// 継続行の寄与（§5.0.5）。各行のインデントから 2 文字落とした残り（B-37）。
    pub extra: Vec<&'a str>,
}

impl<'a> Field<'a> {
    /// 継続行まで含めた値（LF 連結）。
    pub fn full_value(&self) -> String {
        let mut out = self.value.to_owned();
        for text in &self.extra {
            out.push('\n');
            out.push_str(text);
        }
        out
    }

    /// このフィールドが占める行数（1 行目 + 継続行）。
    pub fn line_count(&self) -> usize {
        1 + self.extra.len()
    }

    /// §5.0.6 の表に無いキーか（**W011**）。
    pub fn is_unknown(&self) -> bool {
        self.key == FieldKey::Unknown
    }

    /// 値の中の参照トークン（§6.2）。`cast` / `pov` / `material` / `mode` の値に使う。
    ///
    /// 位置は**値の先頭からの byte**。行内の位置にするには [`Field::value_at`] を足す。
    pub fn refs(&self) -> Vec<CharRef<'a>> {
        ref_tokens(self.value)
    }

    /// 値が整数ならその値（§5.3.2 `length`）。形が合わなければ `None`（診断は出さない、B-39）。
    pub fn number(&self) -> Option<i64> {
        self.value.parse().ok()
    }

    /// `seed:` / `payoff:` の値の形（§5.3.2）。値が空なら `None`。
    pub fn foreshadow(&self) -> Option<Foreshadow<'a>> {
        if self.value.is_empty() {
            return None;
        }
        Some(split_foreshadow(self.value))
    }
}

/// 継続行（§5.0.5「値は 1 行。複数行が必要な場合は継続行として 2 スペース以上のインデント」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Continuation<'a> {
    /// 書かれたままの 1 行。
    pub raw: &'a str,
    /// 先頭の空白文字数（2 以上）。
    pub indent: usize,
    /// インデントを除いた残り（字句器の報告そのまま）。
    pub text: &'a str,
}

/// 参照トークン（§6.2）。`'名前[:修飾子]`。
///
/// M2 は**形だけ**を読む。名前を `characters.md` / `words.md` の実体に解決し、修飾子が
/// モード式（`mode_expr`）か資料タイトル（`material_title`）かを決めるのは M3（§6.2.4・§6.4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CharRef<'a> {
    /// 値の先頭からの byte 位置（`'` の位置）。
    pub at: usize,
    /// 値の先頭からの byte 範囲（`'` からトークンの終わりまで）。
    pub range: Range<usize>,
    /// 名前（`'` と明示形の波括弧を除く。§5.5 `char_name` = 空白を含まない 1 字以上）。
    pub name: &'a str,
    /// §6.2.3 の明示形 `'{...}` だったか。
    pub explicit: bool,
    /// `:` の後ろ（§6.2.4 `modifier`）。無ければ `None`。
    pub modifier: Option<&'a str>,
}

impl<'a> CharRef<'a> {
    /// 修飾子が `mode_expr`（§6.2.4 `mode_id , [ "+" , mode_id ]`）の形なら各モード ID。
    ///
    /// 形が合わなければ `None`。資料タイトル（`'M0137:旧校舎の構造メモ`）もここに入る。
    pub fn modes(&self) -> Option<Vec<&'a str>> {
        contents::split_modes(self.modifier?)
    }
}

/// `seed:` / `payoff:` の値の形（§5.3.2 `F<id> 説明 -> @範囲`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Foreshadow<'a> {
    /// 伏線 ID（`F01`）。
    pub tag: &'a str,
    /// 説明。`payoff` では普通は空。
    pub note: &'a str,
    /// 設置・回収の場所（`@03-02`）。`->` が無ければ空。
    pub target: &'a str,
}

// ============================================================ 行の分解

/// 1 行を字句解析して木の行にする。
///
/// 診断は出さない（見出しの階層と ID の重複は**ファイルを跨いで**決まるので
/// [`collect`] が出す）。
fn parse_line(raw: &str) -> Line<'_> {
    // §5.3.1 `blank`。空白の文字数は問わない（B-34。保存時に末尾空白は落ちる、§17.1）
    if raw.trim_matches(lexer::is_space).is_empty() {
        return Line::Blank(raw);
    }
    // ①（[`Stage::One`]）で A 形式の行を読む。`! ` は C 形式専用（§5.2.5）なので現れない
    match lexer::classify_at_stage(raw, TextFormat::PlotOutline, lexer::Stage::One) {
        LineKind::Structure(_) => Line::Header(parse_header(raw)),
        LineKind::SectionBreak => Line::Break(contents::parse_break(raw)),
        LineKind::Comment { .. } => Line::Comment(contents::parse_comment(raw)),
        LineKind::Field(found) => Line::Field(parse_field(raw, found)),
        LineKind::Continuation { indent, text } => {
            Line::Continuation(Continuation { raw, indent, text })
        }
        // フィールドに当てはまらない自由記述（§5.3.1 `note_line`）。一字下げも本文
        LineKind::Text(_) | LineKind::IllustrationMemo(_) => Line::Note(raw),
    }
}

/// 見出し行（§5.3.1 `outline_header`）。
fn parse_header(raw: &str) -> HeaderLine<'_> {
    let indent = lexer::leading_indent(raw);
    let sigil_at = contents::body_start(raw);
    let comment = contents::find_comment(raw);
    // `@` から（行末コメントがあればその手前まで）が見出しの中身
    let head = match &comment {
        Some(found) => &raw[sigil_at..found.range.start],
        None => &raw[sigil_at..],
    };
    let after = &head[id::SIGIL.len_utf8()..];
    let id_len = after.find(lexer::is_space).unwrap_or(after.len());
    let (id, depth) = parse_header_id(&after[..id_len]);
    HeaderLine {
        raw,
        indent,
        sigil_at,
        id_text: &after[..id_len],
        id,
        depth,
        title: after[id_len..].trim_matches(lexer::is_space),
        comment,
    }
}

/// `partial_id` をパースする（§5.1）。
///
/// C 形式と違い、**階層数は問わない**（`@01` も `@01-04` も形としては読める）。このファイルの
/// 許容する階層との違いは [`check_header`] が **E020** で報告する（§5.3.2）。
fn parse_header_id(text: &str) -> (Option<SceneId>, usize) {
    match PartialSceneId::parse(text) {
        Ok(found) => (Some(found.expand_low()), found.depth()),
        Err(_) => (None, 0),
    }
}

/// フィールド行（§5.0.5）。継続行は [`attach_continuations`] が後から付ける。
///
/// A / B 形式で同じ形なので B 形式（[`super::plot_episode`]）もこれを使う（B-40）。
pub(crate) fn parse_field<'a>(raw: &'a str, found: lexer::FieldLine<'a>) -> Field<'a> {
    Field {
        raw,
        indent: found.indent,
        key: found.key,
        key_text: found.key_text,
        value: found.value,
        value_at: found.value_at,
        extra: Vec::new(),
    }
}

/// フィールド行の直後の継続行を、そのフィールドの値に付ける（§5.0.5）。
///
/// 直前がフィールド行（またはその継続行）でなければ、継続行はどのフィールドにも属さない
/// （[`Line::Continuation`] のまま残るので round-trip は保たれる。自由記述にはしない）。
/// 字句器は「インデント 2 以上の行」を報告するだけで文脈を知らないので、ここで決める（B-37）。
fn attach_continuations(lines: &mut [Line<'_>]) {
    let mut owner: Option<usize> = None;
    for index in 0..lines.len() {
        if let Line::Field(_) = &lines[index] {
            owner = Some(index);
            continue;
        }
        let Some(text) = continuation_text(&lines[index]) else {
            // 空行・コメント・自由記述は見出しと同じく継続を中断する（B-37）
            owner = None;
            continue;
        };
        if let Some(at) = owner {
            push_extra(&mut lines[at], text);
        }
    }
}

/// 継続行なら、値に寄与するテキスト（インデントから 2 文字落とした残り）。それ以外は `None`。
fn continuation_text<'a>(line: &Line<'a>) -> Option<&'a str> {
    match line {
        Line::Continuation(found) => Some(drop_indent(found.raw)),
        _ => None,
    }
}

/// 継続行のインデントから **先頭 1 タブまたは 2 文字だけ**落とした残り（B-21・B-37）。
///
/// §5.3.6 の `space-mode: ascii` は「整形なし」で形を保つ MUST なので、それ以上の字下げは
/// 相対的な形として残す。先頭 1 タブまたは 2 文字は「値の続き」を示す印なので落とす。
pub(crate) fn drop_indent(raw: &str) -> &str {
    if let Some(rest) = raw.strip_prefix('\t') {
        return rest;
    }
    match raw.char_indices().nth(lexer::CONTINUATION_INDENT) {
        Some((at, _)) => &raw[at..],
        None => "",
    }
}

/// フィールド行に継続行のテキストを 1 行分足す。
fn push_extra<'a>(line: &mut Line<'a>, text: &'a str) {
    if let Line::Field(found) = line {
        found.extra.push(text);
    }
}

// ============================================================ ブロックと診断

/// 解析中のブロック。
struct Open {
    /// 見出し行の index。
    header: usize,
    /// 見出し行の byte 位置（E018 の範囲に使う）。
    header_at: usize,
    /// 見出しの ID。
    id: Option<SceneId>,
    /// 見出しの階層。
    depth: usize,
    /// フィールド行の index。
    fields: Vec<usize>,
    /// 自由記述の index。
    notes: Vec<usize>,
}

/// 行の並びから前文とブロックを作り、診断を出す。
///
/// 診断の並びは**行順**（E002 / E020 / E008 / W011 / W019）のあとに**ブロック順の E018**。
fn collect(
    lines: &[Line<'_>],
    options: &Options,
    out: &mut Vec<Diagnostic>,
) -> (Preamble, Vec<Block>) {
    // §5.3.1 `outline_file = [ preamble ] , { block }`。見出しより前だけが前文
    let head = lines
        .iter()
        .position(|line| matches!(line, Line::Header(_)));
    let mut preamble = Preamble {
        span: 0..head.unwrap_or(lines.len()),
        fields: Vec::new(),
    };
    let mut blocks = Vec::new();
    let mut open: Option<Open> = None;
    let mut expected = options.depth;
    let mut seen: Vec<(SceneId, usize)> = Vec::new();
    let mut missing: Vec<(usize, usize)> = Vec::new();
    let mut at = 0;
    for (index, line) in lines.iter().enumerate() {
        match line {
            Line::Header(found) => {
                if let Some(closed) = open.take() {
                    finish(closed, index, None, lines, &mut blocks, &mut missing);
                }
                check_header(found, at, &mut expected, &mut seen, options, out);
                open = Some(Open {
                    header: index,
                    header_at: at,
                    id: found.id,
                    depth: found.depth,
                    fields: Vec::new(),
                    notes: Vec::new(),
                });
            }
            Line::Break(_) => {
                // §5.3.1 `block_end`。見出しより前の `;` は何もしない（C 形式の B-32 と同じ）
                if let Some(closed) = open.take() {
                    finish(
                        closed,
                        index + 1,
                        Some(index),
                        lines,
                        &mut blocks,
                        &mut missing,
                    );
                }
            }
            Line::Field(_) => {
                if let Some(found) = &mut open {
                    found.fields.push(index);
                } else if index < preamble.span.end {
                    preamble.fields.push(index);
                }
                // ブロックの外（`;` のあと、次の見出しより前）はどのブロックにも属さない（B-38）
                warn_field(line, at, out);
            }
            Line::Note(_) => {
                if let Some(found) = &mut open {
                    found.notes.push(index);
                }
            }
            Line::Continuation(_) | Line::Comment(_) | Line::Blank(_) => {}
        }
        at += line.raw().len() + 1;
    }
    if let Some(closed) = open.take() {
        finish(closed, lines.len(), None, lines, &mut blocks, &mut missing);
    }
    for &(index, offset) in &missing {
        warn_missing_summary(lines, index, offset, out);
    }
    (preamble, blocks)
}

/// ブロックを閉じて `blocks` に積む。`summary` が無ければ E018 の位置を `missing` に記録する。
fn finish(
    open: Open,
    stop: usize,
    end: Option<usize>,
    lines: &[Line<'_>],
    blocks: &mut Vec<Block>,
    missing: &mut Vec<(usize, usize)>,
) {
    let block = Block {
        header: open.header,
        id: open.id,
        depth: open.depth,
        span: open.header..stop,
        end,
        fields: open.fields,
        notes: open.notes,
    };
    if !has_summary(lines, &block.fields) {
        missing.push((block.header, open.header_at));
    }
    blocks.push(block);
}

/// ブロックに `summary` のフィールドがあるか（§5.3.2 MUST。無ければ **E018**）。
fn has_summary(lines: &[Line<'_>], fields: &[usize]) -> bool {
    for &index in fields {
        if let Some(Line::Field(found)) = lines.get(index) {
            if found.key == FieldKey::Summary {
                return true;
            }
        }
    }
    false
}

/// `summary` が無いブロックを **E018** で報告する（§5.3.2、quickfix「空の summary を挿入」）。
fn warn_missing_summary(
    lines: &[Line<'_>],
    index: usize,
    offset: usize,
    out: &mut Vec<Diagnostic>,
) {
    let Some(line) = lines.get(index) else {
        return;
    };
    let range = offset..offset + line.raw().len();
    out.push(Diagnostic::simple(DiagnosticCode::E018, Some(range)));
}

/// 見出し行の診断（**E002** / **E020** / **E022** / **E008**）。
///
/// `expected` はこのファイルの階層で、[`Options::depth`] が `None` のときは最初の見出しで決まる
/// （§5.3.2「ファイル内の見出し階層は揃っていること」）。
fn check_header(
    found: &HeaderLine<'_>,
    at: usize,
    expected: &mut Option<usize>,
    seen: &mut Vec<(SceneId, usize)>,
    options: &Options,
    out: &mut Vec<Diagnostic>,
) {
    let start = at + found.sigil_at;
    let end = start + id::SIGIL.len_utf8() + found.id_text.len();
    let id_range = start..end;
    let Some(id) = found.id else {
        // §6.2.5「行頭（空白のみ先行）→ 構造見出し。`scene_id` としてパース → **E002**」
        out.push(Diagnostic::simple(DiagnosticCode::E002, Some(id_range)));
        return;
    };
    let depth = *expected.get_or_insert(found.depth);
    if depth != found.depth {
        // §5.3.2「ファイル内の見出し階層は揃っていること」
        let args = vec![Arg::number(depth as i64)];
        out.push(Diagnostic::new(
            DiagnosticCode::E020,
            Some(id_range.clone()),
            args,
        ));
    }
    if id.chapter == 0 && !options.allow_chapter_zero {
        // §5.1.2 の `A = 00`（予約番号は E022、第 2.8 版。`docs/history.md` A-9）
        let args = vec![Arg::text(found.id_text)];
        out.push(Diagnostic::new(
            DiagnosticCode::E022,
            Some(id_range.clone()),
            args,
        ));
    }
    if seen.contains(&(id, found.depth)) {
        // §5.3.2「ブロックの見出し ID は同一ファイル内で一意」
        let args = vec![Arg::text(found.id_text)];
        out.push(Diagnostic::new(DiagnosticCode::E008, Some(id_range), args));
    }
    seen.push((id, found.depth));
}

/// フィールド行の診断（**W011** 未知のキー、**W019** 長すぎる `summary`）。
fn warn_field(line: &Line<'_>, at: usize, out: &mut Vec<Diagnostic>) {
    let Line::Field(found) = line else {
        return;
    };
    warn_unknown_key(found, at, out);
    if found.key != FieldKey::Summary {
        return;
    }
    // §5.3.2「`summary` は 1 行 ≤ 120 字」。継続行まで含めた字数で数える（B-38）
    let chars = found.full_value().chars().count();
    if chars > SUMMARY_MAX {
        let start = at + found.value_at;
        let args = vec![Arg::number(chars as i64)];
        let range = start..start + found.value.len();
        out.push(Diagnostic::new(DiagnosticCode::W019, Some(range), args));
    }
}

/// §5.0.6「未知のキーは W011 とし、値は改変せず保持する」。
///
/// A / B 形式で同じ規則なので B 形式（[`super::plot_episode`]）もこれを呼ぶ（B-40）。
/// `offset` はこの行の先頭の byte 位置。
pub(crate) fn warn_unknown_key(found: &Field<'_>, offset: usize, out: &mut Vec<Diagnostic>) {
    if !found.is_unknown() {
        return;
    }
    let start = offset + contents::body_start(found.raw);
    let args = vec![Arg::text(found.key_text)];
    let range = start..start + found.key_text.len();
    out.push(Diagnostic::new(DiagnosticCode::W011, Some(range), args));
}

// ============================================================ 値の形

/// 値の中の参照トークンを拾う（§6.2）。
///
/// `'` は**トークンの先頭**にあるものだけ（空白区切りで切る）。`material: 'M0137 補足` の
/// 「補足」のように `'` の無い語は参照ではない。
fn ref_tokens(value: &str) -> Vec<CharRef<'_>> {
    let mut out = Vec::new();
    let mut start = 0;
    for (at, ch) in value.char_indices() {
        if !lexer::is_space(ch) {
            continue;
        }
        push_ref(value, &mut out, start, at);
        // 区切りの空白は 1 文字とは限らない（全角空白は 3 byte）
        start = at + ch.len_utf8();
    }
    push_ref(value, &mut out, start, value.len());
    out
}

/// `value[start..end]` が参照トークンなら `out` に積む。空トークンは飛ばす。
fn push_ref<'a>(value: &'a str, out: &mut Vec<CharRef<'a>>, start: usize, end: usize) {
    if start >= end {
        return;
    }
    if let Some(found) = split_ref(&value[start..end], start) {
        out.push(found);
    }
}

/// 1 トークンから参照を読む。`'` で始まっていなければ `None`。
fn split_ref(token: &str, at: usize) -> Option<CharRef<'_>> {
    let rest = token.strip_prefix(REF_SIGIL)?;
    let (head, modifier) = match rest.split_once(MODIFIER_SEP) {
        Some((head, tail)) => (head, Some(tail)),
        None => (rest, None),
    };
    let (name, explicit) = explicit_name(head);
    if !contents::is_char_name(name) {
        return None;
    }
    Some(CharRef {
        at,
        range: at..at + token.len(),
        name,
        explicit,
        modifier,
    })
}

/// §6.2.3 の明示形 `'{...}` を剥ぐ。閉じ括弧が無ければ明示形にしない（診断は M3）。
fn explicit_name(text: &str) -> (&str, bool) {
    let Some(rest) = text.strip_prefix(EXPLICIT_OPEN) else {
        return (text, false);
    };
    match rest.strip_suffix(EXPLICIT_CLOSE) {
        Some(name) => (name, true),
        None => (text, false),
    }
}

/// `seed:` / `payoff:` の値を分ける（§5.3.2 `F<id> 説明 -> @範囲`）。
///
/// `->` が無ければ `target` は空。形の検査（`F` の後ろが数字か等）は伏線を追跡する M3 が行う。
fn split_foreshadow(value: &str) -> Foreshadow<'_> {
    let (head, target) = match value.find(ARROW) {
        Some(at) => {
            let rest = &value[at + ARROW.len()..];
            (&value[..at], rest.trim_matches(lexer::is_space))
        }
        None => (value, ""),
    };
    let (tag, note) = split_at_space(head);
    Foreshadow {
        tag,
        note: note.trim_matches(lexer::is_space),
        target,
    }
}

/// 最初の空白で 2 つに分ける（空白が無ければ 2 つ目は空）。
fn split_at_space(text: &str) -> (&str, &str) {
    match text.find(lexer::is_space) {
        Some(at) => text.split_at(at),
        None => (text, ""),
    }
}
