//! テーマ・配色・タイポグラフィ（§11）。
//!
//! **生の色（`Color32::from_rgb`）を書いてよいのはこのファイルだけ**（§11.1 MUST）。ほかは
//! [`Palette`] のトークンか [`token_style`] を通す。

use egui::{Color32, FontData, FontDefinitions, FontFamily, Stroke, Visuals};

use crate::core::features::semantic_tokens::TokenType;
use crate::ui::settings::ThemeChoice;

/// 実際に描く配色（`system` を解決した後）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 暗い配色（OS の設定が分からないときの既定、§11.4）。
    Dark,
    /// 明るい配色。
    Light,
}

impl Mode {
    /// 設定値と OS の設定から決める（OS の設定が分からなければ dark、§11.4）。
    pub fn resolve(choice: ThemeChoice, system_dark: Option<bool>) -> Self {
        match choice {
            ThemeChoice::Dark => Self::Dark,
            ThemeChoice::Light => Self::Light,
            ThemeChoice::System => match system_dark {
                Some(false) => Self::Light,
                _ => Self::Dark,
            },
        }
    }
}

/// セマンティックカラートークン（§11.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// `bg.base` 画面背景。
    pub bg_base: Color32,
    /// `bg.raised` ヘッダ・ステータスバー。
    pub bg_raised: Color32,
    /// `bg.inset` 編集中プレースホルダ・入力欄。
    pub bg_inset: Color32,
    /// `bg.selection` 選択範囲。
    pub bg_selection: Color32,
    /// `fg.primary` 本文。
    pub fg_primary: Color32,
    /// `fg.muted` 補助テキスト。
    pub fg_muted: Color32,
    /// `fg.faint` 見出し・インレイヒント。
    pub fg_faint: Color32,
    /// `border` 必要最小限の区切り。
    pub border: Color32,
    /// `accent` フォーカス・リンク・選択中。
    pub accent: Color32,
    /// `accent.faint` 編集面左線の淡色（§9.2.4。装飾色でコントラスト比の基準の対象外）。
    pub accent_faint: Color32,
    /// `state.unsaved` 未保存。
    pub unsaved: Color32,
    /// `state.error` エラー。
    pub error: Color32,
    /// `state.warn` 警告。
    pub warn: Color32,
    /// `state.ok` 完了・確定（役割語の緑下線、§11.3）。
    pub ok: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// §11.1 の表。
pub const fn palette(mode: Mode) -> Palette {
    match mode {
        Mode::Dark => Palette {
            bg_base: hex(0x15171B),
            bg_raised: hex(0x1D2026),
            bg_inset: hex(0x111317),
            bg_selection: hex(0x2D3A4D),
            fg_primary: hex(0xE6E6E3),
            fg_muted: hex(0xA6A8AD),
            fg_faint: hex(0x82878E),
            border: hex(0x2A2E35),
            accent: hex(0x6FA8FF),
            accent_faint: hex(0x354A6B),
            unsaved: hex(0xE8A33D),
            error: hex(0xE5484D),
            warn: hex(0xD8A200),
            ok: hex(0x49B86B),
        },
        Mode::Light => Palette {
            bg_base: hex(0xFBFBF9),
            bg_raised: hex(0xFFFFFF),
            bg_inset: hex(0xF1F1EE),
            bg_selection: hex(0xD6E4F7),
            fg_primary: hex(0x1B1C1E),
            fg_muted: hex(0x55585E),
            fg_faint: hex(0x727479),
            border: hex(0xDEDED9),
            accent: hex(0x1D5FD0),
            accent_faint: hex(0xADC4EB),
            unsaved: hex(0xB9720A),
            error: hex(0xC62A2F),
            warn: hex(0x8F6B00),
            ok: hex(0x1E7F43),
        },
    }
}

/// トークンの描き方（§11.2）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenStyle {
    /// 文字色。
    pub color: Color32,
    /// 太字（egui 0.27 に太字の字体は無いので、同じ色の細い下地で代用しない。色だけを使う）。
    pub bold: bool,
    /// 斜体。
    pub italic: bool,
    /// 文字の大きさの倍率。
    pub scale: f32,
}

/// §11.2 の表（表に無い 4 種は近い役割の色を使う。`docs/history.md` B-115）。
pub fn token_style(mode: Mode, kind: TokenType) -> TokenStyle {
    let dark = mode == Mode::Dark;
    let pick = |d: u32, l: u32| if dark { hex(d) } else { hex(l) };
    let plain = |color| TokenStyle {
        color,
        bold: false,
        italic: false,
        scale: 1.0,
    };
    match kind {
        TokenType::SigilStructure => TokenStyle {
            bold: true,
            ..plain(pick(0xC792EA, 0x7A3BA8))
        },
        TokenType::SceneId => plain(pick(0x8FB8FF, 0x1D5FD0)),
        TokenType::SceneTitle => plain(pick(0xA6A8AD, 0x55585E)),
        TokenType::SigilRef => TokenStyle {
            bold: true,
            ..plain(pick(0xF0A05A, 0xA85B14))
        },
        // 呼称（§6.1.1 の `Address`）はキャラと同じ色
        TokenType::RefCharacter | TokenType::RefAddress => plain(pick(0xE7C96B, 0x8A6A00)),
        TokenType::RefWord => plain(pick(0x7FD1A7, 0x157A52)),
        // 引用注釈（`^M0137`）は資料と同じ色
        TokenType::RefMaterial | TokenType::Citation => plain(pick(0x9BB4D6, 0x37567F)),
        TokenType::RefUnresolved => plain(pick(0xE5484D, 0xC62A2F)),
        TokenType::ModifierMode => TokenStyle {
            italic: true,
            ..plain(pick(0xC792EA, 0x7A3BA8))
        },
        TokenType::ModifierTitle => plain(pick(0x7E838A, 0x6B6E73)),
        TokenType::DialogueBracket => TokenStyle {
            bold: true,
            ..plain(pick(0x5FC2D6, 0x0C6A7D))
        },
        // ルビの親文字は本文と同じ
        TokenType::DialogueText | TokenType::RubyBase => plain(pick(0xE6E6E3, 0x1B1C1E)),
        TokenType::Speaker => TokenStyle {
            scale: 0.9,
            ..plain(pick(0xE7C96B, 0x8A6A00))
        },
        TokenType::SectionBreak => plain(pick(0x596574, 0x8D929A)),
        TokenType::FieldKey => plain(pick(0x8C9CB8, 0x425777)),
        // 挿絵行（`! `）はコメントと同じ（本文に出ない注記）
        TokenType::Comment | TokenType::Illustration => TokenStyle {
            italic: true,
            ..plain(pick(0x60666E, 0x8B8E93))
        },
        TokenType::RubyText => TokenStyle {
            scale: 0.6,
            ..plain(pick(0xA6A8AD, 0x55585E))
        },
    }
}

/// egui の配色（§11.1 のトークンから作る）。切替アニメーションはしない（§11.4 MUST）。
pub fn visuals(mode: Mode) -> Visuals {
    let colors = palette(mode);
    let mut visuals = match mode {
        Mode::Dark => Visuals::dark(),
        Mode::Light => Visuals::light(),
    };
    visuals.override_text_color = Some(colors.fg_primary);
    visuals.panel_fill = colors.bg_base;
    visuals.window_fill = colors.bg_raised;
    visuals.extreme_bg_color = colors.bg_inset;
    visuals.faint_bg_color = colors.bg_raised;
    visuals.code_bg_color = colors.bg_inset;
    visuals.hyperlink_color = colors.accent;
    visuals.selection.bg_fill = colors.bg_selection;
    visuals.selection.stroke = Stroke::new(1.0_f32, colors.accent);
    visuals.window_stroke = Stroke::new(1.0_f32, colors.border);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, colors.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, colors.fg_primary);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, colors.fg_muted);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, colors.fg_primary);
    visuals.widgets.hovered.bg_fill = colors.bg_selection.gamma_multiply(0.6);
    visuals.widgets.hovered.weak_bg_fill = colors.bg_selection.gamma_multiply(0.6);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, colors.accent);
    visuals.error_fg_color = colors.error;
    visuals.warn_fg_color = colors.warn;
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals
}

/// 同梱フォント（IPA ゴシック。Web は常にこれ、デスクトップは CJK フォントが見つからないときの
/// 代わり。§11.5）。
pub const IPA_FONT_LICENSE: &str =
    include_str!("../../licenses/IPA_Font_License_Agreement_v1.0.txt");

pub const BUNDLED_FONT: &[u8] = include_bytes!("../../assets/ipag.ttf");

/// フォントの名前（egui の `FontDefinitions` の中の）。
const CJK: &str = "novnow-cjk";

/// 記号の受け皿にする egui 既定のフォントの名前（`FontDefinitions::default()` の中の）。
const SYMBOLS: &str = "Hack";

/// デスクトップで探す CJK フォント（先に見つかったもの。§11.5「`fontdb` による自動検出」）。
#[cfg(not(target_arch = "wasm32"))]
const CANDIDATES: &[&str] = &[
    "Noto Sans CJK JP",
    "Noto Sans JP",
    "Source Han Sans JP",
    "Hiragino Sans",
    "Hiragino Kaku Gothic ProN",
    "Yu Gothic UI",
    "Yu Gothic",
    "Meiryo",
    "MS Gothic",
    "IPAexGothic",
    "IPAGothic",
    "TakaoGothic",
    "VL Gothic",
];

/// 本文のフォントを用意する。戻り値は「同梱フォントに頼ったか」（デスクトップで CJK フォントが
/// 見つからなかったときの警告用。§11.5 SHOULD）。
pub fn fonts() -> (FontDefinitions, bool) {
    let (data, fallback) = system_cjk().map_or_else(
        || (FontData::from_static(BUNDLED_FONT), true),
        |found| (found, false),
    );
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(CJK.to_owned(), data);
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        // CJK の全角約物は本文と同じ字体で描き、Hack は数学記号の受け皿に残す。
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, CJK.to_owned());
    }
    // 記号の最後の受け皿に egui 既定の等幅フォント（Hack）を置く。インレイヒントの括弧 `⟨⟩`
    // （U+27E8 / U+27E9、§6.6.2）は Ubuntu-Light にも同梱の IPA ゴシックにも無い（`docs/history.md` B-137）
    if fonts.font_data.contains_key(SYMBOLS) {
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push(SYMBOLS.to_owned());
    }
    (fonts, fallback)
}

#[cfg(not(target_arch = "wasm32"))]
fn system_cjk() -> Option<FontData> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    for name in CANDIDATES {
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(name)],
            ..fontdb::Query::default()
        };
        let Some(id) = db.query(&query) else {
            continue;
        };
        let found = db.with_face_data(id, |data, index| {
            let mut font = FontData::from_owned(data.to_vec());
            font.index = index;
            font
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn system_cjk() -> Option<FontData> {
    None
}

/// フォントの収録字に依存しないテーマアイコン。
#[cfg(feature = "ui")]
pub fn paint_theme_icon(
    painter: &egui::Painter,
    center: egui::Pos2,
    choice: crate::ui::settings::ThemeChoice,
    ink: Color32,
) {
    use crate::ui::settings::ThemeChoice;
    use egui::{epaint::CubicBezierShape, Stroke, Vec2};
    let point = |x: f32, y: f32| center + Vec2::new(x, y);
    let curve = |points: [egui::Pos2; 4]| {
        CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, Stroke::NONE)
            .flatten(Some(0.1))
    };
    match choice {
        ThemeChoice::System => {
            painter.circle_stroke(center, 7.0, Stroke::new(1.5_f32, ink));
            // Left half-disc, curved with a cubic Bézier instead of a font glyph.
            let mut points = curve([
                point(0.0, -6.0),
                point(-8.0, -6.0),
                point(-8.0, 6.0),
                point(0.0, 6.0),
            ]);
            points.push(center);
            painter.add(egui::Shape::convex_polygon(points, ink, Stroke::NONE));
        }
        ThemeChoice::Dark => {
            // A filled crescent made from two cubic Bézier arcs. In particular,
            // this does not paint a background-colored circle over adjacent UI.
            let mut points = curve([
                point(3.0, -7.0),
                point(-9.0, -9.0),
                point(-9.0, 9.0),
                point(3.0, 7.0),
            ]);
            points.extend(curve([
                point(3.0, 7.0),
                point(-2.0, 4.0),
                point(-2.0, -4.0),
                point(3.0, -7.0),
            ]));
            painter.add(egui::Shape::Path(egui::epaint::PathShape {
                points,
                closed: true,
                fill: ink,
                stroke: Stroke::NONE,
            }));
        }
        ThemeChoice::Light => {
            painter.circle_filled(center, 4.0, ink);
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::TAU / 8.0;
                let v = Vec2::new(angle.cos(), angle.sin());
                painter.line_segment(
                    [center + v * 6.0, center + v * 8.0],
                    Stroke::new(1.5_f32, ink),
                );
            }
        }
    }
}
