//! ホバー（§3.4 `hover::at(doc, pos, project)` → `textDocument/hover`）。
//!
//! 参照トークン（`'` と話者注釈 `@`）の上で、解決先の**中身だけ**を返す（文言と組版は UI、
//! §12.2）。未解決のトークンは `None`（E001 / E007 / E012 の診断が出ているので重ねない）。
//! 別種別に同名の項目があれば `others` に種別を並べる（§6.1.1「参照ペインに『別種別の同名項目
//! あり』を併記する」と同じ情報）。

use std::ops::Range;

use serde::Serialize;

use crate::core::features::project::{Project, SiteAt};
use crate::core::model::document::Document;
use crate::core::model::material::MaterialStatus;
use crate::core::resolve::dictionary::{EntryId, EntryKind, Target};
use crate::core::resolve::modes;
use crate::core::syntax::plot_outline::Field;
use crate::core::text::lexer::FieldKey;

/// `alias:` の区切り（§5.5.3 / §5.6。`PLANS.md` §9.2 B-58 と同じ 3 字）。
const ALIAS_SEPARATORS: &[char] = &[',', '、', '，'];

/// ホバーの結果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hover {
    /// ホバーしたトークンの範囲（ファイル単位）。
    pub range: Range<usize>,
    /// 中身。
    pub contents: HoverContents,
}

/// ホバーの中身（種別ごと）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum HoverContents {
    /// キャラ（`characters.md`、§5.5）。
    Character {
        /// H1 の表示名。
        name: String,
        /// `reading:`。
        reading: Option<String>,
        /// `alias:`（書かれた順）。
        aliases: Vec<String>,
        /// `role:`。
        role: Option<String>,
        /// 全モード（書かれた順）。
        modes: Vec<ModeSummary>,
        /// トークンに書かれたモード ID（`'リリア:B+C` なら `B` と `C`）。
        selected: Vec<String>,
        /// 同じキーを持つ別種別（§6.1.1 の W027）。
        others: Vec<EntryKind>,
    },
    /// 用語（`words.md`、§5.6）。
    Word {
        /// H1 の用語。
        term: String,
        /// `reading:`。
        reading: Option<String>,
        /// `category:`。
        category: Option<String>,
        /// `definition:`（継続行を含む全文）。
        definition: Option<String>,
        /// `alias:`。
        aliases: Vec<String>,
        /// 同じキーを持つ別種別。
        others: Vec<EntryKind>,
    },
    /// 資料（`materials/index.json`、§5.8.2）。
    Material {
        /// ID（`M0137`）。
        id: String,
        /// タイトル（切り詰めない）。
        title: String,
        /// 分類。
        category: String,
        /// `status`。
        status: Option<MaterialStatus>,
        /// 同じキーを持つ別種別。
        others: Vec<EntryKind>,
    },
}

/// モード 1 つの要約（§6.4.6 の参照ペインと同じ素材）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModeSummary {
    /// `mode_id`。
    pub id: String,
    /// 表示名（§5.5.2 `mode_label`）。
    pub label: String,
    /// `range:` の値（書かれたまま。無ければ空＝全範囲）。
    pub range: String,
    /// ホバーした行の節で使えるか（節が分からない文書では `true`）。
    pub available: bool,
}

/// `pos` のトークンのホバー（§3.4）。
pub fn at(document: &Document, pos: usize, project: &Project) -> Option<Hover> {
    let site = project.site_at(document, pos)?;
    let entry_id = match &site {
        SiteAt::Ref(found) => found.entry()?,
        SiteAt::Speaker(found) => found.entry?,
    };
    let entry = project.dictionary().entry(entry_id)?;
    let others = others(project, entry_id, entry.kind);
    let contents = match &entry.target {
        Target::Character { index, name } => {
            let characters = project.characters();
            let first = |key: FieldKey| {
                characters
                    .character_fields(*index, key)
                    .first()
                    .map(|field| field.full_value())
            };
            let scene = project.scene_at(
                document,
                crate::core::features::project::line_of(&document.text, pos),
            );
            let modes = modes::definitions(&characters, *index)
                .iter()
                .map(|definition| ModeSummary {
                    id: definition.id().to_string(),
                    label: definition.label().to_string(),
                    range: definition.range_text().to_string(),
                    available: scene.is_none_or(|found| definition.contains(found)),
                })
                .collect();
            let selected = match &site {
                SiteAt::Ref(found) => found
                    .token
                    .modifier
                    .as_ref()
                    .and_then(|modifier| modifier.modes())
                    .map(<[String]>::to_vec)
                    .unwrap_or_default(),
                SiteAt::Speaker(found) => found.modes.iter().map(|(id, _)| id.clone()).collect(),
            };
            HoverContents::Character {
                name: name.clone(),
                reading: first(FieldKey::Reading),
                aliases: aliases(&characters.character_fields(*index, FieldKey::Alias)),
                role: first(FieldKey::Role),
                modes,
                selected,
                others,
            }
        }
        Target::Word { index, term } => {
            let words = project.words();
            let first = |key: FieldKey| {
                words
                    .word_fields(*index, key)
                    .first()
                    .map(|field| field.full_value())
            };
            HoverContents::Word {
                term: term.clone(),
                reading: first(FieldKey::Reading),
                category: first(FieldKey::Category),
                definition: first(FieldKey::Definition),
                aliases: aliases(&words.word_fields(*index, FieldKey::Alias)),
                others,
            }
        }
        Target::Material { id } => {
            let index = project.materials();
            let material = index
                .index
                .as_ref()?
                .items
                .iter()
                .find(|item| item.id == *id)?;
            HoverContents::Material {
                id: material.id.clone(),
                title: material.title.clone(),
                category: material.category.clone(),
                status: material.status_kind(),
                others,
            }
        }
    };
    Some(Hover {
        range: site.range(),
        contents,
    })
}

/// 同じキーの別種別（自分の種別は除く、優先順位順）。
fn others(project: &Project, id: EntryId, own: EntryKind) -> Vec<EntryKind> {
    let mut out: Vec<EntryKind> = project
        .dictionary()
        .conflicts(id)
        .into_iter()
        .filter_map(|other| project.dictionary().entry(other))
        .map(|entry| entry.kind)
        .filter(|kind| *kind != own)
        .collect();
    out.sort_by_key(|kind| kind.priority());
    out.dedup();
    out
}

/// `alias:` の値を区切る（空は捨てる）。
fn aliases(fields: &[&Field<'_>]) -> Vec<String> {
    fields
        .iter()
        .flat_map(|field| field.value.split(ALIAS_SEPARATORS))
        .map(str::trim)
        .filter(|alias| !alias.is_empty())
        .map(str::to_string)
        .collect()
}
