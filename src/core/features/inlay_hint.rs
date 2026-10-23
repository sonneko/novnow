//! インレイヒント（§3.4 `inlay_hint::range(doc, range, project)` → `textDocument/inlayHint`）。
//!
//! §3.4 の表の「資料タイトル・モード名の薄表示」の 2 種を返す（`docs/history.md` B-82）。
//!
//! - **資料タイトル**（§6.6.2）: `'M0137` の直後に `⟨タイトル⟩`。中身は M3 の
//!   [`materials::Hint`]（24 字超は 23 字 + `…`、`status`）。
//! - **モード名**: `'リリア:B` と話者注釈 `@リリア:B` の直後に、モードの表示名（§5.5.2
//!   `mode_label`、`### B 呆れ（強）` の `呆れ（強）`）。同じ ID の変種が複数あるとき
//!   （§6.4.5）は**その行の節を含む変種**の表示名。未定義（E003）・表示名の無いモードは出さない。
//!
//! **括弧 `⟨⟩` と「提案中」「却下」の添え文言は UI が組む**（§12.2）。文字色・サイズ・`Alt+H`・
//! 選択やコピーの対象にしない・エクスポートで出さない（すべて MUST）も UI 側。

use std::ops::Range;

use serde::Serialize;

use crate::core::features::project::{self, Project};
use crate::core::id::SceneId;
use crate::core::model::document::Document;
use crate::core::model::material::MaterialStatus;
use crate::core::resolve::dictionary::Target;
use crate::core::resolve::{materials, modes};
use crate::core::syntax::characters_md::Characters;

/// ヒント 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InlayHint {
    /// 表示位置（トークン直後のファイル内 byte 位置、§6.6.2）。
    pub at: usize,
    /// 中身。
    pub kind: InlayKind,
}

/// ヒントの中身（文言は UI が組む）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum InlayKind {
    /// 資料タイトル（§6.6.2）。
    MaterialTitle {
        /// 表示するタイトル（切り詰め済み）。
        title: String,
        /// 23 字 + `…` に切り詰めたか。
        truncated: bool,
        /// `proposed` / `rejected` なら UI が「提案中」「却下」を添える。
        status: Option<MaterialStatus>,
    },
    /// モードの表示名（書かれた順。`B+C` なら 2 つ）。
    ModeLabels {
        /// 表示名。
        labels: Vec<String>,
    },
}

/// `range` の中（両端を含む）のヒント。位置の昇順。
pub fn range(document: &Document, range: Range<usize>, project: &Project) -> Vec<InlayHint> {
    let Some(format) = document.format.text_format() else {
        return Vec::new();
    };
    let inside = |at: usize| range.start <= at && at <= range.end;
    let mut out = Vec::new();

    // 資料タイトル（§6.6.2）。入口は M3 の `materials::scan_line`（B-71）
    let index = project.materials();
    for line in project::lines_of(&document.text) {
        let line_end = line.start + line.text.len();
        if line_end < range.start || line.start > range.end {
            continue;
        }
        let keep = project::scan_ranges(line.text, format);
        if keep.is_empty() {
            continue;
        }
        let masked = project::mask(line.text, &keep);
        let scan = materials::scan_line(&masked, line.start, project.dictionary(), &index, false);
        for resolved in scan.resolutions {
            if let Some(hint) = resolved.hint {
                if inside(hint.at) {
                    out.push(InlayHint {
                        at: hint.at,
                        kind: InlayKind::MaterialTitle {
                            title: hint.title,
                            truncated: hint.truncated,
                            status: hint.status,
                        },
                    });
                }
            }
        }
    }

    // モードの表示名
    let characters = project.characters();
    let scenes = project.scenes_by_line(document);
    for site in project.ref_sites(document) {
        let Some(ids) = site.token.modifier.as_ref().and_then(|found| found.modes()) else {
            continue;
        };
        let Some(Target::Character { index, .. }) = site
            .entry()
            .and_then(|id| project.dictionary().entry(id))
            .map(|entry| &entry.target)
        else {
            continue;
        };
        let at = site.range().end;
        let scene = scenes.get(site.line).copied().flatten();
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        if let Some(hint) = mode_hint(&characters, *index, &ids, scene, at) {
            if inside(at) {
                out.push(hint);
            }
        }
    }
    for site in project.speaker_sites(document) {
        if site.modes.is_empty() {
            continue;
        }
        let Some(Target::Character { index, .. }) = site
            .entry
            .and_then(|id| project.dictionary().entry(id))
            .map(|entry| &entry.target)
        else {
            continue;
        };
        let at = site.end();
        let scene = scenes.get(site.line).copied().flatten();
        let ids: Vec<&str> = site.modes.iter().map(|(id, _)| id.as_str()).collect();
        if let Some(hint) = mode_hint(&characters, *index, &ids, scene, at) {
            if inside(at) {
                out.push(hint);
            }
        }
    }
    out.sort_by_key(|hint| hint.at);
    out
}

/// モード ID の表示名を集める。1 つも無ければ `None`。
fn mode_hint(
    characters: &Characters<'_>,
    character: usize,
    ids: &[&str],
    scene: Option<SceneId>,
    at: usize,
) -> Option<InlayHint> {
    let mut labels = Vec::new();
    for id in ids {
        let variants = modes::definitions_with_id(characters, character, id);
        let chosen = scene
            .and_then(|found| variants.iter().find(|variant| variant.contains(found)))
            .or_else(|| variants.first());
        if let Some(found) = chosen {
            if !found.label().is_empty() {
                labels.push(found.label().to_string());
            }
        }
    }
    (!labels.is_empty()).then_some(InlayHint {
        at,
        kind: InlayKind::ModeLabels { labels },
    })
}
