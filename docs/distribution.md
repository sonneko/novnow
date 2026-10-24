# novnow 配布方法（Distribution）

本書は、novnow を利用者の手元へ届ける**手順**をまとめる。何を配るか（形・選択肢・名前・MUST）は `REF:docs/specifications/external.md` §20、どう作るかは `REF:docs/designs/external.md` §20 が正本である。本書は「だれが、どこへ、どの順で置くか」だけを扱う。

**いまの状態（2026-10-23）**: ブラウザ版だけが公開済み。デスクトップ版・Python ライブラリ版・VSCode 拡張版・GitHub Actions 版の実装と公開用ワークフローは用意済み。初回リリースはまだ実行しておらず、ブラウザ版以外は未提供である（`REF:docs/plans.md#distribution`）。

---

## 1. 5 形態の置き場所

| 形態 | 選択肢 | 成果物 | 置き場所（公開先） | 利用者の取り方 |
|---|---|---|---|---|
| ブラウザ版 | 1 | サイトの公開物（PWA） | GitHub Pages（`https://sonneko.github.io/novnow/app/`）と公式サイト（`https://novnow.sonneko.com/app/`） | URL を開く |
| デスクトップ版 | 10 | `.tar.gz` / `.zip` の 10 個 | GitHub Releases（タグ `v<版>`） | ダウンロードして展開 |
| Python ライブラリ版 | 5 | ホイール 5 個 | PyPI（`novnow`）と Releases（控え） | `pip install novnow` |
| VSCode 拡張版 | 1 | `.vsix` 1 個＋市場の公開物 | VSCode の市場（`sonneko.novnow`）と Releases | 拡張機能の画面から導入 |
| GitHub Actions 版 | 1 | 複合アクション（タグ） | GitHub のリポジトリ（`sonneko/novnow@v1`） | `uses:` に書く |

- 版番号は 5 形態で共通（`REF:Cargo.toml` の `version`）。
- 実行ファイル・ホイール・`.vsix` の照合値（SHA-256）は、同じリリースに `SHA256SUMS` として置く。
- デスクトップ版の配布ページは `https://github.com/sonneko/novnow/releases/latest`（版を固定するときは `/releases/tag/v<版>`）。
- どの形態も、導入後に配布元へ自動で問い合わせない（更新確認をしない）。

### 1.1 ブラウザ版（公開済み）

- 配布は `REF:.github/workflows/deploy.yml` が行う。`main` の `dist` を GitHub Pages へ出し、公式サイト（`novnow.sonneko.com`）から同じアプリを開ける。
- 成果物は trunk の `dist`（`index.html`・`sw.js`・`manifest.webmanifest`・WASM・`assets/`・`guide/`）。`REF:index.html` の `copy-dir` が `guide/` を dist に載せる。
- 更新はタグではなく `main` へのマージで反映される。版番号は画面の情報に表示し、`REF:Cargo.toml` と一致させる。
- Service Worker が古い資産を配り続けないよう、`REF:sw.js` のキャッシュ名を `novnow-offline-v<版>` とし、`REF:scripts/release-matrix.py` の `--check` で版の一致を確かめる。公開後に `https://sonneko.github.io/novnow/app/` をシークレットウィンドウで開き、読み込みとオフライン起動を確かめる。
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
4. `REF:scripts/release-check.sh` で、`.vsix` を含む 16 個の成果物・照合値の行数・版の一致を確かめる。公開後のタグ検査には `--remote` を付ける。

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

- ソース配布は出さない（利用者に Rust を要求しない。仕様 §20.3.1）。
- ネットワークに出られない環境向けに、ホイールを直接指定して導入できる（`pip install ./novnow-<版>-cp39-abi3-<タグ>.whl`）。
- 導入の確認は `python -c "import novnow; print(novnow.__version__)"`。版は本体と一致する。

### 3.1 公開の手順（保守）

1. デスクトップ版のリリースが緑になったことを確認する（仕様 §20.6.1 の順序）。
2. `REF:.github/workflows/release.yml` をタグで起動し、TestPyPI へリハーサル公開する。手動の公開試験には `REF:.github/workflows/wheels.yml` を使う。
3. 公開入力で PyPI を選び、`environment: pypi` の承認を通して本番の PyPI へ公開する。
4. 5 つのホイールが揃っていること、`novnow.__version__` が `REF:Cargo.toml` と一致することを確認する。
5. 壊れた版は削除せず、PyPI の yank で印を付ける（仕様 §20.6.3）。

---

## 4. VSCode 拡張版（1 通り・実装済み）

- 識別子は `sonneko.novnow`。市場から入れるか、Releases の `.vsix` から入れる。
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
2. `workflow_dispatch` で `vsce publish` を実行する（タグでは自動公開しない）。
3. 市場の版と `.vsix` の版が同じであることを確認する。

---

## 5. GitHub Actions 版（1 通り・実装済み）

```yaml
- uses: sonneko/novnow@v1
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
2. `v1` をそのコミットへ付け替える（互換性を壊す版を出すときだけ `v2` を作る）。
3. `REF:.github/workflows/action-smoke.yml` は完全なリリースの成功後に自動で走る。手動でやり直すときは `workflow_dispatch` を使う。3 OS で E 診断の失敗も確かめる。

---

## 6. 版を出すときの順序（全体）

仕様 §20.6.1 の順序を、そのままジョブの依存にする。

| 順 | すること | 失敗したら |
|---|---|---|
| 1 | 版番号を `REF:Cargo.toml` で決め、`release-matrix.py --bump` で Python・拡張・Actions の版を揃える | 公開しない |
| 2 | デスクトップ 10 + 照合値を作り、リリースへ添付する | 公開しない（成果物だけ差し替えない） |
| 3 | ホイール 5 を作り、TestPyPI → PyPI へ公開する | 以降を止める。タグは残す |
| 4 | `.vsix` を添付し、市場へ公開する | 以降を止める。市場の版で直す |
| 5 | Actions の `v<版>` と `v1` を公開する | 利用者へ「未提供」と表示する |
| 6 | README・サイト・手引きに実際の取得先を載せ、未公開の形態は「未提供」と記す | 文書だけが古い状態になる。次の版までに直す |

- 版ごとの成果物は削除しない。重大な不具合は新しい版で直す（仕様 §20.6.3）。
- 公開を止めるときは、同じ版の 5 形態をまとめて「未提供」に戻す。形態ごとに残さない。

---

## 7. 確認用の早見表

| 確かめたいこと | 見るもの |
|---|---|
| デスクトップ 10・ホイール 5・`.vsix` 1 が揃っている | `SHA256SUMS` の行数と Releases の添付 |
| 名前が仕様どおり | `novnow-<版>-<対象環境>-<gui\|nogui>` の 1 形だけか |
| 版が 5 形態で同じ | `--version`・`novnow.__version__`・拡張の版・Actions のタグ |
| 照合値が一致する | `sha256sum -c SHA256SUMS`（macOS は `shasum -a 256 -c`） |
| 実行時に通信しない | 起動して何も取得しないこと（プロキシを切って起動する） |
| プロジェクトの外へ書かない | 起動と執筆の前後で、プロジェクトの外のファイル一覧が変わらないこと（履歴ファイルだけが変わる） |
