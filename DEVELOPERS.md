# novnow — 開発者向け資料

決定論的アルゴリズムによる小説執筆 IDE。

> **この文書の位置づけ**
>
> - 仕様の正本は **[`SPECIFICATIONS.md`](./SPECIFICATIONS.md)**（v1 仕様 第 2.2 版）。
> - 実装順序と見積りは **[`PLANS.md`](./PLANS.md)**（段階 ① の実装計画）。
> - 本書は「手を動かすときに開く頁」であり、仕様と食い違った場合は **`SPECIFICATIONS.md` が優先する**。
>
> **現在の `src/` は v1 の層構造の「空の骨格」（M0 完了時点）である。**
> v0（プロトタイプ）の UI コードは M0 で削除し、`v0-final` タグに保存した。
> `src/core/**` `src/ports/**` `src/adapters/**` `src/ui/**` は中身が空で、
> 実装は M1 以降（`PLANS.md` §5）で順に埋まる。本書の「モジュール構成」は
> **v1 の目標形**であり、M0 時点のツリーはそれと一致している（中身だけが空）。
>
> **GUI は M7 まで動かない**（`PLANS.md` §3 I3）。M0 時点の `cargo run` は
> 使用法を表示して終了コード 2 で終了するだけの暫定版である。

---

## 1. ビルドと起動

### 1.1 必要なもの

| 用途 | 要件 |
|---|---|
| ネイティブ | Rust stable（edition 2021）、`cargo` |
| Web | `rustup target add wasm32-unknown-unknown` ＋ `cargo install trunk` |
| 日本語表示 | システムに日本語フォント（§7） |

### 1.2 ネイティブ

```bash
# 既存プロジェクトを開く（必須ファイルが欠けていれば GUI を出さずにエラー終了）
cargo run --release -- ./my-novel

# 新規プロジェクトを雛形から作成
cargo run --release -- new ./my-novel

# 検査のみ（CI 用。診断を出して終了する）
cargo run --release -- check ./my-novel

cargo build --release    # 生成物: target/release/novnow
```

> **v0 からの変更点**: 新規作成は `--init <dir>` ではなく **サブコマンド `new`** になった（`SPECIFICATIONS.md` §14.1）。
> 「ディレクトリはフラグなし位置引数で指定する」という指示 2 に合わせたため。

> **M0 時点の実態（注意）**: 上のコマンドは **v1 の目標形**であり、まだ実装されていない。
> 現在の `src/main.rs` は引数に関わらず使用法を stderr に出して終了コード 2 で終了するだけ
> （`SPECIFICATIONS.md` §14.3 の判定順と §14.1 のサブコマンドは M6 で実装する）。

### 1.3 Web

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk          # または cargo binstall trunk

trunk serve                  # 開発（127.0.0.1:8080）
trunk build --release        # 成果物は ./dist/
```

`./dist/` はディレクトリをルートパスとして配信すること（相対パス解決のため）。

### 1.4 テスト

```bash
cargo test                        # すべて
cargo test --test layering        # 層構造の検査（本書 3 節 / 仕様 §3.2・§12.2）だけ
```

CI で常設するテストは `PLANS.md` §6 を参照。`tests/layering.rs` は **落ちたらマージしない**（例外なし）。

`tests/layering.rs` の中身は次の 2 系統（6 テスト）。

| テスト | 検査内容 |
|---|---|
| `core_has_no_forbidden_dependency` | `src/core/**` の AST（`syn`）を訪走し、§3.2 の禁止依存を検出する。`use` 文・`extern crate`・式／型／パターン／マクロ呼び出しのパス・属性・マクロ本体のトークン列が対象。許可リストに無いクレートの `use` も落とす |
| `core_has_no_user_facing_text` | 同様に日本語の文字列リテラル（§12.2）を検出する。コメント・doc コメント・char リテラルは対象外。**例外は `i18n: format-vocabulary` の doc 行を付けた `const` / `static` の中だけ**（§5.0.6 のフィールドキー別名表のような「ファイル形式の語彙」は UI 言語が `en` でも受理するのでユーザー向け文言ではない。`PLANS.md` §9.2 B-19。§12.2 に「字句データは文言に含まない」と明記されて A-5 は解消済み）。免除が関数に及ばないことは自己テストのフィクスチャで固定してある |
| `checker_catches_*` / `checker_allows_*`（4 本） | **検査器そのもの**の自己テスト。「違反を書いたら本当に落ちるか」「正当な core コードを誤検出しないか」を固定例で担保する |

> 検査器を直すときは、必ず自己テストの固定例（`MUST_CATCH_*` / `MUST_PASS_*`）も一緒に更新すること。
> 自己テストが無い検査器は、改修で検出漏れが混入しても CI が緑のままになる。

`tests/diagnostics_catalog.rs` は **仕様書そのものを読む**テスト（`include_str!("../SPECIFICATIONS.md")`）。
§7 / §7.1 / §7.2 の表を Markdown から読み直し、`src/core/diagnostics.rs` の **77 コード**と
件数・コード文字列・重大度・引数の数・文書位置の有無・既定の有効/無効・実行コスト表・variant の
doc コメントまで突き合わせる。診断コードを足す／直すときは **先に `SPECIFICATIONS.md` の表**を直すこと
（`PLANS.md` §6.3「仕様書 → ゴールデン → 実装」）。表に無いコードを enum に足すと
`the_catalog_has_no_code_the_spec_does_not_list` が、文言の引数を変えると
`every_spec_row_is_in_the_catalog` が落ちる。

---

## 2. CLI 文法（`SPECIFICATIONS.md` §14.1）

```
novnow <dir>                    既存プロジェクトを開く（フラグなし位置引数）
novnow new [dir]                dir（既定 "."）に新規プロジェクトを作る
novnow check <dir>              妥当性検査のみ行い、診断を出して終了（CI 用）
novnow export <dir> [options]   アノテーション除去エクスポート（§17）
novnow migrate <dir>            v0 → v1 移行（§20）
novnow mcp <dir>                MCP サーバとして起動（§15.8.3）
novnow --gui                    引数があっても GUI 起動を強制
novnow --help | -h
novnow --version | -V
```

- 第 1 位置引数が `new` / `check` / `export` / `migrate` / `mcp` と**完全一致**したときのみサブコマンド。
  それ以外はプロジェクトディレクトリとして解釈する（`new` という名前のディレクトリは `novnow ./new`）。
- フラグを取るのは `export` だけ。
- **`lsp` サブコマンドは v1 には無い**（裁定 U-3 は保留）。ただし `core/features/*` は LSP のメソッドと
  1:1 で対応する純粋関数として書くこと（§3.4）。将来 `novnow lsp` を足すときに core を触らずに済ませるため。
- 必須ファイルが欠けている場合は **stderr にエラー＋解説 URL＋`novnow new` の案内だけを出して終了**し、
  GUI は一切出さない（§14.2、指示 2 の明文）。

---

## 3. 層構造（`SPECIFICATIONS.md` §3.2）

v1 の最重要の制約。**`tests/layering.rs` が CI で機械的に検査する。**

```
main ──→ ui ──→ ports ←── adapters
          │       │
          └──→ core ←┘          core は誰にも依存しない
```

| 層 | 依存してよいもの | 禁止 |
|---|---|---|
| `core/` | `serde` / `serde_json`、`regex`、`aho-corasick`、`ropey`、`anyhow` / `thiserror`、`unicode-normalization`、std の純粋データ構造 | **`std::fs`、`std::net`、`std::time::SystemTime`、`egui`、`tokio`、フォント、ロケール文言** |
| `ports/` | `core` の型、トレイト定義のみ | 実装 |
| `adapters/` | `ports` の実装。ここだけが I/O を持つ | `ui` への依存 |
| `ui/` | `core`（読む）・`ports`（呼ぶ）・`egui` | `adapters` への直接依存 |

- **`core` は文言を持たない。** 診断は `E001` のようなコードと引数だけを返し、日本語/英語への変換は `ui/i18n/` が行う（§12）。
- **`core` は時刻を読まない。** 必要なら `ports::clock` を経由する。
- 許可クレートを増やすときは、**先に `SPECIFICATIONS.md` §3.2 の表を更新してから** `Cargo.toml` を触る（MUST）。

---

## 4. モジュール構成（v1 の目標ツリー・`SPECIFICATIONS.md` §3.3）

```
src/
  main.rs                  起動のみ（CLI 解析 → GUI or CLI）
  core/
    id.rs                  SceneId / PartialSceneId / RangeExpr
    text/                  rope.rs  position.rs  lexer.rs  incremental.rs
    syntax/                contents.rs  plot_outline.rs  plot_episode.rs
                           characters_md.rs  words_md.rs  ideas_md.rs
                           materials_index.rs  prompt_md.rs
    model/                 Project, Document, Character, Mode, RolePattern,
                           Word, Material, Idea, Task
    resolve/               dictionary.rs  modes.rs  role_language.rs
                           materials.rs  context_rules.rs
    diagnostics.rs         コード定義と生成（文言は持たない）
    features/              LSP に 1:1 対応する純粋 API（§3.4）
                           semantic_tokens  completion  hover  inlay_hint
                           code_action  definition  references  document_symbol
                           formatting  rename  folding
    export.rs              アノテーション除去（§17.1）
    aicontext/             packet.rs  budget.rs
  ports/                   storage.rs  clock.rs  mcp.rs  env.rs  dialog.rs
  adapters/                storage_fs.rs  storage_idb.rs  mcp_stdio.rs
                           clock_system.rs  dialog_rfd.rs
  ui/
    shell.rs               共通シェル（ステータスバー・パレット・モーダル・IME 状態）
    theme.rs  i18n/
    widgets/               context_view  section_editor  reference_pane
                           syntax_text  inlay
    workspaces/            write  plot_episode  plot_outline  idea
                           material  cast  agent  review
```

`core/reader.rs` と `ui/workspaces/reader.rs`（縦書きプレビュー）は **v1 追加仕様 #27** であり、
v1 本体では作らない（§0.6.1）。

---

## 5. カスタム文法（`SPECIFICATIONS.md` §5）

v1 は **8 形式**を解析する。v0 の 2 形式から大きく増えているので、`§5` を読んでから実装すること。

| 形式 | ファイル | 主なトークン |
|---|---|---|
| A | `plots/00-*.txt`（全体・章プロット） | `@` 見出し、`//` コメント、プロット行 |
| B | `plots/*.txt`（話プロット） | `@` 見出し、`;`、`when:` / `space:` / `blurb:` などのフィールド |
| C | `contents/*.txt`（本文） | `@` 見出し、行末 `@キャラ名`、`'参照`、`;`、`^` 引用注釈、`! ` 挿絵メモ、`# ` コメント |
| D | `characters.md` | 見出し、`attributes` / `personality` / `emotion` / `couple` / `tone` / `role` |
| E | `words.md` | 用語と説明 |
| F | `ideas.md` | 着想行と `status` |
| G | `materials/index.json` | 資料 ID（`M` / `S` 接頭辞）とタイトル |
| H | `prompts/*.md` | エージェント用メタ指示 |

**行頭シジル（`@` `;` `'` `//` `#` `!` `^`）は `const SIGIL_TABLE` による表駆動で扱う**（MUST）。
追加仕様で増えるため、`match` を直書きしないこと（`PLANS.md` §1.5-4）。

`SceneId` は 4 階層（`章-話-場面-節`）。見出しで節を省略した場合は `section = 0`。
複数行トークンは存在しないので、行単位で完全に分割でき、編集行のみ再トークン化できる。

---

## 6. 日本語入力（IME）— 実装時にまず読むこと

`SPECIFICATIONS.md` **§9.8.5** に実装規約がある。要点だけ再掲する。

- **egui 0.27.2 のイベント名は `Event::Ime` ではない。**
  `Event::CompositionStart` / `CompositionUpdate(String)` / `CompositionEnd(String)` を購読する。
  （`Event::Ime(ImeEvent)` への統合は egui 0.28.0 から。v1 は 0.27.2 に固定する）
- native（`egui-winit`）でも Web（`eframe` の text agent）でも**同じイベント名**が来る。`#[cfg]` で分岐しない。
- `egui::TextEditState::has_ime` は `pub(crate)` で**読めない**。`ui/shell.rs` に自前の状態機械を持つ。
- **変換中は `Event::Key` 起因の処理をすべて抑止する**（ショートカットだけでなく `Enter` の節確定と `Esc` のモーダル閉じも）。
- **`PlatformOutput.ime` を出力しないと native で IME が有効にならない。**
  `egui-winit` は `ime` が `Some` のフレームでしか `set_ime_allowed(true)` を呼ばないため、
  `egui::TextEdit` を使わない自前エディタでは `ctx.output_mut(|o| o.ime = Some(IMEOutput { rect, cursor_rect }))` が必須。
- 張り付き防止: `WindowFocused(false)` と「2 秒間 IME イベントなし」で `composing` を落とす。
- **egui を上げるときは 0.29.x を飛ばす。** 0.29.0 で Linux の IME が壊れ（emilk/egui#5544）、
  修正（#5198）は 0.30.0 以降に入った。

---

## 7. 日本語フォント要件

egui の既定フォントは CJK グリフを含まないため、**システムに日本語対応フォントが必要**。
起動時に `fontdb` で次の優先順位で自動検出する。

1. Noto Sans CJK JP / Noto Sans JP
2. Meiryo / Yu Gothic / MS PGothic（Windows）
3. Hiragino Sans（macOS）
4. TakaoPGothic / IPAGothic / VL PGothic（Linux）

見つからない場合、日本語部分が □ や ? で表示される。
Web 版は wasm に同梱したフォントを強制的に読み込む。

---

## 8. 画面構成とショートカット

v1 の画面は **3 ペイン ＋ ワークスペース切替** になる（v0 の固定 3 ペインとは別物）。
正確な定義は `SPECIFICATIONS.md` §9（共通シェル）と §10（ワークスペース）を参照。

| 分類 | キー | 備考 |
|---|---|---|
| ペイン移動 | `Ctrl+1` / `Ctrl+2` / `Ctrl+3` | §9.8.1 |
| ワークスペース切替 | `Alt+1`〜`Alt+9` | **`Alt+7`（reader）は v1 では欠番**（追加仕様 #27） |
| 階層の上下 | `Ctrl+B` / `Ctrl+Shift+B` / `Ctrl+Enter` | 全体 → 章 → 話 → 本文の 4 段階（指示 11） |
| 着想の捕捉 | `Ctrl+Shift+I` | どこからでも 1 行書いて `Enter`。画面遷移しない（MUST） |
| 資料検索 | `Ctrl+M` | §16 |
| ジャンプ | `Ctrl+G` | SceneId 指定 |
| 検索 | `Ctrl+F` / `Ctrl+Shift+F` | 正規表現モードは追加仕様 |

**保存の扱いが v0 と根本的に違う。** v1 では `Ctrl+S` を主役に据えない。
節の位置が変わった瞬間に保存し、**ユーザーに節単位の保存/未保存を意識させない**（§8.2）。
全体の保存状態だけを右下のステータスバーにアイコンで出す。

---

## 9. 情報パネル（参照ペイン）

カーソル行に直接含まれる要素 > 同一場面 > 同一話、の順で推論する（`core/resolve/context_rules.rs`）。
用語検出は Aho-Corasick による複数パターン一括マッチ、参照は**最短一致**（§6.1）。

**カード UI にしないこと**（§9.5.1、MUST）。最小限の見出しスタイルを付けたただのテキストとして並べる。
v0 の `panes/info_panel.rs` はこの規約に反するので流用しない。

役割語が正しく付いている台詞には**緑の下線**を引く。付いていない場合は**何も表示しない**（エラーにしない、§6.4）。

---

## 10. AI エージェント（`SPECIFICATIONS.md` §15）

- **LLM へのアクセスはすべて MCP 経由**（MUST）。`adapters/mcp_stdio.rs` 以外から外部通信しない。
  `tests/no_llm_sdk.rs` が CI で検査する。
- **AI に何をどこまで任せるかは、ユーザーが委譲レベルで決める**（README の思想、§15.1.1）。
  既定は `propose`（提案のみ・人間が採否）だが、`apply`（自動適用）/ `autonomous`（連鎖ごと自律）に切り替えられる。
  自律させても上限（`agents.limits`）と総停止（§15.7.4）は必ずユーザーが握る。
- 役割は**最小単位に分割し、必要なコンテキストを設計する**。人間と LLM で情報源と優先順位を一致させる（§15.4）。
- 委譲モデルの個別判断（次タスク起票は integrator のみ、MCP 書き込みツールの `role` 引数必須、
  キー操作など）は `PLANS.md` §9.4（B-6〜B-15）にユーザー決定済みで記録している。
- メタ指示（プロンプト）は**ユーザーが自由に書き換えられる**。プロジェクトごとに持ち、プロジェクト間で共有しない。
  既定プロンプトはプロジェクト外の「一切変更されない場所」に置き、いつでも復元できるようにする（裁定 U-7）。
- **プロジェクトディレクトリ外に置いてよい情報は「プロジェクトディレクトリパスの履歴」だけ**（規則 10、MUST）。
  `tests/storage_policy.rs` が検査する。

---

## 11. 作業を始める前に

1. `SPECIFICATIONS.md` §0（読み方）と §3（アーキテクチャ）を読む。
2. `PLANS.md` §1（段階の切り分け）で、その作業が ①（v1 本体）か ②（追加仕様）かを確認する。
3. 該当マイルストーン（`PLANS.md` §5）の「① での削り」を読む。**追加仕様を先回りして実装しない。**
4. `PLANS.md` §1.5 の「① が ② のために守る 10 項目」を読む。未知キーの温存など、
   後から直すのが高くつく事項が並んでいる。
5. 設計判断は `PLANS.md` §9 で確認する。委譲モデル（§15.1.1）に関する判断 B-6〜B-15 は
   §9.4 でユーザー決定済み。**勝手に変えない。**
6. コミットは機能単位で小さく行う（`PLANS.md` §10.1 と整合。マイルストーン単位でまとめない）。

## ライセンス

MIT
