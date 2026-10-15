//! 入力補完（§3.4 `completion::at(doc, pos, project)` → `textDocument/completion`、§9.7.3）。
//!
//! | きっかけ | 候補 |
//! |---|---|
//! | `'` の後ろ（打ちかけのキーを含む。`'{` も） | キャラ → 用語 → 資料の順、**各最大 8 件**（§9.7.3） |
//! | キャラ参照・話者注釈の直後の `:`（`+` の後ろも） | そのキャラのモード。**`range` 外は灰色で下に並べる**（[`CompletionItem::faint`]） |
//! | 行頭の `@` | 近くの SceneId（次の場面、今の場面の次の節。A 形式は次の章・話） |
//! | 行末の `@` | キャラ（話者注釈） |
//!
//! # 並び順（§9.7.3、決定論）
//!
//! 「現在の節に登場 > 同じ話に登場 > 直近使用 > 五十音（`reading` 優先）」。**直近使用は UI の
//! 状態**（core は持たない、§3.2）なので、ここでは飛ばして残りの 3 段で並べる（`PLANS.md`
//! §9.2 B-85。UI が直近使用で安定ソートし直せば仕様の順になる）。「同じ話」は**この文書**
//! （本文・話プロットは 1 ファイル 1 話、§4.1）。五十音は `reading:`、無ければキーそのものの
//! 文字コード順。
//!
//! # 挿入する文字列
//!
//! キーをそのまま挿入すると最短一致（§6.2.1）で別の項目に解決される場合（`リリア` がある
//! ときの `リリアナ`）や、キーが区切り文字を含む場合は**明示形 `{キー}`** を挿入する。

use std::ops::Range;

use serde::Serialize;

use crate::core::features::project::{self, Project};
use crate::core::id::{Digits, SceneId};
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::resolve::dictionary::{self, EntryKind, Target};
use crate::core::resolve::modes;
use crate::core::syntax::{contents, plot_episode, plot_outline};
use crate::core::text::lexer::FieldKey;

/// 種別ごとの最大件数（§9.7.3「各最大 8 件」）。
pub const PER_KIND: usize = 8;

/// 候補の種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CompletionKind {
    /// キャラ（`'` の後ろ・話者注釈）。
    Character,
    /// 用語。
    Word,
    /// 資料 ID。
    Material,
    /// モード ID。
    Mode,
    /// 構造見出しの SceneId。
    SceneId,
}

/// 候補 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompletionItem {
    /// 表示する名前（キー・ID）。
    pub label: String,
    /// 種別。
    pub kind: CompletionKind,
    /// 補足（資料のタイトル・モードの表示名・読み）。無ければ空。
    pub detail: String,
    /// 挿入する文字列。
    pub insert_text: String,
    /// 置き換える範囲（打ちかけの部分。ファイル単位）。
    pub range: Range<usize>,
    /// 灰色で出すか（§9.7.3「`range` 外のモードは灰色で下に並べ」）。
    pub faint: bool,
}

/// `pos` での補完候補（§3.4）。きっかけが無ければ空。
pub fn at(document: &Document, pos: usize, project: &Project) -> Vec<CompletionItem> {
    let pos = project::floor_boundary(&document.text, pos);
    let line_index = project::line_of(&document.text, pos);
    let line_start = document.text[..pos].rfind('\n').map_or(0, |at| at + 1);
    let line_end = document.text[pos..]
        .find('\n')
        .map_or(document.text.len(), |at| pos + at);
    let before = &document.text[line_start..pos];
    let after = &document.text[pos..line_end];

    if let Some(found) = mode_context(before) {
        return mode_items(document, project, line_index, line_start, &found);
    }
    if let Some((start, explicit)) = ref_context(before) {
        let typed = &before[start..];
        return ref_items(
            document,
            project,
            line_index,
            line_start + start..pos,
            typed,
            explicit,
        );
    }
    if let Some(start) = header_context(before) {
        return scene_items(
            document,
            project,
            line_index,
            line_start + start..pos,
            &before[start..],
        );
    }
    if after.trim_matches(|c: char| c.is_whitespace()).is_empty() {
        if let Some(start) = speaker_context(before) {
            let typed = &before[start..];
            return speaker_items(
                document,
                project,
                line_index,
                line_start + start..pos,
                typed,
            );
        }
    }
    Vec::new()
}

// ============================================================ きっかけの判定

/// `'名前:`（または `@名前:`）の後ろのモード。
struct ModeContext<'a> {
    /// キャラの名前（書かれたまま）。
    name: &'a str,
    /// 打ちかけのモード ID の行内の開始位置。
    start: usize,
    /// 打ちかけのモード ID。
    typed: &'a str,
}

fn mode_context(before: &str) -> Option<ModeContext<'_>> {
    let start = before
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
        .last()
        .map_or(before.len(), |(at, _)| at);
    let typed = &before[start..];
    let head = before[..start]
        .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '+');
    let head = head.strip_suffix(':')?;
    // `:` の前が `'名前` か `@名前`
    let name_start = head
        .char_indices()
        .rev()
        .find(|(_, c)| *c == dictionary::REF_SIGIL || *c == '@')
        .map(|(at, _)| at)?;
    let name = head[name_start + 1..].trim_start_matches(dictionary::EXPLICIT_OPEN);
    let name = name.trim_end_matches(dictionary::EXPLICIT_CLOSE);
    if name.is_empty()
        || name
            .chars()
            .any(|c| c.is_whitespace() || dictionary::is_delimiter(c))
    {
        return None;
    }
    Some(ModeContext { name, start, typed })
}

/// `'` の後ろの打ちかけ。`(キーの行内の開始位置, 明示形 '{ か)`。
fn ref_context(before: &str) -> Option<(usize, bool)> {
    let quote = before.rfind(dictionary::REF_SIGIL)?;
    // 直前が ASCII 英数字なら英語のアポストロフィ（B-60）
    if before[..quote]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '\\')
    {
        return None;
    }
    let rest = &before[quote + 1..];
    if let Some(inner) = rest.strip_prefix(dictionary::EXPLICIT_OPEN) {
        if inner.contains(dictionary::EXPLICIT_CLOSE) {
            return None;
        }
        return Some((quote + 1 + 1, true));
    }
    if rest
        .chars()
        .any(|c| c.is_whitespace() || dictionary::is_delimiter(c) || c == ':')
    {
        return None;
    }
    Some((quote + 1, false))
}

/// 行頭の `@`（構造見出し）。打ちかけは数字と `-` だけ。
fn header_context(before: &str) -> Option<usize> {
    let body = before.trim_start_matches(crate::core::text::lexer::is_space);
    let rest = body.strip_prefix('@')?;
    if !rest.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return None;
    }
    Some(before.len() - rest.len())
}

/// 行末の `@`（話者注釈）。`@` の前に本文がある。
fn speaker_context(before: &str) -> Option<usize> {
    let at = before.rfind('@')?;
    let rest = &before[at + 1..];
    if rest
        .chars()
        .any(|c| c.is_whitespace() || dictionary::is_delimiter(c) || c == ':')
    {
        return None;
    }
    if before[..at].trim().is_empty() || before[..at].ends_with('\\') {
        return None;
    }
    Some(at + 1)
}

// ============================================================ 候補

fn ref_items(
    document: &Document,
    project: &Project,
    line: usize,
    range: Range<usize>,
    typed: &str,
    explicit: bool,
) -> Vec<CompletionItem> {
    let dictionary = project.dictionary();
    let wanted = dictionary::normalize(typed);
    let ranks = Ranks::new(document, project, line);
    let readings = Readings::new(project);
    let mut out = Vec::new();
    for kind in [EntryKind::Character, EntryKind::Word, EntryKind::Material] {
        // 実体ごとに 1 件（H1 が打ちかけに合えば H1、合わなければ最初に合った別名）
        let mut chosen: Vec<(String, Target, String)> = Vec::new();
        for entry in dictionary.entries() {
            let same_kind = entry.kind == kind
                || (kind == EntryKind::Material && entry.kind == EntryKind::MaterialTitle);
            if !same_kind || !entry.normalized.starts_with(&wanted) {
                continue;
            }
            // §6.1.4（F-24）: `characters.md` にもある用語は出さない（登場人物側に統合する）
            if entry.kind == EntryKind::Word && dictionary.is_character_word_duplicate(&entry.key) {
                continue;
            }
            let label = match &entry.target {
                Target::Material { id } => id.clone(),
                _ => entry.key.clone(),
            };
            let main = match &entry.target {
                Target::Character { name, .. } => name == &entry.key,
                Target::Word { term, .. } => term == &entry.key,
                Target::Material { .. } => entry.kind == EntryKind::Material,
            };
            match chosen
                .iter_mut()
                .find(|(_, target, _)| project::same_target(target, &entry.target))
            {
                Some(slot) if main => slot.0 = label,
                Some(_) => {}
                None => {
                    let key = readings.sort_key(&entry.target, &entry.key);
                    chosen.push((label, entry.target.clone(), key));
                }
            }
        }
        chosen.sort_by(|left, right| {
            (ranks.of(&left.1), &left.2, &left.0).cmp(&(ranks.of(&right.1), &right.2, &right.0))
        });
        for (label, target, _) in chosen.into_iter().take(PER_KIND) {
            let insert = insertion(project, &label, &target, explicit);
            out.push(CompletionItem {
                detail: readings.detail(&target),
                kind: match kind {
                    EntryKind::Character => CompletionKind::Character,
                    EntryKind::Word => CompletionKind::Word,
                    _ => CompletionKind::Material,
                },
                label,
                insert_text: insert,
                range: range.clone(),
                faint: false,
            });
        }
    }
    out
}

fn speaker_items(
    document: &Document,
    project: &Project,
    line: usize,
    range: Range<usize>,
    typed: &str,
) -> Vec<CompletionItem> {
    let wanted = dictionary::normalize(typed);
    let ranks = Ranks::new(document, project, line);
    let readings = Readings::new(project);
    let characters = project.characters();
    let mut chosen: Vec<(String, Target, String)> = characters
        .characters
        .iter()
        .enumerate()
        .filter(|(_, found)| dictionary::normalize(found.name).starts_with(&wanted))
        .map(|(index, found)| {
            let target = Target::Character {
                index,
                name: found.name.to_string(),
            };
            let key = readings.sort_key(&target, found.name);
            (found.name.to_string(), target, key)
        })
        .collect();
    chosen.sort_by(|left, right| {
        (ranks.of(&left.1), &left.2, &left.0).cmp(&(ranks.of(&right.1), &right.2, &right.0))
    });
    chosen
        .into_iter()
        .map(|(label, target, _)| CompletionItem {
            detail: readings.detail(&target),
            kind: CompletionKind::Character,
            insert_text: label.clone(),
            label,
            range: range.clone(),
            faint: false,
        })
        .collect()
}

fn mode_items(
    document: &Document,
    project: &Project,
    line: usize,
    line_start: usize,
    found: &ModeContext<'_>,
) -> Vec<CompletionItem> {
    let dictionary = project.dictionary();
    let Some(Target::Character { index, .. }) = dictionary
        .lookup(found.name, Some(&[EntryKind::Character]))
        .and_then(|id| dictionary.entry(id))
        .map(|entry| entry.target.clone())
    else {
        return Vec::new();
    };
    let characters = project.characters();
    let scene = project.scene_at(document, line);
    let range = line_start + found.start..line_start + found.start + found.typed.len();
    let mut available = Vec::new();
    let mut faint = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for definition in modes::definitions(&characters, index) {
        let id = definition.id().to_string();
        if seen.iter().any(|other| other.eq_ignore_ascii_case(&id)) {
            continue;
        }
        if !id
            .to_ascii_lowercase()
            .starts_with(&found.typed.to_ascii_lowercase())
        {
            continue;
        }
        seen.push(id.clone());
        let variants = modes::definitions_with_id(&characters, index, &id);
        let usable = scene.is_none_or(|at| variants.iter().any(|variant| variant.contains(at)));
        let label = scene
            .and_then(|at| variants.iter().find(|variant| variant.contains(at)))
            .or_else(|| variants.first())
            .map(|variant| variant.label().to_string())
            .unwrap_or_default();
        let item = CompletionItem {
            label: id.clone(),
            kind: CompletionKind::Mode,
            detail: label,
            insert_text: id,
            range: range.clone(),
            faint: !usable,
        };
        if usable {
            available.push(item);
        } else {
            faint.push(item);
        }
    }
    available.extend(faint);
    available
}

fn scene_items(
    document: &Document,
    project: &Project,
    line: usize,
    range: Range<usize>,
    typed: &str,
) -> Vec<CompletionItem> {
    let digits = project.digits();
    let mut ids: Vec<String> = Vec::new();
    match document.format {
        DocumentFormat::Contents | DocumentFormat::PlotEpisode => {
            let Some((chapter, episode)) = project::episode_of_path(&document.path) else {
                return Vec::new();
            };
            let (previous, section) = previous_header(document, project, line);
            let next_scene = previous.map_or(1, |found| found.scene + 1);
            ids.push(display(
                SceneId {
                    chapter,
                    episode,
                    scene: next_scene,
                    section: 0,
                },
                3,
                digits,
            ));
            if let Some(found) = previous {
                ids.push(display(
                    SceneId {
                        section: section + 1,
                        ..found
                    },
                    4,
                    digits,
                ));
            }
        }
        DocumentFormat::PlotOutline => {
            let parsed = plot_outline::parse(&document.text);
            let previous = parsed
                .blocks
                .iter()
                .filter(|block| block.header < line)
                .filter_map(|block| block.id.map(|id| (id, block.depth)))
                .next_back();
            let is_all = document.path == project::ALL_PLOT_PATH;
            let next = match (previous, is_all) {
                (Some((id, _)), true) => SceneId {
                    chapter: id.chapter + 1,
                    ..SceneId::default()
                },
                (Some((id, _)), false) => SceneId {
                    episode: id.episode + 1,
                    ..id
                },
                (None, true) => SceneId {
                    chapter: 1,
                    ..SceneId::default()
                },
                (None, false) => SceneId {
                    chapter: project::chapter_of_path(&document.path).unwrap_or(1),
                    episode: 1,
                    ..SceneId::default()
                },
            };
            ids.push(display(next, if is_all { 1 } else { 2 }, digits));
        }
        _ => return Vec::new(),
    }
    ids.into_iter()
        .filter(|id| id.starts_with(typed))
        .map(|id| CompletionItem {
            label: id.clone(),
            kind: CompletionKind::SceneId,
            detail: String::new(),
            insert_text: id,
            range: range.clone(),
            faint: false,
        })
        .collect()
}

/// `line` より前の最後の場面見出しの ID と、`line` の時点の節番号。
fn previous_header(document: &Document, project: &Project, line: usize) -> (Option<SceneId>, u16) {
    let mut previous = None;
    let mut section = 0;
    match document.format {
        DocumentFormat::Contents => {
            let parsed = contents::parse_with(&document.text, &project.contents_options());
            for found in &parsed.sections {
                let Some(header) = found.scene_header else {
                    continue;
                };
                if header >= line {
                    break;
                }
                previous = found.scene;
                section = found.number;
            }
        }
        DocumentFormat::PlotEpisode => {
            let parsed = plot_episode::parse_with(&document.text, &project.contents_options());
            for scene in &parsed.scenes {
                if scene.header >= line {
                    break;
                }
                previous = scene.id;
                section = scene.sections.last().map_or(0, |found| found.number);
            }
        }
        _ => {}
    }
    (previous, section)
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
        .take(depth)
        .map(|(value, width)| format!("{value:0width$}", width = usize::from(*width)))
        .collect::<Vec<_>>()
        .join("-")
}

/// 「現在の節に登場 = 0 / 同じ話に登場 = 1 / それ以外 = 2」。
struct Ranks {
    section: Vec<Target>,
    episode: Vec<Target>,
}

impl Ranks {
    fn new(document: &Document, project: &Project, line: usize) -> Self {
        let scenes = project.scenes_by_line(document);
        let here = scenes.get(line).copied().flatten();
        let mut section = Vec::new();
        let mut episode = Vec::new();
        let mut push = |target: &Target, same_section: bool| {
            if same_section
                && !section
                    .iter()
                    .any(|seen| project::same_target(seen, target))
            {
                section.push(target.clone());
            }
            if !episode
                .iter()
                .any(|seen| project::same_target(seen, target))
            {
                episode.push(target.clone());
            }
        };
        for site in project.ref_sites(document) {
            if let Some(entry) = site.entry().and_then(|id| project.dictionary().entry(id)) {
                let same = here.is_some() && scenes.get(site.line).copied().flatten() == here;
                push(&entry.target, same);
            }
        }
        for site in project.speaker_sites(document) {
            if let Some(entry) = site.entry.and_then(|id| project.dictionary().entry(id)) {
                let same = here.is_some() && scenes.get(site.line).copied().flatten() == here;
                push(&entry.target, same);
            }
        }
        Self { section, episode }
    }

    fn of(&self, target: &Target) -> u8 {
        if self
            .section
            .iter()
            .any(|seen| project::same_target(seen, target))
        {
            0
        } else if self
            .episode
            .iter()
            .any(|seen| project::same_target(seen, target))
        {
            1
        } else {
            2
        }
    }
}

/// 読み（`reading:`）と資料のタイトル。候補ごとに木をパースし直さないよう 1 回だけ集める。
struct Readings {
    characters: Vec<String>,
    words: Vec<String>,
    titles: Vec<(String, String)>,
}

impl Readings {
    fn new(project: &Project) -> Self {
        let characters = project.characters();
        let words = project.words();
        let first = |fields: Vec<&crate::core::syntax::plot_outline::Field<'_>>| {
            fields
                .first()
                .map(|field| field.value.to_string())
                .unwrap_or_default()
        };
        Self {
            characters: (0..characters.characters.len())
                .map(|index| first(characters.character_fields(index, FieldKey::Reading)))
                .collect(),
            words: (0..words.words.len())
                .map(|index| first(words.word_fields(index, FieldKey::Reading)))
                .collect(),
            titles: project
                .materials()
                .index
                .map(|index| {
                    index
                        .items
                        .into_iter()
                        .map(|item| (item.id, item.title))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// 補足（資料はタイトル、キャラ・用語は読み）。
    fn detail(&self, target: &Target) -> String {
        match target {
            Target::Character { index, .. } => {
                self.characters.get(*index).cloned().unwrap_or_default()
            }
            Target::Word { index, .. } => self.words.get(*index).cloned().unwrap_or_default(),
            Target::Material { id } => self
                .titles
                .iter()
                .find(|(found, _)| found == id)
                .map(|(_, title)| title.clone())
                .unwrap_or_default(),
        }
    }

    /// 五十音の並びのキー（`reading:` があればそれ、無ければキー）。
    fn sort_key(&self, target: &Target, key: &str) -> String {
        let reading = match target {
            Target::Material { .. } => String::new(),
            _ => self.detail(target),
        };
        if reading.is_empty() {
            key.to_string()
        } else {
            reading
        }
    }
}

/// 挿入する文字列。最短一致で別の項目になるなら明示形（`{キー}`）。
fn insertion(project: &Project, key: &str, target: &Target, explicit: bool) -> String {
    if explicit {
        return format!("{key}{}", dictionary::EXPLICIT_CLOSE);
    }
    let probe = format!("{}{key}", dictionary::REF_SIGIL);
    let token = dictionary::resolve_ref(&probe, 0, project.dictionary());
    let resolves_here = token.key_range.end == probe.len()
        && token
            .resolved
            .and_then(|id| project.dictionary().entry(id))
            .is_some_and(|entry| project::same_target(&entry.target, target));
    if resolves_here {
        key.to_string()
    } else {
        format!(
            "{}{key}{}",
            dictionary::EXPLICIT_OPEN,
            dictionary::EXPLICIT_CLOSE
        )
    }
}
