# novnow 設計書 — ストレージと起動仕様

本書は `REF:docs/specifications/platform.md` の各節に対応する実装設計を記述する。

## 13. ストレージ

### 13.1 規則 10 — プロジェクト外に出してよい情報

状態: 実装完了（REF:src/core/policy/write_policy.rs, REF:src/adapters/storage_fs.rs, REF:src/ports/dialog.rs, REF:tests/storage_policy.rs, REF:tests/layering.rs）

- **境界判定（`WritePolicy`）**: `REF:src/core/policy/write_policy.rs` がシンボリックリンク解決後の絶対パスを受け取り、現在開いている `project_root` 配下または `history_file` 1 個のみを許可する（`B-54`）。
- **明示エクスポート先の特例（`ChosenPath` 関門、`B-108`）**: `ChosenPath`（`REF:src/ports/dialog.rs`、プライベートフィールドにより保存ダイアログまたは CLI の `--out` 引数解析以外での構築を型レベルで禁止）と `WritePolicy::check_chosen_export` / `FsGate::write_chosen_export`（`REF:src/adapters/storage_fs.rs`）のみが、ユーザーが明示指定した拡張子一致ファイル 1 個への単発書き込みを許可する。
- **単一関門の強制と違反時の挙動（`B-20`, `B-107`）**: `src/` 全体でファイル書き込み API を呼んでよいのは `REF:src/adapters/storage_fs.rs` のみであり、`REF:tests/layering.rs`（`only_the_write_gate_touches_the_file_system_for_writing`）と `REF:tests/storage_policy.rs` で機械的に強制する。ポリシー違反時はデバッグビルドでは `debug_assert!` で即座にパニックし、リリースビルドでは `Err(WriteViolation::ForbiddenTarget)`（診断 `E060`）を返してシステムコールを一切発行しない。
- **外部シンボリックリンクの遮断**: プロジェクト内から外部を指すシンボリックリンクは `REF:src/adapters/storage_fs.rs` の列挙・読み取りで除外する。

### 13.2 デスクトップ: パス履歴ファイル

状態: 実装完了（REF:src/core/policy/recent.rs, REF:src/adapters/recent_file.rs, REF:tests/recent_file.rs）

- 履歴 JSON スキーマ（`schema_version: 1`, `items: [{ path, opened_at }]`、上限 32 件、重複統合）は `RecentProjects`（`REF:src/core/policy/recent.rs`）に純粋データ構造として実装し、OS 別の保存先解決と `FsGate` 経由の直接上書きは `REF:src/adapters/recent_file.rs` が担う（検証: `REF:tests/recent_file.rs`）。

### 13.3 Web: IndexedDB

状態: 未実装・実装計画済み（REF:src/adapters/storage_idb.rs, REF:tests/storage_idb.rs, REF:docs/plans.md#plan-S-03）

- DB 名 `novnow`（version 1）の 3 オブジェクトストア（`projects`, `files`, `app`）と `APP_KEYS`（`recent_projects` のみ許可）、残容量 10MB 未満の `W063`、IndexedDB 利用不可時の `E062`、初回 `sample` プロジェクト投入は `REF:src/adapters/storage_idb.rs` に実装する。
- スキーマ定数と仕様表の一致は `REF:tests/storage_idb.rs` が `include_str!` で検証し、ブラウザ実機動作は `REF:scripts/web-smoke.mjs` で検証する。

- **[S-03] Web 版の PWA 化とオフライン動作（未実装・実装計画済み: `REF:docs/plans.md#plan-S-03`）**:
  - `REF:index.html` に Web App Manifest と Service Worker 登録を追加し、オフラインキャッシュと `REF:README.md` への PWA 案内を実装する。

### 13.4 プロジェクト選択モーダル（Web）

状態: 未実装・実装計画済み（REF:src/ui/web.rs, REF:scripts/web-smoke.mjs, REF:docs/plans.md#plan-F-33）

- Web 起動時および `Ctrl+O` 時のプロジェクト選択モーダル（一覧・検索 `/`・新規作成 `Ctrl+N`・Zip 取込 `Ctrl+I`・改名 `F2`・削除 `Delete`・単一タブ利用の注記 `project.pick.single_tab`）は `WebPicker`（`REF:src/ui/web.rs`）に実装する（検証: `REF:scripts/web-smoke.mjs`）。

- **[F-33 / S-02] Web 版の同名プロジェクト作成禁止と初回起動ウェルカム案内（未実装・実装計画済み: `REF:docs/plans.md#plan-F-33`, `REF:docs/plans.md#plan-S-02`）**:
  - `REF:src/ui/web.rs` と `REF:src/adapters/storage_idb.rs` で既存プロジェクトと同一タイトルの作成・改名を拒否し（F-33）、履歴が空の初回起動時およびパレットからウェルカム案内モーダルを表示する（S-02）。

### 13.5 Zip インポート / エクスポート

状態: 未実装・実装計画済み（REF:src/core/policy/archive.rs, REF:src/adapters/zip.rs, REF:tests/zip_roundtrip.rs, REF:docs/plans.md#plan-S-10）

- アーカイブ対象の選別（ドット始まりファイルの除外と `.gitignore` / `.gitkeep` の例外許可、パストラバーサル `..` の拒否）は `core::policy::archive`（`REF:src/core/policy/archive.rs`）が担い、Deflate 圧縮・展開は `adapters::zip`（`REF:src/adapters/zip.rs`）が担う（検証: `REF:tests/zip_roundtrip.rs`）。

- **[S-10 / S-16] デスクトップ Zip インポートと Web⇔デスクトップ移行ヘルプ（未実装・実装計画済み: `REF:docs/plans.md#plan-S-10`, `REF:docs/plans.md#plan-S-16`）**:
  - `REF:src/cli/mod.rs` と `REF:src/ui/app.rs` にデスクトップでの Zip 導入を追加し（S-10）、Zip を使った Web⇔デスクトップ移行手順のヘルプ画面を追加する（S-16）。

## 14. 起動仕様

### 14.1 CLI 文法

状態: 未実装・実装計画済み（REF:src/cli/mod.rs, REF:src/main.rs, REF:tests/cli.rs, REF:docs/plans.md#plan-S-10）

- 引数解析とサブコマンド（`<dir>`, `new`, `check`, `export`, `mcp`）およびフラグ（`--gui`, `-h`/`--help`, `-V`/`--version`）の振り分けは `REF:src/cli/mod.rs` に実装する（検証: `REF:tests/cli.rs`）。

- **[S-10] `novnow import <zip> <dir>` サブコマンド（未実装・実装計画済み: `REF:docs/plans.md#plan-S-10`）**:
  - `REF:src/cli/mod.rs` に `import <zip> <dir>` を追加し、`REF:src/adapters/zip.rs` で展開・検証を行う。

### 14.2 起動時のスキャンとエラー出力

状態: 実装完了（REF:src/cli/open.rs, REF:src/cli/check.rs, REF:src/core/features/place.rs, REF:tests/cli.rs）

- 起動前スキャン、不足項目一覧の表示、`novnow new` の案内、作家の言葉での診断位置表記 `[本文 @01-04-02-03]`（`REF:src/core/features/place.rs`）、および終了コード `0`〜`5` の制御は `REF:src/cli/open.rs` と `REF:src/cli/check.rs` に実装する。
- 仕様書 §14.2.2〜§14.2.4 の出力例との一字一句の一致は `REF:tests/cli.rs` で検証する。

### 14.3 GUI 直接起動の判定

状態: 実装完了（REF:src/cli/mod.rs, REF:src/main.rs, REF:tests/cli.rs）

- 優先順位（`--help`/`--version` → サブコマンド → `--gui` → 位置引数 `<dir>` → Web → 非 TTY → TTY 引数なしは終了コード 2）は `REF:src/cli/mod.rs` に実装する。
- Windows の `AttachConsole(ATTACH_PARENT_PROCESS)` は `REF:src/main.rs` の `#[cfg(windows)]` ブロックにのみ置く（`REF:tests/layering.rs` で検査）。

### 14.4 GUI 起動時のプロジェクト選択モーダル（デスクトップ）

状態: 未実装・実装計画済み（REF:src/ui/app.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-40）

- デスクトップで `<dir>` なしに GUI 起動した際のプロジェクト選択画面（`recent.json` の履歴一覧、存在しないパスの灰色表示と履歴削除、フォルダ選択ダイアログ、新規作成）は `NativePicker`（`REF:src/ui/app.rs`）に実装する。

- **[F-40 / S-11 / S-17] デスクトップ選択画面の `Ctrl+O` 2 段化・日付表示・履歴削除（未実装・実装計画済み: `REF:docs/plans.md#plan-F-40`, `REF:docs/plans.md#plan-S-11`, `REF:docs/plans.md#plan-S-17`）**:
  - `REF:src/ui/app.rs` の `NativePicker` で、`Ctrl+O` 押下時は保存して履歴一覧画面へ戻り、そこから「フォルダを開く」を選ぶ 2 段構成に統一する（F-40）。
  - 各履歴項目に最終利用日付 `opened_at` を表示し（S-11）、「データは消さず履歴のみを一覧から削除する」旨を明示した履歴削除機能を追加する（S-17）。

### 14.5 `novnow new` の生成物

状態: 実装完了（REF:src/core/template/mod.rs, REF:src/cli/new.rs, REF:tests/template.rs, REF:tests/cli.rs）

- 初期ファイル一式の生成と既存ファイル保護（上書きせず `kept` / `created` を表示）、生成直後の `novnow check` 診断 0 件保証は `REF:src/core/template/mod.rs` と `REF:src/cli/new.rs` に実装し、`REF:tests/template.rs` と `REF:tests/cli.rs` で検証する。

### 14.6 既存本文テキストからの移行支援

状態: 未実装・未計画（REF:docs/plans.md#unplanned-large）

- **[L-01] 既存の本文テキストから `novnow` に移行するための手段の提供（未実装・未計画: `REF:docs/plans.md#unplanned-large`）**:
  - 比較的大規模な機能追加であり、現時点では実装および詳細計画の対象外（未実装・未計画）とする。
