# novnow 設計書 — ストレージと起動仕様

本書は `REF:docs/specifications/platform.md` の各節に対応する実装設計を記述する。

## 13. ストレージ

### 13.1 規則 10 — プロジェクト外に出してよい情報

状態: 実装完了（REF:src/core/policy/write_policy.rs, REF:src/adapters/storage_fs.rs, REF:src/ports/dialog.rs, REF:tests/storage_policy.rs, REF:tests/layering.rs）

- **境界判定（`WritePolicy`）**: `REF:src/core/policy/write_policy.rs` がシンボリックリンク解決後の絶対パスを受け取り、現在開いている `project_root` 配下または `history_file` 1 個のみを許可する（`B-54`）。
- **明示エクスポート先の特例（`ChosenPath` 関門、`B-108`）**: `ChosenPath`（`REF:src/ports/dialog.rs`、プライベートフィールドにより保存ダイアログまたは CLI の `--out` 引数解析以外での構築を型レベルで禁止）と `WritePolicy::check_chosen_export` / `FsGate::write_chosen_export`（`REF:src/adapters/storage_fs.rs`）のみが、ユーザーが明示指定した拡張子一致ファイル 1 個への単発書き込みを許可する。
- **単一関門の強制と違反時の挙動（`B-20`, `B-107`）**: `src/` 全体でファイル書き込み API を呼んでよいのは `REF:src/adapters/storage_fs.rs` のみであり、`REF:tests/layering.rs`（`only_the_write_gate_touches_the_file_system_for_writing`）と `REF:tests/storage_policy.rs` で機械的に強制する。ポリシー違反時はデバッグビルドでは `debug_assert!` で即座にパニックし、リリースビルドでは `Err(WriteViolation::ForbiddenTarget)`（診断 `E060`）を返してシステムコールを一切発行しない。
- 配布物取得の限定例外は `REF:vscode/src/binary.ts` と `REF:scripts/action/install.sh` の専用保存領域で扱い、作品の `FsGate` / `WritePolicy` を緩めない。原稿・設定・ログを取得キャッシュへ保存しない。
- **外部シンボリックリンクの遮断**: プロジェクト内から外部を指すシンボリックリンクは `REF:src/adapters/storage_fs.rs` の列挙・読み取りで除外する。

### 13.2 デスクトップ: パス履歴ファイル

状態: 実装完了（REF:src/core/policy/recent.rs, REF:src/adapters/recent_file.rs, REF:tests/recent_file.rs）

- 履歴 JSON スキーマ（`schema_version: 1`, `items: [{ path, opened_at }]`、上限 32 件、重複統合）は `RecentProjects`（`REF:src/core/policy/recent.rs`）に純粋データ構造として実装し、OS 別の保存先解決と `FsGate` 経由の直接上書きは `REF:src/adapters/recent_file.rs` が担う（検証: `REF:tests/recent_file.rs`）。

### 13.3 Web: IndexedDB

状態: 実装完了（REF:src/adapters/storage_idb.rs, REF:tests/storage_idb.rs, REF:index.html, REF:sw.js, REF:manifest.webmanifest, REF:scripts/web-smoke.mjs）

- DB 名 `novnow`（version 1）の 3 オブジェクトストア（`projects`, `files`, `app`）と `APP_KEYS`（`recent_projects` のみ許可）、残容量 10MB 未満の `W063`、IndexedDB 利用不可時の `E062`、初回 `sample` プロジェクト投入は `REF:src/adapters/storage_idb.rs` に実装する。
- スキーマ定数と仕様表の一致は `REF:tests/storage_idb.rs` が `include_str!` で検証し、ブラウザ実機動作は `REF:scripts/web-smoke.mjs` で検証する。

- **[S-03] Web 版の PWA 化とオフライン動作（実装完了: `REF:index.html`, `REF:sw.js`, `REF:README.md`, `REF:scripts/web-smoke.mjs`）**:
  - `REF:index.html` に Web App Manifest（`REF:manifest.webmanifest`）と Service Worker（`REF:sw.js`）登録を設け、オフラインキャッシュと `REF:README.md` への PWA 案内を実装する。`REF:scripts/pwa-assets.py` を Trunk post_build で実行し、全資源の一覧と内容ハッシュを worker に封入する。`REF:sw.js` は全件キャッシュが成功したときだけ activate する。旧キャッシュを install 中に消さない。`REF:scripts/web-smoke.mjs` では追加のオンライン再読み込みを挟まず、オフライン再起動と IndexedDB の継続性を確認する。

### 13.4 プロジェクト選択モーダル（Web）

状態: 未実装・実装計画済み（REF:src/ui/web.rs, REF:src/adapters/storage_idb.rs, REF:scripts/web-smoke.mjs, REF:tests/gui.rs, REF:docs/plans.md#issue-29-b8）

- Web 起動時および `Ctrl+O` 時のプロジェクト選択モーダル（一覧・検索 `/`・新規作成 `Ctrl+N`・Zip 取込 `Ctrl+I`・改名 `F2`・削除 `Delete`・単一タブ利用の注記 `project.pick.single_tab`）は `WebPicker`（`REF:src/ui/web.rs`）に実装する（検証: `REF:scripts/web-smoke.mjs`）。

- **[F-33 / S-02] Web 版の同名プロジェクト作成禁止と初回起動ウェルカム案内（実装完了: `REF:docs/plans.md#plan-F-33`, `REF:docs/plans.md#plan-S-02`）**:
  - `REF:src/ui/web.rs` と `REF:src/adapters/storage_idb.rs` で既存プロジェクトと同一タイトルの作成・改名を拒否する（F-33 は実装完了。`REF:tests/storage_idb.rs` と実ブラウザ `REF:scripts/web-smoke.mjs` で検証済み）。履歴が空の初回起動時に案内を表示し、`>welcome` パレットからウェルカム案内モーダルを再表示する（S-02 は `REF:src/ui/app.rs`, `REF:src/ui/web.rs`, `REF:tests/gui.rs` で確認済み）。

- **[Issue #29 / B8] 入口ページの初期言語（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b8`）**:
  - `REF:index.html` の言語固定を解き、本体読み込み前の静的な表示もブラウザ（OS）のロケールで出す（`lang` 属性と読み込み中文言の出し分け）。選択画面に言語切替ボタンは置かない現行維持を確認し、`REF:scripts/web-smoke.mjs` で ja/en 両ロケールの初期表示を確かめる。

### 13.5 Zip インポート / エクスポート

状態: 未実装・実装計画済み（REF:src/core/policy/archive.rs, REF:src/adapters/zip.rs, REF:src/cli/import.rs, REF:src/ui/app.rs, REF:tests/zip_roundtrip.rs, REF:tests/cli.rs, REF:docs/plans.md#issue-29-b10）

- アーカイブ対象の選別（ドット始まりファイルの除外と `.gitignore` / `.gitkeep` の例外許可、パストラバーサル `..` の拒否）は `core::policy::archive`（`REF:src/core/policy/archive.rs`）が担い、Deflate 圧縮・展開は `adapters::zip`（`REF:src/adapters/zip.rs`）が担う（検証: `REF:tests/zip_roundtrip.rs`）。

- **[S-10 / S-16] デスクトップ Zip インポートと Web⇔デスクトップ移行ヘルプ（実装完了: `REF:src/cli/import.rs`, `REF:src/ui/app.rs`, `REF:tests/cli.rs`, `REF:tests/gui.rs`）**:
  - `REF:src/cli/mod.rs` と `REF:src/ui/app.rs` にデスクトップでの Zip 導入を追加し（S-10 は `REF:tests/cli.rs`, `REF:tests/zip_roundtrip.rs` で安全な復元を検証）、Zip を使った Web⇔デスクトップ移行手順のヘルプ画面を追加する（S-16 は `REF:src/ui/app.rs`, `REF:tests/gui.rs` で検証済み）。

## 14. 起動仕様

- **[Issue #29 / B10] `.novnow` パッケージ（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b10`）**:
  - `.novnow` は実体が通常の Zip で、`.zip` と同じ安全規則（パス検証・未知ファイルの保持・上書き禁止）で扱う。インポート時に novnow が知らないファイルを落とさないことを往復テストへ足す（§4.3 の未知ファイルの扱いと整合させる）。

### 14.1 CLI 文法

状態: 未実装・実装計画済み（REF:src/cli/mod.rs, REF:src/main.rs, REF:tests/cli.rs, REF:docs/plans.md#issue-29-x1）

- 引数解析とサブコマンド（`<dir>`, `new`, `check`, `export`, `mcp`, `import`, `migrate`）およびフラグ（`--gui`, `-h`/`--help`, `-V`/`--version`）の振り分けは `REF:src/cli/mod.rs` に実装する（検証: `REF:tests/cli.rs`）。

- **[S-10] `novnow import <zip> <dir>` サブコマンド（実装完了: `REF:docs/plans.md#plan-S-10`）**:
  - `REF:src/cli/mod.rs` に `import <zip> <dir>` を追加し、`REF:src/adapters/zip.rs` で展開・検証を行う。

- **[Issue #29 / X1] `migrate` の引数拡張（未実装・実装計画済み: `REF:docs/plans.md#issue-29-x1`）**:
  - 複数ファイル・フォルダ入力（§14.6）の CLI 形式は、§14.6 の拡張（別 PR）の実装時に本節の文法へ合わせて取り込む。既存の `migrate <text> <dir>` の並びがそのまま使える形（最後の引数が移行先）を保つ。

### 14.2 起動時のスキャンとエラー出力

状態: 未実装・実装計画済み（REF:src/cli/open.rs, REF:src/cli/check.rs, REF:src/core/features/place.rs, REF:tests/cli.rs, REF:docs/plans.md#issue-29-b10）

- 起動前スキャン、不足項目一覧の表示、`novnow new` の案内、作家の言葉での診断位置表記 `[本文 @01-04-02-03]`（`REF:src/core/features/place.rs`）、および終了コード `0`〜`5` の制御は `REF:src/cli/open.rs` と `REF:src/cli/check.rs` に実装する。
- 仕様書 §14.2.2〜§14.2.4 の出力例との一字一句の一致は `REF:tests/cli.rs` で検証する。
- §14.2.1 の 3 つの URL は `REF:src/core/model/project.rs` の定数だけが持ち、`REF:src/cli/mod.rs`・
  `REF:src/cli/open.rs`・`REF:src/ui/app.rs` は定数を参照する（直書きしない）。3 URL の公開物の実在は
  `REF:docs/script/reference.py` の検査 11 が照合する（対応するページは `guide/` 配下に生成する）。
- 案内文は「何が足りないか」の列挙で終わらせず、次にすること（`novnow new` / フォルダを開く）を
  最後の 1 文に必ず置く（利用者向けガイドと同じ言い回しにする）。

- **[Issue #29 / B10] 出力例の新名称への差し替え（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b10`）**:
  - 仕様 §14.2.2 / §14.2.3 の出力ブロックは現実装の stderr とバイト単位で照合されるため、B10 で不足一覧の実装（`project.toml` / `plots/all.cnov`）と同時に仕様のブロックも差し替える。それまでは現行のブロックが現実装の正本である。

### 14.3 GUI 直接起動の判定

状態: 実装完了（REF:src/cli/mod.rs, REF:src/main.rs, REF:tests/cli.rs）

- 優先順位（`--help`/`--version` → サブコマンド → `--gui` → 位置引数 `<dir>` → Web → 非 TTY → TTY 引数なしは終了コード 2）は `REF:src/cli/mod.rs` に実装する。
- Windows の `AttachConsole(ATTACH_PARENT_PROCESS)` は `REF:src/main.rs` の `#[cfg(windows)]` ブロックにのみ置く（`REF:tests/layering.rs` で検査）。

### 14.4 GUI 起動時のプロジェクト選択モーダル（デスクトップ）

状態: 実装完了（REF:src/ui/app.rs, REF:tests/gui.rs）

- デスクトップで `<dir>` なしに GUI 起動した際のプロジェクト選択画面（`recent.json` の履歴一覧、存在しないパスの灰色表示と履歴削除、フォルダ選択ダイアログ、新規作成）は `NativePicker`（`REF:src/ui/app.rs`）に実装する。

- **[F-40 / S-11 / S-17] デスクトップ選択画面の `Ctrl+O` 2 段化・日付表示・履歴削除（実装完了: `REF:docs/plans.md#plan-F-40`, `REF:docs/plans.md#plan-S-11`, `REF:docs/plans.md#plan-S-17`）**:
  - `REF:src/ui/app.rs` の `NativePicker` で、`Ctrl+O` 押下時は保存して履歴一覧画面へ戻り、そこから「フォルダを開く」を選ぶ 2 段構成に統一する（F-40）。
  - 各履歴項目に最終利用日付 `opened_at` を表示する（S-11、`REF:src/ui/app.rs` の native picker テストで確認）。「データは消さず履歴のみを一覧から削除する」旨を明示した履歴削除機能を提供する（S-17、`REF:tests/recent_file.rs` で実データ不変を確認）。

### 14.5 `novnow new` の生成物

状態: 未実装・実装計画済み（REF:src/core/template/mod.rs, REF:src/cli/new.rs, REF:tests/template.rs, REF:tests/cli.rs, REF:docs/plans.md#issue-29-b10）

- 初期ファイル一式の生成と既存ファイル保護（上書きせず `kept` / `created` を表示）、生成直後の `novnow check` 診断 0 件保証は `REF:src/core/template/mod.rs` と `REF:src/cli/new.rs` に実装し、`REF:tests/template.rs` と `REF:tests/cli.rs` で検証する。

- **[Issue #29 / B10] 生成物の改名（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b10`）**:
  - `novnow new` の生成物は新しい名前（`project.toml` / `.cnov`、`schema_version = 2`）で作る。仕様 §14.5 の一覧と `REF:tests/template.rs` の照合リストは先に新名称へ更新済みであり、B10 で生成側を合わせる。

### 14.6 既存本文テキストからの移行支援

状態: 未実装・実装計画済み（REF:src/core/migration.rs, REF:src/cli/migrate.rs, REF:src/adapters/storage_fs.rs, REF:src/ui/app.rs, REF:docs/plans.md#issue-29-x1）

- 純粋な変換関数 `convert` は `REF:src/core/migration.rs`。既定雛形からサンプルを除去し、1 節の本文と空の構造を生成する。
- CLI の読み取り・プレビュー・確定は `REF:src/cli/migrate.rs`。移行先を排他的に作成する `FsStorage::create_fresh`（`REF:src/adapters/storage_fs.rs`）で既存先を拒否する。書き込みは既存の単一関門を使用する。
- `REF:tests/migration.rs` で記号の往復・正規化、`REF:tests/cli.rs` でプレビュー非変更・入力不正・既存先拒否・入力元保持を検証する。
- デスクトップは `REF:src/ui/app.rs` の NativePicker とダイアログポートから入力選択→プレビュー→新規先へ作成→開くを接続する。Web は初期移行の対象外。

- **[Issue #29 / X1] 候補提示型の分割と Shift_JIS 対応（未実装・実装計画済み: `REF:docs/plans.md#issue-29-x1`）**:
  - 文字コードはファイル取り込みでのみバイト列から判定し、UTF-8 を優先して読めなければ Shift_JIS として変換する（曖昧な場合はプレビュー切替）。GUI の貼り付けは Unicode のままで、文字化けらしい場合だけ修復候補を出す。Web は対象外を維持する。
  - 候補生成は見出し行（「第○章」「第○話」等と固有名詞の章名）・記号だけの区切り行・連続空行・段落区切りを用い、章/話/場面/節のツリーをプレビューで直せるようにする。複数入力は自然順で 1 ファイル = 1 話、1 フォルダ = 1 章。見出しのタイトル部分はタイトルへ使い、構造番号は出現順に振り直す。話者推定はしない。
