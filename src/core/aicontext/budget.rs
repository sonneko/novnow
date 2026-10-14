//! 文字数予算（§15.4.2）と切り詰め（§15.4.3）。**決定論**（同じ入力なら同じ結果）。
//!
//! # 予算
//!
//! 役割ごとの表（§15.4.2）。integrator は表に無いので、統合に要る `9 history`（タスクの状態）を厚くした
//! 列を持つ（第 2.16 版で表に追加、`PLANS.md` §9.2 B-140）。`prompts/<role>.md` の `max-chars` は
//! その役割の**合計**を上書きし、各ブロックの予算は表の比率のまま伸び縮みする。送れる上限は
//! `min(合計, agents.max_chars_per_request)`。
//!
//! # 切り詰め
//!
//! ブロックの中身は「部品」（[`Part`]）の並びで、部品ごとに §15.4.3 の順位（1〜7）を持つ。順位の無い
//! 部品は**決して削らない**（`5 cast`・`3 plot` の当該節・`8 constraints`・`1 project`、それと `4 target`
//! の current）。
//!
//! 1. ブロックごとに予算まで削る（順位 → 同じ順位の中の順の順。各部品は末尾から削る）。
//! 2. まだ上限を超えていれば、全ブロックをまたいで同じ順で削る。
//! 3. それでも超えるなら「超過」（送れない。ドライランで赤、§15.9）。
//!
//! 削った部品は `…（n 文字省略）` を末尾に付ける（全部削っても見出しと印は残る＝何が落ちたかが分かる）。
//! 文字数はブロックの本文（見出しの行を除く、ラベルと改行を含む）を Unicode のスカラー値で数える。

use crate::core::resolve::context_rules::PacketBlock;

/// 省略の印の前半（§15.4.3 の書式。パケットの書式の語彙）。
/// i18n: format-vocabulary
pub const OMITTED_OPEN: &str = "…（";
/// 省略の印の後半。
/// i18n: format-vocabulary
pub const OMITTED_CLOSE: &str = " 文字省略）";

/// 表の列（§15.4.2、ブロックの固定順）。
pub fn table(role: &str) -> [usize; 9] {
    match role {
        "plotter" => [400, 1200, 2000, 600, 800, 400, 600, 600, 400],
        "stylist" => [800, 0, 400, 2400, 1600, 400, 0, 600, 400],
        "logician" => [300, 1600, 2000, 2400, 800, 800, 600, 600, 400],
        "researcher" => [200, 300, 600, 300, 200, 200, 3000, 600, 200],
        "ideator" => [300, 800, 800, 400, 600, 400, 1000, 600, 300],
        "integrator" => [300, 1200, 600, 300, 300, 200, 200, 600, 2300],
        // writer と、表に無い名前
        _ => [400, 300, 1200, 2400, 1200, 600, 800, 600, 400],
    }
}

/// ブロックの予算（`max_chars` があれば表の比率で伸び縮みさせる）。
pub fn budgets(role: &str, max_chars: Option<usize>) -> [usize; 9] {
    let base = table(role);
    let total: usize = base.iter().sum();
    match max_chars {
        Some(want) if want > 0 && total > 0 && want != total => {
            base.map(|value| value * want / total)
        }
        _ => base,
    }
}

/// 送れる上限（`min(合計, max_chars_per_request)`。`max_chars_per_request` が 0 なら合計だけ）。
pub fn cap(role: &str, max_chars: Option<usize>, max_chars_per_request: usize) -> usize {
    let total = max_chars
        .filter(|found| *found > 0)
        .unwrap_or_else(|| table(role).iter().sum());
    if max_chars_per_request == 0 {
        total
    } else {
        total.min(max_chars_per_request)
    }
}

/// 省略の印。
pub fn omitted(count: usize) -> String {
    format!("{OMITTED_OPEN}{count}{OMITTED_CLOSE}")
}

/// 部品の書き方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// `label: 中身`（中身が複数行なら [`Style::Block`]）。
    Inline,
    /// `label:` の次の行から中身。
    Block,
    /// 中身だけ（`- name: ...` のような 1 件）。
    Bare,
}

/// ブロックの部品。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// ラベル（`all`・`before 01-04-02-01`・`style` など。[`Style::Bare`] なら説明用）。
    pub label: String,
    /// 元の中身（削る前）。
    pub text: String,
    /// 書き方。
    pub style: Style,
    /// §15.4.3 の順位（1〜7）。`None` は決して削らない。
    pub rank: Option<u8>,
    /// 同じ順位の中で削る順（小さいほど先）。
    pub seq: u32,
    /// 残した文字数（削っていなければ中身の全部）。
    pub keep: usize,
}

impl Part {
    /// 部品を作る（削らない）。
    pub fn new(label: impl Into<String>, text: impl Into<String>, style: Style) -> Self {
        let text = text.into();
        let keep = text.chars().count();
        Self {
            label: label.into(),
            text,
            style,
            rank: None,
            seq: 0,
            keep,
        }
    }

    /// 削ってよい部品にする。
    pub fn trimmable(mut self, rank: u8, seq: u32) -> Self {
        self.rank = Some(rank);
        self.seq = seq;
        self
    }

    /// 元の文字数。
    pub fn total(&self) -> usize {
        self.text.chars().count()
    }

    /// 削った文字数。
    pub fn omitted(&self) -> usize {
        self.total().saturating_sub(self.keep)
    }

    /// 削った後の中身（印付き）。
    pub fn content(&self) -> String {
        let omitted = self.omitted();
        if omitted == 0 {
            return self.text.clone();
        }
        let mut out: String = self.text.chars().take(self.keep).collect();
        out.push_str(&omitted_marker(omitted));
        out
    }

    /// 削られた部分（「何が落ちたか」の表示用）。
    pub fn dropped(&self) -> String {
        self.text.chars().skip(self.keep).collect()
    }

    /// 書いた形（中身が空なら空）。
    pub fn rendered(&self) -> String {
        let content = self.content();
        if content.is_empty() {
            return String::new();
        }
        match self.style {
            Style::Bare => content,
            Style::Inline if !content.contains('\n') => format!("{}: {content}", self.label),
            _ => format!("{}:\n{content}", self.label),
        }
    }

    fn size_with(&mut self, keep: usize) -> usize {
        let saved = self.keep;
        self.keep = keep;
        let size = self.rendered().chars().count();
        self.keep = saved;
        size
    }

    /// 書いた形の文字数が `need` 以上減るまで末尾から削る（全部削っても足りなければ全部）。
    /// 減った文字数を返す。
    fn shrink(&mut self, need: usize) -> usize {
        let before = self.rendered().chars().count();
        if need == 0 || self.keep == 0 {
            return 0;
        }
        let goal = before.saturating_sub(need);
        // keep を減らすと文字数は（弱く）単調に減る。goal 以下になる最大の keep を二分探索で探す
        let (mut low, mut high) = (0usize, self.keep);
        if self.size_with(0) > goal {
            self.keep = 0;
        } else {
            while low < high {
                let mid = (low + high).div_ceil(2);
                if self.size_with(mid) <= goal {
                    low = mid;
                } else {
                    high = mid - 1;
                }
            }
            self.keep = low;
        }
        before.saturating_sub(self.rendered().chars().count())
    }
}

/// 省略の印（[`omitted`] と同じ。部品の中で使う名前）。
fn omitted_marker(count: usize) -> String {
    omitted(count)
}

/// ブロック 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// どのブロックか。
    pub kind: PacketBlock,
    /// 予算。
    pub budget: usize,
    /// 人間が外した（§15.7.2）。本文は空になる。
    pub excluded: bool,
    /// 部品。
    pub parts: Vec<Part>,
}

impl Block {
    /// 本文（見出しの行を除く）。
    pub fn body(&self) -> String {
        if self.excluded {
            return String::new();
        }
        self.parts
            .iter()
            .map(Part::rendered)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 本文の文字数。
    pub fn size(&self) -> usize {
        self.body().chars().count()
    }

    /// どこかを削ったか。
    pub fn trimmed(&self) -> bool {
        !self.excluded && self.parts.iter().any(|part| part.omitted() > 0)
    }

    /// 予算を超えているか（決して削らない部品のせいで超えることがある）。
    pub fn over(&self) -> bool {
        self.size() > self.budget
    }

    fn order(&self) -> Vec<usize> {
        let mut out: Vec<usize> = (0..self.parts.len())
            .filter(|index| self.parts[*index].rank.is_some())
            .collect();
        out.sort_by_key(|index| (self.parts[*index].rank, self.parts[*index].seq, *index));
        out
    }

    /// 1 段目: 予算まで削る。
    pub fn fit(&mut self) {
        if self.excluded {
            return;
        }
        for index in self.order() {
            let size = self.size();
            if size <= self.budget {
                break;
            }
            self.parts[index].shrink(size - self.budget);
        }
    }
}

/// 2 段目: 全ブロックの合計が `cap` に収まるまで、ブロックをまたいで §15.4.3 の順に削る。
pub fn fit_total(blocks: &mut [Block], cap: usize) {
    let mut order: Vec<(u8, u32, usize, usize)> = Vec::new();
    for (at, block) in blocks.iter().enumerate() {
        if block.excluded {
            continue;
        }
        for (index, part) in block.parts.iter().enumerate() {
            if let Some(rank) = part.rank {
                order.push((rank, part.seq, at, index));
            }
        }
    }
    order.sort();
    for (_, _, at, index) in order {
        let total: usize = blocks.iter().map(Block::size).sum();
        if total <= cap {
            break;
        }
        blocks[at].parts[index].shrink(total - cap);
    }
}
