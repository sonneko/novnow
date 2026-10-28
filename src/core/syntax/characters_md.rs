//! `characters.md`（§5.5）のパーサ。
//!
//! ```ebnf
//! characters_md = [ file_preamble ] , { character } ;
//! character     = "# " , char_name , eol , { field_line | section } ;
//! section       = "## " , section_key , eol , section_body ;
//! section_key   = "modes" | "attributes" | "tone" | "relations" | "notes"
//!               | 「モード」|「口調」|「関係」|「メモ」;   (* §5.0.6 の別名のうち 4 語 *)
//! section_body  = { field_line | bullet | pattern_line | free_line | blank | comment_line } ;
//! mode_def      = "### " , mode_id , [ ws , mode_label ] , eol , { field_line | pattern_line | free_line } ;
//! mode_id       = /[A-Za-z][A-Za-z0-9_]{0,15}/ ;
//! pattern_line  = ws? , "「" , pattern , "」" , { ws? , "「" , pattern , "」" } , eol ;
//! ```
//!
//! # 意味を持つ行は 4 種だけ（§5.5.1）
//!
//! 1. `# 見出し`（H1 = キャラ、H2 = セクション、H3 = モード）
//! 2. フィールド行 `key: value`（§5.0.5。判定は字句器に任せる）
//! 3. 箇条書き `- ...`
//! 4. 鉤括弧行 `「...」`（役割語パターン、§5.5.5）
//!
//! それ以外は**直前のフィールド／セクションに属する自由記述**（`note` 相当）として
//! [`Line::Text`] に載せるだけで、木は内容を解釈しない。「曖昧さのない MD」の代償として、
//! 書かれたものは必ずそのまま残る（§5.5.1）。`#### ` 以上の見出しも 4 種に無いので自由記述。
//!
//! 字句規則が先に効くので次も 4 種には**入らない**（いずれも行はそのまま保持される）:
//!
//! - **インデント 2 以上の行は継続行**（§5.0.5）。`  - 入れ子の箇条書き` は箇条書きではなく
//!   継続行になり、直前がフィールド行ならその値の一部になる。
//! - `#恩田恵`（`#` の直後に空白が無い）と `#### 深い見出し`（§5.5.1 が数えない階層）。
//! - `-5`（`-` の直後に空白が無い。§5.0.2 が `#1 位` を誤認しないのと同じ理由）。
//! - 閉じない `「` や、最後の `」` の後ろに空白以外が残る行（§5.5.2 `pattern_line`）。
//!
//! 木に載らない行もある（`## ` が H1 の外にある、`### ` が `## ` の外にある、H1 直下の
//! 箇条書き）。§5.5.2 の文法にそれらの並びが無いからで、**行としては残る**ので round-trip は
//! 保たれる。`### ` の外に出た `range:` 等のフィールド行はキャラクタ直下に入れて値を失わない。
//!
//! # 行と木
//!
//! [`Characters::lines`] は**書かれたままの 1 行ずつ**を持ち、[`Characters::to_text`] はそれを
//! LF で連結するので入力とバイト単位で一致する（`docs/history.md` の完了条件 2）。
//! [`Character`] / [`Section`] / [`ModeDef`] は同じ行を **index で指すだけ**で複製を持たない
//! （A 形式の [`super::plot_outline::Block`] と同じ流儀）。
//!
//! # MD 方言の共通部
//!
//! `words.md`（§5.6）は「`characters.md` と同一の MD 方言を使う」ので、行の分類
//! （[`Line`] / [`Heading`] / [`Bullet`] / [`PatternLine`] / [`parse_line`] /
//! [`attach_continuations`]）は**このファイルが持ち**、`words_md` / `ideas_md` / `prompt_md` が
//! `pub(crate)` で共有する。A / B 形式が `contents.rs` の共通部を共有したのと同じ流儀
//! （`docs/history.md` B-40: §3.3 のツリーに共通ファイルが無いので移設はしない）。
//! フィールド行まわり（[`Field`] / [`Continuation`]）は A 形式のものが正。
//!
//! # 診断（M2 で出すもの）
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **W011** | §5.0.6 の表に無いフィールドキー（§5.0.6「値は改変せず保持」） | キー |
//! | **E002** | `range:` の値が範囲式としてパースできない（§5.1） | 値 |
//! | **E016** | `range:` の左端 > 右端（§5.1 の境界検査） | 値 |
//! | **E017** | 同一キャラ内の同一 `mode_id` で `range` が重なる（§5.5.4） | 2 件目の `range`（無ければ見出しの ID） |
//!
//! 並びは**行順の W011** のあとに**モード順の E002 / E016 / E017**（A 形式の E018 と同じ流儀）。
//!
//! # M2 で出さない診断
//!
//! - **E013**（`relations` の参照が解決できない、§5.5.6）・**E004**（未定義のモード ID）:
//!   辞書が要るので M3（§6.4）。M2 は `'参照` と `mode_id` の**形だけ**を読む。
//! - **W084**（台詞内の呼称が `address:` に反する、§5.5.6）: 本文と配役を突き合わせる
//!   §7.2 の検査なので M4 以降。
//! - `address:`（§5.5.6）・`icon:`（§5.5.3）・`color:`（§5.5.3）は §5.0.6 の表にあるので
//!   キーとして解し、**W011 は出さない**。値はそのまま保持する。`icon` の **E062** /
//!   **W068**–**W070**（§5.5.3）と **W084** / **W085**（§5.5.6）はプロジェクト全体を見る
//!   M4 の検査器（`core::features::diagnose`）が出す。
//! - `### <mode_id>` が `mode_id` の形に合わないとき: §7 に対応コードが無いので診断せず、
//!   [`ModeDef::valid_id`] が `false` になるだけ（A 形式の B-39 と同じ流儀）。

use std::ops::Range;

use serde::Serialize;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::id::{RangeError, RangeExpr, RangeTerm};
use crate::core::text::lexer::{self, FieldKey, LineKind, TextFormat};

use super::contents;
use super::plot_outline::{self, Continuation, Field};

/// 見出しのシジル（§5.5.2 `"# "`）。
pub const HEADING: char = '#';

/// §5.5.1 が意味を持つとする見出しの最大階層（H1 / H2 / H3）。
pub const HEADING_LEVELS: usize = 3;

/// 箇条書きのシジル（§5.5.1「箇条書き `- ...`」）。
///
/// Markdown は `*` と `+` も箇条書きに使うが、§5.5.1 が挙げるのは `-` だけなので
/// `*` / `+` で始まる行は自由記述（[`Line::Text`]）として保持する。
pub const BULLET: char = '-';

/// 役割語パターンの開き鉤括弧（§5.5.5）。
pub const PATTERN_OPEN: char = '「';

/// 役割語パターンの閉じ鉤括弧（§5.5.5）。
pub const PATTERN_CLOSE: char = '」';

/// `## ` のセクションキー（§5.5.2 `section_key`）。
pub const SECTION_MODES: &str = "modes";
/// §5.5.2 `section_key`。
pub const SECTION_ATTRIBUTES: &str = "attributes";
/// §5.5.2 `section_key`。
pub const SECTION_TONE: &str = "tone";
/// §5.5.2 `section_key`。
pub const SECTION_RELATIONS: &str = "relations";
/// §5.5.2 `section_key`。
pub const SECTION_NOTES: &str = "notes";

/// `relations` の行を分ける半角コロン（§5.5.6 `'参照: 説明`）。
const RELATION_SEP: char = ':';
/// `relations` の行を分ける全角コロン（§5.0.5 と同じくどちらも受理する）。
const RELATION_SEP_FULL: char = '：';
/// 参照のシジル（§6.2）。
const REF_SIGIL: char = '\'';

/// `characters.md` 1 ファイル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Characters<'a> {
    /// 全行（書かれたまま）。[`Characters::to_text`] はこれを LF で連結する。
    pub lines: Vec<Line<'a>>,
    /// 最初の H1 より前の行（§5.5.2 `file_preamble`）。
    pub preamble: Preamble,
    /// キャラクタ（H1 の順）。
    pub characters: Vec<Character<'a>>,
    /// 診断（§7）。位置は `text` の中の byte range。
    pub diagnostics: Vec<Diagnostic>,
}

/// `characters.md` のテキストをパースする。
///
/// `text` は **LF 改行**であること（§5.0.4。読み込み時に
/// [`crate::core::text::lexer::normalize_on_load`] を通す）。
///
/// 壊れた入力でも部分木と診断を返す（`docs/history.md` の完了条件 3）。H1 の前に `## ` があっても
/// パニックしない（行は前文として残る）し、閉じない `「` は自由記述になる。
pub fn parse(text: &str) -> Characters<'_> {
    let mut lines: Vec<Line<'_>> = text.split('\n').map(parse_line).collect();
    attach_continuations(&mut lines);
    let mut diagnostics = Vec::new();
    let (preamble, characters) = collect(&lines, &mut diagnostics);
    warn_ranges(&characters, &line_offsets(&lines), &mut diagnostics);
    Characters {
        lines,
        preamble,
        characters,
        diagnostics,
    }
}

impl<'a> Characters<'a> {
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

    /// `lines[index]` を含むキャラクタ。前文の行は `None`。
    pub fn character_of(&self, index: usize) -> Option<&Character<'a>> {
        self.characters
            .iter()
            .find(|found| found.span.contains(&index))
    }

    /// `lines[index]` を含むセクション（H2）。キャラクタ直下の行は `None`。
    pub fn section_of(&self, index: usize) -> Option<&Section<'a>> {
        let found = self.character_of(index)?;
        found
            .sections
            .iter()
            .find(|section| section.span.contains(&index))
    }

    /// `lines[index]` を含むモード（H3）。セクション直下の行は `None`。
    pub fn mode_of(&self, index: usize) -> Option<&ModeDef<'a>> {
        let found = self.section_of(index)?;
        found.modes.iter().find(|mode| mode.span.contains(&index))
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

    /// `index` 番目のキャラクタ直下の `key` のフィールド（出現順）。
    pub fn character_fields(&self, index: usize, key: FieldKey) -> Vec<&Field<'a>> {
        match self.characters.get(index) {
            Some(found) => pick(&self.lines, &found.fields, key),
            None => Vec::new(),
        }
    }

    /// `index` 番目のキャラクタの `at` 番目のセクションの `key` のフィールド（出現順）。
    pub fn section_fields(&self, index: usize, at: usize, key: FieldKey) -> Vec<&Field<'a>> {
        let Some(found) = self.characters.get(index) else {
            return Vec::new();
        };
        match found.sections.get(at) {
            Some(section) => pick(&self.lines, &section.fields, key),
            None => Vec::new(),
        }
    }

    /// `index` / `at` / `mode` で指すモードの `key` のフィールド（出現順）。
    pub fn mode_fields(
        &self,
        index: usize,
        at: usize,
        mode: usize,
        key: FieldKey,
    ) -> Vec<&Field<'a>> {
        let Some(found) = self.characters.get(index) else {
            return Vec::new();
        };
        let Some(section) = found.sections.get(at) else {
            return Vec::new();
        };
        match section.modes.get(mode) {
            Some(found) => pick(&self.lines, &found.fields, key),
            None => Vec::new(),
        }
    }
}

/// 最初の H1 より前の行（§5.5.2 `file_preamble`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preamble {
    /// 行範囲（`Characters::lines` の中）。H1 が無ければ全行。
    pub span: Range<usize>,
    /// フィールド行の index（出現順）。
    pub fields: Vec<usize>,
}

/// 1 キャラクタ（§5.5.2 `character`）。`# 名前` から次の `# ` まで。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Character<'a> {
    /// H1 の行 index。
    pub heading: usize,
    /// キャラ名（§5.5.2 `char_name`）。行末コメントは除く。
    pub name: &'a str,
    /// `name` の行内 byte 位置。
    pub name_at: usize,
    /// 行範囲。H1 を含み、次の H1 を含まない。
    pub span: Range<usize>,
    /// キャラクタ直下（どの `## ` にも入らない）フィールド行の index（出現順）。
    pub fields: Vec<usize>,
    /// キャラクタ直下の自由記述の index（出現順）。
    ///
    /// §5.5.2 の `character` に箇条書きとパターン行は無いので、H1 直下の `- ...` と `「...」`
    /// は**木に載せない**（行としては保持されるので round-trip は保たれる）。
    pub free: Vec<usize>,
    /// セクション（`## ` の順）。
    pub sections: Vec<Section<'a>>,
}

impl<'a> Character<'a> {
    /// `key` のセクション（§5.5.2 `section_key`）。複数あれば最初。
    pub fn section(&self, key: SectionKey) -> Option<&Section<'a>> {
        self.sections.iter().find(|found| found.key == key)
    }
}

/// `## ` セクション（§5.5.2 `section`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section<'a> {
    /// H2 の行 index。
    pub heading: usize,
    /// 引き当てたセクションキー。§5.5.2 の 5 種に無ければ [`SectionKey::Other`]。
    pub key: SectionKey,
    /// **書かれたままの**セクション名（別名表記と `Other` の中身を保つため）。
    pub key_text: &'a str,
    /// 行範囲。H2 を含み、次の H2 / H1 を含まない（中の `### ` は含む）。
    pub span: Range<usize>,
    /// セクション直下（どの `### ` にも入らない）フィールド行の index（出現順）。
    pub fields: Vec<usize>,
    /// 箇条書き行の index（出現順）。
    pub bullets: Vec<usize>,
    /// 役割語パターン行の index（出現順）。`## tone` 直下ならキャラ共通（§5.5.5）。
    pub patterns: Vec<usize>,
    /// 自由記述の index（出現順）。
    pub free: Vec<usize>,
    /// `relations` の行（§5.5.6）。[`SectionKey::Relations`] のセクションだけで読む。
    pub relations: Vec<Relation<'a>>,
    /// モード定義（`### ` の順）。§5.5.4 は `## modes` の下に置く。
    pub modes: Vec<ModeDef<'a>>,
}

/// §5.5.2 の `section_key`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SectionKey {
    /// `modes`（§5.5.4）。
    Modes,
    /// `attributes`。
    Attributes,
    /// `tone`（§5.5.5 のキャラ共通パターンを置く場所）。
    Tone,
    /// `relations`（§5.5.6）。
    Relations,
    /// `notes`。
    Notes,
    /// 5 種のどれでもない。**行はそのまま保持される**ので情報は失われない。
    Other,
}

/// 1 モード（§5.5.2 `mode_def`）。`### <mode_id> <表示名>` から次の見出しまで。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModeDef<'a> {
    /// H3 の行 index。
    pub heading: usize,
    /// `mode_id`（§5.5.2 `### ` の直後の 1 トークン）。
    pub id: &'a str,
    /// `id` の行内 byte 位置（**E017** の範囲に使う）。
    pub id_at: usize,
    /// 表示名（§5.5.2 `mode_label`）。無ければ空。
    pub label: &'a str,
    /// `id` が §5.5.2 の `mode_id` の形（`/[A-Za-z][A-Za-z0-9_]{0,15}/`）に合うか。
    ///
    /// 合わなくても診断は出さない（§7 に対応コードが無い。A 形式の B-39 と同じ流儀）。
    pub valid_id: bool,
    /// 行範囲。H3 を含み、次の `### ` / `## ` / `# ` を含まない。
    pub span: Range<usize>,
    /// フィールド行の index（出現順）。§5.5.4 の `range` `personality` `emotion` `role` `note`。
    pub fields: Vec<usize>,
    /// 役割語パターン行の index（出現順）。このモード専用（§5.5.5）。
    pub patterns: Vec<usize>,
    /// 自由記述の index（出現順）。
    pub free: Vec<usize>,
    /// `range:` の値（§5.5.4）。**書かれていなければ全範囲**。
    pub range: Option<RangeValue<'a>>,
}

/// `range:` フィールドの値（§5.5.4 のモード、§5.6 の用語）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RangeValue<'a> {
    /// フィールド行の index。
    pub line: usize,
    /// **書かれたままの**値（`@01 .. @02`）。
    pub text: &'a str,
    /// 値の行内 byte 位置（[`Field::value_at`]）。
    pub at: usize,
    /// 範囲式としてパースでき、左端 ≤ 右端だったか。
    ///
    /// `false` のときは **E002**（パース不能）または **E016**（左右が逆）が出ている。
    pub valid: bool,
}

impl RangeValue<'_> {
    /// 範囲式（§5.1）。パースできなければ `None`（診断は [`parse`] が出している）。
    pub fn expr(&self) -> Option<RangeExpr> {
        RangeExpr::parse(self.text).ok()
    }
}

/// `## relations` の 1 行（§5.5.6 `'参照: 説明`）。
///
/// §5.5.1 の 4 種に当てはまらないので行は [`Line::Text`] のままで、これはそこから読んだ意味。
/// 参照の実体が無いときの **E013** は辞書が要るので M3（§6.4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Relation<'a> {
    /// 行の index。
    pub line: usize,
    /// `'` の行内 byte 位置。
    pub at: usize,
    /// 参照トークン（`'` 込み）の行内 byte 範囲。
    pub range: Range<usize>,
    /// 参照先の名前（`'` を除く）。
    pub name: &'a str,
    /// 区切り（`:` / `：`）の後ろの説明。区切りが無ければ空。
    pub note: &'a str,
}

/// MD 方言の 1 行。
///
/// `words.md` / `ideas.md` / `prompts/*.md` も同じ分類を使う（§5.6「`characters.md` と
/// 同一の MD 方言」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Line<'a> {
    /// 見出し（`# ` / `## ` / `### `、§5.5.1 の 1）。
    Heading(Heading<'a>),
    /// フィールド行（§5.0.5、§5.5.1 の 2）。継続行は [`Field::extra`] に入る。
    Field(Field<'a>),
    /// 継続行（§5.0.5）。直前がフィールド行ならその値の続き。
    Continuation(Continuation<'a>),
    /// 箇条書き（`- ...`、§5.5.1 の 3）。
    Bullet(Bullet<'a>),
    /// 役割語パターン行（`「...」「...」`、§5.5.1 の 4 / §5.5.5）。
    Pattern(PatternLine<'a>),
    /// 自由記述。4 種に当てはまらない行すべて（§5.5.1「`note` 相当として保持」）。
    Text(&'a str),
    /// 行コメント（`//`、§5.0.2）。`# ` は `.cnov` 専用のコメント（§5.0.8）なので
    /// MD では見出しになる。
    Comment(contents::CommentLine<'a>),
    /// 空行（空白だけの行を含む。`docs/history.md` B-34）。
    Blank(&'a str),
}

impl<'a> Line<'a> {
    /// 書かれたままの 1 行。
    pub fn raw(&self) -> &'a str {
        match self {
            Self::Heading(found) => found.raw,
            Self::Field(found) => found.raw,
            Self::Continuation(found) => found.raw,
            Self::Bullet(found) => found.raw,
            Self::Pattern(found) => found.raw,
            Self::Text(found) => found,
            Self::Comment(found) => found.raw,
            Self::Blank(found) => found,
        }
    }

    /// 空行か（空白だけの行を含む）。
    pub fn is_blank(&self) -> bool {
        matches!(self, Self::Blank(_))
    }
}

/// 見出し行（§5.5.1 の 1）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Heading<'a> {
    /// 書かれたままの 1 行。
    pub raw: &'a str,
    /// 先頭の空白文字数。
    pub indent: usize,
    /// `#` の並びの byte 位置。
    pub sigil_at: usize,
    /// 階層（`#` の個数。1..=[`HEADING_LEVELS`]）。
    pub level: usize,
    /// 見出しのテキスト（`#` と空白を除き、行末コメントより前）。前後の空白は除く。
    pub text: &'a str,
    /// `text` の行内 byte 位置。
    pub text_at: usize,
    /// 行末コメント（§5.0.2）。
    pub comment: Option<contents::Comment<'a>>,
}

impl<'a> Heading<'a> {
    /// `### A 日常` を `mode_id` と表示名に分ける（§5.5.2 `[ ws , mode_label ]`）。
    ///
    /// 見出しのテキストを最初の空白で切るだけなので、H1（キャラ名）や H2（セクション名）に
    /// 使っても「1 トークン目と残り」が返る。
    pub fn split_label(&self) -> (&'a str, &'a str) {
        match self.text.find(lexer::is_space) {
            Some(at) => {
                let (head, rest) = self.text.split_at(at);
                (head, rest.trim_matches(lexer::is_space))
            }
            None => (self.text, ""),
        }
    }
}

/// 箇条書き行（§5.5.1 の 3）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bullet<'a> {
    /// 書かれたままの 1 行。
    pub raw: &'a str,
    /// 先頭の空白文字数。
    pub indent: usize,
    /// `-` の byte 位置。
    pub marker_at: usize,
    /// `-` の後ろ（前後の空白を除く）。
    pub text: &'a str,
}

/// 役割語パターン行（§5.5.5）。1 行に複数パターンを並べられる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PatternLine<'a> {
    /// 書かれたままの 1 行。
    pub raw: &'a str,
    /// 先頭の空白文字数。
    pub indent: usize,
    /// パターン（書いた順）。
    pub patterns: Vec<Pattern<'a>>,
}

/// 1 パターン（`「` と `」` に囲まれた文字列、§5.5.5）。
///
/// パターン言語の意味論（`*` の扱い等）は §6.5 / 付録 B で、マッチャ化は M3
/// （`resolve/role_language.rs`）。M2 は**位置と文字列だけ**を読む。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pattern<'a> {
    /// パターン本体の行内 byte 位置（`「` の直後）。
    pub at: usize,
    /// パターン本体の行内 byte 範囲。
    pub range: Range<usize>,
    /// パターン本体（鉤括弧を除く）。
    pub text: &'a str,
}

// ============================================================ 行の分解

/// 1 行を字句解析して MD 方言の行にする。
///
/// `words_md` / `ideas_md` / `prompt_md` もこれを使う（§5.6「同一の MD 方言」）。
/// 診断は出さない（未知キーの W011 は**行の byte 位置**が要るので [`collect`] が出す）。
pub(crate) fn parse_line(raw: &str) -> Line<'_> {
    // §5.0.1 の blank。空白の文字数は問わない（B-34）
    if raw.trim_matches(lexer::is_space).is_empty() {
        return Line::Blank(raw);
    }
    // `# ` は MD では見出し（§5.0.8 の行コメントは `.cnov` だけ。字句器の `Formats::CNOV` が担保）
    if let Some(found) = parse_heading(raw) {
        return Line::Heading(found);
    }
    // 有効なシジル（[`lexer::Stage::One`]）で MD の行を読む
    match lexer::classify_at_stage(raw, TextFormat::Markdown, lexer::Stage::One) {
        LineKind::Field(found) => Line::Field(plot_outline::parse_field(raw, found)),
        LineKind::Continuation { indent, text } => {
            Line::Continuation(Continuation { raw, indent, text })
        }
        LineKind::Comment { .. } => Line::Comment(contents::parse_comment(raw)),
        // 箇条書きと鉤括弧行は字句表に無い（§5.0.2 は `.cnov` のシジルを並べた表）ので
        // こちらで分類する。どちらも形が合わなければ自由記述のまま
        _ => {
            if let Some(found) = parse_bullet(raw) {
                return Line::Bullet(found);
            }
            match parse_patterns(raw) {
                Some(found) => Line::Pattern(found),
                None => Line::Text(raw),
            }
        }
    }
}

/// 見出し行（§5.5.1 の 1）。`#` の個数が 1..3 で、直後が空白または行末のときだけ見出し。
///
/// `#恩田恵`（空白なし）と `#### 深い見出し` は §5.5.1 の 4 種に無いので `None`
/// （＝自由記述として保持する）。
pub(crate) fn parse_heading(raw: &str) -> Option<Heading<'_>> {
    let indent = lexer::leading_indent(raw);
    let sigil_at = contents::body_start(raw);
    let rest = &raw[sigil_at..];
    let level = rest.chars().take_while(|&c| c == HEADING).count();
    if level == 0 || level > HEADING_LEVELS {
        return None;
    }
    let after = &rest[level..];
    match after.chars().next() {
        // `#` だけの行も階層だけを持つ見出し（テキストは空）
        None => {}
        Some(c) if lexer::is_space(c) => {}
        Some(_) => return None,
    }
    let comment = contents::find_comment(raw);
    // `#` から（行末コメントがあればその手前まで）が見出しの中身
    let head = match &comment {
        Some(found) => &raw[sigil_at..found.range.start],
        None => &raw[sigil_at..],
    };
    let after_hashes = &head[level..];
    let text = after_hashes.trim_matches(lexer::is_space);
    let skipped = after_hashes.len() - after_hashes.trim_start_matches(lexer::is_space).len();
    Some(Heading {
        raw,
        indent,
        sigil_at,
        level,
        text,
        text_at: sigil_at + level + skipped,
        comment,
    })
}

/// 箇条書き行（§5.5.1 の 3）。`-` の直後が空白または行末のときだけ箇条書き。
///
/// `-5` のように値が続く行は箇条書きにしない（§5.0.2 が `#1 位` や `!?` を誤認しないのと
/// 同じ理由）。
pub(crate) fn parse_bullet(raw: &str) -> Option<Bullet<'_>> {
    let indent = lexer::leading_indent(raw);
    let marker_at = contents::body_start(raw);
    let rest = &raw[marker_at..];
    let after = rest.strip_prefix(BULLET)?;
    match after.chars().next() {
        None => {}
        Some(c) if lexer::is_space(c) => {}
        Some(_) => return None,
    }
    Some(Bullet {
        raw,
        indent,
        marker_at,
        text: after.trim_matches(lexer::is_space),
    })
}

/// 役割語パターン行（§5.5.5）。行全体が `「...」` の並びのときだけパターン行。
///
/// 閉じない `「` や、最後の `」` の後ろに空白以外の残りがある行は `None`
/// （＝自由記述としてそのまま保持する。壊れた入力でパニックしない、完了条件 3）。
pub(crate) fn parse_patterns(raw: &str) -> Option<PatternLine<'_>> {
    let indent = lexer::leading_indent(raw);
    let mut at = contents::body_start(raw);
    let mut patterns = Vec::new();
    loop {
        let after_open = raw[at..].strip_prefix(PATTERN_OPEN)?;
        let close = after_open.find(PATTERN_CLOSE)?;
        let text = &after_open[..close];
        let text_at = at + PATTERN_OPEN.len_utf8();
        patterns.push(Pattern {
            at: text_at,
            range: text_at..text_at + text.len(),
            text,
        });
        at = text_at + text.len() + PATTERN_CLOSE.len_utf8();
        let tail = raw[at..].trim_start_matches(lexer::is_space);
        if tail.is_empty() {
            return Some(PatternLine {
                raw,
                indent,
                patterns,
            });
        }
        // 次のグループは `「` で始まっていなければならない（§5.5.2 `{ ws? , "「" ... }`）
        if !tail.starts_with(PATTERN_OPEN) {
            return None;
        }
        at = raw.len() - tail.len();
    }
}

/// フィールド行の直後の継続行を、そのフィールドの値に付ける（§5.0.5）。
///
/// A 形式の [`super::plot_outline`] と同じ規則だが、対象が MD 方言の [`Line`] なので
/// こちらに置く（行の型が違うだけで規則は 1 つ。B-37・B-40）。
pub(crate) fn attach_continuations(lines: &mut [Line<'_>]) {
    let mut owner: Option<usize> = None;
    for index in 0..lines.len() {
        if let Line::Field(_) = &lines[index] {
            owner = Some(index);
            continue;
        }
        // 継続行でなければ（空行・コメント・自由記述・箇条書き・パターン行）継続を中断する。
        // `&'a str` を先に写してから `lines` を可変借用する（同じ行を同時に借りられない）
        let Some(raw) = continuation_raw(&lines[index]) else {
            owner = None;
            continue;
        };
        if let Some(at) = owner {
            push_extra(&mut lines[at], plot_outline::drop_indent(raw));
        }
    }
}

/// 継続行なら書かれたままの 1 行。それ以外は `None`。
fn continuation_raw<'a>(line: &Line<'a>) -> Option<&'a str> {
    match line {
        Line::Continuation(found) => Some(found.raw),
        _ => None,
    }
}

/// フィールド行に継続行のテキストを 1 行分足す。
fn push_extra<'a>(line: &mut Line<'a>, text: &'a str) {
    if let Line::Field(found) = line {
        found.extra.push(text);
    }
}

// ============================================================ 木と診断

/// 解析中のモード（§5.5.2 `mode_def`）。見出しの中身は開いたときに写しておく。
struct OpenMode<'a> {
    /// H3 の行 index。
    heading: usize,
    /// `mode_id`。
    id: &'a str,
    /// `id` の行内 byte 位置。
    id_at: usize,
    /// 表示名。
    label: &'a str,
    /// フィールド行の index。
    fields: Vec<usize>,
    /// パターン行の index。
    patterns: Vec<usize>,
    /// 自由記述の index。
    free: Vec<usize>,
}

/// 解析中のセクション（§5.5.2 `section`）。
struct OpenSection<'a> {
    /// H2 の行 index。
    heading: usize,
    /// セクションキー。
    key: SectionKey,
    /// 書かれたままのセクション名。
    key_text: &'a str,
    /// フィールド行の index。
    fields: Vec<usize>,
    /// 箇条書き行の index。
    bullets: Vec<usize>,
    /// パターン行の index。
    patterns: Vec<usize>,
    /// 自由記述の index。
    free: Vec<usize>,
    /// モード。
    modes: Vec<OpenMode<'a>>,
}

/// 解析中のキャラクタ（§5.5.2 `character`）。
struct OpenCharacter<'a> {
    /// H1 の行 index。
    heading: usize,
    /// キャラ名。
    name: &'a str,
    /// `name` の行内 byte 位置。
    name_at: usize,
    /// キャラクタ直下のフィールド行の index。
    fields: Vec<usize>,
    /// キャラクタ直下の自由記述の index。
    free: Vec<usize>,
    /// セクション。
    sections: Vec<OpenSection<'a>>,
}

/// 行の並びから前文とキャラクタを作り、行順の診断（W011）を出す。
///
/// `range` の診断（E002 / E016 / E017）はモードの並びが要るので [`warn_ranges`] が出す。
fn collect<'a>(lines: &[Line<'a>], out: &mut Vec<Diagnostic>) -> (Preamble, Vec<Character<'a>>) {
    // §5.5.2 `characters_md = [ file_preamble ] , { character }`。H1 より前だけが前文
    let head = lines
        .iter()
        .position(|line| matches!(line, Line::Heading(found) if found.level == 1));
    let mut preamble = Preamble {
        span: 0..head.unwrap_or(lines.len()),
        fields: Vec::new(),
    };
    let mut characters: Vec<Character<'_>> = Vec::new();
    let mut open: Option<OpenCharacter<'_>> = None;
    let mut at = 0;
    for (index, line) in lines.iter().enumerate() {
        if let Line::Field(found) = line {
            // §5.0.6「未知のキーは W011 とし、値は改変せず保持する」（行順）
            plot_outline::warn_unknown_key(found, at, out);
        }
        match line {
            Line::Heading(head) => match head.level {
                1 => {
                    if let Some(closed) = open.take() {
                        characters.push(finish_character(closed, index, lines));
                    }
                    open = Some(OpenCharacter {
                        heading: index,
                        name: head.text,
                        name_at: head.text_at,
                        fields: Vec::new(),
                        free: Vec::new(),
                        sections: Vec::new(),
                    });
                }
                // H2: セクションを開く（H1 の外は木に載せない。行は残るので round-trip は保たれる）
                2 => {
                    if let Some(found) = open.as_mut() {
                        found.sections.push(OpenSection {
                            heading: index,
                            key: section_key_of(head.text),
                            key_text: head.text,
                            fields: Vec::new(),
                            bullets: Vec::new(),
                            patterns: Vec::new(),
                            free: Vec::new(),
                            modes: Vec::new(),
                        });
                    }
                }
                // H3: モードを開く（§5.5.4 は `## modes` の下。セクションの外は木に載せない）
                _ => {
                    if let Some(section) = open.as_mut().and_then(|found| found.sections.last_mut())
                    {
                        let (id, label) = head.split_label();
                        section.modes.push(OpenMode {
                            heading: index,
                            id,
                            id_at: head.text_at,
                            label,
                            fields: Vec::new(),
                            patterns: Vec::new(),
                            free: Vec::new(),
                        });
                    }
                }
            },
            Line::Field(_) => push_field(&mut preamble, &mut open, index),
            Line::Bullet(_) => push_bullet(&mut open, index),
            Line::Pattern(_) => push_pattern(&mut open, index),
            Line::Text(_) => push_text(&mut open, index),
            // 継続行は持ち主のフィールドに既に入っている。コメントと空行は木に載せない
            Line::Continuation(_) | Line::Comment(_) | Line::Blank(_) => {}
        }
        at += line.raw().len() + 1;
    }
    if let Some(closed) = open.take() {
        characters.push(finish_character(closed, lines.len(), lines));
    }
    (preamble, characters)
}

/// フィールド行を、いま開いている一番内側の入れ物に積む。
fn push_field(preamble: &mut Preamble, open: &mut Option<OpenCharacter<'_>>, index: usize) {
    let Some(found) = open.as_mut() else {
        preamble.fields.push(index);
        return;
    };
    let Some(section) = found.sections.last_mut() else {
        found.fields.push(index);
        return;
    };
    match section.modes.last_mut() {
        Some(mode) => mode.fields.push(index),
        None => section.fields.push(index),
    }
}

/// 箇条書き行を積む（§5.5.2 `section_body` にだけ現れる。モード内は自由記述と同じ扱い）。
fn push_bullet(open: &mut Option<OpenCharacter<'_>>, index: usize) {
    let Some(section) = open
        .as_mut()
        .and_then(|found| found.sections.last_mut())
        .filter(|section| section.modes.is_empty())
    else {
        return;
    };
    section.bullets.push(index);
}

/// 役割語パターン行を積む（§5.5.5。モード内ならモード専用、それ以外はキャラ共通）。
fn push_pattern(open: &mut Option<OpenCharacter<'_>>, index: usize) {
    let Some(section) = open.as_mut().and_then(|found| found.sections.last_mut()) else {
        return;
    };
    match section.modes.last_mut() {
        Some(mode) => mode.patterns.push(index),
        None => section.patterns.push(index),
    }
}

/// 自由記述を積む（§5.5.1「直前のフィールド／セクションに属する自由記述」）。
fn push_text(open: &mut Option<OpenCharacter<'_>>, index: usize) {
    let Some(found) = open.as_mut() else {
        return;
    };
    let Some(section) = found.sections.last_mut() else {
        found.free.push(index);
        return;
    };
    match section.modes.last_mut() {
        Some(mode) => mode.free.push(index),
        None => section.free.push(index),
    }
}

/// キャラクタを確定する。
///
/// セクションの `span` の終わりは**次のセクションの見出し行**（最後はキャラクタの終わり）。
/// モードも同じ（次のモードの見出し行、最後はセクションの終わり）。
fn finish_character<'a>(open: OpenCharacter<'a>, stop: usize, lines: &[Line<'a>]) -> Character<'a> {
    let OpenCharacter {
        heading,
        name,
        name_at,
        fields,
        free,
        sections,
    } = open;
    let starts: Vec<usize> = sections.iter().map(|found| found.heading).collect();
    let mut built = Vec::with_capacity(sections.len());
    for (at, section) in sections.into_iter().enumerate() {
        let end = starts.get(at + 1).copied().unwrap_or(stop);
        built.push(finish_section(section, end, lines));
    }
    Character {
        heading,
        name,
        name_at,
        span: heading..stop,
        fields,
        free,
        sections: built,
    }
}

/// セクションを確定する。
fn finish_section<'a>(open: OpenSection<'a>, stop: usize, lines: &[Line<'a>]) -> Section<'a> {
    let OpenSection {
        heading,
        key,
        key_text,
        fields,
        bullets,
        patterns,
        free,
        modes,
    } = open;
    // §5.5.6 の `'参照: 説明` は `## relations` の中だけで意味を持つ
    let relations = match key {
        SectionKey::Relations => free
            .iter()
            .filter_map(|&index| parse_relation(lines, index))
            .collect(),
        _ => Vec::new(),
    };
    let starts: Vec<usize> = modes.iter().map(|found| found.heading).collect();
    let mut built = Vec::with_capacity(modes.len());
    for (at, mode) in modes.into_iter().enumerate() {
        let end = starts.get(at + 1).copied().unwrap_or(stop);
        built.push(finish_mode(mode, end, lines));
    }
    Section {
        heading,
        key,
        key_text,
        span: heading..stop,
        fields,
        bullets,
        patterns,
        free,
        relations,
        modes: built,
    }
}

/// モードを確定する（§5.5.2 `mode_def`）。
fn finish_mode<'a>(open: OpenMode<'a>, stop: usize, lines: &[Line<'a>]) -> ModeDef<'a> {
    let OpenMode {
        heading,
        id,
        id_at,
        label,
        fields,
        patterns,
        free,
    } = open;
    let range = fields.iter().find_map(|&index| range_value(lines, index));
    ModeDef {
        heading,
        id,
        id_at,
        label,
        valid_id: contents::is_mode_id(id),
        span: heading..stop,
        fields,
        patterns,
        free,
        range,
    }
}

/// `range:` フィールドなら [`RangeValue`]（§5.5.4 / §5.6）。それ以外は `None`。
///
/// §5.6 の `words.md` も同じ形なので [`super::words_md`] が呼ぶ（B-40 の流儀）。
pub(crate) fn range_value<'a>(lines: &[Line<'a>], index: usize) -> Option<RangeValue<'a>> {
    let Line::Field(found) = lines.get(index)? else {
        return None;
    };
    if found.key != FieldKey::Range {
        return None;
    }
    Some(RangeValue {
        line: index,
        text: found.value,
        at: found.value_at,
        valid: matches!(RangeExpr::parse(found.value), Ok(expr) if expr.check().is_ok()),
    })
}

/// `range:` の値を検査して **E002**（範囲式として読めない）/ **E016**（左端 > 右端）を出し、
/// 比較に使える範囲を返す。形が読めないときは `None`（呼び出し側は**比較しない**）。
///
/// §5.5.4（モード）と §5.6（用語）で同じ規則なので [`super::words_md`] も呼ぶ。
/// `at` は値の先頭の byte 位置。
pub(crate) fn check_range(
    found: &RangeValue<'_>,
    at: usize,
    out: &mut Vec<Diagnostic>,
) -> Option<RangeExpr> {
    let range = at..at + found.text.len();
    match RangeExpr::parse(found.text) {
        Ok(expr) => {
            if expr.check().is_err() {
                out.push(Diagnostic::new(
                    DiagnosticCode::E016,
                    Some(range),
                    Vec::new(),
                ));
            }
            Some(expr)
        }
        // `range:` だけの行は「範囲を書かなかった」と読む（§5.5.4 の省略と同じ）
        Err(RangeError::Empty) => Some(all_range()),
        Err(_) => {
            out.push(Diagnostic::new(
                DiagnosticCode::E002,
                Some(range),
                Vec::new(),
            ));
            None
        }
    }
}

/// §5.5.2 の `section_key` を引き当てる。
///
/// 5 種の ascii のほか、§5.0.6 の別名表に準拠した日本語表記（`## 口調` `## 関係` `## メモ`
/// `## モード`）も受ける。別名は字句器の表（[`lexer::lookup_key`]）を引くだけで、
/// このファイルに日本語リテラルは置かない（§12.2）。
pub(crate) fn section_key_of(text: &str) -> SectionKey {
    match text {
        SECTION_MODES => SectionKey::Modes,
        SECTION_ATTRIBUTES => SectionKey::Attributes,
        SECTION_TONE => SectionKey::Tone,
        SECTION_RELATIONS => SectionKey::Relations,
        SECTION_NOTES => SectionKey::Notes,
        // §5.5.2「セクションとして意味が対応する 4 語だけを受ける」。それ以外
        // （`## 性格` 等）は `Other` として保持し、中の行は自由記述として残す
        _ => match lexer::lookup_key(text) {
            FieldKey::Tone => SectionKey::Tone,
            FieldKey::Relation => SectionKey::Relations,
            FieldKey::Note => SectionKey::Notes,
            FieldKey::Mode => SectionKey::Modes,
            _ => SectionKey::Other,
        },
    }
}

/// `## relations` の 1 行を読む（§5.5.6 `'参照: 説明`）。
///
/// 参照は**行頭**（空白のあと）に `'` がある行だけ。`'` の後ろは最初の空白またはコロンまでが
/// 名前で、コロン以降が説明。§6.2.4 の修飾子（`'名前:モード`）と違い、ここでのコロンは
/// 説明の区切りなので修飾子としては読まない（§5.5.6 の例が正本）。
fn parse_relation<'a>(lines: &[Line<'a>], index: usize) -> Option<Relation<'a>> {
    let Line::Text(raw) = lines.get(index)? else {
        return None;
    };
    let at = contents::body_start(raw);
    let rest = raw[at..].strip_prefix(REF_SIGIL)?;
    let end = rest
        .find(|c: char| lexer::is_space(c) || c == RELATION_SEP || c == RELATION_SEP_FULL)
        .unwrap_or(rest.len());
    let name = &rest[..end];
    if !contents::is_char_name(name) {
        return None;
    }
    let name_at = at + REF_SIGIL.len_utf8();
    let token_end = name_at + name.len();
    let after = &raw[token_end..];
    let note = match after
        .strip_prefix(RELATION_SEP)
        .or_else(|| after.strip_prefix(RELATION_SEP_FULL))
    {
        Some(tail) => tail.trim_matches(lexer::is_space),
        None => "",
    };
    Some(Relation {
        line: index,
        at,
        range: at..token_end,
        name,
        note,
    })
}

/// 範囲の診断（**E002** / **E016** / **E017**）をモード順に出す。
///
/// - **E002**: 値が空でなく、範囲式としてパースできない（§5.1）。
/// - **E016**: パースできたが左端 > 右端（§5.1 の境界検査）。
/// - **E017**: **同一キャラクタ内の同一 `mode_id`** で範囲が重なる（§5.5.4）。2 件目に出す。
///   `range` を書かなかったモードは全範囲（§5.5.4）なので同じ ID の他の定義と必ず重なる。
///   値が壊れていて範囲を決められないモードは**比較しない**（E002 だけを出す）。
fn warn_ranges(characters: &[Character<'_>], offsets: &[usize], out: &mut Vec<Diagnostic>) {
    for found in characters {
        let mut seen: Vec<Seen> = Vec::new();
        for section in &found.sections {
            for mode in &section.modes {
                seen.push(classify_mode(mode, offsets, out));
            }
        }
        warn_overlaps(&seen, out);
    }
}

/// 重複判定のために集めた 1 モードの範囲。
struct Seen<'a> {
    /// `mode_id`。
    id: &'a str,
    /// 範囲。`None` なら**決められない**（E002 が出ている）ので比較しない。
    expr: Option<RangeExpr>,
    /// **E017** の範囲（`range` の値、無ければ見出しの `mode_id`）。
    at: Range<usize>,
}

/// 1 モードの `range` を読んで、E002 / E016 を出し、重複判定用の情報を返す。
fn classify_mode<'a>(
    mode: &'a ModeDef<'a>,
    offsets: &[usize],
    out: &mut Vec<Diagnostic>,
) -> Seen<'a> {
    let Some(found) = &mode.range else {
        // §5.5.4「`range` 省略時は全範囲（`@00 ..`）」
        let start = offsets[mode.heading] + mode.id_at;
        return Seen {
            id: mode.id,
            expr: Some(all_range()),
            at: start..start + mode.id.len(),
        };
    };
    let start = offsets[found.line] + found.at;
    let expr = check_range(found, start, out);
    Seen {
        id: mode.id,
        expr,
        at: start..start + found.text.len(),
    }
}

/// 同一 `mode_id` で範囲が重なる組を探し、2 件目に **E017** を出す（§5.5.4）。
fn warn_overlaps(seen: &[Seen<'_>], out: &mut Vec<Diagnostic>) {
    for (index, mine) in seen.iter().enumerate() {
        let Some(expr) = &mine.expr else {
            continue;
        };
        let overlaps = seen[..index].iter().any(|other| {
            other.id == mine.id
                && other
                    .expr
                    .as_ref()
                    .is_some_and(|theirs| expr.intersects(theirs))
        });
        if overlaps {
            out.push(Diagnostic::new(
                DiagnosticCode::E017,
                Some(mine.at.clone()),
                vec![Arg::text(mine.id)],
            ));
        }
    }
}

/// 全範囲（§5.5.4「`range` 省略時は全範囲（`@00 ..`）」）。両端が開放の 1 項。
pub(crate) fn all_range() -> RangeExpr {
    RangeExpr {
        terms: vec![RangeTerm {
            start: None,
            end: None,
        }],
    }
}

/// 各行の先頭の byte 位置（`lines.len() + 1` 要素）。
///
/// MD 方言の診断は行内の相対位置を持つので、絶対位置にするときに使う
/// （[`super::words_md`] / [`super::ideas_md`] / [`super::prompt_md`] も呼ぶ）。
pub(crate) fn line_offsets(lines: &[Line<'_>]) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(lines.len() + 1);
    let mut at = 0;
    for line in lines {
        offsets.push(at);
        at += line.raw().len() + 1;
    }
    offsets.push(at);
    offsets
}

/// `indices` の中から**書かれたままのキー**が `key_text` のフィールド行を引く。
///
/// 字句器は §5.0.6 の表に無いキーをすべて [`FieldKey::Unknown`] にするので、どの未知キーかを
/// 知るには [`Field::key_text`] で比べる。§5.0.6「値は改変せず保持する」（縫い代 2）の中身を
/// 読む手段で、MD 方言を使うパーサ（[`super::words_md`] / [`super::ideas_md`]）が共有する。
pub(crate) fn unknown_field<'s, 'a>(
    lines: &'s [Line<'a>],
    indices: &[usize],
    key_text: &str,
) -> Option<&'s Field<'a>> {
    indices.iter().find_map(|&index| match lines.get(index) {
        Some(Line::Field(found)) if found.key_text == key_text => Some(found),
        _ => None,
    })
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
