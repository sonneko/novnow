//! C 形式（本文 `contents/AA-BB.txt`）のパーサ（`SPECIFICATIONS.md` §5.2）。
//!
//! 字句器（[`crate::core::text::lexer`]）が**行単位**で決めた種別を、§5.2.1 の文法に
//! 組み立て直す。判定は行内で完結するので（§5.0.1）[`Line`] は 1 行ずつ差し替えられる
//! （§6.7 のインクリメンタル更新の単位）。**前後の行を見るのは節番号だけ**なので、
//! ファイル全体が要るのは [`Contents::sections`] だけ（§5.2.2、§6.7 の再番号付けと同じ境目）。
//!
//! # 原文を 1 バイトも捨てない
//!
//! 各行は `raw`（改行を含まない原文）を持ち、[`Contents::to_text`] は `raw` を LF で連結して
//! **入力と一致する文字列**を返す（`PLANS.md` §5.3 の完了条件 2）。① で解析しない構文 —
//! `^M0137` 引用注釈（§5.2.4）、`! ` 挿絵メモ（§5.2.5）、`# ` コメント（§5.0.8）、
//! `'` 参照（§5.2.1 `ref`）、行内コメント `/* … */` — は [`TextLine::body`] にそのまま残り、
//! round-trip で戻る（本書 §1.2 の 10 / 23 / 15）。
//!
//! # ① で解析する構文（`PLANS.md` §5.3 作業項目 1）
//!
//! | 構文 | 仕様 | 型 |
//! | --- | --- | --- |
//! | 場面見出し `@A-B-C[-D] タイトル` | §5.2.1 | [`SceneLine`] |
//! | 節区切り `;`（行末コメント可） | §5.2.1 | [`BreakLine`] |
//! | 行コメント `// …` | §5.0.2 | [`CommentLine`] |
//! | 空行 | §5.2.1 `blank` | [`Line::Blank`] |
//! | 本文と行末話者注釈 `@名前[:モード]` | §5.2.1 | [`TextLine`]・[`Speaker`] |
//! | ルビ `｜親字《読み》`・`漢字《読み》` | §5.2.1 | [`Ruby`] |
//! | 節番号（D） | §5.2.2 | [`Section`] |
//!
//! 話者注釈の**名前解決**（§6.2.5 の辞書最短一致、E012）と 1 節あたりのモード数制限（E005）、
//! `'` 参照の解決は辞書が要るので **M3**（§6.4）。ここは形だけを決める。
//!
//! # 位置
//!
//! - [`Line`] 側の `range` / `sigil_at` は**その行の `raw` の中**の byte offset。
//! - [`Diagnostic::range`] は**文書全体（[`parse`] に渡した `text`）の中**の byte offset
//!   （§3.4「core は内部的に byte offset を使う」）。
//!
//! # パニックしない
//!
//! 壊れた入力でも部分木と診断を返す（§4.3-4、`PLANS.md` §5.3 の完了条件 3）。ID が壊れた
//! 見出しも節を作り（[`Section::scene`] が `None`）、話者注釈の形に合わない `@` は W002 を
//! 出して本文に残す。
//!
//! # 改行
//!
//! `text` は **LF 改行**であること。読み込み時に [`crate::core::text::lexer::normalize_on_load`]
//! を通す（§5.0.4「内部表現は LF に統一」MUST）ので、CRLF のファイルはそこで LF になる。

use std::ops::Range;

use serde::Serialize;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::id::{self, PartialSceneId, SceneId};
use crate::core::text::lexer::{self, LineKind, TextFormat};

/// §5.0.2 の行コメントのシジル（`lexer::SIGIL_TABLE` の `Sigil::LineComment` と同じ）。
const LINE_COMMENT: &str = "//";

/// C 形式の見出しに書ける最少の階層数（§5.2.1 `scene_id = num "-" num "-" num [ "-" num ]`）。
const HEADER_DEPTH: usize = 3;

/// 階層数だけが違う見出し（`@01-04`）の **E020** に渡す `{0}`（§6.2.5、第 2.8 版）。
///
/// C・B 形式は 3 階層と 4 階層の両方を受け付けるので、数 1 つではなく範囲を渡す
/// （`{0}` に 3 を入れると「3 階層だけ」と読めて `@01-04-02-03` と矛盾する。`PLANS.md` §9.1 A-10）。
/// 言語に依らない数字と `-` だけなので core が持ってよい（§12.2 の文言には当たらない）。
pub const HEADER_DEPTH_RANGE: &str = "3-4";

/// `mode_id = /[A-Za-z][A-Za-z0-9_]{0,15}/`（§5.5）の長さ上限（先頭 1 字＋最大 15 字）。
const MODE_ID_MAX: usize = 16;

/// 話者注釈の名前とモード式を分ける（§5.1 `speaker_annot`）。
const SPEAKER_SEP: char = ':';

/// `mode_expr = mode_id , [ "+" , mode_id ]`（§5.1）の連結子。
pub(crate) const MODE_JOIN: char = '+';

/// ルビの括弧（§5.2.1 `ruby`）。
const RUBY_OPEN: char = '《';

/// ルビの閉じ括弧。
const RUBY_CLOSE: char = '》';

/// ルビの明示形 `｜base《reading》` の区切り。**半角 `|` ではない**（§5.2.1）。
const RUBY_BAR: char = '｜';

/// 漢字の code point 範囲（**上端は含まない**）。ルビの親文字になれる文字、§5.2.1 `kanji_run`。
///
/// CJK 統合漢字 U+4E00–U+9FFF・拡張 A U+3400–U+4DBF・互換漢字 U+F900–U+FAFF。
/// `kanji_run` は仕様書に定義が無いのでここで決めた（`PLANS.md` §9.2 B-33）。
const KANJI: &[Range<u32>] = &[0x4E00..0xA000, 0x3400..0x4DC0, 0xF900..0xFB00];

/// 繰返し字（`々` U+3005・`〻` U+303B）。`人々《ひとびと》` のように漢字列に含める。
const KANJI_MARKS: &[u32] = &[0x3005, 0x303B];

/// C 形式の解析に効く設定（`project.json` の `structure`、§4.5 / §5.1.2）。
///
/// **core は設定ファイルを読めない**（§3.2）ので、呼び出し側が値を渡す
/// （[`crate::core::id::Digits`] と同じ扱い）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Options {
    /// `A = 00`（未配属ストック章、§5.1.2）を許すか。
    ///
    /// `false` のとき `@00-…` の見出しは **E022**（予約番号、第 2.8 版。旧 E020）。§5.1.2「`00` は番号としては正当な値であり、
    /// パースを拒否しない（MUST）」により既定は `true`（ID としては常に有効で、拒否するのは
    /// この設定を `false` にしたプロジェクトだけ）。
    pub allow_chapter_zero: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            allow_chapter_zero: true,
        }
    }
}

/// C 形式 1 ファイルの解析結果（§5.2）。
///
/// `text` を借用しているので、元の文字列を生かしているあいだだけ使える。編集で文字列が
/// 変わったら解析し直す（§6.7）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Contents<'a> {
    /// 全行。**改行を含まない**（[`Contents::to_text`] が LF を挟んで復元する）。
    pub lines: Vec<Line<'a>>,
    /// 節（§5.2.2）。**ファイル先頭の場面見出しより前の行はどの節にも属さない**
    /// （`PLANS.md` §9.2 B-32）。
    pub sections: Vec<Section>,
    /// 診断（§7）。位置は `text` の中の byte range。行の順・節の順に並ぶ。
    pub diagnostics: Vec<Diagnostic>,
}

/// C 形式を解析する（§5.2）。設定は既定値（[`Options::default`]）。
///
/// 壊れた入力でもパニックしない（§4.3-4）。
pub fn parse(text: &str) -> Contents<'_> {
    parse_with(text, &Options::default())
}

/// 設定を渡して C 形式を解析する。
pub fn parse_with<'a>(text: &'a str, options: &Options) -> Contents<'a> {
    let mut lines = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen: Vec<(SceneId, usize)> = Vec::new();
    let mut at = 0;
    for raw in text.split('\n') {
        let line = parse_line(raw, at, options, &mut diagnostics);
        if let Line::Scene(found) = &line {
            warn_duplicate(found, at, &mut seen, &mut diagnostics);
        }
        lines.push(line);
        at += raw.len() + 1;
    }
    let sections = number_sections(&lines, &mut diagnostics);
    Contents {
        lines,
        sections,
        diagnostics,
    }
}

impl<'a> Contents<'a> {
    /// 原文を復元する（`lines` の `raw` を LF で連結）。
    ///
    /// [`parse`] に渡した文字列と**一致する**（`PLANS.md` §5.3 の完了条件 2）。末尾改行の有無、
    /// 空白だけの行、① が解析しない構文もすべて戻る。
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

    /// `lines[index]` の行が属する節（§5.2.2）。
    ///
    /// 場面見出しより前の行と、見出し行・区切り行 `;` 自身はどの節にも属さない（`None`）。
    pub fn section_of(&self, index: usize) -> Option<&Section> {
        self.sections
            .iter()
            .find(|section| section.span.contains(&index))
    }
}

/// 1 行（§5.2.1 `line`）。
///
/// ① は `illust_line`（§5.2.5、② の追加仕様 23）と `comment_line` の `# `（§5.0.8、②）を
/// 作らない。それらは [`Line::Text`] として本文に残る。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Line<'a> {
    /// 場面見出し（§5.2.1 `scene_header`）。
    Scene(SceneLine<'a>),
    /// 節区切り（§5.2.1 `section_break`）。
    Break(BreakLine<'a>),
    /// 行コメント（§5.2.1 `comment_line`）。
    Comment(CommentLine<'a>),
    /// 空行（§5.2.1 `blank`）。中身は**空白文字列のまま**の原文。
    Blank(&'a str),
    /// 本文行（§5.2.1 `text_line`）。
    Text(TextLine<'a>),
}

impl<'a> Line<'a> {
    /// 行全体（改行を含まない原文）。
    pub fn raw(&self) -> &'a str {
        match self {
            Self::Scene(line) => line.raw,
            Self::Break(line) => line.raw,
            Self::Comment(line) => line.raw,
            Self::Blank(line) => line,
            Self::Text(line) => line.raw,
        }
    }

    /// 空行か（§5.2.2「節の末尾の空行」を数えるのに使う）。
    pub fn is_blank(&self) -> bool {
        matches!(self, Self::Blank(_))
    }
}

/// 場面見出し行（§5.2.1 `scene_header`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SceneLine<'a> {
    /// 行全体。
    pub raw: &'a str,
    /// 先頭の空白文字数（`ws?`）。
    pub indent: usize,
    /// `@` を除いた ID 部分。**壊れていても書かれたまま**（round-trip 用）。
    pub id_text: &'a str,
    /// パースした場面 ID。3〜4 階層でなければ `None`（診断は 1〜2 階層なら E020、
    /// 形が崩れていれば E002。§6.2.5）。
    pub id: Option<SceneId>,
    /// 書かれた階層の数。`01-04` なら 2、形が数字と `-` でなければ 0。
    pub depth: usize,
    /// タイトル（`@01-04-02 図書室の攻防` の `図書室の攻防`）。無ければ `""`。
    pub title: &'a str,
    /// 行末コメント（§5.2.1 `[ comment ]`）。
    pub comment: Option<Comment<'a>>,
}

impl<'a> SceneLine<'a> {
    /// 見出しに `-D` が明示されていればその節番号（§5.2.2「その見出し直後の節番号を `D` から
    /// 開始する」）。
    pub fn section_start(&self) -> Option<u16> {
        if self.depth > HEADER_DEPTH {
            self.id.map(|found| found.section)
        } else {
            None
        }
    }
}

/// 節区切り行（§5.2.1 `section_break`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BreakLine<'a> {
    /// 行全体。
    pub raw: &'a str,
    /// 先頭の空白文字数（`ws?`）。
    pub indent: usize,
    /// `;` の byte 位置（`raw` の中）。
    pub sigil_at: usize,
    /// 行末コメント（`; // メモ`、§5.2.1 `[ comment ]`）。
    pub comment: Option<Comment<'a>>,
}

/// 行コメント行（§5.2.1 `comment_line`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentLine<'a> {
    /// 行全体。
    pub raw: &'a str,
    /// 先頭の空白文字数（`ws?`）。
    pub indent: usize,
    /// コメント本体。行頭コメントなので `//` は必ずある（§5.0.2）。
    pub comment: Comment<'a>,
}

/// 行末コメント（§5.0.2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comment<'a> {
    /// `//` を含む範囲（`raw` の中）。
    pub range: Range<usize>,
    /// `//` の直後から行末まで（**トリムしていない**）。
    pub text: &'a str,
}

/// 本文行（§5.2.1 `text_line`）。
///
/// `raw` は `body` ＋ 話者注釈 ＋ 行末コメントに**隙間なく分かれる**（それぞれ `range` で
/// 切り出せる）。行内コメント `/* … */` は [`TextLine::inline_comment`] に位置だけを持ち、
/// 文字列は `body` に残る（除去は §17.1 のエクスポート、M9）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextLine<'a> {
    /// 行全体。
    pub raw: &'a str,
    /// 先頭の空白文字数。2 以上は継続行（§5.0.5）だが、C 形式では一字下げ・段落字下げの
    /// **正当な本文**なので [`Line::Text`] として扱う（`PLANS.md` §9.2 B-31）。
    pub indent: usize,
    /// 本文（話者注釈と行末コメントを除いた部分。先頭の空白は**含む**）。
    pub body: &'a str,
    /// 行末話者注釈（§5.2.1 `speaker_annot`）。
    pub speaker: Option<Speaker<'a>>,
    /// ルビ（§5.2.1 `ruby`）。出現順。
    pub rubies: Vec<Ruby<'a>>,
    /// 行内コメント `/* … */` の範囲（`raw` の中）。閉じていなければ `None`（§5.0.1）。
    pub inline_comment: Option<Range<usize>>,
    /// 行末コメント（§5.0.2）。
    pub comment: Option<Comment<'a>>,
}

/// 行末話者注釈（§5.2.1 `speaker_annot`、§6.2.5「行末の `@名前`」）。
///
/// **名前の解決は M3**（§6.2.5 の辞書最短一致、E012）。ここは形だけ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Speaker<'a> {
    /// 注釈全体（**直前の空白を含む**、§17.1-5「直前の空白も削除」）。`raw` の中の範囲。
    pub range: Range<usize>,
    /// `@` の byte 位置（`raw` の中）。
    pub sigil_at: usize,
    /// キャラ名（`@` の直後から `:` または注釈の終わりまで）。
    pub name: &'a str,
    /// モード式（`:B` / `:B+D` の部分）。無ければ空。§5.1 `mode_expr` は最大 2 つ。
    pub modes: Vec<&'a str>,
}

/// ルビ（§5.2.1 `ruby`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ruby<'a> {
    /// ルビ全体の範囲（`raw` の中）。`｜` や親文字を含む。
    pub range: Range<usize>,
    /// 親文字（`｜base《reading》` の base、または `《` の直前の漢字列）。
    pub base: &'a str,
    /// 読み（`《…》` の内側）。
    pub reading: &'a str,
    /// `｜` で始まる明示形か（`false` なら漢字列に直接 `《` が続く形）。
    pub explicit: bool,
}

/// 1 つの節（§5.2.2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    /// 節番号（D）。§5.2.2 の規則で確定した値。
    pub number: u16,
    /// 所属する場面。見出しの ID が壊れていれば `None`（診断は E002 / E020）。
    pub scene: Option<SceneId>,
    /// この節を支配する場面見出しの行 index（`lines` の中）。
    pub scene_header: Option<usize>,
    /// 節の行（**見出し行と区切り行 `;` を含まない**）。`lines` の index 範囲。
    pub span: Range<usize>,
    /// 節の内容（`span` から**末尾の空行を除いた**もの、§5.2.2 MUST）。
    pub content: Range<usize>,
}

/// `number_sections` の内部状態（いま開いている節）。
#[derive(Debug, Clone, Copy)]
struct Open {
    /// 次に閉じる節の番号。
    number: u16,
    /// 支配する場面の見出しから引き継いだ ID。
    scene: Option<SceneId>,
    /// 支配する場面の見出しの行 index。
    header: Option<usize>,
}

impl Open {
    /// `;` を跨いだ次の節（§5.2.2「以降 `;` ごとに `D` が 1 ずつ増える」）。
    ///
    /// `saturating_add` は 10000 個以上の `;` で `u16` が溢れて**パニックしない**ため
    /// （§4.3-4）。§5.1 の `num = /\d{1,4}/` を超えた番号は M4 の診断が拾う。
    fn next_section(self) -> Self {
        Self {
            number: self.number.saturating_add(1),
            scene: self.scene,
            header: self.header,
        }
    }
}

/// §5.2.2 の節番号を確定する作業場（節を積みながら診断を溜める）。
struct Numbering {
    /// 各行の開始 byte offset。
    offsets: Vec<usize>,
    /// 確定した節。
    sections: Vec<Section>,
    /// 節の診断（W012）。
    diagnostics: Vec<Diagnostic>,
}

impl Numbering {
    fn new(lines: &[Line<'_>]) -> Self {
        Self {
            offsets: line_offsets(lines),
            sections: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// 節を 1 つ確定する。内容が空なら **W012**（§5.2.2）。
    fn push(&mut self, open: Open, span: Range<usize>, lines: &[Line<'_>]) {
        let content = content_span(&span, lines);
        if content.is_empty() {
            self.warn_empty(&span, lines);
        }
        self.sections.push(Section {
            number: open.number,
            scene: open.scene,
            scene_header: open.header,
            span,
            content,
        });
    }

    /// 空の節の **W012**（§5.2.2「`;` が連続した場合、空の節が生成される」）。
    ///
    /// 指す行は**節を閉じた行**（次の区切り `;` か次の見出し）。quickfix「削除」で消せる行に
    /// 合わせる。ファイル末尾まで閉じられていないときは、節を開いた行（見出しか `;`）を指す
    /// （末尾の空行を指すと幅 0 の範囲になってしまう）。
    fn warn_empty(&mut self, span: &Range<usize>, lines: &[Line<'_>]) {
        let target = if span.end < lines.len() {
            span.end
        } else {
            span.start.saturating_sub(1)
        };
        let at = self.offsets[target];
        let range = at..at + lines[target].raw().len();
        let found = Diagnostic::simple(DiagnosticCode::W012, Some(range));
        self.diagnostics.push(found);
    }
}

fn parse_line<'a>(
    raw: &'a str,
    offset: usize,
    options: &Options,
    out: &mut Vec<Diagnostic>,
) -> Line<'a> {
    // §5.2.1 `blank = ws? , eol`。字句器は空行を Text / Continuation で返すのでここで決める。
    // 空白の文字数は問わない（保存時に末尾空白は落ちる、§17.1。`PLANS.md` §9.2 B-34）
    if raw.trim_matches(lexer::is_space).is_empty() {
        return Line::Blank(raw);
    }
    match lexer::classify(raw, TextFormat::Contents) {
        LineKind::Structure(_) => Line::Scene(parse_scene(raw, offset, options, out)),
        LineKind::SectionBreak => Line::Break(parse_break(raw)),
        LineKind::Comment { .. } => Line::Comment(parse_comment(raw)),
        // C 形式にフィールド行は無い（§5.0.5 の表、`TextFormat::allows_field`）。継続行も
        // 一字下げ・段落字下げの本文なので [`Line::Text`]（B-31）。② の `! ` も本文
        LineKind::Field(_)
        | LineKind::Continuation { .. }
        | LineKind::IllustrationMemo(_)
        | LineKind::Text(_) => Line::Text(parse_text(raw, offset, out)),
    }
}

pub(crate) fn parse_scene<'a>(
    raw: &'a str,
    offset: usize,
    options: &Options,
    out: &mut Vec<Diagnostic>,
) -> SceneLine<'a> {
    let indent = lexer::leading_indent(raw);
    let sigil = body_start(raw);
    let comment = find_comment(raw);
    // `@` から（行末コメントがあればその手前まで）が見出しの中身
    let head = match &comment {
        Some(found) => &raw[sigil..found.range.start],
        None => &raw[sigil..],
    };
    let after = &head[id::SIGIL.len_utf8()..];
    let id_len = after.find(lexer::is_space).unwrap_or(after.len());
    let id_text = &after[..id_len];
    let title = after[id_len..].trim_matches(lexer::is_space);
    let (id, depth) = parse_scene_id(id_text);
    let id_start = offset + sigil;
    let id_range = id_start..id_start + id::SIGIL.len_utf8() + id_text.len();
    match id {
        // §6.2.5（第 2.8 版）: ID としては読めるが階層が足りない（`@01-04`）なら E020。
        // 作者の次の行動は「形を直す」ではなく「対応するファイルへ移す」なので（A-10）
        None if depth > 0 => {
            let args = vec![Arg::text(HEADER_DEPTH_RANGE)];
            out.push(Diagnostic::new(DiagnosticCode::E020, Some(id_range), args));
        }
        // それ以外の形の崩れ（数字と `-` でない・5 階層以上）は E002
        None => out.push(Diagnostic::simple(DiagnosticCode::E002, Some(id_range))),
        Some(found) => warn_reserved(found, id_text, options, &id_range, out),
    }
    SceneLine {
        raw,
        indent,
        id_text,
        id,
        depth,
        title,
        comment,
    }
}

/// `scene_id` をパースする（§5.1）。3〜4 階層のときだけ [`SceneId`] を返す。
fn parse_scene_id(text: &str) -> (Option<SceneId>, usize) {
    match PartialSceneId::parse(text) {
        Ok(found) if found.depth() >= HEADER_DEPTH => (Some(found.expand_low()), found.depth()),
        Ok(found) => (None, found.depth()),
        Err(_) => (None, 0),
    }
}

/// 予約番号の見出し（§5.1.2 の `C = 00` と、拒否設定の `A = 00`）を **E022** で報告する。
///
/// `{0}` は書かれたままの ID（`@` を除く）。第 2.8 版で E020 から分けた（`PLANS.md` §9.1 A-9）。
fn warn_reserved(
    id: SceneId,
    id_text: &str,
    options: &Options,
    range: &Range<usize>,
    out: &mut Vec<Diagnostic>,
) {
    let scene_zero = id.scene == 0;
    let chapter_zero = id.chapter == 0 && !options.allow_chapter_zero;
    if scene_zero || chapter_zero {
        let args = vec![Arg::text(id_text)];
        out.push(Diagnostic::new(
            DiagnosticCode::E022,
            Some(range.clone()),
            args,
        ));
    }
}

/// 見出しの ID が同一ファイル内で重複していないか（**E008**、§7「同一ファイル内の ID 重複」）。
///
/// C 形式（本文）と B 形式（話プロット）で共有する（B-40）。比べるのは **ID と階層の組**:
/// `@01-04-02` の後の `@01-04-02-03` は「同じ場面の 3 節目から」（§5.2.2）で重複ではない。
/// C 形式は M2 では検査していなかった（M4 で追加。`PLANS.md` §9.2 B-87）。
pub(crate) fn warn_duplicate(
    found: &SceneLine<'_>,
    offset: usize,
    seen: &mut Vec<(SceneId, usize)>,
    out: &mut Vec<Diagnostic>,
) {
    // ID が壊れているときは E002 / E020 が出ている（[`parse_scene`]）のでここでは何もしない
    let Some(id) = found.id else {
        return;
    };
    let start = offset + body_start(found.raw);
    let range = start..start + id::SIGIL.len_utf8() + found.id_text.len();
    if seen.contains(&(id, found.depth)) {
        let args = vec![Arg::text(found.id_text)];
        out.push(Diagnostic::new(DiagnosticCode::E008, Some(range), args));
    }
    seen.push((id, found.depth));
}

pub(crate) fn parse_break<'a>(raw: &'a str) -> BreakLine<'a> {
    BreakLine {
        raw,
        indent: lexer::leading_indent(raw),
        sigil_at: body_start(raw),
        comment: find_comment(raw),
    }
}

pub(crate) fn parse_comment<'a>(raw: &'a str) -> CommentLine<'a> {
    // Hash comments occupy the entire line, even when their text contains `//`.
    let comment = if raw.trim_start().starts_with('#') {
        whole_line_comment(raw)
    } else {
        find_comment(raw).unwrap_or_else(|| whole_line_comment(raw))
    };
    CommentLine {
        raw,
        indent: lexer::leading_indent(raw),
        comment,
    }
}

/// 行全体を 1 件のコメントとして扱う（`# ` コメントおよび解析失敗時の保険）。
fn whole_line_comment(raw: &str) -> Comment<'_> {
    Comment {
        range: 0..raw.len(),
        text: raw,
    }
}

fn parse_text<'a>(raw: &'a str, offset: usize, out: &mut Vec<Diagnostic>) -> TextLine<'a> {
    let indent = lexer::leading_indent(raw);
    let comment = find_comment(raw);
    let inline = lexer::find_inline_comment(raw);
    // §5.2.1 の並ぶ順（本文 → 引用注釈 → 話者注釈 → コメント）どおり、右から剥がしていく
    let head = match &comment {
        Some(found) => &raw[..found.range.start],
        None => raw,
    };
    let at_signs = at_positions(head, inline.as_ref());
    let speaker = trailing_speaker(head, &at_signs);
    let body = match &speaker {
        Some(found) => &raw[..found.range.start],
        None => head,
    };
    let rubies = find_rubies(body, &escaped_positions(body));
    warn_bare_at(&at_signs, speaker.as_ref(), offset, out);
    TextLine {
        raw,
        indent,
        body,
        speaker,
        rubies,
        inline_comment: inline,
        comment,
    }
}

/// 行末話者注釈を探す（§5.2.1 `speaker_annot`、§6.2.5「行末の `@名前`」）。
///
/// **行末側の最後の `@` だけ**が話者注釈になれる（それより前の `@` は W002）。形が
/// `@char_name [":" mode_expr]` に合わなければ話者注釈ではない（`@` は W002 のまま残る）。
fn trailing_speaker<'a>(text: &'a str, at_signs: &[usize]) -> Option<Speaker<'a>> {
    let sigil_at = *at_signs.last()?;
    let end = text.trim_end_matches(lexer::is_space).len();
    let tail = &text[sigil_at + id::SIGIL.len_utf8()..end];
    let (name, modes) = split_speaker(tail)?;
    // 直前の空白も注釈に含める（§17.1-5「直前の空白も削除」）
    let start = text[..sigil_at].trim_end_matches(lexer::is_space).len();
    Some(Speaker {
        range: start..text.len(),
        sigil_at,
        name,
        modes,
    })
}

/// `char_name [":" mode_expr]` を分解する（§5.1）。
fn split_speaker(tail: &str) -> Option<(&str, Vec<&str>)> {
    match tail.split_once(SPEAKER_SEP) {
        Some((name, expr)) => {
            let modes = split_modes(expr)?;
            is_char_name(name).then_some((name, modes))
        }
        None => is_char_name(tail).then_some((tail, Vec::new())),
    }
}

/// `mode_expr = mode_id , [ "+" , mode_id ]`（§5.1）。3 つ以上あれば話者注釈にしない。
pub(crate) fn split_modes(expr: &str) -> Option<Vec<&str>> {
    let modes: Vec<&str> = expr.split(MODE_JOIN).collect();
    if modes.len() > 2 {
        return None;
    }
    for mode in &modes {
        if !is_mode_id(mode) {
            return None;
        }
    }
    Some(modes)
}

/// `mode_id = /[A-Za-z][A-Za-z0-9_]{0,15}/`（§5.5）。
pub(crate) fn is_mode_id(text: &str) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() || text.len() > MODE_ID_MAX {
        return false;
    }
    text.chars()
        .skip(1)
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// キャラ名（§5.5 `characters.md` の `# char_name`）。空白を含まないこと。
///
/// `char_name` は仕様書に文字種の定義が無いので「空白を含まない 1 字以上」とした
/// （`PLANS.md` §9.2 B-33）。名前の中に `:` があるとモード式と区別できないので、
/// その行は話者注釈にならない（W002）。
pub(crate) fn is_char_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(lexer::is_space)
}

/// 話者注釈にならなかった `@` は **W002**（§6.2.5「それ以外の位置」、quickfix `\@`）。
fn warn_bare_at(
    at_signs: &[usize],
    speaker: Option<&Speaker<'_>>,
    offset: usize,
    out: &mut Vec<Diagnostic>,
) {
    for &sigil in at_signs {
        if speaker.is_some_and(|found| found.sigil_at == sigil) {
            continue;
        }
        let range = offset + sigil..offset + sigil + id::SIGIL.len_utf8();
        out.push(Diagnostic::simple(DiagnosticCode::W002, Some(range)));
    }
}

/// エスケープ（§5.0.3）されていない `@` の byte 位置。行内コメントの中は除く。
fn at_positions(text: &str, inline: Option<&Range<usize>>) -> Vec<usize> {
    let escaped = escaped_positions(text);
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(found) = text[at..].find(id::SIGIL) {
        let index = at + found;
        at = index + id::SIGIL.len_utf8();
        if escaped.contains(&index) {
            continue;
        }
        if inline.is_some_and(|range| range.contains(&index)) {
            continue;
        }
        out.push(index);
    }
    out
}

/// エスケープ（§5.0.3 `\x`）でリテラル化された文字の byte 位置。
fn escaped_positions(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for escape in lexer::find_escapes(text) {
        if escape.target.is_some() {
            out.push(escape.start + 1);
        }
    }
    out
}

/// ルビを探す（§5.2.1 `ruby`）。
///
/// `《` を左から見て、閉じ `》` があり読みが空でなければルビ。親文字が取れなければ
/// （`｜` が無い・`《` の直前が漢字でない）ルビではなく本文のまま。`》` が行内に無ければ
/// それ以降を見ない（§5.0.1「1 行は常に単独で字句解析できる」）。
fn find_rubies<'a>(text: &'a str, escaped: &[usize]) -> Vec<Ruby<'a>> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(found) = text[at..].find(RUBY_OPEN) {
        let open = at + found;
        let reading_start = open + RUBY_OPEN.len_utf8();
        at = reading_start;
        if escaped.contains(&open) {
            continue;
        }
        let Some(found_close) = text[reading_start..].find(RUBY_CLOSE) else {
            break;
        };
        let close = reading_start + found_close;
        let reading = &text[reading_start..close];
        if reading.is_empty() {
            continue;
        }
        let Some(base) = ruby_base(text, open) else {
            continue;
        };
        let end = close + RUBY_CLOSE.len_utf8();
        out.push(Ruby {
            range: base.start..end,
            base: base.text,
            reading,
            explicit: base.explicit,
        });
    }
    out
}

/// `《` の親文字（§5.2.1 `ruby` の 2 形）。`start` はルビ全体の開始位置。
struct Base<'a> {
    /// `｜`（明示形）または漢字列（省略形）の先頭の byte 位置。
    start: usize,
    /// 親文字。
    text: &'a str,
    /// `｜base《reading》` の形か。
    explicit: bool,
}

fn ruby_base(text: &str, open: usize) -> Option<Base<'_>> {
    explicit_base(text, open).or_else(|| kanji_base(text, open))
}

/// 形 1: `｜base《reading》`（§5.2.1）。`｜` と `《` のあいだに `》` や `｜` があれば別の
/// ルビの残骸なので形 1 ではない。
fn explicit_base(text: &str, open: usize) -> Option<Base<'_>> {
    let bar = text[..open].rfind(RUBY_BAR)?;
    let start = bar + RUBY_BAR.len_utf8();
    let base = &text[start..open];
    if base.is_empty() || base.contains(RUBY_CLOSE) || base.contains(RUBY_BAR) {
        return None;
    }
    Some(Base {
        start: bar,
        text: base,
        explicit: true,
    })
}

/// 形 2: `kanji_run《reading》`（§5.2.1）。`《` の直前の漢字連続を親文字にする。
fn kanji_base(text: &str, open: usize) -> Option<Base<'_>> {
    let mut start = open;
    for (index, ch) in text[..open].char_indices().rev() {
        if !is_kanji(ch) {
            break;
        }
        start = index;
    }
    if start == open {
        return None;
    }
    Some(Base {
        start,
        text: &text[start..open],
        explicit: false,
    })
}

/// 漢字（ルビの親文字になれる文字、§5.2.1 `kanji_run`）。定義は [`KANJI`] の doc を参照。
fn is_kanji(c: char) -> bool {
    let code = u32::from(c);
    if KANJI_MARKS.contains(&code) {
        return true;
    }
    KANJI.iter().any(|range| range.contains(&code))
}

/// 行末コメント（§5.0.2）。`://` は URL なのでコメントにしない（字句器と同じ規則）。
///
/// `.txt` 3 形式に共通の形なので A 形式（[`super::plot_outline`]）と B 形式
/// （[`super::plot_episode`]）もこれを使う。[`Comment`] も同じ理由で共用する。
pub(crate) fn find_comment(raw: &str) -> Option<Comment<'_>> {
    let start = lexer::find_line_comment(raw)?;
    let text_start = start + LINE_COMMENT.len();
    Some(Comment {
        range: start..raw.len(),
        text: &raw[text_start..],
    })
}

/// 最初の非空白文字の byte 位置（行頭シジルの位置）。A / B 形式でも同じ計算が要る。
pub(crate) fn body_start(raw: &str) -> usize {
    raw.len() - lexer::after_indent(raw).len()
}

/// §5.2.2 の節番号を確定する。
///
/// - 場面見出しの直後から最初の `;` までが D=1。以降 `;` ごとに 1 増える。
/// - 見出しに `-D` が明示されていれば、その直後の節番号を D から開始する。
/// - 節の末尾の空行は内容に含めない。内容が空の節は **W012**。
/// - 場面見出しより前の行は節を作らない（B-32）。`;` も見出しより前では何もしない。
/// - ファイル末尾、最後の区切りの後ろに本文が無ければ節を作らない（`;` で終わるファイルに
///   空の節を足さない）。
fn number_sections(lines: &[Line<'_>], out: &mut Vec<Diagnostic>) -> Vec<Section> {
    let mut numbering = Numbering::new(lines);
    let mut open: Option<Open> = None;
    let mut start = 0;
    for (index, line) in lines.iter().enumerate() {
        match line {
            Line::Scene(found) => {
                if let Some(current) = open {
                    numbering.push(current, start..index, lines);
                }
                open = Some(Open {
                    number: found.section_start().unwrap_or(1),
                    scene: found.id,
                    header: Some(index),
                });
                start = index + 1;
            }
            Line::Break(_) => {
                if let Some(current) = open {
                    numbering.push(current, start..index, lines);
                    open = Some(current.next_section());
                    start = index + 1;
                }
            }
            Line::Comment(_) | Line::Blank(_) | Line::Text(_) => {}
        }
    }
    // ファイル末尾。最後の区切りの後ろに**本文が無ければ**節を作らない（`;` や改行で終わる
    // ファイルに空の節を足さない）。W012 は区切りに挟まれた空の節だけに出る
    let tail = start..lines.len();
    let has_body = !content_span(&tail, lines).is_empty();
    if let Some(current) = open.filter(|_| has_body) {
        numbering.push(current, tail, lines);
    }
    out.extend(numbering.diagnostics);
    numbering.sections
}

/// `span` から末尾の空行を除く（§5.2.2「節の末尾の空行は節の内容に含めない」MUST）。
fn content_span(span: &Range<usize>, lines: &[Line<'_>]) -> Range<usize> {
    let mut end = span.end;
    while end > span.start && lines[end - 1].is_blank() {
        end -= 1;
    }
    span.start..end
}

/// 各行の開始 byte offset（LF 区切り、[`Contents::to_text`] と同じ並び）。
fn line_offsets(lines: &[Line<'_>]) -> Vec<usize> {
    let mut out = Vec::with_capacity(lines.len());
    let mut at = 0;
    for line in lines {
        out.push(at);
        at += line.raw().len() + 1;
    }
    out
}
