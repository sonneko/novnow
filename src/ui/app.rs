//! GUI の合成の根（`eframe::App`）。§9 の共通シェル・§14.4 のプロジェクト選択・§8 の保存をつなぐ。
//!
//! 描画とキー操作はここと `ui::shell`・`ui::widgets`、状態の規則は egui に依存しない
//! `ui::workbench`・`ui::save_machine`・`ui::settings`・`ui::palette` に置く（テストはそちら）。
//!
//! 規則 10: eframe の `persistence` feature は使わない（ウィンドウの位置も覚えない）。書くのは
//! プロジェクトの中（本文・`.novnow/ui.json`）と、デスクトップのパス履歴（`RecentFile`）だけ。

use std::path::PathBuf;

use egui::{
    Align, Align2, Color32, Context, FontFamily, FontId, Frame, Id, Key, Layout, Margin, Modifiers,
    Pos2, Rect, RichText, Sense, Stroke, Vec2,
};

use crate::core::diagnostics::Diagnostic;
use crate::core::export::{self as export_text, State as ExportState};
use crate::core::features::Project;
use crate::ports::dialog::ChosenPath;
use crate::ui::i18n::{self, Locale};
use crate::ui::palette;
use crate::ui::save_machine::{Indicator, State};
use crate::ui::settings::{LanguageChoice, PaneSide, UiSettings};
use crate::ui::shell::{self, Command, ImeState};
use crate::ui::theme::{self, Mode};
use crate::ui::widgets::{context_view, reference_pane, section_editor, slice_editor};
use crate::ui::workbench::Workbench;
use crate::ui::workspaces::{self, Workspace};

#[cfg(not(target_arch = "wasm32"))]
pub use native::run;

/// 開いたプロジェクトに対する I/O（デスクトップはファイル、Web は IndexedDB）。
pub trait Host {
    /// ファイルを書く（プロジェクトの中）。失敗は表示用の文言。
    fn write(&mut self, path: &str, text: &str) -> Result<(), String>;
    /// 外部変更の監視 1 回分（§8.5）: 変わったファイルと中身（消えたら `None`）。
    fn poll(&mut self) -> Vec<(String, Option<String>)>;
    /// プロジェクトの名前（ヘッダ・ウィンドウ題名）。
    fn name(&self) -> String;
    /// 今の日時 `YYYYMMDD-HHMM`（テキストのエクスポートの既定の名前、§17.2）。
    fn stamp(&self) -> String;
    /// 今の日時（RFC 3339。アイデアの `created`・資料の `saved_at` / `updated_at`）。
    fn now(&self) -> String;
    /// プロジェクトの中のファイルを消す（資料の採用で `inbox/` から `docs/` へ動かすとき、§16.1）。
    fn remove(&mut self, path: &str) -> Result<(), String> {
        Err(format!("cannot remove {path}"))
    }
    /// 「名前を付けて保存」の保存ダイアログ（§17.2・§13.1.1 の特例）。`suggested` は既定のファイル名。
    /// 取り消されたとき・ダイアログを持たない環境では `None`。
    fn choose_export(&mut self, suggested: &str) -> Option<ChosenPath> {
        let _ = suggested;
        None
    }
    /// 選ばれた保存先にテキストを書く（関門の特例の口だけを通る）。書いた場所（表示用）を返す。
    fn write_export(&mut self, chosen: ChosenPath, text: &str) -> Result<String, ExportFailure> {
        let _ = text;
        Err(ExportFailure::Failed(
            chosen.path().to_string_lossy().into_owned(),
        ))
    }
    /// プロジェクトの中のファイルに追記する（監査ログ `agents/log/*.jsonl`、§5.10.5）。
    fn append(&mut self, path: &str, text: &str) -> Result<(), String> {
        let _ = text;
        Err(format!("cannot append to {path}"))
    }
    /// LLM（stdio の MCP サーバ）を呼べるか（Web は呼べない、§15.8.2）。
    fn mcp_available(&self) -> bool {
        false
    }
    /// MCP の呼び出しを始める（別のスレッドで走る）。番号を返す。呼べなければ `None`。
    fn mcp_start(&mut self, job: crate::ui::agents::Job) -> Option<u64> {
        let _ = job;
        None
    }
    /// 終わった呼び出し（[`crate::ui::agents::Finished`]）。
    fn mcp_poll(&mut self) -> Vec<crate::ui::agents::Finished> {
        Vec::new()
    }
    /// 呼び出しを取り消す。
    fn mcp_cancel(&mut self, job: u64) {
        let _ = job;
    }
    /// 環境変数（`agents/mcp.json` の `${NAME}` を解決する。値は画面にもログにも出さない）。
    fn env_var(&self, name: &str) -> Option<String> {
        let _ = name;
        None
    }
    /// ディスク上にその名前の何か（ファイル・ディレクトリ・リンク）がある。一覧に無いのにあるもの（外を指す
    /// シンボリックリンク）には既定のプロンプトを書かない（`PLANS.md` §9.2 B-155）。
    fn occupied(&self, path: &str) -> bool {
        let _ = path;
        false
    }
    /// 保存 1 回分（§8.3）: `writes` を書き、全部書けたら `removals` を消す。同期の Host（デスクトップ・
    /// テスト）は既定のまま、その場で書いて結果を返す。非同期の Host（Web の IndexedDB）は始めるだけで
    /// `None` を返し、終わったら [`Host::save_finished`] が結果を返す（`PLANS.md` §9.2 B-156）。
    fn save(
        &mut self,
        writes: Vec<(String, String)>,
        removals: Vec<String>,
    ) -> Option<SaveOutcome> {
        Some(save_now(self, writes, removals))
    }
    /// 非同期の保存（[`Host::save`] が `None` を返したもの）の結果。終わっていなければ `None`。
    fn save_finished(&mut self) -> Option<SaveOutcome> {
        None
    }
    /// 後から分かったこと（Web の非同期の書き込みの失敗・Zip の書き出しの結果など）。毎フレーム読む。
    fn take_events(&mut self) -> Vec<HostEvent> {
        Vec::new()
    }
    /// 未保存の変更があるか（毎フレーム。Web はタブを閉じるときの確認に使う）。
    fn unsaved_changed(&mut self, unsaved: bool) {
        let _ = unsaved;
    }
    /// Zip でエクスポートする（§13.5）。`name` は既定のファイル名（`<題>.zip`）。デスクトップは保存
    /// ダイアログで選んだ場所に書いてその場で結果を返す（取り消したら `Some(Ok(None))`）。Web は
    /// ダウンロードを始めるだけで `None` を返し、結果は [`Host::take_events`] で知らせる。
    fn export_zip(&mut self, name: &str) -> Option<Result<Option<String>, String>> {
        let _ = name;
        Some(Err("zip export is not available here".to_owned()))
    }
}

/// 保存 1 回分をその場で行う（[`Host::save`] の既定）: `writes` を書き、全部書けたら `removals` を消す
/// （動かした文書の元は、新しい側を書けてから消す）。
pub fn save_now<H: Host + ?Sized>(
    host: &mut H,
    writes: Vec<(String, String)>,
    removals: Vec<String>,
) -> SaveOutcome {
    let mut outcome = SaveOutcome::default();
    for (path, text) in writes {
        match host.write(&path, &text) {
            Ok(()) => outcome.written.push(path),
            Err(message) => outcome.failures.push((path, message)),
        }
    }
    if !outcome.failures.is_empty() {
        outcome.removals_left = removals;
        return outcome;
    }
    for path in removals {
        if let Err(message) = host.remove(&path) {
            outcome.failures.push((path.clone(), message));
            outcome.removals_left.push(path);
        }
    }
    outcome
}

/// 保存 1 回分の結果（[`Host::save`]）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveOutcome {
    /// 書けたファイル。
    pub written: Vec<String>,
    /// 書けなかった（消せなかった）ファイルと、その理由。
    pub failures: Vec<(String, String)>,
    /// まだ消していない元のファイル（次の保存でもう一度）。
    pub removals_left: Vec<String>,
}

/// Host が後から知らせること（[`Host::take_events`]。文言は `NovnowApp` がカタログで引く）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvent {
    /// 保存の外で書いたもの（`ui.json`・監査ログ・`exports/`）が書けなかった。
    WriteFailed(String, String),
    /// Zip を書き出した（Web はダウンロードを始めた）。
    ZipExported(String),
    /// Zip を書き出せなかった。
    ZipFailed(String),
    /// 保存領域の空きが少ない（W062、§13.3.7）。
    LowSpace,
}

/// 「名前を付けて保存」に失敗した理由（文言は `NovnowApp` がカタログで引く）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportFailure {
    /// 関門が拒んだ（E060: 名前が `.txt` / `.md` で終わらない・履歴ファイル。§13.1.1）。
    Refused(Diagnostic),
    /// 書けなかった（OS のメッセージ）。
    Failed(String),
}

/// フォーカス中のペイン（§9.2.3。外周 2px の `accent` 枠で示す）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Context,
    Editor,
    Reference,
}

/// 画面の上に重ねるもの（同時に 1 つ）。
#[derive(Debug, Clone, PartialEq)]
enum Overlay {
    None,
    Palette {
        input: String,
        selected: usize,
    },
    Jump {
        input: String,
        missed: bool,
    },
    Help,
    Welcome,
    Migration,
    McpConfig {
        input: String,
        error: Option<String>,
    },
    SaveError,
    CloseGuard,
    /// `Ctrl+F` / `Ctrl+H`（`replace` が `Some` なら置換も）。背景を暗くしない。
    /// `fresh` は開いた直後のフレーム（検索欄にフォーカスを移す）。
    Find {
        query: String,
        replace: Option<String>,
        fresh: bool,
    },
    /// `Ctrl+Shift+F`。
    Search {
        query: String,
        selected: usize,
    },
    /// `Ctrl+Space` / `'` の直後。キャレットの下に出す（背景を暗くしない）。
    Completion {
        selected: usize,
    },
    /// `Ctrl+Shift+M` / `Ctrl+Shift+W`。キャレットの下に出す。
    Info {
        lines: Vec<String>,
    },
    /// `Ctrl+Shift+;` の確認（§9.2.1-2: 破壊的操作の確認には文字ラベルを置いてよい）。
    ConfirmDelete,
    /// 開けなかったときなどの表示（§14.2.2 と同じ内容）。
    Message(String),
    /// `Ctrl+.`。キャレットの下に出す。
    QuickFix {
        actions: Vec<crate::core::features::code_action::CodeAction>,
        selected: usize,
    },
    /// `Ctrl+Shift+D`（§8.5）。
    Diff,
    /// `>export`（§17.2）。範囲（空欄なら全体）と状態の絞り込み。`fresh` は開いた直後のフレーム。
    Export {
        range: String,
        state: Option<ExportState>,
        fresh: bool,
    },
    /// 本文の最大幅設定。
    LineWidth {
        input: String,
    },
    /// `Ctrl+Shift+I` アイデアの即時入力（§10.4。どのワークスペースからでも、画面遷移なし）。
    Capture {
        input: String,
    },
    /// `idea` の `Ctrl+Enter` 昇格の行き先（§10.4）。`characters` はキャラのメモを選んだ後のキャラ名。
    Promote {
        selected: usize,
        characters: Option<Vec<String>>,
    },
    /// `Ctrl+M` 資料検索（§16.3）。`create` は `Ctrl+Enter`（新しい資料を作って挿入）が押された。
    Materials {
        query: String,
        selected: usize,
        create: bool,
    },
    /// 出典の無い資料の採用の確認（§16.7「採用ボタンに確認を挟む」。§9.2.1-2 の文字ラベルを置く）。
    ConfirmApprove {
        id: String,
    },
    /// `material` の `Ctrl+Shift+N` 新しい資料（提案）のタイトル。
    NewMaterial {
        input: String,
    },
    /// agent の `N` 新しいタスク（§15.7.1。役割は `↑` / `↓`、対象・目的・コメントは入力欄）。
    AgentNew {
        role: usize,
        scope: String,
        intent: String,
        comment: String,
        examples: String,
        fresh: bool,
    },
    /// agent の `Shift+R` の確認（§15.7.5 (1)「n 件 / 合計入力 m 文字 / 使用サーバ」）。
    AgentConfirm {
        ids: Vec<String>,
        chars: usize,
        servers: Vec<String>,
        blocked: Vec<String>,
    },
    /// 却下・差し戻しのコメント（必須、§15.7.3）。
    AgentComment {
        proposal: String,
        revert: bool,
        input: String,
        missing: bool,
    },
    /// 確定済みの節への提案の採用の確認（§15.9「採用ボタンに確認を 1 段挟む」）。
    AgentFixed {
        proposal: String,
        text: Option<String>,
    },
    /// `>prompt reset <name>`（§15.5.4 (3)。現行版と既定版を並べ、元に戻せないことを書く）。
    PromptReset {
        name: String,
    },
    /// `>prompt diff <name>`（§15.5.4 (4)。並べて見るだけ）。
    PromptDiff {
        name: String,
    },
    /// `F2` / `>rename` キャラ名・用語の一括改名（§3.4）。
    Rename {
        document: crate::core::model::document::Document,
        pos: usize,
        input: String,
        error: Option<crate::core::features::rename::RenameError>,
    },
    /// `'名前` の `Shift+Enter` / `Shift+F12` / `>references` 使用箇所の一覧（§3.4・§9.7.1）。
    References {
        name: String,
        locations: Vec<crate::core::features::Location>,
        selected: usize,
    },
}

/// `Ctrl+M` で選んだ資料をどうするか（§9.8.3・§16.3、`PLANS.md` §9.2 B-134）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum MaterialAction {
    /// `Enter`: 今のワークスペースのエディタに挿入する（write・アイデアなどは `'M0137`、話プロットは
    /// `material: 'M0137` の行。material と review では開く）。
    Insert(String),
    /// `Shift+Enter`: material ワークスペースで開く。
    Open(String),
    /// `Ctrl+Enter`: 検索語から新しい資料（提案）を作って挿入する。
    Create(String),
}

/// 開いているプロジェクト。
pub struct Session {
    host: Box<dyn Host>,
    bench: Workbench,
    settings_dirty: bool,
    last_poll: f64,
    context_cache: context_view::Cache,
    editor_cache: section_editor::Cache,
    reference_cache: reference_pane::Cache,
    diag_count: Option<(u64, String, usize)>,
    notice: Option<(String, f64)>,
    font_toast: Option<(String, f64)>,
    workspace_switched_at: Option<f64>,
    /// 節エディタの選択範囲（節の本文の中の byte。前のフレームの）。
    selection: Option<std::ops::Range<usize>>,
    /// キャレットの矩形（前のフレームの）。
    caret: Option<Rect>,
    /// 次のフレームで節エディタに設定する選択範囲（検索の一致・補完の確定）。
    pending_selection: Option<std::ops::Range<usize>>,
    /// 最近開いたファイル（先頭が今。`Ctrl+Tab`）。
    recent: Vec<(String, crate::core::id::SceneId)>,
    /// 最後に検索した文字列（`F3`）。
    last_find: String,
    /// 全体検索の結果（文字列ごとに作り直す）。
    search_cache: Option<(String, u64, Vec<crate::ui::assist::Hit>)>,
    /// `Ctrl+M` の結果（文字列と文書の世代ごとに作り直す）。
    material_cache: Option<(String, u64, Vec<crate::core::features::materials_ops::Hit>)>,
    /// 衝突したファイルの外部の版（§8.5。消えたなら `None`）。
    conflict_texts: std::collections::BTreeMap<String, Option<String>>,
    /// エクスポートの下書き（文書の世代・範囲・状態が同じあいだは作り直さない）。
    export_cache: Option<ExportDraft>,
    /// ワークスペースごとの状態（§10.0。切替で破棄しない）。
    spaces: workspaces::Spaces,
    /// ステータスバーの `⚑` が押された（次に点検の診断を開く。§9.6.1）。
    open_review: bool,
    /// エージェントの実行の状態（§15.7.4・§15.7.5）。
    agents: crate::ui::agents::Runtime,
    /// ステータスバーの `⟳` が押された（次に agent ワークスペースを開く。§15.7.4）。
    open_agent: bool,
    /// 書き込み中の保存で渡した中身（非同期の保存のあいだも編集できるので、終わったときに比べる。B-156）。
    saving: Option<std::collections::BTreeMap<String, String>>,
    /// 保存領域が足りないことを知らせた（同じ失敗のあいだは何度も出さない、§13.3.7）。
    quota_shown: bool,
}

/// エクスポートの下書き（`>export` のモーダルが毎フレーム作り直さないため）。
struct ExportDraft {
    /// 鍵: 文書の世代・範囲の文字列・状態。
    key: (u64, String, Option<ExportState>),
    /// 範囲が読めなければ `None`。
    range: Option<Option<crate::core::id::RangeExpr>>,
    /// 出す節の数。
    sections: usize,
    /// 書き出した文字列（`.txt` の形）。
    text: String,
}

impl Session {
    /// 読み込んだプロジェクトから作る。
    pub fn new(host: Box<dyn Host>, project: Project, settings: &UiSettings) -> Self {
        let bench = Workbench::new(
            project,
            settings.last_scene.as_deref(),
            settings.autosave_ms,
        );
        Self {
            host,
            bench,
            settings_dirty: false,
            last_poll: 0.0,
            context_cache: context_view::Cache::default(),
            editor_cache: section_editor::Cache::default(),
            reference_cache: reference_pane::Cache::default(),
            diag_count: None,
            notice: None,
            font_toast: None,
            workspace_switched_at: None,
            selection: None,
            caret: None,
            pending_selection: None,
            recent: Vec::new(),
            last_find: String::new(),
            search_cache: None,
            material_cache: None,
            conflict_texts: std::collections::BTreeMap::new(),
            export_cache: None,
            spaces: workspaces::Spaces::default(),
            open_review: false,
            agents: crate::ui::agents::Runtime::default(),
            open_agent: false,
            saving: None,
            quota_shown: false,
        }
    }

    /// 開いたときのエージェントの準備（U-7・§15.5.4）: `prompts/` の欠けた既定ファイルを書き出す。
    /// `prompts/` が丸ごと無かったら I050 を知らせる。エージェントが無効なら何もしない。
    pub fn prepare_agents(&mut self, locale: Locale) {
        if let Some(message) =
            crate::ui::agents::write_prompts(&mut self.bench, self.host.as_ref(), 0.0, locale)
        {
            self.notice = Some((message, 0.0));
        }
    }

    /// エージェントの実行の状態（テスト用の読み口）。
    pub fn agents(&self) -> &crate::ui::agents::Runtime {
        &self.agents
    }

    /// エクスポートの下書きを（必要なら作り直して）返す（§17.2）。
    fn export_draft(&mut self, range: &str, state: Option<ExportState>) -> &ExportDraft {
        let key = (self.bench.generation(), range.trim().to_owned(), state);
        if self
            .export_cache
            .as_ref()
            .is_none_or(|draft| draft.key != key)
        {
            let project = self.bench.project();
            let parsed = if key.1.is_empty() {
                Some(None)
            } else {
                export_text::parse_range(&key.1).ok().map(Some)
            };
            let (sections, text) = match &parsed {
                Some(range) => {
                    let mut options =
                        export_text::Options::from_settings(&project.settings().export);
                    options.state = state;
                    let chosen = export_text::select(project, range.as_ref(), state);
                    let text = export_text::render(project, &chosen, &options);
                    (chosen.len(), text)
                }
                None => (0, String::new()),
            };
            self.export_cache = Some(ExportDraft {
                key,
                range: parsed,
                sections,
                text,
            });
        }
        self.export_cache.as_ref().expect("the draft was just made")
    }

    /// 既定の書き出し先の名前（§17.2 `<title>-<範囲>-<YYYYMMDD-HHMM>.txt`）。題名が空ならプロジェクトの名前。
    fn export_name(
        &self,
        range: Option<&crate::core::id::RangeExpr>,
        format: export_text::Format,
    ) -> String {
        let project = self.bench.project();
        let title = match project.settings().title.trim() {
            "" => self.host.name(),
            title => title.to_owned(),
        };
        let label = export_text::range_label(range, project.digits());
        export_text::file_name(&title, &label, &self.host.stamp(), format)
    }

    /// 選択範囲（無ければキャレットの位置、それも無ければ節の先頭）のファイルの中の byte 範囲。
    fn file_selection(&self) -> Option<(String, std::ops::Range<usize>)> {
        let (document, start) = self.bench.spliced_document()?;
        let selection = self.selection.clone().unwrap_or(0..0);
        Some((
            document.path,
            start + selection.start..start + selection.end,
        ))
    }

    /// 今のファイルで `query` の次 / 前の一致へ移る（`F3` / `Shift+F3` / 検索欄の `Enter`）。
    fn find_step(&mut self, query: &str, forward: bool) -> bool {
        self.bench.commit();
        let Some((path, here)) = self.file_selection() else {
            return false;
        };
        let text = self.bench.project().text(&path).to_owned();
        let matches = crate::ui::assist::find_all(&text, query);
        let Some(found) = crate::ui::assist::step(&matches, here, forward) else {
            return false;
        };
        self.last_find = query.to_owned();
        if let Some(offset) = self.bench.reveal(&path, found.start) {
            self.pending_selection = Some(offset..offset + found.len());
        }
        true
    }
}

/// 起動前の画面（§14.4 のプロジェクト選択）。
pub trait Picker {
    /// 描く。プロジェクトを開いたら `Some`。
    fn show(&mut self, ctx: &Context, locale: Locale, mode: Mode) -> Option<(Session, UiSettings)>;
    /// `Ctrl+O`（選択画面の中）。
    fn open_folder(&mut self);
    /// 選択画面に戻ってきた（一覧を読み直す）。
    fn refresh(&mut self) {}
    /// 選択画面での `Esc`（入力中の名前などをやめる。選択画面そのものは閉じない、§13.4）。
    fn escape(&mut self) {}
}

enum Stage {
    Picker,
    Open(Box<Session>),
}

/// novnow の GUI。
pub struct NovnowApp {
    stage: Stage,
    picker: Box<dyn Picker>,
    settings: UiSettings,
    env_locale: Locale,
    ime: ImeState,
    overlay: Overlay,
    focus: Pane,
    applied: Option<(Mode, u32)>,
    font_warning: bool,
    allow_close: bool,
    mac: bool,
    /// 次のフレームの頭で戻すフォーカス（矢印のショートカットの後。`frame` の注記）。
    refocus: Option<egui::Id>,
    /// 書き終えたら選択画面に戻る（Web / デスクトップの `Ctrl+O`）。
    leaving: bool,
}

impl NovnowApp {
    /// 起動。`picker` はプロジェクトを開く前の画面、`opened` は起動時に開けたプロジェクト。
    pub fn new(
        ctx: &Context,
        env_locale: Locale,
        picker: Box<dyn Picker>,
        opened: Option<(Session, UiSettings)>,
    ) -> Self {
        let (fonts, fallback) = theme::fonts();
        ctx.set_fonts(fonts);
        let (stage, settings) = match opened {
            Some((session, settings)) => (Stage::Open(Box::new(session)), settings),
            None => (Stage::Picker, UiSettings::default()),
        };
        Self {
            stage,
            picker,
            settings,
            env_locale,
            ime: ImeState::default(),
            overlay: Overlay::None,
            focus: Pane::Editor,
            applied: None,
            font_warning: fallback && !cfg!(target_arch = "wasm32"),
            allow_close: false,
            mac: ctx.os() == egui::os::OperatingSystem::Mac,
            refocus: None,
            leaving: false,
        }
    }

    /// 開いているプロジェクトの編集状態（テスト用の読み口）。
    pub fn workbench(&self) -> Option<&Workbench> {
        match &self.stage {
            Stage::Open(session) => Some(&session.bench),
            Stage::Picker => None,
        }
    }

    /// 今のワークスペース（プロジェクトを開いていなければ `None`）。
    pub fn workspace(&self) -> Option<Workspace> {
        match &self.stage {
            Stage::Open(session) => Some(session.spaces.switcher.current()),
            Stage::Picker => None,
        }
    }

    /// ワークスペースごとの状態（テスト用の読み口）。
    pub fn spaces(&self) -> Option<&workspaces::Spaces> {
        match &self.stage {
            Stage::Open(session) => Some(&session.spaces),
            Stage::Picker => None,
        }
    }

    /// エージェントの実行の状態（テスト用の読み口）。
    pub fn agents(&self) -> Option<&crate::ui::agents::Runtime> {
        match &self.stage {
            Stage::Open(session) => Some(&session.agents),
            Stage::Picker => None,
        }
    }

    /// 何か重ねて出しているか（パレット・ヘルプ・モーダル）。
    pub fn overlay_open(&self) -> bool {
        self.overlay != Overlay::None
    }

    /// `ui.json` の中身（今の値）。
    pub fn settings(&self) -> &UiSettings {
        &self.settings
    }

    /// IME の変換中か（§9.8.5）。
    pub fn composing(&self) -> bool {
        self.ime.composing()
    }

    /// 今の言語（§12.4: `ui.json` → `NOVNOW_LANG` → OS → ja）。
    pub fn locale(&self) -> Locale {
        match self.settings.language {
            LanguageChoice::Ja => Locale::Ja,
            LanguageChoice::En => Locale::En,
            LanguageChoice::System => self.env_locale,
        }
    }

    fn mode(&self, ctx: &Context, system_dark: Option<bool>) -> Mode {
        let _ = ctx;
        Mode::resolve(self.settings.theme, system_dark)
    }

    /// 1 フレーム（`eframe::App::update` から呼ぶ）。`system_dark` は OS の配色（分からなければ `None`）。
    pub fn frame(&mut self, ctx: &Context, system_dark: Option<bool>) {
        let mode = self.mode(ctx, system_dark);
        self.apply_style(ctx, mode);
        let now = ctx.input(|input| input.time);
        let locale = self.locale();
        // 前のフレームの矢印のショートカットで egui が動かしたフォーカスを戻す（下の注記）
        if let Some(id) = self.refocus.take() {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
        // egui は矢印キーで（修飾キーが付いていても）フォーカスを隣のウィジェットへ動かす（フォーカス中の
        // ウィジェットが矢印を取らないとき。フレームの終わりに動く）。novnow は矢印でのフォーカス移動を
        // 使わない（`Alt+↑/↓` などのショートカットと、一覧・タイムラインの選択に使う）ので、矢印が
        // 押されたフレームの終わりのフォーカスを、次のフレームの頭で戻す
        let arrow_pressed = ctx.input(|input| {
            input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight,
                        pressed: true,
                        ..
                    }
                )
            })
        });
        // §9.8.5 (3): IME → 変換中なら Key を捨てる → ショートカット
        let text_focused = ctx.wants_keyboard_input();
        let workspace = self.workspace().unwrap_or_default();
        let commands = ctx.input_mut(|input| {
            shell::take_commands_in(input, &mut self.ime, self.mac, text_focused, workspace)
        });
        let focus_lost = ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::WindowFocused(false)))
        });
        for command in commands {
            self.run(ctx, command, now);
        }
        if matches!(self.stage, Stage::Open(_))
            && self.overlay == Overlay::None
            && !self.ime.composing()
            && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::F2))
        {
            self.run(ctx, Command::Rename, now);
        }
        // 入力欄を持たない重ね物（確認・行き先の選択）が出ている間は、下のエディタにキーを渡さない
        // （`Enter` や矢印が、重ね物と同時にエディタの改行・カーソル移動にもなるため）。閉じたら戻す
        if matches!(
            self.overlay,
            Overlay::ConfirmDelete
                | Overlay::ConfirmApprove { .. }
                | Overlay::Promote { .. }
                | Overlay::AgentConfirm { .. }
                | Overlay::AgentFixed { .. }
                | Overlay::PromptReset { .. }
                | Overlay::PromptDiff { .. }
        ) {
            if let Some(id) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(id));
            }
        }
        match &mut self.stage {
            Stage::Picker => {
                if let Some((session, settings)) = self.picker.show(ctx, locale, mode) {
                    self.settings = settings;
                    self.stage = Stage::Open(Box::new(session));
                    self.overlay = Overlay::None;
                    self.focus = Pane::Editor;
                    ctx.request_repaint();
                }
            }
            Stage::Open(session) if self.leaving && session.saving.is_none() => {
                // `Ctrl+O`: 書き終えたら選択画面へ（書けなければ戻らずに知らせる）
                self.leaving = false;
                match session.bench.save.state() {
                    State::Clean => {
                        self.stage = Stage::Picker;
                        self.overlay = Overlay::None;
                        self.picker.refresh();
                        ctx.request_repaint();
                    }
                    State::Failed => self.overlay = Overlay::SaveError,
                    // まだ書くものがある（保存中に打った分）: もう一度書いてから
                    State::Dirty | State::Saving => {
                        session.bench.save.request_now();
                        self.leaving = true;
                        self.workbench_frame(ctx, mode, locale, now, focus_lost);
                    }
                }
            }
            Stage::Open(session) => {
                if self.font_warning {
                    // §11.5 SHOULD: 起動エラーにせず、警告を出して同梱フォントで続ける
                    self.font_warning = false;
                    session.font_toast =
                        Some((i18n::text(locale, "status.font_fallback").to_owned(), now));
                }
                self.workbench_frame(ctx, mode, locale, now, focus_lost);
            }
        }
        if let Stage::Open(session) = &self.stage {
            if let Some((text, started)) = &session.font_toast {
                if now - started < 4.0 {
                    let colors = theme::palette(mode);
                    egui::Area::new(Id::new("font-toast"))
                        .order(egui::Order::Foreground)
                        .anchor(Align2::LEFT_BOTTOM, Vec2::new(12.0, -32.0))
                        .interactable(false)
                        .show(ctx, |ui| {
                            Frame::none()
                                .fill(colors.bg_raised)
                                .inner_margin(Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.label(text);
                                });
                        });
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                        (started + 4.0 - now).max(0.0),
                    ));
                }
            }
        }
        self.close_guard(ctx, now);
        if arrow_pressed {
            self.refocus = ctx.memory(|memory| memory.focused());
        }
        // 次の再描画（保存のタイマ・外部変更の監視 1s・OS の配色の追従 1s、§11.4）
        ctx.request_repaint_after(std::time::Duration::from_millis(1000));
    }

    fn apply_style(&mut self, ctx: &Context, mode: Mode) {
        let scale_key = (self.settings.font_scale * 100.0).round() as u32;
        if self.applied == Some((mode, scale_key)) {
            return;
        }
        ctx.set_visuals(theme::visuals(mode));
        let scale = self.settings.font_scale;
        ctx.style_mut(|style| {
            use egui::TextStyle;
            let base = |text: &TextStyle| match text {
                TextStyle::Small => 10.0,
                TextStyle::Body | TextStyle::Button | TextStyle::Monospace => 13.0,
                TextStyle::Heading => 18.0,
                TextStyle::Name(_) => 13.0,
            };
            for (text, font) in style.text_styles.iter_mut() {
                font.size = base(text) * scale;
            }
            // 切替アニメーションはしない（§11.4 MUST）
            style.animation_time = 0.0;
            style.interaction.selectable_labels = false;
            style.spacing.interact_size.y = (24.0 * scale).max(24.0);
        });
        self.applied = Some((mode, scale_key));
    }

    fn run(&mut self, ctx: &Context, command: Command, now: f64) {
        let locale = self.locale();
        // `Ctrl+M` の中の `Ctrl+Enter` は「新しい資料を作って挿入」、もう一度 `Ctrl+M` で閉じる
        if let Overlay::Materials { create, .. } = &mut self.overlay {
            match command {
                Command::ConfirmNext => {
                    *create = true;
                    return;
                }
                Command::MaterialSearch => {
                    self.overlay = Overlay::None;
                    return;
                }
                _ => {}
            }
        }
        // 出典の無い資料の採用の確認は `Ctrl+Enter` でも確定する
        if let Overlay::ConfirmApprove { .. } = &self.overlay {
            if command == Command::ConfirmNext {
                self.overlay = Overlay::None;
                self.approve_material(now, true);
                return;
            }
        }
        match command {
            Command::ToggleLanguage => {
                self.settings.language = match locale {
                    Locale::Ja => LanguageChoice::En,
                    Locale::En => LanguageChoice::Ja,
                };
                self.settings_changed();
            }
            Command::CycleTheme => {
                self.settings.theme = self.settings.theme.next();
                self.settings_changed();
            }
            Command::InlayHints => {
                self.settings.inlay_hints = !self.settings.inlay_hints;
                self.settings_changed();
            }
            Command::FontUp | Command::FontDown => {
                self.settings.step_font(command == Command::FontUp);
                self.settings_changed();
            }
            Command::FontReset => {
                self.settings.font_scale = 1.0;
                self.settings_changed();
            }
            Command::Help => {
                self.overlay = if self.overlay == Overlay::Help {
                    Overlay::None
                } else {
                    Overlay::Help
                };
            }
            Command::Escape => {
                if matches!(self.stage, Stage::Picker) {
                    self.picker.escape();
                }
                if self.overlay != Overlay::None {
                    self.overlay = Overlay::None;
                }
                self.focus = Pane::Editor;
                if let Stage::Open(session) = &self.stage {
                    focus_workspace(ctx, session);
                }
            }
            Command::Open => {
                match &mut self.stage {
                    Stage::Picker => self.picker.open_folder(),
                    Stage::Open(session) => {
                        // 今のプロジェクトを書き終えてから切り替える（書けなければ切り替えない）
                        session.bench.commit();
                        session.bench.edited(now);
                        session.bench.save.request_now();
                        save_if_due(session, &mut self.settings, now);
                        // 両プラットフォームとも、保存完了後に履歴一覧へ戻る。
                        // フォルダダイアログは一覧の「開く」からのみ起動する。
                        self.leaving = true;
                        self.overlay = Overlay::None;
                        return;
                    }
                }
            }
            _ => {}
        }
        if self.run_workspace(ctx, command, now) {
            return;
        }
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        let bench = &mut session.bench;
        match command {
            Command::Palette => {
                self.overlay = Overlay::Palette {
                    input: String::new(),
                    selected: 0,
                };
            }
            Command::Jump => {
                self.overlay = Overlay::Jump {
                    input: String::new(),
                    missed: false,
                };
            }
            Command::SaveNow => {
                bench.commit();
                bench.edited(now);
                bench.save.request_now();
            }
            Command::Reload => {
                // F5: 監視を今すぐ 1 回回す（外部変更の読み直し、§8.5）
                session.last_poll = f64::NEG_INFINITY;
            }
            Command::FocusContext => self.focus = Pane::Context,
            Command::FocusEditor => {
                self.focus = Pane::Editor;
                ctx.memory_mut(|memory| memory.request_focus(section_editor::editor_id(bench)));
            }
            Command::FocusReference => {
                self.focus = Pane::Reference;
                self.settings.reference_visible = true;
                // 資料の候補があれば最初の候補へ（`Enter` でプロットの `material:` に足す、B-136）
                let first = session
                    .reference_cache
                    .content()
                    .materials
                    .iter()
                    .find(|found| found.candidate)
                    .map(|found| reference_pane::candidate_id(&found.id));
                if let Some(id) = first {
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
            }
            Command::ToggleReference => {
                self.settings.reference_visible = !self.settings.reference_visible;
            }
            Command::SwapReference => {
                self.settings.reference_pane_side = match self.settings.reference_pane_side {
                    PaneSide::Right => PaneSide::Left,
                    PaneSide::Left => PaneSide::Right,
                };
                session.settings_dirty = true;
            }
            Command::Nav(nav) => {
                if bench.go(nav) {
                    session.settings_dirty = true;
                    ctx.memory_mut(|memory| memory.request_focus(section_editor::editor_id(bench)));
                }
            }
            Command::ConfirmNext => {
                if bench.confirm_next(now) {
                    session.settings_dirty = true;
                    ctx.memory_mut(|memory| memory.request_focus(section_editor::editor_id(bench)));
                }
            }
            Command::NewSection => {
                if bench.new_section(now) {
                    session.settings_dirty = true;
                    ctx.memory_mut(|memory| memory.request_focus(section_editor::editor_id(bench)));
                }
            }
            Command::MainRatio(grow) => {
                self.settings.step_ratio(true, grow);
                session.settings_dirty = true;
            }
            Command::ContextRatio(grow) => {
                self.settings.step_ratio(false, grow);
                session.settings_dirty = true;
            }
            Command::Find | Command::Replace => {
                let query = session
                    .selection
                    .clone()
                    .filter(|range| !range.is_empty())
                    .and_then(|range| bench.buffer().get(range).map(str::to_owned))
                    .unwrap_or_else(|| session.last_find.clone());
                self.overlay = Overlay::Find {
                    query,
                    replace: (command == Command::Replace).then(String::new),
                    fresh: true,
                };
            }
            Command::FindNext(forward) => {
                let query = match &self.overlay {
                    Overlay::Find { query, .. } => query.clone(),
                    _ => session.last_find.clone(),
                };
                if !query.is_empty() {
                    session.find_step(&query, forward);
                }
            }
            Command::Search => {
                self.overlay = Overlay::Search {
                    query: session.last_find.clone(),
                    selected: 0,
                };
            }
            Command::Complete => {
                self.overlay = Overlay::Completion { selected: 0 };
            }
            Command::Modes | Command::Term => {
                let lines = session
                    .bench
                    .spliced_document()
                    .and_then(|(document, start)| {
                        let caret =
                            start + session.selection.clone().map_or(0, |range| range.start);
                        let hover = crate::core::features::hover::at(
                            &document,
                            caret,
                            session.bench.project(),
                        )?;
                        crate::ui::assist::info_lines(&hover, command == Command::Modes)
                    });
                if let Some(lines) = lines {
                    self.overlay = Overlay::Info { lines };
                }
            }
            Command::DeleteSection => {
                if session.bench.current().is_some() {
                    self.overlay = Overlay::ConfirmDelete;
                }
            }
            Command::Export => {
                // 節エディタの中身も含めて書き出す
                session.bench.commit();
                self.overlay = Overlay::Export {
                    range: String::new(),
                    state: None,
                    fresh: true,
                };
            }
            Command::QuickFix => {
                let actions = session
                    .bench
                    .spliced_document()
                    .map(|(document, start)| {
                        let selection = session.selection.clone().unwrap_or(0..0);
                        quick_fixes(
                            &document,
                            start + selection.start..start + selection.end,
                            session.bench.project(),
                        )
                    })
                    .unwrap_or_default();
                if actions.is_empty() {
                    session.notice = Some((i18n::text(locale, "quickfix.none").to_owned(), now));
                } else {
                    self.overlay = Overlay::QuickFix {
                        actions,
                        selected: 0,
                    };
                }
            }
            Command::Register { character } => {
                let name = session
                    .selection
                    .clone()
                    .filter(|range| !range.is_empty())
                    .and_then(|range| session.bench.buffer().get(range).map(str::to_owned))
                    .map(|text| text.trim().to_owned())
                    .filter(|text| !text.is_empty() && !text.contains('\n'));
                match name {
                    Some(name) => {
                        let kind = if character {
                            crate::core::resolve::dictionary::EntryKind::Character
                        } else {
                            crate::core::resolve::dictionary::EntryKind::Word
                        };
                        register_entry(session, kind, &name, locale, now);
                    }
                    None => {
                        session.notice =
                            Some((i18n::text(locale, "register.no_selection").to_owned(), now));
                    }
                }
            }
            Command::Diff => {
                if session.bench.conflicts().next().is_some() {
                    self.overlay = Overlay::Diff;
                }
            }
            Command::RecentFile(forward) => {
                let target = if forward {
                    session.recent.get(1).cloned()
                } else if session.recent.len() > 1 {
                    session.recent.last().cloned()
                } else {
                    None
                };
                if let Some((path, id)) = target {
                    if let Some(index) = crate::core::features::sections::position_of(
                        session.bench.sections(),
                        &path,
                        id,
                    ) {
                        if session.bench.go_to(index) {
                            session.settings_dirty = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn settings_changed(&mut self) {
        if let Stage::Open(session) = &mut self.stage {
            session.settings_dirty = true;
        }
    }

    // ============================================================ ワークスペース（§10）

    /// ワークスペースの切替と、ワークスペースごとに意味が変わるキー。受け持ったら `true`
    /// （`run` の残り = write の処理には進まない）。
    fn run_workspace(&mut self, ctx: &Context, command: Command, now: f64) -> bool {
        let locale = self.locale();
        let Stage::Open(session) = &mut self.stage else {
            return false;
        };
        let current = session.spaces.switcher.current();
        match command {
            Command::Workspace(number) => {
                if let Some(target) = Workspace::of_number(number) {
                    self.switch_to(ctx, target, now);
                }
                return true;
            }
            Command::PreviousWorkspace => {
                if let Some(previous) = session.spaces.switcher.previous() {
                    self.switch_to(ctx, previous, now);
                }
                return true;
            }
            Command::Capture => {
                // §10.4 MUST: どこからでも開き、画面は変えない
                self.overlay = Overlay::Capture {
                    input: String::new(),
                };
                return true;
            }
            Command::MaterialSearch => {
                // §16.3: どのワークスペースからでも開く。挿入先は今のワークスペースのエディタ
                session.bench.commit();
                self.overlay = Overlay::Materials {
                    query: String::new(),
                    selected: 0,
                    create: false,
                };
                return true;
            }
            Command::Review => {
                // §10.9.2: 整合チェックの一括実行。点検の診断タブで数え直す
                session.spaces.review.rerun();
                if current != Workspace::Review {
                    self.switch_to(ctx, Workspace::Review, now);
                }
                return true;
            }
            Command::Up => {
                match current {
                    Workspace::Write => {
                        // §10.1: 同じ節の話プロットへ（初めてでなくても今の節に着地する）
                        if let Some(id) = session.bench.current_id() {
                            session.bench.commit();
                            session.spaces.episode.land(&session.bench, id);
                            session.spaces.switcher.mark_visited(Workspace::PlotEpisode);
                        }
                        self.switch_to(ctx, Workspace::PlotEpisode, now);
                    }
                    Workspace::PlotEpisode => {
                        // 話 → 章プロットのその話のブロック
                        if let Some((chapter, episode)) = session.spaces.episode.episode() {
                            session.bench.commit();
                            session
                                .spaces
                                .outline
                                .land_chapter(&session.bench, chapter, episode);
                            session.spaces.switcher.mark_visited(Workspace::PlotOutline);
                        }
                        self.switch_to(ctx, Workspace::PlotOutline, now);
                    }
                    Workspace::PlotOutline => {
                        session.spaces.outline.up(&mut session.bench);
                    }
                    _ => {}
                }
                return true;
            }
            Command::Format => {
                session.bench.commit();
                let document = if current == Workspace::Write {
                    session.bench.spliced_document().map(|(doc, _)| doc)
                } else {
                    session.bench.slice_document().map(|(doc, _)| doc)
                };
                if let Some(document) = document {
                    let edits = crate::core::features::formatting::document(
                        &document,
                        session.bench.project(),
                    );
                    if edits.is_empty() {
                        session.notice = Some((
                            i18n::format(locale, "format.unchanged", &[&document.path]),
                            now,
                        ));
                    } else {
                        let mut edit = crate::core::features::WorkspaceEdit::default();
                        for item in edits {
                            edit.push(&document.path, item);
                        }
                        session.bench.apply_edit(&edit, now);
                        session.notice =
                            Some((i18n::format(locale, "format.done", &[&document.path]), now));
                    }
                }
                return true;
            }
            Command::Rename => {
                session.bench.commit();
                let target = active_document_and_caret(session, current)
                    .and_then(|(document, caret)| {
                        let prepared = crate::core::features::rename::prepare(
                            &document,
                            caret,
                            session.bench.project(),
                        )?;
                        Some((document, caret, prepared))
                    })
                    .or_else(|| {
                        if current != Workspace::Cast {
                            return None;
                        }
                        let index = session.spaces.cast.selected()?;
                        let pos = character_definition_pos(session.bench.project(), index)?;
                        let document = session.bench.project().document("characters.md")?.clone();
                        let prepared = crate::core::features::rename::prepare(
                            &document,
                            pos,
                            session.bench.project(),
                        )?;
                        Some((document, pos, prepared))
                    });
                match target {
                    Some((document, pos, prepared)) => {
                        self.overlay = Overlay::Rename {
                            document,
                            pos,
                            input: prepared.placeholder,
                            error: None,
                        };
                    }
                    None => {
                        session.notice =
                            Some((i18n::text(locale, "rename.not_renamable").to_owned(), now));
                    }
                }
                return true;
            }
            Command::Definition => {
                session.bench.commit();
                let location =
                    active_document_and_caret(session, current).and_then(|(document, caret)| {
                        crate::core::features::definition::at(
                            &document,
                            caret,
                            session.bench.project(),
                        )
                    });
                match location {
                    Some(loc) if loc.path == "words.md" => {
                        let line = session
                            .bench
                            .project()
                            .document("words.md")
                            .map_or(0, |doc| {
                                crate::core::features::project::line_of(&doc.text, loc.range.start)
                            });
                        let index = session
                            .bench
                            .project()
                            .words()
                            .words
                            .iter()
                            .position(|word| word.heading == line)
                            .unwrap_or(0);
                        let lines = term_lines(session.bench.project(), index);
                        self.overlay = Overlay::Info { lines };
                    }
                    Some(loc) => {
                        self.review_jump(
                            ctx,
                            workspaces::review::Target::Document(loc.path, Some(loc.range.start)),
                            now,
                        );
                    }
                    None => {
                        session.notice =
                            Some((i18n::text(locale, "definition.none").to_owned(), now));
                    }
                }
                return true;
            }
            Command::References => {
                session.bench.commit();
                let found =
                    active_document_and_caret(session, current).and_then(|(document, caret)| {
                        let def = crate::core::features::definition::at(
                            &document,
                            caret,
                            session.bench.project(),
                        )?;
                        let name = session
                            .bench
                            .project()
                            .document(&def.path)
                            .and_then(|doc| doc.text.get(def.range.clone()))
                            .unwrap_or("")
                            .trim()
                            .to_owned();
                        let locations = crate::core::features::references::at(
                            &document,
                            caret,
                            session.bench.project(),
                        );
                        Some((name, locations))
                    });
                match found {
                    Some((name, locations)) => {
                        self.overlay = Overlay::References {
                            name,
                            locations,
                            selected: 0,
                        };
                    }
                    None => {
                        session.notice =
                            Some((i18n::text(locale, "definition.none").to_owned(), now));
                    }
                }
                return true;
            }
            _ => {}
        }
        if current == Workspace::Write {
            return false;
        }
        let bench = &mut session.bench;
        let spaces = &mut session.spaces;
        match (current, command) {
            (Workspace::PlotEpisode, Command::Nav(nav)) => {
                spaces.episode.nav(bench, nav);
            }
            (Workspace::PlotEpisode, Command::ConfirmNext) => {
                if let Some(row) = spaces.episode.current_row(bench) {
                    self.go_write(ctx, row.id, now);
                }
            }
            (Workspace::PlotEpisode, Command::InsertSection) => {
                let scene = bench.current_id().unwrap_or_default();
                spaces.episode.insert(bench, scene, now);
            }
            (Workspace::PlotOutline, Command::Nav(nav)) => {
                spaces.outline.nav(bench, nav);
            }
            (Workspace::PlotOutline, Command::InsertSection) => {
                spaces.outline.insert(bench, now);
            }
            (Workspace::PlotOutline, Command::Fold(fold)) => {
                spaces.outline.set_folded(bench, fold);
            }
            (Workspace::Review, Command::Nav(nav)) => {
                spaces.review.nav(bench, locale, nav);
            }
            (Workspace::Review, Command::Tab(_)) => {
                spaces.review.next_tab();
            }
            (Workspace::Review, Command::ConfirmNext) => {
                if let Some(target) = spaces.review.target(bench, locale) {
                    self.review_jump(ctx, target, now);
                }
            }
            (Workspace::Cast, Command::Nav(nav)) => {
                spaces.cast.nav(bench, nav);
            }
            (Workspace::Cast, Command::Tab(forward)) => {
                spaces.cast.step_tab(forward);
            }
            (Workspace::Cast, Command::Scan) => {
                spaces.cast.scan();
                // 一覧は右のペインに出すので、畳んでいれば出す
                self.settings.reference_visible = true;
                session.settings_dirty = true;
            }
            (Workspace::Material, Command::Nav(nav)) => {
                spaces.material.nav(bench, nav);
            }
            (Workspace::Material, Command::ConfirmNext) => {
                self.approve_material(now, false);
            }
            (Workspace::Material, Command::Drop) => {
                let stamp = session.host.now();
                let outcome = spaces.material.reject(bench, now, &stamp);
                self.material_outcome(outcome, now);
            }
            (Workspace::Material, Command::InsertSection) => {
                self.overlay = Overlay::NewMaterial {
                    input: String::new(),
                };
            }
            (Workspace::Material, Command::FocusEditor) => {
                spaces.material.focus_editor();
            }
            (Workspace::Idea, Command::Nav(nav)) => {
                spaces.idea.nav(bench, nav);
            }
            (Workspace::Idea, Command::Drop) => {
                if let Some(id) = spaces.idea.drop_selected(bench, now) {
                    session.notice = Some((i18n::format(locale, "idea.dropped", &[&id]), now));
                }
            }
            (Workspace::Idea, Command::ConfirmNext) => {
                if spaces.idea.current(bench).is_some() {
                    self.overlay = Overlay::Promote {
                        selected: 0,
                        characters: None,
                    };
                }
            }
            (Workspace::PlotOutline, Command::ConfirmNext) => {
                if let Some(workspaces::plot_outline::Down::Episode(chapter, episode)) =
                    spaces.outline.down(bench)
                {
                    spaces.episode.land_episode(bench, chapter, episode);
                    spaces.switcher.mark_visited(Workspace::PlotEpisode);
                    self.switch_to(ctx, Workspace::PlotEpisode, now);
                }
            }
            (Workspace::Agent, Command::StopAll) => {
                let message = crate::ui::agents::stop_all(
                    bench,
                    session.host.as_mut(),
                    &mut session.agents,
                    now,
                    locale,
                );
                session.notice = Some((message, now));
            }
            (Workspace::Agent, Command::Escape) => {
                spaces.agent.escape();
            }
            (_, Command::QuickFix) => {
                // M8 のエディタ（Slice）でも `Ctrl+.`。資料 ID はプロットに書く（§6.6.4）ので、E006 の
                // クイックフィックス（§6.6.3）は話プロットで要る（`PLANS.md` §9.2 B-137）
                let actions = bench
                    .slice_document()
                    .map(|(document, start)| {
                        let selection = bench.slice_caret().unwrap_or(0..0);
                        quick_fixes(
                            &document,
                            start + selection.start..start + selection.end,
                            bench.project(),
                        )
                    })
                    .unwrap_or_default();
                if actions.is_empty() {
                    session.notice = Some((i18n::text(locale, "quickfix.none").to_owned(), now));
                } else {
                    self.overlay = Overlay::QuickFix {
                        actions,
                        selected: 0,
                    };
                }
            }
            (_, Command::Jump | Command::Palette | Command::SaveNow | Command::Reload) => {
                return false
            }
            (_, Command::Export | Command::Search | Command::Help | Command::Escape) => {
                return false
            }
            (_, Command::ToggleReference | Command::SwapReference | Command::MainRatio(_)) => {
                return false
            }
            (_, Command::FocusReference | Command::Diff | Command::Open) => return false,
            _ => {
                // write の節エディタに向けたキー（検索・補完・節の削除など）はここでは何もしない
                let _ = locale;
            }
        }
        true
    }

    // ============================================================ エージェント（§15）

    /// agent ワークスペースからの頼みを実行する（§15.7）。
    fn agent_actions(&mut self, actions: Vec<workspaces::agent::Action>, now: f64) {
        use crate::core::agents::files::{self, Board};
        use workspaces::agent::Action;
        let locale = self.locale();
        for action in actions {
            let Stage::Open(session) = &mut self.stage else {
                return;
            };
            let mut notices: Vec<String> = Vec::new();
            match action {
                Action::StopAll => {
                    notices.push(crate::ui::agents::stop_all(
                        &mut session.bench,
                        session.host.as_mut(),
                        &mut session.agents,
                        now,
                        locale,
                    ));
                }
                Action::EditMcp => {
                    self.overlay = Overlay::McpConfig {
                        input: session
                            .bench
                            .project()
                            .text(crate::core::agents::mcp_config::PATH)
                            .to_owned(),
                        error: None,
                    };
                }
                Action::New => {
                    let digits = session.bench.project().digits();
                    let scope = session
                        .bench
                        .current_id()
                        .map(|id| format!("@{}", id.display(digits)))
                        .unwrap_or_default();
                    self.overlay = Overlay::AgentNew {
                        role: 0,
                        scope,
                        intent: String::new(),
                        comment: String::new(),
                        examples: String::new(),
                        fresh: true,
                    };
                }
                Action::Run(ids) => {
                    notices = crate::ui::agents::enqueue(
                        &ids,
                        &mut session.bench,
                        session.host.as_mut(),
                        &mut session.agents,
                        now,
                        locale,
                    );
                }
                Action::Confirm(ids) => {
                    let (chars, servers, blocked) = confirm_summary(session, &ids, locale);
                    self.overlay = Overlay::AgentConfirm {
                        ids,
                        chars,
                        servers,
                        blocked,
                    };
                }
                Action::Cancel(id) => {
                    crate::ui::agents::cancel(
                        &id,
                        &mut session.bench,
                        session.host.as_mut(),
                        &mut session.agents,
                        now,
                    );
                    session.spaces.agent.marked.retain(|found| found != &id);
                }
                Action::DryRun(ids) => {
                    let stamp = session.host.now();
                    let title = session.host.name();
                    session.bench.commit();
                    let mut saved = Vec::new();
                    for id in &ids {
                        if let Some((path, text)) = crate::core::agents::runner::dry_run(
                            session.bench.project(),
                            id,
                            &stamp,
                            &title,
                        ) {
                            session.bench.replace_document(&path, text, now);
                            saved.push(path);
                        }
                    }
                    if !saved.is_empty() {
                        notices.push(i18n::format(
                            locale,
                            "agent.dry_run_saved",
                            &[&saved.join(", ")],
                        ));
                    }
                }
                Action::Exclude { task, block } => {
                    session.bench.commit();
                    let board = Board::of(session.bench.project());
                    if block.is_mandatory() {
                        notices.push(i18n::text(locale, "agent.block.locked").to_owned());
                    } else if let Some(found) = board.task(&task) {
                        if matches!(found.status.as_str(), "draft" | "ready")
                            && !session.agents.holds(&task)
                        {
                            let mut blocks = files::excluded_blocks(found);
                            match blocks.iter().position(|seen| *seen == block) {
                                Some(at) => {
                                    blocks.remove(at);
                                }
                                None => blocks.push(block),
                            }
                            let mut updated = found.clone();
                            updated.exclude = files::exclude_value(&blocks);
                            session.bench.replace_document(
                                &files::task_path(&task),
                                files::write_task(&updated),
                                now,
                            );
                        } else {
                            notices.push(i18n::text(locale, "agent.exclude_locked").to_owned());
                        }
                    }
                }
                Action::Accept { proposal, text } => {
                    session.bench.commit();
                    let project = session.bench.project();
                    let fixed = Board::of(project)
                        .proposal(&proposal)
                        .is_some_and(|found| crate::core::agents::apply::is_fixed(project, found));
                    if fixed {
                        self.overlay = Overlay::AgentFixed { proposal, text };
                    } else {
                        notices.push(accept_proposal(
                            session,
                            &proposal,
                            text.as_deref(),
                            now,
                            locale,
                        ));
                    }
                }
                Action::Reject { proposal, revert } => {
                    self.overlay = Overlay::AgentComment {
                        proposal,
                        revert,
                        input: String::new(),
                        missing: false,
                    };
                }
                Action::Approve(task) => {
                    let count = crate::ui::agents::approve(&task, &mut session.bench, now);
                    notices.push(i18n::format(
                        locale,
                        "agent.approved",
                        &[&count.to_string()],
                    ));
                }
                Action::Level { role, level } => {
                    if let Some(message) = crate::ui::agents::set_level(
                        &role,
                        level,
                        &mut session.bench,
                        session.host.as_mut(),
                        now,
                        locale,
                    ) {
                        notices.push(message);
                    }
                }
            }
            if !notices.is_empty() {
                session.notice = Some((notices.join(" / "), now));
            }
        }
    }

    /// パレットのエージェントのコマンド（§15.7.6 `>agent delegation` / `>agent stop-all`、
    /// §15.5.4 `>prompt reset` / `>prompt diff`）。
    /// Zip でエクスポートする（§13.5）。編集中の中身を先に書く（Web の書き込みは列の順なので、Zip は
    /// その保存の後の中身になる）。デスクトップで書けなければ書き出さずに知らせる。
    fn export_zip(&mut self, now: f64) {
        let locale = self.locale();
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        session.bench.commit();
        session.bench.edited(now);
        session.bench.save.request_now();
        save_if_due(session, &mut self.settings, now);
        if session.saving.is_none() && session.bench.save.state() != State::Clean {
            self.overlay = Overlay::SaveError;
            return;
        }
        let name = zip_name(session);
        if let Some(result) = session.host.export_zip(&name) {
            session.notice = Some((zip_result_text(locale, result), now));
        }
    }

    fn agent_command(&mut self, command: AgentCommand, now: f64) {
        let locale = self.locale();
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        match command {
            AgentCommand::Level(role, level) => {
                if let Some(message) = crate::ui::agents::set_level(
                    &role,
                    level,
                    &mut session.bench,
                    session.host.as_mut(),
                    now,
                    locale,
                ) {
                    session.notice = Some((message, now));
                }
            }
            AgentCommand::StopAll => {
                let message = crate::ui::agents::stop_all(
                    &mut session.bench,
                    session.host.as_mut(),
                    &mut session.agents,
                    now,
                    locale,
                );
                session.notice = Some((message, now));
            }
            AgentCommand::PromptReset(name) => {
                session.bench.commit();
                self.overlay = Overlay::PromptReset { name };
            }
            AgentCommand::PromptDiff(name) => {
                session.bench.commit();
                self.overlay = Overlay::PromptDiff { name };
            }
        }
    }

    /// ワークスペースを切り替える（§10.0）。切替は保存のきっかけ（§8.2）。確認は出さない。
    fn switch_to(&mut self, ctx: &Context, target: Workspace, now: f64) -> bool {
        let locale = self.locale();
        let Stage::Open(session) = &mut self.stage else {
            return false;
        };
        if !target.available() {
            let name = i18n::text(locale, target.label_key()).to_owned();
            session.notice = Some((i18n::format(locale, "workspace.unavailable", &[&name]), now));
            return false;
        }
        // 開いている文書の一部を書き戻して閉じ、すぐに保存する
        session.bench.close_slice();
        session.bench.commit();
        session.bench.save.request_now();
        if !session.spaces.switcher.switch(target) {
            return false;
        }
        session.workspace_switched_at = Some(now);
        if session.spaces.switcher.first_visit(target) {
            // 初めて開くワークスペースは今の節の対応物に着地する（SceneId の維持、§10 MUST）
            let id = session.bench.current_id();
            match (target, id) {
                (Workspace::PlotEpisode, Some(id)) => {
                    session.spaces.episode.land(&session.bench, id);
                }
                (Workspace::PlotOutline, Some(id)) => {
                    session
                        .spaces
                        .outline
                        .land_chapter(&session.bench, id.chapter, id.episode);
                }
                (Workspace::PlotOutline, None) => {
                    session.spaces.outline.land_all(&session.bench, 0);
                }
                (Workspace::Material, _) => {
                    // 今の節の明示参照 → 条件一致の候補 → 一覧の最初（B-134）
                    let materials = &session.reference_cache.content().materials;
                    let explicit: Vec<String> = materials
                        .iter()
                        .filter(|found| !found.candidate)
                        .map(|found| found.id.clone())
                        .collect();
                    let candidates: Vec<String> = materials
                        .iter()
                        .filter(|found| found.candidate)
                        .map(|found| found.id.clone())
                        .collect();
                    session
                        .spaces
                        .material
                        .land(&session.bench, &explicit, &candidates);
                }
                _ => {}
            }
            session.spaces.switcher.mark_visited(target);
        }
        // キャレットの下に出すもの（補完・クイックフィックス・情報）は閉じる
        if matches!(
            self.overlay,
            Overlay::Completion { .. }
                | Overlay::QuickFix { .. }
                | Overlay::Info { .. }
                | Overlay::Find { .. }
        ) {
            self.overlay = Overlay::None;
        }
        self.focus = Pane::Editor;
        let _ = ctx;
        true
    }

    /// 点検の行から該当箇所へ（§10.9.2「`Enter` で該当節へジャンプ」、B-132）。文書の種類で
    /// 開くワークスペースが決まる（本文 → write、話プロット → plot-episode、全体 / 章プロット →
    /// plot-outline、`ideas.md` → idea、`characters.md` → cast、資料 MD と `materials/index.json` →
    /// material）。それ以外は開く画面が無いので知らせるだけ。
    fn review_jump(&mut self, ctx: &Context, target: workspaces::review::Target, now: f64) {
        use crate::core::model::document::DocumentFormat;
        use workspaces::review::Target;
        let locale = self.locale();
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        match target {
            Target::Scene(id) => {
                if id.scene > 0 {
                    self.go_write(ctx, id, now);
                } else {
                    let episode = id.episode;
                    session.bench.commit();
                    if episode > 0 {
                        session
                            .spaces
                            .outline
                            .land_chapter(&session.bench, id.chapter, episode);
                    } else {
                        session.spaces.outline.land_all(&session.bench, id.chapter);
                    }
                    session.spaces.switcher.mark_visited(Workspace::PlotOutline);
                    self.switch_to(ctx, Workspace::PlotOutline, now);
                }
            }
            Target::Document(path, byte) => {
                let Some(document) = session.bench.project().document(&path).cloned() else {
                    return;
                };
                let line = byte.map_or(0, |at| {
                    crate::core::features::project::line_of(&document.text, at)
                });
                match document.format {
                    DocumentFormat::Contents => {
                        session.bench.reveal(&path, byte.unwrap_or(0));
                        session.settings_dirty = true;
                        self.switch_to(ctx, Workspace::Write, now);
                        if let Stage::Open(session) = &self.stage {
                            let id = section_editor::editor_id(&session.bench);
                            ctx.memory_mut(|memory| memory.request_focus(id));
                        }
                    }
                    DocumentFormat::PlotEpisode => {
                        let found =
                            session
                                .bench
                                .project()
                                .scene_at(&document, line)
                                .or_else(|| {
                                    crate::core::features::project::episode_of_path(&path).map(
                                        |(chapter, episode)| crate::core::id::SceneId {
                                            chapter,
                                            episode,
                                            scene: 0,
                                            section: 0,
                                        },
                                    )
                                });
                        if let Some(id) = found {
                            session.bench.commit();
                            if id.scene > 0 {
                                session.spaces.episode.land(&session.bench, id);
                            } else {
                                session.spaces.episode.land_episode(
                                    &session.bench,
                                    id.chapter,
                                    id.episode,
                                );
                            }
                            session.spaces.switcher.mark_visited(Workspace::PlotEpisode);
                            self.switch_to(ctx, Workspace::PlotEpisode, now);
                        }
                    }
                    DocumentFormat::PlotOutline => {
                        let chapter = crate::core::features::project::chapter_of_path(&path);
                        let blocks = crate::core::features::plots::outline_blocks(
                            session.bench.project(),
                            chapter,
                        );
                        let block = blocks
                            .iter()
                            .rev()
                            .find(|block| block.lines.start <= line)
                            .map(|block| block.id);
                        session.bench.commit();
                        match (chapter, block) {
                            (Some(number), found) => session.spaces.outline.land_chapter(
                                &session.bench,
                                number,
                                found.map_or(0, |id| id.episode),
                            ),
                            (None, found) => session
                                .spaces
                                .outline
                                .land_all(&session.bench, found.map_or(0, |id| id.chapter)),
                        }
                        session.spaces.switcher.mark_visited(Workspace::PlotOutline);
                        self.switch_to(ctx, Workspace::PlotOutline, now);
                    }
                    DocumentFormat::Ideas => {
                        session.spaces.idea.select_line(&session.bench, line);
                        self.switch_to(ctx, Workspace::Idea, now);
                    }
                    DocumentFormat::Characters => {
                        session.spaces.cast.select_line(&session.bench, line);
                        session.spaces.switcher.mark_visited(Workspace::Cast);
                        self.switch_to(ctx, Workspace::Cast, now);
                    }
                    DocumentFormat::MaterialDoc => {
                        session.spaces.material.select_path(&session.bench, &path);
                        session.spaces.switcher.mark_visited(Workspace::Material);
                        self.switch_to(ctx, Workspace::Material, now);
                    }
                    DocumentFormat::MaterialsIndex => {
                        // 位置の前にある最後の `"id"` の資料（無ければ一覧の最初）
                        let id = byte.and_then(|at| material_id_before(&document.text, at));
                        if let Some(id) = id {
                            session.spaces.material.select(&session.bench, &id);
                        }
                        session.spaces.switcher.mark_visited(Workspace::Material);
                        self.switch_to(ctx, Workspace::Material, now);
                    }
                    _ => {
                        session.notice =
                            Some((i18n::format(locale, "review.cannot_open", &[&path]), now));
                    }
                }
            }
        }
    }

    /// `material` の `Ctrl+Enter`: 選んでいる資料を採用する（索引に無い資料 MD なら登録する）。
    /// 出典が無ければ確認を挟む（§16.7。`confirmed` は確認を経た後）。
    fn approve_material(&mut self, now: f64, confirmed: bool) {
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        let stamp = session.host.now();
        let outcome = session
            .spaces
            .material
            .approve(&mut session.bench, now, &stamp, confirmed);
        self.material_outcome(outcome, now);
    }

    /// 採用・却下・登録の結果を通知（と確認）にする。
    fn material_outcome(&mut self, outcome: Option<workspaces::material::Outcome>, now: f64) {
        use workspaces::material::Outcome;
        let locale = self.locale();
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        let message = match outcome {
            None => return,
            Some(Outcome::NeedsConfirm(id)) => {
                self.overlay = Overlay::ConfirmApprove { id };
                return;
            }
            Some(Outcome::Approved(id)) => i18n::format(locale, "material.approved", &[&id]),
            Some(Outcome::Rejected(id)) => i18n::format(locale, "material.rejected", &[&id]),
            Some(Outcome::Registered(id, path)) => {
                i18n::format(locale, "material.registered", &[&path, &id])
            }
            Some(Outcome::Already(id)) => i18n::format(locale, "material.already", &[&id]),
            Some(Outcome::Occupied(path)) => i18n::format(locale, "material.occupied", &[&path]),
        };
        session.notice = Some((message, now));
    }

    /// 資料 `id` を material ワークスペースで開く。
    fn open_material(&mut self, ctx: &Context, id: &str, now: f64) {
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        session.bench.commit();
        session.spaces.material.select(&session.bench, id);
        session.spaces.switcher.mark_visited(Workspace::Material);
        self.switch_to(ctx, Workspace::Material, now);
    }

    /// `Ctrl+M` で選んだ資料を挿入する・開く・作る（§16.3、B-134）。
    fn material_action(&mut self, ctx: &Context, action: MaterialAction, now: f64) {
        use crate::core::features::materials_ops;
        let locale = self.locale();
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        let workspace = session.spaces.switcher.current();
        let id = match action {
            MaterialAction::Open(id) => {
                self.open_material(ctx, &id, now);
                return;
            }
            MaterialAction::Insert(id) => id,
            MaterialAction::Create(query) => {
                let (title, category) = materials_ops::draft_of(&query);
                let title = if title.trim().is_empty() {
                    i18n::text(locale, "material.untitled").to_owned()
                } else {
                    title
                };
                let stamp = session.host.now();
                session.bench.commit();
                let index_text = session
                    .bench
                    .project()
                    .text(materials_ops::INDEX_PATH)
                    .to_owned();
                let Some(created) =
                    materials_ops::propose_with(&index_text, &title, &category, "", "", &stamp)
                else {
                    session.notice =
                        Some((i18n::text(locale, "material.index_broken").to_owned(), now));
                    return;
                };
                session
                    .bench
                    .replace_document(&created.path, created.body, now);
                session
                    .bench
                    .replace_document(materials_ops::INDEX_PATH, created.index, now);
                session.notice = Some((
                    i18n::format(locale, "material.created", &[&created.id]),
                    now,
                ));
                created.id
            }
        };
        let reference = format!("'{id}");
        match workspace {
            Workspace::Write => {
                // 節エディタのキャレット（選択していれば選択を置き換える）
                if session.bench.current().is_none() {
                    return;
                }
                let length = session.bench.buffer().len();
                let range = session.selection.clone().unwrap_or(length..length);
                let from = range.start.min(range.end).min(length);
                let to = range.start.max(range.end).min(length);
                let buffer = session.bench.buffer_mut();
                if buffer.is_char_boundary(from) && buffer.is_char_boundary(to) {
                    buffer.replace_range(from..to, &reference);
                    let caret = from + reference.len();
                    session.bench.edited(now);
                    session.pending_selection = Some(caret..caret);
                    session.notice = Some((i18n::format(locale, "material.inserted", &[&id]), now));
                }
                let editor = section_editor::editor_id(&session.bench);
                ctx.memory_mut(|memory| memory.request_focus(editor));
            }
            Workspace::PlotEpisode => {
                self.append_material(ctx, &id, now);
            }
            Workspace::Material | Workspace::Review => {
                self.open_material(ctx, &id, now);
            }
            _ => {
                if let Some(caret) = session.bench.insert_in_slice(&reference, now) {
                    slice_editor::select(ctx, &session.bench, caret..caret);
                    session.notice = Some((i18n::format(locale, "material.inserted", &[&id]), now));
                }
            }
        }
    }

    /// 資料 `id` を今の節のプロットに `material: 'M0137` の行で足す（参照ペインの候補・話プロットの
    /// `Ctrl+M`。§10.2・§16.4、B-136）。話プロットではエディタの中身の最後に、write では話プロットの
    /// その節の最後に足す。
    fn append_material(&mut self, ctx: &Context, id: &str, now: f64) {
        use crate::core::features::plots;
        let locale = self.locale();
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        let line = format!("material: '{id}");
        let digits = session.bench.project().digits();
        match session.spaces.switcher.current() {
            Workspace::PlotEpisode => {
                let scene = session
                    .spaces
                    .episode
                    .current_row(&session.bench)
                    .map(|row| row.id);
                if session.bench.append_to_slice(&line, now) {
                    // 参照ペインが明示参照として読み直すように、すぐに書き戻す
                    session.bench.commit();
                    session.bench.save.request_now();
                    let place = scene.map_or_else(String::new, |found| {
                        format!("@{}", plots::short_id(found, digits))
                    });
                    session.notice = Some((
                        i18n::format(locale, "material.appended", &[id, &place]),
                        now,
                    ));
                    let editor = slice_editor::editor_id(&session.bench);
                    ctx.memory_mut(|memory| memory.request_focus(editor));
                }
            }
            _ => {
                let Some(scene) = session.bench.current_id() else {
                    return;
                };
                let project = session.bench.project();
                let path = project.episode_plot_path(scene.chapter, scene.episode);
                let rows = plots::episode_rows(project, scene.chapter, scene.episode);
                let text = project.text(&path).to_owned();
                match plots::append_line(&text, &rows, scene, &line) {
                    Some(updated) => {
                        session.bench.replace_document(&path, updated, now);
                        session.notice = Some((
                            i18n::format(
                                locale,
                                "material.appended",
                                &[id, &format!("@{}", plots::short_id(scene, digits))],
                            ),
                            now,
                        ));
                    }
                    None => {
                        session.notice =
                            Some((i18n::format(locale, "material.no_plot_row", &[&path]), now));
                    }
                }
                self.focus = Pane::Editor;
                if let Stage::Open(session) = &self.stage {
                    let editor = section_editor::editor_id(&session.bench);
                    ctx.memory_mut(|memory| memory.request_focus(editor));
                }
            }
        }
    }

    /// write の節 `id` へ降りる（`plot-episode` の `Ctrl+Enter`、§10.0 の着地点、B-128）。
    ///
    /// - 本文にその節があればそこへ。場面の行なら場面の最初の節へ。
    /// - 本文のファイルはあるが節が無ければ、近い節へ（空の節は作らない）。
    /// - 本文のファイルが無ければ、場面見出しだけの新しい節として開く（書き始めるまで作らない）。
    fn go_write(&mut self, ctx: &Context, id: crate::core::id::SceneId, now: f64) {
        let locale = self.locale();
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        session.bench.close_slice();
        let digits = session.bench.project().digits();
        let path = session
            .bench
            .project()
            .contents_path(id.chapter, id.episode);
        let query = if id.section == 0 {
            crate::core::id::SceneId { section: 0, ..id }
                .display(digits)
                .split('-')
                .take(3)
                .collect::<Vec<_>>()
                .join("-")
        } else {
            id.display(digits)
        };
        let exact = session.bench.sections().iter().position(|section| {
            section.path == path
                && section.id.scene == id.scene
                && (id.section == 0 || section.id.section == id.section)
        });
        if let Some(index) = exact {
            session.bench.go_to(index);
        } else if session.bench.project().document(&path).is_some() {
            if let Some(index) =
                crate::core::features::sections::position_of(session.bench.sections(), &path, id)
            {
                session.bench.go_to(index);
            }
            session.notice = Some((
                i18n::format(locale, "episode.prose_missing", &[&query]),
                now,
            ));
        } else {
            let scene = crate::core::id::SceneId { section: 0, ..id };
            let title = session
                .spaces
                .episode
                .rows(&session.bench)
                .iter()
                .find(|row| row.id == scene)
                .map(|row| row.summary.clone())
                .unwrap_or_default();
            let shown: Vec<String> = scene
                .display(digits)
                .split('-')
                .take(3)
                .map(str::to_owned)
                .collect();
            let header = if title.is_empty() {
                format!("@{}", shown.join("-"))
            } else {
                format!("@{} {title}", shown.join("-"))
            };
            session.bench.start_file(&path, id, &header);
        }
        session.settings_dirty = true;
        session.bench.save.request_now();
        self.switch_to(ctx, Workspace::Write, now);
        if let Stage::Open(session) = &self.stage {
            let editor = section_editor::editor_id(&session.bench);
            ctx.memory_mut(|memory| memory.request_focus(editor));
        }
    }

    // ============================================================ 執筆画面（§9.1）

    fn workbench_frame(
        &mut self,
        ctx: &Context,
        mode: Mode,
        locale: Locale,
        now: f64,
        focus_lost: bool,
    ) {
        let colors = theme::palette(mode);
        let scale = self.settings.font_scale;
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        // 画面離脱で即時保存（§8.2）
        if focus_lost {
            session.bench.commit();
            session.bench.save.request_now();
        }
        // ① ヘッダ（28px・ラベル無し）
        let mut switch_to = None;
        let mut open_palette = false;
        let mut open_help = false;
        let crumb = match session.spaces.switcher.current() {
            Workspace::Write => session.bench.breadcrumb(),
            Workspace::PlotEpisode => session.spaces.episode.breadcrumb(&session.bench),
            Workspace::PlotOutline => session
                .spaces
                .outline
                .breadcrumb(&session.bench, i18n::text(locale, "outline.all")),
            Workspace::Idea => session.spaces.idea.selected().map(str::to_owned),
            Workspace::Material => session.spaces.material.selected().map(str::to_owned),
            Workspace::Cast => {
                let selected = session.spaces.cast.selected();
                session
                    .spaces
                    .cast
                    .rows(&session.bench)
                    .iter()
                    .find(|row| Some(row.index) == selected)
                    .map(|row| row.name.clone())
            }
            _ => None,
        };
        egui::TopBottomPanel::top("header")
            .exact_height(28.0)
            .frame(
                Frame::none()
                    .fill(colors.bg_raised)
                    .inner_margin(Margin::symmetric(10.0, 4.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // ワークスペースインジケータ（§9.2.1。図形だけ、今のものを accent）
                    let current = session.spaces.switcher.current();
                    if let Some(target) = workspaces::indicator(
                        ui,
                        mode,
                        locale,
                        current,
                        project_has_problems(session, locale),
                    ) {
                        switch_to = Some(target);
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(session.host.name())
                            .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                            .color(colors.fg_faint),
                    );
                    ui.add_space(8.0);
                    if let Some(crumb) = crumb {
                        ui.label(
                            RichText::new(crumb)
                                .font(FontId::new(12.0 * scale, FontFamily::Monospace))
                                .color(colors.fg_muted),
                        );
                    }
                    if session
                        .workspace_switched_at
                        .is_some_and(|at| now - at < 1.0)
                    {
                        ui.label(
                            RichText::new(i18n::text(
                                locale,
                                session.spaces.switcher.current().label_key(),
                            ))
                            .color(colors.fg_faint),
                        );
                        ctx.request_repaint_after(std::time::Duration::from_secs(1));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("?").underline())
                                    .small()
                                    .min_size(Vec2::splat(24.0)),
                            )
                            .on_hover_text(i18n::text(locale, "command.help"))
                            .clicked()
                        {
                            open_help = true;
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Ctrl+P").underline())
                                    .small()
                                    .min_size(Vec2::splat(24.0)),
                            )
                            .on_hover_text(i18n::text(locale, "help.palette"))
                            .clicked()
                        {
                            open_palette = true;
                        }
                        let theme_hover = ui.painter().add(egui::Shape::Noop);
                        let theme_button = ui
                            .add(
                                egui::Button::new(" ")
                                    .frame(false)
                                    .min_size(Vec2::splat(24.0)),
                            )
                            .on_hover_text(i18n::text(locale, "header.theme"));
                        theme_button.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                i18n::text(locale, "header.theme"),
                            )
                        });
                        theme::paint_theme_icon(
                            ui.painter(),
                            theme_button.rect.center(),
                            self.settings.theme,
                            colors.fg_primary,
                        );
                        ui.painter().line_segment(
                            [
                                theme_button.rect.left_bottom(),
                                theme_button.rect.right_bottom(),
                            ],
                            Stroke::new(0.6_f32, colors.border),
                        );
                        if theme_button.hovered() {
                            ui.painter().set(
                                theme_hover,
                                egui::Shape::rect_filled(
                                    theme_button.rect,
                                    2.0,
                                    colors.bg_selection.gamma_multiply(0.6),
                                ),
                            );
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if theme_button.clicked() {
                            self.settings.theme = self.settings.theme.next();
                            session.settings_dirty = true;
                        }
                        // §12.5: 記号ボタン（ja は「あ」、en は「A」。カタログの header.language_symbol）。文字ラベルは置かない
                        let symbol = i18n::text(locale, "header.language_symbol");
                        let language_hover = ui.painter().add(egui::Shape::Noop);
                        let language = ui.add(
                            egui::Button::new(RichText::new(symbol).underline())
                                .frame(false)
                                .min_size(Vec2::splat(24.0)),
                        );
                        language.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                i18n::text(locale, "header.language"),
                            )
                        });
                        if language.hovered() {
                            ui.painter().set(
                                language_hover,
                                egui::Shape::rect_filled(
                                    language.rect,
                                    2.0,
                                    colors.bg_selection.gamma_multiply(0.6),
                                ),
                            );
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if language.clicked() {
                            self.settings.language = match locale {
                                Locale::Ja => LanguageChoice::En,
                                Locale::En => LanguageChoice::Ja,
                            };
                            session.settings_dirty = true;
                        }
                    });
                });
            });
        if let Some(target) = switch_to {
            self.switch_to(ctx, target, now);
        }
        if open_help {
            self.run(ctx, Command::Help, now);
        }
        if open_palette {
            self.run(ctx, Command::Palette, now);
        }
        if let Stage::Open(session) = &mut self.stage {
            if std::mem::take(&mut session.open_review) {
                self.run(ctx, Command::Review, now);
            }
        }
        // エージェント: 終わった呼び出しを 1 件ずつ処理し、待っているものを走らせる（§15.7.5）
        let mut open_agent = false;
        if let Stage::Open(session) = &mut self.stage {
            let notices = crate::ui::agents::tick(
                &mut session.bench,
                session.host.as_mut(),
                &mut session.agents,
                now,
                locale,
            );
            if !notices.is_empty() {
                session.notice = Some((notices.join(" / "), now));
            }
            if session.agents.busy() {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
            open_agent = std::mem::take(&mut session.open_agent);
            if open_agent {
                if let Some(task) = session
                    .agents
                    .running
                    .first()
                    .map(|found| found.task.clone())
                {
                    session.spaces.agent.select(&task);
                }
                session.spaces.agent.column = workspaces::agent::Column::Tasks;
            }
        }
        if open_agent {
            self.switch_to(ctx, Workspace::Agent, now);
        }
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        // ④ ステータスバー（22px）
        let indicator = session.bench.save.indicator(now);
        let mut open_error = false;
        egui::TopBottomPanel::bottom("status")
            .exact_height(22.0)
            .frame(
                Frame::none()
                    .fill(colors.bg_raised)
                    .inner_margin(Margin::symmetric(10.0, 2.0)),
            )
            .show(ctx, |ui| {
                open_error = status_bar(ui, session, locale, mode, scale, indicator, now);
            });
        if open_error {
            self.overlay = Overlay::SaveError;
        }
        // ③ 参照ペイン（既定 38%、最小 320px / 最大 560px）。幅 < 900px では畳む（§9.1）
        let width = ctx.screen_rect().width();
        let show_reference =
            self.settings.reference_visible && (width >= 900.0 || self.focus == Pane::Reference);
        let workspace = session.spaces.switcher.current();
        if workspace != Workspace::Write {
            let reference_width = (width * (1.0 - self.settings.main_ratio)).clamp(320.0, 560.0);
            let view = workspaces::View {
                mode,
                locale,
                scale,
                now,
                reference: show_reference
                    .then_some((self.settings.reference_pane_side, reference_width)),
                inlay_hints: self.settings.inlay_hints,
                overlay: self.overlay != Overlay::None,
            };
            // `Ctrl+.` のポップアップが出ている間は、↑/↓/Enter/Tab を Slice のエディタより先に取る
            let popup = if matches!(self.overlay, Overlay::QuickFix { .. }) {
                popup_keys(ctx)
            } else {
                (false, false, false)
            };
            let mut jump = None;
            let mut cast_jump = None;
            let mut agent_actions = Vec::new();
            let overlay_open = self.overlay != Overlay::None;
            let request = match workspace {
                Workspace::Cast => {
                    let (request, target) = workspaces::cast::show(
                        ctx,
                        &mut session.spaces.cast,
                        &mut session.bench,
                        &view,
                    );
                    cast_jump = target;
                    request
                }
                Workspace::Review => {
                    let (request, target) = workspaces::review::show(
                        ctx,
                        &mut session.spaces.review,
                        &mut session.bench,
                        &view,
                    );
                    jump = target;
                    request
                }
                Workspace::PlotEpisode => workspaces::plot_episode::show(
                    ctx,
                    &mut session.spaces.episode,
                    &mut session.bench,
                    &view,
                ),
                Workspace::PlotOutline => workspaces::plot_outline::show(
                    ctx,
                    &mut session.spaces.outline,
                    &mut session.bench,
                    &view,
                ),
                Workspace::Idea => {
                    workspaces::idea::show(ctx, &mut session.spaces.idea, &mut session.bench, &view)
                }
                Workspace::Material => {
                    let stamp = session.host.now();
                    workspaces::material::show(
                        ctx,
                        &mut session.spaces.material,
                        &mut session.bench,
                        &view,
                        &stamp,
                    )
                }
                Workspace::Agent => {
                    let servers = crate::ui::agents::servers(&session.bench, session.host.as_ref());
                    let stamp = session.host.now();
                    let title = session.host.name();
                    let available = session.host.mcp_available();
                    let info = workspaces::agent::Info {
                        runtime: &session.agents,
                        available,
                        servers: &servers,
                        stamp: &stamp,
                        title: &title,
                        overlay: overlay_open,
                    };
                    agent_actions = workspaces::agent::show(
                        ctx,
                        &mut session.spaces.agent,
                        &mut session.bench,
                        &view,
                        &info,
                    );
                    None
                }
                _ => None,
            };
            match request {
                Some(workspaces::Request::Shortcut(command)) => self.run(ctx, command, now),
                Some(workspaces::Request::Down) => self.run(ctx, Command::ConfirmNext, now),
                Some(workspaces::Request::Drop) => self.run(ctx, Command::Drop, now),
                Some(workspaces::Request::Notice(message)) => {
                    if let Stage::Open(session) = &mut self.stage {
                        session.notice = Some((message, now));
                    }
                }
                Some(workspaces::Request::AppendMaterial(id)) => {
                    self.append_material(ctx, &id, now);
                }
                Some(workspaces::Request::OpenMaterial(id)) => {
                    self.switch_to(ctx, Workspace::Material, now);
                    if let Stage::Open(session) = &mut self.stage {
                        session.spaces.material.select(&session.bench, &id);
                    }
                }
                Some(workspaces::Request::Register { kind, name }) => {
                    if let Stage::Open(session) = &mut self.stage {
                        register_entry(session, kind, &name, locale, now);
                    }
                }
                None => {}
            }
            if !agent_actions.is_empty() {
                self.agent_actions(agent_actions, now);
            }
            if let Some(target) = jump {
                self.review_jump(ctx, target, now);
            }
            if let Some(found) = cast_jump {
                self.review_jump(
                    ctx,
                    workspaces::review::Target::Document(found.path, Some(found.byte)),
                    now,
                );
            }
            if let (Overlay::QuickFix { actions, selected }, Stage::Open(session)) =
                (&mut self.overlay, &mut self.stage)
            {
                let anchor = slice_editor::caret_rect(ctx);
                if !quick_fix(
                    ctx, session, actions, selected, popup, anchor, locale, mode, now,
                ) {
                    self.overlay = Overlay::None;
                }
            }
            self.common_tail(ctx, locale, now);
            return;
        }
        // 参照ペインの中身は畳んでいても更新する（ミニヘッダーの有効モードにも使う）
        if let Some(next) = session.reference_cache.update(&session.bench, now) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((next - now).max(0.0)));
        }
        if show_reference {
            let panel = match self.settings.reference_pane_side {
                PaneSide::Right => egui::SidePanel::right("reference"),
                PaneSide::Left => egui::SidePanel::left("reference"),
            };
            let default_width = (width * (1.0 - self.settings.main_ratio)).clamp(320.0, 560.0);
            let response = workspaces::fixed_width(panel, default_width, 14.0)
                .frame(
                    Frame::none()
                        .fill(colors.bg_base)
                        .inner_margin(Margin::symmetric(14.0, 4.0)),
                )
                .show(ctx, |ui| {
                    reference_pane::show(ui, &session.reference_cache, locale, mode, scale)
                });
            if self.focus == Pane::Reference {
                focus_ring(ctx, response.response.rect, colors.accent);
            }
            match response.inner {
                Some(reference_pane::Chosen::Material(id)) => self.append_material(ctx, &id, now),
                Some(reference_pane::Chosen::OpenMaterial(id)) => {
                    self.switch_to(ctx, Workspace::Material, now);
                    if let Stage::Open(session) = &mut self.stage {
                        session.spaces.material.select(&session.bench, &id);
                    }
                }
                // §6.2.6（F-23）: 診断からの自動回復（1 形態素の登録）
                Some(reference_pane::Chosen::Register { kind, name }) => {
                    if let Stage::Open(session) = &mut self.stage {
                        register_entry(session, kind, &name, locale, now);
                    }
                }
                None => {}
            }
        }
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        // 補完のポップアップが出ている間は、↑/↓/Enter/Tab を節エディタより先に取る
        let mut completion_keys = (false, false, false);
        if matches!(
            self.overlay,
            Overlay::Completion { .. } | Overlay::QuickFix { .. }
        ) {
            completion_keys = popup_keys(ctx);
        }
        if let Some(range) = session.pending_selection.take() {
            section_editor::select(ctx, &session.bench, range);
        }
        // ② メインカラム: 文脈ビュー（55%）/ 節エディタ（45%）
        let mut clicked = None;
        let mut changed = false;
        let mut editor_output = section_editor::EditorOutput::default();
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(colors.bg_base)
                    .inner_margin(Margin::symmetric(16.0, 6.0)),
            )
            .show(ctx, |ui| {
                if session.bench.current().is_none() {
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "editor.no_sections"))
                            .color(colors.fg_muted),
                    );
                    return;
                }
                let total = ui.available_height();
                let context_height = (total * self.settings.context_ratio).max(80.0);
                let digits = session.bench.project().digits();
                let scene = session
                    .bench
                    .current()
                    .map(|current| current.id.display(digits))
                    .unwrap_or_default();
                let (section_chars, _, _) = session.bench.counts();
                let target = session.bench.current().and_then(|current| {
                    crate::core::features::reference::target_length(
                        session.bench.project(),
                        current.id,
                    )
                });
                let placeholder = context_view::Placeholder {
                    scene: &scene,
                    chars: section_chars,
                    target,
                    indicator,
                    accessible: i18n::text(locale, "editor.editing"),
                };
                let context =
                    ui.allocate_ui(Vec2::new(ui.available_width(), context_height), |ui| {
                        ui.set_min_height(context_height);
                        context_view::show(
                            ui,
                            &session.bench,
                            &mut session.context_cache,
                            mode,
                            scale,
                            self.settings.context_sections,
                            self.settings.line_chars(locale == Locale::En),
                            locale,
                            &placeholder,
                        )
                    });
                clicked = context.inner;
                if self.focus == Pane::Context {
                    focus_stripe(ctx, context.response.rect, colors.accent);
                }
                ui.add_space(6.0);
                mini_header(ui, session, mode, scale);
                let editor = ui.allocate_ui(ui.available_size(), |ui| {
                    section_editor::show(
                        ui,
                        &mut session.bench,
                        &mut session.editor_cache,
                        section_editor::Look {
                            mode,
                            scale,
                            max_line_chars: self.settings.line_chars(locale == Locale::En),
                            inlay: self.settings.inlay_hints.then_some(locale),
                        },
                    )
                });
                editor_output = editor.inner;
                ui.painter().rect_stroke(
                    editor.response.rect.shrink(0.5),
                    0.0,
                    Stroke::new(1.0_f32, colors.border),
                );
                changed = editor_output.changed;
                if self.focus == Pane::Editor {
                    focus_stripe(ctx, editor.response.rect, colors.accent);
                }
            });
        if editor_output.selection.is_some() {
            session.selection = editor_output.selection.clone();
            session.caret = editor_output.caret;
        }
        if changed {
            session.bench.edited(now);
            // §9.7.3: `'` を入力した瞬間（`:` はキャラ参照の直後、`@` は行頭 / 行末）に補完を出す
            let before = session.selection.as_ref().and_then(|range| {
                session.bench.buffer()[..range.start.min(session.bench.buffer().len())]
                    .chars()
                    .last()
            });
            if matches!(before, Some('\'' | ':' | '@')) && self.overlay == Overlay::None {
                self.overlay = Overlay::Completion { selected: 0 };
            }
        }
        // 最近開いたファイル（`Ctrl+Tab`）
        if let Some(current) = session.bench.current() {
            let entry = (current.path.clone(), current.id);
            match session.recent.first_mut() {
                Some(first) if first.0 == entry.0 => *first = entry,
                _ => {
                    session.recent.retain(|(path, _)| path != &entry.0);
                    session.recent.insert(0, entry);
                    session.recent.truncate(20);
                }
            }
        }
        if let Overlay::Completion { selected } = &mut self.overlay {
            let (down, up, enter) = completion_keys;
            if !completion(ctx, session, selected, (down, up, enter), mode, now) {
                self.overlay = Overlay::None;
            }
        }
        if let Overlay::QuickFix { actions, selected } = &mut self.overlay {
            let anchor = session.caret;
            if !quick_fix(
                ctx,
                session,
                actions,
                selected,
                completion_keys,
                anchor,
                locale,
                mode,
                now,
            ) {
                self.overlay = Overlay::None;
            }
        }
        if let Some(index) = clicked {
            if session.bench.go_to(index) {
                session.settings_dirty = true;
                ctx.memory_mut(|memory| {
                    memory.request_focus(section_editor::editor_id(&session.bench))
                });
            }
        }
        self.common_tail(ctx, locale, now);
    }

    /// どのワークスペースでも毎フレーム行うこと: 保存・外部変更の監視・重ねるもの。
    fn common_tail(&mut self, ctx: &Context, locale: Locale, now: f64) {
        let mode = self.applied.map_or(Mode::Dark, |(mode, _)| mode);
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        // 保存（§8.2）
        save_if_due(session, &mut self.settings, now);
        if let Some(at) = session.bench.save.next_wakeup(now) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((at - now).max(0.0)));
        }
        // 後から分かったこと（Web の非同期の書き込み・Zip の書き出し）
        for event in session.host.take_events() {
            session.notice = Some((host_event_text(locale, &event), now));
        }
        let unsaved = session.saving.is_some() || session.bench.save.state() != State::Clean;
        session.host.unsaved_changed(unsaved);
        // 保存領域が足りない（§13.3.7）: Zip でエクスポートしてから不要なプロジェクトを消す導線を出す
        let quota = session.bench.save.state() == State::Failed
            && session
                .bench
                .save
                .error()
                .is_some_and(|error| error.contains(QUOTA_EXCEEDED));
        if quota && !session.quota_shown && self.overlay == Overlay::None {
            session.quota_shown = true;
            self.overlay = Overlay::SaveError;
        } else if !quota {
            session.quota_shown = false;
        }
        // 外部変更の監視（1,000ms、§8.5）
        if now - session.last_poll >= 1.0 {
            session.last_poll = now;
            for (path, text) in session.host.poll() {
                match session.bench.external_change(&path, text.clone()) {
                    crate::ui::workbench::Reaction::Reloaded => {
                        let message = i18n::format(locale, "diag.I052", &[&path]);
                        session.notice = Some((message, now));
                    }
                    crate::ui::workbench::Reaction::Conflict => {
                        session.conflict_texts.insert(path, text);
                    }
                    crate::ui::workbench::Reaction::Unchanged => {}
                }
            }
        }
        self.overlays(ctx, mode, locale, now);
    }

    // ============================================================ 重ねるもの

    fn overlays(&mut self, ctx: &Context, mode: Mode, locale: Locale, now: f64) {
        let colors = theme::palette(mode);
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        let mut close = false;
        let mut to_write = false;
        let mut material = None;
        let mut confirm_approve = false;
        let mut jump_target = None;
        let workspace = session.spaces.switcher.current();
        match &mut self.overlay {
            Overlay::None => {}
            Overlay::Palette { input, selected } => {
                let (mode_kind, query) = palette::split(input);
                let items = palette_items(&session.bench, locale, mode_kind, query, input);
                let tab = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
                let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
                let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
                // §9.7.1「`Shift+Enter` は別の意味で開く」（辞書なら使用箇所の一覧）
                let other = ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Enter));
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                if down && *selected + 1 < items.len() {
                    *selected += 1;
                }
                if up {
                    *selected = selected.saturating_sub(1);
                }
                if tab {
                    if let Some(item) = items.get(*selected) {
                        if let Some(next) = palette::complete_token(input, &item.key) {
                            *input = next;
                            *selected = 0;
                        }
                    }
                }
                let mut chosen = None;
                let outside = popup(ctx, "palette", colors, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(input)
                            .hint_text(i18n::text(locale, "palette.placeholder"))
                            .desired_width(520.0),
                    );
                    edit.request_focus();
                    if edit.changed() {
                        *selected = 0;
                    }
                    ui.set_width(540.0);
                    ui.add_space(4.0);
                    if items.is_empty() {
                        ui.label(
                            RichText::new(if mode_kind == palette::Mode::Material {
                                i18n::text(locale, "palette.no_materials")
                            } else {
                                i18n::text(locale, "palette.no_matches")
                            })
                            .color(colors.fg_faint),
                        );
                    }
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for (index, item) in items.iter().enumerate() {
                                let label = palette::ellipsize(&item.label, 70);
                                let text = RichText::new(label).color(if index == *selected {
                                    colors.accent
                                } else {
                                    colors.fg_primary
                                });
                                if crate::ui::widgets::selectable_action(
                                    ui,
                                    index == *selected,
                                    text,
                                )
                                .clicked()
                                {
                                    chosen = Some(index);
                                }
                            }
                        });
                    if input.is_empty() {
                        ui.label(
                            RichText::new(i18n::text(locale, "palette.guide"))
                                .color(colors.fg_faint),
                        );
                    }
                });
                if outside {
                    close = true;
                }
                if enter || other {
                    chosen = Some(*selected);
                }
                if let Some(index) = chosen {
                    if let Some(item) = items.get(index) {
                        match &item.action {
                            PaletteAction::Section(at) => {
                                session.bench.go_to(*at);
                                session.settings_dirty = true;
                                to_write = workspace != Workspace::Write;
                            }
                            PaletteAction::SceneDrill(prefix) => {
                                self.overlay = Overlay::Palette {
                                    input: format!("@{prefix}-"),
                                    selected: 0,
                                };
                                return;
                            }
                            PaletteAction::Command(command) => {
                                let command = *command;
                                self.overlay = Overlay::None;
                                self.run(ctx, command, now);
                                return;
                            }
                            PaletteAction::Material(id) => {
                                material = Some(MaterialAction::Open(id.clone()));
                            }
                            PaletteAction::Character(index) if other => {
                                // §9.7.1: Shift+Enter は使用箇所の一覧（`references::at`）
                                let pos = character_definition_pos(session.bench.project(), *index)
                                    .unwrap_or(0);
                                let locations = session
                                    .bench
                                    .project()
                                    .document("characters.md")
                                    .map(|doc| {
                                        crate::core::features::references::at(
                                            doc,
                                            pos,
                                            session.bench.project(),
                                        )
                                    })
                                    .unwrap_or_default();
                                self.overlay = Overlay::References {
                                    name: item.key.clone(),
                                    locations,
                                    selected: 0,
                                };
                                return;
                            }
                            PaletteAction::Term(index) if other => {
                                // §9.7.1: Shift+Enter は使用箇所の一覧（`references::at`）
                                let pos = word_definition_pos(session.bench.project(), *index)
                                    .unwrap_or(0);
                                let locations = session
                                    .bench
                                    .project()
                                    .document("words.md")
                                    .map(|doc| {
                                        crate::core::features::references::at(
                                            doc,
                                            pos,
                                            session.bench.project(),
                                        )
                                    })
                                    .unwrap_or_default();
                                self.overlay = Overlay::References {
                                    name: item.key.clone(),
                                    locations,
                                    selected: 0,
                                };
                                return;
                            }
                            PaletteAction::Character(index) => {
                                // §9.7.1: Enter は定義へ（`definition::at` → `characters.md`）
                                let index = *index;
                                let pos = character_definition_pos(session.bench.project(), index)
                                    .unwrap_or(0);
                                let def =
                                    session.bench.project().document("characters.md").and_then(
                                        |doc| {
                                            crate::core::features::definition::at(
                                                doc,
                                                pos,
                                                session.bench.project(),
                                            )
                                        },
                                    );
                                session.bench.commit();
                                if let Some(loc) = def {
                                    if let Some(doc) = session.bench.project().document(&loc.path) {
                                        let line = crate::core::features::project::line_of(
                                            &doc.text,
                                            loc.range.start,
                                        );
                                        session.spaces.cast.select_line(&session.bench, line);
                                    } else {
                                        session.spaces.cast.select(index);
                                    }
                                } else {
                                    session.spaces.cast.select(index);
                                }
                                session.spaces.switcher.mark_visited(Workspace::Cast);
                                self.overlay = Overlay::None;
                                self.switch_to(ctx, Workspace::Cast, now);
                                return;
                            }
                            PaletteAction::Term(index) => {
                                let pos = word_definition_pos(session.bench.project(), *index)
                                    .unwrap_or(0);
                                let _ =
                                    session
                                        .bench
                                        .project()
                                        .document("words.md")
                                        .and_then(|doc| {
                                            crate::core::features::definition::at(
                                                doc,
                                                pos,
                                                session.bench.project(),
                                            )
                                        });
                                let lines = term_lines(session.bench.project(), *index);
                                self.overlay = Overlay::Info { lines };
                                return;
                            }
                            PaletteAction::Agent(command) => {
                                let command = command.clone();
                                self.overlay = Overlay::None;
                                self.agent_command(command, now);
                                return;
                            }
                            PaletteAction::Welcome => {
                                self.overlay = Overlay::Welcome;
                                return;
                            }
                            PaletteAction::Migration => {
                                self.overlay = Overlay::Migration;
                                return;
                            }
                            PaletteAction::McpConfig => {
                                let path = crate::core::agents::mcp_config::PATH;
                                self.overlay = Overlay::McpConfig {
                                    input: session.bench.project().text(path).to_owned(),
                                    error: None,
                                };
                                return;
                            }
                            PaletteAction::LineWidth => {
                                self.overlay = Overlay::LineWidth {
                                    input: self
                                        .settings
                                        .max_line_chars
                                        .map_or(String::new(), |n| n.to_string()),
                                };
                                return;
                            }
                            PaletteAction::Close => {
                                self.overlay = Overlay::None;
                                return;
                            }
                            PaletteAction::AgentRoot => {
                                self.overlay = Overlay::Palette {
                                    input: ">agent ".into(),
                                    selected: 0,
                                };
                                return;
                            }
                            PaletteAction::ExportZip => {
                                self.overlay = Overlay::None;
                                self.export_zip(now);
                                return;
                            }
                        }
                    }
                    close = true;
                }
            }
            Overlay::McpConfig { input, error } => {
                let mut save = false;
                popup(ctx, "mcp-config", colors, |ui| {
                    ui.set_width(620.0);
                    ui.label("agents/mcp.json");
                    ui.add(
                        egui::TextEdit::multiline(input)
                            .code_editor()
                            .desired_rows(18)
                            .desired_width(f32::INFINITY),
                    );
                    if let Some(message) = error {
                        ui.colored_label(colors.error, message);
                    }
                    ui.horizontal(|ui| {
                        if crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "help.save_mcp"),
                        )
                        .clicked()
                        {
                            save = true;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "help.close"))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                if save {
                    match crate::core::agents::mcp_config::parse(input) {
                        Ok(_) => {
                            session.bench.replace_document(
                                crate::core::agents::mcp_config::PATH,
                                input.clone(),
                                now,
                            );
                            close = true;
                        }
                        Err(message) => *error = Some(message),
                    }
                }
            }
            Overlay::Welcome | Overlay::Migration => {
                let migration = self.overlay == Overlay::Migration;
                popup(
                    ctx,
                    if migration { "migration" } else { "welcome" },
                    colors,
                    |ui| {
                        let key = if migration {
                            "help.migration"
                        } else {
                            "help.welcome"
                        };
                        ui.set_width(500.0);
                        ui.label(i18n::text(locale, key));
                        ui.hyperlink_to(
                            i18n::text(locale, "help.website"),
                            crate::core::model::project::PROJECT_STRUCTURE_DOC_URL,
                        );
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "help.close"))
                            .clicked()
                        {
                            close = true;
                        }
                    },
                );
            }
            Overlay::LineWidth { input } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                popup(ctx, "line-width", colors, |ui| {
                    ui.label(i18n::text(locale, "palette.line_width_hint"));
                    ui.add(egui::TextEdit::singleline(input).desired_width(160.0))
                        .request_focus();
                });
                if enter {
                    if input.trim().is_empty() {
                        self.settings.max_line_chars = None;
                        session.settings_dirty = true;
                        close = true;
                    } else if let Ok(value) = input.trim().parse::<i64>() {
                        self.settings.max_line_chars = Some(value.clamp(20, 120) as usize);
                        session.settings_dirty = true;
                        close = true;
                    }
                }
            }
            Overlay::Jump { input, missed } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                popup(ctx, "jump", colors, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(input)
                            .hint_text("@01-04-02-03")
                            .desired_width(240.0),
                    );
                    edit.request_focus();
                    if *missed {
                        ui.label(
                            RichText::new(i18n::text(locale, "jump.not_found")).color(colors.error),
                        );
                    }
                });
                if enter {
                    match workspace {
                        // plot-outline: 章（話があればそのブロック）へ
                        Workspace::PlotOutline => {
                            match crate::core::features::sections::query_id(input) {
                                Some(id) if id.chapter > 0 => {
                                    session.bench.commit();
                                    session.spaces.outline.land_chapter(
                                        &session.bench,
                                        id.chapter,
                                        id.episode,
                                    );
                                    close = true;
                                }
                                _ => *missed = true,
                            }
                        }
                        // plot-episode: その話の節（無ければ場面・話の先頭）に着地する
                        Workspace::PlotEpisode => {
                            match crate::core::features::sections::query_id(input) {
                                Some(id) if id.chapter > 0 && id.episode > 0 => {
                                    session.bench.commit();
                                    session.spaces.episode.land(&session.bench, id);
                                    close = true;
                                }
                                _ => *missed = true,
                            }
                        }
                        _ => {
                            if session.bench.jump(input) {
                                session.settings_dirty = true;
                                close = true;
                                to_write = workspace != Workspace::Write;
                            } else {
                                *missed = true;
                            }
                        }
                    }
                }
            }
            Overlay::Capture { input } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                popup(ctx, "capture", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "idea.capture_title"))
                            .color(colors.fg_faint),
                    );
                    let edit = ui.add(
                        egui::TextEdit::singleline(input)
                            .hint_text(i18n::text(locale, "idea.capture_hint"))
                            .desired_width(520.0),
                    );
                    edit.request_focus();
                });
                if enter {
                    if !input.trim().is_empty() {
                        let text = session
                            .bench
                            .project()
                            .text(crate::core::features::ideas::PATH)
                            .to_owned();
                        let (updated, id) = crate::core::features::ideas::capture(
                            &text,
                            input,
                            &session.host.now(),
                        );
                        session.bench.replace_document(
                            crate::core::features::ideas::PATH,
                            updated,
                            now,
                        );
                        session.notice = Some((i18n::format(locale, "idea.captured", &[&id]), now));
                    }
                    close = true;
                }
            }
            Overlay::Promote {
                selected,
                characters,
            } => {
                let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
                let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                let scene = session.bench.current_id();
                let digits = session.bench.project().digits();
                let options: Vec<String> = match characters {
                    Some(names) => names.clone(),
                    None => {
                        let plot = match scene {
                            Some(id) => i18n::format(
                                locale,
                                "idea.promote_plot",
                                &[&crate::core::features::plots::short_id(
                                    crate::core::id::SceneId { section: 0, ..id },
                                    digits,
                                )],
                            ),
                            None => i18n::text(locale, "idea.promote_plot_none").to_owned(),
                        };
                        vec![
                            plot,
                            i18n::text(locale, "idea.promote_term").to_owned(),
                            i18n::text(locale, "idea.promote_character").to_owned(),
                            i18n::text(locale, "idea.promote_material").to_owned(),
                        ]
                    }
                };
                if down && *selected + 1 < options.len() {
                    *selected += 1;
                }
                if up {
                    *selected = selected.saturating_sub(1);
                }
                let mut chosen = enter.then_some(*selected);
                popup(ctx, "promote", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "idea.promote_title"))
                            .color(colors.fg_faint),
                    );
                    ui.add_space(4.0);
                    for (index, option) in options.iter().enumerate() {
                        let text = RichText::new(format!("{}  {option}", index + 1)).color(
                            if index == *selected {
                                colors.accent
                            } else {
                                colors.fg_primary
                            },
                        );
                        if crate::ui::widgets::selectable_action(ui, index == *selected, text)
                            .clicked()
                        {
                            chosen = Some(index);
                        }
                    }
                });
                if let Some(index) = chosen {
                    let target = match (characters.is_some(), index) {
                        (true, found) => Some(workspaces::idea::Target::Character(found)),
                        (false, 0) => scene.map(workspaces::idea::Target::Plot),
                        (false, 1) => Some(workspaces::idea::Target::Term),
                        (false, 2) => {
                            let names: Vec<String> = session
                                .bench
                                .project()
                                .characters()
                                .characters
                                .iter()
                                .map(|found| found.name.to_string())
                                .collect();
                            if names.is_empty() {
                                session.notice = Some((
                                    i18n::text(locale, "idea.no_characters").to_owned(),
                                    now,
                                ));
                                close = true;
                            } else {
                                *characters = Some(names);
                                *selected = 0;
                            }
                            None
                        }
                        (false, _) => Some(workspaces::idea::Target::Material),
                    };
                    if let Some(target) = target {
                        let stamp = session.host.now();
                        match session
                            .spaces
                            .idea
                            .promote(&mut session.bench, target, now, &stamp)
                        {
                            Some((id, to)) => {
                                session.notice =
                                    Some((i18n::format(locale, "idea.promoted", &[&id, &to]), now));
                            }
                            None => {
                                session.notice = Some((
                                    i18n::text(locale, "idea.promote_failed").to_owned(),
                                    now,
                                ));
                            }
                        }
                        close = true;
                    } else if index == 0 && characters.is_none() {
                        close = true;
                    }
                }
            }
            Overlay::Help => {
                popup(ctx, "help", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "help.title")).color(colors.fg_faint),
                    );
                    ui.add_space(6.0);
                    egui::Grid::new("help-keys")
                        .num_columns(2)
                        .spacing([16.0, 4.0])
                        .show(ui, |ui| {
                            for (keys, description) in HELP_KEYS {
                                ui.label(
                                    RichText::new(*keys)
                                        .font(FontId::new(12.0, FontFamily::Monospace))
                                        .color(colors.fg_muted),
                                );
                                ui.label(i18n::text(locale, description));
                                ui.end_row();
                            }
                        });
                });
            }
            Overlay::SaveError => {
                let message = session.bench.save.error().unwrap_or_default().to_owned();
                let quota = message.contains(QUOTA_EXCEEDED);
                let mut zip = false;
                popup(ctx, "save-error", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "status.save_failed")).color(colors.error),
                    );
                    ui.label(i18n::text(locale, "status.dont_close"));
                    if quota {
                        // §13.3.7: Zip でエクスポートしてから不要なプロジェクトを削除する導線
                        ui.add_space(4.0);
                        ui.label(i18n::text(locale, "status.quota"));
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new(&message).color(colors.fg_muted));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.retry"))
                            .clicked()
                        {
                            session.bench.save.request_now();
                            close = true;
                        }
                        if quota
                            && crate::ui::widgets::action_button(
                                ui,
                                i18n::text(locale, "command.export_zip"),
                            )
                            .clicked()
                        {
                            zip = true;
                            close = true;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.close"))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                if zip {
                    let name = zip_name(session);
                    if let Some(result) = session.host.export_zip(&name) {
                        session.notice = Some((zip_result_text(locale, result), now));
                    }
                }
            }
            Overlay::Find {
                query,
                replace,
                fresh,
            } => {
                let mut step: Option<bool> = None;
                let mut replace_one = false;
                let mut replace_all = false;
                let count = session
                    .bench
                    .spliced_document()
                    .map(|(document, _)| crate::ui::assist::find_all(&document.text, query).len())
                    .unwrap_or(0);
                egui::Area::new(Id::new("find"))
                    .order(egui::Order::Foreground)
                    .anchor(Align2::RIGHT_TOP, Vec2::new(-24.0, 40.0))
                    .show(ctx, |ui| {
                        Frame::none()
                            .fill(colors.bg_raised)
                            .stroke(Stroke::new(1.0_f32, colors.border))
                            .inner_margin(Margin::same(8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let edit = ui.add(
                                        egui::TextEdit::singleline(query)
                                            .hint_text(i18n::text(locale, "find.placeholder"))
                                            .desired_width(220.0),
                                    );
                                    if *fresh {
                                        edit.request_focus();
                                        *fresh = false;
                                    }
                                    if edit.lost_focus()
                                        && ui.input(|input| input.key_pressed(Key::Enter))
                                    {
                                        step = Some(!ui.input(|input| input.modifiers.shift));
                                        edit.request_focus();
                                    }
                                    ui.label(
                                        RichText::new(i18n::format(
                                            locale,
                                            "find.count",
                                            &[&count.to_string()],
                                        ))
                                        .color(colors.fg_faint),
                                    );
                                });
                                if let Some(replacement) = replace {
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(replacement)
                                                .hint_text(i18n::text(
                                                    locale,
                                                    "find.replace_placeholder",
                                                ))
                                                .desired_width(220.0),
                                        );
                                        // 置換は上書き（破壊的操作）なので文字ラベルを置く（§9.2.1-2）
                                        if crate::ui::widgets::action_button(
                                            ui,
                                            i18n::text(locale, "find.replace"),
                                        )
                                        .clicked()
                                        {
                                            replace_one = true;
                                        }
                                        if crate::ui::widgets::action_button(
                                            ui,
                                            i18n::text(locale, "find.replace_all"),
                                        )
                                        .clicked()
                                        {
                                            replace_all = true;
                                        }
                                    });
                                }
                            });
                    });
                if !query.is_empty() {
                    session.last_find = query.clone();
                }
                if replace_one {
                    let replacement = replace.clone().unwrap_or_default();
                    let selected = session.selection.clone().filter(|range| {
                        session.bench.buffer().get(range.clone()) == Some(query.as_str())
                    });
                    if let Some(range) = selected {
                        session
                            .bench
                            .buffer_mut()
                            .replace_range(range.clone(), &replacement);
                        session.bench.edited(now);
                        let end = range.start + replacement.len();
                        session.selection = Some(end..end);
                    }
                    step = Some(true);
                }
                if replace_all && !query.is_empty() {
                    session.bench.commit();
                    if let Some(current) = session.bench.current() {
                        let text = session
                            .bench
                            .project()
                            .text(&current.path)
                            .replace(query.as_str(), replace.as_deref().unwrap_or_default());
                        session.bench.replace_current_file(text, now);
                    }
                }
                if let Some(forward) = step {
                    let query = query.clone();
                    session.find_step(&query, forward);
                }
            }
            Overlay::Search { query, selected } => {
                let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
                let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                let generation = session.bench.generation();
                let stale = !matches!(
                    &session.search_cache,
                    Some((cached, at, _)) if cached == query && *at == generation
                );
                if stale {
                    let hits = crate::ui::assist::search(session.bench.project(), query);
                    session.search_cache = Some((query.clone(), generation, hits));
                }
                let hits = session
                    .search_cache
                    .as_ref()
                    .map(|(_, _, hits)| hits.clone())
                    .unwrap_or_default();
                if down && *selected + 1 < hits.len() {
                    *selected += 1;
                }
                if up {
                    *selected = selected.saturating_sub(1);
                }
                let mut chosen = enter.then_some(*selected);
                popup(ctx, "search", colors, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(query)
                            .hint_text(i18n::text(locale, "search.placeholder"))
                            .desired_width(520.0),
                    );
                    edit.request_focus();
                    if edit.changed() {
                        *selected = 0;
                    }
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for (index, hit) in hits.iter().enumerate() {
                                let color = if index == *selected {
                                    colors.accent
                                } else {
                                    colors.fg_primary
                                };
                                let text = RichText::new(format!(
                                    "{}:{}  {}",
                                    hit.path, hit.line, hit.text
                                ))
                                .color(color);
                                if crate::ui::widgets::selectable_action(
                                    ui,
                                    index == *selected,
                                    text,
                                )
                                .clicked()
                                {
                                    chosen = Some(index);
                                }
                            }
                        });
                });
                if let Some(hit) = chosen.and_then(|index| hits.get(index)) {
                    session.last_find = query.clone();
                    if let Some(offset) = session.bench.reveal(&hit.path, hit.range.start) {
                        session.pending_selection = Some(offset..offset + hit.range.len());
                    }
                    session.settings_dirty = true;
                    close = true;
                }
            }
            Overlay::Completion { .. } | Overlay::QuickFix { .. } => {}
            Overlay::Materials {
                query,
                selected,
                create,
            } => {
                let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
                let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                let open = ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Enter));
                let generation = session.bench.generation();
                let stale = !matches!(
                    &session.material_cache,
                    Some((cached, at, _)) if cached == query && *at == generation
                );
                if stale {
                    let hits = crate::core::features::materials_ops::search(
                        session.bench.project(),
                        query,
                    );
                    session.material_cache = Some((query.clone(), generation, hits));
                }
                let hits = session
                    .material_cache
                    .as_ref()
                    .map(|(_, _, hits)| hits.clone())
                    .unwrap_or_default();
                if down && *selected + 1 < hits.len() {
                    *selected += 1;
                }
                if up {
                    *selected = selected.saturating_sub(1);
                }
                *selected = (*selected).min(hits.len().saturating_sub(1));
                let mut chosen = enter.then_some(*selected);
                popup(ctx, "materials", colors, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(query)
                            .hint_text(i18n::text(locale, "material.search_hint"))
                            .desired_width(560.0),
                    );
                    edit.request_focus();
                    if edit.changed() {
                        *selected = 0;
                    }
                    ui.add_space(4.0);
                    if hits.is_empty() {
                        ui.label(
                            RichText::new(i18n::text(locale, "material.search_none"))
                                .color(colors.fg_muted),
                        );
                    }
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for (index, hit) in hits.iter().enumerate() {
                                let is_selected = index == *selected;
                                let category = if hit.category.is_empty() {
                                    String::new()
                                } else {
                                    format!("#{}", hit.category)
                                };
                                let text = RichText::new(
                                    format!(
                                        "{}  {}  {}  {}",
                                        hit.id, hit.title, category, hit.date
                                    )
                                    .trim_end()
                                    .to_owned(),
                                )
                                .color(if is_selected {
                                    colors.accent
                                } else {
                                    colors.fg_primary
                                });
                                if crate::ui::widgets::selectable_action(ui, is_selected, text)
                                    .clicked()
                                {
                                    chosen = Some(index);
                                }
                                if is_selected {
                                    // 選んだ行の下に要約と出典（§16.3）
                                    for (line, color) in [
                                        (&hit.summary, colors.fg_muted),
                                        (&hit.source, colors.fg_faint),
                                    ] {
                                        if !line.is_empty() {
                                            ui.label(
                                                RichText::new(format!("    {line}")).color(color),
                                            );
                                        }
                                    }
                                }
                            }
                        });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "material.search_keys"))
                            .small()
                            .color(colors.fg_faint),
                    );
                });
                if std::mem::take(create) {
                    material = Some(MaterialAction::Create(query.clone()));
                } else if open {
                    material = hits
                        .get(*selected)
                        .map(|hit| MaterialAction::Open(hit.id.clone()));
                } else if let Some(hit) = chosen.and_then(|index| hits.get(index)) {
                    material = Some(MaterialAction::Insert(hit.id.clone()));
                }
                if material.is_some() {
                    close = true;
                }
            }
            Overlay::ConfirmApprove { id } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                let mut approve = enter;
                popup(ctx, "confirm-approve", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::format(locale, "diag.W065", &[id.as_str()]))
                            .color(colors.warn),
                    );
                    ui.label(i18n::text(locale, "material.confirm_approve"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "material.confirm_yes"),
                        )
                        .clicked()
                        {
                            approve = true;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.cancel"))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                if approve {
                    confirm_approve = true;
                    close = true;
                }
            }
            Overlay::NewMaterial { input } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                popup(ctx, "new-material", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "material.new_title"))
                            .color(colors.fg_faint),
                    );
                    let edit = ui.add(
                        egui::TextEdit::singleline(input)
                            .hint_text(i18n::text(locale, "material.new_hint"))
                            .desired_width(520.0),
                    );
                    edit.request_focus();
                });
                if enter {
                    let (title, category) =
                        crate::core::features::materials_ops::draft_of(input.trim());
                    let title = if title.trim().is_empty() {
                        i18n::text(locale, "material.untitled").to_owned()
                    } else {
                        title
                    };
                    let stamp = session.host.now();
                    let created = session.spaces.material.create(
                        &mut session.bench,
                        &title,
                        &category,
                        now,
                        &stamp,
                    );
                    if let Some(id) = created {
                        session.notice =
                            Some((i18n::format(locale, "material.created", &[&id]), now));
                    }
                    close = true;
                }
            }
            Overlay::AgentNew {
                role,
                scope,
                intent,
                comment,
                examples,
                fresh,
            } => {
                let roles = crate::core::agents::BUILTIN_ROLES;
                let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
                let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                if down {
                    *role = (*role + 1).min(roles.len() - 1);
                }
                if up {
                    *role = role.saturating_sub(1);
                }
                popup(ctx, "agent-new", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "agent.new_title")).color(colors.fg_faint),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "agent.new_role")).color(colors.fg_muted),
                    );
                    ui.horizontal_wrapped(|ui| {
                        for (index, name) in roles.iter().enumerate() {
                            if crate::ui::widgets::selectable_action(
                                ui,
                                index == *role,
                                RichText::new(*name),
                            )
                            .clicked()
                            {
                                *role = index;
                            }
                        }
                    });
                    ui.label(
                        RichText::new(i18n::text(locale, "agent.new_scope")).color(colors.fg_muted),
                    );
                    ui.add(egui::TextEdit::singleline(scope).desired_width(520.0));
                    ui.label(
                        RichText::new(i18n::text(locale, "agent.new_intent"))
                            .color(colors.fg_muted),
                    );
                    let edit = ui.add(egui::TextEdit::singleline(intent).desired_width(520.0));
                    if *fresh {
                        edit.request_focus();
                        *fresh = false;
                    }
                    ui.label(
                        RichText::new(i18n::text(locale, "agent.new_comment"))
                            .color(colors.fg_muted),
                    );
                    ui.add(egui::TextEdit::singleline(comment).desired_width(520.0));
                    ui.label(i18n::text(locale, "agent.new_examples"));
                    ui.add(
                        egui::TextEdit::multiline(examples)
                            .hint_text(i18n::text(locale, "agent.new_examples_hint"))
                            .desired_rows(2)
                            .desired_width(520.0),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(i18n::text(locale, "agent.new_hint")).color(colors.fg_faint),
                    );
                });
                if enter {
                    let stamp = session.host.now();
                    session.bench.commit();
                    let plan = crate::core::agents::runner::new_tasks(
                        session.bench.project(),
                        &crate::core::agents::runner::Draft {
                            role: roles[*role],
                            scope,
                            intent,
                            comment,
                            examples,
                            initiator: "human",
                            status: "draft",
                        },
                        &stamp,
                    );
                    crate::ui::agents::execute(
                        &plan,
                        &mut session.bench,
                        session.host.as_mut(),
                        now,
                    );
                    let created: Vec<String> = plan
                        .writes
                        .iter()
                        .filter_map(|change| {
                            change
                                .path
                                .strip_prefix(crate::core::agents::files::TASKS_DIR)
                                .map(|name| name.trim_end_matches(".md").to_owned())
                        })
                        .collect();
                    let mut notices: Vec<String> = plan
                        .notices
                        .iter()
                        .map(|notice| crate::ui::agents::notice_text(locale, notice))
                        .collect();
                    if let Some(last) = created.last() {
                        session.spaces.agent.select(last);
                        notices.insert(
                            0,
                            i18n::format(locale, "agent.created", &[&created.join(", ")]),
                        );
                    } else if notices.is_empty() {
                        notices.push(i18n::format(
                            locale,
                            "agent.new_no_scope",
                            &[scope.as_str()],
                        ));
                    }
                    session.notice = Some((notices.join(" / "), now));
                    close = true;
                }
            }
            Overlay::AgentConfirm {
                ids,
                chars,
                servers,
                blocked,
            } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                let mut run = enter;
                popup(ctx, "agent-confirm", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::format(
                            locale,
                            "agent.confirm_title",
                            &[&ids.len().to_string()],
                        ))
                        .color(colors.fg_primary),
                    );
                    ui.label(i18n::format(
                        locale,
                        "agent.confirm_chars",
                        &[&chars.to_string()],
                    ));
                    ui.label(i18n::format(
                        locale,
                        "agent.confirm_servers",
                        &[&servers.join(", ")],
                    ));
                    for line in blocked.iter() {
                        ui.label(RichText::new(line).color(colors.warn));
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "agent.confirm_yes"),
                        )
                        .clicked()
                        {
                            run = true;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.cancel"))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                if run {
                    let ids = ids.clone();
                    let notices = crate::ui::agents::enqueue(
                        &ids,
                        &mut session.bench,
                        session.host.as_mut(),
                        &mut session.agents,
                        now,
                        locale,
                    );
                    session.spaces.agent.marked.clear();
                    if !notices.is_empty() {
                        session.notice = Some((notices.join(" / "), now));
                    }
                    close = true;
                }
            }
            Overlay::AgentComment {
                proposal,
                revert,
                input,
                missing,
            } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                popup(ctx, "agent-comment", colors, |ui| {
                    let title = if *revert {
                        i18n::text(locale, "agent.revert_title")
                    } else {
                        i18n::text(locale, "agent.reject_title")
                    };
                    ui.label(RichText::new(title).color(colors.fg_faint));
                    let edit = ui.add(egui::TextEdit::singleline(input).desired_width(520.0));
                    edit.request_focus();
                    if *missing {
                        ui.label(
                            RichText::new(i18n::text(locale, "agent.comment_required"))
                                .color(colors.error),
                        );
                    }
                });
                if enter {
                    if input.trim().is_empty() {
                        *missing = true;
                    } else {
                        let message =
                            reject_proposal(session, proposal, input, *revert, now, locale);
                        session.notice = Some((message, now));
                        close = true;
                    }
                }
            }
            Overlay::AgentFixed { proposal, text } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                let mut accept = enter;
                popup(ctx, "agent-fixed", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "agent.fixed_confirm")).color(colors.warn),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "agent.accept_yes"),
                        )
                        .clicked()
                        {
                            accept = true;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.cancel"))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                if accept {
                    let message = accept_proposal(session, proposal, text.as_deref(), now, locale);
                    session.notice = Some((message, now));
                    close = true;
                }
            }
            Overlay::PromptReset { name } => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                let mut reset = enter;
                let targets = crate::core::agents::prompts::reset_targets(name);
                popup(ctx, "prompt-reset", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::format(locale, "prompt.reset_title", &[name.as_str()]))
                            .color(colors.fg_primary),
                    );
                    ui.label(
                        RichText::new(i18n::text(locale, "prompt.reset_warning"))
                            .color(colors.error),
                    );
                    ui.add_space(4.0);
                    prompt_pairs(ui, session.bench.project(), &targets, locale, colors);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "prompt.reset_yes"),
                        )
                        .clicked()
                        {
                            reset = true;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.cancel"))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                if reset {
                    let count = crate::ui::agents::reset_prompts(
                        name,
                        &mut session.bench,
                        session.host.as_ref(),
                        now,
                    );
                    session.notice = Some((
                        i18n::format(locale, "prompt.reset_done", &[&count.to_string()]),
                        now,
                    ));
                    close = true;
                }
            }
            Overlay::PromptDiff { name } => {
                let targets = crate::core::agents::prompts::reset_targets(name);
                popup(ctx, "prompt-diff", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::format(locale, "prompt.diff_title", &[name.as_str()]))
                            .color(colors.fg_primary),
                    );
                    ui.add_space(4.0);
                    prompt_pairs(ui, session.bench.project(), &targets, locale, colors);
                    ui.add_space(8.0);
                    if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.close"))
                        .clicked()
                    {
                        close = true;
                    }
                });
            }
            Overlay::Diff => {
                let conflicts: Vec<String> = session.bench.conflicts().map(str::to_owned).collect();
                if conflicts.is_empty() {
                    close = true;
                }
                let mut keep: Option<String> = None;
                let mut take: Option<String> = None;
                popup(ctx, "diff", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "diff.title")).color(colors.fg_faint),
                    );
                    ui.add_space(6.0);
                    for path in &conflicts {
                        let mine = session.bench.project().text(path).lines().count();
                        let theirs = session
                            .conflict_texts
                            .get(path)
                            .and_then(|text| text.as_ref().map(|text| text.lines().count()));
                        ui.label(RichText::new(path).color(colors.fg_primary));
                        ui.label(
                            RichText::new(i18n::format(
                                locale,
                                "diff.lines",
                                &[
                                    &mine.to_string(),
                                    &theirs.map_or_else(|| "—".to_owned(), |n| n.to_string()),
                                ],
                            ))
                            .color(colors.fg_muted),
                        );
                        ui.horizontal(|ui| {
                            // どちらも上書き・破棄（破壊的操作）なので文字ラベルを置く（§9.2.1-2）
                            if crate::ui::widgets::action_button(
                                ui,
                                i18n::text(locale, "diff.keep_mine"),
                            )
                            .clicked()
                            {
                                keep = Some(path.clone());
                            }
                            if crate::ui::widgets::action_button(
                                ui,
                                i18n::text(locale, "diff.take_theirs"),
                            )
                            .clicked()
                            {
                                take = Some(path.clone());
                            }
                        });
                        ui.add_space(6.0);
                    }
                });
                if let Some(path) = keep {
                    session.bench.keep_mine(&path, now);
                    session.conflict_texts.remove(&path);
                }
                if let Some(path) = take {
                    let text = session.conflict_texts.remove(&path).flatten();
                    session.bench.take_external(&path, text);
                }
            }
            Overlay::Info { lines } => {
                let anchor = session.caret.map_or(Pos2::new(120.0, 120.0), |rect| {
                    rect.left_bottom() + Vec2::new(0.0, 6.0)
                });
                egui::Area::new(Id::new("info"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(anchor)
                    .show(ctx, |ui| {
                        Frame::none()
                            .fill(colors.bg_raised)
                            .stroke(Stroke::new(1.0_f32, colors.border))
                            .inner_margin(Margin::same(8.0))
                            .show(ui, |ui| {
                                ui.set_max_width(420.0);
                                for (index, line) in lines.iter().enumerate() {
                                    let color = if index == 0 {
                                        colors.accent
                                    } else if line.starts_with('×') {
                                        colors.fg_faint
                                    } else {
                                        colors.fg_primary
                                    };
                                    ui.label(RichText::new(line).color(color));
                                }
                            });
                    });
            }
            Overlay::ConfirmDelete => {
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                let mut confirmed = enter;
                popup(ctx, "confirm-delete", colors, |ui| {
                    ui.label(i18n::text(locale, "delete.confirm"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "delete.yes"))
                            .clicked()
                        {
                            confirmed = true;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.cancel"))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                if confirmed {
                    session.bench.delete_section(now);
                    session.settings_dirty = true;
                    close = true;
                }
            }
            Overlay::Export {
                range,
                state,
                fresh,
            } => {
                // 先に消費する（`Shift+Enter` は `Enter` より先に。素の `Enter` は Shift も拾うため）
                let cycle = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
                let save_as_key = ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Enter));
                let write_key = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                let copy_key =
                    ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::C));
                if cycle {
                    // すべて → review 以上 → fixed だけ → すべて
                    *state = match *state {
                        None | Some(ExportState::Draft) => Some(ExportState::Review),
                        Some(ExportState::Review) => Some(ExportState::Fixed),
                        Some(ExportState::Fixed) => None,
                    };
                }
                let draft = session.export_draft(range, *state);
                let readable = draft.range.is_some();
                let sections = draft.sections;
                let chars = crate::core::features::sections::count_chars(&draft.text);
                let mut action = if save_as_key {
                    Some(ExportAction::SaveAs)
                } else if write_key {
                    Some(ExportAction::Write)
                } else if copy_key {
                    Some(ExportAction::Copy)
                } else {
                    None
                };
                popup(ctx, "export", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "export.title")).color(colors.fg_faint),
                    );
                    ui.add_space(6.0);
                    let edit = ui.add(
                        egui::TextEdit::singleline(range)
                            .hint_text(i18n::text(locale, "export.range_hint"))
                            .desired_width(440.0),
                    );
                    // `Tab` は状態の切替に使うので、フォーカスは範囲の欄に置いたままにする
                    edit.request_focus();
                    *fresh = false;
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        for (choice, key) in [
                            (None, "export.state_all"),
                            (Some(ExportState::Review), "export.state_review"),
                            (Some(ExportState::Fixed), "export.state_fixed"),
                        ] {
                            let selected = *state == choice;
                            let text = RichText::new(i18n::text(locale, key)).color(if selected {
                                colors.accent
                            } else {
                                colors.fg_muted
                            });
                            if crate::ui::widgets::selectable_action(ui, selected, text).clicked() {
                                *state = choice;
                            }
                        }
                    });
                    ui.add_space(6.0);
                    if readable {
                        let count = i18n::format(
                            locale,
                            "export.preview",
                            &[&sections.to_string(), &chars.to_string()],
                        );
                        ui.label(RichText::new(count).color(colors.fg_muted));
                    } else {
                        ui.label(
                            RichText::new(i18n::text(locale, "export.bad_range"))
                                .color(colors.error),
                        );
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let enabled = readable && sections > 0;
                        for (kind, key) in [
                            (ExportAction::Write, "export.write"),
                            (ExportAction::SaveAs, "export.save_as"),
                            (ExportAction::Copy, "export.copy"),
                        ] {
                            let button = egui::Button::new(
                                RichText::new(i18n::text(locale, key)).underline(),
                            )
                            .min_size(Vec2::splat(24.0));
                            let response = ui.add_enabled(enabled, button);
                            if enabled && response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if response.clicked() {
                                action = Some(kind);
                            }
                        }
                    });
                });
                if let Some(kind) = action {
                    if let Some(result) = export_now(ctx, session, kind, locale, now) {
                        match result {
                            Ok(message) => {
                                session.notice = Some((message, now));
                                close = true;
                            }
                            Err(message) => self.overlay = Overlay::Message(message),
                        }
                    }
                }
            }
            Overlay::Message(text) => {
                popup(ctx, "message", colors, |ui| {
                    for line in text.lines() {
                        ui.label(RichText::new(line).color(colors.fg_muted));
                    }
                    ui.add_space(8.0);
                    if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.close"))
                        .clicked()
                    {
                        close = true;
                    }
                });
            }
            Overlay::CloseGuard => {
                popup(ctx, "close-guard", colors, |ui| {
                    ui.label(
                        RichText::new(i18n::text(locale, "status.save_failed")).color(colors.error),
                    );
                    ui.label(i18n::text(locale, "status.dont_close"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if crate::ui::widgets::action_button(
                            ui,
                            i18n::text(locale, "modal.discard_and_close"),
                        )
                        .clicked()
                        {
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            close = true;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.retry"))
                            .clicked()
                        {
                            session.bench.save.request_now();
                            close = true;
                        }
                        if crate::ui::widgets::action_button(ui, i18n::text(locale, "modal.cancel"))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
            }
            Overlay::Rename {
                document,
                pos,
                input,
                error,
            } => {
                use crate::core::features::rename::RenameError;
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                popup(ctx, "rename", colors, |ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(input)
                            .hint_text(i18n::text(locale, "rename.placeholder"))
                            .desired_width(320.0),
                    );
                    edit.request_focus();
                    if edit.changed() {
                        *error = None;
                    }
                    if let Some(err) = error {
                        let key = match err {
                            RenameError::NotRenamable => "rename.not_renamable",
                            RenameError::Empty => "rename.empty",
                            RenameError::Invalid => "rename.invalid",
                            RenameError::Duplicate => "rename.duplicate",
                        };
                        ui.label(RichText::new(i18n::text(locale, key)).color(colors.error));
                    }
                });
                if enter {
                    match crate::core::features::rename::apply(
                        document,
                        *pos,
                        input,
                        session.bench.project(),
                    ) {
                        Ok(edit) => {
                            let count: usize =
                                edit.changes.iter().map(|change| change.edits.len()).sum();
                            session.bench.apply_edit(&edit, now);
                            session.notice = Some((
                                i18n::format(
                                    locale,
                                    "rename.done",
                                    &[input.trim(), &count.to_string()],
                                ),
                                now,
                            ));
                            close = true;
                        }
                        Err(err) => {
                            *error = Some(err);
                        }
                    }
                }
            }
            Overlay::References {
                name,
                locations,
                selected,
            } => {
                let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
                let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
                let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                if down && *selected + 1 < locations.len() {
                    *selected += 1;
                }
                if up {
                    *selected = selected.saturating_sub(1);
                }
                let mut chosen = enter.then_some(*selected);
                popup(ctx, "references", colors, |ui| {
                    let title = if locations.is_empty() {
                        i18n::format(locale, "references.none", &[name.as_str()])
                    } else {
                        i18n::format(
                            locale,
                            "references.title",
                            &[name.as_str(), &locations.len().to_string()],
                        )
                    };
                    ui.label(RichText::new(title).color(colors.fg_faint));
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for (index, loc) in locations.iter().enumerate() {
                                let (line_num, snippet) = session
                                    .bench
                                    .project()
                                    .document(&loc.path)
                                    .map(|doc| {
                                        let starts = crate::core::features::project::line_starts_of(
                                            &doc.text,
                                        );
                                        let line = crate::core::features::project::line_of(
                                            &doc.text,
                                            loc.range.start,
                                        );
                                        let start = starts.get(line).copied().unwrap_or(0);
                                        let end = doc.text[start..]
                                            .find('\n')
                                            .map_or(doc.text.len(), |at| start + at);
                                        let shown: String =
                                            doc.text[start..end].trim().chars().take(80).collect();
                                        (line + 1, shown)
                                    })
                                    .unwrap_or((1, String::new()));
                                let color = if index == *selected {
                                    colors.accent
                                } else {
                                    colors.fg_primary
                                };
                                let label =
                                    RichText::new(format!("{}:{line_num}  {snippet}", loc.path))
                                        .color(color);
                                if crate::ui::widgets::selectable_action(
                                    ui,
                                    index == *selected,
                                    label,
                                )
                                .clicked()
                                {
                                    chosen = Some(index);
                                }
                            }
                        });
                });
                if let Some(loc) = chosen.and_then(|index| locations.get(index)).cloned() {
                    jump_target = Some((loc.path, loc.range));
                    close = true;
                }
            }
        }
        if close {
            self.overlay = Overlay::None;
            focus_workspace(ctx, session);
        }
        if to_write {
            self.switch_to(ctx, Workspace::Write, now);
            if let Stage::Open(session) = &self.stage {
                let id = section_editor::editor_id(&session.bench);
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
        }
        if confirm_approve {
            self.approve_material(now, true);
        }
        if let Some(action) = material {
            self.material_action(ctx, action, now);
        }
        if let Some((path, range)) = jump_target {
            if let Stage::Open(session) = &mut self.stage {
                if let Some(offset) = session.bench.reveal(&path, range.start) {
                    session.pending_selection = Some(offset..offset + range.len());
                }
            }
            self.review_jump(
                ctx,
                workspaces::review::Target::Document(path, Some(range.start)),
                now,
            );
        }
    }

    /// 終了ガード（§8.4）: 未保存なら止めて保存し、保存が終わったら閉じる。失敗中は 3 択。
    fn close_guard(&mut self, ctx: &Context, now: f64) {
        let requested = ctx.input(|input| input.viewport().close_requested());
        let Stage::Open(session) = &mut self.stage else {
            return;
        };
        if self.allow_close {
            return;
        }
        if requested {
            session.bench.commit();
            session.bench.save.request_now();
            save_if_due(session, &mut self.settings, now);
            if session.bench.save.guards_close() || session.bench.unsaved().next().is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if session.bench.save.state() == State::Failed {
                    self.overlay = Overlay::CloseGuard;
                } else {
                    // 保存が済んだら閉じる（次のフレームで）
                    ctx.request_repaint();
                    ctx.data_mut(|data| data.insert_temp(Id::new("close-after-save"), true));
                }
                return;
            }
            flush_settings(session, &mut self.settings);
        }
        let pending = ctx.data(|data| {
            data.get_temp::<bool>(Id::new("close-after-save"))
                .unwrap_or(false)
        });
        if pending && !session.bench.save.guards_close() && session.bench.unsaved().next().is_none()
        {
            flush_settings(session, &mut self.settings);
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// 保存の時刻なら書く（§8.2）。`ui.json` の変更も一緒に書く。
/// 保存領域が足りないときの失敗の文言に含まれる語（`StorageError::QuotaExceeded` の `Debug`、§13.3.7）。
const QUOTA_EXCEEDED: &str = "QuotaExceeded";

/// Host が後から知らせたことの文言。
fn host_event_text(locale: Locale, event: &HostEvent) -> String {
    match event {
        HostEvent::WriteFailed(path, message) => {
            i18n::format(locale, "status.write_failed", &[path, message])
        }
        HostEvent::ZipExported(name) => i18n::format(locale, "status.zip_exported", &[name]),
        HostEvent::ZipFailed(message) => i18n::format(locale, "status.zip_failed", &[message]),
        HostEvent::LowSpace => i18n::diagnostic(
            locale,
            &crate::core::diagnostics::Diagnostic::simple(
                crate::core::diagnostics::DiagnosticCode::W062,
                None,
            ),
        ),
    }
}

/// Zip の既定の名前の元（`project.json` の題、無ければプロジェクトの名前）。
fn zip_name(session: &Session) -> String {
    match session.bench.project().settings().title.trim() {
        "" => session.host.name(),
        title => title.to_owned(),
    }
}

/// [`Host::export_zip`] がその場で返した結果の文言（取り消したときは「書き出しませんでした」）。
fn zip_result_text(locale: Locale, result: Result<Option<String>, String>) -> String {
    match result {
        Ok(Some(written)) => i18n::format(locale, "status.zip_exported", &[&written]),
        Ok(None) => i18n::text(locale, "status.zip_canceled").to_owned(),
        Err(message) => i18n::format(locale, "status.zip_failed", &[&message]),
    }
}

fn save_if_due(session: &mut Session, settings: &mut UiSettings, now: f64) {
    if session.saving.is_some() {
        // 非同期の保存（Web の IndexedDB）の結果を待つ（B-156）
        if let Some(outcome) = session.host.save_finished() {
            finish_save(session, settings, now, outcome);
        }
        return;
    }
    if session.bench.save.due(now) {
        // 書き込む資料 MD の `updated_at` と `checksum` を索引に（§16.6）
        let stamp = session.host.now();
        session.bench.touch_materials(&stamp);
        let writes = session.bench.take_writes();
        // 動かした文書の元（資料の採用 `inbox/` → `docs/`）は、新しい側を書けてから消す（Host::save）
        let removals = session.bench.take_removals();
        session.bench.save.begin(now);
        session.saving = Some(writes.iter().cloned().collect());
        if let Some(outcome) = session.host.save(writes, removals) {
            finish_save(session, settings, now, outcome);
        }
    } else if session.settings_dirty && session.bench.save.state() == State::Clean {
        flush_settings(session, settings);
    }
}

/// 保存 1 回分の結果を受け取る（同期ならその場、非同期なら後のフレーム）。
fn finish_save(session: &mut Session, settings: &mut UiSettings, now: f64, outcome: SaveOutcome) {
    let handed = session.saving.take().unwrap_or_default();
    // 書いているあいだに変わったファイルは未保存のまま残す（次の保存で書く）
    let written: Vec<String> = outcome
        .written
        .into_iter()
        .filter(|path| {
            handed
                .get(path)
                .is_none_or(|text| session.bench.project().text(path) == text)
        })
        .collect();
    for path in &outcome.removals_left {
        session.bench.removal_failed(path);
    }
    session
        .bench
        .write_finished(now, &written, &outcome.failures);
    if outcome.failures.is_empty() {
        flush_settings(session, settings);
    }
}

/// `ui.json` を書く（プロジェクトの中、§4.4.1・§11.4）。
fn flush_settings(session: &mut Session, settings: &mut UiSettings) {
    if !session.settings_dirty {
        return;
    }
    settings.last_scene = session.bench.last_scene();
    if session
        .host
        .write(crate::ui::settings::UI_JSON, &settings.to_json())
        .is_ok()
    {
        session.settings_dirty = false;
    }
}

/// ステータスバー（§9.6.1）。保存インジケータが押されたら `true`。
fn status_bar(
    ui: &mut egui::Ui,
    session: &mut Session,
    locale: Locale,
    mode: Mode,
    scale: f32,
    indicator: Indicator,
    now: f64,
) -> bool {
    let colors = theme::palette(mode);
    let font = FontId::new(11.0 * scale, FontFamily::Proportional);
    let mono = FontId::new(11.0 * scale, FontFamily::Monospace);
    let mut clicked = false;
    ui.horizontal_centered(|ui| {
        let width = ui.available_width();
        // Reserve the rightmost save state before placing lower-priority labels.
        // A clipped long notice or diagnostic count must never push it off-screen.
        let reserved = if indicator == Indicator::Failed {
            170.0
        } else if session.bench.save.guards_close() {
            46.0
        } else {
            23.0
        };
        ui.allocate_ui_with_layout(
            Vec2::new((width - reserved).max(0.0), ui.available_height()),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                let mut hidden = Vec::new();
                let dot = || {
                    RichText::new("  ·  ")
                        .font(font.clone())
                        .color(colors.fg_faint)
                };
                if let Some(crumb) = session.bench.breadcrumb() {
                    if width >= 900.0 {
                        ui.label(
                            RichText::new(crumb.clone())
                                .font(mono.clone())
                                .color(colors.fg_muted),
                        );
                        ui.label(dot());
                    } else {
                        hidden.push(crumb);
                    }
                }
                let (section, episode, project) = session.bench.counts();
                let unit = i18n::text(locale, "status.chars_unit");
                let chars = format!(
                    "{}{unit} / {}{unit} / {}{unit}",
                    context_view::group(section),
                    context_view::group(episode),
                    context_view::group(project)
                );
                if width >= 410.0 {
                    ui.label(
                        RichText::new(&chars)
                            .font(mono.clone())
                            .color(colors.fg_muted),
                    );
                    ui.label(dot());
                } else {
                    hidden.push(chars);
                }
                let page = session
                    .bench
                    .project()
                    .settings()
                    .writing
                    .manuscript_page_chars
                    .max(1) as f64;
                let pages = format!(
                    "{:.1}{}",
                    episode as f64 / page,
                    i18n::text(locale, "status.pages_unit")
                );
                if width >= 680.0 {
                    ui.label(
                        RichText::new(&pages)
                            .font(mono.clone())
                            .color(colors.fg_muted),
                    );
                } else {
                    hidden.push(pages);
                }
                if width < 950.0 {
                    if let Some((message, at)) = &session.notice {
                        if now - at < 4.0 {
                            hidden.push(message.clone());
                        }
                    }
                }
                if !hidden.is_empty() {
                    ui.label(RichText::new("…").color(colors.fg_faint))
                        .on_hover_text(hidden.join(" · "));
                }
                // 診断件数（E + W、今のファイル。0 件なら出さない）
                let count = diagnostic_count(session);
                if count > 0 {
                    ui.label(dot());
                    let flag = crate::ui::widgets::clickable_label(
                        ui,
                        RichText::new(format!("⚑{count}"))
                            .font(mono.clone())
                            .color(colors.warn),
                        egui::Id::new("status-open-review"),
                    )
                    .on_hover_text(i18n::text(locale, "status.open_review"));
                    if flag.clicked() {
                        session.open_review = true;
                    }
                }
                if session.bench.conflicts().next().is_some() {
                    ui.label(dot());
                    ui.label(RichText::new("⇄").font(mono.clone()).color(colors.error))
                        .on_hover_text(i18n::text(locale, "status.conflict"));
                }
                // 稼働表示（§15.7.4: `⟳ T0042 writer` / `⟳ 3件`、自律なら `(自律)`。クリックで一覧）
                if let Some(label) = crate::ui::agents::status_label(&session.agents, locale) {
                    ui.label(dot());
                    let running = crate::ui::widgets::clickable_label(
                        ui,
                        RichText::new(label).font(mono.clone()).color(colors.accent),
                        egui::Id::new("status-open-agent"),
                    )
                    .on_hover_text(i18n::text(locale, "status.open_agent"));
                    if running.clicked() {
                        session.open_agent = true;
                    }
                }
                if let Some((message, at)) = &session.notice {
                    if now - at < 4.0 && width >= 950.0 {
                        ui.label(dot());
                        ui.label(
                            RichText::new(message)
                                .font(font.clone())
                                .color(colors.fg_faint),
                        );
                    }
                }
                // F-05: live modifier-prefix completion, without displacing the save indicator.
                let mods = ui.ctx().input(|input| input.modifiers);
                let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
                if mods.command || (mac && mods.mac_cmd) {
                    let key = if mods.shift {
                        "status.shortcut_shift"
                    } else if mods.alt {
                        "status.shortcut_alt"
                    } else {
                        "status.shortcut_ctrl"
                    };
                    let detail = i18n::text(locale, key);
                    if ui.available_width() > 230.0 {
                        ui.label(dot());
                        ui.label(
                            RichText::new(palette::ellipsize(detail, 23))
                                .font(font.clone())
                                .color(colors.fg_faint),
                        )
                        .on_hover_text(detail);
                    }
                }
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (rect, response) = ui.allocate_exact_size(
                Vec2::splat(24.0),
                if indicator == Indicator::Failed {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            let painter = ui.painter_at(rect);
            let center = rect.center();
            if indicator == Indicator::Failed {
                painter.line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(0.6_f32, colors.border),
                );
                if response.hovered() {
                    painter.rect_filled(rect, 2.0, colors.bg_selection.gamma_multiply(0.6));
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
            match indicator {
                Indicator::Nothing => {}
                Indicator::Unsaved => {
                    painter.circle_filled(center, 4.0, colors.unsaved);
                }
                Indicator::Spinner => {
                    let angle = (now * 6.0) as f32;
                    let points: Vec<Pos2> = (0..12)
                        .map(|step| {
                            let t = angle + step as f32 * 0.4;
                            center + Vec2::new(t.cos(), t.sin()) * 5.0
                        })
                        .collect();
                    painter.add(egui::Shape::line(
                        points,
                        Stroke::new(1.5_f32, colors.fg_muted),
                    ));
                    ui.ctx().request_repaint();
                }
                Indicator::Failed => {
                    let top = center + Vec2::new(0.0, -6.0);
                    let left = center + Vec2::new(-6.0, 5.0);
                    let right = center + Vec2::new(6.0, 5.0);
                    painter.add(egui::Shape::convex_polygon(
                        vec![top, right, left],
                        colors.error,
                        Stroke::NONE,
                    ));
                    painter.text(
                        center + Vec2::new(0.0, 1.0),
                        Align2::CENTER_CENTER,
                        "!",
                        FontId::new(9.0, FontFamily::Proportional),
                        colors.bg_base,
                    );
                }
            }
            let tooltip = match indicator {
                Indicator::Unsaved => Some("status.unsaved"),
                Indicator::Spinner => Some("status.saving"),
                Indicator::Failed => Some("status.save_failed"),
                Indicator::Nothing => None,
            };
            let response = match tooltip {
                Some(key) => {
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, i18n::text(locale, key))
                    });
                    response.on_hover_text(i18n::text(locale, key))
                }
                None => response,
            };
            if response.clicked() && indicator == Indicator::Failed {
                clicked = true;
            }
            // 「閉じないで」（錠前の開いた形）。Dirty / Saving / SaveFailed のときだけ（§8.4）
            if session.bench.save.guards_close() {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Label,
                        i18n::text(locale, "status.dont_close"),
                    )
                });
                let painter = ui.painter_at(rect.expand(2.0));
                let color = if indicator == Indicator::Failed {
                    colors.error
                } else {
                    colors.fg_muted
                };
                let body = Rect::from_center_size(
                    rect.center() + Vec2::new(0.0, 2.5),
                    Vec2::new(9.0, 6.0),
                );
                painter.rect_stroke(body, 1.0, Stroke::new(1.2_f32, color));
                // 開いた掛け金（左だけ下りている）
                let top = body.min.y - 4.0;
                painter.line_segment(
                    [
                        Pos2::new(body.min.x + 2.0, body.min.y),
                        Pos2::new(body.min.x + 2.0, top),
                    ],
                    Stroke::new(1.2_f32, color),
                );
                painter.line_segment(
                    [
                        Pos2::new(body.min.x + 2.0, top),
                        Pos2::new(body.max.x - 2.0, top),
                    ],
                    Stroke::new(1.2_f32, color),
                );
                painter.line_segment(
                    [
                        Pos2::new(body.max.x - 2.0, top),
                        Pos2::new(body.max.x - 2.0, top + 2.0),
                    ],
                    Stroke::new(1.2_f32, color),
                );
                if indicator == Indicator::Failed {
                    ui.label(
                        RichText::new(i18n::text(locale, "status.dont_close"))
                            .font(font.clone())
                            .color(colors.error),
                    );
                }
            }
        });
    });
    clicked
}

/// 節エディタのミニヘッダー（§9.4.2 の ① の部分: パンくず ＋ 有効モードのバッジ。比率バーは ②）。
fn mini_header(ui: &mut egui::Ui, session: &Session, mode: Mode, scale: f32) {
    let colors = theme::palette(mode);
    let mode_color = theme::token_style(
        mode,
        crate::core::features::semantic_tokens::TokenType::ModifierMode,
    )
    .color;
    let font = FontId::new(11.0 * scale, FontFamily::Monospace);
    ui.horizontal(|ui| {
        ui.set_height(18.0);
        if let Some(crumb) = session.bench.breadcrumb() {
            ui.label(
                RichText::new(crumb)
                    .font(font.clone())
                    .color(colors.fg_faint),
            );
        }
        for cast in &session.reference_cache.content().cast {
            if cast.modes.is_empty() {
                continue;
            }
            let ids: Vec<&str> = cast
                .modes
                .iter()
                .map(|heading| heading.split(' ').next().unwrap_or(""))
                .collect();
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("{} {}", cast.name, ids.join("+")))
                    .font(FontId::new(11.0 * scale, FontFamily::Proportional))
                    .color(mode_color),
            );
        }
    });
}

/// 補完のポップアップ（§9.7.3）。候補が無くなったら `false`（閉じる）。確定したら挿入して閉じる。
fn completion(
    ctx: &Context,
    session: &mut Session,
    selected: &mut usize,
    keys: (bool, bool, bool),
    mode: Mode,
    now: f64,
) -> bool {
    let Some((document, start)) = session.bench.spliced_document() else {
        return false;
    };
    let caret = session.selection.clone().map_or(0, |range| range.end);
    let items =
        crate::core::features::completion::at(&document, start + caret, session.bench.project());
    if items.is_empty() {
        return false;
    }
    let (down, up, enter) = keys;
    if down && *selected + 1 < items.len() {
        *selected += 1;
    }
    if up {
        *selected = selected.saturating_sub(1);
    }
    *selected = (*selected).min(items.len() - 1);
    let mut chosen = enter.then_some(*selected);
    let colors = theme::palette(mode);
    let anchor = session.caret.map_or(Pos2::new(120.0, 120.0), |rect| {
        rect.left_bottom() + Vec2::new(0.0, 4.0)
    });
    egui::Area::new(Id::new("completion"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor)
        .show(ctx, |ui| {
            Frame::none()
                .fill(colors.bg_raised)
                .stroke(Stroke::new(1.0_f32, colors.border))
                .inner_margin(Margin::same(6.0))
                .show(ui, |ui| {
                    // 候補は折り返さない（幅が足りなければ Area が画面の内側へ寄せる）
                    ui.style_mut().wrap = Some(false);
                    for (index, item) in items.iter().enumerate().take(24) {
                        let color = if item.faint {
                            colors.fg_faint
                        } else if index == *selected {
                            colors.accent
                        } else {
                            colors.fg_primary
                        };
                        let text = if item.detail.is_empty() {
                            item.label.clone()
                        } else {
                            format!("{}  {}", item.label, item.detail)
                        };
                        if crate::ui::widgets::selectable_action(
                            ui,
                            index == *selected,
                            RichText::new(text).color(color),
                        )
                        .clicked()
                        {
                            chosen = Some(index);
                        }
                    }
                });
        });
    let Some(item) = chosen.and_then(|index| items.get(index)) else {
        return true;
    };
    let length = session.bench.buffer().len();
    let from = item.range.start.saturating_sub(start).min(length);
    let to = item.range.end.saturating_sub(start).min(length).max(from);
    let buffer = session.bench.buffer_mut();
    if buffer.is_char_boundary(from) && buffer.is_char_boundary(to) {
        buffer.replace_range(from..to, &item.insert_text);
        let caret = from + item.insert_text.len();
        session.bench.edited(now);
        session.pending_selection = Some(caret..caret);
    }
    false
}

/// カーソル位置（`range`、`document` の中の byte）のクイックフィックス。既定（`Enter` 一発で適用）を先頭に。
fn quick_fixes(
    document: &crate::core::model::document::Document,
    range: std::ops::Range<usize>,
    project: &crate::core::features::Project,
) -> Vec<crate::core::features::code_action::CodeAction> {
    let mut actions = crate::core::features::code_action::at(document, range, project);
    actions.sort_by_key(|action| !action.preferred);
    actions
}

/// ポップアップ（補完・クイックフィックス）の ↑ / ↓ / Enter（Tab）を、エディタより先に取る。
fn popup_keys(ctx: &Context) -> (bool, bool, bool) {
    ctx.input_mut(|input| {
        let down = input.consume_key(Modifiers::NONE, Key::ArrowDown);
        let up = input.consume_key(Modifiers::NONE, Key::ArrowUp);
        let enter = input.consume_key(Modifiers::NONE, Key::Enter)
            || input.consume_key(Modifiers::NONE, Key::Tab);
        (down, up, enter)
    })
}

/// クイックフィックスのポップアップ（§9.8.3 `Ctrl+.`）。`anchor` はキャレットの矩形（節エディタか
/// M8 のエディタの）。適用したら閉じる（`false`）。
#[allow(clippy::too_many_arguments)]
fn quick_fix(
    ctx: &Context,
    session: &mut Session,
    actions: &[crate::core::features::code_action::CodeAction],
    selected: &mut usize,
    keys: (bool, bool, bool),
    anchor: Option<Rect>,
    locale: Locale,
    mode: Mode,
    now: f64,
) -> bool {
    let (down, up, enter) = keys;
    if down && *selected + 1 < actions.len() {
        *selected += 1;
    }
    if up {
        *selected = selected.saturating_sub(1);
    }
    let mut chosen = enter.then_some(*selected);
    let colors = theme::palette(mode);
    let anchor = anchor.map_or(Pos2::new(120.0, 120.0), |rect| {
        rect.left_bottom() + Vec2::new(0.0, 4.0)
    });
    egui::Area::new(Id::new("quick-fix"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor)
        .show(ctx, |ui| {
            Frame::none()
                .fill(colors.bg_raised)
                .stroke(Stroke::new(1.0_f32, colors.border))
                .inner_margin(Margin::same(6.0))
                .show(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    for (index, action) in actions.iter().enumerate() {
                        let label = action_label(locale, action);
                        let color = if index == *selected {
                            colors.accent
                        } else {
                            colors.fg_primary
                        };
                        if crate::ui::widgets::selectable_action(
                            ui,
                            index == *selected,
                            RichText::new(label).color(color),
                        )
                        .clicked()
                        {
                            chosen = Some(index);
                        }
                    }
                });
        });
    match chosen.and_then(|index| actions.get(index)) {
        Some(action) => {
            session.bench.apply_edit(&action.edit, now);
            false
        }
        None => true,
    }
}

/// 選択中のテキスト（または診断が指した 1 形態素）を `words.md` / `characters.md` に登録する。
///
/// §6.2.6（F-23）の参照ペインのボタンと、`Ctrl+W` / `Ctrl+Shift+W`（[`Command::Register`]）が
/// 同じ経路を通る。編集の作り方は core の [`register_edit`](crate::core::features::code_action::register_edit)。
fn register_entry(
    session: &mut Session,
    kind: crate::core::resolve::dictionary::EntryKind,
    name: &str,
    locale: Locale,
    now: f64,
) {
    let edit =
        crate::core::features::code_action::register_edit(session.bench.project(), kind, name);
    session.bench.apply_edit(&edit, now);
    let key = match kind {
        crate::core::resolve::dictionary::EntryKind::Character => "register.character_done",
        _ => "register.word_done",
    };
    session.notice = Some((i18n::format(locale, key, &[name]), now));
}

/// クイックフィックスの名前（`action.<種別>` に引数を埋める）。
fn action_label(locale: Locale, action: &crate::core::features::code_action::CodeAction) -> String {
    use crate::core::features::code_action::ActionKind;
    let key = match action.kind {
        ActionKind::RegisterWord => "action.RegisterWord",
        ActionKind::RegisterCharacter => "action.RegisterCharacter",
        ActionKind::ReplaceWithCandidate => "action.ReplaceWithCandidate",
        ActionKind::CloseExplicitForm => "action.CloseExplicitForm",
        ActionKind::ReplaceMode => "action.ReplaceMode",
        ActionKind::KeepFirstTwoModes => "action.KeepFirstTwoModes",
        ActionKind::ReplaceTitle => "action.ReplaceTitle",
        ActionKind::RewriteIndexTitle => "action.RewriteIndexTitle",
        ActionKind::UseOtherMaterialId => "action.UseOtherMaterialId",
        ActionKind::ReplaceMaterialId => "action.ReplaceMaterialId",
        ActionKind::Renumber => "action.Renumber",
        ActionKind::SwapRange => "action.SwapRange",
        ActionKind::InsertSummary => "action.InsertSummary",
        ActionKind::InsertDefinition => "action.InsertDefinition",
        ActionKind::RemoveModifier => "action.RemoveModifier",
        ActionKind::EscapeAt => "action.EscapeAt",
        ActionKind::ReplaceKey => "action.ReplaceKey",
        ActionKind::RemoveSection => "action.RemoveSection",
        ActionKind::UseExplicitForm => "action.UseExplicitForm",
    };
    format!(
        "{}  {}",
        action.diagnostic.as_str(),
        i18n::format(locale, key, &[&action.arg])
    )
}

/// The header badge reflects the whole project (not just the current document).
/// Reuse the previous scan while typing; scanning every file on every keystroke
/// would violate the editor's latency budget. Recalculate after the save settles.
fn project_has_problems(session: &mut Session, locale: Locale) -> bool {
    use crate::core::diagnostics::Severity;
    let rows = if matches!(session.bench.save.state(), State::Dirty | State::Saving) {
        session.spaces.review.cached().unwrap_or_default()
    } else {
        session.spaces.review.diagnostics(&session.bench, locale)
    };
    rows.iter()
        .any(|row| matches!(row.severity, Severity::Error | Severity::Warning))
}

fn diagnostic_count(session: &mut Session) -> usize {
    let Some(current) = session.bench.current() else {
        return 0;
    };
    let key_path = current.path.clone();
    if let Some((generation, path, count)) = &session.diag_count {
        if path == &key_path
            && (*generation == session.bench.generation()
                || matches!(session.bench.save.state(), State::Dirty | State::Saving))
        {
            return *count;
        }
    }
    // Diagnostics on the whole document are deliberately deferred until save;
    // the immediate typing path must not reparse a 100k-character file.
    if matches!(session.bench.save.state(), State::Dirty | State::Saving) {
        return 0;
    }
    let count = session
        .bench
        .project()
        .document(&key_path)
        .map(|document| {
            crate::core::diagnostics::document(document, session.bench.project())
                .iter()
                .filter(|found| {
                    matches!(
                        found.severity(),
                        crate::core::diagnostics::Severity::Error
                            | crate::core::diagnostics::Severity::Warning
                    )
                })
                .count()
        })
        .unwrap_or(0);
    session.diag_count = Some((session.bench.generation(), key_path, count));
    count
}

fn focus_ring(ctx: &Context, rect: Rect, color: Color32) {
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("focus-ring"),
    ))
    .rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(2.0_f32, color));
}

fn focus_stripe(ctx: &Context, rect: Rect, color: Color32) {
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("focus-stripe"),
    ))
    .rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
        0.0,
        color,
    );
}

/// 画面上部の中央に出す小窓（パレット・ジャンプ・ヘルプ・モーダル）。背景を暗くする。
/// 重ね物を閉じた後・`Esc` のフォーカス: write は節エディタ、M8 のワークスペースは Slice のエディタ。
/// agent にはエディタが無いので、フォーカスを手放す（素のキーが agent の列に届くように）。
fn focus_workspace(ctx: &Context, session: &Session) {
    let id = match session.spaces.switcher.current() {
        Workspace::Write => section_editor::editor_id(&session.bench),
        Workspace::Agent => {
            if let Some(id) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(id));
            }
            return;
        }
        _ => slice_editor::editor_id(&session.bench),
    };
    ctx.memory_mut(|memory| memory.request_focus(id));
}

/// 提案を採用する（§15.7.3 `A`。`text` は一部採用のテキスト）。知らせる文言を返す。
fn accept_proposal(
    session: &mut Session,
    id: &str,
    text: Option<&str>,
    now: f64,
    locale: Locale,
) -> String {
    session.bench.commit();
    let stamp = session.host.now();
    match crate::core::agents::apply::accept(session.bench.project(), id, text, &stamp) {
        Ok(plan) => {
            crate::ui::agents::execute(&plan, &mut session.bench, session.host.as_mut(), now);
            i18n::format(locale, "agent.accepted", &[id])
        }
        Err(_) => i18n::format(locale, "agent.refused", &[id]),
    }
}

/// 却下・差し戻し（§15.7.3 `X`。コメント必須）。知らせる文言を返す。
fn reject_proposal(
    session: &mut Session,
    id: &str,
    comment: &str,
    revert: bool,
    now: f64,
    locale: Locale,
) -> String {
    session.bench.commit();
    let stamp = session.host.now();
    let project = session.bench.project();
    let result = if revert {
        crate::core::agents::apply::revert(project, id, comment, &stamp)
    } else {
        crate::core::agents::apply::reject(project, id, comment, &stamp)
    };
    match result {
        Ok(plan) => {
            crate::ui::agents::execute(&plan, &mut session.bench, session.host.as_mut(), now);
            if revert {
                i18n::format(locale, "agent.reverted", &[id])
            } else {
                i18n::format(locale, "agent.rejected", &[id])
            }
        }
        Err(_) => i18n::format(locale, "agent.refused", &[id]),
    }
}

/// `Shift+R` の確認の中身（§15.7.5 (1)）: 合計入力・使用サーバ・走れないものの理由。
fn confirm_summary(
    session: &Session,
    ids: &[String],
    locale: Locale,
) -> (usize, Vec<String>, Vec<String>) {
    let stamp = session.host.now();
    let title = session.host.name();
    let mut chars = 0;
    let mut servers: Vec<String> = Vec::new();
    let mut blocked = Vec::new();
    for id in ids {
        let env = |name: &str| session.host.env_var(name);
        match crate::core::agents::runner::prepare(
            session.bench.project(),
            id,
            &stamp,
            &title,
            &env,
        ) {
            Ok(start) => {
                chars += start.input_chars;
                if !servers.contains(&start.server.name) {
                    servers.push(start.server.name.clone());
                }
            }
            Err(blocker) => blocked.push(crate::ui::agents::blocker_text(locale, id, &blocker)),
        }
    }
    (chars, servers, blocked)
}

/// `>prompt reset` / `>prompt diff` の左右 2 ペイン（現行版と既定版の全文。色分けしない、§15.5.4 (3)）。
fn prompt_pairs(
    ui: &mut egui::Ui,
    project: &crate::core::features::project::Project,
    targets: &[(&'static str, &'static str)],
    locale: Locale,
    colors: theme::Palette,
) {
    let font = FontId::new(11.0, FontFamily::Monospace);
    egui::ScrollArea::vertical()
        .max_height(440.0)
        .show(ui, |ui| {
            ui.set_width(760.0);
            for (path, default) in targets {
                let current = project.document(path).map(|found| found.text.clone());
                ui.label(RichText::new(*path).color(colors.fg_primary));
                if current.as_deref() == Some(*default) {
                    ui.label(
                        RichText::new(i18n::text(locale, "prompt.reset_same"))
                            .color(colors.fg_faint),
                    );
                    ui.add_space(6.0);
                    continue;
                }
                ui.columns(2, |columns| {
                    columns[0].label(
                        RichText::new(i18n::text(locale, "prompt.reset_current"))
                            .color(colors.fg_faint),
                    );
                    columns[0].label(
                        RichText::new(current.unwrap_or_else(|| {
                            i18n::text(locale, "prompt.reset_missing").to_owned()
                        }))
                        .font(font.clone())
                        .color(colors.fg_muted),
                    );
                    columns[1].label(
                        RichText::new(i18n::text(locale, "prompt.reset_default"))
                            .color(colors.fg_faint),
                    );
                    columns[1].label(
                        RichText::new(*default)
                            .font(font.clone())
                            .color(colors.fg_muted),
                    );
                });
                ui.add_space(6.0);
            }
        });
}

fn popup(ctx: &Context, id: &str, colors: theme::Palette, add: impl FnOnce(&mut egui::Ui)) -> bool {
    let screen = ctx.screen_rect();
    // Catch clicks outside the foreground area; never let a click through to an
    // editor or destructive control beneath it (F-27).
    let outside = egui::Area::new(Id::new((id, "shield")))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.allocate_exact_size(screen.size(), Sense::click())
                .1
                .clicked()
        })
        .inner;
    ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, Id::new("dim")))
        .rect_filled(screen, 0.0, Color32::from_black_alpha(96));
    egui::Area::new(Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 72.0))
        .show(ctx, |ui| {
            Frame::none()
                .fill(colors.bg_raised)
                .stroke(Stroke::new(1.0_f32, colors.border))
                .inner_margin(Margin::same(12.0))
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    add(ui);
                });
        });
    outside
}

/// `>export` のモーダルでの操作（§17.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportAction {
    /// `Enter`: プロジェクトの `exports/` に書く（既定の名前）。
    Write,
    /// `Shift+Enter`: 名前を付けて保存（保存ダイアログ。§13.1.1 の特例）。
    SaveAs,
    /// `Ctrl+Shift+C`: クリップボードへ。
    Copy,
}

/// エクスポートを実行する（下書きはモーダルが作ったもの）。何もしなかった（当てはまる節が無い・
/// 範囲が読めない・ダイアログを取り消した）なら `None`、結果の文言（成功は通知・失敗はモーダル）。
fn export_now(
    ctx: &Context,
    session: &mut Session,
    kind: ExportAction,
    locale: Locale,
    now: f64,
) -> Option<Result<String, String>> {
    let _ = now;
    let draft = session.export_cache.as_ref()?;
    let range = draft.range.clone()?;
    if draft.sections == 0 {
        return None;
    }
    let text = draft.text.clone();
    let state = draft.key.2;
    match kind {
        ExportAction::Copy => {
            let chars = crate::core::features::sections::count_chars(&text);
            ctx.copy_text(text);
            Some(Ok(i18n::format(
                locale,
                "export.copied",
                &[&chars.to_string()],
            )))
        }
        ExportAction::Write => {
            let name = session.export_name(range.as_ref(), export_text::Format::Text);
            let path = format!("exports/{name}");
            Some(match session.host.write(&path, &text) {
                Ok(()) => Ok(i18n::format(locale, "export.written", &[&path])),
                Err(error) => Err(i18n::format(locale, "export.failed", &[&error])),
            })
        }
        ExportAction::SaveAs => {
            let name = session.export_name(range.as_ref(), export_text::Format::Text);
            let chosen = session.host.choose_export(&name)?;
            // `.md` を選んだら見出しを `#` で書き直す（§17.2）
            let format = export_text::Format::of_name(&chosen.path().to_string_lossy());
            let text = if format == export_text::Format::Markdown {
                let project = session.bench.project();
                let mut options = export_text::Options::from_settings(&project.settings().export);
                options.state = state;
                options.format = format;
                export_text::run(project, range.as_ref(), &options)
            } else {
                text
            };
            Some(match session.host.write_export(chosen, &text) {
                Ok(written) => Ok(i18n::format(locale, "export.written", &[&written])),
                Err(ExportFailure::Refused(diagnostic)) => {
                    let reason = i18n::diagnostic(locale, &diagnostic);
                    Err(i18n::format(locale, "export.failed", &[&reason]))
                }
                Err(ExportFailure::Failed(error)) => {
                    Err(i18n::format(locale, "export.failed", &[&error]))
                }
            })
        }
    }
}

/// パレットの候補 1 つ。
struct PaletteItem {
    key: String,
    label: String,
    action: PaletteAction,
}

#[derive(Debug, Clone)]
enum PaletteAction {
    Section(usize),
    /// Open the next level of the @ chapter → episode → scene → section tree.
    SceneDrill(String),
    Command(Command),
    /// `M` / `#` の資料（material ワークスペースで開く）。
    Material(String),
    /// `'` のキャラ（`characters.md` の H1 の順。cast で開く）。
    Character(usize),
    /// `'` の用語（`words.md` の H1 の順。全文を出す）。
    Term(usize),
    /// エージェントのコマンド（`>agent …` / `>prompt …`）。
    Agent(AgentCommand),
    /// `>zip`: Zip でエクスポートする（§13.5）。
    ExportZip,
    /// パレットを閉じる。
    Close,
    /// `>agent` 階層の入口。
    AgentRoot,
    LineWidth,
    Welcome,
    Migration,
    McpConfig,
}

/// パレットのエージェントのコマンド（§15.7.6・§15.5.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum AgentCommand {
    /// `>agent delegation <role> <level>`。
    Level(String, crate::core::model::delegation::Delegation),
    /// `>agent stop-all`。
    StopAll,
    /// `>prompt reset <name>`（`all` は全部）。
    PromptReset(String),
    /// `>prompt diff <name>`。
    PromptDiff(String),
}

/// `>prompt reset` / `>prompt diff` の名前（既定の役割・共有の 2 つ）。
fn prompt_names() -> Vec<&'static str> {
    crate::core::template::DEFAULT_PROMPTS
        .iter()
        .map(|(path, _)| crate::core::agents::prompts::file_name(path))
        .collect()
}

/// パレットのエージェントのコマンドの候補（`>agent delegation <role> <level>` は 7 × 4 件）。
fn agent_palette_items(locale: Locale) -> Vec<PaletteItem> {
    use crate::core::model::delegation::Delegation;
    let mut out = Vec::new();
    for role in crate::core::agents::BUILTIN_ROLES {
        for level in [
            Delegation::Off,
            Delegation::Propose,
            Delegation::Apply,
            Delegation::Autonomous,
        ] {
            let key = format!("agent delegation {role} {}", level.as_str());
            out.push(PaletteItem {
                label: format!(
                    ">{key}  {}",
                    i18n::format(locale, "command.agent_delegation", &[role, level.as_str()])
                ),
                key,
                action: PaletteAction::Agent(AgentCommand::Level((*role).to_owned(), level)),
            });
        }
    }
    out.push(PaletteItem {
        key: "agent mcp".into(),
        label: format!(">agent mcp  {}", i18n::text(locale, "help.edit_mcp")),
        action: PaletteAction::McpConfig,
    });
    out.push(PaletteItem {
        key: "agent stop-all".to_owned(),
        label: format!(
            ">agent stop-all  {}",
            i18n::text(locale, "command.agent_stop_all")
        ),
        action: PaletteAction::Agent(AgentCommand::StopAll),
    });
    let mut names = prompt_names();
    names.push("all");
    for name in names {
        let key = format!("prompt reset {name}");
        out.push(PaletteItem {
            label: format!(
                ">{key}  {}",
                i18n::format(locale, "command.prompt_reset", &[name])
            ),
            key,
            action: PaletteAction::Agent(AgentCommand::PromptReset(name.to_owned())),
        });
    }
    for name in prompt_names() {
        let key = format!("prompt diff {name}");
        out.push(PaletteItem {
            label: format!(
                ">{key}  {}",
                i18n::format(locale, "command.prompt_diff", &[name])
            ),
            key,
            action: PaletteAction::Agent(AgentCommand::PromptDiff(name.to_owned())),
        });
    }
    out
}

/// 用語の全文（パレットの `'用語` の `Enter`。`Ctrl+Shift+W` と同じ形: 1 行目が用語）。
fn term_lines(project: &crate::core::features::project::Project, index: usize) -> Vec<String> {
    use crate::core::text::lexer::FieldKey;
    let words = project.words();
    let Some(word) = words.words.get(index) else {
        return Vec::new();
    };
    let mut out = vec![word.term.trim().to_owned()];
    if let Some(definition) = words.word_fields(index, FieldKey::Definition).first() {
        out.extend(definition.full_value().lines().map(str::to_owned));
    }
    out
}

/// `materials/index.json` の byte `at` を含む行までで最後の `"id": "…"` の値（点検から資料を開く）。
fn material_id_before(text: &str, at: usize) -> Option<String> {
    let mut end = at.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let line_end = text[end..]
        .find('\n')
        .map_or(text.len(), |offset| end + offset);
    let key = text[..line_end].rfind("\"id\"")?;
    let rest = text[key + 4..].trim_start().strip_prefix(':')?;
    let rest = rest.trim_start().strip_prefix('"')?;
    let value: String = rest.chars().take_while(|c| *c != '"').collect();
    (!value.is_empty()).then_some(value)
}

/// `characters.md` の `index` 番目のキャラの H1 名前の先頭 byte。
fn character_definition_pos(
    project: &crate::core::features::project::Project,
    index: usize,
) -> Option<usize> {
    project
        .definition_of(&crate::core::resolve::dictionary::Target::Character {
            index,
            name: String::new(),
        })
        .map(|loc| loc.range.start)
}

/// `words.md` の `index` 番目の用語の H1 の先頭 byte。
fn word_definition_pos(
    project: &crate::core::features::project::Project,
    index: usize,
) -> Option<usize> {
    project
        .definition_of(&crate::core::resolve::dictionary::Target::Word {
            index,
            term: String::new(),
        })
        .map(|loc| loc.range.start)
}

/// 今のワークスペースの編集対象の文書とカーソル位置（byte）。
fn active_document_and_caret(
    session: &Session,
    workspace: Workspace,
) -> Option<(crate::core::model::document::Document, usize)> {
    if workspace == Workspace::Write {
        let (document, start) = session.bench.spliced_document()?;
        let caret = start + session.selection.as_ref().map_or(0, |range| range.start);
        Some((document, caret))
    } else {
        let (document, start) = session.bench.slice_document()?;
        let caret = start + session.bench.slice_caret().map_or(0, |range| range.start);
        Some((document, caret))
    }
}

/// パレットのコマンド（`>`）。キーは英語の固定名（§9.7.1 の例 `>theme`）。
const PALETTE_COMMANDS: &[(&str, &str, Command)] = &[
    ("theme", "command.theme", Command::CycleTheme),
    ("language", "command.language", Command::ToggleLanguage),
    ("save", "command.save", Command::SaveNow),
    ("reload", "command.reload", Command::Reload),
    ("new-section", "command.new_section", Command::NewSection),
    ("jump", "command.jump", Command::Jump),
    ("reference", "command.reference", Command::ToggleReference),
    ("inlay-hints", "command.inlay_hints", Command::InlayHints),
    ("export", "command.export", Command::Export),
    ("format", "command.format", Command::Format),
    ("rename", "command.rename", Command::Rename),
    ("definition", "command.definition", Command::Definition),
    ("references", "command.references", Command::References),
    ("help", "command.help", Command::Help),
];

/// All remaining keyboard commands are also reachable through `>` (F-15).
const SHORTCUT_PALETTE_COMMANDS: &[(&str, &str, Command)] = &[
    ("palette", "Ctrl+P", Command::Palette),
    ("open", "Ctrl+O", Command::Open),
    ("font-up", "Ctrl++", Command::FontUp),
    ("font-down", "Ctrl+-", Command::FontDown),
    ("font-reset", "Ctrl+0", Command::FontReset),
    ("focus-context", "Ctrl+1", Command::FocusContext),
    ("focus-editor", "Ctrl+2", Command::FocusEditor),
    ("focus-reference", "Ctrl+3", Command::FocusReference),
    ("swap-reference", "Ctrl+Shift+\\", Command::SwapReference),
    ("confirm-next", "Ctrl+Enter", Command::ConfirmNext),
    ("delete-section", "Ctrl+Shift+;", Command::DeleteSection),
    ("find", "Ctrl+F", Command::Find),
    ("replace", "Ctrl+H", Command::Replace),
    ("search", "Ctrl+Shift+F", Command::Search),
    ("find-next", "F3", Command::FindNext(true)),
    ("find-previous", "Shift+F3", Command::FindNext(false)),
    ("complete", "Ctrl+Space", Command::Complete),
    ("modes", "Ctrl+Shift+M", Command::Modes),
    ("term", "Ctrl+Alt+Shift+W (Web)", Command::Term),
    ("recent-next", "Ctrl+Tab", Command::RecentFile(true)),
    (
        "recent-previous",
        "Ctrl+Shift+Tab",
        Command::RecentFile(false),
    ),
    ("quick-fix", "Ctrl+.", Command::QuickFix),
    (
        "register-term",
        "Ctrl+T",
        Command::Register { character: false },
    ),
    (
        "register-character",
        "Ctrl+Alt+Shift+T (Web)",
        Command::Register { character: true },
    ),
    ("diff", "Ctrl+Shift+D", Command::Diff),
    (
        "ratio-context-up",
        "Ctrl+Alt+Up",
        Command::ContextRatio(true),
    ),
    (
        "ratio-context-down",
        "Ctrl+Alt+Down",
        Command::ContextRatio(false),
    ),
    (
        "ratio-main-left",
        "Ctrl+Alt+Shift+Left",
        Command::MainRatio(false),
    ),
    (
        "ratio-main-right",
        "Ctrl+Alt+Shift+Right",
        Command::MainRatio(true),
    ),
    (
        "previous-section",
        "Alt+Up",
        Command::Nav(crate::ui::workbench::Nav::PrevSection),
    ),
    (
        "next-section",
        "Alt+Down",
        Command::Nav(crate::ui::workbench::Nav::NextSection),
    ),
    (
        "previous-scene",
        "Alt+Left",
        Command::Nav(crate::ui::workbench::Nav::PrevScene),
    ),
    (
        "next-scene",
        "Alt+Right",
        Command::Nav(crate::ui::workbench::Nav::NextScene),
    ),
    (
        "previous-file",
        "Ctrl+Alt+Left",
        Command::Nav(crate::ui::workbench::Nav::PrevFile),
    ),
    (
        "next-file",
        "Ctrl+Alt+Right",
        Command::Nav(crate::ui::workbench::Nav::NextFile),
    ),
    (
        "file-start",
        "Ctrl+Home",
        Command::Nav(crate::ui::workbench::Nav::FileStart),
    ),
    (
        "file-end",
        "Ctrl+End",
        Command::Nav(crate::ui::workbench::Nav::FileEnd),
    ),
    ("previous-workspace", "Alt+0", Command::PreviousWorkspace),
    ("up", "Ctrl+B", Command::Up),
    ("new-item", "Ctrl+Alt+Shift+N (Web)", Command::InsertSection),
    ("capture", "Ctrl+Alt+Shift+I (Web)", Command::Capture),
    ("review", "Ctrl+Shift+E", Command::Review),
    ("drop", "Ctrl+Delete", Command::Drop),
    ("next-tab", "Ctrl+Shift+Right", Command::Tab(true)),
    ("previous-tab", "Ctrl+Shift+Left", Command::Tab(false)),
    ("scan", "Ctrl+Shift+V", Command::Scan),
    ("material-search", "Ctrl+M", Command::MaterialSearch),
    ("stop-all", "Ctrl+Shift+X", Command::StopAll),
    ("fold", "Ctrl+Shift+LeftBracket", Command::Fold(true)),
    ("unfold", "Ctrl+Shift+RightBracket", Command::Fold(false)),
    ("workspace-write", "Alt+1", Command::Workspace(1)),
    ("workspace-episode", "Alt+2", Command::Workspace(2)),
    ("workspace-outline", "Alt+3", Command::Workspace(3)),
    ("workspace-idea", "Alt+4", Command::Workspace(4)),
    ("workspace-material", "Alt+5", Command::Workspace(5)),
    ("workspace-cast", "Alt+6", Command::Workspace(6)),
    ("workspace-reader", "Alt+7", Command::Workspace(7)),
    ("workspace-agent", "Alt+8", Command::Workspace(8)),
    ("workspace-review", "Alt+9", Command::Workspace(9)),
];

fn palette_items(
    bench: &Workbench,
    locale: Locale,
    mode: palette::Mode,
    query: &str,
    input: &str,
) -> Vec<PaletteItem> {
    let mut result: Vec<PaletteItem> = match mode {
        palette::Mode::Command => {
            let mut all: Vec<PaletteItem> = PALETTE_COMMANDS
                .iter()
                .map(|(name, key, command)| PaletteItem {
                    key: (*name).to_owned(),
                    label: format!(">{name}  {}", i18n::text(locale, key)),
                    action: PaletteAction::Command(*command),
                })
                .collect();
            all.extend(
                SHORTCUT_PALETTE_COMMANDS
                    .iter()
                    .map(|(name, shortcut, command)| PaletteItem {
                        key: (*name).to_owned(),
                        label: format!(">{name}  {shortcut}"),
                        action: PaletteAction::Command(*command),
                    }),
            );
            all.push(PaletteItem {
                key: "welcome".into(),
                label: format!(">welcome  {}", i18n::text(locale, "help.welcome_title")),
                action: PaletteAction::Welcome,
            });
            all.push(PaletteItem {
                key: "migration".into(),
                label: format!(">migration  {}", i18n::text(locale, "help.migration_title")),
                action: PaletteAction::Migration,
            });
            all.push(PaletteItem {
                key: "mcp".into(),
                label: format!(">mcp  {}", i18n::text(locale, "help.edit_mcp")),
                action: PaletteAction::McpConfig,
            });
            all.push(PaletteItem {
                key: "line-width".into(),
                label: format!(">line-width  {}", i18n::text(locale, "palette.line_width")),
                action: PaletteAction::LineWidth,
            });
            all.push(PaletteItem {
                key: "zip".to_owned(),
                label: format!(">zip  {}", i18n::text(locale, "command.export_zip")),
                action: PaletteAction::ExportZip,
            });
            if (query.starts_with("agent") && (query != "agent" || input.ends_with(' ')))
                || query.starts_with("prompt")
            {
                all.extend(agent_palette_items(locale));
            } else {
                all.push(PaletteItem {
                    // The trailing space marks a subcommand boundary for Tab completion.
                    key: "agent ".into(),
                    label: ">agent  Agent settings ›".into(),
                    action: PaletteAction::AgentRoot,
                });
            }
            let ranked = palette::rank(query, &all, |item| (item.key.as_str(), item.key.as_str()));
            ranked
                .into_iter()
                .map(|item| PaletteItem {
                    key: item.key.clone(),
                    label: item.label.clone(),
                    action: item.action.clone(),
                })
                .collect()
        }
        palette::Mode::Help => {
            let all: Vec<_> = PALETTE_COMMANDS
                .iter()
                .map(|(name, key, command)| PaletteItem {
                    key: (*name).to_owned(),
                    label: format!("? {name}  {}", i18n::text(locale, key)),
                    action: PaletteAction::Command(*command),
                })
                .collect();
            palette::rank(query, &all, |item| (&item.key, &item.label))
                .into_iter()
                .map(|item| PaletteItem {
                    key: item.key.clone(),
                    label: item.label.clone(),
                    action: item.action.clone(),
                })
                .collect()
        }
        palette::Mode::Scene => {
            // Broadest level first. Choosing a branch appends '-' and narrows the
            // next frame; a leaf jumps to the section. Typed prefixes still filter.
            let depth = query.split('-').count().saturating_sub(1).min(3);
            let digits = bench.project().digits();
            let mut nodes = std::collections::BTreeMap::<String, PaletteItem>::new();
            for (index, section) in bench.sections().iter().enumerate() {
                let full = section.id.display(digits);
                let prefix = full
                    .split('-')
                    .take(depth + 1)
                    .collect::<Vec<_>>()
                    .join("-");
                if !prefix.starts_with(query) {
                    continue;
                }
                let branch = depth < 3;
                let title = match depth {
                    0 => bench.project().all_plot().and_then(|plot| {
                        plot.blocks
                            .iter()
                            .find(|block| {
                                block.id.is_some_and(|id| id.chapter == section.id.chapter)
                            })
                            .and_then(|block| plot.lines.get(block.header))
                            .map(|line| {
                                line.raw()
                                    .split_once(char::is_whitespace)
                                    .map_or("", |(_, title)| title)
                            })
                    }),
                    1 => bench
                        .project()
                        .chapter_plot(section.id.chapter)
                        .and_then(|plot| {
                            plot.blocks
                                .iter()
                                .find(|block| {
                                    block.id.is_some_and(|id| id.episode == section.id.episode)
                                })
                                .and_then(|block| plot.lines.get(block.header))
                                .map(|line| {
                                    line.raw()
                                        .split_once(char::is_whitespace)
                                        .map_or("", |(_, title)| title)
                                })
                        }),
                    _ => Some(section.scene_title.as_str()),
                }
                .unwrap_or("");
                let label = if branch {
                    format!("@{prefix}-*  {title}")
                } else {
                    format!("@{prefix}  {}", section.scene_title)
                };
                nodes.entry(prefix.clone()).or_insert(PaletteItem {
                    key: prefix.clone(),
                    label,
                    action: if branch {
                        PaletteAction::SceneDrill(prefix)
                    } else {
                        PaletteAction::Section(index)
                    },
                });
            }
            nodes.into_values().take(palette::LIMIT).collect()
        }
        palette::Mode::Fuzzy => {
            let digits = bench.project().digits();
            let all: Vec<PaletteItem> = bench
                .sections()
                .iter()
                .enumerate()
                .map(|(index, section)| {
                    let id = section.id.display(digits);
                    PaletteItem {
                        label: format!("@{id}  {}", section.scene_title),
                        key: id,
                        action: PaletteAction::Section(index),
                    }
                })
                .collect();
            let ranked =
                palette::rank(query, &all, |item| (item.key.as_str(), item.label.as_str()));
            ranked
                .into_iter()
                .map(|item| PaletteItem {
                    key: item.key.clone(),
                    label: item.label.clone(),
                    action: item.action.clone(),
                })
                .collect()
        }
        palette::Mode::Material => {
            // `#取材 甲子園` の `#` はカテゴリの語として §16.3 の検索にそのまま渡す（`#` だけなら全部）
            let text = if input.trim_start().starts_with('#') {
                input.trim()
            } else {
                query
            };
            let text = if text == "#" { "" } else { text };
            crate::core::features::materials_ops::search(bench.project(), text)
                .into_iter()
                .take(palette::LIMIT)
                .map(|hit| {
                    let category = if hit.category.is_empty() {
                        String::new()
                    } else {
                        format!("#{}", hit.category)
                    };
                    PaletteItem {
                        label: format!("{}  {}  {}  {}", hit.id, hit.title, category, hit.date)
                            .trim_end()
                            .to_owned(),
                        key: hit.id.clone(),
                        action: PaletteAction::Material(hit.id),
                    }
                })
                .collect()
        }
        palette::Mode::Dictionary => {
            use crate::core::text::lexer::FieldKey;
            let project = bench.project();
            // (鍵, 照合する文字列, 候補)。照合は名前・読み・別名
            let mut all: Vec<(String, String, PaletteItem)> = Vec::new();
            let characters = project.characters();
            for (index, character) in characters.characters.iter().enumerate() {
                let name = character.name.trim().to_owned();
                let mut haystack = name.clone();
                for key in [FieldKey::Reading, FieldKey::Alias] {
                    for field in characters.character_fields(index, key) {
                        haystack.push(' ');
                        haystack.push_str(&field.full_value());
                    }
                }
                all.push((
                    name.clone(),
                    haystack,
                    PaletteItem {
                        label: format!("'{name}  {}", i18n::text(locale, "palette.character")),
                        key: name,
                        action: PaletteAction::Character(index),
                    },
                ));
            }
            let words = project.words();
            for (index, word) in words.words.iter().enumerate() {
                let term = word.term.trim().to_owned();
                // §6.1.4（F-24）: `characters.md` にもある名前は登場人物側に統合して、
                // パレットに同じ名前を 2 行出さない
                if project.dictionary().is_character_word_duplicate(&term) {
                    continue;
                }
                let mut haystack = term.clone();
                for key in [FieldKey::Reading, FieldKey::Alias] {
                    for field in words.word_fields(index, key) {
                        haystack.push(' ');
                        haystack.push_str(&field.full_value());
                    }
                }
                let definition = words
                    .word_fields(index, FieldKey::Definition)
                    .first()
                    .map(|field| field.full_value())
                    .unwrap_or_default();
                let first = definition.lines().next().unwrap_or("").to_owned();
                all.push((
                    term.clone(),
                    haystack,
                    PaletteItem {
                        label: format!("'{term}  {}  {first}", i18n::text(locale, "palette.term"))
                            .trim_end()
                            .to_owned(),
                        key: term,
                        action: PaletteAction::Term(index),
                    },
                ));
            }
            palette::rank(query, &all, |entry| (entry.0.as_str(), entry.1.as_str()))
                .into_iter()
                .map(|entry| PaletteItem {
                    key: entry.2.key.clone(),
                    label: entry.2.label.clone(),
                    action: entry.2.action.clone(),
                })
                .collect()
        }
    };
    result.push(PaletteItem {
        key: "~close".into(),
        label: i18n::text(locale, "palette.close").into(),
        action: PaletteAction::Close,
    });
    result
}

/// キーボードヘルプ（§9.8 のうち M7・M8 で動くもの）。左はキー、右はカタログのキー。
const HELP_KEYS: &[(&str, &str)] = &[
    ("Ctrl+P", "help.palette"),
    ("Ctrl+G", "help.jump"),
    ("Ctrl+S", "help.save"),
    ("Alt+↑ / Alt+↓", "help.section"),
    ("Alt+← / Alt+→", "help.scene"),
    ("Ctrl+Alt+← / →", "help.file"),
    ("Ctrl+Enter", "help.confirm_next"),
    ("Ctrl+;", "help.new_section"),
    ("Ctrl+1 / 2 / 3", "help.focus"),
    ("Ctrl+\\", "help.reference"),
    ("Ctrl+Shift+\\", "help.swap"),
    ("Ctrl+Alt+↑ / ↓", "help.context_ratio"),
    ("Ctrl+Alt+Shift+← / →", "help.main_ratio"),
    ("Ctrl+Plus / Minus / 0", "help.font"),
    ("Ctrl+Alt+L", "help.language"),
    ("Ctrl+Alt+T", "help.theme"),
    ("Alt+H", "help.inlay_hints"),
    ("F5", "help.reload"),
    ("Ctrl+F / Ctrl+H", "help.find"),
    ("F3 / Shift+F3", "help.find_next"),
    ("Ctrl+Shift+F", "help.search"),
    ("Ctrl+Space", "help.complete"),
    ("Ctrl+Shift+M", "help.modes"),
    ("Ctrl+Shift+W", "help.term"),
    ("Ctrl+Shift+;", "help.delete_section"),
    ("Ctrl+Tab", "help.recent"),
    ("Ctrl+O", "help.open"),
    ("Ctrl+.", "help.quick_fix"),
    ("Ctrl+T / Ctrl+Shift+T", "help.register"),
    ("Ctrl+Shift+D", "help.diff"),
    ("Alt+1..9 / Alt+0", "help.workspace"),
    ("Ctrl+B", "help.up"),
    ("Ctrl+Shift+I", "help.capture"),
    ("Ctrl+Shift+E", "help.review"),
    ("Ctrl+M", "help.material"),
    ("Ctrl+Shift+N", "help.insert"),
    ("Ctrl+Delete", "help.drop"),
    ("Ctrl+Shift+← / →", "help.tab"),
    ("Ctrl+Shift+V", "help.scan"),
    ("F2", "help.rename"),
    ("Ctrl+Shift+[ / ]", "help.fold"),
    ("Esc", "help.escape"),
];

// ============================================================ デスクトップ

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};

    use egui::{Context, FontFamily, FontId, Frame, Key, Margin, Modifiers, RichText, Vec2};

    use super::{Host, NovnowApp, Picker, Session};
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
            // ネイティブはタイムゾーンが分からないので UTC（`PLANS.md` §9.2 B-92・B-123）
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
        ) -> Result<String, super::ExportFailure> {
            // 名前・履歴ファイルの判定を先に（違反を関門の口に渡すと debug ビルドは panic する）
            let gate = self.storage.gate().clone();
            if let Err(violation) = gate.check_chosen_export(&chosen) {
                return Err(super::ExportFailure::Refused(violation.diagnostic()));
            }
            gate.write_chosen_export(chosen, text.as_bytes())
                .map(|path| cli::display_path(&path))
                .map_err(|error| super::ExportFailure::Failed(format!("{error:?}")))
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

    /// §14.4 のプロジェクト選択（デスクトップ）。
    struct NativePicker {
        recent: Vec<(String, bool, String)>,
        selected: usize,
        problem: Option<(PathBuf, String, bool)>,
        want_folder: bool,
        want_new: bool,
        want_import: bool,
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
            // Delete removes only the history entry, never the project's files.
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
                            ui.hyperlink_to(
                                i18n::text(locale, "help.website"),
                                crate::core::model::project::PROJECT_STRUCTURE_DOC_URL,
                            );
                            ui.label(
                                RichText::new(i18n::text(locale, "project.pick.empty"))
                                    .color(colors.fg_faint),
                            );
                        }
                        for (index, (path, exists, opened_at)) in self.recent.iter().enumerate() {
                            let name = Path::new(path)
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| path.clone());
                            let shown = if *exists {
                                format!(
                                    "{name}    {path}    {}",
                                    opened_at.get(..10).unwrap_or(opened_at)
                                )
                            } else {
                                format!("{}    {path}", i18n::text(locale, "project.pick.missing"))
                            };
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
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new("×").underline())
                                            .small()
                                            .min_size(Vec2::splat(24.0)),
                                    )
                                    .on_hover_text(i18n::text(
                                        locale,
                                        "project.pick.remove_history",
                                    ))
                                    .clicked()
                                {
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
                                                ui.hyperlink(url);
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
}

/// [`MemoryHost`] の時計（`YYYYMMDD-HHMM`。テストで既定のファイル名を決め打ちできるように固定）。
pub const MEMORY_STAMP: &str = "20261013-2140";

/// [`MemoryHost`] の時計（RFC 3339）。
pub const MEMORY_NOW: &str = "2026-10-13T21:40:00+09:00";

/// メモリの `Host`（テストと、ファイルを持たない環境の確認用）。中身は `Rc` で共有するので、
/// `Session` に渡した後も外から読める。
#[derive(Debug, Default, Clone)]
pub struct MemoryHost {
    /// 共有の中身。
    pub files: std::rc::Rc<std::cell::RefCell<MemoryFiles>>,
}

/// [`MemoryHost`] の中身。
#[derive(Debug, Default)]
pub struct MemoryFiles {
    /// 書かれたファイル（パス → 中身）。
    pub written: std::collections::BTreeMap<String, String>,
    /// 書き込みの回数。
    pub writes: usize,
    /// 書き込みを失敗させる。
    pub fail: bool,
    /// 次の監視で外部変更として返すもの。
    pub changes: Vec<(String, Option<String>)>,
    /// 次の「名前を付けて保存」のダイアログで選ばれるパス（`None` なら取り消し）。
    pub chosen: Option<String>,
    /// 「名前を付けて保存」で書かれたもの（パス → 中身）。
    pub exported: std::collections::BTreeMap<String, String>,
    /// 消したファイル（順に）。
    pub removed: Vec<String>,
    /// 追記されたもの（監査ログ。パス → 中身）。
    pub appended: std::collections::BTreeMap<String, String>,
    /// LLM を呼べる環境にする（MCP の台本を使う）。
    pub mcp: bool,
    /// MCP の台本: 呼び出しが終わるたびに先頭から 1 つずつ返す（無ければ `text.`）。
    pub replies: std::collections::VecDeque<
        Result<crate::ports::mcp::McpReply, crate::ports::mcp::McpError>,
    >,
    /// 始まった呼び出し（順に。番号は 1 から）。
    pub jobs: Vec<crate::ui::agents::Job>,
    /// まだ終わっていない呼び出しの番号。
    pub pending: Vec<u64>,
    /// 呼び出しを終わらせずに止めておく（実行中の表示・並列の確かめ）。
    pub hold: bool,
    /// 取り消された呼び出しの番号。
    pub canceled: Vec<u64>,
    /// 環境変数。
    pub env: std::collections::BTreeMap<String, String>,
    /// 一覧に無いがディスクにあることにするパス（外を指すシンボリックリンクの代わり）。
    pub occupied: Vec<String>,
    /// 保存を非同期にする（Web の IndexedDB の代わり。`save` は始めるだけで、[`MemoryFiles::release_save`]
    /// で終わらせる。B-156）。
    pub async_saves: bool,
    /// 始まったまま終わっていない保存（`async_saves`）。
    pub held_save: Option<HeldSave>,
    /// 終わった保存の結果（次のフレームで `save_finished` が返す）。
    pub finished_save: Option<SaveOutcome>,
}

/// [`MemoryFiles::held_save`] の中身（書くものと、消すもの）。
pub type HeldSave = (Vec<(String, String)>, Vec<String>);

impl MemoryFiles {
    /// 止めておいた保存を終わらせる（書き込みは `fail` に従う）。
    pub fn release_save(&mut self) {
        let Some((writes, removals)) = self.held_save.take() else {
            return;
        };
        let mut outcome = SaveOutcome::default();
        for (path, text) in writes {
            if self.fail {
                outcome.failures.push((path, "disk full".to_owned()));
            } else {
                self.writes += 1;
                self.written.insert(path.clone(), text);
                outcome.written.push(path);
            }
        }
        if outcome.failures.is_empty() {
            self.removed.extend(removals);
        } else {
            outcome.removals_left = removals;
        }
        self.finished_save = Some(outcome);
    }
}

impl Host for MemoryHost {
    fn write(&mut self, path: &str, text: &str) -> Result<(), String> {
        let mut files = self.files.borrow_mut();
        if files.fail {
            return Err("disk full".into());
        }
        files.writes += 1;
        files.written.insert(path.to_owned(), text.to_owned());
        Ok(())
    }

    fn poll(&mut self) -> Vec<(String, Option<String>)> {
        std::mem::take(&mut self.files.borrow_mut().changes)
    }

    fn save(
        &mut self,
        writes: Vec<(String, String)>,
        removals: Vec<String>,
    ) -> Option<SaveOutcome> {
        if self.files.borrow().async_saves {
            self.files.borrow_mut().held_save = Some((writes, removals));
            return None;
        }
        Some(save_now(self, writes, removals))
    }

    fn save_finished(&mut self) -> Option<SaveOutcome> {
        self.files.borrow_mut().finished_save.take()
    }

    fn name(&self) -> String {
        "memory".into()
    }

    fn stamp(&self) -> String {
        MEMORY_STAMP.into()
    }

    fn now(&self) -> String {
        MEMORY_NOW.into()
    }

    fn remove(&mut self, path: &str) -> Result<(), String> {
        let mut files = self.files.borrow_mut();
        files.written.remove(path);
        files.removed.push(path.to_owned());
        Ok(())
    }

    fn choose_export(&mut self, suggested: &str) -> Option<ChosenPath> {
        let _ = suggested;
        let path = self.files.borrow_mut().chosen.take()?;
        Some(ChosenPath::from_save_dialog(
            PathBuf::from(path),
            crate::ports::dialog::ExportKind::Text,
        ))
    }

    fn write_export(&mut self, chosen: ChosenPath, text: &str) -> Result<String, ExportFailure> {
        let name = chosen.path().to_string_lossy().into_owned();
        // 関門と同じ名前の規則（§13.1.1 の特例: `.txt` / `.md` だけ）
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".txt") && !lower.ends_with(".md") {
            let violation = crate::core::policy::PolicyViolation {
                target: chosen.path().to_path_buf(),
                reason: crate::core::policy::Violation::NotAnExport,
            };
            return Err(ExportFailure::Refused(violation.diagnostic()));
        }
        let mut files = self.files.borrow_mut();
        if files.fail {
            return Err(ExportFailure::Failed("disk full".into()));
        }
        files.exported.insert(name.clone(), text.to_owned());
        Ok(name)
    }

    fn append(&mut self, path: &str, text: &str) -> Result<(), String> {
        let mut files = self.files.borrow_mut();
        if files.fail {
            return Err("disk full".into());
        }
        files
            .appended
            .entry(path.to_owned())
            .or_default()
            .push_str(text);
        Ok(())
    }

    fn mcp_available(&self) -> bool {
        self.files.borrow().mcp
    }

    fn mcp_start(&mut self, job: crate::ui::agents::Job) -> Option<u64> {
        let mut files = self.files.borrow_mut();
        if !files.mcp {
            return None;
        }
        files.jobs.push(job);
        let number = files.jobs.len() as u64;
        files.pending.push(number);
        Some(number)
    }

    fn mcp_poll(&mut self) -> Vec<crate::ui::agents::Finished> {
        let mut files = self.files.borrow_mut();
        if files.hold {
            return Vec::new();
        }
        let pending = std::mem::take(&mut files.pending);
        pending
            .into_iter()
            .map(|number| {
                let reply = files.replies.pop_front().unwrap_or_else(|| {
                    Ok(crate::ports::mcp::McpReply {
                        text: "text.".to_owned(),
                        model: "memory".to_owned(),
                        truncated: false,
                    })
                });
                (number, reply, 10)
            })
            .collect()
    }

    fn mcp_cancel(&mut self, job: u64) {
        let mut files = self.files.borrow_mut();
        files.pending.retain(|number| *number != job);
        files.canceled.push(job);
    }

    fn env_var(&self, name: &str) -> Option<String> {
        self.files.borrow().env.get(name).cloned()
    }

    fn occupied(&self, path: &str) -> bool {
        self.files
            .borrow()
            .occupied
            .iter()
            .any(|found| found == path)
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
