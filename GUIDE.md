# novnow — 小説執筆のための専用統合開発環境（IDE）

`novnow` は、長編小説・ライトノベル・Web 小説の執筆に特化した専用 IDE です。デスクトップアプリ（Linux / macOS / Windows）およびブラウザだけで動作する Web アプリ（WebAssembly + IndexedDB）の両方で動作します。

---

## 主な特徴

1. **1 節（セクション）を最小単位とする集中執筆（`write` ワークスペース）**
   - 画面下段の「節エディタ」では現在執筆中の 1 節だけに集中でき、上段の「文脈ビュー」に前後最大 3 節が読み取り専用で表示されます。
   - 右側の「参照ペイン」には、現在の節に関わるプロット・登場人物（有効モード・役割語）・用語・資料・診断が自動抽出されて並びます。
2. **軽量な専用テキスト記法と無損失保存**
   - 本文（`contents/AA-BB.txt`）、全体・章プロット（`plots/all.txt`, `plots/AA.txt`）、話プロット（`plots/AA-BB.txt`）、登場人物（`characters.md`）、用語（`words.md`）、アイデア（`ideas.md`）、資料（`materials/`）をすべてプレーンテキスト（UTF-8）で管理し、Git でそのまま差分管理できます。
3. **意味解決エンジンと 76 種のリアルタイム診断**
   - `'名前` による辞書参照（最短一致・NFKC 正規化）、状況別キャラクターモード解決、役割語パターン照合（台詞の口調一致を緑下線で可視化）、初出より前の用語使用検出、文長・連続文末・表記揺れなどの執筆支援診断を提供します。
4. **キーボード完結の 8 ワークスペースと統一パレット**
   - `Alt+1`〜`Alt+9` で本文（`write`）・話プロット（`plot-episode`）・全体プロット（`plot-outline`）・アイデア（`idea`）・資料（`material`）・配役（`cast`）・エージェント（`agent`）・点検（`review`）を即座に切り替えられます。
   - `Ctrl+P` の統一パレットから先頭 1 文字（`@` 構造、`'` 辞書、`#` 資料、`>` コマンド）でプロジェクト内のあらゆる要素へジャンプできます。
5. **人間の統制下で動く AI エージェント連携（MCP 対応）**
   - 役割ごと（`writer`, `plotter`, `ideator`, `researcher`, `stylist`, `logician`, `integrator`）に 4 段階の委譲レベル（`off` / `propose` / `apply` / `autonomous`）を設定でき、変更提案の差分レビューや `novnow mcp` による MCP サーバ連携に対応しています。
6. **アノテーション除去エクスポート**
   - 執筆用の構造見出し・コメント・`'` 参照・資料タグを除去し、投稿サイトや縦書きビューア向けのプレーンテキスト（`.txt`）または Markdown（`.md`）を出力します。

---

## クイックスタート

### 1. ビルド

```bash
# CLI のみ（GUI なし）
cargo build --release

# デスクトップ GUI を含むビルド
cargo build --release --features ui
```

### 2. 新規プロジェクトの作成と検証（CLI）

```bash
# 新しい小説プロジェクトを作成
novnow new my-novel

# プロジェクト構造と原稿全体の診断を実行
novnow check my-novel

# 本文からアノテーションを除去してエクスポート
novnow export my-novel --format txt --out manuscript.txt
```

### 3. GUI の起動

```bash
# プロジェクトディレクトリを指定して起動
novnow my-novel

# プロジェクト選択画面から起動
novnow --gui
```

付属のサンプルプロジェクト（`examples/sample-novel`）は `novnow check examples/sample-novel` または `novnow examples/sample-novel` で試すことができます。

---

## ドキュメント構成

- **仕様書（ユーザー向けリファレンス）**: [`docs/specifications/`](./docs/specifications/)
  - [`overview.md`](./docs/specifications/overview.md): 設計原則・用語・アーキテクチャ
  - [`project.md`](./docs/specifications/project.md): プロジェクト構造・`project.json`・サンプル構成
  - [`syntax.md`](./docs/specifications/syntax.md): 各ファイルの構文仕様（EBNF）
  - [`semantics.md`](./docs/specifications/semantics.md): 辞書・最短一致・モード解決・役割語パターン言語
  - [`diagnostics.md`](./docs/specifications/diagnostics.md): 診断カタログ（全 76 コード）
  - [`editing.md`](./docs/specifications/editing.md): 節単位編集・自動保存・外部変更競合・変更提案
  - [`ui.md`](./docs/specifications/ui.md): 画面仕様・ワークスペース・テーマ・国際化
  - [`platform.md`](./docs/specifications/platform.md): ストレージ境界（規則 10）・Web IndexedDB・CLI 起動仕様
  - [`agent.md`](./docs/specifications/agent.md): AI エージェント・委譲レベル・MCP・プロンプト仕様
  - [`workflows.md`](./docs/specifications/workflows.md): 資料収集・エクスポート・性能品質要件
- **設計書（実装設計・対応ソースコード）**: [`docs/designs/`](./docs/designs/)
- **開発者ガイド**: [`docs/developers.md`](./docs/developers.md)
- **開発計画（段階 ② ロードマップ）**: [`docs/plans.md`](./docs/plans.md)
- **変更履歴・意思決定アーカイブ**: [`docs/history.md`](./docs/history.md)

## オフライン利用（Web）

ブラウザ版は初回オンライン起動後、PWA としてホーム画面へ追加できます。Service Worker が読み込んだアプリ資産をキャッシュし、以後はオフラインでも起動できます。プロジェクトはブラウザの IndexedDB に保存されるため、定期的に Zip エクスポートでバックアップしてください。初回読み込みや更新には通信が必要です。
