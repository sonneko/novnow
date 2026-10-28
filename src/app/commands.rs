//! パレット（`>`）が実行するコマンドの表（§9.7・§19.2.3）。**egui に依存しない**。
//!
//! GUI のパレット候補（`ui::app::palette_items`）とヘッドレスの `session.command`
//! （§19.2.3）が**同じ表**を使う。表を 2 箇所に置くと片方だけにコマンドが増える
//! （ドリフト）ので、ここが単一の真実。

use crate::app::command::Command;

/// パレットのコマンド（`>`）。キーは英語の固定名（§9.7.1 の例 `>theme`）。
pub const PALETTE_COMMANDS: &[(&str, &str, Command)] = &[
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

/// 残りのキーボード操作も `>`（パレット）から辿れるようにするための一覧（F-15）。
pub const SHORTCUT_PALETTE_COMMANDS: &[(&str, &str, Command)] = &[
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
        Command::Nav(crate::app::workbench::Nav::PrevSection),
    ),
    (
        "next-section",
        "Alt+Down",
        Command::Nav(crate::app::workbench::Nav::NextSection),
    ),
    (
        "previous-scene",
        "Alt+Left",
        Command::Nav(crate::app::workbench::Nav::PrevScene),
    ),
    (
        "next-scene",
        "Alt+Right",
        Command::Nav(crate::app::workbench::Nav::NextScene),
    ),
    (
        "previous-file",
        "Ctrl+Alt+Left",
        Command::Nav(crate::app::workbench::Nav::PrevFile),
    ),
    (
        "next-file",
        "Ctrl+Alt+Right",
        Command::Nav(crate::app::workbench::Nav::NextFile),
    ),
    (
        "file-start",
        "Ctrl+Home",
        Command::Nav(crate::app::workbench::Nav::FileStart),
    ),
    (
        "file-end",
        "Ctrl+End",
        Command::Nav(crate::app::workbench::Nav::FileEnd),
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
