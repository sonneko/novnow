//! 読者モードの純粋な組版（§17.3.3）。字送りと版面の寸法は呼び出し側が渡す（core はフォントも GUI も知らない）。
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub rows: usize,
    pub columns: usize,
    pub vertical: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            rows: 42,
            columns: 18,
            vertical: true,
        }
    }
}
/// フォントの寸法は呼び出し側が渡す。core はフォントにも GUI にも触れない（§3.2）。
#[derive(Debug, Clone)]
pub struct Metrics {
    pub advance: f32,
    pub line_advance: f32,
    /// 使用フォントに縦書き字形として収録されているコードポイント。括弧については
    /// 置換先の互換形（[`VERTICAL_FORMS`] の第 2 要素）、長音・ダッシュについては字そのものを入れる。
    pub vertical_forms: std::collections::BTreeSet<char>,
    pub glyph_advances: std::collections::BTreeMap<char, f32>,
}
impl Default for Metrics {
    fn default() -> Self {
        Self {
            advance: 1.0,
            line_advance: 1.0,
            vertical_forms: Default::default(),
            glyph_advances: Default::default(),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub ch: char,
    pub text: String,
    pub ruby: String,
    pub row: usize,
    pub column: usize,
    pub rotate: bool,
    pub compact: bool,
    pub punctuation: bool,
    pub x: f32,
    pub y: f32,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    pub glyphs: Vec<Glyph>,
}

/// 組版結果と、各ページ・各節の対応関係（§17.3）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutWithScenes {
    pub pages: Vec<Page>,
    /// 各ページの先頭に対応する `SceneId`。
    pub page_scenes: Vec<Option<crate::core::id::SceneId>>,
    /// 各節が始まる 0 始まりのページ番号。
    pub section_pages: Vec<(crate::core::id::SceneId, usize)>,
}

#[derive(Clone)]
struct Unit {
    ch: char,
    text: String,
    ruby: String,
    rotate: bool,
    compact: bool,
    byte_offset: usize,
}
/// i18n: format-vocabulary
const NO_START: &str =
    "、。，．）］｝〉》」』】！？ゝゞ々ーぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮ";
/// i18n: format-vocabulary
const NO_END: &str = "（［｛〈《「『【";
/// i18n: format-vocabulary
const PUNCTUATION: &str = "、。";
/// i18n: format-vocabulary
const VERTICAL_ROTATE: &str = "ー―…";
fn units(text: &str, vertical: bool) -> Vec<Unit> {
    let rubies = crate::core::syntax::contents::rubies_of(text);
    let mut prepared: Vec<(char, String, usize)> = Vec::new();
    let mut at = 0;
    for ruby in rubies {
        prepared.extend(
            text[at..ruby.range.start]
                .char_indices()
                .map(|(rel, ch)| (ch, String::new(), at + rel)),
        );
        let reading: Vec<char> = ruby.reading.chars().collect();
        let base: Vec<char> = ruby.base.chars().collect();
        for (index, ch) in base.iter().enumerate() {
            let lo = index * reading.len() / base.len().max(1);
            let hi = (index + 1) * reading.len() / base.len().max(1);
            prepared.push((*ch, reading[lo..hi].iter().collect(), ruby.range.start));
        }
        at = ruby.range.end;
    }
    prepared.extend(
        text[at..]
            .char_indices()
            .map(|(rel, ch)| (ch, String::new(), at + rel)),
    );
    let mut result = Vec::new();
    let mut index = 0;
    while index < prepared.len() {
        let (ch, _, byte_offset) = &prepared[index];
        let mut count = 1;
        if ch.is_ascii_alphabetic() {
            while index + count < prepared.len() && prepared[index + count].0.is_ascii_alphabetic()
            {
                count += 1;
            }
        }
        let latin = count >= 3;
        if ch.is_ascii_digit() {
            count = 1;
            while index + count < prepared.len() && prepared[index + count].0.is_ascii_digit() {
                count += 1;
            }
        }
        if vertical && ch.is_ascii_digit() && count == 2 {
            result.push(Unit {
                ch: *ch,
                text: prepared[index..index + 2].iter().map(|p| p.0).collect(),
                ruby: String::new(),
                rotate: false,
                compact: true,
                byte_offset: *byte_offset,
            });
            index += 2;
            continue;
        }
        let length = if latin { count } else { 1 };
        for (ch, ruby, pos) in &prepared[index..index + length] {
            result.push(Unit {
                ch: *ch,
                text: ch.to_string(),
                ruby: ruby.clone(),
                rotate: vertical && (latin || VERTICAL_ROTATE.contains(*ch)),
                compact: false,
                byte_offset: *pos,
            });
        }
        index += length;
    }
    result
}
/// 括弧とその縦書き互換形（Unicode の縦書き提示形）。UI は使用フォントに
/// 互換形が収録されているかを確かめて [`Metrics::vertical_forms`] へ渡す。
/// i18n: format-vocabulary
pub const VERTICAL_FORMS: &[(char, char)] = &[
    ('（', '︵'),
    ('）', '︶'),
    ('［', '﹇'),
    ('］', '﹈'),
    ('｛', '︷'),
    ('｝', '︸'),
    ('「', '﹁'),
    ('」', '﹂'),
    ('『', '﹃'),
    ('』', '﹄'),
    ('【', '︻'),
    ('】', '︼'),
    ('〈', '︿'),
    ('〉', '﹀'),
    ('《', '︽'),
    ('》', '︾'),
];

fn vertical_char(ch: char) -> char {
    VERTICAL_FORMS
        .iter()
        .find(|(from, _)| *from == ch)
        .map_or(ch, |(_, to)| *to)
}
/// 字送りを 1 文字 = 1 マスとして組む簡易版。実際のフォントの字送りで組むときは [`layout`]。
pub fn layout_cells(text: &str, options: Options) -> Vec<Page> {
    layout(text, options, &Metrics::default())
}
/// 字送りと縦書き字形の有無は呼び出し側が [`Metrics`] で渡す（§17.3.3）。
pub fn layout(text: &str, options: Options, metrics: &Metrics) -> Vec<Page> {
    layout_internal(text, &[], &[], options, metrics).pages
}

/// 節ごとの変換断片 [`crate::core::export::FragmentWithScenes`] を組版し、
/// 各ページの先頭 `SceneId` と各節の開始ページ番号を同じ組版結果から返す（§17.3）。
pub fn layout_with_scenes(
    fragment: &crate::core::export::FragmentWithScenes,
    options: Options,
    metrics: &Metrics,
) -> LayoutWithScenes {
    layout_internal(
        &fragment.text,
        &fragment.byte_scenes,
        &fragment.section_bytes,
        options,
        metrics,
    )
}

fn layout_internal(
    text: &str,
    byte_scenes: &[(std::ops::Range<usize>, crate::core::id::SceneId)],
    section_bytes: &[(crate::core::id::SceneId, usize)],
    options: Options,
    metrics: &Metrics,
) -> LayoutWithScenes {
    let rows = options.rows.clamp(1, 100);
    let columns = options.columns.clamp(1, 100);
    let extent = if options.vertical { rows } else { columns };
    let breadth = if options.vertical { columns } else { rows };
    let units = units(text, options.vertical);
    let mut lines: Vec<Vec<Unit>> = vec![Vec::new()];
    for unit in units {
        if unit.ch == '\r' {
            continue;
        }
        if unit.ch == '\n' {
            lines.push(Vec::new());
            continue;
        }
        if lines.last().unwrap().len() >= extent {
            let mut moved = Vec::new();
            if NO_START.contains(unit.ch) && extent > 1 {
                if let Some(last) = lines.last_mut().unwrap().pop() {
                    moved.push(last);
                }
            }
            while lines
                .last()
                .unwrap()
                .last()
                .is_some_and(|u| NO_END.contains(u.ch))
            {
                moved.push(lines.last_mut().unwrap().pop().unwrap());
            }
            while moved.last().is_some_and(|u| NO_START.contains(u.ch))
                && !lines.last().unwrap().is_empty()
            {
                moved.push(lines.last_mut().unwrap().pop().unwrap());
            }
            moved.reverse();
            lines.push(moved);
        }
        lines.last_mut().unwrap().push(unit);
    }
    let advance = if metrics.advance.is_finite() {
        metrics.advance.max(0.01)
    } else {
        1.0
    };
    let line_advance = if metrics.line_advance.is_finite() {
        metrics.line_advance.max(0.01)
    } else {
        1.0
    };
    let scene_at = |byte_offset: usize| -> Option<crate::core::id::SceneId> {
        byte_scenes
            .iter()
            .find(|(range, _)| range.contains(&byte_offset))
            .map(|(_, id)| *id)
    };
    let mut pages = vec![Page::default()];
    let mut glyph_positions: Vec<(usize, usize, Option<crate::core::id::SceneId>)> = Vec::new();
    for (line, units) in lines.into_iter().enumerate() {
        if line > 0 && line % breadth == 0 {
            pages.push(Page::default());
        }
        let page_index = pages.len() - 1;
        let mut feed = 0.0;
        for (cell, unit) in units.into_iter().enumerate() {
            let (row, column) = if options.vertical {
                (cell, columns - 1 - line % breadth)
            } else {
                (line % breadth, cell)
            };
            let punctuation = options.vertical && PUNCTUATION.contains(unit.ch);
            let (x, y) = if options.vertical {
                (column as f32 * line_advance, feed)
            } else {
                (feed, row as f32 * line_advance)
            };
            let measured = metrics
                .glyph_advances
                .get(&unit.ch)
                .copied()
                .filter(|n| n.is_finite() && *n > 0.0)
                .unwrap_or(advance);
            feed += if unit.compact { advance } else { measured };
            // 括弧の互換形への置換は、使用フォントに互換形が収録されている場合だけ（§17.3.3）。
            // 収録が無ければ元の括弧を 90° 回転して描く（互換形の豆腐を出さない）。
            let mut text = unit.text;
            let mut rotate = unit.rotate
                && !(metrics.vertical_forms.contains(&unit.ch)
                    && VERTICAL_ROTATE.contains(unit.ch));
            if options.vertical && !unit.compact {
                let form = vertical_char(unit.ch);
                if form != unit.ch {
                    if metrics.vertical_forms.contains(&form) {
                        text = form.to_string();
                        rotate = false;
                    } else {
                        rotate = true;
                    }
                }
            }
            glyph_positions.push((page_index, unit.byte_offset, scene_at(unit.byte_offset)));
            pages.last_mut().unwrap().glyphs.push(Glyph {
                ch: unit.ch,
                text,
                ruby: unit.ruby,
                row,
                column,
                rotate,
                compact: unit.compact,
                punctuation,
                x: x + if punctuation { line_advance * 0.5 } else { 0.0 },
                y: y - if punctuation { advance * 0.25 } else { 0.0 },
            });
        }
    }
    while pages.len() > 1 && pages.last().unwrap().glyphs.is_empty() {
        pages.pop();
    }
    let last_page = pages.len().saturating_sub(1);
    let mut page_scenes = vec![None; pages.len()];
    for &(page_idx, _, scene) in &glyph_positions {
        if page_idx < page_scenes.len() && page_scenes[page_idx].is_none() && scene.is_some() {
            page_scenes[page_idx] = scene;
        }
    }
    let mut carry = section_bytes.first().map(|(id, _)| *id);
    for slot in &mut page_scenes {
        if let Some(id) = *slot {
            carry = Some(id);
        } else {
            *slot = carry;
        }
    }
    let section_pages = section_bytes
        .iter()
        .map(|&(id, start_byte)| {
            let page = glyph_positions
                .iter()
                .find(|(_, _, scene)| *scene == Some(id))
                .or_else(|| {
                    glyph_positions
                        .iter()
                        .find(|(_, byte_offset, _)| *byte_offset >= start_byte)
                })
                .map_or(last_page, |(page_idx, _, _)| (*page_idx).min(last_page));
            (id, page)
        })
        .collect();
    LayoutWithScenes {
        pages,
        page_scenes,
        section_pages,
    }
}
