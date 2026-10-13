//! B 形式（話プロット、§5.4）のパーサ。
//!
//! `plots/AA-BB.txt` は本文（C 形式、§5.2）と **1:1 で節が対応する**形式で、A 形式（§5.3）とは
//! 別物である（§5.3 指示 11）。違いは 2 つ:
//!
//! - 見出しは**場面**（`@A-B-C[-D]`、3〜4 階層）で、C 形式の `scene_header` と同じ形。
//! - フィールドに**スコープ**がある。§5.4.1 の EBNF は
//!   `scene_block = scene_header , { scene_field | comment_line | blank } , section , …` で、
//!   `section = { point_line | section_field | comment_line | blank }` である。つまり
//!   **見出しから最初の `point_line` の直前まで**が場面レベル（`scene_field`）の領域で、
//!   それ以降（`;` を跨いだあとも）は節レベル（`section_field`）になる。§5.4.2 の「適用」列
//!   （`check` `length` `state` は節だけ）ともこの読み方で整合する。
//!
//! ```ebnf
//! episode_plot  = [ preamble ] , { scene_block } ;
//! scene_block   = scene_header , { scene_field | comment_line | blank } , section ,
//!                 { ";" eol , section } ;
//! section       = { point_line | section_field | comment_line | blank } ;
//! point_line    = inline+ , eol ;
//! ```
//!
//! # 行と場面・節
//!
//! [`EpisodePlot::lines`] は**書かれたままの 1 行ずつ**を持ち、[`EpisodePlot::to_text`] はそれを
//! LF で連結するので入力とバイト単位で一致する（§5.3 完了条件 2 と同じ要求を B 形式にも適用）。
//! [`Preamble`]・[`Scene`]・[`Section`] は同じ行を **index で指すだけ**で、行の複製を持たない
//! （C 形式の [`super::contents::Section`] と同じ流儀）。
//!
//! 節番号（`D`）は §5.2.2 と同じ規則で確定する: 見出しに `D` の明示があればそこから、
//! 無ければ 1 から始まり、`;` を跨ぐごとに 1 ずつ増える。本文と 1:1 で対応するためには
//! 両者が同じ番号を振る必要がある（節数の食い違いは **W030**、M4 の検査器）。
//!
//! # フィールドと未知キー
//!
//! フィールド行の判定は字句器（§5.0.5）に任せ、A 形式（[`super::plot_outline`]）と同じ
//! [`Field`] / [`Continuation`] / [`CharRef`] 型を使う（B-40）。§5.0.6 の表に無いキーは
//! **W011** を出して**値を改変せずに保持する**ので、② のキー（`when` `space` `space-mode`
//! `senses` `illustration`）はこの経路でそのまま残る。
//!
//! # 診断（M2 で出すもの）
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **E002** | 見出しの ID の形が崩れている（§6.2.5） | `@id` |
//! | **E020** | 見出しの ID は読めるが 1〜2 階層（§6.2.5、第 2.8 版。`{0}` は `3-4`） | `@id` |
//! | **E022** | 見出しが予約番号（`C = 00`、または設定で拒否する `A = 00`、§5.1.2） | `@id` |
//! | **E008** | 見出しの ID が同一ファイル内で重複（§7「同一ファイル内の ID 重複」） | 2 件目の `@id` |
//! | **E005** | 1 つの `mode:` に同じキャラを 2 回、または 1 キャラに 3 つ以上のモード（§5.4.2） | 該当トークン |
//! | **W011** | 未知のフィールドキー（§5.0.6） | キー |
//!
//! 並びは**行順**（すべて 1 回の走査で出す。A 形式のように後から足す診断が無い）。
//!
//! # M2 で出さない診断
//!
//! - **E003**（モードが存在しない）・**E004**（モードの有効範囲外）: `characters.md` の辞書が
//!   要るので M3（§6.4）。§5.4.2 の制約のうち M2 で出せるのは E005 だけ。
//! - **W030**（プロットと本文の節数の食い違い、§5.4.2）: 2 ファイルを比べるので M4 の検査器。
//! - **W083**（`senses` の偏り、§5.4.4）: ② のキーが絡むので ② の実装時。
//! - **W012**（空の節、§5.2.2）: B 形式では空の節が正当（§5.4.5 の例は `;` 直後の節にだけ
//!   `illustration:` を書く）。§5.4 に対応する記述が無いので出さない。
//! - `state` / `length` の値が形に合わないとき: §7 に対応コードが無いので診断せず
//!   [`Field::number`] が `None` を返す（`PLANS.md` §9.2 B-39）。
//!
//! # `.txt` 3 形式の共通部
//!
//! 見出し行の分解（E002 / E020 / E022 込み）は C 形式の [`contents::parse_scene`] を、行末コメント・
//! 行頭空白の byte 位置・`mode_id` の形は [`super::contents`] のものをそのまま使う。
//! フィールド行とその診断は A 形式の [`super::plot_outline`] のものを使う（B-40）。

use std::ops::Range;

use serde::Serialize;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::id::{self, SceneId};
use crate::core::text::lexer::{self, FieldKey, LineKind, TextFormat};

use super::contents;
use super::plot_outline;

/// フィールド行・継続行・参照トークンの型は A 形式（§5.3）と共通（B-40）。
///
/// §3.3 の木に共有の字句ヘルパ用ファイルは無いので、フィールド行の型は A 形式の
/// [`super::plot_outline`] が持ち、B 形式はここから同じ名前で公開する。
pub use super::plot_outline::{CharRef, Continuation, Field};

/// B 形式の設定は C 形式（§5.2）と同じ。
///
/// 見出しは §5.2.1 と同じ `scene_header`（3〜4 階層）で、`A = 00`（0 章）を拒否するかだけ
/// `structure.allow_chapter_zero`（§5.1.2）で決まる。B 形式に固有の設定は無い。
pub use super::contents::Options;

/// 1 節で 1 キャラが持てるモード数の上限（§6.4.4「1 キャラ 1 節につき**最大 2 モード**」）。
const MODE_MAX: usize = 2;

/// B 形式 1 ファイル（`plots/AA-BB.txt`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EpisodePlot<'a> {
    /// 全行（書かれたまま）。[`EpisodePlot::to_text`] はこれを LF で連結する。
    pub lines: Vec<Line<'a>>,
    /// 最初の場面見出しより前の行。どの場面にも節にも属さない（B-32 と同じ流儀）。
    pub preamble: Preamble,
    /// 場面（§5.4.1 `scene_block`）。見出しの順。
    pub scenes: Vec<Scene>,
    /// 診断（§7）。位置は `text` の中の byte range。
    pub diagnostics: Vec<Diagnostic>,
}

/// B 形式のテキストをパースする（設定は [`Options::default`]）。
///
/// `text` は **LF 改行**であること（§5.0.4。読み込み時に
/// [`crate::core::text::lexer::normalize_on_load`] を通す）。
pub fn parse(text: &str) -> EpisodePlot<'_> {
    parse_with(text, &Options::default())
}

/// 設定を指定してパースする。
///
/// 壊れた入力でも部分木と診断を返す（§4.3-4、§5.3 完了条件 3 と同じ要求）。ID が壊れた
/// 見出しも場面を作り（[`Scene::id`] が `None`）、節は `;` の数だけできる。
pub fn parse_with<'a>(text: &'a str, options: &Options) -> EpisodePlot<'a> {
    let mut diagnostics = Vec::new();
    let lines = parse_lines(text, options, &mut diagnostics);
    let (preamble, scenes) = collect(&lines);
    EpisodePlot {
        lines,
        preamble,
        scenes,
        diagnostics,
    }
}

impl<'a> EpisodePlot<'a> {
    /// 入力と**バイト単位で一致する**テキストに戻す（§5.3 完了条件 2）。
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

    /// `lines[index]` を含む場面。前文の行と場面の外の行は `None`。
    pub fn scene_of(&self, index: usize) -> Option<&Scene> {
        self.scenes.iter().find(|scene| scene.span.contains(&index))
    }

    /// `lines[index]` を含む節。
    pub fn section_of(&self, index: usize) -> Option<&Section> {
        for scene in &self.scenes {
            for section in &scene.sections {
                if section.span.contains(&index) {
                    return Some(section);
                }
            }
        }
        None
    }

    /// `lines[index]` がフィールド行ならその中身。
    pub fn field(&self, index: usize) -> Option<&Field<'a>> {
        match self.lines.get(index) {
            Some(Line::Field(found)) => Some(found),
            _ => None,
        }
    }

    /// `index` 番目の場面の**場面レベル**のフィールド（§5.4.1 `scene_field`、出現順）。
    pub fn scene_fields(&self, index: usize, key: FieldKey) -> Vec<&Field<'a>> {
        let Some(scene) = self.scenes.get(index) else {
            return Vec::new();
        };
        pick(&self.lines, &scene.fields, key)
    }

    /// `index` 番目の場面の `at` 番目の**節レベル**のフィールド（§5.4.1 `section_field`）。
    pub fn section_fields(&self, index: usize, at: usize, key: FieldKey) -> Vec<&Field<'a>> {
        let Some(scene) = self.scenes.get(index) else {
            return Vec::new();
        };
        let Some(section) = scene.sections.get(at) else {
            return Vec::new();
        };
        pick(&self.lines, &section.fields, key)
    }
}

/// `indices` の行から `key` のフィールドを集める（出現順）。
fn pick<'s, 'a>(lines: &'s [Line<'a>], indices: &[usize], key: FieldKey) -> Vec<&'s Field<'a>> {
    let mut out = Vec::new();
    for &index in indices {
        if let Some(Line::Field(found)) = lines.get(index) {
            if found.key == key {
                out.push(found);
            }
        }
    }
    out
}

/// B 形式の 1 行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Line<'a> {
    /// 場面見出し（§5.4.1 `scene_header`。§5.2.1 と同じ形なので C 形式の型をそのまま使う）。
    Scene(contents::SceneLine<'a>),
    /// 節区切り `;`（§5.4.1）。行末コメントを許す（§5.0.2）。
    Break(contents::BreakLine<'a>),
    /// フィールド行（§5.0.5）。場面レベルか節レベルかは [`Scene::fields`] か
    /// [`Section::fields`] のどちらに入っているかで分かる。
    Field(Field<'a>),
    /// 継続行（§5.0.5）。直前がフィールド行ならその値の続き（B-37）。
    Continuation(Continuation<'a>),
    /// `point_line`（節の内容、§5.4.1）。**話者注釈もルビも読まない**（あれは C 形式の
    /// `text_line` の規則で、プロットの自由文には適用されない）。
    Point(&'a str),
    /// 行コメント（`//`、§5.0.2）。`# ` は ② なので ① では [`Line::Point`] になる。
    Comment(contents::CommentLine<'a>),
    /// 空行（空白だけの行を含む。`PLANS.md` §9.2 B-34）。
    Blank(&'a str),
}

impl<'a> Line<'a> {
    /// 書かれたままの 1 行。
    pub fn raw(&self) -> &'a str {
        match self {
            Self::Scene(found) => found.raw,
            Self::Break(found) => found.raw,
            Self::Field(found) => found.raw,
            Self::Continuation(found) => found.raw,
            Self::Point(found) => found,
            Self::Comment(found) => found.raw,
            Self::Blank(found) => found,
        }
    }

    /// 空行か（空白だけの行を含む）。
    pub fn is_blank(&self) -> bool {
        matches!(self, Self::Blank(_))
    }
}

/// 最初の場面見出しより前の行。どの場面にも節にも属さない（B-32 と同じ流儀）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preamble {
    /// 行範囲（`EpisodePlot::lines` の中）。見出しが無ければ全行。
    pub span: Range<usize>,
    /// フィールド行の index（出現順）。
    pub fields: Vec<usize>,
}

/// 1 場面（§5.4.1 `scene_block`）。本文（C 形式）の 1 場面と対応する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Scene {
    /// 見出し行の index（`EpisodePlot::lines` の中）。
    pub header: usize,
    /// 見出しの ID。3〜4 階層のときだけ `Some`。それ以外は `None`（**E002** / **E020**）。
    pub id: Option<SceneId>,
    /// 書かれた階層の数（`@01-04-02` = 3）。壊れていれば 0。
    pub depth: usize,
    /// 場面の行範囲。見出し行を含み、次の見出しは含まない。
    pub span: Range<usize>,
    /// **場面レベル**のフィールド行の index（§5.4.1 `scene_field`、§5.4.2「適用」列）。
    ///
    /// 見出しから最初の `point_line`（無ければ最初の `;`）の直前までがここに入る。
    pub fields: Vec<usize>,
    /// 節（§5.4.1 `section`）。`;` で区切られた順。
    pub sections: Vec<Section>,
}

/// 1 節（§5.4.1 `section`）。本文（C 形式）の 1 節と 1:1 で対応する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    /// 節番号（`D`）。§5.2.2 と同じ規則で確定した値。
    pub number: u16,
    /// 節の行範囲（**区切り `;` を含まない**）。
    ///
    /// 最初の節は見出し直後ではなく**最初の `point_line` から**始まる（§5.4.1）。内容が
    /// 何も無ければ空範囲になる（§5.4.5 の例）。
    pub span: Range<usize>,
    /// 節の内容（`span` から**末尾の空行を除いた**もの、§5.2.2 MUST）。
    pub content: Range<usize>,
    /// 節を閉じた `;` の行 index。閉じられていなければ `None`。
    pub break_line: Option<usize>,
    /// **節レベル**のフィールド行の index（出現順）。
    pub fields: Vec<usize>,
    /// `point_line`（節の内容）の index（出現順）。
    pub points: Vec<usize>,
}

// ============================================================ 行の分解

/// 全行を字句解析し、**行順**に診断を出す。
///
/// 見出しの診断（E002 / E020 / E022 / E008）とフィールドの診断（W011 / E005）はどちらも 1 行だけで
/// 判定できるので（E008 は既出の ID を覚えるだけ）、A 形式のように後から並べ直さない。
fn parse_lines<'a>(text: &'a str, options: &Options, out: &mut Vec<Diagnostic>) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut seen: Vec<(SceneId, usize)> = Vec::new();
    let mut at = 0;
    for raw in text.split('\n') {
        lines.push(parse_line(raw, at, options, &mut seen, out));
        at += raw.len() + 1;
    }
    attach_continuations(&mut lines);
    lines
}

/// 1 行を字句解析して木の行にする。
fn parse_line<'a>(
    raw: &'a str,
    offset: usize,
    options: &Options,
    seen: &mut Vec<(SceneId, usize)>,
    out: &mut Vec<Diagnostic>,
) -> Line<'a> {
    // §5.4.1 の `blank`。空白の文字数は問わない（B-34）
    if raw.trim_matches(lexer::is_space).is_empty() {
        return Line::Blank(raw);
    }
    // ①（[`Stage::One`]）で B 形式の行を読む。`! ` は C 形式専用（§5.2.5）なので現れない
    match lexer::classify_at_stage(raw, TextFormat::PlotEpisode, lexer::Stage::One) {
        LineKind::Structure(_) => {
            // 見出しの分解と E002 / E020 / E022 は C 形式と完全に同じ（§5.4.1 `scene_header`）
            let found = contents::parse_scene(raw, offset, options, out);
            warn_duplicate(&found, offset, seen, out);
            Line::Scene(found)
        }
        LineKind::SectionBreak => Line::Break(contents::parse_break(raw)),
        LineKind::Comment { .. } => Line::Comment(contents::parse_comment(raw)),
        LineKind::Field(found) => {
            let field = plot_outline::parse_field(raw, found);
            warn_field(&field, offset, out);
            Line::Field(field)
        }
        LineKind::Continuation { indent, text } => {
            Line::Continuation(Continuation { raw, indent, text })
        }
        // 節の内容（§5.4.1 `point_line`）。② の `# ` も ① では自由文のまま
        LineKind::Text(_) | LineKind::IllustrationMemo(_) => Line::Point(raw),
    }
}

/// 見出しの ID が同一ファイル内で重複していないか（**E008**、§7「同一ファイル内の ID 重複」）。
fn warn_duplicate(
    found: &contents::SceneLine<'_>,
    offset: usize,
    seen: &mut Vec<(SceneId, usize)>,
    out: &mut Vec<Diagnostic>,
) {
    // ID が壊れているときは E002 / E020 が出ている（[`contents::parse_scene`]）のでここでは何もしない
    let Some(id) = found.id else {
        return;
    };
    let start = offset + contents::body_start(found.raw);
    let range = start..start + id::SIGIL.len_utf8() + found.id_text.len();
    if seen.contains(&(id, found.depth)) {
        let args = vec![Arg::text(found.id_text)];
        out.push(Diagnostic::new(DiagnosticCode::E008, Some(range), args));
    }
    seen.push((id, found.depth));
}

/// フィールド行の診断（**W011** 未知のキー、**E005** `mode:` の形）。
fn warn_field(found: &Field<'_>, offset: usize, out: &mut Vec<Diagnostic>) {
    plot_outline::warn_unknown_key(found, offset, out);
    if found.key == FieldKey::Mode {
        warn_modes(found, offset, out);
    }
}

/// §5.4.2「1 節の `mode` に同じキャラを 2 回書くのは **E005**。1 キャラに 3 つ以上のモード
/// （`A+B+C`）も **E005**」。
///
/// 位置は**値の中の該当トークン**（[`Field::value_at`] が要る理由）。M2 は 1 つの `mode:` の
/// 値の中だけで数える: 場面レベルと節レベルの継承・統合は §6.4 の解決層（M3）が行う。
/// `mode_id` の形をしていない修飾子（資料タイトルなど）は E003 / E004 の領域なので触らない。
fn warn_modes(found: &Field<'_>, offset: usize, out: &mut Vec<Diagnostic>) {
    let value_at = offset + found.value_at;
    let mut seen: Vec<&str> = Vec::new();
    for reference in found.refs() {
        let Some(modifier) = reference.modifier else {
            continue;
        };
        let twice = seen.contains(&reference.name);
        let too_many = mode_count(modifier).is_some_and(|count| count > MODE_MAX);
        if twice || too_many {
            let range = token_range(value_at, &reference);
            out.push(Diagnostic::simple(DiagnosticCode::E005, Some(range)));
        }
        if !twice {
            seen.push(reference.name);
        }
    }
}

/// `mode_expr`（§5.1 `mode_id , [ "+" , mode_id ]`）なら**モードの数**、そうでなければ `None`。
///
/// `mode_id` の形をしていない部分が 1 つでもあればモード式ではない（資料タイトルなど）ので、
/// E005 ではなく E003 / E004（M3、`characters.md` の辞書が要る）の領域。空の修飾子
/// （`'アルス:`）も `mode_id` でないので `None`。
fn mode_count(text: &str) -> Option<usize> {
    let parts = text.split(contents::MODE_JOIN);
    if !parts.clone().all(contents::is_mode_id) {
        return None;
    }
    Some(parts.count())
}

/// 値の中のトークンの位置を行内の位置から文書内の位置にする。
fn token_range(value_at: usize, found: &CharRef<'_>) -> Range<usize> {
    value_at + found.range.start..value_at + found.range.end
}

/// フィールド行の直後の継続行を、そのフィールドの値に付ける（§5.0.5、B-37）。
///
/// A 形式の [`super::plot_outline`] と同じ規則だが、行の型が形式ごとに違うのでループは
/// ここにも持つ（`drop_indent` の 2 文字という規則そのものは共有している）。
fn attach_continuations(lines: &mut [Line<'_>]) {
    let mut owner: Option<usize> = None;
    for index in 0..lines.len() {
        if let Line::Field(_) = &lines[index] {
            owner = Some(index);
            continue;
        }
        let Some(text) = continuation_text(&lines[index]) else {
            // 空行・コメント・自由文は見出しと同じく継続を中断する
            owner = None;
            continue;
        };
        if let Some(at) = owner {
            if let Line::Field(found) = &mut lines[at] {
                found.extra.push(text);
            }
        }
    }
}

/// 継続行なら、値に寄与するテキスト（インデントから 2 文字落とした残り）。それ以外は `None`。
fn continuation_text<'a>(line: &Line<'a>) -> Option<&'a str> {
    match line {
        Line::Continuation(found) => Some(plot_outline::drop_indent(found.raw)),
        _ => None,
    }
}

// ============================================================ 場面と節

/// 解析中の節。
struct OpenSection {
    /// 節の最初の行（見出しの直後、または `;` の直後）。
    start: usize,
    /// 次に閉じる節の番号（`D`）。
    number: u16,
    /// 節レベルのフィールド行の index。
    fields: Vec<usize>,
    /// `point_line` の index。
    points: Vec<usize>,
}

impl OpenSection {
    /// `;` を跨いだ次の節（§5.2.2「以降 `;` ごとに `D` が 1 ずつ増える」）。
    ///
    /// `saturating_add` は 10000 個以上の `;` で `u16` が溢れて**パニックしない**ため（§4.3-4）。
    fn next_section(&self, start: usize) -> Self {
        Self {
            start,
            number: self.number.saturating_add(1),
            fields: Vec::new(),
            points: Vec::new(),
        }
    }

    /// この節が占める行範囲（区切り `;` を含まない）。
    fn span(&self, stop: usize) -> Range<usize> {
        self.start..stop
    }
}

/// 解析中の場面。
struct OpenScene {
    /// 見出し行の index。
    header: usize,
    /// 見出しの ID。
    id: Option<SceneId>,
    /// 見出しの階層。
    depth: usize,
    /// 場面レベルのフィールド行の index（最初の `;` より前）。
    fields: Vec<usize>,
    /// 確定した節。
    sections: Vec<Section>,
    /// いま開いている節。
    section: OpenSection,
    /// まだ `scene_field` の領域にいる（＝フィールドは場面レベルに入る、§5.4.1）。
    ///
    /// 最初の `point_line` か最初の `;` で `false` になる。
    scene_scope: bool,
}

/// 行の並びから前文・場面・節を作る（診断は [`parse_lines`] が出し終わっている）。
fn collect(lines: &[Line<'_>]) -> (Preamble, Vec<Scene>) {
    // §5.4.1 `episode_plot = [ preamble ] , { scene_block }`。見出しより前だけが前文
    let head = lines.iter().position(|line| matches!(line, Line::Scene(_)));
    let mut preamble = Preamble {
        span: 0..head.unwrap_or(lines.len()),
        fields: Vec::new(),
    };
    let mut scenes = Vec::new();
    let mut open: Option<OpenScene> = None;
    for (index, line) in lines.iter().enumerate() {
        match line {
            Line::Scene(found) => {
                if let Some(current) = open.take() {
                    finish_scene(current, index, lines, &mut scenes);
                }
                open = Some(OpenScene {
                    header: index,
                    id: found.id,
                    depth: found.depth,
                    fields: Vec::new(),
                    sections: Vec::new(),
                    section: OpenSection {
                        start: index + 1,
                        number: found.section_start().unwrap_or(1),
                        fields: Vec::new(),
                        points: Vec::new(),
                    },
                    scene_scope: true,
                });
            }
            Line::Break(_) => {
                // 見出しより前の `;` は何もしない（C 形式の B-32 と同じ）
                let Some(current) = open.as_mut() else {
                    continue;
                };
                if current.scene_scope {
                    // 節の内容が 1 行も無いまま区切られた（§5.4.5 の例）
                    current.section.start = index;
                }
                let next = current.section.next_section(index + 1);
                let closed = std::mem::replace(&mut current.section, next);
                current
                    .sections
                    .push(close_section(closed, index, Some(index), lines));
                current.scene_scope = false;
            }
            Line::Field(_) => match open.as_mut() {
                Some(current) if current.scene_scope => current.fields.push(index),
                Some(current) => current.section.fields.push(index),
                None if index < preamble.span.end => preamble.fields.push(index),
                // 場面の外（前文より後ろで、見出しが無いまま）はどのスコープにも入らない
                None => {}
            },
            Line::Point(_) => {
                // 見出しより前の自由文はどの節にも入らない（前文、B-32 と同じ）
                let Some(current) = open.as_mut() else {
                    continue;
                };
                if current.scene_scope {
                    // §5.4.1: `section` は最初の `point_line` から始まる
                    current.scene_scope = false;
                    current.section.start = index;
                }
                current.section.points.push(index);
            }
            Line::Continuation(_) | Line::Comment(_) | Line::Blank(_) => {}
        }
    }
    if let Some(current) = open.take() {
        finish_scene(current, lines.len(), lines, &mut scenes);
    }
    (preamble, scenes)
}

/// 場面を閉じて `scenes` に積む。
fn finish_scene(mut open: OpenScene, stop: usize, lines: &[Line<'_>], scenes: &mut Vec<Scene>) {
    if open.scene_scope {
        // 場面レベルの領域のまま次の見出し（またはファイル末尾）まで来た＝節は 1 行も無い
        // （§5.4.1 `section` は 0 行でもよい）。場面フィールドを節の内容に含めない。
        open.section.start = stop;
    }
    // ファイル末尾。最後の区切りの後ろに**内容が無ければ**節を作らない（B-35 と同じ規則）
    if has_body(&open.section.span(stop), lines) {
        let closed = std::mem::replace(
            &mut open.section,
            OpenSection {
                start: stop,
                number: 0,
                fields: Vec::new(),
                points: Vec::new(),
            },
        );
        open.sections.push(close_section(closed, stop, None, lines));
    }
    scenes.push(Scene {
        header: open.header,
        id: open.id,
        depth: open.depth,
        span: open.header..stop,
        fields: open.fields,
        sections: open.sections,
    });
}

/// 節を確定する。
fn close_section(
    open: OpenSection,
    stop: usize,
    break_line: Option<usize>,
    lines: &[Line<'_>],
) -> Section {
    let span = open.span(stop);
    Section {
        number: open.number,
        content: content_span(&span, lines),
        break_line,
        fields: open.fields,
        points: open.points,
        span,
    }
}

/// `span` から末尾の空行を除く（§5.2.2「節の末尾の空行は節の内容に含めない」MUST）。
fn content_span(span: &Range<usize>, lines: &[Line<'_>]) -> Range<usize> {
    let mut end = span.end;
    while end > span.start && lines[end - 1].is_blank() {
        end -= 1;
    }
    span.start..end
}

/// 範囲に空行以外の行があるか（C 形式の `number_sections` の `has_body` と同じ判定）。
fn has_body(span: &Range<usize>, lines: &[Line<'_>]) -> bool {
    !content_span(span, lines).is_empty()
}
