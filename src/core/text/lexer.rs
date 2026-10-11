//! §5.0 の共通字句規則。
//!
//! すべての novnow テキスト形式（`contents/*.txt` / `plots/*.txt` / `*.md`）に共通の字句を、
//! **行単位**で扱う（§5.0.1「複数行にまたがるトークンは存在しない。1 行は常に単独で字句解析
//! できる」）。これはインクリメンタル再解析（[`super::incremental`]）と LSP 化の前提。
//!
//! このファイルが担うのは次の 4 つ:
//!
//! 1. **行頭シジルの判定**（§5.0.2）。優先順位は [`SIGIL_TABLE`] の**配列順**で固定する
//!    （`PLANS.md` §7 の縫い代 4「表駆動」）。② のシジル（`# ` / `! `）は行を消すのではなく
//!    [`Stage::Two`] として残すので、② の実装はフラグを切り替えるだけでよい。
//! 2. **エスケープ**（§5.0.3）。有効なのは `\@ \' \; \/ \「 \」 \\` のみで、他は 2 文字のまま
//!    扱い**診断を出さない**。
//! 3. **フィールド行**（§5.0.5 の 7 段の手順）。全角 `：` は直後の空白を問わず受理し、
//!    半角 `:` は「空白・行末・§5.0.6 の表にあるキー」のときに受理する。表に無い ascii キーは
//!    空白が無ければフィールド行にしないので、**URL 判定の除外リストを持たない**
//!    （`http://` `doi:` `mailto:` `10:30` は自動的に本文のまま）。継続行は 2 スペース以上のインデント。
//! 4. **改行・BOM・末尾改行の正規化**（§5.0.4）。Unicode 正規化は**しない**
//!    （照合時のみ §6.1.3、M3 の仕事）。
//!
//! §5.0.2 の表のうち**行内に現れるシジル**（`'` 参照・`'{ }`・`:` 修飾子・`+` 併用・`「」` 会話・
//! `^` 引用注釈・`..` 範囲演算子）は、このファイルでは**行の種別を変えない**。
//! `'` と `「」` のトークン分割は M2（`core/syntax/*`）・M3（`core/resolve/*`）、
//! `..` は [`crate::core::id`]（M1 で実装済み）、`^` は ②（`PLANS.md` §1.2 の 10）。
//! 行頭の判定だけこのファイルで行い、行内の意味は各形式のパーサに委ねる。
//!
//! §12.2 により core はユーザー向け文言を持たない。ここにある日本語は**字句データ**
//! （エスケープ対象の `「」`）だけで、メッセージ文字列は無い。

use std::borrow::Cow;
use std::ops::Range;

use serde::{Deserialize, Serialize};

/// テキスト形式（§5.0.2 の「出現場所」列に対応する）。
///
/// シジルとフィールド行は形式によって受理範囲が違う（例: `;` は C / B 形式のみ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextFormat {
    /// `contents/AA-BB.txt`（C 形式、§5.2）。
    Contents,
    /// `plots/all.txt` / `plots/AA.txt`（A 形式、§5.3）。
    PlotOutline,
    /// `plots/AA-BB.txt`（B 形式、§5.4）。
    PlotEpisode,
    /// `*.md`（`characters.md` / `words.md` / `ideas.md` / `materials/**` / `prompts/*.md`）。
    Markdown,
}

impl TextFormat {
    /// `.txt`（A / B / C 形式）か。§5.0.8 の `# ` コメントは `.txt` のみで有効。
    ///
    /// §5.0.2 の「出現場所」は [`Formats`] の定数に 1 箇所だけ書き、ここからは引くだけにする
    /// （同じ仕様を 2 箇所に書くと片方だけ変わる）。
    pub fn is_txt(self) -> bool {
        Formats::TXT.contains(self)
    }

    /// フィールド行（§5.0.5）を書ける形式か。C 形式（本文）にはフィールド行が無い。
    pub fn allows_field(self) -> bool {
        Formats::FIELD_FORMATS.contains(self)
    }

    /// 節区切り `;`（§5.0.2）を書ける形式か。C 形式と B 形式のみ。
    pub fn allows_section_break(self) -> bool {
        Formats::CB.contains(self)
    }
}

/// シジルが出現し得る形式の集合（ビットマスク）。
///
/// `const` のテーブル（[`SIGIL_TABLE`]）の中で組み合わせられるよう、操作はすべて `const fn`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Formats(u8);

impl Formats {
    /// どの形式でも現れない（和集合の単位元）。
    pub const NONE: Self = Self(0b0000);
    /// C 形式。
    pub const CONTENTS: Self = Self(0b0001);
    /// A 形式。
    pub const PLOT_OUTLINE: Self = Self(0b0010);
    /// B 形式。
    pub const PLOT_EPISODE: Self = Self(0b0100);
    /// `*.md`。
    pub const MARKDOWN: Self = Self(0b1000);
    /// A / B（§5.0.6 の「使用形式」列で最も多い組み合わせ）。
    pub const AB: Self = Self(0b0110);
    /// C / B（§5.0.2 の `;` 節区切りの出現場所）。
    pub const CB: Self = Self(0b0101);
    /// `.txt` 3 形式（§5.0.8 の `# ` は `.md` では見出しなので含まない）。
    pub const TXT: Self = Self(0b0111);
    /// フィールド行を書ける形式（§5.0.2 の `key:` の出現場所「A/B 形式, `*.md`」）。
    pub const FIELD_FORMATS: Self = Self(0b1110);
    /// すべての形式。
    pub const ALL: Self = Self(0b1111);

    /// 和集合。
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `format` を含むか。
    pub const fn contains(self, format: TextFormat) -> bool {
        let bit = match format {
            TextFormat::Contents => Self::CONTENTS.0,
            TextFormat::PlotOutline => Self::PLOT_OUTLINE.0,
            TextFormat::PlotEpisode => Self::PLOT_EPISODE.0,
            TextFormat::Markdown => Self::MARKDOWN.0,
        };
        self.0 & bit != 0
    }
}

/// 実装段階（`PLANS.md` §1.2 の ① / ②）。
///
/// ② の機能を ① で**動かさない**ための印。テーブルの行は消さずに残すので、
/// ② の実装は `Two` を `One` に変えるだけで済む（縫い代 4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// v1 本体（①）で有効。
    One,
    /// v1 追加仕様（②）で有効になる。**① では通常テキストとして扱う**。
    Two,
}

/// 行頭シジルの種類（§5.0.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sigil {
    /// `@` 構造見出し（行末話者注釈の判定は M2 のパーサが行う）。
    Structure,
    /// `;` 節区切り。**行全体が `;` のとき、または行末コメントが続くときのみ**。
    SectionBreak,
    /// `//` 行コメント（以降行末まで）。
    LineComment,
    /// `# ` 行コメント（§5.0.8）。**②**。
    HashComment,
    /// `! ` 挿絵メモ（§5.2.5）。**②**。
    IllustrationMemo,
}

/// [`SIGIL_TABLE`] の 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigilSpec {
    /// 行頭と比較する文字列（`#` のように空白を含まない形。空白は `requires_space` で要求する）。
    pub prefix: &'static str,
    /// シジルの種類。
    pub sigil: Sigil,
    /// 直後に半角または全角の空白を要求するか（§5.0.2 MUST: `#1 位` や `!?` を誤認しない）。
    pub requires_space: bool,
    /// 行全体が `prefix` だけのときのみ一致させるか（`;` の節区切り）。行末コメント
    /// （`// …`）だけは許す（§5.2.1 `section_break = ws? , ";" , [ comment ] , eol`）。
    pub whole_line: bool,
    /// 受理する形式。
    pub formats: Formats,
    /// 実装段階。
    pub stage: Stage,
}

/// 行頭シジルの表（§5.0.2）。**配列の順がそのまま優先順位**で、固定。
///
/// `PLANS.md` §7 の縫い代 4「行頭シジルの表駆動: `const SIGIL_TABLE` に
/// `(prefix, kind, requires_space)` を並べ、lexer はこれを走査する」の実体。
/// ② で `# ` / `! ` を有効化するときも、字句器を書き直さずに `stage` を変えるだけにする。
pub const SIGIL_TABLE: &[SigilSpec] = &[
    SigilSpec {
        prefix: "@",
        sigil: Sigil::Structure,
        requires_space: false,
        whole_line: false,
        formats: Formats::ALL,
        stage: Stage::One,
    },
    SigilSpec {
        prefix: ";",
        sigil: Sigil::SectionBreak,
        requires_space: false,
        whole_line: true,
        formats: Formats::CB,
        stage: Stage::One,
    },
    SigilSpec {
        prefix: "#",
        sigil: Sigil::HashComment,
        requires_space: true,
        whole_line: false,
        formats: Formats::TXT,
        stage: Stage::Two,
    },
    SigilSpec {
        prefix: "!",
        sigil: Sigil::IllustrationMemo,
        requires_space: true,
        whole_line: false,
        formats: Formats::CONTENTS,
        stage: Stage::Two,
    },
    SigilSpec {
        prefix: "//",
        sigil: Sigil::LineComment,
        requires_space: false,
        whole_line: false,
        formats: Formats::ALL,
        stage: Stage::One,
    },
];

/// 継続行とみなす最小インデント（§5.0.5「2 スペース以上のインデント」）。
pub const CONTINUATION_INDENT: usize = 2;

/// 1 行の字句的な種別。
///
/// 範囲はすべて **byte offset**（§3.4「core は内部的に byte offset を使う」）ではなく
/// 行スライスからの借用で返す。呼び出し側は `&line[..]` で好きな位置を取り出せる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind<'a> {
    /// `@` で始まる行。中身は `@` を除いた残り（構造見出し §5.2.1 / 行末話者注釈の区別は
    /// 形式ごとのパーサが行う。C 形式は `crate::core::syntax::contents`）。
    Structure(&'a str),
    /// `;` の節区切り（§5.0.2）。**行全体が `;` のとき、または `;` の後ろが行コメント
    /// だけのとき**（§5.2.1 `section_break = ws? , ";" , [ comment ] , eol`）。
    SectionBreak,
    /// 行頭コメント。`text` はシジルの直後から行末まで（トリムしていない）。
    Comment {
        /// どのコメントシジルか。
        sigil: Sigil,
        /// コメント本体。
        text: &'a str,
    },
    /// フィールド行（§5.0.5）。
    Field(FieldLine<'a>),
    /// 継続行（2 スペース以上のインデント、§5.0.5）。
    ///
    /// 直前の行がフィールド行かどうかは**行単位では判定できない**ので、呼び出し側
    /// （M2 のパーサ）が文脈を見て決める。字句器は「インデントが 2 以上の行」を報告するだけ。
    Continuation {
        /// 先頭の空白文字数。
        indent: usize,
        /// インデントを除いた残り。
        text: &'a str,
    },
    /// `! ` 挿絵メモ（§5.2.5）。**②** なので [`Stage::One`] では作られない。
    IllustrationMemo(&'a str),
    /// 通常のテキスト行（本文）。
    Text(&'a str),
}

/// フィールド行の分解結果（§5.0.5）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLine<'a> {
    /// 先頭の空白文字数（`ws?`）。
    pub indent: usize,
    /// 表引きしたキー。**未知のキーは [`FieldKey::Unknown`]**（W011、値は改変せず保持）。
    pub key: FieldKey,
    /// **書かれたままの**キー（日本語別名や未知キーを保持するため。§5.0.6
    /// 「保存時に正準形へ正規化しない」）。
    pub key_text: &'a str,
    /// 値（前後の空白を除く）。`key:` だけの行は空文字列。
    pub value: &'a str,
}

/// §5.0.6 のフィールドキー（① の範囲）。
///
/// **② のキーは含めない**（`PLANS.md` §1.2）: `when`（3）/ `senses`（7）/ `space`・
/// `space-mode`（21）/ `illustration`（23）/ `blurb`（17）/ `address`（8）/ `icon`（24）。
/// ① ではそれらは [`FieldKey::Unknown`] として**値を改変せず保持する**（縫い代 1・2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKey {
    /// `summary`
    Summary,
    /// `goal`
    Goal,
    /// `turn`
    Turn,
    /// `state`
    State,
    /// `cast`
    Cast,
    /// `mode`
    Mode,
    /// `pov`
    Pov,
    /// `seed`
    Seed,
    /// `payoff`
    Payoff,
    /// `material`
    Material,
    /// `length`
    Length,
    /// `check`
    Check,
    /// `note`
    Note,
    /// `alias`
    Alias,
    /// `range`
    Range,
    /// `reading`
    Reading,
    /// `definition`
    Definition,
    /// `category`
    Category,
    /// `tone`
    Tone,
    /// `personality`
    Personality,
    /// `emotion`
    Emotion,
    /// `role`
    Role,
    /// `relation`
    Relation,
    /// `default-mode`
    DefaultMode,
    /// `tags`
    Tags,
    /// `source`
    Source,
    /// `condition`
    Condition,
    /// `context`
    Context,
    /// §5.0.6 の表に無いキー（**W011**。値は改変せず保持する）。
    Unknown,
}

impl FieldKey {
    /// §5.0.6 の表の行。[`FieldKey::Unknown`] なら `None`。
    pub fn row(self) -> Option<&'static KeyRow> {
        FIELD_KEYS.iter().find(|row| row.0 == self)
    }

    /// 正準形（`ascii_key`）。[`FieldKey::Unknown`] なら `None`。
    pub fn ascii_key(self) -> Option<&'static str> {
        self.row().map(|row| row.1)
    }

    /// 日本語別名。[`FieldKey::Unknown`] なら `None`。
    pub fn alias(self) -> Option<&'static str> {
        self.row().map(|row| row.2)
    }

    /// §5.0.6 の「使用形式」。**字句解析では使わない**（形式ごとの妥当性は M2 のパーサが見る）。
    pub fn formats(self) -> Option<Formats> {
        self.row().map(|row| row.3)
    }
}

/// [`FIELD_KEYS`] の 1 行 = `(キー, 正準形の ascii_key, 日本語別名, 使用形式)`。
pub type KeyRow = (FieldKey, &'static str, &'static str, Formats);

/// §5.0.6 のフィールドキー別名表（固定・双方向 1:1）。① の 28 件。
///
/// i18n: format-vocabulary
///
/// **日本語別名はユーザー向け文言ではない**: UI 言語が `en` でも受理する（§12 の明文）ので
/// ロケール表（`ui/i18n/`）には置けず、ファイル形式の語彙として core が持つ。§12.2 が禁じる
/// のはメッセージであり、`tests/layering.rs` はこの doc 行を付けた `const` の中だけ
/// 日本語リテラルを免除する（`PLANS.md` §9.2 B-19）。
///
/// **② のキーは含めない**（`PLANS.md` §1.2）: `when`（3）/ `senses`（7）/ `space`・
/// `space-mode`（21）/ `illustration`（23）/ `blurb`（17）/ `address`（8）/ `icon`（24）。
/// ① では表に無いので [`FieldKey::Unknown`]（W011）になり、値は改変せず保持する。
/// 保存時に正準形へは**正規化しない**（§5.0.6「作家の表記を保つ」）。
pub const FIELD_KEYS: &[KeyRow] = &[
    (FieldKey::Summary, "summary", "要約", Formats::AB),
    (FieldKey::Goal, "goal", "目的", Formats::AB),
    (FieldKey::Turn, "turn", "転換", Formats::PLOT_OUTLINE),
    (FieldKey::State, "state", "状態", Formats::AB),
    (FieldKey::Cast, "cast", "配役", Formats::AB),
    (FieldKey::Mode, "mode", "モード", Formats::PLOT_EPISODE),
    (FieldKey::Pov, "pov", "視点", Formats::AB),
    (FieldKey::Seed, "seed", "伏線", Formats::AB),
    (FieldKey::Payoff, "payoff", "回収", Formats::AB),
    (FieldKey::Material, "material", "資料", Formats::AB),
    (FieldKey::Length, "length", "分量", Formats::AB),
    (FieldKey::Check, "check", "確認", Formats::PLOT_EPISODE),
    (FieldKey::Note, "note", "メモ", Formats::FIELD_FORMATS),
    (FieldKey::Alias, "alias", "別名", Formats::MARKDOWN),
    (FieldKey::Range, "range", "範囲", Formats::MARKDOWN),
    (FieldKey::Reading, "reading", "読み", Formats::MARKDOWN),
    (
        FieldKey::Definition,
        "definition",
        "定義",
        Formats::MARKDOWN,
    ),
    (FieldKey::Category, "category", "分類", Formats::MARKDOWN),
    (FieldKey::Tone, "tone", "口調", Formats::MARKDOWN),
    (
        FieldKey::Personality,
        "personality",
        "性格",
        Formats::MARKDOWN,
    ),
    (FieldKey::Emotion, "emotion", "感情", Formats::MARKDOWN),
    (FieldKey::Role, "role", "役割", Formats::MARKDOWN),
    (FieldKey::Relation, "relation", "関係", Formats::MARKDOWN),
    (
        FieldKey::DefaultMode,
        "default-mode",
        "既定モード",
        Formats::MARKDOWN,
    ),
    (FieldKey::Tags, "tags", "タグ", Formats::MARKDOWN),
    (FieldKey::Source, "source", "出典", Formats::MARKDOWN),
    (FieldKey::Condition, "condition", "条件", Formats::MARKDOWN),
    (FieldKey::Context, "context", "文脈", Formats::MARKDOWN), // §5.0.6: prompts/*.md
];

/// 1 行を字句解析する（§5.0.1「1 行は常に単独で字句解析できる」）。
///
/// ① の範囲（[`Stage::One`]）。② のシジル（`# ` / `! `）は通常テキストとして扱う。
pub fn classify(line: &str, format: TextFormat) -> LineKind<'_> {
    classify_at_stage(line, format, Stage::One)
}

/// `stage` の範囲で 1 行を字句解析する。
///
/// 判定順は §5.0.2 の固定順: `@` → `;` → `# ` → `! ` → `//` → フィールド行 → 継続行 →
/// 通常テキスト。順そのものは [`SIGIL_TABLE`] の配列順で、字句器には書いていない
/// （`PLANS.md` §7 の縫い代 4）。
///
/// [`Stage::Two`] を渡すと ② の行（`PLANS.md` §1.2 の 15・23）も有効になる。v1 本体は
/// [`Stage::One`] だけを呼び、② の実装は呼び出し側のフラグを切り替えるだけで済む。
pub fn classify_at_stage(line: &str, format: TextFormat, stage: Stage) -> LineKind<'_> {
    for spec in SIGIL_TABLE {
        if stage == Stage::One && spec.stage == Stage::Two {
            continue;
        }
        if !spec.formats.contains(format) {
            continue;
        }
        let Some(rest) = match_sigil(line, spec) else {
            continue;
        };
        return match spec.sigil {
            Sigil::Structure => LineKind::Structure(rest),
            Sigil::SectionBreak => LineKind::SectionBreak,
            Sigil::IllustrationMemo => LineKind::IllustrationMemo(rest),
            Sigil::LineComment | Sigil::HashComment => LineKind::Comment {
                sigil: spec.sigil,
                text: rest,
            },
        };
    }

    if format.allows_field() {
        if let Some(field) = parse_field_line(line) {
            return LineKind::Field(field);
        }
    }

    let indent = leading_indent(line);
    if indent >= CONTINUATION_INDENT {
        return LineKind::Continuation {
            indent,
            text: after_indent(line),
        };
    }

    LineKind::Text(line)
}

/// シジルが行頭に一致するかを調べ、一致したらシジルの直後を返す。
///
/// - **行頭の前に 0〜1 文字の空白を許す**（§5.2.1 の `scene_header` / `section_break` /
///   `comment_line` / `illust_line` / `blank` はいずれも `ws?` で始まり、§5.0.5 の
///   フィールド行も 0〜1 文字）。2 文字以上は継続行のために予約されているので
///   シジルを見ない（`PLANS.md` §9.2 B-18・B-31）。
/// - `whole_line`（`;`）は、残りが空か **行コメントだけ**のときに一致する
///   （§5.2.1 `section_break = ws? , ";" , [ comment ] , eol`。§5.0.2 の表の
///   「行全体が `;` のときのみ」はこの読み方で、`PLANS.md` §9.1 A-9 に記録した）。
fn match_sigil<'a>(line: &'a str, spec: &SigilSpec) -> Option<&'a str> {
    if leading_indent(line) >= CONTINUATION_INDENT {
        return None;
    }
    let rest = after_indent(line).strip_prefix(spec.prefix)?;
    if spec.requires_space {
        // §5.0.2 MUST: `# ` と `! ` は直後に半角または全角の空白が必要
        return rest.strip_prefix(is_space);
    }
    if spec.whole_line {
        let body = strip_line_comment(rest);
        if !body.trim_matches(is_space).is_empty() {
            return None;
        }
    }
    Some(rest)
}

/// 半角空白・タブ・全角空白（U+3000）か。
///
/// §5.0.4「行末の半角/全角空白は保存時に削除」と §5.0.2「`# ` と `! ` は直後に半角または
/// 全角の空白が必要」の両方で使う**同一の定義**。他の Unicode 空白（U+00A0 等）は
/// 本文の一部として扱う（作家の入力を変えない、§5.0.4）。
pub fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{3000}')
}

/// 行頭の空白文字数。
pub fn leading_indent(line: &str) -> usize {
    line.chars().take_while(|&c| is_space(c)).count()
}

/// 行頭の空白を除いた残り。
pub fn after_indent(line: &str) -> &str {
    line.trim_start_matches(is_space)
}

/// フィールド行をパースする（§5.0.5 の 7 段の手順）。フィールド行でなければ `None`。
fn parse_field_line(line: &str) -> Option<FieldLine<'_>> {
    // 手順 1: 2 文字以上のインデントは継続行のために予約されている（`PLANS.md` §9.2 B-18）
    let indent = leading_indent(line);
    if indent >= CONTINUATION_INDENT {
        return None;
    }
    // 手順 2・3: 最初のコロンで切り、キー候補が `field_key` かを見る
    let (key_text, colon, rest) = split_field_key(after_indent(line))?;
    let key = lookup_key(key_text);
    // 手順 4: 全角 `：` は直後の空白を問わない（`要約：あらすじ` を受ける）
    // 手順 5: 半角 `:` は「空白・行末・表にあるキー」のときだけ
    // 手順 6: 行末は空の値として受理する（§5.0.4 の行末空白削除と両立）
    if colon == ':' {
        let space_after = rest.is_empty() || rest.starts_with(is_space);
        if !space_after && key == FieldKey::Unknown {
            return None;
        }
    }
    Some(FieldLine {
        indent,
        key,
        key_text,
        value: rest.trim_matches(is_space),
    })
}

/// 最初のコロン（半角 `:` / 全角 `：`）で切り、`(キー候補, コロン, コロン以降)` を返す。
///
/// §5.0.5 の手順 2・3。コロンが複数あれば**最初のもので切る**ので、値にコロンが残ってよい
/// （`source:https://example.com`）。キー候補が `field_key` でなければ `None`。
fn split_field_key(body: &str) -> Option<(&str, char, &str)> {
    let idx = body.find([':', '：'])?;
    let key_text = body[..idx].trim_end_matches(is_space);
    if !is_key_text(key_text) {
        return None;
    }
    let colon = body[idx..].chars().next()?;
    Some((key_text, colon, &body[idx + colon.len_utf8()..]))
}

/// キーとして受理できる文字列か（§5.0.5 手順 3、`field_key = ascii_key | ja_alias`）。
///
/// 表に無い ascii キー（`foo` 等）は**受理する**（[`FieldKey::Unknown`] → W011。ただし半角
/// コロンで空白が無いときは手順 5 で落ちる）。表に無い日本語（`注：これは本文` のような散文）は
/// **受理しない**。
pub fn is_key_text(text: &str) -> bool {
    if text.starts_with(|c: char| c.is_ascii_lowercase()) {
        return is_ascii_key(text);
    }
    lookup_key(text) != FieldKey::Unknown
}

/// §5.0.5 の `ascii_key = /[a-z][a-z0-9-]{0,23}/` に一致するか。
pub fn is_ascii_key(text: &str) -> bool {
    let bytes = text.as_bytes();
    // 1 文字目 + 最大 23 文字
    if bytes.is_empty() || bytes.len() > 24 {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes[1..].iter().copied().all(is_key_char)
}

/// `ascii_key` の 2 文字目以降に使える文字（`[a-z0-9-]`）。
fn is_key_char(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'
}

/// キー文字列（正準形または日本語別名）を [`FieldKey`] に引き当てる。
///
/// 表に無ければ [`FieldKey::Unknown`]（W011）。値は呼び出し側が改変せずに保持する。
pub fn lookup_key(text: &str) -> FieldKey {
    for (field, key, alias, _) in FIELD_KEYS {
        if *key == text || *alias == text {
            return *field;
        }
    }
    FieldKey::Unknown
}

/// §5.0.3 の有効なエスケープ（`\` の直後に来られる文字）。
///
/// これ以外の `\x` は `\x` の 2 文字としてそのまま扱い、**診断を出さない**（§5.0.3）。
pub const ESCAPE_TARGETS: &[char] = &['@', '\'', ';', '/', '「', '」', '\\'];

/// `\` の出現 1 件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Escape {
    /// `\` の byte offset。
    pub start: usize,
    /// `\` と直後の文字を合わせた byte 長（行末の `\` だけは 1）。
    pub len: usize,
    /// `\` の直後の文字。行末で直後の文字が無ければ `None`。
    pub target: Option<char>,
    /// [`ESCAPE_TARGETS`] にあるか。`false` なら 2 文字のまま扱う。
    pub valid: bool,
}

/// 行内の `\` をすべて探す（有効・無効の両方）。
///
/// `\\` は 1 件のエスケープとして消費するので、`\\\\` は 2 件になる。
pub fn find_escapes(line: &str) -> Vec<Escape> {
    let mut out = Vec::new();
    let mut chars = line.char_indices();
    while let Some((start, ch)) = chars.next() {
        if ch != '\\' {
            continue;
        }
        match chars.next() {
            Some((target_start, target)) => {
                let len = target_start + target.len_utf8() - start;
                out.push(Escape {
                    start,
                    len,
                    target: Some(target),
                    valid: ESCAPE_TARGETS.contains(&target),
                });
            }
            // 行末の `\` はエスケープ対象が無いので 1 文字のまま
            None => out.push(Escape {
                start,
                len: 1,
                target: None,
                valid: false,
            }),
        }
    }
    out
}

/// 有効なエスケープだけを解除する（§5.0.3 / §17.1 のエクスポートで使う）。
///
/// 無効な `\x` は 2 文字のまま残す。解除するものが無ければ借用を返す（確保しない）。
pub fn unescape(line: &str) -> Cow<'_, str> {
    let escapes = find_escapes(line);
    if !escapes.iter().any(|e| e.valid) {
        return Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len());
    let mut cursor = 0;
    for escape in escapes {
        out.push_str(&line[cursor..escape.start]);
        match escape.target {
            Some(target) if escape.valid => out.push(target),
            _ => out.push_str(&line[escape.start..escape.start + escape.len]),
        }
        cursor = escape.start + escape.len;
    }
    out.push_str(&line[cursor..]);
    Cow::Owned(out)
}

/// 未エスケープの `//` の byte offset（§5.0.2「行内: 以降行末までコメント」）。
///
/// 次の 2 つはコメントの始めにしない。
///
/// - `\/` はエスケープ（§5.0.3）。**有効でない** `\x` も 2 文字を消費するので、
///   `\/` の後の `/` もコメントの始めにならない。
/// - **直前が `:` の `//`**（§5.0.2「`://` は行内コメントにしない」MUST）。URL の scheme
///   区切りなので、`https://example.com` や `source:https://…` の値を壊さない。
pub fn find_line_comment(line: &str) -> Option<usize> {
    let mut chars = line.char_indices();
    while let Some((idx, ch)) = chars.next() {
        if ch == '\\' {
            chars.next();
            continue;
        }
        if ch != '/' || !line[idx + 1..].starts_with('/') {
            continue;
        }
        if idx > 0 && line[..idx].ends_with(':') {
            continue;
        }
        return Some(idx);
    }
    None
}

/// 未エスケープの `//` より前を返す（§5.0.2「以降行末までコメント」）。
///
/// `//` が行頭にあれば空文字列になる。[`classify`] は**行頭**のシジルだけを判定するので、
/// 行内に現れた `//` の除去は形式ごとのパーサ（M2）がこの関数で行う。
pub fn strip_line_comment(line: &str) -> &str {
    match find_line_comment(line) {
        Some(idx) => &line[..idx],
        None => line,
    }
}

/// 行内コメント `/* ... */` の byte 範囲（§5.0.2 InlineComment）。
///
/// **単一行内で閉じること**が仕様なので、`*/` が無ければ `None`（閉じない `/*` の診断は
/// M4 の §7 が決める）。入れ子の `/*` は扱わない（仕様に無い）。
pub fn find_inline_comment(line: &str) -> Option<Range<usize>> {
    let mut chars = line.char_indices();
    while let Some((idx, ch)) = chars.next() {
        if ch == '\\' {
            chars.next();
            continue;
        }
        if ch != '/' || !line[idx + 1..].starts_with('*') {
            continue;
        }
        let rest_start = idx + 2;
        let close = line[rest_start..].find("*/")?;
        return Some(idx..rest_start + close + 2);
    }
    None
}

/// UTF-8 BOM（§5.0.4「BOM があれば剥がして読み、保存時は付けない」MUST）。
pub const BOM: &str = "\u{feff}";

/// `project.json.newline`（§4.5）。既定は `lf`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Newline {
    /// LF（既定）。
    #[default]
    Lf,
    /// CRLF。
    Crlf,
    /// CR。
    Cr,
}

impl Newline {
    /// 実際の改行文字列。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
            Self::Cr => "\r",
        }
    }
}

/// BOM を剥がす。無ければそのまま返す。
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix(BOM).unwrap_or(text)
}

/// 読み込み時の正規化（§5.0.4）: BOM を剥がし、CRLF / CR を LF にする。
///
/// Unicode 正規化は**しない**（「作家の入力を変えないため」。NFKC 相当の比較は
/// 辞書照合時のみで M3 の §6.1.3）。CR を含むときだけ確保する。
pub fn normalize_on_load(text: &str) -> Cow<'_, str> {
    let text = strip_bom(text);
    if !text.contains('\r') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            out.push('\n');
            // CRLF は 1 つの改行として扱う
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
        } else {
            out.push(ch);
        }
    }
    Cow::Owned(out)
}

/// 保存時の正規化（§5.0.4）:
///
/// - 行末の半角/全角空白を削除する（MUST）。**行頭のインデントは保持する**。
/// - ファイル末尾を改行 1 個で終える（MUST）。
/// - 改行を `newline` の形式に揃える。BOM は付けない。
/// - Unicode 正規化はしない。
///
/// 空の文書は空のまま返す。改行 1 個を強制すると「空の 1 行」が増え、
/// 読み込み → 保存の往復で文書が変わってしまうため。
pub fn normalize_for_save(text: &str, newline: Newline) -> String {
    let normalized = normalize_on_load(text);
    let mut lines: Vec<&str> = normalized.split('\n').collect();
    // 末尾改行によって生まれる空要素を除く（末尾改行の有無に結果を左右させない）
    while lines.last() == Some(&"") {
        lines.pop();
    }
    let mut out = String::with_capacity(normalized.len());
    for line in lines {
        out.push_str(line.trim_end_matches(is_space));
        out.push_str(newline.as_str());
    }
    out
}
