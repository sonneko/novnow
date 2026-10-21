# 開発者ガイド（Developers Guide）

本書は `novnow` の開発に参加する人間および AI エージェント向けの固定情報をまとめる。現在の進捗や直近の作業状態は `REF:docs/handoff.md`、一時的な計画は `REF:docs/plans.md`、過去の意思決定やマイルストーン履歴は `REF:docs/history.md` を参照すること。

---

## 1. ドキュメント管理規則（必読・削除禁止）

以下のブロックはユーザーから指示されたドキュメント管理規則の原文であり、明示的な削除指示がない限り一字一句変更・削除してはならない。

```text
1. ドキュメント整理
ドキュメントが肥大化してきたため、整理を行う。「最初はXだったが、Yに変更した」のような歴史的情報は読む際のノイズになるため、現行の仕様や設計からは分離し、`docs/history.md` に移動する。この際、`docs/history.md` は先頭から全文を読まなくても `grep` 等で機械的に該当箇所を特定できる構造にすること。

docs/
├ script/ : ドキュメントのルールが守られているか確認するスクリプト群（`duplicate.py` を含む）
├ handoff.md : 開発を始めるにあたって必要な最小限の流動的情報（現在の進捗など）。毎セッション更新する
├ add-dependencies.md : 依存関係を追加する際に見るべき内容。基本変更しない。
├ specifications/
│ └ **.md : 複数のマークダウンファイルに分類された純粋な仕様書（DSLのEBNF、プロジェクトのディレクトリ構造、novnowの応答など、ユーザーから見える仕様のみを記述する。将来的に公開リファレンスとして利用する可能性がある）
├ designs/
│ └ **.md : 複数のマークダウンファイルに分類されたコードの設計書（ファイル構成や章立ては `specifications` と完全に一致させる）
├ plans.md : 一時的な計画や方針、途中の履歴などを記述するファイル（マイルストーン達成時に `history.md` へ移動する）
├ developers.md : 開発環境の構築手順、このプロンプトで指示しているドキュメント管理ルール、ソースコードのディレクトリ構造、スクリプトの使い方、ユーザーの開発の進め方、ArenaAgentの仕様、その他仕様外の注意点など、開発開始時に必要な固定情報
└ history.md : 変更履歴や過去の経緯などを記録するファイル（追記専用）

ドキュメント管理のルールは以下の通りとする。
・複数のドキュメントに重複した情報を記述しない。ただし、実装状況などどうしても重複が避けられない情報については、`docs/script/duplicate.py` によって機械的（決定論的）に内容の同一性が検証されている場合に限り許容する。
・矛盾が生じた場合の優先順位は「`specifications/**.md` > `designs/**.md` > ソースコード」とする。例えば、仕様書にない機能が実装されている場合は、仕様書の記述漏れか不要な実装である可能性を疑う。逆に仕様書にある機能が実装されていない場合は、単に未実装であるか、仕様変更の反映漏れの可能性がある。なお、仕様書や設計書を修正する際は、単なる記述ミスの修正を除き、人間の承認を得ること。
・`specifications/**.md` にはコードに関する情報を一切含めず、純粋な仕様のみを記述する。また、他のファイルへの参照も一切含めない。
・`designs/**.md` の構造は `specifications/**.md` と一致させる。例えば、`specifications/A.md` のB章に対応する実装は、`designs/A.md` のB章に記述する。また、各章には実装状況（実装待ち / 実装中 / 実装完了：（関連するソースコードへの参照必須））を記録する。
・他ファイルへの参照は `specifications/**.md` 以外では積極的に行ってよい。ただし、機械的に管理できるよう、すべての参照には `REF` プレフィックスを付与するなどの工夫を行うこと。
・ここで指示したドキュメント管理ルール自体も `docs/developers.md` にそのまま記載し、明示的な削除指示がない限り削除しないこと。
```

### 1.1 `REF:` 参照記法と同期ブロックの規約

1. **ファイル参照（`REF:`）**:
   - `docs/specifications/**.md` 以外のドキュメントからリポジトリ内のファイルを参照するときは、必ず `` `REF:<相対パス>` `` または `` `REF:<相対パス>#<アンカー>` `` の形式で記述する（例: `REF:src/core/id.rs`, `REF:docs/plans.md#stage-2`）。
   - `docs/designs/**.md` の各 `### ` 節の冒頭には、必ず次のいずれかの形式で実装状態を 1 行で記載する。
     - `状態: 実装完了（REF:src/...）`
     - `状態: 実装中（REF:src/...）`
     - `状態: 未実装・実装計画済み（REF:src/..., REF:docs/plans.md#...）`
     - `状態: 未実装・未計画（REF:docs/plans.md#unplanned-large）`
     - `状態: 実装待ち（段階 ② 対象: REF:docs/plans.md#stage-2）`
2. **重複許可ブロック（`SYNC` マーカー）**:
   - `REF:docs/handoff.md` と `REF:docs/plans.md` の間で共有するマイルストーン達成状況など、重複が避けられない箇所は `<!-- SYNC:START <id> -->` と `<!-- SYNC:END <id> -->` で囲み、`REF:docs/script/duplicate.py` でバイト単位の一致を検証する。
3. **仕様書と設計書の対応表**:
   - `docs/specifications/<name>.md` と `docs/designs/<name>.md` は以下の 11 ファイル対で構成し、`## ` および `### ` 見出しを完全一致させる。
     1. `overview.md`（§1 設計原則・§2 用語・§3 アーキテクチャと処理モデル）
     2. `project.md`（§4 プロジェクト構造・付録 C サンプルプロジェクト）
     3. `syntax.md`（§5 ファイル形式）
     4. `semantics.md`（§6 意味解決エンジン・付録 B 役割語パターン言語）
     5. `diagnostics.md`（§7 診断カタログ）
     6. `editing.md`（§8 編集モデルと保存）
     7. `ui.md`（§9 画面仕様・§10 ワークスペース・§11 テーマ・§12 国際化）
     8. `platform.md`（§13 ストレージ・§14 起動仕様）
     9. `agent.md`（§15 AI エージェント・付録 A 既定プロンプト）
     10. `workflows.md`（§16 資料収集・§17 エクスポート・§18 性能・品質要件）
     11. `interfaces.md`（§19 外部インターフェース：ヘッドレス全機能 API 群）

---

## 2. 開発環境の構築手順

### 2.1 必要なツールチェーン（通常環境）

- **Rust**: `.github/toolchain.env`（`REF:.github/toolchain.env`）で指定されたバージョン（`1.99.0`）、ターゲット `x86_64-unknown-linux-gnu` および `wasm32-unknown-unknown`、コンポーネント `rustfmt` / `clippy`。
- **Linux システムライブラリ（`ui` feature ビルド時）**:
  - `libxcb-render0-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`, `libxkbcommon-dev`, `libssl-dev`, `libgtk-3-dev`
- **Web ビルド・スモークテスト時**:
  - `trunk`（`0.21.14`）、`wasm-bindgen-cli`（`0.2.118`）、`nodejs` / `playwright` / Headless Chromium。

### 2.2 Arena サンドボックス環境でのツールチェーン復旧

Arena サンドボックスには初期状態で `rustc` / `cargo` が入っておらず、`static.rust-lang.org` への通信も遮断されている。その代わり、リポジトリ自身の `cache/rust-toolchain` ブランチと `cache/sandbox-tools` ブランチにツールチェーン・vendored crates・ブラウザ試験ツール一式が退避されている。

1. **Rust ツールチェーンと vendored crates の展開（約 30 秒）**:
   ```bash
   bash scripts/bootstrap-toolchain.sh
   ```
   これにより `/home/user/build/`（スナップショット対象外）に Rust 1.99.0、`wasm32-unknown-unknown` 標準ライブラリ、全依存クレート（`criterion` を含む 493 クレート）、`/home/user/build/env.sh` が展開される。
2. **各コマンド実行時の環境有効化**:
   `bash` ツール呼び出しは毎回新しいシェルで起動するため、Rust コマンドを実行する際は必ず先頭に `. /home/user/build/env.sh` を付ける。
   ```bash
   . /home/user/build/env.sh && cargo test --locked --all-targets --no-fail-fast
   ```

---

## 3. ソースコードのディレクトリ構造

```text
novnow/
├── Cargo.toml                 # クレートマニフェスト（REF:Cargo.toml）
├── Cargo.lock                 # 依存ロックファイル（v4）
├── Trunk.toml                 # Web 版ビルド設定（REF:Trunk.toml）
├── index.html                 # Web 版エントリ HTML（REF:index.html）
├── assets/
│   └── ipag.ttf               # Web / CJK フォールバック用同梱フォント（IPA ゴシック）
├── benches/
│   └── core.rs                # §18.1 性能目標の criterion ベンチマーク（REF:benches/core.rs）
├── docs/
│   ├── developers.md          # 本書（固定の開発者情報・ドキュメント規則）
│   ├── handoff.md             # セッション引き継ぎ情報（REF:docs/handoff.md）
│   ├── add-dependencies.md    # 依存追加時用ドキュメント
│   ├── plans.md               # 段階 ② 計画・進行中マイルストーン（REF:docs/plans.md）
│   ├── history.md             # 過去の仕様改訂・意思決定・完了履歴（追記専用、REF:docs/history.md）
│   ├── script/
│   │   └── duplicate.py       # ドキュメント管理規則・重複・REF 検査スクリプト（REF:docs/script/duplicate.py）
│   ├── specifications/        # 純粋な仕様書（11 ファイル）
│   └── designs/               # 対応する設計書（11 ファイル）
├── examples/
│   └── sample-novel/          # 付録 C のサンプルプロジェクト一式（REF:examples/sample-novel/project.json）
├── scripts/
│   ├── bootstrap-toolchain.sh # サンドボックス用 Rust ツールチェーン展開（REF:scripts/bootstrap-toolchain.sh）
│   ├── verify.sh              # コミット前の一括検証スクリプト（REF:scripts/verify.sh）
│   ├── web-smoke.sh           # Web 版ビルド＋Headless Chromium E2E 起動（REF:scripts/web-smoke.sh）
│   └── web-smoke.mjs          # Playwright E2E シナリオ本体（REF:scripts/web-smoke.mjs）
├── src/
│   ├── main.rs                # バイナリ入口（REF:src/main.rs）
│   ├── lib.rs                 # ライブラリ根（REF:src/lib.rs）
│   ├── cli/                   # CLI サブコマンド（open, new, check, export, mcp）
│   ├── core/                  # 純粋ドメイン層（id, text, syntax, model, resolve, diagnostics, features, policy, export, template, aicontext, agents）
│   ├── ports/                 # 外部境界 trait（storage, clock, mcp, env, dialog）
│   ├── adapters/              # 環境別実装（storage_fs, storage_idb, recent_file, zip, project_load, env_system, mcp_stdio, clock_system, dialog_rfd）
│   └── ui/                    # egui ベース GUI / Web UI（i18n, app, web, shell, workbench, save_machine, settings, palette, assist, agents, theme, widgets, workspaces）
└── tests/                     # 統合テスト・アーキテクチャ境界検査・仕様照合テスト群
```

---

## 4. スクリプトの使い方

### 4.1 `bash scripts/verify.sh`（コミット前の必須検証）

`REF:scripts/verify.sh` は次を一括で実行する。コミット・プッシュ前に必ず実行し、すべて通ることを確認する。

1. `python3 docs/script/duplicate.py`（ドキュメント規則・重複・`REF:` リンク・仕様/設計見出し一致の検査）
2. `cargo fmt --all -- --check`
3. `cargo clippy --locked --all-targets -- -D warnings`（`ui` feature なし）
4. `cargo clippy --locked --all-targets --features ui -- -D warnings`（`ui` feature あり）
5. `cargo test --locked --all-targets --no-fail-fast`（`ui` feature なし）
6. `cargo test --locked --all-targets --features ui --no-fail-fast`（`ui` feature あり）
7. `cargo check --locked --target wasm32-unknown-unknown --no-default-features --features web`

### 4.2 `python3 docs/script/duplicate.py`（ドキュメント規則検査）

ドキュメントのみを編集した際にも単体で高速に実行できる。終了コード `0` で全規則合格、非 `0` で違反箇所を標準エラーに出力する。

### 4.3 `bash scripts/web-smoke.sh`（Web 版実機スモークテスト）

`cache/sandbox-tools` ブランチから `trunk`・`wasm-bindgen`・Playwright・Headless Chromium を `/home/user/build/web-tools` に展開し、Web 版バンドルをビルドした上で 18 シナリオのブラウザ自動テスト（`REF:scripts/web-smoke.mjs`）を実行する。UI や Web アダプタ（`REF:src/ui/web.rs`, `REF:src/adapters/storage_idb.rs`）を変更した際に実行する。

### 4.4 サンドボックス環境での所要時間目安

`bash` ツールの `timeout` 設定（デフォルト 30s、最大 1800s）を決める際の目安:

| コマンド | 初回（コールドビルド） | 2 回目以降（インクリメンタル） | 推奨 `timeout` |
| --- | --- | --- | --- |
| `bash scripts/bootstrap-toolchain.sh` | 約 25〜30 秒 | 約 1 秒（展開済みスキップ） | `120` |
| `cargo fmt --all -- --check` | 約 1 秒 | 約 1 秒 | `30` |
| `cargo clippy --locked --all-targets -- -D warnings` | 約 20〜35 秒 | 約 3〜6 秒 | `180` |
| `cargo clippy --locked --all-targets --features ui -- -D warnings` | 約 60〜90 秒 | 約 5〜10 秒 | `300` |
| `cargo test --locked --all-targets --no-fail-fast` | 約 30〜50 秒 | 約 8〜15 秒 | `300` |
| `cargo test --locked --all-targets --features ui --no-fail-fast` | 約 60〜110 秒 | 約 15〜25 秒 | `300` |
| `cargo check --locked --target wasm32-unknown-unknown --no-default-features --features web` | 約 45〜75 秒 | 約 4〜8 秒 | `300` |
| `bash scripts/verify.sh`（全 7 ステップ通し） | 約 150〜240 秒 | 約 35〜60 秒 | `600` |
| `bash scripts/web-smoke.sh`（初回展開＋WASM ビルド＋Playwright 18 件） | 約 120〜180 秒 | 約 35〜60 秒 | `600` |

### 4.5 CI レポート生成スクリプト（`.github/report.py`）の仕組み

CI の各ジョブでは、`cargo` の標準出力・標準エラーをそのままログに流すだけでなく、`REF:.github/report.py` にパイプして **3 種類の出力** を同時に生成している。

```text
cargo clippy --message-format=json ... 2>&1 \
  | python3 .github/report.py clippy --title "Clippy (all targets)"
```

1. **端末ログ（ANSI カラー付き）**: ステップのログ画面でそのまま読める整形済み出力。
2. **GitHub Annotations（`::error` / `::warning`）**: compiler JSON メッセージ（`reason == "compiler-message"`）からファイル名・行・列・診断コード・レンダリング済みメッセージを抽出し、PR の Files changed タブや Checks タブに直接インライン注釈を表示する（上限 50 件）。
3. **Job Summary（`$GITHUB_STEP_SUMMARY`）**: 各ステップの成否・所要時間・エラー抜粋・テスト実行結果（スイート別 pass / fail / ignored 件数）を Markdown テーブルとして出力する。

ローカルで動作を試す場合は次のコマンドを実行できる:

```bash
cargo test --locked --all-targets --no-fail-fast 2>&1 \
  | python3 .github/report.py test --title "Local test run"
```

---

## 5. ユーザーの開発の進め方（作業の作法）

1. **段階 ① と段階 ② の区分**:
   - **段階 ①（M0〜M11）**: コア仕様一式の実装・検証・リリース準備（完了）。
   - **段階 ②**: `REF:docs/plans.md#stage-2` に記載された追加仕様 27 項目。段階 ① で設けた 10 個の縫い代（`REF:docs/designs/overview.md` §3.3）を壊さずに段階的に実装する。
2. **ブランチと PR の運用**:
   - セッションに紐づく専用作業ブランチ（`arena/...`）でのみ作業し、別ブランチの作成や切り替えは行わない。
   - **作業の区切りごとにこまめにコミット & プッシュ**する。
   - PR の作成・更新はエージェントが行うが、**PR のマージは必ずユーザー自身が行う**（エージェントが勝手に `gh pr merge` を実行してはならない）。
   - 作業完了時は PR が Ready for review であること、コンフリクトがないこと、GitHub Actions の全チェックが green であることを確認してから報告する。
3. **自律判断と人間承認の境界**:
   - 仕様書（`docs/specifications/**.md`）および設計書（`docs/designs/**.md`）で既に決まっている事項の実装・テスト・バグ修正は確認を求めず自律的に進める。
   - 仕様書や設計書の内容自体を変更する場合は、単なる記述ミスの修正を除き、人間の承認を得ること。

---

## 6. ArenaAgent の仕様と環境上の制約

1. **通信ホワイトリスト**:
   - サンドボックスから外部通信が可能なホストは `github.com`, `codeload.github.com`, `api.github.com`, `registry.npmjs.org`, `pypi.org`, `files.pythonhosted.org` の 6 つに限定される。
   - `crates.io` / `static.crates.io` / `dl.google.com` 等は直接到達できないため、新しいクレートやバイナリがどうしても必要な場合は `.github/workflows/sandbox-tools.yml`（`REF:.github/workflows/sandbox-tools.yml`）等の GitHub Actions ワークフロー経由で `cache/*` ブランチに退避する。
2. **コンテナ再起動とスナップショットの挙動**:
   - セッションが長くなるとターン間でサンドボックスのコンテナが再起動することがある。
   - スナップショットで永続化されるのは `/home/user/novnow` 配下のみであり、除外ディレクトリ（`target/`, `dist/`, `build/`, `node_modules/`, `.cache/` 等）および `/home/user/build/` は消える。
   - コンテナ再起動時は `.git/refs/heads/...` が古い状態に戻ることがあるため、必ず次の手順で復旧する:
     ```bash
     bash scripts/bootstrap-toolchain.sh
     git fetch origin <現在のブランチ名>
     git reset --mixed FETCH_HEAD
     ```
3. **パッチセット上限（128MB / 10,000 ファイル）**:
   - ターン終了時の累積成果物スナップショットには約 128MB または 10,000 ファイルの上限がある。大きな一時ファイルや成果物は必ず `/home/user/build/` または `/tmp/`、もしくは除外ディレクトリ名（`target/`, `dist/` 等）の下に置き、リポジトリ内の追跡対象外パスに放置しない。
4. **GitHub CLI（`gh`）での PR 更新**:
   - `gh pr edit` は GitHub 側の Classic Projects 廃止エラー（`Projects (classic) is being deprecated`）で失敗することがあるため、PR のタイトルや本文を更新する際は REST API を直接呼ぶ:
     ```bash
     gh api repos/sonneko/novnow/pulls/<PR番号> -X PATCH --input - <<'JSON'
     {"title": "...", "body": "..."}
     JSON
     ```

---

## 7. テスト戦略・ハーネス・ヘッドレス描画ガイド

### 7.1 テスト戦略と層別の責務

テストは `tests/` 配下に機能別の統合テストとして配置し、`src/` 内の単体テストと合わせて以下の 5 層で検証する:

1. **アーキテクチャ・不変条件検査（`REF:tests/layering.rs`, `REF:tests/no_llm_sdk.rs`）**:
   - `syn` による AST 走査で `core` の純粋性（`egui` / `std::fs` / `std::process` / `std::time::SystemTime` 禁止）、書き込み単一関門 `FsGate`、日本語文字列リテラル禁止（`/// i18n: format-vocabulary` を除く）、`unsafe` の Windows FFI 2 箇所限定、クレート許可リストを検証する。
2. **仕様書直結の契約テスト（`REF:tests/diagnostics_catalog.rs`, `REF:tests/i18n_catalog.rs`, `REF:tests/storage_idb.rs`, `REF:tests/template.rs`）**:
   - `include_str!` で `docs/specifications/*.md` の表やコードブロックを直接パースし、診断コード・重大度・引数・IndexedDB ストア定義・テンプレート構成が仕様書と一字一句一致することを検証する。
3. **構文・無損失往復・ゴールデンテスト（`REF:tests/roundtrip.rs`, `REF:tests/golden.rs`, `REF:tests/contents.rs` 他）**:
   - 全 7 ファイル形式の `parse(text).to_text() == text`（BOM / CRLF 正規化後）と、入力フィクスチャに対する構文木・診断の完全一致を検証する。
4. **意味解決・機能・CLI・MCP テスト（`REF:tests/dictionary.rs`, `REF:tests/modes.rs`, `REF:tests/role_language.rs`, `REF:tests/export_invariants.rs`, `REF:tests/cli.rs`, `REF:tests/mcp_stdio.rs` 他）**:
   - 辞書最短一致、モード 8 段優先順位、役割語 5 アトム、エクスポート 14 段パイプライン、CLI 終了コード `0`〜`5`、MCP 11 ツールを検証する。
5. **GUI ヘッドレス・ブラウザ実機 E2E テスト（`REF:tests/gui.rs`, `REF:tests/workspaces.rs`, `REF:scripts/web-smoke.mjs`）**:
   - `egui::Context::run` による全 9 ワークスペース・パレット・保存状態機械・IME 状態遷移のヘッドレス検証と、Headless Chromium + Playwright による Web 版実機 18 シナリオ検証を行う。

### 7.2 ゴールデンテスト（`tests/golden/`）の運用規則

- `tests/golden/` 配下の期待出力ファイルは**手書き・手動編集を禁止**し、必ずテストコードの確定ロジックから生成した出力を人間が diff レビューした上でコミットする。
- 構文や診断コードの仕様変更を行わない限り、既存ゴールデン出力の差分が発生した場合は退行バグとして扱う。

### 7.3 GUI テストハーネス（`Harness` / `MemoryHost` / `MemoryFiles`）

`REF:tests/gui.rs` および `REF:tests/workspaces.rs` では、OS ウィンドウや GPU を一切使わずに `egui::Context::run` で `NovnowApp` を駆動するヘッドレスハーネスを使用している。

- **`MemoryFiles` / `MemoryHost`**: ファイルシステム・クリップボード・時刻・MCP 実行をすべてオンメモリで模擬する `AppHost` 実装。
- **2 フレームウォームアップ規則**: `egui` のモーダル（`Window` / `Area`）や `request_focus()`、レイアウト矩形計算は初回フレームのパス完了後に確定するため、パレット開閉やキー入力のテストでは `step_with` の前後で 1〜2 フレーム回してからアサーションを行う。

### 7.4 ヘッドレス PNG スクリーンショット出力（`NOVNOW_SHOT`）

GUI の見た目（パレット位置、下線・波線の座標、枠線やバッジの重なりなど）を視覚的に確認したい場合は、`egui` の `Tessellation` 出力をソフトウェアラスタライズして PNG に書き出すテストヘルパを利用できる。

```bash
. /home/user/build/env.sh && NOVNOW_SHOT=/tmp/shot.png cargo test --locked --features ui --test gui renders_screenshot -- --exact --nocapture
```

生成された `/tmp/shot.png` を `read_file` ツールで読み込むことで、サンドボックス内でも実際の描画結果を直接目視確認できる。

---

## 8. 仕様外の注意点（技術的な落とし穴一覧）

これまでの開発（M0〜M11 および Web 実機検証）で実際に遭遇した落とし穴と対処法を領域別に記録する。

### 8.1 Rust ツールチェーン・Clippy（`rustc 1.99.0`）

1. **`io::Error::other(e)` の強制**: `io::Error::new(io::ErrorKind::Other, e)` は Rust 1.99 の Clippy で `io_other_error` 警告（`-D warnings` でエラー）になるため、常に `io::Error::other(e)` を使う。
2. **`collapsible_if` の厳格化**: ネストした `if let` や `if a { if b { ... } }` は `if a && b` または let-chains（`if let Some(x) = ... && cond`）にまとめる。
3. **`needless_range_loop`**: 添字 `i` を配列アクセス `arr[i]` にしか使わない `for i in 0..n` は `arr.iter().enumerate()` に書き換える。
4. **`DoubleEndedIterator` 未実装イテレータの `.rev()`**: カスタムイテレータで末尾から走査したい場合は一度 `collect::<Vec<_>>()` してから `.into_iter().rev()` を使う。

### 8.2 WASM ターゲット（`wasm32-unknown-unknown`）と `cfg` 分岐

5. **CLI 専用モジュールの `cfg` ガード**: `src/cli/**` や `adapters::mcp_stdio`、`adapters::storage_fs` など OS プロセス・ファイルシステムを使うモジュールは `#[cfg(not(target_arch = "wasm32"))]` でガードし、`wasm32` ビルドに混入させない。
6. **`rfd 0.14` の同期 API 非対応**: `rfd::FileDialog`（同期 API）は `wasm32` に存在しないため、Web 版のファイル取込・保存は `REF:src/ui/web.rs` 内の非同期 `<input type="file">` / Blob ダウンロードブリッジを使用する。
7. **`Instant` の非対応**: `std::time::Instant` は `wasm32-unknown-unknown` でパニックするため、経過時間やデバウンス計算は `ports::clock::Clock` または `egui::InputState::time`（秒単位 `f64` → ミリ秒 `u64`）を経由する。

### 8.3 レイヤリング・アーキテクチャ検査（`REF:tests/layering.rs`）

8. **`macro_rules!` 内の `/// i18n: format-vocabulary` 属性の罠**: マクロ定義内部のアイテムに付けた `///` doc コメントは展開後の内側アイテムの属性になり、マクロを呼び出す外側の関数の属性にはならない。`tests/layering.rs` の AST 検査を通すには、マクロ呼び出しを含む**外側の関数や定数自体**に `/// i18n: format-vocabulary` を付与する。
9. **文字列リテラル内の全角記号**: `src/core/**`・`src/cli/**`・`src/ui/**` では、全角括弧 `「」` や `…`、`—` を含むリテラルも CJK / 全角文字検査に引っかかる場合があるため、表示用文言はすべて `ja.toml` / `en.toml` に置き、構文記号定数には `/// i18n: format-vocabulary` を付与する。
10. **テストコードからのファイル書き込み**: `src/` 内で `std::fs::write` / `OpenOptions::write` 等を呼べるのは `REF:src/adapters/storage_fs.rs` のみである。新しい保存処理を追加する際は必ず `FsGate` を経由させる。

### 8.4 パーサ・字句解析の注意点

11. **インデント継続行とシジル優先順位（`B-18`, `B-21`）**: 空白 2 文字以上またはタブ 1 文字以上で始まる行は、行頭が `@` や `;` であってもシジル行ではなく継続行（C 形式では字下げされた本文行）として扱う。
12. **URL スキームとフィールド行の非衝突（`B-24`）**: `https://...` や時刻 `10:30` をフィールド行やコメントと誤認しないよう、半角 `:` の直後に空白がない場合は既知フィールドキー表に含まれる識別子のみをフィールド行と判定し、直前が `:` の `//` はコメント開始と見なさない。
13. **行末コメントと無損失往復（`A-14`, `B-36`）**: フィールド値の後ろの `//` コメントを AST 構築時に切り落とすと `to_text()` で消えてしまうため、構文木には行末コメント込みで保持し、値を解釈するアクセサ（`Field::value_clean()` 等）で除去する。

### 8.5 名前解決・モード・役割語・横断検査の注意点

14. **`scan_line` の行内相対オフセットと診断のファイル絶対オフセット（`B-56`）**: `dictionary::scan_line` が返す `RefToken.range` は**行頭からの相対 byte offset** であり、`Diagnostic.range` は**ファイル先頭からの絶対 byte offset** である。呼び出し側で `line_start` の加算漏れ・二重加算に注意する。
15. **NFKC 正規化と長音記号統一の順序（`A-19`, `B-57`）**: 全角マイナス `－`（U+FF0D）は NFKC 正規化で ASCII ハイフン `-`（U+002D）に変わるため、NFKC の**前**に長音統一を行わないと `－` → `ー` の同一視が機能しない。
16. **`Characters::character_of` の引数**: `character_of(line_idx)` の引数は `characters` ベクタの添字ではなく**行番号（0 基点）**である。`modes::Target.character`（キャラ添字）を渡すと別のキャラを参照してしまうため混同しないこと。
17. **役割語パターンのワイルドカード下線除外（`A-21`, `B-68`）**: 役割語一致の緑下線はパターン全体ではなく、ワイルドカード `*` / `〜` が消費した自由テキスト範囲を除いた固定語・語クラス部分のみに引く。

### 8.6 ストレージ・ポリシー・MCP・CLI の注意点

18. **`WritePolicy` のデバッグビルドパニック（`B-20`, `B-107`）**: `WritePolicy::check` はデバッグビルド（`cargo test` を含む）では違反時に `debug_assert!` でパニックする仕様である。ポリシー違反時の `Err(WriteViolation)` 返却をテストする際は `WritePolicy::check_non_panicking` または専用の検証経路を使う。
19. **末尾 `;` と仮想空節スライス（`B-112`）**: C 形式パーサはファイル末尾の `;` 直後に空節を作らない（`W012` 回避）一方、GUI の節分割（`Ctrl+Enter`）では末尾に `;` を挿入した直後に新しい空節へ移動できる必要があるため、`sections::slices` が末尾 `;` の後ろに仮想スライスを 1 個補う設計になっている。

### 8.7 GUI（`egui 0.27.2`）の注意点

20. **`consume_key` の修飾キー順序（`B-132`）**: `egui::InputState::consume_key(Modifiers::CTRL, Key::D)` は Shift や Alt が同時に押されていてもマッチしてしまうことがあるため、`Ctrl+Shift+D` や `Ctrl+Alt+...` の判定を単一 `Ctrl+...` の判定より**必ず先**に置く。
21. **Web 版でのプロジェクト選択画面と `F2` キー**: `Key::F2` はプロジェクトを開いている状態（`Stage::Open` かつオーバーレイなし）では識別子の一括改名（`Command::Rename`）に使い、Web のプロジェクト選択画面（`Stage::Picker`）では選択中プロジェクトの名称変更に使う。`REF:src/ui/shell.rs` で無条件に `F2` を消費すると `Stage::Picker` に届かなくなるため、`REF:src/ui/app.rs` 側でステージを見て振り分ける。
22. **折り返し行末での下線終端 X 座標**: `Galley::cursor_from_ccursor` にスパン終端文字インデックスを渡すと、その文字が折り返し行末のとき次の行の先頭（`x = 0`）が返り、下線の幅が 0 または逆転する。下線の終端 X は必ずスパン内の最後のグリフの右端 `glyph.pos.x + glyph.size.x` から算出する。
23. **特殊記号 `⟨⟩`（U+27E8 / U+27E9）のフォントフォールバック**: `assets/ipag.ttf`（IPA ゴシック）単体には数学山括弧 `⟨` `⟩` のグリフが含まれないため、`REF:src/ui/theme.rs` では CJK フォントの後ろに egui 標準の `Hack` 等幅フォントをフォールバック維持している。`family.clear()` で既存フォントを全消去してはならない。
24. **egui 0.29.x へのアップグレード禁止（`R-3`）**: `egui 0.29.x` には CJK IME 変換確定文字が消失・重複する既知バグ（`emilk/egui#5544`）があるため、将来アップグレードする場合は 0.29 系を飛ばして 0.31 以降へ移行する。

### 8.8 Web 版・Playwright E2E（`REF:scripts/web-smoke.sh`）の注意点

25. **Web 版（eframe 0.27 / egui-winit）でのキーボード節移動と IME フォーカス**: Web 版の `eframe 0.27` はブラウザ上の隠し `<input id="egui_text_agent">` を使って文字入力と IME イベントを受け取る。キーボードショートカット（`Alt+↓` / `Alt+↑` / `Ctrl+P` → `Enter` 等）でフォーカス先ウィジェット ID が変わったフレームでは、ブラウザ側の隠し `<input>` が blur されたままになることがあるため、`REF:src/ui/web.rs` の `sync_text_agent` で毎フレーム `egui_text_agent.focus()` を同期している。削除してはならない。
26. **Trunk 0.21 の `data-trunk` アセットパス解決**: `REF:index.html` 内の `<link data-trunk ... href="...">` は `index.html` からの相対パスで解決されるため、先頭に `/` を付けず `href="assets/..."` と記述する。
27. **IndexedDB 非同期トランザクションと描画フレーム**: Web 版でのプロジェクト切替・Zip インポート完了通知は非同期コールバックからキュー経由で次フレームの `NovnowApp::update` に渡されるため、Playwright テスト側では状態反映まで `requestAnimationFrame` 2 回分またはステータスバー変化を待機する。

### 8.9 GitHub CLI / Git 操作の注意点

28. **`gh pr edit` の Classic Projects 廃止エラー**: `gh pr edit` は GraphQL の Classic Projects 廃止エラーで失敗するため、PR のタイトル・本文更新には `gh api repos/sonneko/novnow/pulls/<PR番号> -X PATCH --input -` を使用する。
29. **ロックファイル v4（`Cargo.lock`）**: 本リポジトリの `Cargo.lock` は format version 4（Rust 1.99.0）を使用しているため、古い `cargo`（1.77 等）では `--locked` が読めない。GitHub Actions 等で扱う際は `.github/toolchain.env` のバージョンまたは `curl` による直接取得を使う。

