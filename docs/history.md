# 変更履歴・意思決定アーカイブ（History — 追記専用）

本書は過去の仕様改訂履歴、アーキテクチャ・仕様の意思決定ログ（「最初は X だったが Y に変更した」等の経緯）、および達成済みマイルストーンの記録を保持する**追記専用（append-only）**ファイルである。

---

## 0. `grep` による機械検索ガイド

本書を先頭から全文読まずに目的の経緯を特定するため、すべての記録エントリは次の固定ヘッダとタグ行を持つ。

- 見出し形式: `### [HIST:<CATEGORY>:<ID>] <YYYY-MM-DD> <要約タイトル>`
- タグ行形式: `[HIST-TAG: category=<CATEGORY> id=<ID> topics=<カンマ区切りキーワード>]`
- `<CATEGORY>` の種別:
  - `SPEC-REV`: 仕様書の版改訂履歴（`v1.0`〜`v2.19`）
  - `DECISION`: 設計・仕様の意思決定と経緯（`D-001`〜`D-016`）
  - `MILESTONE`: 完了した開発マイルストーンと PR・計測結果（`M0`〜`M11`, `DOC-REORG`）

よく使う検索コマンド例:
```bash
# 全エントリの目次と行番号を一覧する
grep -n '^### \[HIST:' docs/history.md

# 特定のカテゴリ（例: 意思決定 DECISION）のみを抽出する
grep -n '\[HIST-TAG: category=DECISION' docs/history.md

# キーワード（例: ime, storage, role_language, mcp, v0）で検索する
grep -n -i '\[HIST-TAG:.*topics=.*ime' docs/history.md
```

---

## 1. 仕様改訂履歴（`SPEC-REV`）

### [HIST:SPEC-REV:v1.0] 2026-04-19 初版策定
[HIST-TAG: category=SPEC-REV id=v1.0 topics=initial,dsl,architecture,workspaces]
- 初版仕様を策定。1 節 = 1 編集単位、A/B/C/MD の 4 形式、役割語パターン言語、76/81 診断、8 ワークスペース、MCP 連携、規則 10（プロジェクト自己完結）を定義。

### [HIST:SPEC-REV:v2.0] 2026-04-19 全面改訂（残課題 70 件の解消）
[HIST-TAG: category=SPEC-REV id=v2.0 topics=lexer,dictionary,ime,accessibility,export,mcp]
- 字句規則、最短一致辞書、IME 変換中ガード、アクセシビリティ、エクスポートパイプライン、MCP プロトコル等の残課題 70 件を仕様として確定。

### [HIST:SPEC-REV:v2.1] 2026-04-19 再監査による曖昧箇所 28 件の解消
[HIST-TAG: category=SPEC-REV id=v2.1 topics=audit,edge_cases,diagnostics]
- 境界条件・エラー時のフォールバック・診断発火条件の曖昧箇所 28 件を解消。

### [HIST:SPEC-REV:v2.2] 2026-04-19 EBNF・索引・設定フォールバック等 19 件の確定
[HIST-TAG: category=SPEC-REV id=v2.2 topics=ebnf,materials_index,ui_json,i18n]
- A/B 形式の EBNF、資料索引 `materials/index.json` の整合規則、`ui.json` フォールバック、国際化カタログ規約など 19 件を確定。

### [HIST:SPEC-REV:v2.3] 2026-04-19 継続行・範囲式・正規化等 12 件の確定
[HIST-TAG: category=SPEC-REV id=v2.3 topics=continuation_line,range_expr,nfkc,palette]
- 継続行インデント、範囲式の閉区間判定、NFKC 辞書正規化、パレットタイブレーク等 12 件を確定。

### [HIST:SPEC-REV:v2.4] 2026-04-19 モード併用上限・行位置・エクスポート等 13 件の確定
[HIST-TAG: category=SPEC-REV id=v2.4 topics=modes,E005,export,storage]
- モード同時指定上限（最大 2 個、3 個以上は `E005`）、エクスポート時の空白・ルビ処理等 13 件を確定。

### [HIST:SPEC-REV:v2.5] 2026-04-19 EBNF 改行・エスケープ・役割語等 11 件の確定
[HIST-TAG: category=SPEC-REV id=v2.5 topics=ebnf,escape,role_language,agents]
- EBNF の改行表現、7 種エスケープ解除、役割語アンカー規則、エージェントタスク状態等 11 件を確定。

### [HIST:SPEC-REV:v2.6] 2026-04-19 最短一致・範囲解決・CLI・MCP 等 14 件の確定
[HIST-TAG: category=SPEC-REV id=v2.6 topics=shortest_match,modes,cli,mcp]
- `'` 参照の最短一致規則、同名 `mode_id` の `range:` 変種選択規則、CLI 終了コード、MCP ツール引数等 14 件を確定。

### [HIST:SPEC-REV:v2.7] 2026-04-19 フィールドキー別名・NFKC・パレット等 11 件の確定
[HIST-TAG: category=SPEC-REV id=v2.7 topics=field_alias,nfkc,palette,export]
- 日本語フィールドキー別名表、NFKC 長音正規化、パレット先頭文字分岐等 11 件を確定。

### [HIST:SPEC-REV:v2.8] 2026-04-19 エスケープ・インレイ・保存・MCP 等 10 件の確定
[HIST-TAG: category=SPEC-REV id=v2.8 topics=escape,inlay_hint,save_machine,mcp]
- エスケープの有効位置、インレイヒント省略幅、保存ステートマシンの指数バックオフ、MCP 監査ログ等 10 件を確定。

### [HIST:SPEC-REV:v2.9] 2026-04-19 役割語下線・自動保存・エクスポート等 9 件の確定
[HIST-TAG: category=SPEC-REV id=v2.9 topics=role_language_underline,autosave,export]
- 役割語一致時の下線範囲（ワイルドカード除外の和集合）、自動保存デバウンス、エクスポート見出しオプション等 9 件を確定。

### [HIST:SPEC-REV:v2.10] 2026-04-19 会話開閉・モード重複・CLI 等 6 件の確定
[HIST-TAG: category=SPEC-REV id=v2.10 topics=dialogue,modes,cli]
- 会話括弧の開閉整合（`E009` / `W011`）、同一節 `mode:` での同一キャラ重複（`E005`）、CLI 引数解釈順等 6 件を確定。

### [HIST:SPEC-REV:v2.11] 2026-04-19 ルビ優先・エスケープ・範囲構文等 7 件の確定
[HIST-TAG: category=SPEC-REV id=v2.11 topics=ruby,escape,range_expr]
- ルビの縦棒あり/なし境界、エスケープ後の字句扱い、範囲式の空白許容等 7 件を確定。

### [HIST:SPEC-REV:v2.12] 2026-04-19 コメント内エスケープ・行末話者・字下げ等 5 件の確定
[HIST-TAG: category=SPEC-REV id=v2.12 topics=comment,speaker_annotation,indent]
- ブロックコメント終端エスケープ `\*\/`、行末話者注釈の厳密位置、エクスポート時の行頭全角空白字下げ等 5 件を確定。

### [HIST:SPEC-REV:v2.13] 2026-04-19 フィールド判定・空節・インレイ等 6 件の確定
[HIST-TAG: category=SPEC-REV id=v2.13 topics=field_line,empty_section,inlay_hint]
- フィールド行の 7 段判定（URI スキーム除外）、空節 `W012` の発火条件、インレイヒントのカーソル透過等 6 件を確定。

### [HIST:SPEC-REV:v2.14] 2026-04-19 提案ファイル命名・MCP 等 3 件の確定
[HIST-TAG: category=SPEC-REV id=v2.14 topics=proposals,mcp]
- 人間用変更提案 `H####.diff` とエージェント提案 `P####.diff` の命名区分、MCP ツール戻り値形式等 3 件を確定。

### [HIST:SPEC-REV:v2.15] 2026-04-19 Windows コンソールアタッチとヘルプ規約の確定
[HIST-TAG: category=SPEC-REV id=v2.15 topics=windows,AttachConsole,cli]
- Windows での `windows_subsystem = "windows"` と `AttachConsole(ATTACH_PARENT_PROCESS)` の併用規則を確定。

### [HIST:SPEC-REV:v2.16] 2026-04-19 27 個の追加仕様と段階 ①/② ロードマップの統合
[HIST-TAG: category=SPEC-REV id=v2.16 topics=stage2,seams,E029]
- 追加仕様 27 項目と段階 ① / 段階 ② の区分、および段階 ① で仕込む 10 個の縫い代を策定。プロンプト必須セクション欠落の `E029` を追加。

### [HIST:SPEC-REV:v2.17] 2026-04-20 M0 着手前監査による実装仕様の確定
[HIST-TAG: category=SPEC-REV id=v2.17 topics=egui,crate_allowlist,ime,storage_policy,ExportFormat]
- egui 0.27 での IME 状態機械、ベースライン下線自前描画、インレイ隙間確保、`core` クレート許可リスト、規則 10 の単一関門 `FsGate`、エクスポート純粋関数シグネチャを確定。

### [HIST:SPEC-REV:v2.18] 2026-04-21 M7 着手前監査による GUI/Web 境界の確定
[HIST-TAG: category=SPEC-REV id=v2.18 topics=dialog_rfd,ChosenPath,wasm32,gui]
- ファイル選択ダイアログ `ports::dialog::Dialog` と `ChosenPath` 証票型、`rfd` / `directories` / `fontdb` のターゲット別依存設定を確定。

### [HIST:SPEC-REV:v2.19] 2026-10-14 v0 移行仕様の全面廃止とドキュメント分割
[HIST-TAG: category=SPEC-REV id=v2.19 topics=v0_removal,diagnostics_76,criterion,docs_reorg]
- v0 ユーザーが存在しないため、v0→v1 マイグレーション仕様（旧 §14.6、`novnow migrate`、旧 v0 検知メッセージ、診断 `E061`/`W062`/`I051`/`I003`/`I004` の 5 件）を全面廃止し、診断カタログを 76 件（39 + 10 + 27）に整理。
- `[dev-dependencies]` の許可リストに `criterion` を追加。
- 単一ファイルだった旧 `SPECIFICATIONS.md` を廃止し、`docs/specifications/*.md`（純粋仕様 10 ファイル）と `docs/designs/*.md`（設計 10 ファイル）に分離。

### [HIST:SPEC-REV:v2.20] 2026-10-14 実機レビューに基づく修正 44 件・小規模機能追加 18 件・大規模機能追加 1 件の仕様追加
[HIST-TAG: category=SPEC-REV id=v2.20 topics=ui_refinements,palette,morpheme,pwa,zip_import,performance,plans]
- 実機操作に基づく UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 件（`F-01`〜`F-44`）および小規模機能追加 18 件（`S-01`〜`S-18`）を `docs/specifications/*.md` と `docs/designs/*.md` に「未実装・実装計画済み」として追加し、`REF:docs/plans.md#planned-fixes-and-features` に実装計画を策定。
- 既存の本文テキストから `novnow` に移行する手段（`L-01`）を `REF:docs/designs/platform.md` §14.6 および `REF:docs/plans.md#unplanned-large` に「未実装・未計画」として追加。

---

## 2. アーキテクチャ・仕様の意思決定ログ（`DECISION`）

### [HIST:DECISION:D-001] 2026-04-20 `core` 層の純粋性と AST 検査の導入
[HIST-TAG: category=DECISION id=D-001 topics=core,layering,syn,purity]
- **背景**: ドメインロジックが UI（`egui`）や OS ファイルシステム・非同期ランタイムと結合すると、デスクトップと Web（WASM）の挙動乖離やテスト困難を招く。
- **決定**: `src/core/**` から `ports` / `adapters` / `ui` / `cli` および `std::fs` / `std::net` / `std::process` / `std::env` / `std::time::SystemTime` への参照を禁止し、`REF:tests/layering.rs` で `syn` を用いた AST 検査により CI で常時強制することとした。

### [HIST:DECISION:D-002] 2026-04-20 診断カタログと i18n カタログの先行固定
[HIST-TAG: category=DECISION id=D-002 topics=diagnostics,i18n,catalog_tests]
- **背景**: 診断コードと日英メッセージを各機能の実装時に散発的に追加すると、仕様書の表との食い違いやプレースホルダ不一致が発生しやすい。
- **決定**: M1 の段階で全診断コード（`REF:src/core/diagnostics.rs`）と日英カタログ（`REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`）を先に全件定義し、仕様書の Markdown 表を `include_str!` で直接パースして照合するテスト（`REF:tests/diagnostics_catalog.rs`, `REF:tests/i18n_catalog.rs`）で固定した。

### [HIST:DECISION:D-003] 2026-04-20 フィールド行の 7 段判定と URI スキーム保護
[HIST-TAG: category=DECISION id=D-003 topics=lexer,field_line,uri]
- **背景**: A/B/MD 形式の自由記述行に `https://example.com` や時刻 `10:30` が書かれた際、単純な `:` 分割ではフィールド行と誤認される問題があった。
- **決定**: `REF:src/core/text/lexer.rs` に 7 段のフィールド行判定を設け、キー文字種制限（ASCII 英小文字・数字・`_`・`-` または閉じた日本語別名表）と直後の空白必須規則により、URI スキームや時刻表記がフィールド行と誤認されない構造とした。

### [HIST:DECISION:D-004] 2026-04-20 辞書照合における長音正規化 + NFKC と最短一致トライ木
[HIST-TAG: category=DECISION id=D-004 topics=dictionary,nfkc,shortest_match,trie]
- **背景**: 日本語原稿では全角/半角や長音記号の揺れがあり、また `'あおいちゃん` のように助詞・接尾辞が続く場合に最長一致では区切りを誤ることがあった。
- **決定**: `REF:src/core/resolve/dictionary.rs` で長音記号統一＋Unicode NFKC 正規化を行い、トライ木で最初に終端に達したキーで確定する「最短一致」を採用した。より長い名前を参照したい場合は明示境界 `'{あおいちゃん}` を使う規則とした。

### [HIST:DECISION:D-005] 2026-04-20 役割語パターンの 2 重正規表現コンパイル
[HIST-TAG: category=DECISION id=D-005 topics=role_language,regex,underline]
- **背景**: 役割語は「台詞がパターンに一致するか」の高速判定と、「ワイルドカード `*` を除いた固定語部分のみに緑下線を引く」ためのバイト範囲算出の両方が必要だった。
- **決定**: `REF:src/core/resolve/role_language.rs` の `MatcherSet` で、全体一致用の `RegexSet` と、ワイルドカード以外の連続アトムをキャプチャグループ化した個別 `Regex` を同時にコンパイルしてキャッシュする設計とした。

### [HIST:DECISION:D-006] 2026-04-20 規則 10（書き込み境界）の単一関門 `FsGate`
[HIST-TAG: category=DECISION id=D-006 topics=write_policy,FsGate,storage_fs,ChosenPath]
- **背景**: 「開いているプロジェクトディレクトリと履歴ファイル 1 個、および明示指定されたエクスポート先 1 個以外に一切書き込まない」という規則 10 を確実に守る必要があった。
- **決定**: 判定ロジックを `WritePolicy`（`REF:src/core/policy/write_policy.rs`）に集約し、実ファイル書き込み API の呼び出しを `REF:src/adapters/storage_fs.rs` の `FsGate` のみに限定して `REF:tests/layering.rs` で他の全モジュールからの書き込み API 呼び出しを禁止した。

### [HIST:DECISION:D-007] 2026-04-20 LLM SDK 非依存と MCP stdio クライアント/サーバの自前実装
[HIST-TAG: category=DECISION id=D-007 topics=mcp,agents,no_llm_sdk]
- **背景**: 特定の商用 LLM SDK（OpenAI / Anthropic 等）への依存を排除し、ローカル・外部の任意のージェントランタイムと連携可能にする必要があった。
- **決定**: JSON-RPC 2.0 over stdio の MCP クライアント（`REF:src/adapters/mcp_stdio.rs`）と MCP サーバ（`REF:src/core/agents/server.rs`, `REF:src/cli/mcp.rs`）を標準ライブラリ＋`serde_json` で実装し、`REF:tests/no_llm_sdk.rs` で外部 AI SDK の混入を禁止した。

### [HIST:DECISION:D-008] 2026-04-21 egui 0.27 におけるベースライン下線・インレイ隙間・IME 状態機械
[HIST-TAG: category=DECISION id=D-008 topics=egui,underline,inlay_hint,ime]
- **背景**: `egui 0.27.2` の標準 `TextFormat::underline` は単色実線のみで波線や点線を引けず、また `TextEditState::has_ime` が private フィールドであるため外部から変換中状態を参照できなかった。
- **決定**:
  1. 下線・波線・点線は `Galley` のグリフ座標から `REF:src/ui/widgets/syntax_text.rs` で直接ペイントする。
  2. インレイヒントは `LayoutJob` の該当 section の `leading_space` で隙間を確保し、`REF:src/ui/widgets/inlay.rs` でバッジを重ね描きする。
  3. IME 変換状態は `REF:src/ui/shell.rs` の `ImeState` で `Event::Composition*` を自前追跡し、変換中は全 `Event::Key` をショートカット判定前に破棄する。

### [HIST:DECISION:D-009] 2026-04-21 Web 版の非同期ファイル I/O ブリッジと IndexedDB 永続化
[HIST-TAG: category=DECISION id=D-009 topics=web,wasm,indexeddb,storage_idb]
- **背景**: ブラウザ（`wasm32-unknown-unknown`）では同期ファイルシステム API や `rfd::FileDialog`（同期版）が使えない。
- **決定**: IndexedDB アダプタ（`REF:src/adapters/storage_idb.rs`）とメモリキャッシュを組み合わせ、Zip 取込・書き出しは `REF:src/ui/web.rs` の非同期 `<input type="file">` / Blob ダウンロードブリッジで処理する構成とした。

### [HIST:DECISION:D-010] 2026-04-21 サンドボックス環境向け `cache/rust-toolchain` ブランチの構築
[HIST-TAG: category=DECISION id=D-010 topics=sandbox,toolchain,ci,vendor]
- **背景**: Arena サンドボックスからは `static.rust-lang.org` や `crates.io` に直接アクセスできないため、サンドボックス内でビルド・テストを完結させる仕組みが必要だった。
- **決定**: GitHub Actions（`REF:.github/workflows/toolchain-export.yml`）で Rust sysroot と `cargo vendor` 一式を `cache/rust-toolchain` ブランチに退避し、`REF:scripts/bootstrap-toolchain.sh` で `codeload.github.com` から 30 秒で展開できる構成とした。

### [HIST:DECISION:D-011] 2026-04-22 Web 実機スモークテスト環境の `cache/sandbox-tools` 退避
[HIST-TAG: category=DECISION id=D-011 topics=sandbox,web_smoke,playwright,trunk]
- **背景**: Web 版（WASM）の描画・IndexedDB・IME・キーボード操作をサンドボックス内で実機検証するため、`trunk`・`wasm-bindgen`・Playwright・Headless Chromium が必要だった。
- **決定**: `REF:.github/workflows/sandbox-tools.yml` でこれらを `cache/sandbox-tools` ブランチに退避し、`REF:scripts/web-smoke.sh` で自動展開して 18 シナリオの E2E テスト（`REF:scripts/web-smoke.mjs`）を実行可能にした。

### [HIST:DECISION:D-012] 2026-04-22 Web 版でのキーボード節移動時の IME 隠し input 同期（`sync_text_agent`）
[HIST-TAG: category=DECISION id=D-012 topics=web,ime,egui_text_agent,focus]
- **背景**: Web 版でマウスクリックではなく `Alt+↓` 等のショートカットで節を移動した際、`eframe 0.27` の隠し `<input id="egui_text_agent">` が blur されたままになり、直後の文字入力が取りこぼされる現象が判明した。
- **決定**: `REF:src/ui/web.rs` に `sync_text_agent` を実装し、毎フレーム末に `ctx.wants_keyboard_input()` を確認して隠し `<input>` の focus/blur を同期することで解消した。

### [HIST:DECISION:D-013] 2026-04-22 同梱フォント `ipag.ttf` における `⟨⟩` グリフのフォールバック維持
[HIST-TAG: category=DECISION id=D-013 topics=font,ipag,inlay_hint,fallback]
- **背景**: インレイヒントで使用する数学括弧 `⟨` `⟩`（U+27E8 / U+27E9）が `assets/ipag.ttf` に含まれておらず、既存フォントリストを `clear()` すると豆腐（`□`）になる問題があった。
- **決定**: `REF:src/ui/theme.rs` で CJK フォントを先頭（index 0）に挿入しつつ、egui 既定の `Hack` 等幅フォントをフォールバックリストに残すことで、CJK 文字と `⟨⟩` の双方を正しく描画させた。

### [HIST:DECISION:D-014] 2026-04-22 C 形式（本文）における連続空行の無損失保持
[HIST-TAG: category=DECISION id=D-014 topics=contents,plot_outline,formatting,empty_lines]
- **背景**: 本文（C 形式）の散文では作家が意図的に置いた空行数が意味を持つ一方、`>format` 実行時は最大 2 行に圧縮する仕様となっていた。
- **決定**: 通常保存時は本文の空行を無損失で保持し、明示的なフォーマッタ `core::features::formatting::format`（`REF:src/core/features/formatting.rs`）およびアウトライン `Outline::to_text`（`REF:src/core/syntax/plot_outline.rs`）で連続空行を最大 2 行に正規化する設計とした。

### [HIST:DECISION:D-015] 2026-10-14 継続行インデントにおける先頭タブ 1 文字の除去規則
[HIST-TAG: category=DECISION id=D-015 topics=plot_outline,lexer,continuation_line,indent]
- **背景**: 共通字句規則（§5.0.6）では継続行のインデントを「2 個以上の半角空白または 1 個以上のタブ」と定めているが、`plot_outline.rs` の `drop_indent` が空白 2 文字のみを剥がし先頭タブを剥がしていなかった。
- **決定**: `REF:src/core/syntax/plot_outline.rs` の `drop_indent` を修正し、先頭タブ 1 文字または半角空白 2 文字を剥がして `EpisodePlot`（`REF:src/core/syntax/plot_episode.rs`）と完全に揃えた。

### [HIST:DECISION:D-016] 2026-10-14 v0→v1 マイグレーション機能の全面撤廃とドキュメント分離
[HIST-TAG: category=DECISION id=D-016 topics=v0_removal,docs_reorg,duplicate_py]
- **背景**: v0 形式の既存ユーザーは存在しないため、v0 検知・移行ロジック（`src/cli/migrate.rs` 等）は不要な複雑性とドキュメント肥大化の原因となっていた。またルートの `SPECIFICATIONS.md` / `PLANS.md` / `DEVELOPERS.md` が肥大化し、仕様と実装設計と過去履歴が混在していた。
- **決定**:
  1. v0→v1 マイグレーションのコード・仕様・診断 5 件を全面撤去した。
  2. ルートの 3 ドキュメントを廃止し、`docs/specifications/*.md`（純粋仕様）、`docs/designs/*.md`（実装設計・状態・`REF:`）、`REF:docs/developers.md`、`REF:docs/plans.md`、`REF:docs/handoff.md`、`REF:docs/history.md` に分離し、`REF:docs/script/duplicate.py` で機械検査する体制へ移行した。

### [HIST:DECISION:D-017] 2026-10-15 規則原文ブロックと duplicate.py の食い違い解消
[HIST-TAG: category=DECISION id=D-017 topics=docs_reorg,duplicate_py,verbatim]
- **背景**: `REF:docs/developers.md` §1 の規則原文（フェンス内）と `REF:docs/script/duplicate.py` の `VERBATIM_RULES_BLOCK` が 2 行食い違っており、`docs` 検査（rule-3）が赤になっていた。`REF:docs/add-dependencies.md` の追加時に片側だけが更新されたまま残っていたのが原因。
- **決定**: ユーザーの裁定（2026-10-15）により `REF:docs/developers.md` の原文を正とし、`REF:docs/script/duplicate.py` の `VERBATIM_RULES_BLOCK` を実ディレクトリ構成に合わせた（`├ add-dependencies.md` 行を追加し、廃止済みルート 3 ファイルに触れる文を削除）。

---

## 3. 達成済みマイルストーン履歴（`MILESTONE`）

### [HIST:MILESTONE:M0] 2026-04-20 M0 骨格・型・層境界検査の構築（PR #1）
[HIST-TAG: category=MILESTONE id=M0 topics=skeleton,id_range,layering,PR-1]
- クレート骨格（`core`, `ports`, `adapters`, `ui`, `cli`）、`SceneId` / `RangeExpr`（`REF:src/core/id.rs`）、および `syn` による層境界・純粋性検査（`REF:tests/layering.rs`）を実装。

### [HIST:MILESTONE:M1] 2026-04-20 M1 診断カタログ・国際化カタログ・テキスト基盤（PR #2, PR #3）
[HIST-TAG: category=MILESTONE id=M1 topics=diagnostics,i18n,lexer,rope,PR-2,PR-3]
- 診断カタログ全件（`REF:src/core/diagnostics.rs`）、日英カタログ（`REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`）、共通字句規則（`REF:src/core/text/lexer.rs`）、Rope バッファ・位置変換・SHA-256（`REF:src/core/text/mod.rs`）を実装。

### [HIST:MILESTONE:M2] 2026-04-20 M2 全ファイル形式パーサと無損失往復変換（PR #4）
[HIST-TAG: category=MILESTONE id=M2 topics=syntax,parsers,roundtrip,golden,PR-4]
- C 形式（`contents.rs`）、A 形式（`plot_outline.rs`）、B 形式（`plot_episode.rs`）、`characters_md.rs`、`words_md.rs`、`ideas_md.rs`、`materials_index.rs`、`prompt_md.rs`、`project_json.rs` の全 9 パーサと無損失 `to_text()` を実装。

### [HIST:MILESTONE:M3] 2026-04-20 M3 意味解決エンジン・役割語・インクリメンタル再解析（PR #5）
[HIST-TAG: category=MILESTONE id=M3 topics=dictionary,modes,role_language,incremental,PR-5]
- 3 供給元統合辞書と最短一致トライ木（`REF:src/core/resolve/dictionary.rs`）、モード優先順位解決（`REF:src/core/resolve/modes.rs`）、役割語パターンコンパイラ（`REF:src/core/resolve/role_language.rs`）、資料解決（`REF:src/core/resolve/materials.rs`）、1 行再字句解析（`REF:src/core/text/incremental.rs`）を実装。

### [HIST:MILESTONE:M4] 2026-04-20 M4 LSP 11 機能・エクスポート・コンテキスト優先順位（PR #6）
[HIST-TAG: category=MILESTONE id=M4 topics=lsp_features,export,context_rules,PR-6]
- `src/core/features/` の LSP 11 機能モジュール、14 段エクスポートパイプライン（`REF:src/core/export.rs`）、参照ペイン・AI コンテキスト共通の優先順位規則（`REF:src/core/resolve/context_rules.rs`）を実装。

### [HIST:MILESTONE:M5] 2026-04-21 M5 ストレージ境界・規則 10・雛形生成（PR #7）
[HIST-TAG: category=MILESTONE id=M5 topics=write_policy,storage_fs,recent,zip,template,PR-7]
- `WritePolicy` と単一書き込み関門 `FsGate`（`REF:src/adapters/storage_fs.rs`）、履歴ファイル `recent.rs`、Zip 往復変換 `zip.rs`、初期雛形 `core::template` を実装。

### [HIST:MILESTONE:M6] 2026-04-21 M6 AI エージェント・コンテキスト予算・MCP・CLI（PR #8）
[HIST-TAG: category=MILESTONE id=M6 topics=agents,aicontext,mcp,cli,PR-8]
- 委譲レベル 4 段階と安全装置 A1〜A8（`REF:src/core/agents/runner.rs`）、9 セクション AI コンテキスト組み立てとトークン予算切り詰め（`REF:src/core/aicontext/budget.rs`）、MCP クライアント/サーバ、および CLI 全サブコマンド（`REF:src/cli/mod.rs`）を実装。

### [HIST:MILESTONE:M7] 2026-04-21 M7 共通シェル・`write` ワークスペース・保存状態機械・IME（PR #9）
[HIST-TAG: category=MILESTONE id=M7 topics=ui,shell,workbench,save_machine,ime,theme,PR-9]
- 共通シェル（`REF:src/ui/app.rs`）、文脈ビュー、節エディタ、参照ペイン、統一パレット、保存状態機械（`REF:src/ui/save_machine.rs`）、IME 状態機械（`REF:src/ui/shell.rs`）、Dark/Light テーマ（`REF:src/ui/theme.rs`）を実装。

### [HIST:MILESTONE:M8] 2026-04-21 M8 残り 7 ワークスペースと人間用提案モード（PR #10, PR #11）
[HIST-TAG: category=MILESTONE id=M8 topics=workspaces,plot,idea,material,cast,agent,review,proposals,PR-10,PR-11]
- `plot-episode`, `plot-outline`, `idea`, `material`, `cast`, `agent`, `review` の 7 ワークスペース（`REF:src/ui/workspaces/mod.rs`）と人間用変更提案 `H####.diff`（`REF:src/core/agents/files.rs`）を実装。

### [HIST:MILESTONE:M9] 2026-04-22 M9 Web 版（WASM / IndexedDB / WebPicker / E2E スモーク）（PR #12〜#15）
[HIST-TAG: category=MILESTONE id=M9 topics=web,wasm,indexeddb,web_smoke,PR-12,PR-13,PR-14,PR-15]
- IndexedDB アダプタ（`REF:src/adapters/storage_idb.rs`）、Web プロジェクト選択モーダル・Zip I/O ブリッジ・`beforeunload` 保護・`sync_text_agent`（`REF:src/ui/web.rs`）、および Headless Chromium による 18 シナリオの自動スモークテスト（`REF:scripts/web-smoke.mjs`）を実装。

### [HIST:MILESTONE:M10] 2026-04-22 M10 エージェント実接続・ドライラン・提案差分レビュー（PR #11, PR #12）
[HIST-TAG: category=MILESTONE id=M10 topics=agents_ui,mcp_stdio,diff_review,PR-11,PR-12]
- `agent` ワークスペースからの非同期タスク実行・ドライラン・節単位採否レビュー・MCP サーバ死活監視（`REF:src/ui/agents.rs`）を接続。

### [HIST:MILESTONE:M11] 2026-10-14 M11 性能ベンチマーク・サンプル v1 化・v0 撤去・ドキュメント再編（PR #16）
[HIST-TAG: category=MILESTONE id=M11 topics=benchmarks,criterion,sample_project,v0_removal,docs_reorg,PR-16]
- **LSP 11 機能の完全接続**: `definition`, `references`, `folding`, `formatting`, `rename` を含む全 11 モジュールを `src/ui/` に接続し、`REF:tests/layering.rs` で常時検証。
- **v0→v1 移行機能の完全撤去**: `src/cli/migrate.rs` や旧 v0 診断 5 件を撤去し、診断カタログを 76 件に統一。
- **性能ベンチマーク計測結果（`REF:benches/core.rs`、`cargo bench --locked --bench core -- --quick`）**:
  - `single_line_relex`: **462.53 ns**（目標 `<= 1 ms`）
  - `role_language_10_patterns`: **1.92 µs**（目標 `<= 20 µs`）
  - `dictionary_trie_5000`: **1.16 ms**（目標 `<= 10 ms`）
  - `reference_pane_rebuild`: **38.17 µs**（目標 `<= 16 ms`）
  - `export_100k_chars`: **7.21 ms**（目標 `<= 100 ms`）
- **Web リリースバンドルサイズ**:
  - `novnow_bg.wasm` raw: **13,819,966 bytes** / gzip: **7,077,276 bytes（6.75 MiB）**（目標 `<= 8.00 MiB`）。
- **サンプルとドキュメントの再編成**: `examples/sample-novel/` を付録 C の v1 `sample` プロジェクトに更新し、`docs/` 配下へのドキュメント分割と `REF:docs/script/duplicate.py` による機械検査を導入。
