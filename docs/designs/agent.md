# novnow 設計書 — AI エージェント

本書は `REF:docs/specifications/agent.md` の各節に対応する実装設計を記述する。

## 15. AI エージェント

### 15.1 原則

状態: 実装完了（REF:src/core/model/delegation.rs, REF:src/core/agents/runner.rs, REF:src/core/agents/apply.rs, REF:tests/delegation.rs）

- 4 段階の委譲レベル（`off`, `propose`, `apply`, `autonomous`）と安全装置 A1〜A8（自動適用前の E 系診断検証ゲート、直前スナップショット `P####.diff` 保存、`state: fixed` 書き換え拒否と通知、連鎖上限・文字数上限・総停止）は `REF:src/core/model/delegation.rs`, `REF:src/core/agents/apply.rs`, `REF:src/core/agents/runner.rs` に実装する（検証: `REF:tests/delegation.rs`）。
- 自動適用ゲート（`apply::validate`）は対象ファイルを仮想的に書き換えた上で E 系診断件数が適用前より増加しないことを必須条件とし、1 件でも増加する場合は自動適用を拒否して `awaiting_review`（`E035`）へフォールバックさせる（`B-10`, `B-91`）。

- **[Issue #29] エージェント機能の凍結（方針の明記）**: 仕様 §15.1 の A9 のとおり、エージェント機能は現行の §15 の範囲で凍結する。以後の作業は不具合修正と形式・診断変更への追随に限る。

### 15.2 役割（6 + 統合）

状態: 実装完了（REF:src/core/syntax/prompt_md.rs, REF:src/core/agents/runner.rs, REF:tests/agents_files.rs）

- 組み込み 7 役割（`writer`, `plotter`, `ideator`, `researcher`, `stylist`, `logician`, `integrator`）は `BUILTIN_ROLES`（`REF:src/core/syntax/prompt_md.rs`）に配列として定義し（縫い代 10）、役割ごとの既定委譲レベルと書き込み可能スコープ検査を `REF:src/core/agents/runner.rs` が担う（範囲外のファイル変更を含む提案は `E034` で拒否する、`B-90`）。

### 15.3 タスクモデル

状態: 実装完了（REF:src/core/model/task.rs, REF:src/core/agents/files.rs, REF:src/core/agents/runner.rs, REF:tests/agents_files.rs）

- タスクの状態遷移（`queued`, `running`, `awaiting_review`, `applied`, `rejected`, `failed`, `cancelled`）と `T####.md` / `T####.result.md` / `P####.diff` の生成・更新は `REF:src/core/model/task.rs`, `REF:src/core/agents/files.rs`, `REF:src/core/agents/runner.rs` に実装する（`B-88`, `B-89`）。

### 15.4 コンテキスト設計

状態: 実装完了（REF:src/core/aicontext/packet.rs, REF:src/core/aicontext/budget.rs, REF:tests/aicontext.rs, REF:docs/plans.md#plan-S-01）

- 固定 9 セクション（`1 project` 〜 `9 history`）の組み立ては `core::aicontext::packet`（`REF:src/core/aicontext/packet.rs`）が `core::resolve::context_rules` と同じ優先順位規則を用いて生成する。
- 役割別のトークン予算配分と優先順位に基づく決定論的切り詰め（トークン数概算は CJK 1 文字 ≒ 1.5 トークン、ASCII 4 文字 ≒ 1 トークンの純粋関数で行い外部トークナイザに依存しない、`B-87`）は `core::aicontext::budget`（`REF:src/core/aicontext/budget.rs`）が担う（検証: `REF:tests/aicontext.rs`）。

- **[S-01] 指定した話の本文・プロットを「例」としてコンテキストに付与（実装完了: `REF:src/core/aicontext/packet.rs`, `REF:tests/aicontext.rs`）**:
  - `REF:src/core/aicontext/packet.rs` と `REF:src/core/aicontext/budget.rs` で、タスクに指定された `examples` 範囲の本文またはプロットを抽出し、コンテキストパケットおよび `{{examples}}` 変数へ組み込む。

### 15.5 プロンプト設計

状態: 実装完了（REF:src/core/syntax/prompt_md.rs, REF:src/core/agents/prompts.rs, REF:src/core/template/mod.rs, REF:tests/prompt_md.rs, REF:tests/template.rs）

- 既定プロンプトの埋め込み（`REF:src/core/template/mod.rs`）、欠落ファイルの自動補完と `I050` 通知、`{{include:...}}` 展開、閉じた 20 変数の展開、プロンプト診断（`E029` / `W015`）および `>prompt reset` / `>prompt diff` は `REF:src/core/agents/prompts.rs` と `REF:src/ui/app.rs` に実装する。

### 15.6 エージェント間通信

状態: 実装完了（REF:src/core/agents/files.rs, REF:src/core/agents/runner.rs, REF:tests/agents_files.rs, REF:tests/delegation.rs）

- スレッドファイル `agents/threads/TH####.md` の読み書きと、受信側ロールの委譲レベルに応じた承認待ち（`pending`）/ 自動中継（`approved_by: auto`）および連鎖深度 `max_chain_depth` の制御は `REF:src/core/agents/files.rs` と `REF:src/core/agents/runner.rs` に実装する。

### 15.7 人間向け管制 UI（`agent` ワークスペース）

状態: 実装完了（REF:src/ui/workspaces/agent.rs, REF:src/app/agents.rs, REF:src/ui/app.rs, REF:tests/workspaces.rs, REF:tests/gui.rs）

- `agent` ワークスペース（`Alt+8`）の 4 ペイン描画、タスク起票・ドライラン・提案の節単位採否（`A` / `X` / `E`）・差し戻し（`U`）・役割別委譲レベル変更（`1`〜`4`）・総停止（`Ctrl+Shift+X`）は `REF:src/ui/workspaces/agent.rs` と `REF:src/app/agents.rs` に実装する。

- **[F-19 / S-01] `mcp.json` 編集モーダルへの移行と起票時の「例」指定 UI（実装完了: `REF:docs/plans.md#plan-F-19`, `REF:docs/plans.md#plan-S-01`）**:
  - `REF:src/ui/workspaces/agent.rs` と `REF:src/ui/app.rs` に `agents/mcp.json` 編集モーダル呼び出し（F-19）およびタスク起票時の参考例範囲入力欄（S-01）を追加する。

### 15.8 MCP

状態: 実装完了（REF:src/ports/mcp.rs, REF:src/adapters/mcp_stdio.rs, REF:src/core/agents/server.rs, REF:src/cli/mcp.rs, REF:tests/mcp_stdio.rs, REF:tests/no_llm_sdk.rs）

- **MCP クライアント**: `ports::mcp::McpClient`（`REF:src/ports/mcp.rs`）と `std::process` + スレッドによる JSON-RPC 2.0 stdio 実装 `adapters::mcp_stdio`（`REF:src/adapters/mcp_stdio.rs`）が外部 MCP サーバと通信する。
- **MCP サーバ（`novnow mcp`）**: 11 個のツール（読み取り 7 個 + 提案 2 個 + 直接書込 2 個）のディスパッチは `core::agents::server`（`REF:src/core/agents/server.rs`）が純粋に処理し、`REF:src/cli/mcp.rs` が stdio 入出力を担う（検証: `REF:tests/mcp_stdio.rs`）。
- LLM SDK 非依存の強制は `REF:tests/no_llm_sdk.rs` で検査する。

### 15.9 失敗モードと安全装置

状態: 実装完了（REF:src/core/agents/output.rs, REF:src/core/agents/apply.rs, REF:src/core/agents/runner.rs, REF:tests/agents_files.rs, REF:tests/delegation.rs）

- 出力契約違反の検出（`REF:src/core/agents/output.rs`）、自動適用前の構文・参照検証ゲート（`REF:src/core/agents/apply.rs`）、文字数上限 `max_chars_per_task` による切り詰め、ループ遮断は `REF:src/core/agents/runner.rs` に実装する。

## 付録 A: 既定プロンプトの要件

### A.1 `_shared/output-contract.md`（全役割が参照）

状態: 実装完了（REF:src/core/template/mod.rs, REF:tests/template.rs）

- 全ロール共通の出力契約テンプレートは `REF:src/core/template/mod.rs` に収録し、`REF:tests/template.rs` で必須要素を検証する。

### A.2 `writer.md`

状態: 実装完了（REF:src/core/template/mod.rs, REF:tests/template.rs）

- 本文執筆ロールの既定プロンプトを `REF:src/core/template/mod.rs` に収録し、`REF:tests/template.rs` で検証する。

### A.3 `plotter.md`

状態: 実装完了（REF:src/core/template/mod.rs, REF:tests/template.rs）

- プロット構築ロールの既定プロンプトを `REF:src/core/template/mod.rs` に収録し、`REF:tests/template.rs` で検証する。

### A.4 `ideator.md`

状態: 実装完了（REF:src/core/template/mod.rs, REF:tests/template.rs）

- アイデア発想ロールの既定プロンプトを `REF:src/core/template/mod.rs` に収録し、`REF:tests/template.rs` で検証する。

### A.5 `researcher.md`

状態: 実装完了（REF:src/core/template/mod.rs, REF:tests/template.rs）

- 資料調査ロールの既定プロンプトを `REF:src/core/template/mod.rs` に収録し、`REF:tests/template.rs` で検証する。

### A.6 `stylist.md`

状態: 実装完了（REF:src/core/template/mod.rs, REF:tests/template.rs）

- 文体・推敲ロールの既定プロンプトを `REF:src/core/template/mod.rs` に収録し、`REF:tests/template.rs` で検証する。

### A.7 `logician.md`

状態: 実装完了（REF:src/core/template/mod.rs, REF:tests/template.rs）

- 整合性検証ロールの既定プロンプトを `REF:src/core/template/mod.rs` に収録し、`REF:tests/template.rs` で検証する。

### A.8 `integrator.md`（任意）

状態: 実装完了（REF:src/core/template/mod.rs, REF:tests/template.rs）

- 統合・次タスク起票ロールの既定プロンプトを `REF:src/core/template/mod.rs` に収録し、`REF:tests/template.rs` で検証する。
