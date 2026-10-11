//! 役割語パターン（§6.5.1 / §6.5.2）。
//!
//! **マッチャ化は M3**（`resolve/role_language.rs`）。ここはパターンの文字列と、
//! §6.5.2 のアンカー規則（固定）だけを持つ。正規表現には変換しない。

use serde::{Deserialize, Serialize};

/// 完全一致の前置き（§6.5.2「`=` 前置 → 完全一致」）。
pub const EXACT_PREFIX: char = '=';

/// 0 文字以上の任意文字列（§6.5.1 のアトム）。
pub const WILDCARDS: &[char] = &['*', '〜'];

/// 照合のアンカー（§6.5.2 の表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Anchor {
    /// **文末一致**（`$`）。先頭がワイルドカードで末尾がリテラル、またはワイルドカード無し。
    SentenceEnd,
    /// **文頭一致**（`^`）。先頭がリテラルで末尾がワイルドカード。
    SentenceStart,
    /// 位置自由。両端がワイルドカード。
    Anywhere,
    /// **完全一致**（発話全体が一致）。`=` 前置。
    Exact,
}

/// 1 パターン（§6.5.1）。`「` `」` の**内側**の文字列を持つ。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RolePattern {
    /// 書かれたままのパターン（鉤括弧を除く）。`=` 前置も**含めたまま**保持する。
    pub source: String,
}

impl RolePattern {
    /// `source` から作る。
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }

    /// `=` 前置（§6.5.2「完全一致」）か。
    pub fn is_exact(&self) -> bool {
        self.source.starts_with(EXACT_PREFIX)
    }

    /// `=` を除いた本体。
    pub fn body(&self) -> &str {
        self.source
            .strip_prefix(EXACT_PREFIX)
            .unwrap_or(&self.source)
    }

    /// §6.5.2 のアンカー（固定規則）。
    ///
    /// 先頭・末尾がワイルドカード（`*` / `〜`）かどうかだけで決まる。`…` や `?` や語クラス
    /// `(体言)` はワイルドカードではない（§6.5.1 のアトムは 5 種あるが、アンカーを見るのは
    /// `*` / `〜` の 2 種だけ）。
    pub fn anchor(&self) -> Anchor {
        if self.is_exact() {
            return Anchor::Exact;
        }
        let body = self.body();
        let head = body.chars().next().is_some_and(is_wildcard);
        let tail = body.chars().next_back().is_some_and(is_wildcard);
        match (head, tail) {
            (true, true) => Anchor::Anywhere,
            // 末尾がワイルドカードなら文頭一致。それ以外（両方リテラルを含む）は文末一致が既定
            (false, true) => Anchor::SentenceStart,
            (true, false) | (false, false) => Anchor::SentenceEnd,
        }
    }
}

/// §6.5.1 のワイルドカード（`*` / `〜`）か。
fn is_wildcard(c: char) -> bool {
    WILDCARDS.contains(&c)
}
