# novnow 設計書 — 外部インターフェース（ヘッドレス全機能 API 群）

本書は `REF:docs/specifications/interfaces.md` の各節に対応する実装設計を記述する。実装順序・マイルストーン・受け入れ基準は `REF:docs/plans.md#stage-3` に定める。

## 19. 外部インターフェース（ヘッドレス全機能 API 群）

5 インターフェースの共通基盤は、新設する `src/app/` 層（常時コンパイル・egui 非依存）と、そこが公開する `Session` ファサードである。現状は描画と無縁の状態機械が `REF:src/ui/mod.rs` の `ui` feature ゲートの内側に閉じているため、段階 ③ の H1 でこれらを `src/app/` へ移し、GUI（`REF:src/ui/app.rs`）を Session の表示層の 1 つ、CLI・MCP・LSP・Python をプロトコル層にする。依存の向きは `ui → app → adapters → ports ← core` へ拡張し、`REF:docs/designs/overview.md` の §3.3 は H1 の実装時に改訂する。

### 19.1 共通原則

状態: 実装完了（REF:src/app/ops/mod.rs, REF:src/app/session.rs, REF:tests/api.rs）

- **単一実装点（H1）**: `Workbench`（`REF:src/app/workbench.rs`）、`SaveMachine`（`REF:src/app/save_machine.rs`）、`UiSettings`（`REF:src/app/settings.rs`）、エージェント実行状態（`REF:src/app/agents.rs`）、検索状態（`REF:src/app/assist.rs`）、パレットの純粋論理（`REF:src/app/palette.rs`）、`Command`（`REF:src/ui/shell.rs`）、`Switcher`（`REF:src/ui/workspaces/mod.rs`）、`Host` トレイト（現在 `REF:src/ui/app.rs` 内）を `src/app/` へ移す。移設は「移動 + 旧パスからの `pub use` 再エクスポート」で段階的に行い、GUI 側テスト（`REF:tests/gui.rs`, `REF:tests/workspaces.rs`）を毎段グリーンに保つ。
- **層規則**: `REF:tests/layering.rs` に `src/app/` の禁止規則を追加する — egui / eframe / rfd / fontdb の import 禁止。書き込みは従来どおり `FsGate`（`REF:src/adapters/storage_fs.rs`）と `WritePolicy`（`REF:src/core/policy/write_policy.rs`）を経由し、書き込み関門テストの走査対象に新層を含める。
- **権限モデル**: 委譲関門は MCP ツール境界（`REF:src/core/agents/server.rs`）に集約する。人間経路は関門を迂回するが、規則 10 の制約は全経路で等しく効く。
- **原子性**: 複数ファイル操作は `Workbench::take_writes()` のバッチに一旦蓄積し、失敗時は全件を破棄する（既存の `write_finished` 失敗経路の拡張）。
- **エラーモデル**: `app::Error`（thiserror は既存依存）が分類・kind・message・path を保持する。CLI では `Exit`（`REF:src/cli/mod.rs`）、MCP では `isError` 結果、LSP では JSON-RPC コード、Python では例外クラスへ変換する。
- **国際化**: 常時コンパイル済みの `ui::i18n`（`REF:src/ui/i18n/mod.rs`）を再利用する。`--lang`・初期化オプション・`set_locale` は同一の解決関数へ合流させる。app 層から `ui::i18n` への依存は、CLI が既に持つ例外として許容する。
- **決定論**: JSON 出力のキー順はソート順へ統一し（OPS の結果シリアライズ箇所でソート済み中間表現を経由）、ゴールデンのバイト冪等を保証する。

### 19.2 Rust API

状態: 実装完了（REF:src/app/session.rs, REF:src/app/ops/mod.rs, REF:tests/api.rs, REF:tests/api_fs.rs）

- `src/app/session.rs`（新設）: `pub struct Session` が移設済み状態機械を合成する。`Session::open` / `create` は `REF:src/cli/open.rs` の起動前検証を再利用し、`with_host` はメモリホストの注入を受ける（`REF:tests/gui.rs` のモックの発想を公開 API へ昇格）。
- `src/app/ops.rs`（新設）: ディスパッチ表 `OPS: &[OpDef]`（正規操作 ID・引数スキーマ・ハンドラ・関門の有無・例外・対応パレットコマンド）。`api.list`・MCP の `inputSchema`・CLI ヘルプ・Python スタブの単一の真実となる。
- ドメインハンドルは `&mut Session` を借りる薄いラッパ（`session.section()` → `SectionOps`）とし、メソッド名は正規操作 ID の動詞と 1:1 対応。多階層ドメインは動詞側へ畳む（§19.1.3）。
- `session.command` のコマンド表は `REF:src/ui/app.rs` の `PALETTE_COMMANDS` / `SHORTCUT_PALETTE_COMMANDS` 定数を app 層へ引き上げ、GUI と API が同一の表を共有する（ドリフト排除）。
- イベントキュー: セッション内 `VecDeque<AppEvent>` と `poll_events()` / `on_event()`。LLM 呼び出し・URL 取得のワーカは `REF:src/app/agents.rs` の既存スレッド + チャネル方式を踏襲し、`cancel(handle)` は同じチャネルで中止フラグを伝える。
- 検証: `tests/api.rs`（新設・計画パス）でメモリホストによる全操作・エラー分類・イベント順序をカバーし、仕様 §19.2.1 の例は doc-test として常に実行する。

### 19.3 Python API

状態: 未実装・実装計画済み（REF:docs/add-dependencies.md, REF:docs/plans.md#stage-3）

- ルート `Cargo.toml` をワークスペース化: `members = [".", "python/novnow-py"]`、`default-members = ["."]`。既存の `cargo test --locked` と `REF:scripts/verify.sh` の挙動は不変で、Cargo.lock は 1 本のまま統合する。
- `python/novnow-py/`（新クレート）: pyo3（`abi3-py39` + `extension-module`、cdylib）。`#[pyclass] Session` が app 層を包み、メソッドは `OPS` 表からマクロ生成して手書きの写しを置かない。データ受け渡しは dict / list / str、Zip 操作は bytes。
- GIL: ブロック区間（FsGate I/O・LLM・URL 取得）は `Python::allow_threads` で解放する。Session のメソッド呼び出しは pyo3 の借用規則で直列化され、§19.2.5 の排他的所有にそのまま対応する。
- 型スタブ `novnow.pyi` と `py.typed` は `OPS` 表からビルドスクリプトで生成し、ホイールへ同梱する。
- ビルド・配布は maturin（pip で導入 — pypi.org はサンドボックスから到達可能）。pyo3-ffi は生成済みバインディングを使うため、extension-module のビルドに Python.h も libpython リンクも要らず、サンドボックスの python3（3.11）で `pyo3-build-config` のインタプリタ検出が足りる。
- pyo3 系クレートの vendoring 手順は `REF:docs/plans.md#stage-3` の §5.3（`REF:.github/workflows/sandbox-tools.yml` の Git Blob 差分追加）に従う。
- 検証: `python/tests/`（pytest、pip 導入）、`REF:scripts/verify.sh` への `--python` 段階、CI のホイール行列ジョブ（5 プラットフォーム構成）。

### 19.4 LSP（Language Server Protocol）

状態: 実装完了（REF:src/lsp/server.rs, REF:src/lsp/methods.rs, REF:src/cli/lsp.rs, REF:tests/lsp.rs）

- `src/lsp/`（新設、`cli` と同じくネイティブターゲット限定でコンパイル）: `framing.rs`（Content-Length フレーミングの手書き実装 — 追加依存ゼロ、serde_json は既存。stdio ループの構成は `REF:src/adapters/mcp_stdio.rs` を踏襲）、`server.rs`（リクエスト路由）、`methods.rs`（§19.4.2 の標準メソッド対応）、`commands.rs`（`executeCommand` → `OPS` ディスパッチ）、`convert.rs`（UTF-16 ↔ バイト位置、`REF:src/core/text/position.rs` の行表を利用）。
- 文書同期は Full: 受け取った全文を `REF:src/core/text/incremental.rs` の差分編集器で rope に適用する。単節診断は `REF:src/core/features/sections.rs` のスコープ限定機構を再利用し、`publishDiagnostics` を §7.4 の地域化メッセージで配信する。
- サーバ→クライアントは §19.4.4 の `novnow/*` 通知 4 種と `workspace/applyEdit`（リネーム・提案の適用）。通知ペイロードは §19.2.4 の `AppEvent` と同じシリアライズを共有する。
- 起動口は `REF:src/cli/mod.rs` の `lsp` サブコマンド（`mcp` と同じ cfg ゲート）。
- 検証: `tests/lsp.rs`（新設・計画パス）がスクリプト化した JSON-RPC で実 stdio フレーミングを駆動する（`REF:tests/mcp_stdio.rs` と同じ方式）。CJK とサロゲートペアの UTF-16 往復を含む。

### 19.5 MCP サーバ拡張

状態: 実装完了（REF:src/core/agents/server.rs, REF:src/cli/mcp.rs, REF:tests/cli.rs）

- `REF:src/core/agents/server.rs` のツール表を 14 件からカタログ全件へ拡張する。既存 14 件の名前・スキーマ・挙動はバイト単位で凍結（`REF:tests/mcp_stdio.rs` が回帰ガード）、新規分は app 層の `OPS` 表から生成し `api.list` との二重管理をなくす。
- 呼び出しごとのプロジェクト読み直しは現行 `run()` の方式を維持する。関門付き書き込みは既存の直接書き込み経路（E036 検証・提案のみ記録・§15.8.4 監査ログ）を再利用する。
- 安全除外の 4 操作はツール表へ登録せず、直接呼び出しには method-not-found と `isError` の二重で拒否する。
- `plot.write` の `level` 拡張（`outline` / `chapter`）は `REF:src/core/features/plots.rs` のレベル別検証を汎用化する。既存 `episode` 呼び出しの動作は不変。
- 検証: `REF:tests/mcp_stdio.rs` を拡張（新規ツール・関門挙動・除外・`tools/list` = `api.list` 一致）。

### 19.6 CLI 拡張

状態: 実装完了（REF:src/cli/mod.rs, REF:src/cli/json.rs, REF:src/cli/api.rs, REF:tests/cli.rs）

- `REF:src/cli/mod.rs`: サブコマンド解決表へ `lsp` / `api` と §19.7 の全ドメイン名を追加する。既存コマンド語が最優先（凍結）。`src/cli/` 直下に `api.rs`（`api list`）・`lsp.rs`（サーバ起動）・`json.rs`（`--json` エンベロープと `--lang` の共通処理）を新設する。
- 1 コマンド = 1 `Session`: 起動検証は `REF:src/cli/open.rs` を再利用 → 操作実行 → `take_writes()` のフラッシュ → §19.1.7 の写像で終了コードを返す。シグナル中断時はフラッシュを行わない。
- 新規文言は `REF:src/ui/i18n/ja.toml`, en.toml の両辞書へ追加し、`REF:tests/i18n_catalog.rs` が両言語のキー対応を検証する。
- 検証: `REF:tests/cli.rs` を拡張 — 既存サブコマンドのバイト不変、ドメイン動詞の挙動、`--json` ゴールデン、終了コード 0/2/3/4/5 の網羅。ゴールデンは `REF:docs/developers.md` §7.2 の生成手順に従う。

### 19.7 機能パリティカタログ

状態: 実装完了（REF:src/app/ops/mod.rs, REF:tests/api_parity.rs, REF:tests/golden/api_catalog.json）

- 契約テスト `tests/api_parity.rs`（新設・計画パス）: `REF:docs/specifications/interfaces.md` を include_str! で読み §19.7 の表を行単位で解析する（`REF:tests/diagnostics_catalog.rs` と同じ「仕様直結」方式）。各正規操作について (1) `OPS` 表、(2) CLI ディスパッチ、(3) LSP ルーティング、(4) MCP ツール（または「M—」例外）、(5) Python スタブのメソッド、の存在を検証する。例外列（R— / P— / L— / M— / C—）も解析し、「提供しない」ことも宣言どおりか検証する。
- `api.list` の機械可読出力は `tests/golden/api_catalog.json`（新設・計画パス）としてゴールデン化し、カタログの意図しないドリフトを双方向に検出する。
- 表の行の追加・削除は仕様変更にあたるため、`REF:docs/developers.md` §2 の規則により人間の承認を経る。

### 19.8 版・互換性・配布

状態: 未実装・実装計画済み（REF:src/cli/mod.rs, REF:docs/plans.md#stage-3）

- 版番号は `VERSION` 定数（`env!("CARGO_PKG_VERSION")`、`REF:src/cli/mod.rs`）を単一の真実とし、`Session::version()`・`novnow.__version__`・LSP の `serverInfo`・MCP のサーバ情報が共有する。
- 互換性の凍結は回帰テストが守る: 既存 14 ツールは `REF:tests/mcp_stdio.rs`、既存サブコマンドと終了コードは `REF:tests/cli.rs`、カタログ全体は `tests/golden/api_catalog.json`。
- 配布: ホイールのビルド・公開ジョブを `REF:.github/workflows/` 配下へ追加する。ウェブサイト（Astro、`REF:website/package.json`）には API リファレンスページを H7 で追加し、ゴールデンのカタログを静的 JSON として公開する。
