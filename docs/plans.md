# 開発計画（Plans）

本書は現在の開発状況サマリと段階 ②（追加仕様 27 項目）の実装計画を記述する。完了済みの段階 ① マイルストーン（M0〜M11）の詳細履歴と過去の設計判断ログ（D-001〜D-015）は `REF:docs/history.md` に移動済みである。

---

## 1. 段階 ①〜② の全体進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 123 節 / 実装中 0 節 / 実装待ち 2 節 | 全 10 設計書（`docs/designs/*.md`）の `### ` 節（合計 125 節） | `REF:docs/designs/ui.md`, `REF:docs/designs/workflows.md` |
| 段階 ②（追加仕様 1〜27） | 未着手（計画待機） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
<!-- SYNC:END stage1-status -->

---

<a id="stage-2"></a>
## 2. 段階 ② 実装計画（追加仕様 1〜27）

段階 ① で整備した 10 個の拡張点（縫い代、`REF:docs/designs/overview.md` §3.3）を土台として、以下の 27 項目を段階 ② で実装する。

### 2.1 文脈・世界観・プロット拡張（追加仕様 1〜8）

| 番号 | 項目 | 主な変更対象 | 関連する診断・縫い代 |
|---|---|---|---|
| 1 | 物語内時計・タイムライン（`when:` フィールドとタイムラインタブ） | `REF:src/core/syntax/plot_episode.rs`, `REF:src/ui/workspaces/plot_outline.rs` | `E031`, `W088`, `W089`（縫い代 2, 3, 8） |
| 2 | キャラの節単位状態スナップショット（`cast-state:`） | `REF:src/core/syntax/plot_episode.rs`, `REF:src/ui/widgets/reference_pane.rs` | `W090`（縫い代 2, 6） |
| 3 | 読者/キャラの知識境界と劇的アイロニー（`knows:`） | `REF:src/core/syntax/plot_episode.rs`, `REF:src/core/features/project.rs` | `W091`（縫い代 2, 3） |
| 4 | 伏線レジストリの一次市民化（`# F####` と回収率マトリクス） | `REF:src/core/syntax/plot_episode.rs`, `REF:src/ui/workspaces/review.rs` | `W092`, `I053`（縫い代 2, 3） |
| 5 | 五感パレット（`senses:` と参照ペイン五感セクション） | `REF:src/core/features/reference.rs`, `REF:src/ui/widgets/reference_pane.rs` | `W093`（縫い代 2, 6） |
| 6 | 場面の空間・小道具スケッチ（`space:` と参照ペイン空間セクション） | `REF:src/core/features/reference.rs`, `REF:src/ui/widgets/reference_pane.rs` | 縫い代 2, 6 |
| 7 | 挿絵・ビジュアル資料（`S####` と参照ペイン挿絵セクション） | `REF:src/core/model/material.rs`, `REF:src/ui/widgets/reference_pane.rs` | 縫い代 6, 7 |
| 8 | 声紋カード・台詞比率メーター（文長ヒストグラム・一人称検査） | `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/workspaces/cast.rs` | `W094`, `W095`（縫い代 3, 8） |

### 2.2 エディタ・執筆体験拡張（追加仕様 9〜16）

| 番号 | 項目 | 主な変更対象 | 関連する診断・縫い代 |
|---|---|---|---|
| 9 | 文脈ビューの粒度切替（全文 ↔ 1 行要約 ↔ 見出しのみ、`Alt+-` / `Alt+=`） | `REF:src/ui/widgets/context_view.rs`, `REF:src/ui/shell.rs` | `REF:docs/designs/ui.md` §9.3 |
| 10 | 節内ブランチ（別案テイク `=== take: <name> ===` と `Alt+[` / `Alt+]`） | `REF:src/core/text/lexer.rs`, `REF:src/core/syntax/contents.rs` | `I002`（縫い代 4） |
| 11 | インライン下書きメモ（`??` / `？？` と未解決メモ集計） | `REF:src/core/text/lexer.rs`, `REF:src/core/export.rs` | `W080`（縫い代 3, 4） |
| 12 | ゴーストテキスト（プロット要点の薄字オーバーレイ、`Alt+G`） | `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/shell.rs` | `REF:docs/designs/ui.md` §9.4 |
| 13 | タイプライタスクロールとフォーカス調光（`Ctrl+Alt+Z`） | `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/settings.rs` | 縫い代 1 |
| 14 | 節の結合・移動・章話リナンバ（`Ctrl+Shift+M` 等） | `REF:src/core/features/sections.rs`, `REF:src/ui/workspaces/plot_outline.rs` | `REF:docs/designs/ui.md` §10.3 |
| 15 | 原稿用紙換算・セッション統計・締め切りペース表示 | `REF:src/core/features/project.rs`, `REF:src/ui/app.rs` | 縫い代 1, 9 |
| 16 | スナップショット（マイルストーン凍結 `.novnow/snapshots/`） | `REF:src/core/policy/write_policy.rs`, `REF:src/ui/app.rs` | `REF:docs/designs/editing.md` §8.6 |

### 2.3 日本語組版・文体点検拡張（追加仕様 17〜21）

| 番号 | 項目 | 主な変更対象 | 関連する診断・縫い代 |
|---|---|---|---|
| 17 | リズム・文体ヒートマップ（`review` の「文体」タブ） | `REF:src/core/features/project.rs`, `REF:src/ui/workspaces/review.rs` | `W081`, `W082`, `W085`, `W086`, `W087` |
| 18 | 視点（POV）漏れ検査の精密化 | `REF:src/core/features/project.rs` | `W084` |
| 19 | 表記揺れ・誤用辞書・組版規則の拡張 | `REF:src/core/features/project.rs`, `REF:src/core/model/project.rs` | `W096`, `W097`, `W098`, `W099`（縫い代 1, 3, 8） |
| 20 | ルビ一貫性ポリシー（`first_in_chapter` / `first_in_work` 等） | `REF:src/core/export.rs`, `REF:src/core/features/project.rs` | `I001`（縫い代 1） |
| 21 | 台詞話者の逆引き・無標識会話の検出強化 | `REF:src/core/features/project.rs` | `W083` |

### 2.4 AI エージェント・出力拡張（追加仕様 22〜27）

| 番号 | 項目 | 主な変更対象 | 関連する診断・縫い代 |
|---|---|---|---|
| 22 | 選択範囲インライン指示（`Ctrl+K` と `P####.diff` 即時プレビュー） | `REF:src/ui/app.rs`, `REF:src/ui/agents.rs` | `REF:docs/designs/agent.md` §15.3 |
| 23 | ロールプレイ対話モード（キャラ憑依インタビュー `@char`） | `REF:src/core/agents/runner.rs`, `REF:src/ui/workspaces/agent.rs` | `REF:docs/designs/agent.md` §15.6 |
| 24 | 7 番目の専門ロール `continuity`（設定考証・時系列監査） | `REF:src/core/syntax/prompt_md.rs`, `REF:src/core/template/mod.rs` | 縫い代 10 |
| 25 | コンテキスト予算の可視化とピン留め | `REF:src/core/aicontext/budget.rs`, `REF:src/ui/workspaces/agent.rs` | `REF:docs/designs/agent.md` §15.4 |
| 26 | 投稿サイト別エクスポートプリセット（`kakuyomu` / `narou` / `aozora`） | `REF:src/core/export.rs`, `REF:src/cli/export.rs`, `REF:src/ui/app.rs` | `REF:docs/designs/workflows.md` §17.1 |
| 27 | 縦書き・ページ組版プレビュー（`reader` ワークスペース、`Alt+7`） | `REF:src/ui/workspaces/mod.rs`, `REF:src/ui/app.rs` | `REF:docs/designs/ui.md` §10.7, `REF:docs/designs/workflows.md` §17.3 |
