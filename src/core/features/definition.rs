//! 定義へ移動（§3.4 `definition::at(doc, pos, project)` → `textDocument/definition`）。
//!
//! `'名前` と話者注釈 `@名前` → `characters.md` の H1 の名前（別名で書いても**表示名**へ。
//! 別名は入口にすぎない、§6.1.1）。用語 → `words.md` の H1。資料 → 資料 MD がプロジェクトに
//! あればその 1 行目、無ければ `materials/index.json` の ID（[`Project::definition_of`]）。

use crate::core::features::project::Project;
use crate::core::features::Location;
use crate::core::model::document::Document;
use crate::core::resolve::dictionary::SourceFile;

/// `pos` のトークンの定義の位置。未解決・トークンの外なら `None`。
pub fn at(document: &Document, pos: usize, project: &Project) -> Option<Location> {
    if let Some(site) = project.site_at(document, pos) {
        let target = project.target_of(&site)?;
        return project.definition_of(target);
    }
    // 定義側（`characters.md` / `words.md` の H1 の名前や別名）の上でも引けるようにする
    let is_source = [SourceFile::Characters, SourceFile::Words]
        .iter()
        .any(|file| file.path() == document.path);
    if !is_source {
        return None;
    }
    let entry = project.dictionary().entries().iter().find(|entry| {
        entry.origin.file.path() == document.path
            && entry
                .origin
                .range
                .as_ref()
                .is_some_and(|range| range.start <= pos && pos <= range.end)
    })?;
    project.definition_of(&entry.target)
}
