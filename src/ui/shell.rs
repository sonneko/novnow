//! 共通シェルの入力処理: IME 変換中の判定（§9.8.5）とキーボード（§9.8）。
//!
//! 1 フレームの入力処理は必ずこの順（§9.8.5 (3)、MUST）:
//!
//! ```text
//! ImeState::update()  →  composing なら Key イベントを全部捨てる  →  ショートカット解釈  →  テキスト編集
//! ```
//!
//! [`take_commands`] がこの 3 段を行い、解釈したショートカットのイベントは**消費する**
//! （節エディタの `TextEdit` に `Ctrl+Enter` などが届かないように）。

use egui::{Event, InputState, Key, Modifiers};

/// IME の変換状態（§9.8.5 (2)。`TextEditState::has_ime` は外から読めないので自前で持つ）。
#[derive(Debug, Default, Clone)]
pub struct ImeState {
    composing: bool,
    last_event_at: Option<f64>,
}

/// 変換中のまま `Composition*` が途絶えたときに解除するまでの秒数（§9.8.5 (4)）。
pub const IME_TIMEOUT_SECS: f64 = 2.0;

impl ImeState {
    /// 毎フレーム、ショートカット解釈より前に呼ぶ（MUST）。
    pub fn update(&mut self, input: &InputState) {
        self.update_with(&input.events, input.time);
    }

    /// [`ImeState::update`] の本体（イベント列と時刻だけを見る。テスト用にも使う）。
    pub fn update_with(&mut self, events: &[Event], now: f64) {
        let mut text_or_key = false;
        for event in events {
            match event {
                Event::CompositionStart | Event::CompositionUpdate(_) => {
                    self.composing = true;
                    self.last_event_at = Some(now);
                }
                Event::CompositionEnd(_) => {
                    self.composing = false;
                    self.last_event_at = Some(now);
                }
                // フェイルセーフ 1: フォーカスを失ったら解除（§9.8.5 (4)）
                Event::WindowFocused(false) => {
                    self.composing = false;
                }
                Event::Text(_) | Event::Key { .. } => text_or_key = true,
                _ => {}
            }
        }
        // フェイルセーフ 2: 最後の Composition* から 2 秒、その間 Text も Key も来ていなければ解除
        if self.composing {
            if text_or_key {
                // 変換中の打鍵（Key / Text）が来ている間は張り付きではない
                self.last_event_at = Some(now);
            } else if self
                .last_event_at
                .is_some_and(|at| now - at >= IME_TIMEOUT_SECS)
            {
                self.composing = false;
            }
        }
    }

    /// 変換中か。
    pub fn composing(&self) -> bool {
        self.composing
    }
}

pub use crate::app::command::Command;

/// キー 1 つをショートカットに読む。`text_focused` は入力欄にフォーカスがあるか（`?` の判定）。
///
/// macOS では `Option` が文字入力に使われるので、`Alt` 系（矢印以外）は `Ctrl+Alt` に読み替える
/// （§9.8.4 MUST。`egui::Modifiers::ctrl` は macOS でも Control キー）。
pub fn command_of(
    key: Key,
    modifiers: Modifiers,
    mac: bool,
    text_focused: bool,
) -> Option<Command> {
    use crate::ui::workbench::Nav;
    let ctrl = modifiers.ctrl || (modifiers.mac_cmd && mac);
    let alt = modifiers.alt;
    let shift = modifiers.shift;
    let arrow = matches!(
        key,
        Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight
    );
    // macOS の読み替え: 矢印以外の `Alt+X` は `Ctrl+Alt+X` で受ける
    let plain_alt = if mac && !arrow {
        ctrl && alt
    } else {
        alt && !ctrl
    };
    let command = match key {
        Key::P if ctrl && !alt && shift => Command::HumanProposal,
        Key::Comma if ctrl && alt && !shift => Command::SuggestComma,
        Key::Comma if plain_alt && !shift => Command::InsertComma,
        Key::S if ctrl && alt && !shift => Command::StatsMode,
        Key::Num1 if ctrl && shift && !alt => Command::CarryWorkspace(1),
        Key::Num2 if ctrl && shift && !alt => Command::CarryWorkspace(2),
        Key::Num3 if ctrl && shift && !alt => Command::CarryWorkspace(3),
        Key::Num4 if ctrl && shift && !alt => Command::CarryWorkspace(4),
        Key::Num5 if ctrl && shift && !alt => Command::CarryWorkspace(5),
        Key::Num6 if ctrl && shift && !alt => Command::CarryWorkspace(6),
        Key::Num7 if ctrl && shift && !alt => Command::CarryWorkspace(7),
        Key::Num8 if ctrl && shift && !alt => Command::CarryWorkspace(8),
        Key::Num9 if ctrl && shift && !alt => Command::CarryWorkspace(9),
        Key::P if ctrl && !alt && !shift => Command::Palette,
        Key::G if ctrl && !alt && !shift => Command::Jump,
        Key::O if ctrl && !alt && !shift => Command::Open,
        Key::S if ctrl && !alt && !shift => Command::SaveNow,
        Key::F5 if !ctrl && !alt => Command::Reload,
        Key::Plus | Key::Equals if ctrl && !alt => Command::FontUp,
        Key::Minus if ctrl && !alt => Command::FontDown,
        Key::Num0 if ctrl && !alt && !shift => Command::FontReset,
        Key::Num1 if ctrl && !alt && !shift => Command::FocusContext,
        Key::Num2 if ctrl && !alt && !shift => Command::FocusEditor,
        Key::Num3 if ctrl && !alt && !shift => Command::FocusReference,
        Key::Backslash if ctrl && !alt && shift => Command::SwapReference,
        Key::Backslash if ctrl && !alt => Command::ToggleReference,
        Key::L if ctrl && alt => Command::ToggleLanguage,
        Key::T if ctrl && alt && !shift => Command::CycleTheme,
        Key::Questionmark if !text_focused && !ctrl && !alt => Command::Help,
        Key::Slash if !text_focused && shift && !ctrl && !alt => Command::Help,
        Key::Escape => Command::Escape,
        Key::Enter if ctrl && !alt && !shift => Command::ConfirmNext,
        Key::Semicolon if ctrl && !alt && !shift => Command::NewSection,
        // `Shift+;` は配列によって `:` として来る
        Key::Semicolon | Key::Colon if ctrl && !alt && shift => Command::DeleteSection,
        Key::F if ctrl && !alt && shift => Command::Search,
        Key::F if ctrl && !alt => Command::Find,
        Key::H if ctrl && !alt && !shift => Command::Replace,
        Key::H if plain_alt && !shift => Command::InlayHints,
        Key::F3 if !ctrl && !alt => Command::FindNext(!shift),
        Key::F12 if !ctrl && !alt && !shift => Command::Definition,
        Key::F12 if !ctrl && !alt && shift => Command::References,
        Key::Space if ctrl && !alt && !shift => Command::Complete,
        Key::M if ctrl && !alt && shift => Command::Modes,
        Key::M if ctrl && !alt => Command::MaterialSearch,
        Key::W if ctrl && shift => Command::Term, // Ctrl+Alt+Shift+W: Web close-window-safe fallback
        Key::Tab if ctrl && !alt => Command::RecentFile(!shift),
        Key::Period if ctrl && !alt && !shift => Command::QuickFix,
        Key::T if ctrl && (!alt || shift && alt) => Command::Register { character: shift },
        Key::D if ctrl && !alt && shift => Command::Diff,
        Key::ArrowUp if ctrl && alt && !shift => Command::ContextRatio(true),
        Key::ArrowDown if ctrl && alt && !shift => Command::ContextRatio(false),
        Key::ArrowLeft if ctrl && alt && shift => Command::MainRatio(false),
        Key::ArrowRight if ctrl && alt && shift => Command::MainRatio(true),
        Key::ArrowLeft if ctrl && alt => Command::Nav(Nav::PrevFile),
        Key::ArrowRight if ctrl && alt => Command::Nav(Nav::NextFile),
        Key::ArrowUp if plain_alt && !shift => Command::Nav(Nav::PrevSection),
        Key::ArrowDown if plain_alt && !shift => Command::Nav(Nav::NextSection),
        Key::ArrowLeft if plain_alt && !shift => Command::Nav(Nav::PrevScene),
        Key::ArrowRight if plain_alt && !shift => Command::Nav(Nav::NextScene),
        Key::Home if ctrl && !alt && !shift => Command::Nav(Nav::FileStart),
        Key::End if ctrl && !alt && !shift => Command::Nav(Nav::FileEnd),
        Key::Num0 if plain_alt && !shift => Command::PreviousWorkspace,
        Key::Num1 if plain_alt && !shift => Command::Workspace(1),
        Key::Num2 if plain_alt && !shift => Command::Workspace(2),
        Key::Num3 if plain_alt && !shift => Command::Workspace(3),
        Key::Num4 if plain_alt && !shift => Command::Workspace(4),
        Key::Num5 if plain_alt && !shift => Command::Workspace(5),
        Key::Num6 if plain_alt && !shift => Command::Workspace(6),
        Key::Num7 if plain_alt && !shift => Command::Workspace(7),
        Key::Num8 if plain_alt && !shift => Command::Workspace(8),
        Key::Num9 if plain_alt && !shift => Command::Workspace(9),
        Key::B if ctrl && !alt => Command::Up,
        Key::N if ctrl && shift => Command::InsertSection, // Ctrl+Alt+Shift+N: Web incognito-safe fallback
        Key::I if ctrl && shift => Command::Capture, // Ctrl+Alt+Shift+I: Web DevTools-safe fallback
        Key::E if ctrl && !alt && shift => Command::Review,
        _ => return None,
    };
    Some(command)
}

/// ワークスペースの中だけで効くキー（テキストの編集と重なるので、そのワークスペースでだけ取る）。
///
/// - `idea`: `Ctrl+Delete` で破棄（§10.4。編集欄の「後ろの単語を消す」より優先する）。
/// - `material`: `Ctrl+Delete` で却下（§16.1。`idea` と同じ理由で優先する、B-134）。
/// - `review`: `Ctrl+Shift+→` / `←` でタブ（§10.9.1。`review` には編集欄が無い）。
/// - `cast`: `Ctrl+Shift+→` / `←` でタブ（§10.6。中央の編集欄の単語選択より優先する）、
///   `Ctrl+Shift+V` で未一致の台詞（§10.6）。
/// - `agent`: `Ctrl+Shift+X` で総停止（§15.7.4）。
pub fn local_command_of(
    key: Key,
    modifiers: Modifiers,
    mac: bool,
    workspace: crate::ui::workspaces::Workspace,
) -> Option<Command> {
    use crate::ui::workspaces::Workspace;
    let ctrl = modifiers.ctrl || (modifiers.mac_cmd && mac);
    let alt = modifiers.alt;
    let shift = modifiers.shift;
    match (workspace, key) {
        (Workspace::Idea | Workspace::Material, Key::Delete) if ctrl && !alt && !shift => {
            Some(Command::Drop)
        }
        (Workspace::Review, Key::ArrowRight) if ctrl && shift && !alt => Some(Command::Tab(true)),
        (Workspace::Review, Key::ArrowLeft) if ctrl && shift && !alt => Some(Command::Tab(false)),
        (Workspace::Cast, Key::ArrowRight) if ctrl && shift && !alt => Some(Command::Tab(true)),
        (Workspace::Cast, Key::ArrowLeft) if ctrl && shift && !alt => Some(Command::Tab(false)),
        (Workspace::Cast, Key::V) if ctrl && shift && !alt => Some(Command::Scan),
        (Workspace::Agent, Key::X) if ctrl && shift && !alt => Some(Command::StopAll),
        (Workspace::PlotOutline | Workspace::Write, Key::OpenBracket) if ctrl && shift && !alt => {
            Some(Command::Fold(true))
        }
        (Workspace::PlotOutline | Workspace::Write, Key::CloseBracket) if ctrl && shift && !alt => {
            Some(Command::Fold(false))
        }
        _ => None,
    }
}

/// 1 フレームの入力処理の前半（§9.8.5 (3)）: IME の判定 → 変換中なら Key を全部捨てる →
/// ショートカットを解釈して、そのイベントを消費する。
pub fn take_commands(
    input: &mut InputState,
    ime: &mut ImeState,
    mac: bool,
    text_focused: bool,
) -> Vec<Command> {
    take_commands_in(
        input,
        ime,
        mac,
        text_focused,
        crate::ui::workspaces::Workspace::Write,
    )
}

/// [`take_commands`] のワークスペース付き（[`local_command_of`] を先に見る）。
pub fn take_commands_in(
    input: &mut InputState,
    ime: &mut ImeState,
    mac: bool,
    text_focused: bool,
    workspace: crate::ui::workspaces::Workspace,
) -> Vec<Command> {
    ime.update(input);
    if ime.composing() {
        // 変換候補の選択に Enter / Esc / Space / 矢印が使われる（§9.8.5 (3) MUST）
        input
            .events
            .retain(|event| !matches!(event, Event::Key { .. }));
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut shortcut_text = Vec::new();
    input.events.retain(|event| {
        let Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            return true;
        };
        let found = local_command_of(*key, *modifiers, mac, workspace)
            .or_else(|| command_of(*key, *modifiers, mac, text_focused));
        match found {
            Some(command) => {
                if (modifiers.alt || modifiers.ctrl || modifiers.command)
                    && key.name().chars().count() == 1
                {
                    shortcut_text.push(key.name());
                }
                out.push(command);
                false
            }
            None => true,
        }
    });
    // eframe can deliver a printable Text event alongside Alt+digit when no text
    // agent was focused (notably Reader -> an editable workspace). Consume the
    // associated character too, without dropping unrelated typing or IME commits.
    input.events.retain(|event|!matches!(event,Event::Text(text)if shortcut_text.iter().any(|key|text.eq_ignore_ascii_case(key))));
    // `?` は Text イベントとして来ることもある（キー配列による）
    if !text_focused && !out.contains(&Command::Help) {
        let before = input.events.len();
        input
            .events
            .retain(|event| !matches!(event, Event::Text(text) if text == "?"));
        if input.events.len() != before {
            out.push(Command::Help);
        }
    }
    out
}

/// Web でのキーの押され方（[`browser_default_blocked`] の入力。`KeyboardEvent` から作る）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BrowserKeys {
    /// `Ctrl` か `Cmd`。
    pub command: bool,
    /// `Alt`（macOS の `Option`）。
    pub alt: bool,
    /// `Shift`。
    pub shift: bool,
    /// `AltGr`（欧州の配列で `@` `{` などを打つ。字の入力なので止めない）。
    pub alt_graph: bool,
    /// macOS か。
    pub mac: bool,
}

/// Web で、このキーのブラウザの既定の動作（印刷・ページの保存・検索バー・戻る など）を止めるか
/// （novnow のショートカットに使うため。`PLANS.md` §9.2 B-158）。`key` は `KeyboardEvent.key`。
///
/// 止めないもの: コピー / 貼り付け / 切り取り（`Ctrl+C` / `V` / `X`。貼り付けはブラウザのイベントで
/// 受ける）・再読み込み（`Ctrl+R` / `F5`）・開発者ツール（`Ctrl+Shift+I` / `J`・`F12`）・全画面（`F11`）・
/// `AltGr` の字・修飾キーの無い字（打った字は入力欄へ）・macOS の `Option` の字（novnow は `Ctrl+Option`
/// で受ける）。ブラウザがページに渡さないもの（Chrome の `Ctrl+N` / `Ctrl+T` / `Ctrl+W`）は止められない。
pub fn browser_default_blocked(key: &str, keys: BrowserKeys) -> bool {
    if keys.alt_graph {
        return false;
    }
    let lower = key.to_ascii_lowercase();
    if keys.command {
        let browser = match lower.as_str() {
            "c" | "v" | "x" | "r" => !keys.shift && !keys.alt,
            "i" | "j" => keys.shift && !keys.alt,
            _ => false,
        };
        return !browser;
    }
    if keys.alt {
        // 戻る / 進む（`Alt+←/→`）とメニュー（`Alt+字`）を奪われないように、novnow の形（`Alt+数字`・
        // `Alt+矢印`・`Alt+H`）だけ止める
        let single =
            lower.chars().count() == 1 && lower.chars().all(|ch| ch.is_ascii_alphanumeric());
        return !keys.mac && (single || key.starts_with("Arrow"));
    }
    // F2（名前の変更、§13.4）・F3（次を検索）。F1（ヘルプ）・F7（キャレットブラウズ）も奪われないように
    matches!(key, "F1" | "F2" | "F3" | "F7")
}
