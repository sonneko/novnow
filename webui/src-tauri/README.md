# webui/src-tauri — 第 2 の画面実装のネイティブ実行（Tauri 2・開発版）

`REF:docs/plans.md#webui-w5`（W5）の置き場所。仕様は `REF:docs/specifications/webui.md` §21.4、
設計は `REF:docs/designs/webui.md` §21.4。同じ `webui/` の画面を、WebView の中で動かす。

- 置き換えが済むまで **配布形態（§20）には入れない**。配るのは CI の成果物（`webui-native-dev`）だけ。
- 版は `0.0.0` のまま（W6 で `Cargo.toml` の `version` に揃える）。

## いまの状態（W5 の範囲）

- 入っているのは **Rust 側**（コマンド 4 つ・許可・窓の設定）と、その検査。
- 画面はまだ `wasmBridge()` を固定で使う（`webui/src/main.ts`。橋渡しの置き場所は L0 の持ち物）。
  `invoke` の橋渡しが入るまでは、**この窓の中でもブラウザ用の橋渡しが動く**。WebView は
  `incognito` なので、その間は何も保存されない（プロジェクトの外に原稿を残さないため）。
  開発版の成果物を配るのは、画面が下の「コマンド」を呼ぶようになってからにする。
- 窓そのもの（Tauri のビルド）は、Tauri のクレートと WebView の開発ライブラリが要る。
  作業サンドボックスでは組めないので、実機か CI で確かめる。

## 構成

| 場所 | 役割 | Tauri が要るか |
|---|---|---|
| `src/main.rs` | コマンド 4 つと Tauri への配線（状態は `Mutex<NativeSession>` 1 つ） | 要る |
| `tauri.conf.json` | 画面（Vite の `dist`）・窓・ビルドの前処理 | 要る |
| `capabilities/default.json` | WebView に与える許可（3 つだけ） | 要る |
| `icons/` | 窓のアイコン（`REF:icon.svg` から作る。下の「アイコン」） | 要る |
| `Cargo.lock` | Tauri 側の依存の固定（下の「lock を揃える」） | 要る |
| `bridge/` | **コマンドの中身**（`Session` の所有・JSON の形・新規作成の確認）。機能核と `serde_json` だけに依存する | 要らない |
| `bridge/examples/stdio.rs` | コマンド 4 つを標準入出力から叩く口（窓なしで試すため） | 要らない |
| `bridge/tests/` | Rust 側のテスト（コマンドの形・新規作成・外部変更・規則 10・スレッド） | 要らない |

`bridge/` を分けてあるのは、Tauri のクレートと WebView の開発ライブラリが無い環境（作業サンドボックス・
CI の `webui` ジョブ）でも、コマンドの中身を組んで試せるようにするため。`src/main.rs` は
`bridge/` の関数をそのまま呼ぶだけにしてある。

## コマンド（設計 §21.4）

4 つだけ。引数も戻り値も **文字列の JSON** で、ブラウザ用の橋渡し
（`REF:crates/novnow-wasm/src/lib.rs`）と同じ形を返す。引数の名前は下の綴りのまま渡す
（`create_options`。camelCase に変えない）。

| コマンド | 引数 | 戻り値 |
|---|---|---|
| `open` | `path`: プロジェクトのディレクトリ。`create_options`: 省くか `null` なら既存を開く。JSON のオブジェクト（`{"title":"…"}`。`{}` でもよい）なら新しく作ってから開く | `run` と同じ形。成功の `result` は `{"locale":"ja","root":"<絶対パス>","version":"<版>"}` |
| `run` | `op`: 正規操作 ID。`args`: 引数の JSON（省ける。空文字・`null` は `{}`） | `{"ok":true,"result":…}` / `{"ok":false,"error":{"code":…,"kind":…,"message":…,"path":…}}` |
| `poll_events` | なし | `AppEvent` の配列（開く前は `[]`） |
| `poll_external` | なし | `run` と同じ形。`result` は `{"changes":[{"change":"updated","path":"…"}],"count":1}` |

```js
// `withGlobalTauri` を立ててあるので、npm の依存なしで呼べる
const { invoke } = window.__TAURI__.core;
const opened = JSON.parse(await invoke('open', { path, create_options: null }));
const info = JSON.parse(await invoke('run', { op: 'project.info', args: '{}' }));
const events = JSON.parse(await invoke('poll_events'));
const external = JSON.parse(await invoke('poll_external'));
// ダイアログ（許可してあるのは選択と保存だけ）
const dir = await window.__TAURI__.dialog.open({ directory: true });
const out = await window.__TAURI__.dialog.save({ filters: [{ name: 'text', extensions: ['txt', 'md'] }] });
```

- **`open`**: 既存を開くときは `Session::open`（§14.2 の起動前チェックと同じ検証。必須のものが
  欠けていれば `project` / `S022` で断る）。開けなかったら、開いていたセッションはそのまま残る。
  新しく作るのは **無いディレクトリか空のディレクトリだけ**（既に何かあれば `io` で断り、1 バイトも
  変えない）。確かめるのは機能核が実際に書く場所（シンボリックリンクと `..` を、機能核と同じ関数で
  解いた先）。
- **`run`**: 機能核（`Session::run`）の答えをそのまま返す。**例外は `project.new` だけ**で、
  `path` に既に何かあれば機能核を呼ばずに `io` で断る（`open` の新規作成と同じ規則）。作れたときの
  戻り値は機能核のまま。`project.new` は作るだけで、開いているセッションは替わらない。新しく
  作って開くなら `open(path, create_options)` を使う。
  **機能核の既知の動き**: `Session::create`（`project.new` もこれを呼ぶ）は雛形をそのまま書くので、
  既存の作品に対して呼ぶと `project.toml`・`characters.md`・`words.md`・`ideas.md`・
  `materials/index.json`・`plots/all.cnov`・`plots/01.cnov` が雛形に置き換わる（§14.5 は
  `novnow new` に「既存ファイルは絶対に上書きしない」を求め、§19.6.1 は `project.new` を同じ
  動作としている）。直すのは機能核の側（`REF:src/app/session.rs`）で、直るまで橋渡しが先に
  確かめて断る（`bridge/tests/create.rs`）。機能核が直ったら、この確認は外せる。
- **`poll_external`**: 画面が **1 秒ごと**に呼ぶ（§8.5）。`changes` は Host が見た差分
  （`Session::poll_external` の戻り値。本文は載せない）で、`change` は `updated` / `removed` /
  `unreadable`（`code` と `message` も持つ）。**セッションの反応は `poll_events` の出来事で受け取る**:
  `document_changed` の `change` が `external`（読み直した）・`conflict`（未保存の変更と衝突）・
  `unchanged`（中身は同じ）、`save_state_changed` の `conflict`、`external_unreadable`。
  **機能核の既知の動き**: 1 ファイルだけの保存は、次の監視で `updated` として現れる（ヘッドレスの
  `FsHost` が、書いたスタンプを覚えない）。中身は同じなので反応は `unchanged` だが、**保存のあと
  監視が回る前に同じファイルを打鍵していると `conflict` になる**。再現は
  `bridge/tests/external.rs` の `a_single_file_save_is_not_an_external_change`（`#[ignore]`）。
  直すのは機能核の側（`REF:src/app/fs_host.rs`）で、ここでは回避していない。
- **panic**: 関門はデバッグビルドで違反を `panic!` にする（§13.1.2）。橋渡しは受け止めて `io` の
  失敗を返し、**セッションを捨てる**（次の操作は `session is not open`。`open` し直す）。
  リリースビルドでは E060 の失敗が返り、セッションは残る。

### 画面の橋渡し（`Bridge`）との対応

`REF:webui/src/lib/bridge/types.ts` の口は、4 つのコマンドだけで満たせる。

| `Bridge` | ネイティブでの実現 |
|---|---|
| `open(root)` | `open`（`root` はディレクトリ。選ぶのは `dialog.open({ directory: true })`） |
| `run` / `pollEvents` | `run` / `poll_events` |
| `flush` | `run` の `save.now`（`FsHost` は同期で書き終える。保存領域へ流す段は無い） |
| `pendingWrites` | 常に 0 |
| `projectId` / `locale` / `version` | `open` の `result` の `root` / `locale` / `version` |
| `setLocale` | `run` の `settings.set`（`key: "language"`） |
| `text` | 生成物の文言カタログ（`webui/src/lib/i18n/catalog.ts`） |
| 外部変更（ブラウザには無い） | 1 秒ごとに `poll_external` → `poll_events` |
| 書き出し先の指定 | `dialog.save(…)` で選んだパスを `export.text` の `out` に渡す |

## 許可と規則 10（§21.4.1）

- 書くのは機能核の `FsHost`（関門 `FsGate`）だけ。`src/` にも `bridge/src/` にも、ファイルに書く
  API は無い（`REF:webui/tests/l6.test.mjs` が `REF:tests/layering.rs` と同じ一覧で確かめる）。
- 許可は `core:default`・`dialog:allow-save`・`dialog:allow-open` の 3 つ。`fs` / `http` / `shell` /
  `updater` / `autostart` のプラグインは足さない（`tauri-plugin-fs` は dialog が型を借りるために
  `Cargo.lock` に現れるが、初期化も許可もしていない）。
- WebView は `incognito`（永続の保存領域を持たない）。設定・キャッシュ・履歴をプロジェクトの外に
  残さないため。
- 窓は OS の既定の見た目のまま（独自の枠・タイトルバーを作らない）。自動更新・更新確認は無い。
- `window.confirm` / `window.alert` は使えない（dialog のプラグインが非同期の版に差し替えるが、
  `dialog:allow-message` を与えていない。`confirm()` は Promise を返すので常に真になる）。

## 試す（Tauri 無しで動く範囲）

```bash
# 受け入れ（ブラウザ版と同じ JSON・同じファイル）と、Rust 側のテストをまとめて
cd webui && node --test tests/l6.test.mjs

# Rust 側だけ。依存の版は root の Cargo.lock に合わせる（`bridge/Cargo.lock` は置かない）
cd webui/src-tauri/bridge && cp ../../../Cargo.lock Cargo.lock && cargo test
```

## 窓を動かす（Tauri が要る）

Linux は `libwebkit2gtk-4.1-dev`・`libappindicator3-dev`・`librsvg2-dev`（`REF:docs/developers.md` §2.1）。
`pkg-config` が `dbus-1` を見つけられない環境では `libdbus-1-dev` も入れる（tauri の既定の feature
`dbus` が、`tao` → `libdbus-sys` を通して、組むときに探す）。
Tauri の CLI は無くてもよい（`beforeDevCommand` / `beforeBuildCommand` は CLI を使う人のための設定）。
下の `cargo build` は、CLI（2.12.4）の `tauri build --no-bundle` が走らせる
`cargo build --bins --features tauri/custom-protocol --release` に `--locked` を足したもの。

```bash
# 開発: Vite の開発サーバ（ポート 5273）を見る
cd webui && npm run dev                      # 端末 1
cd webui/src-tauri && cargo run              # 端末 2

# 開発版の実行ファイル（CI の成果物 `webui-native-dev` の中身）
bash scripts/webui-build.sh && (cd webui && npm ci && npm run build)
cd webui/src-tauri && cargo build --release --locked --features tauri/custom-protocol
# → target/release/novnow-webui-native（Windows は .exe）
```

## lock を揃える

機能核の依存は、ブラウザ版（root の `Cargo.lock`）と同じ版で固定する（別の版で組むと、同じ操作が
同じファイルを書く保証が崩れる）。root の `Cargo.lock` や機能核の版が変わったら作り直す。

```bash
cd webui/src-tauri && cp ../../Cargo.lock Cargo.lock && cargo update --workspace
WEBUI_NATIVE_STRICT=1 node --test ../tests/l6.test.mjs   # 版の食い違いを「省く」でなく失敗にする
```

## アイコン

`icons/icon.png`（512px・RGBA。Unix の窓）と `icons/icon.ico`（Windows）は `REF:icon.svg` から作る。

```bash
# @resvg/resvg-js で icon.svg を 512 / 256 / 64 / 48 / 32 / 16 px の PNG にしてから
convert icon-256.png icon-64.png icon-48.png icon-32.png icon-16.png icons/icon.ico
```
