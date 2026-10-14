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
//! # `^` と `$` が指す範囲（**A-23**、第 2.8 版）
//!
//! **文**の先頭・末尾。文は発話（`「」` の内側の文字列）を文末記号 `。！？!?…` の**連続の直後**で
//! 区切った単位で、[`sentences`] が作る。頭にある文末記号の連続は次の文に含め
//! （`…あの…その…` → `…あの…` / `その…`。B.3 の 2 行目が成り立つのはこのため）、
//! 文の前後の空白は含めない（`えっ！　ちょっ…` の 2 文目は `ちょっ…`）。
//! **`=` 前置の完全一致だけは文に分けず発話全体**を見る（§6.5.2 の表「発話全体が一致」）。
//! 1 つの台詞の複数の文が一致したら、全部に下線を引く（[`Matcher::find_all`]）。
//! 経緯: 第 2.7 版は B.3 の 1 行目の不一致例「だよ！って何」に合わせて発話全体としていたが、
//! ユーザー裁定（2026-10-13）で文単位に戻し、不一致例を「本当だよ！？」に差し替えた
//! （`PLANS.md` §9.1 **A-23**、細部は §9.2 **B-74**）。
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
//! 1 台詞あたり（適用パターン数 × 文数）回の正規表現検索。パターンは
//! [`MatcherSet`] に**事前コンパイル**して [`Cache`] でモードごとに持つ（文ごとに `RegexSet` で
//! 「そもそも一致しないパターン」を先に落とすので、10 パターンでも全部を個別には検索しない）。
//! 目標は 1 行 10 パターンで **20µs 以内** → **release 実測 0.68µs**（同じ台詞に
//! 100 パターンを当てても 9.8µs）。計測は `tests/role_language.rs` の
//! `ten_patterns_on_one_line_stay_fast`（debug では緩い上限にしてある）。

use std::collections::BTreeMap;
use std::ops::Range;

use regex::{Regex, RegexSet};

use crate::core::model::project::Lexicon;
use crate::core::model::role_pattern::{Anchor, RolePattern, WILDCARDS};
use crate::core::text::lexer;

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

/// §6.5.2 の文末記号（第 2.8 版）。この記号の**連続の直後**で発話を文に区切る（[`sentences`]）。
///
/// `‥` と `.` は省略記号のアトム（[`ELLIPSIS_REGEX`]）には入るが、文末記号ではない
/// （§6.5.2 が挙げるのはこの 6 字だけ）。
pub const SENTENCE_ENDS: &[char] = &['。', '！', '？', '!', '?', '…'];

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
    /// マッチ全体（キャプチャグループ 0）。位置は**発話**の中の byte（文の中ではない）。
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
    /// （`project.json.lexicon.word_classes` に不正な値を書いた場合）。そのパターンだけを捨てる
    /// （§6.5.1 第 2.9 版。他のパターンは生かす）。報告は `project.json` 側の **E027**
    /// （`syntax::project_json`、`PLANS.md` §9.1 A-24）。§6.5.4「一致しない場合は何もしない」と
    /// 同じく、役割語の色が消えるだけで執筆は止まらない（§4.3-4）。
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

    /// 発話 1 件を照合し、**最初に一致した文**の結果を返す（§6.5.4）。一致しなければ `None`
    /// （**何も返さない**のが MUST）。全部の文の結果が要るなら [`Matcher::find_all`]。
    ///
    /// `utterance` は `「」` の**内側**（[`dialogue_spans`] で取り出す。§6.5.3 の MUST）。
    pub fn find(&self, utterance: &str) -> Option<Hit> {
        if self.anchor == Anchor::Exact {
            return self.find_in(utterance, 0..utterance.len());
        }
        sentences(utterance)
            .into_iter()
            .find_map(|span| self.find_in(utterance, span))
    }

    /// 発話 1 件を照合し、**一致した文ごと**の結果を文の順に返す（§6.5.2、第 2.8 版）。
    ///
    /// `^` / `$` は文の先頭・末尾を指すので、文ごとに正規表現を当てる。**`=` 前置の完全一致だけは
    /// 発話全体**に 1 回だけ当てる。位置はどちらも `utterance` の中の byte。
    pub fn find_all(&self, utterance: &str) -> Vec<Hit> {
        if self.anchor == Anchor::Exact {
            return self
                .find_in(utterance, 0..utterance.len())
                .into_iter()
                .collect();
        }
        sentences(utterance)
            .into_iter()
            .filter_map(|span| self.find_in(utterance, span))
            .collect()
    }

    /// `utterance[span]` だけを照合する（`^` / `$` は `span` の両端）。位置は `utterance` の byte。
    fn find_in(&self, utterance: &str, span: Range<usize>) -> Option<Hit> {
        let part = utterance.get(span.clone())?;
        let captures = self.regex.captures(part)?;
        let shift = |found: Range<usize>| found.start + span.start..found.end + span.start;
        let whole = shift(captures.get(0)?.range());
        let mut underlines = Vec::new();
        for group in captures.iter().skip(1).flatten() {
            if group.start() < group.end() {
                underlines.push(shift(group.range()));
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

/// 発話を §6.5.2 の**文**に区切る（第 2.8 版、`PLANS.md` §9.1 A-23 / §9.2 B-74）。
///
/// - 文末記号（[`SENTENCE_ENDS`]）の**連続の直後**で区切る（`！？` も `……` も 1 つの区切り）。
/// - 文の頭（発話の頭・区切りの直後）にある文末記号の連続は区切りにせず、**次の文に含める**
///   （`…あの…その…` → `…あの…` / `その…`）。文末記号だけの発話は 1 文になる。
/// - 前後の空白（[`lexer::is_space`]）は文に含めない。空白だけ・空の発話は文を返さない。
///
/// 返すのは `utterance` の中の byte 範囲（昇順・重なりなし）。
pub fn sentences(utterance: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    // 今の文の始まり（最初の空白でない文字）
    let mut start: Option<usize> = None;
    // 今の文の最後の空白でない文字の直後
    let mut end = 0;
    // 今の文に文末記号でない文字があるか（頭の文末記号の連続は区切りにしない）
    let mut content = false;
    // 文を閉じる文末記号の連続の中にいるか
    let mut closing = false;
    for (at, character) in utterance.char_indices() {
        let next = at + character.len_utf8();
        if SENTENCE_ENDS.contains(&character) {
            start.get_or_insert(at);
            closing |= content;
            end = next;
            continue;
        }
        if closing {
            if let Some(begin) = start.take() {
                out.push(begin..end);
            }
            content = false;
            closing = false;
        }
        if lexer::is_space(character) {
            continue;
        }
        start.get_or_insert(at);
        content = true;
        end = next;
    }
    if let Some(begin) = start {
        out.push(begin..end);
    }
    out
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

    /// 一致した結果全部（パターンの書いた順、同じパターンの中は文の順）。
    ///
    /// 文ごとに `RegexSet` で候補を絞ってから個別に検索する（§6.5.5）。`=` 前置の完全一致は
    /// 発話全体に 1 回だけ当てる（[`Matcher::find_all`] と同じ結果）。
    pub fn hits(&self, utterance: &str) -> Vec<Hit> {
        let spans = sentences(utterance);
        // 文ごとの候補（`RegexSet` が作れなかったときは全パターンが候補）
        let candidates: Vec<Option<regex::SetMatches>> = spans
            .iter()
            .map(|span| {
                let part = utterance.get(span.clone()).unwrap_or_default();
                self.set.as_ref().map(|set| set.matches(part))
            })
            .collect();
        let mut out = Vec::new();
        for (index, matcher) in self.matchers.iter().enumerate() {
            if matcher.anchor == Anchor::Exact {
                out.extend(matcher.find_in(utterance, 0..utterance.len()));
                continue;
            }
            for (span, found) in spans.iter().zip(&candidates) {
                let candidate = found.as_ref().is_none_or(|set| set.matched(index));
                if candidate {
                    out.extend(matcher.find_in(utterance, span.clone()));
                }
            }
        }
        out
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
