# novnow 設計書 — 外部インターフェース（ヘッドレス全機能 API 群）

本書は `REF:docs/specifications/interfaces.md` の各節に対応する実装設計を記述する。実装順序・マイルストーン・受け入れ基準は `REF:docs/plans.md#stage-3` に定める。

## 19. 外部インターフェース（ヘッドレス全機能 API 群）

5 インターフェースの共通基盤は、常時コンパイル・egui 非依存の `REF:src/app/` 層と `Session` ファサード（`REF:src/app/session.rs`）である。GUI は表示層、CLI・MCP・LSP・Python はプロトコル層として接続する。依存境界は `REF:docs/designs/overview.md` §3.3 を参照する。

### 19.1 共通原則

状態: 実装完了（REF:src/app/ops/mod.rs, REF:src/app/session.rs, REF:tests/api.rs）

- **単一実装点（H1）**: `Workbench`（`REF:src/app/workbench.rs`）、`SaveMachine`（`REF:src/app/save_machine.rs`）、`UiSettings`（`REF:src/app/settings.rs`）、エージェント実行状態（`REF:src/app/agents.rs`）、検索状態（`REF:src/app/assist.rs`）、パレットの純粋論理（`REF:src/app/palette.rs`）、`Command`（`REF:src/app/command.rs`）、`Switcher`（`REF:src/app/workspace.rs`）、`Host` トレイト（`REF:src/app/host.rs`）を app 層に置く。GUI 側の互換性は `REF:tests/gui.rs` と `REF:tests/workspaces.rs` で検査する。
- **層規則**: `REF:tests/layering.rs` に `src/app/` の禁止規則を追加する — egui / eframe / rfd / fontdb の import 禁止。書き込みは従来どおり `FsGate`（`REF:src/adapters/storage_fs.rs`）と `WritePolicy`（`REF:src/core/policy/write_policy.rs`）を経由し、書き込み関門テストの走査対象に新層を含める。
- **権限モデル**: 委譲関門は MCP ツール境界（`REF:src/core/agents/server.rs`）に集約する。人間経路は関門を迂回するが、規則 10 の制約は全経路で等しく効く。
- **原子性**: 複数ファイル操作は `Workbench::take_writes()` のバッチに一旦蓄積し、失敗時は全件を破棄する（既存の `write_finished` 失敗経路の拡張）。
- **エラーモデル**: `app::Error`（thiserror は既存依存）が分類・kind・message・path を保持する。CLI では `Exit`（`REF:src/cli/mod.rs`）、MCP では `isError` 結果、LSP では JSON-RPC コード、Python では例外クラスへ変換する。
- **国際化**: 常時コンパイル済みの `ui::i18n`（`REF:src/ui/i18n/mod.rs`）を再利用する。`--lang`・初期化オプション・`set_locale` は同一の解決関数へ合流させる。app 層から `ui::i18n` への依存は、CLI が既に持つ例外として許容する。
- **決定論**: JSON 出力のキー順はソート順へ統一し（OPS の結果シリアライズ箇所でソート済み中間表現を経由）、ゴールデンのバイト冪等を保証する。

### 19.2 Rust API

状態: 実装完了（REF:src/app/session.rs, REF:src/app/ops/mod.rs, REF:tests/api.rs, REF:tests/api_fs.rs）

- `src/app/session.rs`（新設）: `pub struct Session` が移設済み状態機械を合成する。`Session::open` / `create` は `REF:src/cli/open.rs` の起動前検証を再利用し、`with_host` はメモリホストの注入を受ける（`REF:tests/gui.rs` のモックの発想を公開 API へ昇格）。
- `REF:src/app/ops/mod.rs`: ディスパッチ表 `OPS: &[OpDef]`（正規操作 ID・引数スキーマ・ハンドラ・関門の有無・例外・対応パレットコマンド）。`api.list`・MCP の `inputSchema`・CLI ヘルプ・Python スタブの単一の真実となる。
- ドメインハンドルは `&mut Session` を借りる薄いラッパ（`session.section()` → `SectionOps`）とし、メソッド名は正規操作 ID の動詞と 1:1 対応。多階層ドメインは動詞側へ畳む（§19.1.3）。
- `session.command` のコマンド表は `REF:src/app/commands.rs` の `PALETTE_COMMANDS` / `SHORTCUT_PALETTE_COMMANDS` を使い、GUI と API が同じ表を共有する（ドリフト排除）。
- イベントキュー: セッション内 `VecDeque<AppEvent>` と `poll_events()` / `on_event()`。LLM 呼び出し・URL 取得のワーカは `REF:src/app/agents.rs` の既存スレッド + チャネル方式を踏襲し、`cancel(handle)` は同じチャネルで中止フラグを伝える。
- 検証: `REF:tests/api.rs`でメモリホストによる全操作・エラー分類・イベント順序をカバーし、基本形に対応する API 呼び出しは `REF:tests/api_fs.rs` とライブラリの doc-test で検証する。

### 19.3 Python API

状態: 実装完了（REF:python/novnow-py/src/lib.rs, REF:python/novnow-py/python/novnow/__init__.py, REF:src/app/driver.rs, REF:tests/api_driver.rs, REF:.github/workflows/wheels.yml）

- `REF:Cargo.toml` は `default-members = ["."]`、`exclude = ["python/novnow-py"]` とする。Python 拡張は独立ワークスペースと `REF:python/novnow-py/Cargo.lock` を持ち、本体の既定ビルドへ巻き込まない。
- `python/novnow-py/`（新クレート）: pyo3（`abi3-py39` + `extension-module`、cdylib）。`#[pyclass] Session` はワーカーへの汎用呼び出し口を持つ。Python のドメインメソッドは正規操作カタログを使って動的に解決し、操作本体の写しを置かない。データ受け渡しは dict / list / str、Zip 操作は bytes。
- GIL: ブロック区間（FsGate I/O・LLM・URL 取得）は `Python::allow_threads` で解放する。`REF:src/app/driver.rs` のワーカースレッドが Session を所有し、Python 側の `Mutex<Driver>` が操作を直列化する。ロック取得も GIL を解放した区間で行う（`REF:python/novnow-py/src/lib.rs`）。
- 型スタブ `REF:python/novnow-py/python/novnow/__init__.pyi` は `REF:scripts/pyi-site.py` がカタログから生成する。`REF:python/novnow-py/python/novnow/py.typed` とともにホイールへ同梱する。
- ビルド・配布は maturin（pip で導入 — pypi.org はサンドボックスから到達可能）。pyo3-ffi は生成済みバインディングを使うため、extension-module のビルドに Python.h も libpython リンクも要らず、サンドボックスの python3（3.11）で `pyo3-build-config` のインタプリタ検出が足りる。
- pyo3 系クレートの vendoring 手順は `REF:docs/plans.md#stage-3` の §5.3（`REF:.github/workflows/sandbox-tools.yml` の Git Blob 差分追加）に従う。
- 検証: `REF:python/novnow-py/tests/`（pytest、pip 導入）、`REF:scripts/verify.sh` への `--python` 段階、CI のホイール行列ジョブ（5 プラットフォーム構成）。

### 19.4 LSP（Language Server Protocol）

状態: 実装完了（REF:src/lsp/server.rs, REF:src/lsp/methods.rs, REF:src/cli/lsp.rs, REF:tests/lsp.rs）

- `src/lsp/`（新設、`cli` と同じくネイティブターゲット限定でコンパイル）: `framing.rs`（Content-Length フレーミングの手書き実装 — 追加依存ゼロ、serde_json は既存。stdio ループの構成は `REF:src/adapters/mcp_stdio.rs` を踏襲）、`server.rs`（リクエスト路由）、`methods.rs`（§19.4.2 の標準メソッド対応）、`commands.rs`（`executeCommand` → `OPS` ディスパッチ）、`convert.rs`（UTF-16 ↔ バイト位置、`REF:src/core/text/position.rs` の行表を利用）。
- 文書同期は Full: 受け取った全文を `REF:src/core/text/incremental.rs` の差分編集器で rope に適用する。単節診断は `REF:src/core/features/sections.rs` のスコープ限定機構を再利用し、`publishDiagnostics` を §7.4 の地域化メッセージで配信する。
- サーバ→クライアントは §19.4.4 の `novnow/*` 通知 4 種と `workspace/applyEdit`（リネーム・提案の適用）。通知ペイロードは §19.2.4 の `AppEvent` と同じシリアライズを共有する。
- 起動口は `REF:src/cli/mod.rs` の `lsp` サブコマンド（`mcp` と同じ cfg ゲート）。
- 検証: `REF:tests/lsp.rs`がスクリプト化した JSON-RPC で実 stdio フレーミングを駆動する（`REF:tests/mcp_stdio.rs` と同じ方式）。CJK とサロゲートペアの UTF-16 往復を含む。

### 19.5 MCP サーバ拡張

状態: 実装完了（REF:src/core/agents/server.rs, REF:src/cli/mcp.rs, REF:tests/cli.rs）

- `REF:src/core/agents/server.rs` のツール表を 14 件から「凍結 14 件 + §19.7 で「M—」の印が無い操作 72 件（`novnow.` + 操作 ID）= 86 件」へ拡張する。全件ではない: 「M—」の 35 件（状態性・安全性・バイナリ）は載せない。既存 14 件の名前・スキーマ・挙動はバイト単位で凍結（`REF:tests/golden/mcp_tools.json` が定義のゴールデン、`REF:tests/mcp_frozen.rs` が内訳と回帰ガード、`REF:tests/mcp_stdio.rs` がクライアント側の往復）、新規分は app 層の `OPS` 表から生成し `api.list` との二重管理をなくす。
- 呼び出しごとのプロジェクト読み直しは現行 `run()` の方式を維持する。関門付き書き込みは既存の直接書き込み経路（E036 検証・提案のみ記録・§15.8.4 監査ログ）を再利用する。
- 安全除外の 4 操作はツール表へ登録せず、直接呼び出しには method-not-found と `isError` の二重で拒否する。
- `plot.write` の `level` 拡張（`outline` / `chapter`）は `REF:src/core/features/plots.rs` のレベル別検証を汎用化する。既存 `episode` 呼び出しの動作は不変。
- 検証: `REF:tests/mcp_frozen.rs`（凍結 14 件のバイト一致・`inputSchema` の型検査・ツール表の内訳 86 件）と `REF:tests/mcp_stdio.rs`（往復・関門挙動・除外）。

### 19.6 CLI 拡張

状態: 実装完了（REF:src/cli/mod.rs, REF:src/cli/json.rs, REF:src/cli/api.rs, REF:tests/cli.rs）

- `REF:src/cli/mod.rs`: サブコマンド解決表へ `lsp` / `api` と §19.7 の全ドメイン名を追加する。既存コマンド語が最優先（凍結）。`src/cli/` 直下に `api.rs`（`api list`）・`lsp.rs`（サーバ起動）・`json.rs`（`--json` エンベロープと `--lang` の共通処理）を新設する。
- 1 コマンド = 1 `Session`: 起動検証は `REF:src/cli/open.rs` を再利用 → 操作実行 → `take_writes()` のフラッシュ → §19.1.7 の写像で終了コードを返す。シグナル中断時はフラッシュを行わない。
- 新規文言は `REF:src/ui/i18n/ja.toml`, en.toml の両辞書へ追加し、`REF:tests/i18n_catalog.rs` が両言語のキー対応を検証する。
- 検証: `REF:tests/cli.rs` を拡張 — 既存サブコマンドのバイト不変、ドメイン動詞の挙動、`--json` ゴールデン、終了コード 0/2/3/4/5 の網羅。ゴールデンは `REF:docs/developers.md` §7.2 の生成手順に従う。

### 19.7 機能パリティカタログ

状態: 実装完了（REF:src/app/ops/mod.rs, REF:tests/api_parity.rs, REF:tests/golden/api_catalog.json）

- 契約テスト `REF:tests/api_parity.rs`: `REF:docs/specifications/interfaces.md` を include_str! で読み §19.7 の表を行単位で解析する（`REF:tests/diagnostics_catalog.rs` と同じ「仕様直結」方式）。各正規操作について (1) `OPS` 表、(2) CLI ディスパッチ、(3) LSP ルーティング、(4) MCP ツール（または「M—」例外）、(5) Python スタブのメソッド、の存在を検証する。例外列（R— / P— / L— / M— / C—）も解析し、「提供しない」ことも宣言どおりか検証する。
- `api.list` の機械可読出力は `REF:tests/golden/api_catalog.json` としてゴールデン化し、カタログの意図しないドリフトを双方向に検出する。
- 表の行の追加・削除は仕様変更にあたるため、`REF:docs/developers.md` §5 の規則により人間の承認を経る。

- **[Issue #29 / B3・B5] `section.split` の追加と `reference.pane` の返却値拡張（実装完了: `REF:docs/plans.md#issue-29-b3`）**:
  - `section.split` を仕様 §19.7 の表の `section.confirm_next` の次へ足し（120 操作、カタログ版 1.1）、OPS・型付き Rust API（`Session::section().split`）・ゴールデン・Python スタブ・Web サイトの API 表を同時に更新した。引数は対象の節（省略時は今の節）と節先頭からのバイト位置で、意味は §8.1.1（検証: `REF:tests/api.rs` の `section_split_splits_at_a_byte_position_and_delete_takes_the_plot_too`）。
  - `reference.pane` の各行へ移動先と動作（§9.5.5）を含める変更は B5 で行った。

### 19.8 版・互換性・配布

状態: 実装完了（REF:src/app/ops/mod.rs, REF:src/cli/api.rs, REF:tests/golden/api_catalog.json, REF:scripts/api-site.py）

- 版番号は `VERSION` 定数（`env!("CARGO_PKG_VERSION")`、`REF:src/cli/mod.rs`）を単一の真実とし、`Session::version()`・`novnow.__version__`・LSP の `serverInfo`・MCP のサーバ情報が共有する。
- 互換性の凍結は回帰テストが守る: 既存 14 ツールは `REF:tests/mcp_stdio.rs`、既存サブコマンドと終了コードは `REF:tests/cli.rs`、カタログ全体は `tests/golden/api_catalog.json`。
- 配布: ホイールのビルド・公開ジョブを `REF:.github/workflows/` 配下へ追加する。ウェブサイト（Astro、`REF:website/package.json`）には API リファレンスページを H7 で追加し、ゴールデンのカタログを静的 JSON として公開する。
