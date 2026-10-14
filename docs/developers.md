# 開発者ガイド（Developers Guide）

本書は `novnow` の開発に参加する人間および AI エージェント向けの固定情報をまとめる。現在の進捗や直近の作業状態は `REF:docs/handoff.md`、一時的な計画は `REF:docs/plans.md`、過去の意思決定やマイルストーン履歴は `REF:docs/history.md` を参照すること。

---

## 1. ドキュメント管理規則（必読・削除禁止）

以下のブロックはユーザーから指示されたドキュメント管理規則の原文であり、明示的な削除指示がない限り一字一句変更・削除してはならない。

```text
1. ドキュメント整理
ドキュメントが肥大化してきたため、整理を行う。「最初はXだったが、Yに変更した」のような歴史的情報は読む際のノイズになるため、現行の仕様や設計からは分離し、`docs/history.md` に移動する。この際、`docs/history.md` は先頭から全文を読まなくても `grep` 等で機械的に該当箇所を特定できる構造にすること。
また、現在ルートディレクトリにある `DEVELOPERS.md`、`PLANS.md`、`SPECIFICATIONS.md` は廃止し、以下の構成に移行する。

docs/
├ script/ : ドキュメントのルールが守られているか確認するスクリプト群（`duplicate.py` を含む）
├ handoff.md : 開発を始めるにあたって必要な最小限の流動的情報（現在の進捗など）。毎セッション更新する
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
   - `docs/specifications/<name>.md` と `docs/designs/<name>.md` は以下の 10 ファイル対で構成し、`## ` および `### ` 見出しを完全一致させる。
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
│   ├── plans.md               # 段階 ② 計画・進行中マイルストーン（REF:docs/plans.md）
│   ├── history.md             # 過去の仕様改訂・意思決定・完了履歴（追記専用、REF:docs/history.md）
│   ├── script/
│   │   └── duplicate.py       # ドキュメント管理規則・重複・REF 検査スクリプト（REF:docs/script/duplicate.py）
│   ├── specifications/        # 純粋な仕様書（10 ファイル）
│   └── designs/               # 対応する設計書（10 ファイル）
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

## 7. 仕様外の注意点（技術的な落とし穴）

1. **Web 版（eframe 0.27 / egui-winit）でのキーボード節移動と IME フォーカス**:
   - Web 版の `eframe 0.27` はブラウザ上の隠し `<input id="egui_text_agent">` を使って文字入力と IME イベントを受け取る。
   - マウスクリックではなくキーボードショートカット（`Alt+↓` / `Alt+↑` / `Ctrl+P` → `Enter` 等）でフォーカス先ウィジェット ID が変わったフレームでは、ブラウザ側の隠し `<input>` が blur されたままになることがある。`REF:src/ui/web.rs` の `sync_text_agent` で毎フレーム `egui_text_agent.focus()` を同期しているのはこのためであり、削除してはならない。
2. **Web 版での `rfd 0.14` ファイルダイアログ**:
   - `rfd::FileDialog`（同期 API）は `wasm32` に存在しないため、Web 版のファイル取込・保存は `REF:src/ui/web.rs` 内の非同期 `<input type="file">` / Blob ダウンロードブリッジを使用する。
3. **Web 版でのプロジェクト選択画面と `F2` キー**:
   - `Key::F2` はプロジェクトを開いている状態（`Stage::Open` かつオーバーレイなし）では識別子の一括改名（`Command::Rename`）に使い、Web のプロジェクト選択画面（`Stage::Picker`）では選択中プロジェクトの名称変更に使う。`REF:src/ui/shell.rs` で無条件に `F2` を消費すると `Stage::Picker` に届かなくなるため、`REF:src/ui/app.rs` 側でステージを見て振り分ける。
4. **Trunk 0.21 の `data-trunk` アセットパス解決**:
   - `REF:index.html` 内の `<link data-trunk ... href="...">` は `index.html` からの相対パスで解決されるため、先頭に `/` を付けず `href="assets/..."` と記述する。
5. **特殊記号 `⟨⟩`（U+27E8 / U+27E9）のフォントフォールバック**:
   - `assets/ipag.ttf`（IPA ゴシック）単体には数学括弧 `⟨` `⟩` のグリフが含まれないため、`REF:src/ui/theme.rs` では CJK フォントの後ろに egui 標準の `Hack` 等幅フォントをフォールバック維持している。`family.clear()` で既存フォントを全消去してはならない。
