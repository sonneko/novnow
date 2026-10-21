//! ワークスペース（§10）の識別子と切替の規則。**egui に依存しない**。
//!
//! 描画（`ui::workspaces::*`）は `ui` feature の中だけだが、どれが開けるか・直前はどれか
//! という規則はヘッドレスのセッション（§19.7 の `session.workspace_get` /
//! `session.workspace_switch`）も同じものを使う。

/// ワークスペース（§10 の表の順。番号が `Alt+N`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Workspace {
    /// 1 本文執筆（§10.1）。
    #[default]
    Write,
    /// 2 話プロット（§10.2）。
    PlotEpisode,
    /// 3 全体 / 章プロット（§10.3）。
    PlotOutline,
    /// 4 アイデア（§10.4）。
    Idea,
    /// 5 資料（§10.5 / §16）。
    Material,
    /// 6 配役・モード・役割語（§10.6）。
    Cast,
    /// 7 読者モード（§10.7）。v1 では欠番（追加仕様 #27 で ② へ）。
    Reader,
    /// 8 エージェント管制（§10.8）。M10 まで欠番。
    Agent,
    /// 9 点検（§10.9）。
    Review,
}

impl Workspace {
    /// §10 の表の順。
    pub const ALL: [Workspace; 9] = [
        Workspace::Write,
        Workspace::PlotEpisode,
        Workspace::PlotOutline,
        Workspace::Idea,
        Workspace::Material,
        Workspace::Cast,
        Workspace::Reader,
        Workspace::Agent,
        Workspace::Review,
    ];

    /// `Alt+N` の `N`（1〜9）から。
    pub fn of_number(number: u8) -> Option<Self> {
        number
            .checked_sub(1)
            .and_then(|index| Self::ALL.get(usize::from(index)).copied())
    }

    /// `Alt+N` の `N`（1〜9）。
    pub fn number(self) -> u8 {
        Self::ALL
            .iter()
            .position(|found| *found == self)
            .map_or(0, |index| index as u8 + 1)
    }

    /// §10 の表の ID（`plot-episode` など）。
    pub fn id(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::PlotEpisode => "plot-episode",
            Self::PlotOutline => "plot-outline",
            Self::Idea => "idea",
            Self::Material => "material",
            Self::Cast => "cast",
            Self::Reader => "reader",
            Self::Agent => "agent",
            Self::Review => "review",
        }
    }

    /// 名前のカタログの鍵（ツールチップ・アクセシブル名・通知）。
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Write => "header.workspace.write",
            Self::PlotEpisode => "header.workspace.plot-episode",
            Self::PlotOutline => "header.workspace.plot-outline",
            Self::Idea => "header.workspace.idea",
            Self::Material => "header.workspace.material",
            Self::Cast => "header.workspace.cast",
            Self::Reader => "header.workspace.reader",
            Self::Agent => "header.workspace.agent",
            Self::Review => "header.workspace.review",
        }
    }

    /// v1 のこの版で開けるか（`reader` は ②。B-126。`agent` は M10 で開けるようになった）。
    pub fn available(self) -> bool {
        true
    }

    /// 開けるワークスペース（ヘッダのインジケータに並べる順）。
    pub fn available_all() -> impl Iterator<Item = Workspace> {
        Self::ALL.into_iter().filter(|found| found.available())
    }
}

/// 今のワークスペースと直前のワークスペース（`Alt+0`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Switcher {
    current: Workspace,
    previous: Option<Workspace>,
    /// 一度でも開いたワークスペース（初めて開くときだけ今の節の対応物に着地する）。
    visited: Vec<Workspace>,
}

impl Switcher {
    /// 今のワークスペース。
    pub fn current(&self) -> Workspace {
        self.current
    }

    /// 直前のワークスペース。
    pub fn previous(&self) -> Option<Workspace> {
        self.previous
    }

    /// `to` へ切り替える。開けない（欠番）・今と同じなら `false`。
    pub fn switch(&mut self, to: Workspace) -> bool {
        if !to.available() || to == self.current {
            return false;
        }
        if !self.visited.contains(&self.current) {
            self.visited.push(self.current);
        }
        self.previous = Some(self.current);
        self.current = to;
        true
    }

    /// 直前のワークスペースへ（`Alt+0`）。直前が無ければ `false`。
    pub fn back(&mut self) -> bool {
        match self.previous {
            Some(previous) => self.switch(previous),
            None => false,
        }
    }

    /// `workspace` を初めて開くところか（切り替えた直後に呼ぶ）。
    pub fn first_visit(&self, workspace: Workspace) -> bool {
        !self.visited.contains(&workspace)
    }

    /// `workspace` を開いたことにする（初回の着地を済ませた後）。
    pub fn mark_visited(&mut self, workspace: Workspace) {
        if !self.visited.contains(&workspace) {
            self.visited.push(workspace);
        }
    }
}
