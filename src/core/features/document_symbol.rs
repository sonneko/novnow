//! 構造ツリー（§3.4 `document_symbol::all(doc)` → `textDocument/documentSymbol`）。
//!
//! v1 の用途は構造ツリー・ジャンプ（§3.4 の表）。**プロジェクトを見ない**（§3.4 の表の
//! シグネチャが `all(doc)` だけ）ので、パーサの設定は既定値（§4.5 の既定）で読む。
//!
//! | 形式 | 返すもの |
//! |---|---|
//! | C（本文） | 場面見出し → 節（§5.2.2 の節番号） |
//! | B（話プロット） | 場面見出し → 節 |
//! | A（全体・章プロット） | ブロック（1 階層は章、2 階層は話） |
//! | `characters.md` | キャラ（H1） → モード（`### <mode_id> <表示名>`） |
//! | `words.md` | 用語（H1） |
//! | `ideas.md` | アイデア（H1） |
//! | `prompts/**/*.md` | 役割（H1） → セクション（H2） |
//!
//! 名前と詳細は**ファイルに書かれた文字列そのもの**（ID・タイトル・名前）で、UI の文言では
//! ない（§12.2 の対象外）。節は名前に節番号（`1`, `2`, …）を入れる。

use std::ops::Range;

use serde::Serialize;

use crate::core::features::project;
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::syntax::{
    characters_md, contents, ideas_md, plot_episode, plot_outline, prompt_md, words_md,
};

/// シンボルの種別（LSP の `SymbolKind` への写し方はアダプタが決める）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum SymbolKind {
    /// 章（A 形式の 1 階層ブロック）。
    Chapter,
    /// 話（A 形式の 2 階層ブロック）。
    Episode,
    /// 場面（C / B 形式の見出し、A 形式の 3 階層以上のブロック）。
    Scene,
    /// 節（§5.2.2）。
    Section,
    /// キャラ（`characters.md` の H1）。
    Character,
    /// モード（`### <mode_id>`）。
    Mode,
    /// 用語（`words.md` の H1）。
    Word,
    /// アイデア（`ideas.md` の H1）。
    Idea,
    /// 役割（`prompts/**/*.md` の H1）。
    Role,
    /// プロンプトのセクション（H2）。
    PromptSection,
}

/// シンボル 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentSymbol {
    /// 名前（ID・名前・節番号）。
    pub name: String,
    /// 詳細（タイトル・表示名）。無ければ空。
    pub detail: String,
    /// 種別。
    pub kind: SymbolKind,
    /// シンボル全体（見出しから次の見出しの手前まで）の byte 範囲。
    pub range: Range<usize>,
    /// 名前の部分（選択・ジャンプ先）の byte 範囲。
    pub selection_range: Range<usize>,
    /// 子。
    pub children: Vec<DocumentSymbol>,
}

/// 文書の構造ツリー（§3.4）。
pub fn all(document: &Document) -> Vec<DocumentSymbol> {
    let text = document.text.as_str();
    let lines = Lines::new(text);
    match document.format {
        DocumentFormat::Contents => contents_symbols(&contents::parse(text), &lines),
        DocumentFormat::PlotEpisode => episode_symbols(&plot_episode::parse(text), &lines),
        DocumentFormat::PlotOutline => outline_symbols(&plot_outline::parse(text), &lines),
        DocumentFormat::Characters => character_symbols(&characters_md::parse(text), &lines),
        DocumentFormat::Words => word_symbols(&words_md::parse(text), &lines),
        DocumentFormat::Ideas => idea_symbols(&ideas_md::parse(text), &lines),
        DocumentFormat::Prompt | DocumentFormat::CustomPrompt | DocumentFormat::SharedPrompt => {
            prompt_symbols(&prompt_md::parse(text), &lines)
        }
        _ => Vec::new(),
    }
}

/// 行 index → byte 位置の変換。
struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            starts: project::line_starts_of(text),
        }
    }

    fn start(&self, line: usize) -> usize {
        self.starts.get(line).copied().unwrap_or(self.text.len())
    }

    /// 行の終わり（改行の手前）。
    fn end(&self, line: usize) -> usize {
        match self.starts.get(line + 1) {
            Some(next) => next - 1,
            None => self.text.len(),
        }
    }

    /// 行範囲 `span`（終わりを含まない）の byte 範囲。末尾の空行は含めない。
    fn span(&self, span: &Range<usize>) -> Range<usize> {
        let start = self.start(span.start);
        let mut last = span.end.saturating_sub(1).max(span.start);
        while last > span.start
            && self.text[self.start(last)..self.end(last)]
                .trim()
                .is_empty()
        {
            last -= 1;
        }
        start..self.end(last).max(start)
    }
}

/// 見出し行（`@id title`）の名前・詳細・選択範囲。
fn header(indent: usize, id_text: &str, title: &str, at: usize) -> (String, String, Range<usize>) {
    let start = at + indent;
    (
        id_text.to_string(),
        title.to_string(),
        start..start + 1 + id_text.len(),
    )
}

fn contents_symbols(parsed: &contents::Contents<'_>, lines: &Lines<'_>) -> Vec<DocumentSymbol> {
    let mut out: Vec<DocumentSymbol> = Vec::new();
    let mut open: Option<usize> = None;
    for section in &parsed.sections {
        let Some(header_line) = section.scene_header else {
            continue;
        };
        if open != Some(header_line) {
            let Some(contents::Line::Scene(found)) = parsed.lines.get(header_line) else {
                continue;
            };
            let (name, detail, selection) = header(
                found.indent,
                found.id_text,
                found.title,
                lines.start(header_line),
            );
            out.push(DocumentSymbol {
                name,
                detail,
                kind: SymbolKind::Scene,
                range: lines.span(&(header_line..header_line + 1)),
                selection_range: selection,
                children: Vec::new(),
            });
            open = Some(header_line);
        }
        if let Some(scene) = out.last_mut() {
            let range = lines.span(&section.span);
            scene.range.end = scene.range.end.max(range.end);
            scene.children.push(section_symbol(section.number, range));
        }
    }
    out
}

fn section_symbol(number: u16, range: Range<usize>) -> DocumentSymbol {
    DocumentSymbol {
        name: number.to_string(),
        detail: String::new(),
        kind: SymbolKind::Section,
        selection_range: range.start..range.start,
        range,
        children: Vec::new(),
    }
}

fn episode_symbols(
    parsed: &plot_episode::EpisodePlot<'_>,
    lines: &Lines<'_>,
) -> Vec<DocumentSymbol> {
    let mut out = Vec::new();
    for scene in &parsed.scenes {
        let Some(plot_episode::Line::Scene(found)) = parsed.lines.get(scene.header) else {
            continue;
        };
        let (name, detail, selection) = header(
            found.indent,
            found.id_text,
            found.title,
            lines.start(scene.header),
        );
        let children = scene
            .sections
            .iter()
            .map(|section| section_symbol(section.number, lines.span(&section.span)))
            .collect();
        out.push(DocumentSymbol {
            name,
            detail,
            kind: SymbolKind::Scene,
            range: lines.span(&scene.span),
            selection_range: selection,
            children,
        });
    }
    out
}

fn outline_symbols(parsed: &plot_outline::Outline<'_>, lines: &Lines<'_>) -> Vec<DocumentSymbol> {
    let mut out = Vec::new();
    for block in &parsed.blocks {
        let Some(found) = parsed.header(block.header) else {
            continue;
        };
        let start = lines.start(block.header) + found.sigil_at;
        let kind = match block.depth {
            1 => SymbolKind::Chapter,
            2 => SymbolKind::Episode,
            _ => SymbolKind::Scene,
        };
        out.push(DocumentSymbol {
            name: found.id_text.to_string(),
            detail: found.title.to_string(),
            kind,
            range: lines.span(&block.span),
            selection_range: start..start + 1 + found.id_text.len(),
            children: Vec::new(),
        });
    }
    out
}

fn character_symbols(
    parsed: &characters_md::Characters<'_>,
    lines: &Lines<'_>,
) -> Vec<DocumentSymbol> {
    let mut out = Vec::new();
    for found in &parsed.characters {
        let at = lines.start(found.heading) + found.name_at;
        let mut children = Vec::new();
        for section in &found.sections {
            for mode in &section.modes {
                let id_at = lines.start(mode.heading) + mode.id_at;
                children.push(DocumentSymbol {
                    name: mode.id.to_string(),
                    detail: mode.label.to_string(),
                    kind: SymbolKind::Mode,
                    range: lines.span(&mode.span),
                    selection_range: id_at..id_at + mode.id.len(),
                    children: Vec::new(),
                });
            }
        }
        out.push(DocumentSymbol {
            name: found.name.to_string(),
            detail: String::new(),
            kind: SymbolKind::Character,
            range: lines.span(&found.span),
            selection_range: at..at + found.name.len(),
            children,
        });
    }
    out
}

fn word_symbols(parsed: &words_md::Words<'_>, lines: &Lines<'_>) -> Vec<DocumentSymbol> {
    parsed
        .words
        .iter()
        .map(|found| {
            let at = lines.start(found.heading) + found.term_at;
            DocumentSymbol {
                name: found.term.to_string(),
                detail: String::new(),
                kind: SymbolKind::Word,
                range: lines.span(&found.span),
                selection_range: at..at + found.term.len(),
                children: Vec::new(),
            }
        })
        .collect()
}

fn idea_symbols(parsed: &ideas_md::Ideas<'_>, lines: &Lines<'_>) -> Vec<DocumentSymbol> {
    parsed
        .ideas
        .iter()
        .map(|found| {
            let at = lines.start(found.heading) + found.id_at;
            DocumentSymbol {
                name: found.id.to_string(),
                detail: found.title.to_string(),
                kind: SymbolKind::Idea,
                range: lines.span(&found.span),
                selection_range: at..at + found.id.len(),
                children: Vec::new(),
            }
        })
        .collect()
}

fn prompt_symbols(parsed: &prompt_md::Prompt<'_>, lines: &Lines<'_>) -> Vec<DocumentSymbol> {
    let children: Vec<DocumentSymbol> = parsed
        .sections
        .iter()
        .map(|section| {
            let start = lines.start(section.heading);
            let raw = &lines.text[start..lines.end(section.heading)];
            let at = start + raw.find(section.key_text).unwrap_or(0);
            DocumentSymbol {
                name: section.key_text.to_string(),
                detail: String::new(),
                kind: SymbolKind::PromptSection,
                range: lines.span(&section.span),
                selection_range: at..at + section.key_text.len(),
                children: Vec::new(),
            }
        })
        .collect();
    let Some(role) = &parsed.role else {
        return children;
    };
    let at = lines.start(role.heading) + role.name_at;
    let end = children
        .iter()
        .map(|child| child.range.end)
        .max()
        .unwrap_or(lines.end(role.heading));
    vec![DocumentSymbol {
        name: role.name.to_string(),
        detail: String::new(),
        kind: SymbolKind::Role,
        range: lines.start(role.heading)..end,
        selection_range: at..at + role.name.len(),
        children,
    }]
}
