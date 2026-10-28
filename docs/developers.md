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
   - `docs/specifications/<name>.md` と `docs/designs/<name>.md` は以下の 12 ファイル対で構成し、`## ` および `### ` 見出しを完全一致させる。
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
     12. `external.md`（§20 配布形態：ブラウザ・デスクトップ・Python・VSCode 拡張・GitHub Actions）

---

## 2. 開発環境の構築手順

### 2.1 必要なツールチェーン（通常環境）

- **Rust**: `REF:rust-toolchain.toml` の `channel = "stable"`（サンドボックスの退避ツールチェーンは 1.99.0。`REF:.github/toolchain.env` の `TOOLCHAIN=stable` も同じ指定）。ターゲット `x86_64-unknown-linux-gnu` および `wasm32-unknown-unknown`、コンポーネント `rustfmt` / `clippy`。
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
   これにより `/home/user/build/`（スナップショット対象外）に現行の stable（1.99.0）、`wasm32-unknown-unknown` 標準ライブラリ、全依存クレート（`criterion` を含む約 500 クレート）と pyo3 系、`/home/user/build/env.sh` が展開される。**サンドボックスの版は固定されない**ので、CI（`dtolnay/rust-toolchain@stable`）と一致しないことがある。
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
├── rust-toolchain.toml        # ツールチェーン指定（stable / rustfmt・clippy / wasm32）
├── Trunk.toml                 # Web 版ビルド設定（REF:Trunk.toml）
├── index.html                 # Web 版エントリ HTML（REF:index.html）
├── sw.js / manifest.webmanifest  # PWA のサービスワーカーとマニフェスト
├── icon.svg                   # アプリアイコン
├── README.md / README.en.md / GUIDE.md  # 利用者向け（日本語 / 英語 / 手引き）
├── assets/
│   └── ipag.ttf               # Web / CJK フォールバック用同梱フォント（IPA ゴシック）
├── benches/
│   └── core.rs                # §18.1 性能目標の criterion ベンチマーク（REF:benches/core.rs）
├── docs/                      # 本リポジトリのドキュメント体系（§1）
│   ├── developers.md          # 本書（固定の開発者情報・ドキュメント規則）
│   ├── handoff.md             # セッション引き継ぎ情報（REF:docs/handoff.md）
│   ├── add-dependencies.md    # 依存追加時用ドキュメント
│   ├── plans.md               # 進行中の計画・受け入れ基準（REF:docs/plans.md）
│   ├── history.md             # 過去の仕様改訂・意思決定・完了履歴（追記専用、REF:docs/history.md）
│   ├── verify-rule10.md       # 規則 10（プロジェクト外に書かない）の確認手順
│   ├── script/                # duplicate.py（文書規則）/ reference.py（実装との突き合わせ）/ 各 test_*.py
│   ├── specifications/        # 純粋な仕様書（12 ファイル）
│   └── designs/               # 対応する設計書（12 ファイル）
├── examples/
│   ├── api_catalog.rs         # 正規操作カタログのゴールデン生成（REF:examples/api_catalog.rs）
│   ├── mcp_tools.rs           # MCP ツール定義のゴールデン生成（REF:examples/mcp_tools.rs）
│   ├── sample-novel/          # 付録 C のサンプルプロジェクト一式（REF:examples/sample-novel/project.toml）
│   └── manual-check/          # GUI の動作確認用プロジェクト
├── scripts/
│   ├── bootstrap-toolchain.sh # サンドボックス用 Rust ツールチェーン展開（REF:scripts/bootstrap-toolchain.sh）
│   ├── bootstrap-jpeg.sh      # 同梱 JPEG デコーダの展開
│   ├── verify.sh              # コミット前の一括検証スクリプト（REF:scripts/verify.sh）
│   ├── python-check.sh        # Python API の検査（REF:scripts/python-check.sh）
│   ├── pyi-site.py            # 型スタブの生成・照合（REF:scripts/pyi-site.py）
│   ├── api-site.py            # Web サイトの API 表の生成・照合（REF:scripts/api-site.py）
│   ├── diagnostics-site.py    # Web サイトの診断解説の生成・照合（REF:scripts/diagnostics-site.py）
│   ├── ci-report.sh           # CI の状態・注釈・PR コメントを読む（REF:scripts/ci-report.sh）
│   ├── web-smoke.sh           # Web 版ビルド＋Headless Chromium E2E 起動（REF:scripts/web-smoke.sh）
│   └── web-smoke.mjs          # Playwright E2E シナリオ本体（REF:scripts/web-smoke.mjs）
├── src/
│   ├── main.rs                # バイナリ入口（REF:src/main.rs）
│   ├── lib.rs                 # ライブラリ根（REF:src/lib.rs）
│   ├── core/                  # 純粋ドメイン層（REF:docs/designs/overview.md §3）
│   │   ├── id.rs / text/ / syntax/ / model/ / resolve/ / diagnostics.rs
│   │   ├── features/          # LSP 相当の純粋関数とワークスペース補助
│   │   ├── policy/            # 規則 10・履歴・Zip・外部変更
│   │   ├── export.rs / reader.rs / migration.rs / annotations.rs
│   │   └── aicontext/ / agents/ / template/
│   ├── ports/                 # 外部境界 trait（storage, clock, mcp, env, dialog）
│   ├── adapters/              # 環境別実装（storage_fs, storage_idb, recent_file, zip, project_load,
│   │                          #   env_system, mcp_stdio, mcp_http, clock_system, dialog_rfd）
│   ├── app/                   # 描画と無縁の状態機械と 121 操作の機能核（REF:docs/specifications/interfaces.md §19）
│   │   ├── session.rs / driver.rs / workbench.rs / save_machine.rs / settings.rs
│   │   ├── palette.rs / assist.rs / agents.rs / command.rs / commands.rs
│   │   ├── host.rs / memory_host.rs / fs_host.rs / event.rs / error.rs / workspace.rs
│   │   └── ops/               # 正規操作カタログ（project, editing, knowledge, lang, agent, session_ops）
│   ├── cli/                   # CLI サブコマンド（open, new, check, export, import, migrate, mcp, lsp, api, json）
│   ├── lsp/                   # LSP サーバ（framing, convert, methods, commands, server）
│   └── ui/                    # egui ベース GUI / Web UI（`ui` feature のときだけ）
│       ├── i18n/              # ja.toml / en.toml（GUI と CLI が共有。常にビルド）
│       ├── app.rs / picker.rs / palette_items.rs / status.rs / shell.rs / theme.rs / web.rs
│       ├── widgets/           # context_view, section_editor, reference_pane, syntax_text, inlay, portrait
│       └── workspaces/        # 9 ワークスペース（write, plot-episode, plot-outline, idea, material,
│                              #   cast, reader, agent, review）と② のタブ群（tabs, reader, stats,
│                              #   regex_search, addresses, local_llm, proposals）
├── python/
│   └── novnow-py/             # PyO3 拡張（python/novnow/__init__.pyi は自動生成）REF:python/novnow-py/README.md
├── website/                   # Astro の公式サイト（診断・API のページは仕様書から生成）
└── tests/                     # 統合テスト・アーキテクチャ境界検査・仕様照合テスト群
```

リポジトリ直下に置いてよいのは、ビルド・配布・文書・サイトに直接必要なファイルとディレクトリだけとし、動作確認用のプロジェクトは `examples/` 配下に置く。

---

## 4. スクリプトの使い方

### 4.1 `bash scripts/verify.sh`（コミット前の必須検証）

Actions取得スクリプトのオフライン回帰は `REF:scripts/action/test_install.py` で検査する。macOSでは標準Bash 3.2を明示し、空引数・終了コード・export先・バックスラッシュを含むパスのSHA-256を確認する。

`REF:scripts/verify.sh` は次を一括で実行する（`--quick` で wasm を、`--python` で Python だけに絞れる）。コミット・プッシュ前に必ず実行し、すべて通ることを確認する。

1. `python3 docs/script/duplicate.py`（ドキュメント規則・重複・`REF:` リンク・仕様/設計見出し一致の検査）
2. `python3 -B -m unittest discover -s docs/script -p 'test_*.py'`（文書検査器自身の回帰）
3. `python3 docs/script/reference.py`（文書と実装の突き合わせ。§4.6）
4. `python3 scripts/diagnostics-site.py --check`（Web サイトの診断解説が全コードを網羅すること）
5. `python3 scripts/api-site.py --check`（Web サイトの API 表が仕様 §19.7 の写しであること）
6. `bash scripts/python-check.sh`（型スタブの生成一致・構文・位置引数の束縛）
7. `cargo fmt --all --check`
8. `cargo clippy --locked --all-targets -- -D warnings`（`ui` feature なし）
9. `cargo clippy --locked --all-targets --features ui -- -D warnings`（`ui` feature あり）
10. `cargo test --locked --all-targets --no-fail-fast --features ui`（`ui` feature あり・全ターゲット）
11. `cargo test --locked --all-targets --no-fail-fast`（`ui` feature なし・全ターゲット）
12. `cargo check --locked --target wasm32-unknown-unknown --features ui`

`--python` を付けると 6 が本検査（拡張の release ビルド + pytest。pytest が無い環境では skip）に変わる。
4 と 5 は `verify.sh` では `site-check` の 1 ステップにまとまる（合計 11 ステップ・12 の検査）。
**1〜5 は CI の `docs` ジョブと同一**（§4.6 の検査 5 がこの一致を機械的に保証する）。

### 4.2 `python3 docs/script/duplicate.py`（ドキュメント規則検査）

ドキュメントのみを編集した際にも単体で高速に実行できる。終了コード `0` で全規則合格、非 `0` で違反箇所を標準エラーに出力する。

同じ「書いた内容が実装と合っているか」を、規約で決まる範囲（`REF:` の実在・見出し一致・`SYNC` の一致）だけでなく**実物との突き合わせ**でも検査する仕組みを §4.6 に示す。

### 4.3 `bash scripts/web-smoke.sh`（Web 版実機スモークテスト）

`cache/sandbox-tools` ブランチから `trunk`・`wasm-bindgen`・Playwright・Headless Chromium を `/home/user/build/web-tools` に展開し、Web 版バンドルをビルドした上で、IndexedDB・IME・オフライン（PWA）・Zip 入出力・エクスポートなどのブラウザ自動テスト（`REF:scripts/web-smoke.mjs` の `check(...)`）を実行する。UI や Web アダプタ（`REF:src/ui/web.rs`, `REF:src/adapters/storage_idb.rs`）を変更した際に実行する。

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
| `cargo check --locked --target wasm32-unknown-unknown --features ui` | 約 45〜75 秒 | 約 4〜8 秒 | `300` |
| `bash scripts/verify.sh`（11 ステップ・12 検査の通し。増分ビルドで実測 70 秒） | 約 300〜420 秒 | 約 60〜120 秒 | `900` |
| `bash scripts/web-smoke.sh`（初回展開＋WASM ビルド＋Playwright 18 件） | 約 120〜180 秒 | 約 35〜60 秒 | `600` |

### 4.5 CI レポート生成スクリプト（`.github/report.py`）の仕組み

CI の各ジョブでは、`cargo` の標準出力・標準エラーを `tee` でファイルに残したうえで `REF:.github/report.py` に渡し、**3 種類の出力** を同時に生成している。

```text
cargo clippy --message-format=json ... 2>&1 | tee /tmp/ci.log
CI_LOG=/tmp/ci.log CI_JOB=Clippy CI_KIND=clippy CI_STATUS=${{ steps.cargo.outcome }} \
  python3 .github/report.py /tmp/body.md
```

引数は **PR コメント本文の出力先パス 1 つだけ**。入力は環境変数 `CI_LOG`（`cargo` を `tee` したファイル）・`CI_JOB`（表示名）・`CI_KIND`（`fmt` / `clippy` / `test` / `wasm`）・`CI_STATUS`（`success` / `failure`）・`CI_SHA` で渡す。

1. **端末ログ（ANSI カラー付き）**: ステップのログ画面でそのまま読める整形済み出力。
2. **GitHub Annotations（`::error` / `::warning`）**: compiler JSON メッセージ（`reason == "compiler-message"`）からファイル名・行・列・診断コード・レンダリング済みメッセージを抽出し、PR の Files changed タブや Checks タブに直接インライン注釈を表示する（上限 50 件）。
3. **Job Summary（`$GITHUB_STEP_SUMMARY`）**: 各ステップの成否・所要時間・エラー抜粋・テスト実行結果（スイート別 pass / fail / ignored 件数）を Markdown テーブルとして出力する。

ローカルで動作を試す場合は次のコマンドを実行できる:

```bash
cargo test --locked --all-targets --no-fail-fast 2>&1 | tee /tmp/ci.log
CI_LOG=/tmp/ci.log CI_JOB="Local test run" CI_KIND=test CI_STATUS=success \
  python3 .github/report.py /tmp/body.md
```

（`REF:scripts/verify.sh --report` が失敗時に同じ整形を自動で行う。）

### 4.6 `python3 docs/script/reference.py`（文書と実装の突き合わせ）

`REF:docs/script/duplicate.py` が見るのは**文書の規約**（構成・重複・`REF:` の実在・見出し一致・`SYNC` の一致）である。
規約を守っていても「中身が実装とずれている」ことはあり得るので、`REF:docs/script/reference.py` が次の 13 項目を機械検査する。

| # | 検査 | 何を捕まえるか | 実例（この検査で実際に見つかったもの） |
|---|---|---|---|
| 1 | 廃止名の参照 | コード・スクリプト・CI に `SPECIFICATIONS.md` / `PLANS.md` / `DEVELOPERS.md` が残る（`Cargo.toml` と `.github/pull_request_template.md` も見る） | 分割前の文書名を指すコメント 327 箇所 |
| 2 | 仕様書の節番号 | `docs/specifications/x.md` の `§N.M` がその仕様書に無い | `interfaces.md` §19.13（新仕様では §19.1.2） |
| 3 | history の ID | `docs/history.md` と書いて `B-nnn` / `A-nn` を指すが実在しない | 節番号の付け替えで生じた幽霊参照 |
| 4 | `src/` ツリー | `REF:docs/developers.md` §3 のツリーと `src/` の実物が違う | `app/` と `lsp/` の記載漏れ |
| 5 | `verify.sh` と CI | CI が走らせる静的検査を `verify.sh` が走らせていない | Web サイトの生成物 2 検査の欠落 |
| 6 | ルート文書の重複 | `README.md` / `README.en.md` / `GUIDE.md` の間に丸写しの 3 行ブロック | README と GUIDE の機能説明 21 行 |
| 7 | 数の主張 | 「N ワークスペース」「MCP N ツール」「N の正規操作」が実装と違う | README の「8 個のワークスペース」という記述（実装は 9） |
| 8 | `include_str!` | テスト・本体が埋め込むパスが存在しない | 仕様書の改名に追随していないテスト |
| 9 | リンクとアンカー | ルート文書の相対リンク・画像、`REF:…` の `#アンカー` が飛べない | 見出しを書き換えた後の `REF:` のアンカー |
| 10 | Actions の節約 | `concurrency` の無いワークフロー、draft で走る PR ジョブ、廃止ランナー（`macos-13`）、変更フィルタの消失、`[ci skip]` フックの欠落 | `REF:.github/workflows/wheels.yml` に `concurrency` が無かった（4 OS の行列が二重に走る） |
| 11 | 公開する手引き | 仕様 §14.2.1 の 3 URL の実体、`docs/users` の参照方向、ガイドモードの段階と手引きの一致 | 生成前の `guide/publish.html` の 404（アプリのヘルプから開けなかった） |
| 12 | 配布形態 | 仕様 §20 の選択肢の数（1・10・5・1・1）、名前の形、廃止した軸（同時起動の制限・同梱物の外置き）が仕様・設計・手順・利用者向けへ戻る。数を書き写した README・`docs/users/install.md` の値、全ソースを含む最終成果物数、配布設計の廃止済み認証・設定も見る | デスクトップ版を 20 通りと書いたまま残った表（軸を廃止した直後） |
| 13 | 検証の地図 | §4.6.2 の地図に書いた検査のパス・名前が実在しない（検査一覧そのものの陳腐化） | （陳腐化の予防として追加） |

```bash
python3 docs/script/reference.py          # 検査（違反があれば終了コード 1）
python3 docs/script/reference.py --list   # 検査項目の一覧
```

**数の主張（7）を足した理由**: 文書の数は、コードを 1 行直すたびに古くなる。`REF:docs/plans.md` の `SYNC` ブロックは
節数の集計を守っているが、それ以外の本文中の数（ワークスペース数・ツール数・操作数）は誰も見ていなかった。

**リンク（9）を足した理由**: 節の改名は `duplicate.py` の `REF:` パス検査を通ってしまう（ファイルは在る）。
飛び先だけが消える壊れ方は、読むまで気づけない。

**配布形態（12）を足した理由**: 配布の選択肢は 2026-10-23 に 3 度作り直した（同時起動の制限 → 同梱物の外置き → どちらも廃止して 10 通り）。
数と軸は仕様書の 1 つの表にあるが、README・手引き・サイトへ同じ数を書き写すため、直し忘れが起きやすい。
数を仕様書の表 1 か所から読み、ほかの文書がその値を使っていることまで見る。

**`verify.sh` と CI の一致（5）を足した理由**: 「手元で `verify.sh` が緑なら CI も緑」という運用が、
CI だけに検査が増えた時点で静かに壊れるため。CI の `docs` ジョブに静的検査を足したら `verify.sh` にも足す必要が出る。

#### 4.6.1 それでも機械で見られないもの（人が確かめる）

以下は「正解」が機械的に決まらないため、この検査の対象外である。変更時は該当箇所を人が読むこと。

- **散文の意味**: 説明が現在の挙動と食い違う（例: 「右側の参照ペイン」が実際は左）かどうか。
- **仕様書の内部矛盾**: 「以下の N 項目」と表の行数が食い違う箇所（構造検査では見えない。表の行数は数えられるが、どちらが正しいかは決められない）。
- **仕様と実装のどちらを直すかの判断**: 優先順位は仕様書 > 設計書 > コード（§1）だが、どちらが誤りかは人間が決める。
- **画像・スクリーンショットの鮮度**: `REF:screenshot.png` などの画像が現在の UI と一致するか。
- **外部サイト（`REF:website/`）の文面**: 生成物（診断・API 表）は `--check` で守れるが、紹介文は手書き。

新しい検査を足すときは `REF:docs/script/reference.py` の `CHECKS` に 1 項目追加し、
`REF:docs/script/test_reference.py` に判定の単体テストを足す（検査器自身の退行を防ぐ）。

#### 4.6.2 検証の地図（領域別）

「何がどの検査で守られているか」の領域別の一覧である。検査のパスと名前はバッククォートで書き、
実在を検査 13（`check_verification_map`）が確かめる。空白地帯は隠さず書く。

| 領域 | 主な検査 | 空白地帯（機械で見ていない） |
|---|---|---|
| `src/core` | `tests/model.rs` / `tests/lexer.rs` / `tests/diagnostics_catalog.rs` / `tests/diagnostics_document.rs` / `tests/golden.rs` / `tests/layering.rs`（層・文字列の AST 検査） | 性能の回帰（ベンチは目安のみ）、網羅的な入力生成 |
| `src/app` | `tests/api.rs` / `tests/api_fs.rs` / `tests/api_driver.rs` / `tests/api_parity.rs` / `tests/api_coverage.rs` | 同時編集の競合は代表ケース中心 |
| `src/ui` | `tests/gui.rs` / `tests/workspaces.rs` / `tests/harness/mod.rs` | 実描画の見た目（§4.6.1）、実機の IME とフォーカス（§4.9） |
| CLI | `tests/cli.rs`（仕様の出力ブロックを `spec_block` で直接照合） | 実端末での対話表示 |
| LSP | `tests/lsp.rs` | 実エディタでの接続（§4.9） |
| Python | `scripts/python-check.sh` / `python/novnow-py/tests/test_binding.py` | 実機ホイールの取り込み確認（§4.9） |
| Web | `scripts/web-smoke.sh` / `scripts/web-smoke.mjs` / `tests/storage_idb.rs` | iOS Safari の実機動作（再現不可、M-6） |
| 文書 | `docs/script/duplicate.py` / `docs/script/reference.py` / `docs/script/test_reference.py` | 散文の意味のずれ（§4.6.1） |
| 配布 | `tests/distribution.rs` / `scripts/release-check.sh` / `scripts/release-matrix.py` | 市場公開の実績（発行者トークン待ち）、実機での起動（§4.9） |

---

### 4.7 Actions を無駄に使わない（既定は走らせない）

GitHub Actions は**走った分だけ**課金される。このリポジトリは CI の行列が重く
（Rust の 5 ジョブ＋ wasm ＋ bench、Python ホイールは 5 構成・デスクトップは 10 構成）、
文書やコメントだけの変更で回すと、ほぼ確実に捨てる実行にお金を払うことになる。
そこで**既定は走らせない**、かつ**プラットフォームの行列は必要な分だけ**走らせる。

| 仕組み | 実体 | 効く場面 |
|---|---|---|
| コミット既定で `[ci skip]` | `REF:.githooks/commit-msg`（`bash REF:scripts/install-git-hooks.sh` で有効化） | push・PR の全ワークフロー。`[ci run]` と書いたときだけ付かない |
| 必要なときに 1 回だけ回す | `bash REF:scripts/ci-run.sh [ci\|wheels] [watch]` | `workflow_dispatch`。push し直さずにブランチを指定して回せる。wheels / desktop-check / pages-check / vscode は `scope` 入力で full / develop を選べる（既定 full） |
| 変更フィルタ | `REF:.github/workflows/ci.yml` の `docs` ジョブ（`rust_changed`） | 文書だけの変更で Rust 5 ジョブを skip |
| `paths` / `paths-ignore` | 各ワークフロー | 無関係な変更でワークフロー自体を起動しない |
| `concurrency` | 全ワークフロー（PR は `cancel-in-progress`） | 同じブランチを続けて push したとき、古い実行を取り消す |
| draft PR では走らせない | 全ワークフローの `if: github.event_name != 'pull_request' \|\| !github.event.pull_request.draft` | 作業中の push で行列を埋めない（検査 10 が全ワークフローで見る） |
| ブランチ別の CI スコープ（`CI_SCOPE`） | wheels / desktop-check / pages-check / vscode の 4 ワークフローが PR の対象ブランチ（push は push 先ブランチ）で行列を決める。`REF:scripts/release-matrix.py --scope` | develop 向け PR で全プラットフォームの行列を走らせない（次節） |

**回す・回さないの目安**

| 変更したもの | 回すもの |
|---|---|
| 文書（`docs/`）・コードのコメント・整形だけ | 回さない（`scripts/verify.sh` を手元で回す）。PR を作る場合は必須チェックが揃わないので、`bash scripts/ci-run.sh ci watch` と `gh workflow run wheels.yml --ref <ブランチ> -f scope=develop` を各 1 回（desktop・pages・website は PR で自動実行される） |
| Rust のロジック・テスト・`Cargo.toml` / `Cargo.lock`・`src/ui/**`・`src/wasm*` | `bash scripts/ci-run.sh ci watch` |
| Web 版の画面・IndexedDB・PWA（`scripts/web-smoke.sh` で手元確認できる） | 手元で `bash scripts/web-smoke.sh`、必要なら `ci` |
| `python/**`・PyO3 のバージョン | `bash scripts/ci-run.sh wheels`（＋ `ci`） |
| ワークフロー自体・リリース前 | `ci` と `wheels` の両方 |

**所要時間の実測（2026-10-26、PR #36 と develop の実行。待ち時間短縮が主目的＝N29-01）**

| ワークフロー | 壁時計 | クリティカルパス |
|---|---|---|
| CI | 約 3.3 分 | Benchmarks が約 3.0 分（Test 約 1.8 分より長い）。bench 用とリリース用の 2 系統のコンパイルが直列だった |
| Desktop artifacts | 約 6.5 分 | Windows の GUI ビルド約 6.4 分（行列内は並行） |
| wheels | 約 5.5 分 | ホイール試験約 2.9 分（ビルドの後段） |
| VSCode Extension | 約 4.2 分 | Electron 試験（Windows）約 3.9 分 |
| Pages integration | 通常約 3.2 分 | たまにブラウザ試験が 13 分級に伸びる（上限 20 分で打ち切る設定あり） |

実測を受けて CI は次を直した（§18.4 の検証項目そのものは減らしていない）:
`bench` ジョブからリリースビルドのフレーム予算テストを `perf` ジョブへ分けて並行にし、
重複していた `cargo bench --no-run` と wasm ジョブの `cargo check`（wasm32）を畳んだ
（wasm32 のコンパイル自体は `trunk build --release` が担う）。
Desktop の Windows ビルドが全体の最長なので、PR 全体の待ち時間はそちらが決める。

**ブランチ別の CI スコープ（2026-10-27、開発速度を第一に）**: プラットフォームの行列は対象ブランチで切り替える。
`CI_SCOPE`（4 ワークフローの workflow 先頭で同じ式）は、`main` 向け PR・`main` への push で `full`、
`develop` 向け PR・`develop` への push で `develop`。`workflow_dispatch` は `scope` 入力で選ぶ（既定 `full`＝リリースリハーサル）。

| 行列 | full（main 向け PR・main push・手動実行） | develop（develop 向け PR・develop push） |
|---|---|---|
| Desktop artifacts | 10 構成（5 対象環境 × gui/nogui） | Linux x64 の nogui のみ（1 構成） |
| wheels ビルド | 5 構成 | Linux x64 のみ（1 構成。Python 試験が導入するホイールと同一） |
| wheels Python 試験 | 3.9 / 3.11 / 3.13 | 3.9 / 3.13（abi3-py39 の境界 2 版） |
| Pages ブラウザ試験 | chromium / firefox / webkit | chromium のみ（firefox / webkit は偽陽性が多発するため develop 向け PR では実行しない） |
| VSCode Electron | ubuntu / macos / windows | ubuntu のみ |

CI コア（文書整合・Rustfmt・Clippy・全テスト・WASM サイズ・ベンチマーク・性能予算・Python API 検査）
はスコープに関係なく常時実行する。develop 向け PR のプラットフォームの行列ジョブは 21 本（desktop 10・wheel 5・
Electron 3・ブラウザ 3）から 4 本（desktop 1・wheel 1・Electron 1・ブラウザ 1）になる（Python 試験は 3 版から 2 版）。
`main` への統合（リリースゲート）では full 行列が走り、macOS / Windows / arm64 のビルドはそこで検証される。

**develop への push（マージコミット）でも同じ検査を報告する**: 必須チェックの平台 2 行（wheels の Linux x64・
desktop の Linux x64 nogui）が develop 自身のコミットでも報告されるよう、wheels と desktop-check は
`develop` への push トリガーを持つ（wheels は PR と同一の `paths`、desktop-check は PR と同様に無制限。
pages・CI は元々 develop への push で走る）。コード PR のマージ後、develop 自身で 11 件の必須チェックが
揃って緑になる（文書のみ PR では Rust 5 ジョブが変更フィルタで skip され既存どおり赤くなる＝次節の落とし穴）。

**必須チェック**: `develop` の必須チェックは develop スコープで走る 11 件（2026-10-27。
旧来は full 行列を含む 23 件）。`main` は従来どおり3ブラウザ・full 行列を含む 18 件。
一覧の正本は `REF:docs/distribution.md` §8。ruleset（`protect-develop`・`protect-main`）の適用は所有者が行う。
`main` 向け PR で firefox / webkit が失敗した場合は内容を確認し、実バグに見えるものは直すか Issue 化する。

**落とし穴**: `[ci skip]` が付いたコミットが PR の頭だと、PR のチェックは**空**になる。
マージ前に `bash scripts/ci-run.sh ci watch` で 1 回だけ回し、緑を確認してからマージする
（`gh pr checks` が空なら「回していない」の意味であり、緑ではない）。
文書のみ変更の PR は wheels 行列（`paths` で起動しない）と、変更フィルタで skip された Rust 5 ジョブが
揃わないため、`ci` と `wheels`（`scope=develop`）の 2 つの dispatch を回してからマージする（上の目安表）。

検査 10 はこの仕組みの**土台が消えていないか**（`concurrency`・draft の条件・廃止ランナーの不在・
変更フィルタ・フック・`ci-run.sh`）を見る。仕組みを外すと文書検査が落ちるので、意図的に外すときは
`REF:docs/script/reference.py` の `check_actions_budget` も一緒に直す。

---

### 4.8 ユーザー向け文書（`REF:docs/users/`）

Web サイト側を変更したら `npm ci --prefix website && npm run check:site --prefix website` も実行する。
これは通常の `REF:scripts/verify.sh` に含まれない Node ビルドと生成 HTML の検証で、
`REF:.github/workflows/website.yml` と `REF:.github/workflows/deploy.yml` でも実行する。
検査範囲と開発時の base 補正は `REF:website/README.md` を参照する。


`REF:docs/specifications/` は「決まり」、`REF:docs/users/` は「使い方」である。両者は読者が違うので分ける。

| 文書 | 中身 | 読者 |
|---|---|---|
| `REF:docs/users/start.md` | はじめての手引き（30 分で 1 本書く） | 使い始める人 |
| `REF:docs/users/cheatsheet.md` | 記号とキーの早見表 | 書きながら手元に置く人 |
| `REF:docs/users/glossary.md` | 用語の定義（用語は言い換えない） | 途中で言葉に詰まった人 |
| `REF:docs/users/faq.md` | よくある質問（保存・波線・外部連携） | 困った人 |
| `REF:docs/users/project-structure.md` | 作品フォルダの中身とファイル名 | 中身を確かめたい人 |
| `REF:docs/users/git.md` | Git で履歴を残す（任意） | 上級者 |
| `REF:docs/users/publish.md` | 書き出した原稿の公開 | 公開したい人 |

**参照の向き（MUST）**

- `REF:docs/users/` から参照してよいのは `REF:docs/specifications/`（と `REF:docs/users/` 自身）だけ。
  開発者向け文書（`REF:docs/developers.md`・`REF:docs/plans.md`・`REF:docs/history.md`）は参照しない。
- 仕様書から `REF:docs/users/` を参照しない（仕様書は純粋仕様のまま）。
- 逆向きが必要に見えたら、**仕様書の節を指すのが正しい**（使い方の説明が仕様に無いなら、仕様の記述漏れを疑う）。

**書き方**

- 用語は仕様書の語をそのまま使う（「節」「場面」「モード」「役割語」）。**言い換えず、定義を足す**。
- 1 文 60 字以内、1 段落 3 文以内。漢字を詰め込まない（`REF:docs/script/easy-read.py` が機械で見る）。
- 手順は番号付きで 1 ステップ 1 動作。各手順に「うまくいかないとき」を 1〜2 行置く。
- インストールの節は `REF:docs/users/install.md` に置く。配布物が無い間は「準備中」と書き、取得先が決まったら同じ節を実値へ差し替える。ソースからのビルドは `REF:GUIDE.md` に逃がす。
- 仕様書の節を指すときは `§N.M` を添える（サイト生成がアンカーに変える）。

**サイトへの反映**

この 2 つのディレクトリがサイトの正本で、`REF:scripts/docs-site.py` が
`REF:website/src/content/docs/` と `REF:website/src/data/docs-nav.ts` を生成する。
**生成物は手で編集しない。** docs を直したらスクリプトを実行し、差分も一緒にコミットする。
`REF:scripts/verify.sh` の `site-check` が `--check` で一致を見張るので、忘れると落ちる。

    python3 scripts/docs-site.py          # 生成する（サイトのページと `guide/*.html`）
    python3 scripts/docs-site.py --check  # 一致を確かめる（CI と同じ）
    python3 docs/script/easy-read.py      # 読みやすさ（1 文 60 字・1 段落 3 文・漢字 40% 以下）

`REF:guide/*.html` は**アプリの公開サイトに置く手引き**で、同じ生成器が作る（手で編集しない）。
仕様 §14.2.1 の 3 URL がここに実在し、`REF:docs/script/reference.py` の検査 11 が CI で見張る。

**3 か所で同じ内容にする**

`REF:docs/users/start.md` の段階が、公開サイトの「はじめての手引き」とアプリ内のガイドモード
（`REF:docs/specifications/ui.md` §9.10）の元になる。段階の数・順序・文言を変えるときは 3 か所同時に直す
（`REF:docs/plans.md#plan-docs-site` の生成と、`REF:docs/plans.md#plan-guide-mode` の段階定義）。

### 4.9 実機チェックリスト（集約先）

自動検証（Linux サンドボックスと CI）では再現できない確認は、ここに集約する。
実機確認は各作業の完了条件には含めない（所有者決定 M-6）。リリース前にこの表を通し、
結果は該当する計画・既知の問題へ反映する。配布手順（`REF:scripts/release-check.sh`）の
判定とは別であり、ここから参照する形に留める。

**対象環境**: Windows / macOS / Linux（デスクトップ）、iOS Safari PWA、Android Chrome を想定する。

| 観点 | 確認すること | 由来 |
|---|---|---|
| IME | 変換候補窓の位置、確定だけが本文に入ること、節エディタへキーボードで移った直後に日本語が打てること | B-158 / §9.8.5 |
| フォーカス | 節移動を繰り返しても表示位置がずれないこと（iOS PWA＋物理キーボード） | N29-07 / KI-002 |
| フォント | 読者モード縦書きで括弧類が豆腐（欠字）にならないこと、同梱フォントとシステムフォントの差 | N29-03 |
| キー配列 | JIS 配列で記号キーのショートカット（節削除など）に届くこと、US 配列との差 | N29-05 |
| ファイルダイアログ | 開く・保存・取り込みのダイアログが OS 標準で出て、キャンセル時に何も起きないこと | §13 |
| 表示 | 高 DPI・拡大率、ダーク／ライト、狭幅（モバイル幅）での崩れ | §11 |
| 入力全般 | タッチ操作（タップで開く・選ぶ）、ソフトウェアキーボードの開閉 | §10 |
| 保存 | 外部でファイルを書き換えたときの競合表示、容量不足時の失敗表示 | §8 |
| Python | 各 OS のホイールが入って `import novnow` できること | `REF:docs/plans.md` §5.6 |
| 配布 | 各形態の成果物が実機で起動すること（配布の受け入れの未チェック分） | `REF:docs/plans.md` §7.2 |

確認で問題が見つかったら、公開版の症状は `REF:docs/known-issues/ja.md` へ載せ（§9）、
仕様・実装の直しは通常の作業として計画へ起こす。

---

## 5. ユーザーの開発の進め方（作業の作法）

公開版と現在の開発状況は `REF:docs/handoff.md` を参照する。v1 は将来の安定化目標であり、完了済みと表記しない。過去の呼称と決定の記録は `REF:docs/history.md` に隔離する。

1. **段階 ① と段階 ② の区分**:
   - **段階 ①（M0〜M11）**: コア仕様一式の実装・検証・リリース準備（完了）。
   - **段階 ②**: `REF:docs/plans.md#stage-2` に記載された追加仕様 27 項目。段階 ① で設けた 10 個の縫い代（`REF:docs/designs/overview.md` §3.3）を壊さずに段階的に実装する。
2. **ブランチと PR の運用（v0.1.0 以降は GitFlow）**:
   - `main` は死守する。公開版だけを置き、**バージョン更新を伴う release / hotfix PR 以外はマージ禁止**。通常の修正や文書更新も直接入れない。
   - `develop` は次版の統合先。通常は作業ブランチ → `develop`、リリース時は `develop` → `release/<版>` → `main` の順に PR を作る。安定化中の修正は `develop` にも戻す。
   - 緊急修正は公開版から `hotfix/<版>` を準備し、必ず版を上げて `main` と `develop` の両方へ戻す。リリースタグはマージ後の `main` 上のコミットへ付ける。
   - ブランチ保護・必須チェック・承認・公開設定は `REF:docs/distribution.md` §8 に従う。GitFlow の長期ブランチとリリース用ブランチの準備は人間が別セッションで行う。この Arena セッションのブランチ制約は優先される。
   - セッションに紐づく専用作業ブランチ（`arena/...`）でのみ作業し、別ブランチの作成や切り替えは行わない。
   - **作業の区切りごとにこまめにコミット & プッシュ**する。
   - PR の作成・更新はエージェントが行うが、**PR のマージは必ずユーザー自身が行う**（エージェントが勝手に `gh pr merge` を実行してはならない）。
   - 作業完了時は PR が Ready for review であること、コンフリクトがないこと、GitHub Actions の必須チェックが green であることを確認してから報告する（`develop` 向け PR の Firefox / WebKit ブラウザ試験は必須チェックではない。§4.7）。
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
   - 辞書最短一致、モード 8 段優先順位、役割語 5 アトム、エクスポート 14 段パイプライン、CLI 終了コード `0`〜`5`、MCP 14 ツールを検証する。
5. **GUI ヘッドレス・ブラウザ実機 E2E テスト（`REF:tests/gui.rs`, `REF:tests/workspaces.rs`, `REF:scripts/web-smoke.mjs`）**:
   - `egui::Context::run` による全 9 ワークスペース・パレット・保存状態機械・IME 状態遷移のヘッドレス検証と、Headless Chromium + Playwright による Web 版実機シナリオ検証（`REF:scripts/web-smoke.mjs` の `check(...)`）を行う。

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

### 8.1 Rust ツールチェーン・Clippy（サンドボックスは `rustc 1.99.0`）

1. **`io::Error::other(e)` の強制**: `io::Error::new(io::ErrorKind::Other, e)` は新しい Clippy で `io_other_error` 警告（`-D warnings` でエラー）になるため、常に `io::Error::other(e)` を使う。
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
29. **ロックファイル v4（`Cargo.lock`）**: 本リポジトリの `Cargo.lock` は format version 4（Rust 1.78 以降）を使用しているため、古い `cargo`（1.77 等）では `--locked` が読めない。GitHub Actions 等で扱う際は `dtolnay/rust-toolchain@stable` などの新しいツールチェーンを使う。


## 9. 公開版の既知の問題の管理

- 利用者向けの現行一覧は `REF:docs/known-issues/ja.md` と `REF:docs/known-issues/en.md` を原本とする。`REF:website/src/components/KnownIssues.astro` がMarkdownを直接表示する。Web側に本文を転記しない。
- 発見時に識別子・対象の公開版・環境・症状・回避策・対応状況を両言語で記録する。READMEには不具合本文を重複掲載しない。
- コード修正、develop/mainへのマージだけでは一覧から削除しない。修正版の公開と修正確認が完了してから、当該項目を両言語の現行一覧から削除する。公開済みの履歴はリリースノートと `REF:docs/history.md` に残す。項目がなくなった場合も一覧ページは維持する。
- ダウンロード先と公開版の表示は `REF:website/src/data/release.ts` で管理する。開発版のCargo.tomlから自動推測せず、公開済みの版・成果物を確認したときだけ更新する。サイトの見出しと告知もこの値だけを使うので、`REF:scripts/website-check.py` は見出しの版を `release.ts` と突き合わせる（`Cargo.toml` とは比べない。版更新 PR から公開までの間は意図的に食い違う）。`release.ts` の版が `Cargo.toml` より新しいときだけ誤りとする。
- 今回のv0.1.0公開告知に限り、所有者承認により版更新なしでサイト・文書とActions修正をmainへ統合する。順序は作業ブランチ→develop→mainとし、両マージは所有者が行う。タグ変更・追加リリースは行わない。通常のGitFlow規則を一般的に緩和するものではない。
