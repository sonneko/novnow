//! プロジェクトの集約（§3.4 の `(&Project, &Document, Position)` の `&Project`）。
//!
//! # `model::Project` との違い
//!
//! [`crate::core::model::Project`] は **`project.toml` 1 ファイル**（§4.5）の中身。ここの
//! [`Project`] は §3.4 の 11 機能と `diagnostics::document` が受け取る**プロジェクト全体**で、
//! 設定（`model::Project`）・開いている文書（[`Document`]）・構築済みの辞書を持つ。
//! `model/` に置かないのは、辞書（`resolve`）と構文木（`syntax`）を使うから
//! （`model` は `syntax` を使えない、`docs/history.md` B-29）。
//!
//! # 持つもの（`docs/history.md` B-76）
//!
//! - **文書は所有型**（`path → Document`）。`core/syntax/*` の木は原文を借用する
//!   （`Characters<'a>` 等）ので、木を持つと自己参照になる。木は**機能を呼ぶたびにパースし直す**
//!   （M2 のパーサは 1 万行でも数 ms。打鍵ごとの再解析は §6.7 の増分のフレームが担う）。
//! - **辞書だけは持ち続ける**（§6.1.2「`characters.md` / `words.md` / `materials/index.json` の
//!   いずれかが変化したときのみ再構築する」）。[`Project::insert`] がこの 3 ファイルのときだけ
//!   作り直す。
//! - **core は I/O をしない**（§3.2）。文書の読み込みと保存は呼び出し側（M5 のストレージ）。
//!   ここは渡された文書を覚えるだけ。
//! - **設定は `project.toml` の文書から取る**（M5 の設定読み込み、`docs/history.md` B-89）。
//!   `project.toml` を [`Project::insert`] / [`Project::with_documents`] で渡すと
//!   [`project_toml::parse`] の結果で設定を置き換える（JSON として読めなければ既定値、§4.3-4）。
//!   E025 / E027 はその文書の診断（`diagnostics::document`）に出る。
//!
//! # 参照の走査（11 機能の共通部品）
//!
//! `'` の参照（§6.2）は「全形式の本文部」（§5.0.2 の表）に現れる。どこを本文部とみなすかは
//! [`scan_ranges`] が決め（`docs/history.md` B-77）、[`Project::ref_sites`] がその範囲だけを
//! 辞書で走査する。位置は**ファイル単位の byte**（B-1・B-56）で返す。

use std::collections::BTreeMap;
use std::ops::Range;

use crate::core::diagnostics::Diagnostic;
use crate::core::features::Location;
use crate::core::id::{Digits, SceneId};
use crate::core::model::document::{Document, DocumentFormat};
use crate::core::model::project::Project as Settings;
use crate::core::resolve::dictionary::{
    self, Dictionary, EntryId, EntryKind, RefToken, SourceFile, Target,
};
use crate::core::syntax::{
    characters_md, contents, materials_index, plot_episode, plot_outline, project_toml, words_md,
};
use crate::core::text::lexer::{self, LineKind, TextFormat};
use crate::core::text::position::LineTable;

/// `plots/all.cnov`（§4.1 の全体プロット）。
pub const ALL_PLOT_PATH: &str = "plots/all.cnov";

/// プロジェクトの集約（§3.4）。
#[derive(Debug)]
pub struct Project {
    settings: Settings,
    pub assets: BTreeMap<String, Vec<u8>>,
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
            assets: BTreeMap::new(),
            files: BTreeMap::new(),
            dictionary: Dictionary::empty(),
            dictionary_diagnostics: Vec::new(),
        }
    }

    /// 文書をまとめて渡して作る（辞書は最後に 1 回だけ作る）。
    ///
    /// `documents` に `project.toml` があれば、`settings` ではなく**その中身**を設定にする（B-89）。
    pub fn with_documents(
        settings: Settings,
        documents: impl IntoIterator<Item = Document>,
    ) -> Self {
        let mut project = Self::new(settings);
        for document in documents {
            project.adopt_settings(&document);
            project.files.insert(document.path.clone(), document);
        }
        project.rebuild_dictionary();
        project
    }

    /// 文書を足す／置き換える。辞書の供給元（§6.1.1 の 3 ファイル）なら辞書を作り直す。
    /// `project.toml` なら設定をその中身で置き換える（B-89）。
    pub fn insert(&mut self, document: Document) {
        let rebuild = is_dictionary_source(&document.path);
        self.adopt_settings(&document);
        self.files.insert(document.path.clone(), document);
        if rebuild {
            self.rebuild_dictionary();
        }
    }

    /// 文書を外す。辞書の供給元なら辞書を作り直す。`project.toml` なら設定を既定値に戻す。
    pub fn remove(&mut self, path: &str) -> Option<Document> {
        let removed = self.files.remove(path);
        if let Some(document) = &removed {
            if document.format == DocumentFormat::ProjectFile {
                self.settings = Settings::default();
            }
            if is_dictionary_source(path) {
                self.rebuild_dictionary();
            }
        }
        removed
    }

    /// `project.toml` の文書なら、その中身を設定にする（読めなければ既定値、§4.3-4）。
    fn adopt_settings(&mut self, document: &Document) {
        if document.format == DocumentFormat::ProjectFile {
            self.settings = project_toml::settings(&document.text);
        }
    }

    /// 設定（`project.toml`）を差し替える。`project.toml` の文書を後から `insert` すると、
    /// そちらの中身で置き換わる。
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// 設定（`project.toml`、§4.5）。
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// パス（プロジェクトルートからの相対、`/` 区切り）で文書を引く。
    pub fn document(&self, path: &str) -> Option<&Document> {
        self.files.get(path)
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.files.contains_key(path) || self.assets.contains_key(path)
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

    /// `contents/AA-BB.cnov`（§4.1）。
    pub fn contents_path(&self, chapter: u16, episode: u16) -> String {
        format!("contents/{}.cnov", self.episode_stem(chapter, episode))
    }

    /// `plots/AA-BB.cnov`（§4.1 の話プロット）。
    pub fn episode_plot_path(&self, chapter: u16, episode: u16) -> String {
        format!("plots/{}.cnov", self.episode_stem(chapter, episode))
    }

    /// `plots/AA.cnov`（§4.1 の章プロット）。
    pub fn chapter_plot_path(&self, chapter: u16) -> String {
        let digits = self.digits();
        format!("plots/{}.cnov", pad(chapter, digits.chapter))
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

    /// A 形式のパーサの設定。`plots/all.cnov` は 1 階層、`plots/AA.cnov` は 2 階層（§5.3.2）。
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

    /// 章プロット（`plots/AA.cnov`）の木。文書が無ければ `None`。
    pub fn chapter_plot(&self, chapter: u16) -> Option<plot_outline::Outline<'_>> {
        let path = self.chapter_plot_path(chapter);
        let found = self.files.get(&path)?;
        Some(plot_outline::parse_with(
            &found.text,
            &self.outline_options(&path),
        ))
    }

    /// 全体プロット（`plots/all.cnov`）の木。文書が無ければ `None`。
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
    /// 見出しより前の行や、ID が壊れた見出しの下の行は `None`。B 形式の場面レベルの行
    /// （見出し直下のフィールド）は節 0。
    pub fn scene_at(&self, document: &Document, line: usize) -> Option<SceneId> {
        self.scenes_by_line(document).get(line).copied().flatten()
    }

    /// 行ごとの節（[`Project::scene_at`] を全行ぶん）。C 形式・B 形式以外は空。
    pub fn scenes_by_line(&self, document: &Document) -> Vec<Option<SceneId>> {
        match document.format {
            DocumentFormat::Contents => contents_scenes(&contents::parse_with(
                &document.text,
                &self.contents_options(),
            )),
            DocumentFormat::PlotEpisode => episode_scenes(&plot_episode::parse_with(
                &document.text,
                &self.contents_options(),
            )),
            _ => Vec::new(),
        }
    }

    /// `pos` にある参照（`'` トークンか話者注釈）。トークンの直後（`pos == end`）も含める
    /// （打鍵した直後のカーソル位置でホバー・定義ジャンプができるように）。
    pub fn site_at(&self, document: &Document, pos: usize) -> Option<SiteAt> {
        let line = line_of(&document.text, pos);
        if let Some(found) = self
            .ref_sites(document)
            .into_iter()
            .filter(|site| site.line == line)
            .find(|site| {
                let range = site.range();
                range.start <= pos && pos <= range.end
            })
        {
            return Some(SiteAt::Ref(found));
        }
        self.speaker_sites(document)
            .into_iter()
            .filter(|site| site.line == line)
            .find(|site| site.sigil <= pos && pos <= site.end())
            .map(SiteAt::Speaker)
    }

    /// 参照の解決先（辞書のエントリの `Target`）。
    pub fn target_of(&self, site: &SiteAt) -> Option<&Target> {
        let id = match site {
            SiteAt::Ref(found) => found.entry()?,
            SiteAt::Speaker(found) => found.entry?,
        };
        self.dictionary.entry(id).map(|entry| &entry.target)
    }

    /// 解決先の定義の位置（§3.4 `definition`）。キャラと用語は H1 の名前、資料は資料 MD が
    /// プロジェクトにあればその 1 行目、無ければ `materials/index.json` の ID。
    pub fn definition_of(&self, target: &Target) -> Option<Location> {
        match target {
            Target::Character { index, .. } => {
                let tree = self.characters();
                let found = tree.characters.get(*index)?;
                let path = SourceFile::Characters.path();
                let start = line_starts_of(self.text(path))
                    .get(found.heading)
                    .copied()?;
                let at = start + found.name_at;
                Some(Location {
                    path: path.to_string(),
                    range: at..at + found.name.len(),
                })
            }
            Target::Word { index, .. } => {
                let tree = self.words();
                let found = tree.words.get(*index)?;
                let path = SourceFile::Words.path();
                let start = line_starts_of(self.text(path))
                    .get(found.heading)
                    .copied()?;
                let at = start + found.term_at;
                Some(Location {
                    path: path.to_string(),
                    range: at..at + found.term.len(),
                })
            }
            Target::Material { id } => {
                let index = self.materials();
                let material = index
                    .index
                    .as_ref()?
                    .items
                    .iter()
                    .find(|item| item.id == *id)?;
                let path = material.path.trim_start_matches("./");
                if let Some(found) = self.files.get(path) {
                    let end = found.text.find('\n').unwrap_or(found.text.len());
                    return Some(Location {
                        path: path.to_string(),
                        range: 0..end,
                    });
                }
                let entry = self.dictionary.entries().iter().find(|entry| {
                    entry.kind == EntryKind::Material
                        && matches!(&entry.target, Target::Material { id: found } if found == id)
                })?;
                Some(Location {
                    path: SourceFile::MaterialsIndex.path().to_string(),
                    range: entry.origin.range.clone()?,
                })
            }
        }
    }

    /// プロジェクト全体で `target` を指している参照（`'` トークンと話者注釈）。パス → 位置の順。
    pub fn references_to(&self, target: &Target) -> Vec<Location> {
        let mut out = Vec::new();
        for document in self.files.values() {
            for site in self.ref_sites(document) {
                let Some(found) = site.entry().and_then(|id| self.dictionary.entry(id)) else {
                    continue;
                };
                if same_target(&found.target, target) {
                    out.push(Location {
                        path: document.path.clone(),
                        range: site.range(),
                    });
                }
            }
            for site in self.speaker_sites(document) {
                let Some(found) = site.entry.and_then(|id| self.dictionary.entry(id)) else {
                    continue;
                };
                if same_target(&found.target, target) {
                    out.push(Location {
                        path: document.path.clone(),
                        range: site.sigil..site.end(),
                    });
                }
            }
        }
        out.sort_by(|left, right| {
            (left.path.as_str(), left.range.start).cmp(&(right.path.as_str(), right.range.start))
        });
        out
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

/// 行ごとの節（C 形式）。見出しより前・ID の壊れた見出しの下は `None`。
pub fn contents_scenes(parsed: &contents::Contents<'_>) -> Vec<Option<SceneId>> {
    let mut out = vec![None; parsed.lines.len()];
    for section in &parsed.sections {
        let Some(scene) = section.scene else {
            continue;
        };
        for line in section.span.clone() {
            if let Some(slot) = out.get_mut(line) {
                *slot = Some(SceneId {
                    section: section.number,
                    ..scene
                });
            }
        }
    }
    out
}

/// 行ごとの節（B 形式）。場面レベルの行（見出し直下のフィールド）は節 0。
pub fn episode_scenes(parsed: &plot_episode::EpisodePlot<'_>) -> Vec<Option<SceneId>> {
    let mut out = vec![None; parsed.lines.len()];
    for scene in &parsed.scenes {
        let Some(id) = scene.id else {
            continue;
        };
        for line in scene.span.clone() {
            if let Some(slot) = out.get_mut(line) {
                *slot = Some(id);
            }
        }
        for section in &scene.sections {
            for line in section.span.clone() {
                if let Some(slot) = out.get_mut(line) {
                    *slot = Some(SceneId {
                        section: section.number,
                        ..id
                    });
                }
            }
        }
    }
    out
}

/// [`Project::site_at`] の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteAt {
    /// `'` 参照。
    Ref(RefSite),
    /// 話者注釈。
    Speaker(SpeakerSite),
}

impl SiteAt {
    /// 参照全体の範囲（ファイル単位）。
    pub fn range(&self) -> Range<usize> {
        match self {
            Self::Ref(found) => found.range(),
            Self::Speaker(found) => found.sigil..found.end(),
        }
    }
}

impl SpeakerSite {
    /// 注釈の終わり（モード式があればその後ろ）。
    pub fn end(&self) -> usize {
        self.modes
            .last()
            .map_or(self.name_range.end, |(_, range)| range.end)
    }
}

/// 同じ実体か（キャラと用語は添字、資料は ID で比べる）。
pub fn same_target(left: &Target, right: &Target) -> bool {
    match (left, right) {
        (Target::Character { index: a, .. }, Target::Character { index: b, .. }) => a == b,
        (Target::Word { index: a, .. }, Target::Word { index: b, .. }) => a == b,
        (Target::Material { id: a }, Target::Material { id: b }) => a == b,
        _ => false,
    }
}

/// `pos` を文字境界まで切り下げる（UI から来た位置が文字の途中でもパニックしない、§4.3-4）。
pub fn floor_boundary(text: &str, pos: usize) -> usize {
    let mut at = pos.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// `pos` を含む行の番号。
pub fn line_of(text: &str, pos: usize) -> usize {
    let end = pos.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
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

/// `contents/01-04.cnov` / `plots/01-04.cnov` → `(1, 4)`。話のファイルでなければ `None`。
pub fn episode_of_path(path: &str) -> Option<(u16, u16)> {
    let stem = path
        .strip_prefix("contents/")
        .or_else(|| path.strip_prefix("plots/"))?
        .strip_suffix(".cnov")?;
    let (chapter, episode) = stem.split_once('-')?;
    Some((number(chapter)?, number(episode)?))
}

/// `plots/01.cnov` → `1`。章プロットでなければ `None`。
pub fn chapter_of_path(path: &str) -> Option<u16> {
    let stem = path.strip_prefix("plots/")?.strip_suffix(".cnov")?;
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

/// `'` を走査してよい範囲（**行内相対**）。§5.0.2 の「全形式の本文部」（`docs/history.md` B-77）。
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
