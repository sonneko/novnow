//! キャラ名・用語の一括改名（§3.4 `rename::prepare/apply` → `textDocument/rename`）。
//!
//! # 何を改名するか（`docs/history.md` B-84）
//!
//! **カーソルの下の「キー」1 つ**（H1 の表示名か、`alias:` の 1 項目）。`'アルス` の上なら
//! `アルス` を、別名 `'錬金少年` の上なら別名 `錬金少年` を改名する。書き換えるのは
//!
//! - 定義（`characters.md` / `words.md` の H1、または `alias:` のその項目）
//! - そのキーで解決された全文書の `'` 参照（別名で書かれた参照は**別名のまま**残る）
//! - キャラなら、そのキーと完全一致する話者注釈 `@名前`（§6.2.5、B-61）
//!
//! 資料（ID・タイトル）は `materials/index.json` が正本なので改名の対象外（`None`）。
//!
//! # 安全性
//!
//! - 空・区切り文字（§6.2.1 の `DELIMITERS` と空白）や `'` `@` `:` `{` `}` `+` を含む名前は
//!   [`RenameError::Invalid`]。正規化（§6.1.3）して既存のキーと同じになる名前は
//!   [`RenameError::Duplicate`]（W027 / E008 を自分で作らない）。
//! - 新しい名前の**真の接頭辞が別のキー**だと、最短一致（§6.2.1）で `'新しい名前` が短いほうに
//!   解決されてしまう（§6.2.2）。そのときは参照を**明示形 `'{新しい名前}`** で書く。

use std::ops::Range;

use serde::Serialize;

use crate::core::features::project::{Project, SiteAt};
use crate::core::features::{TextEdit, WorkspaceEdit};
use crate::core::model::document::Document;
use crate::core::resolve::dictionary::{self, EntryId, EntryKind};

/// 名前に使えない文字（§6.2 の参照の構文に使う記号）。区切り文字と空白は別に見る。
const FORBIDDEN: &[char] = &['\'', '@', ':', '{', '}', '+', '\\'];

/// 改名の準備（LSP の `prepareRename`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrepareRename {
    /// この文書の中で改名の対象になる範囲（キーの部分）。
    pub range: Range<usize>,
    /// いまの名前（入力欄の初期値）。
    pub placeholder: String,
}

/// 改名できない理由（文言は UI、§12.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum RenameError {
    /// カーソルの下に改名できるもの（解決済みのキャラ・用語のキー）が無い。
    NotRenamable,
    /// 新しい名前が空。
    Empty,
    /// 新しい名前に使えない文字がある。
    Invalid,
    /// 新しい名前が既存のキーと（正規化して）同じ。
    Duplicate,
}

/// `pos` のキーが改名できるなら、その範囲といまの名前。
pub fn prepare(document: &Document, pos: usize, project: &Project) -> Option<PrepareRename> {
    let (id, range) = key_at(document, pos, project)?;
    let entry = project.dictionary().entry(id)?;
    Some(PrepareRename {
        range,
        placeholder: entry.key.clone(),
    })
}

/// `pos` のキーを `new_name` に改名する編集。
pub fn apply(
    document: &Document,
    pos: usize,
    new_name: &str,
    project: &Project,
) -> Result<WorkspaceEdit, RenameError> {
    let (id, _) = key_at(document, pos, project).ok_or(RenameError::NotRenamable)?;
    let dictionary = project.dictionary();
    let entry = dictionary.entry(id).ok_or(RenameError::NotRenamable)?;
    let new_name = new_name.trim_matches(|c: char| c.is_whitespace());
    if new_name.is_empty() {
        return Err(RenameError::Empty);
    }
    let invalid = new_name
        .chars()
        .any(|c| c.is_whitespace() || dictionary::is_delimiter(c) || FORBIDDEN.contains(&c));
    if invalid {
        return Err(RenameError::Invalid);
    }
    let normalized = dictionary::normalize(new_name);
    let taken = dictionary
        .entries()
        .iter()
        .any(|other| other.normalized == normalized && other.key != entry.key);
    if taken {
        return Err(RenameError::Duplicate);
    }
    if new_name == entry.key {
        return Ok(WorkspaceEdit::default());
    }
    // §6.2.2: 真の接頭辞が別のキーなら、参照は明示形で書く
    let explicit = new_name
        .char_indices()
        .skip(1)
        .any(|(at, _)| dictionary.lookup(&new_name[..at], None).is_some());

    let mut edit = WorkspaceEdit::default();
    if let Some(range) = entry.origin.range.clone() {
        edit.push(
            entry.origin.file.path(),
            TextEdit {
                range,
                new_text: new_name.to_string(),
            },
        );
    }
    for found in project.documents() {
        for site in project.ref_sites(found) {
            if site.entry() != Some(id) {
                continue;
            }
            let replacement = if site.token.explicit || !explicit {
                TextEdit {
                    range: site.key_range(),
                    new_text: new_name.to_string(),
                }
            } else {
                TextEdit {
                    range: site.range().start..site.key_range().end,
                    new_text: format!(
                        "{}{}{new_name}{}",
                        dictionary::REF_SIGIL,
                        dictionary::EXPLICIT_OPEN,
                        dictionary::EXPLICIT_CLOSE
                    ),
                }
            };
            edit.push(&found.path, replacement);
        }
        if entry.kind == EntryKind::Character {
            for site in project.speaker_sites(found) {
                if site.entry == Some(id) && site.name == entry.key {
                    edit.push(
                        &found.path,
                        TextEdit {
                            range: site.name_range.clone(),
                            new_text: new_name.to_string(),
                        },
                    );
                }
            }
        }
    }
    Ok(edit)
}

/// `pos` の下のキー（エントリ）と、この文書の中のその範囲。キャラ・用語だけ。
fn key_at(document: &Document, pos: usize, project: &Project) -> Option<(EntryId, Range<usize>)> {
    let dictionary = project.dictionary();
    let renamable = |id: EntryId| {
        dictionary
            .entry(id)
            .is_some_and(|entry| matches!(entry.kind, EntryKind::Character | EntryKind::Word))
    };
    match project.site_at(document, pos) {
        Some(SiteAt::Ref(site)) => {
            let id = site.entry()?;
            return renamable(id).then(|| (id, site.key_range()));
        }
        Some(SiteAt::Speaker(site)) => {
            let id = site.entry?;
            return renamable(id).then(|| (id, site.name_range.clone()));
        }
        None => {}
    }
    // 定義側（H1 の名前・`alias:` の項目）の上
    dictionary
        .entries()
        .iter()
        .enumerate()
        .find(|(_, entry)| {
            entry.origin.file.path() == document.path
                && matches!(entry.kind, EntryKind::Character | EntryKind::Word)
                && entry
                    .origin
                    .range
                    .as_ref()
                    .is_some_and(|range| range.start <= pos && pos <= range.end)
        })
        .and_then(|(_, entry)| {
            let id = dictionary.lookup(&entry.key, Some(&[entry.kind]))?;
            Some((id, entry.origin.range.clone()?))
        })
}
