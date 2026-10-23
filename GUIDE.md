# novnow — 小説執筆のための専用統合開発環境（IDE）

`novnow` は、長編小説・ライトノベル・Web 小説の執筆に特化した専用 IDE です。デスクトップアプリ（Linux / macOS / Windows）およびブラウザだけで動作する Web アプリ（WebAssembly + IndexedDB）の両方で動作します。

---

## 主な特徴

機能の一覧と説明は [`README.md`](./README.md#エディタの仕様の要約) に集約しています（同じ説明を 2 か所に置かないため）。本手引きは「ビルドして、書いて、書き出す」までの手順に絞ります。

要点だけ:

- **1 節を最小単位とする集中執筆**（`write` ワークスペース。上段は前後 3 節の文脈ビュー、右は参照ペイン）。
- **プレーンテキスト + Git で管理できる専用記法**（本文・プロット・登場人物・用語・アイデア・資料）。
- **意味解決エンジンと 76 種のリアルタイム診断**（`'名前` の辞書参照・モード解決・役割語の照合）。
- **キーボード完結の 9 ワークスペース**（`Alt+1`〜`Alt+9`。`write` `plot-episode` `plot-outline` `idea` `material` `cast` `reader` `agent` `review`）と統一パレット `Ctrl+P`。
- **人間の統制下で動く AI エージェント連携**（4 段階の委譲レベル・MCP）。
- **アノテーション除去エクスポート**（`.txt` / `.md`）。

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
  全 11 冊（概要・プロジェクト構造・構文・意味解決・診断・編集・UI・プラットフォーム・エージェント・ワークフロー・外部インターフェース）。各冊の役割は [`README.md`](./README.md#仕様について) の一覧を参照してください（同じ表を 2 か所に置かないため）。
- **設計書（実装設計・対応ソースコード）**: [`docs/designs/`](./docs/designs/)
- **開発者ガイド**: [`docs/developers.md`](./docs/developers.md)
- **開発計画（段階 ② ロードマップ）**: [`docs/plans.md`](./docs/plans.md)
- **変更履歴・意思決定アーカイブ**: [`docs/history.md`](./docs/history.md)

## オフライン利用（Web）

ブラウザ版は初回オンライン起動後、PWA としてホーム画面へ追加できます。Service Worker が読み込んだアプリ資産をキャッシュし、以後はオフラインでも起動できます。プロジェクトはブラウザの IndexedDB に保存されるため、定期的に Zip エクスポートでバックアップしてください。初回読み込みや更新には通信が必要です。
