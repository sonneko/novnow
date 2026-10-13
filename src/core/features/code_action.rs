//! クイックフィックス（§3.4 `code_action::at(doc, range, project)` → `textDocument/codeAction`、
//! §7 の「クイックフィックス」列）。
//!
//! `range` に掛かる診断（[`diagnostics::document`]）ごとに、**適用後のテキスト編集**を返す
//! 純粋関数（`PLANS.md` §5.5 作業項目 2）。表示名は [`ActionKind`] で返し、文言は UI が
//! `ui/i18n/` で引く（§12.2）。候補の名前（置き換え先のキーなど）は [`CodeAction::arg`]。
//!
//! # ① で編集を返すもの（`PLANS.md` §9.2 B-86）
//!
//! | コード | §7 のクイックフィックス | [`ActionKind`] |
//! |---|---|---|
//! | E001 | 用語として登録 / キャラとして登録 / 類似候補に置換 | `RegisterWord` / `RegisterCharacter` / `ReplaceWithCandidate`（近い順に 3 件） |
//! | E002 | 近い形式に修正 | `CloseExplicitForm`（閉じていない `'{…` に `}` を足す。最長の辞書キーの後ろ、無ければ最初の空白の手前） |
//! | E003 / E004 | 既存モードに置換 / 他モードへ | `ReplaceMode`（E003 は全モード、E004 はその節で使えるモード） |
//! | E005 | 先頭 2 つを残す | `KeepFirstTwoModes` |
//! | E006 | §6.6.3 の 3 種 | `ReplaceTitle`（既定）/ `RewriteIndexTitle` / `UseOtherMaterialId` |
//! | E007 | 近い ID に置換 | `ReplaceMaterialId`（M3 の候補 5 件） |
//! | E008 / E022 | 後続を採番し直す / 番号をずらす | `Renumber`（同じ階層の空き番号＝最大 + 1） |
//! | E012 | キャラとして登録 | `RegisterCharacter` |
//! | E013 | 候補に置換 | `ReplaceWithCandidate` |
//! | E016 | 入れ替え | `SwapRange` |
//! | E018 | 空の summary を挿入 | `InsertSummary` |
//! | E021 | 空の definition を挿入 | `InsertDefinition` |
//! | E026 | 修飾子を削除 | `RemoveModifier` |
//! | W002 | `\@` にエスケープ | `EscapeAt` |
//! | W011 | 近いキーに置換 | `ReplaceKey`（§5.0.6 の表のうちその形式で使えるキー、近い順に 3 件） |
//! | W012 | 削除 | `RemoveSection`（空の節を閉じた `;` の行を消す） |
//! | W029 | 明示形 `'{…}` に変更 | `UseExplicitForm` |
//!
//! ファイルをまたぐ移動（E020「対応ファイルへ移動」・E024・W031「プロットへ移動」）、
//! 設定や UI の操作（E025「既定値で開く」・W022 / E004 の「range 拡張」・E004「変種を追加」・
//! E003「モードを新規定義」・E007「新規資料を作成」・E017「範囲を分割」）は、テキスト編集
//! だけでは決まらない（どこへ・どの範囲に、を作家が選ぶ）ので ① では返さない（B-86）。

use std::ops::Range;

use serde::Serialize;

use crate::core::diagnostics::{self, Arg, Diagnostic, DiagnosticCode};
use crate::core::features::project::{self, Project, RefSite};
use crate::core::features::{TextEdit, WorkspaceEdit};
use crate::core::id::{Digits, SceneId};
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::resolve::dictionary::{self, EntryKind, ModifierKind, SourceFile, Target};
use crate::core::resolve::{materials, modes};
use crate::core::syntax::{contents, plot_episode, plot_outline};
use crate::core::text::lexer;

/// 類似候補の最大件数（E001 / E013 / W011）。
pub const CANDIDATES: usize = 3;

/// 修正の種類（UI が `ui/i18n/` で文言を引く）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ActionKind {
    /// 用語として `words.md` に登録する（E001）。
    RegisterWord,
    /// キャラとして `characters.md` に登録する（E001 / E012）。
    RegisterCharacter,
    /// 類似候補に置き換える（E001 / E013）。
    ReplaceWithCandidate,
    /// 閉じていない明示形を閉じる（E002）。
    CloseExplicitForm,
    /// 別のモードに置き換える（E003 / E004）。
    ReplaceMode,
    /// 先頭 2 つのモードを残す（E005）。
    KeepFirstTwoModes,
    /// index のタイトルで置き換える（E006、既定）。
    ReplaceTitle,
    /// index 側のタイトルをファイルの記述に合わせる（E006）。
    RewriteIndexTitle,
    /// 同じタイトルを持つ別 ID に貼り替える（E006）。
    UseOtherMaterialId,
    /// 近い資料 ID に置き換える（E007）。
    ReplaceMaterialId,
    /// 見出しの番号を空き番号に振り直す（E008 / E022）。
    Renumber,
    /// 範囲の左右を入れ替える（E016）。
    SwapRange,
    /// 空の `summary:` を挿入する（E018）。
    InsertSummary,
    /// 空の `definition:` を挿入する（E021）。
    InsertDefinition,
    /// 修飾子を削除する（E026）。
    RemoveModifier,
    /// `@` を `\@` にエスケープする（W002）。
    EscapeAt,
    /// 近いフィールドキーに置き換える（W011）。
    ReplaceKey,
    /// 空の節を削除する（W012）。
    RemoveSection,
    /// 明示形 `'{…}` にする（W029）。
    UseExplicitForm,
}

/// 修正 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeAction {
    /// 種類。
    pub kind: ActionKind,
    /// 対象の診断。
    pub diagnostic: DiagnosticCode,
    /// 候補の名前（置き換え先・登録する名前・新しい番号）。無ければ空。
    pub arg: String,
    /// 適用する編集。
    pub edit: WorkspaceEdit,
    /// 既定の修正か（§6.6.3「既定・`Enter` 一発で適用」）。
    pub preferred: bool,
}

/// `range`（両端を含む）に掛かる診断の修正（§3.4）。診断の順（§9.5.2）→ 修正の順。
pub fn at(document: &Document, range: Range<usize>, project: &Project) -> Vec<CodeAction> {
    let found = diagnostics::document(document, project);
    let mut out = Vec::new();
    for diagnostic in &found {
        let Some(span) = &diagnostic.range else {
            continue;
        };
        if span.end < range.start || span.start > range.end {
            continue;
        }
        let context = Context {
            document,
            project,
            diagnostic,
            span: span.clone(),
        };
        out.extend(context.actions());
    }
    out
}

/// 1 つの診断の修正を組む材料。
struct Context<'a> {
    document: &'a Document,
    project: &'a Project,
    diagnostic: &'a Diagnostic,
    span: Range<usize>,
}

impl Context<'_> {
    fn actions(&self) -> Vec<CodeAction> {
        match self.diagnostic.code {
            DiagnosticCode::E001 => self.unresolved(),
            DiagnosticCode::E002 => self.close_explicit().into_iter().collect(),
            DiagnosticCode::E003 | DiagnosticCode::E004 => self.replace_mode(),
            DiagnosticCode::E005 => self.keep_two().into_iter().collect(),
            DiagnosticCode::E006 => self.title_fixes(),
            DiagnosticCode::E007 => self.material_ids(),
            DiagnosticCode::E008 | DiagnosticCode::E022 => self.renumber().into_iter().collect(),
            DiagnosticCode::E012 => vec![self.register(EntryKind::Character, &self.text_arg(0))],
            DiagnosticCode::E013 => self.candidates(&self.text_arg(0), &[EntryKind::Character]),
            DiagnosticCode::E016 => self.swap_range().into_iter().collect(),
            DiagnosticCode::E018 => {
                vec![self.insert_after_line("summary: ", ActionKind::InsertSummary)]
            }
            DiagnosticCode::E021 => {
                vec![self.insert_after_line("definition: ", ActionKind::InsertDefinition)]
            }
            DiagnosticCode::E026 => vec![self.action(
                ActionKind::RemoveModifier,
                String::new(),
                self.here(self.span.clone(), ""),
                false,
            )],
            DiagnosticCode::W002 => vec![self.action(
                ActionKind::EscapeAt,
                String::new(),
                self.here(self.span.start..self.span.start, "\\"),
                false,
            )],
            DiagnosticCode::W011 => self.replace_key(),
            DiagnosticCode::W012 => vec![self.remove_line()],
            DiagnosticCode::W029 => self.explicit_form().into_iter().collect(),
            _ => Vec::new(),
        }
    }

    fn action(
        &self,
        kind: ActionKind,
        arg: String,
        edit: WorkspaceEdit,
        preferred: bool,
    ) -> CodeAction {
        CodeAction {
            kind,
            diagnostic: self.diagnostic.code,
            arg,
            edit,
            preferred,
        }
    }

    /// この文書だけの 1 編集。
    fn here(&self, range: Range<usize>, text: &str) -> WorkspaceEdit {
        WorkspaceEdit::single(
            self.document.path.clone(),
            vec![TextEdit {
                range,
                new_text: text.to_string(),
            }],
        )
    }

    fn text_arg(&self, index: usize) -> String {
        match self.diagnostic.args.get(index) {
            Some(Arg::Text(value)) => value.clone(),
            Some(Arg::Number(value)) => value.to_string(),
            None => String::new(),
        }
    }

    /// 診断の範囲と同じ（または含む）`'` 参照。
    fn ref_site(&self) -> Option<RefSite> {
        self.project
            .ref_sites(self.document)
            .into_iter()
            .find(|site| {
                let range = site.range();
                range.start <= self.span.start && self.span.end <= range.end
            })
    }

    // ------------------------------------------------------------ E001 / E012 / E013

    fn unresolved(&self) -> Vec<CodeAction> {
        let key = self.text_arg(0);
        if key.is_empty() {
            return Vec::new();
        }
        let mut out = vec![
            self.register(EntryKind::Word, &key),
            self.register(EntryKind::Character, &key),
        ];
        out.extend(self.candidates(&key, &[EntryKind::Character, EntryKind::Word]));
        out
    }

    /// `characters.md` / `words.md` の末尾に項目を足す（無ければファイルを作る）。
    fn register(&self, kind: EntryKind, name: &str) -> CodeAction {
        let (path, block, action) = match kind {
            EntryKind::Word => (
                SourceFile::Words.path(),
                format!("# {name}\ndefinition: \n"),
                ActionKind::RegisterWord,
            ),
            _ => (
                SourceFile::Characters.path(),
                format!("# {name}\n"),
                ActionKind::RegisterCharacter,
            ),
        };
        let text = self.project.text(path);
        let separator = if text.is_empty() || text.ends_with("\n\n") {
            ""
        } else if text.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        let edit = WorkspaceEdit::single(
            path,
            vec![TextEdit {
                range: text.len()..text.len(),
                new_text: format!("{separator}{block}"),
            }],
        );
        self.action(action, name.to_string(), edit, false)
    }

    /// 近いキーへの置き換え（正規化後のレーベンシュタイン距離、近い順・同点はキー順）。
    fn candidates(&self, key: &str, kinds: &[EntryKind]) -> Vec<CodeAction> {
        let wanted = dictionary::normalize(key);
        let limit = (wanted.chars().count() as u32 / 2).max(1);
        let mut found: Vec<(u32, String)> = Vec::new();
        for entry in self.project.dictionary().entries() {
            if !kinds.contains(&entry.kind) || entry.normalized == wanted {
                continue;
            }
            let distance = materials::distance(&wanted, &entry.normalized);
            if distance <= limit && !found.iter().any(|(_, seen)| *seen == entry.key) {
                found.push((distance, entry.key.clone()));
            }
        }
        found.sort();
        // 置き換えるのはキーの部分（`'` と `{}` は残す）。E013 は `'名前` の名前の部分
        let target = match self.ref_site() {
            Some(site) => site.key_range(),
            None => {
                let text = self
                    .document
                    .text
                    .get(self.span.clone())
                    .unwrap_or_default();
                match text.find(key) {
                    Some(at) => self.span.start + at..self.span.start + at + key.len(),
                    None => return Vec::new(),
                }
            }
        };
        found
            .into_iter()
            .take(CANDIDATES)
            .map(|(_, name)| {
                self.action(
                    ActionKind::ReplaceWithCandidate,
                    name.clone(),
                    self.here(target.clone(), &name),
                    false,
                )
            })
            .collect()
    }

    // ------------------------------------------------------------ E002

    fn close_explicit(&self) -> Option<CodeAction> {
        let site = self.ref_site()?;
        if !site.token.unclosed {
            return None;
        }
        let key = site.key_range();
        let inner = self.document.text.get(key.clone())?;
        // 最長の辞書キーで閉じる。無ければ最初の空白の手前、それも無ければ末尾
        let mut close = None;
        for (at, c) in inner.char_indices() {
            let end = at + c.len_utf8();
            if self
                .project
                .dictionary()
                .lookup(&inner[..end], None)
                .is_some()
            {
                close = Some(end);
            }
        }
        let close = close
            .or_else(|| inner.find(|c: char| c.is_whitespace()))
            .unwrap_or(inner.len());
        let at = key.start + close;
        Some(self.action(
            ActionKind::CloseExplicitForm,
            inner[..close].to_string(),
            self.here(at..at, &dictionary::EXPLICIT_CLOSE.to_string()),
            true,
        ))
    }

    // ------------------------------------------------------------ E003 / E004 / E005

    fn replace_mode(&self) -> Vec<CodeAction> {
        let name = self.text_arg(0);
        let wrong = self.text_arg(1);
        let dictionary = self.project.dictionary();
        let Some(Target::Character { index, .. }) = dictionary
            .lookup(&name, Some(&[EntryKind::Character]))
            .and_then(|id| dictionary.entry(id))
            .map(|entry| entry.target.clone())
        else {
            return Vec::new();
        };
        let Some(range) = self.mode_range(&wrong) else {
            return Vec::new();
        };
        let characters = self.project.characters();
        let scene = self.project.scene_at(
            self.document,
            project::line_of(&self.document.text, self.span.start),
        );
        let mut ids: Vec<String> = Vec::new();
        for definition in modes::definitions(&characters, index) {
            let id = definition.id().to_string();
            if id.eq_ignore_ascii_case(&wrong)
                || ids.iter().any(|seen| seen.eq_ignore_ascii_case(&id))
            {
                continue;
            }
            let usable = self.diagnostic.code == DiagnosticCode::E003
                || scene.is_none_or(|at| {
                    modes::definitions_with_id(&characters, index, &id)
                        .iter()
                        .any(|variant| variant.contains(at))
                });
            if usable {
                ids.push(id);
            }
        }
        ids.into_iter()
            .map(|id| {
                self.action(
                    ActionKind::ReplaceMode,
                    id.clone(),
                    self.here(range.clone(), &id),
                    false,
                )
            })
            .collect()
    }

    /// 診断の範囲の中で、モード ID `id` が書かれている範囲。
    fn mode_range(&self, id: &str) -> Option<Range<usize>> {
        let text = self.document.text.get(self.span.clone())?;
        let colon = text.rfind(':')?;
        let mut at = colon + 1;
        for part in text[colon + 1..].split('+') {
            let trimmed = part.trim_end_matches(|c: char| c.is_whitespace());
            if trimmed.eq_ignore_ascii_case(id) {
                let start = self.span.start + at;
                return Some(start..start + trimmed.len());
            }
            at += part.len() + 1;
        }
        None
    }

    fn keep_two(&self) -> Option<CodeAction> {
        let site = self.ref_site()?;
        let modifier = site.token.modifier.as_ref()?;
        if modifier.kind != ModifierKind::TooManyModes {
            return None;
        }
        let kept: Vec<&str> = modifier.text.split('+').take(2).collect();
        let kept = kept.join("+");
        let range = shift(&modifier.text_range, site.line_start);
        Some(self.action(
            ActionKind::KeepFirstTwoModes,
            kept.clone(),
            self.here(range, &kept),
            true,
        ))
    }

    // ------------------------------------------------------------ E006 / E007

    /// M3 の `materials::resolve` の結果（修正の中身は M3 が作っている、B-72）。
    fn with_resolved<T>(
        &self,
        then: impl FnOnce(&RefSite, &materials::Resolved<'_>) -> T,
    ) -> Option<T> {
        let site = self.ref_site()?;
        let index = self.project.materials();
        let line = self.document.text.split('\n').nth(site.line)?;
        let resolved = materials::resolve(
            line,
            &site.token,
            self.project.dictionary(),
            &index,
            site.line_start,
        );
        Some(then(&site, &resolved))
    }

    fn title_fixes(&self) -> Vec<CodeAction> {
        self.with_resolved(|site, resolved| {
            let mut out = Vec::new();
            let (Some(fixes), Some(modifier)) = (&resolved.fixes, &site.token.modifier) else {
                return out;
            };
            let title_range = shift(&modifier.text_range, site.line_start);
            out.push(self.action(
                ActionKind::ReplaceTitle,
                fixes.replace_title.clone(),
                self.here(title_range, &fixes.replace_title),
                true,
            ));
            if let Some(edit) = self.rewrite_index(resolved, &fixes.rewrite_index) {
                out.push(self.action(
                    ActionKind::RewriteIndexTitle,
                    fixes.rewrite_index.clone(),
                    edit,
                    false,
                ));
            }
            if let Some(other) = &fixes.other_id {
                let id = other.to_string();
                let key = site.key_range();
                out.push(self.action(
                    ActionKind::UseOtherMaterialId,
                    id.clone(),
                    self.here(key, &id),
                    false,
                ));
            }
            out
        })
        .unwrap_or_default()
    }

    /// `materials/index.json` の該当資料の `title` の値を書き換える。原文に同じ書き方の
    /// タイトルが 1 か所だけあるときに限る（JSON を整形し直さない、§4.3-4 の「原文を壊さない」）。
    fn rewrite_index(
        &self,
        resolved: &materials::Resolved<'_>,
        title: &str,
    ) -> Option<WorkspaceEdit> {
        let material = resolved.material?;
        let path = SourceFile::MaterialsIndex.path();
        let raw = self.project.text(path);
        let old = serde_json::to_string(&material.title).ok()?;
        let new = serde_json::to_string(title).ok()?;
        if raw.matches(&old).count() != 1 {
            return None;
        }
        let at = raw.find(&old)?;
        Some(WorkspaceEdit::single(
            path,
            vec![TextEdit {
                range: at..at + old.len(),
                new_text: new,
            }],
        ))
    }

    fn material_ids(&self) -> Vec<CodeAction> {
        self.with_resolved(|site, resolved| {
            resolved
                .suggestions
                .iter()
                .map(|material| {
                    self.action(
                        ActionKind::ReplaceMaterialId,
                        material.id.clone(),
                        self.here(site.key_range(), &material.id),
                        false,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
    }

    // ------------------------------------------------------------ E008 / E022

    /// 見出しの番号を、同じ階層の最大 + 1 に振り直す。
    fn renumber(&self) -> Option<CodeAction> {
        let text = self.document.text.get(self.span.clone())?;
        let written = text.strip_prefix('@')?;
        let digits = self.project.digits();
        let fresh = match self.document.format {
            DocumentFormat::Contents | DocumentFormat::PlotEpisode => {
                let used = self.used_scenes();
                let id = crate::core::id::SceneId::parse(written).ok()?;
                let next = used
                    .iter()
                    .filter(|other| other.chapter == id.chapter && other.episode == id.episode)
                    .map(|other| other.scene)
                    .max()
                    .unwrap_or(0)
                    + 1;
                let depth = written.split('-').count();
                display(
                    SceneId {
                        scene: next,
                        section: 0,
                        ..id
                    },
                    depth.min(3),
                    digits,
                )
            }
            DocumentFormat::PlotOutline => {
                let parsed = plot_outline::parse(&self.document.text);
                let depth = written.split('-').count();
                let id = crate::core::id::PartialSceneId::parse(written)
                    .ok()?
                    .expand_low();
                let level = |found: &SceneId| match depth {
                    1 => found.chapter,
                    2 => found.episode,
                    3 => found.scene,
                    _ => found.section,
                };
                let next = parsed
                    .blocks
                    .iter()
                    .filter(|block| block.depth == depth)
                    .filter_map(|block| block.id)
                    .filter(|other| depth == 1 || other.chapter == id.chapter)
                    .map(|other| level(&other))
                    .max()
                    .unwrap_or(0)
                    + 1;
                let fresh = match depth {
                    1 => SceneId {
                        chapter: next,
                        ..id
                    },
                    2 => SceneId {
                        episode: next,
                        ..id
                    },
                    3 => SceneId { scene: next, ..id },
                    _ => SceneId {
                        section: next,
                        ..id
                    },
                };
                display(fresh, depth, digits)
            }
            _ => return None,
        };
        let start = self.span.start + 1;
        Some(self.action(
            ActionKind::Renumber,
            fresh.clone(),
            self.here(start..self.span.end, &fresh),
            true,
        ))
    }

    /// C / B 形式の文書の場面見出しの ID。
    fn used_scenes(&self) -> Vec<SceneId> {
        let options = self.project.contents_options();
        match self.document.format {
            DocumentFormat::Contents => contents::parse_with(&self.document.text, &options)
                .lines
                .iter()
                .filter_map(|line| match line {
                    contents::Line::Scene(found) => found.id,
                    _ => None,
                })
                .collect(),
            DocumentFormat::PlotEpisode => plot_episode::parse_with(&self.document.text, &options)
                .scenes
                .iter()
                .filter_map(|scene| scene.id)
                .collect(),
            _ => Vec::new(),
        }
    }

    // ------------------------------------------------------------ E016 / E018 / E021 / W012

    fn swap_range(&self) -> Option<CodeAction> {
        let text = self.document.text.get(self.span.clone())?;
        let op = text.find("..")?;
        let left = &text[..op];
        let right = &text[op + 2..];
        let (left_body, right_body) = (left.trim(), right.trim());
        if left_body.is_empty() || right_body.is_empty() {
            return None;
        }
        let left_pad = &left[left_body.len()..];
        let right_pad = &right[..right.len() - right.trim_start().len()];
        let swapped = format!("{right_body}{left_pad}..{right_pad}{left_body}");
        Some(self.action(
            ActionKind::SwapRange,
            swapped.clone(),
            self.here(self.span.clone(), &swapped),
            true,
        ))
    }

    /// 診断の行（見出し）の直後に 1 行を挿入する。
    fn insert_after_line(&self, line: &str, kind: ActionKind) -> CodeAction {
        let text = &self.document.text;
        let end = text[self.span.start..]
            .find('\n')
            .map_or(text.len(), |at| self.span.start + at);
        let edit = if end < text.len() {
            self.here(end + 1..end + 1, &format!("{line}\n"))
        } else {
            self.here(end..end, &format!("\n{line}"))
        };
        self.action(kind, String::new(), edit, true)
    }

    /// 診断の行（空の節を閉じた `;`）を改行ごと消す。
    fn remove_line(&self) -> CodeAction {
        let text = &self.document.text;
        let start = text[..self.span.start].rfind('\n').map_or(0, |at| at + 1);
        let end = text[self.span.start..]
            .find('\n')
            .map_or(text.len(), |at| self.span.start + at + 1);
        self.action(
            ActionKind::RemoveSection,
            String::new(),
            self.here(start..end, ""),
            false,
        )
    }

    // ------------------------------------------------------------ W011 / W029

    fn replace_key(&self) -> Vec<CodeAction> {
        let written = self.text_arg(0);
        let Some(format) = self.document.format.text_format() else {
            return Vec::new();
        };
        let mut found: Vec<(u32, &str)> = Vec::new();
        for (_, ascii, alias, formats) in lexer::FIELD_KEYS {
            if !formats.contains(format) {
                continue;
            }
            let distance = materials::distance(&written.to_lowercase(), ascii)
                .min(materials::distance(&written, alias));
            let limit = (written.chars().count() as u32 / 2).max(1);
            if distance <= limit && !found.iter().any(|(_, seen)| seen == ascii) {
                found.push((distance, ascii));
            }
        }
        found.sort();
        found
            .into_iter()
            .take(CANDIDATES)
            .map(|(_, key)| {
                self.action(
                    ActionKind::ReplaceKey,
                    key.to_string(),
                    self.here(self.span.clone(), key),
                    false,
                )
            })
            .collect()
    }

    fn explicit_form(&self) -> Option<CodeAction> {
        let site = self.ref_site()?;
        let dictionary = self.project.dictionary();
        let longer = dictionary.entry(site.token.longer?)?;
        let replacement = site.token.explicit_form(dictionary)?;
        // `'` の直後から、正規化して長いほうのキーになるところまでが書かれた長いキー
        let start = site.key_range().start;
        let rest = self.document.text.get(start..)?;
        let mut end = None;
        for (at, c) in rest.char_indices() {
            if c == '\n' {
                break;
            }
            let stop = at + c.len_utf8();
            if dictionary::normalize(&rest[..stop]) == longer.normalized {
                end = Some(start + stop);
                break;
            }
        }
        let end = end?;
        Some(self.action(
            ActionKind::UseExplicitForm,
            longer.key.clone(),
            self.here(site.range().start..end, &replacement),
            true,
        ))
    }
}

/// `depth` 階層ぶんの ID 文字列（`01-04-03`）。
fn display(id: SceneId, depth: usize, digits: Digits) -> String {
    let levels = [
        (id.chapter, digits.chapter),
        (id.episode, digits.episode),
        (id.scene, digits.scene),
        (id.section, digits.section),
    ];
    levels
        .iter()
        .take(depth.max(1))
        .map(|(value, width)| format!("{value:0width$}", width = usize::from(*width)))
        .collect::<Vec<_>>()
        .join("-")
}

fn shift(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start + by..range.end + by
}
