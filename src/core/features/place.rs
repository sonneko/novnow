//! 診断の「どこ」を作家の言葉で表す（§14.2.5 の `novnow check`・§10.9.2 の診断タブの「位置」。
//! ユーザー裁定 2026-10-13・仕様 第 2.11 版、`PLANS.md` §9.2 B-110）。
//!
//! `パス:行:列` はエンジニア向けの位置で、作家には「どの節の・どの人物の」が分からない。そこで
//! 文書の種類ごとに意味のある名前（構造 ID・人物名・資料 ID とタイトルなど）を返す。
//!
//! | 文書 | 返すもの |
//! |---|---|
//! | 本文 `contents/AA-BB.txt` | その節の構造 ID（`01-04-02-03`）。区切り `;` の行はそれが閉じる節、場面見出しの行は場面（`01-04-02`）。最初の場面見出しより前は話（`01-04`） |
//! | 話プロット `plots/AA-BB.txt` | 同上（見出し直下の場面のフィールドは場面） |
//! | 章プロット `plots/AA.txt` | ブロックの ID（`01-04`）。最初のブロックより前は章（`01`） |
//! | 全体プロット `plots/all.txt` | ブロックの ID（`01`）。最初のブロックより前は ID なし |
//! | `characters.md` | 人物名（モードの中ならモード） |
//! | `words.md` | 用語 |
//! | `ideas.md` | アイデアの ID とタイトル |
//! | `materials/index.json`・`materials/docs/*.md` | 資料の ID とタイトル |
//! | `prompts/**/*.md` | 役割（セクションの中ならセクション） |
//! | `project.json` | 作品の設定 |
//!
//! **文言は持たない**（§12.2）。「本文」「登場人物」などの言葉は `ui/i18n` の `place.*` が付ける。
//! 名前と ID はファイルに書かれたままの文字列。

use crate::core::features::document_symbol::{self, DocumentSymbol, SymbolKind};
use crate::core::features::Project;
use crate::core::id::{PartialSceneId, SceneId};
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::syntax::{contents, plot_episode};
use crate::core::text::position::LineTable;

/// 作家の言葉での「どこ」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// 本文（C 形式）の構造 ID（`@` を付けない。`01-04-02-03`）。
    Contents(String),
    /// 話プロット（B 形式）の構造 ID。
    EpisodePlot(String),
    /// 章プロット（`plots/AA.txt`）のブロックの ID（`01-04`）、または章（`01`）。
    ChapterPlot(String),
    /// 全体プロット（`plots/all.txt`）のブロックの ID（`01`）。ブロックの外なら `None`。
    WholePlot(Option<String>),
    /// 登場人物。`mode` はモードの見出しのまま（`A 日常`）。
    Character {
        /// 名前。
        name: String,
        /// モード（モードの中のとき）。
        mode: Option<String>,
    },
    /// 用語。
    Word(String),
    /// アイデア。
    Idea {
        /// ID（`I0007`）。
        id: String,
        /// タイトル。
        title: String,
    },
    /// 資料。
    Material {
        /// ID（`M0137`）。
        id: String,
        /// タイトル（索引に無ければ空）。
        title: String,
    },
    /// プロンプト。
    Prompt {
        /// 役割（H1）。
        role: String,
        /// セクション（`## ` の中のとき）。
        section: Option<String>,
    },
    /// 作品の設定（`project.json`）。
    Settings,
}

/// `document` の `byte`（無ければ文書全体）の「どこ」。決まらなければ `None`（パスだけで足りる）。
pub fn at(document: &Document, byte: Option<usize>, project: &Project) -> Option<Place> {
    match document.format {
        DocumentFormat::Contents => scene_place(document, byte, project).map(Place::Contents),
        DocumentFormat::PlotEpisode => scene_place(document, byte, project).map(Place::EpisodePlot),
        DocumentFormat::PlotOutline => Some(outline_place(document, byte, project)),
        DocumentFormat::Characters => {
            let symbols = document_symbol::all(document);
            let character = enclosing(&symbols, byte?)?;
            let mode = byte
                .and_then(|at| containing(&character.children, at))
                .filter(|found| found.kind == SymbolKind::Mode)
                .map(heading_text);
            Some(Place::Character {
                name: character.name.clone(),
                mode,
            })
        }
        DocumentFormat::Words => {
            let symbols = document_symbol::all(document);
            enclosing(&symbols, byte?).map(|found| Place::Word(found.name.clone()))
        }
        DocumentFormat::Ideas => {
            let symbols = document_symbol::all(document);
            enclosing(&symbols, byte?).map(|found| Place::Idea {
                id: found.name.clone(),
                title: found.detail.clone(),
            })
        }
        DocumentFormat::MaterialsIndex => material_in_index(document, byte?),
        DocumentFormat::MaterialDoc => material_doc(document, project),
        DocumentFormat::Prompt | DocumentFormat::CustomPrompt | DocumentFormat::SharedPrompt => {
            let symbols = document_symbol::all(document);
            let role = symbols.first()?;
            let section = byte
                .and_then(|at| containing(&role.children, at))
                .map(|found| found.name.clone());
            Some(Place::Prompt {
                role: role.name.clone(),
                section,
            })
        }
        DocumentFormat::ProjectFile => Some(Place::Settings),
        _ => None,
    }
}

/// 本文・話プロット: その行の節の構造 ID。見出しより前（または位置なし）はファイル名の話。
fn scene_place(document: &Document, byte: Option<usize>, project: &Project) -> Option<String> {
    let digits = project.digits();
    if let Some(at) = byte {
        let line = LineTable::new(&document.text).line_of_byte(at);
        let found = match document.format {
            DocumentFormat::Contents => contents_line(document, line, project),
            _ => episode_line(document, line, project),
        };
        if let Some(id) = found {
            return Some(id.display(digits));
        }
    }
    file_id(document).map(|id| id.display(digits))
}

/// C 形式の行の節。区切り `;` の行はそれが閉じる節、場面見出しの行は場面（節 0）。
fn contents_line(document: &Document, line: usize, project: &Project) -> Option<SceneId> {
    let parsed = contents::parse_with(&document.text, &project.contents_options());
    let mut closing = None;
    for section in &parsed.sections {
        let Some(scene) = section.scene else {
            continue;
        };
        if section.span.contains(&line) {
            return Some(SceneId {
                section: section.number,
                ..scene
            });
        }
        if section.scene_header == Some(line) {
            return Some(SceneId {
                section: 0,
                ..scene
            });
        }
        if section.span.end == line && closing.is_none() {
            closing = Some(SceneId {
                section: section.number,
                ..scene
            });
        }
    }
    closing
}

/// B 形式の行の節。区切り `;` の行はそれが閉じる節、見出しと場面のフィールドは場面（節 0）。
fn episode_line(document: &Document, line: usize, project: &Project) -> Option<SceneId> {
    let parsed = plot_episode::parse_with(&document.text, &project.contents_options());
    for scene in &parsed.scenes {
        let Some(id) = scene.id else {
            continue;
        };
        if !scene.span.contains(&line) {
            continue;
        }
        for section in &scene.sections {
            if section.span.contains(&line) || section.break_line == Some(line) {
                return Some(SceneId {
                    section: section.number,
                    ..id
                });
            }
        }
        return Some(SceneId { section: 0, ..id });
    }
    None
}

/// A 形式: 位置を含むブロックの ID。ブロックの外は、章プロットならファイル名の章、全体プロットなら `None`。
fn outline_place(document: &Document, byte: Option<usize>, project: &Project) -> Place {
    let whole = document.path.trim_start_matches("./") == "plots/all.txt";
    let symbols = document_symbol::all(document);
    let block = byte.and_then(|at| enclosing(&symbols, at));
    match (whole, block) {
        (true, Some(found)) => Place::WholePlot(Some(found.name.clone())),
        (true, None) => Place::WholePlot(None),
        (false, Some(found)) => Place::ChapterPlot(found.name.clone()),
        (false, None) => match file_id(document) {
            Some(id) => Place::ChapterPlot(id.display(project.digits())),
            None => Place::ChapterPlot(String::new()),
        },
    }
}

/// ファイル名（`01-04.txt` / `01.txt`）の構造 ID。
fn file_id(document: &Document) -> Option<PartialSceneId> {
    let name = document.path.rsplit('/').next()?;
    let stem = name.strip_suffix(".txt")?;
    PartialSceneId::parse(stem).ok()
}

/// `byte` 以前に始まる最後のシンボル（次の見出しまでがそのシンボル）。
fn enclosing(symbols: &[DocumentSymbol], byte: usize) -> Option<&DocumentSymbol> {
    symbols.iter().rev().find(|found| found.range.start <= byte)
}

/// `byte` を範囲に含むシンボル（子の範囲の外＝親の地の部分なら `None`）。
fn containing(symbols: &[DocumentSymbol], byte: usize) -> Option<&DocumentSymbol> {
    symbols
        .iter()
        .find(|found| found.range.start <= byte && byte <= found.range.end)
}

/// モードの見出しのまま（`A 日常`）。
fn heading_text(found: &DocumentSymbol) -> String {
    if found.detail.is_empty() {
        found.name.clone()
    } else {
        format!("{} {}", found.name, found.detail)
    }
}

/// `materials/index.json`: 位置より前で最後に現れる項目の `"id"` の値。原文から探すので、項目の中で
/// `id` より前に書かれたキーは 1 つ前の項目に数えられることがある（JSON の位置を持たないため）。
fn material_in_index(document: &Document, byte: usize) -> Option<Place> {
    let parsed = crate::core::syntax::materials_index::parse(&document.text);
    let index = parsed.index?;
    let raw = document.text.as_str();
    let mut cursor = 0;
    let mut found = None;
    for item in &index.items {
        let needle = format!("\"{}\"", item.id);
        let Some(offset) = raw[cursor..].find(&needle) else {
            continue;
        };
        let at = cursor + offset;
        if at > byte {
            break;
        }
        found = Some(Place::Material {
            id: item.id.clone(),
            title: item.title.clone(),
        });
        cursor = at + needle.len();
    }
    found
}

/// 資料の本体: 索引の `path` が一致する項目。無ければファイル名の `M0137-…` から ID だけ。
fn material_doc(document: &Document, project: &Project) -> Option<Place> {
    let materials = project.materials();
    if let Some(index) = &materials.index {
        if let Some(item) = index.items.iter().find(|item| item.path == document.path) {
            return Some(Place::Material {
                id: item.id.clone(),
                title: item.title.clone(),
            });
        }
    }
    let name = document.path.rsplit('/').next()?;
    let id = name.split(['-', '.']).next()?;
    let is_id =
        id.len() == 5 && id.starts_with('M') && id[1..].bytes().all(|byte| byte.is_ascii_digit());
    is_id.then(|| Place::Material {
        id: id.to_owned(),
        title: String::new(),
    })
}
