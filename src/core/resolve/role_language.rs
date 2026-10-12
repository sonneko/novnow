//! 役割語パターンのマッチャ（§6.5 / 付録 B、指示 6）。
//!
//! `「` `」` に囲んで書かれたパターンは**正規表現そのものではなく**、5 種のアトムだけを持つ
//! 小言語（§6.5.1）。ここがそれを正規表現に変換し、**コンパイル済み**で持つ。
//!
//! # 5 アトム（§6.5.1 / 付録 B.1）
//!
//! | 記法 | 意味 | 変換後 | 下線 |
//! |---|---|---|---|
//! | `*` / `〜` | 0 文字以上の任意文字列（改行を含まない） | `[^\n]*?`（**非貪欲**） | **引かない** |
//! | `…` / `...` / `‥` | 間（1 個以上の省略記号・句点の連続） | `[…‥.。]{1,6}` | 引く |
//! | `(体言)` 等 | 語クラス（`project.json.lexicon.word_classes`） | 定義された正規表現 | 引く |
//! | `?` | 任意の 1 文字 | `[^\n]` | 引く |
//! | それ以外 | リテラル（メタ文字はエスケープ） | `regex::escape` 相当 | 引く |
//!
//! # アンカー（§6.5.2、固定）
//!
//! パターンの先頭・末尾がワイルドカード（`*` / `〜`）かどうかだけで決まる
//! （[`RolePattern::anchor`] が M2 で持つ）。`…` や `?` や語クラスはワイルドカードではない。
//!
//! | 形 | アンカー |
//! |---|---|
//! | 先頭ワイルドカード・末尾リテラル（`「*だよ！」`） | 末尾固定（`$`） |
//! | 先頭リテラル・末尾ワイルドカード（`「ちょっ…*」`） | 先頭固定（`^`） |
//! | 両端ワイルドカード（`「*ヨシ*」`） | 位置自由 |
//! | ワイルドカード無し（`「です。」`） | 末尾固定（既定） |
//! | `=` 前置（`「=ほう…」`） | 完全一致（`^…$`） |
//!
//! # `^` と `$` が指す範囲（**A-23**）
//!
//! **発話全体**（`「」` の内側の文字列）の先頭・末尾。§6.5.2 は「発話テキストを `。！？!?…` の
//! 直後で分割した単位（＝文）」を照合すると書くが、それだと付録 **B.3 の 1 行目の不一致例**
//! （`「だよ！って何」`）が一致してしまう（先頭の `だよ！` が 1 つの「文」になるため）。
//! B.3 は**テストの期待値**と明記された表で、`PLANS.md` §5.4 の M3 完了条件 1 なので
//! **B.3 を正**とした（A-21 と同じ裁定の流儀）。文単位にすると B.3 の 2 行目
//! （`「…あの…その…」` の `…あの…`）も一致しなくなる。
//! **仕様書は第 2.7 版でこの読み方に直した**（経緯は `PLANS.md` §9.1 **A-23**＝解消済み）。
//! 台詞の途中の文末にも当てたいときは `*よ。*` のように両端をワイルドカードにする（位置自由）。
//!
//! # 下線（§6.5.4 / 付録 B.4、**A-21**）
//!
//! マッチ全体のうち **`*` / `〜`（`[^\n]*?`）が消費した範囲だけを除いた**部分に緑の下線を引く。
//! 語クラス `(体言)` と省略記号 `…` と `?` は**下線に含める**（B.3 の 4 行目が
//! `(体言)！` → `静粛に！` の全体を下線としているため。B.4 の旧字句「語クラスを除く」は
//! B.3 と矛盾するので B.3 を正とした＝ A-21。**第 2.7 版で §6.5.4 と B.4 を修正済み**）。
//! 実装は**下線に含めるアトムの連続をキャプチャグループで囲んだ**
//! 正規表現を別に生成し、group 1 以降の範囲を並べる（B.4「キャプチャグループを自動挿入」）。
//!
//! 複数パターンが一致したら、一致範囲の**和集合に 1 本**の下線を引く（二重線にしない）。
//! **一致しない場合は何も返さない**（診断も色も出さない、§6.5.4 の MUST）。
//!
//! # 照合対象（§6.5.3）
//!
//! C 形式の `dialogue.text`（`「」` / `『』` の**内側**）だけ。地の文は照合しない（MUST）。
//! 内側の範囲を取り出すのは [`dialogue_spans`]。話者が不明な台詞を照合しないのは
//! **呼び出し側**の仕事（話者は行末の話者注釈 → 直前の注釈の継続 → プロットの `pov` の順に
//! 決まる。適用パターン集合は [`crate::core::resolve::modes::applicable_patterns`]）。
//!
//! # 計算量（§6.5.5）
//!
//! 1 台詞あたり（適用パターン数 × 一致候補数）回の正規表現検索。パターンは
//! [`MatcherSet`] に**事前コンパイル**して [`Cache`] でモードごとに持つ（`RegexSet` で
//! 「そもそも一致しないパターン」を先に落とすので、10 パターンでも全文を検索しない）。
//! 目標は 1 行 10 パターンで **20µs 以内** → **release 実測 0.68µs**（同じ台詞に
//! 100 パターンを当てても 9.8µs）。計測は `tests/role_language.rs` の
//! `ten_patterns_on_one_line_stay_fast`（debug では緩い上限にしてある）。

use std::collections::BTreeMap;
use std::ops::Range;

use regex::{Regex, RegexSet};

use crate::core::model::project::Lexicon;
use crate::core::model::role_pattern::{Anchor, RolePattern, WILDCARDS};

/// §6.5.1 のワイルドカード（`*` / `〜`）の変換結果。**非貪欲**（B.3 の 1 行目
/// `[^\n]*?だよ！$` が「もう行く」を食い過ぎないのはこのため）。
pub const WILDCARD_REGEX: &str = "[^\\n]*?";

/// §6.5.1 の省略記号（`…` / `...` / `‥`）の変換結果。`。`（句点）も含む 1〜6 連続。
pub const ELLIPSIS_REGEX: &str = "[…‥.。]{1,6}";

/// §6.5.1 の `?`（任意の 1 文字）の変換結果。改行は含まない。
pub const ANY_CHAR_REGEX: &str = "[^\\n]";

/// 半角 3 点 `...` も省略記号（§6.5.1）。2 点以下はリテラル。
const DOTS: &str = "..";

/// 語クラスを囲む括弧（§6.5.1 `(体言)`）。**半角**。
pub const CLASS_OPEN: char = '(';

/// 語クラスの閉じ括弧。
pub const CLASS_CLOSE: char = ')';

/// 台詞の括弧（§6.5.3「`「」` / `『』` の内側のみ」）。
pub const QUOTES: &[(char, char)] = &[('「', '」'), ('『', '』')];

/// §6.5.1 の**既定の語クラス**（`project.json.lexicon.word_classes` 未指定時）。
///
/// i18n: format-vocabulary
///
/// キーは §4.5 の例（`"lexicon": { "word_classes": { "体言": "…" } }`）どおり**括弧を除いた**形。
/// 中身は日本語の文字種を含むが、これはユーザー向け文言ではなく**ファイル形式の語彙**なので
/// §12.2 の免除（`i18n: format-vocabulary` の doc 行、`PLANS.md` §9.2 B-19）で core が持つ。
/// `project.json` に定義があれば**そちらが優先する**（§6.5.1「`project.json` で定義」）。
pub const DEFAULT_WORD_CLASSES: &[(&str, &str)] = &[
    (
        "体言",
        "[\\p{Han}\\p{Katakana}\\p{Hiragana}A-Za-z0-9ー]{1,24}",
    ),
    ("用言", "[\\p{Han}\\p{Hiragana}]{1,16}"),
    ("数", "[0-9０-９一二三四五六七八九十百千万]{1,8}"),
];

/// §6.5.1 の 5 アトム。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    /// `*` / `〜` → `[^\n]*?`。**下線に含めない**（A-21）。
    Wildcard,
    /// `…` / `...` / `‥` → `[…‥.。]{1,6}`。
    Ellipsis,
    /// `(体言)` 等 → 語クラスの正規表現。中身は**括弧を除いたクラス名**。
    WordClass(String),
    /// `?` → `[^\n]`。
    AnyChar,
    /// それ以外の 1 文字（正規表現のメタ文字はエスケープする）。
    Literal(char),
}

impl Atom {
    /// このアトムが食った範囲に**下線を引くか**（A-21: 除くのは `*` / `〜` だけ）。
    pub fn is_underlined(&self) -> bool {
        !matches!(self, Self::Wildcard)
    }
}

/// パターン本体（`=` 前置を除いた部分）をアトムに割る（§6.5.1）。
///
/// 未知の語クラス（`(ほげ)` のように `project.json` にも既定にも無いもの）は
/// **括弧ごとリテラル**にする（§7 に対応する診断コードが無いので、勝手に無視もしない）。
pub fn atoms(body: &str, lexicon: &Lexicon) -> Vec<Atom> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some((first, tail)) = next_char(rest) {
        rest = tail;
        if WILDCARDS.contains(&first) {
            out.push(Atom::Wildcard);
        } else if first == '…' || first == '‥' {
            out.push(Atom::Ellipsis);
        } else if first == '.' && rest.starts_with(DOTS) {
            out.push(Atom::Ellipsis);
            rest = &rest[DOTS.len()..];
        } else if first == '?' {
            out.push(Atom::AnyChar);
        } else if first == CLASS_OPEN {
            match class_name(rest).filter(|(name, _)| class_regex(name, lexicon).is_some()) {
                Some((name, after)) => {
                    out.push(Atom::WordClass(name.to_string()));
                    rest = after;
                }
                // 未知のクラス名は `(` をリテラルにして、残りを普通に読み直す
                None => out.push(Atom::Literal(first)),
            }
        } else {
            out.push(Atom::Literal(first));
        }
    }
    out
}

/// `(` の直後から `)` までの語クラス名。空・括弧なしは `None`。
fn class_name(rest: &str) -> Option<(&str, &str)> {
    let close = rest.find(CLASS_CLOSE)?;
    let name = &rest[..close];
    if name.is_empty() || name.contains(CLASS_OPEN) {
        return None;
    }
    Some((name, &rest[close + CLASS_CLOSE.len_utf8()..]))
}

/// 語クラスの正規表現（`project.json.lexicon.word_classes` が優先、無ければ §6.5.1 の既定）。
pub fn class_regex<'a>(name: &str, lexicon: &'a Lexicon) -> Option<&'a str> {
    if let Some(found) = lexicon.word_classes.get(name) {
        return Some(found.as_str());
    }
    DEFAULT_WORD_CLASSES
        .iter()
        .find(|&&(key, _)| key == name)
        .map(|&(_, value)| value)
}

/// 1 アトムの正規表現。
fn atom_regex(atom: &Atom, lexicon: &Lexicon) -> String {
    match atom {
        Atom::Wildcard => WILDCARD_REGEX.to_string(),
        Atom::Ellipsis => ELLIPSIS_REGEX.to_string(),
        Atom::AnyChar => ANY_CHAR_REGEX.to_string(),
        // `atoms` が作るときに存在を確認しているので、ここに来るのは呼び出し側が
        // `Atom` を自分で作った場合だけ（保険としてエスケープしたクラス名に倒す）
        Atom::WordClass(name) => class_regex(name, lexicon)
            .unwrap_or(name.as_str())
            .to_string(),
        Atom::Literal(character) => regex::escape(&character.to_string()),
    }
}

/// アトム列を正規表現にする。
///
/// `groups` が `true` なら**下線に含めるアトムの連続をキャプチャグループで囲む**
/// （付録 B.4「リテラル部分を `(...)` で囲んだ正規表現を生成する」）。`false` なら
/// 付録 B.3 の「正規表現」列そのものになる。
fn render(list: &[Atom], lexicon: &Lexicon, groups: bool) -> String {
    let mut out = String::new();
    let mut run = String::new();
    for atom in list {
        if atom.is_underlined() {
            run.push_str(&atom_regex(atom, lexicon));
            continue;
        }
        flush(&mut out, &mut run, groups);
        out.push_str(&atom_regex(atom, lexicon));
    }
    flush(&mut out, &mut run, groups);
    out
}

/// 溜まったリテラル部分を（必要なら括弧で囲んで）吐く。
fn flush(out: &mut String, run: &mut String, groups: bool) {
    if run.is_empty() {
        return;
    }
    if groups {
        out.push('(');
        out.push_str(run);
        out.push(')');
    } else {
        out.push_str(run);
    }
    run.clear();
}

/// §6.5.2 のアンカーを付ける。
fn anchored(body: &str, anchor: Anchor) -> String {
    match anchor {
        Anchor::Exact => format!("^{body}$"),
        Anchor::SentenceStart => format!("^{body}"),
        Anchor::SentenceEnd => format!("{body}$"),
        Anchor::Anywhere => body.to_string(),
    }
}

/// パターンを正規表現に変換する（**付録 B.3 の「正規表現」列そのもの**）。
///
/// キャプチャグループは付けない（下線用のグループ入りは [`Matcher::new`] が内部で作る）。
pub fn to_regex(pattern: &RolePattern, lexicon: &Lexicon) -> String {
    anchored(
        &render(&atoms(pattern.body(), lexicon), lexicon, false),
        pattern.anchor(),
    )
}

/// 1 回の一致（§6.5.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// マッチ全体（キャプチャグループ 0）。
    pub whole: Range<usize>,
    /// **下線を引く範囲**。ワイルドカードが食った部分は除く（A-21）。マージ済み・昇順。
    ///
    /// 空にならない（下線が 1 本も無い一致は [`Matcher::find`] が `None` を返す）。
    pub underlines: Vec<Range<usize>>,
}

/// 1 パターンのマッチャ（**コンパイル済み**、§6.5.5）。
#[derive(Debug, Clone)]
pub struct Matcher {
    /// 書かれたままのパターン（`=` 前置を含む。[`RolePattern::source`]）。
    source: String,
    /// §6.5.2 のアンカー。
    anchor: Anchor,
    /// 付録 B.3 の「正規表現」列（キャプチャグループなし）。
    plain: String,
    /// 下線用のキャプチャグループ入り正規表現。
    regex: Regex,
}

impl Matcher {
    /// パターンをコンパイルする。
    ///
    /// **`None` になるのは語クラスの正規表現が壊れているときだけ**
    /// （`project.json.lexicon.word_classes` に不正な値を書いた場合）。§7 に対応する診断コードが
    /// 無いので黙って捨てる（§9.1 **A-24**）。§6.5.4「一致しない場合は何もしない」と同じく、
    /// 役割語の色が消えるだけで執筆は止まらない（§4.3-4）。
    pub fn new(pattern: &RolePattern, lexicon: &Lexicon) -> Option<Self> {
        let list = atoms(pattern.body(), lexicon);
        let anchor = pattern.anchor();
        let plain = anchored(&render(&list, lexicon, false), anchor);
        let regex = Regex::new(&anchored(&render(&list, lexicon, true), anchor)).ok()?;
        Some(Self {
            source: pattern.source.clone(),
            anchor,
            plain,
            regex,
        })
    }

    /// 書かれたままのパターン（`「` `」` の内側。`=` 前置を含む）。
    pub fn source(&self) -> &str {
        &self.source
    }

    /// §6.5.2 のアンカー。
    pub fn anchor(&self) -> Anchor {
        self.anchor
    }

    /// 付録 B.3 の「正規表現」列（キャプチャグループなし）。
    pub fn regex_source(&self) -> &str {
        &self.plain
    }

    /// 発話 1 件を照合する（§6.5.4）。一致しなければ `None`（**何も返さない**のが MUST）。
    ///
    /// `utterance` は `「」` の**内側**（[`dialogue_spans`] で取り出す。§6.5.3 の MUST）。
    pub fn find(&self, utterance: &str) -> Option<Hit> {
        let captures = self.regex.captures(utterance)?;
        let whole = captures.get(0)?.range();
        let mut underlines = Vec::new();
        for group in captures.iter().skip(1).flatten() {
            if group.start() < group.end() {
                underlines.push(group.range());
            }
        }
        // 下線が 1 本も無い（`「*」` のようにワイルドカードだけのパターン）ときは
        // 「一致しなかった」扱いにする。何もないところに色は付けられない（§6.5.4）
        if underlines.is_empty() {
            return None;
        }
        Some(Hit {
            whole,
            underlines: union(underlines),
        })
    }
}

/// 1 キャラ（または 1 モード）の**適用パターン集合**（§6.5.3）。
///
/// 中身は [`crate::core::resolve::modes::applicable_patterns`] が作った
/// 「モード専用 ∪ キャラ共通 `## tone`」をそのまま渡す。
#[derive(Debug, Clone)]
pub struct MatcherSet {
    matchers: Vec<Matcher>,
    /// §6.5.5「パターンはモードごとに `RegexSet` へ事前コンパイルしてキャッシュする」。
    /// 作るのに失敗したら `None`（全パターンを個別に検索するだけ。結果は変わらない）
    set: Option<RegexSet>,
}

impl MatcherSet {
    /// パターン列から作る。**コンパイルできないパターンは捨てる**（[`Matcher::new`]）。
    pub fn new(patterns: &[RolePattern], lexicon: &Lexicon) -> Self {
        let matchers = patterns
            .iter()
            .filter_map(|pattern| Matcher::new(pattern, lexicon))
            .collect::<Vec<_>>();
        let set = RegexSet::new(matchers.iter().map(|found| found.plain.as_str())).ok();
        Self { matchers, set }
    }

    /// コンパイルできたパターンの数。
    pub fn len(&self) -> usize {
        self.matchers.len()
    }

    /// パターンが 1 つも無い（＝照合しない）。
    pub fn is_empty(&self) -> bool {
        self.matchers.is_empty()
    }

    /// コンパイル済みマッチャ（書いた順）。
    pub fn matchers(&self) -> &[Matcher] {
        &self.matchers
    }

    /// 一致したパターン全部（書いた順）。`RegexSet` で候補を絞ってから個別に検索する。
    pub fn hits(&self, utterance: &str) -> Vec<Hit> {
        let candidates: Vec<usize> = match &self.set {
            Some(set) => set.matches(utterance).iter().collect(),
            None => (0..self.matchers.len()).collect(),
        };
        candidates
            .into_iter()
            .filter_map(|index| self.matchers.get(index)?.find(utterance))
            .collect()
    }

    /// **下線の和集合**（§6.5.4「複数パターンが一致したら、一致範囲の和集合に 1 本の
    /// 下線を引く（二重線にしない）」）。重なった・隣り合った範囲は 1 本にまとめる。
    pub fn underlines(&self, utterance: &str) -> Vec<Range<usize>> {
        union(
            self.hits(utterance)
                .into_iter()
                .flat_map(|hit| hit.underlines)
                .collect(),
        )
    }
}

/// `MatcherSet` のキャッシュ（§6.5.5「モードごとに事前コンパイルしてキャッシュする」）。
///
/// 正規表現のコンパイルはパターンの本数に比例して効くので、**編集のたび**に作り直さない。
/// キーは呼び出し側が決める（例: `"恩田恵:A"`、キャラ共通は `"恩田恵:*"`）。
#[derive(Debug, Clone, Default)]
pub struct Cache {
    lexicon: Lexicon,
    sets: BTreeMap<String, MatcherSet>,
}

impl Cache {
    /// `project.json` の `lexicon`（§4.5）で空のキャッシュを作る。
    pub fn new(lexicon: Lexicon) -> Self {
        Self {
            lexicon,
            sets: BTreeMap::new(),
        }
    }

    /// 語クラスの定義（`project.json` の値と既定の両方を見るのは [`class_regex`]）。
    pub fn lexicon(&self) -> &Lexicon {
        &self.lexicon
    }

    /// `key` のパターン集合。**無ければ作って覚える**（2 回目以降はコンパイルしない）。
    ///
    /// 同じ `key` に別のパターンを渡しても**覚えているほうを返す**（モードの中身が変わったら
    /// [`Cache::forget`] で捨てる）。
    pub fn set_for(&mut self, key: &str, patterns: &[RolePattern]) -> &MatcherSet {
        if !self.sets.contains_key(key) {
            let built = MatcherSet::new(patterns, &self.lexicon);
            self.sets.insert(key.to_string(), built);
        }
        // 直前に入れたので必ずある
        self.sets.get(key).unwrap_or(&EMPTY_SET)
    }

    /// `key` のキャッシュを捨てる（`characters.md` を編集したあと）。
    pub fn forget(&mut self, key: &str) -> bool {
        self.sets.remove(key).is_some()
    }

    /// 全部捨てる（`project.json` の `lexicon` を書き換えたあと）。
    pub fn clear(&mut self) {
        self.sets.clear();
    }

    /// 覚えているパターン集合の数。
    pub fn len(&self) -> usize {
        self.sets.len()
    }

    /// 何も覚えていない。
    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }
}

/// [`Cache::set_for`] の保険（通常は unreachable）。
static EMPTY_SET: MatcherSet = MatcherSet {
    matchers: Vec::new(),
    set: None,
};

/// §6.5.3 の照合対象 = `「」` / `『』` の**内側**の範囲。地の文は返さない（MUST）。
///
/// 入れ子（`「『…』」`）は内側から返す。閉じ括弧が無い場合は**行末まで**を内側とする
/// （書きかけの台詞でも役割語が見えたほうが役に立つ。原文は書き換えないので round-trip は
/// 影響を受けない）。種類の違う閉じ括弧（`「…』`）は閉じとして扱わない。
pub fn dialogue_spans(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    // (期待する閉じ括弧, 内側の開始 byte)
    let mut stack: Vec<(char, usize)> = Vec::new();
    for (at, character) in text.char_indices() {
        if let Some(&(open, close)) = QUOTES.iter().find(|&&(open, _)| open == character) {
            stack.push((close, at + open.len_utf8()));
            continue;
        }
        if let Some(&(close, start)) = stack.last() {
            if close == character {
                stack.pop();
                out.push(start..at);
            }
        }
    }
    for (_, start) in stack {
        out.push(start..text.len());
    }
    // **和集合にはしない**: 入れ子（`「『…』」`）は内側と外側の両方が台詞なので、
    // まとめると内側の位置が失われる（下線の和集合は §6.5.4 なので [`union`] を使う）
    out.sort_unstable_by_key(|found| (found.start, found.end));
    out.dedup();
    out
}

/// 範囲を昇順に並べ、重なり・隣接を 1 本にまとめる（§6.5.4「二重線にしない」）。
pub fn union(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.retain(|found| found.start < found.end);
    ranges.sort_unstable_by_key(|found| (found.start, found.end));
    let mut out: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match out.last_mut() {
            // 隣接（`a..b` と `b..c`）も 1 本にする。下線は連続して見えるべきなので
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => out.push(range),
        }
    }
    out
}

/// 次の 1 文字と残り（`str` の先頭から）。
fn next_char(text: &str) -> Option<(char, &str)> {
    let mut chars = text.chars();
    let first = chars.next()?;
    Some((first, chars.as_str()))
}
