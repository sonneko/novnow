//! アノテーション除去エクスポート（§17.1・§17.2、`PLANS.md` §5.10 M9）。
//!
//! [`run`] は**純粋関数**（MUST）: 文書を読むだけで、ファイルも時計も知らない。書き出し先を決めて
//! 書くのは呼び出し側（CLI の `novnow export`・GUI の `>export`）で、既定の名前は [`file_name`] が
//! 作る（日時の文字列は呼び出し側が時計から作って渡す）。
//!
//! # 処理順（§17.1 の表。① は 3b / 3c / 5b / 10b を飛ばす、`PLANS.md` §1.2）
//!
//! | # | 処理 | ここでの扱い |
//! |---|---|---|
//! | 1 | 対象節の収集 | [`select`]: 範囲式（§17.2）と状態の絞り込み（`--state`、`PLANS.md` §9.2 B-122） |
//! | 2 | 構造見出し行の除去 | 見出し行は節の行に入らない（`contents::Section::span`）。`insert_headings` なら章・話のタイトルを入れる |
//! | 3 | コメントの除去 | `//` 行は行ごと、行末の `//` と行内の `/* */` はその部分だけ。残りが空なら行ごと |
//! | 4 | フィールド行の除去 | C 形式にフィールド行は無い（§5.0.5。`key: value` の形の行も本文） |
//! | 5 | 行末話者注釈の除去 | 直前の空白ごと（`contents::Speaker::range` が空白を含む） |
//! | 6 | 参照の平文化 | `'名前` / `'{名前}` → 書かれた名前。修飾子 `:…` は消す。未解決の参照も `'` だけ消す |
//! | 7 | 資料参照の除去 | `'M0137[:…]` / `'S0042`（名前部が資料 ID の形）は修飾子ごと消す |
//! | 8 | エスケープの解除 | `\@` → `@` など（§5.0.3 の 7 種だけ。ほかの `\x` はそのまま） |
//! | 9 | ルビ | `keep_ruby` なら書かれたまま、でなければ親文字だけ |
//! | 10 | 節区切りの変換 | 出す節と節のあいだ 1 か所につき 1 回（`blank-line` / `none` / `separator`） |
//! | 11 | 連続空行の圧縮 | 2 行以上 → 1 行（先頭の空行も落とす） |
//! | 12 | 行頭字下げ | `indent_first_line` なら本文の行の頭に全角空白。`「` で始まる行と、既に字下げした行には入れない |
//! | 13 | 改行コード | `export.newline`（既定 `crlf`） |
//! | 14 | 末尾 | 末尾の空行を削り、改行 1 個で終える（出すものが無ければ空文字列） |
//!
//! 判断の記録は `PLANS.md` §9.2 B-122〜B-124。

use std::collections::BTreeMap;
use std::ops::Range;

use crate::core::features::sections::{self, SectionRef};
use crate::core::features::Project;
use crate::core::id::{Digits, RangeError, RangeExpr, RangeTerm, SceneId};
use crate::core::model::material::{MaterialId, PREFIX_MATERIAL, PREFIX_SEARCH};
use crate::core::model::project::{Export, Newline};
use crate::core::resolve::dictionary::{self, Dictionary};
use crate::core::syntax::contents::{self, Line, TextLine};
use crate::core::syntax::{plot_episode, plot_outline};
use crate::core::text::lexer::{self, FieldKey};

/// `paragraph_break: "separator"` の区切り行（§17.1-10）。
pub const SEPARATOR: &str = "\u{3000}\u{3000}\u{FF0A}";

/// 行頭字下げに入れる全角空白（§17.1-12）。
const INDENT: char = '\u{3000}';

/// 参照のシジル（§5.0.2 `RefSigil`）。
const REF_SIGIL: char = dictionary::REF_SIGIL;

/// 字下げしない行の先頭（§17.1-12「`「` で始まる行には入れない」）。
const OPEN_QUOTE: char = '「';

/// 範囲「全体」の、既定のファイル名での書き方（§17.2 `<範囲>`）。
pub const WHOLE: &str = "all";

/// ファイル名で範囲式の `..` の代わりに書く文字。
const LABEL_RANGE: char = '~';

/// ファイル名で範囲式の `,` の代わりに書く文字。
const LABEL_UNION: char = '_';

/// 節区切り `;` の変換（§17.1-10、`export.paragraph_break`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Break {
    /// `blank-line`: 空行 1 行（既定）。
    #[default]
    BlankLine,
    /// `none`: 何も入れない。
    None,
    /// `separator`: [`SEPARATOR`] の行。
    Separator,
}

impl Break {
    /// `project.json` の値から引く。表に無ければ `None`（呼び出し側は既定値に落とす）。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "blank-line" => Some(Self::BlankLine),
            "none" => Some(Self::None),
            "separator" => Some(Self::Separator),
            _ => None,
        }
    }
}

/// 出力の形式（§17.2「`.txt`（既定）、`.md`（見出しを `#` として出力）」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// `.txt`。
    #[default]
    Text,
    /// `.md`。挿入する見出しを `#`（章）/ `##`（話）で書く。
    Markdown,
}

impl Format {
    /// 書き出し先の名前から決める（`.md` で終われば Markdown、それ以外は `.txt`）。
    pub fn of_name(name: &str) -> Self {
        if name.to_ascii_lowercase().ends_with(".md") {
            Self::Markdown
        } else {
            Self::Text
        }
    }

    /// 拡張子（ドットなし）。
    pub fn extension(self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Markdown => "md",
        }
    }
}

/// 確定度（§5.3.2 / §5.4.2 の `state`）。**順序がある**: `draft` < `review` < `fixed`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    /// 下書き（`state` が書かれていないときもこれ）。
    Draft,
    /// 見直し中。
    Review,
    /// 確定。
    Fixed,
}

impl State {
    /// 全部（`draft` → `fixed` の順）。
    pub const ALL: [Self; 3] = [Self::Draft, Self::Review, Self::Fixed];

    /// フィールドの値・`--state` の引数から引く（前後の空白は無視）。表に無ければ `None`。
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "draft" => Some(Self::Draft),
            "review" => Some(Self::Review),
            "fixed" => Some(Self::Fixed),
            _ => None,
        }
    }

    /// ファイルに書く形。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Review => "review",
            Self::Fixed => "fixed",
        }
    }
}

/// エクスポートの設定（§4.5 `export` と、§17.2 のその回だけの指定）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Options {
    /// 節区切りの変換（§17.1-10）。
    pub paragraph_break: Break,
    /// 行頭字下げ（§17.1-12）。
    pub indent_first_line: bool,
    /// ルビを残すか（§17.1-9）。
    pub keep_ruby: bool,
    /// 章・話のタイトルを見出しとして入れるか（§17.1-2）。
    pub insert_headings: bool,
    /// 改行コード（§17.1-13）。
    pub newline: Newline,
    /// 出力の形式（§17.2）。
    pub format: Format,
    /// 状態の絞り込み（§17.2 `--state`）。`Some(s)` なら実効の状態が `s` 以上の節だけ（B-122）。
    pub state: Option<State>,
}

impl Options {
    /// `project.json` の `export` から作る（`paragraph_break` が表に無い値なら既定の `blank-line`）。
    /// 形式は `.txt`、状態の絞り込みは無し。
    pub fn from_settings(export: &Export) -> Self {
        Self {
            paragraph_break: Break::parse(&export.paragraph_break).unwrap_or_default(),
            indent_first_line: export.indent_first_line,
            keep_ruby: export.keep_ruby,
            insert_headings: export.insert_headings,
            newline: export.newline,
            format: Format::Text,
            state: None,
        }
    }
}

// ============================================================ 範囲（§17.2）

/// 範囲の指定を読む（§17.2 の「章 / 話 / 場面 / 節」「複数範囲」）。
///
/// §5.1 の範囲式（`@01 .. @03, @05`）に加えて、`@` を省いた書き方（`01-04`、`01 .. 03, 05`）も
/// 受け付ける（シェルによっては `@` が特別な意味を持つため。B-123）。左端 > 右端の項は誤り。
pub fn parse_range(text: &str) -> Result<RangeExpr, RangeError> {
    let parsed = match RangeExpr::parse(text) {
        Err(RangeError::MissingSigil) => RangeExpr::parse(&with_sigils(text)),
        other => other,
    }?;
    parsed.check()?;
    Ok(parsed)
}

/// `@` の無い ID の前に `@` を足す（`01 .. 03, 05` → `@01 .. @03, @05`）。
fn with_sigils(text: &str) -> String {
    let sigil = |part: &str| {
        let part = part.trim();
        if part.is_empty() || part.starts_with('@') {
            part.to_owned()
        } else {
            format!("@{part}")
        }
    };
    text.split(',')
        .map(|term| match term.split_once("..") {
            Some((head, tail)) => format!("{} .. {}", sigil(head), sigil(tail)),
            None => sigil(term),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// 既定のファイル名での範囲の書き方（§17.2 `<範囲>`）。全体は [`WHOLE`]、項は `01-04`・
/// `01~03`（区間）・`~03` / `03~`（開放端）で、複数の項は `_` でつなぐ。
pub fn range_label(range: Option<&RangeExpr>, digits: Digits) -> String {
    let Some(range) = range else {
        return WHOLE.to_owned();
    };
    let term = |term: &RangeTerm| match (term.start, term.end) {
        (Some(start), Some(end)) if start == end => start.display(digits),
        (start, end) => format!(
            "{}{LABEL_RANGE}{}",
            start.map(|id| id.display(digits)).unwrap_or_default(),
            end.map(|id| id.display(digits)).unwrap_or_default()
        ),
    };
    range
        .terms
        .iter()
        .map(term)
        .collect::<Vec<_>>()
        .join(&LABEL_UNION.to_string())
}

/// 既定の書き出し先のファイル名（§17.2 `<title>-<範囲>-<YYYYMMDD-HHMM>.txt`）。
///
/// `stamp` は `YYYYMMDD-HHMM`（呼び出し側が時計から作る。core は時刻を読まない）。題名の中の
/// ファイル名に使えない文字（`/ \ : * ? " < > |` と制御文字）は `_` にし、ドットで始まる題名は
/// 先頭に `_` を足す。題名が空なら `project`。
pub fn file_name(title: &str, range: &str, stamp: &str, format: Format) -> String {
    let mut name: String = title
        .trim()
        .chars()
        .map(|ch| {
            if matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_control()
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    if name.is_empty() {
        name.push_str("project");
    }
    if name.starts_with('.') {
        name.insert(0, '_');
    }
    format!("{name}-{range}-{stamp}.{}", format.extension())
}

// ============================================================ 対象節（§17.1-1）

/// 出す節（§17.1-1）。ファイル名の順 → ファイルの中の順（`sections::all` と同じ並び）。
///
/// `range` が `None` なら全体。`state` が `Some(s)` なら、実効の状態（[`effective_state`]）が
/// `s` 以上の節だけ。
pub fn select(
    project: &Project,
    range: Option<&RangeExpr>,
    state: Option<State>,
) -> Vec<SectionRef> {
    let mut states = States::new(project);
    sections::all(project)
        .into_iter()
        .filter(|section| range.is_none_or(|range| range.contains(section.id)))
        .filter(|section| state.is_none_or(|least| states.of(section.id) >= least))
        .collect()
}

/// 節の実効の状態（`PLANS.md` §9.2 B-122）。
///
/// 近いものから順に見て、最初に書かれていたもの: 話プロット（B 形式）の**節**の `state` →
/// 同じ場面の場面レベルの `state` → 章プロット（`plots/AA.txt`）の**話**のブロック → 全体プロット
/// （`plots/all.txt`）の**章**のブロック。どこにも無ければ `draft`。値が `draft` / `review` /
/// `fixed` のどれでもなければ書かれていないものとして扱う。
pub fn effective_state(project: &Project, id: SceneId) -> State {
    States::new(project).of(id)
}

/// [`effective_state`] の表引き（プロットは話・章ごとに 1 回だけ読む）。
struct States<'p> {
    project: &'p Project,
    /// 全体プロットの章の状態（初めて要るときに読む）。
    chapters: Option<BTreeMap<u16, State>>,
    /// 章プロットの話の状態（章ごと）。
    episodes: BTreeMap<u16, BTreeMap<u16, State>>,
    /// 話プロットの節・場面の状態（話ごと）。
    plots: BTreeMap<(u16, u16), PlotStates>,
}

/// 1 つの話プロットに書かれた状態。
#[derive(Debug, Default)]
struct PlotStates {
    /// (場面, 節) → 節レベルの `state`。
    sections: BTreeMap<(u16, u16), State>,
    /// 場面 → 場面レベルの `state`。
    scenes: BTreeMap<u16, State>,
}

impl<'p> States<'p> {
    fn new(project: &'p Project) -> Self {
        Self {
            project,
            chapters: None,
            episodes: BTreeMap::new(),
            plots: BTreeMap::new(),
        }
    }

    fn of(&mut self, id: SceneId) -> State {
        let project = self.project;
        let plot = self
            .plots
            .entry((id.chapter, id.episode))
            .or_insert_with(|| plot_states(project, id.chapter, id.episode));
        if let Some(found) = plot.sections.get(&(id.scene, id.section)) {
            return *found;
        }
        if let Some(found) = plot.scenes.get(&id.scene) {
            return *found;
        }
        let episodes = self.episodes.entry(id.chapter).or_insert_with(|| {
            match project.chapter_plot(id.chapter) {
                Some(outline) => outline_states(&outline, |block| {
                    (block.chapter == id.chapter).then_some(block.episode)
                }),
                None => BTreeMap::new(),
            }
        });
        if let Some(found) = episodes.get(&id.episode) {
            return *found;
        }
        let chapters = self
            .chapters
            .get_or_insert_with(|| match project.all_plot() {
                Some(outline) => outline_states(&outline, |block| Some(block.chapter)),
                None => BTreeMap::new(),
            });
        chapters.get(&id.chapter).copied().unwrap_or(State::Draft)
    }
}

/// フィールドの値を状態として読む（行末コメントは除く）。
fn field_state(value: &str) -> Option<State> {
    State::parse(lexer::strip_line_comment(value))
}

/// A 形式のブロックの `state`（`key` がブロックの ID から表の鍵を作る。`None` のブロックは飛ばす）。
fn outline_states(
    outline: &plot_outline::Outline<'_>,
    key: impl Fn(SceneId) -> Option<u16>,
) -> BTreeMap<u16, State> {
    let mut out = BTreeMap::new();
    for (index, block) in outline.blocks.iter().enumerate() {
        let Some(id) = block.id else {
            continue;
        };
        let Some(key) = key(id) else {
            continue;
        };
        let found = outline
            .fields_of(index, FieldKey::State)
            .first()
            .and_then(|field| field_state(field.value));
        if let Some(state) = found {
            out.entry(key).or_insert(state);
        }
    }
    out
}

/// 話プロット（B 形式）の節・場面の `state`。
fn plot_states(project: &Project, chapter: u16, episode: u16) -> PlotStates {
    let mut out = PlotStates::default();
    let Some(plot) = project.episode_plot(chapter, episode) else {
        return out;
    };
    for (index, scene) in plot.scenes.iter().enumerate() {
        let Some(id) = scene.id else {
            continue;
        };
        if id.chapter != chapter || id.episode != episode {
            continue;
        }
        let first = |fields: Vec<&plot_episode::Field<'_>>| {
            fields.first().and_then(|field| field_state(field.value))
        };
        if let Some(state) = first(plot.scene_fields(index, FieldKey::State)) {
            out.scenes.entry(id.scene).or_insert(state);
        }
        for (at, section) in scene.sections.iter().enumerate() {
            if let Some(state) = first(plot.section_fields(index, at, FieldKey::State)) {
                out.sections
                    .entry((id.scene, section.number))
                    .or_insert(state);
            }
        }
    }
    out
}

// ============================================================ 見出し（§17.1-2）

/// 章のタイトル（全体プロットの `@AA タイトル`）。無ければ `None`。
fn chapter_title(project: &Project, chapter: u16) -> Option<String> {
    let outline = project.all_plot()?;
    title_of(&outline, |id| id.chapter == chapter)
}

/// 話のタイトル（章プロットの `@AA-BB タイトル`）。無ければ `None`。
fn episode_title(project: &Project, chapter: u16, episode: u16) -> Option<String> {
    let outline = project.chapter_plot(chapter)?;
    title_of(&outline, |id| {
        id.chapter == chapter && id.episode == episode
    })
}

fn title_of(
    outline: &plot_outline::Outline<'_>,
    wanted: impl Fn(SceneId) -> bool,
) -> Option<String> {
    outline.blocks.iter().find_map(|block| {
        let id = block.id?;
        if !wanted(id) {
            return None;
        }
        let title = outline.header(block.header)?.title.trim();
        (!title.is_empty()).then(|| title.to_owned())
    })
}

// ============================================================ 本体

/// 出力の 1 行（§17.1 の 11〜14 を最後にまとめて行うための中間形）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Out {
    /// 本文の行（字下げの対象）。
    Text(String),
    /// 空行。
    Blank,
    /// 挿入した見出し（1 = 章、2 = 話）。
    Heading(u8, String),
    /// [`SEPARATOR`] の行。
    Separator,
}

/// エクスポートする（§17.1）。`range` が `None` なら全体。
///
/// 出すものが無ければ空文字列。それ以外は `options.newline` の改行で、末尾は改行 1 個。
pub fn run(project: &Project, range: Option<&RangeExpr>, options: &Options) -> String {
    render(project, &select(project, range, options.state), options)
}

/// [`select`] で選んだ節を書き出す（§17.1 の 2〜14。節の数を先に知りたい呼び出し側のため）。
/// `options.state` はここでは見ない（選ぶのは [`select`]）。
pub fn render(project: &Project, chosen: &[SectionRef], options: &Options) -> String {
    let dictionary = project.dictionary();
    let mut out: Vec<Out> = Vec::new();
    let mut previous: Option<SceneId> = None;
    // 同じファイルの節が続くあいだは、解析し直さない
    let mut parsed: Option<(String, Vec<Line<'_>>)> = None;
    for section in chosen {
        boundary(project, previous, section.id, options, &mut out);
        previous = Some(section.id);
        if parsed
            .as_ref()
            .is_none_or(|(path, _)| *path != section.path)
        {
            let lines = project.document(&section.path).map(|document| {
                contents::parse_with(&document.text, &project.contents_options()).lines
            });
            parsed = Some((section.path.clone(), lines.unwrap_or_default()));
        }
        let Some((_, lines)) = &parsed else {
            continue;
        };
        for line in lines.get(section.span.clone()).unwrap_or_default() {
            if let Some(found) = convert_line(line, dictionary, options.keep_ruby) {
                out.push(found);
            }
        }
    }
    finish(out, options)
}

/// 節と節のあいだ（§17.1-2 の見出しと §17.1-10 の区切り）。
///
/// 見出しを入れたところには区切りを入れない（見出しの前後の空行が区切りになる）。場面・話・章の
/// 境目も、`;` と同じく 1 回の区切りにする（B-124）。
fn boundary(
    project: &Project,
    previous: Option<SceneId>,
    id: SceneId,
    options: &Options,
    out: &mut Vec<Out>,
) {
    let new_chapter = previous.is_none_or(|last| last.chapter != id.chapter);
    let new_episode = new_chapter || previous.is_some_and(|last| last.episode != id.episode);
    let mut headed = false;
    if options.insert_headings {
        let mut heading = |level: u8, title: Option<String>| {
            if let Some(title) = title {
                out.push(Out::Blank);
                out.push(Out::Heading(level, title));
                out.push(Out::Blank);
                headed = true;
            }
        };
        if new_chapter {
            heading(1, chapter_title(project, id.chapter));
        }
        if new_episode {
            heading(2, episode_title(project, id.chapter, id.episode));
        }
    }
    if previous.is_some() && !headed {
        match options.paragraph_break {
            Break::BlankLine => out.push(Out::Blank),
            Break::None => {}
            Break::Separator => out.push(Out::Separator),
        }
    }
}

/// 節の中の 1 行（§17.1 の 3〜9）。消える行は `None`。
fn convert_line(line: &Line<'_>, dictionary: &Dictionary, keep_ruby: bool) -> Option<Out> {
    match line {
        Line::Blank(_) => Some(Out::Blank),
        Line::Text(text) => convert_text(text, dictionary, keep_ruby).map(Out::Text),
        // 見出し行と `;` は節の行に入らない（`contents::Section::span`）。行コメントは行ごと消す
        Line::Scene(_) | Line::Break(_) | Line::Comment(_) => None,
    }
}

/// 本文の 1 行の書き換え（§17.1 の 3・5〜9）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Edit<'a> {
    /// 消す（行内コメント・資料参照）。
    Drop,
    /// この文字列に置き換える（参照の名前・ルビの親文字）。
    Text(&'a str),
    /// この 1 文字に置き換える（エスケープの解除）。
    Char(char),
}

/// 本文の 1 行を書き換える。残りが空（コメントや資料参照だけの行）なら `None`（行ごと消す）。
fn convert_text(text: &TextLine<'_>, dictionary: &Dictionary, keep_ruby: bool) -> Option<String> {
    // 行末コメントと話者注釈（直前の空白ごと）は `body` に入っていない（§17.1 の 3・5）
    let body = text.body;
    let clamp = |range: &Range<usize>| range.start.min(body.len())..range.end.min(body.len());
    let mut edits: Vec<(Range<usize>, Edit<'_>)> = Vec::new();
    // 3: 行内コメント
    let inline = text
        .inline_comment
        .as_ref()
        .map(clamp)
        .filter(|range| !range.is_empty());
    if let Some(range) = &inline {
        edits.push((range.clone(), Edit::Drop));
    }
    // 8: エスケープ（§5.0.3 の 7 種だけ）
    let escapes: Vec<lexer::Escape> = lexer::find_escapes(body)
        .into_iter()
        .filter(|escape| escape.valid)
        .collect();
    for escape in &escapes {
        if let Some(target) = escape.target {
            edits.push((escape.start..escape.start + escape.len, Edit::Char(target)));
        }
    }
    // 6・7: 参照。行内コメントとエスケープした文字は空白に伏せてから探す（`\'` は参照にしない）
    let mut hidden: Vec<Range<usize>> = escapes
        .iter()
        .map(|escape| escape.start..escape.start + escape.len)
        .collect();
    hidden.extend(inline.clone());
    let masked = mask(body, &hidden);
    let tokens = dictionary::scan_line(&masked, 0, dictionary).tokens;
    for token in &tokens {
        let edit = match plain_key(&body[token.key_range.clone()]) {
            Some(name) => Edit::Text(name),
            None => Edit::Drop,
        };
        edits.push((token.range.clone(), edit));
    }
    // 名前の無い `'`（直後が区切り文字か行末。`'『星屑の時計』` など）もシジルなので消す（B-124）
    for (at, _) in masked.match_indices(REF_SIGIL) {
        let covered = tokens.iter().any(|token| token.range.contains(&at));
        if !covered && is_bare_sigil(&masked, at) {
            edits.push((at..at + REF_SIGIL.len_utf8(), Edit::Drop));
        }
    }
    // 9: ルビ
    if !keep_ruby {
        for ruby in &text.rubies {
            if ruby.range.end <= body.len() {
                edits.push((ruby.range.clone(), Edit::Text(ruby.base)));
            }
        }
    }
    edits.sort_by_key(|(range, _)| (range.start, std::cmp::Reverse(range.end)));
    let mut out = String::with_capacity(body.len());
    let mut at = 0;
    for (range, edit) in edits {
        // 重なったら先に始まるものを採る（参照の名前の中のルビなど。B-124）
        if range.start < at {
            continue;
        }
        out.push_str(&body[at..range.start]);
        match edit {
            Edit::Drop => {}
            Edit::Text(text) => out.push_str(text),
            Edit::Char(target) => out.push(target),
        }
        at = range.end;
    }
    out.push_str(&body[at..]);
    let trimmed = out.trim_end_matches(lexer::is_space);
    if trimmed.trim_start_matches(lexer::is_space).is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}

/// `at` の `'` が、名前を持たないシジルか（§6.2.1 の開始条件 — 直前が ASCII の英数字でない — を
/// 満たし、直後が区切り文字か行末。`dictionary::scan_line` が「ただの引用符」として飛ばすもの）。
///
/// 英語の短縮形（`don't`）は直前が英数字なので当たらず、`リリア's` は直後が区切り文字でないので
/// 当たらない（どちらもアポストロフィとして残す。B-63）。
fn is_bare_sigil(text: &str, at: usize) -> bool {
    let after_word = text[..at]
        .chars()
        .next_back()
        .is_some_and(|prev| prev.is_ascii_alphanumeric());
    let next = text[at + REF_SIGIL.len_utf8()..].chars().next();
    !after_word && next.is_none_or(dictionary::is_delimiter)
}

/// 参照の名前部を平文にする（§17.1 の 6・7）。消すなら `None`。
///
/// 解決できた参照の名前部に修飾子は入っていない（`RefToken::key_range`）。未解決の参照は区切り文字
/// までが名前部なので `:` を含みうる: `:` の前が資料 ID の形なら資料参照として消し（`'S0042:…`）、
/// `:` の後ろがモード式（§5.1 `mode_expr`）なら修飾子として落とす（`'未登録:B` → `未登録`）。
fn plain_key(key: &str) -> Option<&str> {
    let (head, tail) = match key.split_once(':') {
        Some((head, tail)) => (head, Some(tail)),
        None => (key, None),
    };
    if is_material_id(head) {
        return None;
    }
    match tail {
        Some(modes) if contents::split_modes(modes).is_some() => Some(head),
        _ => Some(key),
    }
}

/// 名前部が資料 ID の形（`M0137` / `S0042`、§17.1-7）か。
fn is_material_id(key: &str) -> bool {
    MaterialId::parse(key)
        .is_some_and(|id| id.prefix == PREFIX_MATERIAL || id.prefix == PREFIX_SEARCH)
}

/// `hide` の範囲を ASCII の空白に置き換えた文字列（byte 長は同じ。範囲は文字境界にあること）。
fn mask(text: &str, hide: &[Range<usize>]) -> String {
    if hide.is_empty() {
        return text.to_owned();
    }
    let mut bytes = text.as_bytes().to_vec();
    for range in hide {
        for byte in bytes.get_mut(range.clone()).unwrap_or_default() {
            *byte = b' ';
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| text.to_owned())
}

/// §17.1 の 11〜14。
fn finish(lines: Vec<Out>, options: &Options) -> String {
    // 11: 連続空行の圧縮（先頭の空行も落とす）
    let mut compact: Vec<Out> = Vec::with_capacity(lines.len());
    for line in lines {
        let blank = line == Out::Blank;
        if blank && compact.last().is_none_or(|last| *last == Out::Blank) {
            continue;
        }
        compact.push(line);
    }
    // 14: 末尾の空行
    while compact.last() == Some(&Out::Blank) {
        compact.pop();
    }
    if compact.is_empty() {
        return String::new();
    }
    let newline = options.newline.text();
    let mut out = String::new();
    for line in compact {
        match line {
            Out::Text(text) => {
                // 12: 行頭字下げ
                let indent = options.indent_first_line
                    && !text.starts_with(OPEN_QUOTE)
                    && !text.starts_with(lexer::is_space);
                if indent {
                    out.push(INDENT);
                }
                out.push_str(&text);
            }
            Out::Blank => {}
            Out::Heading(level, title) => {
                if options.format == Format::Markdown {
                    out.push_str(&"#".repeat(usize::from(level)));
                    out.push(' ');
                }
                out.push_str(&title);
            }
            Out::Separator => out.push_str(SEPARATOR),
        }
        // 13: 改行コード
        out.push_str(newline);
    }
    out
}
