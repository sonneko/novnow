# novnow 設計書 — ファイル形式（構文仕様）

本書は `REF:docs/specifications/syntax.md` の各節に対応する実装設計を記述する。

## 5. ファイル形式（構文仕様）

### 5.0 共通字句規則

状態: 実装完了（REF:src/core/text/lexer.rs, REF:tests/lexer.rs）

- 行頭シジル判定（`SIGIL_TABLE`）・7 種のエスケープ解除・フィールド行の 7 段判定・継続行インデント判定（2 空白以上または先頭タブ 1 文字以上）・BOM/CRLF 正規化は `core::text::lexer`（`REF:src/core/text/lexer.rs`）に実装する。
- 日本語フィールドキー別名表は `/// i18n: format-vocabulary` を付与した定数配列として保持し、URI スキームとの非交差と合わせて `REF:tests/lexer.rs` で検証する。

### 5.1 SceneId と範囲式

状態: 実装完了（REF:src/core/id.rs, REF:tests/id_range.rs）

- `SceneId`・`PartialSceneId`・`RangeTerm`・`RangeExpr` のパース、階層比較、閉区間判定（`contains`）、逆転範囲や予約番号 `00` の検出（`E016` / `E020` / `E022`）は `REF:src/core/id.rs` に実装し、`REF:tests/id_range.rs` で検証する。

### 5.2 本文形式（C 形式）— `contents/AA-BB.txt`

状態: 実装中（REF:src/core/syntax/contents.rs, REF:tests/contents.rs, REF:tests/golden.rs, REF:docs/plans.md#plan-S-08）

- 場面見出し `@A-B-C[-D]`、節区切り `;` による節番号 `D` の自動採番、空節の `W012`、行末話者注釈 `@char[:mode]`、会話スパン `「」` / `『』`、ルビ 2 形式、コメント範囲の解析と無損失 `to_text()` は `core::syntax::contents`（`REF:src/core/syntax/contents.rs`）に実装する（検証: `REF:tests/contents.rs`, `REF:tests/golden.rs`）。

- **[S-08] 会話閉じ括弧直後の `@` による話者予測補完（未実装・実装計画済み: `REF:docs/plans.md#plan-S-08`）**:
  - `REF:src/core/features/completion.rs` のトリガー判定に「同一行のカーソル前に `」` または `』` があり、その後に `@`（および入力途中のキャラ名/モード）が続く」ケースを追加し、`characters.md` の登場人物名と `:モードID` を候補として返す。

### 5.3 アウトライン形式（A 形式）— `plots/all.txt`, `plots/AA.txt`

状態: 実装完了（REF:src/core/syntax/plot_outline.rs, REF:tests/plot_outline.rs, REF:tests/golden.rs）

- 章ブロック `@AA` / 話ブロック `@AA-BB`、フィールド行と継続行（`drop_indent` で先頭タブ 1 文字または空白 2 文字を除去）、未知フィールド温存、無損失往復変換 `Outline::to_text` は `core::syntax::plot_outline`（`REF:src/core/syntax/plot_outline.rs`）に実装する（検証: `REF:tests/plot_outline.rs`, `REF:tests/roundtrip.rs`）。

### 5.4 話プロット形式（B 形式）— `plots/AA-BB.txt`

状態: 実装完了（REF:src/core/syntax/plot_episode.rs, REF:tests/plot_episode.rs, REF:tests/golden.rs）

- 場面見出し `@A-B-C` と `;` で区切られる節ブロック、場面レベル・節レベルのフィールド（`pov`, `mode`, `state`, `material`, `foreshadow`, `payoff`, `note`）、自由記述行、無損失往復変換 `EpisodePlot::to_text` は `core::syntax::plot_episode`（`REF:src/core/syntax/plot_episode.rs`）に実装する（検証: `REF:tests/plot_episode.rs`, `REF:tests/roundtrip.rs`）。

### 5.5 `characters.md`

状態: 実装完了（REF:src/core/syntax/characters_md.rs, REF:tests/characters_md.rs, REF:tests/golden.rs）

- H1 キャラ定義、固定 5 セクション（`attributes`, `tone`, `modes`, `relations`, `notes`）、H3 モード定義（`### <mode_id> <label>`）と同名 `mode_id` の `range:` 変種、役割語パターン行 `「...」` の抽出と無損失往復変換は `core::syntax::characters_md`（`REF:src/core/syntax/characters_md.rs`）に実装する（検証: `REF:tests/characters_md.rs`）。

### 5.6 `words.md`

状態: 実装完了（REF:src/core/syntax/words_md.rs, REF:tests/words_md.rs, REF:tests/golden.rs）

- H1 用語定義、フィールド行（`reading`, `alias`, `category`, `first`, `note`, `color`）、自由記述、重複定義 `E015` の検出と無損失往復変換は `core::syntax::words_md`（`REF:src/core/syntax/words_md.rs`）に実装する（検証: `REF:tests/words_md.rs`）。

### 5.7 `ideas.md`

状態: 実装完了（REF:src/core/syntax/ideas_md.rs, REF:src/core/features/ideas.rs, REF:tests/ideas_md.rs, REF:tests/ideas_ops.rs）

- `# I#### タイトル` の解析・ID 重複検査（`E026`）・無損失往復変換は `core::syntax::ideas_md`（`REF:src/core/syntax/ideas_md.rs`）が担い、採番・状態変更・プロットへの昇格操作は `core::features::ideas`（`REF:src/core/features/ideas.rs`）が担う（検証: `REF:tests/ideas_md.rs`, `REF:tests/ideas_ops.rs`）。

### 5.8 資料

状態: 実装完了（REF:src/core/model/material.rs, REF:src/core/syntax/materials_index.rs, REF:tests/materials_index.rs）

- `MaterialId`（接頭辞 `M` / `S` + 4 桁番号）と `MaterialRecord` は `REF:src/core/model/material.rs` に定義し、`materials/index.json` のパース・未知キー温存・重複 ID 検査（`E026`）・整形出力は `core::syntax::materials_index`（`REF:src/core/syntax/materials_index.rs`）が担う（検証: `REF:tests/materials_index.rs`）。

### 5.9 `prompts/*.md`

状態: 実装中（REF:src/core/syntax/prompt_md.rs, REF:src/core/agents/prompts.rs, REF:tests/prompt_md.rs, REF:docs/plans.md#plan-S-01）

- H1 ロール名、フィールド行（`version`）、H2 セクション（`system`, `task`, `output`）、`{{include:...}}` と 20 個の閉じた変数 `{{...}}` の検査（`W015` / `E029` / `E030`）は `core::syntax::prompt_md`（`REF:src/core/syntax/prompt_md.rs`）および `core::agents::prompts`（`REF:src/core/agents/prompts.rs`）に実装する（検証: `REF:tests/prompt_md.rs`）。

- **[S-01] 参考例展開変数 `{{examples}}`（未実装・実装計画済み: `REF:docs/plans.md#plan-S-01`）**:
  - `REF:src/core/syntax/prompt_md.rs` の `VARIABLES` 定数配列に `"examples"` を追加し、`REF:src/core/agents/prompts.rs` の変数展開でタスクの参考例テキストを注入する。

### 5.10 `agents/` のファイル形式

状態: 実装中（REF:src/core/agents/files.rs, REF:tests/agents_files.rs, REF:docs/plans.md#plan-S-01）

- タスク（`agents/tasks/T####.md`）、結果（`agents/results/T####.result.md`）、提案（`agents/proposals/P####.diff`、および人間用 `H####.diff`）、スレッド（`agents/threads/TH####.md`）、JSON Lines 監査ログ（`agents/log/YYYY-MM-DD.jsonl`）のパース・直列化・単調増加 ID 採番は `core::agents::files`（`REF:src/core/agents/files.rs`）および `core::features::proposals`（`REF:src/core/agents/files.rs`）に実装する（検証: `REF:tests/agents_files.rs`, `REF:tests/workspaces.rs`）。

- **[S-01] タスクの `examples:` フィールド（未実装・実装計画済み: `REF:docs/plans.md#plan-S-01`）**:
  - `REF:src/core/model/task.rs` の `Task` および `REF:src/core/agents/files.rs` のタスクパーサ・シリアライザに `examples: Vec<ExampleSpec>`（範囲式＋`contents`/`plot`/`both`）を追加する。

### 5.11 `agents/mcp.json`

状態: 実装中（REF:src/core/agents/mcp_config.rs, REF:tests/agents_files.rs, REF:docs/plans.md#plan-F-19）

- `agents/mcp.json` のパース、サーバ定義順の保持、`env` 内の `${VAR}` 環境変数展開と未定義エラー（`E030`）、非 `stdio` トランスポートや絶対パスコマンドの警告（`W033`）は `core::agents::mcp_config`（`REF:src/core/agents/mcp_config.rs`）に実装する（検証: `REF:tests/agents_files.rs`）。

- **[F-19] アプリ内 `mcp.json` 編集モーダル（未実装・実装計画済み: `REF:docs/plans.md#plan-F-19`）**:
  - `REF:src/ui/app.rs` の `Overlay` に `McpJsonModal` を追加し、`REF:src/core/agents/mcp_config.rs` の `parse` を用いて編集中テキストをリアルタイム検証した上で `agents/mcp.json` へ原子的に保存する。

### 5.12 フォーマッタ（正規化）仕様

状態: 実装完了（REF:src/core/features/formatting.rs, REF:src/ui/app.rs, REF:tests/features.rs）

- 行末空白除去、構造見出し直後の `1` 半角スペース正規化、フィールド行の `: ` 正規化（全角 `：` は保持）、連続空行の最大 2 行への圧縮、末尾 LF 1 個の保証は `core::features::formatting::format`（`REF:src/core/features/formatting.rs`）に実装し、コマンドパレット `>format`（`REF:src/ui/app.rs`）から呼び出す（検証: `REF:tests/features.rs`, `REF:tests/workspaces.rs`）。
