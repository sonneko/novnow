//! 素のテキストからの移行（§14.6）。復号済みの原稿を、候補の提示をもとに
//! 章・話・場面・節へ分け、新しいプロジェクトの一式として書き出す。
//! 純粋な関数だけを置く（入出力も、元のパスも持たない）。

use std::cmp::Ordering;

use crate::core::template::{self, TemplateFile};
use crate::core::text::lexer::{self, Newline};

/// 移行でプロジェクトを作れない理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationError {
    /// 分けた後に本文が 1 行も残らない（見出しと区切りだけだった）。
    Empty,
}

/// 復号済みの入力ファイル 1 つ。引数の順に並べる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceUnit {
    /// 表示用の名前（ファイル名）。プロジェクトの中へは書かない。
    pub name: String,
    /// 復号済みの文字列。改行はまだ正規化していない。
    pub text: String,
    /// フォルダ入力を与えたときの先頭のファイルで `true`。その前には話の境界ではなく
    /// 章の境界の候補を置く（§14.6.2）。
    pub starts_chapter: bool,
}

/// 境界の強さ。宣言の順に強くなるので `max` が意味を持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Section,
    Scene,
    Episode,
    Chapter,
}

/// 2 行のあいだの分割の候補。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    pub level: Level,
    /// 見出しの行から取ったタイトル。見出しでない候補では `None`。
    pub title: Option<String>,
}

/// 解析した原稿の要素 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element {
    /// A body line.
    Line(String),
    /// A split candidate.
    Boundary(Boundary),
    /// 直前の境界が取り込んだ行（見出し・区切り・境界が使った空行）。その境界を
    /// 外すと、本文の行へ戻る。
    Hidden(String),
}

/// 解析した原稿。本文の行と、書き換えられる分割の候補。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub elements: Vec<Element>,
}

impl Draft {
    /// 要素の添字 `at` の境界を変える。`None` を渡すと境界を外し、それが取り込んでいた
    /// 行を本文へ戻す。`at` が境界でなければ `false` を返す。
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

/// 確定した構造。構造番号は書き出すときに並び順から振る。
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
    /// 節は本文の行の並び（先頭と末尾の空行は含まない）。
    pub sections: Vec<Vec<String>>,
}

impl Book {
    /// プレビューに出す（章・話・場面・節）の件数。
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

/// ファイル名の自然順。連続する数字は数として比べる（`2.txt` が `10.txt` より前）。
/// 全角の数字も数字として数える（§14.6.2）。
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

/// 名前の付いた見出しに付いてよい番号の前置き（「番外編その○」。§14.6.2）。
/// i18n: format-vocabulary
const SIDESTORY_NUMBER_PREFIX: &str = "その";

/// 見出しの行は単独の短い行。その上限の文字数（§14.6.2）。
const HEADING_MAX_CHARS: usize = 40;

/// 区切りの行を作る記号（§14.6.2）。同じ記号が 3 つ以上並んだ行を区切りとする
/// （空白は数えない）。台詞の間を取る `…` や `・` は、場面の区切りと取り違えないよう
/// 入れていない。
const SEPARATOR_CHARS: &[char] = &[
    '＊', '*', '※', '＃', '#', '―', '－', '-', '=', '＝', '◇', '◆', '○', '●', '△', '▲', '□', '■',
    '☆', '★',
];

/// 名前の付いた見出しと、それが開く階層（§14.6.2）。
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

/// 入力全体の分割の候補を作る（§14.6.2）。この段階では候補を自動で確定しない。
/// 確定するのは `structure` で、（書き換えられたかもしれない）下書きから結果を決める。
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
    // 末尾の改行のぶん、`split` は最後に空の "" を返す。
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
            // 空行の連なりが境界になるのは、本文の行と本文の行のあいだのときだけ。
            // 手前は本文の行があり、後ろはこの単位の中に行が続くこと（ファイルの
            // 境目は自前の候補を持つ）。
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

/// 見出しの形（§14.6.2）だけを見る。「単独の短い行か」は見ない。階層と、行の残りから
/// 取ったタイトルを返す。
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
        // 「番外編その○」のように番号が付いても、話の見出しとして扱う。
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

/// 見出しの語の後ろを確かめ、そこからタイトルを取る。外側の `None` は「この行は
/// 見出しではない」、内側の `None` は「タイトルの無い見出し」。`allow_punct` は語の直後の
/// `、` `。` も許す（数字が前にある「第○章」「第○話」は安全だが、「幕間、…」で始まる
/// 地の文を見出しにしてはならない）。
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

/// 見出しの語とタイトルのあいだに置いてよい文字。
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

/// 区切りの行。区切りの記号が 3 つ以上（空白は数えない。§14.6.2）。
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

/// 下書きから構造を決める。重なった候補は入れ子の単位を開く。本文の行を持たない
/// 単位は最後に刈り取るので、冒頭の「第一章」＋「第一話」はどちらのタイトルも残り、
/// 構造番号は出現順に振られる（§14.6.2）。
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

/// 本文の行を持たない節・場面・話・章を落とし、節の先頭と末尾の空行も落とす
/// （途中の空行は残す）。
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

/// 確定した構造を、新しいプロジェクトのファイル一式として書き出す。既定の雛形の
/// サンプルの本文とプロットを、移行した構造で置き換える。あらすじは空のままにし、
/// 辞書のファイルも空から始める（サンプルの語句が混ざらないように）。
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

/// 移行した本文がそのままの文字で残るよう、novnow の記号をエスケープする（§5.0.3）。
fn escape_into(out: &mut String, line: &str) {
    for ch in line.chars() {
        if lexer::ESCAPE_TARGETS.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
}

/// 解析・確定・書き出しを 1 回で行う。候補は生成されたとおりに使う（CLI の経路。
/// デスクトップのプレビューは、この間に下書きを書き換える）。
pub fn convert(title: &str, units: &[SourceUnit]) -> Result<Vec<TemplateFile>, MigrationError> {
    render(title, &structure(&analyze(units)))
}
