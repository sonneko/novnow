//! アプリケーション状態と egui 更新ループ。
//!
//! 対象 API: egui / eframe 0.27 系。
//! * `eframe::App::update(&mut self, ctx, frame)`
//! * `egui::{SidePanel, TopBottomPanel, CentralPanel}::show(ctx, ..)`
//! * `TextEditOutput.cursor_range: Option<CursorRange>`、`CursorRange.primary.ccursor.index`
//! * スタイルは `ctx.set_style` + `Visuals::dark_mode`
#![allow(dead_code)]

use crate::panes::{self, editor, file_tree, info_panel, FocusPane, TreeRow};
use crate::project::loader;
use crate::project::state::ProjectState;
use crate::project::watcher::{self, FileChanged};
use crate::text::buffer::TextBuffer;
use crate::text::tokenizer::{parse_header, parse_speaker, FileKind};
use crate::types::characters::Character;
use crate::types::materials::Material;
use crate::types::plots::SceneId;
use crate::types::words::Word;

use aho_corasick::AhoCorasick;
use egui::Context;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// 情報パネルに渡す推論結果。
#[derive(Debug, Clone, Default)]
pub struct ContextInfo {
    pub scene_id: Option<SceneId>,
    pub plot_text: String,
    pub characters: Vec<Character>,
    pub words: Vec<Word>,
    pub materials: Vec<Material>,
    pub warnings: Vec<String>,
}

/// 検索モード（Ctrl+F などで使用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchMode {
    None,
    Find,
    Replace,
    Jump,
}

pub struct NovnowApp {
    pub runtime: Runtime,
    pub state: ProjectState,

    /// 開いているファイルのバッファ（タブ切替を見越した複数バッファ）。
    pub buffers: HashMap<PathBuf, TextBuffer>,
    pub active_path: Option<PathBuf>,
    /// 最近開いたファイル（Ctrl+Tab 用）。
    pub recent: Vec<PathBuf>,

    /// ファイルツリー。
    pub tree_rows: Vec<TreeRow>,
    pub tree_selection: Option<usize>,
    pub hovered_tree_row: Option<usize>,
    pub hovered_path: Option<PathBuf>,

    pub focus: FocusPane,

    // エディタ下部の数式バー。
    pub editor_focus_line: usize,
    pub editor_bar_text: String,

    // 情報パネル（デバウンス付き推論）。
    pub context: Option<ContextInfo>,
    pub context_dirty: bool,
    pub context_stale: bool,
    pub last_cursor_change: Instant,
    pub auto_context: bool,

    // 検索・ジャンプ。
    pub search_mode: SearchMode,
    pub search_query: String,
    pub replace_query: String,

    // ステータスメッセージ。
    pub status: String,
    pub status_until: Instant,

    // 外部変更監視。
    pub watch_rx: Option<Receiver<FileChanged>>,

    // Aho-Corasick（用語検出）のキャッシュ。辞書変更時に再構築。 (AC matcher, patterns, pattern_idx -> word_key)
    pub ac: Option<(AhoCorasick, Vec<String>, Vec<String>)>,
}

impl NovnowApp {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Runtime, state: ProjectState) -> Self {
        configure_style(&cc.egui_ctx);
        let tree_rows = panes::build_rows(&state.contents_files, &state.plots_files);

        // 外部変更監視スレッド。
        let (tx, rx) = channel();
        watcher::spawn(state.root.clone(), tx);

        let mut app = Self {
            runtime,
            state,
            buffers: HashMap::new(),
            active_path: None,
            recent: Vec::new(),
            tree_rows,
            tree_selection: None,
            hovered_tree_row: None,
            hovered_path: None,
            focus: FocusPane::Editor,
            editor_focus_line: 0,
            editor_bar_text: String::new(),
            context: None,
            context_dirty: true,
            context_stale: false,
            last_cursor_change: Instant::now(),
            auto_context: true,
            search_mode: SearchMode::None,
            search_query: String::new(),
            replace_query: String::new(),
            status: String::new(),
            status_until: Instant::now(),
            watch_rx: Some(rx),
            ac: None,
        };
        app.rebuild_ac();
        app
    }

    // ---- バッファ操作 ------------------------------------------------------

    pub fn active_buffer(&self) -> Option<&TextBuffer> {
        self.active_path.as_ref().and_then(|p| self.buffers.get(p))
    }

    /// ファイルを開く（既に開いていれば切替のみ）。
    pub fn open_file(&mut self, path: &PathBuf) {
        if self.buffers.contains_key(path) {
            self.active_path = Some(path.clone());
            self.touch_recent(path);
            self.sync_editor_focus();
            self.mark_context_dirty();
            return;
        }

        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                self.set_status(format!("開けません: {e}"));
                return;
            }
        };
        let kind = classify(path);
        let buf = TextBuffer::new(path.clone(), kind, text);
        self.buffers.insert(path.clone(), buf);
        self.active_path = Some(path.clone());
        self.touch_recent(path);
        self.sync_editor_focus();
        self.mark_context_dirty();
        self.set_status(format!("開きました: {}", short(path)));
    }

    fn touch_recent(&mut self, path: &PathBuf) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.clone());
        if self.recent.len() > 16 {
            self.recent.truncate(16);
        }
    }

    /// エディタ下部バーの対象行をカーソル位置に合わせる。
    pub fn sync_editor_focus(&mut self) {
        if let Some(buf) = self.active_buffer() {
            let cursor_line = buf.cursor_line;
            let bar_text = buf.line_text(cursor_line);
            self.editor_focus_line = cursor_line;
            self.editor_bar_text = bar_text;
        }
        self.update_tree_selection_to_active();
    }

    fn update_tree_selection_to_active(&mut self) {
        if let Some(active) = self.active_path.clone() {
            if let Some(idx) = self
                .tree_rows
                .iter()
                .position(|r| r.path.as_ref() == Some(&active))
            {
                self.tree_selection = Some(idx);
            }
        }
    }

    /// 現在ファイルを保存。
    pub fn save_active(&mut self) {
        let Some(path) = self.active_path.clone() else {
            self.set_status("保存対象がありません");
            return;
        };
        let Some(buf) = self.buffers.get(&path) else {
            return;
        };
        let text = buf.rope.to_string();
        match std::fs::write(&path, text) {
            Ok(()) => {
                let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                if let Some(b) = self.buffers.get_mut(&path) {
                    b.dirty = false;
                    b.modified = mtime;
                }
                self.set_status(format!("保存しました: {}", short(&path)));
            }
            Err(e) => self.set_status(format!("保存失敗: {e}")),
        }
    }

    /// 全バッファ保存。
    pub fn save_all(&mut self) {
        let paths: Vec<PathBuf> = self
            .buffers
            .iter()
            .filter(|(_, b)| b.dirty)
            .map(|(p, _)| p.clone())
            .collect();
        let mut count = 0;
        for p in paths {
            if let Some(b) = self.buffers.get(&p) {
                if std::fs::write(&p, b.rope.to_string()).is_ok() {
                    count += 1;
                }
            }
            let mtime = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
            if let Some(b) = self.buffers.get_mut(&p) {
                b.dirty = false;
                b.modified = mtime;
            }
        }
        self.set_status(format!("{count} 件を保存しました"));
    }

    /// 下部バーの編集を Rope の対象行へ反映する。
    pub fn commit_bar_edit(&mut self) {
        let line = self.editor_focus_line;
        let new_text = self.editor_bar_text.clone();
        let path = match self.active_path.clone() {
            Some(p) => p,
            None => return,
        };
        if let Some(buf) = self.buffers.get_mut(&path) {
            buf.apply_line_edit(line, &new_text);
        }
        self.set_status(format!("行 {} を更新しました", line + 1));
        self.mark_context_dirty();
        self.sync_editor_focus();
    }

    /// 節単位スクロール（カーソル行を最寄りの節見出しへ移動）。
    pub fn jump_section(&mut self, dir: i32) {
        let (cur, headers) = {
            let Some(buf) = self.active_buffer() else {
                return;
            };
            let n = buf.line_count();
            let headers: Vec<usize> = (0..n)
                .filter(|&i| buf.line_text(i).trim_start().starts_with('@'))
                .collect();
            (buf.cursor_line, headers)
        };
        if headers.is_empty() {
            return;
        }
        let target = if dir < 0 {
            headers
                .iter()
                .rev()
                .find(|&&h| h < cur)
                .copied()
                .unwrap_or(headers[0])
        } else {
            headers
                .iter()
                .find(|&&h| h > cur)
                .copied()
                .unwrap_or(*headers.last().unwrap())
        };
        let path = match self.active_path.clone() {
            Some(p) => p,
            None => return,
        };
        if let Some(buf) = self.buffers.get_mut(&path) {
            buf.cursor_line = target;
            buf.cursor_col = 0;
        }
        self.sync_editor_focus();
        self.mark_context_dirty();
    }

    // ---- コンテキスト推論 --------------------------------------------------

    pub fn mark_context_dirty(&mut self) {
        self.context_dirty = true;
        self.last_cursor_change = Instant::now();
        self.context_stale = true;
    }

    /// カーソル位置の SceneId を特定（直近の上方向の見出し）。
    pub fn current_scene_id(&self) -> Option<SceneId> {
        let buf = self.active_buffer()?;
        let line = buf.cursor_line;
        for i in (0..=line).rev() {
            if i >= buf.line_count() {
                continue;
            }
            let t = buf.line_text(i);
            if let Some((id, _)) = parse_header(t.trim_start()) {
                return Some(id);
            }
        }
        None
    }

    fn rebuild_ac(&mut self) {
        let mut terms_with_keys: Vec<(String, String)> = Vec::new();
        for (key, w) in &self.state.words {
            for term in w.all_terms() {
                if !term.is_empty() {
                    terms_with_keys.push((term, key.clone()));
                }
            }
        }
        terms_with_keys.sort_by(|a, b| a.0.cmp(&b.0));
        terms_with_keys.dedup_by(|a, b| a.0 == b.0);

        if terms_with_keys.is_empty() {
            self.ac = None;
            return;
        }

        let terms: Vec<String> = terms_with_keys.iter().map(|(t, _)| t.clone()).collect();
        let word_keys: Vec<String> = terms_with_keys.into_iter().map(|(_, k)| k).collect();

        match AhoCorasick::new(terms.iter().map(String::as_str)) {
            Ok(ac) => self.ac = Some((ac, terms, word_keys)),
            Err(_) => self.ac = None,
        }
    }

    /// 情報パネル用のコンテキストを実際に計算する。
    fn compute_context(&self) -> ContextInfo {
        let mut info = ContextInfo {
            warnings: self.state.warnings.clone(),
            ..Default::default()
        };

        let sid = self.current_scene_id();
        info.scene_id = sid;

        // --- 対応プロット（超詳細） ---------------------------------------
        if let Some(sid) = sid {
            let node = self.state.plot(&sid).or_else(|| {
                self.state
                    .plot(&SceneId::new(sid.chapter, sid.episode, sid.scene, 0))
            });
            info.plot_text = match node {
                Some(n) => n.text.clone(),
                None => self.read_plot_from_disk(&sid).unwrap_or_default(),
            };
        }

        // --- カーソル行・アクティブファイル全体のテキストを収集 ------------
        let scan_text = self
            .active_buffer()
            .map(|buf| buf.rope.to_string())
            .unwrap_or_default();

        let mut direct_names: Vec<String> = Vec::new();
        if let Some(buf) = self.active_buffer() {
            let cur = buf.line_text(buf.cursor_line);
            if let Some((name, _)) = parse_speaker(&cur) {
                direct_names.push(name);
            }
        }

        // --- 登場キャラ（優先順位: カーソル行 > ファイル内話者 > 本文中の辞書名） --
        let mut seen: Vec<String> = Vec::new();
        for name in &direct_names {
            push_unique(&mut seen, name);
        }
        if let Some(buf) = self.active_buffer() {
            for i in 0..buf.line_count() {
                let t = buf.line_text(i);
                if let Some((name, _)) = parse_speaker(&t) {
                    push_unique(&mut seen, &name);
                }
            }
        }
        if !scan_text.is_empty() {
            for (name, ch) in &self.state.characters {
                for alias in ch.all_names() {
                    if !alias.is_empty() && scan_text.contains(&alias) {
                        push_unique(&mut seen, name);
                        break;
                    }
                }
            }
        }
        for name in seen {
            if let Some(c) = self.state.character(&name).cloned() {
                info.characters.push(c);
            }
        }

        // --- 用語（Aho-Corasick で検出: O(1) キー引継ぎ） --------------------
        if !scan_text.is_empty() {
            if let Some((ac, _terms, word_keys)) = &self.ac {
                let mut hit_terms: Vec<String> = Vec::new();
                for m in ac.find_overlapping_iter(&scan_text) {
                    let key = &word_keys[m.pattern().as_usize()];
                    push_unique(&mut hit_terms, key);
                }
                for t in hit_terms {
                    if let Some(w) = self.state.word(&t).cloned() {
                        info.words.push(w);
                    }
                }
            }
        }

        // --- 関連資料 -------------------------------------------------------
        if let Some(sid) = sid {
            let ep_key = sid.episode_key();
            for m in self.state.materials.values() {
                if m.path.contains(&ep_key) || m.path.contains(&sid.chapter_key()) {
                    info.materials.push(m.clone());
                }
            }
        }

        info
    }

    /// 起動時に構築されていない場合、plots ディレクトリまたは開いているバッファから直接読む。
    fn read_plot_from_disk(&self, sid: &SceneId) -> Option<String> {
        let path = crate::project::resolver::plots_path(&self.state.root, sid);
        let text = if let Some(buf) = self.buffers.get(&path) {
            buf.rope.to_string()
        } else {
            std::fs::read_to_string(path).ok()?
        };
        let file = loader::parse_plot_file(&sid.episode_key(), &text);
        for node in file.nodes {
            if let Some(id) = node.scene {
                if id.chapter == sid.chapter && id.episode == sid.episode && id.scene == sid.scene {
                    return Some(node.body);
                }
            }
        }
        None
    }

    /// デバウンス後にコンテキストを更新する（100ms）。
    fn maybe_update_context(&mut self) {
        if !self.context_dirty || !self.auto_context {
            return;
        }
        if self.last_cursor_change.elapsed() < Duration::from_millis(100) {
            return;
        }
        let info = self.compute_context();
        self.context = Some(info);
        self.context_dirty = false;
        self.context_stale = false;
    }

    fn force_context_update(&mut self) {
        let info = self.compute_context();
        self.context = Some(info);
        self.context_dirty = false;
        self.context_stale = false;
    }

    // ---- プロジェクト再読み込み -------------------------------------------

    fn reload_project(&mut self) {
        let root = self.state.root.clone();
        match self.runtime.block_on(loader::load_project(root)) {
            Ok(mut st) => {
                // 走査済みファイル一覧は維持する。
                st.contents_files = self.state.contents_files.clone();
                st.plots_files = self.state.plots_files.clone();
                self.state = st;
                self.tree_rows =
                    panes::build_rows(&self.state.contents_files, &self.state.plots_files);
                self.rebuild_ac();
                self.force_context_update();
                self.set_status("プロジェクト JSON を再読み込みしました");
            }
            Err(e) => self.set_status(format!("再読み込み失敗: {e}")),
        }
    }

    fn handle_external_changes(&mut self) {
        let mut changed: Vec<PathBuf> = Vec::new();
        if let Some(rx) = &self.watch_rx {
            while let Ok(ev) = rx.try_recv() {
                changed.push(ev.path);
            }
        }
        if changed.is_empty() {
            return;
        }
        for p in changed {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            match name {
                "characters.json" | "words.json" | "index.json" | "ideas.json" => {
                    self.reload_project();
                }
                _ => {
                    // 編集中ファイルが外部で変更された場合はカーソル位置を維持して再読込。
                    let Some(cur) = self.active_path.clone() else {
                        continue;
                    };
                    if cur != p {
                        continue;
                    }
                    let Some(buf) = self.buffers.get(&p) else {
                        continue;
                    };
                    if buf.dirty {
                        continue;
                    }
                    let disk_mtime = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
                    if let (Some(m1), Some(m2)) = (buf.modified, disk_mtime) {
                        if m1 == m2 {
                            // 自作保存による変更イベントのためスキップ
                            continue;
                        }
                    }
                    if let Ok(t) = std::fs::read_to_string(&p) {
                        let cursor = buf.cursor_line;
                        let kind = classify(&p);
                        let mut new_buf = TextBuffer::new(p.clone(), kind, t);
                        new_buf.cursor_line = cursor.min(new_buf.line_count().saturating_sub(1));
                        new_buf.modified = disk_mtime;
                        self.buffers.insert(p.clone(), new_buf);
                        self.set_status("外部変更を再読込しました");
                    }
                }
            }
        }
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_until = Instant::now() + Duration::from_secs(4);
    }

    // ---- ショートカット ----------------------------------------------------

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let modal = self.search_mode != SearchMode::None;
        let mut action = Action::None;

        ctx.input(|i| {
            let m = i.modifiers;
            let ctrl = m.ctrl || m.command;
            let shift = m.shift;
            if modal {
                if i.key_pressed(egui::Key::Escape) {
                    action = Action::Escape;
                }
                return;
            }
            if ctrl && i.key_pressed(egui::Key::O) {
                action = Action::OpenFolder;
            } else if ctrl && shift && i.key_pressed(egui::Key::S) {
                action = Action::SaveAll;
            } else if ctrl && i.key_pressed(egui::Key::S) {
                action = Action::Save;
            } else if ctrl && i.key_pressed(egui::Key::Tab) {
                action = Action::CycleRecent;
            } else if ctrl && i.key_pressed(egui::Key::G) {
                action = Action::Jump;
            } else if ctrl && i.key_pressed(egui::Key::F) {
                action = Action::Find;
            } else if ctrl && i.key_pressed(egui::Key::H) {
                action = Action::Replace;
            } else if ctrl && i.key_pressed(egui::Key::R) {
                action = Action::RefreshRight;
            } else if ctrl && i.key_pressed(egui::Key::Num1) {
                action = Action::Focus1;
            } else if ctrl && i.key_pressed(egui::Key::Num2) {
                action = Action::Focus2;
            } else if ctrl && i.key_pressed(egui::Key::Num3) {
                action = Action::Focus3;
            } else if ctrl && i.key_pressed(egui::Key::B) {
                action = Action::OpenPlot;
            } else if ctrl && i.key_pressed(egui::Key::M) {
                action = Action::OpenMaterial;
            } else if ctrl && shift && i.key_pressed(egui::Key::T) {
                action = Action::NewCharacter;
            } else if ctrl && i.key_pressed(egui::Key::T) {
                action = Action::NewWord;
            } else if i.key_pressed(egui::Key::F5) {
                action = Action::ReloadJson;
            } else if ctrl && i.key_pressed(egui::Key::ArrowUp) {
                action = Action::FocusUp;
            } else if ctrl && i.key_pressed(egui::Key::ArrowDown) {
                action = Action::FocusDown;
            } else if i.key_pressed(egui::Key::Escape) {
                action = Action::Escape;
            }
        });

        match action {
            Action::None => {}
            Action::Save => self.save_active(),
            Action::SaveAll => self.save_all(),
            Action::ReloadJson | Action::RefreshRight => self.reload_project(),
            Action::OpenFolder => self.open_folder_dialog(),
            Action::CycleRecent => self.cycle_recent(),
            Action::Jump => {
                self.search_mode = SearchMode::Jump;
                self.search_query.clear();
            }
            Action::Find => {
                self.search_mode = SearchMode::Find;
                self.search_query.clear();
            }
            Action::Replace => {
                self.search_mode = SearchMode::Replace;
                self.search_query.clear();
            }
            Action::Focus1 => self.focus = FocusPane::Tree,
            Action::Focus2 => self.focus = FocusPane::Editor,
            Action::Focus3 => self.focus = FocusPane::Info,
            Action::FocusUp => self.focus = self.focus.prev(),
            Action::FocusDown => self.focus = self.focus.next(),
            Action::OpenPlot => self.open_current_plot(),
            Action::OpenMaterial => self.show_material_info(),
            Action::NewWord => self.register_selection_as_word(false),
            Action::NewCharacter => self.register_selection_as_word(true),
            Action::Escape => self.search_mode = SearchMode::None,
        }
    }

    fn open_folder_dialog(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            let names = ["characters.json", "words.json"];
            let mut root = dir.clone();
            if !names.iter().any(|n| root.join(n).exists()) {
                // 直下サブディレクトリに 1 つだけプロジェクトがあればそれを採用する。
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.is_dir() && names.iter().all(|n| p.join(n).exists()) {
                            root = p;
                            break;
                        }
                    }
                }
            }
            self.load_new_root(root);
        }
    }

    fn load_new_root(&mut self, root: PathBuf) {
        match self.runtime.block_on(loader::load_project(root.clone())) {
            Ok(st) => {
                self.state = st;
                self.buffers.clear();
                self.active_path = None;
                self.recent.clear();
                self.tree_rows =
                    panes::build_rows(&self.state.contents_files, &self.state.plots_files);
                self.rebuild_ac();
                self.force_context_update();
                let (tx, rx) = channel();
                watcher::spawn(root, tx);
                self.watch_rx = Some(rx);
                let title = self.state.title.clone();
                self.set_status(format!("プロジェクトを開きました: {title}"));
            }
            Err(e) => self.set_status(format!("読み込み失敗: {e}")),
        }
    }

    fn cycle_recent(&mut self) {
        if self.recent.len() < 2 {
            return;
        }
        if let Some(active) = self.active_path.clone() {
            if let Some(pos) = self.recent.iter().position(|p| *p == active) {
                let p = self.recent.remove(pos);
                self.recent.push(p);
            }
        }
        if let Some(next) = self.recent.first().cloned() {
            self.open_file(&next);
        }
    }

    fn open_current_plot(&mut self) {
        if let Some(sid) = self.current_scene_id() {
            let p = crate::project::resolver::plots_path(&self.state.root, &sid);
            if p.exists() {
                self.open_file(&p);
                self.set_status(format!("プロットを開きました: {}", sid.episode_key()));
            } else {
                self.set_status(format!("プロットが見つかりません: {}", p.display()));
            }
        }
    }

    fn show_material_info(&mut self) {
        self.force_context_update();
        self.focus = FocusPane::Info;
        self.set_status("関連資料を右パネルに表示しました");
    }

    fn register_selection_as_word(&mut self, as_character: bool) {
        let sel = self.selected_or_current_text();
        let term = sel.trim().to_string();
        if term.is_empty() {
            self.set_status("選択テキストがありません");
            return;
        }
        let path = if as_character {
            self.state.root.join("characters.json")
        } else {
            self.state.root.join("words.json")
        };

        // 既存 JSON を読み、追記して保存する。
        let text = std::fs::read_to_string(&path).unwrap_or_else(|_| "[]".to_string());
        let mut value: serde_json::Value =
            serde_json::from_str(&text).unwrap_or(serde_json::json!([]));
        if !value.is_array() {
            value = serde_json::json!([value]);
        }
        if let Some(arr) = value.as_array_mut() {
            if as_character {
                arr.push(serde_json::json!({
                    "name": term,
                    "role": "",
                    "personality": "",
                    "speech": "",
                    "notes": "novnow から登録"
                }));
            } else {
                arr.push(serde_json::json!({
                    "term": term,
                    "reading": "",
                    "definition": "",
                    "category": ""
                }));
            }
        }
        if let Ok(pretty) = serde_json::to_string_pretty(&value) {
            if std::fs::write(&path, pretty).is_ok() {
                self.reload_project();
                self.set_status(format!(
                    "{} に登録しました: {}",
                    if as_character {
                        "characters.json"
                    } else {
                        "words.json"
                    },
                    term
                ));
            }
        }
    }

    /// 選択テキストがあればそれを返し、無ければ現在行を対象にする。
    fn selected_or_current_text(&self) -> String {
        if let Some(buf) = self.active_buffer() {
            if let Some(sel) = buf.selected_text() {
                if !sel.trim().is_empty() {
                    return sel;
                }
            }
            buf.line_text(buf.cursor_line)
        } else {
            String::new()
        }
    }

    fn apply_search(&mut self, replace: bool) {
        let q = self.search_query.clone();
        if q.is_empty() {
            self.search_mode = SearchMode::None;
            return;
        }
        let rep = self.replace_query.clone();
        let Some(path) = self.active_path.clone() else {
            self.search_mode = SearchMode::None;
            return;
        };

        // 借用を長引かせないよう、必要な情報は先にスナップショットする。
        let (text, cursor_line, cursor_col, hit_line) = match self.buffers.get(&path) {
            Some(buf) => {
                let text = buf.rope.to_string();
                // 現在カーソルのバイト位置を計算
                let start_line = buf.cursor_line.min(buf.rope.len_lines().saturating_sub(1));
                let line_start_char = buf.rope.line_to_char(start_line);
                let current_char_idx = (line_start_char + buf.cursor_col).min(buf.rope.len_chars());
                let current_byte_idx = buf.rope.char_to_byte(current_char_idx).min(text.len());

                // 次の検索位置: 現在カーソルが既に一致文字列の先頭にある場合は 1 バイト進めた位置から検索
                let search_from = if text[current_byte_idx..].starts_with(&q) {
                    current_byte_idx + q.len().max(1)
                } else {
                    current_byte_idx
                };

                // 現在位置以降を検索し、なければ文頭からラップアラウンド検索
                let hit_byte = if search_from < text.len() {
                    text[search_from..]
                        .find(&q)
                        .map(|p| search_from + p)
                        .or_else(|| text[..search_from].find(&q))
                } else {
                    text.find(&q)
                };

                let hit_line = hit_byte.map(|pos| {
                    let char_idx = text[..pos].chars().count();
                    buf.rope.char_to_line(char_idx.min(buf.rope.len_chars()))
                });
                (text, buf.cursor_line, buf.cursor_col, hit_line)
            }
            None => {
                self.search_mode = SearchMode::None;
                return;
            }
        };

        if replace {
            let new_text = text.replace(&q, &rep);
            if new_text != text {
                let kind = classify(&path);
                let mut nb = TextBuffer::new(path.clone(), kind, new_text);
                nb.cursor_line = cursor_line.min(nb.line_count().saturating_sub(1));
                nb.cursor_col = cursor_col;
                nb.dirty = true;
                self.buffers.insert(path.clone(), nb);
                self.mark_context_dirty();
                self.set_status(format!("置換しました: {q} → {rep}"));
            } else {
                self.set_status("一致なし");
            }
        } else if let Some(line) = hit_line {
            if let Some(b) = self.buffers.get_mut(&path) {
                b.cursor_line = line;
                b.cursor_col = 0;
            }
            self.sync_editor_focus();
            self.mark_context_dirty();
            self.set_status(format!("一致: {} 行目", line + 1));
        } else {
            self.set_status("一致なし");
        }
        self.search_mode = SearchMode::None;
    }

    fn apply_jump(&mut self) {
        let q = self.search_query.trim().to_string();
        if q.is_empty() {
            self.search_mode = SearchMode::None;
            return;
        }
        match q.parse::<SceneId>() {
            Ok(sid) => {
                let path = crate::project::resolver::contents_path(&self.state.root, &sid);
                if !path.exists() {
                    self.set_status(format!("ファイルがありません: {}", path.display()));
                    self.search_mode = SearchMode::None;
                    return;
                }
                self.open_file(&path);
                let target = self.active_buffer().and_then(|buf| {
                    (0..buf.line_count()).find(|&i| {
                        parse_header(buf.line_text(i).trim_start())
                            .map(|(id, _)| id.display() == sid.display())
                            .unwrap_or(false)
                    })
                });
                if let (Some(line), Some(path)) = (target, self.active_path.clone()) {
                    if let Some(buf) = self.buffers.get_mut(&path) {
                        buf.cursor_line = line;
                        buf.cursor_col = 0;
                    }
                }
                self.sync_editor_focus();
                self.mark_context_dirty();
                self.set_status(format!("ジャンプ: {}", sid.display()));
            }
            Err(e) => self.set_status(format!("SceneId として解釈できません: {e}")),
        }
        self.search_mode = SearchMode::None;
    }

    /// 検索 / 置換 / ジャンプのモーダル。
    fn show_modal(&mut self, ctx: &Context) {
        if self.search_mode == SearchMode::None {
            return;
        }
        let mode = self.search_mode.clone();
        let title = match mode {
            SearchMode::Find => "検索 (Ctrl+F)",
            SearchMode::Replace => "置換 (Ctrl+H)",
            SearchMode::Jump => "SceneId ジャンプ (Ctrl+G)",
            SearchMode::None => return,
        };

        let mut open = true;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("検索:");
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query).desired_width(260.0),
                    );
                    r.request_focus();
                });
                match mode {
                    SearchMode::Replace => {
                        ui.horizontal(|ui| {
                            ui.label("置換:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.replace_query)
                                    .desired_width(260.0),
                            );
                        });
                        ui.label(egui::RichText::new("Enter で置換 / Esc で閉じる").small().weak());
                    }
                    SearchMode::Jump => {
                        ui.label(
                            egui::RichText::new("例: 01-04-02 または 01-04-02-03")
                                .small()
                                .weak(),
                        );
                    }
                    _ => {
                        ui.label(
                            egui::RichText::new("Enter で最初の一致へ / Esc で閉じる")
                                .small()
                                .weak(),
                        );
                    }
                }
            });

        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if !open || esc {
            self.search_mode = SearchMode::None;
        } else if enter {
            match mode {
                SearchMode::Find => self.apply_search(false),
                SearchMode::Replace => self.apply_search(true),
                SearchMode::Jump => self.apply_jump(),
                SearchMode::None => {}
            }
        }
    }
}

#[derive(PartialEq)]
enum Action {
    None,
    Save,
    SaveAll,
    ReloadJson,
    RefreshRight,
    OpenFolder,
    CycleRecent,
    Jump,
    Find,
    Replace,
    Focus1,
    Focus2,
    Focus3,
    FocusUp,
    FocusDown,
    OpenPlot,
    OpenMaterial,
    NewWord,
    NewCharacter,
    Escape,
}

impl eframe::App for NovnowApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // 外部変更の取り込み。
        self.handle_external_changes();

        // ショートカット。
        self.handle_shortcuts(ctx);

        // モーダル入力。
        self.show_modal(ctx);

        // ---- 上メニューバー ---------------------------------------------
        egui::TopBottomPanel::top("menu-bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!("novnow — {}", self.state.title)).strong(),
                );
                ui.separator();
                ui.label(format!(
                    "フォーカス: {}",
                    match self.focus {
                        FocusPane::Tree => "1 ファイル",
                        FocusPane::Editor => "2 エディタ",
                        FocusPane::Info => "3 情報",
                    }
                ));
                ui.separator();
                ui.label(format!(
                    "カーソル: L{} / 節 {}",
                    self.active_buffer().map(|b| b.cursor_line + 1).unwrap_or(0),
                    self.current_scene_id()
                        .map(|s| s.display())
                        .unwrap_or_else(|| "—".into())
                ));
                ui.separator();
                if ui.button("ショートカット").clicked() {
                    self.status = shortcuts_help();
                    self.status_until = Instant::now() + Duration::from_secs(30);
                }
            });
        });

        // ---- 下ステータスバー -------------------------------------------
        egui::TopBottomPanel::bottom("status-bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let msg = if Instant::now() < self.status_until {
                    self.status.clone()
                } else if let Some(buf) = self.active_buffer() {
                    format!(
                        "{} ・ {} 行 ・ {} 文字 ・ {}",
                        buf.path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        buf.line_count(),
                        buf.char_count(),
                        if buf.dirty { "未保存" } else { "保存済み" }
                    )
                } else {
                    "ファイル未選択".to_string()
                };
                ui.label(egui::RichText::new(msg).small());
            });
        });

        // ---- 左カラム（上: ファイルリーダー / 下: エディタ） --------------
        egui::SidePanel::left("left")
            .resizable(true)
            .default_width((ctx.available_rect().width() * 0.42).max(320.0))
            .show(ctx, |ui| {
                let h = ui.available_height();
                egui::TopBottomPanel::top("tree-pane")
                    .resizable(true)
                    .default_height((h * 0.5).max(120.0))
                    .show_inside(ui, |ui| {
                        file_tree::show(self, ui);
                        file_tree::show_viewer(self, ui);
                    });
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    editor::show(self, ui);
                });
            });

        // ---- 右カラム（情報パネル） --------------------------------------
        egui::CentralPanel::default().show(ctx, |ui| {
            info_panel::show(self, ui);
        });

        // 情報パネル更新（100ms デバウンス）。
        self.maybe_update_context();

        if self.context_stale {
            ctx.request_repaint_after(Duration::from_millis(60));
        }
    }
}

fn push_unique(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}

fn short(p: &std::path::Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn classify(path: &PathBuf) -> FileKind {
    let comps: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    if comps.iter().any(|c| c == "contents") {
        FileKind::Contents
    } else if comps.iter().any(|c| c == "plots") {
        FileKind::Plots
    } else {
        FileKind::Plain
    }
}

fn configure_style(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.visuals.dark_mode = true;
    ctx.set_style(style);

    // 日本語グリフを持つシステムフォントを自動検出して追加する。
    // egui の既定フォントはラテン文字のみなので、漢字・ひらがな表示のために必要。
    if let Some(data) = find_jp_font() {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "jp".to_owned(),
            egui::FontData::from_owned(data),
        );
        fonts.families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "jp".to_owned());
        fonts.families
            .get_mut(&egui::FontFamily::Monospace)
            .unwrap()
            .push("jp".to_owned());
        ctx.set_fonts(fonts);
    }
}

/// 一般的な OS フォントパスから日本語フォントを探す。
fn find_jp_font() -> Option<Vec<u8>> {
    for path in [
        // Linux (Noto CJK)
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        // Windows
        "C:\\Windows\\Fonts\\meiryo.ttc",
        "C:\\Windows\\Fonts\\YuGothM.ttc",
        "C:\\Windows\\Fonts\\msgothic.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
    ] {
        if let Ok(data) = std::fs::read(path) {
            return Some(data);
        }
    }
    None
}

fn shortcuts_help() -> String {
    "Ctrl+O フォルダを開く / Ctrl+S 保存 / Ctrl+Shift+S 全保存 / Ctrl+↑↓ フォーカス移動 / \
     Ctrl+Tab 最近のファイル / Ctrl+G ジャンプ / Ctrl+F 検索 / Ctrl+H 置換 / Ctrl+R 再読込 / \
     Ctrl+1-3 ペイン / Ctrl+B プロットを開く / Ctrl+M 資料表示 / F5 JSON 再読込 / \
     Ctrl+T 用語登録 / Ctrl+Shift+T キャラ登録 / Alt+↑↓ 節スクロール"
        .to_string()
}
