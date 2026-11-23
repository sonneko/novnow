# novnow 設計書 — 第 2 の画面実装（Web UI とネイティブ実行）

本書は `REF:docs/specifications/webui.md` の各節に対応する実装設計と現状を記述する。実施状況・残件は `REF:docs/plans.md#webui` にまとめる。第 1 の画面実装（egui + trunk の Web 版 / eframe のデスクトップ版）は維持し、第 2 は並行して開発・提供する。第2のために既存の `REF:src/ui/` は変更しない。W6 で `REF:scripts/verify.sh` と CI に WebUI 検査を配線済み。

## 21. 第 2 の画面実装

第 2 の画面実装は 3 つの層で作る。**同じ 1 つの機能核（`REF:src/app/mod.rs` のセッションと 121 操作）**を、2 つの実行環境から同じ JSON の口で叩く。

| 層 | 置き場所 | 役割 | 実行環境 |
|---|---|---|---|
| 画面 | `REF:webui/...`（Svelte + TypeScript + Vite） | 描画・入力・レイアウト・i18n・テーマ。ドメイン規則を持たない | ブラウザ / Tauri の WebView |
| 橋渡し（ブラウザ） | `REF:crates/novnow-wasm/`（cdylib） | Cargo と `wasm-bindgen` で `Session` を包む。Host は IndexedDB | ブラウザ |
| 橋渡し（ネイティブ） | `REF:webui/src-tauri/`（Tauri 2）・`REF:webui/src-tauri/bridge/` | `NativeSession` が `Session` を所有し、Tauri command は JSON を返す。実装とテストは存在するが、現在の `webui/src/main.ts` は `wasmBridge()` 固定で native bridge を画面に接続していない（§20.6.5） | 開発用デスクトップ基盤 |

- **正規操作の口は 1 つ**: どちらの橋渡しも `Session::run(op, args) -> Value`（`REF:src/app/session.rs`）と同じ JSON を返す。画面は操作 ID と JSON だけを知り、`core` の型を知らない（§19.1.4 の単一の真実）。
- **画面は 2 実装ではない**: ブラウザと Tauri が提供対象になった場合は同じ `webui/` を配る。差分は `Bridge` の実装（`wasm` / `invoke`）だけに閉じる。現状は `wasmBridge()` のみ接続され、native invoke 配線は未実装。画面側に `if (isTauri)` を散らさない。
- **egui 版との共有**: 文言カタログ・テーマの色・操作カタログは、既存の正本から**生成**して使う。手で写さない（§21.2）。

### 21.1 位置づけと適用範囲

状態: 実装完了（REF:src/app/session.rs, REF:webui/src/main.ts, REF:webui/src/lib/coverage.ts）

第2の画面とブラウザ基盤は実装済み。配布・パリティの現状と未到達の画面仕様は §21.3 / §21.4 / §21.5 と仕様 §20.6.5 を参照。

- **並行運用**: `REF:src/ui/` は凍結しない。修正（F 系・B 系）は従来どおり第 1 に入れる。第 2 は「同じ版の同じ機能核」を別の描画面から使うので、`core` / `app` の変更は自動的に両方へ効く。
- **系統の識別**: 画面は操作 ID の呼び出しに系統の情報を足さない（§19.1.5 の関門は人間経路として通さない）。系統の違いは保存領域（§21.3）と配布物（§21.4）だけに現れる。
- **版**: 第 2 は `REF:Cargo.toml` の `version` を唯一の正本とする。`scripts/release-matrix.py --bump` が `webui/package.json`・`package-lock.json`・`webui/src-tauri/tauri.conf.json` の版も一緒に更新し、`--check` がずれを検出する（W6 で実装済み）。
- **画面仕様の写像**: §9〜§12 の節のうち到達対象・既知ギャップを `webui/src/lib/coverage.ts` の `SPEC_GAPS` として管理する。§8〜§12を網羅する節別の画面対応表は未作成。
- **未提供の明示**: 未実装の画面は「この版では使えません」を出す（文言は `REF:src/ui/i18n/ja.toml` の `workspace.unavailable` を使う）。空の画面・動かないボタンを置かない。

### 21.2 画面の要件

状態: 実装中（REF:webui/src/lib/shell/, REF:webui/src/lib/editor/, REF:webui/src/lib/workspaces/, REF:webui/src/lib/coverage.ts）

シェル・エディタ・9ワークスペース・IME/キー配線は実装済み。仕様全体の到達判定は §21.5、残る4件は仕様 §20.6.5 を参照。

- **コンポーネントの骨格**: `webui/src/lib/shell/`（`Header.svelte` / `StatusBar.svelte` / `Splitter.svelte`）、`webui/src/lib/context/ContextView.svelte`、`webui/src/lib/editor/SectionEditor.svelte`、`webui/src/lib/reference/ReferencePane.svelte`、`webui/src/lib/workspaces/`（9 ワークスペース）。分割比・折り畳みの規則は §9.1 のまま、`.novnow/ui.json` に保存する（保存は `settings.set` 操作を通す）。
- **エディタ**: `SectionEditor` は `<textarea>` を使わず、行単位の `<div>` と hidden の入力要素で組む（構文ハイライト・下線・インレイヒントを重ねるため）。キャレット位置は `position.*` 系の操作で UTF-16 コード単位に変換して扱う（§3.4 の位置表現）。診断の波線は行内の範囲を span の背景で描く。
- **IME**: `compositionstart` / `compositionupdate` / `compositionend` を取り、変換中は `keydown` のハンドラを早期 return する（§9.8.5）。終了イベントが来ない場合に備え、最後の変換イベントから 2 秒で解除するタイマを持つ。変換候補の位置は入力要素の位置で決まるので、キャレット追従の hidden 要素を常にキャレット直下へ置く。Playwright による実機相当の画面全体 IME 検証は未作成（`REF:docs/developers.md` §7.5）。
- **キー**: `keydown` は `webui/src/lib/keyboard/keys.ts` で受けて `Command`（`REF:src/app/command.rs` と同じ名前）へ写す。修飾キーの順序は第 1 と同じ規則（`Ctrl+Shift+…` を `Ctrl+…` より先に判定する。`REF:docs/developers.md` §8.7 の 20）。
- **ダイアログ**: ブラウザは `<input type="file">` と Blob ダウンロードで書き出す（`REF:src/ui/web.rs` と同じ考え方）。Tauri は `tauri-plugin-dialog` の保存・選択を使い、選ばれたパスを `export.text` の `out` に渡す（§13.1.1 の特例の口をそのまま使う）。
- **外部変更**: ブラウザは監視しない（§13.3）。native bridge は `poll_external` を提供し、画面が 1 秒ごとに呼ぶ設計だが、現在の `Bridge` 型・`webui/src/main.ts` には接続されておらず、画面からの監視は未提供である（仕様 §20.6.5）。
- **文言**: `scripts/i18n-ts.py` が `REF:src/ui/i18n/ja.toml` と `REF:src/ui/i18n/en.toml` から `webui/src/lib/i18n/catalog.ts` を生成する（W3 で実装済み）。第 2 用の文言を手で書かない。プレースホルダは `{0}` の形をそのまま保ち、`--check` で生成物との一致を検査する。
- **テーマ**: `scripts/theme-ts.py` が `REF:docs/specifications/ui.md` §11.1 の表から `webui/src/lib/theme/tokens.ts` を生成する（W3 で実装済み）。CSS 変数（`--bg-base` など）に写し、コンポーネントは変数だけを参照する。第 1 の `REF:src/ui/theme.rs` と同じ表を読み、`--check` で一致を検査する。
- **アクセシビリティ**: 押せる要素は `<button>` / `<a>` を使い、アイコンだけの要素に `aria-label` を付ける。当たり判定は `::before` の絶対配置で 24×24px 以上に広げる（§9.9.2 の 3、見た目を変えない）。

### 21.3 ブラウザでの提供

状態: 実装完了（REF:src/adapters/storage_idb.rs, REF:.github/workflows/deploy.yml, REF:.github/workflows/nightly.yml, REF:scripts/webui-build.sh）

ブラウザ版のビルド・PWA・`webui/` と `nightly-webui/` の配備は実装済み。画面仕様の未到達は仕様 §20.6.5 を参照。

- **ビルド**: `webui/` を Vite で静的ファイルにし、`REF:scripts/webui-build.sh`（wasm の組立）と `npm run build --prefix webui`（Vite）が `webui/dist/` へ出す（W4）。第 1 のブラウザ版（Trunk の `REF:Trunk.toml`）は `dist/` の別ディレクトリ（`app/`）に置き、入口を分ける。第 2 は `webui/` 配下（`sonneko.github.io/novnow/webui/`）に出し、`REF:index.html` とサイトの導線から辿れるようにする。
- **WASM の組立**: `REF:crates/novnow-wasm/` は `crate-type = ["cdylib", "rlib"]` と `wasm-bindgen` を使う。`scripts/webui-build.sh` が `wasm32-unknown-unknown` 向けに Cargo でビルドし、`wasm-bindgen --target web` で `webui/src/wasm/` へ生成する（W2・W4 で実装済み。生成物は Git に入れない）。公開する型は 3 つに絞る。
  - `NovnowSession`: `open(root_text)` / `run(op, args_json) -> string` / `poll_events() -> string` / `flush()` / `locale` の getter・setter。
  - `open_project_files(files_json)`: Zip 取り込みの入力を渡す（`REF:src/adapters/zip.rs` を共有）。
  - `version()`: 版文字列（`session.version()`）。
- **Host の実装**: ブラウザ用 Host は `REF:crates/novnow-wasm/src/host.rs` と `REF:crates/novnow-wasm/src/storage.rs` に実装済みである。同期の `Host` と非同期 IndexedDB の差を吸収するため、起動時にプロジェクトをメモリへ読み、書き込みはメモリの正本へ反映して IndexedDB のキューへ流す（`REF:src/app/memory_host.rs` が先例）。読み取りはメモリから返し、書き込みの失敗は `flush` / `poll_events` で `SaveStateChanged(state=error)` として返す。
- **保存領域の分離**: 第 2 のブラウザ版は IndexedDB の DB 名を `novnow-webui`（検証版は `novnow-webui-nightly`）にする（§13.3.2 の `novnow` と共有しない。§21.3.2）。`app` ストアの許可キーは `recent_projects` だけのままにする。
- **PWA**: `webui/` のビルドにも `manifest.webmanifest` と Service Worker を持たせる（`REF:scripts/pwa-assets.py` の考え方で、ハッシュ付き資源を先に固定する）。第 1 の `REF:sw.js` とはスコープが違うので互いに干渉しない。配置は `REF:webui/public/` の 2 ファイル（`sw.js`・`manifest.webmanifest`）で、Vite が `webui/dist/` の直下へ写す。登録は `REF:webui/index.html` が `./manifest.webmanifest` の読み込みと `./sw.js`（スコープ `./`）の登録を行う。キャッシュ名は `self.registration.scope` を含める（`novnow-webui-<入口>`）ので、`/novnow/webui/` と `/novnow/nightly-webui/` のキャッシュは入口ごとに分かれる。第 1 の worker が資源一覧を封入して全件取得成功で activate するのに対し、第 2 は入口の `index.html` を install で固定し、以降の GET を随時キャッシュする（§13.3.7）。
- **Pages の 2 チャネル配置**: GitHub Pages の入口は 4 つ（`app/`・`webui/`・`nightly-app/`・`nightly-webui/`）。第 2 は正式版が `webui/`、開発版が `nightly-webui/` で、ビルドは 1 つ（`webui/dist/`）を両方へ写す（`REF:.github/workflows/deploy.yml`・`REF:.github/workflows/nightly.yml`）。入れ替えの規則は `REF:scripts/channel-pages.py` が持つ: `nightly` の公開は `nightly-webui/` だけを `REF:scripts/publish-channel.sh` が渡す `--incoming-webui` の木で差し替え、`stable` の公開は `webui/` を置いたうえで `nightly-webui/` を `webui/` の写しでリセットする。payload の検証名は `novnow-nightly-webui.tar.gz`（`webui/dist/` を tar にしたもの）。入口の一覧・起動は `REF:scripts/website-check.py`（`--website-only`）と `REF:scripts/pages-e2e.mjs` が見る。
- **CI の配線**: `REF:.github/workflows/ci.yml` の `docs` ジョブの変更フィルタは `webui/**` のときに `webui_changed=true` を立て、`crates/novnow-wasm/*` のときは `rust_changed` と `webui_changed` の両方を立てる（第 2 の wasm の実体は `REF:crates/novnow-wasm/` にあり、画面だけを直したときに Rust ジョブを回さないため）。`webui` ジョブは `REF:scripts/webui-check.sh --full` に続けて `cargo test --manifest-path webui/src-tauri/bridge/Cargo.toml` を実行する。ブリッジのクレート名は `novnow-native-bridge`（`REF:webui/src-tauri/bridge/Cargo.toml`）で、Tauri に依存しないので WebView の開発ライブラリが無い CI でも組める。手元の `REF:scripts/verify.sh` は `webui` ジョブで `REF:scripts/webui-check.sh`（`--full` なし）だけを回し、Node 22 と本番ビルドを要する部分とブリッジのテストは CI に任せる（同じところまで手元で見るときは `bash scripts/webui-check.sh --full` とブリッジの `cargo test`）。

### 21.4 ネイティブ実行での提供

状態: 実装中（REF:webui/src-tauri/src/main.rs, REF:webui/src-tauri/bridge/src/, REF:webui/src-tauri/bridge/tests/）

Tauri のアプリ設定・command と `NativeSession` bridge、bridge のテストは実装済み。Tauri 画面から bridge を呼ぶ配線と配布物のビルドは未完了であり、§9.6 / §9.7 の native 画面経路は仕様 §20.6.5 の制限に含む。

- **Tauri の置き場所**: `REF:webui/src-tauri/` にアプリ設定・capability・起動コードがあり、`REF:webui/src-tauri/bridge/` に `NativeSession` を実装している。`tauri.conf.json` の `frontendDist` は Vite の `dist`。`beforeDevCommand` / `beforeBuildCommand` は `npm run dev` / `npm run build`。ただし `webui/src/main.ts` は現在 `wasmBridge()` 固定であり、Tauri invoke bridge は画面から未使用である。
- **コマンド**: `webui/src-tauri/src/main.rs` に 4 つだけ置く。`open(path, create_options)`, `run(op, args)`, `poll_events()`, `poll_external()`。中では `Session` を 1 つだけ `Mutex` で持つ。引数・戻り値は JSON 文字列で、ブラウザ側の `run` と同じ形にする（画面から見て 2 つの橋渡しが同じ）。
- **Host**: `REF:src/app/fs_host.rs` をそのまま使う（デスクトップの第 1 と同じ関門 `FsGate` を通る）。新しいアダプタを書かない。したがって規則 10 の検査（`REF:tests/layering.rs` の書き込み API の検査）はそのまま効く。
- **権限**: `capabilities/default.json` で許可するのは `core:default`・`dialog:allow-save`・`dialog:allow-open` だけにする。`fs` プラグインは**使わない**（ファイル入出力は Rust 側の `Host` を通す）。HTTP・シェル・自動起動の許可を置かない（§21.4.1）。
- **配布物**: Tauri のアプリソースは存在するが、現在の CI は native アプリ成果物をビルドせず、bridge crate のテストを実行する。§20.6.4 の nightly 成果物は 4 入口のブラウザ版・CLI・VSIX であり、native Tauri アプリは含まない。置き換え前の第2の native 実行は §21.4.2 に従い正式な配布形態に加えず、配布物の追加・名前・同梱物・署名は別途承認なしに決めない。`REF:action.yml`・`REF:scripts/release-matrix.py` は変更しない。
- **窓**: 幅 900px 未満で参照ペインを畳む規則（§9.1）は WebView の幅で判定する。`titleBarStyle` などの OS 固有の見た目は既定のままにし、独自の枠を作らない（§20.1.2 の「配布物の作りを分ける軸を置かない」に合わせる）。
- **IME**: WebView（WebKit / WebView2）の IME は §9.8.5 の規則で扱える。Tauri 側で入力方式を横取りしない。実機確認は `REF:docs/developers.md` §4.9 の表に 1 行足す。

### 21.5 機能パリティと移行の完了条件

状態: 実装中（REF:webui/tests/parity.mjs, REF:webui/src/lib/coverage.ts, REF:tests/golden/api_catalog.json）

API パリティ判定は実装済み（119/119 対象操作、PENDING 0）。§9〜§12 の画面仕様には4件の SPEC_GAPS が残るため §21.5.2 #1 は未達で、実機確認の #3 も未実施。§20 改訂の #2 は PR #84 で承認済みだが、置き換え判断ではない。

- **API パリティの判定**: Node 側の回帰（`webui/tests/parity.mjs`）が `REF:tests/golden/api_catalog.json` の `operations` と、`webui/src/lib/bridge/catalog.ts` に列挙した操作 ID を突き合わせる。現在の結果はカタログ 121 操作中、到達 119・未提供 0・仕様上除外 2（`session.poll_events` / `api.list`）、判定 `equivalent`。これは API 操作単位の判定であり、画面仕様の4件の未到達（仕様 §20.6.5）を解消せず、§21.5.2 #1 の達成を意味しない。
- **画面仕様の未提供一覧**: `webui/src/lib/coverage.ts` は API 操作の `PENDING` と、別枠の画面仕様 `SPEC_GAPS` を持つ。`parity.mjs` が API の PENDING と到達集合を照合し、`b7-gaps.test.mjs` / `b8-closure.test.mjs` が選定した画面仕様上の根拠と4件の SPEC_GAPS 維持を検査する。画面仕様の4件は仕様 §20.6.5 に文書化済みであり、PENDING 0 だけをもって置き換え可とはしない。
- **置き換えの実施**: §21.5.2 の5条件がすべて満たされ、sonneko が置き換えを判断するまで行わない。§20 改訂は PR #84 で別途承認済みだが、その承認は置き換え判断ではない。現在 #1（未提供 0 件）は4件の SPEC_GAPS により未達、#3（実機確認）は未実施である。第1の公開物は §20.6.3 に従って取り下げない。
- **移行**: 作品の移行は Zip の入出力だけを使う（`import` / `export` の操作）。ブラウザの保存領域どうしを直接読む処理を書かない（W7 の判定でも確認する）。
