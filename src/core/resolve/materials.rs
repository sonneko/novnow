//! 資料 ID の解決とインレイヒント（§6.6、指示 9）。
//!
//! # 参照の 3 経路（§6.6.1）
//!
//! | 書き方 | 経路 | 振る舞い |
//! |---|---|---|
//! | `'M0137` | [`Route::Id`] | ID のみ。タイトルは**インレイヒント**（ファイルには書かれない） |
//! | `'M0137:科学の甲子園 本戦の運営と実技課題` | [`Route::IdWithTitle`] | タイトル明示。index と照合し、不一致は **E006** |
//! | `'科学の甲子園 本戦の運営と実技課題` | [`Route::Title`] | タイトル直接参照（辞書の `MaterialTitle`）。**ID へ正規化するクイックフィックス**を出す |
//!
//! どれの `'` トークンも M3 の[`辞書`](crate::core::resolve::dictionary)が既に引いていて
//! （§6.1.1 の `Material` / `MaterialTitle`、優先順位は `Character > Word > Material >
//! MaterialTitle`）、ここは**その結果を `materials/index.json` の実体に当てる**だけ。
//! 未解決（**E001**）・閉じない明示形（**E002**）・付けられない修飾子（**E026**）・
//! より長い候補（**W029**）は [`dictionary::token_diagnostics`] の担当なのでここでは出さない。
//!
//! # 診断（§6.6.3 / §6.6.4）
//!
//! | コード | 条件 | 位置 |
//! |---|---|---|
//! | **E006** | 明示タイトルが index の `title` と**正規化比較（§6.1.3）で不一致** | `modifier.text_range`（タイトル部分だけ） |
//! | **E007** | 存在しない ID | 参照トークン全体 |
//! | **W031** | 本文（C 形式）に書かれた資料参照 | 参照トークン全体 |
//!
//! E007 には**補完候補を 5 件**付ける（§6.6.3「近い ID / タイトルを 5 件提示」）。
//! 近さは正規化後のレーベンシュタイン距離（[`distance`]）で、ID とタイトルの**近いほう**を使う。
//!
//! # インレイヒント（§6.6.2）
//!
//! 表示位置は**トークン直後**、内容は `⟨タイトル⟩`。24 字を超える場合は **23 字 + `…`**。
//! `status` が `proposed` なら `⟨タイトル・提案中⟩`、`rejected` なら `⟨タイトル・却下⟩`。
//!
//! ただし**括弧と status の添え文言は core が作らない**（§12.2「core にユーザー向け文言を
//! 置かない」。`en` では `⟨title · proposed⟩` になるため）。ここが返す [`Hint`] は
//! 「どの位置に・何字で・どの status か」だけで、文言は `ui/i18n`（§12.3）が組む。
//! 文字色・フォントサイズ・**選択やコピーの対象にしないこと**・`Alt+H` のトグル・
//! エクスポートや読者モードで出さないこと（すべて §6.6.2 の MUST）は UI 側の責任。
//!
//! # クイックフィックス（§6.6.3 の 3 種）
//!
//! [`TitleFixes`] が**中身だけ**を返す（文言は同上）。1 が既定（`Enter` 一発）。

use std::ops::Range;

use crate::core::diagnostics::{Arg, Diagnostic, DiagnosticCode};
use crate::core::model::material::{Material, MaterialId, MaterialStatus};
use crate::core::resolve::dictionary::{
    self, normalize, Dictionary, EntryKind, RefToken, Target, REF_SIGIL,
};
use crate::core::syntax::materials_index::Index;

/// §6.6.3 の補完候補の件数（「近い ID / タイトルを **5 件**提示」）。
pub const SUGGESTION_LIMIT: usize = 5;

/// §6.6.2 のインレイヒントの上限（「**24 字**を超える場合は…」）。
pub const HINT_MAX_CHARS: usize = 24;

/// 上限を超えたときに残す字（「…**23 字** + `…`」）。
pub const HINT_KEEP_CHARS: usize = 23;

/// 切り詰めのしるし（`…`。§6.6.2）。
pub const HINT_ELLIPSIS: char = '…';

/// §6.6.1 の 3 経路。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    /// `'M0137`（ID のみ）。タイトルはインレイヒントで表示する。
    Id,
    /// `'M0137:タイトル`（タイトル明示。index と照合する）。
    IdWithTitle,
    /// `'タイトル`（タイトル直接参照。辞書の `MaterialTitle`）。
    Title,
}

/// §6.6.2 のインレイヒント（**中身だけ**。文言は UI/i18n が組む）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    /// 表示位置（**トークン直後**の byte 位置、ファイル単位）。
    pub at: usize,
    /// 表示するタイトル（§6.6.2 の 24 字ルールで切り詰めたあと）。
    pub title: String,
    /// 切り詰めたか（`true` なら末尾は [`HINT_ELLIPSIS`]）。
    pub truncated: bool,
    /// §5.8.2 の `status`。表に無い値なら `None`（添え文言を付けない）。
    ///
    /// `Proposed` / `Rejected` のときだけ UI が添え文言を付ける（§6.6.2）。
    pub status: Option<MaterialStatus>,
}

/// §6.6.3 のクイックフィックス 3 種（**中身だけ**。文言は UI/i18n が組む）。
///
/// 文字列は**所有する**（タイトルは §5.8.2 で 1〜80 字と短いので複製のコストは無視でき、
/// 借用だと「参照トークン」と「index」の 2 つの生存期間を 1 本のライフタイムに縛ってしまう）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleFixes {
    /// 1（**既定**・`Enter` 一発で適用）: 明示タイトルを **index のタイトルで置換**する。
    pub replace_title: String,
    /// 2: **index 側**のタイトルをファイルの記述に合わせる（`index.json` を書き換える）。
    pub rewrite_index: String,
    /// 3: 書かれたタイトルを持つ**別 ID** があればその ID（無ければ `None`）。
    pub other_id: Option<MaterialId>,
}

/// 1 つの `'` 参照の解決結果（§6.6）。
#[derive(Debug, Clone)]
pub struct Resolved<'a> {
    /// §6.6.1 のどの経路か。資料を指していないトークンなら `None`。
    pub route: Option<Route>,
    /// 辞書が指した ID（**index に実体が無くても入る**。E007 の引数と同じ）。
    pub id: Option<String>,
    /// `materials/index.json` の実体。無ければ `None`（**E007**）。
    pub material: Option<&'a Material>,
    /// §6.6.2 のインレイヒント。実体が無ければ `None`。
    pub hint: Option<Hint>,
    /// §6.6.3 のクイックフィックス。**E006** のときだけ入る。
    pub fixes: Option<TitleFixes>,
    /// §6.6.1 の 3 行目（タイトル直接参照）の**ID へ正規化するクイックフィックス**。
    ///
    /// 中身は置換後のトークン全体（`'M0137`）。タイトルはインレイヒントで出るので
    /// 毎回書く必要はない、という提案（**E006 と違い診断は出ない**）。
    pub normalize_to_id: Option<String>,
    /// §6.6.3 の補完候補（近い順に最大 [`SUGGESTION_LIMIT`] 件）。**E007** のときだけ入る。
    pub suggestions: Vec<&'a Material>,
    /// 診断（**E006** / **E007**）。範囲は**ファイル単位**の byte。
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> Resolved<'a> {
    /// 空の結果（資料を指していないトークン用）。
    pub fn empty() -> Self {
        Self {
            route: None,
            id: None,
            material: None,
            hint: None,
            fixes: None,
            normalize_to_id: None,
            suggestions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// `materials/index.json` に実体があったか。
    pub fn is_resolved(&self) -> bool {
        self.material.is_some()
    }
}

/// 1 行の走査結果（[`scan_line`]）。
#[derive(Debug, Clone)]
pub struct Scan<'a> {
    /// 資料を指していたトークン 1 件ごとの解決結果（行内の順）。
    pub resolutions: Vec<Resolved<'a>>,
    /// 診断（**E006** / **E007** と、`in_body` のときの **W031**）。行内の順。
    pub diagnostics: Vec<Diagnostic>,
}

/// 参照トークンが指す資料 ID（辞書の `Material` / `MaterialTitle` の両方）。
///
/// 資料を指していなければ `None`。**index に実体があるかは見ない**（それは [`resolve`]）。
pub fn material_id_of<'a>(token: &RefToken, dictionary: &'a Dictionary) -> Option<&'a str> {
    let entry = dictionary.entry(token.resolved?)?;
    match &entry.target {
        Target::Material { id } => Some(id.as_str()),
        _ => None,
    }
}

/// §6.6.1 のどの経路で引いたか。資料を指していなければ `None`。
pub fn route_of(token: &RefToken, dictionary: &Dictionary) -> Option<Route> {
    let entry = dictionary.entry(token.resolved?)?;
    let Target::Material { .. } = &entry.target else {
        return None;
    };
    if entry.kind == EntryKind::MaterialTitle {
        return Some(Route::Title);
    }
    Some(match &token.modifier {
        Some(modifier) if modifier.is_title() => Route::IdWithTitle,
        _ => Route::Id,
    })
}

/// 参照トークン 1 件を解決する（§6.6.1〜§6.6.3）。
///
/// `line` はその行の原文（[`RefToken::key`] が名前部を切り出すのに要る）、`line_start` は
/// その行の先頭の byte 位置（辞書のトークンは**行内相対**なので、診断とインレイヒントの位置を
/// **ファイル単位**にするのに使う。`PLANS.md` §9.2 B-56）。
///
/// 辞書に無いトークンのうち**資料 ID の形**（`M0137`）をしたものには §6.6.3 の **E007** を出す
/// （`'しらぬ人` のように ID の形でないものは **E001** のまま＝辞書の担当）。
pub fn resolve<'a>(
    line: &str,
    token: &RefToken,
    dictionary: &Dictionary,
    index: &'a Index<'_>,
    line_start: usize,
) -> Resolved<'a> {
    let mut out = Resolved::empty();
    let Some(route) = route_of(token, dictionary) else {
        return unknown(line, token, index, line_start, out);
    };
    // `route_of` が `Some` なら ID も必ずある
    let Some(id) = material_id_of(token, dictionary) else {
        return out;
    };
    out.route = Some(route);
    out.id = Some(id.to_string());

    let Some(material) = index.find(id) else {
        // 辞書は index から作るので通常はここに来ない（index を差し替えた直後など）。
        // §6.6.3「存在しない ID は **E007**（補完候補として近い ID / タイトルを 5 件提示）」
        out.diagnostics.push(e007(token, line_start, id));
        out.suggestions = suggestions(index, id, SUGGESTION_LIMIT);
        return out;
    };
    out.material = Some(material);
    out.hint = Some(hint(material, line_start + token.range.end));
    if route == Route::Title {
        // §6.6.1「ID へ正規化するクイックフィックスを提供」
        out.normalize_to_id = Some(format!("{REF_SIGIL}{}", material.id));
    }

    // §6.6.3「明示タイトルが index の `title` と**正規化比較（§6.1.3）で不一致**のとき E006」
    let Some(modifier) = token.modifier.as_ref().filter(|found| found.is_title()) else {
        return out;
    };
    if same_title(&modifier.text, &material.title) {
        return out;
    }
    out.diagnostics.push(Diagnostic::new(
        DiagnosticCode::E006,
        // 波線は `modifier.title` の範囲（§6.6.3）
        Some(shift(&modifier.text_range, line_start)),
        vec![Arg::text(material.id.clone())],
    ));
    out.fixes = Some(title_fixes(index, &modifier.text, material));
    out
}

/// 辞書に無いトークンの処理（**資料 ID の形**なら E007、そうでなければ何もしない）。
fn unknown<'a>(
    line: &str,
    token: &RefToken,
    index: &'a Index<'_>,
    line_start: usize,
    mut out: Resolved<'a>,
) -> Resolved<'a> {
    let Some(id) = unknown_material_id(line, token) else {
        return out;
    };
    // 明示形が閉じていない（E002）や長さ違い（W029）は辞書の診断と両立するので触らない
    out.id = Some(id.clone());
    out.diagnostics.push(e007(token, line_start, &id));
    out.suggestions = suggestions(index, &id, SUGGESTION_LIMIT);
    out
}

/// 未解決トークンが**資料 ID の形**（§5.8.2 `M` / `S` + 4 桁以上）ならその ID。
///
/// 辞書は index から作るので、形が合っていて辞書に無い＝**index に無い ID**（§6.6.3 の E007）。
/// 形が違えば `'しらぬ人` と同じ未解決参照なので **E001**（辞書の担当）に任せる。
pub fn unknown_material_id(line: &str, token: &RefToken) -> Option<String> {
    if token.is_resolved() {
        return None;
    }
    let key = token.key(line);
    MaterialId::parse(key)?;
    Some(key.to_string())
}

/// §6.6.3 の **E007**（「資料 {0} は存在しません」）。範囲は参照トークン全体。
fn e007(token: &RefToken, line_start: usize, id: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::E007,
        Some(shift(&token.range, line_start)),
        vec![Arg::text(id)],
    )
}

/// §6.6.2 のインレイヒント（**中身だけ**。括弧と status の添え文言は UI が組む）。
///
/// `at` はトークン直後の byte 位置（ファイル単位）。
pub fn hint(material: &Material, at: usize) -> Hint {
    let (title, truncated) = hint_title(&material.title);
    Hint {
        at,
        title,
        truncated,
        status: material.status_kind(),
    }
}

/// §6.6.2 の「24 字を超える場合は 23 字 + `…`」。`(表示する文字列, 切り詰めたか)`。
///
/// **字**は Unicode の文字数で数える（byte 数ではない。CJK は 1 字 3 byte なので
/// byte で数えると 8 字で切れてしまう）。
pub fn hint_title(title: &str) -> (String, bool) {
    if title.chars().count() <= HINT_MAX_CHARS {
        return (title.to_string(), false);
    }
    let mut out: String = title.chars().take(HINT_KEEP_CHARS).collect();
    out.push(HINT_ELLIPSIS);
    (out, true)
}

/// タイトルの照合（§6.6.3「**正規化比較（§6.1.3）**」）。
///
/// 正規化は長音統一 → NFKC → 小文字化 → 空白除去（[`dictionary::normalize`]）なので、
/// `'M0137:科学の甲子園　本戦の運営と実技課題`（全角空白）は一致する。
pub fn same_title(written: &str, in_index: &str) -> bool {
    normalize(written) == normalize(in_index)
}

/// §6.6.3 のクイックフィックス 3 種（**中身だけ**）。
///
/// 3 番目は「**同名タイトルを持つ別 ID** が存在する場合」だけ（§6.6.3）。照合は
/// [`same_title`] と同じ正規化で、`Index::find_by_title`（完全一致）ではない。
pub fn title_fixes(index: &Index<'_>, written: &str, material: &Material) -> TitleFixes {
    let other_id = items(index)
        .iter()
        .find(|found| found.id != material.id && same_title(written, &found.title))
        .and_then(|found| found.material_id());
    TitleFixes {
        replace_title: material.title.clone(),
        rewrite_index: written.to_string(),
        other_id,
    }
}

/// §6.6.3 の補完候補（「近い ID / タイトルを 5 件提示」）。近い順・最大 `limit` 件。
///
/// 近さは**正規化後**のレーベンシュタイン距離（[`distance`]）で、ID とタイトルの
/// **近いほう**をその資料の距離とする。同じ距離なら ID の辞書順（決定的にするため）。
pub fn suggestions<'a>(index: &'a Index<'_>, query: &str, limit: usize) -> Vec<&'a Material> {
    let query = normalize(query);
    let mut scored: Vec<(u32, &str, &Material)> = items(index)
        .iter()
        .map(|found| {
            let by_id = distance(&query, &normalize(&found.id));
            let by_title = distance(&query, &normalize(&found.title));
            (by_id.min(by_title), found.id.as_str(), found)
        })
        .collect();
    scored.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(right.1)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, _, found)| found)
        .collect()
}

/// レーベンシュタイン距離（挿入・削除・置換の最少回数）。
///
/// 正規化済みの文字列に使う（[`suggestions`]）。資料は数百件までなので
/// O(文字数²) の素直な DP で足りる（§18.1 の予算は参照解決のほうが支配的）。
pub fn distance(left: &str, right: &str) -> u32 {
    let right_chars: Vec<char> = right.chars().collect();
    let mut previous: Vec<u32> = (0..=right_chars.len() as u32).collect();
    let mut current = vec![0u32; right_chars.len() + 1];
    for (at, l) in left.chars().enumerate() {
        current[0] = at as u32 + 1;
        for (other, r) in right_chars.iter().enumerate() {
            let cost = u32::from(l != *r);
            current[other + 1] = (previous[other] + cost)
                .min(previous[other + 1] + 1)
                .min(current[other] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right_chars.len()]
}

/// §6.6.4 の **W031**（「資料参照はプロットに書くことを推奨します」）。
///
/// 資料 ID は**プロットに書く**もの（指示 9）。本文（C 形式）にも書けるが、エクスポート時に
/// 消える前提の注釈なので促すだけ（**W** であって E ではない＝保存も実行もできる）。
/// **ファイルが本文かどうかは呼び出し側が知る**（core は I/O をしない、§3.2）ので、
/// 範囲だけもらって診断を作る。
pub fn body_diagnostic(range: Range<usize>) -> Diagnostic {
    Diagnostic::simple(DiagnosticCode::W031, Some(range))
}

/// 1 行を走査して、**辞書と資料の両方の診断**を返す（§6.2 / §6.6）。
///
/// M4 が行単位で診断を組み立てるときの入口。`dictionary::scan_line` と
/// `dictionary::token_diagnostics` と `resolve` を**ここで一度に**呼ぶので、
/// **E001（未解決）と E007（未知の資料 ID）が 2 重に出ない**ようになっている
/// （資料 ID の形をした未解決トークンは E007 だけ。`PLANS.md` §9.2 **B-71**）。
///
/// `in_body` が `true`（C 形式の本文）なら、資料参照のトークンごとに §6.6.4 の **W031** を足す。
/// **ファイルが本文かどうかは呼び出し側が知る**（core は I/O をしない、§3.2）。
pub fn scan_line<'a>(
    line: &str,
    line_start: usize,
    dictionary: &Dictionary,
    index: &'a Index<'_>,
    in_body: bool,
) -> Scan<'a> {
    let scanned = dictionary::scan_line(line, line_start, dictionary);
    let mut out = Scan {
        resolutions: Vec::new(),
        diagnostics: Vec::new(),
    };
    for token in &scanned.tokens {
        let found = resolve(line, token, dictionary, index, line_start);
        let is_material = found.route.is_some() || found.id.is_some();
        if !is_material {
            // 資料と関係のないトークンは辞書の診断だけ（E001 / E002 / E026 / W029）
            out.diagnostics
                .extend(dictionary::token_diagnostics(line, token, dictionary));
            continue;
        }
        if in_body {
            // §6.6.4「本文にも書けるが、エクスポート時に消える前提の注釈なので W031 で促す」
            out.diagnostics
                .push(body_diagnostic(shift(&token.range, line_start)));
        }
        let mut from_dictionary = dictionary::token_diagnostics(line, token, dictionary);
        if found
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::E007)
        {
            // E007 が出るトークンに E001 を重ねない（B-71）
            from_dictionary.retain(|diagnostic| diagnostic.code != DiagnosticCode::E001);
        }
        out.diagnostics.extend(from_dictionary);
        out.diagnostics.extend(found.diagnostics.iter().cloned());
        out.resolutions.push(found);
    }
    out
}

/// `materials/index.json` の資料（**書かれた順**）。JSON が壊れていれば空（§4.3-4・**E025**）。
fn items<'a>(index: &'a Index<'_>) -> &'a [Material] {
    index
        .index
        .as_ref()
        .map(|found| found.items.as_slice())
        .unwrap_or(&[])
}

/// 行内相対の範囲をファイル単位の範囲にする。
fn shift(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start + by..range.end + by
}
