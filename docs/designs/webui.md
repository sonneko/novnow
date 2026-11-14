# novnow 設計書 — 第 2 の画面実装（Web UI とネイティブ実行）

本書は `REF:docs/specifications/webui.md` の各節に対応する実装設計を記述する。実装順序・受け入れ基準は `REF:docs/plans.md#webui` に置く。第 1 の画面実装（egui + trunk の Web 版 / eframe のデスクトップ版）は**そのまま維持**し、本書の第 2 の画面実装を並行して作る。既存の `REF:src/ui/` は変更しない。第 2 の本体は新しい依存（Node / Svelte / Tauri）で作るため、`REF:scripts/verify.sh` の対象を段階的に広げる（`REF:docs/plans.md#webui` の W6）。

## 21. 第 2 の画面実装

第 2 の画面実装は 3 つの層で作る。**同じ 1 つの機能核（`REF:src/app/mod.rs` のセッションと 121 操作）**を、2 つの実行環境から同じ JSON の口で叩く。

| 層 | 置き場所 | 役割 | 実行環境 |
|---|---|---|---|
| 画面 | `REF:webui/...`（Svelte + TypeScript + Vite） | 描画・入力・レイアウト・i18n・テーマ。ドメイン規則を持たない | ブラウザ / Tauri の WebView |
| 橋渡し（ブラウザ） | `REF:crates/novnow-wasm/...`（wasm-pack の cdylib） | `Session` を wasm-bindgen で包む。Host は IndexedDB | ブラウザ |
| 橋渡し（ネイティブ） | `REF:webui/...`（Tauri 2。未作成） | コマンドの中で `Session` を所有する。Host はファイルシステム | デスクトップ |

- **正規操作の口は 1 つ**: どちらの橋渡しも `Session::run(op, args) -> Value`（`REF:src/app/session.rs`）と同じ JSON を返す。画面は操作 ID と JSON だけを知り、`core` の型を知らない（§19.1.4 の単一の真実）。
- **画面は 2 実装ではない**: ブラウザと Tauri は同じ `webui/` を配る。差分は `Bridge` の実装（`wasm` / `invoke`）だけに閉じる。画面側に `if (isTauri)` を散らさない。
- **egui 版との共有**: 文言カタログ・テーマの色・操作カタログは、既存の正本から**生成**して使う。手で写さない（§21.2）。

### 21.1 位置づけと適用範囲

状態: 未実装・実装計画済み（REF:docs/plans.md#webui, REF:docs/plans.md#webui-w1）

- **並行運用**: `REF:src/ui/` は凍結しない。修正（F 系・B 系）は従来どおり第 1 に入れる。第 2 は「同じ版の同じ機能核」を別の描画面から使うので、`core` / `app` の変更は自動的に両方へ効く。
- **系統の識別**: 画面は操作 ID の呼び出しに系統の情報を足さない（§19.1.5 の関門は人間経路として通さない）。系統の違いは保存領域（§21.3）と配布物（§21.4）だけに現れる。
- **版**: 第 2 は `REF:Cargo.toml` の `version` を唯一の正本とする。`webui/package.json` と `webui/src-tauri/tauri.conf.json` の版は、`REF:scripts/release-matrix.py --bump` の対象に後から加える（W6。それまでは 0.0.0 のまま開発し、公開しない）。
- **画面仕様の写像**: §9〜§12 の各節を Svelte のコンポーネントへ 1 対 1 で対応させる。対応表は `REF:webui/...`（未作成）に置く。実装が進んだ時点で作り、本書へは書かない（先に書くと腐る）。
- **未提供の明示**: 未実装の画面は「この版では使えません」を出す（文言は `REF:src/ui/i18n/ja.toml` の `workspace.unavailable` を使う）。空の画面・動かないボタンを置かない。

### 21.2 画面の要件

状態: 未実装・実装計画済み（REF:docs/plans.md#webui, REF:docs/plans.md#webui-w2）

- **コンポーネントの骨格**: `webui/src/lib/shell/`（`Header.svelte` / `StatusBar.svelte` / `Splitter.svelte`）、`webui/src/lib/context/ContextView.svelte`、`webui/src/lib/editor/SectionEditor.svelte`、`webui/src/lib/reference/ReferencePane.svelte`、`webui/src/lib/workspaces/`（9 ワークスペース）。分割比・折り畳みの規則は §9.1 のまま、`.novnow/ui.json` に保存する（保存は `settings.set` 操作を通す）。
- **エディタ**: `SectionEditor` は `<textarea>` を使わず、行単位の `<div>` と hidden の入力要素で組む（構文ハイライト・下線・インレイヒントを重ねるため）。キャレット位置は `position.*` 系の操作で UTF-16 コード単位に変換して扱う（§3.4 の位置表現）。診断の波線は行内の範囲を span の背景で描く。
- **IME**: `compositionstart` / `compositionupdate` / `compositionend` を取り、変換中は `keydown` のハンドラを早期 return する（§9.8.5）。終了イベントが来ない場合に備え、最後の変換イベントから 2 秒で解除するタイマを持つ。変換候補の位置は入力要素の位置で決まるので、キャレット追従の hidden 要素を常にキャレット直下へ置く。
- **キー**: `keydown` を 1 か所（`webui/src/lib/keys/`）で受けて `Command`（`REF:src/app/command.rs` と同じ名前）へ写す。修飾キーの順序は第 1 と同じ規則（`Ctrl+Shift+…` を `Ctrl+…` より先に判定する。`REF:docs/developers.md` §8.7 の 20）。
- **ダイアログ**: ブラウザは `<input type="file">` と Blob ダウンロードで書き出す（`REF:src/ui/web.rs` と同じ考え方）。Tauri は `tauri-plugin-dialog` の保存・選択を使い、選ばれたパスを `export.text` の `out` に渡す（§13.1.1 の特例の口をそのまま使う）。
- **外部変更**: ブラウザは監視しない（§13.3）。Tauri は `poll` を 1 秒ごとに回し、`session.poll_external` と同じ差分を反映する。
- **文言**: `scripts/i18n-ts.py`（未作成。計画は `REF:docs/plans.md#webui-w3`） が `REF:src/ui/i18n/ja.toml` と `REF:src/ui/i18n/en.toml` から `webui/src/lib/i18n/catalog.ts` を生成する（W3）。第 2 用の文言を手で書かない。プレースホルダは `{0}` の形をそのまま保つ。
- **テーマ**: `scripts/theme-ts.py`（未作成。計画は `REF:docs/plans.md#webui-w3`） が `REF:docs/specifications/ui.md` §11.1 の表から `webui/src/lib/theme/tokens.ts` を生成する。CSS 変数（`--bg-base` など）に写し、コンポーネントは変数だけを参照する。第 1 の `REF:src/ui/theme.rs` と同じ表を読むので、色の食い違いは表の変更時に同時に起きる。
- **アクセシビリティ**: 押せる要素は `<button>` / `<a>` を使い、アイコンだけの要素に `aria-label` を付ける。当たり判定は `::before` の絶対配置で 24×24px 以上に広げる（§9.9.2 の 3、見た目を変えない）。

### 21.3 ブラウザでの提供

状態: 未実装・実装計画済み（REF:docs/plans.md#webui-w4）

- **ビルド**: `webui/` を Vite で静的ファイルにし、`scripts/webui-build.sh`（未作成。計画は `REF:docs/plans.md#webui-w4`） が `dist/` へ出す（W4）。第 1 のブラウザ版（Trunk の `REF:Trunk.toml`）は `dist/` の別ディレクトリ（`app/`）に置き、入口を分ける。第 2 は `webui/` 配下（`sonneko.github.io/novnow/webui/`）に出し、`REF:index.html` とサイトの導線から辿れるようにする。
- **WASM の組立**: `REF:crates/novnow-wasm/...` は `crate-type = ["cdylib", "rlib"]`、`wasm-bindgen` を使い、`wasm-pack build --target web` で `pkg/` を出す。公開する型は 3 つに絞る。
  - `NovnowSession`: `open(root_text)` / `run(op, args_json) -> string` / `poll_events() -> string` / `flush()` / `locale` の getter・setter。
  - `open_project_files(files_json)`: Zip 取り込みの入力を渡す（`REF:src/adapters/zip.rs` を共有）。
  - `version()`: 版文字列（`session.version()`）。
- **Host の実装**: ブラウザ用の Host（`REF:crates/novnow-wasm/...`（未作成）の中）は、既存の `REF:src/adapters/storage_idb.rs` をそのまま使えない（同期の `Host` と非同期の IndexedDB の差）。そこで「起動時にプロジェクトの全文をメモリへ読み、書き込みはメモリへ反映して IndexedDB へ非同期で流す」形にする（`REF:src/app/memory_host.rs` を土台にし、書き込みキューを 1 本持つ）。読み取りは常にメモリから返すので `Session` は同期のままでよい。書き込みの失敗は `poll_events` の次回に `SaveStateChanged(state=error)` として返す。
- **保存領域の分離**: 第 2 のブラウザ版は IndexedDB の DB 名を `novnow-webui`（検証版は `novnow-webui-nightly`）にする（§13.3.2 の `novnow` と共有しない。§21.3.2）。`app` ストアの許可キーは `recent_projects` だけのままにする。
- **PWA**: `webui/` のビルドにも `manifest.webmanifest` と Service Worker を持たせる（`REF:scripts/pwa-assets.py` の考え方で、ハッシュ付き資源を先に固定する）。第 1 の `REF:sw.js` とはスコープが違うので互いに干渉しない。

### 21.4 ネイティブ実行での提供

状態: 未実装・実装計画済み（REF:docs/plans.md#webui-w5）

- **Tauri の置き場所**: `REF:webui/...`（未作成）。`tauri.conf.json` の `frontendDist` は Vite の `dist`。`beforeDevCommand` / `beforeBuildCommand` は `npm run dev` / `npm run build`。
- **コマンド**: `webui/src-tauri/src/main.rs` に 4 つだけ置く。`open(path, create_options)`, `run(op, args)`, `poll_events()`, `poll_external()`。中では `Session` を 1 つだけ `Mutex` で持つ。引数・戻り値は JSON 文字列で、ブラウザ側の `run` と同じ形にする（画面から見て 2 つの橋渡しが同じ）。
- **Host**: `REF:src/app/fs_host.rs` をそのまま使う（デスクトップの第 1 と同じ関門 `FsGate` を通る）。新しいアダプタを書かない。したがって規則 10 の検査（`REF:tests/layering.rs` の書き込み API の検査）はそのまま効く。
- **権限**: `capabilities/default.json` で許可するのは `core:default`・`dialog:allow-save`・`dialog:allow-open` だけにする。`fs` プラグインは**使わない**（ファイル入出力は Rust 側の `Host` を通す）。HTTP・シェル・自動起動の許可を置かない（§21.4.1）。
- **配布物**: 置き換えが済むまで §20 の 5 形態に加えない。開発版は CI の成果物（`webui-native-dev`）としてだけ出す（W5）。名前・同梱物・署名の扱いは §20 の改訂時に決める。`REF:action.yml`・`REF:scripts/release-matrix.py` は触らない。
- **窓**: 幅 900px 未満で参照ペインを畳む規則（§9.1）は WebView の幅で判定する。`titleBarStyle` などの OS 固有の見た目は既定のままにし、独自の枠を作らない（§20.1.2 の「配布物の作りを分ける軸を置かない」に合わせる）。
- **IME**: WebView（WebKit / WebView2）の IME は §9.8.5 の規則で扱える。Tauri 側で入力方式を横取りしない。実機確認は `REF:docs/developers.md` §4.9 の表に 1 行足す。

### 21.5 機能パリティと移行の完了条件

状態: 未実装・実装計画済み（REF:docs/plans.md#webui-w7, REF:docs/plans.md#webui-parity）

- **判定の作り方**: `REF:crates/novnow-wasm/...`（未作成）の Rust 側テストではなく、Node 側の回帰（`webui/tests/parity.mjs`）で判定する。`REF:tests/golden/api_catalog.json` の `operations` を列挙し、画面が `run` を呼べる操作 ID の集合と突き合わせる。画面の操作 ID は TS の定数（`webui/src/lib/ops.ts`）に 1 か所だけ書き、テストがカタログとの差を出す。
- **画面仕様の未提供一覧**: `webui/src/lib/coverage.ts` に §9〜§12 の節番号と状態（`done` / `pending`）を持つ表を置く。`pending` の件数を `webui/tests/coverage.mjs` が数え、0 件でなければ置き換えの判定を通さない。表はリリースノートとサイトへ同じ値で出す（§21.5.1 の公開）。
- **置き換えの実施**: 置き換えは版更新の PR の中で行い、`REF:docs/specifications/external.md` §20 の改訂と同じ PR に載せる。第 1 の成果物の生成を止めるだけで、公開済みの版の成果物は消さない（`REF:scripts/release-check.sh` の判定は据え置き）。
- **移行**: 作品の移行は Zip の入出力だけを使う（`import` / `export` の操作）。ブラウザの保存領域どうしを直接読む処理を書かない（W7 の判定でも確認する）。
