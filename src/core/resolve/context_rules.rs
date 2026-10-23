//! 参照ペインの優先順位と、ContextPacket との**共用**（§9.5.2 / §15.4）。
//!
//! このファイルは**規則だけ**を持つ（純粋なデータと順位）。中身を集めるのは M4 の
//! `core/features/*`（参照ペイン）と M10 の ContextPacket 生成で、**両方がここを引く**ことが
//! §15.4 の MUST（「UI と LLM で情報源と優先順位を一致させる」）の実体になる。
//! 描画（§9.5.1「カード UI にしない」）と文言（§12.2）は UI 側。
//!
//! # 優先順位（§9.5.2、v0 踏襲・**固定**）
//!
//! **カーソル行 > 同一節 > 同一場面 > 同一話**（[`Scope`]）。
//! 「カーソル行に直接含まれる要素」が最優先で、遠いものほど後ろ。どれにも入らない項目は
//! **ペインに出さない**（[`Layout::scope_of_line`] が `None`）。
//!
//! # 表示順（§9.5.2 の 9 セクション、**固定**）
//!
//! [`PaneSection`] が 1〜9 の順と「内容が無かったときの振る舞い」を持つ。
//! **節の出現規則（MUST）**: 5（五感）・6（空間）・8（挿絵）は内容が無ければ
//! **見出しごと表示しない**。1〜4・7・9 は内容が無ければ「—」も書かずに省略する
//! （空の見出しが並ぶと情報密度が落ち、P3「ノイズを増やさない」に反するため）。
//!
//! # ContextPacket（§15.4.1 の 9 ブロック、**固定順**）
//!
//! [`PacketBlock`] が 1〜9 の順・Markdown 見出し・「常に含まれる（外せない）」と
//! 「決して削らない」の 2 つの MUST を持つ。**どのペインのセクションがどのブロックと
//! 同じ情報源か**は [`PaneSection::packet_blocks`] が対応表になっている（§15.4 の MUST）。

use std::ops::Range;

use crate::core::diagnostics::{Diagnostic, DiagnosticCode, Severity};

/// §9.5.2 の優先順位（v0 踏襲・**固定**）。
///
/// **カーソル行 > 同一節 > 同一場面 > 同一話**。派生した `Ord` もこの順（近いほうが小さい）なので
/// `sort_by_key` にそのまま渡せる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    /// カーソル行に**直接含まれる**要素（§9.5.2「カーソル行」）。
    CursorLine,
    /// 同じ**節**（D、§5.1）。
    Section,
    /// 同じ**場面**（C）。
    Scene,
    /// 同じ**話**（B）。
    Episode,
}

/// ペインに出す**最も遠い**スコープ（§9.5.2「カーソル行 > 同一節 > 同一場面 > 同一話」）。
///
/// これより遠い（別の話の）項目はペインに出さない。ContextPacket 側のブロックごとの範囲は
/// §15.4.1 が決める（`3 plot` は当該節、`4 target` は前後の節を含む等）ので、
/// **この定数はペイン専用**。
pub const PANE_FARTHEST: Scope = Scope::Episode;

impl Scope {
    /// 近い順（`CursorLine` = 0）。
    pub fn order(self) -> u8 {
        match self {
            Self::CursorLine => 0,
            Self::Section => 1,
            Self::Scene => 2,
            Self::Episode => 3,
        }
    }

    /// **`self` を上限（これより遠くは見ない）としたとき `other` を含むか**。
    ///
    /// `self` が `other` と同じか**遠い**なら `true`。`PANE_FARTHEST.includes(found)` が
    /// 「その項目をペインに出すか」の判定になる（§9.5.2 は同一話まで）。
    pub fn includes(self, other: Self) -> bool {
        self.order() >= other.order()
    }

    /// 近い順に全部。
    pub const ALL: [Self; 4] = [Self::CursorLine, Self::Section, Self::Scene, Self::Episode];

    /// UI が i18n キー（§12.3 `scope.<key>`）を引くための ascii 名。
    pub fn key(self) -> &'static str {
        match self {
            Self::CursorLine => "cursor-line",
            Self::Section => "section",
            Self::Scene => "scene",
            Self::Episode => "episode",
        }
    }
}

/// 節・場面・話の**行範囲**（§9.5.2 の優先順位を適用するための枠）。
///
/// **呼び出し側が木から作る**。core は I/O をせず形式も問わないので（§3.2）、B 形式
/// （`plot_episode::Scene` / `Section` の `span`）でも C 形式（`contents::Section`）でも、
/// 資料や用語のように「1 ファイル＝1 話」のものを渡してもよい。
///
/// 範囲は**行 index**（半開区間）。byte 位置と混ぜないこと（`docs/history.md` B-56）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    /// カーソル行。
    pub cursor_line: usize,
    /// カーソル行を含む**節**の行範囲。
    pub section: Option<Range<usize>>,
    /// カーソル行を含む**場面**の行範囲（節を含む）。
    pub scene: Option<Range<usize>>,
    /// カーソル行を含む**話**の行範囲（場面を含む）。
    pub episode: Option<Range<usize>>,
}

impl Layout {
    /// `line` がどのスコープに属するか。**どれにも入らなければ `None`**（ペインに出さない）。
    ///
    /// カーソル行は節にも場面にも話にも含まれるが、**最も近いもの 1 つ**だけを返す
    /// （§9.5.2 の「カーソル行 > 同一節 > …」は優先順位なので、2 重に数えない）。
    pub fn scope_of_line(&self, line: usize) -> Option<Scope> {
        if line == self.cursor_line {
            return Some(Scope::CursorLine);
        }
        let spans = [
            (Scope::Section, &self.section),
            (Scope::Scene, &self.scene),
            (Scope::Episode, &self.episode),
        ];
        spans.into_iter().find_map(|(scope, span)| {
            span.as_ref()
                .filter(|found| found.contains(&line))
                .map(|_| scope)
        })
    }

    /// 並べ替えのキー（近い順 → 行の順）。スコープ外なら `None`。
    pub fn rank(&self, line: usize) -> Option<(Scope, usize)> {
        self.scope_of_line(line).map(|scope| (scope, line))
    }

    /// `(行, 項目)` を**近い順**に並べ替える（同じスコープなら行の順。決定的にするため）。
    ///
    /// **スコープ外の項目は捨てる**（§9.5.2「同一話」より遠いものはペインに出さない）。
    /// `farthest` に [`PANE_FARTHEST`] を渡すのが既定の使い方。
    pub fn ranked<T>(
        &self,
        farthest: Scope,
        items: impl IntoIterator<Item = (usize, T)>,
    ) -> Vec<(Scope, usize, T)> {
        let mut out: Vec<(Scope, usize, T)> = items
            .into_iter()
            .filter_map(|(line, item)| {
                let scope = self.scope_of_line(line)?;
                farthest.includes(scope).then_some((scope, line, item))
            })
            .collect();
        out.sort_by_key(|found| (found.0, found.1));
        out
    }

    /// §9.5.2 の 1「**今の節のプロット**（話プロットの当該節テキスト。**無ければ場面テキスト**）」。
    ///
    /// 節に 1 行も無いときは場面の範囲に倒す（継承。§9.5.3 が「上位階層から継承した」ことを
    /// 見出しの右に薄く添えるので、呼び出し側は返ってきた範囲が `section` と違うことで分かる）。
    pub fn plot_span(&self) -> Option<Range<usize>> {
        match &self.section {
            Some(found) if !found.is_empty() => Some(found.clone()),
            _ => self.scene.clone(),
        }
    }
}

/// §9.5.2 の 2「上位プロット（章プロットの `summary` 1 行 → 全体プロットの `summary` 1 行。
/// **計 2 行だけ**）」の行数。
pub const UPPER_PLOT_LINES: usize = 2;

/// §9.5.2 の表示順（**固定**）の 9 セクション。
///
/// 順も「内容が無かったときの振る舞い」も仕様どおりで、UI はこの enum を**並べるだけ**。
/// 見出しの文言は §12.2 により core が持たないので、[`PaneSection::key`] で
/// i18n キー（§12.3 `pane.<key>`）を引く。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaneSection {
    /// 1. **今の節のプロット**（話プロットの当該節テキスト。無ければ場面テキスト）。
    Plot,
    /// 2. **上位プロット**（章 → 全体の `summary`。計 [`UPPER_PLOT_LINES`] 行だけ）。
    UpperPlot,
    /// 3. **配役**: 有効モードの情報のみ（§6.4.6）。
    ///
    /// 中身は [`crate::core::resolve::modes::Assignment::definitions`]（E003 / E004 の ID は
    /// 入らない）と [`crate::core::resolve::modes::applicable_patterns`]。
    Cast,
    /// 4. **用語**: この節に出現する用語の `term` と `definition`（1 行目のみ）。
    ///
    /// 出現検出は [`crate::core::resolve::dictionary::Dictionary::occurrences`]（Aho-Corasick）。
    Words,
    /// 5. **五感**: `senses:` の宣言（§5.4.4）。**無ければ見出しごと出さない**（MUST）。
    Senses,
    /// 6. **空間**: `space:` の内容（§5.3.6）。4 行超は折りたたみ。**無ければ見出しごと出さない**。
    Space,
    /// 7. **資料**: 明示参照（`'M...`）→ 本文の引用注釈（`^M...`、§5.2.4）→
    ///    条件一致候補（§16.4）の順。各行 `M0137 タイトル` + `summary`。
    ///
    /// 参照の解決は [`crate::core::resolve::materials`]。**このセクションの中身の並び順は
    /// §9.5.2 が決めている**（スコープの優先順位ではない）ので、[`PaneSection::source_order`]
    /// を見る。
    Materials,
    /// 8. **挿絵**: この節の `! ` 行と `illustration:`（§5.2.5）。**無ければ見出しごと出さない**。
    Illustrations,
    /// 9. **診断**: **この節に関するもののみ**。`E` → `W` の順、各 1 行。
    Diagnostics,
}

impl PaneSection {
    /// §9.5.2 の表示順（1〜9）。
    pub fn order(self) -> u8 {
        match self {
            Self::Plot => 1,
            Self::UpperPlot => 2,
            Self::Cast => 3,
            Self::Words => 4,
            Self::Senses => 5,
            Self::Space => 6,
            Self::Materials => 7,
            Self::Illustrations => 8,
            Self::Diagnostics => 9,
        }
    }

    /// §9.5.2 の**節の出現規則（MUST）**: 内容が無いときに**見出しごと**表示しないか。
    ///
    /// 5（五感）・6（空間）・8（挿絵）が `true`。1〜4・7・9 は `false`（内容が無ければ
    /// 「—」も書かずに**中身だけ**省略する）。
    pub fn hides_heading_when_empty(self) -> bool {
        matches!(self, Self::Senses | Self::Space | Self::Illustrations)
    }

    /// i18n キー（§12.3 `pane.<key>`）に使う ascii 名。
    pub fn key(self) -> &'static str {
        match self {
            Self::Plot => "plot",
            Self::UpperPlot => "upper-plot",
            Self::Cast => "cast",
            Self::Words => "words",
            Self::Senses => "senses",
            Self::Space => "space",
            Self::Materials => "materials",
            Self::Illustrations => "illustrations",
            Self::Diagnostics => "diagnostics",
        }
    }

    /// §15.4.1 の ContextPacket の**同じ情報源**のブロック（§15.4 の MUST「UI と LLM で
    /// 情報源と優先順位を一致させる」）。対応が無ければ空（`9 診断` はパケットに入れない）。
    ///
    /// 逆引き（ブロック → セクション）は [`PacketBlock::pane_sections`]。
    pub fn packet_blocks(self) -> &'static [PacketBlock] {
        match self {
            // §15.4.1 の `3 plot` = 当該場面のプロット全文＋当該節のプロット行
            Self::Plot => &[PacketBlock::Plot],
            // `2 outline` = all（当該章の summary）＋ chapter（当該話の summary + goal）
            Self::UpperPlot => &[PacketBlock::Outline],
            // `5 cast` = 有効モードのキャラのみ（§6.4.6 と同じ集合。MUST で削らない）
            Self::Cast => &[PacketBlock::Cast],
            // `6 lexicon` = 当該節に現れる用語の term と definition 1 行
            Self::Words => &[PacketBlock::Lexicon],
            // `senses:`（§5.4.4）は B 形式のフィールドなので `3 plot` に入る
            Self::Senses => &[PacketBlock::Plot],
            // `space:`（§5.3.6）も B 形式のフィールド
            Self::Space => &[PacketBlock::Plot],
            // `7 materials` = 参照された資料の id / title / summary / 本文
            Self::Materials => &[PacketBlock::Materials],
            // `illustration:`（§5.2.5）は `3 plot`、`! ` 行は本文なので `4 target` に入る
            Self::Illustrations => &[PacketBlock::Plot, PacketBlock::Target],
            // §15.4.1 の 9 ブロックに診断は無い（LLM には診断ではなく制約を渡す）
            Self::Diagnostics => &[],
        }
    }

    /// §9.5.2 の 7（資料）の**中身の並び順**: 明示参照 → 引用注釈 → 条件一致候補。
    ///
    /// この 3 つだけはスコープの優先順位ではなく**経路の順**で並ぶ（§9.5.2 の明文）。
    /// 0 が先頭。それ以外のセクションは `None`（スコープ順でよい）。
    pub fn source_order(self, source: MaterialSource) -> Option<u8> {
        match self {
            Self::Materials => Some(match source {
                MaterialSource::Explicit => 0,
                MaterialSource::Quote => 1,
                MaterialSource::Condition => 2,
            }),
            _ => None,
        }
    }

    /// 表示順に全部。
    pub const ALL: [Self; 9] = [
        Self::Plot,
        Self::UpperPlot,
        Self::Cast,
        Self::Words,
        Self::Senses,
        Self::Space,
        Self::Materials,
        Self::Illustrations,
        Self::Diagnostics,
    ];
}

/// §9.5.2 の 7（資料）の 3 経路（**この順**に並べる）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaterialSource {
    /// 明示参照（`'M0137`、§6.6.1）。
    Explicit,
    /// 本文の引用注釈（`^M0137`、§5.2.4）。
    Quote,
    /// 条件一致候補（§16.4 の `conditions`）。
    Condition,
}

/// §15.4.1 の ContextPacket のブロック（**固定順**）。
///
/// 送信本文は Markdown で、**ブロックの順序と見出しは固定**（LLM 側のパース安定性のため）。
/// 文字数予算は §15.4.2（`prompts/*.md` の `max-chars` で上書き可）で、M10 が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PacketBlock {
    /// 1 `project`: title / genre / 文体規約。
    Project,
    /// 2 `outline`: all（当該章の summary）＋ chapter（当該話の summary + goal）。
    Outline,
    /// 3 `plot`: scene（当該場面のプロット全文）＋ section（当該節のプロット行と check）。
    Plot,
    /// 4 `target`: current（当該節の本文）＋ before（直前 2 節）＋ after（直後 1 節）。
    Target,
    /// 5 `cast`: **有効モードのキャラのみ**（§6.4.6）。
    Cast,
    /// 6 `lexicon`: 当該節に現れる用語の term と definition 1 行。
    Lexicon,
    /// 7 `materials`: 参照された資料の id / title / summary / 本文（予算内で切り詰め）。
    Materials,
    /// 8 `constraints`: `prompts/<role>.md` の output セクション＋機械的制約。
    Constraints,
    /// 9 `history`: 同一節に対する直近 3 件のタスクの採否と人間のコメント。
    History,
}

impl PacketBlock {
    /// §15.4.1 の並び順（1〜9）。
    pub fn order(self) -> u8 {
        match self {
            Self::Project => 1,
            Self::Outline => 2,
            Self::Plot => 3,
            Self::Target => 4,
            Self::Cast => 5,
            Self::Lexicon => 6,
            Self::Materials => 7,
            Self::Constraints => 8,
            Self::History => 9,
        }
    }

    /// §15.4.1 の Markdown 見出し（**固定**。ascii なので §12.2 の対象外）。
    pub fn heading(self) -> &'static str {
        match self {
            Self::Project => "## 1 project",
            Self::Outline => "## 2 outline",
            Self::Plot => "## 3 plot",
            Self::Target => "## 4 target",
            Self::Cast => "## 5 cast",
            Self::Lexicon => "## 6 lexicon",
            Self::Materials => "## 7 materials",
            Self::Constraints => "## 8 constraints",
            Self::History => "## 9 history",
        }
    }

    /// ブロックの名前（見出しの番号の後ろ。§5.10.1 の `exclude:` と §5.9.2 の `context:` に書く名前）。
    pub fn name(self) -> &'static str {
        let heading = self.heading();
        heading.rsplit(' ').next().unwrap_or(heading)
    }

    /// 名前から（[`PacketBlock::name`] の逆）。
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|found| found.name() == name.trim())
    }

    /// §5.9.2「`context:` は 9 ブロックから部分集合を選ぶ。**`constraints` と `project` は
    /// 常に含まれる（外せない、MUST）**」。
    pub fn is_mandatory(self) -> bool {
        matches!(self, Self::Project | Self::Constraints)
    }

    /// §15.4.1「**決して削らないもの（MUST）**: `5 cast`（有効モード）、`3 plot` の当該節、
    /// `8 constraints`、`1 project` の文体規約」。
    ///
    /// 文字数予算が足りないときに**切り詰めてはならない**ブロック（§15.4.4 の切り詰め順は
    /// M10 が持つ。ここは「削ってよい順」ではなく「削ってはいけない」の集合）。
    pub fn is_never_trimmed(self) -> bool {
        matches!(
            self,
            Self::Project | Self::Plot | Self::Cast | Self::Constraints
        )
    }

    /// 固定順に全部。
    pub const ALL: [Self; 9] = [
        Self::Project,
        Self::Outline,
        Self::Plot,
        Self::Target,
        Self::Cast,
        Self::Lexicon,
        Self::Materials,
        Self::Constraints,
        Self::History,
    ];

    /// このブロックと同じ情報源のペインのセクション（§9.5.2）。
    ///
    /// [`PaneSection::packet_blocks`] の逆引き。**対応するセクションが無いブロックもある**
    /// （`1 project` と `8 constraints` と `9 history` はペインに出ない。`4 target` は
    /// エディタそのもの）。
    pub fn pane_sections(self) -> Vec<PaneSection> {
        PaneSection::ALL
            .into_iter()
            .filter(|found| found.packet_blocks().contains(&self))
            .collect()
    }
}

/// §9.5.2 の 9「診断: **この節に関するもののみ**。`E` → `W` の順、各 1 行」の並び順。
///
/// 重大度（[`Severity`]）→ 文書内の位置 の順。同じ位置ならコード順（決定的にするため）。
pub fn diagnostic_rank(diagnostic: &Diagnostic) -> (u8, usize, &'static str) {
    (
        severity_rank(diagnostic.code.severity()),
        diagnostic.range.clone().map_or(0, |found| found.start),
        diagnostic.code.as_str(),
    )
}

/// §9.5.2 の 9 の `E` → `W` の順（`I` は最後。§9.5.2 は E と W しか名指ししていないが、
/// §7.2 の I を捨てるとレビュー画面と食い違うので末尾に並べる）。
pub fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}

/// 診断を §9.5.2 の 9 の順に並べ替える（**この節に関するものだけ**を渡すこと。
/// 節で絞るのは呼び出し側＝「この節に関するもののみ」は文書ごとの診断の選択なので、
/// 位置を持たない §7.1 の [`crate::core::diagnostics::Anchor::Status`] はここに来ない）。
pub fn sort_diagnostics(list: &mut [Diagnostic]) {
    list.sort_by(|left, right| diagnostic_rank(left).cmp(&diagnostic_rank(right)));
}

/// `codes` を §9.5.2 の 9 の順（`E` → `W` → `I`、位置は分からないのでコード順）に並べる。
pub fn sort_codes(list: &mut [DiagnosticCode]) {
    list.sort_by(|left, right| {
        severity_rank(left.severity())
            .cmp(&severity_rank(right.severity()))
            .then_with(|| left.as_str().cmp(right.as_str()))
    });
}
