# novnow — 次世代的な世界観管理システム群

（**[English version is HERE](./README.en.md)**）


`novnow` は、長編小説・ライトノベル・Web小説・ノベルゲーム・シナリオなど、あなた自身の世界を育み、広めるためのツール群です。

novnow形式を管理するためのnovnowエディタは、デスクトップアプリ（Windows / macOS / Linux）とブラウザの両方で動きます。画面もキーボード操作も共通で、同じプロジェクトをそのまま開けます。

## どこで動くか

| 動かし方 | 準備 | 向いている人 |
|---|---|---|
| **ブラウザ**（インストール不要） | `https://novnow.sonneko.com/app/` を開くだけ | まず試したい・環境を変えたくない |
| デスクトップ（Windows / macOS / Linux） | ビルドする（配布物は準備中） | 手元のフォルダで原稿を管理したい |
| コマンド（CLI） | 同上 | 原稿をまとめて検査・書き出ししたい |
| エディタ連携（LSP） | 同上 | VS Code など手持ちのエディタで書きたい |
| AI 連携（MCP） | 同上 | AI に作品を読ませたい |
| Python | 同上 | 作業を自動化したい |

画面の構成・キー操作・保存される内容は、ブラウザとデスクトップで同じです。
どの入口から触っても同じ結果になります（同じ 1 本の実装を共有しています）。

使い方は [ユーザーガイド](./docs/users/README.md) から。まずは [はじめての手引き](./docs/users/start.md) をどうぞ。

![novnow エディタの画面（write ワークスペース）](screenshot.png)

---

## novnow形式とは？

「novnow形式」は、あなたの頭の中にある世界観や物語自体を管理するための形式です。

PDFが文書を管理するファイル形式であるのと全く同じように、novnow形式は世界観を管理するファイル形式です。novnowファイルを送信するだけであなたの世界観を他人に伝えることができます。

複数人で一つの世界観を育みたいときにも最適です。

また、拡張機能としてAIに伝えるための機能もサポートしています。

## 具体的にどんなもの？

- キャラクタのモード・役割語の設定
- 用語辞書
- アイデアの蓄積
- 本文
- 物語全体のプロット・章単位のプロット・話単位のプロット
- 資料
- キャラアイコン

以上の情報を統合して管理できます。

例えば、本文なら以下のように書きます。

```
@01-04-01 放課後の図書室
静まり返った'魔法学院の図書室。高い天井まで届く本棚の隙間から、午後の柔らかい光が差し込んでいる。
古びた羊皮紙の匂いが漂う一角で、'アルスは厚みのある魔導書を開いていた。

@01-04-02 図書室の攻防
「やっぱり'アルス、ここにいたのね」@リリア

呆れたような声とともに、'リリアが隣の椅子を引き寄せた。
整えられた銀髪を揺らしながら、彼女は'アルスの手元にある本を覗き込む。

「明日から期末試験だって分かってる？ '学院長が『今回の赤点者は補習漬けだ』って言ってたでしょ」@リリア
;
「分かってるよ。でも、これを見てくれ」@アルス

'アルスは指先で羊皮紙に描かれた複雑な魔法陣を指し示した。

「'旧校舎の地下遺跡に眠っているっていう『'星屑の時計』の設計図だ。もしこれが本物なら、失われた古代の錬金術が証明できる」@アルス
;
「またそんな眉唾な噂話を……」@リリア

'リリアは溜め息をついたが、図面の精巧な書き込みに目を奪われると、それ以上言葉が続かなくなった。
彼女の青い瞳に、小さな好奇心の火が灯るのを'アルスは見逃さなかった。
;
「……で、本当にそこへ行くつもりなの？」@リリア
「今日の放課後、'旧校舎の鍵が開いている時間を見計らって行く。'リリアも来るだろ？」@アルス
「……付き合ってあげるわよ。あなたが迷子になったら困るからね」@リリア

そう言ってそっぽを向く'リリアの唇端が、わずかに持ち上がっていた。

```

読み手にとっての解釈の余地がないように「novnow形式」は設計されています。

novnow形式は、小説をテキストデータでエクスポートする機能を備えています。novnow形式特有の記号はボタン一つで全て取り除かれます。

将来的に、小説以外のさまざまな媒体（ノベルゲーム、3DCGアニメーションなど）へとエクスポートする機能を研究開発の末、実装する予定です。

## エディタの仕様の要約

1. **1 節（セクション）を最小単位とする集中執筆（`write` ワークスペース）**
   - 画面下段の「節エディタ」では現在執筆中の 1 節だけに集中でき、上段の「文脈ビュー」に前後最大 3 節が読み取り専用で表示されます。
   - 右側の「参照ペイン」には、現在の節に関わるプロット・登場人物（有効モード・役割語）・用語・資料・診断が自動抽出されて並びます。ペインの表示・左右入替・比率はショートカットで変えられます。
2. **軽量な専用テキスト記法と無損失保存**
   - 本文（`contents/AA-BB.txt`）、全体・章プロット（`plots/all.txt`, `plots/AA.txt`）、話プロット（`plots/AA-BB.txt`）、登場人物（`characters.md`）、用語（`words.md`）、アイデア（`ideas.md`）、資料（`materials/`）をすべてプレーンテキスト（UTF-8）で管理し、Git でそのまま差分管理できます。
   - 原文は 1 バイトも捨てません（読み込んだ本文をそのまま書き戻せます）。プロジェクトは `project.json` を中心に自己完結し、`.novnow/` のキャッシュだけが Git の管理外になります。
3. **意味解決エンジンと 76 種のリアルタイム診断**
   - `'名前` による辞書参照（最短一致・NFKC 正規化）、状況別キャラクターモード解決、役割語パターン照合（台詞の口調一致を緑下線で可視化）、初出より前の用語使用検出、文長・連続文末・表記揺れなどの執筆支援診断を提供します。
   - 診断は 76 種すべてに一意のコード（`E001`〜`W0xx`／`I0xx`）があり、`Ctrl+.` のクイックフィックスと `review` ワークスペースの一覧から辿れます。
4. **キーボード完結の 9 ワークスペースと統一パレット**
   - `Alt+1`〜`Alt+9` で本文（`write`）・話プロット（`plot-episode`）・構成（`plot-outline`）・アイデア（`idea`）・資料（`material`）・配役（`cast`）・読者（`reader`）・エージェント（`agent`）・点検（`review`）を即座に切り替えられます。切替時も現在の節は維持されます。
   - `Ctrl+P` の統一パレットから先頭 1 文字（`@` 構造、`'` 辞書、`#` 資料、`>` コマンド）でプロジェクト内のあらゆる要素へジャンプできます。`Ctrl+G` の番号ジャンプ、`F5` の再読込、`?` のショートカット一覧もあります。
   - 日本語・英語の UI（`Ctrl+Alt+L`）と、システム追従のダーク／ライトテーマ（`Ctrl+Alt+T`）を備えます。
5. **人間の統制下で動く AI エージェント連携（MCP 対応）**
   - 7 つの役割（`writer`, `plotter`, `ideator`, `researcher`, `stylist`, `logician`, `integrator`）ごとに 4 段階の委譲レベル（`off` / `propose` / `apply` / `autonomous`）を設定でき、変更提案の差分レビューを経て初めて本文に反映されます。
   - `novnow mcp <dir>` で MCP サーバ（14 ツール）として外部の AI クライアントから使えます。プロンプトと ContextPacket は決定論的に組み立てられ、文字数予算を持ちます。
6. **アノテーション除去エクスポート**
   - 執筆用の構造見出し・コメント・`'` 参照・資料タグを除去し、投稿サイトや縦書きビューア向けのプレーンテキスト（`.txt`）または Markdown（`.md`）を出力します。
   - プロジェクト全体の Zip 書き出し・読み込みにも対応します。

---

## インストールと起動

現時点ではソースからのビルドのみです（バイナリ配布の準備中）。Rust の stable ツールチェーンが必要です。

```bash
git clone https://github.com/sonneko/novnow.git
cd novnow

# CLI のみ（GUI のクレートを落とすので軽い）
cargo build --release

# デスクトップ GUI を含む
cargo build --release --features ui
```

```bash
# 新規プロジェクトを作る / 診断する / 書き出す
./target/release/novnow new my-novel
./target/release/novnow check my-novel
./target/release/novnow export my-novel --format txt --out manuscript.txt

# GUI を開く（ディレクトリ指定、または --gui で選択画面から）
./target/release/novnow my-novel
./target/release/novnow --gui
```

付属のサンプルは `./target/release/novnow check examples/sample-novel` で試せます。Web 版は `trunk serve` でローカルに立てられます（手順は [`GUIDE.md`](./GUIDE.md)）。

---

## 外部インターフェース

GUI でできることは、すべてヘッドレスからも操作できます（詳細は [`interfaces.md`](./docs/specifications/interfaces.md)）。

| 入口 | 起動 | 何ができるか |
|---|---|---|
| CLI | `novnow new` / `check` / `export` / `import` / `migrate` / `api` / `mcp` / `lsp` | プロジェクト作成・診断・書き出し・移行・API 一覧。`novnow <domain> <verb> <dir>` の統一書式 |
| LSP | `novnow lsp <dir>` | エディタから補完・ホバー・定義移動・診断・整形・折りたたみ（LSP 3.17） |
| MCP | `novnow mcp <dir>` | AI クライアント向けの 14 ツール |
| Python | `import novnow` | 同じ 119 の正規操作を Python から（PyO3・abi3-py39 のホイール） |
| Rust | `novnow` クレート | `core` / `app` の API を直接利用 |

```bash
novnow api list             # 119 の正規操作の一覧（--json で機械可読）
novnow check my-novel       # 診断を出して終了する（エラーがあれば終了コード 5）
novnow structure tree my-novel --json                    # 正規操作も --json で機械可読
```

## 仕様について

エディタおよびnovnow形式の全ての仕様は、明文化されています。

- [`overview.md`](./docs/specifications/overview.md): 設計原則・用語・アーキテクチャ
- [`project.md`](./docs/specifications/project.md): プロジェクト構造・`project.json`・サンプル構成
- [`syntax.md`](./docs/specifications/syntax.md): 各ファイルの構文仕様
- [`semantics.md`](./docs/specifications/semantics.md): 辞書・最短一致・モード解決・役割語パターン言語
- [`diagnostics.md`](./docs/specifications/diagnostics.md): 診断カタログ
- [`editing.md`](./docs/specifications/editing.md): 節単位編集・自動保存・外部変更競合・変更提案
- [`ui.md`](./docs/specifications/ui.md): 画面仕様・ワークスペース・テーマ・国際化
- [`platform.md`](./docs/specifications/platform.md): ストレージ境界・Web IndexedDB・CLI 起動仕様
- [`agent.md`](./docs/specifications/agent.md): AI エージェント・委譲レベル・MCP・プロンプト仕様
- [`workflows.md`](./docs/specifications/workflows.md): 資料収集・エクスポート・性能品質要件
- [`interfaces.md`](./docs/specifications/interfaces.md): CLI・LSP・MCP・Python・Rust の外部インターフェース

公式サイト（[https://novnow.sonneko.com/](https://novnow.sonneko.com/)）では、機能ガイド・診断コードの一覧・API リファレンスを読めます。仕様書はすべてこのリポジトリの [`docs/specifications/`](./docs/specifications/) にあり、実装と同じコミットで管理されています。

開発に参加する場合は [`GUIDE.md`](./GUIDE.md)（ビルドと執筆の手順）と [`docs/developers.md`](./docs/developers.md)（開発環境・文書規則・検証手順）から始めてください。


## ライセンスについて

novnowはどなたでも無料で利用可能です。有料になることは決してありません。

「novnow形式」の仕様および「novnowエディタ」のソースコードは、両方とも`MIT License`で自由に利用できます。
