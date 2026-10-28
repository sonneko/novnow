//! 起動前の画面（§14.4 のプロジェクト選択）とデスクトップの Host（`ui` feature）。
//!
//! `Picker` は GUI の起動前の画面を抽象化する（デスクトップは `native::NativePicker`、Web は
//! `ui::web::WebPicker`）。`native` はファイルシステムを持つデスクトップの実装。

use std::path::PathBuf;

use egui::{Context, Frame, Margin, RichText};

use crate::app::settings::UiSettings;
use crate::ui::app::Session;
use crate::ui::i18n::Locale;
use crate::ui::theme::Mode;
use crate::{ui::i18n, ui::theme};

pub trait Picker {
    /// 描く。プロジェクトを開いたら `Some`。
    fn show(&mut self, ctx: &Context, locale: Locale, mode: Mode) -> Option<(Session, UiSettings)>;
    /// `Ctrl+O`（選択画面の中）。
    fn open_folder(&mut self);
    /// 選択画面に戻ってきた（一覧を読み直す）。
    fn refresh(&mut self) {}
    /// 選択画面での `Esc`（入力中の名前などをやめる。選択画面そのものは閉じない、§13.4）。
    fn escape(&mut self) {}
    /// この端末ではじめて起動したか（保存済みプロジェクト 0 件 / 履歴 0 件。§9.10 の自動で出る条件）。
    /// 判定できない実装は `false`（自動では出さない）。
    fn first_visit(&self) -> bool {
        false
    }
}

/// 起動時に何もしない `Picker`（テスト・Web の仮置き）。
#[derive(Debug, Default)]
pub struct NoPicker;

impl Picker for NoPicker {
    fn show(&mut self, ctx: &Context, locale: Locale, mode: Mode) -> Option<(Session, UiSettings)> {
        let colors = theme::palette(mode);
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(colors.bg_base)
                    .inner_margin(Margin::same(32.0)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "project.pick.title"))
                            .color(colors.fg_primary),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "project.pick.empty"))
                            .color(colors.fg_faint),
                    );
                });
            });
        None
    }

    fn open_folder(&mut self) {}
}

/// `PathBuf` を使う公開 API の型を固定する（`main.rs` から呼ぶ）。
pub type StartPath = Option<PathBuf>;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod native {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};

    use egui::{Context, FontFamily, FontId, Frame, Key, Margin, Modifiers, RichText, Vec2};

    use super::Picker;
    use crate::adapters::block_on;
    use crate::adapters::clock_system::SystemClock;
    use crate::adapters::dialog_rfd::RfdDialog;
    use crate::adapters::env_system::SystemEnv;
    use crate::adapters::mcp_stdio::StdioClient;
    use crate::adapters::project_load;
    use crate::adapters::recent_file::RecentFile;
    use crate::adapters::storage_fs::{FsGate, FsStorage};
    use crate::cli::{self, Cli, Terminal};
    use crate::core::model::project::ProjectError;
    use crate::core::policy::external_change::{ChangeKind, Snapshot};
    use crate::core::policy::ProjectPath;
    use crate::ports::clock::Clock;
    use crate::ports::dialog::{ChosenPath, Dialog};
    use crate::ports::env::Env;
    use crate::ports::mcp::McpClient;
    use crate::ui::agents::{Finished, Job};
    use crate::ui::app::{save_now, Host, NovnowApp, SaveOutcome, Session};
    use crate::ui::i18n::{self, Locale};
    use crate::ui::settings::{LanguageChoice, UiSettings, UI_JSON};
    use crate::ui::theme::{self, Mode};

    /// GUI を起動する（`main.rs` の `Outcome::Gui`）。`project` は起動前の検査を通ったディレクトリ。
    pub fn run(project: Option<PathBuf>) -> Result<(), String> {
        let env = SystemEnv;
        let env_locale = Locale::detect(None, &env);
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("novnow")
                .with_inner_size([1280.0, 800.0])
                .with_min_inner_size([640.0, 400.0]),
            follow_system_theme: true,
            // 規則 10: ウィンドウの位置・大きさを覚えない（persistence feature も使わない）
            persist_window: false,
            ..Default::default()
        };
        eframe::run_native(
            "novnow",
            options,
            Box::new(move |cc| {
                let picker = Box::new(NativePicker::new(env_locale));
                let opened = project.and_then(|root| open_project(&root).ok());
                Box::new(App(NovnowApp::new(
                    &cc.egui_ctx,
                    env_locale,
                    picker,
                    opened,
                )))
            }),
        )
        .map_err(|error| error.to_string())
    }

    struct App(NovnowApp);

    impl eframe::App for App {
        fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
            let system_dark = frame
                .info()
                .system_theme
                .map(|theme| theme == eframe::Theme::Dark);
            self.0.frame(ctx, system_dark);
        }
    }

    /// デスクトップのプロジェクト（`FsStorage` と外部変更の監視・MCP の呼び出し）。
    struct FsHost {
        storage: FsStorage,
        snapshot: Snapshot,
        mcp: Calls,
        probes: Calls,
    }

    /// MCP の呼び出し（1 回 1 スレッド。結果は通り道で UI のスレッドへ返す、§15.7.5）。
    struct Calls {
        next: u64,
        cancels: Vec<(u64, Arc<AtomicBool>)>,
        sender: mpsc::Sender<Finished>,
        receiver: mpsc::Receiver<Finished>,
    }

    impl Default for Calls {
        fn default() -> Self {
            let (sender, receiver) = mpsc::channel();
            Self {
                next: 1,
                cancels: Vec::new(),
                sender,
                receiver,
            }
        }
    }

    impl Host for FsHost {
        fn save(
            &mut self,
            writes: Vec<(String, String)>,
            removals: Vec<String>,
        ) -> Option<SaveOutcome> {
            if removals.is_empty() {
                return Some(save_now(self, writes, removals));
            }
            let parsed: Result<Vec<_>, _> = writes
                .iter()
                .map(|(p, t)| ProjectPath::new(p).map(|p| (p, t.clone())))
                .collect();
            let deleted: Result<Vec<_>, _> = removals.iter().map(|p| ProjectPath::new(p)).collect();
            let result = match (parsed, deleted) {
                (Ok(w), Ok(r)) => self.storage.write_batch(&w, &r),
                _ => Err("invalid batch path".to_owned()),
            };
            Some(match result {
                Ok(stamps) => {
                    for (p, stamp) in stamps {
                        self.snapshot.acknowledge(p, stamp);
                    }
                    SaveOutcome {
                        written: writes.into_iter().map(|(p, _)| p).collect(),
                        ..Default::default()
                    }
                }
                Err(error) => SaveOutcome {
                    failures: writes
                        .into_iter()
                        .map(|(p, _)| (p, error.clone()))
                        .collect(),
                    removals_left: removals,
                    ..Default::default()
                },
            })
        }

        fn write(&mut self, path: &str, text: &str) -> Result<(), String> {
            let path = ProjectPath::new(path).map_err(|error| format!("{error:?}"))?;
            let stamp = self
                .storage
                .write_bytes(&path, text.as_bytes())
                .map_err(|error| format!("{error:?}"))?;
            self.snapshot.acknowledge(path, stamp);
            Ok(())
        }

        fn poll(&mut self) -> Vec<(String, Option<String>)> {
            let Ok(changes) = block_on(project_load::poll(&self.storage, &mut self.snapshot))
            else {
                return Vec::new();
            };
            changes
                .into_iter()
                .filter(|change| project_load::is_document(&change.path))
                .map(|change| {
                    let text = match change.kind {
                        ChangeKind::Removed => None,
                        _ => self
                            .storage
                            .read_bytes(&change.path)
                            .ok()
                            .and_then(|bytes| String::from_utf8(bytes).ok())
                            .map(|text| {
                                crate::core::text::lexer::normalize_on_load(&text).into_owned()
                            }),
                    };
                    (change.path.as_str().to_owned(), text)
                })
                .collect()
        }

        fn name(&self) -> String {
            self.storage
                .root()
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        }

        fn stamp(&self) -> String {
            // ネイティブはタイムゾーンが分からないので UTC（`docs/history.md` B-92・B-123）
            let clock = SystemClock;
            crate::adapters::clock_system::compact_stamp(
                clock.now_millis(),
                clock.utc_offset_minutes(),
            )
        }

        fn now(&self) -> String {
            crate::adapters::clock_system::now_rfc3339(&SystemClock)
        }

        fn remove(&mut self, path: &str) -> Result<(), String> {
            let path = ProjectPath::new(path).map_err(|error| format!("{error:?}"))?;
            match block_on(crate::ports::storage::Storage::remove(&self.storage, &path)) {
                // もう無ければ消えたのと同じ（資料の採用で、元の inbox のファイルを外で消していた）
                Ok(()) | Err(crate::ports::storage::StorageError::NotFound(_)) => {}
                Err(error) => return Err(format!("{error:?}")),
            }
            // 自分で消したものを外部変更として拾わない（§8.5）
            self.snapshot.forget(&path);
            Ok(())
        }

        fn choose_export(&mut self, suggested: &str) -> Option<ChosenPath> {
            // 初期値はプロジェクトの `exports/`（まだ無ければプロジェクトのルート）。作りはしない
            let exports = self.storage.root().join("exports");
            let dir = if exports.is_dir() {
                exports
            } else {
                self.storage.root().to_path_buf()
            };
            block_on(RfdDialog.save_text(&dir.join(suggested)))
        }

        fn write_export(
            &mut self,
            chosen: ChosenPath,
            text: &str,
        ) -> Result<String, crate::ui::app::ExportFailure> {
            // 名前・履歴ファイルの判定を先に（違反を関門の口に渡すと debug ビルドは panic する）
            let gate = self.storage.gate().clone();
            if let Err(violation) = gate.check_chosen_export(&chosen) {
                return Err(crate::ui::app::ExportFailure::Refused(
                    violation.diagnostic(),
                ));
            }
            gate.write_chosen_export(chosen, text.as_bytes())
                .map(|path| cli::display_path(&path))
                .map_err(|error| crate::ui::app::ExportFailure::Failed(format!("{error:?}")))
        }

        fn append(&mut self, path: &str, text: &str) -> Result<(), String> {
            let path = ProjectPath::new(path).map_err(|error| format!("{error:?}"))?;
            block_on(crate::ports::storage::Storage::append(
                &self.storage,
                &path,
                text,
            ))
            .map(|_| ())
            .map_err(|error| format!("{error:?}"))
        }

        fn mcp_available(&self) -> bool {
            true
        }

        fn mcp_start(&mut self, job: Job) -> Option<u64> {
            let number = self.mcp.next;
            self.mcp.next += 1;
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&cancel);
            let sender = self.mcp.sender.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("novnow-mcp-{number}"))
                .spawn(move || {
                    let started = std::time::Instant::now();
                    let result = StdioClient.call(&job.server, &job.call, &flag);
                    let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let _ = sender.send((number, result, elapsed));
                });
            if spawned.is_err() {
                return None;
            }
            self.mcp.cancels.push((number, cancel));
            Some(number)
        }

        fn mcp_poll(&mut self) -> Vec<Finished> {
            let done: Vec<Finished> = self.mcp.receiver.try_iter().collect();
            self.mcp
                .cancels
                .retain(|(number, _)| !done.iter().any(|(finished, _, _)| finished == number));
            done
        }

        fn local_probe_start(&mut self, job: Job) -> Option<u64> {
            let number = self.probes.next;
            self.probes.next += 1;
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&cancel);
            let sender = self.probes.sender.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("novnow-mcp-{number}"))
                .spawn(move || {
                    let started = std::time::Instant::now();
                    let result = StdioClient.call(&job.server, &job.call, &flag);
                    let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let _ = sender.send((number, result, elapsed));
                });
            if spawned.is_err() {
                return None;
            }
            self.probes.cancels.push((number, cancel));
            Some(number)
        }

        fn local_probe_poll(&mut self) -> Vec<Finished> {
            let done: Vec<Finished> = self.probes.receiver.try_iter().collect();
            self.probes
                .cancels
                .retain(|(number, _)| !done.iter().any(|(finished, _, _)| finished == number));
            done
        }

        fn mcp_cancel(&mut self, job: u64) {
            if let Some((_, flag)) = self.mcp.cancels.iter().find(|(number, _)| *number == job) {
                flag.store(true, Ordering::Relaxed);
            }
        }

        fn env_var(&self, name: &str) -> Option<String> {
            SystemEnv.var(name)
        }

        fn occupied(&self, path: &str) -> bool {
            ProjectPath::new(path).is_ok_and(|path| self.storage.exists(&path))
        }

        fn export_zip(&mut self, name: &str) -> Option<Result<Option<String>, String>> {
            // 保存ダイアログの初期値は `exports/<名前>.zip`（§13.5。プロジェクトの外も選べる＝ §13.1.1 の特例）
            let initial = self
                .storage
                .absolute(&crate::adapters::zip::export_path(name));
            let Some(chosen) = block_on(RfdDialog.save_zip(&initial)) else {
                return Some(Ok(None));
            };
            Some(
                crate::adapters::zip::export_to_chosen(&self.storage, chosen)
                    .map(|path| Some(cli::display_path(&path)))
                    .map_err(|error| format!("{error:?}")),
            )
        }
    }

    /// ディレクトリを開く。開けなければ §14.2.2 と同じ表示の文字列。
    fn open_project(root: &Path) -> Result<(Session, UiSettings), String> {
        let gate = Arc::new(FsGate::new(&SystemEnv));
        let storage = FsStorage::open(gate.clone(), root).map_err(|error| format!("{error:?}"))?;
        let loaded =
            block_on(project_load::load(&storage)).map_err(|error| format!("{error:?}"))?;
        if let Err(error) = &loaded.validation {
            return Err(format!("{error:?}"));
        }
        let settings = ProjectPath::new(UI_JSON)
            .ok()
            .and_then(|path| storage.read_bytes(&path).ok())
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(|text| UiSettings::parse(&text))
            .unwrap_or_default();
        let snapshot = Snapshot::from_entries(storage.list_files().unwrap_or_default());
        // パス履歴（§13.2。失敗しても開くのは止めない）
        let _ = RecentFile::new(gate).push(storage.root(), &SystemClock);
        let host = FsHost {
            storage,
            snapshot,
            mcp: Calls::default(),
            probes: Calls::default(),
        };
        let mut session = Session::new(Box::new(host), loaded.project, &settings);
        // `prompts/` の欠けた既定ファイルを書き出す（U-7・§15.5.4。I050 は `prompts/` が無かったとき）
        let locale = match settings.language {
            LanguageChoice::Ja => Locale::Ja,
            LanguageChoice::En => Locale::En,
            LanguageChoice::System => Locale::detect(None, &SystemEnv),
        };
        session.prepare_agents(locale);
        Ok((session, settings))
    }

    /// 起動前の検査（`cli::open::precheck`）と同じ表示を文字列で受け取る。
    fn precheck_report(dir: &Path, locale: Locale) -> Result<PathBuf, (String, bool)> {
        let env = SystemEnv;
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let terminal = Terminal {
            stdin: false,
            stdout: false,
            stderr: false,
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let incomplete = {
            let gate = Arc::new(FsGate::new(&env));
            FsStorage::open(gate, dir)
                .ok()
                .and_then(|storage| block_on(project_load::listing(&storage)).ok())
                .map(|listing| {
                    matches!(
                        crate::core::model::project::validate(&listing),
                        Err(ProjectError::Incomplete { .. })
                    )
                })
                .unwrap_or(false)
        };
        let mut cli = Cli::new(&env, &mut out, &mut err, cwd, terminal);
        cli.set_locale(locale);
        let result = cli::open::precheck(&mut cli, dir);
        drop(cli);
        result.map_err(|_| (String::from_utf8_lossy(&err).into_owned(), incomplete))
    }

    /// `novnow new` と同じ雛形を作る（§14.4「ここに新規作成する」・「新規作成」）。
    fn create_here(dir: &Path, locale: Locale) -> Result<(), String> {
        let env = SystemEnv;
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let terminal = Terminal {
            stdin: false,
            stdout: false,
            stderr: false,
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut cli = Cli::new(&env, &mut out, &mut err, cwd, terminal);
        cli.set_locale(locale);
        let exit = cli::new::run(&mut cli, dir);
        drop(cli);
        if exit == cli::Exit::Ok {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&err).into_owned())
        }
    }

    /// パスの「最後に開いた日」は、見えている選択行のもの（§14.4）。
    fn recent_caption(path: &str, exists: bool, opened_at: &str, locale: Locale) -> String {
        if !exists {
            return format!("{}    {path}", i18n::text(locale, "project.pick.missing"));
        }
        let name = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_owned());
        format!(
            "{name}    {path}    {}",
            opened_at.get(..10).unwrap_or(opened_at)
        )
    }

    /// 移行のプレビューと確定で共通の変換（§14.6: 読み替え→候補生成→確定→描画）。
    /// 移行（§14.6）の 1 つの入力。
    struct MigrationInput {
        name: String,
        bytes: Vec<u8>,
        /// プレビューで切り替えた文字コード（`None` = 自動判定: UTF-8 優先）。
        encoding: Option<crate::core::text::encoding::SourceEncoding>,
    }

    /// 移行（§14.6）の状態: 選んだ入力と、編集できる分割候補。
    struct MigrationState {
        title: String,
        inputs: Vec<MigrationInput>,
        draft: crate::core::migration::Draft,
    }

    impl MigrationState {
        fn new(inputs: Vec<MigrationInput>) -> Self {
            let title = inputs
                .first()
                .map(|input| {
                    input
                        .name
                        .trim_end_matches(".txt")
                        .trim_end_matches(".md")
                        .to_owned()
                })
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| "untitled".to_owned());
            Self {
                draft: Self::draft_of(&inputs),
                title,
                inputs,
            }
        }

        /// 入力を読み替えて候補を作る（文字コードの切り替えで作成し直す）。
        fn draft_of(inputs: &[MigrationInput]) -> crate::core::migration::Draft {
            let mut units = Vec::new();
            for input in inputs {
                let decoded = match input.encoding {
                    Some(encoding) => {
                        crate::core::text::encoding::decode_as(&input.bytes, encoding)
                    }
                    None => crate::core::text::encoding::decode(&input.bytes),
                };
                let Ok(decoded) = decoded else {
                    return crate::core::migration::Draft::default();
                };
                units.push(crate::core::migration::SourceUnit {
                    name: input.name.clone(),
                    text: decoded.text,
                    starts_chapter: false,
                });
            }
            crate::core::migration::analyze(&units)
        }

        /// 候補から構造を確定してプロジェクトのファイルを組む。
        fn files(&self) -> Result<Vec<crate::core::template::TemplateFile>, String> {
            let book = crate::core::migration::structure(&self.draft);
            crate::core::migration::render(&self.title, &book).map_err(|e| format!("{e:?}"))
        }
    }

    /// §14.4 のプロジェクト選択（デスクトップ）。
    struct NativePicker {
        recent: Vec<(String, bool, String)>,
        selected: usize,
        problem: Option<(PathBuf, String, bool)>,
        want_folder: bool,
        want_new: bool,
        want_import: bool,
        want_migrate: bool,
        migration: Option<MigrationState>,
        locale: Locale,
    }

    impl NativePicker {
        fn new(locale: Locale) -> Self {
            let gate = Arc::new(FsGate::new(&SystemEnv));
            let recent = RecentFile::new(gate)
                .load()
                .items
                .into_iter()
                .map(|item| {
                    let exists = Path::new(&item.path).is_dir();
                    (item.path, exists, item.opened_at)
                })
                .collect();
            Self {
                recent,
                selected: 0,
                problem: None,
                want_folder: false,
                want_new: false,
                want_import: false,
                want_migrate: false,
                migration: None,
                locale,
            }
        }

        fn try_open(&mut self, dir: &Path) -> Option<(Session, UiSettings)> {
            match precheck_report(dir, self.locale) {
                Ok(root) => match open_project(&root) {
                    Ok(opened) => Some(opened),
                    Err(message) => {
                        self.problem = Some((root, message, false));
                        None
                    }
                },
                Err((report, incomplete)) => {
                    self.problem = Some((dir.to_path_buf(), report, incomplete));
                    None
                }
            }
        }
    }

    impl Picker for NativePicker {
        fn first_visit(&self) -> bool {
            self.recent.is_empty()
        }

        fn open_folder(&mut self) {
            self.want_folder = true;
        }

        fn refresh(&mut self) {
            *self = Self::new(self.locale);
        }

        fn show(
            &mut self,
            ctx: &Context,
            locale: Locale,
            mode: Mode,
        ) -> Option<(Session, UiSettings)> {
            self.locale = locale;
            let colors = theme::palette(mode);
            let mut opened = None;
            if self.want_migrate {
                self.want_migrate = false;
                if let Some(picked) = block_on(RfdDialog.pick_texts()) {
                    let inputs = picked
                        .into_iter()
                        .map(|file| MigrationInput {
                            name: file.name,
                            bytes: file.bytes,
                            encoding: None,
                        })
                        .collect();
                    self.migration = Some(MigrationState::new(inputs));
                }
            }
            let mut migration_confirm = false;
            let mut migration_cancel = false;
            if let Some(state) = &mut self.migration {
                egui::Window::new(i18n::text(locale, "stage.migrate"))
                    .collapsible(false)
                    .show(ctx, |ui| {
                        ui.label(i18n::text(locale, "stage.migrate_notice"));
                        ui.text_edit_singleline(&mut state.title);
                        // 入力（曖昧な文字コードはここで切り替えられる）
                        let mut encoding_change: Option<(
                            usize,
                            crate::core::text::encoding::SourceEncoding,
                        )> = None;
                        for (index, input) in state.inputs.iter().enumerate() {
                            let ambiguous = crate::core::text::encoding::decode(&input.bytes)
                                .is_ok_and(|decoded| decoded.ambiguous);
                            ui.horizontal(|ui| {
                                ui.monospace(&input.name);
                                if ambiguous {
                                    let current = match input.encoding {
                                        Some(encoding) => encoding,
                                        None => crate::core::text::encoding::SourceEncoding::Utf8,
                                    };
                                    for (name, encoding) in [
                                        (
                                            "UTF-8",
                                            crate::core::text::encoding::SourceEncoding::Utf8,
                                        ),
                                        (
                                            "Shift_JIS",
                                            crate::core::text::encoding::SourceEncoding::ShiftJis,
                                        ),
                                    ] {
                                        let active = current == encoding;
                                        if ui
                                            .add_enabled(!active, egui::Button::new(name))
                                            .clicked()
                                        {
                                            encoding_change = Some((index, encoding));
                                        }
                                    }
                                }
                            });
                        }
                        if let Some((index, encoding)) = encoding_change {
                            state.inputs[index].encoding = Some(encoding);
                            state.draft = MigrationState::draft_of(&state.inputs);
                        }
                        let book = crate::core::migration::structure(&state.draft);
                        let (chapters, episodes, scenes, sections) = book.counts();
                        let counts = [
                            chapters.to_string(),
                            episodes.to_string(),
                            scenes.to_string(),
                            sections.to_string(),
                        ];
                        let structure = i18n::text(locale, "stage.migrate_structure");
                        let structure = structure.replace("{0}", &counts[0]);
                        let structure = structure.replace("{1}", &counts[1]);
                        let structure = structure.replace("{2}", &counts[2]);
                        let structure = structure.replace("{3}", &counts[3]);
                        ui.label(structure);
                        // 分割候補（各候補を他の段または境界なしへ変更できる）
                        let mut boundary_change: Option<(
                            usize,
                            Option<crate::core::migration::Level>,
                        )> = None;
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .show(ui, |ui| {
                                for (at, element) in state.draft.elements.iter().enumerate() {
                                    let crate::core::migration::Element::Boundary(boundary) =
                                        element
                                    else {
                                        continue;
                                    };
                                    let context = if let Some(title) = &boundary.title {
                                        title.clone()
                                    } else {
                                        state
                                            .draft
                                            .elements
                                            .iter()
                                            .skip(at + 1)
                                            .find_map(|element| {
                                                if let crate::core::migration::Element::Line(line) =
                                                    element
                                                {
                                                    (!line.is_empty())
                                                        .then(|| line.chars().take(24).collect())
                                                } else {
                                                    None
                                                }
                                            })
                                            .unwrap_or_default()
                                    };
                                    ui.horizontal(|ui| {
                                        ui.monospace(context);
                                        for (key, level) in [
                                            (
                                                "stage.migrate_chapter",
                                                Some(crate::core::migration::Level::Chapter),
                                            ),
                                            (
                                                "stage.migrate_episode",
                                                Some(crate::core::migration::Level::Episode),
                                            ),
                                            (
                                                "stage.migrate_scene",
                                                Some(crate::core::migration::Level::Scene),
                                            ),
                                            (
                                                "stage.migrate_section",
                                                Some(crate::core::migration::Level::Section),
                                            ),
                                            ("stage.migrate_none", None),
                                        ] {
                                            let active = level == Some(boundary.level);
                                            if ui
                                                .add_enabled(
                                                    !active,
                                                    egui::Button::new(i18n::text(locale, key)),
                                                )
                                                .clicked()
                                            {
                                                boundary_change = Some((at, level));
                                            }
                                        }
                                    });
                                }
                            });
                        if let Some((at, level)) = boundary_change {
                            state.draft.set_boundary(at, level);
                        }
                        match state.files() {
                            Ok(_) => {
                                migration_confirm = crate::ui::widgets::action_button(
                                    ui,
                                    i18n::text(locale, "stage.confirm"),
                                )
                                .clicked();
                            }
                            Err(error) => {
                                ui.label(format!("{error:?}"));
                            }
                        }
                        migration_cancel = crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "stage.cancel"),
                        )
                        .clicked();
                    });
            }
            if migration_cancel {
                self.migration = None;
            }
            if migration_confirm {
                if let Some(state) = self.migration.take() {
                    if let Some(parent) = block_on(RfdDialog.pick_folder()) {
                        let safe = ProjectPath::new(&state.title)
                            .ok()
                            .filter(|p| !p.as_str().contains('/'));
                        if let Some(name) = safe {
                            let dir = parent.join(name.as_str());
                            let result = (|| {
                                let files = state.files()?;
                                let storage = FsStorage::create_fresh(
                                    Arc::new(FsGate::new(&SystemEnv)),
                                    &dir,
                                )
                                .map_err(|e| format!("{e:?}"))?;
                                for file in files {
                                    let path = ProjectPath::new(&file.path)
                                        .map_err(|e| format!("{e:?}"))?;
                                    storage
                                        .write_bytes(&path, file.text.as_bytes())
                                        .map_err(|e| format!("{e:?}"))?;
                                }
                                Ok::<_, String>(())
                            })();
                            match result {
                                Ok(()) => return self.try_open(&dir),
                                Err(message) => self.problem = Some((dir, message, false)),
                            }
                        }
                    }
                }
            }
            if self.want_import {
                self.want_import = false;
                if let Some(zip_file) = block_on(RfdDialog.pick_zip()) {
                    if let Some(dir) = block_on(RfdDialog.pick_folder()) {
                        match crate::adapters::zip::import(&zip_file.bytes) {
                            Ok(imported) => {
                                let gate = Arc::new(FsGate::new(&SystemEnv));
                                match FsStorage::create(gate, &dir).and_then(|storage| {
                                    crate::adapters::zip::import_project(&storage, &imported)
                                        .map_err(|error| {
                                            crate::ports::storage::StorageError::Io {
                                                path: dir.display().to_string(),
                                                kind: std::io::ErrorKind::Other,
                                                message: format!("{error:?}"),
                                            }
                                        })?;
                                    Ok(())
                                }) {
                                    Ok(()) => {
                                        if let Some(opened) = self.try_open(&dir) {
                                            return Some(opened);
                                        }
                                    }
                                    Err(error) => {
                                        self.problem = Some((dir, format!("{error:?}"), false))
                                    }
                                }
                            }
                            Err(error) => self.problem = Some((dir, format!("{error:?}"), false)),
                        }
                    }
                }
            }
            if self.want_folder || self.want_new {
                let new = self.want_new;
                self.want_folder = false;
                self.want_new = false;
                if let Some(dir) = block_on(RfdDialog.pick_folder()) {
                    if new {
                        match create_here(&dir, locale) {
                            Ok(()) => opened = self.try_open(&dir),
                            Err(report) => self.problem = Some((dir, report, false)),
                        }
                    } else {
                        opened = self.try_open(&dir);
                    }
                }
            }
            if opened.is_some() {
                return opened;
            }
            // ↑/↓ + Enter（§14.4）
            let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
            let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
            let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
            let delete = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete));
            if down && self.selected + 1 < self.recent.len() {
                self.selected += 1;
            }
            if up {
                self.selected = self.selected.saturating_sub(1);
            }
            let mut choose: Option<PathBuf> = None;
            if enter && self.recent.is_empty() {
                self.want_folder = true;
            }
            if enter {
                if let Some((path, true, _)) = self.recent.get(self.selected) {
                    choose = Some(PathBuf::from(path));
                }
            }
            let mut create_at: Option<PathBuf> = None;
            // `Delete` が消すのは履歴の 1 行だけで、プロジェクトのファイルは消さない（§13.4）。
            let mut remove_history: Option<String> = delete
                .then(|| self.recent.get(self.selected).map(|row| row.0.clone()))
                .flatten();
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
                        if self.recent.is_empty() {
                            ui.label(i18n::text(locale, "help.welcome"));
                            crate::ui::widgets::external_link(
                                ui,
                                i18n::text(locale, "help.website"),
                                crate::core::model::project::PROJECT_STRUCTURE_DOC_URL,
                                &colors,
                            );
                            ui.label(
                                RichText::new(i18n::text(locale, "project.pick.empty"))
                                    .color(colors.fg_faint),
                            );
                        }
                        for (index, (path, exists, opened_at)) in self.recent.iter().enumerate() {
                            let shown = recent_caption(path, *exists, opened_at, locale);
                            let color = if !exists {
                                colors.fg_faint
                            } else if index == self.selected {
                                colors.accent
                            } else {
                                colors.fg_primary
                            };
                            ui.horizontal(|ui| {
                                let response = crate::ui::widgets::selectable_action(
                                    ui,
                                    index == self.selected,
                                    RichText::new(shown).color(color),
                                );
                                if response.clicked() && *exists {
                                    choose = Some(PathBuf::from(path));
                                }
                                let remove = ui
                                    .add(
                                        egui::Button::new(RichText::new("×").underline())
                                            .small()
                                            .min_size(Vec2::splat(24.0)),
                                    )
                                    .on_hover_text(i18n::text(
                                        locale,
                                        "project.pick.remove_history",
                                    ));
                                if remove.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if remove.clicked() {
                                    remove_history = Some(path.clone());
                                }
                            });
                        }
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.add_space((ui.available_width() - 320.0).max(0.0) / 2.0);
                            if crate::ui::widgets::action_button(
                                ui,
                                format!("📂 {}", i18n::text(locale, "project.pick.open")),
                            )
                            .clicked()
                            {
                                self.want_folder = true;
                            }
                            ui.add_space(12.0);
                            if crate::ui::widgets::action_button(
                                ui,
                                format!("＋ {}", i18n::text(locale, "project.pick.new")),
                            )
                            .clicked()
                            {
                                self.want_new = true;
                            }
                            ui.add_space(12.0);
                            if crate::ui::widgets::action_button(
                                ui,
                                i18n::text(locale, "project.pick.import"),
                            )
                            .clicked()
                            {
                                self.want_import = true;
                            }
                            if crate::ui::widgets::action_button(
                                ui,
                                i18n::text(locale, "stage.migrate"),
                            )
                            .clicked()
                            {
                                self.want_migrate = true;
                            }
                        });
                        if let Some((dir, report, incomplete)) = &self.problem {
                            ui.add_space(20.0);
                            Frame::none()
                                .fill(colors.bg_inset)
                                .inner_margin(Margin::same(12.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    for line in report.lines() {
                                        if let Some(start) = line.find("https://") {
                                            let url = line[start..].trim();
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(&line[..start])
                                                        .color(colors.fg_muted),
                                                );
                                                crate::ui::widgets::external_link(
                                                    ui, url, url, &colors,
                                                );
                                            });
                                        } else {
                                            ui.label(RichText::new(line).color(colors.fg_muted));
                                        }
                                    }
                                    if *incomplete {
                                        ui.add_space(8.0);
                                        // 勝手に作らない（明示操作が必要、§14.4）
                                        if crate::ui::widgets::action_button(
                                            ui,
                                            i18n::text(locale, "project.pick.create_here"),
                                        )
                                        .clicked()
                                        {
                                            create_at = Some(dir.clone());
                                        }
                                    }
                                });
                        }
                    });
                });
            if let Some(path) = remove_history {
                let gate = Arc::new(FsGate::new(&SystemEnv));
                if RecentFile::new(gate).remove(&path).is_ok() {
                    self.recent.retain(|(entry, _, _)| entry != &path);
                    self.selected = self.selected.min(self.recent.len().saturating_sub(1));
                }
            }
            if let Some(dir) = create_at {
                match create_here(&dir, locale) {
                    Ok(()) => {
                        self.problem = None;
                        return self.try_open(&dir);
                    }
                    Err(report) => self.problem = Some((dir, report, false)),
                }
            }
            if self.want_folder || self.want_new || self.want_import {
                ctx.request_repaint();
            }
            let _ = Vec2::ZERO;
            choose.and_then(|dir| self.try_open(&dir))
        }
    }
    #[cfg(test)]
    mod picker_tests {
        use super::*;

        #[test]
        fn native_history_rows_display_the_date_and_missing_paths() {
            assert_eq!(
                recent_caption("/books/novel", true, "2026-10-15T09:30:00Z", Locale::Ja),
                "novel    /books/novel    2026-10-15"
            );
            assert!(
                recent_caption("/books/gone", false, "2026-10-14", Locale::Ja)
                    .contains(i18n::text(Locale::Ja, "project.pick.missing"))
            );
        }
    }
}
