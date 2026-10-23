//! Web の GUI（§13.3〜§13.5、`docs/history.md` B-156〜B-158）。
//!
//! - **プロジェクト選択モーダル**（§13.4）: IndexedDB（`adapters::storage_idb`）のプロジェクトの一覧・
//!   検索・新規作成・Zip の取り込み・名前の変更・削除。初回は `sample` を 1 つだけ作り、自動では
//!   開かない（§13.3.5）。IndexedDB が使えなければ全画面で E062 を出して止まる（§13.3.7）。
//! - **開いたプロジェクトの [`Host`]**（[`WebHost`]）: 書き込みは 1 本の列で順に IndexedDB へ送る
//!   （非同期）。保存の結果は後のフレームで [`Host::save_finished`] が返す。監査ログの追記が
//!   重ならず、保存の後に頼んだ Zip のエクスポートが保存の後の中身を読む。
//! - 非同期の仕事は `wasm_bindgen_futures::spawn_local` で走らせ、結果を `Rc<RefCell<…>>` に置いて
//!   再描画を頼む（egui は毎フレーム状態を読むだけ）。
//! - ブラウザの既定のキー操作（印刷・ページの保存・検索バー・戻る など）が novnow のショートカットを
//!   奪わないように止め、未保存の変更があるままタブを閉じようとしたら確認を出す。

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;

use egui::{Align2, Context, FontFamily, FontId, Frame, Key, Margin, Modifiers, RichText, Sense};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::adapters::clock_system::{self, SystemClock};
use crate::adapters::env_system::SystemEnv;
use crate::adapters::project_load;
use crate::adapters::storage_idb::{self, Idb, IdbStorage, Origin, ProjectRecord};
use crate::core::model::project::ProjectError;
use crate::core::policy::archive::Skipped;
use crate::core::policy::{ExportKind, ProjectPath};
use crate::ports::clock::Clock;
use crate::ports::dialog::ChosenPath;
use crate::ports::storage::{Storage, StorageError};
use crate::ui::app::{ExportFailure, Host, HostEvent, NovnowApp, Picker, SaveOutcome, Session};
use crate::ui::i18n::{self, Locale};
use crate::ui::settings::{LanguageChoice, UiSettings, UI_JSON};
use crate::ui::shell::{self, BrowserKeys};
use crate::ui::theme::{self, Mode};

// 未保存の変更があるか（タブを閉じるときの確認、`install_unload_guard`）。マクロの中の doc コメントは
// 文字列リテラルになって `tests/layering.rs` の「GUI の文言」の検査に掛かるので、ふつうのコメントにする
thread_local! {
    static UNSAVED: Cell<bool> = const { Cell::new(false) };
}

/// GUI を起動する（`main.rs` の wasm の `main`）。
pub fn start() {
    console_error_panic_hook::set_once();
    install_key_guard();
    install_unload_guard();
    let runner = eframe::WebRunner::new();
    spawn_local(async move {
        let locale = Locale::detect(None, &SystemEnv);
        let started = runner
            .start(
                "the_canvas_id",
                eframe::WebOptions::default(),
                Box::new(move |cc| {
                    let picker = WebPicker::new(cc.egui_ctx.clone());
                    Box::new(App(NovnowApp::new(
                        &cc.egui_ctx,
                        locale,
                        Box::new(picker),
                        None,
                    )))
                }),
            )
            .await;
        if let Err(error) = started {
            web_sys::console::error_1(&error);
        }
    });
}

struct App(NovnowApp);

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        let system_dark = frame
            .info()
            .system_theme
            .map(|theme| theme == eframe::Theme::Dark);
        copy_and_cut_keys(ctx);
        self.0.frame(ctx, system_dark);
        sync_text_agent(ctx.wants_keyboard_input());
        write_clipboard(ctx);
    }
}

/// `Ctrl+C` / `Ctrl+X` を egui のコピー / 切り取りにする（`docs/history.md` B-158）。eframe 0.27 の Web は
/// これを `web_sys_unstable_apis` の cfg のときだけ行うが、その cfg は今の web-sys と組み合わせると
/// eframe がコンパイルできない。貼り付け（`paste` イベント）は eframe が受ける。
fn copy_and_cut_keys(ctx: &Context) {
    ctx.input_mut(|input| {
        for event in &mut input.events {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            {
                if modifiers.command && !modifiers.alt && !modifiers.shift {
                    match key {
                        Key::C => *event = egui::Event::Copy,
                        Key::X => *event = egui::Event::Cut,
                        _ => {}
                    }
                }
            }
        }
    });
}

/// egui がコピーした文字（入力欄の選択・エクスポートの「コピー」）をクリップボードに書く
/// （`navigator.clipboard.writeText`。eframe 0.27 の Web は上の cfg が無いと書かない）。
fn write_clipboard(ctx: &Context) {
    let text = ctx.output(|output| output.copied_text.clone());
    if text.is_empty() {
        return;
    }
    if let Some(window) = web_sys::window() {
        let promise = window.navigator().clipboard().write_text(&text);
        spawn_local(async move {
            let _ = JsFuture::from(promise).await;
        });
    }
}

/// eframe の「テキストエージェント」（IME と文字の入力を受ける隠れた `<input>`）を、入力欄に
/// フォーカスがあるあいだ出しておく（`docs/history.md` B-158）。eframe 0.27 はマウスで入力欄を
/// クリックしたときだけ出すので、キーボードで節エディタへ移ったとき（`Ctrl+2`・開いた直後など）は
/// IME が使えず、日本語を打てなかった。入力欄から離れたら隠す（eframe の再フォーカスも止まる）。
fn sync_text_agent(typing: bool) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(agent) = document
        .get_element_by_id("egui_text_agent")
        .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
    else {
        return;
    };
    if typing {
        let focused = document
            .active_element()
            .is_some_and(|element| element.id() == "egui_text_agent");
        if agent.hidden() || !focused {
            agent.set_hidden(false);
            let _ = agent.focus();
        }
    } else if !agent.hidden() {
        agent.set_hidden(true);
    }
}

// ============================================================ ブラウザのキー操作・離脱

/// ブラウザの既定の動作が novnow のショートカットを奪わないよう、修飾キー付きのキーの既定の動作を
/// 止める（eframe が止めるのは `Tab` と `Ctrl+P` だけ。どれを止めるかは
/// [`shell::browser_default_blocked`]）。変換中の IME のキーには触れない。
fn install_key_guard() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let mac = web_sys::window()
        .and_then(|window| window.navigator().platform().ok())
        .is_some_and(|platform| platform.to_ascii_lowercase().contains("mac"));
    let guard =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
            if event.is_composing() || event.key_code() == 229 {
                return;
            }
            let keys = BrowserKeys {
                command: event.ctrl_key() || event.meta_key(),
                alt: event.alt_key(),
                shift: event.shift_key(),
                alt_graph: event.get_modifier_state("AltGraph"),
                mac,
            };
            if shell::browser_default_blocked(&event.key(), keys) {
                event.prevent_default();
            }
        });
    let _ = document.add_event_listener_with_callback("keydown", guard.as_ref().unchecked_ref());
    guard.forget();
}

/// 未保存の変更があるままタブを閉じようとしたら、ブラウザの確認を出す（§8.2。保存は 400ms の
/// 打鍵の切れ目なので、ふつうは出ない）。
fn install_unload_guard() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let guard = Closure::<dyn FnMut(web_sys::BeforeUnloadEvent)>::new(
        |event: web_sys::BeforeUnloadEvent| {
            if UNSAVED.with(Cell::get) {
                event.prevent_default();
                event.set_return_value("unsaved");
            }
        },
    );
    let _ = window.add_event_listener_with_callback("beforeunload", guard.as_ref().unchecked_ref());
    guard.forget();
}

fn now() -> String {
    clock_system::rfc3339_millis(SystemClock.now_millis())
}

fn message(error: &impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

// ============================================================ 開いたプロジェクト

/// 開いたプロジェクトの I/O（IndexedDB）。
struct WebHost {
    idb: Idb,
    storage: IdbStorage,
    name: String,
    ctx: Context,
    queue: Rc<RefCell<Queue>>,
}

/// 書き込みの列（1 本。前の仕事が終わってから次を始める）。
#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    running: bool,
    /// 終わった保存の結果（[`Host::save_finished`]）。
    saved: Option<SaveOutcome>,
    /// 後から知らせること（[`Host::take_events`]）。
    events: Vec<HostEvent>,
    /// 最後に空き容量を確かめた時刻（ミリ秒。§13.3.7 の W062 は 1 分に 1 回まで）。
    estimated_at: i64,
}

/// 列に積む仕事。
enum Job {
    /// 保存の外の書き込み（`ui.json`・`exports/`）。
    Write(String, String),
    /// 追記（監査ログ）。
    Append(String, String),
    /// 消す。
    Remove(String),
    /// 保存 1 回分（§8.3）。
    Save(Vec<(String, String)>, Vec<String>),
    /// Zip のエクスポート（§13.5。ダウンロード）。
    Zip(String),
}

impl WebHost {
    fn new(idb: Idb, storage: IdbStorage, name: String, ctx: Context) -> Self {
        Self {
            idb,
            storage,
            name,
            ctx,
            queue: Rc::default(),
        }
    }

    fn push(&self, job: Job) {
        let start = {
            let mut queue = self.queue.borrow_mut();
            queue.jobs.push_back(job);
            !std::mem::replace(&mut queue.running, true)
        };
        if !start {
            return;
        }
        let queue = Rc::clone(&self.queue);
        let idb = self.idb.clone();
        let storage = self.storage.clone();
        let ctx = self.ctx.clone();
        spawn_local(async move {
            loop {
                let next = queue.borrow_mut().jobs.pop_front();
                let Some(job) = next else {
                    break;
                };
                run(job, &idb, &storage, &queue).await;
                ctx.request_repaint();
            }
            queue.borrow_mut().running = false;
        });
    }
}

/// 列の仕事を 1 つ行う。
async fn run(job: Job, idb: &Idb, storage: &IdbStorage, queue: &Rc<RefCell<Queue>>) {
    let fail = |path: &str, text: String| {
        queue
            .borrow_mut()
            .events
            .push(HostEvent::WriteFailed(path.to_owned(), text));
    };
    match job {
        Job::Write(path, text) => match ProjectPath::new(&path) {
            Ok(target) => {
                if let Err(error) = storage.write(&target, &text).await {
                    fail(&path, message(&error));
                }
            }
            Err(error) => fail(&path, message(&error)),
        },
        Job::Append(path, text) => match ProjectPath::new(&path) {
            Ok(target) => {
                if let Err(error) = storage.append(&target, &text).await {
                    fail(&path, message(&error));
                }
            }
            Err(error) => fail(&path, message(&error)),
        },
        Job::Remove(path) => {
            if let Ok(target) = ProjectPath::new(&path) {
                match storage.remove(&target).await {
                    Ok(()) | Err(StorageError::NotFound(_)) => {}
                    Err(error) => fail(&path, message(&error)),
                }
            }
        }
        Job::Save(writes, removals) => {
            low_space(queue).await;
            let outcome = save(storage, writes, removals).await;
            queue.borrow_mut().saved = Some(outcome);
        }
        Job::Zip(name) => {
            let file = crate::adapters::zip::file_name(&name);
            let result = match idb.export_zip(storage.project_id()).await {
                Ok(bytes) => storage_idb::download(&file, &bytes).map_err(|error| message(&error)),
                Err(error) => Err(message(&error)),
            };
            queue.borrow_mut().events.push(match result {
                Ok(()) => HostEvent::ZipExported(file),
                Err(error) => HostEvent::ZipFailed(error),
            });
        }
    }
}

/// 保存 1 回分（[`Host::save`] の既定と同じ手順を非同期で）。
async fn save(
    storage: &IdbStorage,
    writes: Vec<(String, String)>,
    removals: Vec<String>,
) -> SaveOutcome {
    let parsed: Result<Vec<_>, _> = writes
        .iter()
        .map(|(p, t)| ProjectPath::new(p).map(|p| (p, t.clone())))
        .collect();
    let deleted: Result<Vec<_>, _> = removals.iter().map(|p| ProjectPath::new(p)).collect();
    let result = match (parsed, deleted) {
        (Ok(w), Ok(r)) => storage.write_batch(&w, &r).await.map_err(|e| message(&e)),
        _ => Err("invalid batch path".to_owned()),
    };
    match result {
        Ok(()) => SaveOutcome {
            written: writes.into_iter().map(|(p, _)| p).collect(),
            ..Default::default()
        },
        Err(error) => SaveOutcome {
            failures: writes
                .into_iter()
                .map(|(p, _)| (p, error.clone()))
                .collect(),
            removals_left: removals,
            ..Default::default()
        },
    }
}

/// 保存の前に空き容量を確かめる（§13.3.7: 残り 10MB 未満なら W062。1 分に 1 回まで）。
async fn low_space(queue: &Rc<RefCell<Queue>>) {
    let now = SystemClock.now_millis();
    if now - queue.borrow().estimated_at < 60_000 {
        return;
    }
    queue.borrow_mut().estimated_at = now;
    if let Some((quota, usage)) = storage_idb::estimate().await {
        if storage_idb::is_low_space(quota, usage) {
            queue.borrow_mut().events.push(HostEvent::LowSpace);
        }
    }
}

impl Host for WebHost {
    fn write(&mut self, path: &str, text: &str) -> Result<(), String> {
        // 結果は後で（失敗は `take_events` で知らせる）
        self.push(Job::Write(path.to_owned(), text.to_owned()));
        Ok(())
    }

    fn poll(&mut self) -> Vec<(String, Option<String>)> {
        // 別のタブでの変更は見ない（`docs/history.md` B-157）
        Vec::new()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn stamp(&self) -> String {
        let clock = SystemClock;
        clock_system::compact_stamp(clock.now_millis(), clock.utc_offset_minutes())
    }

    fn now(&self) -> String {
        clock_system::now_rfc3339(&SystemClock)
    }

    fn remove(&mut self, path: &str) -> Result<(), String> {
        self.push(Job::Remove(path.to_owned()));
        Ok(())
    }

    fn choose_export(&mut self, suggested: &str) -> Option<ChosenPath> {
        // 保存先はブラウザが決める（ダウンロード）。名前だけ渡す
        Some(ChosenPath::from_save_dialog(
            PathBuf::from(suggested),
            ExportKind::Text,
        ))
    }

    fn write_export(&mut self, chosen: ChosenPath, text: &str) -> Result<String, ExportFailure> {
        let name = chosen
            .path()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "export.txt".to_owned());
        let mime = if name.ends_with(".md") {
            "text/markdown;charset=utf-8"
        } else {
            "text/plain;charset=utf-8"
        };
        storage_idb::download_as(&name, text.as_bytes(), mime)
            .map(|()| name)
            .map_err(|error| ExportFailure::Failed(message(&error)))
    }

    fn append(&mut self, path: &str, text: &str) -> Result<(), String> {
        self.push(Job::Append(path.to_owned(), text.to_owned()));
        Ok(())
    }

    fn save(
        &mut self,
        writes: Vec<(String, String)>,
        removals: Vec<String>,
    ) -> Option<SaveOutcome> {
        self.push(Job::Save(writes, removals));
        None
    }

    fn save_finished(&mut self) -> Option<SaveOutcome> {
        self.queue.borrow_mut().saved.take()
    }

    fn take_events(&mut self) -> Vec<HostEvent> {
        std::mem::take(&mut self.queue.borrow_mut().events)
    }

    fn unsaved_changed(&mut self, unsaved: bool) {
        UNSAVED.with(|cell| cell.set(unsaved));
    }

    fn export_zip(&mut self, name: &str) -> Option<Result<Option<String>, String>> {
        self.push(Job::Zip(name.to_owned()));
        None
    }
}

/// 雛形（§14.5）のファイル一式（新規作成・足りないファイルの補充）。
fn template_files(title: &str) -> Vec<(ProjectPath, String)> {
    crate::core::template::new_project(title)
        .into_iter()
        .filter_map(|file| Some((ProjectPath::new(file.path).ok()?, file.text)))
        .collect()
}

/// 付録 C の `sample` プロジェクト一式（§13.3.5 初回投入）。
fn sample_files() -> Vec<(ProjectPath, String)> {
    crate::core::template::sample_project()
        .into_iter()
        .filter_map(|file| Some((ProjectPath::new(file.path).ok()?, file.text)))
        .collect()
}

/// 開けなかった理由。
enum OpenError {
    /// §4.3 の不備。
    Invalid(ProjectError),
    /// IndexedDB の失敗。
    Storage(String),
}

/// プロジェクトを読み込んで開く（§13.4）。
async fn open_project(
    idb: &Idb,
    record: &ProjectRecord,
    ctx: &Context,
    locale: Locale,
) -> Result<(Session, UiSettings), OpenError> {
    let storage = idb.storage(&record.id);
    let loaded = project_load::load(&storage)
        .await
        .map_err(|error| OpenError::Storage(message(&error)))?;
    loaded.validation.map_err(OpenError::Invalid)?;
    let settings = match ProjectPath::new(UI_JSON) {
        Ok(path) => storage
            .read(&path)
            .await
            .map(|text| UiSettings::parse(&text))
            .unwrap_or_default(),
        Err(_) => UiSettings::default(),
    };
    // プロジェクト ID の履歴（§13.3.4。失敗しても開くのは止めない）
    let _ = idb.push_recent(&record.id, &now()).await;
    let host = WebHost::new(idb.clone(), storage, record.name.clone(), ctx.clone());
    let mut session = Session::new(Box::new(host), loaded.project, &settings);
    // `prompts/` の欠けた既定ファイルを書き出す（U-7・§15.5.4。I050 は `prompts/` が無かったとき）
    let locale = match settings.language {
        LanguageChoice::Ja => Locale::Ja,
        LanguageChoice::En => Locale::En,
        LanguageChoice::System => locale,
    };
    session.prepare_agents(locale);
    Ok((session, settings))
}

/// 足りないファイルを雛形から補う（既存のファイルには触れない。§14.5 の `new` と同じ）。
async fn fill_missing(idb: &Idb, record: &ProjectRecord) -> Result<usize, StorageError> {
    let storage = idb.storage(&record.id);
    let present: BTreeSet<String> = storage
        .list()
        .await?
        .into_iter()
        .map(|entry| entry.path.as_str().to_owned())
        .collect();
    let mut count = 0;
    for (path, text) in template_files(&record.name) {
        if present.contains(path.as_str()) {
            continue;
        }
        storage.write(&path, &text).await?;
        count += 1;
    }
    Ok(count)
}

// ============================================================ プロジェクト選択モーダル（§13.4）

/// 一覧の 1 行。
#[derive(Debug, Clone)]
struct Row {
    record: ProjectRecord,
    /// 並びの鍵（開いた日時と更新日時の新しいほう。新しい順に並べる）。
    key: String,
    /// 大きさ（byte。数え終わるまで `None`）。
    size: Option<usize>,
}

/// 起動の段階。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Phase {
    /// DB を開いている。
    #[default]
    Starting,
    /// 一覧を出せる。
    Ready,
    /// IndexedDB が使えない（E062。LocalStorage へはフォールバックしない、§13.3.7）。
    Unavailable,
}

/// 開けなかったプロジェクト（§4.3 の不備。取り込んだ直後は「不備のまま取り込む / 取り消す」、§13.5）。
struct Invalid {
    record: ProjectRecord,
    error: ProjectError,
    imported: bool,
}

/// 非同期の仕事と画面で共有する状態。
#[derive(Default)]
struct Shared {
    phase: Phase,
    idb: Option<Idb>,
    rows: Vec<Row>,
    /// 一覧の下に出す知らせ（取り込みの結果・失敗）。
    notice: Option<String>,
    /// 開き終えたプロジェクト（次のフレームで `show` が返す）。
    opened: Option<(Session, UiSettings)>,
    /// 仕事中（重ねて始めない）。
    busy: bool,
    invalid: Option<Invalid>,
    first_visit: bool,
    /// 次に一覧で選ぶプロジェクト（作った・取り込んだもの）。
    select: Option<String>,
}

/// 名前を入力している操作。
enum Edit {
    None,
    /// `Ctrl+N` / `＋`: 新しいプロジェクトの名前。
    New(String),
    /// `F2`: 名前の変更（ID と入力中の名前）。
    Rename(String, String),
    /// `Delete`: 削除の確認（名前の入力が一致したら消す）。
    Delete(ProjectRecord, String),
}

/// Web のプロジェクト選択（§13.4）。
pub struct WebPicker {
    ctx: Context,
    shared: Rc<RefCell<Shared>>,
    started: bool,
    selected: usize,
    search: String,
    /// 次のフレームで検索欄にフォーカスする（起動直後と `/`）。
    focus_search: bool,
    edit: Edit,
    /// 名前の入力欄にフォーカスする（開いた最初のフレーム）。
    focus_edit: bool,
    /// 最後に描いたときの言語（`refresh` の知らせに使う）。
    locale: Locale,
}

impl WebPicker {
    fn new(ctx: Context) -> Self {
        Self {
            ctx,
            shared: Rc::default(),
            started: false,
            selected: 0,
            search: String::new(),
            focus_search: true,
            edit: Edit::None,
            focus_edit: false,
            locale: Locale::Ja,
        }
    }

    /// 非同期の仕事を 1 つ始める（仕事中なら始めない）。
    fn spawn<F>(&self, work: impl FnOnce(Idb, Rc<RefCell<Shared>>, Context) -> F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        let idb = {
            let mut shared = self.shared.borrow_mut();
            if shared.busy {
                return;
            }
            let Some(idb) = shared.idb.clone() else {
                return;
            };
            shared.busy = true;
            idb
        };
        let shared = Rc::clone(&self.shared);
        let ctx = self.ctx.clone();
        let future = work(idb, Rc::clone(&shared), ctx.clone());
        spawn_local(async move {
            future.await;
            shared.borrow_mut().busy = false;
            ctx.request_repaint();
        });
    }

    /// 起動（DB を開く → 永続化の要求 → サンプルの投入 → 一覧）。
    fn start(&self, locale: Locale) {
        let shared = Rc::clone(&self.shared);
        let ctx = self.ctx.clone();
        spawn_local(async move {
            let idb = match Idb::open().await {
                Ok(idb) => idb,
                Err(_) => {
                    shared.borrow_mut().phase = Phase::Unavailable;
                    ctx.request_repaint();
                    return;
                }
            };
            storage_idb::request_persist().await;
            let first_visit = idb
                .recent()
                .await
                .is_ok_and(|recent| recent.items.is_empty());
            let mut notice = None;
            // §13.3.5: 空なら `sample`（付録 C）を 1 つだけ作る（自動では開かない）
            if let Err(error) = idb.seed_sample(&sample_files()).await {
                notice = Some(i18n::format(
                    locale,
                    "project.pick.failed",
                    &[&message(&error)],
                ));
            }
            {
                let mut state = shared.borrow_mut();
                state.idb = Some(idb.clone());
                state.notice = notice;
                state.phase = Phase::Ready;
                state.first_visit = first_visit;
            }
            refresh(&shared, &idb, &ctx, locale).await;
        });
    }

    /// 今の絞り込みで見える行（`(全体の位置, 行)`）。
    fn visible(&self) -> Vec<Row> {
        let needle = self.search.trim().to_lowercase();
        self.shared
            .borrow()
            .rows
            .iter()
            .filter(|row| needle.is_empty() || row.record.name.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    }

    fn open(&self, record: ProjectRecord, locale: Locale) {
        self.spawn(move |idb, shared, ctx| async move {
            let result = open_project(&idb, &record, &ctx, locale).await;
            let mut state = shared.borrow_mut();
            match result {
                Ok(opened) => state.opened = Some(opened),
                Err(OpenError::Invalid(error)) => {
                    state.invalid = Some(Invalid {
                        record,
                        error,
                        imported: false,
                    });
                }
                Err(OpenError::Storage(error)) => {
                    state.notice = Some(i18n::format(locale, "project.pick.failed", &[&error]));
                }
            }
        });
    }

    fn create(&self, name: String, locale: Locale) {
        self.spawn(move |idb, shared, ctx| async move {
            let files = template_files(&name);
            match idb.new_project(&name, Origin::New, &files).await {
                Ok(record) => {
                    let result = open_project(&idb, &record, &ctx, locale).await;
                    let mut state = shared.borrow_mut();
                    match result {
                        Ok(opened) => state.opened = Some(opened),
                        Err(OpenError::Invalid(error)) => {
                            state.invalid = Some(Invalid {
                                record,
                                error,
                                imported: false,
                            });
                        }
                        Err(OpenError::Storage(error)) => {
                            state.notice =
                                Some(i18n::format(locale, "project.pick.failed", &[&error]));
                        }
                    }
                }
                Err(error) => {
                    shared.borrow_mut().notice = Some(i18n::format(
                        locale,
                        "project.pick.failed",
                        &[&message(&error)],
                    ));
                }
            }
        });
    }

    fn rename(&self, id: String, name: String, locale: Locale) {
        self.spawn(move |idb, shared, ctx| async move {
            if let Err(error) = idb.rename_project(&id, &name).await {
                shared.borrow_mut().notice = Some(i18n::format(
                    locale,
                    "project.pick.failed",
                    &[&message(&error)],
                ));
            }
            shared.borrow_mut().select = Some(id);
            refresh(&shared, &idb, &ctx, locale).await;
        });
    }

    fn delete(&self, record: ProjectRecord, locale: Locale) {
        self.spawn(move |idb, shared, ctx| async move {
            let notice = match idb.delete_project(&record.id).await {
                Ok(()) => i18n::format(locale, "project.pick.deleted", &[&record.name]),
                Err(error) => i18n::format(locale, "project.pick.failed", &[&message(&error)]),
            };
            shared.borrow_mut().notice = Some(notice);
            refresh(&shared, &idb, &ctx, locale).await;
        });
    }

    /// 足りないファイルを補ってから開く。
    fn fill_and_open(&self, record: ProjectRecord, locale: Locale) {
        self.spawn(move |idb, shared, ctx| async move {
            if let Err(error) = fill_missing(&idb, &record).await {
                shared.borrow_mut().notice = Some(i18n::format(
                    locale,
                    "project.pick.failed",
                    &[&message(&error)],
                ));
                return;
            }
            let result = open_project(&idb, &record, &ctx, locale).await;
            let mut state = shared.borrow_mut();
            match result {
                Ok(opened) => state.opened = Some(opened),
                Err(OpenError::Invalid(error)) => {
                    state.invalid = Some(Invalid {
                        record,
                        error,
                        imported: false,
                    });
                }
                Err(OpenError::Storage(error)) => {
                    state.notice = Some(i18n::format(locale, "project.pick.failed", &[&error]));
                }
            }
        });
    }

    /// Zip の取り込み（§13.5）: ファイルを選ばせ、新しいプロジェクトレコードに入れる。
    fn import(&self, locale: Locale) {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let Ok(input) = document
            .create_element("input")
            .map(|element| element.unchecked_into::<web_sys::HtmlInputElement>())
        else {
            return;
        };
        input.set_type("file");
        input.set_accept(".zip,application/zip");
        let _ = input.set_attribute("style", "display:none");
        if let Some(body) = document.body() {
            let _ = body.append_child(&input);
        }
        let shared = Rc::clone(&self.shared);
        let ctx = self.ctx.clone();
        let chosen = input.clone();
        let on_change = Closure::once(move |_: web_sys::Event| {
            let file = chosen.files().and_then(|files| files.get(0));
            chosen.remove();
            let Some(file) = file else {
                return;
            };
            let idb = {
                let mut state = shared.borrow_mut();
                let Some(idb) = state.idb.clone() else {
                    return;
                };
                if state.busy {
                    return;
                }
                state.busy = true;
                idb
            };
            spawn_local(async move {
                import_zip(&idb, &file, &shared, &ctx, locale).await;
                shared.borrow_mut().busy = false;
                ctx.request_repaint();
            });
        });
        input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        on_change.forget();
        input.click();
    }
}

/// 一覧を読み直す（大きさは後から 1 つずつ数える）。
async fn refresh(shared: &Rc<RefCell<Shared>>, idb: &Idb, ctx: &Context, locale: Locale) {
    let records = match idb.projects().await {
        Ok(records) => records,
        Err(error) => {
            shared.borrow_mut().notice = Some(i18n::format(
                locale,
                "project.pick.failed",
                &[&message(&error)],
            ));
            ctx.request_repaint();
            return;
        }
    };
    let recent = idb.recent().await.unwrap_or_default();
    shared.borrow_mut().first_visit = recent.items.is_empty();
    let mut rows: Vec<Row> = records
        .into_iter()
        .map(|record| {
            let opened = recent
                .items
                .iter()
                .find(|item| item.id == record.id)
                .map(|item| item.opened_at.clone())
                .unwrap_or_default();
            let key = opened.max(record.updated_at.clone());
            Row {
                record,
                key,
                size: None,
            }
        })
        .collect();
    rows.sort_by(|left, right| right.key.cmp(&left.key));
    let ids: Vec<String> = rows.iter().map(|row| row.record.id.clone()).collect();
    shared.borrow_mut().rows = rows;
    ctx.request_repaint();
    for id in ids {
        if let Ok(size) = idb.project_size(&id).await {
            if let Some(row) = shared
                .borrow_mut()
                .rows
                .iter_mut()
                .find(|row| row.record.id == id)
            {
                row.size = Some(size);
            }
        }
    }
    ctx.request_repaint();
}

/// 取り込みの本体（読み込み → 新しいレコード → §4.3 の検査 → 一覧）。
async fn import_zip(
    idb: &Idb,
    file: &web_sys::File,
    shared: &Rc<RefCell<Shared>>,
    ctx: &Context,
    locale: Locale,
) {
    let fail = |error: String| {
        shared.borrow_mut().notice = Some(i18n::format(locale, "project.pick.failed", &[&error]));
    };
    let bytes = match JsFuture::from(file.array_buffer()).await {
        Ok(buffer) => js_sys::Uint8Array::new(&buffer).to_vec(),
        Err(error) => return fail(format!("{error:?}")),
    };
    let file_name = file.name();
    let name = file_name
        .strip_suffix(".zip")
        .or_else(|| file_name.strip_suffix(".ZIP"))
        .unwrap_or(&file_name)
        .to_owned();
    let (record, skipped) = match idb.import_zip(&name, &bytes).await {
        Ok(imported) => imported,
        Err(error) => return fail(message(&error)),
    };
    let mut notice = i18n::format(locale, "project.pick.imported", &[&record.name]);
    if !skipped.is_empty() {
        notice.push_str(" / ");
        notice.push_str(&i18n::format(
            locale,
            "project.pick.import_skipped",
            &[&skipped_names(&skipped)],
        ));
    }
    let invalid = match project_load::load(&idb.storage(&record.id)).await {
        Ok(loaded) => loaded.validation.err(),
        Err(error) => return fail(message(&error)),
    };
    {
        let mut state = shared.borrow_mut();
        state.notice = Some(notice);
        state.select = Some(record.id.clone());
        state.invalid = invalid.map(|error| Invalid {
            record,
            error,
            imported: true,
        });
    }
    refresh(shared, idb, ctx, locale).await;
}

/// 取り込まなかったエントリの名前（多ければ先頭の 5 件と件数）。
fn skipped_names(skipped: &[Skipped]) -> String {
    let mut names: Vec<&str> = skipped.iter().map(|entry| entry.name.as_str()).collect();
    let more = names.len().saturating_sub(5);
    names.truncate(5);
    let mut text = names.join(", ");
    if more > 0 {
        text.push_str(&format!(" (+{more})"));
    }
    text
}

/// §4.3 の不備の文言。
fn invalid_text(locale: Locale, error: &ProjectError) -> String {
    match error {
        ProjectError::Incomplete { missing } => {
            let names: Vec<String> = missing
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            i18n::format(locale, "project.pick.incomplete", &[&names.join(", ")])
        }
        ProjectError::TooNew { found } => {
            i18n::format(locale, "project.pick.too_new", &[&found.to_string()])
        }
        ProjectError::Encoding { files } => {
            let names: Vec<String> = files
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            i18n::format(locale, "project.pick.encoding", &[&names.join(", ")])
        }
    }
}

/// 大きさの表示（`124 KB` / `2.1 MB`）。
fn size_text(bytes: usize) -> String {
    let kib = bytes as f64 / 1024.0;
    if kib < 1024.0 {
        format!("{} KB", kib.ceil() as u64)
    } else {
        format!("{:.1} MB", kib / 1024.0)
    }
}

/// 日付の表示（ローカル時刻の `YYYY-MM-DD`）。
fn date_text(rfc3339: &str) -> String {
    let Some(millis) = clock_system::parse_utc(rfc3339) else {
        return String::new();
    };
    let day = clock_system::yyyymmdd(millis, SystemClock.utc_offset_minutes());
    if day.len() == 8 {
        format!("{}-{}-{}", &day[0..4], &day[4..6], &day[6..8])
    } else {
        day
    }
}

impl Picker for WebPicker {
    fn first_visit(&self) -> bool {
        self.shared.borrow().first_visit
    }

    fn show(&mut self, ctx: &Context, locale: Locale, mode: Mode) -> Option<(Session, UiSettings)> {
        self.locale = locale;
        if !self.started {
            self.started = true;
            self.start(locale);
        }
        let opened = self.shared.borrow_mut().opened.take();
        if let Some(opened) = opened {
            self.edit = Edit::None;
            return Some(opened);
        }
        let colors = theme::palette(mode);
        let phase = self.shared.borrow().phase;
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(colors.bg_base)
                    .inner_margin(Margin::same(32.0)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(620.0);
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "project.pick.title"))
                            .font(FontId::new(18.0, FontFamily::Proportional))
                            .color(colors.fg_primary),
                    );
                    ui.add_space(16.0);
                    match phase {
                        Phase::Starting => {
                            ui.label(
                                RichText::new(i18n::text(locale, "project.pick.loading"))
                                    .color(colors.fg_muted),
                            );
                        }
                        Phase::Unavailable => {
                            // §13.3.7: 起動を中止し、状況と回避策を全画面で示す
                            let e062 = storage_idb::unavailable_notice();
                            ui.label(
                                RichText::new(i18n::diagnostic(locale, &e062)).color(colors.error),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(i18n::text(locale, "project.pick.unavailable"))
                                    .color(colors.fg_muted),
                            );
                        }
                        Phase::Ready => self.list(ui, ctx, locale, colors),
                    }
                });
            });
        None
    }

    fn open_folder(&mut self) {}

    fn refresh(&mut self) {
        self.edit = Edit::None;
        self.focus_search = true;
        let locale = self.locale;
        self.spawn(move |idb, shared, ctx| async move {
            refresh(&shared, &idb, &ctx, locale).await;
        });
    }

    fn escape(&mut self) {
        if !matches!(self.edit, Edit::None) {
            self.edit = Edit::None;
            self.focus_search = true;
            return;
        }
        let mut shared = self.shared.borrow_mut();
        if shared
            .invalid
            .as_ref()
            .is_some_and(|invalid| !invalid.imported)
        {
            shared.invalid = None;
        }
    }
}

impl WebPicker {
    /// 一覧・検索欄・ボタン・入力中の操作。
    fn list(&mut self, ui: &mut egui::Ui, ctx: &Context, locale: Locale, colors: theme::Palette) {
        let select = self.shared.borrow_mut().select.take();
        if let Some(id) = select {
            self.search.clear();
            if let Some(index) = self.visible().iter().position(|row| row.record.id == id) {
                self.selected = index;
            }
        }
        let rows = self.visible();
        self.selected = self.selected.min(rows.len().saturating_sub(1));
        let busy = self.shared.borrow().busy;
        let editing = !matches!(self.edit, Edit::None);
        let blocked = self.shared.borrow().invalid.is_some();
        // キー（§13.4）: ↑/↓ 選択・Enter 開く・/ 検索・Ctrl+N 新規・Ctrl+I 取り込み・F2 名前の変更・Delete 削除
        let mut open: Option<ProjectRecord> = None;
        if !editing && !blocked {
            let (down, up, enter, new, import, rename, delete) = ctx.input_mut(|input| {
                (
                    input.consume_key(Modifiers::NONE, Key::ArrowDown),
                    input.consume_key(Modifiers::NONE, Key::ArrowUp),
                    input.consume_key(Modifiers::NONE, Key::Enter),
                    input.consume_key(Modifiers::COMMAND, Key::N),
                    input.consume_key(Modifiers::COMMAND, Key::I),
                    input.consume_key(Modifiers::NONE, Key::F2),
                    input.consume_key(Modifiers::NONE, Key::Delete),
                )
            });
            if down && self.selected + 1 < rows.len() {
                self.selected += 1;
            }
            if up {
                self.selected = self.selected.saturating_sub(1);
            }
            if enter {
                open = rows.get(self.selected).map(|row| row.record.clone());
            }
            if new {
                self.edit = Edit::New(String::new());
                self.focus_edit = true;
            }
            if import {
                self.import(locale);
            }
            if rename {
                if let Some(row) = rows.get(self.selected) {
                    self.edit = Edit::Rename(row.record.id.clone(), row.record.name.clone());
                    self.focus_edit = true;
                }
            }
            if delete {
                if let Some(row) = rows.get(self.selected) {
                    self.edit = Edit::Delete(row.record.clone(), String::new());
                    self.focus_edit = true;
                }
            }
            // `/` で検索欄へ（検索欄の外で打ったとき）
            let search_id = egui::Id::new("web-picker-search");
            let slash = !ctx.memory(|memory| memory.has_focus(search_id))
                && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Slash));
            if slash {
                self.focus_search = true;
            }
        }
        // 検索欄（autofocus）
        let search = egui::TextEdit::singleline(&mut self.search)
            .id(egui::Id::new("web-picker-search"))
            .hint_text(i18n::text(locale, "project.pick.search"))
            .desired_width(f32::INFINITY);
        let response = ui.add_enabled(!editing && !blocked, search);
        if self.focus_search && !editing && !blocked {
            response.request_focus();
            self.focus_search = false;
        }
        if response.changed() {
            self.selected = 0;
        }
        ui.add_space(12.0);
        if self.shared.borrow().first_visit {
            ui.label(i18n::text(locale, "help.welcome"));
            crate::ui::widgets::external_link(
                ui,
                i18n::text(locale, "help.website"),
                crate::core::model::project::PROJECT_STRUCTURE_DOC_URL,
                &colors,
            );
        }
        if rows.is_empty() {
            ui.label(
                RichText::new(i18n::text(locale, "project.pick.empty")).color(colors.fg_faint),
            );
        }
        let width = ui.available_width();
        for (index, row) in rows.iter().enumerate() {
            let height = 28.0;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
            let selected = index == self.selected;
            if selected {
                ui.painter().rect_filled(rect, 2.0, colors.accent);
            } else if response.hovered() && !editing && !blocked {
                ui.painter()
                    .rect_filled(rect, 2.0, colors.bg_selection.gamma_multiply(0.6));
            }
            if response.hovered() && !editing && !blocked {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let color = if selected {
                colors.bg_base
            } else {
                colors.fg_primary
            };
            let font = FontId::new(14.0, FontFamily::Proportional);
            let middle = rect.left_center() + egui::vec2(10.0, 0.0);
            let name_rect = ui.painter().text(
                middle,
                Align2::LEFT_CENTER,
                &row.record.name,
                font.clone(),
                color,
            );
            ui.painter().line_segment(
                [name_rect.left_bottom(), name_rect.right_bottom()],
                egui::Stroke::new(0.6_f32, color),
            );
            let size = row.size.map(size_text).unwrap_or_default();
            ui.painter().text(
                rect.right_center() - egui::vec2(10.0, 0.0),
                Align2::RIGHT_CENTER,
                size,
                font.clone(),
                color,
            );
            ui.painter().text(
                rect.right_center() - egui::vec2(110.0, 0.0),
                Align2::RIGHT_CENTER,
                date_text(&row.record.updated_at),
                font,
                color,
            );
            if response.clicked() && !editing && !blocked {
                self.selected = index;
            }
            if response.double_clicked() && !editing && !blocked {
                open = Some(row.record.clone());
            }
        }
        ui.add_space(16.0);
        // 新規 / Zip 読み込み（アイコンだけ。名前はツールチップ）
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 120.0).max(0.0) / 2.0);
            let enabled = !editing && !blocked && !busy;
            let new = ui
                .add_enabled(
                    enabled,
                    egui::Button::new(RichText::new("＋").underline())
                        .min_size(egui::Vec2::splat(24.0)),
                )
                .on_hover_text(i18n::text(locale, "project.pick.new"));
            if enabled && new.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if new.clicked() {
                self.edit = Edit::New(String::new());
                self.focus_edit = true;
            }
            ui.add_space(12.0);
            let import = ui
                .add_enabled(
                    enabled,
                    egui::Button::new(RichText::new("📥").underline())
                        .min_size(egui::Vec2::splat(24.0)),
                )
                .on_hover_text(i18n::text(locale, "project.pick.import"));
            if enabled && import.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if import.clicked() {
                self.import(locale);
            }
        });
        ui.add_space(8.0);
        ui.label(
            RichText::new(i18n::text(locale, "project.pick.keys"))
                .small()
                .color(colors.fg_faint),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(i18n::text(locale, "project.pick.single_tab"))
                .small()
                .color(colors.fg_faint),
        );
        if busy {
            ui.add_space(8.0);
            ui.label(
                RichText::new(i18n::text(locale, "project.pick.working")).color(colors.fg_muted),
            );
        }
        self.edit_panel(ui, ctx, locale, colors);
        self.invalid_panel(ui, locale, colors);
        let notice = self.shared.borrow().notice.clone();
        if let Some(notice) = notice {
            ui.add_space(12.0);
            ui.label(RichText::new(notice).color(colors.fg_muted));
        }
        if let Some(record) = open {
            self.open(record, locale);
        }
    }

    /// 名前の入力（新規・名前の変更・削除の確認）。
    fn edit_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &Context,
        locale: Locale,
        colors: theme::Palette,
    ) {
        let (label, expected, button) = match &self.edit {
            Edit::None => return,
            Edit::New(_) => (
                i18n::text(locale, "project.pick.new_name").to_owned(),
                None,
                i18n::text(locale, "project.pick.ok"),
            ),
            Edit::Rename(..) => (
                i18n::text(locale, "project.pick.rename").to_owned(),
                None,
                i18n::text(locale, "project.pick.ok"),
            ),
            Edit::Delete(record, _) => (
                i18n::format(locale, "project.pick.delete_confirm", &[&record.name]),
                Some(record.name.clone()),
                i18n::text(locale, "project.pick.delete"),
            ),
        };
        let focus = std::mem::take(&mut self.focus_edit);
        let mut submit = false;
        let mut cancel = false;
        let enter = ctx.input(|input| input.key_pressed(Key::Enter));
        let value = match &mut self.edit {
            Edit::New(value) | Edit::Rename(_, value) | Edit::Delete(_, value) => value,
            Edit::None => return,
        };
        ui.add_space(16.0);
        Frame::none()
            .fill(colors.bg_inset)
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(label).color(colors.fg_primary));
                ui.add_space(6.0);
                let response = ui.add(
                    egui::TextEdit::singleline(value)
                        .id(egui::Id::new("web-picker-name"))
                        .desired_width(f32::INFINITY),
                );
                if focus {
                    response.request_focus();
                }
                let ok = match &expected {
                    Some(name) => value.trim() == name,
                    None => !value.trim().is_empty(),
                };
                if response.lost_focus() && enter && ok {
                    submit = true;
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let submit_button = ui.add_enabled(
                        ok,
                        egui::Button::new(RichText::new(button).underline())
                            .min_size(egui::Vec2::splat(24.0)),
                    );
                    if ok && submit_button.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if submit_button.clicked() {
                        submit = true;
                    }
                    if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.cancel"))
                        .clicked()
                    {
                        cancel = true;
                    }
                });
                // 入力が足りないまま Enter を押したら、入力欄にとどまる
                if response.lost_focus() && enter && !ok {
                    response.request_focus();
                }
            });
        if cancel {
            self.edit = Edit::None;
            self.focus_search = true;
            return;
        }
        if !submit {
            return;
        }
        match std::mem::replace(&mut self.edit, Edit::None) {
            Edit::New(name) => self.create(name.trim().to_owned(), locale),
            Edit::Rename(id, name) => self.rename(id, name.trim().to_owned(), locale),
            Edit::Delete(record, _) => self.delete(record, locale),
            Edit::None => {}
        }
        self.focus_search = true;
    }

    /// 開けなかったプロジェクト（§4.3）の知らせと選択肢。
    fn invalid_panel(&mut self, ui: &mut egui::Ui, locale: Locale, colors: theme::Palette) {
        let (record, error, imported) = {
            let shared = self.shared.borrow();
            let Some(invalid) = shared.invalid.as_ref() else {
                return;
            };
            (
                invalid.record.clone(),
                invalid.error.clone(),
                invalid.imported,
            )
        };
        let incomplete = matches!(error, ProjectError::Incomplete { .. });
        let mut fill = false;
        let mut cancel_import = false;
        let mut close = false;
        ui.add_space(16.0);
        Frame::none()
            .fill(colors.bg_inset)
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(&record.name).color(colors.fg_primary));
                ui.label(RichText::new(invalid_text(locale, &error)).color(colors.warn));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if incomplete
                        && crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "project.pick.fill"),
                        )
                        .clicked()
                    {
                        fill = true;
                    }
                    if imported {
                        if crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "project.pick.import_keep"),
                        )
                        .clicked()
                        {
                            close = true;
                        }
                        if crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "project.pick.import_cancel"),
                        )
                        .clicked()
                        {
                            cancel_import = true;
                        }
                    } else if crate::ui::widgets::action_button(
                        ui,
                        i18n::text(locale, "modal.close"),
                    )
                    .clicked()
                    {
                        close = true;
                    }
                });
            });
        if fill || cancel_import || close {
            self.shared.borrow_mut().invalid = None;
            self.focus_search = true;
        }
        if fill {
            self.fill_and_open(record, locale);
        } else if cancel_import {
            self.delete(record, locale);
        }
    }
}
