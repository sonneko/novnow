//! セッション（§19.2）。**egui に依存しない**。novnow の全機能へのヘッドレスな入口。
//!
//! `Session` は 1 つのプロジェクト（またはメモリ上の仮想プロジェクト）に対する全操作の
//! 集合で、§19.1 の 8 つの状態機械（`Workbench`・`SaveMachine`・`UiSettings`・エージェント
//! 実行・検索・パレット論理・コマンド・ワークスペース切替）を合成する。GUI（`ui::app`）も
//! 同じ状態機械を表示するだけなので、同じ入力に同じ結果が返る（§19.1.1）。
//!
//! ```no_run
//! use novnow::app::Session;
//!
//! # fn main() -> novnow::app::error::Result<()> {
//! let mut session = Session::open("my-novel")?;
//! let info = session.project().info()?; // project.info
//! let body = session.section().get("@03:01")?; // section.get
//! session.section().write("@03:01", "新しい本文\n")?; // メモリ上
//! session.save().now()?; // ここで書き込む
//! # let _ = (info, body);
//! # Ok(())
//! # }
//! ```

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::{json, Map, Value};

use crate::app::agents::{self, Runtime};
use crate::app::command::Command;
use crate::app::error::{Class, Error, Result};
use crate::app::event::AppEvent;
use crate::app::host::Host;
use crate::app::ops::{self, Args, OpDef, OPS};

use crate::app::save_machine::State;
use crate::app::settings::UiSettings;
use crate::app::workbench::{Nav, Workbench};
use crate::app::workspace::{Switcher, Workspace};
use crate::core::features::Project;
use crate::core::id::SceneId;
use crate::ui::i18n::{self, Locale};

/// どのインターフェースから呼ばれているか（§19.1.5 の関門の適用と §19.7 の例外列に使う）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interface {
    /// Rust API（§19.2）。
    Rust,
    /// Python API（§19.3）。
    Python,
    /// LSP（§19.4。`workspace/executeCommand` は人間経路）。
    Lsp,
    /// MCP サーバ（§19.5。書き込みは関門を通る）。
    Mcp,
    /// CLI（§19.6）。
    Cli,
}

impl Interface {
    /// `api.list` の例外列の名前（`R` / `P` / `L` / `M` / `C`）。
    pub fn column(self) -> &'static str {
        match self {
            Self::Rust => "R",
            Self::Python => "P",
            Self::Lsp => "L",
            Self::Mcp => "M",
            Self::Cli => "C",
        }
    }

    /// 名前（`--json` の `interface`・監査ログ用）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::Lsp => "lsp",
            Self::Mcp => "mcp",
            Self::Cli => "cli",
        }
    }
}

/// `Session::create` のオプション（§19.2.1。`novnow new` と同じ構造を作る）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreateOptions {
    /// `project.json` の `title`（空ならディレクトリ名）。
    pub title: String,
    /// `project.json` の `author`。
    pub author: String,
    /// `project.json` の `language`（既定 `ja`）。
    pub language: String,
}

/// `export.text` の引数（§17.2 のオプション。`ExportQuery::default()` は全体・既定の形）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportQuery {
    /// 範囲（`01`・`01-04`・`01-04-02-03`・`01-02/05`。空なら全体）。
    pub range: String,
    /// 状態の絞り込み（`draft` / `review` / `fixed`。空なら全部）。
    pub state: String,
    /// 形式（`txt` / `md`。空なら `txt`）。
    pub format: String,
    /// 書き出し先（空ならテキストを返すだけ。§13.1.1 の特例の口を通る）。
    pub out: String,
    /// 章のあらすじを含める。
    pub blurb: bool,
    /// あらすじだけを出す。
    pub blurb_only: bool,
    /// イラストのメモを残す。
    pub keep_illustration_memo: bool,
    /// 素材の在庫を含める。
    pub stock: bool,
}

/// 編集中の検索の状態（`search.*`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Find {
    /// 検索語。
    pub query: String,
    /// 置換語（`search.file` の `replace`）。
    pub replace: Option<String>,
    /// 一致（編集中文書の中の byte 範囲）。
    pub hits: Vec<std::ops::Range<usize>>,
    /// 今の一致（`hits` の添字）。
    pub selected: usize,
}

/// 移動履歴（`nav.prev` / `nav.next`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// パス。
    pub path: String,
    /// 節の ID。
    pub scene: SceneId,
    /// 文書の中の byte 位置。
    pub byte: usize,
}

/// 正規操作の 1 回分の実行結果（`session.run` の内部記録。監査ログと同じ材料）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    /// 正規操作 ID。
    pub op: String,
    /// 成功したか。
    pub ok: bool,
    /// 失敗の分類（成功時は `None`）。
    pub class: Option<Class>,
}

/// 1 つのプロジェクトに対する全操作（§19.2.1）。
///
/// 排他的に所有する（§19.2.5）。破棄（drop）は未保存の変更を破棄する（§9.6 と同じ）。
/// イベント購読の口（[`Session::on_event`]）。
type Callback = Box<dyn FnMut(&AppEvent)>;

pub struct Session {
    pub(crate) host: Box<dyn Host>,
    pub(crate) bench: Workbench,
    pub(crate) settings: UiSettings,
    pub(crate) agents: Runtime,
    pub(crate) switcher: Switcher,
    pub(crate) locale: Locale,
    pub(crate) interface: Interface,
    pub(crate) events: VecDeque<AppEvent>,
    pub(crate) callbacks: Vec<Callback>,
    pub(crate) clock: f64,
    pub(crate) recent: Vec<Place>,
    /// `recent` の今の位置（`Ctrl+Tab` の循環、§9.4.3）。
    pub(crate) recent_at: usize,
    pub(crate) history: Vec<Place>,
    pub(crate) history_at: usize,
    pub(crate) conflicts: BTreeMap<String, Option<String>>,
    pub(crate) find: Option<Find>,
    pub(crate) folded: BTreeSet<u16>,
    pub(crate) saving: Option<BTreeMap<String, String>>,
    pub(crate) trace: Vec<Trace>,
    pub(crate) root: Option<std::path::PathBuf>,
}

impl Session {
    /// 版（§19.8.1。`--version`・`novnow.__version__`・LSP の `serverInfo` と同じ）。
    pub fn version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// カタログの版（§19.8.1「カタログ自体に版を付け、`api.list` の出力に含める」）。
    pub fn catalog_version() -> &'static str {
        ops::CATALOG_VERSION
    }

    /// 読み込んだプロジェクトとホストから作る（GUI の `Session::new` と同じ手順）。
    pub fn from_project(host: Box<dyn Host>, project: Project, settings: &UiSettings) -> Self {
        let bench = Workbench::new(
            project,
            settings.last_scene.as_deref(),
            settings.autosave_ms,
        );
        Self {
            host,
            bench,
            settings: settings.clone(),
            agents: Runtime::default(),
            switcher: Switcher::default(),
            locale: Locale::Ja,
            interface: Interface::Rust,
            events: VecDeque::new(),
            callbacks: Vec::new(),
            clock: 0.0,
            recent: Vec::new(),
            recent_at: 0,
            history: Vec::new(),
            history_at: 0,
            conflicts: BTreeMap::new(),
            find: None,
            folded: BTreeSet::new(),
            saving: None,
            trace: Vec::new(),
            root: None,
        }
    }

    /// メモリ上の仮想プロジェクト（§19.2.1「テスト用。ファイルシステムに触れない」）。
    pub fn with_host(host: Box<dyn Host>) -> Self {
        Self::from_project(host, Project::default(), &UiSettings::default())
    }

    /// メモリホストに文書を入れて開く（テスト用。実ファイルシステムに触れない）。
    pub fn with_documents(
        host: Box<dyn Host>,
        documents: Vec<crate::core::model::document::Document>,
    ) -> Self {
        let project = Project::with_documents(Default::default(), documents);
        Self::from_project(host, project, &UiSettings::default())
    }

    /// 既存のプロジェクトを開く（§14.2 の起動前チェックと同じ検証。ネイティブ専用）。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::open_with(path.as_ref(), Interface::Rust)
    }

    /// [`Session::open`] にインターフェースを指定する形（CLI・LSP・Python が使う）。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_with(path: &std::path::Path, interface: Interface) -> Result<Self> {
        use crate::adapters::project_load;
        use crate::app::fs_host::FsHost;

        let host = FsHost::open(path)?;
        let root = host.root().to_owned();
        let storage = host.storage();
        let loaded = crate::adapters::block_on(project_load::load(storage))
            .map_err(|error| Error::io(format!("{error:?}"), None))?;
        if let Err(error) = &loaded.validation {
            return Err(project_error(interface_locale(interface), error));
        }
        let settings = UiSettings::parse(
            &crate::adapters::block_on(storage_read_settings(storage)).unwrap_or_default(),
        );
        let mut session = Self::from_project(Box::new(host), loaded.project, &settings);
        session.interface = interface;
        session.root = Some(root);
        Ok(session)
    }

    /// 新しいプロジェクトを作る（`novnow new` と同じ構造。ネイティブ専用）。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn create(path: impl AsRef<std::path::Path>, options: CreateOptions) -> Result<Self> {
        use crate::app::fs_host::FsHost;
        use crate::core::template;

        let path = path.as_ref();
        let mut host = FsHost::create(path)?;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let title = if options.title.trim().is_empty() {
            name.clone()
        } else {
            options.title.clone()
        };
        for file in template::new_project(&title) {
            host.write(file.path, &file.text)
                .map_err(|message| Error::io(message, Some(file.path)))?;
        }
        Self::open_with(path, Interface::Rust)
    }

    /// 実ファイルシステムを持たない環境（Web）での既定ホスト（何もしない）。
    #[cfg(target_arch = "wasm32")]
    pub fn open(_path: impl AsRef<std::path::Path>) -> Result<Self> {
        Err(Error::new(
            Class::Io,
            i18n::text(Locale::Ja, "api.error.no_fs").to_owned(),
        ))
    }

    /// [`Session::create`] の Web 形（ファイルシステムが無いので `io` で断る）。
    #[cfg(target_arch = "wasm32")]
    pub fn create(_path: impl AsRef<std::path::Path>, _options: CreateOptions) -> Result<Self> {
        Err(Error::new(
            Class::Io,
            i18n::text(Locale::Ja, "api.error.no_fs").to_owned(),
        ))
    }

    /// プロジェクトの集約（§3.4 の `Project`）。
    pub fn aggregate(&self) -> &Project {
        self.bench.project()
    }

    /// 編集の状態（GUI が描画に使うものと同じ）。
    pub fn workbench(&self) -> &Workbench {
        &self.bench
    }

    /// 編集の状態（可変）。
    pub fn workbench_mut(&mut self) -> &mut Workbench {
        &mut self.bench
    }

    /// ホスト（可変）。
    pub fn host_mut(&mut self) -> &mut dyn Host {
        self.host.as_mut()
    }

    /// 設定（`.novnow/ui.json`、§4.4.1）。
    pub fn settings(&self) -> &UiSettings {
        &self.settings
    }

    /// メッセージの言語（§19.1.8）。
    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// メッセージの言語を切り替える（`set_locale`、§19.1.8）。
    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
        self.settings.language = match locale {
            Locale::Ja => crate::app::settings::LanguageChoice::Ja,
            Locale::En => crate::app::settings::LanguageChoice::En,
        };
        self.push(AppEvent::SettingsChanged {
            key: "language".to_owned(),
        });
    }

    /// どのインターフェースから呼ばれているか。
    pub fn interface(&self) -> Interface {
        self.interface
    }

    /// インターフェースを指定する（CLI・LSP・MCP・Python の适配層が使う）。
    pub fn set_interface(&mut self, interface: Interface) {
        self.interface = interface;
    }

    /// 注入する時刻（秒。§19.1.4「操作の結果は壁時計に依存しない」）。
    pub fn clock(&self) -> f64 {
        self.clock
    }

    /// 注入する時刻を設定する（テストで保存状態機の時間経過を再現する）。
    pub fn set_clock(&mut self, seconds: f64) {
        self.clock = seconds;
    }

    /// 時刻を進める（自動保存の期限・スピナーの判定に効く）。
    pub fn advance(&mut self, seconds: f64) {
        self.clock += seconds;
    }

    // ------------------------------------------------------------ イベント（§19.2.4）

    /// イベントを取り出す（溜まっていなければ空）。
    pub fn poll_events(&mut self) -> Vec<AppEvent> {
        self.events.drain(..).collect()
    }

    /// イベントのコールバックを登録する（§19.2.4「`poll_events()` で取り出すか、コールバックを登録」）。
    pub fn on_event(&mut self, callback: impl FnMut(&AppEvent) + 'static) {
        self.callbacks.push(Box::new(callback));
    }

    /// イベントを積む（コールバックにも渡す）。
    pub fn push(&mut self, event: AppEvent) {
        for callback in &mut self.callbacks {
            callback(&event);
        }
        self.events.push_back(event);
    }

    // ------------------------------------------------------------ 実行（§19.1.4 単一実装点）

    /// 正規操作 ID で操作を実行する（5 インターフェース共通の入口）。
    pub fn run(&mut self, op: &str, args: Value) -> Result<Value> {
        let Some(def) = ops::find(op) else {
            return Err(Error::new(
                Class::Usage,
                i18n::format(self.locale, "api.error.unknown_op", &[op]),
            ));
        };
        let map = match args {
            Value::Object(map) => map,
            Value::Null => Map::new(),
            _ => return Err(Error::usage(self.locale, "arguments")),
        };
        if let Some(column) = def.excepts(self.interface) {
            return Err(Error::unsupported(self.locale, op, column));
        }
        let outcome = (def.handler)(self, &map);
        self.trace.push(Trace {
            op: op.to_owned(),
            ok: outcome.is_ok(),
            class: outcome.as_ref().err().map(|error| error.class),
        });
        outcome
    }

    /// カタログ（`api.list`、§19.8.2）。
    pub fn catalog(&self) -> Value {
        ops::catalog(self.locale)
    }

    /// カタログの 1 件。
    pub fn op(&self, id: &str) -> Option<&'static OpDef> {
        ops::find(id)
    }

    /// カタログの全操作 ID。
    pub fn op_ids() -> impl Iterator<Item = &'static str> {
        OPS.iter().map(|def| def.id)
    }

    /// パレットコマンドを実行する（§19.2.3。GUI の `>` と同じ表）。
    pub fn command(&mut self, name: &str) -> Result<Value> {
        self.run("session.command", json!({ "name": name }))
    }

    /// 長時間の操作を中断する（§19.1.9）。`handle` はタスク ID か呼び出し番号。
    pub fn cancel(&mut self, handle: &str) -> Result<Value> {
        self.run("agent.task_stop", json!({ "id": handle }))
    }

    /// 外部変更を 1 回分取り込む（§8.5・§19.1.6）。
    pub fn poll_external(&mut self) -> Vec<(String, Option<String>)> {
        let changes = self.host.poll();
        for (path, text) in &changes {
            let reaction = self.bench.external_change(path, text.clone());
            self.conflicts.insert(path.clone(), text.clone());
            self.push(AppEvent::DocumentChanged {
                path: path.clone(),
                change: match reaction {
                    crate::app::workbench::Reaction::Reloaded => "external",
                    crate::app::workbench::Reaction::Conflict => "conflict",
                    crate::app::workbench::Reaction::Unchanged => "unchanged",
                }
                .to_owned(),
            });
            if reaction == crate::app::workbench::Reaction::Conflict {
                self.bench.save.request_now();
                self.push(AppEvent::SaveStateChanged {
                    state: "conflict".to_owned(),
                    message: String::new(),
                });
            }
        }
        changes
    }

    /// 未保存の変更をすべて書き出す（`save.now` の中身。§8.3 の 1 回分）。
    pub fn flush(&mut self) -> Result<Value> {
        self.bench.commit();
        let stamp = self.host.now();
        self.bench.touch_materials(&stamp);
        let writes = self.bench.take_writes();
        let removals = self.bench.take_removals();
        let handed: BTreeMap<String, String> = writes.iter().cloned().collect();
        self.bench.save.begin(self.clock);
        self.saving = Some(handed.clone());
        let outcome = match self.host.save(writes, removals) {
            Some(outcome) => outcome,
            None => match self.host.save_finished() {
                Some(outcome) => outcome,
                None => {
                    return Err(Error::new(
                        Class::Conflict,
                        i18n::text(self.locale, "api.error.saving").to_owned(),
                    ))
                }
            },
        };
        let written = outcome.written.clone();
        let failures = outcome.failures.clone();
        self.saving = None;
        self.bench
            .write_finished(self.clock, &outcome.written, &outcome.failures);
        for path in &outcome.removals_left {
            self.bench.removal_failed(path);
        }
        for path in &written {
            self.push(AppEvent::DocumentChanged {
                path: path.clone(),
                change: "written".to_owned(),
            });
        }
        for (path, message) in &failures {
            self.push(AppEvent::WriteFailed {
                path: path.clone(),
                message: message.clone(),
            });
        }
        let state = self.bench.save.state();
        self.push(AppEvent::SaveStateChanged {
            state: save_state_name(state).to_owned(),
            message: failures
                .first()
                .map(|(_, message)| message.clone())
                .unwrap_or_default(),
        });
        if !failures.is_empty() {
            let (path, message) = failures.first().expect("checked above");
            return Err(Error::io(message.clone(), Some(path)));
        }
        Ok(json!({
            "written": written,
            "state": save_state_name(state),
        }))
    }

    /// 保存状態（§9.6 の 6 状態の名前）。
    pub fn save_state(&self) -> &'static str {
        save_state_name(self.bench.save.state())
    }

    /// 実行した操作の記録（テスト・監査用）。
    pub fn trace(&self) -> &[Trace] {
        &self.trace
    }

    /// 開いているプロジェクトのルート（メモリホストでは `None`）。
    pub fn root(&self) -> Option<&std::path::Path> {
        self.root.as_deref()
    }

    /// `agents/` の準備（欠けた既定プロンプトを書く。§15.5.4）。
    pub fn prepare_agents(&mut self) -> Option<String> {
        agents::write_prompts(&mut self.bench, self.host.as_ref(), self.clock, self.locale)
    }

    /// エージェントの実行の状態。
    pub fn agents(&self) -> &Runtime {
        &self.agents
    }

    /// エージェントの実行の状態（可変）。
    pub fn agents_mut(&mut self) -> &mut Runtime {
        &mut self.agents
    }

    /// 現在のワークスペース（§11.1）。
    pub fn workspace(&self) -> Workspace {
        self.switcher.current()
    }

    /// ワークスペースを切り替える（§10.0）。
    pub fn switch_workspace(&mut self, workspace: Workspace) -> bool {
        let switched = self.switcher.switch(workspace);
        if switched {
            self.bench.commit();
        }
        switched
    }

    /// 切替パネル（§11.1）。
    pub fn switcher(&self) -> &Switcher {
        &self.switcher
    }

    /// 折り畳んでいる章（`plot.outline_fold`）。
    pub fn folded(&self) -> &BTreeSet<u16> {
        &self.folded
    }

    /// 移動履歴（`nav.prev` / `nav.next`）。
    pub fn history(&self) -> &[Place] {
        &self.history
    }

    /// 今の検索の状態（`search.*`）。
    pub fn find(&self) -> Option<&Find> {
        self.find.as_ref()
    }

    /// 衝突中のファイル（§8.5）。
    pub fn conflicts(&self) -> &BTreeMap<String, Option<String>> {
        &self.conflicts
    }

    /// 移動履歴に積む（`nav.*` が使う）。
    pub(crate) fn remember(&mut self) {
        if let Some(place) = self.current_place() {
            if self.history.last() != Some(&place) {
                self.history.push(place);
                self.history_at = self.history.len();
            }
        }
    }

    /// 今の位置。
    pub(crate) fn current_place(&self) -> Option<Place> {
        let current = self.bench.current()?;
        let (document, start) = self.bench.spliced_document()?;
        Some(Place {
            path: document.path.clone(),
            scene: current.id,
            byte: start,
        })
    }

    /// 設定を `.novnow/ui.json` へ書く（`settings.set` の後）。
    pub(crate) fn flush_settings(&mut self) -> Result<()> {
        let text = self.settings.to_json();
        self.host
            .write(crate::app::settings::UI_JSON, &text)
            .map_err(|message| Error::io(message, Some(crate::app::settings::UI_JSON)))
    }
}

/// §9.6 の状態の名前（`save.state`・`novnow/saveStateChanged` が共有する）。
pub fn save_state_name(state: State) -> &'static str {
    match state {
        State::Clean => "clean",
        State::Dirty => "dirty",
        State::Saving => "saving",
        State::Failed => "error",
    }
}

/// §14.2.4 のプロジェクト不備を `Error` へ写像する（§19.1.7 の `project` / `io`）。
#[cfg(not(target_arch = "wasm32"))]
fn project_error(locale: Locale, error: &crate::core::model::project::ProjectError) -> Error {
    use crate::core::model::project::ProjectError;
    match error {
        ProjectError::Incomplete { missing } => {
            let names: Vec<String> = missing
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            Error {
                kind: "S022".to_owned(),
                ..Error::new(
                    Class::Project,
                    i18n::format(locale, "api.error.incomplete", &[&names.join(", ")]),
                )
            }
        }
        ProjectError::TooNew { found } => Error {
            kind: "S023".to_owned(),
            ..Error::new(
                Class::Project,
                i18n::format(locale, "api.error.too_new", &[&found.to_string()]),
            )
        },
        ProjectError::Encoding { files } => {
            let first = files
                .first()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            Error {
                kind: "S024".to_owned(),
                path: Some(first.clone()),
                ..Error::new(
                    Class::Io,
                    i18n::format(locale, "api.error.encoding", &[&first]),
                )
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn interface_locale(interface: Interface) -> Locale {
    match interface {
        Interface::Rust | Interface::Python | Interface::Lsp | Interface::Mcp | Interface::Cli => {
            Locale::Ja
        }
    }
}

/// `.novnow/ui.json` を読む（無ければ空）。
#[cfg(not(target_arch = "wasm32"))]
async fn storage_read_settings(storage: &crate::adapters::storage_fs::FsStorage) -> Option<String> {
    use crate::ports::storage::Storage;
    let path =
        crate::core::policy::write_policy::ProjectPath::new(crate::app::settings::UI_JSON).ok()?;
    storage.read(&path).await.ok()
}

// =========================================================================== ドメインハンドル

/// 引数を JSON のオブジェクトにする（`Option` は `None` のとき入れない）。
macro_rules! args {
    ($($name:literal => $value:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut map = serde_json::Map::new();
        $(
            let value = $value;
            if let Some(value) = into_arg(value) {
                map.insert(($name).to_owned(), value);
            }
        )*
        serde_json::Value::Object(map)
    }};
    ($($name:ident => $value:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut map = serde_json::Map::new();
        $(
            let value = $value;
            if let Some(value) = into_arg(value) {
                map.insert(stringify!($name).to_owned(), value);
            }
        )*
        serde_json::Value::Object(map)
    }};
}

/// 1 つの引数を JSON へ（`Option<T>` は `None` で「入れない」）。
fn into_arg<T: serde::Serialize>(value: T) -> Option<Value> {
    match serde_json::to_value(value) {
        Ok(Value::Null) => None,
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

/// ドメインハンドルの共通の骨組み（§19.2.2）。
macro_rules! handle {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        pub struct $name<'s>(pub(crate) &'s mut Session);

        impl<'s> $name<'s> {
            // 正規操作 ID を直接実行する（型付きの口が無い操作の逃げ道）
            pub fn call(&mut self, op: &str, args: Value) -> Result<Value> {
                self.0.run(op, args)
            }

            // セッション本体
            pub fn session(&mut self) -> &mut Session {
                self.0
            }
        }
    };
}

/// 正規操作を 1 つ呼ぶメソッドを定義する（引数は JSON のオブジェクトのキーと同じ名前）。
macro_rules! op {
    ($(#[$meta:meta])* $vis:vis fn $name:ident($($arg:ident : $ty:ty),* $(,)?) -> $id:literal) => {
        $(#[$meta])*
        $vis fn $name(&mut self, $($arg: $ty),*) -> Result<Value> {
            let payload = args! { $($arg => $arg),* };
            self.0.run($id, payload)
        }
    };
}

handle!(
    // `project.*`（§19.7 の `project` ドメイン）。
    ProjectOps
);
handle!(
    // `structure.*`。
    StructureOps
);
handle!(
    // `section.*`。
    SectionOps
);
handle!(
    // `nav.*`。
    NavOps
);
handle!(
    // `document.*`。
    DocumentOps
);
handle!(
    // `plot.*`。
    PlotOps
);
handle!(
    // `save.*`。
    SaveOps
);
handle!(
    // `search.*`。
    SearchOps
);
handle!(
    // `materials.*`。
    MaterialsOps
);
handle!(
    // `idea.*`。
    IdeaOps
);
handle!(
    // `cast.*`。
    CastOps
);
handle!(
    // `dictionary.*`。
    DictionaryOps
);
handle!(
    // `lang.*`（§3.4 の 11 機能）。
    LangOps
);
handle!(
    // `diagnostics.*`。
    DiagnosticsOps
);
handle!(
    // `review.*`。
    ReviewOps
);
handle!(
    // `agent.*`（§15）。
    AgentOps
);
handle!(
    // `prompt.*`。
    PromptOps
);
handle!(
    // `context.*`。
    ContextOps
);
handle!(
    // `mail.*`。
    MailOps
);
handle!(
    // `export.*`（§17.2）。
    ExportOps
);
handle!(
    // `settings.*`。
    SettingsOps
);
handle!(
    // `session.*`。
    SessionOps
);
handle!(
    // `help.*`。
    HelpOps
);
handle!(
    // `api.*`（§19.8.2）。
    ApiOps
);

impl Session {
    /// `project.*`。
    pub fn project(&mut self) -> ProjectOps<'_> {
        ProjectOps(self)
    }
    /// `structure.*`。
    pub fn structure(&mut self) -> StructureOps<'_> {
        StructureOps(self)
    }
    /// `section.*`。
    pub fn section(&mut self) -> SectionOps<'_> {
        SectionOps(self)
    }
    /// `nav.*`。
    pub fn nav(&mut self) -> NavOps<'_> {
        NavOps(self)
    }
    /// `document.*`。
    pub fn document(&mut self) -> DocumentOps<'_> {
        DocumentOps(self)
    }
    /// `plot.*`。
    pub fn plot(&mut self) -> PlotOps<'_> {
        PlotOps(self)
    }
    /// `save.*`。
    pub fn save(&mut self) -> SaveOps<'_> {
        SaveOps(self)
    }
    /// `search.*`。
    pub fn search(&mut self) -> SearchOps<'_> {
        SearchOps(self)
    }
    /// `materials.*`。
    pub fn materials(&mut self) -> MaterialsOps<'_> {
        MaterialsOps(self)
    }
    /// `idea.*`。
    pub fn idea(&mut self) -> IdeaOps<'_> {
        IdeaOps(self)
    }
    /// `cast.*`。
    pub fn cast(&mut self) -> CastOps<'_> {
        CastOps(self)
    }
    /// `dictionary.*`。
    pub fn dictionary(&mut self) -> DictionaryOps<'_> {
        DictionaryOps(self)
    }
    /// `lang.*`。
    pub fn lang(&mut self) -> LangOps<'_> {
        LangOps(self)
    }
    /// `diagnostics.*`。
    pub fn diagnostics(&mut self) -> DiagnosticsOps<'_> {
        DiagnosticsOps(self)
    }
    /// `review.*`。
    pub fn review(&mut self) -> ReviewOps<'_> {
        ReviewOps(self)
    }
    /// `agent.*`。
    pub fn agent(&mut self) -> AgentOps<'_> {
        AgentOps(self)
    }
    /// `prompt.*`。
    pub fn prompt(&mut self) -> PromptOps<'_> {
        PromptOps(self)
    }
    /// `context.*`。
    pub fn context(&mut self) -> ContextOps<'_> {
        ContextOps(self)
    }
    /// `mail.*`。
    pub fn mail(&mut self) -> MailOps<'_> {
        MailOps(self)
    }
    /// `export.*`。
    pub fn export(&mut self) -> ExportOps<'_> {
        ExportOps(self)
    }
    /// `settings.*`。
    pub fn ui_settings(&mut self) -> SettingsOps<'_> {
        SettingsOps(self)
    }
    /// `session.*`。
    pub fn session(&mut self) -> SessionOps<'_> {
        SessionOps(self)
    }
    /// `help.*`。
    pub fn help(&mut self) -> HelpOps<'_> {
        HelpOps(self)
    }
    /// `api.*`。
    pub fn api(&mut self) -> ApiOps<'_> {
        ApiOps(self)
    }
}

// --------------------------------------------------------------------------- 型付きの口

impl<'s> ProjectOps<'s> {
    op!(
        // プロジェクト基本情報（`project.info`）。
        pub fn info() -> "project.info"
    );
    op!(
        // 構造検証（`project.validate`）。
        pub fn validate() -> "project.validate"
    );
    op!(
        // 再読込（`project.reload`）。
        pub fn reload() -> "project.reload"
    );
    op!(
        // 執筆統計（`project.stats`）。
        pub fn stats() -> "project.stats"
    );
    op!(
        // 最近開いたプロジェクト一覧（`project.recent_list`）。
        pub fn recent_list() -> "project.recent_list"
    );
    op!(
        // 最近開いた一覧からの削除（`project.recent_remove`）。
        pub fn recent_remove(path: &str) -> "project.recent_remove"
    );
}

impl<'s> StructureOps<'s> {
    op!(
        // 構成ツリー（`structure.tree`）。
        pub fn tree(level: Option<&str>) -> "structure.tree"
    );
    op!(
        // 位置・ID の解決（`structure.resolve`）。
        pub fn resolve(query: &str) -> "structure.resolve"
    );
    op!(
        // パンくず（`structure.breadcrumb`）。
        pub fn breadcrumb() -> "structure.breadcrumb"
    );
}

impl<'s> SectionOps<'s> {
    op!(
        // セクション一覧（`section.list`）。
        pub fn list(path: Option<&str>) -> "section.list"
    );
    /// セクション取得（`section.get`）。本文のテキストを返す。
    pub fn get(&mut self, scene: &str) -> Result<String> {
        let found = self.0.run("section.get", json!({ "scene": scene }))?;
        Ok(found
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    }
    /// セクション保存（`section.write`。メモリ上。書き出しは `save.now`）。
    pub fn write(&mut self, scene: &str, text: &str) -> Result<Value> {
        self.0
            .run("section.write", json!({ "scene": scene, "text": text }))
    }
    op!(
        // 節の新設（`section.insert`）。
        pub fn insert(after: Option<&str>, text: Option<&str>) -> "section.insert"
    );
    op!(
        // 節の削除（`section.delete`）。
        pub fn delete(scene: &str) -> "section.delete"
    );
    op!(
        // 次の未確認節の確認（`section.confirm_next`）。
        pub fn confirm_next() -> "section.confirm_next"
    );
}

impl<'s> SaveOps<'s> {
    /// 保存（`save.now`）。
    pub fn now(&mut self) -> Result<Value> {
        self.0.run("save.now", json!({}))
    }
    op!(
        // 保存状態の取得（`save.state`）。
        pub fn state() -> "save.state"
    );
    op!(
        // 衝突時に自分の版を維持（`save.keep_mine`）。
        pub fn keep_mine(path: Option<&str>) -> "save.keep_mine"
    );
    op!(
        // 外部変更の読み込み（`save.reload_external`）。
        pub fn reload_external(path: Option<&str>) -> "save.reload_external"
    );
    op!(
        // 自分の版と外部変更の差分（`save.diff_external`）。
        pub fn diff_external(path: Option<&str>) -> "save.diff_external"
    );
}

impl<'s> SearchOps<'s> {
    op!(
        // 検索（`search.file`。編集中文書）。
        pub fn file(query: &str, replace: Option<&str>, regex: Option<bool>) -> "search.file"
    );
    op!(
        // 全置換（`search.replace_file`）。
        pub fn replace_file(query: &str, replace: &str, regex: Option<bool>) -> "search.replace_file"
    );
    /// 全体検索（`search.project`）。
    pub fn project(&mut self, query: &str) -> Result<Value> {
        self.0.run("search.project", json!({ "query": query }))
    }
    op!(
        // 次の候補（`search.find_next`）。
        pub fn find_next() -> "search.find_next"
    );
    op!(
        // 前の候補（`search.find_prev`）。
        pub fn find_prev() -> "search.find_prev"
    );
}

impl<'s> IdeaOps<'s> {
    op!(
        // 着想一覧（`idea.list`）。
        pub fn list() -> "idea.list"
    );
    /// 着想の記録（`idea.capture`）。
    pub fn capture(&mut self, title: &str, kind: &str) -> Result<Value> {
        self.0
            .run("idea.capture", json!({ "title": title, "kind": kind }))
    }
    op!(
        // 着想の昇格（`idea.promote`）。
        pub fn promote(id: &str, target: &str, to: Option<&str>) -> "idea.promote"
    );
    op!(
        // 着想の削除（`idea.drop`）。
        pub fn drop(id: &str) -> "idea.drop"
    );
    op!(
        // 着想メモの書き込み（`idea.write`）。
        pub fn write(id: &str, text: &str) -> "idea.write"
    );
}

impl<'s> AgentOps<'s> {
    op!(
        // タスク一覧（`agent.task_list`）。
        pub fn task_list() -> "agent.task_list"
    );
    op!(
        // タスクの起票（`agent.task_create`）。
        pub fn task_create(
            role: &str,
            scope: Option<&str>,
            intent: Option<&str>,
            comment: Option<&str>,
            examples: Option<&str>,
        ) -> "agent.task_create"
    );
    op!(
        // タスク 1 件の詳細（`agent.task_get`）。
        pub fn task_get(id: &str) -> "agent.task_get"
    );
    op!(
        // タスクの実行（`agent.task_run`。LLM 呼び出し）。
        pub fn task_run(id: &str, role: Option<&str>) -> "agent.task_run"
    );
    op!(
        // ドライラン（`agent.task_dry_run`）。
        pub fn task_dry_run(id: &str) -> "agent.task_dry_run"
    );
    op!(
        // タスクの中止（`agent.task_stop`）。
        pub fn task_stop(id: &str) -> "agent.task_stop"
    );
    op!(
        // 全実行の中止（`agent.stop_all`）。
        pub fn stop_all() -> "agent.stop_all"
    );
    op!(
        // 提案一覧（`agent.proposal_list`）。
        pub fn proposal_list() -> "agent.proposal_list"
    );
    op!(
        // 提案 1 件（`agent.proposal_get`）。
        pub fn proposal_get(id: &str) -> "agent.proposal_get"
    );
    op!(
        // 提案の適用（`agent.proposal_apply`）。
        pub fn proposal_apply(id: &str) -> "agent.proposal_apply"
    );
    op!(
        // 提案の却下（`agent.proposal_reject`）。
        pub fn proposal_reject(id: &str, comment: &str) -> "agent.proposal_reject"
    );
    op!(
        // 委譲レベルの参照（`agent.delegation_get`）。
        pub fn delegation_get() -> "agent.delegation_get"
    );
    op!(
        // 委譲レベルの変更（`agent.delegation_set`）。
        pub fn delegation_set(role: Option<&str>, level: &str) -> "agent.delegation_set"
    );
    op!(
        // MCP 接続設定の参照（`agent.mcp_config_get`）。
        pub fn mcp_config_get() -> "agent.mcp_config_get"
    );
    op!(
        // MCP 接続設定の検証（`agent.mcp_config_validate`）。
        pub fn mcp_config_validate(text: Option<&str>) -> "agent.mcp_config_validate"
    );
    op!(
        // MCP 接続設定の書き込み（`agent.mcp_config_set`）。
        pub fn mcp_config_set(text: &str) -> "agent.mcp_config_set"
    );
    op!(
        // メールスレッド一覧（`agent.thread_list`）。
        pub fn thread_list() -> "agent.thread_list"
    );
    op!(
        // スレッド 1 件（`agent.thread_get`）。
        pub fn thread_get(id: &str) -> "agent.thread_get"
    );
    op!(
        // 監査ログ一覧（`agent.audit_list`）。
        pub fn audit_list() -> "agent.audit_list"
    );
}

impl<'s> ExportOps<'s> {
    /// テキストのエクスポート（`export.text`。§17.2）。
    pub fn text(&mut self, query: ExportQuery) -> Result<Value> {
        let payload = json!({
            "range": query.range,
            "state": query.state,
            "format": query.format,
            "out": query.out,
            "blurb": query.blurb,
            "blurb_only": query.blurb_only,
            "keep_illustration_memo": query.keep_illustration_memo,
            "stock": query.stock,
        });
        self.0.run("export.text", payload)
    }
}

impl<'s> DiagnosticsOps<'s> {
    op!(
        // 診断（`diagnostics.run`。スコープ `single` / `file` / `full`）。
        pub fn run(scope: Option<&str>, path: Option<&str>, scene: Option<&str>) -> "diagnostics.run"
    );
    op!(
        // 地名解決（`diagnostics.place`）。
        pub fn place(path: Option<&str>, pos: Option<usize>) -> "diagnostics.place"
    );
}

impl<'s> SessionOps<'s> {
    op!(
        // 現在のワークスペース（`session.workspace_get`）。
        pub fn workspace_get() -> "session.workspace_get"
    );
    op!(
        // ワークスペースの切替（`session.workspace_switch`）。
        pub fn workspace_switch(workspace: &str) -> "session.workspace_switch"
    );
    op!(
        // 最近開いた文書の循環（`session.recent_document`）。
        pub fn recent_document(back: Option<bool>) -> "session.recent_document"
    );
    /// パレットコマンドの実行（`session.command`。§19.2.3）。
    pub fn command(&mut self, name: &str) -> Result<Value> {
        self.0.run("session.command", json!({ "name": name }))
    }
    /// イベントキューの取得（`session.poll_events`）。
    pub fn poll_events(&mut self) -> Result<Vec<AppEvent>> {
        Ok(self.0.poll_events())
    }
}

impl<'s> ApiOps<'s> {
    /// 正規操作カタログ（`api.list`。§19.8.2）。
    pub fn list(&mut self) -> Result<Value> {
        self.0.run("api.list", json!({}))
    }
}

impl<'s> HelpOps<'s> {
    op!(
        // ヘルプ本文（`help.topic`）。
        pub fn topic(topic: &str) -> "help.topic"
    );
}

impl<'s> NavOps<'s> {
    op!(
        // 文書・ファイルの直接指定（`nav.file`）。
        pub fn file(path: &str, scene: Option<&str>) -> "nav.file"
    );
    op!(
        // セクションへの移動（`nav.section`）。
        pub fn section(scene: &str) -> "nav.section"
    );
    op!(
        // 行への移動（`nav.jump`）。
        pub fn jump(line: Option<usize>, query: Option<&str>) -> "nav.jump"
    );
    op!(
        // 前の位置（`nav.prev`）。
        pub fn prev() -> "nav.prev"
    );
    op!(
        // 次の位置（`nav.next`）。
        pub fn next_scene() -> "nav.next"
    );
    op!(
        // 親ディレクトリへ（`nav.back`）。
        pub fn back() -> "nav.back"
    );
    op!(
        // 子の先頭へ（`nav.forward`）。
        pub fn forward() -> "nav.forward"
    );
    op!(
        // 1 つ上の階層へ（`nav.up`）。
        pub fn up() -> "nav.up"
    );
    op!(
        // 切替パネルの一覧（`nav.switcher`）。
        pub fn switcher() -> "nav.switcher"
    );
}

impl<'s> DocumentOps<'s> {
    /// ドキュメント全文の取得（`document.get`）。
    pub fn get(&mut self, path: &str) -> Result<String> {
        let found = self.0.run("document.get", json!({ "path": path }))?;
        Ok(found
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    }
    op!(
        // ドキュメント全文の書き込み（`document.write`）。
        pub fn write(path: &str, text: &str) -> "document.write"
    );
    op!(
        // 文書のリネーム（`document.rename`）。
        pub fn rename(from: &str, to: &str) -> "document.rename"
    );
}

impl<'s> PlotOps<'s> {
    op!(
        // プロットの取得（`plot.get`）。
        pub fn get(level: &str, scene: Option<&str>, chapter: Option<u16>) -> "plot.get"
    );
    op!(
        // プロットの書き込み（`plot.write`）。
        pub fn write(level: &str, text: &str, scene: Option<&str>, chapter: Option<u16>) -> "plot.write"
    );
    op!(
        // 確認状態の切替（`plot.set_state`）。
        pub fn set_state(scene: &str, state: Option<bool>) -> "plot.set_state"
    );
    op!(
        // 話プロットへの節挿入（`plot.insert_section`）。
        pub fn insert_section(scene: Option<&str>, title: Option<&str>) -> "plot.insert_section"
    );
    op!(
        // 章→話の展開・折り畳み（`plot.outline_fold`）。
        pub fn outline_fold(chapter: u16, fold: bool) -> "plot.outline_fold"
    );
}

impl<'s> MaterialsOps<'s> {
    op!(
        // 素材インボックス一覧（`materials.list`）。
        pub fn list() -> "materials.list"
    );
    op!(
        // 素材カードの取得（`materials.get`）。
        pub fn get(id: &str) -> "materials.get"
    );
    op!(
        // 素材の採用（`materials.approve`）。
        pub fn approve(id: &str) -> "materials.approve"
    );
    op!(
        // 素材の却下（`materials.reject`）。
        pub fn reject(id: &str, note: Option<&str>) -> "materials.reject"
    );
    op!(
        // 素材カードの編集（`materials.update`）。
        pub fn update(id: &str, review: Value) -> "materials.update"
    );
    op!(
        // 素材の添付（`materials.attach`）。
        pub fn attach(id: &str, path: Option<&str>, text: Option<&str>) -> "materials.attach"
    );
    op!(
        // 素材の検索（`materials.search`）。
        pub fn search(query: &str, category: Option<&str>, scope: Option<&str>) -> "materials.search"
    );
    op!(
        // 素材の提案（`materials.propose`）。
        pub fn propose(
            title: &str,
            source: Option<&str>,
            markdown: Option<&str>,
            category: Option<&str>,
        ) -> "materials.propose"
    );
}

impl<'s> CastOps<'s> {
    op!(
        // キャラクター情報・有効なモード（`cast.get`）。
        pub fn get(scene: &str) -> "cast.get"
    );
    op!(
        // キャラクター一覧（`cast.list`）。
        pub fn list() -> "cast.list"
    );
    op!(
        // 呼称タイムライン（`cast.timeline`）。
        pub fn timeline(index: Option<usize>) -> "cast.timeline"
    );
    op!(
        // 本文スキャンで候補抽出（`cast.scan`）。
        pub fn scan() -> "cast.scan"
    );
}

impl<'s> DictionaryOps<'s> {
    op!(
        // 辞書の一覧（`dictionary.list`）。
        pub fn list() -> "dictionary.list"
    );
    op!(
        // 用語の辞書登録（`dictionary.register_term`）。
        pub fn register_term(term: &str, definition: Option<&str>) -> "dictionary.register_term"
    );
    op!(
        // キャラクターの辞書登録（`dictionary.register_character`）。
        pub fn register_character(name: &str, reading: Option<&str>) -> "dictionary.register_character"
    );
    op!(
        // 用語の完全情報（`dictionary.term_full`）。
        pub fn term_full(query: &str) -> "dictionary.term_full"
    );
    op!(
        // 全モード一覧（`dictionary.modes_list`）。
        pub fn modes_list() -> "dictionary.modes_list"
    );
}

impl<'s> LangOps<'s> {
    op!(
        // 字句解析（`lang.tokens`）。
        pub fn tokens(path: Option<&str>, scene: Option<&str>) -> "lang.tokens"
    );
    op!(
        // 補完候補（`lang.completion`）。
        pub fn completion(path: &str, pos: usize) -> "lang.completion"
    );
    op!(
        // カーソル位置の情報（`lang.hover`）。
        pub fn hover(path: &str, pos: usize) -> "lang.hover"
    );
    op!(
        // インラインヒント（`lang.hints`）。
        pub fn hints(path: &str, start: Option<usize>, end: Option<usize>) -> "lang.hints"
    );
    op!(
        // クイックフィックス候補（`lang.code_action`）。
        pub fn code_action(path: &str, start: usize, end: usize) -> "lang.code_action"
    );
    op!(
        // 定義へジャンプ（`lang.definition`）。
        pub fn definition(path: &str, pos: usize) -> "lang.definition"
    );
    op!(
        // 参照検索（`lang.references`）。
        pub fn references(path: &str, pos: usize) -> "lang.references"
    );
    op!(
        // シンボル一覧（`lang.symbols`）。
        pub fn symbols(path: &str) -> "lang.symbols"
    );
    op!(
        // 整形（`lang.format`）。
        pub fn format(path: &str, apply: Option<bool>) -> "lang.format"
    );
    op!(
        // リネームの影響範囲（`lang.rename_preview`）。
        pub fn rename_preview(path: &str, pos: usize, to: Option<&str>) -> "lang.rename_preview"
    );
    op!(
        // リネームの実行（`lang.rename_apply`）。
        pub fn rename_apply(path: &str, pos: usize, to: &str) -> "lang.rename_apply"
    );
    op!(
        // 折り畳み範囲（`lang.folding`）。
        pub fn folding(path: &str) -> "lang.folding"
    );
}

impl<'s> ReviewOps<'s> {
    op!(
        // 見直し項目一覧（`review.list`）。
        pub fn list() -> "review.list"
    );
    op!(
        // 見直し項目への移動（`review.goto`）。
        pub fn goto(index: usize) -> "review.goto"
    );
}

impl<'s> PromptOps<'s> {
    op!(
        // プロンプトファイル一覧（`prompt.list`）。
        pub fn list() -> "prompt.list"
    );
    op!(
        // プロンプト内容の取得（`prompt.get`）。
        pub fn get(name: &str) -> "prompt.get"
    );
    op!(
        // 初期版との差分（`prompt.diff`）。
        pub fn diff(name: &str) -> "prompt.diff"
    );
    op!(
        // プロンプトの初期化（`prompt.reset`）。
        pub fn reset(name: &str) -> "prompt.reset"
    );
}

impl<'s> ContextOps<'s> {
    op!(
        // コンテキストパケットの構築（`context.build`）。
        pub fn build(role: &str, scene: &str) -> "context.build"
    );
}

impl<'s> MailOps<'s> {
    op!(
        // エージェント間のメール（`mail.send`）。
        pub fn send(to: &str, body: &str, kind: Option<&str>, scope: Option<&str>) -> "mail.send"
    );
}

impl<'s> SettingsOps<'s> {
    op!(
        // 設定の取得（`settings.get`）。
        pub fn get() -> "settings.get"
    );
    op!(
        // 設定の変更（`settings.set`）。
        pub fn set(key: &str, value: Value) -> "settings.set"
    );
}

/// コマンド名 → `Command`（`session.command` と CLI が共有する）。
pub fn command_of_name(name: &str) -> Option<(Command, &'static str)> {
    crate::app::commands::PALETTE_COMMANDS
        .iter()
        .chain(crate::app::commands::SHORTCUT_PALETTE_COMMANDS.iter())
        .find(|(key, _, _)| *key == name)
        .map(|(_, label, command)| (*command, *label))
}

/// パレットコマンドの全キー（§19.2.3 の一覧）。
pub fn command_names() -> Vec<&'static str> {
    crate::app::commands::PALETTE_COMMANDS
        .iter()
        .chain(crate::app::commands::SHORTCUT_PALETTE_COMMANDS.iter())
        .map(|(key, _, _)| *key)
        .collect()
}

/// 構造移動（`nav.*` が共有する）。
pub(crate) fn nav_of(name: &str) -> Option<Nav> {
    Some(match name {
        "prev-section" => Nav::PrevSection,
        "next-section" => Nav::NextSection,
        "prev-scene" => Nav::PrevScene,
        "next-scene" => Nav::NextScene,
        "prev-file" => Nav::PrevFile,
        "next-file" => Nav::NextFile,
        "file-start" => Nav::FileStart,
        "file-end" => Nav::FileEnd,
        _ => return None,
    })
}

/// ワークスペース名 → `Workspace`。
pub fn workspace_of(name: &str) -> Option<Workspace> {
    Some(match name {
        "write" => Workspace::Write,
        "plot-episode" => Workspace::PlotEpisode,
        "plot-outline" => Workspace::PlotOutline,
        "idea" => Workspace::Idea,
        "material" => Workspace::Material,
        "cast" => Workspace::Cast,
        "reader" => Workspace::Reader,
        "agent" => Workspace::Agent,
        "review" => Workspace::Review,
        _ => return None,
    })
}

/// `Workspace` → 名前。
pub fn workspace_name(workspace: Workspace) -> &'static str {
    match workspace {
        Workspace::Write => "write",
        Workspace::PlotEpisode => "plot-episode",
        Workspace::PlotOutline => "plot-outline",
        Workspace::Idea => "idea",
        Workspace::Material => "material",
        Workspace::Cast => "cast",
        Workspace::Reader => "reader",
        Workspace::Agent => "agent",
        Workspace::Review => "review",
    }
}

/// 引数の文字列（無ければ `Err(usage)`）。
pub(crate) fn str_arg(locale: Locale, args: &Args, name: &str) -> Result<String> {
    match args.get(name) {
        Some(Value::String(text)) => Ok(text.clone()),
        Some(Value::Number(number)) => Ok(number.to_string()),
        Some(Value::Bool(flag)) => Ok(flag.to_string()),
        Some(_) => Err(Error::usage(locale, name)),
        None => Err(Error::missing(locale, name)),
    }
}

/// 引数の文字列（無ければ既定値）。
pub(crate) fn str_or<'a>(args: &'a Args, name: &str, default: &'a str) -> &'a str {
    args.get(name).and_then(Value::as_str).unwrap_or(default)
}

/// 引数の真偽（無ければ既定値）。
pub(crate) fn bool_or(args: &Args, name: &str, default: bool) -> bool {
    args.get(name).and_then(Value::as_bool).unwrap_or(default)
}

/// 引数の整数（無ければ既定値）。
pub(crate) fn usize_or(args: &Args, name: &str, default: usize) -> usize {
    args.get(name)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(default)
}

/// 引数の整数（`Option`）。
pub(crate) fn usize_opt(args: &Args, name: &str) -> Option<usize> {
    args.get(name)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

/// 4 階層 ID を読む（`@01-04-02-03` / `01-04-02-03` / `01`）。
pub(crate) fn scene_arg(locale: Locale, args: &Args, name: &str) -> Result<SceneId> {
    use crate::core::id::PartialSceneId;
    let text = str_arg(locale, args, name)?;
    let parsed = PartialSceneId::parse_with_sigil(text.trim())
        .or_else(|_| PartialSceneId::parse(text.trim()))
        .map_err(|_| Error::usage(locale, name))?;
    if parsed.depth() != 4 {
        return Err(Error::usage(locale, name));
    }
    Ok(parsed.expand_low())
}

/// 4 階層 ID を読む（部分 ID も許す。`plot.get` の `chapter` 指定など）。
pub fn canonical(value: &Value) -> String {
    let mut buffer = Vec::new();
    write_canonical(&mut buffer, value);
    String::from_utf8(buffer).unwrap_or_else(|_| value.to_string())
}

fn write_canonical(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push(b'{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                serde_json::to_writer(&mut *out, key).expect("a string key serializes");
                out.push(b':');
                write_canonical(out, &map[key]);
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_canonical(out, item);
            }
            out.push(b']');
        }
        other => serde_json::to_writer(&mut *out, other).expect("a JSON value serializes"),
    }
}
