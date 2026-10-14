//! 資料のワークスペース（§10.5 / §16 `material`）の操作: 提案の作成・登録・採用・却下・審査の
//! 書き戻し・保存時の `updated_at` / `checksum`・検索（`Ctrl+M`）・条件一致（§16.4）。
//!
//! LSP には対応しない補助（`PLANS.md` §9.2 B-131・B-134）。**文言は持たない**（§12.2）。どれも
//! 「`materials/index.json` の文字列 → 新しい文字列」の純粋関数で、書き込みは UI が行う。
//!
//! - ID は `next_id` からの単調増加（§5.8.2 MUST）。[`MaterialIndex::issue`] が `next_id` と既存の
//!   最大値の大きいほうから発行するので、`next_id` が壊れていても再利用しない。却下しても ID は
//!   回収しない（§16.1）。
//! - `items` の順は追加順を保つ（§16.2 MUST）。並べ替えは表示だけ。
//! - `index.json` が JSON として読めない（**E025**）ときは**何もしない**（`None`。原文は 1 バイトも
//!   書き換えない、§5.8.2 MUST）。

use crate::core::features::Project;
use crate::core::id::{RangeExpr, SceneId};
use crate::core::model::document::DocumentFormat;
use crate::core::model::material::{
    Condition, Material, MaterialIndex, MaterialStatus, Source, ADDED_BY_HUMAN, PREFIX_MATERIAL,
    PREFIX_SEARCH,
};
use crate::core::resolve::dictionary;
use crate::core::syntax::materials_index;

/// `materials/index.json` のパス（§5.8.1）。
pub const INDEX_PATH: &str = "materials/index.json";

/// 提案中の資料を置くディレクトリ（§5.8.1）。
pub const INBOX_DIR: &str = "materials/inbox/";

/// 採用した資料を置くディレクトリ（§5.8.1）。
pub const DOCS_DIR: &str = "materials/docs/";

/// 資料 MD の 2 行目の ID コメントの前置き（§5.8.3 `<!-- novnow:M0137 -->`）。
const ID_COMMENT: &str = "<!-- novnow:";

/// [`propose`] / [`register`] で作ったもの。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Created {
    /// 発行した ID（`M0142`）。
    pub id: String,
    /// 資料 MD のパス（`materials/inbox/M0142.md`）。
    pub path: String,
    /// 資料 MD の中身（§5.8.3: H1 = タイトル、2 行目に ID の HTML コメント）。
    pub body: String,
    /// 新しい `index.json`。
    pub index: String,
}

/// 読める索引（E025 なら `None`）。索引のファイルが無ければ空の索引（`schema_version: 1`）。
fn read(index_text: &str) -> Option<MaterialIndex> {
    if index_text.trim().is_empty() {
        return Some(MaterialIndex {
            schema_version: 1,
            next_id: 1,
            ..MaterialIndex::default()
        });
    }
    let parsed = materials_index::parse(index_text);
    parsed.index.clone()
}

/// 索引を書く（2 スペースの字下げ・末尾改行）。
fn write(index: &MaterialIndex) -> String {
    let mut out = serde_json::to_string_pretty(index).unwrap_or_default();
    out.push('\n');
    out
}

/// 出典の文字列から `source`（`http://` / `https://` で始まれば `kind: url`、それ以外は
/// `kind: custom` の自由文。空なら出典なし = W065）。
fn source_of(text: &str) -> Source {
    let text = text.trim();
    if text.starts_with("http://") || text.starts_with("https://") {
        Source {
            kind: "url".to_owned(),
            url: Some(text.to_owned()),
            ..Source::default()
        }
    } else {
        Source {
            kind: "custom".to_owned(),
            text: Some(text.to_owned()),
            ..Source::default()
        }
    }
}

/// 人間が集めた資料を**提案**として作る（§16.1「収集 → materials/inbox/ + status=proposed」）。
/// 出典もカテゴリも空（審査で整える。出典が無いので W065 が付く）。
pub fn propose(index_text: &str, title: &str, body: &str, saved_at: &str) -> Option<Created> {
    propose_with(index_text, title, "", body, "", saved_at)
}

/// [`propose`] のカテゴリ・出典つき（`Ctrl+M` の `Ctrl+Enter` は検索語の `#カテゴリ` を渡す）。
pub fn propose_with(
    index_text: &str,
    title: &str,
    category: &str,
    body: &str,
    source: &str,
    saved_at: &str,
) -> Option<Created> {
    propose_as(
        index_text,
        title,
        category,
        body,
        source,
        saved_at,
        ADDED_BY_HUMAN,
    )
}

/// [`propose_with`] の `added_by` 指定版（researcher の提案・`novnow.materials.propose` は
/// `agent:<role>`、§5.8.2）。
pub fn propose_as(
    index_text: &str,
    title: &str,
    category: &str,
    body: &str,
    source: &str,
    saved_at: &str,
    added_by: &str,
) -> Option<Created> {
    let mut index = read(index_text)?;
    let id = index.issue(PREFIX_MATERIAL).to_string();
    let path = format!("{INBOX_DIR}{id}.md");
    let title = title.trim().replace('\n', " ");
    index.items.push(Material {
        id: id.clone(),
        title: title.clone(),
        path: path.clone(),
        source: source_of(source),
        category: category.trim().to_owned(),
        conditions: Vec::new(),
        saved_at: saved_at.to_owned(),
        status: "proposed".to_owned(),
        added_by: added_by.to_owned(),
        ..Material::default()
    });
    let mut text = format!("# {title}\n{ID_COMMENT}{id} -->\n");
    if !body.trim().is_empty() {
        text.push('\n');
        text.push_str(body.trim());
        text.push('\n');
    }
    Some(Created {
        id,
        path,
        body: text,
        index: write(&index),
    })
}

/// 索引に無い資料 MD `path` を登録する（**W064** の「登録する」、§16.2「ワンクリックで登録できる」）。
///
/// 新しい ID を発行する（MD に書かれた ID は使わない。消された資料の ID の再利用を避ける、§5.8.2
/// MUST）。タイトルは 1 行目の H1（無ければファイル名）。`docs/` の下なら `approved`、それ以外は
/// `proposed`（置いた場所が審査の結果を表す）。MD の ID コメント（§5.8.3）は新しい ID に書き換え、
/// 無ければ H1 の次の行に足す（H1 も無ければ H1 ごと先頭に足す）。登録済み・E025 なら `None`。
pub fn register(index_text: &str, path: &str, body: &str, saved_at: &str) -> Option<Created> {
    let mut index = read(index_text)?;
    if index
        .items
        .iter()
        .any(|item| item.path.trim_start_matches("./") == path)
    {
        return None;
    }
    let id = index.issue(PREFIX_MATERIAL).to_string();
    let heading = body
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("# "))
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty());
    let title = heading.clone().unwrap_or_else(|| {
        let name = path.rsplit('/').next().unwrap_or(path);
        name.trim_end_matches(".md").to_owned()
    });
    let status = if path.starts_with(DOCS_DIR) {
        "approved"
    } else {
        "proposed"
    };
    index.items.push(Material {
        id: id.clone(),
        title: title.clone(),
        path: path.to_owned(),
        source: source_of(""),
        category: String::new(),
        conditions: Vec::new(),
        saved_at: saved_at.to_owned(),
        status: status.to_owned(),
        added_by: ADDED_BY_HUMAN.to_owned(),
        ..Material::default()
    });
    let comment = format!("{ID_COMMENT}{id} -->");
    let mut lines: Vec<String> = body.split('\n').map(str::to_owned).collect();
    match lines
        .iter()
        .take(3)
        .position(|line| line.trim_start().starts_with(ID_COMMENT))
    {
        Some(at) => lines[at] = comment,
        None if heading.is_some() => lines.insert(1, comment),
        None => {
            lines.insert(0, comment);
            lines.insert(0, format!("# {title}"));
        }
    }
    Some(Created {
        id,
        path: path.to_owned(),
        body: lines.join("\n"),
        index: write(&index),
    })
}

/// 索引に登録されていない資料 MD（**W064** の対象。パスの順）。索引が読めない（E025）なら空。
pub fn unregistered(project: &Project) -> Vec<String> {
    let index = project.materials();
    let Some(parsed) = index.index.as_ref() else {
        return Vec::new();
    };
    let mut out: Vec<String> = project
        .documents()
        .filter(|document| document.format == DocumentFormat::MaterialDoc)
        .filter(|document| {
            !parsed
                .items
                .iter()
                .any(|item| item.path.trim_start_matches("./") == document.path)
        })
        .map(|document| document.path.clone())
        .collect();
    out.sort();
    out
}

/// 状態を変えた結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// 新しい `index.json`。
    pub index: String,
    /// 資料 MD を動かすなら（元のパス, 新しいパス）。採用で `inbox/` → `docs/`。
    pub rename: Option<(String, String)>,
}

/// 資料 `id` の状態を `status` にする（§16.1: 採用 → `docs/` へ移して `approved`、却下 →
/// `rejected`（ファイルは inbox に残す））。`updated_at` も書く。資料が無い・E025 なら `None`。
pub fn set_status(index_text: &str, id: &str, status: MaterialStatus, now: &str) -> Option<Moved> {
    let mut index = read(index_text)?;
    let item = index.items.iter_mut().find(|item| item.id == id)?;
    item.status = match status {
        MaterialStatus::Approved => "approved",
        MaterialStatus::Proposed => "proposed",
        MaterialStatus::Rejected => "rejected",
    }
    .to_owned();
    item.updated_at = Some(now.to_owned());
    let mut rename = None;
    if status == MaterialStatus::Approved {
        if let Some(name) = item.path.strip_prefix(INBOX_DIR) {
            let to = format!("{DOCS_DIR}{name}");
            rename = Some((item.path.clone(), to.clone()));
            item.path = to;
        }
    }
    Some(Moved {
        index: write(&index),
        rename,
    })
}

/// 本文の `checksum` の値（`sha256:` + 小文字 16 進、§5.8.2 の例の形）。
pub fn checksum(body: &str) -> String {
    format!("sha256:{}", crate::core::text::sha256::hex(body.as_bytes()))
}

/// 資料 MD `path` を保存したときの `updated_at` と `checksum` の書き換え（§16.6「保存のたびに
/// checksum を更新し、updated_at を書き換える」）。`body` は書き込む本文。索引に無いパス・E025 なら
/// `None`。
pub fn touch(index_text: &str, path: &str, now: &str, body: &str) -> Option<String> {
    let mut index = read(index_text)?;
    let item = index
        .items
        .iter_mut()
        .find(|item| item.path.trim_start_matches("./") == path)?;
    item.updated_at = Some(now.to_owned());
    item.checksum = Some(checksum(body));
    Some(write(&index))
}

/// 索引の資料（追加順。E025・索引が無ければ空）。
pub fn items(index_text: &str) -> Vec<Material> {
    if index_text.trim().is_empty() {
        return Vec::new();
    }
    materials_index::parse(index_text)
        .index
        .map(|index| index.items)
        .unwrap_or_default()
}

/// 審査で整える項目（§16.1「タイトル / カテゴリ / conditions / summary を整える」と出典・タグ）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Review {
    /// タイトル。
    pub title: String,
    /// カテゴリ。
    pub category: String,
    /// 要約（空なら書かない）。
    pub summary: String,
    /// タグ（カンマ区切り）。
    pub tags: String,
    /// 条件（1 行 1 件、`scope: @02-03 .. @02-08` / `ref: '名前` / `tag: 文字列` / `text: 自由文`）。
    pub conditions: String,
    /// 出典（URL なら `kind: url`、それ以外は `kind: custom` の自由文。空なら出典なし）。
    pub source: String,
}

impl Review {
    /// 資料から（フォームの初期値）。
    pub fn of(item: &Material) -> Self {
        Self {
            title: item.title.clone(),
            category: item.category.clone(),
            summary: item.summary.clone().unwrap_or_default(),
            tags: item.tags.join(", "),
            conditions: item
                .conditions
                .iter()
                .map(|found| format!("{}: {}", found.kind, found.value))
                .collect::<Vec<_>>()
                .join("\n"),
            source: item.source.shown().to_owned(),
        }
    }
}

/// 資料 `id` を `review` の内容に書き換えた索引（`updated_at` も書く）。資料が無い・E025 なら `None`。
///
/// 条件の行で `kind:` の形でないものは `text` として読む。出典は**表示と変わったときだけ**書き換える
/// （`kind: search` や `accessed_at` を、出典に触れていない審査で消さないため）。
pub fn update(index_text: &str, id: &str, review: &Review, now: &str) -> Option<String> {
    let mut index = read(index_text)?;
    let item = index.items.iter_mut().find(|item| item.id == id)?;
    item.title = review.title.trim().replace('\n', " ");
    item.category = review.category.trim().to_owned();
    let summary = review.summary.trim();
    item.summary = (!summary.is_empty()).then(|| summary.replace('\n', " "));
    item.tags = review
        .tags
        .split([',', '、'])
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect();
    let kept: Vec<Condition> = review
        .conditions
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (kind, value) = match line.split_once(':') {
                Some((kind, value)) if matches!(kind.trim(), "scope" | "ref" | "tag" | "text") => {
                    (kind.trim().to_owned(), value.trim().to_owned())
                }
                _ => ("text".to_owned(), line.to_owned()),
            };
            // 同じ条件の書かれていないキー（縫い代 1）は残す
            let unknown = item
                .conditions
                .iter()
                .find(|found| found.kind == kind && found.value == value)
                .map(|found| found.unknown.clone())
                .unwrap_or_default();
            Condition {
                kind,
                value,
                unknown,
            }
        })
        .collect();
    item.conditions = kept;
    let source = review.source.trim();
    if source != item.source.shown().trim() {
        let mut fresh = source_of(source);
        fresh.unknown = std::mem::take(&mut item.source.unknown);
        fresh.accessed_at = item.source.accessed_at.take();
        item.source = fresh;
    }
    item.updated_at = Some(now.to_owned());
    Some(write(&index))
}

/// 出典が無いか（W065「出典が無い資料」の条件、[`Source::is_missing`]）。
pub fn has_no_source(item: &Material) -> bool {
    item.source.is_missing()
}

/// 検索語から新しい資料のタイトルとカテゴリ（`Ctrl+M` の `Ctrl+Enter`「新規資料を作成」）。
/// 素のテキストの語をタイトルに、最初の `#カテゴリ` をカテゴリにする。それ以外の語は捨てる。
pub fn draft_of(query: &str) -> (String, String) {
    let mut title: Vec<&str> = Vec::new();
    let mut category = String::new();
    for token in query.split_whitespace() {
        if let Some(found) = token.strip_prefix('#') {
            if category.is_empty() {
                category = found.to_owned();
            }
        } else if !is_operator(token) {
            title.push(token);
        }
    }
    (title.join(" "), category)
}

// ============================================================ 検索（§16.3 `Ctrl+M`）

/// 検索の 1 件（§16.3「`M0137  タイトル  #取材  2026-10-01`」と、選んだ行の下の要約と出典）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// ID。
    pub id: String,
    /// タイトル。
    pub title: String,
    /// カテゴリ。
    pub category: String,
    /// `saved_at` の日付（`2026-10-01`）。
    pub date: String,
    /// 要約。
    pub summary: String,
    /// 出典（URL か自由文）。
    pub source: String,
    /// 資料 MD のパス。
    pub path: String,
    /// 状態。
    pub status: String,
    /// 点数（素のテキストの語がどこに当たったか: タイトル 4・タグ 3・要約 2・本文 1 の和）。
    pub score: u32,
    saved_at: String,
}

/// 照合用の形（§6.1.3 の正規化 ＋ 小文字。全角・半角や長音の揺れを吸収する「あいまい一致」）。
fn fold(text: &str) -> String {
    dictionary::normalize(text).to_lowercase()
}

/// §16.3 の検索。語は空白区切りの AND。`#カテゴリ`（完全一致）・`@範囲`（`conditions.scope` と交差）・
/// `'参照`（`conditions.ref` と一致）・`M数字` / `S数字`（ID の前方一致）・`!status`・`~日付`
/// （`saved_at` の前方一致）・素のテキスト（タイトル・タグ・要約・本文の部分一致。§6.1.3 の正規化と
/// 大文字小文字の同一視をしてから比べる）。並びは点数 → `saved_at` の新しい順 → ID（決定論）。
/// `!status` が無ければ `rejected` は出さない（B-134）。
pub fn search(project: &Project, query: &str) -> Vec<Hit> {
    let items = items(project.text(INDEX_PATH));
    let tokens: Vec<&str> = query.split_whitespace().collect();
    let wants_status = tokens.iter().any(|token| token.starts_with('!'));
    let mut out: Vec<Hit> = items
        .iter()
        .filter_map(|item| {
            if !wants_status && item.status == "rejected" {
                return None;
            }
            let score = score(project, item, &tokens)?;
            Some(Hit {
                id: item.id.clone(),
                title: item.title.clone(),
                category: item.category.clone(),
                date: item.saved_at.chars().take(10).collect(),
                summary: item.summary.clone().unwrap_or_default(),
                source: item.source.shown().to_owned(),
                path: item.path.clone(),
                status: item.status.clone(),
                score,
                saved_at: item.saved_at.clone(),
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.saved_at.cmp(&a.saved_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}

/// 資料 1 件が全部の語に当たれば点数。どれかに外れれば `None`。
fn score(project: &Project, item: &Material, tokens: &[&str]) -> Option<u32> {
    let mut texts: Option<(String, String)> = None;
    let mut tags: Option<Vec<String>> = None;
    let mut body: Option<String> = None;
    let mut total = 0;
    for token in tokens {
        let ok = if let Some(category) = token.strip_prefix('#') {
            fold(&item.category) == fold(category)
        } else if token.starts_with('@') {
            match RangeExpr::parse(token) {
                Ok(wanted) => item.conditions.iter().any(|found| {
                    found.kind == "scope"
                        && RangeExpr::parse(&found.value)
                            .is_ok_and(|scope| scope.intersects(&wanted))
                }),
                Err(_) => false,
            }
        } else if let Some(name) = token.strip_prefix('\'') {
            let wanted = fold(name.trim_start_matches('{').trim_end_matches('}'));
            item.conditions
                .iter()
                .any(|found| found.kind == "ref" && fold(ref_name(&found.value)) == wanted)
        } else if let Some(status) = token.strip_prefix('!') {
            item.status == status
        } else if let Some(date) = token.strip_prefix('~') {
            item.saved_at.starts_with(date)
        } else if is_id_prefix(token) {
            item.id.starts_with(&token.to_ascii_uppercase())
        } else {
            let needle = fold(token);
            let (title, summary) = texts.get_or_insert_with(|| {
                (
                    fold(&item.title),
                    fold(item.summary.as_deref().unwrap_or("")),
                )
            });
            let weight = if title.contains(&needle) {
                4
            } else if tags
                .get_or_insert_with(|| item.tags.iter().map(|tag| fold(tag)).collect())
                .iter()
                .any(|tag| tag.contains(&needle))
            {
                3
            } else if summary.contains(&needle) {
                2
            } else if body
                .get_or_insert_with(|| fold(project.text(&item.path)))
                .contains(&needle)
            {
                1
            } else {
                0
            };
            total += weight;
            weight > 0
        };
        if !ok {
            return None;
        }
    }
    Some(total)
}

/// `ref` 条件の値から名前（`'科学の甲子園` / `'{リリアナ}` → `科学の甲子園` / `リリアナ`）。
fn ref_name(value: &str) -> &str {
    value
        .trim()
        .trim_start_matches('\'')
        .trim_start_matches('{')
        .trim_end_matches('}')
}

/// `M01` / `s00` のような ID の前方一致の語か（接頭辞は大文字小文字を問わない）。
fn is_id_prefix(token: &str) -> bool {
    let mut chars = token.chars();
    let prefix = chars.next().map(|c| c.to_ascii_uppercase());
    matches!(prefix, Some(found) if found == PREFIX_MATERIAL || found == PREFIX_SEARCH)
        && token.len() > 1
        && chars.all(|c| c.is_ascii_digit())
}

/// 素のテキストでない語（`#` `@` `'` `!` `~` の前置き・ID）か。
fn is_operator(token: &str) -> bool {
    token.starts_with(['#', '@', '\'', '!', '~']) || is_id_prefix(token)
}

// ============================================================ 条件一致（§16.4）

/// §16.4 の条件一致: 節 `scene` に今必要かもしれない資料（`scope` が節を含む・`ref` の参照が節に
/// 出てくる・`tag` が節のプロットのタグにある）。`text` は機械判定しない。`approved` だけ。
/// `refs` は節に出てくる参照の名前（`'` を除いたもの）、`tags` は節のプロットのタグ。
pub fn matching(
    project: &Project,
    scene: SceneId,
    refs: &[String],
    tags: &[String],
) -> Vec<String> {
    let refs: Vec<String> = refs.iter().map(|name| fold(name)).collect();
    let tags: Vec<String> = tags.iter().map(|tag| fold(tag)).collect();
    items(project.text(INDEX_PATH))
        .into_iter()
        .filter(|item| item.status_kind() == Some(MaterialStatus::Approved))
        .filter(|item| {
            item.conditions
                .iter()
                .any(|found| match found.kind.as_str() {
                    "scope" => {
                        RangeExpr::parse(&found.value).is_ok_and(|scope| scope.contains(scene))
                    }
                    "ref" => refs.contains(&fold(ref_name(&found.value))),
                    "tag" => tags.contains(&fold(found.value.trim())),
                    _ => false,
                })
        })
        .map(|item| item.id)
        .collect()
}
