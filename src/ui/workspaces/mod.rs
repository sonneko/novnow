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
pub mod stage_two;
pub mod write;

pub use crate::app::workspace::{Switcher, Workspace};

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
    /// `material`。
    pub material: material::State,
    /// `review`。
    pub review: review::State,
    /// `cast`。
    pub cast: cast::State,
    /// `agent`。
    pub agent: agent::State,
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
    /// インレイヒントを出すか（`ui.json.inlay_hints`、`Alt+H`。§6.6.2）。
    pub inlay_hints: bool,
    /// A modal/command palette is receiving arrows instead of the list.
    pub overlay: bool,
}

/// Unmodified arrows browse list rows only when no text editor or overlay owns the keys.
/// Structural Alt+arrows still commit and open the destination.
pub(super) fn browse_keys(ctx: &egui::Context, view: &View) -> Option<bool> {
    if view.overlay || ctx.wants_keyboard_input() {
        return None;
    }
    if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
        Some(true)
    } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
        Some(false)
    } else {
        None
    }
}

impl View {
    /// エディタの見た目（[`crate::ui::widgets::slice_editor::show`] に渡す）。
    pub fn look(&self) -> crate::ui::widgets::section_editor::Look {
        crate::ui::widgets::section_editor::Look {
            mode: self.mode,
            scale: self.scale,
            max_line_chars: if self.locale == crate::ui::i18n::Locale::En {
                70
            } else {
                44
            },
            inlay: self.inlay_hints.then_some(self.locale),
        }
    }
}

/// ワークスペースの描画から `NovnowApp` に頼むこと（マウスでの操作。キーは `Command` で来る）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Expose a keyboard-only action as a visible button.
    Shortcut(crate::ui::shell::Command),
    /// `Ctrl+Enter` と同じ（下の段へ降りる・昇格・採用など）。
    Down,
    /// `Ctrl+Delete` と同じ（アイデアの破棄・資料の却下）。
    Drop,
    /// ステータスバーに 1 行出す。
    Notice(String),
    /// 参照ペインの資料の候補を、今の節のプロットの `material:` に足す（§10.2・§16.4、B-136）。
    AppendMaterial(String),
    /// Open the full source material instead of modifying a plot.
    OpenMaterial(String),
    /// 参照ペインの診断からの自動回復（§6.2.6 / §9.5.4、F-23）: 1 形態素を登録する。
    Register {
        /// 登録先（`words.md` / `characters.md`）。
        kind: crate::core::resolve::dictionary::EntryKind,
        /// 登録する名前。
        name: String,
    },
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
    let height = (24.0 * scale).max(24.0);
    let width = ui.available_width().max(24.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, selected, text)
    });
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, 0.0, colors.bg_selection);
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, colors.bg_selection.gamma_multiply(0.6));
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
    let text_width = galley.size().x;
    painter.galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    let end = (x + text_width).min(rect.max.x - 6.0);
    painter.line_segment(
        [
            egui::pos2(x, rect.center().y + 8.0 * scale),
            egui::pos2(end, rect.center().y + 8.0 * scale),
        ],
        Stroke::new(
            0.6_f32,
            if response.hovered() {
                color
            } else {
                colors.border
            },
        ),
    );
    response
}

/// Uniform visible action button used by the workspace panels.
pub fn action_button(ui: &mut Ui, text: impl Into<String>) -> egui::Response {
    crate::ui::widgets::action_button(ui, text)
}

/// Uniform filter/tab/choice affordance used by workspace panels.
pub fn selectable_action(ui: &mut Ui, selected: bool, text: egui::RichText) -> egui::Response {
    crate::ui::widgets::selectable_action(ui, selected, text)
}

/// 幅を決めた横のペイン（egui 0.27 の `exact_width` は中身の最小幅を枠の幅そのものにするので、
/// 内側の余白のぶんだけ外へはみ出す。中身の最小幅を左右の余白を除いた幅にする）。
pub fn fixed_width(panel: egui::SidePanel, width: f32, margin_x: f32) -> egui::SidePanel {
    panel
        .resizable(false)
        .default_width(width)
        .width_range((width - 2.0 * margin_x).max(0.0)..=width)
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
/// Highest outstanding diagnostic severity for the review header badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewProblem {
    Error,
    Warning,
}

pub fn indicator(
    ui: &mut Ui,
    mode: Mode,
    locale: Locale,
    current: Workspace,
    problem: Option<ReviewProblem>,
) -> Option<Workspace> {
    let colors = theme::palette(mode);
    let mut clicked = None;
    for workspace in Workspace::available_all() {
        let name = crate::ui::i18n::text(locale, workspace.label_key()).to_owned();
        let (hit, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
        let rect = Rect::from_center_size(hit.center(), Vec2::splat(18.0));
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
        let painter = ui.painter_at(hit);
        if hovered {
            painter.rect_filled(hit, 2.0, colors.bg_selection.gamma_multiply(0.6));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(0.6_f32, colors.border),
        );
        glyph(
            &painter,
            rect,
            workspace,
            if workspace == Workspace::Review {
                match problem {
                    Some(ReviewProblem::Error) => colors.error,
                    Some(ReviewProblem::Warning) => colors.warn,
                    None => color,
                }
            } else {
                color
            },
        );
        if workspace == Workspace::Review {
            let badge = match problem {
                Some(ReviewProblem::Error) => Some(colors.error),
                Some(ReviewProblem::Warning) => Some(colors.warn),
                None => None,
            };
            if let Some(badge) = badge {
                painter.circle_filled(rect.right_top() + Vec2::new(-1.0, 2.0), 2.5, badge);
            }
        }
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
