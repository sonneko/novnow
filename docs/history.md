# 変更履歴・意思決定アーカイブ（History — 追記専用）

本書は過去の仕様改訂履歴、アーキテクチャ・仕様の意思決定ログ（「最初は X だったが Y に変更した」等の経緯）、および達成済みマイルストーンの記録を保持する**追記専用（append-only）**ファイルである。

---

## 0. `grep` による機械検索ガイド

本書を先頭から全文読まずに目的の経緯を特定するため、すべての記録エントリは次の固定ヘッダとタグ行を持つ。

- 見出し形式: `### [HIST:<CATEGORY>:<ID>] <YYYY-MM-DD> <要約タイトル>`
- タグ行形式: `[HIST-TAG: category=<CATEGORY> id=<ID> topics=<カンマ区切りキーワード>]`
- `<CATEGORY>` の種別:
  - `SPEC-REV`: 仕様書の版改訂履歴（`v1.0`〜`v2.21`）
  - `DECISION`: アーキテクチャ・個別設計判断ログ（`D-001`〜`D-017` および旧計画書 §9.2/§9.4 の `B-1`〜`B-161` 全件）
  - `MILESTONE`: 完了した開発マイルストーンと PR・計測結果（`M0`〜`M11`）
  - `SPEC-FIX`: 仕様の要補正点と裁定アーカイブ（旧計画書 §9.1 の `A-1`〜`A-24` 全件）
  - `SPEC-DECISION`: 仕様策定時の派生決定・未決事項裁定・表記揺れ解決（旧仕様書 §0.5・§0.6.1・付録 D の `D-1`〜`D-20`, `U-1`〜`U-8`）
  - `FUTURE`: 将来機能提案と v1 拡張点の確認アーカイブ（旧仕様書 付録 E の `F1`〜`F12`, `E-1`）
  - `ACCEPTANCE`: 段階 ①（開発指示 1〜13）の検収条件と達成記録（旧仕様書 §19.1〜§19.13 の `IND-01`〜`IND-13`）
  - `RISK`: 段階 ① リスク台帳と IME 事前調査記録（旧計画書 §8 の `R-1`〜`R-10`, `R-2p`）

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

### [HIST:SPEC-REV:v2.21] 2026-10-15 F-42 の助詞空白規則に合わせた診断カタログの日本語列の修正
[HIST-TAG: category=SPEC-REV id=v2.21 topics=i18n,spacing,diagnostics,ja_toml]
- `REF:docs/specifications/diagnostics.md` §7 の日本語メッセージ列 19 行（23 箇所）で、プレースホルダの直後に助詞が続く箇所の半角空白を除去した（`{0} は…` → `{0}は…`）。§12.3 の規則（v2.20 で追加）に合わせる修正で、人間の承認を得て実施（2026-10-15）。
- `REF:src/ui/i18n/ja.toml` の同じ形の 84 箇所も同時に修正し、`REF:tests/i18n_catalog.rs` に「`}` の直後に半角空白＋助詞が続かないこと」の機械検査を追加した（`F-42`）。

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

---

## 4. 仕様の要補正点と裁定アーカイブ（`SPEC-FIX`: `A-1`〜`A-24`）

### [HIST:SPEC-FIX:A-1] 2026-10-09 §3.2 core の許可依存
[HIST-TAG: category=SPEC-FIX id=A-1 topics=A-1,nfkc]
- **対象箇所**: §3.2 core の許可依存
- **問題**: NFKC（§6.1.3）に `unicode-normalization` が要るが、許可リストに無い
- **裁定と解決**: **解消（2026-10-09 / 仕様第 2.1 版）**。§3.2 を許可クレート表に書き換え、`unicode-normalization` を追加した

### [HIST:SPEC-FIX:A-2] 2026-10-09 §17.3.3 と §3.2
[HIST-TAG: category=SPEC-FIX id=A-2 topics=A-2,reader,modes,theme]
- **対象箇所**: §17.3.3 と §3.2
- **問題**: 縦書きレイアウトに**フォント実測値**が要るが、core はフォントを読めない
- **裁定と解決**: **解消（2026-10-09 / 仕様第 2.1 版）**。読者モードを #27 として ② へ移管。設計方針（`layout(text, &FontMetrics, opts)` の純粋関数）は §17.3.3 と §19.14-27 に記録したので ② で引き継ぐ

### [HIST:SPEC-FIX:A-3] 2026-10-09 §3.4 と §9.3.3
[HIST-TAG: category=SPEC-FIX id=A-3 topics=A-3,folding,contents]
- **対象箇所**: §3.4 と §9.3.3
- **問題**: `folding` は ① の 11 機能に含まれるが、折りたたみ UI は追加仕様 9
- **裁定と解決**: **解消（2026-10-09 / 仕様第 2.2 版）**。§3.4 の `folding` 行を「`plot-outline` の章→話展開（§10.3.1）」に修正し、本文折りたたみ UI が追加仕様 9 である旨を注記した

### [HIST:SPEC-FIX:A-4] 2026-10-09 §14.1
[HIST-TAG: category=SPEC-FIX id=A-4 topics=A-4,cli,ui]
- **対象箇所**: §14.1
- **問題**: サブコマンド一覧に `lsp` が無い（U-3 保留の帰結）が、`DEVELOPERS.md` との整合が未確認
- **裁定と解決**: **解消（2026-10-09）**。`DEVELOPERS.md` を全面改訂。`--init` → `new`、層構造、8 形式、IME（§9.8.5）、`Alt+7` 欠番、保存の考え方、`lsp` 不在の理由を反映。**M11 では sample と実装の実態に合わせた微修正だけでよい**

### [HIST:SPEC-FIX:A-5] 2026-10-11 §12.2 コア層の文字列禁止
[HIST-TAG: category=SPEC-FIX id=A-5 topics=A-5,i18n,lexer,testing]
- **対象箇所**: §12.2 コア層の文字列禁止
- **問題**: 「ユーザー向け文言」の定義が、**ファイル形式の語彙**（§5.0.6 の日本語別名表）と区別できない。別名は UI 言語が `en` でも受理する（§12）のでロケール表 `ui/i18n/` には置けず、字句解析を持つ core が持つしかない
- **裁定と解決**: §12.2 に「**字句データ（シジル文字・フィールドキー別名など、ファイル形式の語彙）は文言に含まない**」の 1 文を足す。実装側は `tests/layering.rs` の狭い免除（`i18n: format-vocabulary` の doc 行を付けた `const` / `static` の中だけ）で担保し、検査器の自己テストも足した（§9.2 B-19）。**解消（2026-10-11 / 仕様第 2.5 版）**: ユーザー承認のもと §12.2 に「字句データ（シジル文字・フィールドキー別名など、**ファイル形式の語彙**）は文言に含まない」の 1 文と、免除の書き方（`i18n: format-vocabulary`）を追記した

### [HIST:SPEC-FIX:A-6] 2026-10-11 §5.0.5 フィールド行の ebnf
[HIST-TAG: category=SPEC-FIX id=A-6 topics=A-6,lexer]
- **対象箇所**: §5.0.5 フィールド行の ebnf
- **問題**: (1) `ws (value
- **裁定と解決**: "")` は `:` の直後に**必ず空白**を要求するが、§5.0.4 は保存時に行末の空白を削除する（MUST）ので、空の値のフィールド行 `summary:` は保存のたびにフィールド行でなくなる。(2) 全角コロン＋空白なし（`要約：あらすじ`）は日本語の自然な書き方だが、この ebnf では通常テキストになる

### [HIST:SPEC-FIX:A-7] 2026-10-13 §7 の E004 の文言
[HIST-TAG: category=SPEC-FIX id=A-7 topics=A-7,i18n,diagnostics,characters,modes,testing]
- **対象箇所**: §7 の E004 の文言
- **問題**: ja も en も `{0}` を使わず `{1}`〜`{3}` だけを使う（`モード {1} は {2} では使用できません（有効範囲: {3}）`）。引数の数を「最大の添字 + 1」で数えると **4** になり、`args[0]` はどの言語でも捨てられる
- **裁定と解決**: (1) `{0}`〜`{2}` に振り直す（文言の修正だけで済む）、(2) `{0}` に入れる予定だったもの（節 ID か）を書いて予約のままにする、のどちらか。**実装は現状の仕様どおり 4 引数**で、`tests/diagnostics_catalog.rs` の `e004_skips_the_first_placeholder` が固定している（仕様書を直すとテストが落ちる）。**M3 の `resolve/modes.rs` は `args[0]` にキャラ名を入れて予約している**（§6.4.4 の E004 を出す実装。裁定まで表示は変わらない）。**解消（2026-10-13 / 仕様 第 2.8 版）**: ユーザー裁定は「`{0}` をキャラ名として使う」。§7 の E004 を ja `{0} のモード {1} は {2} では使用できません（有効範囲: {3}）` / en `Mode {1} of {0} is not available at {2} (range: {3})` にした（`resolve/modes.rs` が `args[0]` に入れていたキャラ名がそのまま出る）。テストは `e004_names_the_character_first` に置き換え、添字の抜けを全件で禁じる `every_placeholder_index_is_used` を足した

### [HIST:SPEC-FIX:A-8] 2026-10-13 §7.2 の冒頭文と表
[HIST-TAG: category=SPEC-FIX id=A-8 topics=A-8,diagnostics,agents]
- **対象箇所**: §7.2 の冒頭文と表
- **問題**: 冒頭は「すべて **W（警告）または I（情報）** であり、エラーにしない（P4）」と言うが、表には **E032〜E036・E064 の 6 件のエラー**がある（カスタムロール・委譲レベル・ローカル LLM 接続）
- **裁定と解決**: 冒頭文を「**文章の良し悪し**を言うものは W / I にする（P4）。設定・接続・権限の誤りは E にする」に直す。実装は表に従っており、`Group::Writing` に E が 6 件入っている。**解消（2026-10-13 / 仕様 第 2.8 版）**: 冒頭文を提案どおりに直した（文章の良し悪しを言うものは W / I、設定・接続・権限の誤り＝E032〜E036・E064 だけ E）。`Group::Writing` の doc も合わせた

### [HIST:SPEC-FIX:A-9] 2026-10-13 §5.1.2 の予約番号と §7.2 の E020 の文言
[HIST-TAG: category=SPEC-FIX id=A-9 topics=A-9,i18n,diagnostics,plot_outline,contents,testing]
- **対象箇所**: §5.1.2 の予約番号と §7.2 の E020 の文言
- **問題**: §5.1.2 は「`C = 00` は予約（未使用）。v1 では書いてはならない（**E020**）」「`A = 00` は `structure.allow_chapter_zero = false` にすると **E020** で拒否」と定めるが、E020 の ja 文言は `このファイルには {0} 階層の見出しだけを書けます`（quickfix「対応ファイルへ移動」）で、**番号が予約であること**を表現できない。C 形式は 3 階層と 4 階層の両方を受け付ける（§5.2.1 `scene_id`）ので、`{0}` に 3 を入れると「3 階層だけ」と読めて正当な `@01-04-02-03` と矛盾する
- **裁定と解決**: (1) E020 の文言に理由の枠を足す（`このファイルに書ける見出しは {0} 階層です（{1}）` など）、(2) 予約番号用に別コードを新設する（例 **E021「この番号は予約されています」**、quickfix「番号をずらす」）、のどちらか。**実装は現状の仕様どおり E020 ＋ `{0} = 3`**（`core/syntax/contents.rs` の `warn_reserved`。`tests/contents.rs` の `a_reserved_scene_zero_is_e020` が固定しているので、仕様を直すとテストも直す）。**解消（2026-10-13 / 仕様 第 2.8 版）**: ユーザー裁定は「別コードを新設」。E021 は使用済みなので **E022「予約番号の見出し（§5.1.2）」**（ja `この番号は予約されています: {0}` / en `This number is reserved: {0}` / 処置「番号をずらす」）を §7 に足した。コード数は 78 → **79**（§7 = 38）。`{0}` は書かれたままの ID（`@` を除く）。実装は `contents.rs` の `warn_reserved`（C・B 形式）と `plot_outline.rs` の `check_header`（A 形式の `A = 00`。階層違いと重なれば E020 と E022 を両方出す）。テストは `a_reserved_scene_zero_is_e022` ほか（§9.2 **B-75**）

### [HIST:SPEC-FIX:A-10] 2026-10-13 §6.2.5 の `@` の解決表と §5.3 の見出し階層
[HIST-TAG: category=SPEC-FIX id=A-10 topics=A-10,i18n,diagnostics,plot_outline,plot_episode,contents]
- **対象箇所**: §6.2.5 の `@` の解決表と §5.3 の見出し階層
- **問題**: §6.2.5 は「行頭（空白のみ先行）→ 構造見出し。`scene_id` としてパース → **E002**」と一律に定めるが、§5.3（A 形式）は「ファイル内の見出し階層は揃っていること（`all.txt` に `@01-04` が現れたら **E020**）」と、**階層数の違いだけ**を E020 にしている。C 形式に `@01-04`（2 階層）を書いたとき、§6.2.5 どおりなら E002（quickfix「近い形式に修正」）、§5.3 と揃えるなら E020（quickfix「対応ファイルへ移動」＝ `plots/01-04.txt`）で、**後者のほうが作者の次の行動に合う**
- **裁定と解決**: §6.2.5 の表の「行頭」行を「`scene_id` としてパース。**階層数がその形式の許容と違えば E020**、それ以外の形の不整合は E002」に分ける。**実装は §6.2.5 どおり E002**（`contents.rs` の `parse_scene`。`tests/contents.rs` の `a_malformed_scene_id_is_e002` が固定）。**解消（2026-10-13 / 仕様 第 2.8 版）**: 提案どおり。§6.2.5 の「行頭」行を「形が崩れていれば E002、ID としては読めるが階層数が違えば E020、予約番号は E022」に分けた。C・B 形式の E020 の `{0}` は範囲 `3-4`（`contents::HEADER_DEPTH_RANGE`）で、en 文言は範囲でも読めるよう `Only level {0} headers are allowed here` にした。5 階層以上はどのファイルにも置けないので E002 のまま。テストは `a_shallow_scene_id_is_e020` ほか、ゴールデン 2 件（`contents/broken_unclosed_ref`・`plot_episode/broken_scene_ids`）の期待値も E020 に直した（§9.2 **B-75**）

### [HIST:SPEC-FIX:A-11] 2026-10-11 §5.0.2 のシジル表の `;` と §5.3.1 の `block_end`
[HIST-TAG: category=SPEC-FIX id=A-11 topics=A-11,lexer,testing]
- **対象箇所**: §5.0.2 のシジル表の `;` と §5.3.1 の `block_end`
- **問題**: 表の「出現場所」は `;`（SectionBreak）を「C 形式 / B 形式」としていたが、§5.3.1（A 形式）は `block_end = ws? , ";" , eol` を持ち、§5.3.3・§5.3.4 の**例はどちらも `;` でブロックを閉じている**（4 箇所が要求し、表の 1 箇所だけが否定する）。M1 の字句器は表どおりに実装したので、A 形式では `;` が通常テキストになり**ブロックが閉じなかった**
- **裁定と解決**: **解消（2026-10-11）**。§5.0.2 の `;` の行を「節区切り・ブロック終わり（行全体が `;` のときのみ）／A / B / C 形式（`.txt`）」に直した。実装も `Formats::CB` を廃止して `Formats::TXT` に統一し（`core/text/lexer.rs` の `SIGIL_TABLE` と `TextFormat::allows_section_break`）、`tests/lexer.rs` の 4 テストを「`.txt` 3 形式で節区切り／`.md` では通常テキスト」に直した。`.md` の挙動は変わらない

### [HIST:SPEC-FIX:A-12] 2026-10-11 §5.3.3 の例と §5.0.6 のキー表
[HIST-TAG: category=SPEC-FIX id=A-12 topics=A-12,diagnostics,plot_outline,testing]
- **対象箇所**: §5.3.3 の例と §5.0.6 のキー表
- **問題**: §5.3.3（`plots/all.txt` の例）は前文に `title: 星屑の時計` を書くが、§5.0.6 の 36 行のキー表に **`title` は無い**。仕様どおりに実装すると**仕様書自身の例が W011（未知のキー）を出す**（`tests/plot_outline.rs` の `the_spec_example_for_all_txt_warns_about_the_unknown_title_key` が固定している）。作品の題は `project.json`（§4.5）にも入るので、前文の `title:` が要るのかも不明
- **裁定と解決**: **解消（2026-10-11 / 仕様第 2.6 版）**。ユーザー裁定は「**`title` キーも正式追加する**」（＝上記 (1)）。§5.0.6 の表に `title`（別名「題」、使用形式＝A 形式）を足し、実装は `FieldKey::Title` ＋ `FIELD_KEYS` の 1 行（`Formats::PLOT_OUTLINE`）。§5.3.3 の例は**診断 0 件**になり、`tests/plot_outline.rs` の `the_spec_example_for_all_txt_reads_the_title_key` と `tests/golden/plot_outline/spec_5_3_3.expected.json` が固定する。題は `project.json`（§4.5）にも入るが、前文の `title:` は「このファイルが対象とする作品の題」として両立する（§5.3.3 の前文）。別名の割り当ては §9.2 **B-50**

### [HIST:SPEC-FIX:A-13] 2026-10-14 §5.4.2 の制約と §7.2 の E005 の行
[HIST-TAG: category=SPEC-FIX id=A-13 topics=A-13,i18n,diagnostics,characters,modes]
- **対象箇所**: §5.4.2 の制約と §7.2 の E005 の行
- **問題**: §5.4.2（B 形式）は「1 節の `mode` に**同じキャラを 2 回**書くのは **E005**。1 キャラに 3 つ以上のモード（`A+B+C`）も **E005**」と **2 つの条件**を E005 に割り当てるが、§7.2 の E005 の行は条件が「1 節に 3 つ以上のモード」、ja 文言が `1 つの節に設定できるモードは 2 つまでです`、処置が「先頭 2 つを残す」だけで、**「同じキャラを 2 回」を言い表せない**（文言も処置も 3 モード前提）。§6.4.4 も「1 キャラ 1 節につき最大 2 モード」しか書いていない
- **裁定と解決**: (1) §7.2 の E005 の条件を「1 キャラに 3 つ以上のモード、**または** 1 節の `mode` に同じキャラを 2 回」に広げ、ja 文言を両方に効くものにする、(2)「同じキャラを 2 回」に別のコードを新設する（コードの新設は仕様更新が先、MUST）、のいずれか。**解消（2026-10-14 / 仕様 第 2.17 版）**: §5.4.2 に合わせ、§7 の E005 の条件を「1 キャラに 3 つ以上のモード、または 1 節の `mode` に同じキャラを 2 回」、ja 文言を `1 つの節で同じ人物に設定できるモードは 2 つまでです`、en 文言を `At most two modes per character in a section` に更新し、`src/ui/i18n/{ja,en}.toml` も同期した

### [HIST:SPEC-FIX:A-14] 2026-10-11 §5.0.2 の行内コメントと §5.4.3 の例（フィールド値の後ろの `//`）
[HIST-TAG: category=SPEC-FIX id=A-14 topics=A-14,lexer,characters,modes]
- **対象箇所**: §5.0.2 の行内コメントと §5.4.3 の例（フィールド値の後ろの `//`）
- **問題**: §5.0.2 は「未エスケープの `//` 以降、行末までコメント」を**行内でも**有効なものとして定義し、§5.4.3（B 形式）の例は `mode: 'リリア:B+C          // 呆れモードと好奇心モードの併用（2 つまで）` と**フィールド行の値の後ろに行内コメント**を書く。ところが §5.0.5 のフィールド行の 7 段の手順に行末コメントの除去は無く、仕様どおりに実装すると**コメントが値の一部になる**（§9.2 B-36）。このままだと `length: 500 // 目標` の値が `500 // 目標` になって `Field::number()` が `None` を返す（§6.2 の参照走査も `//` の中まで `'` を探しに行く。実害は無いが無駄）
- **裁定と解決**: (1) §5.0.5 の手順に「値は未エスケープの `//` より前（`://` は URL なので除く、§5.0.2）」を足し、`Field` に `comment: Option<Comment>` を持たせて round-trip を保つ（字句器 1 箇所の変更で A/B 形式に効く）、(2) §5.4.3 の例のコメントを**次の行の行コメント**（`// …`）に直す、のいずれか。**実装はどちらも採らず B-36 のまま**（値に行末コメントを含む。A 形式はこれで緑、B 形式は `two_modes_on_one_character_are_allowed` が固定）。**裁定（2026-10-11）: 今の実装（B-36）で確定**。§5.0.5 にコメント除去は足さず、`Field` に `comment` も持たせない。値を数値として読む層（M3）は行末コメントを自分で落としてから解釈する

### [HIST:SPEC-FIX:A-15] 2026-10-11 §5.0.6 のキー表と §5.5.3 / §5.7 / §5.9.1 / §5.9.2
[HIST-TAG: category=SPEC-FIX id=A-15 topics=A-15,lexer,diagnostics,plot_outline,materials,modes]
- **対象箇所**: §5.0.6 のキー表と §5.5.3 / §5.7 / §5.9.1 / §5.9.2
- **問題**: 各形式の節が**自分で定義しているフィールドキー**が §5.0.6 の 36 行の表に無い。該当は **9 キー**: §5.5.3 の `color`、§5.7 の `status` `created` `relates` `promoted-to`、§5.9.1 の `model-hint` `max-chars` `temperature`、§5.9.2 の `label`。仕様どおりに実装すると**仕様書自身の例が W011（未知のキー）を出す**（§5.5.7 の恩田恵の例で `color` 1 件、§5.7 の例で 4 件、§5.9.1 の例で 3 件、§5.9.2 の例で 3 件。A-12 の `title` と同じ構図で、こちらは 4 形式ぶん）。`status` は §5.0.6 の `state`（別名「状態」）と**意味が近いが別物**（アイデアの inbox / keep / promoted / dropped）なので、別名の追加では解決しない
- **裁定と解決**: **解消（2026-10-11 / 仕様第 2.6 版）**。ユーザー裁定は「**すべてのキーを正式追加する**」（＝上記 (1)。A-12 の `title` と合わせて **10 キー**）。§5.0.6 の表は 36 → **46 行**（① の実装は 28 → **38 キー**）で、`color`（色）・`status`（状況）・`created`（作成日時）・`relates`（関連）・`promoted-to`（昇格先）・`model-hint`（モデル指定）・`max-chars`（最大文字数）・`temperature`（温度）・`label`（表示名）を足した。**別名は双方向 1:1（MUST）なので `state`＝状態 と衝突しないよう `status`＝状況 とした**（§9.2 **B-50**）。`materials/index.json` の JSON キーは表の対象外である旨も注記した。② の 8 キー（本書 §1.2）は表に足さないので ① では W011 のまま。実装は `FieldKey` に 10 variant・`FIELD_KEYS` に 10 行（`title` だけ `Formats::PLOT_OUTLINE`、残り 9 は `Formats::MARKDOWN`）で、`tests/lexer.rs` の `every_key_in_the_table_resolves_both_ways`（38 件）と `the_alias_table_is_one_to_one_in_both_directions` が固定する。W011 が消えた 4 形式の検査は ② のキーと `foo` で残した。仕様書の例 4 件（§5.5.7 / §5.7 / §5.9.1 / §5.9.2）は**診断 0 件**になり、ゴールデン 4 ケースの期待値を更新した

### [HIST:SPEC-FIX:A-16] 2026-10-11 §5.9.2 の例と §15.5.3 の変数表
[HIST-TAG: category=SPEC-FIX id=A-16 topics=A-16,diagnostics,contents,agents,testing]
- **対象箇所**: §5.9.2 の例と §15.5.3 の変数表
- **問題**: §5.9.2（カスタムロールの例）は `本文: {{content_section}}` を使うが、§15.5.3 の**閉じた集合**（19 変数）に `content_section` は無い。本文は `text_current` なので、仕様書自身の例が **E031（使用できない変数）** を出す（`tests/prompt_md.rs` と `tests/golden/prompt_md/spec_5_9_2_custom.expected.json` が固定）
- **裁定と解決**: **解消（2026-10-11 / 仕様第 2.6 版）**。ユーザー裁定は「**サンプルの記述ミスとみなす**」（＝上記 (1)。§15.5.3 の閉じた 19 変数が正本で、`content_section` は足さない）。§5.9.2 の例を `本文: {{text_current}}` に直し、`tests/prompt_md.rs` の `SPEC_CUSTOM` とゴールデンのフィクスチャ `tests/golden/prompt_md/spec_5_9_2_custom.txt` は仕様書から**再抽出**した。例の E031 は消え、`the_custom_role_example_uses_only_variables_of_the_closed_set` が「閉集合の変数だけ」を固定する。**E031 の検査は残す**: `an_unknown_variable_reports_e031` に `{{content_section}}` のケースを追加した（§9.2 **B-52**）

### [HIST:SPEC-FIX:A-17] 2026-10-11 §7 の診断コード表と §5.8.2 / §4.3-4
[HIST-TAG: category=SPEC-FIX id=A-17 topics=A-17,diagnostics,materials,testing]
- **対象箇所**: §7 の診断コード表と §5.8.2 / §4.3-4
- **問題**: `materials/index.json` が **JSON として壊れている**ときの診断コードが §7 の 77 件に無い（`project.json` / `ui.json` / `stats.json` も同じ）。§4.3-4 は「壊れていても開ける。診断として表示し、既定値で続行する（MUST）」と**診断として表示する**ことを要求しているので、コードが無いと MUST を満たせない
- **裁定と解決**: **解消（2026-10-11 / 仕様第 2.6 版）**。ユーザー裁定は「**新しいエラーコードを追加する**」（＝上記 (1)）。§7 に **E025「JSON ファイルがパースできない（§4.3-4）」**を新設した（ja `JSON として読み込めません: {0}` / en `Cannot parse as JSON: {0}` / 処置「既定値で開く / 手動で修正」）。コード数は 77 → **78**（§7 = 37）。番号は E024 の隣で、② の予約域（W080 以降 / E032 以降 / E064 / I054 以降）に触れない空き（§9.2 **B-51**）。§4.3-4 に「JSON がパースできないときは E025」と「**原文は書き換えない**（MUST）」を、§5.8.2 に「E025 のときは空索引の既定値で続行し、E024 / E008 は報告しない」を追記した。実装は `core::syntax::materials_index::parse` が E025（位置＝ファイル全体 `0..raw.len()`、引数 `{0}` ＝パーサのメッセージ）を出し、`Index::error` も残す（呼び出し側がログに書きやすい）。既定値での続行は `Index::index_or_default()`（`tests/materials_index.rs` の 2 テストが固定）。`project.json` / `ui.json` / `stats.json` のパース失敗も**同じ E025** を使う（M5 でストレージ層が出す）〔**M5 で訂正**: `ui.json` / `stats.json` は §4.4.1 の MUST「エラーを出さない」と §4.4.2 により診断を出さない。E025 は `project.json` と `materials/index.json` だけ。§9.2 B-89〕

### [HIST:SPEC-FIX:A-18] 2026-10-13 §7 の E026 の文言
[HIST-TAG: category=SPEC-FIX id=A-18 topics=A-18,i18n,diagnostics,words,materials]
- **対象箇所**: §7 の E026 の文言
- **問題**: ja「`用語に修飾子は付けられません`」/ en「`Modifiers are not allowed on terms`」は**用語専用**の文言だが、§6.1.1 の表は `MaterialTitle`（資料タイトル）にも「（不可。付ければ **E026**）」を割り当てている。資料タイトルに修飾子を付けたときに「用語に…」と出るのは誤り
- **裁定と解決**: 文言を種別に触れない形にする（例: ja「`この参照に修飾子は付けられません`」/ en「`Modifiers are not allowed here`」）か、資料タイトル用に別コードを新設する。実装は §9.2 **B-63** で §6.1.1 の表を優先して E026 を出しているので、**M4 の i18n カタログ（§12.3）着手までに裁定**。**解消（2026-10-13 / 仕様 第 2.8 版）**: ユーザー裁定は「汎用の文言」。ja `この参照に修飾子は付けられません` / en `Modifiers are not allowed on this reference`、条件列は「修飾子を付けられない参照に修飾子（§6.1.1）」

### [HIST:SPEC-FIX:A-19] 2026-10-11 §6.1.3 の正規化の並び
[HIST-TAG: category=SPEC-FIX id=A-19 topics=A-19,nfkc]
- **対象箇所**: §6.1.3 の正規化の並び
- **問題**: 1 が NFKC、3 が「長音符 `ー` / `－` / `—` / `〜` を `ー` に統一」。`－`（U+FF0D）は **NFKC で ASCII の `-` になる**ので 3 の `－` は永久に発火しない（効くのは `—` と `〜` だけ）
- **裁定と解決**: 3（長音統一）を 1（NFKC）より**前**に移す。ASCII の `-` を長音にすると `M-01` のような ID が壊れるので「NFKC のあとにもう一度」ではなく**順序の入れ替え**が正解。実装は §9.2 **B-57** でその順にしてある

### [HIST:SPEC-FIX:A-20] 2026-10-11 §6.2.4 の `material_title` の EBNF
[HIST-TAG: category=SPEC-FIX id=A-20 topics=A-20,diagnostics,materials]
- **対象箇所**: §6.2.4 の `material_title` の EBNF
- **問題**: `material_title = /[^\s「」。、;]+/` は**空白を含まない**が、§6.6.1 の例は `'M0137:科学の甲子園 本戦の運営と実技課題`（タイトルに空白）、§6.2.4 の文脈例外表の例も `'M0137:取材: 甲子園本戦`。EBNF どおりだとタイトルが `科学の甲子園` で切れて **E006**（タイトル不一致）になる
- **裁定と解決**: EBNF を空白を許す形（`/[^「」。、;\n]+/`）に直す。§6.6.1 のタイトルは §5.8.2 の index.json の例と同じ文字列なので、**例ではなく EBNF 側を直す**のが妥当。実装は §9.2 **B-59** で空白を許してある

### [HIST:SPEC-FIX:A-21] 2026-10-11 付録 B.4・§6.5.4 と付録 B.3 の下線範囲
[HIST-TAG: category=SPEC-FIX id=A-21 topics=A-21,role_language,testing]
- **対象箇所**: 付録 B.4・§6.5.4 と付録 B.3 の下線範囲
- **問題**: B.4 は「**ワイルドカード（`[^\n]*?` / 語クラス）が消費した範囲を除いた部分**に下線を引く」と言い、§6.5.4 も「一致した**リテラル部分のみ**に緑の下線」と言う。ところが B.3 の表（**テストの期待値**）は `(体言)！` → `[\p{Han}…]{1,24}！$` の一致例を「`静粛に！`」全体の下線としており、**語クラスが食った部分も下線に含まれる**
- **裁定と解決**: **B.3 を正**とし、下線から除外するのは **`*` / `〜`（`[^\n]*?`）だけ**にする（語クラス `(体言)` と `…` `?` は下線に含める）。B.4 と §6.5.4 の字句を「ワイルドカード（`*` / `〜`）が食った範囲」に直す。理由: `(体言)！` で `！` だけに下線しても作家に何も伝わらない（パターンを書いた意図は「体言＋！」という塊）。実装は `role_language.rs` で B.3 の 8 行をそのまま期待値にする（M3 完了条件 1）

### [HIST:SPEC-FIX:A-22] 2026-10-11 §5.5.3 の `default-mode` の値
[HIST-TAG: category=SPEC-FIX id=A-22 topics=A-22,diagnostics,modes]
- **対象箇所**: §5.5.3 の `default-mode` の値
- **問題**: 表は値を `mode_id`（**1 つ**）としている。ところが §6.4.4 は「1 キャラ 1 節につき**最大 2 モード**（`A+B`）」を認めていて、既定モードだけ 1 つに制限する理由が書かれていない。実装（`core/resolve/modes.rs`、§9.2 **B-65**）は `A+B` を受け付け、3 つ以上は **E005** にしている
- **裁定と解決**: §5.5.3 の値を §5.1 の `mode_expr`（`mode_id , [ "+" , mode_id ]`）に直す。診断は §6.4.4 の **E005** が既にあるので新しいコードは要らない。`tests/modes.rs` の `two_modes_in_the_default_mode_are_accepted` と `three_modes_in_the_default_mode_are_e005_here` が実装側の挙動を固定している

### [HIST:SPEC-FIX:A-23] 2026-10-11 §6.5.2 の「文」と付録 B.3 の 1・2 行目
[HIST-TAG: category=SPEC-FIX id=A-23 topics=A-23,role_language,testing]
- **対象箇所**: §6.5.2 の「文」と付録 B.3 の 1・2 行目
- **問題**: §6.5.2 は「発話テキストを `。！？!?…` の直後で分割した単位（＝文）」を照合し「どちらの文末も照合対象になる」と定める。ところが B.3 の 1 行目は `*だよ！` の**不一致例**を「だよ！って何」としていて、文単位にすると先頭の「だよ！」が 1 文になって**一致してしまう**。2 行目も `…あの…*` の一致例を「__…あの…__その…」としていて、文単位にすると「…」/「あの…」/「その…」に割れて `^[…‥.。]{1,6}あの` が**一致しなくなる**
- **裁定と解決**: **B.3 を正**とする（A-21 と同じ裁定の流儀。B.3 は「テストの期待値」と明記された表で、本書 §5.4 の M3 完了条件 1）。`^` / `$` は**発話全体**（`「」` の内側）の先頭・末尾を指す。§6.5.2 の「文」の段落を「`^` / `$` は発話全体の先頭・末尾。複数の文を含む台詞でも、文末のパターンが当たるのは**台詞の末尾**だけ」に書き換える（第 2.7 版）。文単位の照合を望むなら B.3 の 1・2 行目の例を直す必要があり、実装側は `role_language.rs` の `anchored` 1 関数＋テスト 2 件の変更で切り替えられる

### [HIST:SPEC-FIX:A-24] 2026-10-13 §7 に「語クラスの正規表現が不正」のコードが無い
[HIST-TAG: category=SPEC-FIX id=A-24 topics=A-24,diagnostics,role_language]
- **対象箇所**: §7 に「語クラスの正規表現が不正」のコードが無い
- **問題**: §6.5.1 の語クラスは `project.json.lexicon.word_classes` に**正規表現で**書くので、作家が壊れた値（`[` など）を書きうる。§7 の 78 コードに対応するものが無い（W094 は ② の検索打ち切り）。実装（`core/resolve/role_language.rs`）は `Matcher::new` が `None` を返して**そのパターンだけを黙って捨てる**（§6.5.4「一致しない場合は何もしない」と §4.3-4 に従い、役割語の色が消えるだけで執筆は止まらない）
- **裁定と解決**: (1) `project.json` の検証（M5 の設定読み込み）で診断を 1 つ新設して「語クラスの正規表現が不正」と報告する、(2) ② の予約域（W080 以降 / E032 以降）を避けて §7.1 に置く、のどちらか。**黙って捨てるだけの現状は M5 までに直す**（設定の書き間違いが UI に一切出ないのは「誤りはその場で示す」に反する）。**解消（2026-10-13 / 仕様 第 2.9 版）**: ユーザー裁定は (1)「診断を追加する」。§7 に **E027**「`project.json` の `lexicon.word_classes` の正規表現が不正（§6.5.1）」を新設し（ja `語クラス {0} の正規表現が不正です` / en `Invalid regular expression for word class {0}`、`{0}` はクラス名）、`syntax::project_json` が `project.json` の値の位置に出す。マッチャはそのクラスを使うパターンだけを捨てる（§6.5.1 に追記）。コードは 80 件（§7 = 39）。§9.2 **B-88**・**B-89**

---

## 5. 個別設計判断ログ（`DECISION`: `B-1`〜`B-161`）

### [HIST:DECISION:B-1] 2026-10-11 診断の `Span` 表現
[HIST-TAG: category=DECISION id=B-1 topics=B-1]
- **選択肢・論点**: byte range / (line, col) range
- **決定と理由**: **byte range**（core は byte、UI 側で変換。§3.4 に整合）

### [HIST:DECISION:B-2] 2026-10-11 パーサのエラー回復戦略
[HIST-TAG: category=DECISION id=B-2 topics=B-2]
- **選択肢・論点**: 行単位で捨てる / 部分木を作る
- **決定と理由**: **部分木**（§4.3-4 の「壊れた文書として開く」）

### [HIST:DECISION:B-3] 2026-10-11 `Project` の保持方法
[HIST-TAG: category=DECISION id=B-3 topics=B-3]
- **選択肢・論点**: 全ファイルをメモリ常駐 / 遅延読み
- **決定と理由**: **常駐**（§18.2 の想定規模なら数 MB。単純さを取る）

### [HIST:DECISION:B-4] 2026-10-11 非同期ランタイム
[HIST-TAG: category=DECISION id=B-4 topics=B-4,web]
- **選択肢・論点**: tokio / 自前スレッドプール
- **決定と理由**: **tokio（native のみ）**。wasm は `wasm-bindgen-futures`。adapters に閉じ込める

### [HIST:DECISION:B-5] 2026-10-11 i18n カタログ形式
[HIST-TAG: category=DECISION id=B-5 topics=B-5,i18n]
- **選択肢・論点**: toml / json / fluent
- **決定と理由**: **toml**（§12.3 が `ui/i18n/*.toml` と明記）

### [HIST:DECISION:B-6] 2026-10-10 委譲レベルの粒度
[HIST-TAG: category=DECISION id=B-6 topics=B-6,agents]
- **選択肢・論点**: **全体既定＋役割ごとのみ**（タスク単位の一時上書きは持たない）
- **決定と理由**: 「この 1 件だけ」はレベル変更→実行→戻すで代用できる。状態と監査が増える

### [HIST:DECISION:B-7] 2026-10-14 委譲レベルのセッション一時上書き
[HIST-TAG: category=DECISION id=B-7 topics=B-7,agents]
- **選択肢・論点**: **持たない**（project.json への永続のみ）
- **決定と理由**: 総停止と委譲パネルでレベルはすぐ戻せる。v1.1 の候補（2026-10-14 承認確定）

### [HIST:DECISION:B-8] 2026-10-10 自律連鎖のトリガー
[HIST-TAG: category=DECISION id=B-8 topics=B-8,agents]
- **選択肢・論点**: **次タスクの起票は integrator のみ**
- **決定と理由**: 役割の境界（§15.5.2 約束 1）と整合し暴走しにくい。付録 A.8 とも整合

### [HIST:DECISION:B-9] 2026-10-10 `agents.auto_route`
[HIST-TAG: category=DECISION id=B-9 topics=B-9,agents]
- **選択肢・論点**: **削除**（仕様からもコードからも作らない）
- **決定と理由**: v1 未リリースで v0 にもキーが無いため互換不要。委譲レベルに一本化

### [HIST:DECISION:B-10] 2026-10-10 MCP 書き込みツールの役割解決
[HIST-TAG: category=DECISION id=B-10 topics=B-10,agents]
- **選択肢・論点**: **ツール引数 `role`（必須）で呼び出し役割を指定**
- **決定と理由**: 仕様の「呼び出し役割の委譲レベル」にそのまま対応。濫用はユーザーの設定次第

### [HIST:DECISION:B-11] 2026-10-10 自動適用の検証ゲート
[HIST-TAG: category=DECISION id=B-11 topics=B-11]
- **選択肢・論点**: **E 系のみ**（W は通す）
- **決定と理由**: W は壊れない警告。E だけ止めれば十分で予測可能

### [HIST:DECISION:B-12] 2026-10-10 `state: fixed` への自動書き換え時の通知
[HIST-TAG: category=DECISION id=B-12 topics=B-12]
- **選択肢・論点**: **監査ログ＋ステータスバー 1 行通知**
- **決定と理由**: 確定文章が変わったことを見逃さない。実装は数行

### [HIST:DECISION:B-13] 2026-10-10 委譲パネルの操作
[HIST-TAG: category=DECISION id=B-13 topics=B-13,agents]
- **選択肢・論点**: **`1`〜`4` でレベル変更、`Ctrl+Shift+X` で総停止、コマンドパレット（`>agent delegation` / `>agent stop-all`）でも可**
- **決定と理由**: P3（キーボードで完結）。§9.8.4 の衝突確認どおり数字キーは空いている

### [HIST:DECISION:B-14] 2026-10-10 CI の `spec` ジョブ（§18.4）の導入時期
[HIST-TAG: category=DECISION id=B-14 topics=B-14,web,cli,testing]
- **選択肢・論点**: **M11 で追加**（M0 の ci.yml は fmt / clippy / test / wasm の 4 ジョブ）
- **決定と理由**: 受け入れ基準とテストの対応は実装が揃ってからでないと形骸化する

### [HIST:DECISION:B-15] 2026-10-10 コミット粒度
[HIST-TAG: category=DECISION id=B-15 topics=B-15]
- **選択肢・論点**: **機能単位で小さくコミット**（マイルストーン単位でまとめない）
- **決定と理由**: §10.1（作業項目ごとに PR）と整合

### [HIST:DECISION:B-16] 2026-10-11 M0 の CI `wasm` ジョブの内容
[HIST-TAG: category=DECISION id=B-16 topics=B-16,web,ui]
- **選択肢・論点**: `trunk build --release` + サイズ検査（§18.4）/ `cargo check --target wasm32-unknown-unknown`
- **決定と理由**: **M0 は `cargo check`（§18.4 からの逸脱）**。M0 時点の wasm には起動口（`#[wasm_bindgen(start)]` / eframe 起動）が無く、`trunk build` は「何も描画しない成果物」を作るだけで、同じビルドは `deploy.yml` が main で既に行っているため。§18.4 の `trunk build` + サイズ検査は **GUI が起動する M7 で追加する**

### [HIST:DECISION:B-17] 2026-10-11 `core/id.rs` のエラー表現
[HIST-TAG: category=DECISION id=B-17 topics=B-17,i18n,diagnostics]
- **選択肢・論点**: `thiserror` で `Display` を実装 / 型だけの enum
- **決定と理由**: **型だけの enum**（`IdError` / `RangeError`、`Copy + Debug` のみ）。§12.2 で文言は core に置けないので `Display` を持たせず、M4 の `core/diagnostics.rs` が `RangeError::Reversed` → **E016**（quickfix「入れ替え」）へ写像する

### [HIST:DECISION:B-18] 2026-10-11 インデントした行はフィールド行か継続行か
[HIST-TAG: category=DECISION id=B-18 topics=B-18,lexer]
- **選択肢・論点**: 2 スペース以上でもフィールド行 / 継続行を優先
- **決定と理由**: **継続行を優先**（フィールド行のインデントは 0〜1 文字）。§5.0.5 の ebnf は `ws? field_key`（0〜1 文字）で、2 スペース以上は継続行のために予約されている（§5.0.5 の例）。`FieldLine.indent` は公開するので M2 が継続行の結合を実装できる

### [HIST:DECISION:B-19] 2026-10-11 §5.0.6 の日本語別名表の置き場所
[HIST-TAG: category=DECISION id=B-19 topics=B-19,i18n,testing]
- **選択肢・論点**: `ui/i18n/` / core の `const` 表＋検査の免除 / char 列で持つ
- **決定と理由**: **core の `const FIELD_KEYS` ＋ `tests/layering.rs` の狭い免除**。別名は UI 言語が `en` でも受理する（§12）のでロケール表には置けない。免除は `i18n: format-vocabulary` という doc 行を付けた `const` / `static` の**中だけ**で、関数には効かず、行の完全一致が必要で、普通の `//` コメントでは免除されない（自己テストのフィクスチャで固定）。仕様側の文言は A-5

### [HIST:DECISION:B-20] 2026-10-11 §6.7 の再計算の条件
[HIST-TAG: category=DECISION id=B-20 topics=B-20,contents]
- **選択肢・論点**: 行の種別が変わったら常に / 節番号に影響する種別のときだけ
- **決定と理由**: **影響する種別（`@` と `;`）のときだけ**。本文行がコメントになっても節番号は動かないので、再計算を起こさない方が §6.7 の狙い（編集行だけ触る）に近い。`Renumber` が「何を」返し、**いつ**回すか（300ms / 5 秒のアイドル）は時刻を持てない core（§3.2）ではなく UI / アダプタが決める

### [HIST:DECISION:B-21] 2026-10-14 継続行のインデントの数え方（§5.0.5）
[HIST-TAG: category=DECISION id=B-21 topics=B-21,lexer,plot_outline]
- **選択肢・論点**: 文字数のみ / **2 文字以上の空白、または 1 文字以上のタブ**
- **決定と理由**: 当初は文字数のみ（タブ 1 文字は 2 未満で継続行にならない）としていたが、ユーザー裁定（2026-10-14）により**タブ 1 文字のインデントも継続行として認識する**よう改定（仕様 第 2.17 版 §5.0.5）。`lexer::is_continuation_indent` は行頭空白が 2 文字以上、または `\t` を 1 文字以上含むときに `true` を返し、A / B / MD 形式の `plot_outline::drop_indent` は先頭が `\t` なら 1 タブを、そうでなければ 2 文字を落とす

### [HIST:DECISION:B-22] 2026-10-11 空文書の保存
[HIST-TAG: category=DECISION id=B-22 topics=B-22]
- **選択肢・論点**: 改行 1 個を書く / 空のまま
- **決定と理由**: **空のまま**。§5.0.4 の「末尾は必ず改行 1 個」を空文書に適用すると、読み込み → 保存の往復で「空の 1 行」が増えて文書が変わってしまう

### [HIST:DECISION:B-23] 2026-10-11 行頭シジル表の形
[HIST-TAG: category=DECISION id=B-23 topics=B-23,lexer]
- **選択肢・論点**: `&[(Sigil, Prefix)]`（§5.2 作業項目 4）/ `(prefix, kind, requires_space)` のタプル（§7 縫い代 4）/ 名前付きフィールドの構造体
- **決定と理由**: **`&[SigilSpec]`**（`prefix` / `sigil` / `requires_space` / `whole_line` / `formats` / `stage`）。タプルでは §5.0.2 の「直後に半角または全角の空白が必要」「行全体が `;` のときのみ」「出現場所（形式）」「① / ②」を載せられない。配列の**並び順そのもの**が優先順位で、順序は字句器側に書いていない

### [HIST:DECISION:B-24] 2026-10-11 半角コロンの「賢い判定」（A-6 の裁定 ②）の作り方
[HIST-TAG: category=DECISION id=B-24 topics=B-24,lexer,diagnostics]
- **選択肢・論点**: (a) URL scheme の**除外リスト**（`http:` `mailto:` …）を持ってフィールド化を抑える / (b) §5.0.6 のキー表を**許可リスト**として使い、表に無いキーは `:` の直後に空白を要求する
- **決定と理由**: **(b)**。(a) は漏れ続ける（`doi:` `isbn:` `tel:` `urn:` `gemini:` …）うえ、表に無い ascii キー（`foo:bar`）を W011 の noise に変えてしまう。(b) なら「表に無いキーは空白が要る」だけで URL・DOI・時刻（`10:30`）・Windows パス（`C:\Users\x`）・`git@host:path` が自動的に通常テキストになり、**全角コロンは無条件でフィールド行**（日本語の自然な書き方）にできる。代償は「§5.0.6 のキーを URL scheme と衝突させられない」ことで、これは仕様の MUST＋`tests/lexer.rs` の `no_field_key_collides_with_a_url_scheme`（44 scheme）で固定した。あわせて §5.0.2 の `//` が URL を壊さないよう「直前が `:` の `//` は行内コメントにしない」を追加（`find_line_comment`）

### [HIST:DECISION:B-25] 2026-10-14 ベンチマークと性能 CI（ユーザー裁定 ④）
[HIST-TAG: category=DECISION id=B-25 topics=B-25,role_language,dictionary,export,web,benchmark]
- **選択肢・論点**: 今すぐ Criterion を入れる / 予定だけ書いて後回し
- **決定と理由**: **M11（2026-10-14）で `criterion 0.5.1`（`default-features = false, features = ["cargo_bench_support"]`）を導入し `benches/core.rs` と CI `bench` ジョブを追加**。手順どおり (1) `SPECIFICATIONS.md` §3.2 の許可クレート表に `criterion`（dev-dependencies）を先行追加、(2) `benches/core.rs` に §18.1 の `core` 性能目標 5 本（`single_line_relex` 実測 462ns / `role_language_10_patterns` 実測 1.89µs / `dictionary_trie_5000` 実測 1.14ms / `reference_pane_rebuild` 実測 38.7µs / `export_100k_chars` 実測 7.37ms）を実装、(3) `.github/workflows/ci.yml` に `bench` ジョブ（`cargo bench --locked --no-run` ＋ `cargo bench --locked --bench core -- --quick`）と `wasm` ジョブの `trunk build --release` ＋ gzip 後 wasm サイズ 8MiB 上限検査（§18.1 / §18.4）を追加、(4) `scripts/bootstrap-toolchain.sh` で `criterion` 系の 19 クレートを git blob から展開する仕組みを追加した

### [HIST:DECISION:B-26] 2026-10-11 診断カタログの「仕様書に書いていない属性」
[HIST-TAG: category=DECISION id=B-26 topics=B-26,i18n,diagnostics]
- **選択肢・論点**: 明記のある属性だけ持つ（表に穴を残す）/ 機械的な規則で全コードを埋める
- **決定と理由**: **規則で全部埋める**。穴を残すと M4 で同じ判断をもう一度することになる。明記があるのは §7.1 の全 14 件（表の見出しが「文書位置を持たない通知」）と §7.2 の E032〜E036（同節の注「エラーだが文書位置を持たない」）だけなので、残りは「文書内の byte range を指せるか」で決めた: 接続・保存・検索・件数・プロジェクト全体の通知（E064・W040・W041・W094・W095・I050・I051・I054〜I058）を `Anchor::Status` にした（計 31 件）。トグルは §7.2 の 16 件だけが対象で、仕様書が名指しする有効 9 件・無効 7 件に加え、名指しの無い **W085・W086 を W084 と同じタイミング・範囲の同族 lint として有効側**に寄せた（計 11 件）。`tests/diagnostics_catalog.rs` は**明記のある分だけ**を仕様書から導いて照合し、追加分は「`TOGGLE_ON.len() == 名指し 9 + 2`」の形で固定している

### [HIST:DECISION:B-27] 2026-10-11 §7.2 のコードを ① で enum に入れるか
[HIST-TAG: category=DECISION id=B-27 topics=B-27,diagnostics]
- **選択肢・論点**: §3 I5「§7 の表を**全件**先に定義し、未使用警告を許容する」に従う / §5.5 M4 作業項目 1「**W080 以降・E032 以降・E064・I054 以降は定義しない**（② の予約、本書 §1.5-3）」に従う
- **決定と理由**: **全件定義する（I5 を優先）**。I5 の狙いは「後から足すと番号が散らかる」ことで、§7.2 の番号はすでに仕様書で確定している（② が振り直す余地はない）。「② の予約」は「① で**検査を実装しない**」ことだと読み、コードの定義だけ済ませた（未使用 variant の警告は I5 が明示的に許容している）。**78 件 = §7 の 37 + §7.1 の 14 + §7.2 の 27**（E025 は第 2.6 版で §7 に追加。§9.1 A-17 解消、§9.2 B-51）。§5.5 M4 の作業項目 1 はこの判断に合わせて書き換えた

### [HIST:DECISION:B-28] 2026-10-11 `Diagnostic` の形
[HIST-TAG: category=DECISION id=B-28 topics=B-28,i18n,diagnostics,testing]
- **選択肢・論点**: §5.5 M4 作業項目 1 のスケッチどおり `{ code, range, args: Vec<String> }` / `{ code, args: Vec<Arg>, range: Option<Range<usize>> }`
- **決定と理由**: **`Vec<Arg>` + `Option<Range<usize>>`**（`Arg` は `Text(String)` と `Number(i64)` の 2 種）。`Vec<String>` だと字数（W019 の `{0} 字`）や比率（W090 の `{0}%`）・偏差値（W091）を core が文字列化することになり、単位や桁数を UI が言語ごとに組み直せない（en は `{0} chars`）。`range` を `Option` にしたのは `Anchor::Status` の診断に文書位置が無いから（§7.1）。**文言を持たない**のは §7.2 の MUST どおりで、`DiagnosticCode::as_str()` が返すコード文字列を `ui/i18n/*` のキー `diag.<コード>`（§12.3）に使う。JSON 化（ゴールデンテストの `.expected.json` に出す形）は M2 で決める

### [HIST:DECISION:B-29] 2026-10-11 `core/model/` のファイル配置
[HIST-TAG: category=DECISION id=B-29 topics=B-29,characters,ideas,modes]
- **選択肢・論点**: 型ごとに 1 ファイル / 形式ごとにまとめる / 全部 `mod.rs`
- **決定と理由**: **型ごとに 1 ファイル**。§3.3 が `model/` に並べた 10 型をそのままファイル名にする（`project.rs` / `document.rs` / `character.rs` / `mode.rs` / `role_pattern.rs` / `word.rs` / `material.rs` / `idea.rs` / `task.rs` / `delegation.rs`）。`mod.rs` には `pub mod` と `pub use` だけを置き、`core::model::Project` で引けるようにする。§3.3 は `syntax/` と `resolve/` にはファイル名を書いているが `model/` には型名しか書いていないので、ここは実装者の判断。**依存の向きは `syntax/<形式>.rs` → `model` の 1 本だけ**（`model` は `syntax` を `use` しない）

### [HIST:DECISION:B-30] 2026-10-11 `model` の型の serde と未知キー温存
[HIST-TAG: category=DECISION id=B-30 topics=B-30,i18n,materials,modes,testing]
- **選択肢・論点**: `Serialize` だけ / `Serialize` + `Deserialize` / 手書きパーサ
- **決定と理由**: **両方付ける**（§3.2 は serde / serde_json を許可済み）。`materials/index.json`（§5.8.2）・`project.json`（§4.5）・`.novnow/ui.json` は JSON そのものがストレージ形式なので `Deserialize` が要り、ゴールデンテストの `.expected.json` 比較に `Serialize` が要る。**未知キー温存**（本書 §1.5-2、MUST。§5.3 完了条件 2 のバイト一致はこれでしか満たせない）は `#[serde(flatten)] unknown: BTreeMap<String, serde_json::Value>` で行う。`deny_unknown_fields` は**使わない**。`rename_all` も付けず、フィールド名は仕様書のキー名にそのまま合わせる（JSON の形が仕様書と目視で突き合わせられるように）。TOML は i18n カタログ（B-5）だけで、`model` には TOML の型が無い

### [HIST:DECISION:B-31] 2026-10-11 行頭シジルの前の空白と、C 形式のインデント
[HIST-TAG: category=DECISION id=B-31 topics=B-31,lexer,contents]
- **選択肢・論点**: 0 文字のみ（M1 の字句器の実装）/ 0〜1 文字（§5.2.1 の `ws?`）/ 無制限
- **決定と理由**: **0〜1 文字**に揃えた。§5.0.5 のフィールド行も §5.2.1 の全行も `ws?` で始まるので、字句器の `match_sigil` を「インデント 2 以上は継続行としてシジルを見ない」に直した（B-18 と同じ境目）。2 文字以上のインデントは継続行のために予約されているが、**C 形式にはフィールド行が無い**（§5.0.5 の表）ので継続行は成立せず、一字下げ・段落字下げの**正当な本文**として `Line::Text` に入れる。`;` は §5.2.1 `section_break = ws? , ";" , [ comment ] , eol` どおり**行末コメントを許す**（`whole_line` の判定を `strip_line_comment` の後にした）。`; /* … */` は ① では本文のまま（行内コメントの除去は M9、§17.1）

### [HIST:DECISION:B-32] 2026-10-11 場面見出しより前の行の所属
[HIST-TAG: category=DECISION id=B-32 topics=B-32,contents]
- **選択肢・論点**: 節を作る（`scene = None`）/ どの節にも属さない / 診断で拒否
- **決定と理由**: **どの節にも属さない**。§5.2.2 は「節は場面見出しの直後から始まる」としか定めておらず、§7 に対応するコードも無いので、`Contents::sections` には入れず `Contents::lines` には残す（round-trip は保たれる）。`;` も見出しより前では節を増やさない。UI（§8.1 節単位編集）は「未所属の行」として扱う

### [HIST:DECISION:B-33] 2026-10-11 `kanji_run` と `char_name` の文字種
[HIST-TAG: category=DECISION id=B-33 topics=B-33,diagnostics,contents,modes,dictionary]
- **選択肢・論点**: 仕様書に定義が無い（§5.2.1 の ebnf が使うだけ）
- **決定と理由**: **`kanji_run`** = CJK 統合漢字 U+4E00–9FFF・拡張 A U+3400–4DBF・互換漢字 U+F900–FAFF ＋ 繰返し字 `々` U+3005・`〻` U+303B（`contents.rs` の `KANJI` / `KANJI_MARKS`）。**`char_name`** = 空白（§5.0.4 の 3 種）を含まない 1 字以上。名前に `:` が含まれると `mode_expr` と区別できないので話者注釈にしない（W002）。どちらも M3 の辞書（§5.5）が実体を握るので、辞書照合が入るときに見直す

### [HIST:DECISION:B-34] 2026-10-11 空白だけの行
[HIST-TAG: category=DECISION id=B-34 topics=B-34]
- **選択肢・論点**: `blank = ws? , eol`（字義には空白 0〜1 文字）/ 空白の文字数に関わらず空行
- **決定と理由**: **文字数に関わらず空行**。§5.2.2「節の末尾の空行は節の内容に含めない」を 2 スペースの空白行で外すと、保存時の末尾空白削除（§5.0.4 MUST）の前後で節の内容が変わってしまう

### [HIST:DECISION:B-35] 2026-10-11 空の節（W012）が指す行と、ファイル末尾の節
[HIST-TAG: category=DECISION id=B-35 topics=B-35,diagnostics,contents]
- **選択肢・論点**: どの行を指すか／末尾の区切りの後ろに節を作るか
- **決定と理由**: W012 は**節を閉じた行**（次の `;` か次の見出し）を指す。quickfix「削除」で消せる行にするため。ファイル末尾まで閉じられていないときは**節を開いた行**を指す（末尾の空行を指すと幅 0 の範囲になる）。**最後の区切りの後ろに本文が無ければ節を作らない**ので、`;` や改行で終わるファイルに空の節と W012 は増えない（§5.2.2 の W012 は「`;` が連続した場合」＝区切りに挟まれた空の節だけに出る）

### [HIST:DECISION:B-36] 2026-10-11 フィールド値の行末コメント
[HIST-TAG: category=DECISION id=B-36 topics=B-36,modes]
- **選択肢・論点**: `//` 以降を値から落とす / 値の一部として残す
- **決定と理由**: **値の一部として残す**。§5.0.5 の 7 段の手順に行末コメントの除去が無く、§5.0.2 の `//` は「行頭」か「行内」のコメントを定義するだけでフィールド値の切り詰めを定めていない。落とすと round-trip（§5.3 完了条件 2）が壊れるので、`value` は行末まで（`mode: 'アルス:A // 戦闘` の値は `'アルス:A // 戦闘`）。行内コメントの除去は M9（§17.1）

### [HIST:DECISION:B-37] 2026-10-11 継続行の値への寄与
[HIST-TAG: category=DECISION id=B-37 topics=B-37,lexer,modes]
- **選択肢・論点**: インデントを全部落とす / 2 文字だけ落とす / `raw` をそのまま連結
- **決定と理由**: **インデントから 2 文字（`CONTINUATION_INDENT`）だけ落として LF 連結**。§5.0.5 の「継続行は 2 スペース以上のインデント」の 2 文字は「値の続き」を示す印で、それを超える字下げは**内容**。§5.3.6 の `space-mode: ascii` は「等幅・折り返しなし・**整形なし**」の MUST なので ASCII 図の相対的な字下げを保たねばならず、全部落とすと図が壊れ、`raw` をそのまま連結すると 2 文字の印が値に混ざる。直前がフィールド行でなければ継続行はどのフィールドにも属さない（`Line::Continuation` のまま残るので round-trip は保たれる。自由記述にはしない）。空行・コメント・自由記述・見出しは継続を中断する

### [HIST:DECISION:B-38] 2026-10-11 ブロックの外にあるフィールドと自由記述
[HIST-TAG: category=DECISION id=B-38 topics=B-38,diagnostics]
- **選択肢・論点**: 前文に含める / 直前のブロックに含める / どのブロックにも含めない
- **決定と理由**: **どのブロックにも含めない**（C 形式の B-32 と同じ流儀）。§5.3.1 の EBNF には `block_end` のあと・次の `outline_header` の前にフィールドを置く場所が無く、§7 に対応コードも無い。行は残るので round-trip は保たれ、**W011 / W019 はブロックの外でも出す**（キーと値の形は場所に依らない）。W019 の字数は継続行まで含めた `Field::full_value`（**改行も 1 字**）で数え、範囲は 1 行目の値。`summary` の有無（E018）は**ブロック単位**なので前文とブロック外は要求されない

### [HIST:DECISION:B-39] 2026-10-11 `state` / `length` の値が形に合わないとき
[HIST-TAG: category=DECISION id=B-39 topics=B-39,modes]
- **選択肢・論点**: 診断を出す / `None` にして原文を残す
- **決定と理由**: **`None` にして原文を残す**。§5.3.2 の表は `state`（`draft` / `review` / `fixed`）と `length`（整数）の形を書くが、§7 に対応するコードが無い（コードの新設は仕様更新が先、MUST）。`Field::number()` が `None` を返すだけで、値は `value` に原文のまま残る。`state` の enum 化は検査器（M4）か `core/model/` 側で行う

### [HIST:DECISION:B-40] 2026-10-11 `.txt` 3 形式の共通ヘルパの置き場所
[HIST-TAG: category=DECISION id=B-40 topics=B-40,lexer,plot_outline,plot_episode,contents,modes]
- **選択肢・論点**: `text/lexer.rs` に移す / `syntax/mod.rs` に新設 / C 形式の `syntax/contents.rs` のものを `pub(crate)` で共有
- **決定と理由**: **当面は `contents.rs` のものを `pub(crate)` で共有**（`body_start` / `find_comment` / `is_char_name` / `is_mode_id` / `split_modes` / `parse_break` / `parse_comment` と `Comment` / `BreakLine` / `CommentLine` 型）。同じ規則を 2 箇所に書かないことを優先した。A 形式が 2 番目の消費者なので今はこれで足りるが、**3 番目（B 形式）が来たら `syntax/mod.rs` か `text/lexer.rs` に移す**（`contents.rs` は C 形式のパーサなので、共通部の置き場所としては名前が嘘になる）。`pub(crate)` のまま移すので外部 API は変わらない。**（2026-10-11 更新）B 形式が 3 番目の消費者になったが、移設はしなかった**: §3.3 のファイルツリーに `syntax/` の共通ヘルパ用ファイルが無く、**新しいファイルは仕様更新が先**（§3.2 の MUST）だから。代わりに (a) 見出し行まわり（`parse_scene` / `body_start` / `find_comment` / `is_char_name` / `is_mode_id` / `MODE_JOIN` / `parse_break` / `parse_comment` と `SceneLine` / `BreakLine` / `CommentLine` / `Comment` 型）は `contents.rs` のまま `pub(crate)` で共有、(b) フィールド行まわり（`Field` / `Continuation` / `CharRef` / `parse_field` / `drop_indent` / `warn_unknown_key`）は `plot_outline.rs` が持ち、`plot_episode.rs` が `pub use` で**同じ名前**で公開する（利用側は実体が A 形式にあることを知らなくてよい）。**名前が実態に合わないのは承知の上**で、§3.3 に `syntax/common.rs`（仮）を足す仕様更新があれば (a)(b) とも移す（`pub(crate)` のまま移すので外部 API は変わらない）。B 形式が独自に持つのは `Line` / `attach_continuations` / `content_span` の 3 つだけ（行の型が形式ごとに違うのでループは共有できない。規則そのものは `drop_indent` と §5.2.2 の末尾空行除去で共有している）

### [HIST:DECISION:B-41] 2026-10-11 B 形式のフィールドのスコープ（場面レベル／節レベル）の境目
[HIST-TAG: category=DECISION id=B-41 topics=B-41]
- **選択肢・論点**: 最初の `;` で切る / 最初の `point_line` で切る / §5.4.2 の「適用」列でキーごとに決める
- **決定と理由**: **最初の `point_line`（1 行も無ければ最初の `;`）で切る**。§5.4.1 の EBNF が `scene_block = scene_header , { scene_field

### [HIST:DECISION:B-42] 2026-10-14 MD 方言（`characters.md` / `words.md` / `ideas.md` / `prompts/*.md`）の共通部の置き場所
[HIST-TAG: category=DECISION id=B-42 topics=B-42,lexer,plot_outline,contents,characters,words]
- **選択肢・論点**: `text/lexer.rs` に移す / `syntax/mod.rs` に新設 / `characters_md.rs` のものを `pub(crate)`・`pub use` で共有
- **決定と理由**: **`characters_md.rs` が正本**で、行の分類（`Line` / `Heading` / `Bullet` / `PatternLine` / `Pattern` / `parse_line` / `attach_continuations` / `line_offsets`）と `range` の検査（`range_value` / `check_range` / `all_range`）・`unknown_field`・`section_key_of` を `pub(crate)` で、型は `pub use` で共有する（`words_md.rs` / `ideas_md.rs` / `prompt_md.rs`）。§5.6 が「`characters.md` と同一の MD 方言を使う」と明記しているので**最初に実装する MD 形式が正本**になるのが自然で、B-40（`.txt` 側は `contents.rs` が正本）と同じ判断。§3.3 のツリーに `syntax/common.rs` が無いので**移設はしない**（新しいファイルは仕様更新が先、§3.2 の MUST、2026-10-14 承認確定）。フィールド行まわり（`Field` / `Continuation` / `parse_field` / `drop_indent` / `warn_unknown_key` / `CharRef`）は A 形式の `plot_outline.rs` が正本のまま（MD も §5.0.5 の同じ規則を使う）

### [HIST:DECISION:B-43] 2026-10-11 MD 方言で**入れ子の外**にある見出しと、モード内の箇条書きを木に載せるか
[HIST-TAG: category=DECISION id=B-43 topics=B-43,modes]
- **選択肢・論点**: 全部載せる / 入れ子が正しいものだけ載せる / 行だけ残して木に載せない
- **決定と理由**: **入れ子が正しいものだけ木に載せ、それ以外は行だけ残す**（`lines` にあるので round-trip は保たれ、`Lines` の分類はそのまま見える）。H1 の外にある H2/H3、セクションの外にある H3、`#### ` 以上の深い見出し（§5.5.1 が数えない階層）、モード内にある箇条書き（§5.5.2 の `section_body` にしか現れない）が該当する。**診断は出さない**: §7 に「入れ子が間違っている」コードが無く、§5.5.1 は「それ以外は自由記述として保持」としか言わないので、勝手に E を新設しない（コードの新設は仕様更新が先、MUST）。深い見出しと `#名前`（空白なし）は自由記述として木に載る

### [HIST:DECISION:B-44] 2026-10-11 `range:` の検査（E002 / E016 / E017）の順序と、比較に使う範囲
[HIST-TAG: category=DECISION id=B-44 topics=B-44,diagnostics,words,modes]
- **選択肢・論点**: 行順に全部 / コンテナ順に E002・E016 → 最後に E017 / 書いた順に混在
- **決定と理由**: **行順の W011 → キャラクタ（用語）ごとのモード順に E002 / E016 → 最後に E017**。E017 は「同じ ID の前の定義全部」と比べるので 1 回の走査では出せず、A 形式の E018（行順のあとにブロック順）と同じ流儀で後から並べる。`range:` の値が**範囲式として読めない**（E002）ときは `None` を返して**比較しない**（読めない範囲と交差を言っても作者に行動できない）。**左端 > 右端**（E016）のときは `RangeExpr` ができているので**比較には使う**（`Interval::intersects` は `start <= other.end && other.start <= end` なので逆順の区間でも交差を言う。作者は E016 を先に直す）。`range:` だけの空の値は `Err(RangeError::Empty)` を「範囲を書かなかった」と読んで**全範囲**（§5.5.4「`range` 省略時は全範囲」）にし、診断しない

### [HIST:DECISION:B-45] 2026-10-11 `materials/index.json` の診断（E024 / E008）の byte range の決め方
[HIST-TAG: category=DECISION id=B-45 topics=B-45,diagnostics,materials]
- **選択肢・論点**: serde に位置を持たせる（`serde_path_to_error` 等の新クレート） / `Value` の `span` 機能を使う / 値のリテラルを原文から検索する
- **決定と理由**: **値の JSON リテラルを原文から検索する**（`literal_ranges`）。新クレートは足せない（§3.2 MUST）し、`serde_json` の位置情報なしの `Value` は byte 位置を持たない。検索は「`"path"` の後ろの文字列リテラル」のように**キー名と値の両方**を鍵にするので、同じ値が 2 箇所にあっても最初の未使用のものを採る（`Index::raw` は原文のままなので、位置がずれても round-trip は壊れない）。診断の位置は §7 の「文書位置」列（E024＝`path` の値、E008＝2 件目の `id` の値）に合わせた

### [HIST:DECISION:B-46] 2026-10-11 閉じた集合の値（`newline` / `paragraph_break` / `status` / `delegation` / `kind` 等）を serde の enum にするか
[HIST-TAG: category=DECISION id=B-46 topics=B-46,materials]
- **選択肢・論点**: serde の enum（`#[serde(rename_all)]`） / String ＋ `*_kind()`・`parse()` / `#[serde(other)]` 付き enum
- **決定と理由**: **String で受けて `*_kind()`（enum への変換）と `parse()`（文字列からの生成）を別に持つ**。serde の enum にすると**未知値 1 つでファイル全体がデシリアライズできなくなり**、§4.3-4「壊れていても開ける。既定値で続行する（MUST）」と §1.5-1 の未知キー温存に反する（② が値を足した瞬間に ① のプロジェクトが開けなくなる）。`#[serde(other)]` は unit variant にしか付けられないので入れ子のある値には使えない。enum 自体は `MaterialStatus` / `SourceKind` / `ConditionKind` / `Delegation` / `Newline` 等として持ち、**ドメイン層は enum で書く**（文字列比較を散らさない）

### [HIST:DECISION:B-47] 2026-10-11 `prompts/*.md` の `{{variable}}` の走査範囲と位置
[HIST-TAG: category=DECISION id=B-47 topics=B-47,diagnostics,contents,agents,testing]
- **選択肢・論点**: フィールド値とセクション本文だけ / 行コメントと空行を除く全行 / H1 より後ろだけ
- **決定と理由**: **行コメント（`//`）と空行を除く全行**を走査する。LLM に渡らない部分（コメント）の変数は実行時に展開されないので診断しても行動できず、空行には変数が無い。`# ` の行コメントは ②（§5.0.8）なので ① では**走査対象に残る**（B-36 と同じ理由）。`Variable::at` / `range` は**行内相対**（`Field::value_at` と同じ流儀）で、診断の byte range だけは `line_offsets` を足して絶対位置にする（最初は両方に足して 17 byte ずれるバグを出したので、テストで絶対位置を固定した）。E032 / E033 は `Options::custom = true` のときだけ（既定 7 役割の `prompts/<role>.md` は**上書きが正当**なので同名でも E033 にしない。§5.9.2「上書きは `prompts/<role>.md` 側で行う」）

### [HIST:DECISION:B-48] 2026-10-11 ゴールデンテストの `.expected.json` の `tree` の形
[HIST-TAG: category=DECISION id=B-48 topics=B-48,diagnostics,role_language,testing]
- **選択肢・論点**: 木を serde でそのままダンプ / 形式ごとの射影 / 短いタグ文字列（unit テストと同じ）
- **決定と理由**: **形式ごとの射影**。serde ダンプは行の `raw` と借用スライス全部が入って**人が読めず**、仕様書と突き合わせられない（§6.3「自動生成した期待値は仕様の検証にならない」）。unit テストのタグ文字列は 1 行に詰まっていて差分が読みにくい。射影は「コンテナは `span` の数値、フィールドは**キー文字列の列**（未知キーは `{"unknown": "…"}` で区別）、free / bullets / notes は**件数**、役割語パターンはパターン本体」としたので、**行番号を数える手作業を最小化**しつつ §5.3 完了条件 1 の「構造が期待値と一致する」を目視で検証できる。round-trip（完了条件 2）は JSON には入れず**ハーネスが全ケースで機械的に assert** する（期待値に書くとかえって壊れやすい）。ゴールデンは**既定の `Options`** で読む（`project.json` 由来の設定で変わる診断 E020 / E032 / E033 は unit テストが見る。§6.3 のケースは 1 ファイル 1 入力なので設定を足す仕組みは持たない）

### [HIST:DECISION:B-49] 2026-10-11 `core/model/mod.rs` の再公開の書き方
[HIST-TAG: category=DECISION id=B-49 topics=B-49,modes,testing]
- **選択肢・論点**: `pub use project::Project;` / `pub use self::project::Project;`
- **決定と理由**: **`self::` を付ける**。`tests/layering.rs` の `core_has_no_forbidden_dependency` は `use` / `pub use` の**先頭セグメント**が `ALLOWED_ROOTS`（`std` `core` `alloc` `crate` `self` `super` と許可クレート）にあるかを見るので、同一クレート内のモジュール名を直接書くと落ちる（B-19 の検査器の自己テストが拾った）。`crate::` でも通るが、`mod.rs` の中では `self::` が一番短い

### [HIST:DECISION:B-50] 2026-10-11 第 2.6 版で §5.0.6 の表に足す 10 キーの**日本語別名**の決め方（別名は双方向 1:1 の固定表、MUST）
[HIST-TAG: category=DECISION id=B-50 topics=B-50,lexer,modes]
- **選択肢・論点**: キー名の直訳を機械的に当てる / 既存の別名と衝突しないよう意味で振り分ける / 別名を付けずに正準形だけ受理する
- **決定と理由**: **既存の 46 行と衝突しないよう意味で振り分けた**。`state`＝「状態」が既にあるので `status` は「**状況**」（§5.7 の inbox / keep / promoted / dropped は「状態」ではなく進捗の状況）にし、`created` は `source`＝「出典」と被らないよう「**作成日時**」、`promoted-to` は「**昇格先**」、`model-hint` / `max-chars` / `temperature` / `label` は「**モデル指定** / **最大文字数** / **温度** / **表示名**」とした。「別名を付けない」案は §5.0.6 が「表は**固定・双方向 1:1**」と定め、日本語で書けることが UI 言語 `en` でも受理される要件（§12.2 の字句データ、A-5）なので採れない。衝突の検査は `tests/lexer.rs` の `the_alias_table_is_one_to_one_in_both_directions`（キー集合と別名集合が**互いに素**であることも見る）が機械的に担保する

### [HIST:DECISION:B-51] 2026-10-11 E025（JSON がパースできない）の**番号**と位置・引数
[HIST-TAG: category=DECISION id=B-51 topics=B-51,i18n,diagnostics,materials]
- **選択肢・論点**: §7.1 に置く（E064 以降の空き）/ §7 の E024 の隣 / 既存コードの流用
- **決定と理由**: **§7 の E024 の隣（E025）**。E024 が「`materials/index.json` の `path`」で、E025 は同じファイルの**パース失敗**なので隣が最も読める。空き番号の確認は全 docs / src を grep して **E009–E011 / E014 / E015 / E019 / E022 / E023 / E025 / E027 / E029 がどこにも出現しない**ことを確かめたうえで行い、② の予約域（**W080 以降 / E032 以降 / E064 / I054 以降**、本書 §1.5-3）には触れていない。位置は**ファイル全体**（`0..raw.len()`）: serde はトークン位置を持たないので「どのキーが壊れたか」は言えず、§7.1 の「文書位置を持たない通知」にするよりは**ファイルを開いたまま直せる**ほうが作者の行動に合う（Anchor は `STATUS_ANCHOR` 外なので自動的に `Document`）。引数は **1 個**（`{0}` ＝パーサのメッセージ）で、ja / en の両方の文言が `{0}` を使うので `tests/diagnostics_catalog.rs` の引数検査をそのまま通る。`Group::Core`（Error 1..=31）も既存の規則で自動的に正しくなる

### [HIST:DECISION:B-52] 2026-10-11 仕様の例を直したあと、**消えた診断の網羅**をどう残すか
[HIST-TAG: category=DECISION id=B-52 topics=B-52,diagnostics,agents,testing]
- **選択肢・論点**: 例が直ったので検査も消す / 合成入力で検査だけ残す / ゴールデンに壊れたケースを足す
- **決定と理由**: **合成入力で検査だけ残す**。§5.9.2 の例が `{{text_current}}` になったことで E031 のゴールデン網羅が消えるので、`tests/prompt_md.rs` の `an_unknown_variable_reports_e031` に `{{content_section}}` のケースを追加した（**A-16 で直した綴りそのもの**を「閉集合に無い名前」の例として残すので、回帰が起きても気づける）。同様に W011 は §5.5.7 / §5.7 / §5.9.1 / §5.9.2 の例から消えたので、② のキー（`blurb` `icon` `address`）と `foo` を使う unit テストを 4 形式ぶん残した。「ゴールデンに壊れたケースを足す」案は §6.3 の「入力は**仕様書のコード例をそのまま**」に反するので採らない（壊れた入力のゴールデン 3 件は仕様書の例ではないが、いずれも M2 で既存のものを固定した範囲）

### [HIST:DECISION:B-53] 2026-10-11 E025 の**ゴールデンケース**を足すか
[HIST-TAG: category=DECISION id=B-53 topics=B-53,diagnostics,materials,testing]
- **選択肢・論点**: `materials_index/broken_json.txt` を足す / unit テストだけにしておく
- **決定と理由**: **unit テストだけ**。E025 の引数 `{0}` は serde_json のエラーメッセージ（`trailing comma at line 6 column 3` のように**行・列を含む**）なので、`.expected.json` に書くと `serde_json` を更新しただけでゴールデンが落ちる（仕様の変更でない理由で赤くなる）。`Index::error` も同じメッセージを写像するので同様。§6.3 の「期待値は仕様と突き合わせられるもの」を守るため、E025 は `tests/materials_index.rs` の 2 テスト（コード・位置・引数の数・`contains("expected value")`・既定値での続行・原文の不変）で固定する

### [HIST:DECISION:B-54] 2026-10-11 GUI のクレート（egui / eframe）を既定のビルドに入れるか
[HIST-TAG: category=DECISION id=B-54 topics=B-54,i18n,agents,export,storage_policy,web]
- **選択肢・論点**: 常にビルドする（`Cargo.toml` のまま）/ **`ui` feature にして既定は空** / `default = ["ui"]` にして CI だけ `--no-default-features`
- **決定と理由**: **`ui` feature にして `default = []`**。M7 まで `src/ui/**` は 1 行の stub ばかり（54 行）で egui を一切使わないのに、egui / eframe は依存ツリーが 200 クレート規模あるので、CI の clippy / test / wasm の 3 ジョブが毎回それをビルドしていた（`Swatinem/rust-cache` が効くのは `Cargo.lock` が変わらない間だけ）。ユーザーの「Actions 使用時間をできるだけ抑える」指示にも沿う。対象は §2.1 の表で層が **ui** の 5 クレート（`eframe` / `egui` / `rfd` / `fontdb` / `toml`）で、`zip` / `web-time` / `tokio` / `directories` / wasm 系は **ui ではない**（export・clock・storage・MCP のアダプタが使う）ので既定のまま残した。`default = ["ui"]` ＋ CI だけ `--no-default-features` の案は、手元の `cargo test` が毎回 egui をビルドすることになって開発ループが遅いので採らない（CI の時間だけを問題にすればどちらでもよいが、`cargo build` で GUI が要るのは M7 以降で、そのときは `--features ui` を 1 回付けるだけ）。**回帰の防止**: `tests/layering.rs` の末尾 2 テストが `Cargo.toml`（5 クレートが `optional = true` で `[features] ui` の `dep:` に並ぶ／`default` に `ui` が無い）と `src/lib.rs`・`src/adapters/mod.rs` の `#[cfg(feature = "ui")]` を `include_str!` で読む。`toml` は optional になるとテストからも使えなくなるので **`[dev-dependencies]` にも置いた**（検査のために GUI をビルドしない）。**M7 の作業項目 0** で CI の 3 コマンドに `--features ui` を足す **改（2026-10-13、M6、B-100）**: `toml` は CLI も使う文言カタログのために通常の依存にした（feature の後ろは 4 クレート）。`src/lib.rs` の `pub mod ui;` は cfg なしで、`src/ui/mod.rs` が `i18n` 以外を守る

### [HIST:DECISION:B-55] 2026-10-11 CI の失敗出力をどう出すか
[HIST-TAG: category=DECISION id=B-55 topics=B-55,diagnostics,contents,testing]
- **選択肢・論点**: 生ログの `head` / `tail` を PR コメントに貼る（M0 の形）/ Actions のログを読む（`gh run view --log`）/ **生ログを解析して注釈・サマリ・コメントの 3 面構成にする**
- **決定と理由**: **3 面構成にする**。(1) Actions のログ取得先ホスト（`objects.githubusercontent.com`）には作業環境から到達できず `gh run view --log*` は全部失敗するので「ログを読む」案は最初から使えない（本書 §9.3）。(2) M0 の `head -n 120` / `tail -n 60` は、**テスト失敗の本体がログの真ん中にある**（`---- <name> stdout ----` ブロックは全ターゲットの実行後にまとめて出る）ため、ちょうど落ちる。コンパイルエラーも数十行の `Compiling …` に埋もれる。解析は `.github/report.py`（**標準ライブラリのみ**、runner の python3 に何も入れない）で、rustc の診断（`error[E0716]: …` ＋ `--> file:line:col` ＋ 本文 24 行）・テスト失敗（panic 位置とメッセージ 30 行）・rustfmt の差分（`Diff in <path>:<line>:` を**箇所ごとに**分け、runner の絶対パスを `GITHUB_WORKSPACE` で相対化）を拾う。cargo 自身のまとめ行（`could not compile` / `build failed` / `test failed, to rerun` 等）は**拾わない**（件数の水増しになり、情報は `test result:` の行と失敗一覧で足りる）。**解析できなかったときは生ログの先頭と末尾にフォールバックする**（解析器のバグで失敗理由が消えるのが一番悪い）。**レポート生成は合否に影響させない**（`

### [HIST:DECISION:B-56] 2026-10-11 M3 の `resolve/*` は何を入力にするか
[HIST-TAG: category=DECISION id=B-56 topics=B-56,characters,modes]
- **選択肢・論点**: `core/model` の型（`Character` / `Word` / `Mode`）だけ / `core/syntax` の木だけ / 木を正本にし、意味の塊が要るところで model の型をその場で組む
- **決定と理由**: **3 つ目**。`model` は行の位置を持たないので、それだけでは診断の波線を引く場所が決まらない（§7.2「位置は byte range」）。§6.4.3 の 8 段は C 形式の話者注釈・B 形式の節と場面の `mode:`・A 形式の `cast:`・`characters.md` の `default-mode` と**4 ファイルの木を跨ぐ**ので、位置を持つ側を入力にする。`model` を外したわけではなく、`RolePattern::anchor()`（§6.5.2 の固定表）・`Mode::range_expr()`（§5.1）・`MaterialIndex`（JSON そのものがストレージ形式、§5.8.2）は `resolve` がそのまま呼ぶ。`syntax → model` の書き起こし（`model::Character` を埋める関数）は**参照ペイン（§9.5）とストレージ（M5）が要るとき**に回す。位置の単位は M2 と同じで、ファイル単位の木は**ファイル内 byte**、行単位の型（`RefToken` / `Speaker` / `Pattern`）は**行内相対**

### [HIST:DECISION:B-57] 2026-10-11 §6.1.3 の正規化の実装順と `MAX_KEY_BYTES` の測り方
[HIST-TAG: category=DECISION id=B-57 topics=B-57,nfkc,diagnostics,dictionary]
- **選択肢・論点**: 仕様書の 1〜4 をそのまま（NFKC → 小文字 → 長音 → 空白）/ **長音統一を NFKC より先に** / NFKC の前後で 2 回長音統一する
- **決定と理由**: **長音統一を先に適用する**。仕様書の順だと `－`（U+FF0D）は NFKC で ASCII の `-` になり、3 の「`－` を `ー` に統一」が**永久に発火しない**（`—` と `〜` は NFKC で形が変わらないので効く）。ASCII の `-` を長音にしてしまうと `M-01` のような ID が壊れるので、4 文字（`ー` `－` `—` `〜`）を先に `ー` へ統一してから NFKC する。あわせて **`MAX_KEY_BYTES = 96` は正規化後の byte 数**で測る（これはトライの深さの上限なので、空白を落とした実体で測らないと §6.6.1 の空白入りタイトルを E028 で落としてしまう）。正規化は**1 文字ずつ**行う（`OffsetMap` で byte 位置を元に戻すため）。代償は結合文字の並び（半角カナ＋濁点）が合成されないことで、辞書キーにそれを書く事例は無いと判断した。性能は NFKC が恒等な文字（ASCII・かな・漢字・CJK 約物）を `String` に集めず直接 push して、5,000 エントリで **release 5.3ms**（§18.1 の目標 10ms 以内、完了条件 4）

### [HIST:DECISION:B-58] 2026-10-11 `alias:` の区切り文字（§5.5.3 は「カンマ区切り」とだけ）
[HIST-TAG: category=DECISION id=B-58 topics=B-58,nfkc]
- **選択肢・論点**: 半角 `,` だけ / `,` と読点 `、` / `,` と `、` と全角 `，`
- **決定と理由**: **3 つ全部**。日本語の別名列表で `、` を打つのは自然で、`、` は §6.2.1 の**区切り文字集合に入っている**ので、`、` を含むキーはどのみち参照できない（割らないと「参照できないキー」が黙って登録される）。全角 `，` は NFKC で半角になるが、**割るのは原文**なので明示的に入れる。空の項目（`alias: ,` や末尾カンマ）は登録しない。項目の前後の空白は除く

### [HIST:DECISION:B-59] 2026-10-11 修飾子 `:` の範囲と、半角/全角の扱い
[HIST-TAG: category=DECISION id=B-59 topics=B-59,diagnostics,contents,characters,materials,modes]
- **選択肢・論点**: §6.2.4 の EBNF どおり（`:` は半角のみ、`material_title` は `[^\s「」。、;]+`）/ 全角 `：` も修飾子の区切りにする / 種別で範囲を変える
- **決定と理由**: **半角 `:` のみ＋種別で範囲を変える**。全角 `：` を修飾子にすると本文の `'リリア：待って` が E002 になる（`：` は §6.2.1 の区切り文字集合にあるので、トークンをそこで終えるほうが正しい）。範囲は **`Character` のモード式＝§6.2.1 の区切り文字まで**（空白を含む。さもないと `'リリア:B と '{リリアナ}` の `B` が行末まで飲み込まれて 2 つ目の参照が消える）、**`Material` の資料タイトル＝空白を許す**（§6.6.1 の例 `'M0137:科学の甲子園 本戦の運営と実技課題` は EBNF の `[^\s…]` だと表現できないので、EBNF ではなく例を優先した）。`:` の後ろが空（`'リリア:`）なら修飾子ではない（本文の句読点）。モード式は**英数字・`_`・`+` だけのとき**にモード式として読み、それ以外は句読点扱いにする（`'リリア: 待って` を E002 にしないため）。`+` で割って 3 つ以上なら §6.4.4 の **E005**、形が違えば **E002**

### [HIST:DECISION:B-60] 2026-10-11 `'` の**開始条件**（仕様は書いていない。§6.2.1 の疑似コードは `'` の位置をもらうだけ）
[HIST-TAG: category=DECISION id=B-60 topics=B-60,diagnostics,contents]
- **選択肢・論点**: すべての `'` を参照とする / 直前が ASCII 英数字なら参照としない / M2 の `Field::refs()` と同じ「空白区切りトークンの先頭だけ」
- **決定と理由**: **直前が ASCII 英数字なら参照としない**＋**未解決かつ候補が ASCII 英字 2 文字以下なら黙って無視する**。英語の短縮形・所有格（`don't` / `it's` / `リリア's`）を E001 の山にしないため。「トークンの先頭だけ」は本文が日本語で空白が無いので使えない（`これは'リリア` を取りこぼす）。`'` の直後が区切り文字（`''` や `' 「`）のときもトークンを作らない。解決できた場合は 1〜2 文字でも参照として扱う（本当に短い別名を登録した作家の意図を奪わない）

### [HIST:DECISION:B-61] 2026-10-11 話者注釈 `@名前` の引き方（§6.2.5 は「辞書の `Character` を最短一致」）
[HIST-TAG: category=DECISION id=B-61 topics=B-61,diagnostics,contents,characters,dictionary]
- **選択肢・論点**: 本文と同じ最短一致スキャン / **完全一致**
- **決定と理由**: **完全一致**。名前は M2 の `contents::Speaker` が既に区切っているので（`@` の直後から `:` または注釈の終わりまで）、最短一致をかける対象が「名前の後ろの地の文」ではなく**注釈の中**しかない。`@リリアナ`（辞書は `リリア`）を最短一致で `リリア` に解決すると、残った `ナ` の行き場が無く、E012（`登場人物 リリアナ は characters.md にいません`）より悪い結果になる。種別は `Character` だけに絞る（§6.1.1「`Address` は**話者注釈 `@` の解決対象外**（MUST）。`@あおいちゃん` は E012 のまま」）

### [HIST:DECISION:B-62] 2026-10-11 辞書の索引の持ち方（§18.1「5,000 エントリで 10ms 以内」）
[HIST-TAG: category=DECISION id=B-62 topics=B-62,diagnostics,dictionary]
- **選択肢・論点**: 全キーの `BTreeMap<String, Vec<EntryId>>` ＋トライ / **トライだけ**（衝突したキーだけ別の表に持つ）
- **決定と理由**: **トライだけ**。全キーの索引は 1 件ごとに `String` を複製するので 5,000 件で 10ms を超えた（実測 11.5ms、debug）。完全一致（`lookup`）はトライを最後まで歩くだけで O(キー長) で済み、W027 の検出は「挿入時に終端が既にあったか」で分かり、「別種別の同名項目あり」（§6.1.1 の参照ペイン併記）は**衝突したキーだけ**の表があれば足りる。あわせてトライの子は push して毎回ソートするのではなく**挿入位置に insert** する（子は常に char 昇順）。同じ種別の重複（2 人の別名が同じ等）は W027 にしない（定義側の重複は **E008** として M2 のパーサが出している）で、先に書いたほうが勝つ

### [HIST:DECISION:B-63] 2026-10-11 `MaterialTitle` に修飾子を付けたときの診断
[HIST-TAG: category=DECISION id=B-63 topics=B-63,i18n,diagnostics,words,materials]
- **選択肢・論点**: §6.2.4 にならって診断しない / **E026**（§6.1.1 の表「（不可）」）
- **決定と理由**: **E026**。§6.1.1 の表は `Word` と `MaterialTitle` の両方を「（不可。付ければ **E026**）」としているが、§6.2.4 は「`Word` / `Address` のとき → E026」と 2 種しか名指ししていない。**表（§6.1.1）を優先**する。ただし資料タイトルは `:` を含みうるので（§6.2.4 の文脈例外表「資料タイトル修飾子の中の `:` はタイトルの一部」）、`'取材: 甲子園本戦` は**キーの一致が `:` を飲み込む**ため修飾子は残らない（E026 が出るのは `'M0137:…` のように ID で解決したあとの余分な修飾子だけ）。E026 の ja 文言は「用語に修飾子は付けられません」なので、資料タイトルに付いたときは文言が少しずれる（§7 の文言は M4 の i18n カタログで直す候補。A-18 として §9.1 に記録した）

### [HIST:DECISION:B-64] 2026-10-11 `mode_id` の比較で大文字小文字を区別するか（§6.4.1 は「`(mode_id, range)` の組で一意」としか言っていない）
[HIST-TAG: category=DECISION id=B-64 topics=B-64,nfkc,diagnostics,modes,dictionary]
- **選択肢・論点**: 完全一致 / **ASCII の大文字小文字を無視**
- **決定と理由**: **ASCII の大文字小文字を無視**（`definitions_with_id`）。§6.1.3 の照合が「英字は小文字化」してから比べるので、`mode: 'リリア:b` は辞書側では `b` になる。定義側（`### B`）を完全一致にすると、同じ書き手が「参照は解決できるのにモードが無い（E003）」という食い違いを見る。全角英字などの Unicode の大小は §6.1.3 の NFKC が先に吸収するので ASCII だけで足りる

### [HIST:DECISION:B-65] 2026-10-11 `mode: 'リリア`（名前だけでモードを書いていない）と `default-mode: A+B` の扱い
[HIST-TAG: category=DECISION id=B-65 topics=B-65,diagnostics,modes]
- **選択肢・論点**: 名前だけの参照でも「その段で見つかった」ことにして W006 を止める / **見つからなかった**扱いで次の段へ進む。`default-mode:` は §5.5.3 どおり `mode_id` 1 つだけ受理
- **決定と理由**: **見つからなかった扱いで次の段へ進む**。§6.4.3 の各段は「…に書かれた**モード**」を読むので、モードを書かなかった段は候補にならない（`cast:` に居るだけのキャラから段 5 のモードを作ってはいけない）。`default-mode:` は逆に **`A+B` も受け付ける**: §6.4.4 が 1 キャラ 1 節 最大 2 モードを認めていて、既定だけ 1 つに絞る理由が無い（3 つ以上は E005）。§5.5.3 の「値は `mode_id`」からは外れるので、仕様書の字句は M3 の最後に直す候補（§9.1 A-22）

### [HIST:DECISION:B-66] 2026-10-11 定義の `range:` が壊れているとき（`range: @xx ..`）の変種選択
[HIST-TAG: category=DECISION id=B-66 topics=B-66,diagnostics,characters]
- **選択肢・論点**: 全範囲扱い / どの範囲にも入らない扱い（**E004**）
- **決定と理由**: **全範囲扱い**（E004 を重ねない）。E002 / E016 は M2 の `characters_md::parse` が定義側で既に出しているので、解決層まで「範囲外」と言うと 1 つの書き間違いに 2 種類のエラーが出る。§4.3-4「パース失敗を起動失敗にしない」＝**読めないものは広いほうに倒して**執筆を止めない

### [HIST:DECISION:B-67] 2026-10-11 **E005**（1 節に 3 つ以上のモード／同じキャラを 2 回）を**どの層が出すか**と波線の範囲
[HIST-TAG: category=DECISION id=B-67 topics=B-67,diagnostics,plot_episode,contents,modes,dictionary]
- **選択肢・論点**: 構文層だけ / 解決層だけ / **形を初めて読んだ層だけ**
- **決定と理由**: **形を初めて読んだ層だけ**が出す。同じ形に 2 層が波線を引くと 1 トークンに 2 本の下線が重なる（§6.5.4 が二重線を嫌うのと同じ理由）。分担は (1) B 形式の `mode:` → `plot_episode::parse`（M2、実装・テスト済み）、(2) 1 トークンの中の 3 つ以上（`cast:` の値・本文中の参照）→ `dictionary::token_diagnostics`、(3) `cast:` に同じキャラを 2 回／`default-mode:` に 3 つ以上 → `resolve::modes`（**フィールド全体を読む層が他に無い**）。どの層が報告する場合でも **§7 のクイックフィックス「先頭 2 つを残す」は解決層が適用**するので、3 つ以上でも解決は止まらない（§4.3-4）。あわせて **E005 の波線はトークン全体**に統一した（辞書層は修飾子 `:A+B+C` だけを指していたが、`plot_episode` の M2 の実装とテストがトークン全体なのでそちらに合わせた。§7 の表は位置を規定していない）。**M4 への申し送り**: 診断を組み立てるとき、B 形式の `mode:` の値に (2) を重ねないこと

### [HIST:DECISION:B-68] 2026-10-11 付録 B.4 の下線範囲の作り方（「リテラル部分を `(...)` で囲んだ正規表現を生成する」）
[HIST-TAG: category=DECISION id=B-68 topics=B-68]
- **選択肢・論点**: アトム 1 つにつき 1 グループ / **下線に含めるアトムの連続につき 1 グループ**
- **決定と理由**: **連続につき 1 グループ**。アトムごとにグループを作ると `…あの…` が 3 本（`…` / `あの` / `…`）に割れ、B.3 の 2 行目の期待値（`…あの…` の 1 本）にならない。あわせて**重なった・隣り合った範囲は 1 本にマージする**（`union`）: 別パターンの一致が隣接していれば見た目は 1 本の下線なので、§6.5.4「二重線にしない」と揃う

### [HIST:DECISION:B-69] 2026-10-11 未知の語クラス（`(ほげ)`）と壊れた語クラス正規表現の扱い
[HIST-TAG: category=DECISION id=B-69 topics=B-69]
- **選択肢・論点**: どちらも黙って無視する / **未知はリテラル**・**壊れていればそのパターンだけ捨てる**
- **決定と理由**: 未知のクラス名は**括弧ごとリテラル**（`(ほげ)` は文字列 `(ほげ)` に一致する）。勝手に無視すると「書いたのに何も起きない」になり、作家が原因を推測できない。`project.json` の正規表現が壊れているときは `Matcher::new` が `None` を返し、**そのパターンだけ**捨てる（他は生かす。§4.3-4）。§7 に対応コードが無いので報告はできない（§9.1 **A-24**）

### [HIST:DECISION:B-70] 2026-10-11 §6.5.3 の照合対象（`「」` / `『』` の**内側だけ**）の取り出し方
[HIST-TAG: category=DECISION id=B-70 topics=B-70,role_language]
- **選択肢・論点**: M2 の C 形式に台詞ノードを足す / **`role_language::dialogue_spans` が文字列から取り出す**
- **決定と理由**: **文字列から取り出す**。M2 の C 形式は台詞をノードにしない（`body` の中の `「」` はただの文字列）ので、木を触らずに済むほうを選んだ（B-56「位置が要るところは木、意味の塊が要るところで組む」と同じ流儀）。**入れ子（`「『…』」`）は内側と外側の両方**を返す（和集合にすると内側の位置が失われる）。閉じ括弧が無ければ**行末まで**を内側とする（書きかけの台詞でも役割語が見える。原文は書き換えないので round-trip は不変）。種類の違う閉じ括弧（`「…』`）は閉じとして扱わない

### [HIST:DECISION:B-71] 2026-10-11 **E007**（未知の資料 ID）と **E001**（未解決の参照）の分担。辞書は index から作るので「ID が辞書に無い」＝「index に無い」
[HIST-TAG: category=DECISION id=B-71 topics=B-71,diagnostics,materials,modes,dictionary]
- **選択肢・論点**: 辞書が E001、`materials` が E007（両方出る）/ 形を見て**どちらか 1 つ**
- **決定と理由**: **資料 ID の形（§5.8.2 `M` / `S` ＋ 4 桁以上の数字）をした未解決トークンは E007 だけ**、それ以外は E001 のまま。両方出ると同じ `'M9999` に 2 本の波線が重なる（B-67 と同じ問題）。判定は `materials::unknown_material_id`（`model::material::MaterialId::parse` をそのまま使う）。**M4 の入口は `materials::scan_line`** にして、辞書の診断（E001 / E002 / E026 / W029）と資料の診断（E006 / E007 / W031）を**1 か所で**組む（呼び出し側が 2 つの関数を別々に呼んで重ねないようにするため）

### [HIST:DECISION:B-72] 2026-10-11 §6.6.2 のインレイヒントの文言（`⟨タイトル・提案中⟩`）と §6.6.3 のクイックフィックス 3 種を core が作るか
[HIST-TAG: category=DECISION id=B-72 topics=B-72,i18n,diagnostics]
- **選択肢・論点**: core が完成した文字列を返す / **中身だけ返して文言は UI/i18n**
- **決定と理由**: **中身だけ返す**。§12.2「core にユーザー向け文言を置かない」ため（`en` では `⟨title · proposed⟩` になる）。`Hint` は「位置・切り詰めたタイトル・`status`」だけを持ち、括弧と「提案中」「却下」の添え文言は `ui/i18n`（§12.3）が組む。**24 字 / 23 字 + `…`** の切り詰めは言語に依存しないので core が行う（`hint_title`。字は Unicode の文字数で数える）。`TitleFixes` も「index のタイトル・書かれたタイトル・別 ID」の 3 つの中身だけで、文字列は**所有する**（借用だと参照トークンと index の 2 つの生存期間を 1 本のライフタイムに縛るため）。§6.6.1 の 3 行目の「ID へ正規化するクイックフィックス」は `Resolved.normalize_to_id`（置換後のトークン全体 `'M0137`）で返す。**診断は出さない**（書き方としては正しいので、E006 と混ぜない）

### [HIST:DECISION:B-73] 2026-10-11 §15.4 の MUST「UI と LLM で**情報源と優先順位を一致させる**」をどう機械的に担保するか
[HIST-TAG: category=DECISION id=B-73 topics=B-73]
- **選択肢・論点**: 参照ペイン（§9.5.2）と ContextPacket（§15.4.1）がそれぞれ順番を持つ / **1 枚の対応表を両方が引く**
- **決定と理由**: **`context_rules.rs` に 1 枚の対応表を置く**（`PaneSection::packet_blocks` と逆引きの `PacketBlock::pane_sections`）。§9.5.2 の 9 セクションと §15.4.1 の 9 ブロックは**別の並び**なので「同じもの」とは言えないが、**情報源**は重なる（例: ペインの 5 五感・6 空間・8 挿絵はどれもパケットの `3 plot` に入る。9 診断はパケットに対応ブロックが無い）。対応表を 1 か所に置けば、M4（ペイン）と M10（パケット）が食い違わない。**優先順位の枠（`Layout`）も共用**で、ペインは §9.5.2 の「同一話」まで（`PANE_FARTHEST`）、パケットは §15.4.1 のブロックごとの範囲（`6 lexicon` は当該節など）を**上限として渡す**だけ。`Layout` の行範囲は**呼び出し側が木から作る**（core は I/O をせず形式も問わない、§3.2・B-56）

### [HIST:DECISION:B-74] 2026-10-13 §6.5.2 の**文**の区切り方（第 2.8 版で `^` / `$` を文単位にした、§9.1 A-23）
[HIST-TAG: category=DECISION id=B-74 topics=B-74,lexer,role_language,testing]
- **選択肢・論点**: (a) 文末記号 1 字ごとに区切る / (b) 文末記号の**連続**の直後で区切る。あわせて、頭の文末記号・前後の空白・`=` 前置・複数の一致をどう扱うか
- **決定と理由**: **(b)**。`！？` も `……` も 1 つの区切り。**頭（発話の頭・区切りの直後）の文末記号の連続は区切りにせず次の文に含める**（`…あの…その…` → `…あの…` / `その…`。付録 B.3 の 2 行目がこれで成り立つ）。**前後の空白（`lexer::is_space`）は文に含めない**（`えっ！　ちょっ…` の全角空白は和文の組版慣習で、含めると文頭固定が当たらない）。**`=` 前置だけは発話全体**（§6.5.2 の表「発話全体が一致」）。`‥` と `.` は省略記号のアトムには入るが文末記号にしない（§6.5.2 が挙げる 6 字だけ）。1 つの台詞で複数の文が一致したら**全部**返す（`Matcher::find_all`。`find` は最初の 1 件）。`RegexSet` の前置フィルタは文ごとに当てる。実装は `core/resolve/role_language.rs` の `sentences`、テストは `tests/role_language.rs` の「§6.5.2 の文の区切り」の節

### [HIST:DECISION:B-75] 2026-10-13 E020 / E022 の引数（第 2.8 版、§9.1 A-9・A-10）
[HIST-TAG: category=DECISION id=B-75 topics=B-75,i18n,diagnostics]
- **選択肢・論点**: C・B 形式の E020 の `{0}` に (a) 最少の 3 / (b) 範囲 `3-4` / (c) 書かれた階層数。5 階層以上を E020 にするか。E022 の `{0}` に ID 全体か番号だけか
- **決定と理由**: E020 は **(b) `Arg::text("3-4")`**（C・B 形式は 3 と 4 の両方を受け付けるので、1 つの数だと「3 階層だけ」と読めて A-9 の矛盾が再発する。A 形式はこれまでどおり `Arg::number(期待する階層)`）。en 文言は範囲でも読めるよう `Only level {0} headers are allowed here` に直した（仕様 第 2.8 版）。**5 階層以上**はどの形式のファイルにも置けず「対応ファイルへ移動」できないので **E002** のまま。E022 の `{0}` は**書かれたままの ID**（`@` を除く。E008 と同じ流儀）

### [HIST:DECISION:B-76] 2026-10-13 §3.4 の `&Project`（11 機能が受け取るプロジェクト全体）の形と置き場所
[HIST-TAG: category=DECISION id=B-76 topics=B-76,characters,modes,dictionary]
- **選択肢・論点**: `model::Project`（`project.json`）を使う / 構文木を持つ集約 / **文書を所有し、木は呼ぶたびにパースする集約**
- **決定と理由**: **`core::features::Project`（`features/project.rs`）で、設定・文書（`path → Document`）・構築済みの辞書を所有する**。構文木（`Characters<'a>` 等）は原文を借用するので持つと自己参照になる（unsafe が要る）。木は機能を呼ぶたびにパースし直す（M2 のパーサは速く、打鍵ごとの再解析は §6.7 の増分が担う）。辞書だけは §6.1.2 どおり 3 ファイルが変わったときに作り直して持ち続ける。`model/` に置かないのは `model` が `syntax` を使えないから（B-29）。§3.3 のツリーに `features/project.rs` を足した（仕様 第 2.8 版）

### [HIST:DECISION:B-77] 2026-10-13 `'` を走査する範囲（§5.0.2「全形式の本文部」）
[HIST-TAG: category=DECISION id=B-77 topics=B-77,lexer,contents,dictionary]
- **選択肢・論点**: 行全体 / 形式ごとの木から決める / **字句器の行の種類で決める**
- **決定と理由**: **字句器（`lexer::classify`）の行の種類で決める**（`features::project::scan_ranges`）。見出し・節区切り・行頭コメントは走査しない。フィールド行は値（行末コメントの手前まで。B-36 で値に残るコメントは除く）、継続行は字下げの後ろ、通常の行は行末コメントの手前まで。`*.md` の `#` 見出し行は項目名そのものなので走査しない。行内コメント `/* */` の中も除く。範囲の外は空白（§6.2.1 の区切り文字）に置き換えてから辞書で走査するので、位置は元の行と同じ

### [HIST:DECISION:B-78] 2026-10-13 11 機能の引数の順と位置の単位
[HIST-TAG: category=DECISION id=B-78 topics=B-78,folding,contents]
- **選択肢・論点**: §3.4 の本文「`(&Project, &Document, Position)` 等」の順 / **§3.4 の表の順**。位置は UTF-16 の `Position` / byte
- **決定と理由**: **§3.4 の表の順**（`semantic_tokens::full(doc, project)`、`completion::at(doc, pos, project)` など）。本文の括弧は「引数の種類」の列挙と読む（仕様 第 2.8 版で本文を表に合わせた）。`document_symbol::all(doc)` と `folding::ranges(doc)` はプロジェクトを取らない。位置は**ファイル内の byte**（B-1・§3.4「core は内部的に byte offset」）で、UTF-16 との変換は `LineTable`（アダプタと UI の仕事）。`rename` は `prepare(doc, pos, project)` / `apply(doc, pos, new_name, project)`

### [HIST:DECISION:B-79] 2026-10-13 `diagnostics::document` でモードの診断（E003 / E004 / W006）をどの文書に載せるか
[HIST-TAG: category=DECISION id=B-79 topics=B-79,diagnostics,contents,characters,modes]
- **選択肢・論点**: 解決に関わった全ファイルに載せる / **その文書に書かれた割り当てだけ**
- **決定と理由**: **その文書に書かれた割り当てだけ**（本文を開いて話プロットの `mode:` の E003 が出ることはない）。本文: 話者注釈の修飾子（段 1）と本文中の `'名前:モード`。話プロット: (節, キャラ) の組ごとに 8 段を解いて話プロットの診断だけ残す。キャラは「その場面・節・前文の `mode:` に書かれた人」と「対応する本文の同じ節で話している人」で、**W006 は後者だけ**、しかも**モードを 1 つも定義していないキャラは対象外**（端役の台詞ごとに警告が並ぶ）。章・全体プロット: `cast:`。`characters.md`: `default-mode:` の E003 / E005（どの節にも効くので E004 は出さない）。`mode:` / `cast:` 以外の `'名前:モード` は 8 段を通らないので `modes::check_id`（M4 で公開）で直接見る

### [HIST:DECISION:B-80] 2026-10-13 セマンティックトークンの形と `role-language`
[HIST-TAG: category=DECISION id=B-80 topics=B-80,role_language]
- **選択肢・論点**: 重なりを許す / **重ならない列**。`role-language` を種別にする / **修飾にする**
- **決定と理由**: **重ならない・位置の昇順の列**（LSP の `semanticTokens/full` は重なりを表せない）。内側が外側を切り欠き、優先順位は「台詞 < ルビ < 参照 < 話者注釈 < コメント」。§6.3 の `role-language` は「下線専用、色は変えない」ので**種別ではなく `SemanticToken::role_language`**（LSP の tokenModifiers）。`ref.address` / `citation` / `illustration` は enum にあるが ① では出さない（PLANS §5.5 の表）

### [HIST:DECISION:B-81] 2026-10-13 §6.5.3「直前の話者注釈の継続」の及ぶ範囲
[HIST-TAG: category=DECISION id=B-81 topics=B-81,modes]
- **選択肢・論点**: ファイルの終わりまで / 場面の終わりまで / **節の中だけ**
- **決定と理由**: **節の中だけ**（`;` を跨がない）。節が変われば話者も変わるのが普通で、跨ぐと別の人の台詞に下線が付く。継続した行では話者注釈の修飾子（段 1「その行のみ」）は効かないので、プロット側のモードで解く。それでも決まらなければ話プロットの `pov`（節 → 場面）

### [HIST:DECISION:B-82] 2026-10-13 §3.4 の表の「モード名の薄表示」
[HIST-TAG: category=DECISION id=B-82 topics=B-82,i18n,diagnostics,materials,modes]
- **選択肢・論点**: 出さない（§6.6.2 は資料だけ）/ **モードの表示名を出す**
- **決定と理由**: **出す**。`'リリア:B` と話者注釈 `@リリア:B` の直後に、`### B 呆れ（強）` の表示名を書かれた順に（`B+C` なら 2 つ）。同じ ID の変種はその行の節を含むものの表示名。未定義（E003）と表示名の無いモードは出さない。括弧と文言は UI（§12.2）

### [HIST:DECISION:B-83] 2026-10-13 §5.12 の 2（改行コードの統一）と 6（`: ` の後は空白 1 個）の細部
[HIST-TAG: category=DECISION id=B-83 topics=B-83,lexer]
- **選択肢・論点**: 文書の中で CRLF にする / **保存時に任せる**。全角 `：` も `： 値` にする / **全角の後ろは触らない**
- **決定と理由**: 改行は**保存時**（`lexer::normalize_for_save`）。文書はメモリ上では常に LF（§5.0.4）なので、`formatting` は行末の `\r` を落とすだけ。全角 `：` は §5.0.5 が空白を求めないので**後ろは触らず**、key との間だけ詰める（和文の書き方を変えない）。編集は変わった行ごと

### [HIST:DECISION:B-84] 2026-10-13 改名の単位と、最短一致を壊す名前
[HIST-TAG: category=DECISION id=B-84 topics=B-84,materials,dictionary]
- **選択肢・論点**: 実体（キャラ）の全キーを改名 / **カーソルの下のキー 1 つ**
- **決定と理由**: **キー 1 つ**（H1 の表示名か `alias:` の 1 項目）。そのキーで解決された参照と、キャラなら完全一致の話者注釈を書き換える（別名で書いた参照は別名のまま）。資料は index が正本なので対象外。新しい名前の**真の接頭辞が別のキー**なら最短一致で短いほうに解決されるので（§6.2.2）、参照を**明示形 `'{…}`** で書く。正規化して既存のキーと同じ名前は `Duplicate`

### [HIST:DECISION:B-85] 2026-10-13 §9.7.3 の並び順の「直近使用」
[HIST-TAG: category=DECISION id=B-85 topics=B-85,dictionary]
- **選択肢・論点**: core に使用履歴を渡す引数を足す / **core は飛ばす**
- **決定と理由**: **飛ばす**（使用履歴は UI の状態で、§3.4 のシグネチャ `(doc, pos, project)` に入らない）。core は「現在の節 > 同じ話（＝この文書） > 五十音（`reading` 優先、無ければキーの文字コード順）」で返し、UI が直近使用で安定ソートし直せば仕様の順になる。挿入する文字列は、最短一致で別の項目になる・区切り文字を含むキーなら明示形 `{キー}`

### [HIST:DECISION:B-86] 2026-10-13 ① の `code_action` が返すクイックフィックス
[HIST-TAG: category=DECISION id=B-86 topics=B-86,diagnostics,materials,modes]
- **選択肢・論点**: §7 の列を全部 / **テキスト編集だけで決まるもの**
- **決定と理由**: **テキスト編集だけで決まるもの**（一覧は `code_action.rs` の doc）。ファイルをまたぐ移動（E020・E024・W031）と、作家がどこへ・どの範囲にを選ぶ操作（E025「既定値で開く」・W022 / E004「range 拡張」・E004「変種を追加」・E003「モードを新規定義」・E007「新規資料を作成」・E017「範囲を分割」）は UI の対話が要るので返さない。E006 の「index 側を書き換える」は原文に同じ書き方のタイトルが 1 か所だけあるときに限り、JSON を整形し直さない

### [HIST:DECISION:B-87] 2026-10-13 C 形式（本文）の見出しの重複
[HIST-TAG: category=DECISION id=B-87 topics=B-87,diagnostics,contents]
- **選択肢・論点**: 検査しない（M2 のまま）/ **E008**
- **決定と理由**: **E008**。§7 の E008 は「同一ファイル内の ID 重複」で形式を限らず、B 形式（話プロット）は M2 から検査していた。B 形式の `warn_duplicate` を `contents.rs` に移して共有する（B-40）。比べるのは ID と階層の組（`@01-04-02` の後の `@01-04-02-03` は §5.2.2 の「D 節目から」で重複ではない）

### [HIST:DECISION:B-88] 2026-10-13 §9.1 A-24 の診断コード（語クラスの正規表現が不正）
[HIST-TAG: category=DECISION id=B-88 topics=B-88,diagnostics]
- **選択肢・論点**: §7 に置く / §7.1 に置く / W にする
- **決定と理由**: **§7 の E027**（ユーザー裁定は「診断を足す」＝ A-24 の (1)。番号・表・重大度は実装側に任された）。`project.json` の中に位置を持つので §7.1（位置を持たない通知）ではなく §7。設定の誤りなので E（E025 と同じ扱い）。番号は E025（JSON）の近くの空き番号で、② の予約域（W080 以降 / E032 以降 / E064 / I054 以降）に触れない。不正かどうかは値を**単独で**コンパイルできるか（`Lexicon::invalid_word_classes`）で決める。パターンに埋め込んだときだけ壊れる値（同じ名前付きグループを 2 回使う等）は E027 を出さず、従来どおりそのパターンだけ捨てる（まれで、単独の検査で取りこぼしても執筆は止まらない）。位置は値の文字列リテラル（B-45 と同じく原文から探す）。見つからない書き方（`\u` エスケープ）ならキー → `"word_classes"` → ファイル全体に広げる。仕様 第 2.9 版

### [HIST:DECISION:B-89] 2026-10-13 M5 の「設定の読み込み」の置き場所
[HIST-TAG: category=DECISION id=B-89 topics=B-89,diagnostics,materials,modes]
- **選択肢・論点**: ストレージが `model::Project` を作って渡す / **`project.json` の文書から集約が自分で読む**
- **決定と理由**: `features::Project` が `project.json` の文書を `insert` / `with_documents` で受け取ったときに `syntax::project_json::parse` で設定を置き換える（読めなければ既定値、§4.3-4）。文書を外せば既定値。ストレージは「読んだ文書を渡す」だけで済み、エディタで `project.json` を直したときも同じ経路で設定が変わる（実効値は常に `project.json` の原文の関数）。E025 / E027 は `diagnostics::document` が `project.json` の上に出す。**`.novnow/ui.json` と `stats.json` は壊れていても診断を出さない**（§4.4.1 の MUST「エラーを出さない」と §4.4.2。§9.1 A-17 の解消欄の「`ui.json` / `stats.json` のパース失敗も同じ E025」は仕様と食い違うので取り消した。E025 を出すのは `project.json` と `materials/index.json` だけ）

### [HIST:DECISION:B-90] 2026-10-13 ストレージのポートの形
[HIST-TAG: category=DECISION id=B-90 topics=B-90,storage_policy,web,cli,testing]
- **選択肢・論点**: 同期の trait ／ Web だけ別の API ／ **非同期の trait 1 つ**
- **決定と理由**: `ports::storage::Storage` の `list` / `dirs` / `read` / `write` / `append` / `remove` は `impl Future` を返す（RPITIT。trait に `async fn` と書くと `async_fn_in_trait` の警告で `-D warnings` に落ちるので、宣言は `fn … -> impl Future`、実装は `async fn`）。IndexedDB は非同期の API しか持たないので、両方を 1 つの trait にするにはこれしかない。`Send` は要求しない（Web の Future は `JsValue` を持つ）。ネイティブの実装は即座に完了する Future を返し、CLI とテストは `adapters::block_on`（std だけの 20 行。ネイティブ専用）で待つ。パスは `core::policy::ProjectPath`（ルートからの相対・`/` 区切り。空・絶対・`\`・`.` / `..`・空の要素・`:`・制御文字を作る時点で拒否）なので、trait から字句的にルートの外へは書けない。`dirs()` は §4.3 の必須ディレクトリの判定用で、ディレクトリを持たない IndexedDB は `None`

### [HIST:DECISION:B-91] 2026-10-13 規則 10 の関門の作り
[HIST-TAG: category=DECISION id=B-91 topics=B-91,diagnostics,storage_policy]
- **選択肢・論点**: core が I/O 込みで判定する ／ adapters 全体で書く ／ **関門 1 ファイル ＋ core の字句判定**
- **決定と理由**: `core::policy::WritePolicy` は**解決済みの絶対パス**を字句で判定するだけ（`..` を正規化してルートより上は `Escape`、要素単位の前方一致でプロジェクト内、履歴はそのファイルだけ、履歴の祖先はディレクトリの作成だけ＝`check_dir`）。シンボリックリンクの解決は `adapters::storage_fs::resolve`（存在する最も深い祖先を `canonicalize` し、無い残りを字句のままつなぐ）。書き込みの system call は `storage_fs.rs` の `FsGate` にしか無く、`tests/layering.rs` が `src/` 全体を検査する（§13.1.2 の「`adapters/` 以外」より狭い。仕様 第 2.9 版）。違反は debug で panic・release で E060（`PolicyViolation::diagnostic`）。ユーザーが選んだパスは panic しない `FsGate::check` で先に確かめる。判定はロックを外してから panic する（毒にしない）。履歴の置き場所は `FsGate::new(&impl Env)` が §13.2.1 の規則で決め、パスを渡す口は無い。履歴は `write_history` からしか書けず、他の口は `Allowed::Project` だけを通す。関門が別のプロジェクトを開くと古い `FsStorage` は書けない（§13.1.1「現在開いている」）。解決できないパス（壊れたリンク等）は `Violation::Unresolved` で書かない。削除はリンク自身を消すのでリンクの置き場所で判定する。原子的保存の一時ファイルは前回の残り（リンクを含む）を消してから `create_new` で作り、置き換える前のファイルの権限を引き継ぐ

### [HIST:DECISION:B-92] 2026-10-13 履歴ファイルの書き方と時刻
[HIST-TAG: category=DECISION id=B-92 topics=B-92,web,ui]
- **選択肢・論点**: §8.3 の原子的書き込み ／ **直接上書き**
- **決定と理由**: §13.1.1 は一時ファイルであってもプロジェクト外への書き込みを禁じるので、§8.3 の一時ファイル方式は使えない（仕様 第 2.9 版で §8.3・§13.2.2 に明記）。壊れても §13.2.2 の「空として扱い上書き」で回復する。親ディレクトリの作成は許す。`opened_at` は RFC 3339 の UTC（`Z`）: 標準ライブラリにローカル時差の API が無く、クレートも足せない（`Clock::utc_offset_minutes` はネイティブで 0、Web は `Date#getTimezoneOffset`）。暦の計算は `clock_system` に自前で持つ（Hinnant の算法）。場所は `Env`（`var` / `os` / `home_dir`）から決める純粋な関数（`recent_file::location`）で、`XDG_STATE_HOME` が相対パスなら無視（XDG の規則）、`HOME` が無ければ `directories` の既定。形は `core::policy::recent`（デスクトップの `{path, opened_at}` と Web の `{id, opened_at}` が同じ規則。手で足されたキーは読むときに落ちる＝温存しない、重複は新しいほうを残す、`schema_version` が 1 以外は空）。置き場所が決まらない環境では書かない

### [HIST:DECISION:B-93] 2026-10-13 Zip の書き出し先と取り込みの規則
[HIST-TAG: category=DECISION id=B-93 topics=B-93,contents,export,storage_policy,web]
- **選択肢・論点**: §13.5 の保存ダイアログ ／ **`exports/<name>.zip`**
- **決定と理由**: 規則 10 と §5.6 の完了条件 1（エクスポートと Zip にプロジェクト外のパスを与えたら拒否）を優先し、デスクトップは `exports/<project-name>.zip` に書く（§13.5・§17.2 を仕様 第 2.9 版で修正。§17.2 の「名前を付けて保存」もデスクトップはプロジェクト内に限る）。書き出し先の Zip 自身は入れない（入れると毎回入れ子になる）。ドットで始まる名前は入れず取り込まないが、`novnow new` が作る `.gitignore` / `.gitkeep` は例外（落とすと往復で消え、完了条件 3 の「内容一致」が崩れる）。単一のルートを剥がすのは「全エントリが同じ 1 つのディレクトリの下」のときで、その名前が構成のディレクトリ名（`contents` 等）なら `<root>/project.json` があるときだけ。`..` のようなルートは剥がさない（剥がして安全にはしない）。絶対パス・`..`・`:` を含む名前は取り込まずに一覧で返す（zip slip の防止）。展開後の上限は 1 エントリ 256MiB・合計 1GiB。デスクトップの取り込みは既存のファイルを上書きしない。Web はテキストのみ（UTF-8 でないものは `NotText` で一覧） **改（2026-10-13、仕様 第 2.10 版）**: ユーザー裁定で、デスクトップの書き出し先は「**保存ダイアログで指定したパス**（プロジェクトの外でもよい＝ §13.1.1 の特例）」になった（**B-99**）。`exports/<project-name>.zip` はダイアログの初期値と、ダイアログを通さない `export_project` に残る。取り込みの規則は変わらない

### [HIST:DECISION:B-94] 2026-10-13 §4.3 の判定と読み込みの分担
[HIST-TAG: category=DECISION id=B-94 topics=B-94,diagnostics,modes,export,storage_policy]
- **選択肢・論点**: adapters が判定する ／ **判定は core、材料は adapters**
- **決定と理由**: `core::model::project::validate(&Listing)`（ファイルとディレクトリの集合・`project.json` の中身・UTF-8 として不正なファイルの一覧）。`project.json` が読めない・`schema_version` が数でなければ版の検査を飛ばす（E025 を出して既定値、§4.3-4）。`Encoding` はファイルを列挙する（仕様の例より情報を足した）。材料集めと文書の読み込みは `adapters::project_load`（どの `Storage` でも同じ手順。`load` は `.md` / `.txt` / `.json` を全部読んで UTF-8 を確かめ、`.novnow/`・`exports/`・ドットファイルは除く。§4.2 の形式のものだけを §5.0.4 の正規化にかけて `features::Project` に入れる）。§3.3 のツリーに `adapters/project_load.rs` を足した

### [HIST:DECISION:B-95] 2026-10-13 外部変更の監視の分担
[HIST-TAG: category=DECISION id=B-95 topics=B-95,diagnostics,dictionary,export,web]
- **選択肢・論点**: v0 のように監視スレッドを adapters に置く ／ **規則は core、1 回分は adapters、タイマは UI**
- **決定と理由**: `core::policy::external_change`: スタンプ（更新時刻・長さ）の差分で候補を出し（`Snapshot::diff`）、**中身を比べてから**反応を決める（`react`: 同じなら無視＝自分の保存や `touch`、辞書の 3 ファイルは常に再読込、Clean は再読込 ＋ I052、Dirty は衝突）。自分の保存は `write` が返すスタンプを `acknowledge` する。`.novnow/`・`exports/`・ドットファイルは見ない。1 回分（一覧 → 差分 → 置き換え）は `adapters::project_load::poll`。1,000ms のタイマは M7 の UI（ネイティブはスレッド、Web は `setInterval`）

### [HIST:DECISION:B-96] 2026-10-13 IndexedDB のアダプタの確かめ方と、v0 の取り込み
[HIST-TAG: category=DECISION id=B-96 topics=B-96,diagnostics,storage_policy,web,testing]
- **選択肢・論点**: wasm の実行テストを足す（ブラウザが要る）／ **純粋な部分を分けてネイティブで試し、接続はコンパイルだけ**
- **決定と理由**: `storage_idb.rs` の上半分（レコードの形・`app` の許可キー・W062・ID）は `tests/storage_idb.rs`。`wasm32` だけの `web` は CI の WASM ジョブがコンパイルを確かめ、実行の確認は M7 の Web UI（完了条件に足した）。`app` ストアに書く口は `push_recent` だけ（任意のキーの API を持たない）。トランザクションの境界は §13.3.3 のとおりで、成功は `oncomplete` で判定する（§8.3）。日時は UTC のミリ秒付き RFC 3339（固定幅で文字列の順 = 時刻の順。`by_updated` のため）。サンプルの投入（§13.3.5）は仕組みだけ（中身の付録 C は M11）。UUID は `crypto.getRandomValues`（§3.1 の図の `Random` ポートは足さない）

### [HIST:DECISION:B-97] 2026-10-13 残りのポート
[HIST-TAG: category=DECISION id=B-97 topics=B-97,agents,storage_policy,cli]
- **選択肢・論点**: —
- **決定と理由**: `Clock`（`now_millis` / `utc_offset_minutes`）・`Env`（`var` / `os` / `home_dir`。M6 で言語・`NO_COLOR`・端末の判定に広げる → **改（M6、B-101）**: OS のロケール `locale` を足した。端末かどうかは標準入出力の性質なので `cli::Terminal` が持つ）・`Dialog`（~~**開くためのダイアログだけ**。プロジェクト外には書けないので、保存先を選ぶダイアログは持たない~~ → **改（2026-10-13、B-99）**: §13.1.1 の特例のため、Zip の保存先を選ぶ `save_zip` と、その結果の型 `ChosenPath` を足した → さらに **改（B-109）**: テキストの「名前を付けて保存」の `save_text` も足した）・`McpClient`（§15.8.1 の 1 回の呼び出し。中身は M10）。実装は `adapters/env_system.rs`（§3.3 のツリーに追加）と `clock_system.rs`

### [HIST:DECISION:B-98] 2026-10-13 §5.6 の完了条件 2（実機で一通り操作して、プロジェクト外のファイルが履歴 1 個だけ）の扱い
[HIST-TAG: category=DECISION id=B-98 topics=B-98,cli,ui,testing]
- **選択肢・論点**: M7 まで保留する ／ **M5 はストレージ層で実施して記録し、GUI での実施を M7 の完了条件にする**
- **決定と理由**: M5 の時点では操作できる実機（CLI は M6・GUI は M7）が無い。`docs/verify-rule10.md` に手順（ファイル差分・strace・fs_usage・Process Monitor）と、GUI のライブラリが勝手に書くもの（eframe の `persistence`・GTK の `recently-used.xbel`・シェーダのキャッシュ）を書いた。ストレージ層で一通りの操作をなぞるテスト（`after_a_full_session_…`）を実プロセスとして走らせ、ファイルシステム全体の差分で確かめた（記録は同書 §5）。GUI での実施は §5.8 の完了条件に足した

### [HIST:DECISION:B-99] 2026-10-13 §13.1.1 の特例（保存ダイアログで指定された Zip はプロジェクトの外にも書ける）の作り
[HIST-TAG: category=DECISION id=B-99 topics=B-99,contents,export,storage_policy,testing]
- **選択肢・論点**: 関門の一般の口に許可を足す ／ 指定されたパスを関門に覚えさせる ／ **専用の判定・専用の口・消費される型**
- **決定と理由**: ユーザー裁定（2026-10-13）「保存ダイアログによりユーザーから指定されたパスへの保存は特例として外部保存を許可する」。仕様 第 2.10 版で §13.1.1 に特例を足し、§13.1.2・§13.5・§19.10（と §0 の表・P6・§15.5.4 の言い回し）を直した。(1) 判定は core の `WritePolicy::check_chosen_export` だけで、`check` は特例を知らない（本文・設定・ログ・提案・履歴の口は従来どおり外を拒否する）。条件はファイル名が `.zip`（大文字小文字を問わない）で終わることと、履歴ファイルでないこと。シンボリックリンクは**行き先で**判定する（`.zip` を装ったリンクで任意のファイルを潰させない）。プロジェクト内を指定したら `Allowed::Project`（ふつうの原子的保存）。(2) 書き込みは `FsGate::write_chosen_export(ChosenPath, &[u8])` だけ。`ports::dialog::ChosenPath` は `Clone` を持たず値で消費される（1 回の指定で 1 回）。関門はパスを覚えない。外へは一時ファイルを使わず直接上書きし（§13.1.1「一時ファイルであっても」）、親ディレクトリは作らない（作ると「指定した 1 個」より多くを外に作る。ダイアログで選べるのは既存のディレクトリ）。(3) ダイアログの結果は panic しない `check_chosen_export` で先に確かめる（§13.1.2）。`zip::export_to_chosen` が内部でそうし、書き出し先がプロジェクト内ならその Zip を中身から除く。(4) `Dialog::save_zip(suggested)` を足した（B-97 を改める）。初期値は `exports/<name>.zip`、上書きの確認は OS のダイアログに任せる。(5) ~~§17.2 の「名前を付けて保存」（テキストのエクスポート）は特例に含めない（裁定は Zip について。広げるならユーザーの判断）~~ → **改（2026-10-13、B-109）**: ユーザー裁定でテキストにも広げた。(6) 指定されたパスは履歴にも書かない（§13.1.3）。`tests/storage_policy.rs` に 5 本（core 1・関門 3・リンク 1）を足し、総合テストの期待値を「履歴 ＋ 指定した Zip」にした

### [HIST:DECISION:B-100] 2026-10-13 CLI の文言カタログをどこでビルドするか
[HIST-TAG: category=DECISION id=B-100 topics=B-100,i18n,cli,ui,theme]
- **選択肢・論点**: `ui` feature の後ろのまま（CLI は別の表を持つ）／ CLI 専用のモジュールに写す ／ **`ui::i18n` だけを常にビルドする**
- **決定と理由**: 仕様 §12.2「CLI のメッセージも同じカタログを使う」を守るには、GUI なしのビルド（既定）でもカタログが要る。`toml` を通常の依存にし（feature の後ろは `eframe` / `egui` / `rfd` / `fontdb` の 4 つ。§2.1・B-54 を改める）、`src/lib.rs` の `pub mod ui;` は cfg なし、`src/ui/mod.rs` が `i18n` 以外を `#[cfg(feature = "ui")]` で守る。`tests/layering.rs` が (1) この形、(2) `ui/i18n` が GUI のクレートを使わないこと、(3) `src/cli/**`・`src/main.rs`・`src/ui/i18n/**` に日本語の文字列リテラルが無いこと（§12.2 の検査を CLI に広げた）を固定する。`Cargo.lock` は変わらない（optional でも lock には載っていた）

### [HIST:DECISION:B-101] 2026-10-13 実行時の i18n と言語の決定（§12.3・§12.4）
[HIST-TAG: category=DECISION id=B-101 topics=B-101,i18n,web,cli]
- **選択肢・論点**: —
- **決定と理由**: `ui::i18n`: カタログは言語ごとに初回に 1 度だけ読む（`OnceLock`）。`text` は欠けたキーを `en` → キーそのものに落とし、**デバッグビルドでは panic**（§12.3 の `debug_assert!`。clippy の `assertions_on_constants` を避けて `if cfg!(debug_assertions) { panic! }`）。`fill` は `{n}` を 1 回の走査で埋める（値の中の `{0}` は埋め直さない・範囲外はそのまま）。言語は `Locale::detect(setting, env)`: (1) `ui.json` の `language` が `ja` / `en`（`system` は飛ばす）(2) `NOVNOW_LANG` が `ja…` / `en…`（他の値は無視して次へ）(3) OS のロケール（`ja…` → ja、`C` を含むそれ以外 → en）(4) ja。OS のロケールは `Env::locale` を足して取る（B-97 を改める）: 既定の実装は POSIX の順（`LC_ALL` → `LC_MESSAGES` → `LANG`）の環境変数、`SystemEnv` は無ければ Windows の `GetUserDefaultLocaleName`（FFI）/ Web の `navigator.language`。**CLI は (1) を読まない**（§14.2.3 が `NOVNOW_LANG` → OS → ja と書いており、壊れたプロジェクトでも同じ言語で出すため）。仕様 §14.2.3 の「`LANG`/`LC_ALL`」を POSIX の順に直した（第 2.10 版）

### [HIST:DECISION:B-102] 2026-10-13 起動の判定（§14.3）の細部
[HIST-TAG: category=DECISION id=B-102 topics=B-102,agents,export,cli,ui]
- **選択肢・論点**: —
- **決定と理由**: `cli::decide(args, terminal)` は純粋。順は `--gui` → `--help`/`-h` → `--version`/`-V`（この 3 つは**どの位置にあっても**効く。`novnow check --help` は使い方）→ 第 1 引数がサブコマンドと完全一致 → `-` で始まる知らない引数（2）→ 位置引数 1 つで Open（2 つ目は余分な引数で 2）→ 引数なしは stdin か stdout が端末なら使い方を stderr に出して 2、どちらも端末でなければ GUI（stderr だけが端末でも GUI）。`-` だけはディレクトリ名。`new` は 0〜1 個、`check` / `mcp` はちょうど 1 個、`export` は最初の引数がディレクトリ（`-` で始まれば 2）で残りはそのまま M9 に渡す。§14.2.5 の「未知サブコマンド」は §14.1 の規則（完全一致しない語はディレクトリ）と両立しないので、未知の**オプション**のことと読んだ（`novnow chekc` は「見つからない」で 3）

### [HIST:DECISION:B-103] 2026-10-13 §14.2 の例に無い場合の表示と終了コード
[HIST-TAG: category=DECISION id=B-103 topics=B-103,cli]
- **選択肢・論点**: —
- **決定と理由**: 例のあるもの（不足）は一字一句そのまま（`tests/cli.rs` が仕様書を読み直して照合）。見つからない → 見出し「ディレクトリが見つかりません」＋対象＋構造の解説＋`novnow new <対象>`（3。`new` で作れるので案内する）。ディレクトリでない → 見出し＋対象（3）。読めない（権限など。一覧の失敗を含む）→ 見出しに OS のメッセージ（4）。新しすぎる → 見出しに版＋「novnow を新しい版に更新してください」（3）。UTF-8 の検査は全文を読む `check` だけ（起動前の検査は `listing` で文字コードを読まない、§5.7 作業項目 7）で、該当ファイルを列挙して 4。「対象」は解決済みの絶対パス（Windows の `\\?\` は外して表示）。`novnow new` の案内は対象がカレントディレクトリなら `novnow new`、そうでなければ絶対パス付き（空白や記号を含めば引用符。日本語はそのまま）。色は出力先が端末・`NO_COLOR` なし・Windows でないときだけ（見出しの `novnow:` と `check` のコード）。仕様 §14.2.3〜§14.2.5 に書き足した（第 2.10 版）

### [HIST:DECISION:B-104] 2026-10-13 `novnow new` の雛形の置き場所と中身
[HIST-TAG: category=DECISION id=B-104 topics=B-104,i18n,diagnostics,contents,ideas,agents]
- **選択肢・論点**: CLI に置く ／ 文字列リテラルで core に書く ／ **`core::template` に、ファイルを `include_str!` で埋め込む**
- **決定と理由**: 雛形と既定プロンプトは CLI（M6）・GUI の「新規作成」（M7）・Web のプロジェクト作成（M11）・`prompts/` 欠落時の I050（§4.2）・`>prompt reset`（M10）が共有するので、I/O を持たない core に**データ**として置く。中身はプロジェクトの内容（§12.6 の「翻訳しない」もの）であって UI の文言ではないので、UI 言語に関わらず日本語・ファイルのまま（`src/core/template/project/`）。§12.2 の日本語リテラル検査は `.rs` の中だけを見るので抵触しない。`project.json` は `schema_version` と `title`（ディレクトリ名）だけ（② のキーを書かない・既定値は書かない）。`ideas.md` は見出しを置くとアイデア 1 件になる（§5.7）ので**書き方のコメント 1 行**にした（仕様 §14.5 の「見出しのみ」を直した）。サンプルは 1 名（モード 2 つ・`## tone` に役割語 2 本）・1 語・本文 1 節（話者注釈つき）・3 つのプロット（`mode:` と `check:` つき）。プロンプトは §5.9.1 の形・§15.5.2 の 8 つの約束・付録 A の要素を全役割に入れ、変数は指示に要るものだけ（文脈の残りは ContextPacket が運ぶ、§15.4）。`novnow check` で**診断 0 件**（`tests/template.rs`。B 形式の末尾に `;` を置くと W030 になるので置かない・行頭の `ascii キー:` と §5.0.6 の別名＋コロンを本文に書かない）。URL は書かない（§14.2.1 の MUST）

### [HIST:DECISION:B-105] 2026-10-13 M6 の時点で中身の無いもの（`export` / `mcp`・GUI）の扱いと、§19.2 の GUI の項目
[HIST-TAG: category=DECISION id=B-105 topics=B-105,agents,export,web,ui]
- **選択肢・論点**: 新しい終了コードを作る ／ 0 で終わる ／ **2（使い方の誤り）で説明して終わる**
- **決定と理由**: §14.2.5 に無い番号は作らない。`export` / `mcp` は先に §14.2 の検査をする（不備なら 3）。GUI は M7 なので、`novnow <有効なディレクトリ>` は検査を通ったあと「この novnow には GUI が含まれていません」で 2、引数なし・非端末（GUI からの起動）も同じ。**M6 は履歴に書かない**（積むのは GUI がプロジェクトを開けたとき、§14.4。§5.7 作業項目 7 の「開けたときだけ」は M7 の作業項目 12 で実装する）。§19.2 のうち GUI / Web の項目は M7 / M11 の完了条件に移した

### [HIST:DECISION:B-106] 2026-10-13 `novnow new` の細部（§14.5）
[HIST-TAG: category=DECISION id=B-106 topics=B-106,storage_policy,cli,ui]
- **選択肢・論点**: —
- **決定と理由**: 既定は `.`。無いディレクトリは親ごと作る（`FsStorage::create`。Zip の取り込み先と同じ扱い）。既存のディレクトリが **新しすぎる版**なら何も書かずに §14.2.4 などを出して 3。対象がファイルなら 4（作れない）。存在するファイルは 1 バイトも変えず（`FsStorage::exists` で確かめて書かない）、競合が無ければ見出し＋対象＋案内、あれば「作成したファイル」と「上書きしなかったファイル」の一覧（雛形の順）を標準出力に。何も作らなかったときは見出しを「作るファイルはありませんでした」にする。案内は GUI があれば `novnow <dir>`、無ければ `novnow check <dir>`（`cli::GUI_AVAILABLE`。M6 は false）。`.gitkeep` も「無いファイル」として作る

### [HIST:DECISION:B-107] 2026-10-13 Windows のコンソール（§14.3）と、その検査
[HIST-TAG: category=DECISION id=B-107 topics=B-107,web,ui]
- **選択肢・論点**: `windows-sys` を足す ／ **FFI を 2 本だけ自前で宣言**
- **決定と理由**: 新しいクレートを足さない（`AttachConsole` と `GetUserDefaultLocaleName` の 2 関数だけ）。リリースビルドは `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`、起動直後に `AttachConsole(ATTACH_PARENT_PROCESS)`（失敗は無視、`AllocConsole` は使わない）。**`cmd.exe` は GUI サブシステムの exe の終了を待たない**（出力がプロンプトの後に出て `%ERRORLEVEL%` が取れない）ので、Windows の CI では `start /wait` か PowerShell の `Start-Process -Wait` を使う（仕様 §14.3 に注意を書いた）。作業環境には Windows の std が無いので、CI の wasm ジョブ（名前は必須チェックのため据え置き）に `cargo check --target x86_64-pc-windows-msvc` を足し、novnow 自身の警告も赤にする。「エクスプローラから起動してもコンソールが出ない」は手動の確認が残る（§5.7 の完了条件 3）

### [HIST:DECISION:B-108] 2026-10-13 `novnow check` の出力
[HIST-TAG: category=DECISION id=B-108 topics=B-108,i18n,diagnostics,ui]
- **選択肢・論点**: —
- **決定と理由**: 診断は `diagnostics::document`（M4。GUI の波線と同じもの）を全文書に掛けたもの。標準出力に 1 行 1 件 `パス:行:列: コード 文言`（行と列は 1 始まり・列は文字数＝ `LineTable::char_position_of_byte`。位置を持たない診断は `パス: コード 文言`）。並びはパス → 位置 → コード（文書内は §9.5.2 の順ではなく位置の順にする。CI のログで上から読めるように）。最後に空行と件数の要約（文書数・E / W / I）。E があれば 5、W と I だけなら 0。既定で無効の診断（§7.2）の絞り込みはしない（`document` が出さない）。文書の選び方は `project_load::is_document`（`.gitkeep` などは数えない） → **改（2026-10-13、B-110）**: 位置の後に作家の言葉での「どこ」`[場所]` を足した

### [HIST:DECISION:B-109] 2026-10-13 テキストのエクスポートの書き出し先（§13.1.1 の特例を広げる）
[HIST-TAG: category=DECISION id=B-109 topics=B-109,diagnostics,export,storage_policy,cli,testing]
- **選択肢・論点**: 特例を「保存ダイアログの結果」のまま広げる ／ **書き出しの種類（`ExportKind`）ごとに拡張子を決め、保存ダイアログと `--out` の両方から作る**
- **決定と理由**: ユーザー裁定（2026-10-13）「ユーザーが保存を明示的に指示したときは、ユーザーが指定したパスにだけ保存を許可する」。仕様 第 2.11 版で §13.1.1 を表にし、§13.1.2・§17.2・§19.10・§0 の表・P6 を直した。(1) `core::policy::ExportKind::{Zip, Text}`。書ける拡張子は Zip が `.zip`、Text が `.txt` / `.md`（大文字小文字を問わない）。種類に合わない拡張子（Zip の書き出しで `.txt` を指定する、など）と履歴ファイルは `Violation::NotAnExport`（E060）。種類で縛るのは、「明示した 1 ファイル」の許可で原稿や設定ファイル（`.json` など）を潰させないため。(2) `WritePolicy::check_chosen_export(target, kind)`・`FsGate::{check_chosen_export, write_chosen_export}` は `ChosenPath` の種類を使う。`ChosenPath` は `from_save_dialog(path, kind)`（保存ダイアログの実装とテスト）と `from_command_line(path, kind)`（`novnow export --out`。M9）だけで作る。`Clone` を持たず、1 回の指定で 1 回だけ書ける。(3) `Dialog::save_text(suggested)` を足した（デスクトップの「名前を付けて保存」。初期値は既定の `exports/…`）。(4) `zip::export_to_chosen` は Zip 以外の種類を E060 で拒む。(5) 条件は Zip と同じ（一時ファイルを作らず直接・親ディレクトリは作らない・パスを覚えない・履歴に書かない）。`--out` の親ディレクトリが無ければ I/O エラー（CLI は 4）。`tests/storage_policy.rs` に 2 本（テキストの外への書き出し、種類の取り違え）を足した

### [HIST:DECISION:B-110] 2026-10-13 `novnow check` の「どこ」を作家の言葉で（§14.2.5・§10.9.2）
[HIST-TAG: category=DECISION id=B-110 topics=B-110,i18n,contents,characters,words,ideas]
- **選択肢・論点**: 行の末尾に足す ／ **`パス:行:列:` の直後に `[場所]`** ／ 別の行にする
- **決定と理由**: ユーザー裁定（2026-10-13）「`パス:行:列:` に加えて、エンジニアでなくても分かる場所（本文・プロットは `@A-B-C-D`、資料は ID とタイトルなど）を出す」。(1) 判定は core の `core::features::place::at(document, byte, project) -> Option<Place>`（文言を持たない）。本文・話プロットは行の節（`;` の行はそれが閉じる節、見出しの行と B 形式の場面のフィールドは場面、最初の見出しより前と位置の無い診断はファイル名の話）。章・全体プロットはブロック。`characters.md` は人物（モードの中ならモード）、`words.md` は用語、`ideas.md` は ID とタイトル、資料は索引の項目（原文から `"M0137"` を探す。JSON は位置を持たないため）か、本体なら索引の `path`（無ければファイル名の ID）、プロンプトは役割と `## ` の節、`project.json` は「作品の設定」。それ以外（`agents/` など）は `None` で、`[場所]` ごと省く。(2) 文言は `place.*`（ja / en、15 件）。`ui::i18n::place(locale, &Place)` が GUI と CLI の両方で使う。(3) 並び（パス → 位置 → コード）と件数の要約は変えない。`[…]` は `grep` しやすいように行の先頭のパス・位置を崩さず、その直後に置いた。`tests/place.rs`（9）・`tests/cli.rs`（+1）・`tests/i18n_catalog.rs`（+1。`place.*` の過不足）

### [HIST:DECISION:B-111] 2026-10-13 節エディタの作り（§8.1 と §9.8.5 (5) の食い違い）
[HIST-TAG: category=DECISION id=B-111 topics=B-111,ui]
- **選択肢・論点**: 自前の編集ウィジェット ／ **`egui::TextEdit` ＋ 構文ハイライトの layouter**
- **決定と理由**: §8.1 は「複数行の `TextEdit`」、§9.8.5 (5) は「`TextEdit` をそのまま使わない」と書いていた。`TextEdit` は 0.27.2 で編集中のフレームに `IMEOutput` を出し、undo（§8.6）・選択・IME の確定処理も持つので、作り直す理由が無い。layouter で §11.2 の色、描いた後に `Galley` の行から波線（§11.3）と節内の行番号（§9.4.1）を重ねる。節ごとに `Id` を分けるので undo は節ごとに残る。**§9.8.5 (5) を「`TextEdit` を使わない編集領域だけの規約」に直した（仕様 第 2.12 版）**。§9.8.5 (3) の「変換中は Key を全部捨てる」は `ui::shell::take_commands` が `TextEdit` より先に `InputState::events` から消す

### [HIST:DECISION:B-112] 2026-10-13 節単位編集の置き場所と、末尾の新しい節
[HIST-TAG: category=DECISION id=B-112 topics=B-112,contents,ui,testing,benchmark]
- **選択肢・論点**: UI に持つ ／ **`core::features::sections`（純粋）＋ `ui::workbench`（egui に依存しない状態）**
- **決定と理由**: 節の並び（ファイル名順 → ファイル内の順）・節の本文（`Section::content` = 末尾の空行を除く。末尾の空行は書き戻しても残る）・書き戻し・新しい節・場面/話の移動・SceneId ジャンプを core の純粋関数にし、テストを既定のビルドで回せるようにした。**ファイル・場面の末尾に作った新しい節**は、`;` の後に本文が無いと節にならない（§5.2.2）ので、**書き始めるまで文書に無い節として持つ**（`;` だけ先に入れ、書き戻すとその位置に行が入る）。文字数（§9.6.1）は**空白を除いた生の文字数**（コメント・注釈の記号も数える。M9 のアノテーション除去と揃えるかは M9 で判断）

### [HIST:DECISION:B-113] 2026-10-13 参照ペインの中身（§9.5.2 の ① の 6 セクション）を集める場所
[HIST-TAG: category=DECISION id=B-113 topics=B-113,i18n,lexer,words,materials,modes]
- **選択肢・論点**: UI で集める ／ **`core::features::reference`（純粋。文言なし）**
- **決定と理由**: M10 の ContextPacket と情報源を揃える（§15.4 の MUST）ため core に置いた。1 プロット = 話プロットの当該節の内容行（`point_line`。フィールド行は除く。無ければ場面の `summary`）、2 上位プロット = 章 → 全体の `summary` の 1 行目、3 配役 = 節に出るキャラ（話者注釈 → `'` 参照の出現順）と §6.4 の解決で有効なモード（話者注釈の修飾子は「その行のみ」なので、ペインは**節の既定**で決める）と性格・感情・役割・口調、4 用語 = `term` と `definition` の 1 行目、7 資料 = 明示参照のみ（**引用注釈 `^M` は ②**、条件一致候補 §16.4 は M8）、9 診断 = 節に掛かる E → W。80ms のデバウンスは `ui::widgets::reference_pane`

### [HIST:DECISION:B-114] 2026-10-13 保存の I/O の分け方と `ui.json` の書き込み
[HIST-TAG: category=DECISION id=B-114 topics=B-114,storage_policy,ui,testing,benchmark]
- **選択肢・論点**: ウィジェットが直接書く ／ **`SaveMachine`・`Workbench` は I/O をせず、`ui::app::Host` が書く**
- **決定と理由**: 状態機械を egui と I/O から切り離してテストした（`tests/gui.rs`）。デスクトップの `Host` は `FsStorage`（`FsGate` を通る）で、書いたファイルは外部変更の監視（`Snapshot::acknowledge`）に覚えさせる。**`ui.json` は保存のときに一緒に書き、設定だけの変更（テーマ・言語・比）は Clean ならすぐ書く**。§8.2 の「`ui.json` で 200–2000ms」のキー名が §4.4.1 に無かったので **`autosave_ms`** にした（仕様 第 2.12 版）。eframe の `persistence` は使わず、`persist_window: false`（規則 10）

### [HIST:DECISION:B-115] 2026-10-13 §11.2 の表に無いトークンの色と太字
[HIST-TAG: category=DECISION id=B-115 topics=B-115,contents,materials,ui]
- **選択肢・論点**: 色を足す ／ **近い役割の色を流用**
- **決定と理由**: `ref.address` = キャラの色、`citation` = 資料の色、`illustration` = コメントの色、`ruby.base` = 本文の色（`address`・`citation`・`illustration` は ② のトークンで、① の本文には出ない）。**太字**は egui 0.27 に太字の字体が無いので色だけで示す（同梱フォントも 1 ウェイト）

### [HIST:DECISION:B-116] 2026-10-13 キー割当の衝突（§9.1 と §9.8.2）と macOS の読み替え
[HIST-TAG: category=DECISION id=B-116 topics=B-116]
- **選択肢・論点**: §9.1 を正 ／ **§9.8 を正**
- **決定と理由**: §9.1 の「`Ctrl+Alt+←/→` でメイン/参照の比」は §9.8.2 の「前 / 次の話」と衝突する。§9.8 は「v1 確定版」なので、**比を `Ctrl+Alt+Shift+←/→`** にした（仕様 第 2.12 版）。§9.8.4 の「macOS では `Alt` 系を `Ctrl+Alt` に読み替える」は**矢印以外**に限った（`Option+矢印` は文字を入力しないうえ、読み替えると `Ctrl+Alt+←/→`（話の移動）と衝突するため）

### [HIST:DECISION:B-117] 2026-10-13 M7 のパレットとショートカットの範囲
[HIST-TAG: category=DECISION id=B-117 topics=B-117,materials,dictionary]
- **選択肢・論点**: 全部 M7 ／ **write ワークスペースで閉じるものだけ M7**
- **決定と理由**: パレットは「なし / `@`」= 節（場面タイトル込み）、`>` = コマンド 8 つ、`?` = ヘルプ。**`'`（辞書）と `M` / `#`（資料）は M8**（material / cast ワークスペースと一緒に）。ワークスペースを持つ操作（`Alt+1..9`・`Ctrl+B`・`Ctrl+Shift+B`・`Ctrl+Shift+E`・`Ctrl+Shift+I`・`Ctrl+M`）は M8。`Ctrl+\` / `Ctrl+Shift+\` は ② の一覧（本書 §1.2）にあるが、`ui.json` の `reference_pane_side` は §0 の MUST で、キーも 1 行で済むので入れた

### [HIST:DECISION:B-118] 2026-10-13 Web の GUI（§13.4 のプロジェクト選択と IndexedDB）
[HIST-TAG: category=DECISION id=B-118 topics=B-118,storage_policy,web,ui]
- **選択肢・論点**: M7 で全部 ／ **起動とシェルは M7、選択モーダルの接続は後で**
- **決定と理由**: Web でも同じ `NovnowApp` を `eframe::WebRunner` で起動し、Pages の deploy を戻した（`index.html` に `data-cargo-features="ui"`）。IndexedDB は非同期（`Storage` の Future）で、`Host::write` を同期で返す今の形には「書き込みの結果を後のフレームで受け取る」口が要る。作業環境にブラウザが無く確かめられないので、**M7 では選択モーダルに「準備中」を出す**。M7 の完了条件「Web で IndexedDB の接続を一通り動かす」は未達として残す。→ **2026-10-14 に接続した（B-156〜B-158。ヘッドレスの Chromium で確かめた）**

### [HIST:DECISION:B-119] 2026-10-13 M7 の検索・置換・補完・ポップアップの範囲
[HIST-TAG: category=DECISION id=B-119 topics=B-119,diagnostics,contents,modes,dictionary,ui]
- **選択肢・論点**: 仕様の全部 ／ **write ワークスペースで閉じる形**
- **決定と理由**: `Ctrl+F` / `Ctrl+H` / `F3` は**今のファイルの文字どおりの一致**（大小文字を区別。正規表現と大小文字無視は ② の §9.7.4）。一致が見出し行など節の外にあれば、その後ろの節へ移るだけ（選択しない）。`Ctrl+Shift+F` は**本文（`contents/`）の全ファイル**（プロット・辞書は M8 のワークスペースで開くので、M7 では移動先が無い）。補完（§9.7.3）は `'` `:` `@` の直後と `Ctrl+Space` で `core::features::completion::at` の候補を出し、`↑/↓` と `Enter` / `Tab` で確定（変換中は §9.8.5 で Key が来ない）。`Ctrl+Shift+M` / `Ctrl+Shift+W` は `hover::at` の中身（この節で使えないモードは `×`）。`Ctrl+Shift+;` は確認のうえ節と区切り `;` を消す（**ファイル単位の削除なので `Ctrl+Z` では戻らない**と確認文に書いた。§8.6 の undo は節エディタの中だけ）。`Ctrl+Tab` は直前に開いていたファイル、`Ctrl+Shift+Tab` は最も古いファイル（押し続けての循環はしない）。`Ctrl+O` は今のプロジェクトを書き終えてから、フォルダを選んで §14.2 の検査を通ったら切り替える。`Ctrl+.`（クイックフィックス、19 種の名前は `action.*`）・`Ctrl+T` / `Ctrl+Shift+T`（`code_action::register_edit` を E001 の登録と共有）・`Ctrl+Shift+D`（差分モーダル。**「自分の版を残す / 外部の版を読み込む」の 2 択で、§8.5 の行単位マージは入れていない**）も M7 で入れた。**`Alt+H`（インレイヒント §6.6）は M7 では入れていない**（`TextEdit` の `Galley` に仮想の文字を差し込む口が egui 0.27 に無い。文脈ビューだけに出すか M8 で決める）。→ **M8 で入れた**（エディタに隙間を空けて描く。**B-137**）

### [HIST:DECISION:B-120] 2026-10-13 M7 の後の順番（M8 と M9）
[HIST-TAG: category=DECISION id=B-120 topics=B-120,export,ui]
- **選択肢・論点**: §4.1 の順（M8 → M9） ／ **M9（エクスポート）を先に**
- **決定と理由**: §4.3 の依存では M8・M9・M10 はどれも M7 の後で互いに独立。M9 は core の純粋関数（§17.1）・`novnow export`（§17.2）・GUI の「名前を付けて保存」で閉じる 5 日の段で、M6 の報告へのユーザー裁定（テキストのエクスポートの書き出し先、B-109）をそのまま使える形にできる。M8（18 日）は M7 の `write` の上に 6 つのワークスペースを積む大きな段なので、先に閉じられる M9 を済ませてから入る。§4.2 の「見せられる状態」の順（M8 完了 → M9 完了 = β）は入れ替わるが、β の中身（エージェント以外すべて）は変わらない

### [HIST:DECISION:B-121] 2026-10-13 §11 の色のコントラスト比（§19.1）
[HIST-TAG: category=DECISION id=B-121 topics=B-121,contents,ui,theme]
- **選択肢・論点**: 表の値のまま ／ 文字を大きくする ／ **色の明るさだけを直す**
- **決定と理由**: §11.1 の注記は `fg.faint / bg.base` を dark 4.6:1・light 4.5:1 としていたが、表の値（`#6E7278` / `#87898F`）を WCAG 2.x の式で測ると 3.7:1 / 3.4:1 だった。`fg.faint` は 10〜11px の見出し（§9.2.2・§9.3.1）に使うので §9.9 の「本文 4.5:1」が要る。§11.2 の `comment`（dark 2.8:1）と `section.break`（2.8 / 2.5:1）は「補助 3:1」も満たしていなかった。色相を変えずに明るさだけを動かし、`fg.faint` = `#82878E` / `#727479`、`section.break` = `#596574` / `#8D929A`、`comment`（dark）= `#60666E` にした（仕様 第 2.13 版）。どの色をどちらの基準で測るか（文字の色は `bg.base` に 4.5:1、`fg.*` と `accent` は `bg.raised` にも 4.5:1、本文の大きさの注記と `state.*` は 3:1、`border` は対象外）を §11.1 に書き、`tests/gui.rs` が §11.1 / §11.2 の表と `ui::theme` の一致、と比を検査する

### [HIST:DECISION:B-122] 2026-10-13 エクスポートの状態の絞り込み（§17.2 `--state`）の意味
[HIST-TAG: category=DECISION id=B-122 topics=B-122,export]
- **選択肢・論点**: 節の `state` だけを見る ／ **近いものから順に引き、`s` 以上を出す**
- **決定と理由**: §17.2 は「`--state fixed`（`draft` を除外して公開分だけ出す）」とだけ書く。`state` は A 形式（章・話のブロック）にも B 形式（節）にも書けるので、節の実効の状態を **話プロットの節 → 同じ場面の場面レベル（最初の要点より前。§5.4.2 では節だけのキーだが、書かれていれば場面の既定として読む）→ 章プロットの話のブロック → 全体プロットの章のブロック → `draft`** の順に引く（章を `fixed` にしたら中の節が全部 `fixed` になる、が自然）。順序は `draft` < `review` < `fixed` で、`--state s` は **実効の状態が `s` 以上**の節（`review` なら `review` と `fixed`）。読めない値（`done` など）は書かれていないのと同じ。`core::export::effective_state` / `select`。仕様 第 2.14 版の §17.2 に書いた

### [HIST:DECISION:B-123] 2026-10-13 エクスポートの範囲・出力の細部（§17.2）
[HIST-TAG: category=DECISION id=B-123 topics=B-123,export]
- **選択肢・論点**: —
- **決定と理由**: (1) 範囲は §5.1 の範囲式に加えて **`@` を省いた書き方**（`01-04`、`01 .. 03, 05`）も受け付ける（PowerShell では `@` で始まる引数が特別な意味を持つ）。左端 > 右端は誤り。(2) 既定の名前の `<範囲>` は全体 `all`・項 `01-04` / `01~03` / `~03` / `03~`・複数は `_` でつなぐ。`<title>` は `title`（空ならディレクトリ名）で、`/ \ : * ? " < >

### [HIST:DECISION:B-124] 2026-10-13 §17.1 の表が決めていない変換の細部
[HIST-TAG: category=DECISION id=B-124 topics=B-124,lexer,contents,materials,modes,dictionary]
- **選択肢・論点**: —
- **決定と理由**: (1) **区切り**: 出す節と節のあいだ 1 か所につき 1 回。`;` だけでなく**場面・話・章の境目**（見出し行を除いたところ）も 1 回にする（見出しを消すだけだと場面の切れ目が消えて本文がつながる）。範囲・状態で節が飛んでも 1 回。`insert_headings` で見出しを入れたところには区切りを入れない（見出しの前後に空行）。見出しのタイトルが空なら入れない。(2) **参照**: 名前部が資料 ID の形（`M0137` / `S0042`）なら未解決でも修飾子ごと消す。未解決の参照は名前部を残し、`:` の後ろがモード式なら消す。**直後が区切り文字か行末の `'`**（仕様書 §5.2.3 の例の `'『星屑の時計』`）は `dictionary::scan_line` では「ただの引用符」だが、本文の `'` はシジルで、リテラルの `'` は `\'` で書く約束（§5.0.3）なので消す。英語の短縮形（直前が英数字）と `リリア's`（B-63）は残す。(3) エスケープを解いた文字（`\@` → `@` など）は作家のリテラルなので、不変条件の対象外（結果に `@` で始まる行が出ることはある）。(4) 書き換えが重なったら先に始まるものを採る（参照の名前の中のルビなど）。(5) 先頭の空行も削る。(6) 字下げは本文の 1 行を段落とし、既に空白で始まる行には足さない。(7) フィールド行（4）は C 形式に無い（§5.0.5）ので何もしない。仕様 第 2.14 版の §17.1 に書いた

### [HIST:DECISION:B-125] 2026-10-13 デスクトップのエクスポートの操作（§17.2）
[HIST-TAG: category=DECISION id=B-125 topics=B-125,export,storage_policy,web,ui]
- **選択肢・論点**: ショートカットを足す ／ **パレットの `>export` からモーダル**
- **決定と理由**: §9.8 にエクスポートのキーは無いので、パレットのコマンド `>export` でモーダルを開く。範囲の欄（空欄なら全体）・状態（`Tab` で すべて → review 以上 → fixed だけ）・節の数と字数（文書の世代・範囲・状態が変わったときだけ作り直す）。`Enter` で既定の `exports/…`（`Host::write`）、`Shift+Enter` で名前を付けて保存（`Host::choose_export` → `Host::write_export`。デスクトップは `RfdDialog::save_text` → `FsGate::check_chosen_export` → `write_chosen_export`。初期値は `exports/`、無ければルート。ディレクトリは作らない）、`Ctrl+Shift+C` でクリップボード（`egui::Context::copy_text`）。3 つのボタンの文字は §9.2.1 の 1（アイコンだけでは役割が分からない）。結果はステータスバーの通知、失敗はモーダル。「エクスポート完了ダイアログの CI/CD のリンク」（§17.2 の末尾）は `CICD_GUIDE_DOC_URL` が ② なので出さない（本書 §1.2-14）。Web の Host は B-118 と一緒に後で

### [HIST:DECISION:B-126] 2026-10-13 ワークスペースの切替の細部（§10.0）と、本文以外の文書の編集
[HIST-TAG: category=DECISION id=B-126 topics=B-126,reader,i18n,contents,characters,ideas]
- **選択肢・論点**: 切替のたびに今の節へ着地 ／ **初回だけ着地、2 回目からは自分の最後の位置**
- **決定と理由**: (1) `Alt+1..9`（macOS は `Ctrl+Option`）で切り替え、`Alt+0` で直前のワークスペースへ（続けて押すと 2 つを行き来する）。§10.0 の「SceneId は維持」と「各ワークスペースが最後に居た位置を復元」を両立させるため、**初めて開くワークスペースだけ今の節（write の SceneId）の対応物に着地**し、2 回目からは自分の最後の位置に戻る。位置を持ち越すのは `Ctrl+B`（上へ）・`Ctrl+Enter`（下へ）だけ（`Ctrl+Shift+1..9` の強制持ち越しは ②）。write の今の節（`ui.json.last_scene`）は他のワークスペースでは変わらない。(2) **`reader`（`Alt+7`）は ②、`agent`（`Alt+8`）は M10 まで欠番**で、押すとステータスバーに「〜はこの版ではまだ使えません」を出して切り替えない。ヘッダのインジケータは開ける 7 つの図形を並べ、今のものを `accent`（ツールチップとアクセシブル名はカタログ）。(3) 切替は保存のきっかけ（§8.2）で、確認は出さない。(4) **4 段階の上下**: `Ctrl+B` と `Ctrl+Shift+B` は同じ「1 つ上へ」（write → plot-episode → plot-outline の章 → 全体）、`Ctrl+Enter` は「1 つ下へ」（ワークスペースごとに意味が変わる。write では従来どおり確定して次の節）。(5) 本文以外の文書（話プロット・全体 / 章プロット・`ideas.md`・`characters.md`・資料）は `Workbench` の **Slice**（文書の行の範囲）で開いて編集する。書き戻し・保存・外部変更・衝突の規則は節エディタと同じ（書き戻しは節の移動・保存・切替の直前）。外部変更で読み直したとき・ワークスペースの操作で文書を丸ごと置き換えたときは Slice を閉じ、開いた側が鍵から開き直す。(6) `Ctrl+Delete`（idea の破棄）と `Ctrl+Shift+←/→`（review のタブ）はテキストの編集のキーと重なるので、**そのワークスペースの中だけ**で取る

### [HIST:DECISION:B-127] 2026-10-13 プロットのワークスペースの行の並び（§10.2・§10.3）
[HIST-TAG: category=DECISION id=B-127 topics=B-127]
- **選択肢・論点**: —
- **決定と理由**: `core::features::plots`（LSP に対応しない補助）: 話プロットの節リストは**場面見出しの行**（見出しと場面レベルのフィールドを編集）と**節の行**（節の内容。`;` と末尾の空行を含まない）の並び。節の要約は最初の要点の行、状態は §17.2 と同じ実効の状態（B-122）。節が 1 つも無い場面（見出しだけ）には見出しの直後に空の節の行を置く（書けば最初の節になる）。`Ctrl+Shift+N` は選んだ節の後ろに**空の節**を入れる（閉じる `;` の次に `;` を 1 行。場面の最後の閉じていない節なら `;` を 2 行）。場面の行なら最初の節の前、話プロットが無ければ今の場面の見出し（`@AA-BB-CC`）を書いて作る

### [HIST:DECISION:B-128] 2026-10-13 plot-episode から write へ降りたときの着地（§10.0）
[HIST-TAG: category=DECISION id=B-128 topics=B-128,contents,benchmark]
- **選択肢・論点**: —
- **決定と理由**: 本文にその節があればそこへ（場面の行なら場面の最初の節）。本文のファイルはあるが節が無ければ近い節へ行き、ステータスバーに「本文に @… はまだありません」（§10.0「空の節を作らず」）。**本文のファイルが無ければ**、場面見出しだけの新しい節として開き（`Workbench::start_file`）、**書き始めるまでファイルは作らない**。書くと `@AA-BB-CC タイトル`（話プロットの場面見出しのタイトル）と本文でファイルを作る

### [HIST:DECISION:B-129] 2026-10-13 plot-outline の操作（§10.3。① は単一ビュー）
[HIST-TAG: category=DECISION id=B-129 topics=B-129,folding,contents]
- **選択肢・論点**: —
- **決定と理由**: 左のレベル切替（全体 / 各章。章は全体プロットのブロック・章プロット・本文と話プロットのファイルのどれかに現れるもの）をクリックか `Ctrl+Alt+←/→`（前 / 次のレベル）で選ぶ。中央はブロックの一覧（見出しの ID とタイトル・`summary:` の 1 行目・`state:` の縦線）と、選んだブロックのエディタ（見出しから `;` の前まで）。章のときは上段に**親（全体プロットの章のブロック）**を出す（§19.11）。`Ctrl+B` / `Ctrl+Shift+B` で章 → 全体、`Ctrl+Enter` で全体の章 → その章・章の話 → plot-episode（話の最初の行）。§10.3 にキーの無い「ブロックを足す」は plot-episode と同じ `Ctrl+Shift+N`（次の番号の見出しだけを書いて `;` で閉じる。文書が無ければ作る）。右は伏線（`seed:` / `payoff:` を全体・章・話プロットから集めて伏線 ID で突き合わせ、未回収 → 二重回収 → その他の順）。タイムライン・分量・ストックのタブと折りたたみは ②

### [HIST:DECISION:B-130] 2026-10-13 idea ワークスペースの細部（§10.4・§5.7）
[HIST-TAG: category=DECISION id=B-130 topics=B-130,lexer,diagnostics,contents,words,ideas]
- **選択肢・論点**: —
- **決定と理由**: (1) 一覧は `status` の絞り込み（既定「すべて」= `dropped` 以外、§5.7）とタグの部分一致。`status` が無い・読めない値は `inbox`。(2) **即時入力** `Ctrl+Shift+I` はどのワークスペースからでも開く 1 行の重ね物で、`Enter` で `ideas.md` の**末尾**に `# I####（最大値 + 1）タイトル` / `status: inbox` / `created:`（RFC 3339。デスクトップは UTC、B-92）を積む。画面は変えず、ステータスバーに知らせる。(3) `Ctrl+Delete`（idea の中だけ。B-126）は `status: dropped` にするだけで行を消さない。(4) **昇格** `Ctrl+Enter` の行き先は 4 つ: プロットの節（write の今の場面の最後に、タイトルと本文を要点として足す。場面が無ければ見出しを書く）・用語（`words.md` の末尾に `# タイトル` / `definition:`。本文が無ければタイトル。E021 を出さないため）・キャラのメモ（選んだキャラの最後に自由記述の行として。§5.5.1）・資料（`materials/inbox/` に提案として作る、B-131）。転記の後 `status: promoted` と `promoted-to: <パス>#<場所>` を書く（`status` / `promoted-to` は §5.0.6 の表に無いキーなので、書かれたままのキーで探して書き換え、無ければ最後のフィールド行の後ろに足す）

### [HIST:DECISION:B-131] 2026-10-13 資料の作成・審査の細部（§16.1・§5.8.2）
[HIST-TAG: category=DECISION id=B-131 topics=B-131,diagnostics,contents,materials,benchmark]
- **選択肢・論点**: —
- **決定と理由**: 人間が作る資料も §16.1 の流れどおり**提案**（`materials/inbox/M####.md`・`status: proposed`・`added_by: human`・`source: {kind: custom}`・カテゴリは空）として作り、material ワークスペースで整えて採用する。ファイル名は ID だけ（`M0142.md`。タイトルをローマ字にするクレートは足さない）。ID は `MaterialIndex::issue`（`next_id` と既存の最大値 + 1 の大きいほう）なので、`next_id` が壊れていても再利用しない。採用は `docs/` へ動かして `approved`、却下は `rejected`（ファイルは inbox に残す）で、どちらも `updated_at` を書く。`index.json` が読めない（E025）ときは何もしない（原文を書き換えない、§5.8.2 MUST）。`items` の順は追加順のまま（§16.2）。索引は serde で書き直す（2 スペースの字下げ）。**追記（M8 の資料の仕上げ）**: (1) `checksum` は `sha256:` ＋ 本文（UTF-8）の 16 進。SHA-256 は自前（`core::text::sha256`、FIPS 180-4 の例とブロックの境目で検査。クレートは足さない、§3.2）。(2) 資料 MD を書く保存のたびに、その MD の `updated_at` と `checksum` を書き直す（`Workbench::touch_materials`、保存の直前）。(3) 採用で inbox → docs に動かすとき、古いファイルは書き込みが全部済んでから消す（既に無ければそれでよい）。(4) 検索（§16.3）: 却下は `!` の語が無ければ出さない。語は §6.1.3 の正規化 ＋ 小文字の部分一致、重みはタイトル 4・タグ 3・summary 2・本文 1、並びは点の高い順 → `saved_at` の新しい順 → ID。(5) 審査フォームは打鍵ごとに索引を書くので、`update` は出典の種類が変わっても `accessed_at` と知らないキーを残す

### [HIST:DECISION:B-132] 2026-10-13 review の 2 タブの細部（§10.9。① は診断と進捗）
[HIST-TAG: category=DECISION id=B-132 topics=B-132,folding,diagnostics,contents,characters,words]
- **選択肢・論点**: —
- **決定と理由**: (1) **診断**はプロジェクトの全文書の `diagnostics::document`（`novnow check` と同じもの・同じ並び）に `パス:行:列` と作家の言葉での「どこ」（`core::features::place`）を添える。重大度の絞り込み（E / W / I）と並べ替え（場所順・重大度順・コード順）。文書の世代か言語が変わったときだけ数え直し、`Ctrl+Shift+E`（どこからでも。ステータスバーの `⚑` のクリックも同じ）は review の診断タブへ移って数え直す。§10.9.2 の一括実行の項目のうち、M4 までに診断があるもの（W030・E004・E006・W022・E012 など）はこの一覧に入る。伏線の未回収 / 二重回収は plot-outline の伏線ペインに出す。W080・W084・I054・I055 は ②（診断そのものが未実装）。(2) `Enter`（`Ctrl+Enter`・ダブルクリックも）で該当箇所へ: 本文 → write の節、話プロット → plot-episode、全体 / 章プロット → plot-outline、`ideas.md` → idea。`characters.md`・`words.md`・資料は開く画面が無ければ知らせるだけ（cast・material の段で足す）。(3) **進捗**は本文の節と話プロットの節の和（計画だけの節も 0 字で出す）を章 → 話 → 場面 → 節に積み、文字数（空白除外）・目標比（節は話プロットの `length:`、上の階層は下の目標の合計）・状態（節は実効の状態 B-122、上の階層は下の**最も遅い**状態）を出す。折りたたみ（§9.3.3）は ②。(4) review には編集欄が無いので、素の `↑` / `↓` / `Enter` も効く（テキストにフォーカスが無いとき）

### [HIST:DECISION:B-133] 2026-10-13 cast ワークスペースの細部（§10.6）
[HIST-TAG: category=DECISION id=B-133 topics=B-133,contents,characters,role_language,modes]
- **選択肢・論点**: —
- **決定と理由**: (1) 左はキャラ一覧（`reading:` の順、無ければ名前の順）、中央は `characters.md` のそのキャラの部分（H1 から次の H1 の前まで）のエディタ。(2) 右のタブ（モード / 役割語 / 未一致）は `Ctrl+Shift+→/←`（cast の中だけ取る。中央の編集欄の単語選択より優先する、B-126）で、キーで切り替えるとタブの中身にフォーカスが移る（`Esc` で編集欄へ戻る）。(3) **モードタイムライン**: 横軸はプロジェクトにある章（全体プロット・章プロット・本文と話プロットのファイル）、縦軸は `## modes` のモード、帯はモードの `range:` と章が交わる列（`range:` が無ければ全部）。帯の操作は**タイムラインにフォーカスがあるときだけ**（クリック・ドラッグ・キーでのタブ切替で移る）: `↑/↓` で帯を選び `Shift+←/→` で 1 章ずらす。ドラッグは列の幅で丸めた章数だけずらす。ずらすのは `range:` の値の各 ID の**章の番号だけ**（下の階層と桁数は保つ。章 0 になるなら動かさない）で、`range:` が無い（全範囲）モードは動かさない。(4) **役割語テスター**: キャラ共通（`## tone`）→ モード専用の順に全パターンを並べ、一致 / 不一致と §6.5.1 の正規表現を出す。一致部分は本文と同じ `state.ok` の 2px 下線。(5) **未一致**（`Ctrl+Shift+V`、cast の中だけ）: 本文の台詞のうち、話者が決まり（行末の話者注釈 → 直前の注釈 → `pov:`、§6.5.3）適用パターンがあるのに**どれも一致しなかった**もの（§6.5.4 の例外として一覧だけ。診断は出さない）。ダブルクリックで本文のその台詞へ。呼称タブ（§10.6.1）とアイコン（§5.5.3 `icon:`）は ②

### [HIST:DECISION:B-134] 2026-10-13 資料の診断と登録（W064・W065、§5.8.3・§16.1）
[HIST-TAG: category=DECISION id=B-134 topics=B-134,diagnostics,materials]
- **選択肢・論点**: —
- **決定と理由**: **W065** は「提案中（`proposed`）で出典が無い」資料（`index.json` の、その項目の `id` の値の位置）。**W064** は索引に無い資料 MD（`materials/**/*.md`、1 行目）。登録は material ワークスペースの `Ctrl+Enter`: 必ず新しい ID（`MaterialIndex::issue`）、タイトルは H1（無ければファイル名の語幹）、`docs/` の下なら `approved`・それ以外は `proposed`、ID のコメント `<!-- novnow:M0137 -->` は書き換えるか H1 の下に入れる。§7 の「登録する」「出典を入力」は `code_action` ではなくワークスペースの操作として出す（索引と MD を一緒に書くため。`Ctrl+.` には出ない）

### [HIST:DECISION:B-135] 2026-10-13 material ワークスペースの細部（§10.5・§16.1・§16.3）
[HIST-TAG: category=DECISION id=B-135 topics=B-135,i18n,diagnostics,words,ideas,materials]
- **選択肢・論点**: —
- **決定と理由**: (1) 左は一覧: チップ「却下以外（既定）/ 提案 / 採用 / 却下」と §16.3 の書式の絞り込み欄。未登録の MD を先頭（赤い帯・ID は「—」）、あとは `saved_at` の新しい順。(2) 中央は MD 全体の Slice、右は審査フォーム（カテゴリはカタログのチップ。打鍵ごとに索引を書き、H1 が古いタイトルと同じならタイトルに追従させる。✓ / ✕ はその状態でないときだけ）。(3) キー: `Ctrl+Enter` 採用（W065 なら確認のオーバーレイ）・`Ctrl+Delete` 却下（ファイルは残し、選択は次の行へ）・`Ctrl+Shift+N` 作成（`draft_of`。空なら「無題の資料」）・`Ctrl+3` フォーム・`Ctrl+2` エディタ。着地は 明示参照 → 条件一致の候補 → 一覧の先頭。(4) `Ctrl+M` はどこからでも: `Enter` は挿入（write = キャレットに `'ID`、plot-episode = 節の最後に `material: 'ID` の行、idea / outline / cast = Slice のキャレット、material / review = 開く）、`Shift+Enter` は開く、`Ctrl+Enter` は作ってから挿入、もう一度 `Ctrl+M` で閉じる。(5) パレット: `#` / `M` は資料検索、`'` はキャラと用語（`Enter` = cast で選ぶ / 用語の情報、`Shift+Enter` = 使用箇所の検索）。(6) review の行のうち資料 MD と `index.json` は material ワークスペースで開く。(7) ボタンだけのオーバーレイ（確認など）は開いたフレームでキーボードのフォーカスを手放す（後ろのエディタに `Enter` が入らない）

### [HIST:DECISION:B-136] 2026-10-13 資料の明示参照と条件一致（§16.4・§9.5）
[HIST-TAG: category=DECISION id=B-136 topics=B-136,contents,materials]
- **選択肢・論点**: —
- **決定と理由**: **明示参照** = 本文の参照 ∪ 話プロットの場面見出しの行と節の `material:`。**条件一致の候補** = 採用済みで、`scope` がその節を含むか、`ref` / タグ（B 形式の `tags:` / `タグ:`）が合うもの。参照ペインでは明示参照の後に薄く出す。ダブルクリックか `Ctrl+3` → `Enter` で足す（write は `plots::append_line` で話プロットのその節に、plot-episode は Slice に足して書き戻す）

### [HIST:DECISION:B-137] 2026-10-14 インレイヒントの描き方（§6.6.2）と、M8 のエディタでの `Ctrl+.`
[HIST-TAG: category=DECISION id=B-137 topics=B-137,i18n,contents,materials,modes,ui]
- **選択肢・論点**: 行末にまとめて描く ／ **トークンの直後に隙間を空けて描く**
- **決定と理由**: egui 0.27 の `TextEdit` は文字ではない表示を挟めないので、ヒントの位置から始まるレイアウトのセクションに `leading_space`（ヒントの幅 + 余白 3px × 2）を足して隙間を空け、`TextEdit` の後にそこへ描く（`ui::widgets::inlay`）。文字と `Galley` の対応は変わらない（カーソル・選択・IME はそのまま。トークンの直後のキャレットはヒントの後ろに出る）。行末・末尾のヒントは隙間を空けずに描く。文字は `fg.faint`・本文の 0.85 倍・ベースラインを揃える。文言はカタログ（`editor.inlay_*`。`en` は `⟨{0} · proposed⟩` / `⟨{0} · rejected⟩`、モードは表示名を `+` でつなぐ）。出すのは**エディタだけ**（節エディタと M8 のワークスペースのエディタ）で、文脈ビューなど読むだけの表示には出さない。`Alt+H`（macOS は `Ctrl+Option+H`、§9.8.4）とパレットの `>inlay-hints` で切り替え、`ui.json.inlay_hints` に残す。括弧 `⟨⟩`（U+27E8 / U+27E9）は Ubuntu-Light にも同梱の IPA ゴシックにも無いので、比例フォントの最後に egui 既定の Hack を受け皿として足した。診断の波線の右端は「範囲の最後の字の右端」にした（隙間の下に線を引かない）。**`Ctrl+.`**: 資料 ID はプロットに書く（§6.6.4）ので、M8 のワークスペースでも Slice の中身とキャレットからクイックフィックスを出す（ポップアップは Slice のキャレットの下）

### [HIST:DECISION:B-138] 2026-10-14 「このブロックを外す」トグル（§15.7.2）の置き場所
[HIST-TAG: category=DECISION id=B-138 topics=B-138]
- **選択肢・論点**: セッションだけ ／ **タスクのファイル**
- **決定と理由**: タスクに `exclude:`（ブロック名を `,` 区切り）を足した。ドライランと実行で同じパケットになり、後から何を外したかが分かる。`project` / `constraints` は外せない（書いても無視）。外せるのは `draft` / `ready` のタスクだけ（第 2.16 版 §5.10.1）

### [HIST:DECISION:B-139] 2026-10-14 壊れたプロンプトのコード（§15.5.4 (2) は「E030」）
[HIST-TAG: category=DECISION id=B-139 topics=B-139,diagnostics,agents]
- **選択肢・論点**: E030 のまま ／ **新しいコード E029**
- **決定と理由**: E030 は「MCP サーバの環境変数未解決」と重なるので、空いていた E029「プロンプトに必須のセクションがありません: {0}」を足した（① の §7 の表、文書位置あり＝1 行目）。必須は `## system` と `## task`。`diagnostics::document` が既定役割のプロンプトに出し、その役割だけ実行できない（`runner::Blocker::Prompt`。E031 も同じ）。§7 は 81 コードになった

### [HIST:DECISION:B-140] 2026-10-14 ContextPacket の中身と予算の細部（§15.4）
[HIST-TAG: category=DECISION id=B-140 topics=B-140,contents,words,ideas,materials,agents]
- **選択肢・論点**: —
- **決定と理由**: (1) 9 つの見出しは中身が無くても出す。中身は `ラベル: 値` か `- ` の 1 件。(2) integrator の予算の列 300 / 1,200 / 600 / 300 / 300 / 200 / 200 / 600 / 2,300（計 6,000）。(3) `max-chars` は合計を上書きし、ブロックは表の比率で伸縮。上限は `min(合計, max_chars_per_request)`。(4) 切り詰めは「ブロックごとに予算まで → まだ超えれば全体で」の 2 段、どちらも §15.4.3 の順位 → 同じ順位の中は後ろから。`current` も削らない。削らない部分だけで超えたら送れない（`Packet::over`）。(5) 文字数はブロックの本文（ラベルと改行を含む）の Unicode スカラー値。(6) `4 target` の前後は節の並び。本文に無い節は ID の前後。空なら `(未執筆)`。(7) `5 cast` は節の本文と場面のプロットに出るキャラ（有効モードだけ。`reference::cast_of` を共有）。(8) `6 lexicon` は本文と節のプロットの用語を出現回数順。(9) `7 materials` は明示参照（本文 ∪ 話プロットの `material:`）。ideator には既存のアイデアの一覧。(10) `9 history` は同じ節の直近 3 タスクの状態と却下のコメント＋届いたメッセージ。integrator は全体の直近 10 タスク。(11) `context-hash` は `generated_at:` を除いた SHA-256。(12) 要求は system = `## system`（＋ `## refuse`）、メッセージ = `## task` → パケット → `## examples`。ドライランはパケットだけ。`project.json` に `genre` があれば `1 project` に入れる

### [HIST:DECISION:B-141] 2026-10-14 stylist / logician / integrator の出力の提案（§5.10.3 の `kind` に無い）
[HIST-TAG: category=DECISION id=B-141 topics=B-141,agents]
- **選択肢・論点**: どれかに寄せる ／ **`note` を足す**
- **決定と理由**: `kind: note` を足した。ファイルには適用しない（`apply` でも自動適用しない）。本体が空（`## question` だけ）の出力も `note`。採用は「読んだ」の記録

### [HIST:DECISION:B-142] 2026-10-14 適用の細部と「undo 可能」（§15.7.3）
[HIST-TAG: category=DECISION id=B-142 topics=B-142,contents,ideas,materials,agents]
- **選択肢・論点**: —
- **決定と理由**: (1) 本文: 節があれば置き換え、無ければ同じ場面の前の節の後ろ（`sections::insert_after`）、節 1 で場面も無ければファイルの末尾に `@場面` ごと足す。それ以外は提案のまま。(2) プロット: 話プロットの節の内容の行を置き換える。(3) アイデア: 1 行 1 件（`- ` などの印を落とす・最大 20 件）を `ideas::capture` で足す。(4) 資料: `materials_ops::propose_as`（`added_by: agent:<role>`）。(5) 自動適用・直接書き込みの前の検証は「E 系の数が増えるか」。(6) 採用すると同じ `target` と `kind` の他の `open` は `superseded`（その `review` のタスクは `rejected`）。(7) 一部採用は `partial: true`（`after` は提案のまま）。(8) 適用はエディタの編集ではなくファイルの書き換えなので、「undo」は**差し戻し**（`before` に戻して `rejected`、コメント必須、監査ログの `reject` に `detail: revert`）

### [HIST:DECISION:B-143] 2026-10-14 自律の上限の数え方（§4.5.1 `agents.limits`）
[HIST-TAG: category=DECISION id=B-143 topics=B-143,diagnostics,agents]
- **選択肢・論点**: —
- **決定と理由**: 見るのは**自動で起こす直前だけ**（人間の実行は止めない）。`max_tasks_per_session` は自動で起こした数、`max_chain_hops` は人間が起こしたタスクを 0 として自動で起こすたびに +1、`max_input_chars_per_session` はセッションで送った文字数の合計。`0` は無制限。超えたら起こさず W095 と `limit_stop`

### [HIST:DECISION:B-144] 2026-10-14 タスクの状態の動き（§15.3・§15.7.1）
[HIST-TAG: category=DECISION id=B-144 topics=B-144,diagnostics,contents]
- **選択肢・論点**: —
- **決定と理由**: `N` で `draft`、`R` で `ready`（実行待ちの列に入る）、走ると `running`、終わると `review` / `applied`。失敗は `ready`（再試行しない）、`X` は `canceled`。E035 が数えるのは `draft` / `ready` / `running` だけ（`review` は人間の番なので、同じ節に新しいタスクを作って提案を並べて比べられる）。場面・話のスコープは本文と話プロットの節の和に展開する（節タスクの束）。`propose` の役割（`note` 以外）は確定済みの節では走らない（A7）。`apply` / `autonomous` の自動適用が確定済みを書き換えたら `fixed_overwrite` とステータスバーの通知

### [HIST:DECISION:B-145] 2026-10-14 メッセージの承認（§15.6・§5.10.4）
[HIST-TAG: category=DECISION id=B-145 topics=B-145,contents,agents]
- **選択肢・論点**: —
- **決定と理由**: `## message`（`to:` / `kind:` / `refs:` と本文）は節のスコープのスレッドに積む。送り手（実行時のレベル）と宛先（今のレベル）が両方 `autonomous` なら `approved_by: auto`、それ以外は `pending`（第 2.16 版で値を足した）。人間が agent ワークスペースで承認すると `human`。宛先の役割の `9 history` に載るのは `human` / `auto` だけ。`novnow.mail.send` の送り手は `mcp`

### [HIST:DECISION:B-146] 2026-10-14 監査ログのキー（§5.10.5）
[HIST-TAG: category=DECISION id=B-146 topics=B-146]
- **選択肢・論点**: —
- **決定と理由**: 10 キーは常に書き、値が無ければ `null`。提案に関わるイベントには `proposal`、`delegation_changed` / `chain_hop` / `limit_stop` / 差し戻しには `detail` を足す（値があるときだけ）。ファイルの日付はイベントの `ts` の日付

### [HIST:DECISION:B-147] 2026-10-14 MCP クライアントの細部（§15.8.1・§5.11）
[HIST-TAG: category=DECISION id=B-147 topics=B-147,diagnostics,agents,modes,ui]
- **選択肢・論点**: —
- **決定と理由**: `stdio` だけ（`http` / `sse` は v1 で持たない）。`servers` は定義順（`serde_json` の `Map` は並べ替えるので自前の訪問者で読む）。`roles` の `*` は全役割。`initialize`（protocolVersion `2024-11-05`）→ `notifications/initialized` → `sampling/createMessage`（`systemPrompt`・`maxTokens` 16,000・`modelPreferences` は `model-hint` から・`metadata.novnow`）。サーバからの `ping` には答え、他の要求には -32601。`sampling` が無く `tools` があるサーバには `allow_tools` の先頭のツールを `{system, prompt, role, scope}` で呼ぶ。stderr は捨てる。環境変数は引き継ぎ＋解決した `env`（解決できなければ E030 で走らない）。タイムアウトは `timeout_ms`（既定 60 秒）、出力は 20,000 字で切って `truncated: true`

### [HIST:DECISION:B-148] 2026-10-14 自律の連鎖（§15.1.1・§15.3）
[HIST-TAG: category=DECISION id=B-148 topics=B-148,diagnostics,agents]
- **選択肢・論点**: —
- **決定と理由**: integrator が `autonomous` のときだけ。(1) integrator 以外のタスクが終わるたびに、その節の integrator のタスク（`initiator: agent`・`intent` は「次の 1 手」・`ready`）を起こして走らせる（同じ節に生きている integrator のタスクがあれば起こさない）。(2) integrator の出力の `## task`（`role:` / `scope:` / `intent:`）から、`off` でない integrator 以外の役割の 1 節のタスクを起こして走らせる（E035 の重複は飛ばす）。(3) 起こすたびに 1 ホップ、`chain_hop` を書く。既定の integrator のプロンプトにこの書き方を足した（行頭に `key:` を書かない、B-104）

### [HIST:DECISION:B-149] 2026-10-14 `novnow mcp` の細部（§15.8.3）
[HIST-TAG: category=DECISION id=B-149 topics=B-149,i18n,diagnostics,materials,agents]
- **選択肢・論点**: —
- **決定と理由**: 呼び出しのたびにプロジェクトを読み直す。結果は JSON のテキスト 1 つ、失敗は `isError: true` と文言（E036 などは診断の文言）。`scene` は 4 階層。`novnow.plot.write` の `level` は `episode` だけ。直接書き込みも自動適用と同じ検証を通し、E 系が増えるなら `open` の提案として残して失敗を返す。提案の `origin` は `mcp`（直接書き込みは `mcp:<role>`）。`novnow.materials.propose` の `added_by` は `agent:<role>`（無ければ `agent:mcp`）。起動時に `prompts/` の欠けた既定ファイルを書き出す（`novnow check` は書かない）

### [HIST:DECISION:B-150] 2026-10-14 §7.2 のうち M10 が使う診断（§1.2 の ② の一覧との関係）
[HIST-TAG: category=DECISION id=B-150 topics=B-150,diagnostics,agents]
- **選択肢・論点**: ② のまま予約 ／ **M10 の分だけ ① に**
- **決定と理由**: **E035**（同じ役割 × 同じ節のタスクの重複）・**E036**（`novnow mcp` の委譲レベル不足の書き込み）・**W095**（セッション上限での停止）は M10 の完了条件と §19.8 が求める機能そのものなので ① で実装する（§1.2 の「横断的に ② へ回すもの」と §5.5 の作業項目 1 の範囲を直した）。E032–E034（カスタムロール）・E064（ローカル LLM）・I056 / I057 は ② のまま

### [HIST:DECISION:B-151] 2026-10-14 agent ワークスペースの操作の細部（§15.7。キーの表は仕様 第 2.16 版）
[HIST-TAG: category=DECISION id=B-151 topics=B-151,agents]
- **選択肢・論点**: —
- **決定と理由**: (1) 4 列のうち 1 つにフォーカスがある（`←` / `→`。列のクリックでも移る）。修飾キーなしの文字キーは、テキストの入力欄にフォーカスが無く、**オーバーレイ（ダイアログ・パレット）も開いていない**ときだけ効く。オーバーレイを閉じたらフォーカスを手放す。(2) ① は新しい順。`N` の対象の初期値は今の節。`Shift+R` の確認には件数・入力文字数の合計・使うサーバ・走れないものとその理由を出す。`Enter` は ③ へ。(3) ② の除外は `draft` / `ready` で走っていないタスクだけ。`1 project` と `8 constraints` は外せない（「このブロックは外せません」）。ダブルクリックは `Space` と同じ。外したブロックも予算の表示は残す。(4) ドライランは `agents/results/T####.dryrun.md` に全文を書き、監査ログには書かない（送っていないので）。(5) ステータスバーの `⟳` は 1 件ならタスク ID と役割（自律なら「（自律）」）、複数なら件数。クリックで agent の ① へ移り、その行を選ぶ

### [HIST:DECISION:B-152] 2026-10-14 実行の仕組み（`Host` と毎フレームの見回り）
[HIST-TAG: category=DECISION id=B-152 topics=B-152,contents,agents,web,testing]
- **選択肢・論点**: 非同期ランタイム ／ **スレッドと毎フレームの見回り**
- **決定と理由**: `Host` に `append`（監査ログ）・`mcp_available` / `mcp_start` / `mcp_poll` / `mcp_cancel`・`env_var` を足す。デスクトップは呼び出しごとに 1 スレッド（結果は mpsc、取消は `AtomicBool`。tokio の process / io-util は使わない）、Web は呼べない（「この環境では LLM を呼び出せません」）。毎フレームの `ui::agents::tick` が終わった呼び出しを 1 件ずつ UI スレッドで処理し（**本文への適用はここで逐次**、U-8）、待ち行列から `max_parallel`・同じ節・同じサーバ 2 本の制約で次を出す。実行中は 250ms ごとに再描画。失敗とタイムアウトは `ready` に戻して知らせ、自動で再試行しない。テストの `MemoryHost` は台本（`replies`）を返す

### [HIST:DECISION:B-153] 2026-10-14 総停止とパレットの項目
[HIST-TAG: category=DECISION id=B-153 topics=B-153,agents]
- **選択肢・論点**: —
- **決定と理由**: `Ctrl+Shift+X` / `>agent stop-all` は、自律で走っているタスク（委譲レベルが `autonomous` か、エージェントが起こしたもの）を待ち行列と実行中から取り消し、`apply` / `autonomous` の役割（既定のレベルによるものも）をすべて `propose` に下げて役割ごとに `delegation_changed` を記録する。人が始めた `propose` / `apply` の実行中のタスクは止めない（§15.7.6「レベル変更は実行中のタスクには影響しない」）。止めた件数と変えた役割の数を知らせる。パレットに `>agent delegation <role> <level>`（7 役割 × 4）・`>agent stop-all`・`>prompt reset <名前 / all>`（並置表示 → 確認）・`>prompt diff <名前>` を足す。④ は既定から変わったプロンプトに「編集済」を出す

### [HIST:DECISION:B-154] 2026-10-14 既定プロンプトを書き出す時期と、無効のときの画面
[HIST-TAG: category=DECISION id=B-154 topics=B-154,diagnostics,agents,ui]
- **選択肢・論点**: 初めて agent を開いたとき ／ **プロジェクトを開いたとき**
- **決定と理由**: GUI はプロジェクトを開いたとき（`Session::prepare_agents`）に、`agents.enabled` なら欠けたファイルだけ書く（`prompts/` 自体が無かったときだけ I050）。フォントのフォールバックの通知は I050 を上書きせず「{I050} / {フォント}」とつなぐ。`agents.enabled = false` のときは何も書かず、agent ワークスペースは「エージェントは無効です」**とだけ**出す（§15.7）。ほかのワークスペースはそのまま動く（A6）

### [HIST:DECISION:B-155] 2026-10-14 §19.8「プロンプトがプロジェクト外から読み込まれない」の担保
[HIST-TAG: category=DECISION id=B-155 topics=B-155,diagnostics,agents,export,storage_policy,cli]
- **選択肢・論点**: プロンプトだけ特別扱い ／ **ファイル全体に広げる**
- **決定と理由**: 読み込み元は `prompts/` の文書だけで、埋め込みの既定は生成元（§15.5.4 (1)）。**ファイルが無い役割は E029（`## system` と `## task` が無い）で走らない**（埋め込みの既定や空のプロンプトで代わりに走らない。④ に印）。M5 の読み込みは外を指すファイルのリンクを辿っていた（§13.1.3「読み取りは明示的に選んだパスに限りプロジェクト外でも許可」に反していた）ので、**行き先がルートの外のファイルのリンクは一覧に出さず、名前で読んでも無いもの**とした（ディレクトリのリンクは M5 から辿らない。仕様 §13.1.3 に追記）。書けないもの（規則 10）をプロジェクトに含めないためで、プロンプト以外も同じ（Zip のエクスポートにも外の中身が入らない）。一覧に無いのにディスクに何かあるパスには既定のプロンプトを書かない（GUI は `Host::occupied`、`novnow mcp` は `FsStorage::exists`。書けば規則 10 の違反）。`tests/storage_policy.rs` の `prompts_are_read_only_from_the_project`・`symlinks::files_behind_links_to_outside_are_not_part_of_the_project`、`tests/cli.rs` の `mcp_leaves_prompts_that_are_links_to_outside_alone`

### [HIST:DECISION:B-156] 2026-10-14 非同期の保存（Web の IndexedDB。B-118 で残した「書き込みの結果を後のフレームで受け取る口」）
[HIST-TAG: category=DECISION id=B-156 topics=B-156,diagnostics,export,storage_policy,web,ui]
- **選択肢・論点**: `Host::write` を Web だけ書いたことにする ／ **保存 1 回分を Host に渡し、結果を後で受け取る**
- **決定と理由**: `Host::save(writes, removals) -> Option<SaveOutcome>`（既定はその場で書いて返す＝デスクトップとテストは今までどおり）と `Host::save_finished()` を足した。Web は `None` を返し、終わったら結果を返す。保存の状態は書き終わるまで `Saving`（§8.3「トランザクション成功 = 保存成功」）。**書いているあいだに文書に入った打鍵**（節を移ったなど）は、渡した中身と今の中身が違えば未保存のまま残して次の保存で書く（比べないと消える。`tests/gui.rs` の `an_asynchronous_save_keeps_what_was_committed_while_it_was_writing`）。保存の外の書き込み（`ui.json`・監査ログ・`exports/`）は列に積んで `Ok` を返し、失敗は `Host::take_events` で後から知らせる。Web の書き込みは **1 本の列で順に**行う（監査ログの追記が重ならない・保存の後に頼んだ Zip が保存後の中身を読む）。`Ctrl+O`（Web は選択画面へ戻る）・Zip のエクスポートは、書き終わってから。容量不足（`QuotaExceededError`）は `SaveFailed` として赤い印と、Zip でエクスポートしてから不要なプロジェクトを消す導線のモーダル（§13.3.7）。W062 は保存の前に `navigator.storage.estimate()`（1 分に 1 回まで）

### [HIST:DECISION:B-157] 2026-10-14 Web のプロジェクト選択（§13.4）の細部
[HIST-TAG: category=DECISION id=B-157 topics=B-157,web,ui]
- **選択肢・論点**: —
- **決定と理由**: (1) 一覧は「開いた日時（`recent_projects`）と更新日時の新しいほう」の新しい順。名前・更新日（ローカル時刻の日付）・大きさ（全ファイルの byte の和。後から数える）。選択行は accent の背景。(2) 新規作成（`Ctrl+N`・`＋`）は名前を入れて `novnow new` と同じ雛形（`core::template`。`project.json` の題 = 名前）で作り、そのまま開く。(3) 初回の `sample`（§13.3.5）の中身は、付録 C の v1 版ができる M11 までは同じ雛形（自動では開かない）。(4) Zip の取り込み（`Ctrl+I`・`📥`。§13.4 の図の `⇪` は同梱のフォントにも egui のフォントにも無いので `📥`）の名前は Zip のファイル名（`.zip` を除く）、新しいレコードに入れる。読めないエントリは知らせに並べる。§4.3 の不備があれば「不備のまま取り込む / 取り消す」、開くときの不足（`Incomplete`）は「足りないファイルを補う」（雛形のうち無いものだけ書く。あるものは上書きしない）。(5) 名前の変更（`F2`）はレコードの表示名だけ（`project.json` の題は変えない）。削除（`Delete`）は名前の入力が一致したら（§13.4）。(6) `Esc` は入力中の操作をやめるだけで、選択画面は閉じない。(7) 別のタブでの変更は見ない（Web は 1 つのタブで使う前提。外部変更の監視 §8.5 はデスクトップだけ）。(8) ブラウザがページに渡さないキー（Chrome の `Ctrl+N` など）は `＋` ボタンで代える（仕様 §13.4 に追記）

### [HIST:DECISION:B-158] 2026-10-14 ブラウザとのつなぎ（eframe 0.27 の Web の足りないところ）
[HIST-TAG: category=DECISION id=B-158 topics=B-158,agents,web,cli,ui]
- **選択肢・論点**: —
- **決定と理由**: (1) **キーの既定の動作を止める**: 修飾キー付きのキー（印刷・ページの保存・検索バー・`Alt+←` の「戻る」など）と `F2` / `F3`。止めないのはコピー / 貼り付け / 切り取り・再読み込み・開発者ツール・`AltGr` の字・macOS の `Option` の字・修飾キーの無い字（`shell::browser_default_blocked`）。(2) **IME**: eframe はマウスで入力欄をクリックしたときだけ IME の隠れた入力欄を出すので、キーボードで節エディタに移ると日本語を打てなかった。入力欄にフォーカスがあるあいだは出しておく（`ui::web::sync_text_agent`）。`index.html` の canvas は eframe の前提（`left: 50%` ＋ `translate(-50%)`）に合わせた（変換候補が左に canvas の幅の半分ずれていた）。(3) **コピー / 切り取り**: eframe は `web_sys_unstable_apis` の cfg のときだけ行うが、その cfg は今の web-sys では eframe がコンパイルできないので、`Ctrl+C` / `Ctrl+X` を egui の `Copy` / `Cut` に読み替え、egui がコピーした字を `navigator.clipboard.writeText` で書く。(4) 未保存の変更があるままタブを閉じようとしたら、ブラウザの確認を出す。(5) ダウンロードは `<a download>` を文書に入れて押し、URL は後で捨てる（すぐ捨てると名前が `download` になるブラウザがある）。(6) 確かめは `scripts/web-smoke.sh`（wasm-bindgen CLI は `.github/workflows/sandbox-tools.yml` が git の blob にして PR にコメントしたものを取る。作業環境から GitHub のリリースの asset に届かないため。ブランチもタグも作らない）

### [HIST:DECISION:B-159] 2026-10-14 v0→v1 移行と v0 検知ロジックの全面破棄、および §18.3 `unsafe` 特例・Web 単一タブ注記
[HIST-TAG: category=DECISION id=B-159 topics=B-159,diagnostics,characters,storage_policy,web,testing]
- **選択肢・論点**: v0 移行を実装する ／ **v0 利用者が存在しないため全面破棄**
- **決定と理由**: ユーザー裁定（2026-10-14）により、v0 の利用者が居ないため v0→v1 移行（`novnow migrate`・§20・§13.3.6 LocalStorage 移行・`Origin::Migrated`）と v0 検知（`SchemaState::LegacyV0`・`ProjectError::LegacyV0`・`characters.json` 特別扱い・診断 `W040` / `E061` / `W066` / `W067` / `I053`）を仕様（第 2.17 版）・コード・テスト・計画からすべて破棄した。`schema_version` が無い `project.json` は E025 で既定値（`1`）にフォールバックし、`characters.md` が無いディレクトリは通常の `Incomplete` として扱う。あわせて §18.3 の `unsafe` 禁止に Windows OS API FFI 2 箇所（`AttachConsole` / `GetUserDefaultLocaleName`）のみを特例として明記し `tests/layering.rs` で機械検査、Web のプロジェクト選択画面に単一タブ前提の注記（`project.pick.single_tab`）を表示した

### [HIST:DECISION:B-160] 2026-10-14 付録 C の `sample` プロジェクトの構成
[HIST-TAG: category=DECISION id=B-160 topics=B-160,diagnostics,materials,role_language,modes,web]
- **選択肢・論点**: 別に文字列を持つ ／ **`examples/sample-novel/` を `include_str!` で共有**
- **決定と理由**: `examples/sample-novel/` を付録 C の要件（1 章 1 話 4 節、モード併用 `'リリア:B+C`、`range` 変種、役割語の一致・不一致、資料参照 `'M0001` と条件一致、未回収の伏線 `seed: 時計の鍵`、`state: fixed` の節、アイデア `# I0001`）を 1 回ずつ含み E/W 診断 0 件の最小プロジェクトに改稿し、`core::template::sample_project()` が同じファイルを `include_str!` で埋め込んで Web の初回 `sample`（§13.3.5）に投入するようにした（`tests/template.rs` の `sample_project_covers_appendix_c_with_no_diagnostics` で検証）

### [HIST:DECISION:B-161] 2026-10-14 §19.13-2（`core::features` の 11 機能すべての UI 接続）
[HIST-TAG: category=DECISION id=B-161 topics=B-161,folding,testing,benchmark]
- **選択肢・論点**: —
- **決定と理由**: テストからしか呼ばれていなかった `formatting`・`definition`・`references`・`rename`・`folding`（および直接呼び出しの `document_symbol`）を UI に接続した: (1) `>format`（`Command::Format` → `formatting::document` → `Workbench::apply_edit`）、(2) `'` パレットの `Enter`（`definition::at`）/ `Shift+Enter`（`references::at` → `Overlay::References`）と `F12` / `Shift+F12`（`>definition` / `>references`）、(3) `F2` / `>rename`（`rename::prepare` → `Overlay::Rename` → `rename::apply`、`Cast` ワークスペースでは選択中のキャラにもフォールバック）、(4) `plot-outline` の左の構成ツリーで `folding::ranges` と `document_symbol::all` を用いた章→話展開（`Ctrl+Shift+]` / `Ctrl+Shift+[` とクリックで展開・折り畳み）。`tests/layering.rs` の `ui_calls_all_eleven_lsp_feature_modules` で機械検査

---

## 6. 仕様策定時の派生決定・未決事項裁定・表記揺れ解決（`SPEC-DECISION`）

### [HIST:SPEC-DECISION:S0-5] 2026-04-19 開発指示内の表記揺れと一意解決（旧 §0.5）
[HIST-TAG: category=SPEC-DECISION id=S0-5 topics=reference_pane,characters_md,roles,labels,material_id]
- **1. 参照情報の左右**: 指示 3 は「右の参照情報」、指示 6・8 は「左に表示」と記載されていたため、位置ではなく役割で「参照ペイン（Reference Pane）」と命名し、既定配置は右（`.novnow/ui.json` の `reference_pane_side: "right" | "left"` で左右入替可能）に統一した。
- **2. 登場人物ファイル名**: 指示 6 の `characters.json` と指示 7 の `characters.md` の揺れは、すべて `characters.md` に統一した。
- **3. エージェント役割数**: 6 項目（プロット・本文・アイデア・資料検索・文体チェック・論理整合性チェック）＋「全作業の統合（Integrator）」を、エージェント役割 6 種＋既定で人間が担い任意でエージェント化できる統合役割 1 種（計 7 役割）と整理し、委譲レベルで委任範囲を制御する形にした。
- **4. ラベル撤廃と最小限の見出し**: ペインの役割名ラベル（「ファイル」「エディタ」「情報」等）は全面撤廃し、参照ペイン内部の情報種別見出し（配役 / 用語 等）のみ最小限のスタイル要素として残した。
- **5. 資料 ID の表記**: `M` + 4 桁ゼロ詰め（`M0137`）と定義した。

### [HIST:SPEC-DECISION:S0-6-1] 2026-10-09 読者モード（追加仕様 #27）を段階 ② へ移管した経緯（旧 §0.6.1）
[HIST-TAG: category=SPEC-DECISION id=S0-6-1 topics=reader,vertical_layout,FontMetrics,stage2]
- **工数と技術的不確実性**: 縦書き組版（`core::reader::layout()` ＋ 描画）だけで約 5.5 人日を要し、egui が縦書きレイアウトを持たないため 1 グリフずつ座標計算して描画する必要がある。
- **層構造との緊張**: 組版にはフォント実測値が必要だが `core/` はフォントを読めないため、`FontMetrics` を引数注入する純粋関数設計が必要となった。
- **裁定**: 指示 12 のうちアノテーション除去エクスポート（§17.1 / §17.2）を段階 ① で完全実装し、縦書きプレビュー（読者モード §10.7 / §17.3、`Alt+7`）は追加仕様 #27 として段階 ② へ移管した（ルビ記法と禁則処理の仕様自体は維持）。

### [HIST:SPEC-DECISION:D-1] 2026-04-19 資料 ID を `M` + 4 桁ゼロ詰めとする
[HIST-TAG: category=SPEC-DECISION id=D-1 topics=D-1,materials]
- **決定**: 資料 ID を `M` + 4 桁ゼロ詰めとする
- **理由**: 「通し番号」の要求に対し、ソート可能・検索可能・将来 5 桁へ自然拡張できる形を選んだ

### [HIST:SPEC-DECISION:D-2] 2026-04-19 `words.json` → `words.md`、`ideas.json` → `ideas.md` も実施
[HIST-TAG: category=SPEC-DECISION id=D-2 topics=D-2,characters,words,ideas,dictionary]
- **決定**: `words.json` → `words.md`、`ideas.json` → `ideas.md` も実施
- **理由**: 指示 7 は `characters.md` のみだが、人間が書く辞書だけ形式が割れるとパーサ・補完・ハイライトが二重実装になる。「人間が書く＝MD / 機械が採番する＝JSON」で統一した

### [HIST:SPEC-DECISION:D-3] 2026-04-19 `plots/index.json` と `plots/preparations.json` を廃止
[HIST-TAG: category=SPEC-DECISION id=D-3 topics=D-3,materials]
- **決定**: `plots/index.json` と `plots/preparations.json` を廃止
- **理由**: ディレクトリ走査で導出でき、二重管理の不整合源になるため

### [HIST:SPEC-DECISION:D-4] 2026-04-19 モードの同一 ID・異 range 変種を許可
[HIST-TAG: category=SPEC-DECISION id=D-4 topics=D-4,modes]
- **決定**: モードの同一 ID・異 range 変種を許可
- **理由**: 指示 6 の「章単位で性格が少しずつ変化する」を、別 ID を増やさずに表現するため

### [HIST:SPEC-DECISION:D-5] 2026-04-19 役割語パターンを正規表現そのものにせず小言語にした
[HIST-TAG: category=SPEC-DECISION id=D-5 topics=D-5,role_language]
- **決定**: 役割語パターンを正規表現そのものにせず小言語にした
- **理由**: 指示は「正規表現のような形式」。作家が `\p{Han}` を書く必要をなくし、かつ提示例（`*`, `(体言)`, `…`）をそのまま書けるようにした

### [HIST:SPEC-DECISION:D-6] 2026-04-19 修飾子 `:` をキャラ（モード）と資料（タイトル）で共用
[HIST-TAG: category=SPEC-DECISION id=D-6 topics=D-6,lexer,materials,modes]
- **決定**: 修飾子 `:` をキャラ（モード）と資料（タイトル）で共用
- **理由**: シジルを増やさずに「参照 + その限定」を一貫して表せるため

### [HIST:SPEC-DECISION:D-7] 2026-04-19 ルビ記法 `｜base《ruby》` を採用
[HIST-TAG: category=SPEC-DECISION id=D-7 topics=D-7,reader,modes,export]
- **決定**: ルビ記法 `｜base《ruby》` を採用
- **理由**: エクスポートと（追加仕様 27 の）縦書き読者モードで必要。投稿サイトの慣例に合わせた

### [HIST:SPEC-DECISION:D-8] 2026-04-19 A 形式/B 形式のフィールドキーに日本語別名を用意
[HIST-TAG: category=SPEC-DECISION id=D-8 topics=D-8]
- **決定**: A 形式/B 形式のフィールドキーに日本語別名を用意
- **理由**: 指示 1 の 2 言語対応をファイル形式側にも及ぼしつつ、正準形は ASCII に固定して機械処理を単純に保つため

### [HIST:SPEC-DECISION:D-9] 2026-04-19 統合（integrator）の委譲レベルの既定を `off`（人間が担う）とした
[HIST-TAG: category=SPEC-DECISION id=D-9 topics=D-9,agents]
- **決定**: 統合（integrator）の委譲レベルの既定を `off`（人間が担う）とした
- **理由**: 既定は「人間が最終判断をする」形から始めるため（P5）。ユーザーは `agents.roles.integrator` を `autonomous` にすれば、執筆計画の管理ごと AI に任せられる（§15.1.1）

### [HIST:SPEC-DECISION:D-10] 2026-04-19 履歴ファイルに `opened_at` を持たせた
[HIST-TAG: category=SPEC-DECISION id=D-10 topics=D-10]
- **決定**: 履歴ファイルに `opened_at` を持たせた
- **理由**: 「履歴」には順序が内在するため最小限として許容した。これ以外の情報は持たせない

### [HIST:SPEC-DECISION:D-11] 2026-04-19 保存の既定デバウンスを 400ms とした
[HIST-TAG: category=SPEC-DECISION id=D-11 topics=D-11]
- **決定**: 保存の既定デバウンスを 400ms とした
- **理由**: 打鍵の自然な切れ目（日本語変換確定後）より十分長く、ユーザーが「消えるかも」と感じない十分短さの折衷

### [HIST:SPEC-DECISION:D-12] 2026-04-19 文脈ビューの前後節を既定 3 節 / 本文幅 44 全角字とした
[HIST-TAG: category=SPEC-DECISION id=D-12 topics=D-12,contents]
- **決定**: 文脈ビューの前後節を既定 3 節 / 本文幅 44 全角字とした
- **理由**: 指示 3「前後の節の表示幅を広げる」を具体値にしたもの。文庫本の 1 行 42–43 字に合わせた

### [HIST:SPEC-DECISION:D-13] 2026-04-19 `M` と `S`（検索結果）で ID 連番空間を共有させた
[HIST-TAG: category=SPEC-DECISION id=D-13 topics=D-13]
- **決定**: `M` と `S`（検索結果）で ID 連番空間を共有させた
- **理由**: 接頭辞ごとに連番を持つと、種別を変えたときに参照が壊れる。ID の不変性を優先した

### [HIST:SPEC-DECISION:D-14] 2026-04-19 `senses` / `address` / `when` をすべて「宣言」とし、本文の自動解析をしない
[HIST-TAG: category=SPEC-DECISION id=D-14 topics=D-14,contents]
- **決定**: `senses` / `address` / `when` をすべて「宣言」とし、本文の自動解析をしない
- **理由**: 形態素解析・意味解析を v1 の範囲外（U-5）としたことの帰結。作者のチェックリストとして設計した

### [HIST:SPEC-DECISION:D-15] 2026-04-19 新規の執筆支援診断を W / I のみとし、E を作らなかった
[HIST-TAG: category=SPEC-DECISION id=D-15 topics=D-15]
- **決定**: 新規の執筆支援診断を W / I のみとし、E を作らなかった
- **理由**: 文章の良し悪しを IDE が確定的に判定しないため（P4）

### [HIST:SPEC-DECISION:D-16] 2026-04-19 文体・品質系の診断を既定で無効にした
[HIST-TAG: category=SPEC-DECISION id=D-16 topics=D-16]
- **決定**: 文体・品質系の診断を既定で無効にした
- **理由**: 「書いている最中に言われたくない」種類の指摘であり、P3 に抵触するため。文体ワークスペースを開いたときだけ計算する

### [HIST:SPEC-DECISION:D-17] 2026-04-19 統計から完成予測・達成率・締切を除外した
[HIST-TAG: category=SPEC-DECISION id=D-17 topics=D-17]
- **決定**: 統計から完成予測・達成率・締切を除外した
- **理由**: 執筆は線形に進まない。予測値は作者を急かすだけで、執筆への集中という目的に反する

### [HIST:SPEC-DECISION:D-18] 2026-04-19 並列実行の既定を 2 にした
[HIST-TAG: category=SPEC-DECISION id=D-18 topics=D-18]
- **決定**: 並列実行の既定を 2 にした
- **理由**: 結果を人間が読める速度を超えさせないため。1 では意味が無く、大きすぎるとレビュー待ちが溜まる

### [HIST:SPEC-DECISION:D-19] 2026-04-19 提案モードを「人間用トラックチェンジ」ではなく「人間が使う提案機構」として設計した
[HIST-TAG: category=SPEC-DECISION id=D-19 topics=D-19,agents,modes]
- **決定**: 提案モードを「人間用トラックチェンジ」ではなく「人間が使う提案機構」として設計した
- **理由**: 既存の `agents/proposals/` を再利用し、新しい概念を作らないため（P3）

### [HIST:SPEC-DECISION:D-20] 2026-04-19 Git / CI・CD を解説リンクのみとした
[HIST-TAG: category=SPEC-DECISION id=D-20 topics=D-20,storage_policy]
- **決定**: Git / CI・CD を解説リンクのみとした
- **理由**: 実装するとプロジェクト外への書き込み（規則 10）と版管理の二重実装（U-7）に必ず繋がるため

### [HIST:SPEC-DECISION:U-1] 2026-04-19 バイナリ資料（PDF・画像）の保存 — 採用（ただし将来実装）
[HIST-TAG: category=SPEC-DECISION id=U-1 topics=U-1,materials]
- **対象事項**: バイナリ資料（PDF・画像）の保存
- **裁定**: **採用（ただし将来実装）**
- **v1 および将来への影響**: v1 では実装しない。資料本体は MD のみ。ただし §5.5.3 の `icon:` で `assets/` に PNG/JPEG を置く道は開いた。本格的なバイナリ資料は **付録 E F2** として設計する

### [HIST:SPEC-DECISION:U-2] 2026-04-19 共同編集・マージ — 却下
[HIST-TAG: category=SPEC-DECISION id=U-2 topics=U-2]
- **対象事項**: 共同編集・マージ
- **裁定**: **却下**
- **v1 および将来への影響**: v1 でも将来でも実装しない。ファイルベースなので VCS に委ねる（§14.2.1 の Git 解説リンク）。複数人での同時編集を前提にした機能（ロック・コメント・プレゼンス）を一切作らない（MUST）

### [HIST:SPEC-DECISION:U-3] 2026-04-19 実際の LSP サーバ実装（`novnow lsp`） — 保留
[HIST-TAG: category=SPEC-DECISION id=U-3 topics=U-3,cli]
- **対象事項**: 実際の LSP サーバ実装（`novnow lsp`）
- **裁定**: **保留**
- **v1 および将来への影響**: v1 ではインターフェースの準備のみ（指示 13 の明文）。`core/features/*` の 11 機能が純粋関数で揃っていることを維持し、将来の実装を妨げない。サブコマンド `lsp` は §14.1 の一覧に **入れない**

### [HIST:SPEC-DECISION:U-4] 2026-04-19 縦書きの印刷 / PDF 出力 — 付録 E F4 で上書き
[HIST-TAG: category=SPEC-DECISION id=U-4 topics=U-4,reader,export]
- **対象事項**: 縦書きの印刷 / PDF 出力
- **裁定**: **付録 E F4 で上書き**
- **v1 および将来への影響**: v1 ではエクスポート（テキスト）まで。将来の設計は F4 を参照

### [HIST:SPEC-DECISION:U-5] 2026-04-19 役割語の形態素解析ベース判定 — 保留
[HIST-TAG: category=SPEC-DECISION id=U-5 topics=U-5,role_language]
- **対象事項**: 役割語の形態素解析ベース判定
- **裁定**: **保留**
- **v1 および将来への影響**: v1 は語クラスの正規表現で代替（§6.5.1）。この裁定の帰結として、§9.4.3 の句切れ提案と §9.4.4 の反復検出も **形態素解析を使わない決定論的な手がかり** のみで実装する（MUST）。将来の設計は付録 E F1

### [HIST:SPEC-DECISION:U-6] 2026-04-19 モバイル / タブレット UI — 却下
[HIST-TAG: category=SPEC-DECISION id=U-6 topics=U-6]
- **対象事項**: モバイル / タブレット UI
- **裁定**: **却下**
- **v1 および将来への影響**: v1 でも将来でも実装しない。**キーボード以外の入力手段をサポートしない**（MUST）。タッチ・ペン・ジェスチャ・マウスジェスチャのためのコードを書かない。レスポンシブレイアウトも狭画面向けの特別対応も行わない

### [HIST:SPEC-DECISION:U-7] 2026-04-19 プロンプトの版管理 — 確定: 版管理は一切行わない。既定は不変の場所に保管し、いつでも復元可能にする。v1 に追加
[HIST-TAG: category=SPEC-DECISION id=U-7 topics=U-7]
- **対象事項**: プロンプトの版管理
- **裁定**: **確定: 版管理は一切行わない。既定は不変の場所に保管し、いつでも復元可能にする。v1 に追加**
- **v1 および将来への影響**: §15.5.4 に全面実装。既定プロンプトはバイナリ埋め込み。`.bak-*` 退避は **撤廃**。履歴・世代・差分適用を持たない

### [HIST:SPEC-DECISION:U-8] 2026-04-19 エージェントの並列実行 — 確定: v1 に追加
[HIST-TAG: category=SPEC-DECISION id=U-8 topics=U-8,agents]
- **対象事項**: エージェントの並列実行
- **裁定**: **確定: v1 に追加**
- **v1 および将来への影響**: §15.7.5 に実装。並列化するのは **実行のみ**。自律の度合いは委譲レベル（§15.1.1）が決める。原則 A1 を「委譲レベルは人間が設定する」に改めた

---

## 7. 将来機能提案と v1 拡張点の確認アーカイブ（`FUTURE`: 旧付録 E `F1`〜`F12`）

### [HIST:FUTURE:F1] 2026-04-19 形態素解析による文体解析
[HIST-TAG: category=FUTURE id=F1 topics=F1,agents,dictionary,web]
- **概要**: 辞書ベースの形態素解析器を載せ、品詞・活用・係り受けの情報を使って文体を解析する
- **できるようになること**: 「実際に嗅覚描写があるか」の判定（§5.4.4 の宣言に対する実測）、語クラス `(体言)` の精密化、句切れ提案の精度向上、同語反復の語幹単位での検出
- **v1 側の受け皿**: `project.json.lexicon.word_classes` が正規表現で定義されている（§6.5.1）。ここを解析器の品詞タグに差し替えられる。§9.4.4 の算法は「解析器が無い場合の代替」として明記してある
- **障害**: 辞書のサイズ（数十 MB）が wasm ビルドに乗らない。ライセンス。`core` に重い依存を入れると §3 の層構造が崩れる
- **設計の方向**: 解析器を **MCP サーバとして外に出す**。`core` は品詞タグ列を受け取るだけのインターフェースを持つ。これなら層構造を保ったまま載せられる

### [HIST:FUTURE:F2] 2026-04-19 バイナリ資料（U-1 の将来実装）
[HIST-TAG: category=FUTURE id=F2 topics=F2,contents,materials,web]
- **概要**: PDF・画像・音声を資料として保存し、参照できるようにする
- **配置**: `materials/assets/M0137/` 配下。`index.json` の `path` が MD を指し、MD から `![](../assets/M0137/map.png)` で参照する
- **表示**: 参照ペインではサムネイル（最大 96px）のみ。資料ワークスペースで原寸表示
- **v1 側の受け皿**: `assets/` ディレクトリと `icon:` の読み込み規則（§5.5.3）が、プロジェクト内バイナリの扱いの雛形になっている。Zip 入出力（§13.4）は既にバイナリを通す
- **障害**: Web 版の IndexedDB 容量。PDF のレンダリング（外部クレートが必要で、`core` に入れられない）。Zip サイズの肥大
- **制約案**: 1 ファイル 8MB・プロジェクト合計 256MB の上限。PDF は **1 ページ目のみ** をサムネイル化し、本文は外部ビューアに委ねる

### [HIST:FUTURE:F3] 2026-04-19 執筆リズムの可視化（キーストローク解析）
[HIST-TAG: category=FUTURE id=F3 topics=F3,contents]
- **概要**: 打鍵の間隔・削除率・推敲回数から「筆が乗っている時間帯」を可視化する
- **v1 との距離**: **遠い**。§4.4.2 で「イベントログを作らない」を MUST にしたため、v1 の `stats.json` には必要なデータが無い
- **慎重に扱うべき理由**: 打鍵ログは本文と同等の機微情報である。novnow の価値は「作家の原稿を人質に取らない」ことにあり、詳細な行動ログの保存はこれと緊張する
- **設計の方向**: 保存するなら **セッション単位の統計値だけ**（平均打鍵間隔・削除率）を集計し、生のイベントは **メモリ内で破棄** する。既定オフ・明示的な同意を必須にする

### [HIST:FUTURE:F4] 2026-04-19 縦書き PDF・印刷出力（U-4 の上書き）
[HIST-TAG: category=FUTURE id=F4 topics=F4,reader,modes,export,web]
- **概要**: 読者モードの組版結果を、そのまま PDF / 印刷に出せるようにする
- **できるようになること**: 同人誌の入稿データ作成、紙に出して推敲する、商業出版の初稿
- **前提**: **追加仕様 27（読者モード）の実装が先**。§17.3.3 の組版を `core::reader::layout()` が純粋関数として「文字と座標の列」で返す設計は仕様として確定している（§3.2）ため、27 が入れば PDF 出力は **この座標列を PDF の描画命令に変換するだけ** で実装できる。UI を経由しない
- **追加で要るもの**: フォント埋め込み（サブセット化）、トンボ・塗り足し、ページ番号の体裁選択、見開き対応
- **障害**: フォントのライセンス（埋め込み可否）。Web 版でのフォントバイナリ取得
- **設計の方向**: `core::export::pdf(layout: &[Glyph], opts) -> Vec<u8>` を追加。組版ロジックは **一切変更しない**（読者モードと印刷で見た目が違わないことを保証する）
- **非目標**: DTP 機能（任意の版面設計、画像の回り込み）は持たない。それは InDesign の仕事である

### [HIST:FUTURE:F5] 2026-04-19 複数作品の横断管理
[HIST-TAG: category=FUTURE id=F5 topics=F5,words,dictionary,storage_policy,web]
- **概要**: シリーズ物で、複数プロジェクトの用語・キャラを共有する
- **v1 との衝突**: **規則 10（プロジェクト外に出てよいのは履歴パスのみ）と正面から衝突する**。安易に実装してはならない
- **設計の方向**: プロジェクト外に共有辞書を置くのではなく、**「親プロジェクト」をディレクトリの親子関係で表現** する。`project.json` に `extends: "../shared"` を書き、**相対パスが親ディレクトリを 1 段だけ遡ることを許す** 例外を規則 10 に追加する。書き込みは依然として自プロジェクト内のみ
- **残る課題**: Web 版（IndexedDB）でのディレクトリ親子関係の表現。ID 空間の衝突

### [HIST:FUTURE:F6] 2026-04-19 読者からのフィードバック取り込み
[HIST-TAG: category=FUTURE id=F6 topics=F6,materials,agents]
- **概要**: 投稿サイトの感想・ブックマーク数を取り込み、節と紐づける
- **v1 側の受け皿**: `materials/` と `S####`（検索結果の資料化、§16.3.1）が「外部情報を凍結して保存する」形を既に持っている。感想も同じ器に入る
- **設計の方向**: 取り込みは **MCP サーバ経由**（novnow 本体が投稿サイトの API を叩かない）。取り込んだものは `category: "感想"` の資料として `materials/inbox/` に入り、人間が承認する
- **慎重に扱うべき理由**: 数字（PV・★）を執筆画面の近くに置くと、書く手が止まる。**`write` ワークスペースには絶対に出さない**（MUST にすべき）

### [HIST:FUTURE:F7] 2026-04-19 音声入力・読み上げ
[HIST-TAG: category=FUTURE id=F7 topics=F7,contents,modes]
- **概要**: 音声で口述筆記する / 本文を読み上げて耳で推敲する
- **U-6 との関係**: 「キーボード以外の入力手段をサポートしない」という裁定があるため、**音声入力は却下の範囲に入る**
- **読み上げは別**: **読み上げ（出力）は入力手段ではない** ため、U-6 の却下に抵触しない。耳で聞くと不自然な文が見つかるのは実際に有効である
- **設計の方向**: 読み上げのみを将来検討する。OS の TTS を使い、読者モード（追加仕様 27）で「現在の節から読む」。ルビがある箇所はルビを読む。`icon` と同じく **本文エディタには出さない**

### [HIST:FUTURE:F8] 2026-04-19 空間設計の図形エディタ
[HIST-TAG: category=FUTURE id=F8 topics=F8,materials,modes]
- **概要**: §5.3.6 の `space:` を、テキストではなく図として編集する
- **U-6 との関係**: 図形エディタは本質的にポインタ操作を要求する。**キーボードのみで間取りを描くのは現実的でない**
- **結論**: **U-6 の却下に実質的に含まれる**。v1 の `space-mode: ascii`（手で描いた AA を等幅で表示するだけ）が、この要求に対する novnow の回答である
- **代案**: 外部の図形ツールで描いた結果を、テキスト（AA）または F2 の画像資料として取り込む

### [HIST:FUTURE:F9] 2026-04-19 プロット構造のテンプレート
[HIST-TAG: category=FUTURE id=F9 topics=F9,agents]
- **概要**: 三幕構成・起承転結・序破急などの型を、A 形式のブロック雛形として挿入する
- **v1 側の受け皿**: A 形式は `summary` 以外すべて任意のため、雛形は **単なるテキスト挿入** で実現できる。新しい仕組みが要らない
- **設計の方向**: `prompts/` と同じく、`templates/` をプロジェクト内に置き、既定をバイナリに埋め込む（§15.5.4 と同じ方式）。コマンドパレットから `>template 三幕構成` で挿入
- **慎重に扱うべき理由**: 型を押し付けると作品が画一化する。**既定で何も挿入しない**。使いたい人が明示的に呼ぶ

### [HIST:FUTURE:F10] 2026-04-19 差分レビューの履歴
[HIST-TAG: category=FUTURE id=F10 topics=F10,contents]
- **概要**: 「3 日前の版と比べる」を novnow 内で行う
- **U-7 との関係**: **U-7 の裁定（版管理は一切行わない、Git に丸投げ）が、この項目にも及ぶ**
- **結論**: novnow は版の履歴を持たない。この要求は Git が既に完全に満たしている
- **novnow にできること**: §14.2.1 の Git 解説ページで、「`git diff HEAD~3 -- contents/01-04.txt` で 3 日前と比べられる」ことを **具体的なコマンド付きで** 説明する。これは機能ではなくドキュメントの仕事である

### [HIST:FUTURE:F11] 2026-04-19 エージェントの成果の学習（プロンプトの自動改善）
[HIST-TAG: category=FUTURE id=F11 topics=F11,agents]
- **概要**: 採用／却下の履歴から、プロンプトを自動改善する
- **P5 との関係**: **委譲の範囲（プロンプトを含む）を自分で決めるのは人間の仕事である**（P5）。novnow がユーザーの知らないうちにプロンプトを書き換える機能は、ユーザーのコントロールを奪う
- **結論**: **自動では実装しない**
- **代わりにやること**: プロンプト改善の下書きは、ユーザーがエージェントに依頼する **通常のタスク** として実行できる（出力は §15.1.1 の委譲レベルに従う）。却下コメントは既に ContextPacket の `9 history` に入り（§15.4.1）、人間がそれを読んで判断する。v1 はその材料を揃えることに徹する

### [HIST:FUTURE:F12] 2026-04-19 多言語の原稿
[HIST-TAG: category=FUTURE id=F12 topics=F12,reader,contents,characters,role_language,modes]
- **概要**: 同じ作品の日本語版と英語版を並行して持つ
- **v1 との距離**: UI の多言語化（指示 1）は済んでいるが、**原稿の多言語化は別物**
- **設計の方向**: `contents/` と並べて `contents.en/` を置き、SceneId で対応させる。節単位で対訳表示できる（文脈ビューを左右 2 列にする）
- **障害**: 役割語（§6.5）は日本語固有の概念であり、英語版では意味をなさない。`characters.md` を言語ごとに分けるか、`tone` セクションを言語別にするかの判断が要る
- **縦書き**: 英語版では縦書きが無意味なので、読者モード（追加仕様 27）を言語ごとに切り替える必要がある
- **優先度**: 低い。翻訳は専用ツール（CAT ツール）の領域であり、novnow が中途半端に持つ価値は小さい

### [HIST:FUTURE:E-1] 2026-04-19 将来機能（F1〜F12）を v1 が妨げていないことの確認表
[HIST-TAG: category=FUTURE id=E-1 topics=future_extensions,seams,F1_F12]
- **F1 形態素解析**: 拡張点=`lexicon.word_classes` / MCP 経由の外部解析 ／ 判定=✔ 層構造を壊さずに載る
- **F2 バイナリ資料**: 拡張点=`assets/` / `materials/` / Zip がバイナリを通す ／ 判定=✔
- **F3 打鍵解析**: 拡張点=（意図的に受け皿を作っていない） ／ 判定=△ 慎重に扱うべき領域として明示
- **F4 縦書き PDF**: 拡張点=`core::reader::layout()` が純粋関数で座標列を返す（**追加仕様 27 の実装が前提**） ／ 判定=✔ 27 の後なら最も素直に載る
- **F5 横断管理**: 拡張点=（規則 10 の例外追加が必要） ／ 判定=△ 規則の改定を伴う
- **F6 感想取り込み**: 拡張点=`materials/inbox/` と `S####` の器 ／ 判定=✔
- **F7 読み上げ**: 拡張点=`core::reader::layout()` の文字列（追加仕様 27 が前提） ／ 判定=✔（入力は U-6 で却下）
- **F8 図形エディタ**: 拡張点=`space-mode: ascii` ／ 判定=✖ U-6 に実質的に含まれ、却下
- **F9 テンプレート**: 拡張点=A 形式の任意フィールド / 埋め込み既定の方式 ／ 判定=✔
- **F10 差分履歴**: 拡張点=（Git に委ねる） ／ 判定=✖ U-7 により実装しない
- **F11 プロンプト自動改善**: 拡張点=（P5: 委譲範囲は人間が決める） ／ 判定=✖ 自動では実装しない（依頼ベースのタスクは可）
- **F12 多言語原稿**: 拡張点=SceneId による対応付け ／ 判定=△ 役割語の扱いに未解決の設計課題

---

## 8. 段階 ①（開発指示 1〜13）検収条件と達成アーカイブ（`ACCEPTANCE`）

### [HIST:ACCEPTANCE:IND-01] 2026-10-14 指示 1: ラベル撤廃 / 2 言語 / テーマ（旧 §19.1 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-01 topics=IND-01,i18n,theme,testing]
- [x] 画面上に「ファイル」「エディタ」「情報」という文字が存在しない（UI 文字列の grep テスト）。
- [x] §9.2.2 の撤廃一覧がすべて反映されている。
- [x] `ja.toml` と `en.toml` のキー集合・プレースホルダが完全一致する（`tests/i18n_catalog.rs`）。
- [x] `ui/` 内に i18n を経由しない日本語リテラルが存在しない（grep テスト。例外は i18n カタログのみ）。
- [x] `Ctrl+Alt+L` で全 UI 文言が即座に切り替わる。再起動不要。
- [x] `Ctrl+Alt+T` で `system → dark → light` と切り替わり、OS 設定変更に 1 秒以内に追従する。
- [x] 全トークンのコントラスト比が §9.9 の基準を満たす（テーマのユニットテストで計算）。

### [HIST:ACCEPTANCE:IND-02] 2026-10-14 指示 2: ストレージと起動（旧 §19.2 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-02 topics=IND-02,storage_policy,web,ui]
- [x] Web 版が LocalStorage を使っていない（grep + 実機確認）。
- [x] `projects` / `files` / `app` の 3 ストアが仕様どおり作られる。
- [x] 初回起動で `sample` が 1 件だけ投入され、モーダルに 1 件だけ表示される。自動で開かない。
- [x] プロジェクト未選択では背後の UI が描画されない。`Esc` で閉じない。
- [x] `novnow ./valid-project` で GUI が起動する。
- [x] `novnow ./broken` が §14.2.2 の文面を stderr に出し、終了コード 3 で終了し、**ウィンドウを 1 つも作らない**。
- [x] 出力に `PROJECT_STRUCTURE_DOC_URL` と `novnow new` が含まれる。
- [x] Windows で `.exe` を直接ダブルクリックしてもコンソールウィンドウが出ず、GUI が起動する。
- [x] GUI 起動時にプロジェクト選択モーダルが出て、履歴サジェストとフォルダ選択の両方が使える。

### [HIST:ACCEPTANCE:IND-03] 2026-10-14 指示 3: 編集体験（旧 §19.3 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-03 topics=IND-03,i18n,contents,web,ui,testing]
- [x] ファイルツリー／ディレクトリプレビューの UI が存在しない。
- [x] 文脈ビューの本文カラム幅が全角 44 字を確保できる（1280px 幅のウィンドウで確認）。
- [x] 編集中の節の位置に本文が描画されない（プレースホルダのみ）。
- [x] `Alt+↓` で節を移動した瞬間に保存が走る（ファイル mtime / IDB レコード更新で確認）。
- [x] 保存に関する文言・ボタンが、ステータスバーのインジケータと終了ガード以外に存在しない。
- [x] 未保存状態のときのみ「閉じないで」アイコンが出る。
- [x] 参照ペインに `Frame::group` が使われていない（grep テスト）。
- [x] マウスを一切使わずに、別の話のファイルを開いて 1 節書いて戻ってこられる（手動シナリオ）。

### [HIST:ACCEPTANCE:IND-04] 2026-10-14 指示 4: 構文ハイライト（旧 §19.4 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-04 topics=IND-04,lexer,diagnostics,contents,dictionary,web]
- [x] `'` の後が最短一致で着色される（`tests/dictionary.rs`: 辞書に `リリア`/`リリアナ` 両方があるとき `リリア` が選ばれる）。
- [x] `@` と `'` のシジル自体が本体と別の色になる。
- [x] `「」` と `;` に専用色が付く。
- [x] 未解決参照に赤波線と E001 が出る。
- [x] ハイライトが本文ビュー（文脈ビュー）と節エディタの両方で効く。
- [x] **IME 変換中（§9.8.5）に `Alt+1` / `Enter` / `Esc` / 矢印キーを押しても、ワークスペースが切り替わらず、 節が確定せず、モーダルが閉じない。native と Web の両方で確認する。**
- [x] **変換候補ウィンドウがキャレットの直下に出る（§9.8.5-5 の `IMEOutput` が出力されている）。**

### [HIST:ACCEPTANCE:IND-05] 2026-10-14 指示 5: 用途別 UI（旧 §19.5 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-05 topics=IND-05,ideas,role_language,modes]
- [x] `Alt+1..9` の 9 ワークスペースが存在し、切替で SceneId が維持される。
- [x] `plot-episode` の上段に親（章）プロットが常に表示される。
- [x] `idea` はどのワークスペースからでも `Ctrl+Shift+I` で 1 行捕捉できる。
- [x] `cast` にモードタイムラインと役割語テスターがある。

### [HIST:ACCEPTANCE:IND-06] 2026-10-14 指示 6: モードと役割語（旧 §19.6 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-06 topics=IND-06,diagnostics,characters,role_language,modes,testing]
- [x] `characters.md` の `## modes` が読める。同一 ID・異 range の変種が解決される。
- [x] 話プロットの `mode:` で節/場面にモードを割り当てられる。
- [x] 参照ペインに **有効モードの性格だけ** が出る（他モードが出ないことをテスト）。
- [x] 1 キャラ 3 モードまたは同一キャラ重複で E005、範囲外で E004 が出る。
- [x] 役割語が一致した台詞の **リテラル部分のみ** に緑下線が付く。
- [x] 一致しない台詞には何も表示されない（色も診断も付かないことをテスト）。
- [x] 付録 B の 14 キャラ分のパターンがすべて期待どおりに一致する（表駆動テスト）。

### [HIST:ACCEPTANCE:IND-07] 2026-10-14 指示 7: `characters.md`（旧 §19.7 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-07 topics=IND-07,characters]
- [x] `characters.json` を参照するコードが存在しない。
- [x] `characters.md` のパーサが、意味を持つ 4 種の行だけを解釈し、それ以外を無損失で保持する。
- [x] 不正な構文でも落ちず、診断として報告する。

### [HIST:ACCEPTANCE:IND-08] 2026-10-14 指示 8: AI 環境（旧 §19.8 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-08 topics=IND-08,diagnostics,contents,agents,storage_policy,testing]
- [x] `prompts/` が無いプロジェクトを開くと既定 7 本が生成され、I050 が出る。
- [x] 既存の `prompts/*.md` が上書きされない。
- [x] プロンプトがプロジェクト外から読み込まれない（`tests/storage_policy.rs`）。
- [x] ContextPacket が §15.4.1 の順序・見出しで生成され、予算超過時に §15.4.3 の順で切り詰められる（決定論テスト）。
- [x] `5 cast` と `3 plot(当該節)` と `8 constraints` が切り詰め対象にならない。
- [x] ドライランでパケット全文を確認でき、ブロック単位で除外できる。
- [x] 委譲レベル `propose` では、エージェントの出力が `agents/proposals/` に入り、採用するまで本文が変わらない。
- [x] 委譲レベル `apply` / `autonomous` では、出力が自動適用され、before/after 記録が `agents/proposals/` と監査ログに残る。
- [x] 委譲レベルが UI（§15.7.6）から確認・変更でき、変更が監査ログに `delegation_changed` として残る。
- [x] 委譲レベルの変更が `1`〜`4` キーとコマンドパレット（`>agent delegation <role> <level>`）の両方からでき、 `Ctrl+Shift+X` / `>agent stop-all` で総停止できる（§15.7.4・§15.7.6）。
- [x] MCP サーバの `novnow.section.write` / `novnow.plot.write` が `role` 引数の委譲レベルに従い、 `apply` / `autonomous` 未満または `role` 省略時は E036 を返す（§15.8.3）。
- [x] `state: fixed` への自動書き換え時にステータスバーへ 1 行通知し、監査ログに `fixed_overwrite` が残る（A7）。
- [x] integrator 以外の役割は、委譲レベルが `autonomous` でも他のタスクを起こせない（コードレビュー項目）。
- [x] LLM 呼び出しが MCP 経由のみである（プロバイダ SDK の依存が `Cargo.toml` に無い）。
- [x] 委譲レベル `propose` では、エージェント間メッセージが人間の承認なしに相手のコンテキストに入らない。
- [x] `autonomous` でもセッション上限（`agents.limits`）に達すると自動停止し、W095 が出る。

### [HIST:ACCEPTANCE:IND-09] 2026-10-14 指示 9: 資料（旧 §19.9 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-09 topics=IND-09,diagnostics,materials]
- [x] ID は単調増加し、却下しても再利用されない。
- [x] プロットに `'M0137` と書くとタイトルがインレイヒントで出る（ファイル内容は変わらない）。
- [x] 明示タイトルの不一致で赤波線 E006 とクイックフィックス 3 種が出る。
- [x] `Ctrl+M` でタイトル・カテゴリ・条件・ID のいずれからでも資料に到達できる。
- [x] 条件一致した資料が参照ペインに候補として出る。

### [HIST:ACCEPTANCE:IND-10] 2026-10-14 指示 10: プロジェクト外への書き込み禁止（旧 §19.10 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-10 topics=IND-10,diagnostics,export,storage_policy,web]
- [x] `tests/storage_policy.rs` が、全書き込み経路でプロジェクト外パスを拒否することを検証する。
- [x] 実機で novnow を一通り操作した後、プロジェクト外に作られたファイルが **履歴ファイル 1 個と、ユーザーが明示的に指定した書き出し先のファイル（保存ダイアログ・`novnow export --out`）だけ** である（`strace`/ファイル差分による手動確認手順を用意。特例は §13.1.1。第 2.10 版、第 2.11 版でテキストを足した）。
- [x] 保存ダイアログや `--out` でプロジェクト外の `.zip`（Zip）・`.txt` / `.md`（テキスト）を指定したとき、その 1 個のほかに（一時ファイルを含めて）何も作られない。種類に合わない拡張子のパスを指定すると E060 で書き込まない。
- [x] 履歴ファイルの中身が `path` と `opened_at` のみである。
- [x] Web の `app` ストアに `recent_projects` 以外のキーが無い。

### [HIST:ACCEPTANCE:IND-11] 2026-10-14 指示 11: 4 段階（旧 §19.11 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-11 topics=IND-11,plot_outline,contents]
- [x] `plots/all.txt` と `plots/NN.txt` が同じパーサ（A 形式）で読める。
- [x] `plots/NN-MM.txt` が B 形式、`contents/NN-MM.txt` が C 形式で読める。
- [x] `plot-outline` → `plot-episode` → `write` と降りるとき、常に親が画面上に見えている。
- [x] `writer` の ContextPacket で、全体プロットが 1 行（`summary`）に圧縮されている。

### [HIST:ACCEPTANCE:IND-12] 2026-10-14 指示 12: エクスポート（旧 §19.12 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-12 topics=IND-12,reader,modes,export]
- [x] エクスポート結果に `@` 行・`;` 行・`'`・`//` が残らない（`tests/export_invariants.rs`）。
- [x] 範囲指定（全体 / 章 / 話 / 場面 / 節 / 状態フィルタ）が機能する。
- [x] ルビ記法 `｜base《ruby》` が `keep_ruby` に従って保持 / 除去される。
- [x] エクスポート結果を外部の縦書きビューアで読み返せる（読者モードの代替手段が成立していることの確認）。

### [HIST:ACCEPTANCE:IND-13] 2026-10-14 指示 13: ドメイン分離（旧 §19.13 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-13 topics=IND-13,web,ui]
- [x] `tests/layering.rs` が通る（`core/` に egui・fs・web_sys・tokio が無い）。
- [x] §3.4 の 11 機能が `core::features::*` に純粋関数として存在し、UI から呼ばれている。
- [x] 位置変換（byte ⇔ char ⇔ UTF-16）が `core::text::position` に閉じている。
- [x] クレート分割や `core/` 内での `cfg` による条件コンパイルを **行っていない**（指示 13 の明文。OS・ターゲット別の分岐は `adapters/`・`ui/`・`main.rs` に閉じる）。

### [HIST:ACCEPTANCE:IND-14] 2026-10-14 v1 追加仕様（第 2 次指示）の受け入れ基準（旧 §19.14 検収完了）
[HIST-TAG: category=ACCEPTANCE id=IND-14 topics=IND-14]


---

## 9. 段階 ① リスク台帳アーカイブ（`RISK`: `R-1`〜`R-10`）

### [HIST:RISK:R-1] 2026-10-09 縦書きレイアウト（§17.3.3）が egui で素直に描けない
[HIST-TAG: category=RISK id=R-1 topics=R-1,reader,modes,ui]
- **影響 / 確度 / 判断期限**: — / — / クローズ
- **対策と結果**: **解消（2026-10-09）**。読者モードを追加仕様 #27 として ② へ移管したため、① にこのリスクは無い。② の計画を立てる時点で再度リスクとして起票すること

### [HIST:RISK:R-2] 2026-10-09 IME 変換中の判定（§9.8.4）が egui 0.27 で取れない
[HIST-TAG: category=RISK id=R-2 topics=R-2,agents,web,ui]
- **影響 / 確度 / 判断期限**: — / — / クローズ
- **対策と結果**: **解消（2026-10-09・一次情報で確認）**。egui 0.27.2 には `Event::CompositionStart` / `CompositionUpdate` / `CompositionEnd` があり、native（egui-winit）でも Web（eframe text agent）でも発火する。実装規約を `SPECIFICATIONS.md` §9.8.5 に固定した。**egui を上げる必要は無い**

### [HIST:RISK:R-2p] 2026-10-09 自前エディタで `PlatformOutput.ime` を出し忘れ、native で日本語入力そのものができない
[HIST-TAG: category=RISK id=R-2p topics=R-2p,ui]
- **影響 / 確度 / 判断期限**: 致命的だが原因が分かりにくい / 中 / M7
- **対策と結果**: egui-winit 0.27.2 は `ime` が `Some` のフレームしか `set_ime_allowed(true)` を呼ばない。§9.8.5-5 のとおり `IMEOutput { rect, cursor_rect }` を毎フレーム出す。**M7 の最初の疎通で確認する**

### [HIST:RISK:R-3] 2026-10-09 egui のバージョンアップ圧力
[HIST-TAG: category=RISK id=R-3 topics=R-3,ui]
- **影響 / 確度 / 判断期限**: 全 UI の書き直し / 低 / M0
- **対策と結果**: **① では 0.27.2 に固定**。R-2 が解消したので上げる動機は無くなった。上げる場合は **0.29.x を飛ばす**（Linux IME 回帰 #5544）

### [HIST:RISK:R-4] 2026-10-09 `characters.md` の「曖昧さのない MD」が実際には曖昧
[HIST-TAG: category=RISK id=R-4 topics=R-4,characters]
- **影響 / 確度 / 判断期限**: パーサの手戻り / 中 / M2 前半
- **対策と結果**: M2 で §5.5.7 の例を**最初に**通す。実際に手で書いてみて違和感があれば仕様を直す（仕様が正本なので仕様側を更新する）

### [HIST:RISK:R-5] 2026-10-09 最短一致（§6.2.2）が実用で苛立たしい
[HIST-TAG: category=RISK id=R-5 topics=R-5,dictionary]
- **影響 / 確度 / 判断期限**: 体験の根幹が崩れる / 中 / M7
- **対策と結果**: M7 の内部 α で実文章を書いて検証（I7）。補完（§9.7.3）が十分に効けば問題にならない見込み

### [HIST:RISK:R-6] 2026-10-09 IndexedDB の wasm 実装が重い
[HIST-TAG: category=RISK id=R-6 topics=R-6,storage_policy,web]
- **影響 / 確度 / 判断期限**: M5 が膨らむ / 中 / M5
- **対策と結果**: 生の web-sys は冗長。`adapters/storage_idb.rs` に閉じ込め、Promise ラッパを 1 本作って使い回す

### [HIST:RISK:R-7] 2026-10-09 MCP の仕様理解不足
[HIST-TAG: category=RISK id=R-7 topics=R-7,agents]
- **影響 / 確度 / 判断期限**: M10 が膨らむ / 中 / M10 開始時
- **対策と結果**: M10 着手前に stdio JSON-RPC の最小実装でエコーサーバと疎通する（0.5 日のスパイク）

### [HIST:RISK:R-8] 2026-10-09 122 日の見積りが楽観的
[HIST-TAG: category=RISK id=R-8 topics=R-8]
- **影響 / 確度 / 判断期限**: 納期 / 高 / M7
- **対策と結果**: **M7 完了（75 日）を中間ゲート**にする。ここで実績を見て M8 以降を再見積りする

### [HIST:RISK:R-9] 2026-10-09 ② の 26 件が ① の設計を揺らす
[HIST-TAG: category=RISK id=R-9 topics=R-9]
- **影響 / 確度 / 判断期限**: 手戻り / 中 / 常時
- **対策と結果**: 本書 §7 の縫い代 10 項目をレビュー項目にする。PR テンプレに「縫い代チェック」を入れる

### [HIST:RISK:R-10] 2026-10-09 core の純粋性が UI 実装時に破れる
[HIST-TAG: category=RISK id=R-10 topics=R-10]
- **影響 / 確度 / 判断期限**: 層構造の崩壊 / 中 / 常時
- **対策と結果**: `tests/layering.rs` が CI 必須。例外を 1 つも認めない


### [HIST:DECISION:L01-STAGE2] 2026-10-21 段階②の統合実装判断
[HIST-TAG: category=DECISION id=L01-STAGE2 topics=migration,reader,statistics,mcp,stock]

- 全判断を自律実行して事後報告するというユーザー指示に従い、段階②の詳細を裁定した。
- Reader の「ファイルを変更しない」は原稿・プロット・資料・共有設定に適用し、要求された読書位置・しおりのローカル保存は例外とした。
- ローカル MCP は平文 HTTP の loopback のみ。外部 URL・URL 内の制御文字を拒否し、プロバイダ SDK は導入しない。
- 統計は公開済みの日次 `chars` / 時刻集合 `hours` を維持し、集計時間・セッション数と未知キーを保持する。10分または終了時にだけ保存する。
- 構造移動はネイティブでは全変更前のバイト列を確保して失敗時に関門経由で復元し、Web は単一 IndexedDB トランザクションとする。電源断を含む分散トランザクションではない。
- JPEG は既存 image 依存の codec を有効化し、ネットワーク制約環境の復旧は `REF:scripts/bootstrap-jpeg.sh` で検証済みソースを補う。


### [HIST:DECISION:STAGE2-INPUT] 2026-10-21 読者からエディタへの切替での文字イベント漏れ
[HIST-TAG: category=DECISION id=STAGE2-INPUT topics=web,shortcuts,readonly]

- 読者のキーボード操作とバイナリ画像の Zip 往復を `REF:scripts/web-smoke.mjs` に追加し、読者から資料へ移ったときの Alt+数字の随伴 Text イベント混入を検出した。
- `REF:src/ui/shell.rs` で消費したショートカットに対応する文字だけを同時に消費し、無関係な入力と IME 確定を残す。`REF:tests/stage_two.rs` で固定した。


### [HIST:MILESTONE:L01-STAGE2-COMPLETE] 2026-10-22 L-01・段階②の実装検証完了
[HIST-TAG: category=MILESTONE id=L01-STAGE2-COMPLETE topics=migration,stage2,verification]

- PR #23 のコミット `73efa9e` で CLI・デスクトップの L-01 と追加仕様 #1〜27 を統合した。判断の記録は本書 L01-STAGE2 と STAGE2-INPUT、対応・検収表は `REF:docs/plans.md` §3 / §4。
- 同コミットで `REF:scripts/verify.sh` は 1268 成功・0 失敗（56 ターゲット）、`REF:scripts/web-smoke.sh` は全項目成功。release の打鍵・一覧 16ms 条件と読者組版 3ms 条件も通過した。
- GitHub Actions run 39625763551 の Docs & Change Filter / Rustfmt / Clippy / Test / WASM Build Check / Benchmarks はすべて成功した。この記録は実装コミットの結果であり、将来のコミットの成功を保証するものではない。

### [HIST:FIX:DOCS-MERGE-STAGE23] 2026-10-22 段階②・③文書のマージ重複を解消
[HIST-TAG: category=FIX id=DOCS-MERGE-STAGE23 topics=docs,merge,stage2,stage3]

- PR #24 を含む main の取り込み（`509e564`）で Accept both changes により、段階②実装済みと未着手の進捗行、旧ブランチの引き継ぎ、二重の手順が混在した。片方を丸ごと採用せず、段階②の実装状態と段階③の文書・計画を両立させた。
- `REF:docs/handoff.md` と `REF:docs/plans.md` の共有表を一意にし、11 文書対・134 節（完了126 / 計画済み8）に揃えた。固定の文書管理規則と段階③の H1〜H7 は保持した。
- `REF:docs/script/duplicate.py` に共有表の区分重複・集計行単独一致・設計書数の検査を追加し、再発ケースを `REF:docs/script/test_duplicate.py` に固定した。
- 段階②の GUI 提供と、段階③の API 提供を区別した。操作 ID の追加を無承認で決めず、統合後のカタログ追補を `REF:docs/plans.md#stage-3` の H1 前提作業として明示した。
