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

/// ショートカットで起きること（§9.8.1〜§9.8.3 のうち M7 で動くもの）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `Ctrl+P` コマンドパレット。
    Palette,
    /// `Ctrl+G` SceneId ジャンプ。
    Jump,
    /// `Ctrl+O` プロジェクトを開く。
    Open,
    /// `Ctrl+S` 今すぐ全部書き出す。
    SaveNow,
    /// `F5` プロジェクト再読込。
    Reload,
    /// `Ctrl+Plus` 文字サイズ拡大。
    FontUp,
    /// `Ctrl+Minus` 文字サイズ縮小。
    FontDown,
    /// `Ctrl+0` 文字サイズ既定。
    FontReset,
    /// `Ctrl+1` 文脈ビューへフォーカス。
    FocusContext,
    /// `Ctrl+2` 節エディタへフォーカス。
    FocusEditor,
    /// `Ctrl+3` 参照ペインへフォーカス。
    FocusReference,
    /// `Ctrl+\` 参照ペインの表示トグル。
    ToggleReference,
    /// `Ctrl+Shift+\` 参照ペインの左右入替。
    SwapReference,
    /// `Ctrl+Alt+L` 言語トグル。
    ToggleLanguage,
    /// `Ctrl+Alt+T` テーマ切替。
    CycleTheme,
    /// `?`（入力欄外）キーボードヘルプ。
    Help,
    /// `Esc` モーダルを閉じる / 節エディタへ。
    Escape,
    /// 構造移動（§9.8.2）。
    Nav(crate::ui::workbench::Nav),
    /// `Ctrl+Enter` 確定して次の節へ。
    ConfirmNext,
    /// `Ctrl+;` 直後に新しい節。
    NewSection,
    /// `Ctrl+Alt+Shift+←/→` メイン/参照の比（§9.1。`PLANS.md` §9.2 B-116）。
    MainRatio(bool),
    /// `Ctrl+Alt+↑/↓` 文脈ビュー/節エディタの比（§9.1）。
    ContextRatio(bool),
    /// `Ctrl+F` 検索（今のファイル）。
    Find,
    /// `Ctrl+H` 置換（今のファイル）。
    Replace,
    /// `F3` / `Shift+F3` 次 / 前の一致。
    FindNext(bool),
    /// `Ctrl+Shift+F` プロジェクト全体検索。
    Search,
    /// `Ctrl+Space` 補完。
    Complete,
    /// `Ctrl+Shift+M` 全モード一覧。
    Modes,
    /// `Ctrl+Shift+W` 用語の全文。
    Term,
    /// `Ctrl+Shift+;` 今の節を削除（確認あり）。
    DeleteSection,
    /// `Ctrl+Tab` / `Ctrl+Shift+Tab` 最近開いたファイルを循環。
    RecentFile(bool),
}

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
        Key::T if ctrl && alt => Command::CycleTheme,
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
        Key::F3 if !ctrl && !alt => Command::FindNext(!shift),
        Key::Space if ctrl && !alt && !shift => Command::Complete,
        Key::M if ctrl && !alt && shift => Command::Modes,
        Key::W if ctrl && !alt && shift => Command::Term,
        Key::Tab if ctrl && !alt => Command::RecentFile(!shift),
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
        _ => return None,
    };
    Some(command)
}

/// 1 フレームの入力処理の前半（§9.8.5 (3)）: IME の判定 → 変換中なら Key を全部捨てる →
/// ショートカットを解釈して、そのイベントを消費する。
pub fn take_commands(
    input: &mut InputState,
    ime: &mut ImeState,
    mac: bool,
    text_focused: bool,
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
        match command_of(*key, *modifiers, mac, text_focused) {
            Some(command) => {
                out.push(command);
                false
            }
            None => true,
        }
    });
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
