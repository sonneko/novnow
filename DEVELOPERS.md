# novnow — 決定論的アルゴリズムによる小説執筆IDE

## 起動

```bash
# 既存プロジェクトを開く（必須ファイルが無ければエラー終了）
cargo run --release -- ./my-novel

# 新規プロジェクトを雛形から作成
cargo run --release -- --init ./my-novel
```

ビルド:

```bash
cargo build --release
# 生成物: target/release/novnow
```

Web版ビルド:

```bash
rustup target add wasm32-unknown-unknown

cargo install trunk
# または cargo binstall trunk

trunk build --release

# ./dist/に成果物が生成されます。
# ./dist/以下をルートパスとしてブラウザで開かないと不具合が発生する可能性があります。
```

## 画面構成（3 ペイン）

```
┌───────────────────────────────┬──────────────────────┐
│ 上左: ファイルリーダー          │                      │
│  ・ツリー（contents / plots）   │                      │
│  ・下部にプレビュー付きビューア  │ 右: 情報パネル        │
│     （カーソル行をハイライト）   │  ・現在の節 SceneId   │
├───────────────────────────────┤  ・対応プロット        │
│ 下左: エディタ                 │  ・登場人物・口調      │
│  ・行番号 + 単一 TextEdit       │  ・用語解説            │
│  ・下部に「節」ピンポイント編集  │  ・関連資料            │
│   バー（Excel の数式バー相当）   │  ・警告               │
└───────────────────────────────┴──────────────────────┘
```

## キーボードショートカット

| ショートカット | 動作 |
|---|---|
| Ctrl+O | プロジェクトフォルダを開く |
| Ctrl+S | 現在のファイルを保存 |
| Ctrl+Shift+S | 全バッファ保存 |
| Ctrl+↑ / Ctrl+↓ | ファイルツリーとエディタ間のフォーカス移動 |
| Alt+↑ / Alt+↓ | 節単位スクロール（プレビューと下部バーの対象行を節見出しへ移動） |
| Ctrl+Tab | 最近開いたファイルの切り替え |
| Ctrl+G | 指定 SceneId へジャンプ（例 `01-04-02-03`） |
| Ctrl+F | 現在ファイル内検索 |
| F3 | 次を検索（検索モーダルを閉じている場合も可） |
| Shift+Enter | 検索モーダル内で前を検索 |
| Ctrl+H | 置換 |
| Ctrl+R | プロジェクト JSON を手動再読込 |
| Ctrl+1 / Ctrl+2 / Ctrl+3 | ペイン1（上左）/ ペイン2（下左）/ ペイン3（右）へフォーカス |
| Ctrl+B | 現在行の節に対応するプロットファイルを開く |
| Ctrl+M | 現在の節に対応する資料を検索・表示 |
| F5 | プロジェクト JSON を再読込 |
| Ctrl+T | 選択テキストを用語辞書へ新規登録 |
| Ctrl+Shift+T | 選択テキストをキャラ辞書へ新規登録 |
| Enter | 下部バーの編集を確定（Rope の該当行へ反映） |
| Esc | モーダルを閉じる |

## モジュール構成

```
src/
  main.rs              eframe::run_native のエントリポイント / CLI 解析
  app.rs               App 状態と egui 更新ループ、ショートカット、コンテキスト推論
  panes/
    file_tree.rs       上左: ファイルリーダー + プレビュー
    editor.rs          下左: 単一 TextEdit + 行番号 + 節ピンポイント編集バー
    info_panel.rs      右: 情報パネル
  text/
    buffer.rs          Ropey ラッパ・カーソル位置管理
    tokenizer.rs       カスタム文法のトークナイザ
    incremental.rs     行単位のインクリメンタル更新
  project/
    loader.rs          JSON/テキストの読み書き・雛形生成
    resolver.rs        パス解決・参照追跡
    state.rs           プロジェクト全体のインメモリキャッシュ
    watcher.rs         ファイル監視（自前ポーリング）
  types/
    characters.rs      characters.json
    words.rs           words.json
    plots.rs           plots の SceneId / PlotNode
    materials.rs       materials/index.json
```

## カスタム文法

| ファイル種別 | トークン |
|---|---|
| `contents/*.txt` | SceneHeader `@A-B-C[-D]` / Speaker 行末 `@キャラ名` / Narration / ParagraphBreak `;` / Blank |
| `plots/*.txt` | SceneHeader / Comment `//` / PlotPoint / ParagraphBreak / Blank |

* 複数行トークンは存在しない前提のため、行単位で完全に分割でき、編集行のみ再トークン化できます。
* `SceneId` は 4 階層（`A-B-C-D`）。見出しで D を省略（`@01-04-01`）した場合は `section = 0`。

## 情報パネルの優先順位

カーソル行に直接含まれる要素 > 同一場面の要素 > 同一話の要素、の順で推論します。
用語検出は Aho-Corasick による複数パターン一括マッチです。


## 日本語フォント要件

egui の既定フォントは CJK（漢字・ひらがな・カタカナ）グリフを含まないため、
**システムに日本語対応フォントがインストールされている必要があります**。

起動時に `fontdb` で以下の優先順位で自動検出します:

1. Noto Sans CJK JP / Noto Sans JP
2. Meiryo / Yu Gothic / MS PGothic (Windows)
3. Hiragino Sans (macOS)
4. TakaoPGothic / IPAGothic / VL PGothic (Linux)

インストールされていない場合、日本語部分が □ や ? で表示されます。

Web版の場合は、強制的にWeb版のwasmファイルに付属しているフォントファイルが読み込まれます。

## ライセンス

MIT
