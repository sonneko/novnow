//! 配役のワークスペース（§10.6 `cast`）の補助: キャラの並び・モードタイムライン・役割語テスター・
//! 役割語が一致しなかった台詞。
//!
//! LSP には対応しない補助（`PLANS.md` §9.2 B-133）。**文言は持たない**（§12.2）。

use std::collections::BTreeMap;
use std::ops::Range;

use crate::core::features::project::{self, episode_of_path};
use crate::core::features::Project;
use crate::core::id::{RangeExpr, SceneId};
use crate::core::model::document::DocumentFormat;
use crate::core::model::role_pattern::RolePattern;
use crate::core::resolve::dictionary::{EntryKind, Target as DictTarget};
use crate::core::resolve::{modes, role_language};
use crate::core::syntax::characters_md::{self, SectionKey};
use crate::core::syntax::contents;
use crate::core::text::lexer::FieldKey;

/// `characters.md` のパス（§4.1）。
pub const PATH: &str = "characters.md";

/// キャラ一覧の 1 行（§10.6 の左）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastRow {
    /// `characters.md` のキャラの順番（H1 の順）。
    pub index: usize,
    /// 名前。
    pub name: String,
    /// `reading:`（無ければ空）。
    pub reading: String,
    /// エディタで編集する行（H1 から次の H1 の前まで。末尾の空行を含まない）。
    pub lines: Range<usize>,
}

/// キャラ一覧（`reading` の順。`reading` が無ければ名前で並べる。§10.6「reading 順」）。
pub fn characters(project: &Project) -> Vec<CastRow> {
    let text = project.text(PATH);
    let parsed = characters_md::parse(text);
    let mut out: Vec<CastRow> = parsed
        .characters
        .iter()
        .enumerate()
        .map(|(index, character)| {
            let reading = parsed
                .character_fields(index, FieldKey::Reading)
                .first()
                .map(|field| field.value.trim().to_owned())
                .unwrap_or_default();
            let mut end = character.span.end.min(parsed.lines.len());
            while end > character.heading + 1 && parsed.lines[end - 1].is_blank() {
                end -= 1;
            }
            CastRow {
                index,
                name: character.name.trim().to_owned(),
                reading,
                lines: character.heading..end,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        let key = |row: &CastRow| {
            if row.reading.is_empty() {
                row.name.clone()
            } else {
                row.reading.clone()
            }
        };
        key(a).cmp(&key(b)).then(a.index.cmp(&b.index))
    });
    out
}

// ============================================================ モードタイムライン

/// モードの帯 1 本（§10.6「縦軸にモード、`range` を帯で」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Band {
    /// `mode_id`。
    pub id: String,
    /// 表示名。
    pub label: String,
    /// 書かれたままの `range:`（無ければ空 = 全範囲）。
    pub range: String,
    /// 章ごとに帯が掛かるか（[`Timeline::chapters`] と同じ順）。
    pub covered: Vec<bool>,
    /// `range:` の行（無ければ `None`）。
    pub line: Option<usize>,
}

/// キャラ 1 人のモードタイムライン。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Timeline {
    /// 横軸の章（`@01 … @NN`）。
    pub chapters: Vec<u16>,
    /// 帯（`### ` の順）。
    pub bands: Vec<Band>,
}

/// キャラ `index`（H1 の順）のモードタイムライン。横軸はプロジェクトにある章（無ければ `@01`）。
pub fn timeline(project: &Project, index: usize) -> Timeline {
    let mut chapters = crate::core::features::plots::chapters(project);
    if chapters.is_empty() {
        chapters.push(1);
    }
    let text = project.text(PATH);
    let parsed = characters_md::parse(text);
    let Some(character) = parsed.characters.get(index) else {
        return Timeline {
            chapters,
            bands: Vec::new(),
        };
    };
    let bands = character
        .section(SectionKey::Modes)
        .map(|section| {
            section
                .modes
                .iter()
                .map(|mode| {
                    let expr = mode
                        .range
                        .as_ref()
                        .filter(|found| found.valid)
                        .and_then(|found| found.expr());
                    let covered = chapters
                        .iter()
                        .map(|chapter| match &expr {
                            Some(expr) => RangeExpr::parse(&format!("@{chapter}"))
                                .map(|column| expr.intersects(&column))
                                .unwrap_or(false),
                            None => true,
                        })
                        .collect();
                    Band {
                        id: mode.id.to_owned(),
                        label: mode.label.trim().to_owned(),
                        range: mode
                            .range
                            .as_ref()
                            .map(|found| found.text.trim().to_owned())
                            .unwrap_or_default(),
                        covered,
                        line: mode.range.as_ref().map(|found| found.line),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Timeline { chapters, bands }
}

/// 範囲式の章を `delta` だけずらした文字列（`@01 .. @02` → `@02 .. @03`。下の階層と桁数は保つ）。
/// どれかの章が 1 より小さくなるなら `None`。
pub fn shift_range(range: &str, delta: i32) -> Option<String> {
    let mut out = String::with_capacity(range.len());
    let mut rest = range;
    while let Some(at) = rest.find('@') {
        out.push_str(&rest[..=at]);
        rest = &rest[at + 1..];
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            continue;
        }
        let number: i32 = rest[..digits].parse().ok()?;
        let shifted = number + delta;
        if shifted < 1 {
            return None;
        }
        out.push_str(&format!("{shifted:0digits$}"));
        rest = &rest[digits..];
    }
    out.push_str(rest);
    RangeExpr::parse(&out).ok()?;
    Some(out)
}

/// キャラ `index` のモード `mode`（`### ` の順）の帯を `delta` 章ずらした `characters.md` 全体
/// （§10.6「`Shift+←/→` で `range` が書き換わる」）。`range:` が無い（全範囲）・ずらせないなら `None`。
pub fn shift_band(text: &str, index: usize, mode: usize, delta: i32) -> Option<String> {
    let parsed = characters_md::parse(text);
    let character = parsed.characters.get(index)?;
    let found = character.section(SectionKey::Modes)?.modes.get(mode)?;
    let range = found.range.as_ref()?;
    let shifted = shift_range(range.text, delta)?;
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let line = lines.get_mut(range.line)?;
    let start = range.at.min(line.len());
    let end = (start + range.text.len()).min(line.len());
    line.replace_range(start..end, &shifted);
    Some(lines.join("\n"))
}

// ============================================================ 役割語テスター

/// テスターの 1 行（パターン 1 本）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternCheck {
    /// どこに書かれたパターンか（`None` = `## tone` のキャラ共通、`Some(mode_id)` = モード専用）。
    pub mode: Option<String>,
    /// パターン（`*のね` など）。
    pub source: String,
    /// §6.5.1 の正規表現への変換結果。
    pub regex: String,
    /// 一致したか。
    pub matched: bool,
}

/// テスターの結果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TesterResult {
    /// パターンごとの結果（キャラ共通 → モード専用の順）。
    pub checks: Vec<PatternCheck>,
    /// 下線を引く範囲（台詞の中の byte。どれかのパターンが一致した部分の和）。
    pub underlines: Vec<Range<usize>>,
}

/// キャラ `index` の全パターンで台詞 `utterance` を試す（§10.6 の役割語テスター）。
pub fn test_utterance(project: &Project, index: usize, utterance: &str) -> TesterResult {
    let text = project.text(PATH);
    let parsed = characters_md::parse(text);
    let Some(character) = parsed.characters.get(index) else {
        return TesterResult::default();
    };
    let lexicon = project.settings().lexicon.clone();
    let mut groups: Vec<(Option<String>, Vec<RolePattern>)> = Vec::new();
    if let Some(section) = character.section(SectionKey::Tone) {
        groups.push((None, modes::patterns_of(&parsed, &section.patterns)));
    }
    if let Some(section) = character.section(SectionKey::Modes) {
        for mode in &section.modes {
            groups.push((
                Some(mode.id.to_owned()),
                modes::patterns_of(&parsed, &mode.patterns),
            ));
        }
    }
    let mut checks = Vec::new();
    let mut all: Vec<RolePattern> = Vec::new();
    for (mode, patterns) in groups {
        for pattern in patterns {
            let matcher = role_language::Matcher::new(&pattern, &lexicon);
            let (regex, matched) = match &matcher {
                Some(found) => (
                    found.regex_source().to_owned(),
                    !utterance.is_empty() && found.find(utterance).is_some(),
                ),
                None => (role_language::to_regex(&pattern, &lexicon), false),
            };
            checks.push(PatternCheck {
                mode: mode.clone(),
                source: pattern.source.clone(),
                regex,
                matched,
            });
            all.push(pattern);
        }
    }
    let underlines = if utterance.is_empty() {
        Vec::new()
    } else {
        role_language::MatcherSet::new(&all, &lexicon).underlines(utterance)
    };
    TesterResult { checks, underlines }
}

// ============================================================ 役割語が一致しなかった台詞

/// 一致しなかった台詞 1 つ（§10.6 `Ctrl+Shift+V`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmatched {
    /// 本文のパス。
    pub path: String,
    /// 台詞の先頭の byte（ファイルの中）。
    pub byte: usize,
    /// 節。
    pub scene: SceneId,
    /// 話者の名前。
    pub speaker: String,
    /// 台詞（鉤括弧の中身）。
    pub text: String,
}

/// プロジェクト全体の本文で、話者が決まり適用パターンがあるのに**どれも一致しなかった**台詞
/// （§6.5.4 の例外として一覧だけする。診断は出さない）。話者の決め方は本文の下線と同じ（§6.5.3:
/// 行末の話者注釈 → 直前の注釈 → `pov:`）。
pub fn unmatched_dialogue(project: &Project) -> Vec<Unmatched> {
    let characters = project.characters();
    let dictionary = project.dictionary();
    let mut cache = role_language::Cache::new(project.settings().lexicon.clone());
    let mut documents: Vec<_> = project
        .documents()
        .filter(|document| document.format == DocumentFormat::Contents)
        .collect();
    documents.sort_by(|a, b| a.path.cmp(&b.path));
    let character_index = |name: &str| -> Option<usize> {
        let id = dictionary.lookup(name, Some(&[EntryKind::Character]))?;
        match &dictionary.entry(id)?.target {
            DictTarget::Character { index, .. } => Some(*index),
            _ => None,
        }
    };
    let mut out = Vec::new();
    for document in documents {
        let parsed = contents::parse_with(&document.text, &project.contents_options());
        let episode = episode_of_path(&document.path);
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
        let starts = project::line_starts_of(&document.text);
        let mut povs: BTreeMap<SceneId, Option<usize>> = BTreeMap::new();
        for section in &parsed.sections {
            let Some(scene) = section.scene else {
                continue;
            };
            let scene = SceneId {
                section: section.number,
                ..scene
            };
            let pov = *povs
                .entry(SceneId {
                    section: 0,
                    ..scene
                })
                .or_insert_with(|| {
                    plot.as_ref().and_then(|tree| {
                        let index = tree.scenes.iter().position(|found| {
                            found.id.map(|id| SceneId { section: 0, ..id })
                                == Some(SceneId {
                                    section: 0,
                                    ..scene
                                })
                        })?;
                        let field = tree.scene_fields(index, FieldKey::Pov).into_iter().next()?;
                        let name = field.value.trim().trim_start_matches('\'');
                        character_index(name)
                    })
                });
            let mut current: Option<usize> = None;
            for index in section.span.clone() {
                let Some(contents::Line::Text(text)) = parsed.lines.get(index) else {
                    continue;
                };
                let own = text
                    .speaker
                    .as_ref()
                    .and_then(|found| character_index(found.name));
                if own.is_some() {
                    current = own;
                }
                let spans = role_language::dialogue_spans(text.body);
                if spans.is_empty() {
                    continue;
                }
                let Some(character) = own.or(current).or(pov) else {
                    continue;
                };
                let line_start = starts.get(index).copied().unwrap_or(0);
                let target = modes::Target {
                    scene,
                    character,
                    speaker: own.and(text.speaker.as_ref()),
                    line_start,
                };
                let assignment = modes::resolve(&input, &target);
                let patterns = modes::applicable_patterns(&characters, &assignment, character);
                if patterns.is_empty() {
                    continue;
                }
                let variants: Vec<String> = assignment
                    .definitions
                    .iter()
                    .map(|definition| definition.node.heading.to_string())
                    .collect();
                let key = format!("{character}:{}", variants.join("+"));
                let set = cache.set_for(&key, &patterns);
                let body_at = text.raw.find(text.body).unwrap_or(0);
                for span in spans {
                    let utterance = &text.body[span.clone()];
                    if set.hits(utterance).is_empty() {
                        out.push(Unmatched {
                            path: document.path.clone(),
                            byte: line_start + body_at + span.start,
                            scene,
                            speaker: characters
                                .characters
                                .get(character)
                                .map(|found| found.name.to_string())
                                .unwrap_or_default(),
                            text: utterance.to_owned(),
                        });
                    }
                }
            }
        }
    }
    out
}
