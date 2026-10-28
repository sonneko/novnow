# novnow 配布方法（Distribution）

本書は、novnow を利用者の手元へ届ける**手順**をまとめる。何を配るか（形・選択肢・名前・MUST）は `REF:docs/specifications/external.md` §20、どう作るかは `REF:docs/designs/external.md` §20 が正本である。本書は「だれが、どこへ、どの順で置くか」だけを扱う。

**いまの状態（2026-10-25）**: ブラウザ版だけが公開済み。デスクトップ版・Python ライブラリ版・VSCode 拡張版・GitHub Actions 版の実装と公開用ワークフローは用意済み。初回リリースはまだ実行しておらず、ブラウザ版以外は未提供である（`REF:docs/plans.md#distribution`）。

---

## 1. 5 形態の置き場所

| 形態 | 選択肢 | 成果物 | 置き場所（公開先） | 利用者の取り方 |
|---|---|---|---|---|
| ブラウザ版 | 1 | サイトの公開物（PWA） | GitHub Pages（[`https://sonneko.github.io/novnow/app/`](https://sonneko.github.io/novnow/app/)） | URL を開く |
| デスクトップ版 | 10 | `.tar.gz` / `.zip` の 10 個 | GitHub Releases（タグ `v<版>`） | ダウンロードして展開 |
| Python ライブラリ版 | 5 | ホイール 5 個 | PyPI（`novnow`）と Releases（控え） | `pip install novnow` |
| VSCode 拡張版 | 1 | `.vsix` 1 個 | GitHub Releases（Marketplaceはv1到達までに開始） | VSIXから手動導入 |
| GitHub Actions 版 | 1 | 複合アクション（タグ） | GitHub のリポジトリ（`sonneko/novnow@v0`） | `uses:` に書く |

- 版番号は 5 形態で共通（`REF:Cargo.toml` の `version`）。
- 実行ファイル・ホイール・`.vsix` の照合値（SHA-256）は、同じリリースに `SHA256SUMS` として置く。
- デスクトップ版の配布ページは [`https://github.com/sonneko/novnow/releases/latest`](https://github.com/sonneko/novnow/releases/latest)（版を固定するときは `/releases/tag/v<版>`）。
- どの形態も、導入後に配布元へ自動で問い合わせない（更新確認をしない）。

### 1.1 ブラウザ版（公開済み）

- 配布は `REF:.github/workflows/deploy.yml` が行う。Astro のサイトを `/novnow/` に、Trunk の Web アプリを `/novnow/app/` にまとめ、1 つの Pages artifact として配信する。
- 成果物は trunk の `dist`（`index.html`・`sw.js`・`manifest.webmanifest`・WASM・`assets/`・`guide/`）。`REF:index.html` の `copy-dir` が `guide/` を dist に載せる。
- 更新はタグではなく `main` へのマージで反映される。版番号は画面の情報に表示し、`REF:Cargo.toml` と一致させる。
- Service Worker が古い資産を配り続けないよう、`REF:sw.js` のキャッシュ名を `novnow-offline-<版>` とし、`REF:scripts/release-matrix.py` の `--check` で版の一致を確かめる。公開後に [`https://sonneko.github.io/novnow/app/`](https://sonneko.github.io/novnow/app/) をシークレットウィンドウで開き、読み込みとオフライン起動を確かめる。
- ブラウザ版だけは「取得」が無いので、照合値もタグも持たない。この非対称は仕様 §20.1.3 のとおり。

---

## 2. デスクトップ版（10 通り）

### 2.1 名前の読み方

```
novnow-<版>-<対象環境>-<gui|nogui>.<拡張子>
```

例: `novnow-0.1.0-aarch64-apple-darwin-gui.tar.gz`

- `gui` = 窓あり（CLI としても使える）/ `nogui` = 窓なし（CLI・LSP・MCP 専用）
- Linux と macOS は `.tar.gz`、Windows は `.zip`。macOS の `gui` には `.app` が入る。
- 同梱物（フォント・既定プロンプト・サンプル）は実行ファイルの中にある。アーカイブに置くのは実行ファイル・`LICENSE`・`README.txt` だけである。

### 2.2 10 通りの一覧

| 対象環境 | gui | nogui |
|---|---|---|
| `x86_64-unknown-linux-gnu` | ✓ | ✓ |
| `aarch64-unknown-linux-gnu` | ✓ | ✓ |
| `x86_64-apple-darwin` | ✓ | ✓ |
| `aarch64-apple-darwin` | ✓ | ✓ |
| `x86_64-pc-windows-msvc` | ✓ | ✓ |

### 2.3 どれを選ぶか

利用者への案内は、次の 1 つの質問とパソコンの種類だけで決まる（既定の推奨は `gui`）。

| 質問 | 答え | 選ぶもの |
|---|---|---|
| 画面（窓）が要りますか | 要る | `gui` |
| 画面（窓）が要りますか | 要らない（コマンドやほかのソフトから使う） | `nogui` |

- `nogui` は `--gui` を渡すと終了コード 2 を返す。`new` / `check` / `export` / `import` / `migrate` / `mcp` / `lsp` / `api` は使える。
- 同時に何個でも起動できる（版ごとの違いは無い）。
- インストーラは無い。展開したフォルダをそのまま置き、実行ファイルを起動する。PATH の追加や OS の関連付けは利用者が行う。

### 2.4 各 OS での注意

| OS | 注意 |
|---|---|
| macOS | 公証（notarization）が無いビルドは Gatekeeper が止める。「プライバシーとセキュリティ」から開く手順を手引きに書く。`xattr -d com.apple.quarantine` は最終手段として案内する |
| Windows | 署名が無いビルドは SmartScreen の警告が出る。「詳細情報」→「実行」の手順を手引きに書く |
| Linux | 展開後の実行ファイルに実行権限が付いていることを確認する（`chmod +x` はアーカイブ側で済ませる） |

### 2.5 公開の手順（保守）

1. `REF:scripts/release-matrix.py` の `--bump <版>` で版をそろえ、`--check` と `REF:scripts/verify.sh` を通してからタグ `v<版>` を打つ。
2. タグの push、または `REF:.github/workflows/release.yml` の `workflow_dispatch` で、デスクトップ 10 個・ホイール 5 個と `SHA256SUMS` を作る。
3. リリースノートに、版と 10 通りの一覧（対象環境 × `gui` / `nogui`）を書く。壊れた成果物があれば、その版ごと出し直す（成果物だけを差し替えない）。
4. `REF:scripts/release-check.sh` で、`.vsix` を含む 17 個の成果物（バイナリ等 16 + 全ソース 1）・照合値の行数・版の一致を確かめる。公開後のタグ検査には `--remote` を付ける。

---

## 3. Python ライブラリ版（5 通り）

- 公開名は `novnow`。取得は `pip install novnow`、版の固定は `pip install novnow==<版>`。
- ホイールは `cp39-abi3` の 5 個。CPython 3.9 以降のどの版でも、同じホイールが使える。

| 対象環境 | ホイールのプラットフォームタグ |
|---|---|
| Linux x86_64 | `manylinux_2_28_x86_64` |
| Linux aarch64 | `manylinux_2_28_aarch64` |
| macOS arm64 | `macosx_11_0_arm64` |
| macOS x86_64 | `macosx_11_0_x86_64` |
| Windows x86_64 | `win_amd64` |

- **PyPI には**ソース配布（sdist）は出さない（利用者に Rust を要求しない。仕様 §20.3.1）。
- ネットワークに出られない環境向けに、ホイールを直接指定して導入できる（`pip install ./novnow-<版>-cp39-abi3-<タグ>.whl`）。
- 導入の確認は `python -c "import novnow; print(novnow.__version__)"`。版は本体と一致する。

### 3.1 公開の手順（保守）

1. デスクトップ版のリリースが緑になったことを確認する（仕様 §20.6.1 の順序）。
2. リハーサルには `REF:.github/workflows/wheels.yml` の手動 TestPyPI 公開を使う。本番の Release タグ実行は TestPyPI だけで止まらず、承認後に PyPI も公開する。同じ版を TestPyPI に事前公開すると本番のアップロードが重複で失敗するため、試験には別のプレリリース版を使う。
3. 本番 PyPI は統合 `REF:.github/workflows/release.yml` だけが公開する。タグ実行では自動で、同ワークフローの手動実行では公開入力で PyPI を選び、`environment: pypi` の承認を通して本番の PyPI へ公開する。
4. 5 つのホイールが揃っていること、`novnow.__version__` が `REF:Cargo.toml` と一致することを確認する。
5. 壊れた版は削除せず、PyPI の yank で印を付ける（仕様 §20.6.3）。

---

## 4. VSCode 拡張版（1 通り・実装済み）

- 識別子は `sonneko.novnow`。今回はReleasesの `.vsix` から入れる。Marketplace公開は延期し、遅くともv1到達までに開始する。
- 拡張機能は novnow の実行ファイルを同梱しない。プロジェクトを開いたときに、次の順で探す（仕様 §20.4.2）。
  1. 設定 `novnow.executablePath`
  2. `PATH` 上の `novnow`
  3. 拡張機能が以前に取得した実行ファイル
  4. 自動取得（`novnow.autoDownload` が真のときだけ）
- 自動取得が入れるのは、その環境向けの `nogui`。版は拡張機能と同じ。保存先は拡張機能の保存領域（プロジェクトの中ではない）。
- 取得は照合値と一致したときだけ使う。失敗しても拡張機能は起動し、novnow に依存する機能だけが無効になる。
- 提供するコマンドは「プロジェクトの点検」「テキストの書き出し」「新規プロジェクト」「言語サーバの再起動」の 4 つ。診断・補完などは LSP の返す内容をそのまま見せる。

### 4.1 公開の手順（保守）

1. `vsce package` で `.vsix` を作り、リリースへ添付する。
2. `SHA256SUMS`とVSIXの版を確認し、VSCodeの「VSIXからのインストール」で導入を確認する。
3. Marketplace公開を再開するときは、所有権・PAT・承認ゲートと同じ版の配布を別途整備する。現在のReleaseにMarketplace公開処理はない。

---

## 5. GitHub Actions 版（1 通り・実装済み）

```yaml
- uses: sonneko/novnow@v0
  with:
    path: .
    command: check
```

| 入力 | 既定 | 意味 |
|---|---|---|
| `path` | `.` | 対象のプロジェクト |
| `command` | `check` | 実行するサブコマンド |
| `args` | 空 | 追加の引数（空白区切り。引用符は解釈しない） |
| `version` | 空 | 使う版。空ならアクションのタグの版 |
| `target` | 空 | 対象環境。空ならランナーから自動判定 |

| 出力 | 意味 |
|---|---|
| `exit-code` | novnow の終了コード |
| `findings` | 点検の診断件数（点検以外では空） |
| `export-path` | 書き出したファイル（書き出し以外では空） |

- 取得するのは `nogui` の実行ファイル。ランナーの OS と CPU から対象環境を選ぶ。
- 照合値は毎回確かめる（キャッシュを使っても省略しない）。novnow の終了コードをそのままジョブへ返すので、E 診断があればジョブが失敗する。
- 通信は取得のときだけ。リポジトリの内容は送らない。秘密情報は要らない。

### 5.1 公開の手順（保守）

1. `action.yml` と `scripts/action/install.sh` を含む版をタグ `v<版>` で公開する。
2. 主要版の可動タグ（0.x では `v0`）を同じコミットへ付け替える。プレリリースでは可動タグを動かさない。v0 は形式互換性を保証しないので、利用者には `@v0.1.0` のような完全な版の指定を推奨する。
3. `REF:.github/workflows/action-smoke.yml` は完全なリリースの成功後に自動で走る。手動でやり直すときは `workflow_dispatch` を使う。3 OS で E 診断の失敗も確かめる。

---

## 6. 版を出すときの順序（全体）

タグ実行は、環境承認を挟みながら公開する。手動実行は未公開の版タグ上でのみ可能。公開済みの版を新規実行し直すことはできない（§9）。

| 順 | すること | 失敗したら |
|---|---|---|
| 1 | 版番号を `REF:Cargo.toml` で決め、`python3 scripts/release-matrix.py --bump` で Python・拡張・Actions の版を揃える | 公開しない |
| 2 | デスクトップ 10・ホイール 5 とクレート dry-run を検証し、15 成果物と照合値をリリースへ添付する | 公開しない（成果物だけ差し替えない） |
| 3 | crates.io と、TestPyPI → PyPI の公開を並行して進める | 以降を止める。タグは残す |
| 4 | VSIX と全ソースをReleasesへ添付し、17 成果物の最終照合を行う（Marketplaceには公開しない） | 以降を止める。新しい版で直す |
| 5 | Actions の `v<版>` と `v0` を公開する | 利用者へ「未提供」と表示する |
| 6 | README・サイト・手引きに実際の取得先を載せ、未公開の形態は「未提供」と記す | 文書だけが古い状態になる。次の版までに直す |

- **版を上げると必須チェックの名前も変わる**: ruleset の `required_status_checks` は
  `build (… 0.1.0, novnow-0.1.0-…)` のように**版を含む行列名**をそのまま持つ。`--bump` で版を
  上げた直後は、必須チェックが「報告待ち」のままになって release PR をマージできない。版更新 PR を
  作る前に、`protect-develop` と `protect-main` の必須チェックを、その PR の Checks 一覧に出ている
  新しい名前（`0.1.1` を含む 4 件）へ差し替える。差し替えは管理者が行う（§8）。
- 版ごとの成果物は削除しない。重大な不具合は新しい版で直す（仕様 §20.6.3）。
- 公開を止めるときは、同じ版の 5 形態をまとめて「未提供」に戻す。形態ごとに残さない。

---

## 7. 確認用の早見表

| 確かめたいこと | 見るもの |
|---|---|
| デスクトップ 10・ホイール 5・`.vsix` 1・全ソース 1 が揃っている | `SHA256SUMS` の行数と Releases の添付 |
| 名前が仕様どおり | `novnow-<版>-<対象環境>-<gui\|nogui>` の 1 形だけか |
| 版が 5 形態で同じ | `--version`・`novnow.__version__`・拡張の版・Actions のタグ |
| 照合値が一致する | `sha256sum -c SHA256SUMS`（macOS は `shasum -a 256 -c`） |
| 実行時に通信しない | 起動して何も取得しないこと（プロキシを切って起動する） |
| プロジェクトの外へ書かない | 起動と執筆の前後で、プロジェクトの外のファイル一覧が変わらないこと（本体では履歴だけ。配布物の取得時は専用領域の限定例外を別に検査） |

---

## 8. 外部設定と秘密情報（初回必須）

以下はリポジトリへの変更だけでは完了しない。値を文書・ログ・ソースへ書かず、GitHub の Settings → Environments / Secrets and variables で登録する。crates.io・PyPI・TestPyPIの設定は所有者が確認済み。実公開成功とは区別する。

| サービス / Environment | 必要な設定・権限 | Secret / 認証方式 |
|---|---|---|
| crates.io / `crates-io` | `novnow` 名の利用可否・所有者、GitHub 連携アカウントのメール認証。初回の新規クレート公開が可能で、以降は `novnow` だけに限定した publish token を発行。期限を管理する | `CARGO_REGISTRY_TOKEN`（environment secret）。`cargo publish --locked` が使用 |
| TestPyPI / `testpypi` | `novnow` プロジェクト（初回は pending publisher 可）。Trusted Publisher の owner=`sonneko`、repository=`novnow`、workflow=`release.yml`、environment=`testpypi` | OIDC。API トークン不要。job に `id-token: write` |
| PyPI / `pypi` | TestPyPI と別に同じ公開名を確保。Trusted Publisher の environment=`pypi`、それ以外は上と同じ。必須 reviewer を登録 | OIDC。API トークン不要 |
| VSCode / VSIX | GitHub Releases添付のみ。Marketplaceはv1到達までに開始する保留事項 | 今回はVSCE_PAT不要 |
| GitHub Releases / Actions tags | Actions の書き込み許可、タグ作成権限。バージョンタグは変更・削除を保護し、可動の `v0` はワークフローの更新を許可する | 自動発行の `GITHUB_TOKEN`（`contents: write`）。個人 PAT は不要 |
| GitHub Pages / `github-pages` | Settings → Pages → Source=`GitHub Actions`。main からだけデプロイを許可。カスタムドメインは設定しない | 自動トークン（`pages: write`, `id-token: write`） |

- `crates-io`・`testpypi`・`pypi` は版タグからのみ実行できる deployment policy にし、公開担当者の承認を必須にする。環境名と Trusted Publisher の設定は完全一致が必要。
- ホイール単独試験の `REF:.github/workflows/wheels.yml` から公開する場合は、その workflow 名で **別の** Trusted Publisher が必要。本番 PyPI の Trusted Publisher は `release.yml` だけに設定する。wheels の公開入力は TestPyPI 専用である。
- GitFlow の `main` と `develop` は保護する。直接 push・force push・削除を禁止し、PR、必要な CI 成功を要求する。CI 行列はスコープで切り替える（`docs/developers.md` §4.7）。`main` 向け PR・`main` への push・手動実行は full 行列（desktop 10・wheel 5・ブラウザ3種・Electron 3 OS・Python 3 版）。`develop` 向け PR・`develop` への push は 1 プラットフォームの最小スコープ（Linux x64: desktop nogui＋wheel。ブラウザは Chromium のみ。Electron は ubuntu のみ。Python 試験は 3.9 / 3.13）。必須チェックはスコープで走る項目に一致させる: `develop` は 11 件（2026-10-27 改定。旧来は full 行列を含む 23 件）、`main` は従来どおり3ブラウザ・full 行列を含む 18 件。Firefox / WebKit は偽陽性が多発するため、develop 向け PR では実行せず、`main` 向け PR では必須のまま残す。現在の必要承認数は0、通常のmerge commitで統合する。`main` には release / hotfix の版更新 PR だけを承認する。GitHub 標準の保護だけでは「版更新のみ」は判定できないため、レビューで版差分と PR の由来を確認する。
  - `develop` の必須チェック 11 件（ruleset `protect-develop`）: `Docs & Change Filter` / `Rustfmt` / `Clippy` / `Test` / `WASM Build Check` / `Python API` / `Benchmarks` / `build`（Pages integration の結合ツリー）/ `browsers (chromium)` / Linux x64 の wheel 行列（`build (x86_64-unknown-linux-gnu, ubuntu-latest, …, novnow-<版>-cp39-abi3-manylinux_2_28_x86_64.whl…)` の 1 件）/ Linux x64 の desktop 行列（`build (x86_64-unknown-linux-gnu, ubuntu-latest, …, nogui, novnow-<版>-x86_64-unknown-linux-gnu-nogui.tar.gz…)` の 1 件）。版を含む行列名は版更新で変わるため、追加・更新時はその時点の PR の Checks 一覧から同名を選ぶこと。
  - 平台の 2 行は develop への push でも報告される（wheels の push トリガーは develop かつ PR 同一 `paths`、desktop-check の push トリガーは develop・無制限。2026-10-27 追加）。したがってコード PR のマージ後、develop 自身のコミットでも 11 件が揃う。文書のみ変更のマージでは Rust 5 ジョブが ci.yml の変更フィルタで skip され既存どおり赤くなる（`docs/developers.md` §4.7 の落とし穴）。
- CI の `[ci skip]` 既定と必須チェックを併用するとチェック待ちになる。リリース PR は `[ci run]` で CI を起動し、保護が要求するチェックが実際にその SHA で報告されていることを確認する。手動 CI の結果だけで保護を迂回しない。
- `develop` の作成・既定ブランチの変更・rulesets・環境・秘密情報・公開名の確保は管理者が行う。この作業セッションでは実施しない。

## 9. 初回リリースの実行と障害時の扱い

1. §8 の設定を済ませ、`REF:scripts/release-matrix.py --check`、`REF:scripts/verify.sh`、Web スモーク、ホスト CI・wheels・VSCode Electron・3 OS の実行確認を通す。各 OS の起動、IME、Zip 入出力も確認する。
2. release PR で版を揃え、リリースノート・互換性警告・未署名の有無・既知の制約を確認する。v0.1.0 は最初の番号付き配布であり、過去の `v0-final` とは別物。初回は既存の版番号がすでに 0.1.0 なので、形式的に別の版へ上げ下げしない。
3. 人間が版更新 PR を `main` にマージし、そのコミットに `v0.1.0` を付けて push する。タグは上書きしない。コミットメッセージに `[ci skip]` を残すとタグ起動も止まるので、タグ対象のマージコミットから除く。
4. Release は版一致・main の祖先であることを確認し、デスクトップ 10・wheel 5 とクレート dry-run を検証する。Releases 添付後、crates.io・TestPyPI → PyPI の環境承認を進める。承認待ちは失敗ではない。
5. 最後に VSIX と `novnow-<版>-source.tar.gz`（ドキュメントを含む追跡対象全体）を添付し、17 成果物の `SHA256SUMS` を検証して可動タグを更新する。GitHub 自動生成の Source code (zip/tar.gz) も利用できるが、照合対象は明示添付したソースアーカイブ。PyPI の sdist は作らない。
6. Pages は main 更新でサイトとアプリを一括配置する。Release と別ワークフローなので、両方の成功を確認してから公開完了とする。自動公開が skip された場合は main 上で Deploy を手動起動する。
7. 全公開先とインストール・照合を実測してから、README・サイト・手引きの「未提供」を解除する。今回のv0.1.0公開告知は所有者承認の例外として、版更新なしでdevelopからmainへ統合し、追加リリースしない。通常のmainへの統合は版更新PRに限定する。公開前に利用可能と書き換えない。リリースノートにはその場で実際の状態を記載できる。

**障害時**: 各レジストリの同一版は上書きできない。既存の Release に添付がある状態で、新しい workflow run を開始すると意図的に停止する。失敗したジョブのみを同じ run で再実行し、成功済み公開ジョブを含む「全ジョブ再実行」は避ける。公開自体は成功して応答だけ失敗した場合や、成果物を変更する必要がある場合は、管理者が公開状態を確認し、原則新しいパッチ版で復旧する。タグや公開済み成果物を削除・差し替えしない。

手動 Release は版タグを選んで起動する。公開入力は全て既定 false なので、全て false ならレジストリへは公開せず Releases の添付と、通常版の可動タグを作る。これは完全公開ではない。PyPI を選ぶ場合は TestPyPI も同時に選ぶ。初回本番は入力漏れを避けるためタグ push を推奨する。

## 10. 初回公開の追加確認と既知の未検証事項

- **公開設定は未検証**: Secrets の値は読み出して確認しない。所有者がサービス画面で登録・期限・権限を確認し、環境承認後のホスト実行で検証する（crates.io・PyPI・TestPyPIの設定は所有者確認済み）。crates.io / PyPI / Marketplace の公開は不可逆に近いため、この修正だけで公開しない。
- **Pages のリンク**: `/novnow/` から日本語・英語・手引き・診断・API・画像・言語切替、`/novnow/app/` の WASM・PWA とオフライン起動を確認する。Astro の base 設定に加え、`REF:website/integrations/base-links.mjs` がビルド済み HTML のルート相対 URL を補正する。
- **旧 PWA**: 旧 `/novnow/` スコープの worker は `REF:website/public/sw.js` で登録解除し、サイトとの干渉を止める。作品の IndexedDB は消さない。旧画面が残る場合はオンラインで再読込し、必要ならサイトデータ全消去ではなく Service Worker の登録だけを解除する。新アプリの worker は `/novnow/app/` のみを担当する。
- **署名・ライセンス**: 実物の署名・公証状況と警告回避手順、MIT と同梱 IPA フォントのライセンスを確認する。Linux は glibc / GUI ライブラリの依存を対象機で確認する。未署名のときに署名済みと宣伝しない。
- **バージョン境界**: v0 は安定形式ではない。スキーマ識別子（例: `schema_version: 1`）と製品の v1 リリースは別概念。既存の LocalStorage 読み込みや本文インポートが存在しても、v0 → v1 の形式移行を提供する意味ではない。
- **完全ソース**: `git archive HEAD` の追跡対象全体を含み、`.git`・未追跡ファイル・Secrets・ローカル生成キャッシュを含まない。外部依存そのものは同梱しないため、オフラインビルド用の vendoring 配布ではない。

### 10.1 このセッションで確認できた外部状態（2026-10-25）

GitHub APIでpublic化、4公開environmentとgithub-pages、developの存在、Pagesのworkflow設定を確認した。push起動のCIは実行できた。Secrets一覧とActions手動dispatchは403。CARGO_REGISTRY_TOKEN登録と各Trusted Publisherは所有者からの申告であり、値やレジストリ認証は検証していない。§8の設定がすべて完了したと推測してタグをpushしない。

所有者修正後のAPI再確認では、4公開environmentはmain=branch・版パターン=tag、required_reviewers=sonneko。developの適用対象とmain/developの必要レビュー数1・必須チェックも設定済み。TestPyPI側のtestpypiへの名称修正は所有者申告であり、OIDC公開成功は未検証。

現在の追加確認（所有者の変更後にAPI再確認）:

- main/developともRestrict updatesとlinear history制限は解除済み。PRの許可マージ方法はmergeのみ、必要承認数0。通常のmerge commitを使う。必須CI・force-push/削除禁止は維持されている。リポジトリ全体ではsquash/rebaseも有効だが、この2 rulesetではmergeだけを許可する。
- mainの必須18件、developの23件は現行チェック名と一致する（2026-10-27 再確認: 所有者が ruleset `protect-develop` から `browsers (firefox)` / `browsers (webkit)` を必須から外したため develop は25件から23件になった。`protect-main` では3ブラウザとも必須のまま）。版を含む行列名は将来の版更新で変わるため固定名への改善は別途必要。
- 2026-10-27（CI スコープ、§8 参照）: develop 向け PR は 1 プラットフォームの最小スコープになり、所有者が `protect-develop` の必須チェックを 11 件（Linux x64 以外の 12 のプラットフォーム行列を外した 23 件から）へ更新済み（PR #38 のマージ `6b5853b` での機能的確認済み）。`protect-main` は変更しない（18 件）。同日、必須の平台 2 行を develop 自身の push でも報告するため wheels / desktop-check に `develop` への push トリガーを追加し、4 ワークフローの手動実行に `scope` 入力（full / develop、既定 full）を足した。
- crates.io/PyPI/TestPyPIの設定は所有者が確認済み。こちらからの実公開・OIDC/tokenの実動確認とは区別する。Marketplaceは今回は公開せず、VSIXをReleasesへ添付する。
- Windows/macOSの配布元署名・macOS公証は行わない。署名用Secretsは不要。未署名の警告と導入方法を手引きで案内する。

公開しない事前検査には `REF:.github/workflows/pages-check.yml` と `REF:.github/workflows/desktop-check.yml` を追加した。前者は実Trunk/Astro結合物・3ブラウザエンジン・クレートdry-run、後者は10デスクトップアーカイブと展開後のCLI起動を検査する。GUIの画面/IME/Zip操作、実公開と実デプロイは別の受け入れ確認として残す。個々の最終結果はPR #31のChecksとコメントを参照する。

### 10.2 承認済みの例外と版表現

- IPA Gothic 003.03 のフォントは改変せず、配布ミラーの固定コミットと SHA-256 を `REF:licenses/README.txt` に記録する。同じ配布元の許諾原文は `REF:licenses/IPA_Font_License_Agreement_v1.0.txt`。フォント自体のハッシュ一致を確認済み。本体 MIT とは別の許諾である。外部ライセンス文書だけは資源の外置き禁止の例外とし、配布アーカイブと Web の公開物へ含め、アプリのヘルプでも読めるようにする。
- VSCode の globalStorage と Actions の実行ファイルキャッシュは配布物の取得専用として規則10の例外。実行ファイル・検証情報・短命な展開物に限り、原稿・作品設定・ログには使わない。本体の書き込み境界は変えない。
- Python 配布では PEP 440 表現を使う（例: `0.1.0-rc.1` → `0.1.0rc1`）。`pip install novnow==0.1.0rc1` と wheel 名・METADATA は正規化後の表記、`novnow.__version__` は製品の SemVer。対応は alpha/beta/rc と非負整数だけとし、別の接尾辞やビルドメタデータを混ぜない。
- 補助 wheels の公開入力は TestPyPI だけ。本番 PyPI の Trusted Publisher は統合 Release にのみ紐づける。TestPyPI の所有権・承認は別途管理者が設定する。

- 統合 Release は通常版のみを公開する。現在のVSIX版番号規則は通常版だけを扱うため、試験版のタグは成果物公開前の prepare で拒否する。Python 試験版は補助 wheels → TestPyPI で検証できる。Marketplace を含む全形態の試験版公開を実装済みとは扱わず、将来対応する場合は別途等価版の対応規則を決める。
