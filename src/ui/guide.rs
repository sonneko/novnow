//! ガイドモード（§9.10「はじめての手引き」）。**egui に依存しない**ので、画面を回さずに規則を試せる。
//!
//! - 段階の定義はここ 1 か所（[`STEPS`]）。文言は `src/ui/i18n/{ja,en}.toml` の `guide.*`。
//! - 出る条件もここで決める（[`should_auto_start`]）。呼ぶ側は「保存済みプロジェクトの数」と
//!   `ui.json` の `guide_done` を渡すだけにする。
//! - 段階は**実際の操作**で進む（仕様 §9.10）。ボタンだけで進むのは 2 と 9 だけで、残りは
//!   [`Signals`] が条件を満たしたときに [`Guide::observe`] が送る。先に操作を済ませた段階は飛ばす。

/// 段階が次へ進む条件（仕様 §9.10 の表の「次へ進む条件」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// 作品が開いた（1 節目。プロジェクト選択から開いた時点で満たす）。
    Project,
    /// 「次へ」を押した。
    Button,
    /// 節が N 個以上ある。
    Sections(usize),
    /// `'` の参照が 1 つ以上ある。
    References,
    /// 話プロットが空でない。
    EpisodePlot,
    /// 読者モード（`Alt+7`）を開いた。
    Reader,
    /// エクスポートが完了した。
    Export,
    /// Zip の書き出しが完了した（「あとで」でも閉じられる）。
    Zip,
}

impl Advance {
    /// 押すだけで進む段階か（2 と 9 だけ）。
    pub fn by_button(self) -> bool {
        matches!(self, Self::Button)
    }
}

/// 段階の定義（仕様 §9.10 の表と同じ順序）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuideStep {
    /// 段階の番号（1〜9。`ui.json` には保存しない）。
    pub number: usize,
    /// 吹き出しに出す 1 文のキー（`guide.stepN.text`）。
    pub text: &'static str,
    /// 操作のキー（`guide.stepN.hint`）。
    pub hint: &'static str,
    /// 次へ進む条件。
    pub advance: Advance,
}

/// 9 段階（仕様 §9.10。公開サイトの「はじめての手引き」と同じ順序）。
pub const STEPS: &[GuideStep] = &[
    GuideStep {
        number: 1,
        text: "guide.step1.text",
        hint: "guide.step1.hint",
        advance: Advance::Project,
    },
    GuideStep {
        number: 2,
        text: "guide.step2.text",
        hint: "guide.step2.hint",
        advance: Advance::Button,
    },
    GuideStep {
        number: 3,
        text: "guide.step3.text",
        hint: "guide.step3.hint",
        advance: Advance::Sections(1),
    },
    GuideStep {
        number: 4,
        text: "guide.step4.text",
        hint: "guide.step4.hint",
        advance: Advance::Sections(2),
    },
    GuideStep {
        number: 5,
        text: "guide.step5.text",
        hint: "guide.step5.hint",
        advance: Advance::References,
    },
    GuideStep {
        number: 6,
        text: "guide.step6.text",
        hint: "guide.step6.hint",
        advance: Advance::EpisodePlot,
    },
    GuideStep {
        number: 7,
        text: "guide.step7.text",
        hint: "guide.step7.hint",
        advance: Advance::Reader,
    },
    GuideStep {
        number: 8,
        text: "guide.step8.text",
        hint: "guide.step8.hint",
        advance: Advance::Export,
    },
    GuideStep {
        number: 9,
        text: "guide.step9.text",
        hint: "guide.step9.hint",
        advance: Advance::Zip,
    },
];

/// 観測した状態（描く側が毎フレーム渡す）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Signals {
    /// 作品を開いているか。
    pub project: bool,
    /// 節の数。
    pub sections: usize,
    /// `'` の参照の数。
    pub references: usize,
    /// 話プロットに中身があるか。
    pub episode_plot: bool,
    /// 読者モードを開いたか。
    pub reader: bool,
    /// エクスポートが完了したか。
    pub exported: bool,
    /// Zip の書き出しが完了したか。
    pub zipped: bool,
}

impl Signals {
    /// `advance` の条件を満たしているか。
    pub fn meets(&self, advance: Advance) -> bool {
        match advance {
            Advance::Project => self.project,
            // 押したときに [`Guide::next`] が進める。状態では進まない。
            Advance::Button => false,
            Advance::Sections(count) => self.sections >= count,
            Advance::References => self.references >= 1,
            Advance::EpisodePlot => self.episode_plot,
            Advance::Reader => self.reader,
            Advance::Export => self.exported,
            Advance::Zip => self.zipped,
        }
    }
}

/// ガイドの状態。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Guide {
    /// 出ているか。
    pub active: bool,
    /// 今の段階（0 始まり。[`STEPS`] の添字）。
    pub index: usize,
    /// 最後まで進んだか（閉じた直後に一度だけ知らせるため）。
    pub finished: bool,
}

impl Guide {
    /// 段階 1 から出す。
    pub fn start(&mut self, index: usize) {
        self.active = true;
        self.finished = false;
        self.index = index.min(STEPS.len().saturating_sub(1));
    }

    /// 閉じる（「あとで」・`Esc`・最後まで進んだ）。
    pub fn close(&mut self, finished: bool) {
        self.active = false;
        self.finished = finished;
    }

    /// 今の段階。
    pub fn step(&self) -> Option<&'static GuideStep> {
        if !self.active {
            return None;
        }
        STEPS.get(self.index)
    }

    /// 「次へ」。押すだけで進む段階（2）でだけ進む（`Iterator::next` と紛れないよう別名）。
    pub fn next_step(&mut self) -> bool {
        let Some(step) = self.step() else {
            return false;
        };
        if !step.advance.by_button() {
            return false;
        }
        self.advance_one();
        true
    }

    /// 「戻る」。段階 1 では何もしない。
    pub fn back(&mut self) {
        if self.active {
            self.index = self.index.saturating_sub(1);
            self.finished = false;
        }
    }

    /// 観測した状態で進める（条件を満たした段階を飛ばす）。閉じたときに `true`。
    ///
    /// 仕様 §9.10「先に操作を済ませてしまった段階は、条件を満たした時点で飛ばす」。
    pub fn observe(&mut self, signals: &Signals) -> bool {
        if !self.active {
            return false;
        }
        while let Some(step) = self.step() {
            if step.advance.by_button() || !signals.meets(step.advance) {
                break;
            }
            self.advance_one();
        }
        !self.active && self.finished
    }

    /// 1 段階送る。最後まで来たら閉じる。
    fn advance_one(&mut self) {
        if self.index + 1 >= STEPS.len() {
            self.close(true);
        } else {
            self.index += 1;
        }
    }

    /// 表示用の進み具合（`3 / 9`）。
    pub fn progress(&self) -> (usize, usize) {
        (self.index + 1, STEPS.len())
    }
}

/// ガイドを自動で出すか（仕様 §9.10 の「自動で出る条件」）。
///
/// - 保存済みプロジェクトが 0 件（Web）／プロジェクト履歴が 0 件（デスクトップ）
/// - まだ閉じていない（`guide_done` が `false`）
pub fn should_auto_start(projects: usize, guide_done: bool) -> bool {
    projects == 0 && !guide_done
}

/// `'` の参照の数を数える（段階 5 の条件。本文の `'名前`・`'名前.モード`）。
///
/// 厳密な解析は意味解決（§6）が行うので、ここは**数えるだけ**の軽い走査にする。
/// `'` の直後が空白・改行・`'` のものは参照と数えない（`''` の空参照や、記号だけの行）。
pub fn count_references(text: &str) -> usize {
    let mut count = 0;
    let mut previous = '\0';
    for (index, ch) in text.char_indices() {
        if ch == '\'' {
            let next = text[index + ch.len_utf8()..].chars().next();
            let usable = next.is_some_and(|next| !next.is_whitespace() && next != '\'');
            if usable && previous != '\'' {
                count += 1;
            }
        }
        previous = ch;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_follow_the_specification_table() {
        assert_eq!(STEPS.len(), 9);
        for (index, step) in STEPS.iter().enumerate() {
            assert_eq!(step.number, index + 1);
        }
        assert_eq!(STEPS[1].advance, Advance::Button);
        assert_eq!(STEPS[8].advance, Advance::Zip);
    }

    #[test]
    fn typing_does_not_advance_button_steps() {
        // 添字 2 = 段階 3（節が 1 つ以上）。ボタンでは進まない。
        let mut guide = Guide::default();
        guide.start(2);
        assert!(!guide.next_step());
        assert_eq!(guide.index, 2);
    }

    #[test]
    fn satisfied_steps_are_skipped() {
        let mut guide = Guide::default();
        guide.start(2);
        let signals = Signals {
            project: true,
            sections: 2,
            ..Signals::default()
        };
        guide.observe(&signals);
        // 段階 3・4 は満たすので飛ばし、段階 5（`'` の参照）で止まる
        assert_eq!(guide.index, 4);
    }

    #[test]
    fn satisfying_the_last_step_closes_the_guide() {
        let mut guide = Guide::default();
        guide.start(8);
        assert!(guide.observe(&Signals {
            zipped: true,
            ..Signals::default()
        }));
        assert!(!guide.active);
        assert!(guide.finished);
    }

    #[test]
    fn references_are_counted() {
        assert_eq!(count_references("'太郎 が来た。'太郎.怒"), 2);
        assert_eq!(count_references("ただの ' と '' は数えない"), 0);
    }

    #[test]
    fn auto_start_needs_no_projects_and_no_earlier_close() {
        assert!(should_auto_start(0, false));
        assert!(!should_auto_start(0, true));
        assert!(!should_auto_start(1, false));
    }
}
