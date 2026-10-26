# novnow 設計書 — 概要・原則・処理モデル

本書は `REF:docs/specifications/overview.md` の各節に対応する実装設計を記述する。

## 1. 設計原則

### P1. 1 節が世界の単位である

状態: 実装完了（REF:src/core/id.rs, REF:src/core/features/sections.rs, REF:src/app/workbench.rs）

- 4 階層の構造 ID は `SceneId { chapter, episode, scene, section }`（`REF:src/core/id.rs`）で表す。
- 文書内の節の列挙とテキスト範囲の解決は `core::features::sections`（`REF:src/core/features/sections.rs`）が担い、UI の編集状態は `Workbench`（`REF:src/app/workbench.rs`）が「現在選択中の `SceneId` の本文スライス」として保持する。

### P2. ワーキングメモリを IDE が肩代わりする

状態: 実装完了（REF:src/core/resolve/context_rules.rs, REF:src/core/features/reference.rs, REF:src/ui/widgets/reference_pane.rs）

- 現在の節に対する参照ペインの 6 セクション（プロット / 上位プロット / 配役 / 用語 / 資料 / 診断）の優先順位付けと抽出は `core::resolve::context_rules`（`REF:src/core/resolve/context_rules.rs`）および `core::features::reference`（`REF:src/core/features/reference.rs`）が計算し、`REF:src/ui/widgets/reference_pane.rs` が描画する。
- 検証は `REF:tests/context_rules.rs` および `REF:tests/context_rules.rs` で行う。

### P3. キーボードで完結する

状態: 実装完了（REF:src/ui/shell.rs, REF:src/app/palette.rs, REF:src/ui/app.rs）

- キー入力から `Command` への変換は `ui::shell::take_commands_in`（`REF:src/ui/shell.rs`）に集約し、コマンドパレット・ジャンプパレット（`REF:src/app/palette.rs`）および `NovnowApp::run`（`REF:src/ui/app.rs`）で全機能へ到達できる。
- キー割当の網羅検証は `REF:tests/gui.rs` および `REF:tests/gui.rs` で行う。

### P4. 決定論

状態: 実装完了（REF:src/core/resolve/dictionary.rs, REF:src/core/aicontext/budget.rs, REF:src/app/palette.rs）

- `core` 層は `BTreeMap` / `BTreeSet` と安定したタイブレーク規則（定義順・文字長・バイトオフセット順）を用い、乱数やシステム時刻を一切参照しない（`REF:tests/layering.rs` で強制）。
- 曖昧検索スコアは `REF:src/app/palette.rs`、辞書最短一致は `REF:src/core/resolve/dictionary.rs`、AI コンテキスト切り詰めは `REF:src/core/aicontext/budget.rs` で決定論的に実装されている。

- **[Issue #29] 固定モデルの扱い（方針の明記）**: 仕様 P4 のとおり、部品は実行時に乱数を必要としないものに限る。将来、小さな学習済みモデル（分かち書きの精度向上など）を足す場合は、全員に同じ学習結果を配布する固定物としてのみ入れ、サイズ・依存搬入（`REF:docs/add-dependencies.md`）・決定論の検証をその都度行う。当面は決定論ルールの逐次追加を基本路線とする。

### P5. AI に何を任せるかは人間が決め、その範囲で AI は自律に動く

状態: 実装完了（REF:src/core/model/delegation.rs, REF:src/core/agents/runner.rs, REF:src/app/agents.rs）

- 委譲レベル `DelegationLevel`（`Off` / `Propose` / `Apply` / `Autonomous`）と役割別設定は `REF:src/core/model/delegation.rs` で定義する。
- タスク起票・自動連鎖上限・`state: fixed` 保護・監査ログ記録は `REF:src/core/agents/runner.rs` および `REF:src/core/agents/apply.rs` が純粋関数として判定し、`REF:src/app/agents.rs` が非同期実行を制御する（検証: `REF:tests/delegation.rs`）。
- 委譲モデルの設計不変条件（`REF:docs/history.md` の `B-6`〜`B-13`）:
  - 粒度は「全体既定＋役割ごと」のみとし、セッション一時上書きやタスク単位上書きは持たず `project.json` に永続化する。
  - 自律連鎖で次タスクを起票できるのは `integrator` のみとし、自動適用の検証ゲートは E 系診断の増加有無で判定する。
  - プロンプトの自動学習・自動書き換えは P5 に反するため実装しない（`REF:docs/history.md` の `F11`）。

### P6. プロジェクトは自己完結する

状態: 実装完了（REF:src/core/policy/write_policy.rs, REF:src/adapters/storage_fs.rs, REF:tests/storage_policy.rs）

- 書き込み先の検査は `WritePolicy`（`REF:src/core/policy/write_policy.rs`）が行い、実ファイルシステムへの書き込みシステムコールは `FsGate`（`REF:src/adapters/storage_fs.rs`）の 1 ファイルのみに閉じ込める（`REF:tests/layering.rs` の `only_the_write_gate_touches_the_file_system_for_writing` および `REF:tests/storage_policy.rs` で強制）。
- プロンプトや原稿の版管理機構（`.bak-*` 等）は一切持たず Git に委ねる（`REF:docs/history.md` の `U-7`, `F10`）。

### P7. ドメインは UI を知らない

状態: 実装完了（REF:src/core/mod.rs, REF:tests/layering.rs）

- `src/core/**` は `egui`・I/O・非同期ランタイム・時刻・乱数を一切 import しない。`REF:tests/layering.rs` が `syn` による AST 走査で機械的に検証する。
- 共同編集機能（`U-2`）およびモバイル・タッチ・ペン向け UI（`U-6`）は将来にわたり実装しない設計方針とする（`REF:docs/history.md` の `U-2`, `U-6`）。

## 2. 用語

### 2.1 用語定義

状態: 実装完了（REF:src/core/id.rs, REF:src/core/model/mod.rs, REF:src/core/resolve/mod.rs）

- 仕様の各用語に対応するドメイン型は `REF:src/core/id.rs`（`SceneId`, `PartialSceneId`, `RangeExpr`）、`REF:src/core/model/mod.rs`（`Project`, `Document`, `Character`, `Mode`, `RolePattern`, `Word`, `Material`, `Idea`, `Task`, `Delegation`）、`REF:src/core/resolve/dictionary.rs`（`Dictionary`, `Entry`）に 1 対 1 で定義されている。

## 3. アーキテクチャと処理モデル

### 3.1 層と責務の分離

状態: 実装完了（REF:src/lib.rs, REF:src/core/mod.rs, REF:src/ports/mod.rs, REF:src/adapters/mod.rs, REF:src/app/mod.rs, REF:src/ui/mod.rs）

依存の向きは `ui → app → adapters → ports ← core`（および CLI 合成根 `cli → {app, core, ports, adapters, ui::i18n}`）に固定する（`REF:src/lib.rs`）。段階 ③ の H1 で `app` 層（`REF:src/app/mod.rs`）を新設し、描画と無縁の状態機械（編集・保存・設定・エージェント実行・検索・パレット論理・コマンド・ワークスペース切替・ホスト）を `ui` から移した。`app` は常時ビルドされ、GUI のクレート（egui / eframe / rfd / fontdb / image）を import しない（`REF:tests/layering.rs` の `the_app_layer_does_not_use_the_gui_crates` で強制）。例外は文言カタログ `ui::i18n` のみ（§19.1.8）。

```
  ui/       egui 依存（ui feature 時）。描画・入力・レイアウト・i18n・テーマ
  cli/      ネイティブ CLI 合成の根（main.rs から呼ばれる）
  app/      描画と無縁の状態機械と外部インターフェースの機能核（常時ビルド。GUI クレート禁止）
  core/     純粋ドメイン。パース・解決・診断・コンテキスト・エクスポート変換
  ports/    Storage / Clock / McpClient / Env / Dialog の trait 定義
  adapters/ storage_fs, storage_idb, recent_file, zip, project_load, env_system, mcp_stdio, clock_system, dialog_rfd
```

`ui` は `app` の状態機械を表示するだけの表示層の 1 つであり、CLI・MCP・LSP・Python（§19）は同じ `app` を薄い適合層から呼ぶ（§19.1.4「単一の真実」）。`ui` 側には旧パスの再エクスポート（`pub use crate::app::{agents, assist, palette, save_machine, settings, workbench};` ほか）を残し、既存の参照と文書中のパスを壊さない。

### 3.2 ドメイン純粋性と外部隔離規則

状態: 実装完了（REF:Cargo.toml, REF:tests/layering.rs）

`src/core/**` が使用してよい外部クレートは以下の許可リスト（`REF:Cargo.toml`）に限定する。

| 許可クレート | 用途 |
|---|---|
| `serde` / `serde_json` | 設定・索引・ゴールデンテストのシリアライズ |
| `toml` | カタログ・設定の解析 |
| `regex` | 役割語マッチャ・字句解析 |
| `aho-corasick` | 本文中の用語出現検出 |
| `ropey` | Rope テキストバッファ（`REF:src/core/text/rope.rs`） |
| `anyhow` / `thiserror` | エラー型定義 |
| `unicode-normalization` | NFKC 正規化照合（`REF:src/core/resolve/dictionary.rs`） |
| `tiniestsegmenter` | 未知の `'` 参照の形態素境界（`REF:src/core/resolve/dictionary.rs`、§6.2.6 / S-09） |

`[dev-dependencies]` には AST 検査用の `syn`（`REF:tests/layering.rs`）とベンチマーク用の `criterion`（`REF:benches/core.rs`）のみを許可する。`core/` 内での `#[cfg(...)]` 条件コンパイルおよび `unsafe` は全面禁止とし、`REF:tests/layering.rs` で常時検査する。

### 3.3 機能構成

状態: 実装完了（REF:src/lib.rs, REF:src/main.rs, REF:src/cli/mod.rs, REF:src/core/mod.rs, REF:src/ports/mod.rs, REF:src/adapters/mod.rs, REF:src/app/mod.rs, REF:src/ui/mod.rs）

ソースコードのモジュール配置は以下のとおりである。

- `REF:src/main.rs`: エントリポイント（Windows の `AttachConsole` と CLI / Web 起動分岐）。
- `REF:src/cli/mod.rs`, `REF:src/cli/open.rs`, `REF:src/cli/new.rs`, `REF:src/cli/check.rs`, `REF:src/cli/export.rs`, `REF:src/cli/mcp.rs`: CLI サブコマンド群。
- `REF:src/core/id.rs`: `SceneId` / `PartialSceneId` / `RangeExpr`。
- `REF:src/core/text/mod.rs`: `rope.rs`, `position.rs`, `lexer.rs`, `incremental.rs`, `sha256.rs`。
- `REF:src/core/syntax/mod.rs`: `contents.rs`, `plot_outline.rs`, `plot_episode.rs`, `characters_md.rs`, `words_md.rs`, `ideas_md.rs`, `materials_index.rs`, `prompt_md.rs`, `project_json.rs`。
- `REF:src/core/model/mod.rs`: ドメインモデル型群。
- `REF:src/core/resolve/mod.rs`: `dictionary.rs`, `modes.rs`, `role_language.rs`, `materials.rs`, `context_rules.rs`。
- `REF:src/core/diagnostics.rs`: 76 診断コードの定義とメタデータ表。
- `REF:src/core/features/mod.rs`: ワークスペース補助（`project.rs`, `sections.rs`, `reference.rs`, `references.rs`, `place.rs`, `plots.rs`, `ideas.rs`, `cast.rs`, `materials_ops.rs`）と LSP の言語機能（§3.4 の 12 機能のうち文書診断は `REF:src/core/diagnostics.rs` が担い、残る 11 機能を `core/features` が持つ）: `completion.rs`, `definition.rs`, `references.rs`, `document_symbol.rs`, `folding.rs`, `formatting.rs`, `hover.rs`, `inlay_hint.rs`, `semantic_tokens.rs`, `code_action.rs`, `rename.rs`）。
- `REF:src/core/policy/mod.rs`: `write_policy.rs`, `recent.rs`, `archive.rs`, `external_change.rs`。
- `REF:src/core/export.rs`, `REF:src/core/template/mod.rs`, `REF:src/core/aicontext/mod.rs`, `REF:src/core/agents/mod.rs`。
- `REF:src/ports/mod.rs` & `REF:src/adapters/mod.rs`: ポートと各環境アダプタ。
- `REF:src/app/mod.rs`: 描画と無縁の状態機械（`workbench.rs`, `save_machine.rs`, `settings.rs`, `palette.rs`, `assist.rs`, `agents.rs`, `command.rs`, `commands.rs`, `host.rs`, `workspace.rs`）と外部インターフェースの機能核（§19: `error.rs`, `event.rs`, `session.rs`, `memory_host.rs`, `fs_host.rs`, `driver.rs` と `ops/` の 119 操作）。
- `REF:src/ui/mod.rs`: `i18n/`, `app.rs`, `web.rs`, `shell.rs`, `theme.rs`, `widgets/`, `workspaces/`（`app` 層の 6 モジュールは再エクスポートで旧パスからも解決する）。

段階 ② の追加仕様を無改修で受け入れるための 10 個の拡張点（縫い代）を維持する:
1. JSON 設定（`project.json` / `ui.json` / `index.json`）の未知キー温存（`#[serde(flatten)]`）。
2. A/B/MD 形式の未知フィールド行の温存（`FieldKey::Unknown`）。
3. 診断コード番号の予約領域の保護（`REF:src/core/diagnostics.rs`）。
4. 行頭シジル判定の配列駆動（`SIGIL_TABLE`、`REF:src/core/text/lexer.rs`）。
5. ワークスペース状態の常時保持（`REF:src/ui/app.rs`）。
6. 参照ペインのセクション配列化（`REF:src/core/features/reference.rs`）。
7. 資料 ID 接頭辞の 1 文字抽象化（`MaterialId`、`REF:src/core/model/material.rs`）。
8. 診断の実行タイミング・スコープのテーブル定義（`COST`、`REF:src/core/diagnostics.rs`）。
9. 文字数・原稿用紙換算の単一関数集約（`REF:src/core/features/project.rs`）。
10. エージェント組み込みロールの配列駆動（`BUILTIN_ROLES`、`REF:src/core/syntax/prompt_md.rs`）。

### 3.4 言語機能サービス（LSP 相当機能）

状態: 実装完了（REF:src/core/features/mod.rs, REF:src/ui/app.rs, REF:tests/layering.rs）

11 個の LSP 相当モジュールはすべて `src/core/features/` に純粋関数として実装され、`src/ui/` から直接呼び出されている（`REF:tests/layering.rs` の `ui_calls_all_eleven_lsp_feature_modules` で強制）。

| モジュール | 実装ファイル | 主な UI 呼び出し元 |
|---|---|---|
| `semantic_tokens` | `REF:src/core/features/semantic_tokens.rs` | `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/context_view.rs` |
| `completion` | `REF:src/core/features/completion.rs` | `REF:src/app/assist.rs`, `REF:src/ui/app.rs` |
| `hover` | `REF:src/core/features/hover.rs` | `REF:src/ui/widgets/reference_pane.rs`, `REF:src/ui/app.rs` |
| `inlay_hint` | `REF:src/core/features/inlay_hint.rs` | `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/context_view.rs` |
| `diagnostics` | `REF:src/core/features/project.rs` | `REF:src/ui/app.rs`, `REF:src/ui/widgets/reference_pane.rs` |
| `code_action` | `REF:src/core/features/code_action.rs` | `REF:src/app/assist.rs`, `REF:src/ui/app.rs` |
| `definition` | `REF:src/core/features/definition.rs` | `REF:src/ui/app.rs`（`F12` / `'` パレット `Enter`） |
| `references` | `REF:src/core/features/references.rs` | `REF:src/ui/app.rs`（`Shift+F12` / `'` パレット `Shift+Enter`） |
| `document_symbol` | `REF:src/core/features/document_symbol.rs` | `REF:src/app/assist.rs`, `REF:src/ui/app.rs` |
| `formatting` | `REF:src/core/features/formatting.rs` | `REF:src/ui/app.rs`（`>format`） |
| `rename` | `REF:src/core/features/rename.rs` | `REF:src/ui/app.rs`（`F2` / `>rename`） |
| `folding` | `REF:src/core/features/folding.rs` | `REF:src/ui/workspaces/plot_outline.rs`, `REF:src/ui/app.rs` |

- **[Issue #29 / B5] 参照ペイン行への移動先の付与（実装完了: `REF:docs/plans.md#issue-29-b5`）**:
  - `REF:src/core/features/reference.rs` の行モデルへ、表示文字列とは別に移動先 `Nav` を持たせた。GUI（§9.5.5）とヘッドレス（`reference.pane`、`REF:src/app/ops/knowledge.rs`）は同じ値を共有する。

### 3.5 状態フロー（1 キーストロークの流れ）

状態: 実装完了（REF:src/core/text/incremental.rs, REF:src/app/workbench.rs, REF:src/app/save_machine.rs, REF:benches/core.rs）

- 打鍵時は `LineIndex::relex_line`（`REF:src/core/text/incremental.rs`）が編集行のみを再字句解析し、構造見出し `@` または節区切り `;` に変化があった場合のみ `Renumber::From(line)` を返す。
- `Workbench::edited`（`REF:src/app/workbench.rs`）が `SaveMachine`（`REF:src/app/save_machine.rs`）を `Dirty` に遷移させ、参照ペインキャッシュ（`REF:src/ui/widgets/reference_pane.rs`）は 80ms デバウンス後に再構築する。
- 1 行再字句解析と参照ペイン再構築の性能は `REF:benches/core.rs` で計測する。
