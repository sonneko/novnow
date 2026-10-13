//! 節単位の編集（§8.1）と、節の並び（§9.3 の文脈ビュー・§9.8.2 の構造移動）。
//!
//! LSP には対応しない補助（UI の節エディタと文脈ビューのための純粋関数。`PLANS.md` §9.2 B-112）。
//!
//! - **節の並び**: 本文（`contents/AA-BB.txt`）の節を、ファイル名の順 → ファイルの中の順に並べたもの。
//!   場面見出しの ID が壊れた節（E002 / E020）と、最初の場面見出しより前の行は並びに入らない。
//! - **節の本文**: 節の内容の行（`contents::Section::content`。見出し行と区切り `;` と、末尾の空行を
//!   含まない）を LF でつないだもの。節エディタはこれだけを編集する（末尾の空行は書き戻しでも残る）。
//! - **書き戻し**: 編集した本文で節の行を置き換えたファイル全体の文字列を返す（§8.1「節の移動で
//!   即座に文書へ反映」）。本文に `;` の行を書けば、書き戻した後は節が増える（並びを作り直す）。
//!
//! 文字数（§9.6.1）は**空白を除いた文字数**（[`count_chars`]）。

use std::ops::Range;

use crate::core::features::project::episode_of_path;
use crate::core::features::Project;
use crate::core::id::SceneId;
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::syntax::contents;

/// 並びの中の節 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionRef {
    /// 本文のパス（`contents/01-04.txt`）。
    pub path: String,
    /// 節の構造 ID（4 階層）。
    pub id: SceneId,
    /// 節の内容の行（ファイルの行 index。見出し行・区切り `;`・末尾の空行を含まない）。空の節なら空範囲。
    pub lines: Range<usize>,
    /// 節の行（末尾の空行を含む。区切り `;` の直前まで）。
    pub span: Range<usize>,
    /// この節を支配する場面見出しの行 index。
    pub header: Option<usize>,
    /// 場面のタイトル（見出しの ID の後ろ。無ければ空）。
    pub scene_title: String,
}

impl SectionRef {
    /// 同じ場面か。
    pub fn same_scene(&self, other: &Self) -> bool {
        self.path == other.path
            && SceneId {
                section: 0,
                ..self.id
            } == SceneId {
                section: 0,
                ..other.id
            }
    }
}

/// プロジェクトの全節（ファイル名の順 → ファイルの中の順）。
pub fn all(project: &Project) -> Vec<SectionRef> {
    let mut out = Vec::new();
    for document in project.documents() {
        if document.format == DocumentFormat::Contents && episode_of_path(&document.path).is_some()
        {
            out.extend(of_document(document, project));
        }
    }
    out
}

/// 1 つの本文ファイルの節（ファイルの中の順）。
pub fn of_document(document: &Document, project: &Project) -> Vec<SectionRef> {
    let parsed = contents::parse_with(&document.text, &project.contents_options());
    let lines: Vec<&str> = document.text.split('\n').collect();
    parsed
        .sections
        .iter()
        .filter_map(|section| {
            let scene = section.scene?;
            Some(SectionRef {
                path: document.path.clone(),
                id: SceneId {
                    section: section.number,
                    ..scene
                },
                lines: section.content.clone(),
                span: section.span.clone(),
                header: section.scene_header,
                scene_title: section
                    .scene_header
                    .and_then(|line| lines.get(line))
                    .map(|line| header_title(line))
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// 見出し行（`@01-04-02 図書室の攻防`）のタイトル部分。
fn header_title(line: &str) -> String {
    let rest = line.trim_start().strip_prefix('@').unwrap_or(line);
    match rest.find(char::is_whitespace) {
        Some(at) => rest[at..].trim().to_owned(),
        None => String::new(),
    }
}

/// 節の本文（節の行を LF でつないだもの）。
pub fn text_of(text: &str, section: &SectionRef) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let end = section.lines.end.min(lines.len());
    let start = section.lines.start.min(end);
    lines[start..end].join("\n")
}

/// 節の本文を `body` で置き換えたファイル全体。
///
/// 空の節（行が無い）に本文を書くと、その位置（次の `;` の直前）に行が入る。`body` が空なら
/// 節の行を消す（節は空になるが、区切りは残るので節の数は変わらない）。
pub fn splice(text: &str, section: &SectionRef, body: &str) -> String {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let end = section.lines.end.min(lines.len());
    let start = section.lines.start.min(end);
    let replacement: Vec<&str> = if body.is_empty() {
        Vec::new()
    } else {
        body.split('\n').collect()
    };
    lines.splice(start..end, replacement);
    lines.join("\n")
}

/// 節の直後に空の節を作ったファイル全体と、新しい節の行 index（§9.4.1 の `Ctrl+;`・`Ctrl+Enter`）。
///
/// 節の行（末尾の空行を含む）の直後に区切り `;` を 1 行入れる。新しい節はその `;` の次の行から始まる。
/// ファイルが改行で終わるときの最後の空の「行」（改行の後ろ）は節の外に残す。
pub fn insert_after(text: &str, section: &SectionRef) -> (String, usize) {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let mut at = section.span.end.min(lines.len());
    if at == lines.len() && text.ends_with('\n') {
        at -= 1;
    }
    let at = at.max(section.lines.end.min(lines.len()));
    lines.insert(at, ";");
    (lines.join("\n"), at + 1)
}

/// 空白を除いた文字数（§9.6.1。コメント行・アノテーションも数える）。
pub fn count_chars(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

/// 並び `list` の中で `id` の節の位置。無ければ、同じファイルの同じ場面・最も近い節。
pub fn position_of(list: &[SectionRef], path: &str, id: SceneId) -> Option<usize> {
    if let Some(found) = list
        .iter()
        .position(|item| item.path == path && item.id == id)
    {
        return Some(found);
    }
    let scene = SceneId { section: 0, ..id };
    let mut best: Option<usize> = None;
    for (index, item) in list.iter().enumerate() {
        if item.path != path {
            continue;
        }
        if (SceneId {
            section: 0,
            ..item.id
        }) == scene
            && item.id.section <= id.section.max(1)
        {
            best = Some(index);
        }
    }
    best.or_else(|| list.iter().position(|item| item.path == path))
}

/// 並び `list` の中で、`index` の節の前後の場面の最初の節（§9.8.2 の `Alt+←` / `Alt+→`）。
pub fn scene_step(list: &[SectionRef], index: usize, forward: bool) -> Option<usize> {
    let current = list.get(index)?;
    if forward {
        list.iter()
            .enumerate()
            .skip(index + 1)
            .find(|(_, item)| !item.same_scene(current))
            .map(|(at, _)| at)
    } else {
        // 今の場面の先頭へ戻り、そのさらに前の場面の先頭へ
        let mut at = index;
        while at > 0 && list[at - 1].same_scene(current) {
            at -= 1;
        }
        if at == 0 {
            return None;
        }
        let previous = &list[at - 1];
        let mut start = at - 1;
        while start > 0 && list[start - 1].same_scene(previous) {
            start -= 1;
        }
        Some(start)
    }
}

/// 並び `list` の中で、`index` の節の前後のファイルの最初の節（§9.8.2 の `Ctrl+Alt+←` / `→`）。
pub fn file_step(list: &[SectionRef], index: usize, forward: bool) -> Option<usize> {
    let current = list.get(index)?;
    if forward {
        list.iter()
            .enumerate()
            .skip(index + 1)
            .find(|(_, item)| item.path != current.path)
            .map(|(at, _)| at)
    } else {
        let first_of_current = list.iter().position(|item| item.path == current.path)?;
        if first_of_current == 0 {
            return None;
        }
        let previous = &list[first_of_current - 1].path;
        list.iter().position(|item| &item.path == previous)
    }
}

/// 構造 ID（1〜4 階層。`0104` のような区切り無しも受ける、§9.7.2）に最も近い節の位置。
///
/// 階層が足りなければ、その範囲の最初の節。
pub fn jump(list: &[SectionRef], query: &str) -> Option<usize> {
    let parts = parse_query(query)?;
    list.iter().position(|item| {
        let id = [
            item.id.chapter,
            item.id.episode,
            item.id.scene,
            item.id.section,
        ];
        parts.iter().zip(id.iter()).all(|(want, have)| want == have)
    })
}

/// `@01-04-02`・`01-04-02`・`010402`・`1-4-2` を階層の列にする。区切り無しは 2 桁ずつ。
fn parse_query(query: &str) -> Option<Vec<u16>> {
    let text = query.trim().trim_start_matches('@');
    if text.is_empty() {
        return None;
    }
    let parts: Vec<u16> = if text.contains('-') {
        text.split('-')
            .map(|part| part.parse::<u16>().ok())
            .collect::<Option<Vec<_>>>()?
    } else {
        if !text.bytes().all(|byte| byte.is_ascii_digit()) || !text.len().is_multiple_of(2) {
            return None;
        }
        text.as_bytes()
            .chunks(2)
            .map(|pair| std::str::from_utf8(pair).ok()?.parse::<u16>().ok())
            .collect::<Option<Vec<_>>>()?
    };
    (1..=4).contains(&parts.len()).then_some(parts)
}
