# novnow 設計書 — 資料収集・エクスポート・性能品質

本書は `REF:docs/specifications/workflows.md` の各節に対応する実装設計を記述する。

## 16. 資料収集機能

### 16.1 ワークフロー

状態: 未実装・実装計画済み（REF:src/core/features/reference.rs, REF:src/ui/workspaces/material.rs, REF:tests/materials_ops.rs, REF:docs/plans.md#plan-F-31）

- 資料の新規作成（提案 `proposed` / 承認 `approved`）、採番（`M####`）、索引登録、MD テンプレート生成は `core::features::reference`（`REF:src/core/features/reference.rs`）および `REF:src/ui/workspaces/material.rs` に実装する。

- **[F-31] 新規資料作成の全域有効化とボタン設置（未実装・実装計画済み: `REF:docs/plans.md#plan-F-31`）**:
  - `REF:src/ui/workspaces/material.rs` でペインフォーカスにかかわらず `Ctrl+Shift+N` と画面上のボタンで新規資料を作成可能にする。

### 16.2 インデックス（§5.8.2 の運用規則）

状態: 実装完了（REF:src/core/syntax/materials_index.rs, REF:src/core/features/project.rs, REF:tests/materials_ops.rs）

- `materials/docs/` 内の未登録ファイル検出（`W064`）と索引への自動追加クイックフィックス、および索引登録済みで実ファイルが欠落している項目の検出（`W065`）は `REF:src/core/features/project.rs` と `REF:src/core/features/reference.rs` に実装する（検証: `REF:tests/materials_ops.rs`）。

### 16.3 検索 UX

状態: 実装完了（REF:src/core/features/reference.rs, REF:src/ui/app.rs, REF:tests/materials_ops.rs, REF:tests/workspaces.rs）

- `Ctrl+M` 資料検索モーダルのクエリ解析（`tag:`, `cat:`, `id:`, `scene:`, `status:`, `!status:`）と決定論的スコアリング（ID 完全一致 +500、タイトル先頭一致 +400、部分一致 +250、タグ完全一致 +200、要約一致 +100、本文一致 +50）、および `Enter`（参照挿入）/ `Shift+Enter`（`material` で開く）/ `Ctrl+Enter`（新規提案作成）は `REF:src/core/features/reference.rs` と `REF:src/ui/app.rs` に実装する。

### 16.4 「必要になる条件」と自動提示

状態: 実装完了（REF:src/core/resolve/materials.rs, REF:src/core/features/reference.rs, REF:tests/materials.rs, REF:tests/materials_ops.rs）

- `scenes` 範囲一致・`characters` 登場一致・`keywords` 出現一致による自動提示候補の抽出と、参照ペインの `[候補]` 表示および `Enter` による話プロット `material:` 行への昇格は `REF:src/core/resolve/materials.rs` と `REF:src/core/features/reference.rs` に実装する。

### 16.5 プロットからの参照と表示

状態: 未実装・実装計画済み（REF:src/core/features/reference.rs, REF:src/ui/widgets/reference_pane.rs, REF:tests/context_rules.rs, REF:docs/plans.md#plan-F-34）

- 話プロット・章プロットの `material:` フィールドから解決された資料のタイトル・要点（最大 5 行）の抽出は `REF:src/core/features/reference.rs` が担い、`REF:src/ui/widgets/reference_pane.rs` が描画する。

- **[F-34 / S-07] 要約の「続き...」表示と資料 URL の外部リンク化（未実装・実装計画済み: `REF:docs/plans.md#plan-F-34`, `REF:docs/plans.md#plan-S-07`）**:
  - `REF:src/ui/widgets/reference_pane.rs` と `REF:src/ui/workspaces/material.rs` に切り詰め時の「`続き...`」行表示（F-34）および URL の外部リンクアイコン＋下線＋ホバー青色表示（S-07）を追加する。

### 16.6 資料 MD の編集

状態: 実装完了（REF:src/core/text/sha256.rs, REF:src/core/features/reference.rs, REF:src/ui/workspaces/material.rs, REF:tests/materials_ops.rs）

- 資料 MD 保存時の SHA-256 チェックサム計算（`REF:src/core/text/sha256.rs`）と `materials/index.json` の `checksum` / `updated_at` 自動更新は `REF:src/core/features/reference.rs` に実装する。

### 16.7 エージェントとの連携

状態: 実装完了（REF:src/core/agents/server.rs, REF:src/core/agents/apply.rs, REF:tests/mcp_stdio.rs, REF:tests/agents_files.rs）

- `researcher` ロールおよび MCP `novnow.material.propose` からの資料提案登録（`status: proposed`）と `material` ワークスペースでの承認・却下は `REF:src/core/agents/server.rs` と `REF:src/ui/workspaces/material.rs` に実装する。

## 17. エクスポート

### 17.1 アノテーション除去パイプライン

状態: 実装完了（REF:src/core/export.rs, REF:tests/export.rs, REF:tests/export_invariants.rs）

- 14 段のアノテーション除去パイプライン（対象節収集、構造見出し除去/挿入、コメント除去、行末話者注釈除去、`'` 参照の平文化、資料参照の完全除去、エスケープ解除、ルビ処理、節区切り変換、連続空行圧縮、行頭字下げ、改行コード変換、末尾整形）は純粋関数 `core::export::run`（`REF:src/core/export.rs`）に実装する（`B-98`, `B-99`）。
- エクスポート不変条件（出力に `@` 見出し・`//`・`/* */`・解決済み `'`・資料 ID・単独 `;` 行が残らないこと）は `REF:tests/export_invariants.rs` で検証する。

### 17.2 範囲指定と出力

状態: 実装完了（REF:src/core/export.rs, REF:src/cli/export.rs, REF:src/ui/app.rs, REF:tests/export.rs, REF:tests/cli.rs）

- 範囲式（`@` 省略形を含む `parse_range`）と確定度フィルタ（`effective_state` による `draft` / `review` / `fixed` 絞り込み）、既定ファイル名生成 `file_name` は `REF:src/core/export.rs` に実装し、CLI `novnow export`（`REF:src/cli/export.rs`）および GUI `>export` モーダル（`REF:src/ui/app.rs`）から呼び出す（`B-100`, `B-101`）。

### 17.3 読者モード（`reader` ワークスペース）

状態: 実装待ち（段階 ② 対象: REF:docs/plans.md#stage-2）

- 縦書き読者モードは追加仕様 27（段階 ②）として予約されている。`project.json` の `reader` 設定セクションは未知キー温存（縫い代 1）により段階 ① でも無損失に保持される（`REF:tests/project_json.rs`）。
- **段階 ② 実装時のアーキテクチャ境界（`REF:docs/plans.md#stage-2`）**:
  - `egui` 標準の `TextEdit` / `Label` は横書き専用であるため、読者モードは `egui::Painter` による自前グリフ配置で実装する。
  - 禁則処理・縦中横・ルビ配置・回転グリフ判定・改行および段組み（1 段 / 2 段）の座標計算は GUI に依存しない純粋関数 `core::reader::layout(blocks: &[ReaderBlock], config: &ReaderConfig, metrics: &dyn FontMetrics) -> Vec<Page>` として `src/core/` に実装し（P7 遵守）、`ui::workspaces::reader` は `Page` 内の `PlacedGlyph` 列を `Painter` に描くだけの薄い層とする。
  - 約物の縦書き置換は `ttf-parser` でフォントの GSUB `vert` / `vrt2` テーブルを参照できない環境でも破綻しないよう、Unicode 縦書き形（U+FE10–U+FE19, U+FE30–U+FE4F）へのコードポイント置換表と 90 度時計回り回転描画（`ー` `〜` `…` `‥` `—` 等）の二段構えとする。

## 18. 性能・品質要件

### 18.1 性能目標（既定のリファレンス環境: 4 コア / 8GB / 統合 GPU）

状態: 未実装・実装計画済み（REF:benches/core.rs, REF:tests/incremental.rs, REF:tests/role_language.rs, REF:.github/workflows/ci.yml, REF:docs/plans.md#plan-F-43）

- 主要 5 処理の `criterion` ベンチマークは `REF:benches/core.rs` に実装している（リリース実測値: `single_line_relex` 約 462ns、`role_language_10_patterns` 約 1.89µs、`dictionary_trie_5000` 約 1.14ms、`reference_pane_rebuild` 約 38.7µs、`export_100k_chars` 約 7.37ms、`B-155`）。
- Web リリースビルドの gzip 後 wasm サイズ（上限 8MiB、実測約 6.75MiB）は `REF:.github/workflows/ci.yml` の `wasm` ジョブで検査する。

- **[F-38 / F-43 / S-12] 10 万字打鍵 16ms 以内・アイデア 1,000 件 60fps・文脈ビュー可視範囲レイアウト（未実装・実装計画済み: `REF:docs/plans.md#plan-F-43`）**:
  - 打鍵時の編集中節限定解析（F-43）、一覧の `show_rows` 仮想スクロール（F-38）、文脈ビューのビューポートカリング（S-12）を実装し、`REF:tests/incremental.rs` と `REF:benches/core.rs` で計測・検証する。

### 18.2 メモリ

状態: 実装完了（REF:src/core/text/rope.rs, REF:src/core/text/incremental.rs）

- 本文バッファは `ropey::Rope` ラッパ `TextBuffer`（`REF:src/core/text/rope.rs`）で保持し、行種別インデックス `LineIndex`（`REF:src/core/text/incremental.rs`）は 1 行 1 バイトの `LineTag` のみを保持する。

### 18.3 堅牢性

状態: 実装完了（REF:src/main.rs, REF:src/adapters/env_system.rs, REF:tests/layering.rs, REF:tests/roundtrip.rs）

- 全パーサは不正入力でもパニックせず部分木と診断を返す（`REF:tests/roundtrip.rs`, `REF:tests/golden.rs`）。
- `unsafe` ブロックは Windows FFI の 2 箇所（`REF:src/main.rs` の `AttachConsole`、`REF:src/adapters/env_system.rs` の `GetUserDefaultLocaleName`）のみに限定し、それ以外の `unsafe` がリポジトリ内に存在しないことを `REF:tests/layering.rs`（`unsafe_is_restricted_to_the_two_windows_ffi_sites`）で機械検査する。

### 18.4 継続的検証要件

状態: 未実装・実装計画済み（REF:tests/layering.rs, REF:.github/workflows/ci.yml, REF:scripts/verify.sh, REF:docs/script/duplicate.py, REF:docs/plans.md#plan-S-15）

- CI ワークフロー（`REF:.github/workflows/ci.yml`）の 5 ジョブ（`fmt`, `clippy`, `test`, `wasm`, `bench`）およびローカル検証スクリプト `REF:scripts/verify.sh`、ドキュメント規約検査スクリプト `REF:docs/script/duplicate.py` により全変更を機械的に検証する。

- **[F-30 / F-32 / F-42 / F-43 / S-15] 追加の CI・自動テスト検証項目（未実装・実装計画済み: `REF:docs/plans.md#plan-S-15`）**:
  - `REF:tests/gui.rs`（押せる要素の下線・ホバー・24×24px 当たり判定検査）、`REF:tests/i18n_catalog.rs`（`ja.toml` 助詞前空白検査）、`REF:tests/incremental.rs`（10 万字 1 打鍵 16ms 検査）、および全診断コードの Web サイト解説網羅性 CI チェックを追加する。
