//! Plain-text migration (§14.6): candidate-based splitting of decoded
//! manuscripts into chapters / episodes / scenes / sections, rendered as a
//! fresh project. Pure functions only — no I/O, no source paths.

use std::cmp::Ordering;

use crate::core::template::{self, TemplateFile};
use crate::core::text::lexer::{self, Newline};

/// Why a migration cannot produce a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationError {
    /// Nothing with body text remains after splitting (headings and
    /// separators only).
    Empty,
}

/// One decoded input file, in argument order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceUnit {
    /// Display name (the file's name). Never written into the project.
    pub name: String,
    /// Decoded text, not yet newline-normalized.
    pub text: String,
    /// True for the first file of a folder input: place a chapter boundary
    /// candidate before it instead of an episode one.
    pub starts_chapter: bool,
}

/// Boundary strength. Declaration order makes `max` meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Section,
    Scene,
    Episode,
    Chapter,
}

/// A split candidate between two lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    pub level: Level,
    /// Title taken from a heading line, when the candidate has one.
    pub title: Option<String>,
}

/// One item of the analyzed manuscript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element {
    /// A body line.
    Line(String),
    /// A split candidate.
    Boundary(Boundary),
    /// A line consumed by the boundary before it (a heading, a separator,
    /// or blank lines a boundary used). Restored to a body line when that
    /// boundary is removed.
    Hidden(String),
}

/// The analyzed manuscript: lines plus editable split candidates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub elements: Vec<Element>,
}

impl Draft {
    /// Change the boundary at element index `at`; `None` removes it and
    /// restores the lines it consumed to the body. Returns false when the
    /// element is not a boundary.
    pub fn set_boundary(&mut self, at: usize, level: Option<Level>) -> bool {
        match (self.elements.get(at), level) {
            (Some(Element::Boundary(_)), Some(level)) => {
                if let Some(Element::Boundary(boundary)) = self.elements.get_mut(at) {
                    boundary.level = level;
                }
                true
            }
            (Some(Element::Boundary(_)), None) => {
                let mut i = at + 1;
                while matches!(self.elements.get(i), Some(Element::Hidden(_))) {
                    if let Element::Hidden(text) = &mut self.elements[i] {
                        let text = std::mem::take(text);
                        self.elements[i] = Element::Line(text);
                    }
                    i += 1;
                }
                self.elements.remove(at);
                true
            }
            _ => false,
        }
    }
}

/// The determined structure: numbers are assigned by position at render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Book {
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub title: Option<String>,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    pub title: Option<String>,
    pub scenes: Vec<Scene>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene {
    pub title: Option<String>,
    /// Each section is its body lines (no leading/trailing blank lines).
    pub sections: Vec<Vec<String>>,
}

impl Book {
    /// (chapters, episodes, scenes, sections) for previews.
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let mut episodes = 0;
        let mut scenes = 0;
        let mut sections = 0;
        for chapter in &self.chapters {
            episodes += chapter.episodes.len();
            for episode in &chapter.episodes {
                scenes += episode.scenes.len();
                for scene in &episode.scenes {
                    sections += scene.sections.len();
                }
            }
        }
        (self.chapters.len(), episodes, scenes, sections)
    }
}

/// Natural ordering for file names: digit runs compare as numbers
/// (`2.txt` before `10.txt`). Fullwidth digits count as digits.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn digit(ch: char) -> Option<u32> {
        match ch {
            '0'..='9' => Some(ch as u32 - '0' as u32),
            '０'..='９' => Some(ch as u32 - '０' as u32),
            _ => None,
        }
    }
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        match (a.peek(), b.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&x), Some(&y)) => match (digit(x), digit(y)) {
                (Some(_), Some(_)) => {
                    // 数字列は数値で比較する（`2.txt` < `10.txt`）。同じ値なら
                    // 桁の少ない方が前（`a1` < `a01`）。
                    let run = |chars: &mut std::iter::Peekable<std::str::Chars>| -> (u128, usize) {
                        let mut value = 0u128;
                        let mut len = 0usize;
                        while let Some(&ch) = chars.peek() {
                            match digit(ch) {
                                Some(value_at) => {
                                    value = value.saturating_mul(10) + value_at as u128;
                                    len += 1;
                                    chars.next();
                                }
                                None => break,
                            }
                        }
                        (value, len)
                    };
                    let left = run(&mut a);
                    let right = run(&mut b);
                    match left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                }
                _ => match x.cmp(&y) {
                    Ordering::Equal => {
                        a.next();
                        b.next();
                    }
                    other => return other,
                },
            },
        }
    }
}

/// Number prefix that a named heading may carry (番外編その○, §14.6.2).
/// i18n: format-vocabulary
const SIDESTORY_NUMBER_PREFIX: &str = "その";

/// A heading line is a lone, short line: at most this many characters.
const HEADING_MAX_CHARS: usize = 40;

/// Symbols that make a separator line (§14.6.2) when a line consists of at
/// least three of them (spaces ignored). Ellipsis and nakaguro are excluded
/// so dialogue pauses are never taken for scene breaks.
const SEPARATOR_CHARS: &[char] = &[
    '＊', '*', '※', '＃', '#', '―', '－', '-', '=', '＝', '◇', '◆', '○', '●', '△', '▲', '□', '■',
    '☆', '★',
];

/// Named headings and the level they open (§14.6.2).
/// i18n: format-vocabulary
const NAMED_HEADINGS: &[(&str, Level)] = &[
    ("序章", Level::Chapter),
    ("終章", Level::Chapter),
    ("プロローグ", Level::Chapter),
    ("エピローグ", Level::Chapter),
    ("幕間", Level::Episode),
    ("閑話", Level::Episode),
    ("番外編", Level::Episode),
];

/// Generate split candidates for the whole input (§14.6.2). Candidates are
/// never applied automatically at this stage: `structure` determines the
/// result from the (possibly edited) draft.
pub fn analyze(units: &[SourceUnit]) -> Draft {
    let mut draft = Draft::default();
    for (index, unit) in units.iter().enumerate() {
        if index > 0 {
            draft.elements.push(Element::Boundary(Boundary {
                level: if unit.starts_chapter {
                    Level::Chapter
                } else {
                    Level::Episode
                },
                title: None,
            }));
        }
        analyze_unit(&mut draft, &unit.text);
    }
    draft
}

fn analyze_unit(draft: &mut Draft, text: &str) {
    let normalized = lexer::normalize_for_save(text, Newline::Lf);
    let mut lines: Vec<&str> = normalized.split('\n').collect();
    // split always yields a trailing "" for the final newline.
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.is_empty() {
            let start = i;
            while i < lines.len() && lines[i].is_empty() {
                i += 1;
            }
            let run = i - start;
            // A blank run becomes a boundary only between body lines: a
            // text line before it in the stream and any line after it in
            // this unit (file boundaries carry their own candidate).
            let after_text =
                matches!(draft.elements.last(), Some(Element::Line(text)) if !text.is_empty());
            if after_text && i < lines.len() {
                draft.elements.push(Element::Boundary(Boundary {
                    level: if run >= 2 {
                        Level::Scene
                    } else {
                        Level::Section
                    },
                    title: None,
                }));
                for _ in 0..run {
                    draft.elements.push(Element::Hidden(String::new()));
                }
            } else {
                for _ in 0..run {
                    draft.elements.push(Element::Line(String::new()));
                }
            }
            continue;
        }
        let alone = i == 0 || lines[i - 1].is_empty() || heading_pattern(lines[i - 1]).is_some();
        if alone {
            if let Some((level, title)) = heading_pattern(line) {
                draft
                    .elements
                    .push(Element::Boundary(Boundary { level, title }));
                draft.elements.push(Element::Hidden(line.to_owned()));
                i += 1;
                continue;
            }
        }
        if is_separator(line) {
            draft.elements.push(Element::Boundary(Boundary {
                level: Level::Scene,
                title: None,
            }));
            draft.elements.push(Element::Hidden(line.to_owned()));
        } else {
            draft.elements.push(Element::Line(line.to_owned()));
        }
        i += 1;
    }
}

/// The heading shape (§14.6.2) without the "lone line" context: returns the
/// level and the title taken from the rest of the line.
fn heading_pattern(line: &str) -> Option<(Level, Option<String>)> {
    let trimmed = line.trim();
    if trimmed.chars().count() > HEADING_MAX_CHARS {
        return None;
    }
    let body = trimmed.trim_start_matches([
        ' ', '　', '【', '[', '〈', '《', '「', '『', '（', '(', '〔', '〖',
    ]);
    if let Some(rest) = body.strip_prefix('第') {
        let digits_end = rest.find(|ch: char| !is_numeral(ch)).unwrap_or(rest.len());
        if digits_end == 0 {
            return None;
        }
        let after = &rest[digits_end..];
        if let Some(tail) = after.strip_prefix('章') {
            return title_tail(tail, true).map(|title| (Level::Chapter, title));
        }
        if let Some(tail) = after.strip_prefix('話') {
            return title_tail(tail, true).map(|title| (Level::Episode, title));
        }
        return None;
    }
    for (word, level) in NAMED_HEADINGS {
        let Some(mut tail) = body.strip_prefix(word) else {
            continue;
        };
        // 番外編その○: a numbered side story still names the episode.
        if let Some(numbered) = tail.strip_prefix(SIDESTORY_NUMBER_PREFIX) {
            let digits_end = numbered
                .find(|ch: char| !is_numeral(ch))
                .unwrap_or(numbered.len());
            if digits_end > 0 {
                tail = &numbered[digits_end..];
            }
        }
        return title_tail(tail, false).map(|title| (*level, title));
    }
    None
}

/// Validate what follows a heading keyword and take the title from it.
/// Outer `None` rejects the line as a heading; inner `None` is a heading
/// without a title. `allow_punct` also accepts 、。 right after the keyword
/// (safe for 第○章/第○話， which the numeral anchors, but a prose line
/// starting with 幕間、… must not become a heading).
fn title_tail(tail: &str, allow_punct: bool) -> Option<Option<String>> {
    if tail.is_empty() {
        return Some(None);
    }
    let first = tail.chars().next().expect("non-empty tail");
    let punct = matches!(first, '、' | '。' | '，' | '．');
    if punct && !allow_punct {
        return None;
    }
    if !punct && !is_title_separator(first) {
        return None;
    }
    let mut title = tail
        .trim_start_matches(|ch: char| {
            is_title_separator(ch) || matches!(ch, '、' | '。' | '，' | '．')
        })
        .trim_end()
        .trim_end_matches(['】', '〉', '》', '」', '』', '）', ')', ']'])
        .trim_end()
        .to_owned();
    for (open, close) in [
        ('「', '」'),
        ('『', '』'),
        ('【', '】'),
        ('〈', '〉'),
        ('《', '》'),
        ('（', '）'),
        ('(', ')'),
    ] {
        if title.starts_with(open) && title.ends_with(close) && title.chars().count() >= 2 {
            title = title[open.len_utf8()..title.len() - close.len_utf8()]
                .trim()
                .to_owned();
            break;
        }
    }
    if title.is_empty() {
        Some(None)
    } else {
        Some(Some(title))
    }
}

/// Characters that may sit between a heading keyword and its title.
fn is_title_separator(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '　'
            | '：'
            | ':'
            | '；'
            | ';'
            | '・'
            | '―'
            | 'ー'
            | '－'
            | '-'
            | '‐'
            | '~'
            | '〜'
            | '='
            | '＝'
            | '/'
            | '／'
            | '「'
            | '『'
            | '【'
            | '〈'
            | '《'
            | '（'
            | '('
    )
}

fn is_numeral(ch: char) -> bool {
    matches!(ch, '0'..='9' | '０'..='９')
        || matches!(
            ch,
            '〇' | '零'
                | '一'
                | '二'
                | '三'
                | '四'
                | '五'
                | '六'
                | '七'
                | '八'
                | '九'
                | '十'
                | '百'
                | '千'
                | '万'
                | '壱'
                | '弐'
                | '参'
        )
}

/// A separator line: at least three separator symbols, spaces ignored.
fn is_separator(line: &str) -> bool {
    let mut count = 0usize;
    for ch in line.chars() {
        if ch == ' ' || ch == '　' {
            continue;
        }
        if !SEPARATOR_CHARS.contains(&ch) {
            return false;
        }
        count += 1;
    }
    count >= 3
}

/// Determine the structure from a draft. Stacked candidates open nested
/// units; units that end up without body lines are then pruned, so an
/// opening 「第一章」+「第一話」 keeps both titles and numbers follow the
/// order of appearance.
pub fn structure(draft: &Draft) -> Book {
    fn chapter(title: Option<String>) -> Chapter {
        Chapter {
            title,
            episodes: vec![episode(None)],
        }
    }
    fn episode(title: Option<String>) -> Episode {
        Episode {
            title,
            scenes: vec![scene(None)],
        }
    }
    fn scene(title: Option<String>) -> Scene {
        Scene {
            title,
            sections: vec![Vec::new()],
        }
    }
    fn current_chapter(book: &mut Book) -> &mut Chapter {
        book.chapters.last_mut().expect("a chapter is always open")
    }
    fn current_episode(book: &mut Book) -> &mut Episode {
        current_chapter(book)
            .episodes
            .last_mut()
            .expect("an episode is always open")
    }
    fn current_scene(book: &mut Book) -> &mut Scene {
        current_episode(book)
            .scenes
            .last_mut()
            .expect("a scene is always open")
    }

    let mut book = Book {
        chapters: vec![chapter(None)],
    };
    for element in &draft.elements {
        match element {
            Element::Line(text) => current_scene(&mut book)
                .sections
                .last_mut()
                .expect("a section is always open")
                .push(text.clone()),
            Element::Hidden(_) => {}
            Element::Boundary(boundary) => match boundary.level {
                Level::Chapter => book.chapters.push(chapter(boundary.title.clone())),
                Level::Episode => current_chapter(&mut book)
                    .episodes
                    .push(episode(boundary.title.clone())),
                Level::Scene => current_episode(&mut book)
                    .scenes
                    .push(scene(boundary.title.clone())),
                Level::Section => current_scene(&mut book).sections.push(Vec::new()),
            },
        }
    }
    prune(&mut book);
    book
}

/// Drop sections/scenes/episodes/chapters that hold no body lines, and the
/// blank lines at section edges (interior blank lines stay).
fn prune(book: &mut Book) {
    for chapter in &mut book.chapters {
        for episode in &mut chapter.episodes {
            for scene in &mut episode.scenes {
                for section in &mut scene.sections {
                    match section.iter().position(|line| !line.is_empty()) {
                        None => section.clear(),
                        Some(first) => {
                            section.drain(..first);
                            while section.last().is_some_and(|line| line.is_empty()) {
                                section.pop();
                            }
                        }
                    }
                }
                scene.sections.retain(|section| !section.is_empty());
            }
            episode.scenes.retain(|scene| !scene.sections.is_empty());
        }
        chapter
            .episodes
            .retain(|episode| !episode.scenes.is_empty());
    }
    book.chapters.retain(|chapter| !chapter.episodes.is_empty());
}

/// Render a determined structure as a fresh project's files: the default
/// template with its sample body/plots replaced by the migrated structure.
/// Summaries are left empty; dictionary files start empty so no sample
/// wording leaks in.
pub fn render(title: &str, book: &Book) -> Result<Vec<TemplateFile>, MigrationError> {
    if book.chapters.is_empty() {
        return Err(MigrationError::Empty);
    }
    let mut generated: Vec<TemplateFile> = Vec::new();
    for (chapter_index, chapter) in book.chapters.iter().enumerate() {
        let cc = chapter_index + 1;
        for (episode_index, episode) in chapter.episodes.iter().enumerate() {
            let ee = episode_index + 1;
            let mut text = String::new();
            for (scene_index, scene) in episode.scenes.iter().enumerate() {
                text.push_str(&format!("@{cc:02}-{ee:02}-{:02}", scene_index + 1));
                push_title(&mut text, &scene.title);
                text.push('\n');
                for (section_index, section) in scene.sections.iter().enumerate() {
                    if section_index > 0 {
                        text.push_str(";\n");
                    }
                    for line in section {
                        escape_into(&mut text, line);
                        text.push('\n');
                    }
                }
            }
            generated.push(TemplateFile {
                path: format!("contents/{cc:02}-{ee:02}.cnov"),
                text,
            });
        }
    }
    let mut all = String::new();
    for (chapter_index, chapter) in book.chapters.iter().enumerate() {
        if chapter_index > 0 {
            all.push('\n');
        }
        all.push_str(&format!("@{:02}", chapter_index + 1));
        push_title(&mut all, &chapter.title);
        all.push_str("\nsummary: \n;\n");
    }
    generated.push(TemplateFile {
        path: "plots/all.cnov".to_owned(),
        text: all,
    });
    for (chapter_index, chapter) in book.chapters.iter().enumerate() {
        let cc = chapter_index + 1;
        let mut chapter_plot = String::new();
        for (episode_index, episode) in chapter.episodes.iter().enumerate() {
            if episode_index > 0 {
                chapter_plot.push('\n');
            }
            chapter_plot.push_str(&format!("@{cc:02}-{:02}", episode_index + 1));
            push_title(&mut chapter_plot, &episode.title);
            chapter_plot.push_str("\nsummary: \n;\n");
        }
        generated.push(TemplateFile {
            path: format!("plots/{cc:02}.cnov"),
            text: chapter_plot,
        });
        for (episode_index, episode) in chapter.episodes.iter().enumerate() {
            let ee = episode_index + 1;
            let mut episode_plot = String::new();
            for (scene_index, scene) in episode.scenes.iter().enumerate() {
                episode_plot.push_str(&format!("@{cc:02}-{ee:02}-{:02}", scene_index + 1));
                push_title(&mut episode_plot, &scene.title);
                episode_plot.push('\n');
                for _ in 1..scene.sections.len() {
                    episode_plot.push_str(";\n");
                }
            }
            generated.push(TemplateFile {
                path: format!("plots/{cc:02}-{ee:02}.cnov"),
                text: episode_plot,
            });
        }
    }

    let mut out: Vec<TemplateFile> = Vec::new();
    let mut generated = generated.into_iter();
    let mut inserted = false;
    for mut file in template::new_project(title) {
        if file.path.starts_with("contents/") || file.path.starts_with("plots/") {
            if !inserted {
                out.append(&mut generated.by_ref().collect());
                inserted = true;
            }
            continue;
        }
        if matches!(
            file.path.as_str(),
            "characters.md" | "words.md" | "ideas.md"
        ) {
            file.text = String::new();
        }
        out.push(file);
    }
    debug_assert!(inserted, "the template always has sample contents");
    Ok(out)
}

fn push_title(out: &mut String, title: &Option<String>) {
    if let Some(title) = title {
        out.push(' ');
        out.push_str(title);
    }
}

/// Escape novnow symbols so migrated prose stays literal (§5.0.3).
fn escape_into(out: &mut String, line: &str) {
    for ch in line.chars() {
        if lexer::ESCAPE_TARGETS.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
}

/// Analyze, determine and render in one call with candidates as generated
/// (the CLI path; the desktop preview edits the draft in between).
pub fn convert(title: &str, units: &[SourceUnit]) -> Result<Vec<TemplateFile>, MigrationError> {
    render(title, &structure(&analyze(units)))
}
