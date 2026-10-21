//! 参照ペインの中身（§9.5.2。① の 6 セクション）を集める。
//!
//! LSP には対応しない補助（`PLANS.md` §9.2 B-113）。**文言は持たない**（見出しの言葉は UI の
//! `ui/i18n`、§12.2）。表示順と「内容が無ければ省く」は `resolve::context_rules::PaneSection`。
//!
//! | § 9.5.2 | ここで集めるもの |
//! |---|---|
//! | 1 プロット | 話プロットの当該節の内容行（`point_line`。フィールド行は除く）。無ければ場面の `summary` |
//! | 2 上位プロット | 章プロット → 全体プロットの `summary` の 1 行目（計 2 行まで） |
//! | 3 配役 | この節に出るキャラ（話者注釈・`'` 参照）の、この節で有効なモード（§6.4.6） |
//! | 4 用語 | この節に出る用語の `term` と `definition` の 1 行目 |
//! | 7 資料 | 明示参照（節の `'M…` / `'タイトル` と、話プロットの同じ節・場面レベルの `material:`）の後に、条件一致の候補（§16.4、M8）。引用注釈（`^M…`、§5.2.4）は ②（追加仕様 10） |
//! | 9 診断 | この節に掛かるもの。E → W の順（I は出さない） |
//!
//! 五感・空間・挿絵（5・6・8）は ②（`PLANS.md` §5.8 作業項目 7）。

use std::collections::BTreeSet;
use std::ops::Range;

use crate::core::diagnostics::{self, Diagnostic, Severity};
use crate::core::features::project::{self, episode_of_path};
use crate::core::features::Project;
use crate::core::id::SceneId;
use crate::core::model::document::Document;
use crate::core::resolve::context_rules;
use crate::core::resolve::dictionary::{EntryKind, Target};
use crate::core::resolve::modes;
use crate::core::syntax::{contents, plot_outline};
use crate::core::text::lexer::FieldKey;

/// 参照ペインの中身。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reference {
    /// 1. 今の節のプロット（行ごと、書かれたまま）。
    pub plot: Vec<String>,
    pub senses: Vec<String>,
    pub space: Vec<String>,
    pub space_ascii: bool,
    pub illustrations: Vec<String>,
    /// 2. 上位プロット（章 → 全体）。
    pub upper: Vec<Upper>,
    /// 3. 配役。
    pub cast: Vec<Cast>,
    /// 4. 用語。
    pub words: Vec<WordRef>,
    /// 7. 資料。
    pub materials: Vec<MaterialRef>,
    /// 9. 診断（E → W）。
    pub diagnostics: Vec<Diagnostic>,
}

impl Reference {
    /// 何も無いか。
    pub fn is_empty(&self) -> bool {
        self.plot.is_empty()
            && self.senses.is_empty()
            && self.space.is_empty()
            && self.illustrations.is_empty()
            && self.upper.is_empty()
            && self.cast.is_empty()
            && self.words.is_empty()
            && self.materials.is_empty()
            && self.diagnostics.is_empty()
    }
}

/// 上位プロットの 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upper {
    /// ブロックの構造 ID（`01-04` / `01`。`@` なし）。
    pub id: String,
    /// `summary` の 1 行目。
    pub summary: String,
}

/// 配役の 1 人。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cast {
    /// H1 の表示名。
    pub name: String,
    pub icon: Option<Vec<u8>>,
    /// 有効なモードの見出し（`B 冷静な観察者`）。併用なら複数。決まらなければ空。
    pub modes: Vec<String>,
    /// `personality:`（モード → キャラの順に最初に見つかったもの）。
    pub personality: Option<String>,
    /// `emotion:`。
    pub emotion: Option<String>,
    /// `role:`（モード → キャラ）。
    pub role: Option<String>,
    /// 役割語のパターン（鉤括弧を除いて書かれたまま。モード専用 ∪ キャラ共通）。
    pub tone: Vec<String>,
}

/// 用語の 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordRef {
    /// H1 の用語。
    pub term: String,
    /// `definition:` の 1 行目（無ければ空）。
    pub definition: String,
}

/// 資料の 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialRef {
    /// ID（`M0137`）。
    pub id: String,
    /// タイトル（索引に無ければ空）。
    pub title: String,
    /// `summary`（無ければ空）。
    pub summary: String,
    /// 出典（URL の場合は参照ペインから外部ブラウザへ開ける）。
    pub source: String,
    /// 条件一致の候補か（§16.4。明示参照の後に薄い色で並べる。`false` = 明示参照）。
    pub candidate: bool,
}

/// `document`（本文）の節 `scene`（4 階層）の参照ペイン。`lines` は節の行（`sections::SectionRef::lines`）。
pub fn at(
    document: &Document,
    lines: Range<usize>,
    scene: SceneId,
    project: &Project,
) -> Reference {
    let parsed = contents::parse_with(&document.text, &project.contents_options());
    let starts = project::line_starts_of(&document.text);
    let byte_range = byte_range_of(&starts, &document.text, &lines);
    let rows = crate::core::stage_two::metadata::records(project);
    let get = |key| crate::core::stage_two::metadata::inherited(&rows, scene, key);
    let space = get("space")
        .map(|s| {
            crate::core::stage_two::metadata::asset_path(&s, false)
                .and_then(|p| project.document(&p))
                .map_or(s, |d| d.text.clone())
        })
        .into_iter()
        .collect();
    let mut illustrations: Vec<String> = get("illustration").into_iter().collect();
    for line in document.text.lines().skip(lines.start).take(lines.len()) {
        if let Some(memo) = crate::core::annotations::illustration(line) {
            illustrations.push(memo.to_owned());
        }
    }
    Reference {
        senses: get("senses").into_iter().collect(),
        space_ascii: get("space-mode").as_deref() == Some("ascii"),
        space,
        illustrations,
        plot: plot_lines(project, scene),
        upper: upper_plots(project, scene),
        cast: cast(document, &parsed, &lines, scene, project),
        words: words(document, &lines, project),
        materials: materials(document, &lines, scene, project),
        diagnostics: section_diagnostics(document, &byte_range, project),
    }
}

/// 行の範囲 → byte の範囲（空の節なら、その位置の空範囲）。
fn byte_range_of(starts: &[usize], text: &str, lines: &Range<usize>) -> Range<usize> {
    let start = starts.get(lines.start).copied().unwrap_or(text.len());
    let end = if lines.end > lines.start {
        starts
            .get(lines.end)
            .map(|next| next.saturating_sub(1))
            .unwrap_or(text.len())
    } else {
        start
    };
    start..end.max(start)
}

// ============================================================ 1. プロット

fn plot_lines(project: &Project, scene: SceneId) -> Vec<String> {
    let Some(plot) = project.episode_plot(scene.chapter, scene.episode) else {
        return Vec::new();
    };
    let whole = SceneId {
        section: 0,
        ..scene
    };
    let Some((index, found)) = plot
        .scenes
        .iter()
        .enumerate()
        .find(|(_, found)| found.id.map(|id| SceneId { section: 0, ..id }) == Some(whole))
    else {
        return Vec::new();
    };
    if let Some(section) = found
        .sections
        .iter()
        .find(|section| section.number == scene.section)
    {
        let out: Vec<String> = section
            .points
            .iter()
            .filter_map(|line| plot.lines.get(*line))
            .map(|line| line.raw().trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect();
        if !out.is_empty() {
            return out;
        }
    }
    // 節の内容が無ければ場面のフィールド（summary など）
    plot.scene_fields(index, FieldKey::Summary)
        .iter()
        .map(|field| field.full_value())
        .filter(|value| !value.is_empty())
        .collect()
}

/// 話プロットの当該節の `length:`（目標字数、§5.4.2。文脈ビューのプレースホルダ `412 / 500`、§9.3.2）。
pub fn target_length(project: &Project, scene: SceneId) -> Option<u32> {
    let plot = project.episode_plot(scene.chapter, scene.episode)?;
    let whole = SceneId {
        section: 0,
        ..scene
    };
    let (index, found) = plot
        .scenes
        .iter()
        .enumerate()
        .find(|(_, found)| found.id.map(|id| SceneId { section: 0, ..id }) == Some(whole))?;
    let at = found
        .sections
        .iter()
        .position(|section| section.number == scene.section)?;
    let fields = plot.section_fields(index, at, FieldKey::Length);
    fields.first()?.value.trim().parse().ok()
}

// ============================================================ 2. 上位プロット

fn upper_plots(project: &Project, scene: SceneId) -> Vec<Upper> {
    let digits = project.digits();
    let mut out = Vec::new();
    if let Some(chapter) = project.chapter_plot(scene.chapter) {
        let want = SceneId {
            scene: 0,
            section: 0,
            ..scene
        };
        if let Some(summary) = block_summary(&chapter, want) {
            out.push(Upper {
                id: partial(&format!("{}-{}", want.chapter, want.episode), digits),
                summary,
            });
        }
    }
    if let Some(all) = project.all_plot() {
        let want = SceneId {
            episode: 0,
            scene: 0,
            section: 0,
            ..scene
        };
        if let Some(summary) = block_summary(&all, want) {
            out.push(Upper {
                id: partial(&want.chapter.to_string(), digits),
                summary,
            });
        }
    }
    out
}

/// `01-04` / `01` を桁数に合わせて表示する。
fn partial(text: &str, digits: crate::core::id::Digits) -> String {
    crate::core::id::PartialSceneId::parse(text)
        .map(|id| id.display(digits))
        .unwrap_or_else(|_| text.to_owned())
}

fn block_summary(outline: &plot_outline::Outline<'_>, want: SceneId) -> Option<String> {
    let index = outline
        .blocks
        .iter()
        .position(|block| block.id == Some(want))?;
    let fields = outline.fields_of(index, FieldKey::Summary);
    let first = fields.first()?;
    let line = first.value.trim();
    (!line.is_empty()).then(|| line.to_owned())
}

// ============================================================ 3. 配役

fn cast(
    document: &Document,
    parsed: &contents::Contents<'_>,
    lines: &Range<usize>,
    scene: SceneId,
    project: &Project,
) -> Vec<Cast> {
    let mut order: Vec<usize> = Vec::new();
    characters_in(
        document,
        Some(parsed),
        std::slice::from_ref(lines),
        project,
        &mut order,
    );
    let starts = project::line_starts_of(&document.text);
    let line_start = starts.get(lines.start).copied().unwrap_or(0);
    cast_of(project, &document.path, scene, line_start, order)
}

/// `document` の行 `lines` に出るキャラ（話者注釈 → `'` 参照の出現順、重複なし）を `order` に足す。
/// `parsed` は本文（C 形式）のときだけ渡す（話者注釈は本文にしか無い）。
pub(crate) fn characters_in(
    document: &Document,
    parsed: Option<&contents::Contents<'_>>,
    lines: &[Range<usize>],
    project: &Project,
    order: &mut Vec<usize>,
) {
    let dictionary = project.dictionary();
    let mut push = |index: usize| {
        if !order.contains(&index) {
            order.push(index);
        }
    };
    let sites = project.ref_sites(document);
    for range in lines {
        for line in range.clone() {
            if let Some(contents::Line::Text(text)) = parsed.and_then(|found| found.lines.get(line))
            {
                if let Some(found) = text.speaker.as_ref() {
                    if let Some(index) = character_index(project, found.name) {
                        push(index);
                    }
                }
            }
            for site in &sites {
                if site.line != line {
                    continue;
                }
                if let Some(entry) = site.entry().and_then(|id| dictionary.entry(id)) {
                    if let Target::Character { index, .. } = &entry.target {
                        push(*index);
                    }
                }
            }
        }
    }
}

/// キャラ `order` の配役（§6.4 の解決で有効なモードだけ、§15.4.4）。`path` は本文のパス
/// （話の決定に使う）、`line_start` はモードを解決する位置（節の先頭）。
pub(crate) fn cast_of(
    project: &Project,
    path: &str,
    scene: SceneId,
    line_start: usize,
    order: Vec<usize>,
) -> Vec<Cast> {
    if order.is_empty() {
        return Vec::new();
    }
    let dictionary = project.dictionary();
    let characters = project.characters();
    let episode = episode_of_path(path);
    let plot = episode.and_then(|(chapter, number)| project.episode_plot(chapter, number));
    let chapter_plot = episode.and_then(|(chapter, _)| project.chapter_plot(chapter));
    let all_plot = project.all_plot();
    let input = modes::Input {
        characters: &characters,
        episode: plot.as_ref(),
        chapter: chapter_plot.as_ref(),
        all: all_plot.as_ref(),
        dictionary,
    };
    order
        .into_iter()
        .filter_map(|index| {
            let name = characters.characters.get(index)?.name.to_string();
            let target = modes::Target {
                scene,
                character: index,
                speaker: None,
                line_start,
            };
            let assignment = modes::resolve(&input, &target);
            let first = |key: FieldKey| -> Option<String> {
                characters
                    .character_fields(index, key)
                    .first()
                    .map(|field| field.full_value())
                    .filter(|value| !value.is_empty())
            };
            let mut entry = Cast {
                icon: first(FieldKey::Icon)
                    .and_then(|p| crate::core::stage_two::metadata::asset_path(&p, true))
                    .and_then(|p| project.assets.get(&p).cloned()),
                name,
                modes: assignment
                    .definitions
                    .iter()
                    .map(|definition| {
                        if definition.label().is_empty() {
                            definition.id().to_owned()
                        } else {
                            format!("{} {}", definition.id(), definition.label())
                        }
                    })
                    .collect(),
                personality: None,
                emotion: None,
                role: None,
                tone: Vec::new(),
            };
            for definition in &assignment.definitions {
                let model = modes::to_model(definition, &characters);
                entry.personality = entry.personality.or(model.personality);
                entry.emotion = entry.emotion.or(model.emotion);
                entry.role = entry.role.or(model.role);
            }
            entry.personality = entry.personality.or_else(|| first(FieldKey::Personality));
            entry.role = entry.role.or_else(|| first(FieldKey::Role));
            entry.tone = modes::applicable_patterns(&characters, &assignment, index)
                .iter()
                .map(|pattern| pattern.source.clone())
                .collect();
            Some(entry)
        })
        .collect()
}

fn character_index(project: &Project, name: &str) -> Option<usize> {
    let dictionary = project.dictionary();
    let id = dictionary.lookup(name, Some(&[EntryKind::Character]))?;
    match &dictionary.entry(id)?.target {
        Target::Character { index, .. } => Some(*index),
        _ => None,
    }
}

// ============================================================ 4. 用語

fn words(document: &Document, lines: &Range<usize>, project: &Project) -> Vec<WordRef> {
    let dictionary = project.dictionary();
    let list = project.words();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for site in project.ref_sites(document) {
        if !lines.contains(&site.line) {
            continue;
        }
        let Some(entry) = site.entry().and_then(|id| dictionary.entry(id)) else {
            continue;
        };
        let Target::Word { index, term } = &entry.target else {
            continue;
        };
        if !seen.insert(*index) {
            continue;
        }
        let definition = list
            .word_fields(*index, FieldKey::Definition)
            .first()
            .map(|field| field.value.trim().to_owned())
            .unwrap_or_default();
        out.push(WordRef {
            term: term.clone(),
            definition,
        });
    }
    out
}

// ============================================================ 7. 資料

fn materials(
    document: &Document,
    lines: &Range<usize>,
    scene: SceneId,
    project: &Project,
) -> Vec<MaterialRef> {
    let mut ids: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    // 明示参照（`'M0137` / `'タイトル`）。節に出てくる参照の名前は条件一致（`ref`）にも使う
    collect_refs(
        document,
        std::slice::from_ref(lines),
        project,
        &mut ids,
        &mut names,
    );
    // 話プロットのこの節（と場面レベル）の `material:` も明示参照（§16.4 の追記先。追記した候補は
    // 候補でなくなる、`PLANS.md` §9.2 B-134）
    let plot = plot_scope(project, scene);
    if let Some((path, ranges, _)) = &plot {
        if let Some(found) = project.document(path) {
            collect_refs(found, ranges, project, &mut ids, &mut names);
        }
    }
    for line in contents::parse_with(&document.text, &project.contents_options())
        .lines
        .iter()
        .skip(lines.start)
        .take(lines.len())
    {
        if let contents::Line::Text(text) = line {
            for (_, id) in crate::core::annotations::citations(text.body) {
                if !ids.iter().any(|s| s == id) {
                    ids.push(id.to_owned());
                }
            }
        }
    }
    let explicit = ids.len();
    // §16.4: 条件一致の候補（明示参照していないもの）
    let tags = plot.map(|(_, _, tags)| tags).unwrap_or_default();
    for id in crate::core::features::materials_ops::matching(project, scene, &names, &tags) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let index = project.materials();
    ids.into_iter()
        .enumerate()
        .map(|(at, id)| {
            let found = index
                .index
                .as_ref()
                .and_then(|parsed| parsed.items.iter().find(|item| item.id == id));
            MaterialRef {
                title: found.map(|item| item.title.clone()).unwrap_or_default(),
                summary: found
                    .and_then(|item| item.summary.clone())
                    .unwrap_or_default(),
                source: found
                    .map(|item| item.source.shown().to_owned())
                    .unwrap_or_default(),
                id,
                candidate: at >= explicit,
            }
        })
        .collect()
}

/// `document` の行 `lines`（どれかに入る行）の `'` 参照: 資料なら `ids` に、名前は全部 `names` に
/// （重複なし、出た順）。
pub(crate) fn collect_refs(
    document: &Document,
    lines: &[Range<usize>],
    project: &Project,
    ids: &mut Vec<String>,
    names: &mut Vec<String>,
) {
    let dictionary = project.dictionary();
    for site in project.ref_sites(document) {
        if !lines.iter().any(|range| range.contains(&site.line)) {
            continue;
        }
        if let Some(name) = document.text.get(site.key_range()) {
            if !names.iter().any(|seen| seen == name) {
                names.push(name.to_owned());
            }
        }
        if let Some(entry) = site.entry().and_then(|id| dictionary.entry(id)) {
            if let Target::Material { id } = &entry.target {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
        }
    }
}

/// 話プロットの中の節 `scene` の範囲: (パス, 行の範囲（場面レベルのフィールドの行・節の行）, タグ)。
///
/// タグは `tags:` / `タグ:`（§5.0.6 の表では Markdown 形式のキーだが、表にあるキーは B 形式でも
/// そのまま読む。W011 は表に無いキーだけ）で、§16.4 の `tag` の一致判定に使う。場面レベルのタグは
/// 場面の全部の節に効く。
/// 話プロットか場面が無ければ `None`。
pub(crate) fn plot_scope(
    project: &Project,
    scene: SceneId,
) -> Option<(String, Vec<Range<usize>>, Vec<String>)> {
    let plot = project.episode_plot(scene.chapter, scene.episode)?;
    let whole = SceneId {
        section: 0,
        ..scene
    };
    let (index, found) = plot
        .scenes
        .iter()
        .enumerate()
        .find(|(_, found)| found.id.map(|id| SceneId { section: 0, ..id }) == Some(whole))?;
    // 場面レベル: 見出しの次の行から最初の節の手前まで
    let head_end = found
        .sections
        .first()
        .map_or(found.span.end, |section| section.span.start)
        .max(found.header + 1);
    let mut ranges = Vec::with_capacity(2);
    ranges.push(found.header + 1..head_end);
    let mut fields = plot.scene_fields(index, FieldKey::Tags);
    fields.extend(plot.scene_fields(index, FieldKey::Unknown));
    if scene.section > 0 {
        if let Some(at) = found
            .sections
            .iter()
            .position(|section| section.number == scene.section)
        {
            ranges.push(found.sections[at].span.clone());
            fields.extend(plot.section_fields(index, at, FieldKey::Tags));
            fields.extend(plot.section_fields(index, at, FieldKey::Unknown));
        }
    }
    // §5.0.6 の表の `tags` の行（正準形と日本語別名）。字句器が `Tags` と読まなかった書き方も念のため拾う
    let names = [FieldKey::Tags.ascii_key(), FieldKey::Tags.alias()];
    let tags = fields
        .iter()
        .filter(|field| field.key == FieldKey::Tags || names.contains(&Some(field.key_text)))
        .flat_map(|field| field.value.split([',', '、']))
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .collect();
    let path = project.episode_plot_path(scene.chapter, scene.episode);
    Some((path, ranges, tags))
}

// ============================================================ 9. 診断

fn section_diagnostics(
    document: &Document,
    range: &Range<usize>,
    project: &Project,
) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = diagnostics::document(document, project)
        .into_iter()
        .filter(|found| matches!(found.severity(), Severity::Error | Severity::Warning))
        .filter(|found| {
            found.range.as_ref().is_some_and(|at| {
                // 節の中に掛かるもの（空の節は、その位置に掛かるもの）
                at.start <= range.end && at.end >= range.start
            })
        })
        .collect();
    context_rules::sort_diagnostics(&mut out);
    out
}
