//! `.novnow/ui.json`（§4.4.1）の読み書き。**egui に依存しない**。
//!
//! - 無い・壊れている → 既定値で起動し、エラーを出さない（MUST）。値ごとに読めなければ既定値。
//! - 書き出しは**知らないキーを残す**（読んだ JSON の上に、知っているキーだけを上書きする）。
//! - 書き込み先はプロジェクトの中（規則 10、§11.4）。プロジェクトを開く前は何も保存しない。

use serde_json::{json, Map, Value};

/// `ui.json` のパス（プロジェクトルートからの相対）。
pub const UI_JSON: &str = ".novnow/ui.json";

/// テーマの設定値（§11.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    /// OS / ブラウザに従う（既定）。
    #[default]
    System,
    /// 暗い配色。
    Dark,
    /// 明るい配色。
    Light,
}

impl ThemeChoice {
    /// `system → dark → light → system`（§9.8.1 `Ctrl+Alt+T`）。
    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Dark,
            Self::Dark => Self::Light,
            Self::Light => Self::System,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "system" => Some(Self::System),
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }
}

/// 言語の設定値（§12.4 の 1。`system` は 2 以降に任せる）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageChoice {
    /// 環境変数 → OS のロケール → ja（§12.4 の 2〜4）。
    #[default]
    System,
    /// 日本語。
    Ja,
    /// English。
    En,
}

impl LanguageChoice {
    fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Ja => "ja",
            Self::En => "en",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "system" => Some(Self::System),
            "ja" => Some(Self::Ja),
            "en" => Some(Self::En),
            _ => None,
        }
    }
}

/// 参照ペインの左右（§10.0）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneSide {
    /// 右（既定）。
    #[default]
    Right,
    /// 左。
    Left,
}

/// `ui.json` の中身（アプリが読むキー）。
#[derive(Debug, Clone, PartialEq)]
pub struct UiSettings {
    /// `theme`。
    pub theme: ThemeChoice,
    /// `language`。
    pub language: LanguageChoice,
    /// `reference_pane_side`。
    pub reference_pane_side: PaneSide,
    /// `last_scene`（`01-04-02-03`）。
    pub last_scene: Option<String>,
    /// `pane_ratio.main`（0.3–0.85）。
    pub main_ratio: f32,
    /// `pane_ratio.context_view`（0.2–0.8）。
    pub context_ratio: f32,
    /// `font_scale`（0.8–1.6）。
    pub font_scale: f32,
    /// `max_line_chars`（20–120）。None は表示言語ごとの既定値。
    pub max_line_chars: Option<usize>,
    /// `context_sections`（1–9）。
    pub context_sections: usize,
    /// `autosave_ms`（200–2000。§8.2、第 2.12 版で §4.4.1 に足した）。
    pub autosave_ms: u32,
    /// `inlay_hints`（§6.6.2。`Alt+H` で切り替える。既定 `true`）。
    pub inlay_hints: bool,
    /// 参照ペインを表示するか（`Ctrl+\`。保存しない、セッション内）。
    pub reference_visible: bool,
    pub status_stats_mode: bool,
    pub suggest_comma: bool,
    /// `confirm_section_delete`（§4.4・§8.1.2。空でない節の削除前に確認するか。既定 `true`）。
    pub confirm_section_delete: bool,
    /// はじめての手引きを閉じたか（§9.10。`true` なら自動では出さない）。
    pub guide_done: bool,
    pub folded: Vec<String>,
    /// 読んだ JSON（知らないキーを残すため）。
    raw: Map<String, Value>,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::System,
            language: LanguageChoice::System,
            reference_pane_side: PaneSide::Right,
            last_scene: None,
            main_ratio: 0.62,
            context_ratio: 0.55,
            font_scale: 1.0,
            context_sections: 3,
            max_line_chars: None,
            autosave_ms: 400,
            inlay_hints: true,
            reference_visible: true,
            status_stats_mode: false,
            suggest_comma: false,
            confirm_section_delete: true,
            guide_done: false,
            folded: Vec::new(),
            raw: Map::new(),
        }
    }
}

impl UiSettings {
    /// `ui.json` の文字列から読む。壊れていれば既定値（MUST、エラーは出さない）。
    pub fn parse(text: &str) -> Self {
        let Ok(Value::Object(raw)) = serde_json::from_str::<Value>(text) else {
            return Self::default();
        };
        let mut out = Self::default();
        let str_of = |key: &str| raw.get(key).and_then(Value::as_str);
        if let Some(found) = str_of("theme").and_then(ThemeChoice::parse) {
            out.theme = found;
        }
        if let Some(found) = str_of("language").and_then(LanguageChoice::parse) {
            out.language = found;
        }
        if str_of("reference_pane_side") == Some("left") {
            out.reference_pane_side = PaneSide::Left;
        }
        out.last_scene = str_of("last_scene").map(str::to_owned);
        if let Some(ratio) = raw.get("pane_ratio").and_then(Value::as_object) {
            if let Some(main) = ratio.get("main").and_then(Value::as_f64) {
                out.main_ratio = clamp_ratio(main as f32, 0.3, 0.85, 0.62);
            }
            if let Some(context) = ratio.get("context_view").and_then(Value::as_f64) {
                out.context_ratio = clamp_ratio(context as f32, 0.2, 0.8, 0.55);
            }
        }
        if let Some(scale) = raw.get("font_scale").and_then(Value::as_f64) {
            out.font_scale = clamp_ratio(scale as f32, 0.8, 1.6, 1.0);
        }
        if let Some(chars) = raw.get("max_line_chars").and_then(Value::as_i64) {
            out.max_line_chars = Some(chars.clamp(20, 120) as usize);
        }
        if let Some(count) = raw.get("context_sections").and_then(Value::as_u64) {
            if (1..=9).contains(&count) {
                out.context_sections = count as usize;
            }
        }
        if let Some(ms) = raw.get("autosave_ms").and_then(Value::as_u64) {
            out.autosave_ms = ms.clamp(200, 2000) as u32;
        }
        if let Some(shown) = raw.get("inlay_hints").and_then(Value::as_bool) {
            out.inlay_hints = shown;
        }
        out.status_stats_mode = raw
            .get("status_stats_mode")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        out.suggest_comma = raw
            .get("suggest_comma")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        out.confirm_section_delete = raw
            .get("confirm_section_delete")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        out.guide_done = raw
            .get("guide_done")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        out.folded = raw
            .get("folded")
            .and_then(|v| v.get("context"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        out.raw = raw;
        out
    }

    /// 書き出す JSON（知らないキーは読んだまま残す。末尾に改行）。
    pub fn to_json(&self) -> String {
        let mut raw = self.raw.clone();
        raw.insert("schema_version".into(), json!(1));
        raw.insert("status_stats_mode".into(), json!(self.status_stats_mode));
        raw.insert("suggest_comma".into(), json!(self.suggest_comma));
        raw.insert(
            "confirm_section_delete".into(),
            json!(self.confirm_section_delete),
        );
        raw.insert("guide_done".into(), json!(self.guide_done));
        let mut folded = raw
            .get("folded")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        folded.insert("context".to_owned(), json!(self.folded));
        raw.insert("folded".into(), Value::Object(folded));
        raw.insert("theme".into(), json!(self.theme.key()));
        raw.insert("language".into(), json!(self.language.key()));
        raw.insert(
            "reference_pane_side".into(),
            json!(match self.reference_pane_side {
                PaneSide::Right => "right",
                PaneSide::Left => "left",
            }),
        );
        match &self.last_scene {
            Some(scene) => raw.insert("last_scene".into(), json!(scene)),
            None => raw.remove("last_scene"),
        };
        raw.insert(
            "pane_ratio".into(),
            json!({ "main": round2(self.main_ratio), "context_view": round2(self.context_ratio) }),
        );
        raw.insert("font_scale".into(), json!(round2(self.font_scale)));
        raw.insert("context_sections".into(), json!(self.context_sections));
        match self.max_line_chars {
            Some(chars) => {
                raw.insert("max_line_chars".into(), json!(chars.clamp(20, 120)));
            }
            None => {
                raw.remove("max_line_chars");
            }
        }
        raw.insert("autosave_ms".into(), json!(self.autosave_ms));
        raw.insert("inlay_hints".into(), json!(self.inlay_hints));
        let mut text = serde_json::to_string_pretty(&Value::Object(raw)).unwrap_or_default();
        text.push('\n');
        text
    }

    /// 表示言語に合わせた本文の折り返し文字数。
    pub fn line_chars(&self, english: bool) -> usize {
        self.max_line_chars.unwrap_or(if english { 70 } else { 44 })
    }

    /// 文字サイズを 1 段階変える（§9.8.1 `Ctrl+Plus` / `Ctrl+Minus`、0.1 刻み・0.8–1.6）。
    pub fn step_font(&mut self, up: bool) {
        let next = self.font_scale + if up { 0.1 } else { -0.1 };
        self.font_scale = (next * 10.0).round() / 10.0;
        self.font_scale = self.font_scale.clamp(0.8, 1.6);
    }

    /// 分割比を 5% 変える（§9.1 `Ctrl+Alt+←/→`・`Ctrl+Alt+↑/↓`）。
    pub fn step_ratio(&mut self, main: bool, grow: bool) {
        let delta = if grow { 0.05 } else { -0.05 };
        if main {
            self.main_ratio = clamp_ratio(round2(self.main_ratio + delta), 0.3, 0.85, 0.62);
        } else {
            self.context_ratio = clamp_ratio(round2(self.context_ratio + delta), 0.2, 0.8, 0.55);
        }
    }
}

fn clamp_ratio(value: f32, low: f32, high: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        fallback
    }
}

fn round2(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}
