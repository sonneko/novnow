//! 資料のワークスペース（§10.5 / §16 `material`）の操作: 提案の作成・採用・却下・更新日時。
//!
//! LSP には対応しない補助（`PLANS.md` §9.2 B-131）。**文言は持たない**（§12.2）。どれも
//! 「`materials/index.json` の文字列 → 新しい文字列」の純粋関数で、書き込みは UI が行う。
//!
//! - ID は `next_id` からの単調増加（§5.8.2 MUST）。[`MaterialIndex::issue`] が `next_id` と既存の
//!   最大値の大きいほうから発行するので、`next_id` が壊れていても再利用しない。却下しても ID は
//!   回収しない（§16.1）。
//! - `items` の順は追加順を保つ（§16.2 MUST）。並べ替えは表示だけ。
//! - `index.json` が JSON として読めない（**E025**）ときは**何もしない**（`None`。原文は 1 バイトも
//!   書き換えない、§5.8.2 MUST）。

use crate::core::model::material::{
    Material, MaterialIndex, MaterialStatus, Source, ADDED_BY_HUMAN, PREFIX_MATERIAL,
};
use crate::core::syntax::materials_index;

/// `materials/index.json` のパス（§5.8.1）。
pub const INDEX_PATH: &str = "materials/index.json";

/// 提案中の資料を置くディレクトリ（§5.8.1）。
pub const INBOX_DIR: &str = "materials/inbox/";

/// 採用した資料を置くディレクトリ（§5.8.1）。
pub const DOCS_DIR: &str = "materials/docs/";

/// [`propose`] で作ったもの。
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

/// 人間が集めた資料を**提案**として作る（§16.1「収集 → materials/inbox/ + status=proposed」）。
///
/// `source` は出典の自由文（`kind: custom`。空なら出典なし = W065）。カテゴリは空（審査で整える）。
pub fn propose(index_text: &str, title: &str, body: &str, saved_at: &str) -> Option<Created> {
    propose_with_source(index_text, title, body, "", saved_at)
}

/// [`propose`] の出典つき。
pub fn propose_with_source(
    index_text: &str,
    title: &str,
    body: &str,
    source: &str,
    saved_at: &str,
) -> Option<Created> {
    let mut index = read(index_text)?;
    let id = index.issue(PREFIX_MATERIAL).to_string();
    let path = format!("{INBOX_DIR}{id}.md");
    let title = title.trim().replace('\n', " ");
    index.items.push(Material {
        id: id.clone(),
        title: title.clone(),
        path: path.clone(),
        source: Source {
            kind: "custom".to_owned(),
            text: Some(source.trim().to_owned()),
            ..Source::default()
        },
        category: String::new(),
        conditions: Vec::new(),
        saved_at: saved_at.to_owned(),
        status: "proposed".to_owned(),
        added_by: ADDED_BY_HUMAN.to_owned(),
        ..Material::default()
    });
    let mut text = format!("# {title}\n<!-- novnow:{id} -->\n");
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

/// 資料 MD `path` を保存したときの `updated_at` の書き換え（§16.6）。索引に無いパス・E025 なら `None`。
pub fn touch(index_text: &str, path: &str, now: &str) -> Option<String> {
    let mut index = read(index_text)?;
    let item = index.items.iter_mut().find(|item| item.path == path)?;
    item.updated_at = Some(now.to_owned());
    Some(write(&index))
}
