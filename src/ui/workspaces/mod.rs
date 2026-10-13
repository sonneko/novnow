//! ワークスペース（§10。指示 5「用途別 UI」）。
//!
//! - `Alt+1..9` で切り替え、`Alt+0` で直前のワークスペースへ戻る（§10.0）。`reader`（`Alt+7`）は
//!   追加仕様 #27 で ② へ移したので欠番、`agent`（`Alt+8`）は M10 まで欠番（`PLANS.md` §9.2 B-126）。
//! - 切替で **SceneId は変わらない**（MUST）。各ワークスペースは自分の状態（選んでいる行・開いている
//!   文書）を `Session` の中に持ち続け、切替で破棄・再構築しない（§10.0「事前計算」）。初めて開く
//!   ワークスペースだけ、今の節の対応物に着地する（`Ctrl+Shift+1..9` の強制持ち越しは ②）。
//! - 切替は保存のきっかけ（§8.2「ワークスペース切替」）。確認ダイアログは出さない。
//!
//! 状態の規則（どれが選べるか・直前はどれか）は egui に依存しない [`Switcher`] に置き、描画は
//! ワークスペースごとのファイル（`plot_episode.rs` など）に置く。

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::core::export::State as PlotState;
use crate::ui::i18n::Locale;
use crate::ui::settings::PaneSide;
use crate::ui::theme::{self, Mode};

pub mod agent;
pub mod cast;
pub mod idea;
pub mod material;
pub mod plot_episode;
pub mod plot_outline;
pub mod review;
pub mod write;

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

    /// v1 のこの版で開けるか（`reader` は ②、`agent` は M10。B-126）。
    pub fn available(self) -> bool {
        !matches!(self, Self::Reader | Self::Agent)
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

/// ワークスペースごとの状態（`Session` が持ち続ける。§10.0「破棄・再構築をしない」）。
#[derive(Debug, Default)]
pub struct Spaces {
    /// 今のワークスペースと直前のワークスペース。
    pub switcher: Switcher,
    /// `plot-episode`。
    pub episode: plot_episode::State,
    /// `plot-outline`。
    pub outline: plot_outline::State,
    /// `idea`。
    pub idea: idea::State,
    /// `review`。
    pub review: review::State,
}

/// 描画に要る画面の設定（`NovnowApp` から渡す）。
#[derive(Debug, Clone, Copy)]
pub struct View {
    /// 配色。
    pub mode: Mode,
    /// 言語。
    pub locale: Locale,
    /// 文字の倍率（`ui.json.font_scale`）。
    pub scale: f32,
    /// 今の時刻（秒）。
    pub now: f64,
    /// 参照ペインを出すなら、その側と幅（§9.1。幅 < 900px では畳む）。
    pub reference: Option<(PaneSide, f32)>,
}

/// ワークスペースの描画から `NovnowApp` に頼むこと（マウスでの操作。キーは `Command` で来る）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// `Ctrl+Enter` と同じ（下の段へ降りる・昇格など）。
    Down,
    /// ステータスバーに 1 行出す。
    Notice(String),
}

/// 状態の縦線の色（§10.2: draft = 灰 / review = 橙 / fixed = 緑）。
pub fn state_color(mode: Mode, state: Option<PlotState>) -> Color32 {
    let colors = theme::palette(mode);
    match state {
        Some(PlotState::Fixed) => colors.ok,
        Some(PlotState::Review) => colors.warn,
        _ => colors.fg_faint,
    }
}

/// 1 行に収める（はみ出したら `…`）文字列のレイアウト。
pub fn one_line(text: &str, font: FontId, color: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job.wrap = TextWrapping {
        max_width: width.max(1.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('\u{2026}'),
    };
    job
}

/// 一覧の 1 行の中身（[`list_row`]）。
#[derive(Debug, Clone, Copy, Default)]
pub struct Row<'a> {
    /// 選択中か（`bg.selection`）。
    pub selected: bool,
    /// 左端の 3px の縦線の色（状態）。
    pub bar: Option<Color32>,
    /// 左の淡い番号（空なら出さない）。
    pub label: &'a str,
    /// 本文。
    pub text: &'a str,
    /// 本文を `fg.muted` で描く（場面見出しの行など）。
    pub muted: bool,
}

/// 一覧の 1 行（§10.2 の節リストなど）。左端に 3px の縦線、選択中は `bg.selection`。クリックは
/// 返す `Response` で分かる。
pub fn list_row(ui: &mut Ui, mode: Mode, scale: f32, row: Row<'_>) -> egui::Response {
    let Row {
        selected,
        bar,
        label,
        text,
        muted,
    } = row;
    let colors = theme::palette(mode);
    let height = 24.0 * scale;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, selected, text)
    });
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, 0.0, colors.bg_selection);
    }
    if let Some(color) = bar {
        painter.rect_filled(
            Rect::from_min_size(rect.min + Vec2::new(0.0, 3.0), Vec2::new(3.0, height - 6.0)),
            0.0,
            color,
        );
    }
    let mut x = rect.min.x + 10.0;
    if !label.is_empty() {
        let galley = ui.fonts(|fonts| {
            fonts.layout_job(one_line(
                label,
                FontId::new(11.0 * scale, FontFamily::Monospace),
                colors.fg_faint,
                width,
            ))
        });
        painter.galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley.clone(),
            colors.fg_faint,
        );
        x += galley.size().x.max(22.0 * scale) + 8.0;
    }
    let color = if muted {
        colors.fg_muted
    } else {
        colors.fg_primary
    };
    let galley = ui.fonts(|fonts| {
        fonts.layout_job(one_line(
            text,
            FontId::new(13.0 * scale, FontFamily::Proportional),
            color,
            rect.max.x - x - 6.0,
        ))
    });
    painter.galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    response
}

/// 段の区切り（1px の `border`、上下 4px の余白）。
pub fn separator(ui: &mut Ui, mode: Mode) {
    let colors = theme::palette(mode);
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, colors.border);
    ui.add_space(4.0);
}

/// 中身が無いときの案内（`fg.muted`・13px。§9.2.1 の 3「ヘルプの文言」）。
pub fn hint(ui: &mut Ui, mode: Mode, scale: f32, text: &str) {
    let colors = theme::palette(mode);
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(text)
            .font(FontId::new(13.0 * scale, FontFamily::Proportional))
            .color(colors.fg_muted),
    );
}

/// ヘッダのワークスペースインジケータ（§9.2.1「ワークスペース名はアイコン」）。開けるワークスペースの
/// 図形を並べ、今のものを `accent`、他を `fg.faint` で描く。ラベルは置かず、ツールチップと
/// アクセシブル名（カタログの `header.workspace.<id>`）だけ。クリックされたものを返す。
pub fn indicator(ui: &mut Ui, mode: Mode, locale: Locale, current: Workspace) -> Option<Workspace> {
    let colors = theme::palette(mode);
    let mut clicked = None;
    for workspace in Workspace::available_all() {
        let name = crate::ui::i18n::text(locale, workspace.label_key()).to_owned();
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
        let active = workspace == current;
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, active, &name)
        });
        let hovered = response.hovered();
        let response = response.on_hover_text(format!("{name}  Alt+{}", workspace.number()));
        if response.clicked() {
            clicked = Some(workspace);
        }
        let color = if active {
            colors.accent
        } else if hovered {
            colors.fg_muted
        } else {
            colors.fg_faint
        };
        glyph(&ui.painter_at(rect.expand(2.0)), rect, workspace, color);
        ui.add_space(2.0);
    }
    clicked
}

/// ワークスペースの図形（16px の中に 1.5px の線で描く。§9.2.1）。
pub fn glyph(painter: &Painter, rect: Rect, workspace: Workspace, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    let c = rect.center();
    let at = |x: f32, y: f32| Pos2::new(c.x + x, c.y + y);
    match workspace {
        // 鉛筆
        Workspace::Write => {
            painter.line_segment([at(-5.0, 5.0), at(5.0, -5.0)], stroke);
            painter.line_segment([at(-5.0, 5.0), at(-2.0, 5.0)], stroke);
        }
        // 箇条書き（話プロットの節リスト）
        Workspace::PlotEpisode => {
            for y in [-4.0, 0.0, 4.0] {
                painter.circle_filled(at(-5.0, y), 1.0, color);
                painter.line_segment([at(-2.0, y), at(6.0, y)], stroke);
            }
        }
        // 木（全体 → 章 → 話）: 親の節点から 2 つの子へ枝分かれする
        Workspace::PlotOutline => {
            painter.circle_filled(at(-4.0, -5.0), 1.8, color);
            painter.line_segment([at(-4.0, -4.0), at(-4.0, 5.0)], stroke);
            painter.line_segment([at(-4.0, 0.0), at(0.5, 0.0)], stroke);
            painter.line_segment([at(-4.0, 5.0), at(0.5, 5.0)], stroke);
            painter.circle_filled(at(3.0, 0.0), 1.8, color);
            painter.circle_filled(at(3.0, 5.0), 1.8, color);
        }
        // 電球
        Workspace::Idea => {
            painter.circle_stroke(at(0.0, -1.5), 4.0, stroke);
            painter.line_segment([at(-2.0, 5.0), at(2.0, 5.0)], stroke);
        }
        // 本
        Workspace::Material => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(10.0, 11.0)),
                1.0,
                stroke,
            );
            painter.line_segment([at(-2.0, -5.5), at(-2.0, 5.5)], stroke);
        }
        // 人
        Workspace::Cast => {
            painter.circle_stroke(at(0.0, -3.0), 2.5, stroke);
            painter.line_segment([at(-5.0, 5.5), at(-3.0, 1.5)], stroke);
            painter.line_segment([at(-3.0, 1.5), at(3.0, 1.5)], stroke);
            painter.line_segment([at(3.0, 1.5), at(5.0, 5.5)], stroke);
        }
        // 本（読者）・ロボット（エージェント）は v1 では出さない。形だけ用意する
        Workspace::Reader => {
            painter.line_segment([at(-6.0, -4.0), at(0.0, -2.0)], stroke);
            painter.line_segment([at(6.0, -4.0), at(0.0, -2.0)], stroke);
            painter.line_segment([at(0.0, -2.0), at(0.0, 5.0)], stroke);
        }
        Workspace::Agent => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(10.0, 8.0)), 2.0, stroke);
        }
        // チェック
        Workspace::Review => {
            painter.line_segment([at(-5.0, 0.0), at(-1.5, 4.0)], stroke);
            painter.line_segment([at(-1.5, 4.0), at(5.5, -4.5)], stroke);
        }
    }
}
