# novnow 仕様書 — 外部インターフェース（ヘッドレス全機能 API 群）

本書は、GUI を起動せずに novnow の全機能へ到達するための 5 つの外部インターフェース（Rust API・Python API・LSP・MCP サーバ・CLI）を規定する。本書の目的は「GUI でできることは、例外列に印を付けたものを除き、どのインターフェースからも同じ結果でできる」ことを保証することである。

| インターフェース | 提供形態 | 主な利用者 |
|---|---|---|
| Rust API | ライブラリ（インプロセス） | Rust プログラム、プラグイン、上位ツール |
| Python API | ネイティブ拡張モジュール（インプロセス） | Python スクリプト・ノートブック・自動化 |
| LSP | 標準プロトコルのサーバ（stdio） | 任意のエディタ・IDE |
| MCP サーバ | 標準プロトコルのサーバ（stdio） | AI エージェント（§15 の拡張） |
| CLI | サブコマンド（stdout / stderr） | シェル、スクリプト、CI |

## 19. 外部インターフェース（ヘッドレス全機能 API 群）

### 19.1 共通原則

#### 19.1.1 提供形態と可用性

- 5 つのインターフェースはすべて GUI なしで動作しなければならない（MUST）。「GUI なし」とは、ウィンドウ・描画・フォント・ネイティブファイルダイアログのいずれも必要としないことを意味する。
- Rust API は本体と同名・同版のライブラリとして公開する。ライブラリ利用時に GUI 部を構成要素として必要としない（MUST）。
- Python API は本体と同じ名前のライブラリとしてパッケージレジストリ（PyPI）で公開する（MUST）。本体は**ネイティブ拡張モジュール**であり、Python プログラムから関数呼び出しの速さで直接利用できる。ラッパーはサブプロセスやネットワークを介さない（MUST）。
- LSP サーバ・MCP サーバは標準入力／標準出力で通信するサブコマンドとして提供する（MUST）。ネットワークポートを開かない（§15.5 の localhost 専用サーバの方針と同じ）。
- CLI は既存のサブコマンド（§14.1）を拡張する形で提供する（MUST）。
- 5 つのインターフェースは同一の機能核を共有し、同じ入力に対して同じ可視結果（ファイル内容・診断・出力データ）を返す（MUST）。

#### 19.1.2 全機能パリティ

- GUI が提供する**すべての機能**は、§19.7 のカタログの操作として、例外列に記載されたインターフェースを除くすべてのインターフェースから到達可能でなければならない（MUST）。
- 純粋な表示・対話の機構（ウィンドウ描画、フォーカス枠、トースト表示、アニメーション、IME、ドラッグ操作そのもの、マウス座標）は「機能」に含めない。ただしそれらが**表示するデータ**（診断一覧、統計、提案の差分など）はすべて操作として提供する。
- 将来追加される機能は、提供と同時にこのカタログへ同じ規則で加えなければならない（MUST）。

#### 19.1.3 正規操作 ID と命名規則

すべての機能は**正規操作 ID**（例 `section.get`、`agent.task_run`）で一意に識別する。ID は `<ドメイン>.<動詞>` 形式（小文字 ASCII、区切りは `_`）とし、各インターフェースは次の規則で ID を表面化する（MUST）:

| インターフェース | 命名規則 | 例（`idea.capture`） |
|---|---|---|
| Rust API | `session.<ドメイン>().<動詞>(…)`。多階層ドメインは動詞側に畳む | `session.idea().capture(…)` |
| Python API | `session.<ドメイン>.<動詞>(…)` | `session.idea.capture(...)` |
| LSP | `workspace/executeCommand`（`command` = 正規操作 ID）または対応する標準メソッド | `workspace/executeCommand` |
| MCP サーバ | ツール名 = `novnow.` + 正規操作 ID | `novnow.idea.capture` |
| CLI | `novnow ` + 正規操作 ID（`.` → 空白。動詞内の `_` はそのまま）+ ` <dir> [オペランド] [--オプション]` | `novnow idea capture my-novel --text -` |

- 各操作の引数と戻り値はインターフェース間で同じ意味・同じデータ構造（§5 の JSON 形）でなければならない（MUST）。表面の型（Rust の型、Python のオブジェクト、JSON）への写像は各節の規定に従う。
- 正規操作 ID の完全な一覧は §19.7 のカタログであり、`api.list` 操作（§19.8）で機械可読に取得できる（MUST）。

#### 19.1.4 単一の真実と決定論

- 操作の実装は 1 箇所に置き、5 つのインターフェースはそれを薄い適合層から呼び出す（MUST）。インターフェース固有の再実装を置かない。
- 時刻は実行環境から注入され、操作の結果は壁時計に依存しない（P4。監査ログの記録時刻など、記録が目的の時刻を除く）。
- 出力の JSON はキー順・数値表現・改行を含めて決定論的でなければならない（MUST）。同じ状態への同じ操作は同じバイト列を返す（P4）。
- 操作はプロジェクトの状態だけを入力とし、インターフェースのセッション固有の隠れ状態に依存しない（MUST）。ただし明示的なセッション状態（開いている文書、作業スペース、折り畳み状態）を扱う操作（`session.*`、`save.*`、`nav.*` など）を除く。

#### 19.1.5 権限モデル

- 人間が直接使う経路（CLI・LSP・Rust API・Python API）は**関門を通らない**（§15.1.1 の「CLI・GUI は人間経路で関門なし」と同じ原則）。ファイルシステムの制約（§6.2）と保存状態機（§9.6）はすべての経路で等しく適用される（MUST）。
- MCP サーバのツールは §15.1.1 の関門モデルに従う（MUST）:
  - 書き込むツールは `role` 引数を受け取り、`apply` / `autonomous` のときだけ書き込む。それ以外は `E036`（提案のみ可）を返し、ファイルを一切変更しない。
  - 未実行・未承認の項目を作るツール（`materials.propose`、`section.propose`、`agent.task_create`、`mail.send`—いずれも実行・採用は別の関門を経る）はどの委譲レベルでも関門なしで使える。
  - 人間の専権事項（委譲レベルの変更 `agent.delegation_set`、MCP 設定の書き込み `agent.mcp_config_set`、プロンプトの初期化 `prompt.reset`、設定の変更 `settings.set`）は MCP ツールとして提供しない（MUST）。AI が自らの権限・接続・行動規則を書き換えられるようにしないためである。
- LSP の `workspace/executeCommand` 経由の操作は人間経路として扱う（エディタ利用者が主体）。

#### 19.1.6 状態性と原子性

- 状態を持たないインターフェース（MCP サーバのツール呼び出し、CLI の 1 実行）は、呼び出しごとにプロジェクトを読み直して操作し、結果を返す（MUST）。既存の §15.8 の「呼び出しごとに読み直す」方式を維持する。
- 状態を持つインターフェース（Rust / Python のセッション、LSP の開いた文書、CLI の対話モード）は、メモリ上の編集中状態を保持し、明示的な保存操作（`save.now`）で書き込む（§9.6 と同じ状態機）。
- 複数ファイルにまたがる操作（`document.rename`、`agent.proposal_apply`、`materials.approve` など）は原子性を持つ（MUST）: 途中の失敗時はどのファイルも変更されていない状態に戻すか、変更を一切行わない。部分的な適用を残さない。
- セッションを持つインターフェースは、外部によるファイル変更を検知したら §9.6 の衝突規則（reload / keep mine）と同じ選択肢を操作（`save.reload_external`、`save.keep_mine`）として提供する（MUST）。

#### 19.1.7 エラーモデル

すべてのインターフェースは共通のエラー分類を共有する（MUST）。分類・コード・メッセージは §6.3 のエラーコード表と §12 の i18n 規則に従う。

| 分類 | 意味 | LSP / MCP / JSON | CLI 終了コード（§14.2.5） |
|---|---|---|---|
| `usage` | 引数の形・型の誤り | JSON-RPC `-32602`（Invalid params） | 2 |
| `project` | プロジェクト構造の不正（S021・S022 など） | 同 `-32602` + `data.code` | 3 |
| `io` | ファイルシステムのエラー（S030・S034 など） | JSON-RPC `-32000` + `data.code` | 4 |
| `validation` | 内容の検証エラー（E001〜E040） | 結果内 `isError` / 診断 | 5 |
| `policy` | 権限・関門の拒否（E036 など） | 結果内 `isError` + `data.code` | 5 |
| `not_found` | 対象 ID の不存在（セクション・タスクなど） | JSON-RPC `-32001` + `data.code` | 3 |
| `conflict` | 保存状態機の衝突（外部変更・ロック） | JSON-RPC `-32002` + `data.code` | 4 |
| `timeout` | LLM 応答・処理の時間切れ（§15.8.1 の既定 60s を過ぎた） | JSON-RPC `-32003` | 4 |
| `cancelled` | 利用者が操作を中断した | JSON-RPC `-32800`（Request cancelled） | 0（能動的キャンセル。部分書き込みを残さない） |

- エラー応答は `code`（分類）・`kind`（S/E 番号）・`message`（§12.3 のカタログの言語で地域化）・`path`（該当ファイル、あれば）を含む（MUST）。
- MCP のツールはプロトコル従来どおり、内容エラーを `isError: true` のツール結果として返す（JSON-RPC エラーにしない）。§15.8.3 の既存方式を維持する（MUST）。

#### 19.1.8 国際化

- すべてのインターフェースは `--lang`（CLI）・`locale`（LSP 初期化オプション・MCP 引数）・`set_locale`（Rust / Python セッション）でメッセージ言語を切り替える（§12.4 と同じ決定順のうち明示指定・`ja` / `en`・既定 `ja`）。
- 診断メッセージ・エラーメッセージ・ヘルプ本文は §12.3 の文言カタログから供給される（MUST）。データ（ファイル内容・ID・数値）は地域化しない。

#### 19.1.9 性能とキャンセル

- 各操作の性能目標は §18 の該当項目に従う（例: 診断の単節解析 100ms 以内・全文 1s 以内）。インターフェースの適合層自体の追加遅延は操作あたり 10ms 以内を目安とする（SHOULD）。
- 長時間の操作（`agent.task_run` など、外部の応答を待つもの）はキャンセル手段を提供する（MUST）: セッション API では `session.cancel(handle)`、CLI では割り込みシグナル、MCP / LSP ではプロトコルのキャンセル通知（`$/cancelRequest`）。
- キャンセルされた操作は部分書き込みを残さない（§19.1.6 の原子性）。

### 19.2 Rust API

#### 19.2.1 セッションの開設と基本形

- ライブラリの入口は `Session` 型である。`Session` は 1 つのプロジェクト（またはメモリ上の仮想プロジェクト）に対する全操作の集合を提供する（MUST）。

```rust
// プロジェクトを開く（GUI と同じ検証と読み込み。§14.2 と同じ規則）
let mut session = Session::open("my-novel")?;
// 新規作成（novnow new と同じ構造を作る）
let mut session = Session::create("new-novel", CreateOptions::default())?;
// メモリ上の仮想プロジェクト（テスト用。ファイルシステムに触れない）
let mut session = Session::with_host(mock_host);

let info = session.project().info()?;          // project.info
let tree = session.structure().tree(None)?;        // structure.tree
let sections = session.section().list(None)?;
let scene = sections["sections"][0]["id"].as_str().expect("section ID");
let body = session.section().get(scene)?;   // section.get
session.section().write(scene, "新しい本文\n")?;  // section.write（メモリ上）
session.save().now()?;                         // save.now（ここで書き込み）
let text = session.export().text(ExportQuery::default())?;  // export.text
```

- `Session::open` / `Session::create` は §14.2 の起動前チェックと同じ検証を行い、同じエラー（S021・S022）を返す（MUST）。
- `Session` はファイルシステム・時刻・乱数などの環境依存を**ホスト**として注入できる（MUST）。既定のホストは実ファイルシステム（§6.2 の制約を適用）、テスト用ホストはメモリ。これにより GUI 版とまったく同じコード経路をテストできる。

#### 19.2.2 ドメインハンドルと操作

- 操作はドメイン別のハンドルから呼び出す（MUST）。ハンドルの一覧は §19.7 のカタログのドメイン列と一致する: `project`、`structure`、`section`、`nav`、`document`、`plot`、`save`、`search`、`materials`、`idea`、`cast`、`dictionary`、`lang`、`diagnostics`、`review`、`agent`、`prompt`、`context`、`mail`、`export`、`settings`、`session`、`help`、`api`。
- 引数と戻り値はライブラリの型（構造体・列挙）であり、JSON と同じ構造を表現する（MUST）。文字列は UTF-8。
- すべての操作は `Result<T, Error>` を返す。`Error` は §19.1.7 の分類・コード・メッセージ・パスを持つ。

```rust
session.search().project("クジラ")?;           // search.project
session.idea().capture("鯨の夢", IdeaKind::Image)?;   // idea.capture
session.agent().task_run("T012", RunOptions::default())?;  // agent.task_run
session.session().command("plot-outline")?;    // session.command（パレットコマンド名）
```

#### 19.2.3 パレットコマンドとの対応

- `session.command(name)` は GUI のコマンドパレット（`>`）が実行する全コマンドを名前で実行する（MUST）。コマンド名はパレットに表示される英数字 ID（例 `new-section`、`toggle-theme`、`font-up`）とし、一覧は `api.list` に「コマンドとしてのみ到達可能」の印付きで載る。
- パレットコマンドのうちデータ操作に相当するものは、対応する型付き操作（`section.insert` など）も持つ。`session.command` は GUI 操作の完全な代替経路として常に維持する（MUST）。

#### 19.2.4 イベントとコールバック

- セッションは GUI が描画で表示する状態変化（診断の更新、保存状態の変化、エージェントタスクの進捗、素材取得の完了）を**イベントキュー**として提供する（MUST）。利用者は `session.poll_events()` で取り出すか、コールバックを登録する。
- イベントは同じデータ構造を LSP のカスタム通知（§19.4.4）・Python API のイベントキューと共有する（MUST）。
- 非同期実行（LLM 呼び出し、素材 URL の取得）はセッション内部のワーカが行い、`poll_events` で進捗と完了を受け取る。ライブラリ利用者にスレッド管理を強制しない（MUST）。

#### 19.2.5 スレッド安全とライフサイクル

- `Session` は排他的に所有する（`&mut self`）。複数スレッドからの同時利用は呼び出し側が同期する。内部ワーカへの操作投入はセッション経由で行い、データ競合を起こさない（MUST）。
- `Session` の破棄（drop）は未保存の変更を破棄する。保存状態機の規則（§9.6「外部から閉じられるとき未保存変更は失われる」）と同じ。明示的保存は `save.now`。

### 19.3 Python API

#### 19.3.1 配布と対応環境

- パッケージ名は `novnow`（本体と同名・同版）。パッケージレジストリ（PyPI）から通常の手段（`pip install novnow`）で導入できる（MUST）。名前が確保できない場合の代替名は `novnow-python` とし、`import` 名は常に `novnow`。
- 本体は**ネイティブ拡張モジュール**（インプロセス）であり、Rust API（§19.2）と同じ機能核を呼び出す（MUST）。サブプロセス・JSON ソケット経由のラッパーではない。
- 対応 Python: CPython 3.9 以降。安定 ABI（abi3）でビルドし、1 つの配布物が 3.9 以上の全バージョンで動く（MUST）。
- 対応プラットフォーム: Linux x86_64 / aarch64、macOS x86_64 / arm64、Windows x86_64 の 5 構成（MUST）。ソース配布ではなくビルド済みホイールを各構成へ提供する。
- 型スタブ（`.pyi`）と `py.typed` を同梱し、型チェッカで全 API が検査できる（MUST）。

#### 19.3.2 基本形

```python
import novnow

with novnow.Session("my-novel") as session:      # open。with 終了で閉じる
    info = session.project.info()                 # dict（JSON と同じ構造）
    tree = session.structure.tree()
    scene = session.section.list()["sections"][0]["id"]
    text = session.section.get(scene)
    session.section.write(scene, "新しい本文\n")
    session.save.now()
    report = session.diagnostics.run(path="contents/01-01.cnov")
    for d in report["diagnostics"]:
        print(d["code"], d["line"], d["message"])

session = novnow.create("new-novel")              # project.new
# 既存のタスク T012 と MCP 接続が用意されたプロジェクトでのみ実行する:
# report = session.agent.task_run("T012")           # LLM 利用（§15 と同じ規則）
```

- `Session` はコンテキストマネージャであり、`with` 終了時に閉じる（未保存変更の破棄は §19.2.5 と同じ）。明示的な `session.close()` も持つ。
- データの受け渡しは Python の組み込み型（`dict`・`list`・`str`・`int`・`bool`・`None`）で行い、JSON と同じ構造を表現する（MUST）。専用オブジェクトの属性アクセスも併用できる（例 `info["title"]` と `info.title` の両方）。
- バイト列を返す操作（Zip 導入・書き出し）は `bytes` を使う。

#### 19.3.3 例外

- すべてのエラーは `novnow.NovnowError` の派生例外で通知する（MUST）。例外は §19.1.7 の分類ごとの型（`UsageError`、`ProjectError`、`IOError`、`ValidationError`、`PolicyError`、`NotFoundError`、`ConflictError`、`TimeoutError`、`CancelledError`）を持ち、`code`（S/E 番号）・`message`（地域化済み）・`path` 属性を持つ。

#### 19.3.4 GIL と並行性

- ファイル I/O・LLM 呼び出し・URL 取得などブロックする処理の間は GIL を解放する（MUST）。これにより Python 側スレッドが待機中に動く。
- 1 つの `Session` オブジェクトの操作は直列化する（§19.2.5 と同じ所有規則）。複数スレッドからの同時呼び出しはデータ競合ではなく直列実行として扱う（MUST）。

#### 19.3.5 イベントと長時間操作

- `session.poll_events()` と `session.on_event(callback)` は §19.2.4 と同じイベントを Python オブジェクト（`dict`）で返す（MUST）。
- `agent.task_run` などの長時間操作は `session.cancel(handle)` で中断できる（§19.1.9）。

### 19.4 LSP（Language Server Protocol）

#### 19.4.1 提供形態

- サーバはサブコマンド `novnow lsp [<dir>]` で起動し、標準入力／標準出力で LSP（バージョン 3.17）を話す（MUST）。`<dir>` 省略時はカレントディレクトリをプロジェクトとみなす。
- 通信はヘッダによるフレーミング（`Content-Length`）付きの JSON-RPC 2.0。追加のプロトコル拡張を要求しない（MUST）。
- サーバは 1 プロジェクト = 1 ワークスペースフォルダとして動作する。複数プロジェクトは複数のサーバプロセスで扱う。

#### 19.4.2 標準機能の対応

LSP 標準メソッドと novnow の機能の対応は次のとおり（MUST）:

| LSP メソッド | novnow の機能 |
|---|---|
| `initialize` / `initialized` | プロジェクトの検証・読み込み（§14.2 と同じ規則）。`serverInfo` に名称・版 |
| `textDocument/didOpen` / `didChange` / `didSave` / `didClose` | 本文・プロットの編集中状態（§9.6 の状態機）。変更は単節解析で即時検証。`didClose` は開いている文書の一覧から外すだけで、保存もしない |
| `textDocument/publishDiagnostics` | §3.4 の診断（E001〜E040）。`code` に E 番号、`source` に `"novnow"`、メッセージは §12.4 の言語 |
| `textDocument/completion` | 補完候補（§9.7.3。キャラ・用語・資料・モード・SceneId） |
| `textDocument/hover` | カーソル位置の注解・モード説明（§9.7.1 のホバー情報） |
| `textDocument/codeAction` | クイックフィックス（§7 の各コードの「クイックフィックス」列）。**適用後のテキスト編集を返せるものだけ**を返し、ファイルをまたぐ移動や UI 操作（型の追加・新規資料の作成など）は返さない |
| `textDocument/definition` / `references` | 定義ジャンプ・参照検索（`lang.definition` / `lang.references`） |
| `textDocument/documentSymbol` | セクション一覧（`section.list`）。プロット文書ではプロット節 |
| `workspace/symbol` | 全体検索（`search.project`） |
| `textDocument/formatting` | 整形（`lang.format`。§9.7.3 の規則） |
| `textDocument/prepareRename` | リネームの対象かの確認（操作 ID のときだけ範囲を返す） |
| `textDocument/rename` | 操作 ID のリネーム（`lang.rename_apply`。プレビューは `lang.rename_preview` + `workspace/applyEdit`） |
| `textDocument/foldingRange` | 折り畳み範囲（`lang.folding`、`plot.outline_fold`） |
| `textDocument/inlayHint` | インレイヒント（`lang.hints`） |
| `textDocument/semanticTokens/full` | 意味トークン（§3.4 の `semantic_tokens::full`。注解・参照・モード・資料の色分け） |
| `workspace/executeCommand` | §19.7 のカタログの全操作（`command` = 正規操作 ID、`arguments` = JSON） |
| `workspace/didChangeConfiguration` | 設定の変更通知（`settings.get` の再読込） |
| `shutdown` / `exit` / `$/cancelRequest` | 終了要求・終了・実行中の要求の取り消し。`initialize` より前の要求には `ServerNotInitialized`（-32002）を返す |

- 位置は LSP 標準の UTF-16 行・文字で受け渡し、内部のバイト位置と相互変換する（§3.4「位置表現」）。
- 補完のトリガー文字は `'`（操作 ID）・`@`（キャラクター）・`:`（モード）の 3 つ（§9.7.3 と同じ）。`$`（タグ）と `^`（素材）は補完を出さない（`Ctrl+Space` の明示起動でも同じ）。`$` と `^` は §5.0.3 のとおりエスケープなしで受け付ける。
- 診断の配信範囲は、開いている文書と、打鍵のたびの単節解析の結果（§7.2「実行コストの制御」の範囲）。全体スコープの診断は `workspace/executeCommand`（`diagnostics.run`）で要求できる。

#### 19.4.3 文書同期と保存

- 文書同期は full 同期（`textDocumentSync = 1`）とし、`didChange` で全文を受け取る。内部では差分編集（§3.4）で rope を更新し、10 万行プロジェクトでも応答性目標（§18.1.1）を守る。
- `didSave` は `save.now` と同じ扱い（明示的保存）。自動保存（§4.4 の 400ms）はエディタ側の設定に委ね、サーバは `save.state` で状態を報告する。
- 外部変更の検知（§9.6）はカスタム通知 `novnow/saveStateChanged` でクライアントへ知らせる（MUST）。

#### 19.4.4 カスタム通知

サーバは次の通知をサーバ→クライアントに送る（MUST）。内容は §19.2.4 のイベントと同じデータ構造:

| 通知 | 内容 |
|---|---|
| `novnow/diagnosticsChanged` | 診断の更新（スコープ・件数） |
| `novnow/saveStateChanged` | 保存状態機の状態遷移（`saving` `saved` `conflict` など） |
| `novnow/agentProgress` | エージェントタスクの進捗（タスク ID・段階・メッセージ） |
| `novnow/materialsChanged` | 素材インボックスの変化（新規提案・採用・却下） |

#### 19.4.5 エディタ拡張なしの動作

- カスタム通知を理解しない標準エディタでも、補完・ホバー・診断・コードアクション・リネーム・整形・折り畳みの標準機能がすべて動作する（MUST）。カタログの残り全操作は `workspace/executeCommand` で到達できる（MUST）。

### 19.5 MCP サーバ拡張

#### 19.5.1 既存ツールとの関係

- 既存の 14 ツール（§15.8.3 の表）は名前・引数・動作を**変えない**（MUST）。後方互換の凍結対象とする。
- 既存ツールは正規操作 ID の別名としてカタログに載る（例 `novnow.section.write` = 操作 `section.write`）。新しいツールは `novnow.` + 正規操作 ID の名前で追加する（MUST）。
- ツールの一覧・引数スキーマは `tools/list` で発見でき、`api.list` の出力（§19.8）と一致する（MUST）。
- 呼び出しごとにプロジェクトを読み直す方式（§15.8「1 呼び出し = 読み直す」）を維持する（MUST）。
- §19.7 で「M—」と印を付けた操作はツールとして公開しない（状態性・安全性・バイナリの理由はその行の注記のとおり）。

#### 19.5.2 拡張の規則

- 新しい書き込み系ツールは §19.1.5 の規則に従う: `role` 引数必須、`apply` / `autonomous` のときだけ書き込み、それ以外は `E036`（`isError: true`）。未実行・未承認項目の作成系（§19.1.5 の 4 つ）は無関門。
- `plot.write` の `level` は §15.8.3 の「`episode` のみ」の制限を本節が拡張し、`outline` / `chapter` も受け取る（MUST。既存呼び出しの動作は変わらない）。検証は `section.write` と同じ（E006 経路照合・E007・§10 のレベル別規則）。
- エージェント系ツール（`agent.*`）は §15.2 の 13 状態機・§15.6 の 4 原則をそのまま適用する。セッション制限（§15.3）は MCP 呼び出しにも等しく課す（MUST）。
- 素材系ツール（`materials.*`）の URL 取得は v1 では行わない（§16.1.2 の注記）。実装する版では、§16.1.2 の 1MB 上限・15 秒のタイムアウトに従う（MUST）。
- 監査ログ（§15.8.4）はすべての新しい書き込みツールでも記録する（MUST）。

#### 19.5.3 スキーマ

- 各ツールの `inputSchema` は JSON Schema（draft 2020-12）で、§19.7 のカタログの引数と一致する（MUST）。スキーマは `api.list` の出力から生成し、手書きの複製を置かない。

### 19.6 CLI 拡張

#### 19.6.1 サブコマンド体系

- 既存のサブコマンド（`new`・`check`・`export`・`import`・`mcp`・`open`・`--gui`）は名前・引数・出力・終了コードを**変えない**（MUST）。後方互換の凍結対象とする。
- §14.1 のサブコマンド解決規則（第 1 位置引数の完全一致）は、本節で追加するサブコマンド（`lsp`・`api`・および §19.7 の全ドメイン名）へ同じ規則で拡張される（MUST）。既存コマンド語が優先し、ディレクトリ名との衝突は §14.1 の `./` 規則で回避する。
- 新しい体系は `novnow <ドメイン> <動詞> <dir> [オペランド] [--オプション]`（§19.1.3）。LSP サーバの起動は `novnow lsp [<dir>]`。

```text
novnow <ドメイン> <動詞> <dir> [オペランド] [--オプション]
novnow lsp [<dir>]                       LSP サーバ（§19.4）
novnow api list [--json]                 正規操作カタログの出力（§19.8）
novnow new|check|export|import|mcp …     既存サブコマンド（§14.1。不変）
```

- 既存サブコマンドと正規操作の対応（両方の形を受け付ける。動作は同一）:

| 正規操作 | 既存サブコマンド（優先形） |
|---|---|
| `project.new` | `novnow new <dir>` |
| `project.validate` | `novnow check <dir>` |
| `export.text` | `novnow export <dir> [--out] [--lang]`（§17.2 のオプション不変） |
| `project.import_zip` | `novnow import <zip>`（§14.1.1 の規則不変） |

#### 19.6.2 出力

- 全操作は `--json` で機械可読出力（§19.1.4 の決定論）を返す（MUST）。省略時は人間可読な地域化テキスト（§12.3）。
- `--json` の共通形（エンベロープ）は次のとおり（MUST）:

```json
{ "ok": true, "op": "section.get", "data": { … } }
{ "ok": false, "op": "section.get", "error": { "code": "not_found", "kind": "S025", "message": "…", "path": "docs/03.md" } }
```

- 進捗（素材取得・LLM 呼び出し）は stderr へ、結果は stdout へ出す（MUST）。パイプ利用（`|` `>`）を妨げない。
- `-` をオペランドに指定した箇所は標準入力から読む（§14.1.1 の `import -` と同じ規則）。

#### 19.6.3 終了コード

- §14.2.5 の表（0・2・3・4・5）をすべての新しいサブコマンドに適用する（MUST）。§19.1.7 の分類→終了コードの写像に従う。新しい終了コードを追加しない。
- 割り込みシグナルによる終了は部分書き込みを残さず（§19.1.6）、終了コードは OS の慣例（128 + シグナル番号）に従う。§14.2.5 の表は novnow が自ら選ぶ終了コードだけを定義する。

#### 19.6.4 環境非依存

- 新しいサブコマンドは GUI 部品（ウィンドウ・描画・ダイアログ）を必要としない（MUST）。`open`・`--gui` と GUI 依存の既存動作（Zip 導入のダイアログ）は GUI 版の構成要素があるときだけ有効（§14.1.1 と同じ規則）。

### 19.7 機能パリティカタログ

GUI の全機能を正規操作 ID に対応づけるカタログ。この表がインターフェース間のパリティの単一の真実である（MUST）。列の意味:

- **LSP 標準メソッド**: `workspace/executeCommand` に加えて標準メソッドでも到達できる場合にそのメソッド名。空欄は `executeCommand` のみ。
- **例外**: そのインターフェースで提供しない場合の印。`R—` = Rust API、`P—` = Python API、`L—` = LSP、`M—` = MCP、`C—` = CLI。`M 門番` = MCP では §19.1.5 の関門付き。印のない操作は 5 インターフェースすべてが提供する（MUST）。

| 正規操作 | 説明・GUI 経路 | LSP 標準メソッド | 例外 |
|---|---|---|---|
| project.info | プロジェクト基本情報（タイトル・作者・言語・版）。ワークスペース見出し | | |
| project.new | 新規プロジェクト作成（パレット `>` + `new`、CLI 既存） | | L—（起動前）M—（要プロジェクト） |
| project.validate | 構造検証（CLI `check` 既存。§4.3 の判定） | | |
| project.reload | 再読込（パレット `>` + `reload`） | | M—・C—（状態性） |
| project.stats | 執筆統計（総文字数・台詞比率・話者別の文字数と行数・節数。§19.1.4 の決定論により、壁時計に依存する「本日」「速度」「連続日数」は返さない。それらは画面が `.novnow/stats.json`（§4.4.2）と時計から出す） | | |
| project.recent_list | 最近開いたプロジェクト一覧（`.novnow/recent.json`） | | M—（状態性） |
| project.recent_remove | 最近開いた一覧からの削除（`>` + `forget`） | | M—（状態性） |
| project.import_zip | Zip 導入（§13.5、選択画面・CLI） | | M—（バイナリ） |
| project.export_zip | Zip 書き出し（`>zip`、§13.5） | | M—（バイナリ） |
| structure.tree | 構成ツリー（`Ctrl+Shift+T`） | | |
| structure.resolve | 位置・ID の解決（ジャンプバー `Ctrl+J`） | | |
| structure.breadcrumb | パンくず（現在位置の経路表示） | | M—・C—（状態性） |
| section.list | セクション一覧（指定文書内） | textDocument/documentSymbol | |
| section.get | セクション取得（エディタ表示） | | |
| section.write | セクション保存（`Ctrl+S`、§9.6 状態機経由） | | M 門番 |
| section.insert | 節の新設（`Ctrl+;`、§9.4.1） | | M 門番 |
| section.delete | 節の削除（`>` + `delete-section`、§8.1.2。本文と対応するプロットを一緒に削除） | | M 門番 |
| section.confirm_next | 次の未確認節の確認（節の末尾での `Ctrl+Enter`、§9.4.1） | | M—・C—（状態性） |
| section.split | 節の分割（節エディタの `Ctrl+Enter`、§8.1.1・§9.4.1） | | M 門番 |
| section.propose | 節の提案（本文を書き換えず `agents/proposals/` に置く。§8.7.1。MCP 既存 `novnow.section.propose`）。提案は本文を変えないため関門を置かない（§19.1.5） | | |
| nav.file | 文書・ファイルの直接指定 | | M—・C—（状態性） |
| nav.section | セクションへの移動（`Ctrl+G`） | | M—・C—（状態性） |
| nav.jump | 行への移動（`Ctrl+L`） | | M—・C—（状態性） |
| nav.prev / nav.next | 前の位置・次の位置（`Ctrl+[` `]`） | | M—・C—（状態性） |
| nav.back | 親ディレクトリへ（`Ctrl+H`） | | M—・C—（状態性） |
| nav.forward | 子の先頭へ（`Ctrl+Shift+H`） | | M—・C—（状態性） |
| nav.up | 1 つ上の階層へ（`Ctrl+B`） | | M—・C—（状態性。`plot.get` の level 指定で代替） |
| nav.switcher | 切替パネルの一覧（§11.1 の 4 ワークスペース） | | M—・C—（状態性） |
| document.get | ドキュメント（`.md`）全文の取得 | | |
| document.write | ドキュメント全文の書き込み | | M 門番 |
| document.rename | 文書のリネーム（`>` + `rename-document`） | | M 門番 |
| plot.get | 全体／章プロットの取得（§10.3.1） | | |
| plot.write | 全体／章／話プロットの書き込み（§10.3.2） | | M 門番 |
| plot.set_state | プロットの確認状態（✓）の切替（§10.3.3） | | M 門番 |
| plot.insert_section | 話プロットへの節挿入（`Ctrl+Shift+N`、§10.2） | | M 門番 |
| plot.outline_fold | 構成ツリーの章→話の展開・折り畳み（`Ctrl+Shift+[` `]`） | foldingRange（範囲算出） | 折り畳み状態は `.novnow/ui.json` の `folded.outline` に保存し、再起動後に復元する（§4.4.1・§9.3.3） |
| save.now | 保存（`Ctrl+S`） | | M—・C—（状態性） |
| save.state | 保存状態の取得（§9.6 の 6 状態） | | M—・C—（状態性） |
| save.keep_mine | 衝突時に自分の版を維持（§9.6） | | M—・C—（状態性） |
| save.reload_external | 外部変更の読み込み（§9.6） | | M—・C—（状態性） |
| save.diff_external | 自分の版と外部変更の差分（§9.6 `diff-against-external`） | | M—・C—（状態性） |
| search.file | 検索／置換（`Ctrl+F`、編集中文書） | | 置換は M 門番 |
| search.replace_file | 全置換（§9.7.5、編集中文書） | | M 門番 |
| search.project | 全体検索（`Ctrl+Shift+F`） | workspace/symbol | |
| search.find_next / search.find_prev | 次の候補・前の候補（`F3` `Shift+F3`） | | M—・C—（状態性） |
| materials.list | 素材インボックス一覧（§16.1） | | |
| materials.get | 素材カードの取得（`Ctrl+M`） | | |
| materials.approve | 素材の採用（S#### 化、`inbox/` → `docs/` の移動と `approved` 化。URL からの取得は §16.1.2 のとおり未実装） | | M 門番 |
| materials.reject | 素材の却下（`>` + `reject-material`） | | M 門番 |
| materials.update | 素材カードの編集（メタデータ） | | M 門番 |
| materials.attach | 素材の添付（`>` + `attach-material`） | | M 門番 |
| materials.search | 素材の検索（MCP 既存 `novnow.materials.search`） | | |
| materials.propose | 素材の提案（MCP 既存 `novnow.materials.propose`） | | |
| idea.list | 着想一覧（§10.4） | | |
| idea.capture | 着想の記録（`Ctrl+Shift+I`、種別・キーワード） | | M 門番 |
| idea.promote | 着想の昇格（`>` + `promote-idea`） | | M 門番 |
| idea.drop | 着想の削除（`>` + `drop-idea`） | | M 門番 |
| idea.write | 着想メモの書き込み（§10.4） | | M 門番 |
| cast.get | キャラクター情報・有効なモード（MCP 既存） | | |
| cast.list | キャラクター一覧 | | |
| cast.timeline | 呼称タイムライン（§9.5.2） | | |
| cast.scan | 本文スキャンで候補抽出（`>` + `scan-names`） | | M 門番 |
| dictionary.list | 辞書の一覧（`>` + `show-dictionary`） | | |
| dictionary.register_term | 用語の辞書登録（E032・E030 のクイックフィックス） | | M 門番 |
| dictionary.register_character | キャラクターの辞書登録（`>` + `register-character`） | | M 門番 |
| dictionary.term_full | 用語の完全情報（MCP 既存 `novnow.lexicon.search`） | | |
| dictionary.modes_list | 全モード一覧 | | |
| lang.tokens | 字句解析（§3.4 のトークン列） | | |
| lang.completion | 補完候補（§9.7 の `'` `@` `:` `$` `^`） | textDocument/completion | |
| lang.hover | カーソル位置の情報（§9.7.1） | textDocument/hover | |
| lang.hints | インラインヒント（`>` + `toggle-inlay-hints`） | textDocument/inlayHint | |
| lang.code_action | クイックフィックス候補（§7 の各コードの「クイックフィックス」列） | textDocument/codeAction | 適用は M 門番 |
| lang.definition | 定義へジャンプ（`>` + `goto-definition`） | textDocument/definition | |
| lang.references | 参照検索（`>` + `find-references`） | textDocument/references | |
| lang.symbols | シンボル一覧（`>` + `show-symbols`） | textDocument/documentSymbol | |
| lang.format | 整形（`>` + `format-buffer`、§9.7.3） | textDocument/formatting | |
| lang.rename_preview | リネームの影響範囲（`>` + `rename-symbol`） | textDocument/rename（準備） | |
| lang.rename_apply | リネームの実行 | textDocument/rename | M 門番 |
| lang.folding | 折り畳み範囲（§9.8） | textDocument/foldingRange | |
| diagnostics.run | 診断（`scope` はパスの接頭辞、`path` は文書 1 つ、`scene` は `@AA-BB-CC-DD`。省略すると全部。§7.2 の範囲で計算） | publishDiagnostics（配信） | |
| diagnostics.place | 地名解決（`>` + `resolve-place`、§11.3） | | |
| review.list | 見直し項目一覧（`Ctrl+Shift+R`） | | |
| review.goto | 見直し項目への移動（`>` + `goto-review`） | | M—・C—（状態性） |
| agent.task_list | タスク一覧（`Ctrl+Shift+A`） | | |
| agent.task_create | タスクの起票 | | |
| agent.task_get | タスク 1 件の詳細 | | |
| agent.task_run | タスクの実行（LLM 呼び出し、§15.2） | | M 門番（role 必須・セッション制限適用） |
| agent.task_dry_run | ドライラン（提案のプレビュー） | | |
| agent.task_stop | タスクの中止（`>` + `stop-all` の個別版） | | M 門番 |
| agent.stop_all | 全実行の中止 | | M 門番 |
| agent.proposal_list | 提案一覧（§15.8.1） | | |
| agent.proposal_get | 提案 1 件の内容（before/after 差分） | | |
| agent.proposal_apply | 提案の適用（§15.8.1） | | M 門番 |
| agent.proposal_reject | 提案の却下（`>` + `reject-proposal`） | | M 門番 |
| agent.delegation_get | 委譲レベルの参照 | | M—（安全: 人間専用） |
| agent.delegation_set | 委譲レベルの変更 | | M—（安全: 人間専用） |
| agent.mcp_config_get | MCP 接続設定の参照（`>` + `mcp`） | | |
| agent.mcp_config_validate | MCP 接続設定の検証 | | |
| agent.mcp_config_set | MCP 接続設定の書き込み（§15.5 の検証付き） | | M—（安全: 人間専用） |
| agent.thread_list | メールスレッド一覧（§15.6） | | |
| agent.thread_get | スレッド 1 件の内容 | | |
| agent.audit_list | 監査ログ一覧（§15.8.4） | | |
| context.build | コンテキストパケットの構築（MCP 既存） | | |
| prompt.list | プロンプトファイル一覧 | | |
| prompt.get | プロンプト内容の取得 | | |
| prompt.diff | 初期版との差分（`>` + `prompt diff`） | | |
| prompt.reset | プロンプトの初期化（`>` + `prompt reset`） | | M—（安全: 人間専用） |
| mail.send | エージェント間のメール（MCP 既存。未承認メッセージの積載） | | |
| export.text | テキストのエクスポート（`>export`、§17.2。CLI 既存） | | M 門番（書き込み時。テキストの返却のみは無関門） |
| reference.pane | 参照ペインの全セクション（§9.5.2 の 1〜9）。各行は移動先と動作（§9.5.5）を含む | | |
| settings.get | 設定の取得（§4.4.1 の各キー） | workspace/didChangeConfiguration（通知） | |
| settings.set | 設定の変更（テーマ・フォント倍率・言語・行幅・ペイン構成） | | M—（安全: 人間専用） |
| session.workspace_get | 現在のワークスペース（§11.1 の 4 種） | | M—・C—（状態性） |
| session.workspace_switch | ワークスペースの切替（`Ctrl+1`〜`4`、`Ctrl+Shift+W`） | | M—・C—（状態性） |
| session.recent_document | 最近開いた文書の循環（`Ctrl+Tab`） | | M—・C—（状態性） |
| session.command | パレットコマンドの実行（`>` の全コマンド、§19.2.3） | | M—・C—（状態性） |
| session.poll_events | イベントキューの取得（§19.2.4） | novnow/* 通知（配信） | M—・C—（状態性） |
| help.topic | ヘルプ本文（`>` + `help`・`welcome`・`migration`・ショートカット一覧） | | |
| api.list | 正規操作カタログの機械可読出力（§19.8） | | |

カタログの規則:

- GUI 固有の経路（ショートカット・パレット・ボタン）は「説明・GUI 経路」列に括弧で示す。パレットの全コマンドは `session.command` からも到達できる（§19.2.3）。
- プロジェクトの開閉はカタログの操作ではなく入口（`Session::open` / `novnow lsp` / `novnow <dir>` / 各コマンドの `<dir>` 引数）である。
- `M 門番` の操作は MCP では `role` 引数を受け取り、§19.1.5 の関門を適用する。ほかのインターフェースは関門なし（人間経路）。
- `M—・C—（状態性）` の操作はセッション状態（開いている文書・位置・ワークスペース）を扱うため、状態を持たない MCP・CLI の 1 実行では提供しない。Rust / Python / LSP はセッションを持つので提供する。
- 読者モード（§10.7・§17.3）の操作（`reader.*`）、統計ダッシュボード、呼称マトリクス、五感、正規表現検索（§9.7.4）も全機能パリティの対象である。これらの GUI 機能が利用できることと、5 インターフェースからの提供が完了していることを同一視しない。各機能の外部インターフェース提供までに、その操作をこの表へ対応づける（§19.1.2）。

**`section.split`（節の分割）**: 対象の節と分割位置（節のテキスト先頭からのバイト位置）を受け取り、§8.1.1 の規則で節を 2 つに分ける。GUI の `Ctrl+Enter` による分割（§9.4.1）はこの操作を使う。LSP 標準メソッドは無く、MCP では `section.delete` と同じく関門付きとする。

### 19.8 版・互換性・配布

#### 19.8.1 バージョニング

- 5 つのインターフェースは本体と同じ版番号（§4.3 のセマンティックバージョニング）を報告する（MUST）: `Session::version()`（Rust）、`novnow.__version__`（Python）、`initialize` の `serverInfo.version`（LSP）、`tools/list` のサーバ情報（MCP）、`--version`（CLI）。
- カタログ（§19.7）自体に版を付け、`api.list` の出力に含める（MUST）。操作の追加はマイナー版、既存操作の意味を変える変更はメジャー版を要求する。

#### 19.8.2 発見可能性

- `api.list` は全正規操作を次の情報とともに返す（MUST）: ID・説明（地域化）・引数スキーマ（JSON Schema）・戻り値の形・例外（どのインターフェースが提供しないか）・関門の有無・対応するパレットコマンド名（あれば）。
- `--json` の出力、MCP の `inputSchema`、LSP の `executeCommand` 一覧、Python の型スタブはすべて `api.list` と同じ単一のカタログから生成する（MUST）。手書きの複製を置かない（§19.1.4）。

#### 19.8.3 互換性の約束

- 既存の 14 MCP ツール（§15.8.2）と既存の CLI サブコマンド（§14.1）は凍結対象であり、名前・引数・出力・終了コードを変えない（MUST）。
- 正規操作 ID は追加のみ。廃止は 1 マイナー版以上の予告期間（発見結果での廃止予定の表示）を置く。
- LSP / MCP のプロトコル版はそれぞれ LSP 3.17 / MCP 2025-03-26 以上とし、サーバは `initialize` で交渉する。

#### 19.8.4 配布

- Rust ライブラリは本体クレートと同じ名前で公開する（MUST）。
- Python ホイールは §19.3.1 の 5 プラットフォーム構成でパッケージレジストリへ公開する（MUST）。ビルドは公開パイプライン（CI）で行い、版番号は本体と一致させる。
- LSP / MCP / CLI は本体バイナリに内蔵され、追加の配布物を必要としない（MUST）。
- ウェブサイト（§13）に API リファレンス（カタログ・引数・例）を掲載する。
