//! セマンティックトークン（§3.4 `semantic_tokens::full(doc, project)` →
//! `textDocument/semanticTokens/full`）。種別は §6.3、配色は UI（§11.2）。
//!
//! # 返す形（`docs/history.md` B-80）
//!
//! **重ならない・位置の昇順**のトークン列。LSP の `semanticTokens/full` は重なりを表せないので、
//! 内側のものが外側を切り欠く（`「'リリアが来た」` は `dialogue.bracket` / `sigil.ref` /
//! `ref.character` / `dialogue.text` / `dialogue.bracket` に分かれる）。優先順位は
//! 「台詞 < ルビ < 参照 < 話者注釈 < コメント」で、コメントの中は何も色付けしない（§5.0.2）。
//!
//! §6.3 の `role-language` は「**下線専用、色は変えない**」ので、種別ではなく
//! [`SemanticToken::role_language`]（LSP の tokenModifiers に当たる）として返す。下線の範囲は
//! 台詞ごとに話者を決めて（§6.5.3「行末の話者注釈 → 直前の話者注釈の継続 → プロットの `pov`」）
//! モードを解決し、適用パターン（`modes::applicable_patterns`）を当てた結果（M3 の
//! `role_language::MatcherSet::underlines`）。「直前の話者注釈の継続」は**同じ節の中だけ**
//! （`;` を跨がない。B-81）。
//!
//! 3 つの周辺種別: `ref.address` は `address:` の呼称（§5.5.6）が辞書で解決できたとき、
//! `citation` は本文の引用注釈 `^M...`（§5.2.4、[`crate::core::annotations::citations`]）、
//! `illustration` は C 形式の `! ` 行の全体（§5.2.5、[`crate::core::annotations::illustration`]）。
//! enum は §6.3 の表どおり。

use std::ops::Range;

use serde::Serialize;

use crate::core::features::project::{self, Project};
use crate::core::id::SceneId;
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::resolve::dictionary::{EntryKind, ModifierKind, RefToken, EXPLICIT_OPEN};
use crate::core::resolve::{modes, role_language};
use crate::core::syntax::{contents, plot_episode};
use crate::core::text::lexer::{self, FieldKey, LineKind, TextFormat};

/// §6.3 のトークン種別（`role-language` は [`SemanticToken::role_language`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum TokenType {
    /// `sigil.structure`: 行頭 `@`、話者注釈の `@`。
    SigilStructure,
    /// `sigil.ref`: `'`、`'{`、`}`。
    SigilRef,
    /// `scene.id`: `01-04-02-03` の数値部。
    SceneId,
    /// `scene.title`: 見出し行のタイトル部。
    SceneTitle,
    /// `ref.character`: 解決済みキャラ名。
    RefCharacter,
    /// `ref.address`: 解決済み呼称（§5.5.6）。
    RefAddress,
    /// `ref.word`: 解決済み用語。
    RefWord,
    /// `ref.material`: 解決済み資料 ID（資料タイトルで引いたものを含む）。
    RefMaterial,
    /// `ref.unresolved`: 未解決参照。
    RefUnresolved,
    /// `citation`: 行末の資料引用注釈 `^M0137`（§5.2.4）。
    Citation,
    /// `illustration`: 挿絵メモ行 `! ...`（§5.2.5）。
    Illustration,
    /// `modifier.mode`: `:B` `:B+D`。
    ModifierMode,
    /// `modifier.title`: 資料タイトル修飾子。
    ModifierTitle,
    /// `dialogue.bracket`: `「` `」` `『` `』`。
    DialogueBracket,
    /// `dialogue.text`: 鉤括弧の内側。
    DialogueText,
    /// `speaker`: 話者注釈のキャラ名。
    Speaker,
    /// `section.break`: `;`。
    SectionBreak,
    /// `field.key`: フィールド行のキー部。
    FieldKey,
    /// `comment`: `//`、`/* */`（A / B / C 形式の行頭 `# ` も）。
    Comment,
    /// `ruby.base`: ルビの親文字。
    RubyBase,
    /// `ruby.text`: ルビの読み。
    RubyText,
}

impl TokenType {
    /// §6.3 の表の名前（`sigil.structure` など）。LSP の legend にもこの名前を使う。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SigilStructure => "sigil.structure",
            Self::SigilRef => "sigil.ref",
            Self::SceneId => "scene.id",
            Self::SceneTitle => "scene.title",
            Self::RefCharacter => "ref.character",
            Self::RefAddress => "ref.address",
            Self::RefWord => "ref.word",
            Self::RefMaterial => "ref.material",
            Self::RefUnresolved => "ref.unresolved",
            Self::Citation => "citation",
            Self::Illustration => "illustration",
            Self::ModifierMode => "modifier.mode",
            Self::ModifierTitle => "modifier.title",
            Self::DialogueBracket => "dialogue.bracket",
            Self::DialogueText => "dialogue.text",
            Self::Speaker => "speaker",
            Self::SectionBreak => "section.break",
            Self::FieldKey => "field.key",
            Self::Comment => "comment",
            Self::RubyBase => "ruby.base",
            Self::RubyText => "ruby.text",
        }
    }

    /// 全種別（§6.3 の表の順。`role-language` は修飾なので含まない）。
    pub const ALL: [TokenType; 21] = [
        Self::SigilStructure,
        Self::SigilRef,
        Self::SceneId,
        Self::SceneTitle,
        Self::RefCharacter,
        Self::RefAddress,
        Self::RefWord,
        Self::RefMaterial,
        Self::RefUnresolved,
        Self::Citation,
        Self::Illustration,
        Self::ModifierMode,
        Self::ModifierTitle,
        Self::DialogueBracket,
        Self::DialogueText,
        Self::Speaker,
        Self::SectionBreak,
        Self::FieldKey,
        Self::Comment,
        Self::RubyBase,
        Self::RubyText,
    ];
}

/// §6.3 の `role-language`（下線専用の修飾）の名前。
pub const ROLE_LANGUAGE: &str = "role-language";

/// トークン 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticToken {
    /// 文書の中の byte 範囲。
    pub range: Range<usize>,
    /// 種別。
    pub kind: TokenType,
    /// 役割語の一致部（§6.5.4 の緑の下線、§6.3 `role-language`）。色は変えない。
    pub role_language: bool,
}

/// 文書全体のトークン（§3.4）。重ならず、位置の昇順。
pub fn full(document: &Document, project: &Project) -> Vec<SemanticToken> {
    let Some(format) = document.format.text_format() else {
        return Vec::new();
    };
    let refs = project.ref_sites(document);
    let parsed = (document.format == DocumentFormat::Contents)
        .then(|| contents::parse_with(&document.text, &project.contents_options()));
    let underlines = match &parsed {
        Some(tree) => role_underlines(document, project, tree),
        None => Vec::new(),
    };
    let mut out = Vec::new();
    let mut refs_at = 0usize;
    for line in project::lines_of(&document.text) {
        let mut paint: Vec<Option<TokenType>> = vec![None; line.text.len()];
        let text_line = parsed
            .as_ref()
            .and_then(|tree| match tree.lines.get(line.index) {
                Some(contents::Line::Text(found)) => Some(found),
                _ => None,
            });
        let comments = paint_line(line.text, format, text_line, &mut paint);
        if let Some(text) = text_line {
            for (range, _) in crate::core::annotations::citations(text.body) {
                fill(&mut paint, range, TokenType::Citation);
            }
        }
        if format == TextFormat::Contents
            && crate::core::annotations::illustration(line.text).is_some()
        {
            fill(&mut paint, 0..line.text.len(), TokenType::Illustration);
        }
        while refs_at < refs.len() && refs[refs_at].line < line.index {
            refs_at += 1;
        }
        let mut next = refs_at;
        while next < refs.len() && refs[next].line == line.index {
            paint_ref(&refs[next].token, project, &mut paint);
            next += 1;
        }
        if let Some(found) = text_line.and_then(|text| text.speaker.as_ref()) {
            paint_speaker(found, &mut paint);
        }
        for comment in comments {
            fill(&mut paint, comment, TokenType::Comment);
        }
        let marks: Vec<Range<usize>> = underlines
            .iter()
            .filter(|range| range.start >= line.start && range.end <= line.start + line.text.len())
            .map(|range| range.start - line.start..range.end - line.start)
            .collect();
        encode(&paint, &marks, line.start, &mut out);
    }
    out
}

/// 行の構造（見出し・節区切り・フィールド・台詞・ルビ）を塗り、コメントの範囲を返す
/// （コメントは最後に上塗りする）。
fn paint_line(
    raw: &str,
    format: TextFormat,
    text_line: Option<&contents::TextLine<'_>>,
    paint: &mut [Option<TokenType>],
) -> Vec<Range<usize>> {
    let mut comments = Vec::new();
    let indent = raw.len() - lexer::after_indent(raw).len();
    match lexer::classify(raw, format) {
        LineKind::Structure(rest) => {
            fill(paint, indent..indent + 1, TokenType::SigilStructure);
            let body = match lexer::find_line_comment(rest) {
                Some(at) => {
                    comments.push(indent + 1 + at..raw.len());
                    &rest[..at]
                }
                None => rest,
            };
            let id_len = body.find(lexer::is_space).unwrap_or(body.len());
            let id_start = indent + 1;
            fill(paint, id_start..id_start + id_len, TokenType::SceneId);
            let title = body[id_len..].trim_matches(lexer::is_space);
            if !title.is_empty() {
                let at = id_start + id_len + body[id_len..].find(title).unwrap_or(0);
                fill(paint, at..at + title.len(), TokenType::SceneTitle);
            }
        }
        LineKind::SectionBreak => {
            fill(paint, indent..indent + 1, TokenType::SectionBreak);
            if let Some(at) = lexer::find_line_comment(raw) {
                comments.push(at..raw.len());
            }
        }
        LineKind::Comment { .. } => comments.push(indent..raw.len()),
        LineKind::Field(field) => {
            fill(
                paint,
                indent..indent + field.key_text.len(),
                TokenType::FieldKey,
            );
            if let Some(at) = lexer::find_line_comment(&raw[field.value_at..]) {
                comments.push(field.value_at + at..raw.len());
            }
            push_inline_comment(raw, &mut comments);
        }
        LineKind::Continuation { .. } | LineKind::Text(_) | LineKind::IllustrationMemo(_) => {
            if let Some(text) = text_line {
                paint_dialogue(text.body, paint);
                for ruby in &text.rubies {
                    paint_ruby(ruby, paint);
                }
                if let Some(comment) = &text.comment {
                    comments.push(comment.range.clone());
                }
                if let Some(inline) = &text.inline_comment {
                    comments.push(inline.clone());
                }
            } else {
                if let Some(at) = lexer::find_line_comment(raw) {
                    comments.push(at..raw.len());
                }
                push_inline_comment(raw, &mut comments);
            }
        }
    }
    comments
}

fn push_inline_comment(raw: &str, comments: &mut Vec<Range<usize>>) {
    if let Some(found) = lexer::find_inline_comment(raw) {
        comments.push(found);
    }
}

/// 台詞（§5.0.2 `「」`、C 形式だけ）。外側から塗るので、入れ子の内側の括弧も括弧になる。
fn paint_dialogue(body: &str, paint: &mut [Option<TokenType>]) {
    let mut spans = role_language::dialogue_spans(body);
    spans.sort_by_key(|span| std::cmp::Reverse(span.end - span.start));
    for span in spans {
        fill(paint, span.clone(), TokenType::DialogueText);
        let open = body[..span.start]
            .chars()
            .next_back()
            .map_or(0, char::len_utf8);
        fill(
            paint,
            span.start - open..span.start,
            TokenType::DialogueBracket,
        );
        if let Some(close) = body[span.end..].chars().next() {
            if role_language::QUOTES.iter().any(|(_, end)| *end == close) {
                fill(
                    paint,
                    span.end..span.end + close.len_utf8(),
                    TokenType::DialogueBracket,
                );
            }
        }
    }
}

/// ルビ（§5.2.1 `ruby`）。`｜base《reading》` / `漢字《reading》`。
fn paint_ruby(ruby: &contents::Ruby<'_>, paint: &mut [Option<TokenType>]) {
    let bar = if ruby.explicit {
        ruby.range.end - ruby.range.start - ruby.base.len() - ruby.reading.len() - 6
    } else {
        0
    };
    let base = ruby.range.start + bar;
    fill(paint, base..base + ruby.base.len(), TokenType::RubyBase);
    let reading = base + ruby.base.len() + '《'.len_utf8();
    fill(
        paint,
        reading..reading + ruby.reading.len(),
        TokenType::RubyText,
    );
}

/// `'` 参照（§6.2）。
fn paint_ref(token: &RefToken, project: &Project, paint: &mut [Option<TokenType>]) {
    let start = token.range.start;
    let sigil_end = if token.explicit {
        start + 1 + EXPLICIT_OPEN.len_utf8()
    } else {
        start + 1
    };
    fill(paint, start..sigil_end, TokenType::SigilRef);
    let kind = match token
        .resolved
        .and_then(|id| project.dictionary().entry(id))
        .map(|entry| entry.kind)
    {
        Some(EntryKind::Character) => TokenType::RefCharacter,
        Some(EntryKind::Address) => TokenType::RefAddress,
        Some(EntryKind::Word) => TokenType::RefWord,
        Some(EntryKind::Material | EntryKind::MaterialTitle) => TokenType::RefMaterial,
        None => TokenType::RefUnresolved,
    };
    fill(paint, token.key_range.clone(), kind);
    if token.explicit && !token.unclosed {
        let close = token.key_range.end;
        fill(paint, close..close + 1, TokenType::SigilRef);
    }
    if let Some(modifier) = &token.modifier {
        let kind = match modifier.kind {
            ModifierKind::Title | ModifierKind::NotAllowed => TokenType::ModifierTitle,
            ModifierKind::Modes(_) | ModifierKind::MalformedMode | ModifierKind::TooManyModes => {
                TokenType::ModifierMode
            }
        };
        fill(paint, modifier.range.clone(), kind);
    }
}

/// 話者注釈（§5.2.1 `speaker_annot`）: `@` → `sigil.structure`、名前 → `speaker`、`:B+D` → `modifier.mode`。
fn paint_speaker(speaker: &contents::Speaker<'_>, paint: &mut [Option<TokenType>]) {
    let at = speaker.sigil_at;
    fill(paint, at..at + 1, TokenType::SigilStructure);
    let name = at + 1;
    fill(paint, name..name + speaker.name.len(), TokenType::Speaker);
    if !speaker.modes.is_empty() {
        let modes = name + speaker.name.len();
        let end = speaker.range.end;
        fill(paint, modes..end, TokenType::ModifierMode);
    }
}

fn fill(paint: &mut [Option<TokenType>], range: Range<usize>, kind: TokenType) {
    let end = range.end.min(paint.len());
    for slot in paint.iter_mut().take(end).skip(range.start) {
        *slot = Some(kind);
    }
}

/// 塗った結果を連長圧縮してトークンにする（`marks` は行内の役割語の下線）。
fn encode(
    paint: &[Option<TokenType>],
    marks: &[Range<usize>],
    line_start: usize,
    out: &mut Vec<SemanticToken>,
) {
    let marked = |at: usize| marks.iter().any(|range| range.contains(&at));
    let mut at = 0usize;
    while at < paint.len() {
        let Some(kind) = paint[at] else {
            at += 1;
            continue;
        };
        let role = marked(at);
        let start = at;
        while at < paint.len() && paint[at] == Some(kind) && marked(at) == role {
            at += 1;
        }
        out.push(SemanticToken {
            range: line_start + start..line_start + at,
            kind,
            role_language: role,
        });
    }
}

// ============================================================ 役割語の下線（§6.5）

/// 本文の役割語の下線（ファイル単位）。話者の決め方は §6.5.3（モジュールの doc）。
fn role_underlines(
    document: &Document,
    project: &Project,
    parsed: &contents::Contents<'_>,
) -> Vec<Range<usize>> {
    let characters = project.characters();
    let episode = project::episode_of_path(&document.path);
    let plot = episode.and_then(|(chapter, number)| project.episode_plot(chapter, number));
    let chapter_plot = episode.and_then(|(chapter, _)| project.chapter_plot(chapter));
    let all_plot = project.all_plot();
    let input = modes::Input {
        characters: &characters,
        episode: plot.as_ref(),
        chapter: chapter_plot.as_ref(),
        all: all_plot.as_ref(),
        dictionary: project.dictionary(),
    };
    let starts = project::line_starts_of(&document.text);
    let mut cache = role_language::Cache::new(project.settings().lexicon.clone());
    let mut out = Vec::new();
    for section in &parsed.sections {
        let Some(scene) = section.scene else {
            continue;
        };
        let scene = SceneId {
            section: section.number,
            ..scene
        };
        let pov = plot.as_ref().and_then(|tree| pov_of(project, tree, &scene));
        let mut current: Option<usize> = None;
        for index in section.span.clone() {
            let Some(contents::Line::Text(text)) = parsed.lines.get(index) else {
                continue;
            };
            let own = text
                .speaker
                .as_ref()
                .and_then(|found| character_index(project, found.name));
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
                // 段 1（話者注釈の修飾子）は「その行のみ」（§6.4.3）
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
            for span in spans {
                let utterance = &text.body[span.clone()];
                for mark in set.underlines(utterance) {
                    let start = line_start + span.start + mark.start;
                    out.push(start..start + (mark.end - mark.start));
                }
            }
        }
    }
    out
}

/// 話プロットの `pov`（節 → 場面の順。§6.5.3 の 3 番目の決め方）。
fn pov_of(
    project: &Project,
    plot: &plot_episode::EpisodePlot<'_>,
    scene: &SceneId,
) -> Option<usize> {
    let (index, found) = plot.scenes.iter().enumerate().find(|(_, found)| {
        found.id.is_some_and(|id| {
            id.chapter == scene.chapter && id.episode == scene.episode && id.scene == scene.scene
        })
    })?;
    let section = found
        .sections
        .iter()
        .position(|section| section.number == scene.section);
    let mut fields = section
        .map(|at| plot.section_fields(index, at, FieldKey::Pov))
        .unwrap_or_default();
    fields.extend(plot.scene_fields(index, FieldKey::Pov));
    fields
        .iter()
        .flat_map(|field| field.refs())
        .find_map(|reference| character_index(project, reference.name))
}

fn character_index(project: &Project, name: &str) -> Option<usize> {
    let id = project
        .dictionary()
        .lookup(name, Some(&[EntryKind::Character]))?;
    match &project.dictionary().entry(id)?.target {
        crate::core::resolve::dictionary::Target::Character { index, .. } => Some(*index),
        _ => None,
    }
}
