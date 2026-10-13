//! 使用箇所（§3.4 `references::at(doc, pos, project)` → `textDocument/references`）。
//!
//! v1 の用途は「この用語を使っている節」（§3.4 の表）。`pos` のトークン（または定義側の
//! H1 の名前・別名）と**同じ実体**を指す `'` 参照と話者注釈を、プロジェクトの全文書から集める
//! （別名で書いたものも含む。比較は実体＝キャラ・用語の添字、資料の ID）。定義そのものは
//! 含めない（LSP の `includeDeclaration: false` に当たる）。並びはパス → 位置。

use crate::core::features::project::Project;
use crate::core::features::Location;
use crate::core::model::document::Document;
use crate::core::resolve::dictionary::SourceFile;

/// `pos` の実体を指している参照の一覧。
pub fn at(document: &Document, pos: usize, project: &Project) -> Vec<Location> {
    if let Some(site) = project.site_at(document, pos) {
        return match project.target_of(&site) {
            Some(target) => project.references_to(target),
            None => Vec::new(),
        };
    }
    // 定義側（`characters.md` / `words.md` の H1 の名前や別名）の上でも引けるようにする
    let is_source = [SourceFile::Characters, SourceFile::Words]
        .iter()
        .any(|file| file.path() == document.path);
    if !is_source {
        return Vec::new();
    }
    let entry = project.dictionary().entries().iter().find(|entry| {
        entry.origin.file.path() == document.path
            && entry
                .origin
                .range
                .as_ref()
                .is_some_and(|range| range.start <= pos && pos <= range.end)
    });
    match entry {
        Some(found) => project.references_to(&found.target),
        None => Vec::new(),
    }
}
