//! コマンドパレット（§9.7.1）の純粋な部分: 先頭文字によるモードと、あいまい一致の採点。
//! **egui に依存しない**。
//!
//! 採点（決定論・固定）: `+10` 連続一致 / `+6` 語境界一致 / `+3` 先頭一致 / `-1` スキップ文字。
//! 同点は ID（候補の鍵）の昇順で安定ソート。候補は最大 50 件。

/// 候補の上限（§9.7.1）。
pub const LIMIT: usize = 50;

/// 先頭文字で決まるモード（§9.7.1 の表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// （なし）ファイル・場面・節のあいまい検索。
    Fuzzy,
    /// `>` コマンド。
    Command,
    /// `@` SceneId ジャンプ。
    Scene,
    /// `'` 辞書（キャラ・用語）。
    Dictionary,
    /// `M` / `#` 資料検索。
    Material,
    /// `?` ヘルプ。
    Help,
}

/// 入力をモードと残りに分ける。
pub fn split(input: &str) -> (Mode, &str) {
    let mut chars = input.chars();
    match chars.next() {
        Some('>') => (Mode::Command, chars.as_str().trim()),
        Some('@') => (Mode::Scene, chars.as_str().trim()),
        Some('\'') => (Mode::Dictionary, chars.as_str().trim()),
        Some('#') => (Mode::Material, chars.as_str().trim()),
        // `M` の後が数字のときだけ資料（`M0137`）。`Meiryo` のような語は通常の検索
        Some('M') if chars.clone().next().is_some_and(|c| c.is_ascii_digit()) => {
            (Mode::Material, input.trim())
        }
        Some('?') => (Mode::Help, chars.as_str().trim()),
        _ => (Mode::Fuzzy, input.trim()),
    }
}

/// `query` の文字を順に `candidate` の中に探し、採点する。全部見つからなければ `None`。
///
/// 大文字小文字は区別しない。区切り（`-` `/` `_` `.` 空白 `›`）は query 側では読み飛ばす
/// （`0104` で `01-04` に当たる）。
pub fn score(query: &str, candidate: &str) -> Option<i32> {
    let wanted: Vec<char> = query
        .chars()
        .filter(|c| !is_separator(*c))
        .flat_map(char::to_lowercase)
        .collect();
    if wanted.is_empty() {
        return Some(0);
    }
    let text: Vec<char> = candidate.chars().flat_map(char::to_lowercase).collect();
    let mut total = 0;
    let mut at = 0;
    let mut previous: Option<usize> = None;
    for want in wanted {
        let found = (at..text.len()).find(|&index| text[index] == want)?;
        // 区切りは減点しない（`01-04` の `-` を飛ばしても連続とみなす）
        let skipped = (at..found)
            .filter(|&index| !is_separator(text[index]))
            .count();
        total -= skipped as i32;
        let consecutive =
            previous.is_some_and(|last| (last + 1..found).all(|index| is_separator(text[index])));
        if consecutive {
            total += 10;
        }
        if found == 0 {
            total += 3;
        } else if is_separator(text[found - 1]) {
            total += 6;
        }
        previous = Some(found);
        at = found + 1;
    }
    Some(total)
}

fn is_separator(c: char) -> bool {
    matches!(c, '-' | '/' | '_' | '.' | ' ' | '›' | '@' | '　')
}

/// 候補 `(鍵, 表示)` を採点して並べる（高い順・同点は鍵の昇順・最大 [`LIMIT`] 件）。
pub fn rank<'a, T>(
    query: &str,
    candidates: &'a [T],
    text: impl Fn(&T) -> (&str, &str),
) -> Vec<&'a T> {
    let mut scored: Vec<(i32, &str, &T)> = candidates
        .iter()
        .filter_map(|item| {
            let (key, shown) = text(item);
            score(query, shown).map(|points| (points, key, item))
        })
        .collect();
    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(right.1)));
    scored
        .into_iter()
        .take(LIMIT)
        .map(|(_, _, item)| item)
        .collect()
}
