# novnow 設計書 — 配布形態（外部提供物）

本書は `REF:docs/specifications/external.md` の各節に対応する実装設計を記述する。手順書（だれが何を押すか）は `REF:docs/distribution.md`、実装順序・受け入れ基準は `REF:docs/plans.md#distribution` に置く。build と publish の設定は、既存の `REF:.github/workflows/ci.yml` と `REF:.github/workflows/wheels.yml` の作法（draft では走らせない・`concurrency`・変更フィルタ）をそのまま引き継ぐ。

## 20. 配布形態

配布物の数は 5 形態で 18 通り（ブラウザ 1・デスクトップ 10・Python 5・VSCode 1・GitHub Actions 1）。この数を **1 つの表** から作り、CI と文書の両方がその表を読む。表を 2 か所に書かないための置き場所が `REF:scripts/release-matrix.py` で、成果物の名前・対象環境・ビルドの引数をここから生成する。`REF:tests/distribution.rs` が §20.1.1 の数（1・10・5・1・1）と表の行数を突き合わせる。

### 20.1 配布形態の一覧と共通原則

状態: 実装完了（REF:scripts/release-matrix.py, REF:.github/workflows/release.yml, REF:tests/distribution.rs）

- **版の一元化**: 版番号の正本は `REF:Cargo.toml` の `version` だけとする。Python（`REF:python/novnow-py/Cargo.toml`）・VSCode 拡張（`REF:vscode/package.json`）・Actions の `action.yml` は、リリース時にこの値で書き換える。手で 4 か所を編集しない（`scripts/release-matrix.py` の `--bump` が担う）。
- **成果物の生成**: `scripts/release-matrix.py` が 10 行のデスクトップ行列（対象環境 × gui/nogui）と 5 行のホイール行列を JSON で出す。`REF:.github/workflows/release.yml` はこの JSON を `strategy.matrix` に渡す。
- **照合値**: 初期添付（デスクトップ・ホイール）と最終添付（VSIX・全ソース）の各段階で `REF:scripts/release-matrix.py` が SHA-256 を生成・検証する。初期添付成功後にレジストリ公開を進め、最終添付時に全 17 成果物の `SHA256SUMS` を確定する。
- **実行時の非通信**: 更新確認のコードを置かない。`tests/no_llm_sdk.rs` と同じ発想で、`reqwest` / `ureq` などの HTTP クライアントを本体の依存へ足さない（`REF:tests/layering.rs` の許可リストに追加しない）。
- **署名**: Windows・macOS の配布元署名とmacOS公証は行わない。署名・公証ジョブと認証情報への参照を置かず、`SHA256SUMS` に未署名・公証なしを明記する。
- **ブラウザ版**: `REF:.github/workflows/deploy.yml` が Astro の出力に Trunk の出力を `app/` として追加し、単一の Pages artifact を配信する。版は `REF:index.html` の `novnow-version` メタ情報と画面に表示し、`REF:scripts/release-matrix.py` の `--bump` / `--check` で揃える。

### 20.2 デスクトップ版

状態: 実装完了（REF:src/main.rs, REF:scripts/release-matrix.py, REF:.github/workflows/release.yml, REF:tests/distribution.rs）

- **feature の追加はしない**: 10 通りは「`ui` feature の有無」×「`--target`」だけで作る（2 × 5）。新しい feature も、配布物を分けるビルドスイッチも足さない。同梱物（`REF:src/ui/theme.rs` の `BUNDLED_FONT`、`REF:src/core/template/mod.rs` の `include_str!` 群）は現状どおり常に実行ファイルへ埋め込む。
- **成果物の名前**: `scripts/release-matrix.py` の `artifact_name(version, target, gui)` が唯一の実装点。命名は仕様 §20.2.2 の 1 形だけを許し、テストで 10 通りを列挙して重複が無いことを確かめる。
- **アーカイブの中身**: 実行ファイル・`LICENSE`・`README.txt`（起動方法 3 行）。実行時資源のファイルは入れない（実行ファイルに入っている）。再配布条件の例外として `REF:licenses/IPA_Font_License_Agreement_v1.0.txt` を添付し、`REF:src/ui/theme.rs` にも同じ本文を埋め込みヘルプで表示する。macOS の GUI あり版だけ `.app` を組み立てる（`Contents/MacOS/novnow` と `Info.plist`、`CFBundleExecutable=novnow`、`LSMinimumSystemVersion`）。`.app` を作るのは CI のスクリプト（`scripts/release-matrix.py --bundle-macos`）とする。
- **同時起動**: 制限しない（仕様 §20.2.3）。ロックファイルも排他も作らないので、新しいファイルはプロジェクト外に増えない。
- **なぜ軸を置かないか**: 同梱物を外に置く版を作ると、(1) 配布物の置き場所に依存する読み取り経路が増え、(2) 「実行ファイル 1 個で動く」という検証が 2 通りになり、(3) 得られる削減が同梱フォントの約 6 MB だけである。履歴ファイル以外をプロジェクト外に置かない現状では、利用者から見た違いがほぼ無いため、軸そのものを持たない（2026-10-23 の裁定）。
- **GUI なしのビルド**: `ui` feature を付けない。`--gui` は既存の `Exit::Usage`（`REF:src/cli/mod.rs`）へ流す。`tests/cli.rs` に「引数なし + 非 TTY + GUI なしビルド → 使い方表示・終了コード 2」を足す。
- **Windows のサブシステム**: GUI 版だけ `#![windows_subsystem = "windows"]` を効かせる。既存の `REF:src/main.rs` の cfg を feature で切り替える（`#[cfg_attr(all(windows, feature = "ui"), windows_subsystem = "windows")]`）。
- **CI 行列の負荷**: 10 ジョブは 1 つのワークフローにまとめ、タグ公開と `workflow_dispatch` のときだけ走らせる。PR では 2 通り（`ui` あり/なし）だけをビルドして `cargo build` が通ることを見る。
- **テスト**: `tests/distribution.rs` が (1) 命名の 10 通りと重複の無さ、(2) 同じ版で 5 対象環境 × 2 画面が揃うこと、(3) `ui` の有無で保存形式のテスト（`REF:tests/roundtrip.rs`）と診断のゴールデン（`REF:tests/golden.rs`）が同じ結果になること、を確かめる。同梱物が実行ファイルに含まれることは、GUI なしの成果物に `assets/ipag.ttf` のファイルが入っていないこと（アーカイブの中身）で見る。

### 20.3 Python ライブラリ版

状態: 実装完了（REF:python/novnow-py/pyproject.toml, REF:.github/workflows/wheels.yml, REF:.github/workflows/release.yml, REF:scripts/python-check.sh, REF:tests/distribution.rs）

- **5 構成への拡張**: `REF:.github/workflows/wheels.yml` は Linux x64 / arm64、macOS x64 / arm64、Windows x64 の 5 構成。macOS x64 は ARM64 ランナーからクロスビルドし、Rosetta 上の Intel Python で導入を検査する。`abi3-py39` により CPython 3.9 以降を賄う。
- **manylinux**: Linux は `maturin build --manylinux 2_28` を明示し、ビルドした glibc の下限をホイール名に固定する（`auditwheel` の自動判定に任せない）。
- 統合 Release の prepare は通常版以外を不可逆な公開前に拒否する。Python 試験版だけは補助 wheels で TestPyPI へ送る。
- **版の一致**: ホイールの `version` は `Cargo.toml` から取り、`REF:scripts/release-matrix.py` の `python_version` が PEP 440 の等価表現へ写像する。実行時の製品版は正準 SemVer のままとする。`maturin` は `REF:python/novnow-py/pyproject.toml`の `dynamic = ["version"]` で読む。リリース時に `pyproject.toml` の `project.name = "novnow"` を確認する（現在のフォルダ名 `novnow-py` と公開名は別でよい）。
- **公開**: `REF:.github/workflows/release.yml` のタグ実行は TestPyPI → PyPI を公開する。手動実行は公開入力で選択する。補助の `REF:.github/workflows/wheels.yml` は TestPyPI リハーサル専用で、本番 PyPI を公開しない。認証は各 environment に紐づく Trusted Publisher（OIDC）であり、PyPI API トークンは不要。
- **ソース配布を出さない**: Python 拡張は `maturin build` だけでホイールを作り、PyPI 用 sdist は作らない。本体の Rust クレートは `REF:.github/workflows/release.yml` が crates.io に公開する。全ソースの Releases 添付とは別経路である。
- **テスト**: `REF:scripts/python-check.sh --wheel <ファイル>` でホイールの版とタグを検査する。`--full` は拡張のビルドと pytest も実行する。各対象への導入・import・版一致はホイールワークフローで検査する。

### 20.4 VSCode 拡張版

状態: 未実装・実装計画済み（REF:src/lsp/mod.rs, REF:vscode/package.json, REF:vscode/src/extension.ts, REF:vscode/src/binary.ts, REF:vscode/src/commands.ts, REF:.github/workflows/vscode.yml, REF:docs/plans.md#issue-29-b9）

- **置き場所**: `REF:vscode/`。`package.json`（識別子 `sonneko.novnow`、`engines.vscode`、`activationEvents`、`contributes.configuration`、`contributes.commands`）、`src/extension.ts`（接続とコマンド）、`esbuild.mjs`（1 ファイルへ束ねる）。`novnow` の実行ファイルは同梱しない。
- **依存**: `vscode-languageclient` と `vscode-languageserver-protocol` だけを足す（npm）。esbuild で `dist/extension.js` に束ね、`node_modules` を `.vsix` に入れない。
- **探索順**: `REF:vscode/src/binary.ts` の `resolveBinary()` が設定パス → PATH → 拡張キャッシュ → 自動取得の順に探す。設定パスが存在しない場合も次へ進む。
- **取得**: リリースの URL は `https://github.com/sonneko/novnow/releases/download/v<版>/<成果物名>`。成果物名は `scripts/release-matrix.py` の出力（`nogui`）を `package.json` の `novnow.artifact` 表へリリース時に埋め込む。保存先は `context.globalStorageUri` 配下の `bin/<版>/novnow`（Windows は `.exe`）。`https.get` のリダイレクトを 1 段だけ追い、`crypto.createHash("sha256")` で照合する。失敗したら部分ファイルを消す。
- **設定**: `REF:vscode/package.json` の `novnow.executablePath`（文字列）、`novnow.autoDownload`（真偽、既定 true）、`novnow.extraArgs`（配列）。実行ファイルの版は拡張機能の版に固定する。設定は VSCode 側に置き、プロジェクトへは書かない。
- **LSP の起動**: `novnow lsp <プロジェクトルート>` を stdio で起動する。サーバの生死は `vscode-languageclient` の `Restart` に任せ、再起動コマンドを 1 つ提供する。診断はサーバの `publishDiagnostics` をそのまま使う。
- **言語 ID**: `novnow` を定義し、`contents/**/*.txt` と `plots/**/*.txt` を対象にする。`contributes.languages` に拡張子ではなく `filenames` / `filenamePatterns` で書く（`.txt` 全体を奪わない）。
- **失敗時の見え方**: 取得に失敗したら `window.showErrorMessage` に「手動で入れる場所」の URL を添える。同じ版・対象環境では失敗状態を保存し、自動再試行を止め、`novnow.autoDownload` を無効にしたのと同じ状態にする。
- **テスト**: `@vscode/test-electron` で (1) `executablePath` を与えたときにサーバが起動する、(2) 取得関数が照合値の不一致で例外になる、(3) コマンドが CLI を呼ぶ、の 3 件。CI では `xvfb-run` を使う。
- **公開**: `vsce package` で `.vsix` を作り、リリースへ添付する。Marketplace公開は遅くとも製品v1到達までに開始する保留事項とし、現在のworkflowには公開ジョブ・入力・PAT依存を置かない。

- **[Issue #29 / B9・B10] 両市場への公開準備と `.cnov` 対応（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b9`）**:
  - VS Code Marketplace と Open VSX への公開ジョブを、タグ実行でも自動公開にならない形（手動承認付きの `workflow_dispatch`）で用意する。登録と発行者トークンの用意は所有者の手続きであり、ジョブはトークンが無ければ安全に何もしない。初回公開自体はその後に別途行う。
  - 拡張へ `.cnov` の言語関連付け（`extensions: ['.cnov']`、`REF:vscode/package.json`）と最低限の色付け（シジル・コメント・台詞など、`REF:vscode/syntaxes/novnow.tmLanguage.json`）を足した（B10 で実装完了。公開準備は B9）。

### 20.5 GitHub Actions 版

状態: 実装完了（REF:action.yml, REF:scripts/action/install.sh, REF:scripts/release-tags.sh, REF:.github/workflows/action-smoke.yml, REF:tests/distribution.rs）

- **置き場所**: リポジトリ直下の `REF:action.yml`。`runs.using: composite`。ステップは (1) 対象環境の判定、(2) 取得（`actions/cache` を挟む）、(3) 実行、(4) 出力の書き出し、の 4 つだけにする。
- **対象環境の判定**: `${{ runner.os }}` と `${{ runner.arch }}` から仕様 §20.2.1 の三重項を選ぶ。Windows の既定シェルは PowerShell なので、判定と取得は `shell: bash` のステップに閉じる（ランナーには Git Bash がある。`REF:.github/workflows/wheels.yml` の「`>/dev/null` が `C:\dev\null` になる」注意と同じ理由）。
- **取得の実装**: `REF:scripts/action/install.sh` が `RUNNER_OS` / `RUNNER_ARCH` と入力 `target` から対象を判定する。版は入力 `version`、`REF:action.yml` に埋め込む `NOVNOW_ACTION_VERSION`、`GITHUB_ACTION_REF` の順に選ぶ。ダウンロードは HTTPS の curl、照合は sha256sum または shasum を使う。
- **入力の受け渡し**: 入力は `env:` でスクリプトへ渡し、`args` は `read -r -a` で配列にする。`path` はサブコマンド直後の位置引数に渡し、シェル文字列として連結・評価しない。
- **出力**: `exit-code` は `$GITHUB_OUTPUT` へ書く。`findings` は `check` が終了コード 0 または 5 のとき、追加で `diagnostics run <path> --json` を実行して `count` を読む。人間向けの出力行数は数えない。`export-path` は `--out` に渡した値を返す。
- **失敗の伝え方**: スクリプトは novnow の終了コードをそのまま返し、複合アクション側で `exit $code` する。`continue-on-error` を既定にしない。
- **キャッシュ**: `actions/cache@v4` のキーは `novnow-<版>-<三重項>`。キャッシュを当てても照合は毎回行う（キャッシュの汚染を検出する）。キャッシュの失敗はジョブを止めない。
- **タグ**: リリース時に `v<版>` を打ち、同じコミットへ `v0` を付け替える。付け替えは `REF:scripts/release-tags.sh` に閉じ、`git push --force-with-lease` を使わない（`gh api` で ref を更新する）。
- **テスト**: `REF:.github/workflows/action-smoke.yml` を `workflow_dispatch` または Release 成功後に走らせ、3 OS で `uses: ./` の自己参照により点検を 1 回実行する。PR では走らせない（Actions の使用量を抑える。`REF:docs/developers.md` §4.7）。

### 20.6 公開・版・廃止

状態: 未実装・実装計画済み（REF:src/main.rs, REF:scripts/release-check.sh, REF:scripts/release-tags.sh, REF:scripts/release-matrix.py, REF:.github/workflows/release.yml, REF:docs/plans.md#issue-29-b9）

- **追加の公開物**: `REF:Cargo.toml` のクレートを crates.io に公開し、追跡対象全体（文書を含む）を `git archive` でソースアーカイブ化する。最終照合は `REF:scripts/release-matrix.py` がソースを含めて行う。
- **公開の順序**: `REF:.github/workflows/release.yml` の `needs` で、デスクトップ・ホイール試験・クレート dry-run → Releases 添付 → TestPyPI → PyPI → VSIX 作成 → 最終照合 → 可動タグの順を制御する。crates.io 公開は Releases 添付後に進め、最終照合の前提にする。タグ実行は環境承認後に全公開を行い、手動実行は選択したレジストリのみ公開する。失敗した公開を成功扱いにしない。
- **未提供の表示**: `REF:scripts/release-matrix.py --status` はローカル成果物の準備状態を返す。レジストリの公開成功を判定する API ではない。README・手引き・サイトは公開確認後に保守担当者が更新し、未公開の形態は「未提供」を維持する。
- **取り下げ**: リリースの成果物は削除しない。PyPI は `yank` を使い、`SHA256SUMS` とリリースノートに理由を書く。市場と Actions のタグは戻さず、修正版を出す。
- **検査**: `REF:scripts/release-check.sh` が (1) 10 + 5 + VSIX + 全ソースの成果物が揃う、(2) `SHA256SUMS` の行数が成果物と一致する、(3) 各形態の版が `Cargo.toml` と一致する、(4) 主要版タグが最新の版を指す、を確かめる。`REF:scripts/verify.sh` には入れない（リリース時だけの検査なので、通常のコミットで走らせない）。

- **[Issue #29 / B9] 市場公開を公開完了の条件から外す（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b9`）**:
  - 公開チェック（`REF:scripts/release-check.sh`）は 5 形態の成果物と照合値を従来どおり揃えて判定し、市場への公開は条件に含めない。市場側は手動承認のジョブ（§20.4）で後から進める。
