//! SceneId / PartialSceneId / RangeExpr（`SPECIFICATIONS.md` §5.1）。
//!
//! 番号は `num = /\d{1,4}/`（0〜9999）で、`A-B-C-D` の 4 階層（章・話・場面・節）。
//! `D` を省略した `A-B-C` は `section = 0` で「場面全体」を意味する（§5.1）。
//! 比較は `(chapter, episode, scene, section)` の辞書式順序（`Ord` の derive がそのまま一致する）。
//! `Serialize` は構文木のゴールデンテスト（`PLANS.md` §5.3 作業項目 5・§6.3）が JSON で
//! 比較できるように付ける。
//!
//! **予約番号 `00` はパースを拒否しない（§5.1.2 の MUST）**。`A = 00`（未配属ストック章）と
//! `B = 00`（章あらすじ）の**機能**は追加仕様 22 / 17 で ② だが、番号としての正当性は ① でも
//! 保たれていなければならない（拒否すると ② で既存データを救えない）。
//! したがってこのファイルは `00` に意味を与えない。`structure.allow_chapter_zero` の検査と
//! E020 は**形式ごとのパーサ**の仕事で、C 形式では `core::syntax::contents` が設定を
//! `Options` として受け取って出す（§5.1.2）。
//!
//! 範囲式（§5.1）:
//!
//! - `@01 .. @02` は**閉区間**。左辺の省略階層は最小値 0、右辺の省略階層は最大値 9999 に展開する。
//! - `@03-01 ..` は上端開放、`.. @01-03` は下端開放。
//! - カンマ区切りで複数区間の和集合。
//! - 区間の左端 > 右端 は **E016**（§7）。ここでは [`RangeError::Reversed`] を返し、
//!   診断コードへの変換は M4 の `core/diagnostics.rs` が担う（`PLANS.md` §9.2 B-17）。
//!
//! §12.2 により core はユーザー向け文言を持たないので、エラー型に `Display` は実装しない
//! （`Debug` のみ）。文言は `ui/i18n` のカタログが `(code, params)` から作る。

use serde::Serialize;

/// 1 階層の番号の最大値（§5.1 の `num = /\d{1,4}/`）。
pub const MAX_LEVEL: u16 = 9999;

/// 範囲式の区切り（§5.0.2 の RangeOp）。
pub const RANGE_OP: &str = "..";

/// 構造見出し・範囲式のシジル（§5.0.2 の StructureSigil）。
pub const SIGIL: char = '@';

/// `project.json.structure.*_digits`（§4.5）。
///
/// **core は設定ファイルを読まない**（§3.2）ので、桁数は呼び出し側が引数で渡す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Digits {
    /// 章（A）の表示桁数。
    pub chapter: u8,
    /// 話（B）の表示桁数。
    pub episode: u8,
    /// 場面（C）の表示桁数。
    pub scene: u8,
    /// 節（D）の表示桁数。
    pub section: u8,
}

impl Default for Digits {
    /// §4.5 の `project.json` 既定値（すべて 2 桁）。
    fn default() -> Self {
        Self {
            chapter: 2,
            episode: 2,
            scene: 2,
            section: 2,
        }
    }
}

/// 完全な節 ID（`A-B-C-D`）。
///
/// `section == 0` は「場面全体」を意味する（§5.1）。フィールドの宣言順がそのまま
/// 辞書式順序になるよう `Ord` を derive している。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize)]
pub struct SceneId {
    /// 章（A）。
    pub chapter: u16,
    /// 話（B）。
    pub episode: u16,
    /// 場面（C）。
    pub scene: u16,
    /// 節（D）。0 は「場面全体」。
    pub section: u16,
}

impl SceneId {
    /// 最小値（範囲式の下端開放を展開したときの値）。
    pub const MIN: Self = Self {
        chapter: 0,
        episode: 0,
        scene: 0,
        section: 0,
    };

    /// 最大値（範囲式の上端開放を展開したときの値）。
    pub const MAX: Self = Self {
        chapter: MAX_LEVEL,
        episode: MAX_LEVEL,
        scene: MAX_LEVEL,
        section: MAX_LEVEL,
    };

    /// 4 階層すべてが `MAX_LEVEL` 以内なら `Some`。
    pub fn new(chapter: u16, episode: u16, scene: u16, section: u16) -> Option<Self> {
        let levels = [chapter, episode, scene, section];
        if levels.iter().any(|&level| level > MAX_LEVEL) {
            return None;
        }
        Some(Self {
            chapter,
            episode,
            scene,
            section,
        })
    }

    /// `A-B-C` / `A-B-C-D` をパースする（**シジル `@` は含めない**）。
    ///
    /// 3 階層のときは `section = 0`（場面全体、§5.1）。2 階層以下は [`IdError::TooFewLevels`]。
    pub fn parse(text: &str) -> Result<Self, IdError> {
        let partial = PartialSceneId::parse(text)?;
        if partial.depth() < 3 {
            return Err(IdError::TooFewLevels);
        }
        Ok(partial.expand_low())
    }

    /// シジル付き（`@01-04-02-03`）をパースする。見出し行や範囲式の断片向け。
    pub fn parse_with_sigil(text: &str) -> Result<Self, IdError> {
        let rest = strip_sigil(text).ok_or(IdError::MissingSigil)?;
        Self::parse(rest)
    }

    /// 表示用の文字列（§5.1）。`section == 0` なら 3 階層、それ以外は 4 階層で、
    /// いずれも `digits` でゼロ詰めする。
    pub fn display(&self, digits: Digits) -> String {
        let mut out = pad(self.chapter, digits.chapter);
        push_level(&mut out, self.episode, digits.episode);
        push_level(&mut out, self.scene, digits.scene);
        if self.section != 0 {
            push_level(&mut out, self.section, digits.section);
        }
        out
    }

    /// 場面全体（`section == 0`）を指しているか。
    pub fn is_whole_scene(&self) -> bool {
        self.section == 0
    }

    /// 同じ ID を指す [`PartialSceneId`]（4 階層）。
    pub fn partial(&self) -> PartialSceneId {
        PartialSceneId {
            levels: [self.chapter, self.episode, self.scene, self.section],
            depth: 4,
        }
    }

    /// 場面（C）までを指す [`PartialSceneId`]（3 階層）。節をまとめて扱うときに使う。
    pub fn scene_partial(&self) -> PartialSceneId {
        PartialSceneId {
            levels: [self.chapter, self.episode, self.scene, 0],
            depth: 3,
        }
    }
}

/// 途中まで書かれた ID（§5.1 の `partial_id`）。1〜4 階層。
///
/// 書かれなかった階層は `levels` の中で 0 のまま残るが、**意味を持つのは `depth` まで**。
/// 範囲式の端として使うときは [`expand_low`]（省略 → 0）か [`expand_high`]（省略 → 9999）で
/// [`SceneId`] に展開する（§5.1「左辺の省略階層は最小値、右辺の省略階層は最大値」）。
///
/// [`expand_low`]: PartialSceneId::expand_low
/// [`expand_high`]: PartialSceneId::expand_high
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartialSceneId {
    levels: [u16; 4],
    depth: u8,
}

impl PartialSceneId {
    /// `A` / `A-B` / `A-B-C` / `A-B-C-D` をパースする（**シジル `@` は含めない**）。
    pub fn parse(text: &str) -> Result<Self, IdError> {
        if text.is_empty() {
            return Err(IdError::Empty);
        }
        let mut levels = [0; 4];
        let mut depth = 0;
        for part in text.split('-') {
            if depth == levels.len() {
                return Err(IdError::TooManyLevels);
            }
            levels[depth] = parse_number(part)?;
            depth += 1;
        }
        Ok(Self {
            levels,
            depth: depth as u8,
        })
    }

    /// シジル付き（`@01-04`）をパースする。
    pub fn parse_with_sigil(text: &str) -> Result<Self, IdError> {
        let rest = strip_sigil(text).ok_or(IdError::MissingSigil)?;
        Self::parse(rest)
    }

    /// 書かれた階層の数（1〜4）。
    pub fn depth(&self) -> usize {
        self.depth as usize
    }

    /// `idx` 番目（0 = 章）の番号。`depth` より後は `None`。
    pub fn level(&self, idx: usize) -> Option<u16> {
        if idx < self.depth() {
            Some(self.levels[idx])
        } else {
            None
        }
    }

    /// 章（A）。
    pub fn chapter(&self) -> u16 {
        self.levels[0]
    }

    /// 話（B）。書かれていなければ `None`。
    pub fn episode(&self) -> Option<u16> {
        self.level(1)
    }

    /// 場面（C）。書かれていなければ `None`。
    pub fn scene(&self) -> Option<u16> {
        self.level(2)
    }

    /// 節（D）。書かれていなければ `None`。
    pub fn section(&self) -> Option<u16> {
        self.level(3)
    }

    /// 省略された階層を **0** で埋めて [`SceneId`] にする（範囲の左端・単独指定の下端）。
    pub fn expand_low(&self) -> SceneId {
        SceneId {
            chapter: self.levels[0],
            episode: self.levels[1],
            scene: self.levels[2],
            section: self.levels[3],
        }
    }

    /// 省略された階層を **9999** で埋めて [`SceneId`] にする（範囲の右端）。
    pub fn expand_high(&self) -> SceneId {
        SceneId {
            chapter: self.levels[0],
            episode: fill_high(self.levels[1], self.depth, 1),
            scene: fill_high(self.levels[2], self.depth, 2),
            section: fill_high(self.levels[3], self.depth, 3),
        }
    }

    /// 表示用の文字列。書かれた階層だけを `digits` でゼロ詰めして `-` でつなぐ。
    pub fn display(&self, digits: Digits) -> String {
        let widths = [digits.chapter, digits.episode, digits.scene, digits.section];
        let depth = self.depth();
        let mut out = pad(self.levels[0], widths[0]);
        for (value, width) in self.levels[1..depth].iter().zip(&widths[1..depth]) {
            push_level(&mut out, *value, *width);
        }
        out
    }
}

/// 範囲式の 1 項。
///
/// `start` / `end` が `None` のときは**開放端**（`.. @x` / `@x ..`）。
/// `@x` のみの指定は `start == end == Some(x)` になり、[`interval`] で
/// 「x の下位すべて」に展開される。
///
/// [`interval`]: RangeTerm::interval
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeTerm {
    /// 下端。`None` なら下端開放。
    pub start: Option<PartialSceneId>,
    /// 上端。`None` なら上端開放。
    pub end: Option<PartialSceneId>,
}

impl RangeTerm {
    /// 省略階層と開放端を展開して閉区間にする。
    pub fn interval(&self) -> Interval {
        let start = match self.start {
            Some(partial) => partial.expand_low(),
            None => SceneId::MIN,
        };
        let end = match self.end {
            Some(partial) => partial.expand_high(),
            None => SceneId::MAX,
        };
        Interval { start, end }
    }

    /// 上端が開放されているか（`@03-01 ..`）。
    pub fn is_upper_open(&self) -> bool {
        self.end.is_none()
    }

    /// 下端が開放されているか（`.. @01-03`）。
    pub fn is_lower_open(&self) -> bool {
        self.start.is_none()
    }
}

/// 展開済みの閉区間（§5.1「`@01-04 .. @02-01` は閉区間」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    /// 下端（含む）。
    pub start: SceneId,
    /// 上端（含む）。
    pub end: SceneId,
}

impl Interval {
    /// `id` を含むか。
    pub fn contains(&self, id: SceneId) -> bool {
        self.start <= id && id <= self.end
    }

    /// 左端 > 右端 か。**E016**（§7）の条件。
    pub fn is_reversed(&self) -> bool {
        self.start > self.end
    }

    /// 別の区間と共通部分を持つか。
    pub fn intersects(&self, other: &Interval) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    /// 空区間か（`is_reversed` と同じ条件だが、意味が異なるので分けてある）。
    pub fn is_empty(&self) -> bool {
        self.is_reversed()
    }
}

/// カンマ区切りの和集合（§5.1 の `range_expr`）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RangeExpr {
    /// 項の列（書いた順を保つ）。
    pub terms: Vec<RangeTerm>,
}

impl RangeExpr {
    /// 範囲式をパースする（`@01 .. @02, @05`）。
    ///
    /// 空白は項の前後と `..` の前後だけを許す（§5.1 の `ws?`）。ID の中の空白は誤り。
    /// 空文字列・空の項は [`RangeError::Empty`]。
    pub fn parse(text: &str) -> Result<Self, RangeError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(RangeError::Empty);
        }
        let mut terms = Vec::new();
        for raw in trimmed.split(',') {
            terms.push(parse_term(raw.trim())?);
        }
        Ok(Self { terms })
    }

    /// 項の数。
    pub fn len(&self) -> usize {
        self.terms.len()
    }

    /// 項が無いか。
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// 全項を展開した閉区間。
    pub fn intervals(&self) -> Vec<Interval> {
        self.terms.iter().map(RangeTerm::interval).collect()
    }

    /// `id` をどれかの項が含むか。
    pub fn contains(&self, id: SceneId) -> bool {
        self.intervals().iter().any(|i| i.contains(id))
    }

    /// 別の範囲式と共通部分を持つか（**範囲の交差判定**、§3.3）。
    pub fn intersects(&self, other: &RangeExpr) -> bool {
        let mine = self.intervals();
        let theirs = other.intervals();
        mine.iter().any(|a| theirs.iter().any(|b| a.intersects(b)))
    }

    /// 境界検査（§5.1）。左端 > 右端 の項があれば **E016** 相当の誤りを返す。
    pub fn check(&self) -> Result<(), RangeError> {
        for (idx, interval) in self.intervals().iter().enumerate() {
            if interval.is_reversed() {
                return Err(RangeError::Reversed {
                    term: idx,
                    start: interval.start,
                    end: interval.end,
                });
            }
        }
        Ok(())
    }
}

/// ID のパース誤り。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdError {
    /// 空文字列。
    Empty,
    /// `-` で区切られた 1 要素が空（`01--02`、`01-` 等）。
    EmptyLevel,
    /// 数字以外を含む。
    NotANumber,
    /// 5 桁以上（§5.1 の `num = /\d{1,4}/`）。
    TooManyDigits,
    /// [`SceneId`] として使うのに 3 階層未満。
    TooFewLevels,
    /// 5 階層以上。
    TooManyLevels,
    /// シジル `@` が無い。
    MissingSigil,
}

/// 範囲式のパース誤り。`Reversed` が **E016**（§7）に対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeError {
    /// 式全体または項が空。
    Empty,
    /// ID 部分の誤り。
    Id(IdError),
    /// 項の先頭に `@` が無い。
    MissingSigil,
    /// 左端 > 右端（**E016**）。`term` は 0 始まりの項番号。
    Reversed {
        /// 項番号（0 始まり）。
        term: usize,
        /// 展開後の下端。
        start: SceneId,
        /// 展開後の上端。
        end: SceneId,
    },
}

/// `@` を取り除く。無ければ `None`。
fn strip_sigil(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    trimmed.strip_prefix(SIGIL)
}

/// `0..=9999` の 1 要素をパースする。
fn parse_number(text: &str) -> Result<u16, IdError> {
    if text.is_empty() {
        return Err(IdError::EmptyLevel);
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(IdError::NotANumber);
    }
    if text.len() > 4 {
        return Err(IdError::TooManyDigits);
    }
    // 4 桁以下の ASCII 数字なので u16 に収まる。
    text.parse::<u16>().map_err(|_| IdError::NotANumber)
}

/// 範囲式の 1 項をパースする。
fn parse_term(text: &str) -> Result<RangeTerm, RangeError> {
    if text.is_empty() {
        return Err(RangeError::Empty);
    }
    // 下端開放: `.. @01-03`
    if let Some(rest) = text.strip_prefix(RANGE_OP) {
        let end = parse_sigil_id(rest)?;
        return Ok(RangeTerm {
            start: None,
            end: Some(end),
        });
    }
    let (head, tail) = split_range_op(text);
    let start = parse_sigil_id(head)?;
    let end = match tail {
        // `@01-04`（区間演算子なし）→ 同じ ID の下端〜上端＝その階層の下位すべて
        None => Some(start),
        // `@03-01 ..`（上端開放）
        Some("") => None,
        Some(rest) => Some(parse_sigil_id(rest)?),
    };
    Ok(RangeTerm {
        start: Some(start),
        end,
    })
}

/// 最初の `..` で分割する。無ければ `(text, None)`。
fn split_range_op(text: &str) -> (&str, Option<&str>) {
    match text.find(RANGE_OP) {
        Some(idx) => (&text[..idx], Some(&text[idx + RANGE_OP.len()..])),
        None => (text, None),
    }
}

/// `@` 付きの ID をパースする（前後の空白は許す）。
fn parse_sigil_id(text: &str) -> Result<PartialSceneId, RangeError> {
    let rest = strip_sigil(text).ok_or(RangeError::MissingSigil)?;
    PartialSceneId::parse(rest.trim()).map_err(RangeError::Id)
}

/// `value` を `width` 桁にゼロ詰めする。`width` は 1〜4 にクランプする（§5.1 の桁数）。
fn pad(value: u16, width: u8) -> String {
    let width = usize::from(width.clamp(1, 4));
    format!("{:0w$}", value, w = width)
}

/// `out` に `-` とゼロ詰めした番号を足す。
fn push_level(out: &mut String, value: u16, width: u8) {
    out.push('-');
    out.push_str(&pad(value, width));
}

/// `depth` 未満の階層（＝書かれなかった階層）なら `MAX_LEVEL`、そうでなければ `value`。
fn fill_high(value: u16, depth: u8, idx: u8) -> u16 {
    if idx < depth {
        value
    } else {
        MAX_LEVEL
    }
}
