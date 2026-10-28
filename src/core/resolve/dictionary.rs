//! 辞書（§6.1）と `'` 参照の**最短一致**（§6.2、指示 4 の中核）。
//!
//! # 何をするモジュールか
//!
//! `characters.md` / `words.md` / `materials/index.json` の 3 ファイルから**1 本のトライ**を
//! 作り（§6.1.1「`'` の後ろで解決される辞書は 1 本のトライに統合する」）、テキスト中の
//! `'参照` を解決する。返すのは「どのエントリに当たったか」と「トークンの範囲」だけで、
//! 色（§11.2）もホバー（§9.6）もここは知らない。
//!
//! ```text
//! Characters ─┐
//! Words      ─┼→ Dictionary::from_sources() → Dictionary（トライ＋エントリ表）＋ E028 / W027
//! Index      ─┘                                    │
//!                                                  ↓
//!                  scan_line(line, line_start, &dict) → RefToken（解決済み / 未解決）＋診断
//! ```
//!
//! # 入力と位置（`docs/history.md` B-56）
//!
//! 入力は **`core::syntax` の木**（M2 の成果物）。`core::model` の型
//! （[`Character`](crate::core::model::character::Character) 等）は行の位置を持たないので、
//! それだけでは診断の波線を引く場所が決まらない。意味の塊が要る箇所では model の型を
//! その場で組んで再利用する（`resolve::role_language` が
//! [`RolePattern::anchor`](crate::core::model::role_pattern::RolePattern::anchor) を使う等）。
//!
//! 位置はすべて **byte offset**（§3.4）。[`Origin`] と [`scan_line`] の診断はファイル単位、
//! [`RefToken`] は**行内相対**（M2 の `Speaker` / `Pattern` と同じ流儀）。
//!
//! # 最短一致（§6.2.1 / §6.2.2）
//!
//! 「`@` 及び `'` の後の**最短で何らかの辞書に一致する文字列まで**が範囲」。辞書に `リリア` と
//! `リリアナ` の両方があっても **`リリア`（短い方）** が勝つ。これは指示どおりの決定で、
//! 作家を驚かせないための補助として **W029**（より長い項目にも一致しうる）を必ず付ける
//! （§6.2.2 MUST）。クイックフィックスは明示形 `'{リリアナ}` への書き換え（M4 が作る）。
//!
//! # 正規化（§6.1.3、MUST）
//!
//! 比較は **長音統一 → NFKC → 小文字化 → 空白除去** の順で行い、**元テキストは一切
//! 書き換えない**。仕様書の並び（NFKC が先）だと `－`（U+FF0D）は NFKC で ASCII の `-` に
//! なって長音 `ー` に統一されないので、長音統一を先に適用する（`docs/history.md` B-57）。
//! 空白を落とすので `'科学の甲子園 本戦の運営と実技課題` のように**空白を含む資料タイトル**
//! も 1 キーとして一致する（§6.6.1）。
//!
//! スキャンは正規化後のテキスト上で行い、byte 位置は [`OffsetMap`] で元に戻す。
//! 1 文字ずつ正規化するので**結合文字の並びは合成されない**（`ﾊ` + `゙` → `バ` にならない）。
//! 辞書キーに半角カナ＋濁点を書く事例は無いと判断した（B-57）。
//!
//! # 未知の参照の範囲（§6.2.6、S-09）
//!
//! 辞書に当たらない `'名前` は、以前は区切り文字（句読点・空白・括弧）までを未解決参照の
//! 範囲としていた。いまは**先頭 1 形態素（語幹）**までを範囲とし、E001 の赤波線も
//! 「用語として登録」が登録する名前も、その 1 形態素になる（[`first_morpheme_end`]）。
//! 判定は文字種の連なり（漢字 / カタカナ / ひらがな / 英数字）＋形態素解析
//! （[`tiniestsegmenter`]）＋助詞の切り出し（[`PARTICLES`]）で行う。
//!
//! トークン全体（[`RefToken::range`]）は狭めない。§17.1 のエクスポート（`'` と修飾子を
//! 消して名前部を残す）と §6.6.3 の資料 ID の形の判定が、元の範囲を前提にしているため。
//!
//! # エントリ種別
//!
//! - エントリ種別は **5 つ**（`Character` / `Address` / `Word` / `Material` / `MaterialTitle`）。
//!   [`EntryKind::Address`] は `address:`（§5.5.6）の呼称で、[`Dictionary::from_sources`] が
//!   `characters.md` から作る（`* =>` は相手が定まらないので辞書に入れない）。優先順位
//!   （`Character > Address > Word > Material > MaterialTitle`）は §6.1.1 の固定値で、
//!   **W084** / **W085** はこの種別を使って M4 の検査器が出す。
//! - `MaterialTitle` の修飾子は §6.1.1 の表で「不可」だが、§6.2.4 は `Word` / `Address` だけ
//!   を名指ししている。表の「不可」を優先して **E026** を出す（B-62）。

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::OnceLock;

use aho_corasick::AhoCorasick;
use unicode_normalization::UnicodeNormalization;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::syntax::characters_md::{Characters, Line as MdLine, SectionKey};
use crate::core::syntax::contents::{self, Speaker};
use crate::core::syntax::materials_index::Index;
use crate::core::syntax::words_md::Words;
use crate::core::text::charclass::{classify, CharClass};
use crate::core::text::lexer::{self, FieldKey};

/// 辞書キーの長さの上限（§6.2.1「`MAX_KEY_BYTES = 96`（固定）」）。
///
/// **正規化後の byte 数**で見る。これはトライの深さの上限なので、空白を落としたあとの
/// 長さで測らないと「書かれた形は 96 byte を超えるが実体は短いタイトル」を落としてしまう
/// （`docs/history.md` B-57）。超えたキーは登録時に **E028** で拒否する。
pub const MAX_KEY_BYTES: usize = 96;

/// 参照の記号（§5.1 `ref_char = "'"`）。
pub const REF_SIGIL: char = '\'';

/// 明示形 `'{ ... }` の開き括弧（§6.2.3）。
pub const EXPLICIT_OPEN: char = '{';

/// 明示形 `'{ ... }` の閉じ括弧（§6.2.3）。
pub const EXPLICIT_CLOSE: char = '}';

/// 修飾子の区切り（§6.2.4 `modifier = ":" , …`）。**半角だけ**。
///
/// 全角 `：` は修飾子の区切りにしない。本文は日本語なので `'リリア：待って` の `：` は
/// 句読点であってモード式ではない（`：` は §6.2.1 の区切り文字集合に入っているので、
/// トークンはそこで終わる）。フィールド行 `mode: 'リリア:B` は §5.0.5 が**最初の**
/// `:` / `：` でキーと値を分けたあとの値を走査するので、値の中の半角 `:` が修飾子になる。
pub const MODIFIER_SEP: char = ':';

/// **資料タイトル修飾子**を終わらせる文字（§6.2.4 `material_title`）。
///
/// §6.2.4 の EBNF は `/[^\s「」。、;]+/` だが、それだと §6.6.1 の例
/// `'M0137:科学の甲子園 本戦の運営と実技課題`（タイトルに空白が入る）を表現できない。
/// そこで**空白は区切りにしない**（`docs/history.md` B-60）。半角 `;` と全角 `；` の両方を入れる。
///
/// モード式（`Character` の修飾子）はこれではなく §6.2.1 の区切り文字集合
/// （[`is_delimiter`]、空白と `'` を含む）で終わる。`'リリア:B と '{リリアナ}` の
/// `B` が行末まで飲み込まれないためである（B-62）。
pub const TITLE_DELIMITERS: &[char] = &['「', '」', '『', '』', '。', '、', '；', ';', '\n'];

/// 未解決参照を終わらせる文字（§6.2.1「未解決時の区切り文字集合（固定）」）。
///
/// 空白（半角・全角・タブ）は [`lexer::is_space`] が見るのでここには並べない。
/// **ASCII の `.` と `,` は入っていない**（仕様どおり。全角の `．` `，` は入る）。
pub const DELIMITERS: &[char] = &[
    '"', '「', '」', '『', '』', '、', '。', '，', '．', '！', '？', '；', '：', '@', '\'', '(',
    ')', '（', '）', '/',
];

/// 長音として扱う文字（§6.1.3 の 3）。すべて先頭の `ー`（U+30FC）に統一する。
pub const LONG_VOWELS: &[char] = &['ー', '－', '—', '〜'];

/// 1 エントリの識別子（[`Dictionary::entries`] の添字）。
pub type EntryId = u32;

// ============================================================ 正規化（§6.1.3）

/// §6.1.3 の正規化（照合用）。**元テキストは書き換えない**ので、これは比較用の複製を作る。
pub fn normalize(key: &str) -> String {
    normalize_scanning(key).0
}

/// 正規化と、**正規化後の byte 位置 → 元の byte 位置** の対応表。
pub fn normalize_scanning(text: &str) -> (String, OffsetMap) {
    let mut out = String::with_capacity(text.len());
    let mut records = Vec::new();
    for (at, c) in text.char_indices() {
        let before = out.len();
        push_normalized(&mut out, c);
        if out.len() == before {
            // 空白は落とす（§6.1.3 の 4）。対応表にも載せないので、前後の文字の位置が
            // そのまま「空白を飛ばしたあとなり合う位置」になる。
            continue;
        }
        records.push((before, at, at + c.len_utf8()));
    }
    (out, OffsetMap { records })
}

/// 1 文字を正規化して `out` に足す。空白（と、NFKC が空白を生む場合）は何も足さない。
///
/// **確保を避けるため、NFKC が恒等な文字は直接 push する**（`nfkc()` を 1 文字ずつ
/// `String` に集めると 5,000 エントリで §18.1 の 10ms を超えた）。日本語の本文と
/// 辞書キーのほとんどは [`is_nfkc_stable`] の範囲に収まる。
fn push_normalized(out: &mut String, c: char) {
    if lexer::is_space(c) {
        return;
    }
    let c = unify_long_vowel(c);
    if is_nfkc_stable(c) {
        // ASCII は小文字化だけ（§6.1.3 の 2）。それ以外は形が変わらないのでそのまま
        out.extend(c.to_lowercase());
        return;
    }
    // 半角カナ・全角英数字・単位記号など、NFKC が形を変える文字だけ遅い経路を通る
    let decomposed: String = c.nfkc().collect();
    for c in decomposed.chars() {
        if lexer::is_space(c) {
            // U+3000 などが NFKC で半角空白になる（§6.1.3 の 4）
            continue;
        }
        out.extend(c.to_lowercase());
    }
}

/// NFKC を適用しても形が変わらない文字か（＝直接 push してよいか）。
///
/// 範囲は**狭く**取る。外れた文字は `nfkc()` を通すので、判定が甘くても正しさは
/// 変わらない（速さだけの話）。
fn is_nfkc_stable(c: char) -> bool {
    matches!(c as u32,
        // ASCII（NFKC は恒等。大文字は `to_lowercase` が処理する）
        0x0021..=0x007E
        // CJK 約物（`、` `。` `「」` `『』` `〜` 等。U+3000 の全角空白は上で落ちている）
        | 0x3001..=0x303F
        // ひらがな・かたかな（半角カナ FF61..FF9F は**含まない**。NFKC が全角にする）
        | 0x3041..=0x30FF
        // CJK 統合漢字
        | 0x4E00..=0x9FFF)
}

/// §6.1.3 の 3（長音統一）。4 文字以外はそのまま。**ASCII の `-` は長音にしない**
/// （`M-01` のような ID のハイフンを壊さないため）。
fn unify_long_vowel(c: char) -> char {
    if LONG_VOWELS.contains(&c) {
        LONG_VOWELS[0]
    } else {
        c
    }
}

/// 正規化後の byte 位置を元の byte 位置に戻す表。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OffsetMap {
    /// `(正規化後の開始 byte, 元の開始 byte, 元の終了 byte)`。正規化後の開始 byte 昇順。
    records: Vec<(usize, usize, usize)>,
}

impl OffsetMap {
    /// 正規化テキストの `at` に対応する**元の開始 byte**。
    ///
    /// `at` の位置にある文字を作った原文の文字の先頭を返す。先頭の空白は飛ばされる。
    pub fn original_start(&self, at: usize) -> usize {
        match self
            .records
            .partition_point(|(start, _, _)| *start <= at)
            .checked_sub(1)
        {
            Some(found) => self.records[found].1,
            None => 0,
        }
    }

    /// 正規化テキストの `end`（**排他**）に対応する**元の終了 byte**。
    ///
    /// `end` までに 1 文字も消費していなければ [`OffsetMap::original_start`]`(0)`。
    pub fn original_end(&self, end: usize) -> usize {
        if end == 0 {
            return self.original_start(0);
        }
        match self
            .records
            .partition_point(|(start, _, _)| *start < end)
            .checked_sub(1)
        {
            Some(found) => self.records[found].2,
            None => self.original_start(0),
        }
    }

    /// 対応表が空か（元のテキストが空白だけだった等）。
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// §6.2.1 の区切り文字か。
pub fn is_delimiter(c: char) -> bool {
    lexer::is_space(c) || DELIMITERS.contains(&c)
}

// ============================================================ §6.2.6 形態素境界

/// §6.2.6 の「助詞切り出し」に使う助詞（**長いものから**照合する）。
///
/// 形態素解析（[`tiniestsegmenter`]）は助詞を独立したトークンに切ることが多いが、
/// 語尾に取り込まれてしまう場合の保険として、先頭トークンの末尾から落とす。
/// `です` / `ます` は助動詞だが、`words.md` に `'です` を登録したい作家はいないので同じ扱いにする。
///
/// ここに並ぶのは**日本語という言語の語彙**であって UI の文言ではない（§12.2 の例外。
/// UI 言語が `en` でも `'ゆきこが` の助詞は `が` のままで、カタログには置けない）。
///
/// i18n: format-vocabulary
pub const PARTICLES: &[&str] = &[
    "かしら",
    "ください",
    "ばかり",
    "ましょう",
    "まで",
    "から",
    "より",
    "だけ",
    "ほど",
    "など",
    "って",
    "です",
    "ます",
    "でも",
    "には",
    "とは",
    "は",
    "が",
    "を",
    "に",
    "へ",
    "と",
    "で",
    "も",
    "や",
    "の",
    "か",
    "ね",
    "よ",
    "な",
    "ぞ",
    "ぜ",
    "わ",
    "さ",
];

/// 助詞そのものか（[`PARTICLES`] との完全一致）。
fn is_particle(token: &str) -> bool {
    PARTICLES.contains(&token)
}

/// 先頭トークンの末尾から助詞を 1 つ落とした長さ（落とせなければ `None`）。
///
/// 落としたあとが空になる場合は落とさない（`'だれか` の `か` のような「助詞だけの
/// 形態素」を消して 0 バイトの範囲にしないため）。
fn strip_trailing_particle(token: &str) -> Option<usize> {
    PARTICLES
        .iter()
        .filter_map(|particle| {
            let rest = token.len().checked_sub(particle.len())?;
            (rest > 0 && token.ends_with(particle)).then_some(rest)
        })
        .min()
}

/// 先頭の**文字種の連なり**の長さ（`text` が空なら 0）。
fn first_run_end(text: &str) -> usize {
    let mut class = None;
    for (at, c) in text.char_indices() {
        let found = classify(c);
        match class {
            None => class = Some(found),
            Some(previous) if previous == found => {}
            Some(_) => return at,
        }
    }
    text.len()
}

/// §6.2.6 の簡易形態素境界。`'` の直後から始まる `text` の**先頭 1 形態素（語幹）**の
/// byte 長を返す。
///
/// 手順は 3 段:
///
/// 1. §6.2.1 の区切り文字までを候補にする（句読点・空白・括弧で必ず終わる）。
/// 2. 候補の**先頭の文字種の連なり**（漢字 / カタカナ / ひらがな / 英数字）を取る。
///    ここで切るので `'科学の甲子園` は `科学`、`'リリアは` は `リリア` になる。
///    **英数字の連なりはそのまま返す**（`'M0137` を `M` に割ると §6.6.3 の資料 ID の形が
///    失われ、**E007** が出なくなる）。
/// 3. 連なりの中を [`tiniestsegmenter::tokenize`] に掛け、先頭トークンを採る。さらに末尾の
///    助詞（[`PARTICLES`]）を 1 つ落とす（`'だれか` → `だれ`）。
///
/// `text` の先頭が区切り文字なら 0 を返す（`scan_line` はその場合トークンを作らない）。
pub fn first_morpheme_end(text: &str) -> usize {
    let window = scan_to_delimiter(text);
    if window == 0 {
        return 0;
    }
    let candidate = &text[..window];
    let run = first_run_end(candidate);
    let head = &candidate[..run];
    let Some(first) = candidate.chars().next() else {
        return 0;
    };
    if classify(first) == CharClass::Alphanumeric {
        return run;
    }
    let tokens = tiniestsegmenter::tokenize(head);
    let Some(token) = tokens.first().copied() else {
        return run;
    };
    // 先頭トークンが助詞そのもの（`'はなこ` の `は`）なら、語幹はその先にあるので
    // 形態素では切らずに**文字種の連なり**まで戻す（助詞 1 文字だけの赤波線を作らない）。
    if is_particle(token) {
        return run;
    }
    // 1 つのトークンに丸ごと飲み込まれていたときだけ末尾の助詞を落とす
    // （`'ゆきこが` → `ゆきこ`）。トークンが連なりより短いときは、その境界が
    // 形態素解析の答えなので触らない（`'もののあはれ` の `もの` を `も` にしない）。
    let mut end = if token.len() == run {
        strip_trailing_particle(token).unwrap_or(run)
    } else {
        token.len()
    };
    // 形態素解析の結果が連なりからはみ出すことは無い（同じ文字列を渡している）が、
    // 万一 byte 境界がずれても本文を壊さないよう、必ず**文字の境界**へ丸める。
    end = end.min(run);
    while end > 0 && !candidate.is_char_boundary(end) {
        end -= 1;
    }
    if end == 0 {
        run
    } else {
        end
    }
}

// ============================================================ エントリ

/// エントリの種別（§6.1.1 の表）。
///
/// 衝突時の優先順位は [`EntryKind::priority`] の昇順（小さいほど強い）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub enum EntryKind {
    /// `characters.md` の H1 と `alias`（§6.1.1）。修飾子はモード ID（`:B` / `:B+D`）。
    Character,
    /// `characters.md` の `address:` の値（§5.5.6）。
    ///
    /// `address: '葵あおい => あおいちゃん` の呼称 `あおいちゃん` がここに入り、`'あおいちゃん` が
    /// 辞書で解決できるようになる。`* =>`（既定の呼称）は相手が定まらないので入れない。
    /// 優先順位（`Character > Address > Word > Material > MaterialTitle`）は §6.1.1 の固定値。
    Address,
    /// `words.md` の H1 と `alias`（§6.1.1）。修飾子は付けられない（**E026**）。
    Word,
    /// `materials/index.json` の `id`（§6.1.1）。修飾子は資料タイトル（§6.6）。
    Material,
    /// `materials/index.json` の `title`（§6.1.1）。修飾子は付けられない（**E026**、B-62）。
    MaterialTitle,
}

impl EntryKind {
    /// §6.1.1 の衝突規則（固定）での優先順位。**小さいほうが強い**。
    pub fn priority(self) -> u8 {
        match self {
            Self::Character => 0,
            Self::Address => 1,
            Self::Word => 2,
            Self::Material => 3,
            Self::MaterialTitle => 4,
        }
    }

    /// §6.2.4 で修飾子（`:` の後ろ）を持てる種別か。
    pub fn allows_modifier(self) -> bool {
        matches!(self, Self::Character | Self::Material)
    }

    /// セマンティックトークンの種別（§6.3）。未解決は `ref.unresolved`（ここに無い）。
    pub fn token_kind(self) -> &'static str {
        match self {
            Self::Character => "ref.character",
            Self::Address => "ref.address",
            Self::Word => "ref.word",
            Self::Material | Self::MaterialTitle => "ref.material",
        }
    }
}

/// 解決先（参照ペイン §9.5・ホバー §9.6・定義ジャンプ §3.4 が引く先）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// `characters.md` のキャラ。`index` は [`Characters::characters`] の添字。
    ///
    /// `name` は **H1 の表示名**（別名で引いても表示名を返す。別名はキャラへの入口に
    /// すぎない、という §6.1.1 の `Address` の特則と同じ流儀）。
    Character { index: usize, name: String },
    /// `words.md` の用語。`index` は [`Words::words`] の添字。`term` は H1 の用語。
    Word { index: usize, term: String },
    /// `materials/index.json` の資料 ID（§6.6）。`MaterialTitle` もここに正規化する。
    Material { id: String },
}

/// エントリがどのファイルのどこから来たか（診断の位置。§7.2「位置は byte range」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// ファイル（§4.2 の固定 3 ファイル）。
    pub file: SourceFile,
    /// **ファイル内の byte 範囲**。分からないときは `None`。
    pub range: Option<Range<usize>>,
}

impl Origin {
    /// 行番号（[`LineTable`](crate::core::text::position::LineTable) を持つ呼び出し側が使う）。
    pub fn line(&self, starts: &[usize]) -> Option<usize> {
        let at = self.range.as_ref()?.start;
        Some(
            starts
                .partition_point(|&start| start <= at)
                .saturating_sub(1),
        )
    }
}

/// 辞書の供給元（§6.1.1 の表の「供給元」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceFile {
    /// `characters.md`（§5.5）。
    Characters,
    /// `words.md`（§5.6）。
    Words,
    /// `materials/index.json`（§5.8.2）。
    MaterialsIndex,
}

impl SourceFile {
    /// プロジェクトルートからの相対パス（§4.2 の固定レイアウト）。
    pub fn path(self) -> &'static str {
        match self {
            Self::Characters => "characters.md",
            Self::Words => "words.md",
            Self::MaterialsIndex => "materials/index.json",
        }
    }
}

/// 辞書の 1 エントリ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 種別。
    pub kind: EntryKind,
    /// **書かれたままの**キー（表示名・別名・用語・ID・タイトル）。
    pub key: String,
    /// [`normalize`] 後のキー（照合に使う本体）。
    pub normalized: String,
    /// 解決先。
    pub target: Target,
    /// 出所（診断の位置）。
    pub origin: Origin,
}

impl Entry {
    /// 1 エントリを作る。`normalized` は [`normalize`] で埋める。
    pub fn new(kind: EntryKind, key: impl Into<String>, target: Target, origin: Origin) -> Self {
        let key = key.into();
        let normalized = normalize(&key);
        Self {
            kind,
            key,
            normalized,
            target,
            origin,
        }
    }

    /// 長すぎて登録できないキーか（§6.2.1 → **E028**）。
    pub fn is_too_long(&self) -> bool {
        self.normalized.len() > MAX_KEY_BYTES
    }
}

// ============================================================ トライ

/// 1 ノード。子は char 昇順に並ぶ（[`Trie::child`] が二分探索する）。
#[derive(Debug, Clone, Default)]
struct TrieNode {
    children: Vec<(char, u32)>,
    terminal: Option<EntryId>,
}

/// 正規化キーのトライ（§6.1.2「再構築は O(総キー長)」）。
#[derive(Debug, Clone, Default)]
struct Trie {
    nodes: Vec<TrieNode>,
}

impl Trie {
    fn new() -> Self {
        Self {
            nodes: vec![TrieNode::default()],
        }
    }

    /// 1 キーを登録する。**終端が既にあれば上書きしない**（＝先に挿入した強い種別が勝つ）。
    ///
    /// 戻り値は「終端を新しく置いたか」。
    fn insert(&mut self, key: &str, id: EntryId) -> bool {
        let mut node = 0u32;
        for c in key.chars() {
            node = match self.child(node, c) {
                Some(next) => next,
                None => self.extend(node, c),
            };
        }
        let slot = &mut self.nodes[node as usize].terminal;
        if slot.is_some() {
            return false;
        }
        *slot = Some(id);
        true
    }

    /// 子ノードを 1 つ生やして返す。
    ///
    /// 子は char 昇順を保つ（[`Trie::child`] が二分探索する）。**挿入位置に insert する**ので
    /// 並べ直しは要らない（push して毎回ソートすると 5,000 エントリで §18.1 の 10ms を超えた）。
    fn extend(&mut self, node: u32, c: char) -> u32 {
        let fresh = self.nodes.len() as u32;
        let at = self.nodes[node as usize]
            .children
            .partition_point(|(key, _)| *key < c);
        self.nodes[node as usize].children.insert(at, (c, fresh));
        self.nodes.push(TrieNode::default());
        fresh
    }

    /// 完全一致で終端を引く（[`Dictionary::lookup`] が使う）。途中で道が無ければ `None`。
    fn terminal(&self, key: &str) -> Option<EntryId> {
        let mut node = 0u32;
        for c in key.chars() {
            node = self.child(node, c)?;
        }
        self.nodes[node as usize].terminal
    }

    /// 子を探す（char 昇順なので二分探索）。
    fn child(&self, node: u32, c: char) -> Option<u32> {
        let children = &self.nodes[node as usize].children;
        let at = children.partition_point(|(key, _)| *key < c);
        children
            .get(at)
            .and_then(|(key, next)| (*key == c).then_some(*next))
    }

    /// §6.2.1 の (b) 最短一致スキャン。
    ///
    /// `text` は **正規化済み**の残りテキスト（`'` の後ろ）。最初の終端で最短一致が確定し、
    /// そこから [`MAX_KEY_BYTES`] まで歩いて **W029** 用の「より長い候補」も拾う。
    fn scan(&self, text: &str) -> Option<Scan> {
        let mut node = 0u32;
        let mut consumed = 0usize;
        let mut shortest: Option<(EntryId, usize)> = None;
        let mut longest: Option<(EntryId, usize)> = None;
        for c in text.chars() {
            let Some(next) = self.child(node, c) else {
                break;
            };
            node = next;
            consumed += c.len_utf8();
            if let Some(id) = self.nodes[node as usize].terminal {
                if shortest.is_none() {
                    shortest = Some((id, consumed));
                }
                longest = Some((id, consumed));
            }
            // 疑似コード（§6.2.1）と同じく、終端を確認した**あと**に上限を見る
            if consumed >= MAX_KEY_BYTES {
                break;
            }
        }
        let (entry, end) = shortest?;
        let longer = longest.filter(|(_, at)| *at > end).map(|(id, _)| id);
        Some(Scan { entry, end, longer })
    }
}

/// スキャン結果（正規化テキスト上の長さを含む）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scan {
    entry: EntryId,
    /// 一致した正規化キーの byte 長。
    end: usize,
    /// より長い候補（**W029**）。無ければ `None`。
    longer: Option<EntryId>,
}

// ============================================================ 辞書

/// `'` の後ろで解決される辞書 1 本（§6.1）。
///
/// 構築は [`Dictionary::from_sources`]（3 ファイルから）か [`Dictionary::build`]
/// （エントリ表を渡す。テスト用）。どちらも [`Built`] を返し、
/// **構築時に出る診断**（E028 / W027）を一緒に返す。
///
/// §6.1.2「いずれかが変化したときのみ再構築する」の判定は**呼び出し側**（M5 の
/// ストレージと M6 の状態管理）が行う。ここは純粋な構築関数。
#[derive(Debug)]
pub struct Dictionary {
    entries: Vec<Entry>,
    /// **衝突したキーだけ**の表（正規化キー → 勝者と敗者、優先順位順）。
    ///
    /// 5,000 エントリで毎回キーを複製すると §18.1 の 10ms を超えるので、全キーの索引は
    /// 持たない。完全一致（[`Dictionary::lookup`]）はトライを最後まで歩くだけで済み、
    /// 「別種別の同名項目あり」（§6.1.1）の併記はこの表があれば足りる。
    conflicts: BTreeMap<String, Vec<EntryId>>,
    trie: Trie,
    /// 用語の本文中出現検出（§6.1.2「Aho-Corasick を併用する」）用の遅延キャッシュ。
    words: OnceLock<Option<AhoCorasick>>,
    /// [`Dictionary::words`] のパターン順 → エントリ。
    word_entries: Vec<EntryId>,
}

/// [`Dictionary`] の構築結果。
#[derive(Debug)]
pub struct Built {
    /// 辞書本体。
    pub dictionary: Dictionary,
    /// 構築時に出た診断（**ファイルごと**に選ぶ。§7.2「診断はファイル単位」）。
    pub diagnostics: Vec<FileDiagnostic>,
}

impl Built {
    /// 1 ファイルぶんの診断だけ取り出す。
    pub fn diagnostics_for(&self, file: SourceFile) -> Vec<Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|found| found.file == file)
            .map(|found| found.diagnostic.clone())
            .collect()
    }
}

/// どのファイルの診断か（`Diagnostic` はファイル内の byte range しか持たないので）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiagnostic {
    /// ファイル。
    pub file: SourceFile,
    /// 診断。`range` は**そのファイル内の byte 範囲**。
    pub diagnostic: Diagnostic,
}

impl Dictionary {
    /// 空の辞書（プロジェクトがまだ何も持たないとき。§4.3-4「起動失敗にしない」）。
    pub fn empty() -> Self {
        Self::build(Vec::new()).dictionary
    }

    /// 3 ファイルの木から辞書を作る（§6.1.1 の「供給元」の表）。
    ///
    /// `index.json` が壊れている（**E025**、§4.3-4）ときは [`Index::index_or_default`] の
    /// 空索引を使うので、資料エントリ 0 件で続行する。E025 自身は M2 の
    /// [`parse`](crate::core::syntax::materials_index::parse) が出す（ここでは繰り返さない）。
    pub fn from_sources(
        characters: &Characters<'_>,
        words: &Words<'_>,
        index: &Index<'_>,
    ) -> Built {
        let mut entries = Vec::new();
        collect_characters(characters, &mut entries);
        let starts = line_starts(&characters.lines);
        for address in crate::core::features::metadata::addresses(&characters.to_text()) {
            if address.target == "*" {
                continue;
            }
            if let Some((index, found)) = characters
                .characters
                .iter()
                .enumerate()
                .find(|(_, c)| c.name == address.target)
            {
                for name in address.names {
                    entries.push(Entry::new(
                        EntryKind::Address,
                        name,
                        Target::Character {
                            index,
                            name: found.name.to_owned(),
                        },
                        Origin {
                            file: SourceFile::Characters,
                            range: Some(
                                starts.get(address.line).copied().unwrap_or(0)
                                    ..starts
                                        .get(address.line + 1)
                                        .copied()
                                        .unwrap_or_else(|| characters.to_text().len()),
                            ),
                        },
                    ));
                }
            }
        }
        collect_words(words, &mut entries);
        collect_materials(index, &mut entries);
        Self::build(entries)
    }

    /// エントリ表から辞書を作る。
    ///
    /// 並びは問わない（内部で §6.1.1 の優先順位順に安定ソートする）。構築時に出る診断:
    ///
    /// | コード | 条件 | 位置 |
    /// |---|---|---|
    /// | **E028** | 正規化後のキーが [`MAX_KEY_BYTES`] 超（§6.2.1） | そのキーの出所。**登録しない** |
    /// | **W027** | 同一キーが複数**種別**に存在（§6.1.1） | **負けた側**のエントリの出所 |
    ///
    /// 同じ種別の中の重複（キャラ名と別名が同じ、2 人の別名が同じ等）は W027 にしない。
    /// 定義側の重複は **E008**（同一ファイル内の ID 重複）として M2 のパーサが出している。
    pub fn build(mut entries: Vec<Entry>) -> Built {
        // 優先順位順に並べると、トライへの挿入順がそのまま勝者順になる。
        // `sort_by_key` は**安定**なので、同じ種別の中では書かれた順が残る
        // （＝ E008 が出るような重複で「先に書いたほう」が勝つ）
        entries.sort_by_key(|entry| entry.kind.priority());

        let mut diagnostics = Vec::new();
        let mut kept: Vec<Entry> = Vec::with_capacity(entries.len());
        let mut conflicts: BTreeMap<String, Vec<EntryId>> = BTreeMap::new();
        let mut trie = Trie::new();
        let mut word_entries: Vec<EntryId> = Vec::new();

        for entry in entries {
            // §6.2.1「これを超える辞書キーは登録時に E028 で拒否する」
            if entry.is_too_long() {
                diagnostics.push(FileDiagnostic {
                    file: entry.origin.file,
                    diagnostic: Diagnostic::simple(
                        DiagnosticCode::E028,
                        entry.origin.range.clone(),
                    ),
                });
                continue;
            }
            if entry.normalized.is_empty() {
                // 空白だけのキーは参照しようがない（`'` の直後が区切り文字なら
                // scan_line がトークンを作らない）。対応コードが無いので黙って落とす
                continue;
            }
            let id = kept.len() as EntryId;
            // トライの終端が既にあれば衝突（優先順位順に入れているので先に入ったほうが勝つ）
            match trie.terminal(&entry.normalized) {
                Some(winner) => {
                    // §6.1.1「衝突したキーには起動時に W027 を出す」
                    if kept[winner as usize].kind != entry.kind {
                        diagnostics.push(FileDiagnostic {
                            file: entry.origin.file,
                            diagnostic: Diagnostic::new(
                                DiagnosticCode::W027,
                                entry.origin.range.clone(),
                                vec![Arg::text(entry.key.clone())],
                            ),
                        });
                    }
                    // 衝突したキーだけ表に持つ（全キーの索引は §18.1 の 10ms に収まらない）
                    conflicts
                        .entry(entry.normalized.clone())
                        .or_insert_with(|| vec![winner])
                        .push(id);
                }
                // 終端は上書きしないので、insert は衝突しなかったときだけ
                None => {
                    trie.insert(&entry.normalized, id);
                }
            }
            if entry.kind == EntryKind::Word {
                word_entries.push(id);
            }
            kept.push(entry);
        }

        let dictionary = Self {
            entries: kept,
            conflicts,
            trie,
            words: OnceLock::new(),
            word_entries,
        };
        Built {
            dictionary,
            diagnostics,
        }
    }

    /// エントリ数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 空か。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// エントリを引く。
    pub fn entry(&self, id: EntryId) -> Option<&Entry> {
        self.entries.get(id as usize)
    }

    /// 全エントリ（優先順位順）。
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// §6.2.1 の (b) 最短一致。`text` は**正規化済み**でなければならない。
    fn scan(&self, text: &str) -> Option<Scan> {
        self.trie.scan(text)
    }

    /// §6.1.4（F-24）の重複抑止: 正規化後のキー `key` が **`Character` と `Word` の
    /// 両方**に存在するか。
    ///
    /// `characters.md` の名前・別名は `words.md` に書き足さなくても辞書に入るので、
    /// 作家が同じ名前を用語としても書いたときに 2 件見える。UI（参照ペイン・パレット・
    /// `'` の補完候補）はそのとき **`Character` だけを出す**（`characters.md` のエントリへ
    /// 透過的に統合する）。`words.md` のファイル自体は書き換えない。
    pub fn is_character_word_duplicate(&self, key: &str) -> bool {
        let normalized = normalize(key);
        let Some(ids) = self.conflicts.get(&normalized) else {
            return false;
        };
        let mut character = false;
        let mut word = false;
        for id in ids {
            match self.entry(*id).map(|entry| entry.kind) {
                Some(EntryKind::Character) => character = true,
                Some(EntryKind::Word) => word = true,
                _ => {}
            }
        }
        character && word
    }

    /// 完全一致で引く（話者注釈 `@名前`（§6.2.5）と明示形 `'{名前}`（§6.2.3）用）。
    ///
    /// 正規化してから引き、**優先順位の勝者**を返す。`kinds` が `Some` ならその種別だけに
    /// 絞る（`@` は `Character` のみ。§6.1.1「`Address` は話者注釈 `@` の解決対象外」）。
    pub fn lookup(&self, key: &str, kinds: Option<&[EntryKind]>) -> Option<EntryId> {
        let normalized = normalize(key);
        // 勝者はトライの終端（構築時に優先順位順に入れているので終端 = 勝者）
        let winner = self.trie.terminal(&normalized)?;
        let acceptable = |id: EntryId| match kinds {
            Some(allowed) => allowed.contains(&self.entries[id as usize].kind),
            None => true,
        };
        if acceptable(winner) {
            return Some(winner);
        }
        // 勝者が対象外の種別のときだけ衝突表を見る（`@` が `Character` だけを欲しい場合など）
        self.conflicts
            .get(&normalized)?
            .iter()
            .copied()
            .find(|&id| acceptable(id))
    }

    /// 同名の**別種別**エントリ（§6.1.1「参照ペインに『別種別の同名項目あり』を併記する」）。
    ///
    /// 同じ種別の重複（キャラ名と別名が同じ等）は返さない。それは **E008**（同一ファイル内の
    /// ID 重複）の領分で、W027 も出ない。`id` 自身も含まない。衝突が無ければ空。
    pub fn conflicts(&self, id: EntryId) -> Vec<EntryId> {
        let Some(entry) = self.entry(id) else {
            return Vec::new();
        };
        let kind = entry.kind;
        let Some(same_key) = self.conflicts.get(&entry.normalized) else {
            return Vec::new();
        };
        same_key
            .iter()
            .copied()
            .filter(|&other| {
                other != id && self.entry(other).is_some_and(|found| found.kind != kind)
            })
            .collect()
    }

    /// 本文中の**用語の出現**を拾う（§6.1.2「参照ペインの『この節に出てくる用語』」）。
    ///
    /// `'` の有無を問わない素の文字列を探すので Aho-Corasick を使う（v0 を踏襲）。
    /// 比較は正規化後（§6.1.3）で、返す範囲は**元のテキストの byte 位置**。
    /// 返る順は出現順。重なりのある一致は Aho-Corasick の既定（左寄り・最短）で 1 本だけ。
    pub fn occurrences(&self, text: &str) -> Vec<Occurrence> {
        let Some(automaton) = self.word_automaton() else {
            return Vec::new();
        };
        let (normalized, map) = normalize_scanning(text);
        automaton
            .find_iter(&normalized)
            .map(|found| Occurrence {
                entry: self.word_entries[found.pattern().as_usize()],
                range: map.original_start(found.start())..map.original_end(found.end()),
            })
            .filter(|found| found.range.start < found.range.end)
            .collect()
    }

    /// 用語だけの Aho-Corasick（初回だけ作る）。用語が無ければ `None`。
    fn word_automaton(&self) -> Option<&AhoCorasick> {
        self.words
            .get_or_init(|| {
                let patterns: Vec<&str> = self
                    .word_entries
                    .iter()
                    .map(|&id| self.entries[id as usize].normalized.as_str())
                    .collect();
                if patterns.is_empty() {
                    return None;
                }
                AhoCorasick::new(&patterns).ok()
            })
            .as_ref()
    }
}

/// [`Dictionary::occurrences`] の 1 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    /// 当たったエントリ。
    pub entry: EntryId,
    /// **元のテキスト**の byte 範囲。
    pub range: Range<usize>,
}

// ============================================================ エントリの収集

/// `characters.md` → `Character` エントリ（H1 と `alias`、§6.1.1）。
fn collect_characters(characters: &Characters<'_>, out: &mut Vec<Entry>) {
    let starts = line_starts(&characters.lines);
    for (index, found) in characters.characters.iter().enumerate() {
        let target = Target::Character {
            index,
            name: found.name.to_string(),
        };
        let at = starts.get(found.heading).copied().unwrap_or(0);
        // H1 の表示名（§5.5.2 `char_name`）
        out.push(Entry::new(
            EntryKind::Character,
            found.name,
            target.clone(),
            Origin {
                file: SourceFile::Characters,
                range: Some(shift(
                    &(found.name_at..found.name_at + found.name.len()),
                    at,
                )),
            },
        ));
        // `alias:`（§5.5.3「カンマ区切り。**すべて辞書に登録される**」）
        for line in &found.fields {
            let Some(field) = characters.field(*line) else {
                continue;
            };
            if field.key != FieldKey::Alias {
                continue;
            }
            let base = starts.get(*line).copied().unwrap_or(0);
            out.extend(alias_entries(
                field.value,
                base + field.value_at,
                &target,
                EntryKind::Character,
            ));
        }
    }
}

/// `words.md` → `Word` エントリ（H1 と `alias`、§6.1.1）。
fn collect_words(words: &Words<'_>, out: &mut Vec<Entry>) {
    let starts = line_starts(&words.lines);
    for (index, found) in words.words.iter().enumerate() {
        let target = Target::Word {
            index,
            term: found.term.to_string(),
        };
        let at = starts.get(found.heading).copied().unwrap_or(0);
        out.push(Entry::new(
            EntryKind::Word,
            found.term,
            target.clone(),
            Origin {
                file: SourceFile::Words,
                range: Some(shift(
                    &(found.term_at..found.term_at + found.term.len()),
                    at,
                )),
            },
        ));
        for line in &found.fields {
            let Some(field) = words.field(*line) else {
                continue;
            };
            if field.key != FieldKey::Alias {
                continue;
            }
            let base = starts.get(*line).copied().unwrap_or(0);
            out.extend(alias_entries(
                field.value,
                base + field.value_at,
                &target,
                EntryKind::Word,
            ));
        }
    }
}

/// `alias:` の値を割ってエントリにする（**継続行に書かれた項目は位置が分からない**ので
/// `range: None`。M2 の `Field` は継続行の行番号を持たない）。
fn alias_entries(value: &str, value_at: usize, target: &Target, kind: EntryKind) -> Vec<Entry> {
    let file = match target {
        Target::Character { .. } => SourceFile::Characters,
        Target::Word { .. } => SourceFile::Words,
        Target::Material { .. } => SourceFile::MaterialsIndex,
    };
    split_list(value)
        .into_iter()
        .map(|(at, alias)| {
            Entry::new(
                kind,
                alias,
                target.clone(),
                Origin {
                    file,
                    range: Some(value_at + at..value_at + at + alias.len()),
                },
            )
        })
        .collect()
}

/// `materials/index.json` → `Material`（ID）と `MaterialTitle`（タイトル）（§6.1.1）。
fn collect_materials(index: &Index<'_>, out: &mut Vec<Entry>) {
    let raw = index.raw;
    let mut cursor = 0usize;
    for item in &index.index_or_default().items {
        // JSON のトークン位置は serde が持たないので、値のリテラルを原文から探す
        // （M2 の materials_index.rs と同じ流儀。書いた順にカーソルを進める）
        let id_range = find_literal(raw, &item.id, &mut cursor);
        let title_range = find_literal(raw, &item.title, &mut cursor);
        let target = Target::Material {
            id: item.id.clone(),
        };
        out.push(Entry::new(
            EntryKind::Material,
            item.id.clone(),
            target.clone(),
            Origin {
                file: SourceFile::MaterialsIndex,
                range: id_range,
            },
        ));
        if !item.title.is_empty() {
            out.push(Entry::new(
                EntryKind::MaterialTitle,
                item.title.clone(),
                target,
                Origin {
                    file: SourceFile::MaterialsIndex,
                    range: title_range,
                },
            ));
        }
    }
}

/// JSON リテラル `"value"` を `from` 以降から探す（見つけた位置まで `from` を進める）。
///
/// 返るのは**引用符を除いた値そのもの**の範囲。JSON エスケープ（`\"` 等）を含む値は
/// 見つからないことがある（`None`）。そのときは診断の位置が出ないだけで、解決はできる。
fn find_literal(raw: &str, value: &str, from: &mut usize) -> Option<Range<usize>> {
    if value.is_empty() {
        return None;
    }
    let needle = format!("\"{value}\"");
    let at = raw.get(*from..)?.find(&needle)? + *from;
    *from = at + needle.len();
    Some(at + 1..at + needle.len() - 1)
}

/// MD 方言の木の**行頭 byte 位置**（各行 `raw.len() + 1`（LF）を積む）。
///
/// 最終行のうしろに LF が無いぶんだけ `to_text()` より 1 多い値が末尾に並ぶが、
/// 行の範囲を足すのに使うぶんには問題ない（存在しない行は参照しない）。
fn line_starts(lines: &[MdLine<'_>]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(lines.len());
    let mut at = 0usize;
    for line in lines {
        starts.push(at);
        at += line.raw().len() + 1;
    }
    starts
}

/// 行内相対の範囲をファイル単位の範囲にする。
fn shift(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start + by..range.end + by
}

/// `alias:` の値をカンマで割る（§5.5.3「カンマ区切り」）。
///
/// 区切りは **半角カンマ・読点 `、`・全角カンマ `，`**（B-59）。`、` は §6.2.1 の
/// 区切り文字集合に入っているので、`、` を含むキーはどのみち参照できない。全角カンマは
/// NFKC で半角になるが、**ここでは原文を割る**ので明示的に入れる。
///
/// 返るのは `(値の先頭からの byte 位置, 項目)`。空の項目（`alias: ,` や末尾カンマ）は落とす。
fn split_list(value: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (at, c) in value.char_indices() {
        if !matches!(c, ',' | '、' | '，') {
            continue;
        }
        push_item(value, &mut out, start, at);
        start = at + c.len_utf8();
    }
    push_item(value, &mut out, start, value.len());
    out
}

/// `value[start..end]` を前後の空白を除いて積む。
fn push_item<'a>(value: &'a str, out: &mut Vec<(usize, &'a str)>, start: usize, end: usize) {
    let item = &value[start..end];
    let head = item.len() - item.trim_start_matches(lexer::is_space).len();
    let tail = item.trim_end_matches(lexer::is_space).len();
    if tail == 0 {
        return;
    }
    out.push((start + head, &item[head..tail]));
}

// ============================================================ `'` の解決（§6.2）

/// 1 行の中の `'` 参照を全部解決する。
///
/// `line_start` は**行の先頭のファイル内 byte 位置**。診断の `range` はこれを足した
/// ファイル単位で返す（行内相対が欲しければ 0 を渡す）。[`RefToken`] の範囲は常に
/// **行内相対**（M2 の `Speaker` / `Pattern` と同じ流儀）。
///
/// 走査するのは渡された `line` の中だけなので、コメント（`# …` / `/* … */`）や
/// ルビ（`《…》`）の中を除くかどうかは**呼び出し側**が決める（M4 の `features` は
/// 本文とフィールド値だけを渡す）。
pub fn scan_line(line: &str, line_start: usize, dictionary: &Dictionary) -> LineScan {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut at = 0usize;
    while let Some(found) = line[at..].find(REF_SIGIL) {
        let start = at + found;
        at = start + REF_SIGIL.len_utf8();
        if !starts_ref(line, start) {
            continue;
        }
        let token = resolve_ref(line, start, dictionary);
        // `'` の直後が区切り文字（`''` や `' 「`）なら参照ではなくただの引用符。
        // 英語のアポストロフィ（`リリア's`）も同じ扱いで落とす（B-63）
        if !token.explicit && (token.key_range.is_empty() || is_apostrophe(line, &token)) {
            continue;
        }
        for diagnostic in token_diagnostics(line, &token, dictionary) {
            diagnostics.push(Diagnostic {
                range: diagnostic.range.map(|range| shift(&range, line_start)),
                ..diagnostic
            });
        }
        at = token.range.end.max(at);
        tokens.push(token);
    }
    LineScan {
        tokens,
        diagnostics,
    }
}

/// [`scan_line`] の結果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineScan {
    /// 見つかったトークン（行内の出現順）。
    pub tokens: Vec<RefToken>,
    /// トークンから出る診断（`line_start` を足した**ファイル単位の範囲**）。
    pub diagnostics: Vec<Diagnostic>,
}

/// 未解決の候補が**英語のアポストロフィ**か（B-63）。
///
/// `リリア's` / `M0137's` のように日本語・ID のうしろに所有格が付くと、`'` の前が
/// ASCII の英数字でないので [`starts_ref`] では落とせない。未解決かつ候補が
/// **ASCII の英字 2 文字以下**（`s` `t` `re` `ll` `ve` `m` `d`）なら参照ではないとみなす。
/// 解決できた場合はそのまま参照として扱う（1〜2 文字の別名を本当に登録した作家の
/// 意図を奪わない）。
fn is_apostrophe(line: &str, token: &RefToken) -> bool {
    if token.resolved.is_some() {
        return false;
    }
    let key = token.key(line);
    !key.is_empty() && key.len() <= 2 && key.chars().all(|c| c.is_ascii_alphabetic())
}

/// `'` が参照の始まりか。
///
/// 仕様は `'` の**開始条件**を書いていない（§6.2.1 の疑似コードは「`at`: `'` のバイト位置」
/// をもらうだけ）。英字の短縮形・所有格（`don't` / `it's`）を E001 の山にしないため、
/// **直前が ASCII の英数字なら参照ではない**とする（B-63）。M2 の `Field::refs()` は
/// フィールド値を空白区切りのトークンに分けて先頭の `'` だけを見るが、本文は日本語で
/// 空白が無いのでその規則はそのままでは使えない。
fn starts_ref(line: &str, at: usize) -> bool {
    match line[..at].chars().next_back() {
        Some(prev) => !prev.is_ascii_alphanumeric(),
        None => true,
    }
}

/// §6.2.1 の `resolve_ref`。`at` は `'` の byte 位置（**行内相対**）。
///
/// ```text
/// (a) '{...}   → 明示形（閉じ括弧が無ければ E002、行末までを未解決範囲とする）
/// (b) 最短一致 → 1 文字ずつトライを進め、終端に当たった瞬間に確定
/// (c) 未解決   → 区切り文字までを範囲とし、E001
/// ```
pub fn resolve_ref(line: &str, at: usize, dictionary: &Dictionary) -> RefToken {
    let head = at + REF_SIGIL.len_utf8();
    let rest = &line[head..];

    // (a) 明示形 '{...}（§6.2.3）
    if rest.starts_with(EXPLICIT_OPEN) {
        let inner_at = head + EXPLICIT_OPEN.len_utf8();
        let inner = &line[inner_at..];
        return match inner.find(EXPLICIT_CLOSE) {
            Some(close) => {
                let key_range = inner_at..inner_at + close;
                let closed = key_range.end + EXPLICIT_CLOSE.len_utf8();
                let resolved = dictionary.lookup(&line[key_range.clone()], None);
                let modifier = parse_modifier(line, closed, dictionary, resolved);
                RefToken {
                    range: at..modifier.as_ref().map_or(closed, |found| found.range.end),
                    morpheme_range: key_range.clone(),
                    key_range,
                    explicit: true,
                    unclosed: false,
                    resolved,
                    longer: None,
                    modifier,
                }
            }
            // §6.2.3「閉じ括弧が無い場合は E002（行末までを未解決範囲とする）」
            None => RefToken {
                range: at..line.len(),
                morpheme_range: inner_at..line.len(),
                key_range: inner_at..line.len(),
                explicit: true,
                unclosed: true,
                resolved: None,
                longer: None,
                modifier: None,
            },
        };
    }

    // (b) 最短一致（§6.2.1）
    let (normalized, map) = normalize_scanning(rest);
    if let Some(found) = dictionary.scan(&normalized) {
        let key_end = head + map.original_end(found.end);
        let resolved = Some(found.entry);
        let modifier = parse_modifier(line, key_end, dictionary, resolved);
        return RefToken {
            range: at..modifier.as_ref().map_or(key_end, |found| found.range.end),
            morpheme_range: head..key_end,
            key_range: head..key_end,
            explicit: false,
            unclosed: false,
            resolved,
            longer: found.longer,
            modifier,
        };
    }

    // (c) 未解決: 区切り文字まで（§6.2.1）。**赤波線はそのうち先頭 1 形態素だけ**（§6.2.6）
    let end = head + scan_to_delimiter(rest);
    let morpheme = head + first_morpheme_end(rest);
    RefToken {
        range: at..end,
        morpheme_range: head..morpheme,
        key_range: head..end,
        explicit: false,
        unclosed: false,
        resolved: None,
        longer: None,
        modifier: None,
    }
}

/// 未解決参照の範囲（§6.2.1「未解決時の区切り文字集合（固定）」）。
///
/// 戻り値は `rest` の先頭からの byte 長。
pub fn scan_to_delimiter(rest: &str) -> usize {
    rest.find(is_delimiter).unwrap_or(rest.len())
}

/// 1 トークン（§6.2）。範囲はすべて**行内相対**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefToken {
    /// `'` からトークンの終わりまで（修飾子を含む。明示形では `}` まで）。
    pub range: Range<usize>,
    /// 名前部（`'` と `{` `}` と修飾子を除く）。
    pub key_range: Range<usize>,
    /// §6.2.6 の**先頭 1 形態素（語幹）**の範囲。解決済みなら [`RefToken::key_range`] と同じ。
    ///
    /// **E001 の赤波線はここに引く**（「句読点まで漫然と赤波線を引くのではなく…先頭の
    /// 1 形態素（語幹）のみを未解決参照の範囲として区切り」）。トークン全体（[`RefToken::range`]）
    /// は §17.1 のエクスポート（名前部を残して `'` と修飾子を消す）や §6.6.3 の資料 ID の形の
    /// 判定が使うので、**狭めない**。
    pub morpheme_range: Range<usize>,
    /// §6.2.3 の明示形 `'{...}` だったか。
    pub explicit: bool,
    /// 明示形が**閉じていない**（§6.2.3 → **E002**）。
    pub unclosed: bool,
    /// 解決先のエントリ。未解決なら `None`。
    pub resolved: Option<EntryId>,
    /// より長い辞書項目にも一致しうる（§6.2.2 → **W029**）。
    pub longer: Option<EntryId>,
    /// `:` の後ろ（§6.2.4）。
    pub modifier: Option<Modifier>,
}

impl RefToken {
    /// 名前部（`'` と波括弧を除いた、**書かれたままの**文字列）。
    pub fn key<'a>(&self, line: &'a str) -> &'a str {
        &line[self.key_range.clone()]
    }

    /// §6.2.6 の先頭 1 形態素（語幹）。
    pub fn morpheme<'a>(&self, line: &'a str) -> &'a str {
        &line[self.morpheme_range.clone()]
    }

    /// **E001 の赤波線を引く範囲**（`'` を含み、未解決なら先頭 1 形態素まで）。
    ///
    /// 解決済み・明示形・閉じていない明示形では [`RefToken::range`] と同じ。
    pub fn unresolved_range(&self) -> Range<usize> {
        self.range.start..self.morpheme_range.end
    }

    /// 解決済みか。
    pub fn is_resolved(&self) -> bool {
        self.resolved.is_some()
    }

    /// §6.3 のトークン種別（未解決は `ref.unresolved`）。
    pub fn token_kind(&self, dictionary: &Dictionary) -> &'static str {
        match self.resolved.and_then(|id| dictionary.entry(id)) {
            Some(entry) => entry.kind.token_kind(),
            None => "ref.unresolved",
        }
    }

    /// W029 のクイックフィックスが提案する明示形（§6.2.2「明示形 `'{リリアナ}` に
    /// 書き換える」）。トークン全体（[`RefToken::range`]）をこれで置き換える。
    pub fn explicit_form(&self, dictionary: &Dictionary) -> Option<String> {
        let key = &dictionary.entry(self.longer?)?.key;
        Some(format!("{REF_SIGIL}{EXPLICIT_OPEN}{key}{EXPLICIT_CLOSE}"))
    }
}

/// `:` の後ろの修飾子（§6.2.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modifier {
    /// `:` の行内 byte 位置。
    pub at: usize,
    /// `:` を含む修飾子全体の範囲。
    pub range: Range<usize>,
    /// 値そのものの範囲（`:` と前後の空白を除く）。**E006** の波線はここ（§6.6.3）。
    pub text_range: Range<usize>,
    /// `:` の後ろ（前後の空白を除く）。空なら修飾子として扱わないので常に非空。
    pub text: String,
    /// 解釈。
    pub kind: ModifierKind,
}

/// 修飾子の解釈（§6.2.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModifierKind {
    /// `mode_expr`（`B` / `B+D`）。直前の参照が `Character` のとき。
    Modes(Vec<String>),
    /// 資料タイトル（§6.6）。直前の参照が `Material` のとき。
    Title,
    /// `mode_expr` の形が壊れている（**E002**）。
    MalformedMode,
    /// 3 つ以上のモード（**E005**、§6.4.4）。
    TooManyModes,
    /// 修飾子を付けられない種別（`Word` / `Address` / `MaterialTitle`）に付いた（**E026**）。
    NotAllowed,
}

impl Modifier {
    /// §6.2.4 の `mode_expr` として読めたときのモード ID（1〜2 個）。
    pub fn modes(&self) -> Option<&[String]> {
        match &self.kind {
            ModifierKind::Modes(found) => Some(found),
            _ => None,
        }
    }

    /// 資料タイトルか。
    pub fn is_title(&self) -> bool {
        matches!(self.kind, ModifierKind::Title)
    }
}

/// 修飾子をパースする（§6.2.4）。`at` はキーの直後の byte 位置。
///
/// **未解決の参照に `:` が続く場合は修飾子として読まない**（`'未知: なんとか` の `:` は
/// 本文の句読点。E001 は出ても E026 / E002 は重ねない）。
fn parse_modifier(
    line: &str,
    at: usize,
    dictionary: &Dictionary,
    resolved: Option<EntryId>,
) -> Option<Modifier> {
    if !line[at..].starts_with(MODIFIER_SEP) {
        return None;
    }
    let kind = dictionary.entry(resolved?)?.kind;
    let text_at = at + MODIFIER_SEP.len_utf8();
    let rest = &line[text_at..];
    // 資料タイトルは空白を含む（B-60）。モード式は §6.2.1 の区切り文字で終わる（B-62）
    let end = match kind {
        EntryKind::Material => rest
            .find(|c: char| TITLE_DELIMITERS.contains(&c))
            .unwrap_or(rest.len()),
        _ => scan_to_delimiter(rest),
    };
    let raw = &rest[..end];
    let head = raw.len() - raw.trim_start_matches(lexer::is_space).len();
    let text = raw.trim_matches(lexer::is_space);
    if text.is_empty() {
        // `'リリア:` のように後ろが空 → 修飾子ではない（`:` は本文の句読点）
        return None;
    }
    let parsed = if !kind.allows_modifier() {
        // §6.1.1「（不可。付ければ E026）」
        ModifierKind::NotAllowed
    } else if kind == EntryKind::Character {
        mode_modifier(text)
    } else {
        // §6.1.1 の表で `Material` の修飾子は資料タイトル
        ModifierKind::Title
    };
    Some(Modifier {
        at,
        range: at..text_at + end,
        text_range: text_at + head..text_at + head + text.len(),
        text: text.to_string(),
        kind: parsed,
    })
}

/// `mode_expr = mode_id , [ "+" , mode_id ]`（§5.1 / §6.2.4）。
///
/// **英数字・`_`・`+` 以外を含む場合はモード式として読まない**（B-62）。本文の
/// `'リリア: 待って` の `:` は句読点なので、ここでモード式扱いすると E002 の山になる。
fn mode_modifier(text: &str) -> ModifierKind {
    if !text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '+')
    {
        return ModifierKind::MalformedMode;
    }
    match contents::split_modes(text) {
        Some(modes) => ModifierKind::Modes(modes.into_iter().map(str::to_string).collect()),
        // split_modes が None を返すのは「3 つ以上」か「mode_id の形が違う」。
        // `+` で割った数が 3 以上なら §6.4.4 の E005、それ以外は形の崩れ（E002）
        None => {
            if text.split('+').count() > 2 {
                ModifierKind::TooManyModes
            } else {
                ModifierKind::MalformedMode
            }
        }
    }
}

/// 1 トークンから出る診断（§6.2.1〜§6.2.4）。範囲は**行内相対**。
///
/// | コード | 条件 |
/// |---|---|
/// | **E001** | `'` の後が辞書に一致しない（§6.2.1 の (c)、明示形で実体が無い場合を含む） |
/// | **E002** | 明示形が閉じていない（§6.2.3）／モード式の形が壊れている（§6.2.4） |
/// | **E005** | モード式が 3 つ以上（§6.4.4）。範囲は**トークン全体**（B-67） |
/// | **E026** | 修飾子を付けられない種別に修飾子（§6.1.1） |
/// | **W029** | より長い辞書項目にも一致しうる（§6.2.2、MUST） |
///
/// **E003**（未定義のモード ID）はモードの実体を見るので
/// [`crate::core::resolve::modes`] が出す。**W031**（本文中の資料参照、§6.6.4）は
/// ファイルの形式が要るので呼び出し側が出す。
pub fn token_diagnostics(line: &str, token: &RefToken, dictionary: &Dictionary) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // §6.2.3「閉じ括弧が無い場合は E002（行末までを未解決範囲とする）」
    if token.unclosed {
        out.push(Diagnostic::simple(
            DiagnosticCode::E002,
            Some(token.range.clone()),
        ));
        return out;
    }
    if token.resolved.is_none() {
        // §6.2.6: 赤波線（と「登録する名前」）は句読点までではなく**先頭 1 形態素**まで
        out.push(Diagnostic::new(
            DiagnosticCode::E001,
            Some(token.unresolved_range()),
            vec![Arg::text(token.morpheme(line))],
        ));
    }
    // §6.2.2「この挙動は作家を驚かせうるため、次の補助を MUST で付ける」
    if let Some(longer) = token.longer {
        let key = dictionary.entry(longer).map(|entry| entry.key.clone());
        out.push(Diagnostic::new(
            DiagnosticCode::W029,
            Some(token.key_range.clone()),
            vec![Arg::text(key.unwrap_or_default())],
        ));
    }
    if let Some(modifier) = &token.modifier {
        // **E005 だけトークン全体**に引く: 違反は「その参照に 3 つ以上のモードが付いている」
        // ことなので、波線の単位は `'リリア:A+B+C` の 1 トークン。B 形式の `mode:` を読む
        // `plot_episode::warn_modes`（M2）と同じ範囲にして、ファイルによって波線の長さが
        // 変わらないようにする（`docs/history.md` B-67）。
        // E002（形が壊れている）と E026（付けられない修飾子）は修飾子そのものの問題なので
        // 修飾子の範囲のまま。
        let found = match &modifier.kind {
            ModifierKind::MalformedMode => Some((DiagnosticCode::E002, modifier.range.clone())),
            ModifierKind::TooManyModes => Some((DiagnosticCode::E005, token.range.clone())),
            ModifierKind::NotAllowed => Some((DiagnosticCode::E026, modifier.range.clone())),
            ModifierKind::Modes(_) | ModifierKind::Title => None,
        };
        if let Some((code, range)) = found {
            out.push(Diagnostic::simple(code, Some(range)));
        }
    }
    out
}

// ============================================================ 話者注釈と relations

/// 話者注釈 `@名前` の解決（§6.2.5「行末（`@名前` が行の最後）→ 話者注釈」）。
///
/// 名前は M2 の [`Speaker`] が既に区切っているので、ここでは**完全一致**で引く（B-61）。
/// `Character` だけを対象にする（§6.1.1「`Address` は**話者注釈 `@` の解決対象外**（MUST）。
/// `@あおいちゃん` は E012 のままとする」）。
pub fn resolve_speaker(speaker: &Speaker<'_>, dictionary: &Dictionary) -> Option<EntryId> {
    dictionary.lookup(speaker.name, Some(&[EntryKind::Character]))
}

/// `@名前` の診断（**E012**「`登場人物 {0} は characters.md にいません`」）。
///
/// 範囲は話者注釈全体（[`Speaker::range`]、行内相対。§17.1-5「直前の空白も削除」と
/// 同じ範囲なので、エクスポートで消える部分と波線が一致する）。
pub fn speaker_diagnostic(speaker: &Speaker<'_>, dictionary: &Dictionary) -> Option<Diagnostic> {
    resolve_speaker(speaker, dictionary).map_or_else(
        || {
            Some(Diagnostic::new(
                DiagnosticCode::E012,
                Some(speaker.range.clone()),
                vec![Arg::text(speaker.name)],
            ))
        },
        |_| None,
    )
}

/// `## relations` の参照解決（§5.5.6、**E013**「`参照できません: {0}`」）。
///
/// 行は M2 の [`Relation`](crate::core::syntax::characters_md::Relation) が読む（`'参照: 説明`）。ここでは実体があるか
/// だけを見て、無ければ E013 を返す。範囲は**ファイル単位**（`characters.md`）。
pub fn relation_diagnostics(
    characters: &Characters<'_>,
    dictionary: &Dictionary,
) -> Vec<FileDiagnostic> {
    let starts = line_starts(&characters.lines);
    let mut out = Vec::new();
    for found in &characters.characters {
        let Some(section) = found.section(SectionKey::Relations) else {
            continue;
        };
        for relation in &section.relations {
            if dictionary
                .lookup(relation.name, Some(&[EntryKind::Character]))
                .is_some()
            {
                continue;
            }
            let base = starts.get(relation.line).copied().unwrap_or(0);
            out.push(FileDiagnostic {
                file: SourceFile::Characters,
                diagnostic: Diagnostic::new(
                    DiagnosticCode::E013,
                    Some(shift(&relation.range, base)),
                    vec![Arg::text(relation.name)],
                ),
            });
        }
    }
    out
}
