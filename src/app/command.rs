//! ショートカット・パレットが実行する**コマンド**（§9.8）。**egui に依存しない**。
//!
//! キーの解釈（`ui::shell::command_of`）とパレットの一覧（`ui::app`）の両方がこの列挙を
//! 共有する。ヘッドレスのセッション（§19.2.3 の `session.command`）も同じ表を使うので、
//! GUI 専用のビルドでなくてもコマンド名から操作に到達できる。

/// ショートカットで起きること（§9.8.1〜§9.8.3 のうち M7 で動くもの）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `Ctrl+P` コマンドパレット。
    Palette,
    HumanProposal,
    StatsMode,
    SuggestComma,
    InsertComma,
    CarryWorkspace(u8),
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
    Nav(crate::app::workbench::Nav),
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
    /// `Ctrl+.` クイックフィックス（カーソル位置の診断）。
    QuickFix,
    /// `Ctrl+T` / `Ctrl+Shift+T` 選択文字列を用語 / キャラとして登録。
    Register {
        character: bool,
    },
    /// `Ctrl+Shift+D` 外部変更との差分モーダル（衝突時のみ）。
    Diff,
    /// `>export` テキストのエクスポート（§17.2。ショートカットは無く、パレットから）。
    Export,
    /// `Alt+1..9` ワークスペースの切替（§10.0。番号は §10 の表の `#`）。
    Workspace(u8),
    /// `Alt+0` 直前のワークスペースへ（§10.0）。
    PreviousWorkspace,
    /// `Ctrl+B` / `Ctrl+Shift+B` 4 段階を 1 つ上へ（write → plot-episode → plot-outline の章 → 全体。
    /// §10.1・§10.3、`PLANS.md` §9.2 B-126）。
    Up,
    /// `Ctrl+Shift+N` 話プロットに節を挿入（§10.2）。
    InsertSection,
    /// `Ctrl+Shift+I` アイデアの即時入力（§10.4。どのワークスペースからでも、画面遷移なし）。
    Capture,
    /// `Ctrl+Shift+E` 点検（review）の診断へ（§9.6.1・§10.9.2）。
    Review,
    /// `Ctrl+Delete` アイデアを破棄（`status: dropped`。§10.4）・資料を却下（§16.1）。`idea` と
    /// `material` の中だけ。
    Drop,
    /// `Ctrl+Shift+→` / `Ctrl+Shift+←` タブの切替（§10.9.1・§10.6。`review` と `cast` の中だけ）。
    Tab(bool),
    /// `Ctrl+Shift+V` 役割語が一致しなかった台詞の一覧（§10.6。`cast` の中だけ）。
    Scan,
    /// `Ctrl+M` 資料検索（§16.3・§9.8.3。どのワークスペースからでも。`PLANS.md` §9.2 B-134）。
    MaterialSearch,
    /// `Alt+H` インレイヒントの表示トグル（§6.6.2・§9.8.1。`ui.json.inlay_hints`、B-137）。
    InlayHints,
    /// `Ctrl+Shift+X` エージェントの総停止（§15.7.4。`agent` の中だけ）。
    StopAll,
    /// `>format` 今のファイルの正規化（§5.12）。
    Format,
    /// `F2` / `>rename` キャラ名・用語の一括改名（§3.4）。
    Rename,
    /// `F12` / `>definition` 定義へ移動（§3.4）。
    Definition,
    /// `Shift+F12` / `>references` 使用箇所の一覧（§3.4）。
    References,
    /// `Ctrl+Shift+[` / `Ctrl+Shift+]` 構成ツリーの章→話展開・折り畳み（§10.3.1。`plot-outline` の中だけ）。
    Fold(bool),
}
