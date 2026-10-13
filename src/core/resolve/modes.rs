//! モード解決（§6.4、指示 6 の中核）。
//!
//! # 8 段の解決順序（§6.4.3、固定）
//!
//! **上から順に試して、最初に見つかったものを採用する**。段ごとに「対象のキャラのモードが
//! 1 つ以上書かれている」ものだけを見つけたと数える（`mode: 'リリア` のように名前だけで
//! モードを書かなかった段は**見つけていない**扱いで次の段へ進む。`PLANS.md` §9.2 B-65）。
//!
//! | 段 | 読む場所 | キー |
//! |---|---|---|
//! | 1 | 本文（C 形式）の当該行の話者注釈 `@リリア:C` — **その行のみ** | §5.2.1 `speaker_annot` |
//! | 2 | 話プロット（B 形式）の**当該節**のフィールド | `mode:` |
//! | 3 | 話プロットの**当該場面**（見出し直下）のフィールド | `mode:` |
//! | 4 | 話プロットの preamble（どの見出しより前）のフィールド | `mode:` |
//! | 5 | 章プロット（`plots/AA.txt`）の**当該話ブロック** | `cast:` |
//! | 6 | 全体プロット（`plots/all.txt`）の**当該章ブロック** | `cast:` |
//! | 7 | `characters.md` のキャラ直下 | `default-mode:` |
//! | 8 | どれも無ければ**モード未決定** → **W006** | — |
//!
//! §6.4.2「モードの割り当ては**話プロット（B 形式）の `mode:` フィールドで行う**。本文側の
//! 話者注釈 `@名前:B` は『その台詞だけの一時上書き』」なので、段 1 が最優先で、しかも
//! **その行だけ**に効く（呼び出し側が行ごとに解決する）。
//!
//! # 制約と診断（§6.4.4 / §5.4.2）
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **E003** | そのキャラに定義の無いモード ID | 割り当てを書いた参照トークン |
//! | **E004** | 定義の `range` が対象 SceneId を含まない | 同上 |
//! | **W006** | 8 段のどれもモードを持たない | 話プロットの当該節（無ければ範囲なし） |
//!
//! **E005（1 キャラ 1 節に 3 つ以上のモード／同じキャラを 2 回書いた）は「その形を初めて
//! 読んだ層」だけが出す**（`PLANS.md` §9.2 B-67）。2 層が重ねると 1 つのトークンに 2 本の
//! 波線が引かれるため、担当を分ける。
//!
//! | 書かれた場所 | E005 の担当 |
//! |---|---|
//! | B 形式の `mode:`（3 つ以上・同じキャラを 2 回の両方） | [`plot_episode::parse`](crate::core::syntax::plot_episode::parse)（M2） |
//! | 1 つの `'名前:A+B+C` トークン（`cast:` の値・本文中の参照） | [`dictionary::token_diagnostics`](crate::core::resolve::dictionary::token_diagnostics) |
//! | `cast:` に同じキャラを 2 回／`default-mode:` に 3 つ以上 | **この層**（フィールド全体を読む層が他に無い） |
//!
//! どの段でも §7 のクイックフィックス（「先頭 2 つを残す」）の**効果は適用する**ので、
//! 3 つ以上書かれていても解決は止まらない（§4.3-4「パース失敗を起動失敗にしない」）。
//! 話者注釈（段 1）では E005 は起きない: §5.2.1 の `mode_expr` は最大 2 つなので、
//! [`contents::split_modes`](crate::core::syntax::contents::split_modes) が 3 つ以上を
//! 話者注釈として認識しない（W002 になる）ため。
//!
//! **E017**（同一モード ID の range 重複）は定義側の検査で、M2 の
//! [`characters_md::parse`](crate::core::syntax::characters_md::parse) が出す。§6.4.5
//! 「パース時に検出するので実行時は起きない」どおり、ここでは**出さない**。万一重なって
//! いても先頭の定義を選ぶので結果は決定的。
//!
//! # 変種の選択（§6.4.5）
//!
//! 同一 `mode_id` で `range` が違う定義（§5.5.7 の `### C` が 2 回現れる例）から、
//! **対象 SceneId を含む `range` を持つ定義**を選ぶ。`range:` が書かれていなければ全範囲。
//! `range:` の形が壊れている（M2 が E002 / E016 を出している）ときは**全範囲扱い**にして
//! E004 を重ねない（B-66）。
//!
//! モード ID の比較は **ASCII の大文字小文字を区別しない**（§6.1.3 が名前を小文字化して
//! 比べるのと同じ流儀。`:b` を `### B` に当てられる。B-64）。

use std::ops::Range;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::id::{Digits, RangeExpr, RangeTerm, SceneId};
use crate::core::model::mode::Mode;
use crate::core::resolve::dictionary::{Dictionary, EntryKind};
use crate::core::syntax::characters_md::{Characters, ModeDef, SectionKey};
use crate::core::syntax::contents::{self, Speaker};
use crate::core::syntax::plot_episode::EpisodePlot;
use crate::core::syntax::plot_outline::Outline;
use crate::core::text::lexer::FieldKey;

/// §6.4.3 の 8 段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// 1: 本文の当該行の話者注釈の修飾子（`@リリア:C`）。**その行のみ**。
    SpeakerLine,
    /// 2: 話プロットの当該**節**の `mode:`。
    SectionMode,
    /// 3: 話プロットの当該**場面**（見出し直下）の `mode:`。
    SceneMode,
    /// 4: 話プロットの preamble の `mode:`。
    EpisodePreamble,
    /// 5: 章プロット（`plots/AA.txt`）の当該話ブロックの `cast:`。
    ChapterCast,
    /// 6: 全体プロット（`plots/all.txt`）の当該章ブロックの `cast:`。
    AllCast,
    /// 7: `characters.md` の `default-mode`。
    DefaultMode,
    /// 8: モード未決定（**W006**）。
    Undetermined,
}

impl Step {
    /// §6.4.3 の段番号（1〜8）。
    pub fn order(self) -> u8 {
        match self {
            Self::SpeakerLine => 1,
            Self::SectionMode => 2,
            Self::SceneMode => 3,
            Self::EpisodePreamble => 4,
            Self::ChapterCast => 5,
            Self::AllCast => 6,
            Self::DefaultMode => 7,
            Self::Undetermined => 8,
        }
    }

    /// モードが決まらなかったか（§6.4.3 の段 8）。
    pub fn is_undetermined(self) -> bool {
        self == Self::Undetermined
    }
}

/// 解決の入力になるファイル（診断をどの文書に載せるか決めるのに使う）。
///
/// パスそのものは持たない（`contents/01-04.txt` 等は §4.2 のレイアウトと
/// `project.json.structure.id_digits` から呼び出し側が作る。core は I/O をしない、§3.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum File {
    /// 本文（C 形式、§5.2）。
    Contents,
    /// 話プロット（B 形式、`plots/AA-BB.txt`、§5.4）。
    Episode,
    /// 章プロット（A 形式、`plots/AA.txt`、§5.3）。
    Chapter,
    /// 全体プロット（A 形式、`plots/all.txt`、§5.3）。
    All,
    /// `characters.md`（§5.5）。
    Characters,
}

/// どのファイルの診断か。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiagnostic {
    /// ファイル。
    pub file: File,
    /// 診断。`range` は**そのファイル内の byte 範囲**。
    pub diagnostic: Diagnostic,
}

/// 解決の入力（§6.4.3 の 8 段が読む木）。
///
/// 話プロット・章プロット・全体プロットは**開いていない／存在しないことがある**ので
/// `Option`（無い段は飛ばして次へ進む）。
#[derive(Debug)]
pub struct Input<'a> {
    /// `characters.md`（モードの定義元、§6.4.1）。
    pub characters: &'a Characters<'a>,
    /// `plots/AA-BB.txt`（段 2〜4）。
    pub episode: Option<&'a EpisodePlot<'a>>,
    /// `plots/AA.txt`（段 5）。
    pub chapter: Option<&'a Outline<'a>>,
    /// `plots/all.txt`（段 6）。
    pub all: Option<&'a Outline<'a>>,
    /// `'名前` を解決する辞書（段 1〜6 で参照の実体を見る）。
    pub dictionary: &'a Dictionary,
}

/// 解決の対象。
#[derive(Debug, Clone, Copy)]
pub struct Target<'a> {
    /// 対象の節（§6.4.4 の range 検査と §6.4.5 の変種選択に使う）。
    pub scene: SceneId,
    /// 対象のキャラ（[`Characters::characters`] の添字）。
    ///
    /// 名前から引くのは呼び出し側の仕事（[`Dictionary::lookup`] ＋
    /// [`crate::core::resolve::dictionary::Target::Character`] の `index`）。
    pub character: usize,
    /// 本文の当該行の話者注釈（段 1）。本文以外から解決するときは `None`。
    pub speaker: Option<&'a Speaker<'a>>,
    /// `speaker` の行の先頭（**ファイル内 byte 位置**）。段 1 の診断をファイル単位にする。
    pub line_start: usize,
}

/// 1 つのモード定義（§6.4.1「`(mode_id, range)` の組で一意」）。
///
/// 中身は M2 の木のノードへの参照なので、`personality` / `emotion` / `role` / 役割語パターン
/// （§6.4.6 の参照ペイン表示）は [`ModeDef`] 経由でそのまま読める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Definition<'a> {
    /// `characters.md` のキャラ（[`Characters::characters`] の添字）。
    pub character: usize,
    /// `### <mode_id> <表示名>` のノード。
    pub node: &'a ModeDef<'a>,
}

impl Definition<'_> {
    /// `mode_id`。
    pub fn id(&self) -> &str {
        self.node.id
    }

    /// 表示名（§5.5.2 `mode_label`）。無ければ空。
    pub fn label(&self) -> &str {
        self.node.label
    }

    /// 有効範囲（§5.1）。`range:` が書かれていなければ**全範囲**。
    ///
    /// 形が壊れているときも全範囲（B-66。E002 / E016 は M2 が定義側で出している）。
    pub fn range_expr(&self) -> RangeExpr {
        let written = match self.node.range.as_ref() {
            Some(found) if found.valid => found.text,
            // 書かれていない・形が壊れている → 全範囲
            _ => return everything(),
        };
        RangeExpr::parse(written).unwrap_or_else(|_| everything())
    }

    /// この定義が対象の節で使えるか（§6.4.4 / §6.4.5）。
    pub fn contains(&self, scene: SceneId) -> bool {
        self.range_expr().contains(scene)
    }

    /// §6.4.4 の **E004** の引数 `{3}`（有効範囲）に書く文字列。
    ///
    /// 書かれたままの `range:` の値。`range:` が無ければ全範囲なので E004 は出ない。
    pub fn range_text(&self) -> &str {
        self.node
            .range
            .as_ref()
            .map(|found| found.text)
            .unwrap_or("")
    }
}

/// 全範囲の範囲式（§5.5.4「`range` 省略時は全範囲」）。
fn everything() -> RangeExpr {
    RangeExpr {
        terms: vec![RangeTerm {
            start: None,
            end: None,
        }],
    }
}

/// 解決の結果（§6.4.3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment<'a> {
    /// 何段目で決まったか。[`Step::Undetermined`] ならモード無し（**W006**）。
    pub step: Step,
    /// 採用されたモード ID（**書かれた順**。3 つ以上あったときは §7 のクイックフィックス
    /// 「先頭 2 つを残す」で切り詰めたあと）。
    pub ids: Vec<&'a str>,
    /// 実体（§6.4.5 の変種選択済み、`ids` と同じ順）。
    ///
    /// **E003**（未定義）と **E004**（範囲外）の ID は入らない。§6.4.6「有効モードの
    /// `personality` / `emotion` / `role` / 役割語パターンのみ表示する」の「有効モード」がこれ。
    pub definitions: Vec<Definition<'a>>,
    /// 診断（E003 / E004 / W006 と、この層が担当する E005）。**ファイル単位**の範囲。
    pub diagnostics: Vec<FileDiagnostic>,
}

impl Assignment<'_> {
    /// モードが決まったか。
    pub fn is_determined(&self) -> bool {
        !self.step.is_undetermined()
    }

    /// 2 モード併用か（§6.4.4「2 モードが有効なときの参照ペイン表示は…モードごとに並べる」）。
    pub fn is_combined(&self) -> bool {
        self.definitions.len() > 1
    }
}

/// §6.4.3 の 8 段を上から順に試す（**最初に見つかったものを採用**、固定）。
pub fn resolve<'a>(input: &Input<'a>, target: &Target<'a>) -> Assignment<'a> {
    let episode_starts = input
        .episode
        .map(|found| line_starts(found.lines.iter().map(|line| line.raw().len())))
        .unwrap_or_default();
    let chapter_starts = input
        .chapter
        .map(|found| line_starts(found.lines.iter().map(|line| line.raw().len())))
        .unwrap_or_default();
    let all_starts = input
        .all
        .map(|found| line_starts(found.lines.iter().map(|line| line.raw().len())))
        .unwrap_or_default();

    // 段 1〜7（§6.4.3）
    let steps: [(Step, Option<Candidate<'a>>); 7] = [
        (Step::SpeakerLine, from_speaker(input, target)),
        (
            Step::SectionMode,
            input
                .episode
                .and_then(|episode| from_section_fields(input, target, episode, &episode_starts)),
        ),
        (
            Step::SceneMode,
            input
                .episode
                .and_then(|episode| from_scene_fields(input, target, episode, &episode_starts)),
        ),
        (
            Step::EpisodePreamble,
            input
                .episode
                .and_then(|episode| from_preamble(input, target, episode, &episode_starts)),
        ),
        (
            Step::ChapterCast,
            input.chapter.and_then(|outline| {
                from_outline(input, target, File::Chapter, outline, &chapter_starts)
            }),
        ),
        (
            Step::AllCast,
            input
                .all
                .and_then(|outline| from_outline(input, target, File::All, outline, &all_starts)),
        ),
        (Step::DefaultMode, from_default_mode(input, target)),
    ];
    for (step, candidate) in steps {
        if let Some(found) = candidate {
            return finish(input, target, step, found);
        }
    }

    // 段 8: モード未決定 → W006（§6.4.3）
    Assignment {
        step: Step::Undetermined,
        ids: Vec::new(),
        definitions: Vec::new(),
        diagnostics: vec![FileDiagnostic {
            file: File::Episode,
            diagnostic: Diagnostic::simple(
                DiagnosticCode::W006,
                section_range(input, target, &episode_starts),
            ),
        }],
    }
}

/// 1 キャラの全モード定義（§6.4.1）。書いた順。
///
/// §6.4.3 の段 8（モード未決定）で参照ペインが「全モードの `role` だけを 1 行ずつ畳んで出す」
/// のと、M4 の補完（§9.7.3「モード補完」）が使う。
pub fn definitions<'a>(characters: &'a Characters<'a>, character: usize) -> Vec<Definition<'a>> {
    // `character_of` は**行 index** を取る。ここに来るのは `characters` の**添字**なので
    // 直接引く（`Target::Character { index }` と同じ意味。§6.4.1）
    let Some(found) = characters.characters.get(character) else {
        return Vec::new();
    };
    let Some(section) = found.section(SectionKey::Modes) else {
        return Vec::new();
    };
    section
        .modes
        .iter()
        .map(|node| Definition { character, node })
        .collect()
}

/// `mode_id` が一致する定義だけ（**大文字小文字を区別しない**、B-64）。書いた順。
pub fn definitions_with_id<'a>(
    characters: &'a Characters<'a>,
    character: usize,
    id: &str,
) -> Vec<Definition<'a>> {
    // ASCII の大文字小文字を無視した比較（§6.1.3 の 2「英字は小文字化」と同じ流儀）
    definitions(characters, character)
        .into_iter()
        .filter(|found| found.id().eq_ignore_ascii_case(id))
        .collect()
}

// ============================================================ 段ごとの候補

/// 1 段ぶんの手掛かり。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate<'a> {
    /// どのファイルに書かれていたか。
    file: File,
    /// `(モード ID, その参照トークンの範囲)`。書かれた順。
    found: Vec<(&'a str, Range<usize>)>,
    /// 同じキャラを 2 回書いていたか（§5.4.2 の制約）。切り詰めの判定に使う。
    repeated: bool,
    /// **E005 をこの層が出すか**。同じ形を構文層／辞書層が既に報告しているときは `false`
    /// （B-67）。`false` でも §7 のクイックフィックス（先頭 2 つ）は適用する。
    report_too_many: bool,
    /// **E005** の波線を引く範囲（フィールドの値、話者注釈なら注釈全体）。
    value_range: Range<usize>,
}

/// 段 1: 本文の当該行の話者注釈の修飾子（§6.4.3「その行のみ」）。
fn from_speaker<'a>(input: &Input<'a>, target: &Target<'a>) -> Option<Candidate<'a>> {
    let speaker = target.speaker?;
    if speaker.modes.is_empty() {
        return None;
    }
    // 注釈のキャラが対象のキャラと同じときだけ（別の人の台詞のモードを拾わない）
    if !is_target_character(input, speaker.name, target.character) {
        return None;
    }
    let range = shift(&speaker.range, target.line_start);
    Some(Candidate {
        file: File::Contents,
        found: speaker
            .modes
            .iter()
            .map(|&id| (id, range.clone()))
            .collect(),
        // 話者注釈は 1 行に 1 つなので「同じキャラを 2 回」は起きない。
        // 3 つ以上は §5.2.1 が注釈として認めない（`contents::split_modes` → W002）
        repeated: false,
        report_too_many: false,
        value_range: range,
    })
}

/// 段 2: 話プロットの当該**節**の `mode:`（§6.4.3）。
fn from_section_fields<'a>(
    input: &Input<'a>,
    target: &Target<'a>,
    episode: &'a EpisodePlot<'a>,
    starts: &[usize],
) -> Option<Candidate<'a>> {
    let scene = scene_index(episode, &target.scene)?;
    let at = episode.scenes[scene]
        .sections
        .iter()
        .position(|section| section.number == target.scene.section)?;
    from_lines(
        input,
        target,
        File::Episode,
        episode,
        &episode.scenes[scene].sections[at].fields,
        FieldKey::Mode,
        starts,
    )
}

/// 段 3: 話プロットの当該**場面**（見出し直下）の `mode:`（§6.4.3）。
fn from_scene_fields<'a>(
    input: &Input<'a>,
    target: &Target<'a>,
    episode: &'a EpisodePlot<'a>,
    starts: &[usize],
) -> Option<Candidate<'a>> {
    let scene = scene_index(episode, &target.scene)?;
    from_lines(
        input,
        target,
        File::Episode,
        episode,
        &episode.scenes[scene].fields,
        FieldKey::Mode,
        starts,
    )
}

/// 段 4: 話プロットの preamble の `mode:`（§6.4.3）。
fn from_preamble<'a>(
    input: &Input<'a>,
    target: &Target<'a>,
    episode: &'a EpisodePlot<'a>,
    starts: &[usize],
) -> Option<Candidate<'a>> {
    from_lines(
        input,
        target,
        File::Episode,
        episode,
        &episode.preamble.fields,
        FieldKey::Mode,
        starts,
    )
}

/// 段 5 / 6: A 形式ブロックの `cast:`（§6.4.3、§5.3.2「`'名前[:モード[+モード]]`」）。
fn from_outline<'a>(
    input: &Input<'a>,
    target: &Target<'a>,
    file: File,
    outline: &'a Outline<'a>,
    starts: &[usize],
) -> Option<Candidate<'a>> {
    let block = outline.blocks.iter().find(|block| {
        block
            .id
            .is_some_and(|id| covers(&id, block.depth, &target.scene))
    })?;
    from_fields(
        input,
        target,
        file,
        |line| outline.field(line),
        &block.fields,
        FieldKey::Cast,
        starts,
    )
}

/// 段 7: `characters.md` の `default-mode`（§5.5.3「明示が無い節での既定モード」）。
fn from_default_mode<'a>(input: &Input<'a>, target: &Target<'a>) -> Option<Candidate<'a>> {
    let characters = input.characters;
    let starts = line_starts(characters.lines.iter().map(|line| line.raw().len()));
    let found = characters.characters.get(target.character)?;
    for &line in &found.fields {
        let Some(field) = characters.field(line) else {
            continue;
        };
        if field.key != FieldKey::DefaultMode {
            continue;
        }
        // §5.5.3 の値は `mode_id` 1 つ。`A+B` と書かれていても受け付ける（B-65）。
        // 3 つ以上は §6.4.4 の E005。`default-mode:` の値は `'` を含まないので辞書層の
        // 走査でも見つからず、他のどの層もモード式として読まない → **この層が報告する**。
        let text = field.value.trim_matches(crate::core::text::lexer::is_space);
        if text.is_empty() {
            continue;
        }
        let ids: Vec<&str> = text
            .split(contents::MODE_JOIN)
            .map(|part| part.trim_matches(crate::core::text::lexer::is_space))
            .filter(|part| !part.is_empty())
            .collect();
        if ids.is_empty() {
            continue;
        }
        let base = starts.get(line).copied().unwrap_or(0) + field.value_at;
        return Some(Candidate {
            file: File::Characters,
            report_too_many: ids.len() > 2,
            found: ids
                .into_iter()
                .map(|id| (id, base..base + text.len()))
                .collect(),
            repeated: false,
            value_range: base..base + text.len(),
        });
    }
    None
}

// ============================================================ フィールドの読み取り

/// B 形式のフィールド行（行 index の列）から候補を作る。
fn from_lines<'a>(
    input: &Input<'a>,
    target: &Target<'a>,
    file: File,
    episode: &'a EpisodePlot<'a>,
    lines: &[usize],
    key: FieldKey,
    starts: &[usize],
) -> Option<Candidate<'a>> {
    from_fields(
        input,
        target,
        file,
        |line| episode.field(line),
        lines,
        key,
        starts,
    )
}

/// `key` のフィールド行を走査し、**対象キャラのモード**を集める。
///
/// 値は §5.4.2 / §5.3.2 の「`'名前[:モード[+モード]]` の空白区切り」なので、
/// M2 の [`Field::refs`](crate::core::syntax::plot_outline::Field::refs) が読んだ
/// 参照トークンを辞書で引き、対象のキャラのトークンだけ拾う。
fn from_fields<'a>(
    input: &Input<'a>,
    target: &Target<'a>,
    file: File,
    field_at: impl Fn(usize) -> Option<&'a crate::core::syntax::plot_outline::Field<'a>>,
    lines: &[usize],
    key: FieldKey,
    starts: &[usize],
) -> Option<Candidate<'a>> {
    let mut found: Vec<(&'a str, Range<usize>)> = Vec::new();
    let mut value_range: Option<Range<usize>> = None;
    let mut hits = 0usize;

    for &line in lines {
        let Some(field) = field_at(line) else {
            continue;
        };
        if field.key != key {
            continue;
        }
        let base = starts.get(line).copied().unwrap_or(0) + field.value_at;
        let span = base..base + field.value.len();
        value_range = Some(match value_range {
            Some(seen) => seen.start..span.end.max(seen.end),
            None => span.clone(),
        });
        for reference in field.refs() {
            let range = shift(&reference.range, base);
            if !is_target_character(input, reference.name, target.character) {
                continue;
            }
            hits += 1;
            // `mode: 'リリア`（モードを書いていない）はこの段では「見つけなかった」扱い。
            // 3 つ以上書かれていても §7 のクイックフィックス「先頭 2 つを残す」で解決は続ける
            // ので、[`CharRef::modes`] が 2 つの上限で `None` にした場合は形を見て自分で割る。
            let Some(ids) = reference
                .modes()
                .or_else(|| reference.modifier.and_then(mode_ids))
            else {
                continue;
            };
            found.extend(ids.into_iter().map(|id| (id, range.clone())));
        }
    }

    if found.is_empty() {
        return None;
    }
    // §5.4.2「1 節の `mode` に同じキャラを 2 回書くのは E005」
    let repeated = hits > 1;
    Some(Candidate {
        // B 形式の `mode:` は `plot_episode::parse` が同じ値に E005 を出している（M2）ので
        // 重ねない。A 形式の `cast:` の重複は他の層が見えないのでこの層が報告する（B-67）。
        // 1 トークンの中の 3 つ以上（`cast: 'アルス:A+B+C`）は辞書層の担当なので出さない。
        report_too_many: repeated && file != File::Episode,
        file,
        found,
        repeated,
        value_range: value_range.unwrap_or(0..0),
    })
}

/// 修飾子が `mode_expr`（§5.1 `mode_id , [ "+" , mode_id ]`）の形なら**全**モード ID。
///
/// [`contents::split_modes`] と違って **2 つの上限をかけない**（3 つ以上は §6.4.4 の E005 だが、
/// §7 のクイックフィックスで先頭 2 つを使って解決は続けるため、解決層は中身を見る必要がある）。
/// 1 つでも `mode_id` の形をしていない部分があればモード式ではない（`'M0137:資料の題` や
/// `'アルス:担任`）ので `None`（E003 / E004 / E026 の領域。`plot_episode::mode_count` と同じ流儀）。
fn mode_ids(modifier: &str) -> Option<Vec<&str>> {
    let ids: Vec<&str> = modifier.split(contents::MODE_JOIN).collect();
    if ids.iter().all(|id| contents::is_mode_id(id)) {
        Some(ids)
    } else {
        None
    }
}

/// 参照名が対象のキャラを指すか。
fn is_target_character(input: &Input<'_>, name: &str, character: usize) -> bool {
    let Some(id) = input.dictionary.lookup(name, Some(&[EntryKind::Character])) else {
        return false;
    };
    match input.dictionary.entry(id).map(|entry| &entry.target) {
        Some(crate::core::resolve::dictionary::Target::Character { index, .. }) => {
            *index == character
        }
        _ => false,
    }
}

/// 対象の場面を持つ B 形式の見出し（行 index）。
///
/// B 形式の見出しは `@01-04-02`（場面まで）で書くのが普通なので、**節（D）を落とした
/// 3 階層**で比べる（[`SceneId::scene_partial`]）。見出しに節を書いてあれば
/// （`@01-04-02-03`）それも同じ 3 階層に畳まれるので一致する。
fn scene_index(episode: &EpisodePlot<'_>, scene: &SceneId) -> Option<usize> {
    let wanted = scene.scene_partial();
    episode
        .scenes
        .iter()
        .position(|found| found.id.is_some_and(|id| id.scene_partial() == wanted))
}

/// A 形式のブロックが対象の節を含むか（ブロック自身の階層までで比べる）。
///
/// `plots/all.txt` は depth 1（章）、`plots/AA.txt` は depth 2（話）が既定（§5.3）。
fn covers(block: &SceneId, depth: usize, target: &SceneId) -> bool {
    let left = [block.chapter, block.episode, block.scene, block.section];
    let right = [target.chapter, target.episode, target.scene, target.section];
    let depth = depth.clamp(1, 4);
    left[..depth] == right[..depth]
}

// ============================================================ 確定と診断

/// 候補を確定させる（§6.4.4 の制約検査と §6.4.5 の変種選択）。
fn finish<'a>(
    input: &Input<'a>,
    target: &Target<'a>,
    step: Step,
    mut candidate: Candidate<'a>,
) -> Assignment<'a> {
    let mut diagnostics = Vec::new();

    // §6.4.4「1 キャラ 1 節につき最大 2 モード（`A+B`）。3 つ以上は E005」
    // §5.4.2「1 節の `mode` に同じキャラを 2 回書くのは E005」
    if candidate.repeated || candidate.found.len() > 2 {
        // 波線は**この層が担当するときだけ**（B-67）。他の層と重ねない。
        if candidate.report_too_many {
            diagnostics.push(FileDiagnostic {
                file: candidate.file,
                diagnostic: Diagnostic::simple(
                    DiagnosticCode::E005,
                    Some(candidate.value_range.clone()),
                ),
            });
        }
        // §7 E005 のクイックフィックス「先頭 2 つを残す」どおりにして、解決は止めない
        candidate.found.truncate(2);
    }

    let name = character_name(input.characters, target.character);
    let mut ids = Vec::with_capacity(candidate.found.len());
    let mut definitions = Vec::with_capacity(candidate.found.len());
    for (id, range) in candidate.found {
        ids.push(id);
        match variant(input, target, id) {
            Variant::Found(definition) => definitions.push(definition),
            Variant::Undefined => diagnostics.push(FileDiagnostic {
                file: candidate.file,
                // §7 E003「{0} にモード {1} は定義されていません」
                diagnostic: Diagnostic::new(
                    DiagnosticCode::E003,
                    Some(range),
                    vec![Arg::text(name.clone()), Arg::text(id)],
                ),
            }),
            Variant::OutOfRange(variants) => diagnostics.push(FileDiagnostic {
                file: candidate.file,
                // §7 E004「{0} のモード {1} は {2} では使用できません（有効範囲: {3}）」
                // （第 2.8 版で `{0}` ＝キャラ名を使う文言になった。`PLANS.md` §9.1 A-7）
                diagnostic: Diagnostic::new(
                    DiagnosticCode::E004,
                    Some(range),
                    vec![
                        Arg::text(name.clone()),
                        Arg::text(id),
                        Arg::text(target.scene.display(Digits::default())),
                        Arg::text(range_text_of(&variants)),
                    ],
                ),
            }),
        }
    }

    Assignment {
        step,
        ids,
        definitions,
        diagnostics,
    }
}

/// §6.4.5 の変種選択の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Variant<'a> {
    /// 対象の節を含む定義が（少なくとも 1 つ）あった。先頭を採用する。
    Found(Definition<'a>),
    /// その `mode_id` の定義が無い（**E003**）。
    Undefined,
    /// 定義はあるがどれも `range` が対象を含まない（**E004**）。中身は全変種。
    OutOfRange(Vec<Definition<'a>>),
}

/// 同一 `mode_id` の変種から、対象 SceneId を含む定義を選ぶ（§6.4.5）。
fn variant<'a>(input: &Input<'a>, target: &Target<'a>, id: &str) -> Variant<'a> {
    let variants = definitions_with_id(input.characters, target.character, id);
    if variants.is_empty() {
        return Variant::Undefined;
    }
    // §6.4.5「複数が一致したら E017（定義側の range 重複エラー。パース時に検出するので
    // 実行時は起きない）」→ ここでは**先頭を選ぶ**だけ（E017 は M2 が出している）
    match variants.iter().find(|found| found.contains(target.scene)) {
        Some(found) => Variant::Found(*found),
        None => Variant::OutOfRange(variants),
    }
}

/// **E004** の引数 `{3}`（有効範囲）。変種が複数あれば書いた順に並べる。
fn range_text_of<'a>(variants: &[Definition<'a>]) -> String {
    variants
        .iter()
        .map(|found| found.range_text())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// キャラの表示名（H1）。診断の引数 `{0}` 用。
fn character_name(characters: &Characters<'_>, character: usize) -> String {
    characters
        .characters
        .get(character)
        .map(|found| found.name.to_string())
        .unwrap_or_default()
}

/// **W006**（モード未決定）の位置。話プロットの当該節（無ければ場面、それも無ければ範囲なし）。
///
/// §7 W006 のクイックフィックスは「プロットに `mode:` を追加」なので、波線は
/// **追加する場所**（B 形式の節）に引く。
fn section_range(input: &Input<'_>, target: &Target<'_>, starts: &[usize]) -> Option<Range<usize>> {
    let episode = input.episode?;
    let scene = scene_index(episode, &target.scene)?;
    let found = &episode.scenes[scene];
    let section = found
        .sections
        .iter()
        .find(|section| section.number == target.scene.section);
    let lines = match section {
        Some(found) => &found.span,
        None => &found.span,
    };
    let start = *starts.get(lines.start)?;
    let end = lines.end.checked_sub(1).and_then(|last| {
        let line = episode.lines.get(last)?.raw();
        starts.get(last).map(|at| at + line.len())
    })?;
    Some(start..end.max(start))
}

/// 行の長さの列から行頭 byte 位置を作る。
fn line_starts(lengths: impl IntoIterator<Item = usize>) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut at = 0usize;
    for length in lengths {
        starts.push(at);
        at += length + 1;
    }
    starts
}

/// 行内相対の範囲をファイル単位の範囲にする。
fn shift(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start + by..range.end + by
}

/// §6.4.4 の **E004** の引数 `{2}` に書く節 ID（既定の桁数）。
///
/// `project.json.structure.id_digits` が 2 以外なら呼び出し側が
/// [`SceneId::display`] で作り直す（core は設定を読まない）。
pub fn scene_text(scene: &SceneId) -> String {
    scene.display(Digits::default())
}

/// モデルの [`Mode`] を木のノードから作る（§6.4.1）。
///
/// 参照ペイン（§9.5.2 の 3「配役」）と ContextPacket（§15.4.1 の `5 cast`）が
/// **意味のかたまり**を必要とするので、`model` の型へ写す口をここに置く
/// （`PLANS.md` §9.2 B-56「意味の塊が要るところで model の型をその場で組む」）。
/// 行の位置が要る診断は [`Definition`] 側で行う。
pub fn to_model(definition: &Definition<'_>, characters: &Characters<'_>) -> Mode {
    let mut mode = Mode {
        id: definition.id().to_string(),
        label: definition.label().to_string(),
        range: definition
            .node
            .range
            .as_ref()
            .map(|found| found.text.to_string()),
        ..Mode::default()
    };
    for &line in &definition.node.fields {
        let Some(field) = characters.field(line) else {
            continue;
        };
        let value = field.full_value();
        let text = if value.is_empty() { None } else { Some(value) };
        match field.key {
            FieldKey::Personality => mode.personality = text,
            FieldKey::Emotion => mode.emotion = text,
            FieldKey::Role => mode.role = text,
            FieldKey::Note => mode.note = text,
            _ => {}
        }
    }
    mode.patterns = patterns_of(characters, &definition.node.patterns);
    mode
}

/// 行 index の列（モード専用 or `## tone` 直下）から役割語パターンを model の型にする。
///
/// **適用パターン集合**（§6.5.3）は「有効モードのパターン ∪ キャラ共通 `## tone`」の
/// **和集合**で、モード専用があるときにキャラ共通を打ち消すわけではない（§5.5.5「モード
/// 専用パターンがある場合、そのモードが有効な節では **モード専用 ∪ キャラ共通**」）。
/// 和集合を作るのは [`applicable_patterns`]。
pub fn patterns_of(
    characters: &Characters<'_>,
    lines: &[usize],
) -> Vec<crate::core::model::role_pattern::RolePattern> {
    let mut out = Vec::new();
    for &line in lines {
        let Some(crate::core::syntax::characters_md::Line::Pattern(found)) =
            characters.lines.get(line)
        else {
            continue;
        };
        out.extend(found.patterns.iter().map(|pattern| {
            crate::core::model::role_pattern::RolePattern::new(pattern.text.to_string())
        }));
    }
    out
}

/// §6.5.3 の**適用パターン集合**を作る。
///
/// 「（有効モードのパターン）∪（キャラ共通 `## tone` のパターン）」。有効モードが 2 つなら
/// それぞれの和（§6.5.3）。**上書きではなく和集合**であることは §5.5.5 も同じことを言う
/// （「モード専用パターンがある場合、そのモードが有効な節では **モード専用 ∪ キャラ共通**」）。
/// 順は「モード専用 → キャラ共通」で、同じパターンの文字列は 1 本にする（§6.5.4「複数パターンが
/// 一致したら、一致範囲の和集合に 1 本の下線を引く（二重線にしない）」）。
///
/// モードが決まらない（[`Step::Undetermined`]）ときはキャラ共通だけを使う。
pub fn applicable_patterns(
    characters: &Characters<'_>,
    assignment: &Assignment<'_>,
    character: usize,
) -> Vec<crate::core::model::role_pattern::RolePattern> {
    let mut out: Vec<crate::core::model::role_pattern::RolePattern> = assignment
        .definitions
        .iter()
        .flat_map(|definition| patterns_of(characters, &definition.node.patterns))
        .collect();
    if let Some(found) = characters.characters.get(character) {
        if let Some(section) = found.section(SectionKey::Tone) {
            out.extend(patterns_of(characters, &section.patterns));
        }
    }
    // 同じパターンがモード専用とキャラ共通の両方に書かれていても 1 本にする。
    // 適用集合は高々十数本なので線形探索で足りる（§6.5.5 の計算量は正規表現検索のほうが支配的）
    let mut at = 0;
    while at < out.len() {
        if out[..at].iter().any(|other| *other == out[at]) {
            out.remove(at);
        } else {
            at += 1;
        }
    }
    out
}
