//! プロジェクトの集約（§3.4 の `(&Project, &Document, Position)` の `&Project`）。
//!
//! # `model::Project` との違い
//!
//! [`crate::core::model::Project`] は **`project.json` 1 ファイル**（§4.5）の中身。ここの
//! [`Project`] は §3.4 の 11 機能と `diagnostics::document` が受け取る**プロジェクト全体**で、
//! 設定（`model::Project`）・開いている文書（[`Document`]）・構築済みの辞書を持つ。
//! `model/` に置かないのは、辞書（`resolve`）と構文木（`syntax`）を使うから
//! （`model` は `syntax` を使えない、`PLANS.md` §9.2 B-29）。
//!
//! # 持つもの（`PLANS.md` §9.2 B-76）
//!
//! - **文書は所有型**（`path → Document`）。`core/syntax/*` の木は原文を借用する
//!   （`Characters<'a>` 等）ので、木を持つと自己参照になる。木は**機能を呼ぶたびにパースし直す**
//!   （M2 のパーサは 1 万行でも数 ms。打鍵ごとの再解析は §6.7 の増分のフレームが担う）。
//! - **辞書だけは持ち続ける**（§6.1.2「`characters.md` / `words.md` / `materials/index.json` の
//!   いずれかが変化したときのみ再構築する」）。[`Project::insert`] がこの 3 ファイルのときだけ
//!   作り直す。
//! - **core は I/O をしない**（§3.2）。文書の読み込みと保存は呼び出し側（M5 のストレージ）。
//!   ここは渡された文書を覚えるだけ。
//!
//! # 参照の走査（11 機能の共通部品）
//!
//! `'` の参照（§6.2）は「全形式の本文部」（§5.0.2 の表）に現れる。どこを本文部とみなすかは
//! [`scan_ranges`] が決め（`PLANS.md` §9.2 B-77）、[`Project::ref_sites`] がその範囲だけを
//! 辞書で走査する。位置は**ファイル単位の byte**（B-1・B-56）で返す。

use std::collections::BTreeMap;
use std::ops::Range;

use crate::core::diagnostics::Diagnostic;
use crate::core::id::{Digits, SceneId};
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::model::project::Project as Settings;
use crate::core::resolve::dictionary::{self, Dictionary, EntryId, RefToken, SourceFile};
use crate::core::syntax::{
    characters_md, contents, materials_index, plot_episode, plot_outline, words_md,
};
use crate::core::text::lexer::{self, LineKind, TextFormat};
use crate::core::text::position::LineTable;

/// `plots/all.txt`（§4.1 の全体プロット）。
pub const ALL_PLOT_PATH: &str = "plots/all.txt";

/// プロジェクトの集約（§3.4）。
#[derive(Debug)]
pub struct Project {
    settings: Settings,
    files: BTreeMap<String, Document>,
    dictionary: Dictionary,
    dictionary_diagnostics: Vec<dictionary::FileDiagnostic>,
}

impl Default for Project {
    fn default() -> Self {
        Self::new(Settings::default())
    }
}

impl Project {
    /// 文書の無いプロジェクト（辞書は空）。
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            files: BTreeMap::new(),
            dictionary: Dictionary::empty(),
            dictionary_diagnostics: Vec::new(),
        }
    }

    /// 文書をまとめて渡して作る（辞書は最後に 1 回だけ作る）。
    pub fn with_documents(
        settings: Settings,
        documents: impl IntoIterator<Item = Document>,
    ) -> Self {
        let mut project = Self::new(settings);
        for document in documents {
            project.files.insert(document.path.clone(), document);
        }
        project.rebuild_dictionary();
        project
    }

    /// 文書を足す／置き換える。辞書の供給元（§6.1.1 の 3 ファイル）なら辞書を作り直す。
    pub fn insert(&mut self, document: Document) {
        let rebuild = is_dictionary_source(&document.path);
        self.files.insert(document.path.clone(), document);
        if rebuild {
            self.rebuild_dictionary();
        }
    }

    /// 文書を外す。辞書の供給元なら辞書を作り直す。
    pub fn remove(&mut self, path: &str) -> Option<Document> {
        let removed = self.files.remove(path);
        if removed.is_some() && is_dictionary_source(path) {
            self.rebuild_dictionary();
        }
        removed
    }

    /// 設定（`project.json`）を差し替える。
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// 設定（`project.json`、§4.5）。
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// パス（プロジェクトルートからの相対、`/` 区切り）で文書を引く。
    pub fn document(&self, path: &str) -> Option<&Document> {
        self.files.get(path)
    }

    /// 全文書（パスの昇順）。
    pub fn documents(&self) -> impl Iterator<Item = &Document> {
        self.files.values()
    }

    /// 文書の中身。無ければ空文字列（§4.3-4「無いファイルは既定値で続行」）。
    pub fn text(&self, path: &str) -> &str {
        self.files.get(path).map_or("", |found| found.text.as_str())
    }

    /// 構築済みの辞書（§6.1）。
    pub fn dictionary(&self) -> &Dictionary {
        &self.dictionary
    }

    /// 辞書の構築で出た診断（E028 / W027）のうち、`file` に載るもの。
    pub fn dictionary_diagnostics(&self, file: SourceFile) -> Vec<Diagnostic> {
        self.dictionary_diagnostics
            .iter()
            .filter(|found| found.file == file)
            .map(|found| found.diagnostic.clone())
            .collect()
    }

    /// SceneId の表示桁数（§4.5 `structure.*_digits`）。
    pub fn digits(&self) -> Digits {
        let structure = &self.settings.structure;
        Digits {
            chapter: structure.chapter_digits,
            episode: structure.episode_digits,
            scene: structure.scene_digits,
            section: structure.section_digits,
        }
    }

    /// `contents/AA-BB.txt`（§4.1）。
    pub fn contents_path(&self, chapter: u16, episode: u16) -> String {
        format!("contents/{}.txt", self.episode_stem(chapter, episode))
    }

    /// `plots/AA-BB.txt`（§4.1 の話プロット）。
    pub fn episode_plot_path(&self, chapter: u16, episode: u16) -> String {
        format!("plots/{}.txt", self.episode_stem(chapter, episode))
    }

    /// `plots/AA.txt`（§4.1 の章プロット）。
    pub fn chapter_plot_path(&self, chapter: u16) -> String {
        let digits = self.digits();
        format!("plots/{}.txt", pad(chapter, digits.chapter))
    }

    fn episode_stem(&self, chapter: u16, episode: u16) -> String {
        let digits = self.digits();
        format!(
            "{}-{}",
            pad(chapter, digits.chapter),
            pad(episode, digits.episode)
        )
    }

    /// C 形式のパーサの設定（§5.1.2 `structure.allow_chapter_zero`）。
    pub fn contents_options(&self) -> contents::Options {
        contents::Options {
            allow_chapter_zero: self.settings.structure.allow_chapter_zero,
        }
    }

    /// A 形式のパーサの設定。`plots/all.txt` は 1 階層、`plots/AA.txt` は 2 階層（§5.3.2）。
    pub fn outline_options(&self, path: &str) -> plot_outline::Options {
        let depth = if path == ALL_PLOT_PATH { 1 } else { 2 };
        plot_outline::Options {
            depth: Some(depth),
            allow_chapter_zero: self.settings.structure.allow_chapter_zero,
        }
    }

    /// `characters.md` の木。
    pub fn characters(&self) -> characters_md::Characters<'_> {
        characters_md::parse(self.text(SourceFile::Characters.path()))
    }

    /// `words.md` の木。
    pub fn words(&self) -> words_md::Words<'_> {
        words_md::parse(self.text(SourceFile::Words.path()))
    }

    /// `materials/index.json` の木（無ければ空の索引、§4.3-4）。
    pub fn materials(&self) -> materials_index::Index<'_> {
        materials_index::parse(self.text(SourceFile::MaterialsIndex.path()))
    }

    /// 話プロット（B 形式）の木。文書が無ければ `None`。
    pub fn episode_plot(
        &self,
        chapter: u16,
        episode: u16,
    ) -> Option<plot_episode::EpisodePlot<'_>> {
        let path = self.episode_plot_path(chapter, episode);
        let found = self.files.get(&path)?;
        Some(plot_episode::parse_with(
            &found.text,
            &self.contents_options(),
        ))
    }

    /// 本文（C 形式）の木。文書が無ければ `None`。
    pub fn episode_contents(&self, chapter: u16, episode: u16) -> Option<contents::Contents<'_>> {
        let path = self.contents_path(chapter, episode);
        let found = self.files.get(&path)?;
        Some(contents::parse_with(&found.text, &self.contents_options()))
    }

    /// 章プロット（`plots/AA.txt`）の木。文書が無ければ `None`。
    pub fn chapter_plot(&self, chapter: u16) -> Option<plot_outline::Outline<'_>> {
        let path = self.chapter_plot_path(chapter);
        let found = self.files.get(&path)?;
        Some(plot_outline::parse_with(
            &found.text,
            &self.outline_options(&path),
        ))
    }

    /// 全体プロット（`plots/all.txt`）の木。文書が無ければ `None`。
    pub fn all_plot(&self) -> Option<plot_outline::Outline<'_>> {
        let found = self.files.get(ALL_PLOT_PATH)?;
        Some(plot_outline::parse_with(
            &found.text,
            &self.outline_options(ALL_PLOT_PATH),
        ))
    }

    /// 文書の中の `'` 参照（§6.2）を全部解決する。位置は**ファイル単位**。
    ///
    /// 走査するのは [`scan_ranges`] の範囲だけ（それ以外は空白に置き換えてから走査するので、
    /// 参照が範囲をまたぐことはなく、位置は元の行と同じ）。
    pub fn ref_sites(&self, document: &Document) -> Vec<RefSite> {
        let Some(format) = document.format.text_format() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for line in lines_of(&document.text) {
            let keep = scan_ranges(line.text, format);
            if keep.is_empty() {
                continue;
            }
            let masked = mask(line.text, &keep);
            let scanned = dictionary::scan_line(&masked, line.start, &self.dictionary);
            for token in scanned.tokens {
                out.push(RefSite {
                    line: line.index,
                    line_start: line.start,
                    token,
                });
            }
        }
        out
    }

    /// 本文（C 形式）の話者注釈（§6.2.5）。C 形式以外は空。
    pub fn speaker_sites(&self, document: &Document) -> Vec<SpeakerSite> {
        if document.format != DocumentFormat::Contents {
            return Vec::new();
        }
        let parsed = contents::parse_with(&document.text, &self.contents_options());
        let starts = line_starts_of(&document.text);
        let mut out = Vec::new();
        for (index, line) in parsed.lines.iter().enumerate() {
            let contents::Line::Text(text) = line else {
                continue;
            };
            let Some(speaker) = &text.speaker else {
                continue;
            };
            let start = starts.get(index).copied().unwrap_or(0);
            let name_at = speaker.sigil_at + 1;
            let mut modes = Vec::new();
            let mut at = name_at + speaker.name.len() + 1;
            for mode in &speaker.modes {
                modes.push((mode.to_string(), start + at..start + at + mode.len()));
                at += mode.len() + 1;
            }
            out.push(SpeakerSite {
                line: index,
                line_start: start,
                sigil: start + speaker.sigil_at,
                name_range: start + name_at..start + name_at + speaker.name.len(),
                name: speaker.name.to_string(),
                modes,
                entry: dictionary::resolve_speaker(speaker, &self.dictionary),
            });
        }
        out
    }

    /// C 形式・B 形式の文書の `line` 行目が属する節（§5.2.2 / §5.4.1）。
    ///
    /// 見出しより前の行や、ID が壊れた見出しの下の行は `None`。
    pub fn scene_at(&self, document: &Document, line: usize) -> Option<SceneId> {
        match document.format {
            DocumentFormat::Contents => {
                let parsed = contents::parse_with(&document.text, &self.contents_options());
                let section = parsed
                    .sections
                    .iter()
                    .find(|found| found.span.contains(&line))?;
                let scene = section.scene?;
                Some(SceneId {
                    section: section.number,
                    ..scene
                })
            }
            DocumentFormat::PlotEpisode => {
                let parsed = plot_episode::parse_with(&document.text, &self.contents_options());
                let scene = parsed
                    .scenes
                    .iter()
                    .find(|found| found.span.contains(&line))?;
                let id = scene.id?;
                let number = scene
                    .sections
                    .iter()
                    .find(|found| found.span.contains(&line))
                    .map_or(id.section, |found| found.number);
                Some(SceneId {
                    section: number,
                    ..id
                })
            }
            _ => None,
        }
    }

    fn rebuild_dictionary(&mut self) {
        let characters = characters_md::parse(self.text(SourceFile::Characters.path()));
        let words = words_md::parse(self.text(SourceFile::Words.path()));
        let index = materials_index::parse(self.text(SourceFile::MaterialsIndex.path()));
        let built = Dictionary::from_sources(&characters, &words, &index);
        self.dictionary = built.dictionary;
        self.dictionary_diagnostics = built.diagnostics;
    }
}

/// 文書の中の `'` 参照 1 件。
///
/// [`RefToken`] の範囲は**行内相対**（M3 の流儀）なので、ファイル単位にするときは
/// [`RefSite::range`] などを使う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefSite {
    /// 行番号（0 始まり）。
    pub line: usize,
    /// 行の先頭のファイル内 byte 位置。
    pub line_start: usize,
    /// 辞書の走査結果（範囲は行内相対）。
    pub token: RefToken,
}

impl RefSite {
    /// トークン全体（`'` から修飾子の終わりまで）。ファイル単位。
    pub fn range(&self) -> Range<usize> {
        shift(&self.token.range, self.line_start)
    }

    /// キーの部分（`'` と `{}` を除く）。ファイル単位。
    pub fn key_range(&self) -> Range<usize> {
        shift(&self.token.key_range, self.line_start)
    }

    /// 修飾子（`:` を含む）。ファイル単位。
    pub fn modifier_range(&self) -> Option<Range<usize>> {
        self.token
            .modifier
            .as_ref()
            .map(|found| shift(&found.range, self.line_start))
    }

    /// 解決できたエントリ。
    pub fn entry(&self) -> Option<EntryId> {
        self.token.resolved
    }
}

/// 話者注釈 1 件（C 形式の行末 `@名前[:モード]`、§6.2.5）。範囲はファイル単位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerSite {
    /// 行番号（0 始まり）。
    pub line: usize,
    /// 行の先頭のファイル内 byte 位置。
    pub line_start: usize,
    /// `@` の位置。
    pub sigil: usize,
    /// キャラ名の範囲。
    pub name_range: Range<usize>,
    /// キャラ名（書かれたまま）。
    pub name: String,
    /// モード式（`:B+D` の `B` と `D`）と範囲。
    pub modes: Vec<(String, Range<usize>)>,
    /// 解決できた `Character` エントリ（完全一致、B-61）。
    pub entry: Option<EntryId>,
}

/// 辞書の供給元（§6.1.1 の 3 ファイル）か。
pub fn is_dictionary_source(path: &str) -> bool {
    [
        SourceFile::Characters,
        SourceFile::Words,
        SourceFile::MaterialsIndex,
    ]
    .iter()
    .any(|file| file.path() == path)
}

/// `contents/01-04.txt` / `plots/01-04.txt` → `(1, 4)`。話のファイルでなければ `None`。
pub fn episode_of_path(path: &str) -> Option<(u16, u16)> {
    let stem = path
        .strip_prefix("contents/")
        .or_else(|| path.strip_prefix("plots/"))?
        .strip_suffix(".txt")?;
    let (chapter, episode) = stem.split_once('-')?;
    Some((number(chapter)?, number(episode)?))
}

/// `plots/01.txt` → `1`。章プロットでなければ `None`。
pub fn chapter_of_path(path: &str) -> Option<u16> {
    let stem = path.strip_prefix("plots/")?.strip_suffix(".txt")?;
    number(stem)
}

fn number(text: &str) -> Option<u16> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn pad(value: u16, digits: u8) -> String {
    format!("{value:0width$}", width = usize::from(digits))
}

/// 1 行（`text` は改行を含まない）。
#[derive(Debug, Clone, Copy)]
pub struct LineRef<'a> {
    /// 行番号（0 始まり）。
    pub index: usize,
    /// 行の先頭のファイル内 byte 位置。
    pub start: usize,
    /// 行の中身（LF を含まない）。
    pub text: &'a str,
}

/// LF で区切った行（最後の行が空でも 1 行として返す。[`LineTable`] と同じ数え方）。
pub fn lines_of(text: &str) -> impl Iterator<Item = LineRef<'_>> {
    let mut start = 0usize;
    text.split('\n').enumerate().map(move |(index, line)| {
        let found = LineRef {
            index,
            start,
            text: line,
        };
        start += line.len() + 1;
        found
    })
}

/// 各行の先頭のファイル内 byte 位置。
pub fn line_starts_of(text: &str) -> Vec<usize> {
    let table = LineTable::new(text);
    (0..table.line_count())
        .map(|line| table.line_start(line).unwrap_or(0))
        .collect()
}

/// `'` を走査してよい範囲（**行内相対**）。§5.0.2 の「全形式の本文部」（`PLANS.md` §9.2 B-77）。
///
/// | 行 | 走査する範囲 |
/// |---|---|
/// | 見出し（`@`）・節区切り（`;`）・行頭コメント | なし |
/// | フィールド行（§5.0.5） | 値（行末コメントの手前まで。B-36 の値に残るコメントは除く） |
/// | 継続行 | 字下げの後ろから行末コメントの手前まで |
/// | 通常のテキスト行 | 行頭から行末コメントの手前まで。`*.md` の `#` 見出し行はなし（項目名そのもの） |
///
/// どの行でも行内コメント `/* … */` の中は除く。C 形式の行末の話者注釈は残るが、`'` を
/// 含まないので走査しても何も出ない（話者は [`Project::speaker_sites`] が別に扱う）。
pub fn scan_ranges(raw: &str, format: TextFormat) -> Vec<Range<usize>> {
    let base = match lexer::classify(raw, format) {
        LineKind::Structure(_) | LineKind::SectionBreak | LineKind::Comment { .. } => None,
        LineKind::Field(field) => {
            let value = lexer::strip_line_comment(field.value);
            Some(field.value_at..field.value_at + value.len())
        }
        LineKind::Continuation { text, .. } => {
            let start = raw.len() - text.len();
            Some(start..start + lexer::strip_line_comment(text).len())
        }
        LineKind::IllustrationMemo(_) => None,
        LineKind::Text(text) => {
            if format == TextFormat::Markdown && lexer::after_indent(text).starts_with('#') {
                None
            } else {
                Some(0..lexer::strip_line_comment(text).len())
            }
        }
    };
    let Some(base) = base else {
        return Vec::new();
    };
    match lexer::find_inline_comment(raw) {
        Some(comment) if comment.start < base.end && comment.end > base.start => {
            let mut out = Vec::new();
            if base.start < comment.start {
                out.push(base.start..comment.start);
            }
            if comment.end < base.end {
                out.push(comment.end..base.end);
            }
            out
        }
        _ => vec![base],
    }
}

/// `keep` の外側を ASCII の空白に置き換えた行（byte 長は同じ）。
///
/// 空白は §6.2.1 の区切り文字なので、参照が `keep` の外へ伸びることはない。範囲は文字境界に
/// あるので、結果は常に正しい UTF-8 になる。
pub fn mask(raw: &str, keep: &[Range<usize>]) -> String {
    let mut bytes = raw.as_bytes().to_vec();
    for (at, byte) in bytes.iter_mut().enumerate() {
        if !keep.iter().any(|range| range.contains(&at)) {
            *byte = b' ';
        }
    }
    String::from_utf8(bytes).unwrap_or_default()
}

fn shift(range: &Range<usize>, by: usize) -> Range<usize> {
    range.start + by..range.end + by
}
