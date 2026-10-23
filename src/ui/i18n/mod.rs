//! 文言カタログ（§12.3）と言語の決定（§12.4）。GUI と CLI で共有する（§12.2「CLI のメッセージも
//! 同じカタログを使う」）。
//!
//! - `ja.toml` / `en.toml` を `include_str!` で埋め込み、言語ごとに**初回に 1 度だけ**平らな表
//!   （`diag.E004` → 文言）にする。
//! - プレースホルダは `{0}` `{1}` の位置引数だけ（§12.3）。[`fill`] が埋める。
//! - 欠けたキーは `en` → キーそのもの、の順に落とし、**デバッグビルドでは panic** する（§12.3
//!   「`debug_assert!` で落とす」）。キーの揃いは `tests/i18n_catalog.rs` が CI で見ている。
//! - 言語（§12.4）: `.novnow/ui.json` の `language` → `NOVNOW_LANG` → OS / ブラウザのロケール →
//!   既定 `ja`（[`Locale::detect`]）。CLI は `ui.json` を読まない（§14.2.3、`docs/history.md` B-101）。
//!
//! **egui に依存しない**ので `ui` feature なしでもビルドする（`docs/history.md` B-100。
//! `tests/layering.rs` が検査する）。

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::core::diagnostics::{Arg, Diagnostic};
use crate::core::features::place::Place;
use crate::ports::env::Env;

/// 日本語のカタログ（正本）。
pub const JA_TOML: &str = include_str!("ja.toml");

/// 英語のカタログ。
pub const EN_TOML: &str = include_str!("en.toml");

/// 言語を上書きする環境変数（§12.4 の 2。CLI でも GUI でも有効）。
pub const LANG_VAR: &str = "NOVNOW_LANG";

// ============================================================ 言語

/// UI の言語（§12.1。2 言語を同等に扱う）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    /// 日本語（既定）。
    Ja,
    /// English。
    En,
}

impl Locale {
    /// すべての言語。
    pub const ALL: [Locale; 2] = [Locale::Ja, Locale::En];

    /// `ja` / `en`。
    pub fn code(self) -> &'static str {
        match self {
            Self::Ja => "ja",
            Self::En => "en",
        }
    }

    /// §12.4 の順で決める。`setting` は `.novnow/ui.json` の `language`（CLI は `None`）。
    ///
    /// 1. `setting` が `ja` / `en`（`system` などはここを飛ばす）
    /// 2. `NOVNOW_LANG` が `ja…` / `en…`（それ以外の値は無視して次へ）
    /// 3. OS / ブラウザのロケール（[`Env::locale`]）: `ja…` → ja、それ以外（`C` を含む）→ en
    /// 4. 既定 ja
    pub fn detect(setting: Option<&str>, env: &(impl Env + ?Sized)) -> Self {
        if let Some(locale) = setting.and_then(Self::from_setting) {
            return locale;
        }
        if let Some(locale) = env.var(LANG_VAR).as_deref().and_then(Self::from_override) {
            return locale;
        }
        if let Some(value) = env.locale() {
            return Self::from_os_locale(&value);
        }
        Self::Ja
    }

    /// `ui.json` の `language` の値（`ja` / `en` だけ。§4.4.1）。
    pub fn from_setting(value: &str) -> Option<Self> {
        match value {
            "ja" => Some(Self::Ja),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    /// `NOVNOW_LANG` の値（`ja` / `en` で始まるもの。`ja_JP.UTF-8` も可）。
    pub fn from_override(value: &str) -> Option<Self> {
        let lower = value.trim().to_ascii_lowercase();
        if lower.starts_with("ja") {
            Some(Self::Ja)
        } else if lower.starts_with("en") {
            Some(Self::En)
        } else {
            None
        }
    }

    /// OS / ブラウザのロケール（§12.4 の 3: `ja*` → ja、それ以外 → en）。
    pub fn from_os_locale(value: &str) -> Self {
        if value.trim().to_ascii_lowercase().starts_with("ja") {
            Self::Ja
        } else {
            Self::En
        }
    }
}

// ============================================================ カタログ

/// 1 言語分のカタログ（ドット区切りのキー → 文言）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    entries: HashMap<String, String>,
}

impl Catalog {
    /// TOML を読んで平らにする（`[cli.error]` の `incomplete` が `cli.error.incomplete`）。
    /// 文字列でない値があれば `Err`（キーの名前つき）。
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|error| format!("{error}"))?;
        let mut entries = HashMap::new();
        flatten("", &toml::Value::Table(table), &mut entries)?;
        Ok(Self { entries })
    }

    /// 文言。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// キーの一覧（順不同）。
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

fn flatten(
    prefix: &str,
    value: &toml::Value,
    out: &mut HashMap<String, String>,
) -> Result<(), String> {
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&path, child, out)?;
            }
            Ok(())
        }
        toml::Value::String(text) => {
            out.insert(prefix.to_owned(), text.clone());
            Ok(())
        }
        _ => Err(format!("{prefix} is not a string")),
    }
}

/// 言語のカタログ（初回に 1 度だけ読む）。埋め込みのカタログが壊れていたら、デバッグビルドは
/// panic、リリースビルドは空のカタログ（＝キーがそのまま出る）。
pub fn catalog(locale: Locale) -> &'static Catalog {
    static JA: OnceLock<Catalog> = OnceLock::new();
    static EN: OnceLock<Catalog> = OnceLock::new();
    let (cell, text) = match locale {
        Locale::Ja => (&JA, JA_TOML),
        Locale::En => (&EN, EN_TOML),
    };
    cell.get_or_init(|| match Catalog::parse(text) {
        Ok(catalog) => catalog,
        Err(error) => {
            if cfg!(debug_assertions) {
                panic!("ui/i18n/{}.toml: {error}", locale.code());
            }
            Catalog::default()
        }
    })
}

/// 文言。無ければ `en` → キーそのもの（§12.3。デバッグビルドでは panic）。
pub fn text(locale: Locale, key: &str) -> &str {
    if let Some(found) = catalog(locale).get(key) {
        return found;
    }
    if cfg!(debug_assertions) {
        panic!(
            "ui/i18n: the key {key} is missing in {}.toml",
            locale.code()
        );
    }
    catalog(Locale::En).get(key).unwrap_or(key)
}

/// 文言にプレースホルダの値を埋める。
pub fn format(locale: Locale, key: &str, args: &[&str]) -> String {
    fill(text(locale, key), args)
}

/// 診断の文言（`diag.<コード>` に [`Diagnostic::args`] を埋める）。
pub fn diagnostic(locale: Locale, found: &Diagnostic) -> String {
    let key = format!("diag.{}", found.code.as_str());
    let args: Vec<String> = found
        .args
        .iter()
        .map(|arg| match arg {
            Arg::Text(text) => text.clone(),
            Arg::Number(number) => number.to_string(),
        })
        .collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    fill(text(locale, &key), &args)
}

/// 診断の「どこ」（`core::features::place`）を作家の言葉にする（`place.*`。§14.2.5・§10.9.2、
/// 第 2.11 版）。`novnow check` の `[…]` と、GUI の診断タブの「位置」が同じものを使う。
pub fn place(locale: Locale, found: &Place) -> String {
    match found {
        Place::Contents(id) => format(locale, "place.contents", &[id]),
        Place::EpisodePlot(id) => format(locale, "place.episode_plot", &[id]),
        Place::ChapterPlot(id) if id.is_empty() => {
            text(locale, "place.chapter_plot_head").to_owned()
        }
        Place::ChapterPlot(id) => format(locale, "place.chapter_plot", &[id]),
        Place::WholePlot(Some(id)) => format(locale, "place.whole_plot", &[id]),
        Place::WholePlot(None) => text(locale, "place.whole_plot_head").to_owned(),
        Place::Character { name, mode: None } => format(locale, "place.character", &[name]),
        Place::Character {
            name,
            mode: Some(mode),
        } => format(locale, "place.character_mode", &[name, mode]),
        Place::Word(term) => format(locale, "place.word", &[term]),
        Place::Idea { id, title } => format(locale, "place.idea", &[id, title]),
        Place::Material { id, title } if title.is_empty() => {
            format(locale, "place.material_id", &[id])
        }
        Place::Material { id, title } => format(locale, "place.material", &[id, title]),
        Place::Prompt {
            role,
            section: None,
        } => format(locale, "place.prompt", &[role]),
        Place::Prompt {
            role,
            section: Some(section),
        } => format(locale, "place.prompt_section", &[role, section]),
        Place::Settings => text(locale, "place.settings").to_owned(),
    }
}

/// `{0}` `{1}` … を `args` で埋める（1 回だけ走査するので、値の中の `{0}` は埋め直さない）。
/// 範囲外の番号や数でない `{…}` はそのまま残す（カタログの検査で起きないようにしてある）。
pub fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let inside = &after[..close];
        match inside
            .parse::<usize>()
            .ok()
            .and_then(|index| args.get(index))
        {
            Some(arg) => out.push_str(arg),
            None => {
                out.push('{');
                out.push_str(inside);
                out.push('}');
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}
