//! 編集の状態（§8.1 の節単位編集・§8.2 の保存・§9.8.2 の構造移動）。**egui に依存しない**。
//!
//! 節エディタが持つのは**今の節の本文だけ**（[`Workbench::buffer`]）。文書（`Project` の中の
//! `Document`）へは [`Workbench::commit`] で書き戻す（節の移動・保存の直前、§8.1）。
//! 書き込みはしない: [`Workbench::take_writes`] が「書くべきファイルと中身」を返し、ホスト
//! （`ui::app`）が書いて [`Workbench::write_finished`] で結果を戻す（`docs/history.md` B-114）。
//!
//! 本文以外の文書（話プロット・全体/章プロット・`ideas.md`・`characters.md`・資料）は、M8 の
//! ワークスペースが**行の範囲**（[`Slice`]）で開いて編集する。規則は節エディタと同じで、書き戻しは
//! [`Workbench::commit`]（節の移動・保存・ワークスペースの切替の直前）に一緒に行う（B-126）。

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use crate::app::save_machine::SaveMachine;
use crate::core::features::plots;
use crate::core::features::sections::{self, SectionRef};
use crate::core::features::Project;
use crate::core::id::SceneId;
use crate::core::model::document::Document;

/// 節の削除（[`Workbench::delete_section_at`]）の結果（§8.1.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    /// 消した（本文と対応プロット、あるいは片側だけ）。
    Deleted,
    /// 対応が一意に定まらず、何も消さなかった。
    Ambiguous,
    /// 本文にもプロットにも対象が無い。
    NotFound,
}

/// 構造移動（§9.8.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    /// 前の節（`Alt+↑`）。
    PrevSection,
    /// 次の節（`Alt+↓`）。
    NextSection,
    /// 前の場面（`Alt+←`）。
    PrevScene,
    /// 次の場面（`Alt+→`）。
    NextScene,
    /// 前の話（`Ctrl+Alt+←`）。
    PrevFile,
    /// 次の話（`Ctrl+Alt+→`）。
    NextFile,
    /// ファイルの最初の節（`Ctrl+Home`）。
    FileStart,
    /// ファイルの最後の節（`Ctrl+End`）。
    FileEnd,
}

/// 外部変更への反応（§8.5）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reaction {
    /// 読み直した（I052）。
    Reloaded,
    /// 未保存の変更があるので反映しなかった（衝突）。
    Conflict,
    /// 何もしなかった（中身が同じ）。
    Unchanged,
}

/// 編集中の節（文書の中で節エディタが受け持つ行）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Editing {
    section: SectionRef,
}

/// 本文以外の文書の一部（行の範囲）を編集する口（M8 のワークスペース、`docs/history.md` B-126）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    /// 文書のパス（まだ無いファイルでもよい。書き始めると作る）。
    pub path: String,
    /// 受け持つ行（ファイルの行 index）。空なら、その位置に行を入れる。
    pub lines: Range<usize>,
    /// 開いた側が決める鍵（同じものを開き直さないため。`plot-episode:plots/01-04.cnov:01-04-02-03` など）。
    pub tag: String,
}

type WorkspaceTransaction = (Vec<(String, String)>, Vec<(String, String)>);

/// 編集の状態。
#[derive(Debug)]
pub struct Workbench {
    workspace_undo: Vec<WorkspaceTransaction>,
    workspace_redo: Vec<WorkspaceTransaction>,
    human: Option<(SectionRef, String, String, String)>,
    project: Project,
    list: Vec<SectionRef>,
    editing: Option<Editing>,
    buffer: String,
    /// 保存ステートマシン（§8.2）。
    pub save: SaveMachine,
    unsaved: BTreeSet<String>,
    conflicts: BTreeSet<String>,
    counts: BTreeMap<String, usize>,
    generation: u64,
    slice: Option<Slice>,
    slice_buffer: String,
    /// [`Slice`] のエディタの最後の選択範囲（中身の中の byte。`Ctrl+M` の挿入位置、B-134）。
    slice_caret: Option<Range<usize>>,
    /// まだ無い本文のファイルを開いているとき、書き始めたら先頭に書く場面見出し（B-128）。
    pending_header: Option<String>,
    /// 書き込みの後で消すファイル（資料の採用で `inbox/` から `docs/` へ動かした元、§16.1）。
    removals: BTreeSet<String>,
}

impl Workbench {
    /// プロジェクトを開いた直後。`last_scene`（`ui.json`）があればそこから、無ければ最初の節から。
    pub fn new(project: Project, last_scene: Option<&str>, autosave_ms: u32) -> Self {
        let mut out = Self {
            workspace_undo: Vec::new(),
            workspace_redo: Vec::new(),
            human: None,
            project,
            list: Vec::new(),
            editing: None,
            buffer: String::new(),
            save: SaveMachine::new(autosave_ms),
            unsaved: BTreeSet::new(),
            conflicts: BTreeSet::new(),
            counts: BTreeMap::new(),
            generation: 0,
            slice: None,
            slice_buffer: String::new(),
            slice_caret: None,
            pending_header: None,
            removals: BTreeSet::new(),
        };
        out.rebuild();
        let start = last_scene
            .and_then(|scene| sections::jump(&out.list, scene))
            .or(if out.list.is_empty() { None } else { Some(0) });
        if let Some(index) = start {
            out.enter(index);
        }
        out
    }

    /// プロジェクト（文書・辞書・設定）。
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// 文書が変わるたびに増える番号（描画側のキャッシュの鍵）。
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// 全節の並び（最後に書き戻した時点の）。
    pub fn sections(&self) -> &[SectionRef] {
        &self.list
    }

    /// 編集中の節。
    pub fn current(&self) -> Option<&SectionRef> {
        self.editing.as_ref().map(|editing| &editing.section)
    }

    /// 編集中の節の並びの中の位置。新しく作ったばかりでまだ文書に無い節（ファイル・場面の末尾、
    /// [`Workbench::new_section`]）なら `None`。
    pub fn current_index(&self) -> Option<usize> {
        let current = self.current()?;
        self.list
            .iter()
            .position(|item| item.path == current.path && item.lines.start == current.lines.start)
    }

    /// 並びの中で編集中の節より前にある節の数（文脈ビューの「前」と「後」の境目）。
    pub fn split_index(&self) -> usize {
        let Some(current) = self.current() else {
            return 0;
        };
        let key = (current.path.as_str(), current.lines.start);
        self.list
            .iter()
            .take_while(|item| (item.path.as_str(), item.lines.start) < key)
            .count()
    }

    /// 節エディタの中身。
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// 節エディタの中身（egui の `TextEdit` に渡す）。変えたら [`Workbench::edited`] を呼ぶ。
    pub fn buffer_mut(&mut self) -> &mut String {
        &mut self.buffer
    }

    /// 節エディタで編集があった（§8.2 の Dirty とタイマ再起動）。
    pub fn edited(&mut self, now: f64) {
        if self.human.is_some() {
            return;
        }
        if let Some(editing) = &self.editing {
            self.unsaved.insert(editing.section.path.clone());
        }
        self.save.edited(now);
    }

    pub fn human_active(&self) -> bool {
        self.human.is_some()
    }
    pub fn toggle_human(&mut self, stamp: &str, now: f64) {
        if self.human.is_some() {
            self.finish_human(now);
            return;
        }
        self.commit();
        if let Some(current) = self.current().cloned() {
            let max = self
                .project
                .documents()
                .filter_map(|d| {
                    d.path
                        .strip_prefix("agents/proposals/H")
                        .and_then(|s| s.strip_suffix(".md"))
                        .and_then(|n| n.parse::<u32>().ok())
                })
                .max()
                .unwrap_or(0);
            self.human = Some((
                current,
                self.buffer.clone(),
                format!("H{:04}", max + 1),
                stamp.to_owned(),
            ));
        }
    }
    pub fn finish_human(&mut self, now: f64) {
        let Some((current, before, id, stamp)) = self.human.take() else {
            return;
        };
        let after = std::mem::replace(&mut self.buffer, before.clone());
        if before == after {
            return;
        }
        let proposal = crate::core::agents::files::Proposal {
            id: id.clone(),
            kind: "human".to_owned(),
            target: crate::core::agents::files::target_of(
                &current.path,
                Some(current.id),
                self.project.digits(),
            ),
            origin: "human".to_owned(),
            status: "open".to_owned(),
            created_at: stamp.clone(),
            updated_at: stamp,
            before,
            after,
            ..Default::default()
        };
        let path = crate::core::agents::files::proposal_path(&id);
        self.project.insert(Document::new(
            &path,
            crate::core::agents::files::write_proposal(&proposal),
        ));
        self.unsaved.insert(path);
        self.save.edited(now);
        self.save.request_now();
        self.generation += 1;
    }

    /// 未保存のファイル（パスの昇順）。
    pub fn unsaved(&self) -> impl Iterator<Item = &str> {
        self.unsaved.iter().map(String::as_str)
    }

    /// 外部変更と衝突しているファイル（§8.5）。
    pub fn conflicts(&self) -> impl Iterator<Item = &str> {
        self.conflicts.iter().map(String::as_str)
    }

    /// 節エディタ（と、開いていれば [`Slice`]）の中身を文書へ書き戻す（移動はしない）。並びを作り直す。
    pub fn commit(&mut self) {
        self.commit_slice();
        self.commit_section();
    }

    fn commit_section(&mut self) {
        if self.human.is_some() {
            return;
        }
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let path = editing.section.path.clone();
        let Some(document) = self.project.document(&path) else {
            // まだ無いファイル（[`Workbench::start_file`]）: 書き始めたら場面見出しと一緒に作る
            if self.buffer.is_empty() {
                return;
            }
            let Some(header) = self.pending_header.take() else {
                return;
            };
            let count = sections::line_count(&self.buffer);
            editing.section.lines = 1..1 + count;
            editing.section.span = 1..1 + count;
            let text = format!("{header}\n{}\n", self.buffer);
            self.project.insert(Document::new(path.clone(), text));
            self.unsaved.insert(path);
            self.rebuild();
            return;
        };
        let current = sections::text_of(&document.text, &editing.section);
        if current == self.buffer {
            return;
        }
        let text = sections::splice(&document.text, &editing.section, &self.buffer);
        let count = if self.buffer.is_empty() {
            0
        } else {
            self.buffer.split('\n').count()
        };
        let start = editing.section.lines.start;
        let old_end = editing.section.lines.end;
        editing.section.lines = start..start + count;
        let grow = (start + count) as isize - old_end as isize;
        editing.section.span = editing.section.span.start
            ..((editing.section.span.end as isize + grow).max((start + count) as isize)) as usize;
        let updated = Document::new(path.clone(), text);
        self.project.insert(updated);
        self.unsaved.insert(path);
        self.rebuild();
    }

    /// 貼り付けたテキストを今の節へ入れる（§8.1.3）。
    ///
    /// `selection`（本文の中の byte）を `pasted` で置き換える。改行はそのまま残り、単独 `;` 行・場面
    /// 見出し行（`@A-B-C`）が在ればその位置で節に分けて、分けた後ろは同じファイルの続きの節として
    /// 一括で文書へ入れる（§8.1「節の移動で即座に文書へ反映」）。エディタに残るのは先頭の節の本文。
    ///
    /// 分ける必要が無ければ何もしないで `false`（呼び出し側は普通の貼り付けに任せる）。
    pub fn paste(&mut self, selection: Range<usize>, pasted: &str, now: f64) -> bool {
        if self.human.is_some() {
            return false;
        }
        let Some(editing) = self.editing.as_ref() else {
            return false;
        };
        let path = editing.section.path.clone();
        let section = editing.section.clone();
        // 選択が在れば置き換える（[`sections::split_pasted_sections`] はカーソルへの挿入だけを見る）
        let start = selection.start.min(selection.end).min(self.buffer.len());
        let end = selection.end.max(selection.start).min(self.buffer.len());
        if !self.buffer.is_char_boundary(start) || !self.buffer.is_char_boundary(end) {
            return false;
        }
        let mut base = self.buffer.clone();
        base.replace_range(start..end, "");
        let Some(blocks) = sections::split_pasted_sections(&base, start, pasted) else {
            return false;
        };
        let Some(document) = self.project.document(&path) else {
            // まだ無いファイルを編集中（[`Workbench::start_file`]）: 分けずに普通の貼り付けに任せる
            return false;
        };
        // 節の本文に見出し行と区切り `;` を混ぜた形で書き戻す（節が増えるのは並びの作り直しが拾う）
        let text = sections::splice(&document.text, &section, &blocks.join("\n;\n"));
        let count = sections::line_count(&blocks[0]);
        let first = blocks[0].clone();
        if let Some(editing) = self.editing.as_mut() {
            let start = editing.section.lines.start;
            let old_end = editing.section.lines.end;
            editing.section.lines = start..start + count;
            let grow = (start + count) as isize - old_end as isize;
            editing.section.span = editing.section.span.start
                ..((editing.section.span.end as isize + grow).max((start + count) as isize))
                    as usize;
        }
        self.buffer = first;
        self.project.insert(Document::new(path.clone(), text));
        self.unsaved.insert(path);
        self.rebuild();
        self.edited(now);
        true
    }

    /// 構造移動（§9.8.2）。移動したら即時の保存を求める（§8.1）。動けなければ `false`。
    pub fn go(&mut self, nav: Nav) -> bool {
        self.commit();
        if self.current().is_none() {
            return false;
        }
        let Some(index) = self.current_index() else {
            // まだ文書に無い新しい節（末尾）: 前は直前の節、後は直後の節
            let split = self.split_index();
            let target = match nav {
                Nav::PrevSection | Nav::PrevScene | Nav::PrevFile | Nav::FileStart => {
                    split.checked_sub(1)
                }
                _ => (split < self.list.len()).then_some(split),
            };
            return match target {
                Some(found) => {
                    self.enter(found);
                    self.save.request_now();
                    true
                }
                None => false,
            };
        };
        let target = match nav {
            Nav::PrevSection => index.checked_sub(1),
            Nav::NextSection => (index + 1 < self.list.len()).then_some(index + 1),
            Nav::PrevScene => sections::scene_step(&self.list, index, false),
            Nav::NextScene => sections::scene_step(&self.list, index, true),
            Nav::PrevFile => sections::file_step(&self.list, index, false),
            Nav::NextFile => sections::file_step(&self.list, index, true),
            Nav::FileStart => {
                let path = &self.list[index].path;
                self.list.iter().position(|item| &item.path == path)
            }
            Nav::FileEnd => {
                let path = &self.list[index].path;
                self.list.iter().rposition(|item| &item.path == path)
            }
        };
        match target {
            Some(found) if found != index => {
                self.enter(found);
                self.save.request_now();
                true
            }
            _ => false,
        }
    }

    /// 並びの `index` 番目の節へ（文脈ビューのクリック・パレット）。
    pub fn go_to(&mut self, index: usize) -> bool {
        self.commit();
        if index >= self.list.len() || Some(index) == self.current_index() {
            return false;
        }
        self.enter(index);
        self.save.request_now();
        true
    }

    /// SceneId ジャンプ（§9.7.2。`0104` のような区切り無しも受ける）。
    pub fn jump(&mut self, query: &str) -> bool {
        self.commit();
        match sections::jump(&self.list, query) {
            Some(found) => {
                if Some(found) != self.current_index() {
                    self.enter(found);
                    self.save.request_now();
                }
                true
            }
            None => false,
        }
    }

    /// この節の直後に新しい節を作って移動（§9.4.1 `Ctrl+;`）。
    pub fn new_section(&mut self, now: f64) -> bool {
        self.commit();
        let Some(editing) = self.editing.clone() else {
            return false;
        };
        let Some(document) = self.project.document(&editing.section.path) else {
            return false;
        };
        let (text, line) = sections::insert_after(&document.text, &editing.section);
        let path = editing.section.path.clone();
        self.project.insert(Document::new(path.clone(), text));
        self.unsaved.insert(path.clone());
        self.rebuild();
        let found = self
            .list
            .iter()
            .position(|item| item.path == path && item.lines.start == line);
        match found {
            Some(index) => self.enter(index),
            None => {
                // ファイル・場面の末尾: `;` の後に本文が無いと節にならない（§5.2.2）ので、
                // 書き始めるまでは文書に無い節として持つ（書き戻すとそこに行が入る）
                let section = SectionRef {
                    path: path.clone(),
                    id: SceneId {
                        section: editing.section.id.section + 1,
                        ..editing.section.id
                    },
                    lines: line..line,
                    span: line..line,
                    header: editing.section.header,
                    scene_title: editing.section.scene_title.clone(),
                };
                self.buffer.clear();
                self.editing = Some(Editing { section });
            }
        }
        self.save.edited(now);
        self.save.request_now();
        true
    }

    /// 確定して次の節へ。同じファイルに次の節が無ければ新しい節を作る（§9.4.1 `Ctrl+Enter`）。
    pub fn confirm_next(&mut self, now: f64) -> bool {
        self.commit();
        let Some(index) = self.current_index() else {
            // まだ文書に無い新しい節（空のまま）: もう 1 つ作らない
            return false;
        };
        let same_file_next = self
            .list
            .get(index + 1)
            .is_some_and(|next| next.path == self.list[index].path);
        if same_file_next {
            self.go(Nav::NextSection)
        } else {
            self.new_section(now)
        }
    }

    /// まだ無い本文のファイル `path` を、場面見出し `header`（`@01-05-01 タイトル`）の最初の節として
    /// 開く（`plot-episode` から降りたとき、`docs/history.md` B-128）。**書き始めるまでファイルは作らない**
    /// （§10.0「空の節を作らず、その位置にカーソルだけ置く」）。ファイルがあれば何もしない。
    pub fn start_file(&mut self, path: &str, id: SceneId, header: &str) -> bool {
        self.commit();
        if self.project.document(path).is_some() {
            return false;
        }
        let title = header
            .trim_start()
            .strip_prefix('@')
            .and_then(|rest| rest.find(char::is_whitespace).map(|at| rest[at..].trim()))
            .unwrap_or_default()
            .to_owned();
        self.buffer.clear();
        self.editing = Some(Editing {
            section: SectionRef {
                path: path.to_owned(),
                id: SceneId { section: 1, ..id },
                lines: 1..1,
                span: 1..1,
                header: Some(0),
                scene_title: title,
            },
        });
        self.pending_header = Some(header.to_owned());
        true
    }

    /// 節をカーソルのバイト位置で 2 つに分ける（§8.1.1。正規操作 `section.split` と同じ意味。
    /// GUI の `Ctrl+Enter` はここを通る）。
    ///
    /// `at` は節エディタの中身の中のバイト位置。先頭では何もしない。末尾では
    /// 「確定して次の節へ」([`Workbench::confirm_next`]) と等価になる。何も書かれていない節
    /// （[`sections::is_unwritten`]）では新たな空節を作らない（末尾で同じファイルに次節が
    /// ある場合だけ移動する）。途中で割ったら、下側の新しい節へ移る。
    pub fn split_section(&mut self, at: usize, now: f64) -> bool {
        if self.human.is_some() {
            return false;
        }
        let at = at.min(self.buffer.len());
        if at == 0 || !self.buffer.is_char_boundary(at) {
            return false;
        }
        if sections::is_unwritten(&self.buffer) {
            if at < self.buffer.len() {
                return false;
            }
            // 空節は作らない。同じファイルの次節がある場合だけ移動する
            self.commit();
            let Some(index) = self.current_index() else {
                return false;
            };
            let same_file_next = self
                .list
                .get(index + 1)
                .is_some_and(|next| next.path == self.list[index].path);
            return same_file_next && self.go(Nav::NextSection);
        }
        if at >= self.buffer.len() {
            return self.confirm_next(now);
        }
        let Some((upper, lower)) = sections::split_body(&self.buffer, at) else {
            return false;
        };
        self.commit();
        let Some(editing) = self.editing.clone() else {
            return false;
        };
        let Some(document) = self.project.document(&editing.section.path) else {
            return false;
        };
        let section = editing.section.clone();
        let text = sections::splice(&document.text, &section, &format!("{upper}\n;\n{lower}"));
        let path = section.path.clone();
        self.project.insert(Document::new(path.clone(), text));
        self.unsaved.insert(path.clone());
        self.rebuild();
        // 下側は元の次の番号になる（§5.2.2。位置から決まる）
        let lower_id = SceneId {
            section: section.id.section + 1,
            ..section.id
        };
        let found = sections::position_of(&self.list, &path, lower_id)
            .or_else(|| sections::position_of(&self.list, &path, section.id));
        if let Some(index) = found {
            self.enter(index);
        }
        self.save.edited(now);
        self.save.request_now();
        true
    }

    /// 節 `id` が「空」か（§8.1.2 の確認を省けるか）。本文と対応プロットの両方が
    /// 何も書かれていない（または存在しない）ときだけ空とする。
    pub fn delete_is_empty(&self, id: SceneId) -> bool {
        let contents_empty = if self.current_id() == Some(id) {
            sections::is_unwritten(&self.buffer)
        } else {
            match self.list.iter().find(|item| item.id == id) {
                Some(section) => self.project.document(&section.path).is_none_or(|document| {
                    sections::is_unwritten(&sections::text_of(&document.text, section))
                }),
                None => true,
            }
        };
        let blocks = plots::section_blocks(&self.project, id);
        let plot_empty = match blocks.as_slice() {
            [] => true,
            [block] => {
                let path = self.project.episode_plot_path(id.chapter, id.episode);
                sections::is_unwritten(&sections::text_of_lines(
                    self.project.text(&path),
                    &block.content,
                ))
            }
            _ => false,
        };
        contents_empty && plot_empty
    }

    /// 節 `id` を、対応するプロットブロックと一緒に消す（§8.1.2 の単一経路。全入口共通）。
    ///
    /// 本文・プロットのどちらかで対象が複数あれば、対応が一意に定まらないとして
    /// 何も消さず [`DeleteOutcome::Ambiguous`] を返す。片側にしか無い節は、その側だけを消す。
    pub fn delete_section_at(&mut self, id: SceneId, now: f64) -> DeleteOutcome {
        self.commit();
        let contents: Vec<usize> = self
            .list
            .iter()
            .enumerate()
            .filter(|(_, item)| item.id == id)
            .map(|(index, _)| index)
            .collect();
        let blocks = plots::section_blocks(&self.project, id);
        if contents.len() > 1 || blocks.len() > 1 {
            return DeleteOutcome::Ambiguous;
        }
        if contents.is_empty() && blocks.is_empty() {
            return DeleteOutcome::NotFound;
        }
        let current = self.current_index();
        let mut removed_lines = 0isize;
        let mut removed_path: Option<String> = None;
        if let Some(&index) = contents.first() {
            let section = self.list[index].clone();
            if let Some(document) = self.project.document(&section.path) {
                let text = sections::remove(&document.text, &section);
                removed_lines =
                    document.text.split('\n').count() as isize - text.split('\n').count() as isize;
                removed_path = Some(section.path.clone());
                self.project
                    .insert(Document::new(section.path.clone(), text));
                self.unsaved.insert(section.path.clone());
            }
        }
        if let Some(block) = blocks.first() {
            let path = self.project.episode_plot_path(id.chapter, id.episode);
            let text = plots::remove_block(self.project.text(&path), block);
            self.project.insert(Document::new(path.clone(), text));
            self.unsaved.insert(path);
        }
        self.rebuild();
        if self.list.is_empty() {
            self.editing = None;
            self.buffer.clear();
        } else {
            match current {
                Some(cur) => {
                    let target = match contents.first() {
                        Some(&del) if del < cur => cur - 1,
                        Some(&del) if del == cur => cur.min(self.list.len() - 1),
                        _ => cur.min(self.list.len() - 1),
                    };
                    self.enter(target);
                }
                None => {
                    // まだ文書に無い節を編集中: 同じファイルで行が減ったら位置を寄せる
                    if let (Some(editing), Some(path)) = (&mut self.editing, &removed_path) {
                        if editing.section.path == *path && removed_lines != 0 {
                            let section = &mut editing.section;
                            let shift =
                                |line: usize| (line as isize - removed_lines).max(0) as usize;
                            section.lines = shift(section.lines.start)..shift(section.lines.end);
                            section.span = shift(section.span.start)..shift(section.span.end);
                        }
                    }
                }
            }
        }
        self.save.edited(now);
        self.save.request_now();
        DeleteOutcome::Deleted
    }

    /// 今の節を消して、次の節（無ければ前の節）へ（§9.8.3 `Ctrl+Shift+;`。確認は呼び出し側）。
    pub fn delete_section(&mut self, now: f64) -> bool {
        let Some(id) = self.current_id() else {
            return false;
        };
        // まだ文書に無い節（末尾の仮想節）でプロットにも無ければ、消すものが無い
        if self.current_index().is_none() && plots::section_blocks(&self.project, id).is_empty() {
            return false;
        }
        self.delete_section_at(id, now) == DeleteOutcome::Deleted
    }

    /// 今のファイルの中身を置き換える（`Ctrl+H` の「すべて置換」）。今の節は同じ ID の節を開き直す。
    pub fn replace_current_file(&mut self, text: String, now: f64) -> bool {
        self.commit();
        let Some(editing) = self.editing.clone() else {
            return false;
        };
        let path = editing.section.path.clone();
        if self.project.text(&path) == text {
            return false;
        }
        self.project.insert(Document::new(path.clone(), text));
        self.unsaved.insert(path.clone());
        self.rebuild();
        match sections::position_of(&self.list, &path, editing.section.id) {
            Some(index) => self.enter(index),
            None => {
                self.editing = None;
                self.buffer.clear();
            }
        }
        self.save.edited(now);
        true
    }

    /// ファイルの中の byte 位置を含む節へ移る。移った先の節の本文の中での位置を返す（節の外なら `None`）。
    pub fn reveal(&mut self, path: &str, byte: usize) -> Option<usize> {
        self.commit();
        let text = self.project.text(path).to_owned();
        let line = crate::core::features::project::line_of(&text, byte);
        let index = self
            .list
            .iter()
            .position(|item| item.path == path && item.span.contains(&line))
            .or_else(|| {
                // 見出しの行など節の外: その後ろの最初の節
                self.list
                    .iter()
                    .position(|item| item.path == path && item.lines.start >= line)
            })?;
        if Some(index) != self.current_index() {
            self.enter(index);
            self.save.request_now();
        }
        let section = &self.list[index];
        if !section.lines.contains(&line) {
            return None;
        }
        let starts = crate::core::features::project::line_starts_of(&text);
        let start = starts.get(section.lines.start).copied()?;
        byte.checked_sub(start)
    }

    /// 編集中の節の本文の先頭が、ファイルの中で何 byte 目か（補完・ホバーの位置の換算）。
    /// 返す文書は節エディタの中身を書き戻した形（まだ文書に書き戻していない編集も含む）。
    pub fn spliced_document(&self) -> Option<(Document, usize)> {
        let current = self.current()?;
        let document = self.project.document(&current.path)?;
        let text = sections::splice(&document.text, current, &self.buffer);
        let starts = crate::core::features::project::line_starts_of(&text);
        let start = starts
            .get(current.lines.start)
            .copied()
            .unwrap_or(text.len());
        Some((Document::new(current.path.clone(), text), start))
    }

    /// 複数ファイルにまたがる編集を当てる（クイックフィックス・辞書への登録）。今のファイルの範囲は
    /// [`Workbench::spliced_document`]（節エディタの中身を書き戻した形）の上の byte であること。
    pub fn apply_edit(&mut self, edit: &crate::core::features::WorkspaceEdit, now: f64) -> bool {
        // 節エディタと Slice の中身を先に書き戻す（書き戻した文書は spliced_document と同じ文字列なので、
        // 範囲はそのまま使える。書き戻さずに他のファイルだけを変えると、節を開き直したときに
        // 書き戻していない打鍵が消える）
        self.commit();
        if let Some(slice) = &self.slice {
            if edit.changes.iter().any(|change| change.path == slice.path) {
                self.slice = None;
                self.slice_buffer.clear();
            }
        }
        let before: Vec<_> = edit
            .changes
            .iter()
            .map(|c| (c.path.clone(), self.project.text(&c.path).to_owned()))
            .collect();
        let current = self.editing.clone();
        let mut touched = false;
        for change in &edit.changes {
            let base = self.project.text(&change.path).to_owned();
            let text = crate::core::features::apply_edits(&base, &change.edits);
            self.project
                .insert(Document::new(change.path.clone(), text));
            self.unsaved.insert(change.path.clone());
            touched = true;
        }
        if !touched {
            return false;
        }
        let after = before
            .iter()
            .map(|(p, _)| (p.clone(), self.project.text(p).to_owned()))
            .collect();
        self.workspace_undo.push((before, after));
        if self.workspace_undo.len() > 100 {
            self.workspace_undo.remove(0);
        }
        self.workspace_redo.clear();
        self.rebuild();
        if let Some(current) = current {
            match sections::position_of(&self.list, &current.section.path, current.section.id) {
                Some(index) => self.enter(index),
                None => self.editing = Some(current),
            }
        }
        self.save.edited(now);
        true
    }

    /// 複数ファイルの置換は、取り消しの 1 単位として扱う。後からした編集を上書きしない。
    pub fn workspace_undo_ready(&self, redo: bool) -> bool {
        if self.human_active() {
            return false;
        }
        let stack = if redo {
            &self.workspace_redo
        } else {
            &self.workspace_undo
        };
        stack.last().is_some_and(|(before, after)| {
            let expected = if redo { before } else { after };
            expected.iter().all(|(p, t)| self.project.text(p) == t)
                && self.current().is_none_or(|section| {
                    sections::text_of(self.project.text(&section.path), section) == self.buffer
                })
        })
    }
    pub fn undo_workspace(&mut self, redo: bool, now: f64) -> bool {
        if !self.workspace_undo_ready(redo) {
            return false;
        }
        let item = if redo {
            self.workspace_redo.pop()
        } else {
            self.workspace_undo.pop()
        }
        .unwrap();
        let target = if redo { &item.1 } else { &item.0 };
        let current = self.editing.clone();
        for (path, text) in target {
            self.project
                .insert(Document::new(path.clone(), text.clone()));
            self.unsaved.insert(path.clone());
        }
        self.rebuild();
        if let Some(current) = current {
            if let Some(index) =
                sections::position_of(&self.list, &current.section.path, current.section.id)
            {
                self.enter(index);
            }
        }
        if redo {
            self.workspace_undo.push(item);
        } else {
            self.workspace_redo.push(item);
        }
        self.save.edited(now);
        true
    }

    /// 外部の版で置き換える（§8.5 の差分モーダルの「外部の版を読み込む」）。
    pub fn take_external(&mut self, path: &str, text: Option<String>) {
        self.unsaved.remove(path);
        self.conflicts.remove(path);
        let editing_here = self
            .editing
            .as_ref()
            .is_some_and(|editing| editing.section.path == path);
        if self.slice.as_ref().is_some_and(|slice| slice.path == path) {
            // Slice の未保存の編集も捨てる（閉じて、開いた側が鍵から開き直す）
            self.slice = None;
            self.slice_buffer.clear();
        }
        if editing_here {
            // 節エディタの中身は外部の版で読み直す（未保存の編集は捨てる）
            if let Some(current) = self.editing.as_ref() {
                if let Some(document) = self.project.document(path) {
                    self.buffer = sections::text_of(&document.text, &current.section);
                }
            }
        }
        self.external_change(path, text);
    }

    /// 自分の版を残す（§8.5 の差分モーダル。次の保存で外部の版を上書きする）。
    pub fn keep_mine(&mut self, path: &str, now: f64) {
        self.conflicts.remove(path);
        self.unsaved.insert(path.to_owned());
        self.save.edited(now);
        self.save.request_now();
    }

    // ------------------------------------------------------------ 本文以外の文書（B-126）

    /// 開いている [`Slice`]。
    pub fn slice(&self) -> Option<&Slice> {
        self.slice.as_ref()
    }

    /// [`Slice`] の中身（エディタに渡す）。
    pub fn slice_buffer(&self) -> &str {
        &self.slice_buffer
    }

    /// [`Slice`] の中身（egui の `TextEdit` に渡す）。変えたら [`Workbench::slice_edited`] を呼ぶ。
    pub fn slice_buffer_mut(&mut self) -> &mut String {
        &mut self.slice_buffer
    }

    /// 文書 `path` の行 `lines` を開く（前に開いていたものは書き戻す）。`tag` が同じものを開いて
    /// いれば何もしない。
    pub fn open_slice(&mut self, path: &str, lines: Range<usize>, tag: &str) {
        if self.slice.as_ref().is_some_and(|slice| slice.tag == tag) {
            return;
        }
        self.commit_slice();
        self.slice_buffer = sections::text_of_lines(self.project.text(path), &lines);
        self.slice_caret = None;
        self.slice = Some(Slice {
            path: path.to_owned(),
            lines,
            tag: tag.to_owned(),
        });
    }

    /// [`Slice`] を書き戻して閉じる。
    pub fn close_slice(&mut self) {
        self.commit_slice();
        self.slice = None;
        self.slice_buffer.clear();
        self.slice_caret = None;
    }

    /// [`Slice`] のエディタの選択範囲（エディタが毎フレーム知らせる。フォーカスが外れても最後の値を
    /// 覚えておく）。
    pub fn set_slice_caret(&mut self, range: Range<usize>) {
        self.slice_caret = Some(range);
    }

    /// [`Slice`] のエディタの最後の選択範囲。
    pub fn slice_caret(&self) -> Option<Range<usize>> {
        self.slice_caret.clone()
    }

    /// [`Slice`] の中身の選択範囲（無ければ末尾）を `text` に置き換える（`Ctrl+M` の `Enter`、§9.8.3）。
    /// 置き換えた後のキャレットの位置を返す。Slice を開いていなければ `None`。
    pub fn insert_in_slice(&mut self, text: &str, now: f64) -> Option<usize> {
        self.slice.as_ref()?;
        let length = self.slice_buffer.len();
        let range = self
            .slice_caret
            .clone()
            .filter(|range| {
                range.end <= length
                    && self.slice_buffer.is_char_boundary(range.start)
                    && self.slice_buffer.is_char_boundary(range.end)
            })
            .unwrap_or(length..length);
        let start = range.start.min(range.end);
        let end = range.start.max(range.end);
        self.slice_buffer.replace_range(start..end, text);
        let caret = start + text.len();
        self.slice_caret = Some(caret..caret);
        self.slice_edited(now);
        Some(caret)
    }

    /// [`Slice`] の中身の最後に 1 行足す（話プロットの `Ctrl+M` → `material:` の行、§10.2）。
    /// Slice を開いていなければ `false`。
    pub fn append_to_slice(&mut self, line: &str, now: f64) -> bool {
        if self.slice.is_none() {
            return false;
        }
        let buffer = &mut self.slice_buffer;
        while buffer.ends_with('\n') {
            buffer.pop();
        }
        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(line);
        let caret = buffer.len();
        self.slice_caret = Some(caret..caret);
        self.slice_edited(now);
        true
    }

    /// [`Slice`] のエディタで編集があった（§8.2 の Dirty とタイマ再起動）。
    pub fn slice_edited(&mut self, now: f64) {
        if let Some(slice) = &self.slice {
            self.unsaved.insert(slice.path.clone());
        }
        self.save.edited(now);
    }

    /// [`Slice`] のエディタの中身を書き戻した文書と、中身の先頭の byte 位置（補完・ハイライトの換算）。
    pub fn slice_document(&self) -> Option<(Document, usize)> {
        let slice = self.slice.as_ref()?;
        let text = sections::splice_lines(
            self.project.text(&slice.path),
            &slice.lines,
            &self.slice_buffer,
        );
        let starts = crate::core::features::project::line_starts_of(&text);
        let start = starts.get(slice.lines.start).copied().unwrap_or(text.len());
        Some((Document::new(slice.path.clone(), text), start))
    }

    /// 文書を 1 つ丸ごと置き換える（ワークスペースの操作: 節の挿入・アイデアの破棄など）。
    /// 開いている [`Slice`] は先に書き戻し、同じ文書なら閉じる（開いた側が鍵から開き直す）。
    /// すぐに保存する（操作の単位で保存、§8.2）。
    pub fn replace_document(&mut self, path: &str, text: String, now: f64) -> bool {
        if !self.put_document(path, text) {
            return false;
        }
        self.save.edited(now);
        self.save.request_now();
        true
    }

    /// [`Workbench::replace_document`] の、打鍵の単位の版（資料の審査フォーム、B-134）。保存は
    /// 節エディタと同じ自動保存の待ち時間の後（§8.2）。
    pub fn update_document(&mut self, path: &str, text: String, now: f64) -> bool {
        if !self.put_document(path, text) {
            return false;
        }
        self.save.edited(now);
        true
    }

    /// 文書を置き換える（保存の予約はしない）。中身が同じなら `false`。
    fn put_document(&mut self, path: &str, text: String) -> bool {
        self.commit();
        if self
            .project
            .document(path)
            .is_some_and(|found| found.text == text)
        {
            return false;
        }
        if self.slice.as_ref().is_some_and(|slice| slice.path == path) {
            self.slice = None;
            self.slice_buffer.clear();
            self.slice_caret = None;
        }
        let editing = self.editing.clone();
        self.project.insert(Document::new(path.to_owned(), text));
        self.unsaved.insert(path.to_owned());
        self.changed(path);
        if let Some(editing) = editing {
            if editing.section.path == path {
                match sections::position_of(&self.list, path, editing.section.id) {
                    Some(index) => self.enter(index),
                    None => {
                        self.editing = None;
                        self.buffer.clear();
                    }
                }
            }
        }
        true
    }

    /// 文書 `from` を `to` へ動かす（資料の採用で `inbox/` → `docs/`、§16.1）。`to` を書いてから
    /// `from` を消す（[`Workbench::take_removals`]）。`from` が無い・`to` が既にあるなら `false`。
    pub fn rename_document(&mut self, from: &str, to: &str, now: f64) -> bool {
        self.commit();
        if from == to || self.project.document(to).is_some() {
            return false;
        }
        let Some(document) = self.project.remove(from) else {
            return false;
        };
        if self.slice.as_ref().is_some_and(|slice| slice.path == from) {
            self.slice = None;
            self.slice_buffer.clear();
            self.slice_caret = None;
        }
        self.unsaved.remove(from);
        self.conflicts.remove(from);
        self.removals.insert(from.to_owned());
        self.removals.remove(to);
        self.project
            .insert(Document::new(to.to_owned(), document.text));
        self.unsaved.insert(to.to_owned());
        self.changed(to);
        self.save.edited(now);
        self.save.request_now();
        true
    }

    /// 書き込みの後で消すファイル（[`Workbench::rename_document`] の元）。取り出すと空になる。
    /// 消せなかったものは [`Workbench::removal_failed`] で戻す。
    pub fn remove_document(&mut self, path: &str, now: f64) -> bool {
        self.commit();
        if self.project.remove(path).is_none() {
            return false;
        }
        self.unsaved.remove(path);
        self.removals.insert(path.to_owned());
        self.rebuild();
        self.save.edited(now);
        self.save.request_now();
        true
    }

    pub fn take_removals(&mut self) -> Vec<String> {
        std::mem::take(&mut self.removals).into_iter().collect()
    }

    /// 消せなかったファイルを戻す（次の保存でもう一度消す）。
    pub fn removal_failed(&mut self, path: &str) {
        self.removals.insert(path.to_owned());
    }

    /// 保存の直前: 書き込む資料 MD の `updated_at` と `checksum` を索引に書く（§16.6「保存のたびに
    /// checksum を更新し、updated_at を書き換える」）。索引が変われば索引も書き込みに入る。
    pub fn touch_materials(&mut self, stamp: &str) {
        use crate::core::features::materials_ops;
        self.commit();
        let index_path = materials_ops::INDEX_PATH;
        let before = self.project.text(index_path).to_owned();
        let mut index = before.clone();
        for path in &self.unsaved {
            let is_material = self.project.document(path).is_some_and(|found| {
                found.format == crate::core::model::document::DocumentFormat::MaterialDoc
            });
            if !is_material {
                continue;
            }
            if let Some(updated) =
                materials_ops::touch(&index, path, stamp, self.project.text(path))
            {
                index = updated;
            }
        }
        if index != before {
            self.project
                .insert(Document::new(index_path.to_owned(), index));
            self.unsaved.insert(index_path.to_owned());
            self.changed(index_path);
        }
    }

    fn commit_slice(&mut self) {
        let Some(slice) = self.slice.as_mut() else {
            return;
        };
        let text = self.project.text(&slice.path);
        if sections::text_of_lines(text, &slice.lines) == self.slice_buffer {
            return;
        }
        let updated = sections::splice_lines(text, &slice.lines, &self.slice_buffer);
        let start = slice.lines.start;
        slice.lines = start..start + sections::line_count(&self.slice_buffer);
        let path = slice.path.clone();
        self.project.insert(Document::new(path.clone(), updated));
        self.unsaved.insert(path.clone());
        self.changed(&path);
    }

    /// 文書 `path` が変わった後の並び・世代の更新（本文なら並びを作り直す）。
    fn changed(&mut self, path: &str) {
        if self.project.document(path).map(|found| found.format)
            == Some(crate::core::model::document::DocumentFormat::Contents)
        {
            self.rebuild();
        } else {
            self.generation += 1;
        }
    }

    /// 書くべきファイルと中身（書き戻してから）。書き終えたら [`Workbench::write_finished`]。
    pub fn take_writes(&mut self) -> Vec<(String, String)> {
        self.commit();
        self.unsaved
            .iter()
            .filter_map(|path| {
                self.project
                    .document(path)
                    .map(|document| (path.clone(), document.text.clone()))
            })
            .collect()
    }

    /// 書き込みの結果（失敗したファイルとその内容）。全部成功なら Clean へ。
    pub fn write_finished(&mut self, now: f64, written: &[String], failures: &[(String, String)]) {
        for path in written {
            // 書いている間に同じファイルを編集していれば、未保存のまま残す
            let editing_here = self
                .editing
                .as_ref()
                .is_some_and(|editing| &editing.section.path == path);
            let still_dirty = editing_here
                && self.project.document(path).zip(self.current()).is_some_and(
                    |(document, section)| sections::text_of(&document.text, section) != self.buffer,
                );
            let slice_dirty = self.slice.as_ref().is_some_and(|slice| {
                &slice.path == path
                    && sections::text_of_lines(self.project.text(path), &slice.lines)
                        != self.slice_buffer
            });
            if !still_dirty && !slice_dirty {
                self.unsaved.remove(path);
            }
        }
        match failures.first() {
            Some((path, message)) => self.save.failed(now, format!("{path}: {message}")),
            None => self.save.succeeded(),
        }
    }

    /// 外部で変わったファイル（§8.5）。`text` が `None` なら消えた。
    pub fn external_change(&mut self, path: &str, text: Option<String>) -> Reaction {
        if self.unsaved.contains(path) {
            self.conflicts.insert(path.to_owned());
            return Reaction::Conflict;
        }
        let same = match (&text, self.project.document(path)) {
            (Some(new), Some(old)) => new == &old.text,
            (None, None) => true,
            _ => false,
        };
        if same {
            return Reaction::Unchanged;
        }
        match text {
            Some(text) => {
                self.project.insert(Document::new(path.to_owned(), text));
            }
            None => {
                self.project.remove(path);
            }
        }
        self.conflicts.remove(path);
        // 開いている Slice は閉じる（開いた側が鍵から開き直す）
        if self.slice.as_ref().is_some_and(|slice| slice.path == path) {
            self.slice = None;
            self.slice_buffer.clear();
        }
        // 編集中のファイルなら、同じ節（無ければ近い節）を開き直す
        let editing = self.editing.clone();
        self.rebuild();
        if let Some(editing) = editing {
            if editing.section.path == path {
                match sections::position_of(&self.list, path, editing.section.id) {
                    Some(index) => self.enter(index),
                    None => {
                        self.editing = None;
                        self.buffer.clear();
                    }
                }
            }
        }
        Reaction::Reloaded
    }

    /// 文字数（§9.6.1）: 節 / 話 / プロジェクト（空白除外）。節は節エディタの中身で数える。
    pub fn counts(&self) -> (usize, usize, usize) {
        let section = sections::count_chars(&self.buffer);
        let Some(current) = self.current() else {
            return (0, 0, self.counts.values().sum());
        };
        let document = self.project.document(&current.path);
        let committed = document
            .map(|found| sections::count_chars(&sections::text_of(&found.text, current)))
            .unwrap_or(0);
        if current.id.chapter == 0 || current.id.episode == 0 || self.human_active() {
            return (
                if self.human_active() {
                    committed
                } else {
                    section
                },
                self.counts.get(&current.path).copied().unwrap_or(0),
                self.counts.values().sum(),
            );
        }
        let episode = self.counts.get(&current.path).copied().unwrap_or(0) + section - committed;
        let project = self.counts.values().sum::<usize>() + section - committed;
        (section, episode, project)
    }

    /// 節の並びの見出し（パンくず）`01›04›02›03`。
    pub fn breadcrumb(&self) -> Option<String> {
        let current = self.current()?;
        let digits = self.project.digits();
        Some(
            current
                .id
                .display(digits)
                .split('-')
                .collect::<Vec<_>>()
                .join("›"),
        )
    }

    /// 今の節の ID（`ui.json` の `last_scene`）。
    pub fn last_scene(&self) -> Option<String> {
        let digits = self.project.digits();
        self.current().map(|current| current.id.display(digits))
    }

    fn enter(&mut self, index: usize) {
        self.finish_human(0.0);
        let Some(section) = self.list.get(index).cloned() else {
            return;
        };
        self.pending_header = None;
        self.buffer = self
            .project
            .document(&section.path)
            .map(|document| sections::text_of(&document.text, &section))
            .unwrap_or_default();
        self.editing = Some(Editing { section });
    }

    fn rebuild(&mut self) {
        self.list = sections::all(&self.project);
        self.counts.clear();
        for item in &self.list {
            if item.id.chapter == 0 || item.id.episode == 0 {
                continue;
            }
            let Some(document) = self.project.document(&item.path) else {
                continue;
            };
            *self.counts.entry(item.path.clone()).or_default() +=
                sections::count_chars(&sections::text_of(&document.text, item));
        }
        self.generation += 1;
    }

    /// 編集中の節の ID（テスト用の短い入口）。
    pub fn current_id(&self) -> Option<SceneId> {
        self.current().map(|current| current.id)
    }
}
