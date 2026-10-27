# 開発計画（Plans）

本書は現在の進捗と段階③（§19 外部インターフェース）の実装計画を記録する。L-01・段階②の対応表と検収表は §3・§4 に、完了済みの意思決定・検証履歴は `REF:docs/history.md` に置く。

---

現在のマイルストーンは **v0 次版（Issue #29 の実施）**。v0.1.0 は配布済みであり、次版はファイル形式の非互換な更新（`project.toml` / `.cnov`）を含む（§8）。v1 への到達は当面行わず、開発は v0 で継続する（版間互換の担保はしない、§20.6.2）。公開前のゲートは `REF:docs/distribution.md` §8〜§10 に集約する。

## 1. 全体進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 9 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 141 節 / 実装中 0 節 / 未実装・実装計画済み 1 節 / 未実装・未計画 0 節 / 実装待ち 0 節 | 全 12 設計書の `### ` 節（合計 142 節） | `REF:docs/designs/` |
| 仕様変更・修正（F-01〜F-44） | 実装完了（44 件） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装完了（18 件） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 実装済み（CLI・デスクトップ） | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 実装済み・検証完了（#1〜27） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
| 段階 ③（§19 外部インターフェース） | 実装済み・検証完了（H1〜H7。Python 拡張もサンドボックスでビルドし pytest まで通る。正式な wheel 検証は CI のホイール行列 `REF:.github/workflows/wheels.yml`。`REF:docs/plans.md` §5.5・§5.6） | Rust API・Python API・LSP・MCP 拡張・CLI 拡張による GUI 全機能のヘッドレス公開 | `REF:docs/plans.md#stage-3` |
| 配布（D-01〜D-20） | v0.1.0公開済み。既知の問題は公開版一覧で管理 | 5 形態・17 成果物とSHA256SUMS、公開・検査ワークフロー | `REF:docs/plans.md#distribution` |
<!-- SYNC:END stage1-status -->

---

<a id="planned-fixes-and-features"></a>
## 2. 仕様変更・修正および小規模機能追加の実装計画（状態: 全件実装完了）

以下の 62 項目（修正 44 件 `F-01`〜`F-44`、小規模機能追加 18 件 `S-01`〜`S-18`）の仕様書・設計書への反映、実装と検証を完了した。計画策定時点で完了していた `F-10` / `F-23` / `F-24` / `F-42` / `S-09` も含む。以下の **6 フェーズ（P1〜P6）** は実装時の依存関係と変更対象の近接性に基づく実行順序の記録である。

### 2.1 フェーズ別実行順序

1. **Phase 1（コア・辞書・形態素・インクリメンタル性能）**:
   - 対象: `F-24`, `S-09`, `F-23`, `F-10`, `F-42`（すべて実装完了）、`F-43`, `S-08`
   - 目的: `src/core/` 層の辞書透過統合、簡易形態素境界による `E001` 赤波線範囲切り出しと辞書/キャラ登録アクション、`;` を含むペーストの一括節分割、打鍵時の編集中 1 節限定同期解析（10 万字 1 打鍵 16ms 以内）、`ja.toml` 助詞空白規則を先に固める。
2. **Phase 2（テーマ・タイポグラフィ・クリック部品・アクセシビリティ基盤）**:
   - 対象: `F-01`, `F-11`, `S-18`, `F-29`, `F-30`, `F-32`, `F-37`
   - 目的: テーマ記号 `☾` のベジェ描画化、全角「？」「！」「＊」のフォント統一、`max_line_chars`（`ja`=44, `en`=70）、押せないラベルの非選択化、全クリック対象の下線＋ホバー背景（`bg.selection` 60%）＋ポインタ変更＋24×24px 当たり判定拡張、`TextEdit::lock_focus(true)` と `Tab` フォーカス巡回を共通部品として整備する。
3. **Phase 3（共通シェル・文脈ビュー・節エディタ・参照ペイン・ステータスバー）**:
   - 対象: `F-02`, `F-03`, `F-41`, `F-21`, `F-22`, `F-39`, `S-04`, `S-12`, `F-04`, `F-28`, `F-34`, `F-05`, `F-18`, `S-14`, `S-05`, `F-20`, `S-06`
   - 目的: 本文執筆画面（`write`）と共通シェルの見た目・レイアウト・パフォーマンス（可視範囲レイアウト、診断アコーディオン、修飾キー補完表示、左下トースト、幅別省略優先順）を完成させる。
4. **Phase 4（統一パレットの全面改修とキーボード操作の統一）**:
   - 対象: `F-06`, `F-07`, `F-08`, `F-09`, `F-12`, `F-13`, `F-14`, `F-15`, `F-25`, `F-26`, `F-27`, `S-13`, `F-17`, `F-35`, `F-36`
   - 目的: パレットの位置・z-index・固定幅・外側クリック遮断・未入力ガイド・0 件表示・`@` 広さ優先ドリルダウン・`>` 全ショートカット対応と `>agent` 階層化・`?` 選択実行・`Tab` 補完、および全画面の `↑`/`↓`（一覧）と `Alt+↑`/`Alt+↓`（確定移動）統一を実装する。
5. **Phase 5（各ワークスペース改善・エージェント機能拡張）**:
   - 対象: `F-16`, `F-31`, `F-38`, `F-44`, `S-07`, `F-19`, `S-01`
   - 目的: 全ワークスペースのガイド文ボタン化、`idea` 1,000 件の可視行仮想スクロール、`material` の全域 `Ctrl+Shift+N`・カテゴリチップ枠・条件プレースホルダ・URL 外部リンク、`agents/mcp.json` アプリ内編集モーダル、LLM コンテキストへの参考例（話の本文・プロット）付与を実装する。
6. **Phase 6（プラットフォーム・プロジェクト選択・PWA・CLI `import`・Web サイトエラー解説 CI）**:
   - 対象: `F-33`, `F-40`, `S-02`, `S-03`, `S-10`, `S-11`, `S-15`, `S-16`, `S-17`
   - 目的: `Ctrl+O` の 2 段構成統一、デスクトップ履歴の日付表示と履歴のみ削除、Web 同名プロジェクト禁止、初回ウェルカム案内、PWA オフライン化、`novnow import <zip> <dir>` と移行ヘルプ、および全診断コードの Web サイト解説と CI チェックを実装する。

---

<a id="planned-fixes"></a>
### 2.2 修正項目一覧（F-01〜F-44：実装完了）

<a id="plan-F-01"></a>
- **F-01（実装完了）**: テーマ記号 `◐` / `☾` / `☀` のベジェ描画化
  - 仕様・設計: `REF:docs/designs/ui.md`（§11.4）
  - 変更対象: `REF:src/ui/theme.rs`, `REF:src/ui/app.rs`
  - 計画内容: フォントグリフに存在しない `☾`（U+263E）等の文字描画をやめ、`egui::Painter` のベジェ曲線・円弧で半円（system）・三日月（dark）・太陽（light）を描画する `paint_theme_icon` を実装する。

<a id="plan-F-02"></a>
- **F-02（実装完了）**: 節エディタの枠線外へのテキストはみ出し修正
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.4）
  - 変更対象: `REF:src/ui/widgets/section_editor.rs`
  - 計画内容: 節エディタ枠の `inner_margin` と内部 `ScrollArea` / `TextEdit` のクリップ領域を一致させ、文字や下線が枠外へ出ないよう調整する。

<a id="plan-F-03"></a>
- **F-03（実装完了）**: カーソル寸法と行番号・文字の垂直位置合わせ
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.4）
  - 変更対象: `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/syntax_text.rs`
  - 計画内容: `Galley` の各行 `rect` から行番号のベースライン Y 座標を算出し、キャレット高さをフォントサイズに比例した自然な高さへ揃える。

<a id="plan-F-04"></a>
- **F-04（実装完了）**: 参照ペインで 4 個以上の同じ診断をアコーディオン集約
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.5）
  - 変更対象: `REF:src/ui/widgets/reference_pane.rs`, `REF:src/core/features/reference.rs`
  - 計画内容: 同一 `DiagnosticCode` の診断が 4 件以上ある場合は 1 行の折りたたみヘッダに集約し、クリックまたは `Enter` で展開・折りたたみできるようにする。

<a id="plan-F-05"></a>
- **F-05（実装完了）**: `Cmd` / `Ctrl` 押下中のステータスバーへのショートカット補完表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.6, §9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/app.rs`
  - 計画内容: `ctx.input(|i| i.modifiers)` を監視し、`Cmd` 単体や `Cmd+Shift` 等の押下中に、そのプレフィックスから始まるショートカット候補（`Cmd+Shift+X` 等）をステータスバーへリアルタイム表示する。

<a id="plan-F-06"></a>
- **F-06（実装完了）**: コマンドパレットの位置を画面水平中央・最上部に調整
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/app/palette.rs`
  - 計画内容: パレットの表示位置を VS Code と同じく X 座標中央・上端（ヘッダ直下）に固定する。

<a id="plan-F-07"></a>
- **F-07（実装完了）**: パレットの選択肢に「パレットを閉じる」を追加
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/app/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: パレット候補に常設の「パレットを閉じる（`Esc`）」項目を追加し、選択時にパレットを閉じる。

<a id="plan-F-08"></a>
- **F-08（実装完了）**: パレットが節エディタ枠の下に隠れる z-index の修正
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/widgets/section_editor.rs`
  - 計画内容: パレットの `egui::Area` を `Order::Foreground` で最前面に描画し、節エディタの枠線がパレットの上に重ならないようにする。

<a id="plan-F-09"></a>
- **F-09（実装完了）**: パレットの `>` でエージェント関連設定をサブ階層化
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/app/palette.rs`
  - 計画内容: `>` 直後の初期候補ではエージェント設定を `>agent ` の 1 項目にまとめ、`>agent` 入力または選択後に役割別・操作別の候補を展開する。

<a id="plan-F-10"></a>
- **F-10（実装完了: `REF:src/core/features/sections.rs`, `REF:src/ui/widgets/section_editor.rs`, `REF:src/app/workbench.rs`）**: 本文エディタへのペースト時に改行保持＋`;` で自動節分割
  - 仕様・設計: `REF:docs/designs/editing.md`（§8.1）
  - 変更対象: `REF:src/core/features/sections.rs`, `REF:src/app/workbench.rs`, `REF:src/ui/widgets/section_editor.rs`
  - 計画内容: ペーストされた文字列の改行を維持し、単独 `;` 行または `@A-B-C` 見出し行が含まれる場合は一気に複数の節へ分割して文書へ反映する。

<a id="plan-F-11"></a>
- **F-11（実装完了）**: 「？」「！」「＊」の太さを他文字と統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§11.5）
  - 変更対象: `REF:src/ui/theme.rs`
  - 計画内容: 全角約物（`？` `！` `＊`）が別フォントへフォールバックせず本文 CJK フォントで描画されるようフォントファミリ優先順を調整する。

<a id="plan-F-12"></a>
- **F-12（実装完了）**: パレットの横幅固定と長い候補テキストの `...` 省略
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/app/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: パレットの幅を固定し、長い候補ラベルは `...` で省略してパレットの位置ズレを防ぐ。

<a id="plan-F-13"></a>
- **F-13（実装完了）**: 未入力のパレット下部に接頭辞ルールのガイドを表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/app/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: 入力が空のときパレット下部に「`>` コマンド / `@` 節 / `'` 辞書 / `M`（`#`）資料 / `?` ショートカット」のガイドを表示する。

<a id="plan-F-14"></a>
- **F-14（実装完了）**: パレットで候補が 0 件のときに「ない」旨を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/app/palette.rs`, `REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`
  - 計画内容: 資料が 0 件のときに `M` / `#` を打った場合を含め、候補が 0 件のときはモードに応じた「一致する項目がありません（登録されている資料がありません）」を表示する。

<a id="plan-F-15"></a>
- **F-15（実装完了）**: すべてのショートカットキー操作を `>` パレット経由でも実行可能にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7, §9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/app/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: 全 `Command` を `>` コマンドパレットに公開し、パレットから全ショートカット機能を実行できるようにする。

<a id="plan-F-16"></a>
- **F-16（実装完了）**: ショートカット専用機能の視覚ボタン化（ガイドラベルのボタン化）
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1, §10.0〜§10.9）
  - 変更対象: `REF:src/ui/workspaces/mod.rs`, `REF:src/ui/workspaces/material.rs`, `REF:src/ui/workspaces/idea.rs`, `REF:src/ui/workspaces/plot_episode.rs`, `REF:src/ui/workspaces/plot_outline.rs`, `REF:src/ui/workspaces/cast.rs`, `REF:src/ui/workspaces/agent.rs`, `REF:src/ui/workspaces/review.rs`
  - 計画内容: 「`Ctrl+Shift+N` で新しい資料を作れます」等のガイドラベルをショートカット表記付きの押せるボタンに置き換える。

<a id="plan-F-17"></a>
- **F-17（実装完了）**: ブラウザ・OS に奪われるショートカットの非衝突化調整
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/web.rs`
  - 計画内容: ブラウザ予約キーと被らない代替キー割当および `preventDefault` 調整を行う。

<a id="plan-F-18"></a>
- **F-18（実装完了）**: 同梱フォント使用通知を左下のモードレストースト（数秒で消去）に変更
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.6, §11.5）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: フォントフォールバック通知をステータスバー常駐から外し、起動後 4 秒間だけ左下に表示されるモードレス通知にする。

<a id="plan-F-19"></a>
- **F-19（実装完了）**: コマンドパレットから `agents/mcp.json` 編集モーダルへ移行
  - 仕様・設計: `REF:docs/designs/syntax.md`（§5.11）, `REF:docs/designs/agent.md`（§15.7）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/app/palette.rs`, `REF:src/ui/workspaces/agent.rs`
  - 計画内容: パレット（`>mcp` / `>agent mcp`）および `agent` 画面から `agents/mcp.json` を編集・検証・保存できるモーダルを追加する。

<a id="plan-F-20"></a>
- **F-20（実装完了）**: エラー発生時に `review` ワークスペースアイコンへ視覚変化を付与
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1, §10.9）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: エラーまたは警告が存在するとき、ヘッダの `review` アイコンに色変化とバッジを描画する。

<a id="plan-F-21"></a>
- **F-21（実装完了）**: 文脈ビューの節の境目に横線を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 文脈ビューの各節の間に区切り横線を描画する。

<a id="plan-F-22"></a>
- **F-22（実装完了）**: 文脈ビューの節番号表示を `A-B-C-D`（4 階層完全形）に統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 章省略の `B-C-D` 表記をやめ、`@AA-BB-CC-DD` に統一する。

<a id="plan-F-23"></a>
- **F-23（実装完了: `REF:src/ui/widgets/reference_pane.rs`, `REF:src/ui/app.rs`）**: 参照ペインの診断から形態素単位で辞書・キャラ一覧へ追加する自動回復機能
  - 仕様・設計: `REF:docs/designs/semantics.md`（§6.2）, `REF:docs/designs/ui.md`（§9.5）
  - 変更対象: `REF:src/core/resolve/dictionary.rs`, `REF:src/core/features/code_action.rs`, `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 未解決参照診断に対して形態素解析で 1 形態素を切り出し、参照ペイン上のボタンから `words.md` または `characters.md` へ即時登録できるようにする。

<a id="plan-F-24"></a>
- **F-24（実装完了: `REF:src/core/resolve/dictionary.rs`, `REF:src/app/palette.rs`）**: `characters.md` の名前を透過的に辞書へ自動登録し重複を解消
  - 仕様・設計: `REF:docs/designs/semantics.md`（§6.1）
  - 変更対象: `REF:src/core/resolve/dictionary.rs`, `REF:src/core/features/reference.rs`, `REF:src/app/palette.rs`
  - 計画内容: `words.md` を書き換えずに `characters.md` の内容を透過的に用語辞書としても扱い、重複表示を抑止する。

<a id="plan-F-25"></a>
- **F-25（実装完了）**: パレットの `?` でショートカットを選択・実行可能にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/app/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: `?` モードをショートカットの検索＋選択実行モードとして実装する。

<a id="plan-F-26"></a>
- **F-26（実装完了）**: パレットの `@` 移動を広さ優先の階層ドリルダウン（`@01-* 章タイトル` 等）に変更
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/app/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: `@` 入力時に `@01-*`, `@02-*`（章タイトル付き）から始め、話 → 場面 → 節へと広さ優先で段階的に絞り込む。

<a id="plan-F-27"></a>
- **F-27（実装完了）**: パレット外クリック時に背面要素を押下判定にせずパレットのみ閉じる
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: パレット背面にフルスクリーンのクリック吸収シールド（`Sense::click()`）を配置し、外側クリックでパレットのみを閉じる。

<a id="plan-F-28"></a>
- **F-28（実装完了）**: 参照ペインの各セクション境界に薄めの横線を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.5）
  - 変更対象: `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 上位プロット・配役・用語・資料・診断の各セクション間に 1px の `border` 横線を描画する。

<a id="plan-F-29"></a>
- **F-29（実装完了）**: 押せないラベルのテキスト選択（ダブルクリック青選択）を無効化
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: `style.interaction.selectable_labels = false` を設定し、押せないラベルが選択ハイライトされないようにする。

<a id="plan-F-30"></a>
- **F-30（実装完了）**: 押せる要素すべてに薄い下線＋ホバー変化を付与しテストで保証
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1, §9.9）
  - 変更対象: `REF:src/ui/widgets/mod.rs`, `REF:tests/gui.rs`
  - 計画内容: 全クリック対象に薄い下線とホバー変化を付与し、`REF:tests/gui.rs` で自動検査する。共通アクション・選択・一覧・Web 選択画面・個別のアイコンと外部リンクを監査し、`REF:tests/gui.rs` と `REF:tests/workspaces.rs` で代表的な描画・操作を検証した。

<a id="plan-F-31"></a>
- **F-31（実装完了）**: 資料ワークスペースの `Ctrl+Shift+N` をペインフォーカス非依存にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.5）, `REF:docs/designs/workflows.md`（§16.1）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/shell.rs`
  - 計画内容: 左一覧以外にフォーカスがあっても `Ctrl+Shift+N`（および新規ボタン）で新しい資料を作成できるようにする。

<a id="plan-F-32"></a>
- **F-32（実装完了）**: 全クリック対象に hover 背景（`bg.selection` 60%）＋ポインタ変更＋24×24px 以上の `interact` 矩形を付与
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.9）
  - 変更対象: `REF:src/ui/widgets/mod.rs`, `REF:src/ui/app.rs`, `REF:src/ui/workspaces/mod.rs`, `REF:tests/gui.rs`
  - 計画内容: `allocate_exact_size` の視覚寸法は変えずに `ui.interact` の当たり判定矩形を最低 24×24px に広げ、hover 時の `bg.selection`（60%）塗りと `CursorIcon::PointingHand` を全クリック対象へ適用する。共通部品と個別アイコン、タイムラインの行（24px 以上）まで適用し、`REF:tests/gui.rs` と `REF:tests/workspaces.rs` で hover・当たり判定を検証した。

<a id="plan-F-33"></a>
- **F-33（実装完了）**: Web 版で同じタイトルのプロジェクトを作成・改名できないようにする
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.4）
  - 変更対象: `REF:src/ui/web.rs`, `REF:src/adapters/storage_idb.rs`
  - 計画内容: 既存プロジェクトとタイトルが重複する場合は新規作成・改名を拒否してエラーを表示する。

<a id="plan-F-34"></a>
- **F-34（実装完了）**: 参照ペインの要約が途中で切れるときに「`続き...`」の 1 行を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.5）, `REF:docs/designs/workflows.md`（§16.5）
  - 変更対象: `REF:src/core/features/reference.rs`, `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 行数・文字数上限で切り詰めた要約の末尾にクリック可能な「`続き...`」を表示する。

<a id="plan-F-35"></a>
- **F-35（実装完了）**: 全画面で「一覧移動は `↑`/`↓`、編集確定して移動は `Alt+↑`/`Alt+↓`」に統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/workspaces/material.rs`, `REF:src/ui/workspaces/idea.rs`, `REF:src/ui/workspaces/cast.rs`, `REF:src/ui/workspaces/plot_episode.rs`, `REF:src/ui/workspaces/plot_outline.rs`, `REF:src/ui/workspaces/agent.rs`, `REF:src/ui/workspaces/review.rs`
  - 計画内容: エディタにフォーカスがないときは素の `↑`/`↓` をどの画面でも一覧・タイムライン移動に効かせる。

<a id="plan-F-36"></a>
- **F-36（実装完了）**: 資料ワークスペースのフォーカス移動を `Tab` に移し `Ctrl+3` を参照ペイン移動に統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.8, §10.5）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/shell.rs`
  - 計画内容: `material` 内の `Ctrl+3` 独自バインドを撤去して `Tab` に移し、`Ctrl+3` の意味を共通シェルと統一する。

<a id="plan-F-37"></a>
- **F-37（実装完了）**: `TextEdit` に `lock_focus(true)` を付け、入力欄外では `Tab` でヘッダボタン等を巡回可能にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.4, §9.8）
  - 変更対象: `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/slice_editor.rs`, `REF:src/ui/app.rs`
  - 計画内容: 節エディタを 1 つのフォーカス対象にし、入力欄外での `Tab` フォーカス移動と明瞭なフォーカス枠表示を実装する。

<a id="plan-F-38"></a>
- **F-38（実装完了）**: アイデア 1,000 件の一覧で可視行のみを描画し 60fps を維持
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.4）, `REF:docs/designs/workflows.md`（§18.1）
  - 変更対象: `REF:src/ui/workspaces/idea.rs`
  - 計画内容: `ScrollArea::show_rows` による仮想スクロールを導入する。

<a id="plan-F-39"></a>
- **F-39（実装完了）**: 文脈ビューの節未保存状態を示すオレンジの点を拡大
  - 仕様・設計: `REF:docs/designs/editing.md`（§8.4）, `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`, `REF:src/ui/app.rs`
  - 計画内容: 未保存ドットの直径を 8〜10px 相当に大きくする。

<a id="plan-F-40"></a>
- **F-40（実装完了）**: デスクトップの `Ctrl+O` を「保存してから選択画面（履歴）へ戻る」に統一し 2 段構成にする
  - 仕様・設計: `REF:docs/designs/platform.md`（§14.4）, `REF:docs/designs/ui.md`（§9.8）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: `Ctrl+O` でいきなりフォルダダイアログを開かず、Web 版と同じくプロジェクト選択画面（履歴一覧）へ戻り、そこから「フォルダを開く」で選ぶ 2 段構成に統一する。

<a id="plan-F-41"></a>
- **F-41（実装完了）**: ペインフォーカスを左端 3px 縦線、編集中の節を 1px 枠に分離し、文脈ビューのプレースホルダと対で見せる
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3, §9.4）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 節エディタの全周太青枠をやめ、ペインフォーカスは左端 3px 縦線、編集中節は常時 1px 枠、文脈ビューのプレースホルダにも左端 3px 縦線を付けて対にする。

<a id="plan-F-42"></a>
- **F-42（実装完了: `REF:src/ui/i18n/ja.toml`, `REF:tests/i18n_catalog.rs`）**: `{0}はこの版ではまだ使えません` の空白修正と `ja.toml` 助詞直前空白のテスト検査
  - 仕様・設計: `REF:docs/designs/ui.md`（§12.3）
  - 変更対象: `REF:src/ui/i18n/ja.toml`, `REF:tests/i18n_catalog.rs`, `REF:tests/workspaces.rs`
  - 計画内容: `ja.toml` のテンプレート直後が助詞の箇所の半角空白を削除し、`REF:tests/i18n_catalog.rs` で機械検査する。

<a id="plan-F-43"></a>
- **F-43（実装完了）**: 打鍵時は編集中の節だけ同期解析し、10 万字ファイルで 1 打鍵 16ms 以内を保証・テスト計測
  - 仕様・設計: `REF:docs/designs/semantics.md`（§6.7）, `REF:docs/designs/workflows.md`（§18.1, §18.4）
  - 変更対象: `REF:src/app/workbench.rs`, `REF:src/core/features/project.rs`, `REF:tests/incremental.rs`, `REF:benches/core.rs`
  - 計画内容: 打鍵時の同期解析を編集中の 1 節に限定して他節をデバウンスし、「10 万字のファイルで 1 打鍵 16ms 以内」を計測するテストを追加する。

<a id="plan-F-44"></a>
- **F-44（実装完了）**: 資料画面のカテゴリチップ（「この資料のカテゴリ」直下＋選択枠）と条件欄プレースホルダ化・`fg.faint` 統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.5）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`
  - 計画内容: 8 個のカテゴリチップを「この資料のカテゴリ」見出し直下に置いて選択中を枠線で示し、条件の入力例は入力欄のプレースホルダに移し、説明文の色を `fg.faint` に統一する。

---

<a id="planned-small-features"></a>
### 2.3 機能追加（小規模：S-01〜S-18：実装完了）

<a id="plan-S-01"></a>
- **S-01（実装完了）**: LLM エージェントに「例」として指定した話の本文またはプロットをコンテキスト付与
  - 仕様・設計: `REF:docs/designs/agent.md`（§15.4, §15.7）, `REF:docs/designs/syntax.md`（§5.9, §5.10）
  - 変更対象: `REF:src/core/model/task.rs`, `REF:src/core/agents/files.rs`, `REF:src/core/aicontext/packet.rs`, `REF:src/core/syntax/prompt_md.rs`, `REF:src/ui/workspaces/agent.rs`
  - 計画内容: タスクに参考例範囲 `examples` を持たせ、指定した話・節の本文またはプロットをコンテキストパケットおよび `{{examples}}` 変数として渡す。

<a id="plan-S-02"></a>
- **S-02（実装完了）**: 初回起動時（履歴が空のとき）のアプリ説明・ドキュメントサイト案内とパレットからの再呼び出し
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.4, §14.4）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/web.rs`, `REF:src/app/palette.rs`
  - 計画内容: 履歴が空の初回起動時にウェルカム案内を表示し、コマンドパレットからも常時呼び出し可能にする。

<a id="plan-S-03"></a>
- **S-03（実装完了）**: ブラウザ版の PWA 化・オフライン動作対応と `README.md` への追記
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.3）
  - 変更対象: `REF:index.html`, `REF:Trunk.toml`, `REF:README.md`
  - 計画内容: Web App Manifest と Service Worker を追加してオフライン動作可能にし、`REF:README.md` に PWA 化可能であることを記載する。

<a id="plan-S-04"></a>
- **S-04（実装完了）**: 本文が 0 文字のときに文脈ビューへ 3 行ガイドを薄く表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 本文が空のとき `Alt+数字=画面 / Alt+矢印=節移動 / Ctrl+P=一覧` の 3 行ガイドを `fg.faint` で描画する。

<a id="plan-S-05"></a>
- **S-05（実装完了）**: ヘッダ右に `?` と `Ctrl+P` のボタンを常設
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: 上部ヘッダ右端に `?` ボタンと `Ctrl+P` ボタンを常設する。

<a id="plan-S-06"></a>
- **S-06（実装完了）**: ワークスペース切替時にパンくずの右へ画面名を 1 秒だけ表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.0）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: 画面切替から 1 秒間、パンくずの右にワークスペース名を表示する。

<a id="plan-S-07"></a>
- **S-07（実装完了）**: 資料ビュワーの URL を外部リンクアイコン＋薄黒下線＋ホバー青色で開けるようにする
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.5）, `REF:docs/designs/workflows.md`（§16.5）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 資料内の URL に左下向き（外部リンク）矢印アイコン・薄い下線・ホバー時青色を付け、クリックで開けるようにする。

<a id="plan-S-08"></a>
- **S-08（実装完了）**: 閉じ鉤括弧の後の `@` でキャラ名を予測補完
  - 仕様・設計: `REF:docs/designs/syntax.md`（§5.2）, `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/core/features/completion.rs`, `REF:src/app/assist.rs`
  - 計画内容: `」@` / `』@` 入力時にキャラ名およびモードの補完ポップアップを出す。

<a id="plan-S-09"></a>
- **S-09（実装完了: `REF:src/core/resolve/dictionary.rs`）**: 未知の `'` 用語を形態素解析で区切り赤波線の範囲を決定
  - 仕様・設計: `REF:docs/designs/semantics.md`（§6.2）
  - 変更対象: `REF:src/core/resolve/dictionary.rs`, `REF:tests/dictionary.rs`
  - 計画内容: 未知の `'` 参照の赤波線範囲を形態素解析（文字種境界・助詞切り出し）による 1 形態素に区切る。

<a id="plan-S-10"></a>
- **S-10（実装完了）**: `novnow import <zip> <dir>` の追加とデスクトップ版の Zip 導入対応
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.5, §14.1）
  - 変更対象: `REF:src/cli/mod.rs`, `REF:src/adapters/zip.rs`, `REF:src/ui/app.rs`, `REF:tests/cli.rs`
  - 計画内容: CLI `novnow import <zip> <dir>` とデスクトップ GUI での Zip インポートを実装する。

<a id="plan-S-11"></a>
- **S-11（実装完了）**: デスクトップのプロジェクト選択画面に日付を表示
  - 仕様・設計: `REF:docs/designs/platform.md`（§14.4）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: `NativePicker` の各履歴行に `opened_at` の日付を表示する。

<a id="plan-S-12"></a>
- **S-12（実装完了）**: 文脈ビューを可視範囲だけレイアウトする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）, `REF:docs/designs/workflows.md`（§18.1）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: ビューポート外の節は高さ推定のみでスキップし、可視範囲の節のみレイアウトする。

<a id="plan-S-13"></a>
- **S-13（実装完了: `REF:src/app/palette.rs`, `REF:src/ui/app.rs`, `REF:tests/gui.rs`）**: パレットで個別単位（例: `">ag"` => `">agent "`）の `Tab` 補完
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/app/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: パレットでの `Tab` キー押下時に次のトークン単位まで補完する。

<a id="plan-S-14"></a>
- **S-14（実装完了）**: ステータスバーの幅に応じた省略優先順（保存状態最高優先、`…`＋ホバー詳細）
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.6）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: 幅不足時に「セッション情報 → 原稿用紙枚数 → 文字数」の順で `…` に畳み、ホバーで詳細を出し、保存状態マークを最高優先で残す。

<a id="plan-S-15"></a>
- **S-15（実装完了）**: Web サイトへの全エラー解説の記載と CI での網羅性チェック
  - 仕様・設計: `REF:docs/designs/diagnostics.md`（§7.2）, `REF:docs/designs/workflows.md`（§18.4）
  - 変更対象: `REF:docs/specifications/diagnostics.md`, `REF:tests/diagnostics_catalog.rs`, `REF:.github/workflows/ci.yml`
  - 計画内容: 定義されている全 76 診断コードの原因・具体例・対処（日英）の個別ページを整備し、全コードの解説が存在するかを CI で機械検査する（`REF:website/src/pages/[lang]/diagnostics/[code].astro`, `REF:scripts/diagnostics-site.py`）。

<a id="plan-S-16"></a>
- **S-16（実装完了）**: Web⇔デスクトップ双方向移行のヘルプ（Zip 手順）を追加
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.5）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`
  - 計画内容: Zip を使った Web→デスクトップおよびデスクトップ→Web 移行手順のヘルプ画面を追加する。

<a id="plan-S-17"></a>
- **S-17（実装完了）**: デスクトップ選択画面に「データは消さず履歴のみを消す」ことを明示した履歴削除機能を追加
  - 仕様・設計: `REF:docs/designs/platform.md`（§14.4）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/core/policy/recent.rs`
  - 計画内容: デスクトップの履歴一覧から、実データを消さずに履歴エントリのみを削除する操作と明示説明を追加する。

<a id="plan-S-18"></a>
- **S-18（実装完了: `REF:src/app/settings.rs`, `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/context_view.rs`, `REF:tests/gui.rs`）**: 本文の最大幅を `.novnow/ui.json` の設定にし、`ja` は 44 字、`en` は 70 字前後を既定にする
  - 仕様・設計: `REF:docs/designs/project.md`（§4.4）, `REF:docs/designs/ui.md`（§11.5）
  - 変更対象: `REF:src/app/settings.rs`, `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/context_view.rs`
  - 計画内容: `ui.json` に `max_line_chars` を追加し、言語別既定値（`ja`=44, `en`=70）で折り返し幅を制御する。

---

<a id="unplanned-large"></a>
## 3. 機能追加（比較的大規模：L-01）実装計画

今回の依頼により計画待機を解除し、最小移行案をユーザー承認済み。以下は段階的な実行計画であり、詳細の裁定は最新のユーザー指示に従い自律実行し、判断を履歴に記録する。対応先: `REF:docs/designs/platform.md` §14.6。

1. **仕様詳細の承認**: UTF-8（BOM 可）プレーンテキストを対象に、章・話を自動推定せず、1 ファイルを 1 話・1 場面・1 節へ取り込む最小移行を提案する。原文中の novnow 記号を本文として保護する方法、対応文字コード、分割規則、CLI/Web/デスクトップの提供範囲を確認する。
2. **承認後の仕様・設計反映**: 入力・拒否条件、プレビュー、上書き禁止、失敗時の扱い、完了導線を確定する。仕様・設計の変更は承認後に行う。
3. **純粋な変換**: 正規化・記号保護・テンプレート生成を I/O から分離する。変換結果をエクスポートし、正規化後の原文と比較する往復テストを必須とする。原文の話者や章立てを推測しない。
4. **保存経路**: 既存のポリシー・ストレージ境界を使用し、入力元を変更せず、既存の作品を上書きしない。入力不正・衝突・書き込み失敗のテストを追加する。
5. **操作経路と検収**: 承認された CLI/UI からプレビュー→確定→作品を開く経路を接続する。空入力、改行、BOM、日本語、予約シジル、長文、既存先、プロジェクト外パスを検証する。

状態: 最小移行案をユーザー承認済み。CLI の変換・プレビュー・新規先限定の取り込みを実装済み（`REF:src/core/migration.rs`, `REF:src/cli/migrate.rs`）。記号保護に `\#` / `\!` / `\^` を追加。デスクトップの選択・プレビュー・新規先限定作成を接続済み。Web は対象外。初期 CLI は `migrate <text> <dir> [--preview]`、移行先の親は既存ディレクトリを指定する。

---

<a id="stage-2"></a>
## 4. 段階 ② 実装計画（追加仕様 1〜27）

段階 ① で整備した 10 個の拡張点（縫い代、`REF:docs/designs/overview.md` §3.3）を土台として、第 2 次指示の 26 件と段階 ② へ移管した読者モード（#27）の計 **27 項目**を段階 ② で実装する。

### 4.0 実行順序・完了判定

共通方針: 最新のユーザー指示に従い詳細の裁定を含め自律実行し、変更した判断を履歴へ残す。各バッチはコア→境界→UI→テストの順に接続し、コアだけの実装を機能完了と数えない。受け入れ基準は本書 §4.5 を正本とし、通過したもののみチェックする。既存 10 拡張点・未知キー温存・書き込み単一関門を維持する。

| バッチ | 対象 | 作業と依存関係 | 検証 |
|---|---|---|---|
| B1 構文・出力 | #15 → #10 → #23 → #17 | 行コメント、引用、挿絵、あらすじを順にパーサ・計数・解決・出力へ接続。CLI オプションと UI は同じ出力設定を使用 | 無損失往復、計数、診断、全出力オプション、不変条件 |
| B2 編集体験 | #1, #2, #4, #5, #6, #9 | 位置持ち越し、セッション、句切れ、連続台詞、比率、折りたたみ。設定永続化と検索時展開を含む | GUI キー操作、30 分境界、決定論、折りたたみと出力の独立、性能 |
| B3 プロット・参照 | #3, #7, #8, #21, #24 | 日時、五感、呼称、空間、アイコンを構文から参照ペイン・各タブへ接続。外部参照は境界で検証 | 各診断・例外、呼称編集、25 人境界、欠落ファイル、範囲外拒否 |
| B4 検索・提案 | #19 → #11, #16 | 正規表現・大小文字・時間制限、凍結資料と再検索差分、人間の節全文提案。置換はプレビューと単一 undo | タイムアウトの部分結果、検索対象除外、500 件上限、採番、本文非変更 |
| B5 分析・記録 | #18, #20, #25, #26 | 明示実行の観測、日次統計、キャラ別集計、節長ヒストグラム、分量 T スコア。B1/B2 の共通計数を再利用 | 無効化、未知話者の総和、母集団境界、stats 削除耐性、禁止された評価表示がないこと |
| B6 構造・連携 | #22, #12, #13, #14 | ストックの原子的配属、カスタムロール、管理対象限定 MCP 設定、外部ガイド導線 | 配属の失敗巻き戻し、権限制約、既定ロール衝突、localhost 判定、手書き定義温存 |
| B7 読者モード | #27 | B1 の出力から純粋なフォントメトリクス注入組版、禁則・縦中横・回転・ルビ、章表紙、ページ操作としおりを実装 | 注釈非表示、キーで原稿非変更、組版座標の不変条件テスト、1 ページ 3ms ベンチ |
| B8 総合検収 | L-01 と #1〜27 | 全受け入れ基準を照合し、未検証の OS/性能を分離して記録する | 全検証スクリプト、Web スモーク、サイズ、ベンチ、PR の CI |

**実施記録**: B1〜B7 の各経路を実装し、B8 の最終検証を実施。横断受け入れは `REF:tests/stage_two.rs`、保存失敗の巻き戻しは `REF:tests/storage_fs.rs`、loopback MCP の実通信は `REF:src/adapters/mcp_http.rs` の mock テストで検証する。実装完了時の全体検証は 1268 件成功（56 ターゲット）。release の 16ms / 3ms 条件、ブラウザの読者原稿非変更と画像の Zip 往復を確認。Alt+数字の文字混入を回帰テスト化し、Web スモーク全項目および PR #23 の実装コミットに対する Actions 全6ジョブの成功を確認した。完了記録は `REF:docs/history.md` の L01-STAGE2-COMPLETE。

### 4.1 追加仕様 1〜27 の完全リストと仕様・設計書対応表（旧 §0.6・旧計画書 §1.2 統合）

| # | 追加仕様 | 統合先の仕様・設計書 | 段階 ② で実装する対象箇所・設定キー・診断コード | 検収基準 |
|---|---|---|---|---|
| 1 | ワークスペース・参照ペインの瞬時切替 | `REF:docs/designs/ui.md`（§9.8.1, §10.0） | `Ctrl+Shift+1..9`（強制位置持ち越し）、16ms 切替要件、着地点表、`Ctrl+\`（参照ペイン開閉） | §4.5 `19.14-1` |
| 2 | ステータスバーの執筆セッション情報 | `REF:docs/designs/ui.md`（§9.6.2） | ステータスバーの `+n字 m分` 表示、30 分無操作でのセッション区切り | §4.5 `19.14-2` |
| 3 | タイムライン・作中日時（`when:`） | `REF:docs/designs/syntax.md`（§5.0.6, §5.3.2, §5.3.5, §5.4.2）, `REF:docs/designs/ui.md`（§10.3.2） | `when:` フィールド解釈、`plot-outline` のタイムラインタブ、診断 `W080` | §4.5 `19.14-3` |
| 4 | 自動句切れ提案（決定論・既定オフ） | `REF:docs/designs/ui.md`（§9.4.3）, `REF:docs/designs/project.md`（§4.4.1） | `ui.json.suggest_comma`、`Ctrl+Alt+,` トグル、地の文の読点候補縦線表示 | §4.5 `19.14-4` |
| 5 | 同一話者の連続台詞検出 | `REF:docs/designs/project.md`（§4.5.1）, `REF:docs/designs/diagnostics.md`（§7.2） | `writing.max_consecutive_speeches` 設定キー、診断 `W082` | §4.5 `19.14-5` |
| 6 | 台詞 / 地の文 比率のリアルタイム表示 | `REF:docs/designs/ui.md`（§9.4.2, §10.2）, `REF:docs/designs/diagnostics.md`（§7.2） | 節エディタミニヘッダの比率バー、`plot-episode` 節リスト右端バー、診断 `W090` | §4.5 `19.14-6` |
| 7 | 五感チェック（`senses:`） | `REF:docs/designs/syntax.md`（§5.4.2, §5.4.4）, `REF:docs/designs/ui.md`（§9.5.2, §10.9.3） | `senses:` フィールド、参照ペイン 5 五感セクション、`review` 五感タブ、診断 `W083` | §4.5 `19.14-7` |
| 8 | 呼称マトリクス（`address:`） | `REF:docs/designs/syntax.md`（§5.5.6）, `REF:docs/designs/semantics.md`（§6.1.1, §6.2.4）, `REF:docs/designs/ui.md`（§10.6.1） | `address:` 定義、辞書の `Address` 種別、文脈例外行、`cast` 呼称タブ、診断 `W084` / `W085` | §4.5 `19.14-8` |
| 9 | 構造の折りたたみ（章→話→場面 UI） | `REF:docs/designs/ui.md`（§9.3.3, §9.7.1, §10.3.1）, `REF:docs/designs/project.md`（§4.4.1） | 文脈ビューの章/話/場面折りたたみ UI、`ui.json.folded` 永続化、`Ctrl+Shift+[` / `]` / `Ctrl+K` 系（※`REF:src/core/features/folding.rs` 自体は段階 ① で実装済み） | §4.5 `19.14-9` |
| 10 | 資料引用追跡（`^M0137`） | `REF:docs/designs/syntax.md`（§5.0.2, §5.2.4, §5.4.2）, `REF:docs/designs/ui.md`（§9.5.2）, `REF:docs/designs/workflows.md`（§17.1） | `^M0137` 引用注釈トークン、エクスポート除去規則 5b、未引用資料の診断 `I054` | §4.5 `19.14-10` |
| 11 | 検索結果の資料化（`S####`） | `REF:docs/designs/syntax.md`（§5.8.2）, `REF:docs/designs/workflows.md`（§16.3.1） | `S####` 接頭辞発行、`category: 検索結果`、`source.kind: search`、`[再検索]` 差分、診断 `I058` | §4.5 `19.14-11` |
| 12 | エージェントのカスタムロール | `REF:docs/designs/syntax.md`（§5.9.2）, `REF:docs/designs/agent.md`（§15.2.1） | `prompts/custom/`、`agents.custom_roles`、診断 `E032`–`E034` / `I056` | §4.5 `19.14-12` |
| 13 | ローカル LLM 簡易モード | `REF:docs/designs/agent.md`（§15.8.4）, `REF:docs/designs/project.md`（§4.5.1） | `agents.local_llm` 設定、`managed_by: "novnow-local-llm"` 自動生成、診断 `E064` / `I057` | §4.5 `19.14-13` |
| 14 | Web 解説ドキュメント（Git / CI・CD） | `REF:docs/designs/platform.md`（§14.2.1）, `REF:docs/designs/workflows.md`（§17.2） | `GIT_GUIDE_DOC_URL` / `CICD_GUIDE_DOC_URL` 定数と外部ブラウザ導線 | §4.5 `19.14-14` |
| 15 | `# ` 行コメント | `REF:docs/designs/syntax.md`（§5.0.2, §5.0.7, §5.0.8）, `REF:docs/designs/workflows.md`（§17.1） | `.txt` 3 形式での `# ` 行コメント字句化、エクスポート除去規則 3b | §4.5 `19.14-15` |
| 16 | 人間用トラックチェンジ（提案モード） | `REF:docs/designs/editing.md`（§8.7）, `REF:docs/designs/agent.md`（§15.7.3） | `Ctrl+Alt+P` 提案モードトグル、`H####` 差分と `before` 不一致診断 `W093` | §4.5 `19.14-16` |
| 17 | あらすじ（`@A-00` / `blurb:`） | `REF:docs/designs/syntax.md`（§5.1.2, §5.3.2, §5.3.7）, `REF:docs/designs/workflows.md`（§17.2） | `blurb:` フィールド、`B=00` 章あらすじ、`--include-blurb` / `--blurb-only`、読者モード章表紙、診断 `W086` | §4.5 `19.14-17` |
| 18 | 文体・品質分析ダッシュボード | `REF:docs/designs/ui.md`（§9.4.4, §10.9.4）, `REF:docs/designs/diagnostics.md`（§7.2） | `review` の文体タブ、一文長・同語反復・文頭接続詞連続の観測、診断 `W081` / `W087` / `W088` / `W089` | §4.5 `19.14-18` |
| 19 | 全体検索の正規表現対応 | `REF:docs/designs/ui.md`（§9.7.4）, `REF:docs/designs/diagnostics.md`（§7.2） | `Alt+R` 正規表現モード、`Alt+C` 大小文字区別、1 秒タイムアウト打ち切り診断 `W094` | §4.5 `19.14-19` |
| 20 | 執筆データ統計ダッシュボード | `REF:docs/designs/project.md`（§4.4.2）, `REF:docs/designs/ui.md`（§9.6.3, §10.9.5） | `.novnow/stats.json` 日次集計、`review` 統計タブ、`Ctrl+Alt+S` 統計モード | §4.5 `19.14-20` |
| 21 | 空間設計メモ（`space:`） | `REF:docs/designs/syntax.md`（§5.3.2, §5.3.6, §5.4.2）, `REF:docs/designs/ui.md`（§9.5.2, §9.5.3） | `space:` / `space-mode: ascii`、参照ペイン 6 空間セクション、外部参照欠落診断 `W092` | §4.5 `19.14-21` |
| 22 | 「00 章」ストック | `REF:docs/designs/syntax.md`（§5.1.2）, `REF:docs/designs/project.md`（§4.1, §4.5.1）, `REF:docs/designs/ui.md`（§10.3.3） | `structure.allow_chapter_zero`、`@00-..` ストック章管理と原子的配属、`--include-stock`、診断 `I055` | §4.5 `19.14-22` |
| 23 | 挿絵メモ（`illustration:` / `! `） | `REF:docs/designs/syntax.md`（§5.0.2, §5.2.5, §5.4.2, §5.4.5）, `REF:docs/designs/ui.md`（§9.5.2, §10.9.1）, `REF:docs/designs/workflows.md`（§17.1, §17.2） | `! ` 行と `illustration:`、参照ペイン 8 挿絵セクション、`review` 挿絵一覧タブ、エクスポート 3c と `--keep-illustration-memo` | §4.5 `19.14-23` |
| 24 | キャラアイコン（`icon:`） | `REF:docs/designs/syntax.md`（§5.5.3）, `REF:docs/designs/ui.md`（§9.5.2）, `REF:docs/designs/diagnostics.md`（§7.1, §7.2） | `characters.md` の `icon:` 行、`assets/` 画像読み込みと検証、診断 `W068`–`W070` / `W092` | §4.5 `19.14-24` |
| 25 | キャラ別・節長の統計強化 | `REF:docs/designs/ui.md`（§9.6.3, §10.9.5） | キャラ別台詞数・台詞字数（話者不明 `（不明）` 集計含む）、節長ヒストグラム | §4.5 `19.14-25` |
| 26 | 話の長さバランス偏差値 | `REF:docs/designs/ui.md`（§10.3.4）, `REF:docs/designs/diagnostics.md`（§7.2） | `plot-outline` 分量タブでの T スコア（偏差値）算出、外れ値診断 `W091` | §4.5 `19.14-26` |
| 27 | 読者モード（縦書きプレビュー） | `REF:docs/designs/ui.md`（§10.7）, `REF:docs/designs/workflows.md`（§17.3, §18.1）, `REF:docs/designs/project.md`（§4.4.3, §4.5.1） | `Alt+7`（`reader` ワークスペース）、`REF:src/core/reader.rs`（純粋組版関数 `layout`）と `REF:src/ui/workspaces/reader.rs`（読者 UI）、`.novnow/reader.json`、`project.json.reader`、`reader.exit` 文言、1 ページ 3ms 以内目標 | §4.5 `19.14-27` |
| U-7 | 既定プロンプトの不変保管と復元 | `REF:docs/designs/agent.md`（§15.5.4） | **段階 ①（M10）で実装完了**（`REF:src/core/agents/prompts.rs`、`>prompt reset` / `>prompt diff`） | §4.5 `19.14-U7` |
| U-8 | エージェントの並列実行 | `REF:docs/designs/agent.md`（§15.7.5） | **段階 ①（M10）で実装完了**（`REF:src/core/agents/runner.rs`、`REF:src/app/agents.rs`、`Shift+R`） | §4.5 `19.14-U8` |

---

### 4.2 横断的に段階 ② へ回している要素一覧（旧計画書 §1.2）

- **診断カタログ（`REF:docs/designs/diagnostics.md` §7.2）のうち段階 ② 対象の検査実装**:
  - コード定義と日英カタログ自体は段階 ①（M1）で全件登録済み（`REF:src/core/diagnostics.rs`）。
  - 検査ロジックが段階 ② 対象のもの: `W068`–`W070`（キャラアイコン）、`W080`–`W094`（作中日時・文体・連続台詞・五感・呼称・あらすじ・偏差値・リンク切れ・提案不一致・正規表現タイムアウト）、`E032`–`E034`（カスタムロール）、`E064`（ローカル LLM 接続）、`I054`–`I058`（未引用資料・ストック章・未登録ロール・ローカル LLM 登録・検索結果資料化）。
  - ※`E035`（タスク重複）・`E036`（MCP 委譲レベル不足）・`W095`（自律タスク上限停止）の 3 件は段階 ①（M10）で実装済み。
- **追加設定キー（`REF:docs/designs/project.md` §4.4 / §4.5.1）**:
  - `project.json`: `structure.allow_chapter_zero`、`writing.max_consecutive_speeches` / `long_sentence_chars` / `dialogue_ratio_target`、`export.include_blurb` / `keep_illustration_memo`、`agents.custom_roles` / `local_llm`、`reader.*`。
  - `.novnow/ui.json`: `suggest_comma`、`folded`。
  - `.novnow/stats.json`（§4.4.2）および `.novnow/reader.json`（§4.4.3）。
  - ※段階 ① では `#[serde(flatten)]` による未知キー温存（縫い代 1）を実装済み。
- **セマンティックトークン種別（`REF:docs/designs/semantics.md` §6.3）**:
  - `ref.address`（呼称参照）、`citation`（`^M0137` 引用注釈）、`illustration`（`! ` 挿絵メモ行）。
- **参照ペインのセクション（`REF:docs/designs/ui.md` §9.5.2）**:
  - 表示順 1〜9 のうち `5 五感`・`6 空間`・`8 挿絵` の 3 セクション（段階 ① は 1, 2, 3, 4, 7, 9 の 6 セクションを `Vec<Section>` で構築）。
- **ショートカット（`REF:docs/designs/ui.md` §9.8）**:
  - `Ctrl+Shift+1..9`（強制位置持ち越し切替）、`Ctrl+Alt+S`（統計モード）、`Ctrl+Alt+,`（自動句切れ提案）、`Ctrl+Shift+[` / `]`（本文文脈ビューの折りたたみ）、`Ctrl+K Ctrl+0` / `Ctrl+K Ctrl+J`（全折りたたみ / 全展開）、`Ctrl+Shift+Space`（五感パレット）、`Alt+,`（ローカル LLM 簡易設定）、`Alt+R` / `Alt+C`（全体検索の正規表現 / 大小文字区別）、`Alt+7`（読者モード）。
- **ワークスペースの追加タブ構造（`REF:docs/designs/ui.md` §10.3 / §10.6 / §10.9）**:
  - `plot-outline`（§10.3）のタイムライン・分量・ストックタブ、`cast`（§10.6）の呼称マトリクスタブ、`review`（§10.9）の五感・文体・挿絵・統計タブ。
- **CLI `novnow export` の追加オプション（`REF:docs/designs/workflows.md` §17.2）**:
  - `--include-blurb`、`--blurb-only`、`--keep-illustration-memo`、`--include-stock`。

---

### 4.3 段階 ①・② の境界裁定（旧計画書 §1.3・§1.4）

| 論点 | 裁定 | 理由と現在の状態 |
|---|---|---|
| `core::features::folding` | **段階 ① で実装済み** | §19.13 が「§3.4 の 11 機能が `core::features::*` に存在し UI から呼ばれている」を要求するため、段階 ① で `REF:src/core/features/folding.rs` を実装し `plot-outline` の章→話展開に接続済み。本文文脈ビュー（§9.3.3）の折りたたみ UI と `ui.json.folded` 永続化が段階 ②（#9）。 |
| 全体検索（`Ctrl+Shift+F`） | **通常検索は ①、正規表現は ②** | リテラル全体検索は段階 ① で実装済み（`REF:src/app/assist.rs`）。§9.7.4 の正規表現モード（`Alt+R`）と 1 秒打ち切り（`W094`）が段階 ②（#19）。 |
| §10.3 / §10.6 / §10.9 のタブ | **段階 ① は基本ビュー、段階 ② で拡張タブ追加** | 段階 ② の追加機能（タイムライン・呼称・五感・文体・統計）が入る時点でタブを増やす。 |
| `project.json` の追加キー | **段階 ① から読み飛ばして温存** | `#[serde(flatten)]` によりパースで落とさず保存時も保持する（`REF:tests/roundtrip.rs` で検証済み）。 |
| 読者モード（#27） | **段階 ② へ移管** | 縦書き組版の工数・技術的不確実性と `FontMetrics` 注入設計のため段階 ② へ移管。アノテーション除去エクスポート（§17.1 / §17.2）は段階 ① で実装済み。 |
| `materials/inbox/` | **段階 ① で実装済み** | 資料の提案→承認ワークフロー（§5.8.1 / §16.1）として段階 ① で実装済み（`REF:src/core/features/materials_ops.rs`）。 |
| `prompts/custom/` | **段階 ②（#12）で導入** | カスタムロール（追加仕様 #12）に完全に閉じているため、段階 ① の雛形ではディレクトリを作らない。 |

---

### 4.4 段階 ② のための必須制約（5 原則）と縫い代（10 項目）の確認表（旧計画書 §1.5・§7）

段階 ② の 27 項目を全面改修なしに追加できるよう、段階 ① の実装時点で以下の 10 項目の「縫い代」を仕込み済みであり、今後の変更でもこれを壊してはならない（MUST）。

| # | 縫い代 | 段階 ① での具体的な実装箇所 | 違反したときの段階 ② の手戻りコスト |
|---|---|---|---|
| 1 | 未知キーの温存 | `project.json`（`REF:src/core/model/project.rs`）・`ui.json`（`REF:src/app/settings.rs`）・`index.json`（`REF:src/core/model/material.rs`）を `#[serde(flatten)]` で受け、`REF:tests/roundtrip.rs` で検証 | 設定ファイルの前後互換が切れる |
| 2 | 未知フィールド行の温存 | A/B/MD 形式のパーサ（`REF:src/core/syntax/plot_outline.rs` 等）が未知 `key:` に `W011` を出しつつ行を無損失保持 | `when:` `senses:` `space:` `blurb:` `illustration:` を書いたファイルが壊れる |
| 3 | 診断コードの予約 | `W080`+ / `E032`+ / `E064` / `I054`+ を `REF:src/core/diagnostics.rs` に先行定義し別用途に使わない | 診断番号の振り直しが全体に波及する |
| 4 | 行頭シジルの表駆動 | `REF:src/core/text/lexer.rs` の `const SIGIL_TABLE: &[SigilSpec]` を走査して判定 | `# ` や `! ` の追加で字句器を書き直すことになる |
| 5 | ワークスペース状態の非破棄 | `REF:src/ui/workspaces/mod.rs` の `Switcher` が全ワークスペースの状態構造体を常時保持 | 16ms 切替要件（追加仕様 #1）が達成不能になる |
| 6 | 参照ペインのセクション配列化 | `REF:src/core/features/reference.rs` が `Vec<Section>` を返し、`REF:src/core/resolve/context_rules.rs` の `PaneSection` が全 9 種を定義 | 五感・空間・挿絵セクションの挿入で再構成が必要になる |
| 7 | 資料 ID 接頭辞の 1 文字化 | `REF:src/core/model/material.rs` の `MaterialId` が `M` と `S` の双方を受理 | `S####`（検索結果資料化 #11）導入で既存データの再採番が必要になる |
| 8 | 診断の実行コスト属性の分離 | `REF:src/core/diagnostics.rs` のカタログが `Cost`（常時 / アイドル / 明示）と既定 ON/OFF を保持 | §7.2 の文体診断（既定オフ・明示実行）の制御が入らなくなる |
| 9 | 文字数計測点の集約 | 空白除外文字数・原稿用紙枚数の計算を共通関数に集約 | 台詞比率・統計・偏差値で文字数算法が割れる |
| 10 | ロール一覧の配列管理 | 既定 7 役割を `BUILTIN_ROLES` 配列と `Role` で扱い、`REF:src/core/syntax/prompt_md.rs` に `Options::custom` を用意 | カスタムロール（#12）追加で全分岐を書き直すことになる |

---

### 4.5 段階 ②（追加仕様 1〜27 および U-7 / U-8）の受け入れ基準（旧仕様書 §19.14 完全収録）

**19.14-1 ワークスペース瞬時切替**
- [x] `Alt+1..9` の切替が 16ms 以内に完了し、ローディング表示・アニメーションが出ない。
- [x] `Ctrl+Shift+1..9` が §10.0 の対応表どおりの節に着地する。
- [x] `Ctrl+\` で参照ペインが即座に消え、メインカラムが全幅になる。隠している間も内容の計算が続いている。
- [x] `Alt+0` で直前のワークスペースに戻る。

**19.14-2 セッション情報**
- [x] ステータスバーに `+n字 m分` が出る。削ったときマイナス表示になる。
- [x] 30 分の無操作でセッションが切れ、次の打鍵から新しいセッションが始まる。
- [x] 既定で「毎分の速度」が表示されていない。

**19.14-3 タイムライン**
- [x] `when: 405 春分 09:15` / `when: +1` / `when: 12 .. 14` / `when: ?` が仕様どおり解釈される。
- [x] 作中時間が巻き戻る箇所に W080 が出る。`回想` を含む場合は出ない。
- [x] `when` が 1 つも無いプロジェクトでタイムラインタブが表示されない。
- [x] タイムラインビューから `when` を編集できない。

**19.14-4 句切れ提案**
- [x] 既定でオフ。`Ctrl+Alt+,` でオン。
- [x] 候補位置に縦線だけが出て、文字が挿入されない。
- [x] `「…」` の内側に候補が出ない。
- [x] 同じ入力に対して常に同じ候補が出る（決定論）。

**19.14-5 連続台詞**
- [x] `max_consecutive_speeches` を超えた連続で W082 が出る。
- [x] 地の文を 1 行挟むとカウントがリセットされる。

**19.14-6 台詞比率**
- [x] ミニヘッダーのバーが §9.4.2 の算法どおりの値を示す。
- [x] 目標から外れても色が変わらない。
- [x] 話プロットの節リストにも同じバーが出る。

**19.14-7 五感**
- [x] `senses:` を書いた節だけが集計対象になる。
- [x] 本文を解析して感覚を推定する処理が存在しない（コードレビュー項目）。
- [x] 文体タブの文言が「指摘」ではなく「観測」の語で書かれている。

**19.14-8 呼称**
- [x] `address:` の `=>` 右側に `:` を含めても呼称の一部として読まれる。
- [x] `address` 未定義のキャラでは W084 が出ない。
- [x] 呼称マトリクスでセルを編集すると `characters.md` に書き戻る。
- [x] 登場人物 25 人以上で表形式が自動的に 1 行表示に切り替わる。

**19.14-9 折りたたみ**
- [x] 章・話・場面の単位で畳める。現在の節を含む単位は畳めない。
- [x] 状態が `.novnow/ui.json` に保存され、再起動後に復元される。
- [x] 検索ヒットのある単位が自動展開される。
- [x] エクスポート結果が折りたたみ状態に影響されない。

**19.14-10 資料引用追跡**
- [x] `^M0137` が本文文字数に数えられない。
- [x] 未知の ID で E012 が出る。
- [x] エクスポート結果に `^M` が残らない（`REF:tests/export_invariants.rs`）。
- [x] どこからも引用されていない資料が I054 で通知される。

**19.14-11 検索結果の資料化**
- [x] `S####` が `M####` と同じ `next_id` から採番される。
- [x] 保存後に本文を書き換えても資料の内容が変わらない。
- [x] `[再検索]` が差分を表示し、自動では更新しない。

**19.14-12 カスタムロール**
- [x] `agents.custom_roles` に登録したものだけが UI に出る。
- [x] 委譲レベル `propose` では、カスタムロールの出力が `agents/proposals/` にしか書かれない。
- [x] `context:` で `project` と `constraints` を外せない。
- [x] 既定ロール名と同名で E033 が出る。

**19.14-13 ローカル LLM**
- [x] 設定すると `agents/mcp.json` に `managed_by: "novnow-local-llm"` 付きのサーバが書かれる。
- [x] 手書きのサーバ定義が書き換えられない。
- [x] localhost 以外の URL が拒否される。
- [x] `REF:Cargo.toml` に LLM プロバイダ SDK が無い（`REF:tests/no_llm_sdk.rs`）。

**19.14-14 Web 解説ドキュメント**
- [x] `GIT_GUIDE_DOC_URL` / `CICD_GUIDE_DOC_URL` がハードコードされず定数経由である。
- [x] リンクが外部ブラウザで開く。
- [x] novnow が `.git/` を読まず、`git` コマンドを実行しない（コードレビュー項目）。

**19.14-15 `# ` 行コメント**
- [x] `.txt` で行頭 `# ` がコメントになり、`.md` では見出しのままである。
- [x] `#1` のように空白が無い場合はコメントにならない。
- [x] エクスポート結果に `# ` 行が残らない。

**19.14-16 提案モード**
- [x] 提案モード中、本文ファイルが 1 バイトも変わらない。
- [x] `agents/proposals/H####.md` に `before` / `after` が節全文で書かれる。
- [x] エディタ上に差分が重ねて表示されない。
- [x] 提案の文字数が統計・進捗に含まれない。

**19.14-17 あらすじ**
- [x] `blurb` が本文文字数に含まれない。
- [x] `export.include_blurb = false` のときエクスポートに現れない。
- [x] 読者モードの章表紙に表示される。
- [x] `blurb` が無くても警告が出ない。

**19.14-18 文体・品質分析**
- [x] 総合点・スコア・評価が表示されない。
- [x] 各指標を個別にオフにできる。
- [x] 既定で W081/W087/W088/W089/W090 が編集中に出ない。
- [x] 各指摘から `Enter` で該当箇所へ飛べる。

**19.14-19 正規表現検索**
- [x] 1 秒のタイムアウトで中断し、部分結果が出る。
- [x] 構文エラーが 1 行で説明される。
- [x] 置換がプレビュー必須で、1 つの undo 単位になる。
- [x] `.novnow/` と `exports/` が検索対象外である。

**19.14-20 統計ダッシュボード**
- [x] `.novnow/stats.json` に日次集計のみが書かれ、イベントログが作られない。
- [x] ファイルを削除しても他の機能に影響が無い。
- [x] 完成予測・達成率・締切が表示されない。
- [x] `Ctrl+Alt+S` で統計モードに切り替わり、パンくず・診断・保存表示が消えない。

**19.14-21 空間設計メモ**
- [x] `space-mode: ascii` で等幅・折り返し無しで表示される。
- [x] 外部ファイル参照がプロジェクト外を指すと E062 になる。
- [x] 図の内容を解釈する処理が存在しない（コードレビュー項目）。

**19.14-22 00 章ストック**
- [x] `@00-01-02-03` が正当な SceneId としてパースされる。
- [x] ツリーでストック章が最後に表示される。
- [x] 配属操作が原子的に行われ、途中失敗で全巻き戻しされる。
- [x] 範囲「全体」のエクスポートにストック章が含まれない。

**19.14-23 挿絵メモ**
- [x] `! ` 行が本文文字数に数えられない。
- [x] エクスポートで行ごと消える。`--keep-illustration-memo` では `［挿絵: …］` になる。
- [x] `! ` 行と `illustration:` が §10.9 の一覧に統合され、出典欄で区別される。

**19.14-24 キャラアイコン**
- [x] `assets/` 外を指すと E062 になる。
- [x] 本文エディタ内にアイコンが出ない。
- [x] `icon` を 1 件も定義していないプロジェクトでアイコン用の余白が無い。
- [x] ファイルが無くてもエラーにならず W070 が出る。

**19.14-25 キャラ別・節長統計**
- [x] 話者不明の台詞が `（不明）` として計上され、総和が本文の台詞総量と一致する。
- [x] 節長のヒストグラムと T スコア外れ値が出る。

**19.14-26 分量偏差値**
- [x] 母集団が 3 未満のとき計算されず `—` と表示される。
- [x] 標準偏差 0 のとき全単位が T 50 になる。
- [x] 文字数 0 の単位が母集団から除外される。
- [x] `⚠` が赤ではなく橙である。

**19.14-27 読者モード（縦書きプレビュー）**
- [x] 縦書き（右上から左下へ、行は右から左へ）で表示される。
- [x] 禁則処理・縦中横・ラテン文字 90° 回転・句読点の右上寄せ・モノルビが §17.3.3 のとおり動く。
- [x] `core::reader::layout()` が純粋関数であり、フォント実測値は引数で注入されている（§3.2 を破っていない）。
- [x] 読者モードで注釈・診断・ヒントが一切表示されない。
- [x] どのキー操作でも原稿・プロット・資料・共有設定が変更されない（読書ローカル状態の保存を除く）。
- [x] 1 ページのレイアウト計算が 3ms 以内（§18.1）。
- [x] `Alt+7` が読者モードに割り当てられる（段階 ① で欠番にしていた枠が埋まる）。

**19.14-U7 既定プロンプトの復元（段階 ① M10 で達成済み）**
- [x] 既定プロンプトがバイナリに埋め込まれ、ディスク上のどこにも恒久的に存在しない。
- [x] 本体を更新しても `prompts/` が上書きされない。
- [x] `>prompt reset` が `.bak-*` を作らない。
- [x] 復元前に現行版と既定版が並置表示され、「元に戻せません」と明示される。
- [x] プロンプトのバージョン・世代・履歴を保持する仕組みが存在しない（コードレビュー項目）。

**19.14-U8 並列実行（段階 ① M10 で達成済み）**
- [x] `Shift+R` で選択した複数タスクが `max_parallel` 件ずつ走る。
- [x] 同一 SceneId のタスクが同時に走らない。
- [x] 同一 MCP サーバへの同時接続が 2 を超えない。
- [x] 提案 ID と資料 ID が並列実行でも衝突しない。
- [x] 1 件失敗しても他が続行し、自動再試行されない。
- [x] integrator 以外の役割は、委譲レベルが `autonomous` でも他のタスクを起動する経路が存在しない（コードレビュー項目）。
- [x] `autonomous` で自動起動されたタスクが `起動: agent` と記録され、セッション上限で止まる。
- [x] 本文への適用（採用でも自動適用でも）が逐次である。

---

<a id="stage-3"></a>
## 5. 段階 ③ 実装計画（§19 外部インターフェース：ヘッドレス全機能 API 群）

2026-10-21 のユーザー指示により、次の 3 点が承認・確定した: (1) 文書群へ 11 番目のファイル対 `REF:docs/specifications/interfaces.md` / `REF:docs/designs/interfaces.md`（§19 外部インターフェース）を追加する（developers.md のファイル対規則 10 → 11 の改正を含む）、(2) Python API は PyO3 ベースのネイティブ拡張モジュール（インプロセス直接結合。サブプロセスやネットワークのラッパーではない）として実装する、(3) 完全な仕様の全文を仕様書へ直接収録し、実装計画を本節に定める。

**番号の曖昧性解消**: §4.3・§4.5 に見える「旧仕様書 §19.13 / §19.14」は廃止済み単一ファイル `SPECIFICATIONS.md` の受け入れ基準章を指し、新設の `REF:docs/specifications/interfaces.md` の §19（外部インターフェース）とは別物である。章番号の再利用は旧ファイルの 10（現在 11）分割時に確定した採番規則による。

### 5.1 H0（基礎文書整備完了）: ドキュメント整備

- [x] `REF:docs/specifications/interfaces.md` を新設: §19.1 共通原則〜§19.8 版・互換性・配布。§19.7 は正規操作カタログで、インターフェースごとの例外と関門の印を定義する。
- [x] `REF:docs/designs/interfaces.md` を新設: 見出しは仕様書と完全一致させ、8 つの `###` 節すべての `状態:` を「未実装・実装計画済み」とした。
- [x] `REF:docs/developers.md` §1.1 項目 3 の対応表と §3 ディレクトリツリーのファイル数を 11 へ更新した。
- [x] `REF:docs/script/duplicate.py` の末尾 OK 行の個数表示を、実ファイル一覧に基づく動的なものへ変更した。
- [x] SYNC 進捗表（本書 §1 と `REF:docs/handoff.md` §1 の共有ブロック）へ段階 ③ 行を追加し、設計書節の集計を実測値へ更新する。現在の数値の正本は SYNC 進捗表と文書検査の出力。
- [x] `python3 docs/script/duplicate.py` がエラーゼロで通過した。

**段階②統合後の追補（H1 着手前）**
- [x] §19.7 のカタログと段階②の機能対応表（本書 §4.1）を再照合し、読者・統計・呼称マトリクス・五感・正規表現検索等の操作 ID、引数、例外と権限を確定した。結果は下表（追加仕様 27 件すべてが既存の操作で到達できる。専用操作の新設は不要だった）。H0 の基礎文書追加と、GUI 全機能の API 網羅確定は別物として扱い、後者をここで確定した。

| # | 追加仕様 | 到達する正規操作（`REF:tests/golden/api_catalog.json` の ID） |
|---|---|---|
| 1 | ワークスペース・参照ペインの瞬時切替 | `session.workspace_get` / `session.workspace_switch` / `reference.pane` / `nav.switcher` |
| 2 | ステータスバーの執筆セッション情報 | `project.stats` / `project.info` |
| 3 | タイムライン・作中日時（`when:`） | `cast.timeline` / `plot.get` / `diagnostics.run`（W080） |
| 4 | 自動句切れ提案（決定論・既定オフ） | `lang.hints` / `settings.set`（`suggest_comma`）/ `session.command` |
| 5 | 同一話者の連続台詞検出 | `diagnostics.run`（W082）/ `settings.set` |
| 6 | 台詞 / 地の文 比率 | `diagnostics.run`（W090）/ `review.list` |
| 7 | 五感チェック（`senses:`） | `review.list` / `diagnostics.run`（W083）/ `reference.pane` |
| 8 | 呼称マトリクス（`address:`） | `dictionary.register_character` / `dictionary.modes_list` / `cast.get` / `diagnostics.run`（W084・W085） |
| 9 | 構造の折りたたみ | `plot.outline_fold` / `lang.folding` / `settings.set`（`folded`） |
| 10 | 資料引用追跡（`^M0137`） | `materials.list` / `materials.get` / `lang.tokens` / `diagnostics.run`（I054） |
| 11 | 検索結果の資料化（`S####`） | `materials.propose` / `materials.search` / `search.project` / `diagnostics.run`（I058） |
| 12 | エージェントのカスタムロール | `prompt.list` / `prompt.get` / `agent.delegation_get` |
| 13 | ローカル LLM 簡易モード | `agent.mcp_config_get` / `agent.mcp_config_validate` / `settings.set` |
| 14 | Web 解説ドキュメント | `help.topic` |
| 15 | `# ` 行コメント | `lang.tokens` / `export.text` |
| 16 | 人間用トラックチェンジ（提案モード） | `agent.proposal_list` / `agent.proposal_get` / `agent.proposal_apply` / `agent.proposal_reject` |
| 17 | あらすじ（`@A-00` / `blurb:`） | `export.text`（`blurb`・`blurb_only`）/ `plot.get` / `diagnostics.run`（W086） |
| 18 | 文体・品質分析 | `review.list` / `diagnostics.run`（W081・W087〜W089） |
| 19 | 全体検索の正規表現対応 | `search.project`・`search.file`（`regex`。大小文字の区別は正規表現 `(?i)` で表現する＝汎用操作で覆う） |
| 20 | 執筆データ統計ダッシュボード | `project.stats` / `review.list` |
| 21 | 空間設計メモ（`space:`） | `review.list` / `reference.pane` / `diagnostics.run`（W092） |
| 22 | 「00 章」ストック | `structure.tree` / `section.list` / `settings.set`（`allow_chapter_zero`）/ `export.text`（`stock`）/ `diagnostics.run`（I055） |
| 23 | 挿絵メモ（`illustration:` / `! `） | `review.list` / `reference.pane` / `export.text`（`keep_illustration_memo`） |
| 24 | キャラアイコン（`icon:`） | `cast.get` / `cast.list` / `diagnostics.run`（W068〜W070） |
| 25 | キャラ別・節長の統計強化 | `project.stats` / `cast.get` |
| 26 | 話の長さバランス偏差値 | `project.stats` / `diagnostics.run`（W091） |
| 27 | 読者モードの組版 | `export.text`（`format`・`range`・`state`）/ `nav.file` |

仕様書・設計書の変更は発生しなかった（既存の汎用操作で覆えるため、操作 ID の追加も不要だった）。機械検証は `REF:tests/api_parity.rs`（仕様書 §19.7 の表 ↔ `OPS` 表）と `REF:tests/golden/api_catalog.json`（引数名の正本）が行う。

### 5.2 共通方針: 単一実装点を先に確立する

- どのインターフェースのマイルストーンも、前提は H1 である。`REF:docs/designs/interfaces.md` §19.1 が挙げる状態機械と `Host` トレイトを新設 `src/app/` 層へ移し、旧パスには `pub use` だけを残す。§4.4 の縫い代 #5（`Switcher`）も再エクスポートで生き続けるため、縫い代表の参照先は壊れない。
- インターフェース固有のコードが機能ロジックを再実装することを禁止する。インターフェース層の責務は引数・出力・エラーの翻訳に限る（§19.1.4 単一の真実）。
- 新規依存クレートは H6 の PyO3 系に限定する。H1〜H5 は新規クレートゼロで達成する（LSP のフレーミングは `REF:src/adapters/mcp_stdio.rs` と同じ手書き方式を踏襲）。

### 5.3 マイルストーン一覧

| # | マイルストーン | 主要成果物 | 依存 |
|---|---|---|---|
| H1 | app 層の抽出 | `src/app/` 新層（8 状態機械の移設 + Host）、層規則、GUI からの委任、overview.md §3.3 改訂 | H0 |
| H2 | Rust API | `Session` ファサード、`OPS` ディスパッチ表、ドメインハンドル、イベントキュー、メモリホスト、`tests/api.rs` | H1 |
| H3 | CLI 拡張 | ドメイン動詞文法、`--json` / `--lang`、`api list`、`lsp` 入口、終了コード網羅 | H2 |
| H4 | MCP 拡張 | ツール表 14 → 86（凍結 14 + 「M—」の無い 72）、既存 14 件のバイト凍結回帰、`plot.write` level 汎用化 | H2 |
| H5 | LSP サーバ | `src/lsp/` 5 モジュール、`novnow/*` 通知 4 種、`tests/lsp.rs` | H2 |
| H6 | Python API | `python/novnow-py`、PyO3 vendoring、`.pyi` 生成、wheel CI、pytest 一式 | H2（vendoring は H1 から並行着手可） |
| H7 | パリティと公開 | `tests/api_parity.rs`、カタログゴールデン、website API リファレンス、設計書状態更新、履歴移設 | H3〜H6 |

H3・H4・H5 の 3 系統は相互に交差せず、H2 完了後は任意の順で並行できる。

### 5.4 マイルストーンごとの受け入れ基準（全件達成 — サマリへ圧縮済み）

H1〜H7 の受け入れ基準（各 4〜5 件、計 29 件）はすべて達成した。個条の一覧は
`REF:docs/history.md` の `[HIST-TAG: STAGE-3]` エントリへ移し、ここには達成の要点と
検証の入口だけを残す。

| # | マイルストーン | 基準 | 達成の要点（検証の入口） |
|---|---|---|---|
| H1 | app 層の抽出 | 4 / 4 | 8 状態機械と `Host` を `REF:src/app/` へ移設し旧パスは再エクスポート。`REF:tests/layering.rs` が GUI クレート禁止を強制。`REF:docs/designs/overview.md` §3.2・§3.3 を改訂 |
| H2 | Rust API | 4 / 4 | `Session` + `OPS` 119 操作。`REF:tests/api.rs`（メモリホストのみ）・`REF:tests/api_fs.rs`（実ディレクトリ） |
| H3 | CLI 拡張 | 4 / 4 | ドメイン動詞文法・`--json`（§19.6.2）・`--lang`・終了コード 0/2/3/4/5・`api list` / `lsp`。既存サブコマンドの出力バイトは不変（`REF:tests/cli.rs`） |
| H4 | MCP 拡張 | 4 / 4 | ツール 14 → 86（凍結 14 + 「M—」の無い 72。全 119 操作中 35 件は非公開）。既存 14 件の定義 JSON はバイト一致で凍結（`REF:tests/golden/mcp_tools.json`）。安全除外 4 件・`role` 検査（`REF:src/core/agents/server.rs`） |
| H5 | LSP サーバ | 4 / 4 | `REF:src/lsp/` 6 モジュール。`executeCommand` 121 名・`novnow/*` 通知 4 種・UTF-16 往復（`REF:tests/lsp.rs`） |
| H6 | Python API | 5 / 5 | `python/novnow-py`（abi3-py39）・GIL 解放（`REF:src/app/driver.rs`）・`.pyi` + `py.typed`・5 構成 wheel CI（`REF:tests/api_driver.rs`） |
| H7 | パリティ・公開 | 4 / 4 | `REF:tests/api_parity.rs`・`REF:tests/golden/api_catalog.json`・ウェブサイト `/ja/api`・`/en/api`（`REF:scripts/api-site.py`）・設計書 8 節「実装完了」 |

**§19.3.4 の GIL 解放について**: `Session` は `Send` にできない（Web の `WebHost` が
`Rc<RefCell<..>>` を持つので `Host: Send` を supertrait にできない）。そこで `Session` を
待機区間へ持ち込むのではなく、ワーカースレッド 1 本に所有させる `REF:src/app/driver.rs`
を挟み、`Python::allow_threads` はその 1 往復を包む。設計書 `REF:docs/designs/interfaces.md`
§19.3 の「pyo3 の借用規則で直列化」という記述は、実装では「`Mutex<Driver>` + ワーカー
スレッド 1 本による直列化」で満たしている（§19.3.4 の 2 つの MUST はどちらも成立）。
この設計書の語句の修正は人間の承認を要するため、PR で判断を仰ぐ。

**未検証（意図的）**: pyo3 系クレートはこのサンドボックスから crates.io に到達できないため
`maturin build` と `pytest` を実行していない。`REF:.github/workflows/wheels.yml` のホイール
行列（5 構成 × Python 3.9 / 3.11 / 3.13）が初回の検証になる。Rust 側の GIL 解放の型条件
（`Driver: Send` と待機区間をまたぐ全型）は `REF:tests/api_driver.rs` が検証済み。

### 5.5 PyO3 系の vendoring 手順（状態: 完了。サンドボックスで拡張をビルドできる）

`REF:docs/add-dependencies.md` §2.2 の方法 1（Git Blob 差分追加）に従う。対象クレート一覧は `cargo vendor` の解決結果を正本とし、本節には写さない（写すと版本更新のたびに古くなる）。

**用意できているもの**

1. `REF:.github/workflows/sandbox-tools.yml` の `pyo3-crates` ジョブ。版本は `python/novnow-py/Cargo.toml` から読む（別々に書くとずれるため）。`cargo vendor` の出力を孤立ブランチ `cache/pyo3-crates` へ push し、`SHA256SUMS` / `PYO3_VERSION` / `CRATES` を添える。
2. `REF:scripts/bootstrap-toolchain.sh` の `install_pyo3_vendor`。`cache/pyo3-crates` を取り出し、sha256 を検証して `$BUILD_DIR/vendor-src/` へ**既存を上書きせずに**合わせる（`libc` など重複しうるクレートを守る）。ブランチが無ければ警告して続き、本体のビルドは止めない（`PYO3_BRANCH` で別ブランチを指定できる）。

**2026-10-23 に完了した手順（実績）**

1. 人間が Actions から `sandbox-tools` を `Run workflow`（ref = 作業ブランチ）。`pyo3-crates` ジョブは `workflow_dispatch` でしか走らない（`.github/sandbox-tools.env` は `ENABLED=0`）し、サンドボックスの `GITHUB_TOKEN` は `actions: write` を持たないので dispatch できない（実測: `gh workflow run sandbox-tools.yml` と `gh api -X POST .../dispatches` の双方が HTTP 403 Resource not accessible by integration）。
2. `bash scripts/bootstrap-toolchain.sh` → `cache/pyo3-crates` を展開。**追加 10 クレート**（heck, indoc, portable-atomic, pyo3, pyo3-build-config, pyo3-ffi, pyo3-macros, pyo3-macros-backend, target-lexicon, unindent）、**既存 10 クレートは再利用**（autocfg, cfg-if, libc, memoffset, once_cell, proc-macro2, quote, rustversion, syn, unicode-ident）。
3. `cd python/novnow-py && cargo generate-lockfile --offline`（98 パッケージ、pyo3 0.22.6）→ `cargo clippy --offline --all-targets -- -D warnings`（緑）→ `cargo build --release --offline`（`target/release/lib_core.so` 8.6MB）。
4. `.so` を `python/novnow/_core.so` へ置いて `PYTHONPATH=python python3 -m pytest tests`（**8 passed, 1 skipped**。skip は本物の拡張がある環境で `test_binding.py` が自動的に外れる分）。`REF:scripts/python-check.sh --full` がこの 3〜4 を回す。
5. 生成した `REF:python/novnow-py/Cargo.lock` をリポジトリへ置いた（`python/novnow-py` は root の `[workspace] exclude` なので独自の lock を持つ）。

**この過程で見つかった実装の誤り（拡張をビルドできるようになって初めて出た）**

- `Session::take_events` が「コールバックへ渡す」と「列を取り出す」を 1 つにしていたため、操作のたびに `deliver()` が列を捨て、`poll_events()` が常に空になっていた（§19.3.5 違反）。`collect_events`（積む）と `take_events`（取り出す）へ分離。
- `Py<T>: Clone` は pyo3 0.22 で `py-clone` feature の behind（既定オフ）。`Cargo.toml` で有効化した（`take_events` は常に GIL 下で走る）。
- `add_function` は `Bound` を値で受ける、`Driver::*` の `Result<_, novnow::Error>` は `map_err(into_error)` が要る、`to_python` は `PyResult` を返すので平坦化が要る（8 件）。
- clippy 1.99 は pyo3 の `#[pyfunction]` / `#[pymethods]` 展開に `useless_conversion` を 11 件言う（全件がマクロ展開の中。指摘の列が全て戻り値の `->` を指す）。`[lints.clippy]` で許可し、理由を `Cargo.toml` に書いた。
- `REF:python/novnow-py/tests/test_api.py` が `structure.tree` に `count` を期待していたが、正本の形は `{level, nodes}`（`REF:tests/delegation.rs` と同じ）。テスト側を実態へ合わせた。

**残っているもの**

- `python/novnow-py` は root の `[workspace] exclude` のまま（pyo3 を本体の依存解決へ混ぜず、wasm ビルドを守るため）。正式な wheel 検証は `REF:.github/workflows/wheels.yml`（5 プラットフォーム ＋ Python 3.9 / 3.11 / 3.13 の `pytest python/novnow-py`）が担う。
- `REF:tests/layering.rs` の許可クレート検査と `REF:docs/designs/overview.md` §3.2 の依存表の更新は**人間の承認が必要**なので未着手（pyo3 系は `python/novnow-py` 内に閉じ、本体の層には入らない）。

推移的依存が 20 個を超えた場合は方法 2（`cache/rust-toolchain` 全面再生成、`REF:.github/toolchain.env` の SALT 更新）へ切り替える。新規クレートは `python/novnow-py` 内に閉じるため、LLM SDK 混入の余地はない（`REF:tests/no_llm_sdk.rs` はグリーンのまま）。

### 5.6 テスト戦略（H1〜H7 横断。状態: 未達の 1 件を除き実装済み）

| 決めたこと | 状態 | 実体 |
|---|---|---|
| パリティ契約テストは仕様書 §19.7 の Markdown 表を直接読む（`REF:tests/diagnostics_catalog.rs` と同じ `include_str!` 方式）。カタログの手書き写しはリポジトリへ置かない | 実装完了 | `REF:tests/api_parity.rs` |
| ゴールデン（CLI 出力・api_catalog.json・MCP 既存 14 定義）は `REF:docs/developers.md` §7.2 の生成手順に従い、手編集を禁止する | 実装完了 | `REF:examples/api_catalog.rs` → `REF:tests/golden/api_catalog.json`、`REF:examples/mcp_tools.rs` → `REF:tests/golden/mcp_tools.json`（検査は `REF:tests/mcp_frozen.rs`） |
| LSP はスクリプト化した JSON-RPC で実 stdio 配管を駆動してテストする。外部のテストフレームワークへ依存しない | 実装完了 | `REF:tests/lsp.rs` |
| 正規操作 119 件を 1 回ずつ叩く網羅テストと、カバレッジの可視化 | 実装完了 | `REF:tests/api_coverage.rs`（表は `target/debug/api-coverage.txt`） |
| Python の位置引数の束縛（§19.3.2）と 9 例外（§19.3.3）を**拡張のビルド無し**で検査する | 実装完了 | `REF:python/novnow-py/tests/test_binding.py`（`novnow._core` を偽物にする。10 件） |
| 型スタブ（§19.3.1）は手書きせずカタログから生成する | 実装完了 | `REF:scripts/pyi-site.py` → `REF:python/novnow-py/python/novnow/__init__.pyi` |
| `REF:scripts/verify.sh` の `--python` 段階と `REF:.github/workflows/ci.yml` の python ジョブ | 実装完了 | `REF:scripts/python-check.sh`（`verify.sh` の既定にも入り、`--python` で単独実行。計画では「既定オフ」だったが 1 秒で終わるので既定オンにした） |
| サンドボックスで release ビルドの `.so` を `PYTHONPATH` でスモークする | 実装完了 | `REF:scripts/python-check.sh` `--full`（clippy → release ビルド → `python/novnow/_core.so` へ置いて pytest。8 passed, 1 skipped）。pyo3 の vendor は §5.5 |

Python 一式は `python/novnow-py/tests/`（当初の計画に書いた `python/tests/` ではない）。`test_api.py` は本物の拡張が要るので wheel 検証（`REF:.github/workflows/wheels.yml`）で走り、`test_binding.py` は逆に本物がある環境では自動で skip する（`sys.modules` を汚さないため）。

### 5.7 リスクと対策

| リスク | 深刻度 | 対策 |
|---|---|---|
| 移設する状態機械に egui 型が染み出していて H1 が難航する | 高 | 8 モジュールの型依存を先に機械列挙。染み出し点はトレイト化して ui 側に残す。移設は 1 モジュール 1 コミットで個別巻き戻し可能にする |
| PyO3 の推移的依存が Blob 方式の想定を超える | 中 | Cargo.lock 差分で先に個数を確定。20 個超ならパターン B-2 へ移行 |
| 生成スキーマが MCP 既存 14 ツールの凍結を壊す | 中 | 既存 14 件はバイト比較ゴールデンで固定し、生成経路と分離する |
| LSP の UTF-16 変換が CJK 本文の位置をずらす | 高 | `REF:src/core/text/position.rs` の実績ある LineTable を採用し、往復プロパティテストを追加 |
| `OPS` 表の肥大でコンパイル時間が増える | 低 | ハンドラは関数ポインタ、スキーマは遅延初期化。bench ジョブで回帰を監視 |

### 5.8 段階の受け入れ基準（全マイルストーン共通）

- [x] 仕様 §19 の各 MUST 条項に対応する自動テストが存在し、`bash scripts/verify.sh`（`REF:tests/api.rs`・`REF:tests/api_fs.rs`・`REF:tests/api_driver.rs`・`REF:tests/cli.rs`・`REF:tests/lsp.rs`・`REF:tests/api_parity.rs`）が完全グリーン（1347 passed / 0 failed、65 ターゲット。GUI なしのビルドは 1176 件）。
- [x] §4.4 の 10 縫い代が生存 — `REF:tests/layering.rs` と `REF:tests/roundtrip.rs` の既存アサーションが無変更で通る（段階 ③ で両ファイルのアサーションは変更していない）。
- [x] 仕様書・設計書・許可クレート表の変更はすべて PR 経由で人間の承認を得た。**完了**: PR #25 は 2026-10-23 にマージ済み。対象は `REF:docs/designs/overview.md` §3.2・§3.3 と `REF:docs/designs/interfaces.md` の状態行。許可クレート表（`REF:docs/add-dependencies.md`）の変更は無い（pyo3 系は vendoring で持ち込み、クレート依存には加えていない）。
- [x] 完了後、`REF:docs/history.md` へ `[HIST-TAG: STAGE-3]` エントリを追記し、本書と `REF:docs/handoff.md` の SYNC 進捗表を更新した（134 節すべて実装完了）。

---

<a id="users-and-guide-mode"></a>
## 6. ユーザー向けガイドとガイドモードの実装計画（状態: 一部実施。Phase 0〜4 は実施済み、Phase 5（英語本文・GIF）は未実施）

非エンジニアが「開いて、書いて、書き出せる」までを、**文書・サイト・アプリの 3 か所で同じ内容**にする計画。
仕様は `REF:docs/specifications/ui.md` §9.10（ガイドモード）、`REF:docs/specifications/workflows.md` §18.5
（ドキュメント公開）、`REF:docs/specifications/platform.md` §14.2.1（解説ページ URL）に置いた。

**前提（決定済み）**

- バイナリ配布は後回し。インストールの節は**場所だけ作り、中身は「準備中」**とする。
- 用語（節・場面・話・モード・役割語）は仕様書の語のまま使う。**言い換えず、定義と説明を足す**。
- `REF:docs/users/` は `REF:docs/specifications/` を参照する（逆向きの参照は持たない）。
- サイトは仕様書とユーザー向けガイドを**どちらも docs から生成**し、ガイドから仕様書へリンクする。
- ガイドモードは、Web 版は保存プロジェクト 0 件、デスクトップ GUI 版はプロジェクト履歴 0 件のときだけ自動で出す。

### 6.1 段階

<a id="plan-users-docs"></a>
#### Phase 0: 壊れている導線を直す（半日）— **実施済み**

`REF:scripts/docs-site.py` が `REF:docs/users/*.md` から `REF:guide/*.html`（8 ページ）を作り、
`REF:index.html` の `copy-dir` で dist に載る。仕様 §14.2.1 の 3 URL は公開物に実在する。
存在は `REF:docs/script/reference.py` の検査 11 が CI で見張る。


| # | すること | 完了条件 |
|---|---|---|
| 0-1 | `REF:docs/specifications/platform.md` §14.2.1 の 3 URL に対応する解説ページを作る（`guide/project_structure.html`・`guide/git.html`・`guide/publish.html`）。`REF:index.html` にコピー指定を足し、trunk の dist に載せる | 3 URL が実際に開ける。中身はユーザー向けガイドとサイトへの短い案内 |
| 0-2 | `REF:docs/script/reference.py` に検査 11 を足す（下記 6.3） | 3 URL を消すと `verify.sh` が落ちる |
| 0-3 | CLI の不足ファイル案内（`REF:src/cli/mod.rs`・`REF:src/cli/open.rs`）の文面を平易にする | 済（確認の結果、仕様 §14.2.2 の文面が既に「足りないもの・構造の解説・`novnow new`」の 3 段で案内していたため変更なし） |

<a id="plan-guide-mode"></a>
#### Phase 1: ガイドモード（3 日）— **実施済み**

`REF:src/ui/guide.rs`（9 段階の定義と状態機械）・`REF:src/ui/widgets/guide.rs`（枠・矢印・吹き出し）・
`REF:src/ui/app.rs`（自動で出る判定・状態の観測・キー）・`REF:src/app/settings.rs`（`guide_done`）に実装した。
キーは `Alt+Enter`／`Alt+Backspace`／`Esc` で、**本文の `Enter` と `Backspace` は奪わない**。


| # | すること | 置き場所（予定） |
|---|---|---|
| 1-1 | 段階の定義を 1 か所に持つ（文言 ja/en・進む条件・対象の部品 ID） | 済: `REF:src/ui/guide.rs`（新設）＋ `REF:src/ui/i18n/{ja,en}.toml` の `guide.*` |
| 1-2 | 出る条件の判定（Web=保存 0 / デスクトップ=履歴 0）と、閉じたことの保存 | 済: `Picker::first_visit`（`REF:src/ui/web.rs`・`REF:src/ui/app.rs`）と `.novnow/ui.json` の `guide_done` |
| 1-3 | 吹き出しと矢印の描画（対象の矩形から位置決め、画面端で逃げる、入力を奪わない） | 済: `REF:src/ui/widgets/guide.rs`（新設） |
| 1-4 | 進む条件の監視（節が増えた・`'` が出た・プロットが空でない・読者モードを開いた・エクスポート完了・Zip 完了） | 済: `Session::guide_signals` と `Guide::observe`（`REF:src/ui/app.rs`） |
| 1-5 | ヘルプ（`?`）に「はじめての手引きをもう一度」を足す | 済: `REF:src/ui/app.rs` のヘルプの一覧の下 |
| 1-6 | テスト（条件判定・段階送り・飛ばし・キーボードのみ・ja/en の網羅） | 済: `REF:tests/guide.rs` と `REF:src/ui/guide.rs` の単体テスト。ja/en の網羅は `REF:tests/i18n_catalog.rs` |

<a id="plan-docs-site"></a>
#### Phase 2: サイトの自動ビルド（2 日）— **実施済み**

`REF:scripts/docs-site.py` が正本（`REF:docs/users/*.md`・`REF:docs/specifications/*.md`）を
`REF:website/src/content/docs/` へ写し、目次 `REF:website/src/data/docs-nav.ts` を書き出す。
サイトの URL は `/ja/manual/<slug>`（ガイド）と `/ja/specifications/<name>`（仕様書）で、
`README.md` はそれぞれの index になる。英語は本文が無いので、日本語版へ案内する短いページを出す。

| # | すること | 結果 |
|---|---|---|
| 2-1 | `REF:docs/users/*.md` から Astro ページを生成する | 済（`/ja/manual/start`・`cheatsheet`・`glossary`・`faq`・`project-structure`・`git`・`publish`・index の 8 ページ） |
| 2-2 | 仕様書（11 冊）から `/ja/specifications/<name>` を生成する | 済（11 冊＋index） |
| 2-3 | ガイドから仕様書の節へリンクを張る | 済（`../specifications/x.md#アンカー` を `/ja/specifications/x#...` に書き換え。飛べない日本語見出しには `<a id="install">` などの英字 ID を置く） |
| 2-4 | 生成物の一致を CI で検査（`--check`） | 済（`REF:scripts/verify.sh` の `site-check`） |
| 2-5 | トップと `Get Started` を `REF:docs/users/start.md` の段階と一致させる | 未（アプリ内のガイドモードと同じく Phase 1 の実装に合わせる） |
| 2-6 | トップページから GitHub の URL を外す | 済（`REF:website/src/data/site.ts` の `guideLinks` と、トップの仕様書リンクをサイト内へ） |

「同じ内容を 2 か所に持たない」ため、トップページには説明の本文を置かず、**ガイドへのカードと入口の表だけ**を置く。
本文はすべて `REF:docs/` が正本。

#### Phase 3: 文章の品質を機械で守る（1 日）— **実施済み**

`REF:docs/script/easy-read.py`（標準ライブラリのみ）を `REF:scripts/verify.sh` の `docs-users` と
CI の docs ジョブに入れた。規則は 1 文 60 字・1 段落 3 文・漢字 40% 以下（60 字以上の段落だけ）・
禁止語・です/ます調の混在。回帰テストは `REF:docs/script/test_easy_read.py`。


| # | すること | 基準 |
|---|---|---|
| 3-1 | `docs/script/easy-read.py` を新設（標準ライブラリのみ） | 1 文 60 字以内・1 段落 3 文以内・漢字比率 40% 以下・禁止語（`AST`・`enum`・`デバウンス`・`シリアライズ`）・です/ます混在なし |
| 3-2 | `REF:scripts/verify.sh` の `docs-users` ステップに組み込む | 落ちたときにどの文かが出る |
| 3-3 | 診断の上位 12 件に「利用者向けの対処」を足す | 済: `REF:docs/users/errors.md`（12 件）。`REF:docs/users/faq.md` と `REF:docs/users/README.md` から引ける |

#### Phase 4: 見つけてもらう（1 日）

| # | すること | 完了条件 |
|---|---|---|
| 4-1 | `REF:README.md` / `REF:README.en.md` の冒頭に「どこで動くか」（デスクトップ / ブラウザ / CLI / LSP / MCP / Python）を表で置く | 冒頭 30 行以内にブラウザ版の記載がある |
| 4-2 | サイトのトップに同じ表を置き、`Get Started` へのボタンを最上部に出す | トップから 1 クリックで手引きに入れる |
| 4-3 | インストールの節は「ブラウザ版は不要」「デスクトップは準備中」の 2 段だけにする | 準備中の表示が README・サイト・手引きで同じ |

#### Phase 5: 英語と図（1 日・任意）

- `REF:docs/users/` の英訳（`start.en.md` など）。サイトは `/en` に同じ生成を通す。
- 操作の GIF 3 本（作品を作る／節を書く／書き出す）。撮り方は `REF:docs/developers.md` §7.3 に追記する。

### 6.2 受け入れ基準

- [ ] はじめての人がブラウザだけで、手引きの 1〜9 を**詰まらずに**終えられる（5 人で試す）
- [x] ガイドモードが、Web は保存 0 件のときだけ・デスクトップは履歴 0 件のときだけ自動で出る
- [x] ガイド中も本文が書ける（入力を奪わない）／`Esc` と「あとで」で閉じられる／閉じたら次回は出ない
      （プロジェクトを 1 件も作らずに閉じた場合だけ、保存先が無いので次回も出る。仕様 §9.10 の例外）
- [ ] ガイドの 9 段階と、サイトと `REF:docs/users/start.md` の段階が一致する（検査 11）
- [ ] `REF:docs/users/` の `REF:` が `REF:docs/specifications/` だけを指す（検査 11）
- [ ] 3 つの解説ページ URL が公開物に実在する（検査 11）
- [ ] `docs/script/easy-read.py` が `REF:docs/users/` の全ファイルで通る
- [ ] `bash scripts/verify.sh` が全ステップ緑（`docs-users` を含む）

### 6.3 検査 11（`REF:docs/script/reference.py` に足す）

| # | 検査 | 何を捕まえるか |
|---|---|---|
| 11-1 | 解説ページ URL の実在 | 仕様 §14.2.1 と `REF:src/core/model/project.rs` の URL を集め、対応する公開物（`guide/*.html`）が無い |
| 11-2 | `REF:docs/users/` の参照方向 | ガイドから `docs/developers.md` など仕様書以外を参照している |
| 11-3 | ガイドと手引きの段階一致 | アプリの段階定義と `REF:docs/users/start.md` の見出しの数・順序が違う |
| 11-4 | 用語集の被覆 | 仕様書に出る主要語（節・場面・話・章・モード・役割語・参照ペイン・点検・委譲レベル）が用語集に無い |

### 6.4 リスク

| リスク | 対策 |
|---|---|
| ガイドの文言がサイトと二重管理になる | 段階の定義を 1 つの表から生成し、検査 11-3 で数と順序を突き合わせる |
| 吹き出しが対象の部品を見失う（画面幅・言語で位置が変わる） | 対象を**部品 ID** で指定し、矩形は描画時に取る。見つからない段階は飛ばす |
| ガイドが邪魔で閉じられない | `Esc`・「あとで」・画面外クリックの 3 つで閉じる。閉じたことを保存 |
| 用語を言い換えて仕様と食い違う | 用語集は「言い換えない」を MUST にし、`easy-read.py` の禁止語に「節の言い換え」を入れない |
| サイト生成が重くて CI が長くなる | 生成は `--check` のみ（ビルドはしない）。Astro のビルドはデプロイ時だけ |

---

<a id="distribution"></a>
## 7. 配布の実装計画（5 形態・18 通り。仕様は §20、設計は `REF:docs/designs/external.md`）

**状態**: 実装済み。5 形態の公開経路・リリースのワークフロー・VSCode 拡張・複合アクションをそろえた。ブラウザ版だけが公開済みで、その他は初回リリースまで未提供。実際の公開と 3 OS / 5 対象環境のリモート検証は、リリース時の Actions で行う。

**前提（2026-10-23 のユーザー指示）**

- 配布形態はブラウザ版・デスクトップ版・Python ライブラリ版・VSCode 拡張版・GitHub Actions 版の 5 種。選択肢の数は 1・10・5・1・1。
- デスクトップ版の 10 通りは「GUI あり / GUI なし」× 対象環境 5 通り。配布物を分ける軸はこれだけにする。
- VSCode 拡張版と GitHub Actions 版も実装済み。公開経路は `REF:docs/distribution.md` のとおり。
- インストーラは作らない（規則 10 との衝突を避け、書き込み先を増やさない）。実行時に配布元へ接続しない。
- **軸を増やさない（2026-10-23 の裁定）**: 「同時に 1 つだけ起動するか（シングルトン）」と「同梱物を実行ファイルに埋め込むか（スタンドアロン）」の 2 案は、どちらも採らない。novnow は履歴ファイル以外をプロジェクト外に置かないので、どちらの軸も利用者から見た違いがほぼ無い。同梱物は常に実行ファイルへ埋め込み、同時起動は制限しない。
- リリースのビルドはタグと `workflow_dispatch` のときだけ走らせる（Actions の使用量を抑える。`REF:docs/developers.md` §4.7）。

### 7.1 段階

<a id="plan-distribution-docs"></a>
#### Phase 0: 仕様・設計・手順・導入案内（済）

| # | すること | 置き場所 |
|---|---|---|
| D-01 | 仕様 §20（配布形態の一覧・デスクトップ 10 通り・Python 5 通り・VSCode 拡張・GitHub Actions・公開と廃止）を書く | 済: `REF:docs/specifications/external.md` |
| D-02 | 設計 §20（成果物の名前の生成・同梱物の埋め込み・ホイールの 5 構成・拡張の取得・複合アクションの 4 ステップ）を書く | 済: `REF:docs/designs/external.md`（6 節すべて「実装完了」） |
| D-03 | 配布方法（取得先・名前・選び方・公開手順・切り戻し）を書く | 済: `REF:docs/distribution.md` |
| D-04 | 非エンジニア向けの導入案内を書く | 済: `REF:docs/users/install.md`。`REF:docs/users/README.md`・`REF:docs/users/start.md`・`REF:docs/users/faq.md` から引ける |

<a id="plan-distribution-release"></a>
#### Phase 1: リリースの骨組み（1〜2 日）

| # | すること | 完了条件 |
|---|---|---|
| D-05 | `scripts/release-matrix.py`（成果物の名前・10 + 5 の行列・版の書き換え・`--status`）を作る | 行列が 10 行・5 行になり、名前の重複が無い（`tests/distribution.rs`） |
| D-06 | `.github/workflows/release.yml`（`workflow_dispatch` とタグだけで起動）を作り、`SHA256SUMS` を添付する | 同じタグから 15 個の成果物と照合値が出る。PR では起動しない |
| D-07 | 署名（Windows の `signtool`・macOS の `codesign` / `notarytool`）を任意ステップとして入れる | 鍵が無い環境では飛ばし、`SHA256SUMS` にその旨が残る |

<a id="plan-distribution-desktop"></a>
#### Phase 2: デスクトップ版 10 通り（2〜3 日）

| # | すること | 完了条件 |
|---|---|---|
| D-08 | GUI あり / なしのビルドを 5 対象環境で作る（新しい feature は足さない。`ui` の有無だけ） | 10 通りの成果物が出る。GUI あり / なしで保存形式と診断結果が同じ |
| D-09 | Windows のサブシステムを GUI 版だけ切り替える | GUI なし版で `--gui` が終了コード 2。GUI あり版は CLI としても動く |
| D-10 | macOS の `.app` を含むアーカイブを組み立て、同梱物が別ファイルとして入っていないことを確かめる | `open -a novnow --args <dir>` でプロジェクトが開く。`ipag.ttf` などのファイルがアーカイブに無い |

<a id="plan-distribution-python"></a>
#### Phase 3: Python ライブラリ版 5 通り（1 日）

| # | すること | 完了条件 |
|---|---|---|
| D-11 | ホイール行列を 4 構成から 5 構成へ増やす（macOS x86_64 を追加） | 5 つのホイールが揃い、`cp39-abi3` で 3.9 以降を賄う |
| D-12 | タグで TestPyPI、承認つきで PyPI へ公開する | `pip install novnow==<版>` が 5 環境で通る。`novnow.__version__` が `REF:Cargo.toml` と一致する |

<a id="plan-distribution-vscode"></a>
#### Phase 4: VSCode 拡張版（3 日）

| # | すること | 完了条件 |
|---|---|---|
| D-13 | `vscode/` を作る（`package.json`・LSP 接続・4 つのコマンド・言語 ID） | プロジェクトを開くと診断が出る。拡張がプロジェクトのファイルを直接書かない |
| D-14 | 実行ファイルの探索 4 段と、GUI なしの自動取得（照合つき）を実装する | 未導入の環境で自動取得して接続する。照合の不一致で破棄する。`novnow.autoDownload=false` で取得しない |
| D-15 | `.vsix` をリリースへ添付し、市場への公開はタグ実行または手動の公開入力と environment 承認で行う | 市場と `.vsix` の両方から導入できる |

<a id="plan-distribution-action"></a>
#### Phase 5: GitHub Actions 版（2 日）

| # | すること | 完了条件 |
|---|---|---|
| D-16 | `action.yml`（複合アクション）と `scripts/action/install.sh` を作る | 3 OS で `uses: sonneko/novnow@v0` が点検を実行し、E 診断でジョブが失敗する |
| D-17 | 入力（`path` / `command` / `args` / `version` / `target`）と出力（`exit-code` / `findings` / `export-path`）を実装する | 省略時は点検になり、終了コードがジョブへ伝わる |
| D-18 | リリース時に `v<版>` と `v0` のタグを公開する | 版を固定しても `@v0` でも動く |

<a id="plan-distribution-publish"></a>
#### Phase 6: 公開と掲載（1 日）

| # | すること | 完了条件 |
|---|---|---|
| D-19 | `REF:scripts/release-check.sh` を作る | デスクトップ 10・ホイール 5・`.vsix` 1・全ソース 1 の 17 成果物・照合値の行数・版の一致・タグの指し先を検査する |
| D-20 | 「準備中」を実値へ差し替える（`REF:README.md`・`REF:README.en.md`・`REF:docs/users/install.md`・サイト） | 5 形態の取得先が文書とサイトで同じ。未提供の形態は「未提供」と表示する |

### 7.2 受け入れ基準

- [ ] 同じタグから、デスクトップ 10・ホイール 5 の成果物と `SHA256SUMS` が出る（`release.yml`）
- [ ] 10 通りのうち、GUI あり / なしで保存形式と診断結果が一致する（`tests/distribution.rs`）
- [x] 配布物がプロジェクト外へ新しいファイルを作らない（仕様 §20.2.3。外に書くのは履歴ファイル 1 個だけ）
- [ ] 同じ機械で何個でも起動でき、2 つ目を止める仕組みが無い（ロックファイルも作らない）
- [ ] ホイール 5 つが `cp39-abi3` で、3.9 以降の全版に 1 つずつ入る
- [ ] VSCode 拡張が、未導入の環境で照合済みの実行ファイルを取得して LSP へ接続する
- [ ] 複合アクションが 3 OS で動き、E 診断でジョブが失敗する
- [ ] 実行時に配布元へ接続しない（本体に HTTP クライアントの依存が無い）
- [x] 仕様・設計・手順・利用者向けの導入が `REF:docs/specifications/external.md` を正本として一致する

### 7.3 リスク

| リスク | 対策 |
|---|---|
| リリースの 10 ジョブで Actions の使用量が増える | タグと `workflow_dispatch` だけで起動し、PR では 2 通り（`ui` あり/なし）だけをビルドする |
| macOS x86_64 のランナー待ちが長い | `macos-latest`（arm64）から `--target x86_64-apple-darwin` でクロスビルドする（`macos-13` は使わない） |
| 署名鍵がサンドボックスに無い | 署名は任意ステップにし、鍵が無いときは飛ばして `SHA256SUMS` に記録する |
| 配布物を分ける軸を後から足したくなる | 仕様 §20.2.3 に「軸を置かない」と MUST で書き、`REF:docs/script/reference.py` の検査 12 が数（1・10・5・1・1）と廃止した軸の再来を見張る。`tests/distribution.rs` も 10 通り以外の名前を作れないことを見る |
| VSCode 拡張がバイナリを抱え込んで古くなる | 版は拡張の版に固定し、取得のたびに照合する。自動取得を切れる設定を必ず出す |
| 市場へ出した版を取り消せない | タグでは公開せず `workflow_dispatch` の承認で出し、不具合は新版で直す |
| 利用者向けの手引きが「準備中」のまま残る | Phase 6 の D-20 で README・サイト・手引きを同じ版で差し替える |

---

<a id="issue-29"></a>
## 8. Issue #29（V1 延期・V0 継続）の仕様査定と判断カタログ（状態: 判断・仕様書/設計書への反映・実装計画まで確定。実装は §8.5 のバッチ PR 群で行う）

本節は Issue #29 の全項目について、現行の仕様書・設計書・実装を突き合わせた**査定結果と、
仕様を確定する前に決めるべき判断の選択肢**を記録する。作業の順序は所有者指示どおり
「仕様・設計書更新 → 実装計画立案 → 実装」とし、develop へのマージは所有者が行う。
本節の「推奨」はエージェントの提案であり、仕様書・設計書への反映は所有者の承認後とする
（`REF:docs/developers.md` §5-3）。

### 8.0 前提と読み方

- **版の方針**: Issue #29 冒頭の「V1 延期。まだ、開発は V0 で続ける。」を前提とする。
  本書冒頭の「現在のマイルストーンは v0.1.0 の初回配布」は公開済みのため陳腐化しており、
  仕様更新の際に次版（v0 次版）の位置づけへ書き換える必要がある。
- **完了済みの 2 件**: 「v0.1.0 リリースのための作業」と「v0.x 間ではプロジェクト形式の
  互換性を保証しない。移行機能は作らない」の明言はチェック済みであり、本節では扱わない。
  ただし後者は §8.1 の判断 M-2（互換性ウィンドウを今回使うか）の前提として効いてくる。
- **空のチェックボックスが 1 件ある**（「大きなテキストエリアに左太青線」の次行）。
  欠落した要件なのか誤記なのか、所有者確認が必要（判断 M-5）。
- 項目番号 `N29-xx` は Issue 本文の出現順（チェックボックス単位、完了済み 2 件を含む通し番号）。
- 分類は次の 5 種: **バグ（仕様内修正）** / **仕様変更** / **新機能** / **検討（決めない選択も含む）** /
  **運用・別リポジトリ**。仕様変更と新機能は、該当する仕様書・設計書の節を §8.4 に引く。

### 8.1 全体に関わる判断（先に決めると残りが決まるもの）

- **M-1 スコープと PR の切り方**
  - A: UI バグ・小規模改善（§8.2 の G1〜G7）に絞り、仕様更新→実装までを 1 PR で完走する。
    形式変更などの大テーマ（G8・G11）は「決めないこと」を本節に残して先送りする。
  - B: 全項目を今回決め切る。仕様書・設計書の更新を厚くし、実装は到達可能な範囲まで進める
    （未実装分は設計書の「未実装・実装計画済み」として残る）。
  - C: 今回は仕様・設計の決定と実装計画だけを PR にし、実装は次セッション以降に回す。
  - 推奨: **A**。理由: 大テーマ（JSON/TOML・拡張子・feature 管理・hub）は相互に依存し、
    UI 修正の検証（`REF:scripts/verify.sh` と `REF:scripts/web-smoke.sh`）と混ぜると
    レビュー単位が崩れる。ただし G8・G11 についても本節の選択肢から方針だけは決めておく
    （実装しない判断も含む）ことを勧める。
  - **決定（所有者）: C**。本 PR では仕様書・設計書の更新と実装計画の確定までを行い、実装は次セッション以降の別 PR に回す。判断の対象は #29 の全項目とし、「実装しない」「本体に入れない」という決定も含める。
  - **追加決定（所有者、第 10 回）**: §8.2 に残る推奨案も、仕様書・設計書を編集する前に個別の判断ラウンドで確定する。推奨案をそのまま一括採用はしない。
- **M-2 v0 の互換性ウィンドウを今回使うか**（N29-30「JSON→TOML」・N29-33「.txt でいいのか」に直結）
  - A: v0.1.0 のプロジェクト形式との互換を維持する。非互換変更は項目ごとに個別承認とする。
  - B: 「v0.x 間では互換性を保証しない・移行機能は作らない」という既決方針どおり、
    必要なら非互換変更を行い、旧プロジェクトは開けなくなることを許容する。
  - C: 非互換変更は行うが、旧形式の**読み取り互換**（読んで新形式で保存する）だけは付ける。
    「移行機能は作らない」という既決方針の部分的な撤回になる。
  - D: 今回はファイル形式に一切触らない（N29-30・N29-33 は検討メモだけ残す）。
  - 推奨: 当面 **A** を既定とし、形式変更の是非そのものは §8.2 G8 の個別判断で決める。
    B は既決方針と整合するが、v0.1.0 公開直後に既存利用者のプロジェクトを開けなくする
    コストが、得られる見やすさに対して大きい。
  - **決定（所有者）: B**（推奨 A は不採用）。v0 のうちに形式の形を整えることを優先し、必要なら非互換変更を行う。旧プロジェクトがそのままでは開けなくなることを許容し、移行・読み取り変換機能は作らない。これにより N29-30・N29-33 は「現行維持」を既定とせず、形式変更の是非と新しい形を正面から決める。非互換変更を採る場合は、次版の位置づけ、形式版の扱い、旧形式を開いたときのエラー表示も併せて仕様化する。
- **M-3 モーダル外クリックの統一規則**（詳細は N29-04）
  - A: 全モーダル一律に、外側クリックで閉じる（入力中の内容は破棄する）。
  - B: 階層化する。通常モーダルは閉じる。破壊的操作の確認では「キャンセル」と同じ扱いで閉じる。
    保存失敗・終了ガードなど、閉じると警告の意味が消えるものだけ閉じない。
  - C: 一律に閉じるが、未保存の入力があるモーダルだけ破棄確認を挟む。
  - 推奨: **B**。Issue の要求（全部閉じる）の趣旨は「閉じられなくて詰まる」ことの解消であり、
    保存失敗（`SaveError`）や終了ガード（`CloseGuard`）を誤クリックで消せるようにすると
    §8.4「閉じないでください」表示の意味が無くなるため。
  - **決定（所有者）: B**（階層化）。外側クリックで閉じない重ね物を仕様に列挙し、破壊的操作の確認ではキャンセルと同義にする。入力破棄の扱いは N29-04 の第 6 回決定に従う。
- **M-4 `Ctrl+Enter` の意味変更の進め方**（詳細は N29-06）
  - A: 置き換える。常に「カーソル位置で節を分割する」動作にする（分割位置の詳細は N29-06）。
  - B: 併存させる。現行の「確定して次の節へ」は残し、分割は別キー（候補: `Ctrl+Shift+Enter`）にする。
  - C: 自動で切り替える。カーソルが節の末尾にあるときは現行動作、中間にあるときは分割する。
  - 推奨: **A**（分割位置はカーソルのバイト位置。末尾では現行と等価になるため後方互換の
    体感が保てる）。ただし他ワークスペースの `Ctrl+Enter`（下位へ降りる・採用・昇格）は
    従来どおりとし、変更は本文執筆（`write`）の節エディタに限る。
  - **決定（所有者）: A**（置き換え）。変更範囲は `write` の節エディタに限る。分割位置は第 6 回決定のとおりカーソルのバイト位置どおりとし、節先頭・末尾での扱い、次節が既にある場合、ヘッドレス操作は N29-06 の第 9 回決定に従う。
  - **追加決定（所有者、第 3 回）**: 何も書かれていない節では、`Ctrl/Cmd+Enter` で新たな空節を作らない。誤操作を繰り返しても空節が連鎖的に増えないことを受け入れ条件にする。
- **M-5 空のチェックボックス 1 件の扱い**
  - A: 誤記として無視する。 B: 所有者が要件を補うまで保留として本節に残す。
  - 推奨: **B**（本節末尾の未決リストに残す。仕様更新のブロッカーにはしない）。
  - **決定（所有者）: A**（誤記として閉じる。推奨 B は不採用）。追加要件としては扱わず、未決リストから外す。
- **M-6 実機でしか検証できない項目の扱い**（N29-07 iOS、N29-38 実機網羅、N29-03 の一部）
  - A: サンドボックスで可能な検証（ヘッドレステスト・Web スモーク）までを完了条件とし、
    実機確認は所有者のチェックリストとして分離する。
  - B: 実機確認が終わるまで該当項目を完了にしない。
  - 推奨: **A**。サンドボックスには iOS Safari 実機が無く、B では PR が永久に閉じない。
    代わりに `REF:docs/known-issues/ja.md` の運用（修正版公開まで掲載継続）と組み合わせる。
  - **決定（所有者）: A**。次段の実装 PR は自動検証で完了できる形にし、実機でしか分からない事項は所有者向けチェックリストと known-issues の運用で追う。

### 8.2 項目別の査定

#### G1 モーダルとオーバーレイ

<a id="issue-29-modal"></a>
- **N29-04 全モーダルを外クリックで閉じる＋網羅テスト**［仕様変更］
  - 現状: 共通部品 `popup()`（`REF:src/ui/app.rs`）は全画面シールドで背面クリックを吸収し、
    「外側を押されたか」を戻り値で返すが、**使っているのはパレット 1 件だけ**で、
    残りの呼び出し側は戻り値を捨てている。つまり土台は在り、接続が無い。
    パレット以外で `popup()` を使う重ね物は、ジャンプ・ヘルプ・ウェルカム・移行案内・
    MCP 設定・行幅設定・アイデア即時入力・昇格先選択・全体検索・資料検索・各種確認・
    タスク作成・タスク実行確認・コメント入力・プロンプト復元/差分・外部変更の差分・
    節削除確認・エクスポート・メッセージ・終了ガード・改名・使用箇所一覧・資料名入力・
    保存エラーである。これとは別に、シールドを持たない軽い重ね物（検索/置換バー・補完・
    情報ポップアップ・クイックフィックス）がある。`Esc` は現行でも全重ね物を閉じる。
  - 判断 1（統一規則）: M-3 のとおり。
  - 判断 2（入力破棄）: 入力欄を持つモーダル（MCP 設定・タスク作成・改名・エクスポート範囲など）を
    外クリックで閉じたとき、(a) 無言で破棄する (b) 次に開いたとき入力を復元する（セッション内保持）
    (c) 破棄確認を出す。推奨: **(a)**（`Esc` の現行挙動と一致させ、規則を 1 本にする）。
    ただし MCP 設定だけは既存の検証エラー表示の都合で (b) の余地がある。
  - **決定（所有者、第 6 回）**: 外側クリックまたは `Esc` で閉じた場合、短い入力は無言で破棄してよいが、MCP 設定・タスク作成など項目の多い複雑なフォームは、アプリを閉じるまでのセッション内に限り下書きを保持し、次に開いたときに復元する。どのモーダルを「複雑なフォーム」とするかは、仕様更新時に重ね物ごとに列挙する。下書きはファイルへ保存しない。
  - **決定（所有者、第 12 回）**: セッション内下書きを保持する複雑なフォームは、MCP 設定、タスク作成、エクスポート設定、却下／差し戻しコメント、アイデア即時入力とする。却下／差し戻しコメントは、同じ提案・同じ向きで再び開いた場合だけ復元する。改名、新規資料タイトル、行幅設定、ジャンプ先入力などの短い 1 項目入力は破棄する。
  - 判断 3（軽い重ね物の範囲）: シールドの無い 4 種（検索バー・補完・情報・クイックフィックス）は
    「外クリック＝背面の操作として通しつつ閉じる」が自然であり、モーダルの規則とは分けて
    仕様化する必要がある。推奨: モーダル（`popup()` 系）とポップアップ系で規則を分けて明記する。
  - **決定（所有者、第 11 回）**: 軽い重ね物の規則はモーダルとは分けて明記する。外側クリックでは軽い重ね物を閉じるだけとし、そのクリックを背面のボタンやエディタへの操作としては通さない。
  - 判断 4（網羅テストの作り方）:
    (a) GUI ハーネス（`REF:tests/harness/mod.rs` の `click_at`）で全重ね物を 1 つずつ開き、
        外側座標を押して「閉じたこと・背面が押されていないこと・本文が変わらないこと」を検証する。
    (b) (a) に加え、重ね物の列挙をコンパイル時に漏れなくする仕掛け（全バリアントを列挙する
        テスト用関数に `match` の網羅性を持たせる等）を入れ、将来増えた重ね物のテスト漏れを防ぐ。
    推奨: **(b)**。「網羅」を人の注意ではなく型で守るのがこのリポジトリの流儀
    （`REF:tests/layering.rs` の AST 検査など）と合う。
  - **決定（所有者、第 11 回）**: GUI ハーネスで全重ね物を 1 つずつ開いて外側を押し、閉じるべきものは閉じること、閉じてはいけないものは閉じないこと、背面が操作されないこと、本文が変わらないことを検証する。さらに、全バリアントを列挙するテスト用関数に `match` の網羅性を持たせるなどし、将来追加した重ね物のテスト漏れをコンパイル時に検出できるようにする。
  - 反映先: `REF:docs/specifications/ui.md` §9.7・§9.8（`Esc` の行の拡張として新節）、
    対応する設計書。あわせて §9.2.1（破壊的操作の確認にラベルを置いてよい）との整合を取る。
- **N29-19 タスク作成モーダルの外クリック**［仕様変更（N29-04 の具体例）］
  - 現状: タスク作成（`AgentNew`）も `popup()` の戻り値を捨てており、外クリックでは閉じない。
  - 判断: N29-04 に統合する（個別対応はしない）。入力項目が多い（役割・対象・目的・コメント・例）
    ため、判断 2 で (b) を選ぶ場合の第一候補になる。推奨: 統合し、破棄規則は全体で統一する。
- **N29-16 ヘルプモーダルを中央に・外クリックで閉じる・Esc で消えることを書く**［仕様変更］
  - 現状: `popup()` はパレット以外をすべて「中央・上から 72px」に置く。ヘルプ本文は
    キー一覧のラベル列挙で、Esc でも閉じられるがモーダル内にその案内が無い。
  - 判断 1（配置）: (a) ヘルプだけ画面中央 (b) 全モーダルを画面中央に統一（パレット系は上部維持）
    (c) 現行の上部配置を維持。推奨: **(b)**。読むためのモーダル（ヘルプ・差分・情報系）と
    打つためのモーダル（パレット・検索系）で置き場所の意味が違うため、2 系統に分けるのが一貫する。
    ただし (b) は全モーダルの見た目が変わるので、スクリーンショットでの確認を実装計画に入れる。
  - **決定（所有者、第 11 回）**: ヘルプを含む `popup()` 系モーダルは画面中央へ統一し、コマンドパレット等のパレット系は現行の上部配置を維持する。配置変更後の見た目確認を実装計画に入れる。
  - 判断 2（Esc の明記）: ヘルプにだけ書くか、全モーダルの共通フッタとして
    「Esc / 外側クリックで閉じる」を出すか。推奨: 共通化する（`modal.close` 系の文言を
    `popup()` 側で付けられるようにする）。ラベル撤廃規則 §9.2.1 の例外 3（ヘルプの文言）に当たる。
  - **決定（所有者、第 11 回）**: 「Esc / 外側クリックで閉じる」案内は、外側クリックで閉じられる全モーダルの共通フッタに出す。保存エラーや終了ガードなど外側クリックで閉じてはいけないものには出さない。
  - 判断 3: ヘルプ内のキー一覧をクリックで実行できるようにするかは N29-24 で扱う。

#### G2 節の編集モデル

- **N29-05 節を消す手段がない**［バグ＋仕様変更の可能性］
  - 現状: 削除手段は実装済みである。仕様は `REF:docs/specifications/ui.md` §9.8.3 の
    `Ctrl+Shift+;`（確認モーダル付き）、実装は `REF:src/ui/shell.rs` のキー解釈、
    `REF:src/app/workbench.rs` の `delete_section`、`REF:src/core/features/sections.rs` の `remove`。
    パレットの `>delete-section` とヘッドレス操作 `section.delete` からも届く。
    空の節については診断 W012 のクイックフィックス「削除」もある。それでも「手段がない」と
    感じられた原因の仮説は次のとおり（実装計画の冒頭で切り分ける）:
    (1) **JIS 配列では到達不能の可能性**。egui のキーは論理キーであり、JIS 配列では
        `Shift+;` が `+` になり、`:` は Shift 無しの別キーになる。現行の判定は
        「Shift 付きの `;` または `:`」なので、どちらの物理キーでも成立しない組み合わせがありうる。
    (2) ボタンが無く、ヘルプとパレットでしか発見できない。
    (3) 消せるのは「今編集中の節」だけで、文脈ビューや話プロットの一覧からは消せない。
    (4) 確認文言が「Ctrl+Z では戻せません」であり、§8.6（undo は節をまたがない）と合わせて
        心理的に使いにくい。
  - **所有者確認（第 3 回）**: 主因は「ボタンが欲しい」という発見可能性であり、現行 `Cmd+Enter` の誤操作で空節を大量に作りがちな点が削除需要を高めている。JIS 配列でキーが到達不能という仮説は、今回の症状説明としては採用しない（一般的なキー配列互換の確認事項としては残す）。
  - 判断 1（キーの是正）: (a) JIS でも届くよう判定を広げる（Shift 無しの `Ctrl+:` も受ける等。要実機確認）
    (b) 別キーへ移す（候補: `Ctrl+Shift+Delete`。ブラウザ予約との衝突確認が要る）
    (c) 現キーは残して代替キーを足す。推奨: **(c)**。既存利用者の指を壊さず、§9.8.6 の
    「奪われやすいキーには代替を用意する」という既存方針にも合う。
  - 判断 2（マウス経路）: (a) 節エディタのミニヘッダに削除ボタンを置く
    (b) 文脈ビューの節にも削除を置く (c) 置かない（パレットで足りる）。
    推奨: **(a)**。§9.4.2 は「これ以外を置かない」と定めているため仕様変更が要るが、
    破壊的操作の入口が編集中の節のすぐ上に在るのは発見可能性として妥当。
  - 判断 3（対象範囲）: 話プロット側の節ブロック削除をどうするか。本文の節を消すと
    後続の節番号（D）が繰り上がり、プロットの `mode:` 割当・提案・タスクの対象節がずれる
    （節番号は §5.2.2 のとおり位置から決まる）。現行実装はこの繰り上がりをそのまま受け入れている。
    (a) 現行どおり本文だけを消す (b) プロット側の対応ブロックも一緒に消すか確認する
    (c) 繰り上がりの影響を診断（W030 節数不一致）で知らせるに留める。
    推奨: **(a)＋(c)**（現行維持を確認したうえで、仕様に「番号が繰り上がること」を明記する）。
    恒久 ID 化は形式そのものの変更になるため本 Issue の範囲外とする。
  - 判断 4（取り消し可能性）: 削除を undo 可能にするかは §8.6 の改訂になる。
    (a) 現行どおり戻せない（確認で担保） (b) 削除だけは節をまたぐ undo を許す。
    推奨: **(a)**。undo モデルの例外は混乱の元であり、確認モーダル＋自動保存前の
    バッファ保持で足りる。
  - **決定（所有者、第 3 回）**: マウスで辿れる削除ボタンを設ける。配置と対象範囲は残り判断とし、少なくとも編集中の節を画面上から削除できることを受け入れ条件にする。
  - **決定（所有者、第 4 回）**: 節エディタ／ミニヘッダには削除ボタンを置かず、文脈ビューとプロット一覧から対象節を指定して消せるボタンを設ける。空の節は確認なしで削除し、空でない節は既定では確認するが、その確認の有無を設定で変更できるようにする。設定の保存先、プロット側の操作が本文の節自体・対応するプロットブロック・その両方のどれを消すのか、既存のショートカット／パレットをどう残すかは残り判断とする。
  - **決定（所有者、第 6 回）**: 文脈ビュー／プロット一覧の削除ボタンは、本文の対象節と対応するプロットブロックを一緒に削除する。推奨した本文のみの削除は採らない。対応するプロットブロックが一意に定まらない場合の扱い、確認設定の保存先、既存のショートカット／パレット／ヘッドレス操作にも本文とプロットを一緒に削除する意味を持たせるかは残り判断とする。
  - **決定（所有者、第 8 回）**: ボタン、既存ショートカット、パレット、ヘッドレス操作 `section.delete` はすべて、本文の対象節と対応するプロットブロックを一緒に削除する意味へ統一する。空でない節を確認するかどうかは個人用 `.novnow/ui.json` に保存する。対応するプロットブロックが一意に定まらない場合は何も削除せず、その旨を知らせる。
  - **決定（所有者、第 10 回）**: 現行の `Ctrl+Shift+;` は残し、`Ctrl+Shift+Delete` と、コロンが Shift 無しで出る配列向けの `Ctrl+:` も節削除として受ける。
- **N29-06 `Cmd+Enter` でカーソル位置で節を分割する**［仕様変更］
  - 現状: `Ctrl+Enter`（macOS では Cmd も `ctrl` として受ける、`REF:src/ui/shell.rs`）は
    「節全体を確定して次の節へ。無ければ作る」であり、カーソル位置は使わない
    （`REF:src/app/workbench.rs` の `confirm_next`、仕様 §9.4.1・§9.8.3）。
  - 判断 1（キー割当）: M-4 のとおり。
  - 判断 2（分割位置の単位）: Issue の文言は「カーソルより上／下」である。
    (a) カーソルのバイト位置で割る（行の途中でも割れる）
    (b) カーソル行を境に、上の行群と下の行群で割る（カーソル行は下に含める／上に含めるの別も決める）。
    推奨: **(a)**。ただし話者注釈（行末の `@名前`）や `'` 参照・ルビの途中で割ると
    壊れた行が生まれるため、割った直後に行として成立する位置へ寄せる規則（行境界へのスナップ）を
    併記する必要がある。実装は既存の貼り付け分割（`split_pasted_sections`）と同じ
    「複数節への一括反映」の経路を再利用できる見込み。
  - **決定（所有者、第 6 回）**: 分割位置はカーソルのバイト位置（文字境界）どおりとし、話者注釈・参照・ルビ等の途中でも安全な行境界へ寄せない。推奨したスナップは採らない。分割によって壊れた記法が生じた場合は、既存の診断・表示規則に従う。
  - 判断 3（次節がある場合）: (a) 現在節と次節の間に新しい節を挿入して下半分を入れる
    (b) 既存の次節の先頭に下半分を足す。推奨: **(a)**（Issue の「次の節としてエディタに残す」に沿う）。
  - 判断 4（端の扱い）: カーソルが先頭なら上は空節になる（W012）。(a) そのまま許可する
    (b) 先頭・末尾では分割せず現行動作にする。推奨: **(b)** を含む形で A 案にする
    （末尾＝現行と等価、先頭＝何もしない、として規則を単純に保つ）。
  - **決定（所有者、第 3 回）**: 何も書かれていない節で `Ctrl/Cmd+Enter` を押しても、新たな空節は作らない。内容がある節の末尾で次節が生成された直後に同じ操作を繰り返しても、空節は連鎖的に増えない。空白のみ・コメントのみの節を「何も書かれていない」とみなすかは仕様化時に定義する。
  - 判断 5（ヘッドレス互換）: 正規操作 `section.confirm_next` にはカーソル概念が無い。
    (a) 操作は現行の意味のままにし、分割は GUI 専用の振る舞いとする（§19 のパリティ例外に明記）
    (b) 新操作 `section.split`（位置引数付き）を足す。推奨: **(b)**。§19.1 の「単一の真実」に従い、
    GUI の分割も新操作経由に寄せる。
  - **決定（所有者、第 9 回）**: カーソルが節先頭なら何もしない。節の途中で割る場合は、現在節と既存の次節の間に下側の新しい節を挿入する。節末尾では既存の次節へ移り、無ければ作る（現行の確定して次へ進む動作と等価）。位置引数付きの新操作 `section.split` を追加し、GUI と同じ意味にする。
  - **決定（所有者、第 12 回）**: 「何も書かれていない節」は現行 W012 に揃え、内容行が無い節、または空白行だけの節とする。行コメントは内容として数えるため、コメントのみの節では通常どおり分割・次節作成ができる。
  - 反映先: `REF:docs/specifications/ui.md` §9.4.1・§9.8.3、`REF:docs/specifications/editing.md` §8.1、
    `REF:docs/specifications/interfaces.md` §19.7（(b) の場合）。
- **N29-08 節エディタ上のバーのコントラスト不足**［バグ（仕様との不一致を含む）］
  - 現状: 仕様 §9.4.2 は「幅 64px の横棒＋右に 10px で %」と定めるが、実装は
    egui の `ProgressBar` を全面幅で描き、% 文字をバーの上に重ねている
    （`REF:src/ui/widgets/section_editor.rs` の `show`）。文字色はバーの塗り（`fg.muted`）と
    地の両方に跨るため、比率によって読めなくなる。これは仕様 §9.9 の
    最小コントラスト（補助テキスト 3:1）にも触れる可能性がある。
  - 判断: (a) 仕様どおりの形（短いバー＋横に %）へ実装を戻す
    (b) 全面幅は維持し、% をバーの外（右端の地の上）へ出す
    (c) バーを自前描画にし、塗りと文字の色の組をトークンで固定する。
    推奨: **(a)**。優先順位「仕様書 > 設計書 > コード」に従えば実装側の逸脱であり、
    §9.4.2 の「色は常に単色・警告色にしない」という思想もそのまま守れる。
    話プロット側の同種バー（§10.2、幅 48px）も同じ部品に寄せる。
  - **決定（所有者、第 12 回）**: 仕様 §9.4.2 どおり、幅 64px の横棒の右に 10px で % を出す形へ実装を戻す。話プロット側の幅 48px バーも同じ部品に寄せる。
  - 検証: dark/light 両テーマで比率 0%・50%・100% のスクリーンショット確認を実装計画に入れる。
- **N29-18 0 文字の節でのみカーソルと行番号が上にずれる／N29-23 文字だけが少し上にずれる**［バグ］
  - 現状: 行番号は Galley の各行の矩形中心に 11px 等幅で描き、自前キャレットも
    カーソル矩形の中心を基準に描いている（`REF:src/ui/widgets/section_editor.rs` の
    `paint_line_numbers` と `area`）。本文は 15px・行間 1.8 であり、「行の中心」と
    「文字の視覚中心（アセント基準）」はずれる。空の節では Galley の行高の決まり方が
    通常行と異なる可能性があり、症状が「0 文字のときだけ」出ることと整合する。
    F-03 で一度直した箇所の残存不具合と見る。
  - 判断 1（整列の基準）: (a) ベースライン基準に統一する（行番号・キャレットとも
    本文グリフのベースラインから決める） (b) 現行の中心基準のまま、空行の行高だけを是正する。
    推奨: **(a)**。中心基準はフォントを変えたときに再びずれる。
  - 判断 2（検証方法）: (a) ヘッドレス描画（`NOVNOW_SHOT`）の目視のみ
    (b) Galley の幾何を数値で検証する単体テスト（行番号の描画 Y とグリフのベースラインの差が
    許容範囲内か）を足す。推奨: **(b)**。ピクセル目視は回帰検知にならないため。
  - **決定（所有者、第 12 回）**: 行番号とキャレットは本文グリフのベースラインを基準に整列させる。検証には、0 文字行と通常行の双方で、行番号の描画 Y とグリフのベースラインの差が許容範囲内かを確認する幾何の単体テストを加える。
  - なお本 2 件は同根の可能性が高い。実装計画では 1 項目にまとめ、切り分け結果を履歴に残す。
- **N29-20 大きなテキストエリアに一貫して左太青線を付ける**［仕様変更］
  - 現状: 左端 3px の `accent` 縦線（`focus_stripe`、`REF:src/ui/app.rs`）は
    本文執筆画面の文脈ビューと節エディタにだけ、フォーカス中だけ出る（F-41、仕様 §9.4.5）。
    他の画面の大きなエディタ（`REF:src/ui/widgets/slice_editor.rs` 経由の
    話プロット・構成・配役・アイデア・資料の各エディタ）には線が一切無く、
    「どこがエディタか分かりにくい」という Issue の指摘はこの非対称を指していると読める。
  - 判断 1（線の意味）: (a) 常時表示の「編集可能である」印にする（フォーカス表示は別に要る）
    (b) フォーカス表示のまま、対象を全エディタへ広げる (c) 非フォーカス時は薄い線、
    フォーカス時は濃い線の 2 段にする。推奨: **(c)**。常時濃い青が並ぶと §9.2 の
    「色と配置で示す」原則の中でフォーカスの情報が消えるため。ただし (c) は
    「薄い線」のトークン追加（§11.1）が要る。
  - 判断 2（対象の範囲）: 「大きなテキストエリア」の線引きが要る。候補は、節エディタと
    Slice エディタ群、資料の要約・条件の複数行欄、MCP 設定モーダルのコード欄、
    タスク作成の複数行欄、役割語テスターの入力欄。1 行入力（検索欄など）は除くのが自然。
    推奨: 複数行エディタ全般を対象とし、一覧を仕様に列挙する（列挙しないと将来の追加で漏れる）。
  - 判断 3（衝突する既存の線）: 提案モードの警告色 3px 線（節エディタ）、文脈ビューの
    プレースホルダの 3px 線、話プロットの節リストの状態色 3px 線（§10.2）と意味が競合する。
    推奨: 線の意味を「青＝編集面／フォーカス、橙・緑・灰＝状態」と色で分ける規則を §11 に明記する。
  - **決定（所有者、第 7 回）**: 複数行エディタ全般に薄い左線を常時出し、フォーカス中は濃い青にする二段表示を採る。対象となる複数行エディタは仕様に列挙し、1 行入力は対象外とする。
  - **決定（所有者、第 14 回）**: 対象は、節エディタ、Slice エディタ群（話プロット・構成・配役・アイデア・資料本文）、資料フォームの要約・条件、MCP 設定のコード欄、タスク作成の例欄とする。役割語テスターを含む 1 行入力は対象外とする。青は編集面／フォーカス、橙・緑・灰は状態を表すものとし、同じ左端で重なる場合は警告・状態表示を優先する。

#### G3 パレットとナビゲーション

- **N29-09 矢印キーで候補がスクロールしない／N29-13 スクロールバーの位置がおかしい**［バグ］
  - 現状: パレットの候補一覧は `ScrollArea::vertical().max_height(360.0)` で描くだけで、
    選択行を可視範囲へ寄せる処理が無い（`REF:src/ui/app.rs` のパレット描画）。
    全体検索・資料検索・使用箇所一覧の重ね物も同じ作りである。スクロールバーの位置については、
    `popup()` の枠の内側余白（12px）とパレット幅（564px 固定・内容 540px）の重なりで
    バーが枠の内側に浮いて見える可能性があり、実物確認が要る。
  - 判断 1（追従方法）: (a) 選択行へ `scroll_to_rect` で寄せる（最小移動）
    (b) 選択行を常に中央に置く。推奨: **(a)**（VS Code と同じ感覚で、画面の揺れが小さい）。
  - 判断 2（適用範囲）: パレットだけでなく候補一覧を持つ重ね物すべて（全体検索・資料検索・
    使用箇所・昇格先選択）に広げるか。推奨: 広げる（共通部品化して 1 か所で直す）。
  - **決定（所有者、第 13 回）**: 選択行が見える位置までだけ寄せる最小移動の追従を、パレット、全体検索、資料検索、使用箇所一覧、昇格先選択へ共通部品で適用する。スクロールバーの扱いは、実物のスクリーンショットで症状を確定してから別途決める。
  - 判断 3（スクロールバーの扱い）: (a) 枠の端に寄せる（余白の外へ出す）
    (b) §9.5.1 の参照ペインと同じく「hover 時のみ細く出す」に寄せる。
    推奨: 実物のスクリーンショットで症状を確定してから決める（査定段階では未確定として残す）。
  - **決定（所有者、第 14 回）**: 候補一覧のスクロールバーは、内側余白の中に置かず重ね物の枠の右端へ寄せ、普段は細く目立たせず、ホバーやスクロール操作時に見える形にする。
  - 検証: ハーネスで候補を末尾まで送り、選択行の矩形がスクロール領域内にあることを検証する。
- **N29-10 パレットで最近使ったものを上に出す（アプリを閉じたら消す）**［仕様変更＋新機能］
  - 現状: 候補の順序は §9.7.1 の固定スコア（連続一致・語境界・先頭・スキップ）と ID 昇順のみで、
    利用履歴は使わない（`REF:src/app/palette.rs` の `rank`）。補完（§9.7.3）には
    「直近使用」が既に順位要素として入っている。保存先については §13.1.1 が
    「検索履歴はプロセス内メモリのみ」と既に定めており、Issue の「閉じたら全て消す」は
    現行規則と一致する（ファイルには書かない）。
  - 判断 1（決定論との両立）: 原則 P4 と §9.7.1「スコアリング（決定論、固定）」に触れる。
    (a) 履歴は同点のときだけ効かせる（固定スコアが主、履歴はタイブレーク）
    (b) 履歴にボーナス点を与えて順位を上げる (c) 入力が空のときだけ「最近使ったもの」を先頭に並べ、
    文字を打ったら通常順位に戻す。推奨: **(c)**。検索中の順序決定論を壊さず、
    「開いてすぐ再実行したい」という用途に応えられる。
  - 判断 2（対象モード）: `>` コマンド・`@` 節移動・`'` 辞書・資料・接頭辞なし検索の
    どこまで履歴を持つか。推奨: モードごとに分け、`>` と `@` から始める（効果が大きい順）。
  - 判断 3（保持件数と重複）: 上限（候補: 直近 10 件）と、同じ項目の再実行で先頭へ移す規則を決める。
  - 判断 4（ヘッドレスへの影響）: 履歴は GUI セッション状態に閉じ、ヘッドレスの候補生成には
    入れない（テストの決定論を守る）ことを推奨する。
  - **決定（所有者、第 7 回）**: 履歴は入力が空のときだけ先頭に出し、文字を入力した後は現行の固定スコアと ID 順へ戻す。対象モードは `>` コマンドと `@` 節移動から始め、各モード直近 10 件までをセッション内メモリに保持し、同じ項目を再実行したら先頭へ移す。ヘッドレスには持ち込まない。
- **N29-15 パンくずを押したらパレットを `@` 入力状態で開く**［仕様変更］
  - 現状: ヘッダとミニヘッダとステータスバーのパンくずはすべて押せないラベルであり、
    §9.4.2 はミニヘッダのパンくずを「クリック不可（移動はキーボード）」と明記している。
  - 判断 1（対象）: (a) ヘッダのパンくずだけ (b) 3 か所すべて。推奨: **(a)**。
    ステータスバーは §9.6 の圧縮表示の都合があり、誤クリックの面が増える。
  - 判断 2（初期入力）: (a) `@` だけ (b) 現在位置の接頭辞まで入れる（例: `@01-04-`）。
    推奨: **(a)**（Issue の文言どおり。広さ優先ドリルダウンの入口として素直）。
  - 判断 3（見た目）: 押せる要素の統一表現（下線・ホバー背景、§9.1.4・§9.9.2）を付けること。
    パンくずが無い画面（読者・エージェント等）では出さない現行挙動を維持する。
  - **決定（所有者、第 13 回）**: ヘッダのパンくずだけをボタン化し、押したら `@` だけを入れた状態でパレットを開く。ミニヘッダとステータスバーのパンくずは表示専用のままにする。押せる要素の統一表現を付け、パンくずが無い画面では出さない現行挙動を維持する。
  - 反映先: `REF:docs/specifications/ui.md` §9.1・§9.4.2（「クリック不可」の改訂）。
- **N29-02 グローバルナビの横幅が足りない**［バグ＋仕様変更の可能性］
  - 現状: ヘッダ（高さ 28px）には画面切替アイコン列・プロジェクト名・パンくず・
    切替直後の画面名（1 秒）・右端の 4 ボタン（ヘルプ・パレット・テーマ・言語）が並び、
    プロジェクト名には省略処理が無い。幅が狭いときの規則はステータスバーにはある（§9.6.3）が
    ヘッダには無い。参照ペインは幅 900px 未満で畳まれるが、ヘッダは無段階である。
  - 判断: (a) ステータスバーと同じ「幅に応じた省略優先順」をヘッダにも導入する
    （例: 画面名の一時表示 → プロジェクト名の省略 → パンくずの省略、切替アイコンと右端ボタンは残す）
    (b) パレットボタンの「Ctrl+P」文字をアイコン化して幅を稼ぐ
    (c) 2 段組みにする。推奨: **(a)** を主にし、プロジェクト名は末尾省略＋ホバー全文とする。
    (c) は §9.1 の骨格（28px）そのものの変更になるため避ける。
  - **決定（所有者、第 13 回）**: ヘッダにも幅に応じた省略優先順を導入し、画面名の一時表示、プロジェクト名、パンくずの順に省略する。画面切替アイコンと右端の 4 ボタンは残し、プロジェクト名は末尾省略＋ホバーで全文表示とする。
  - 検証: ハーネスの `frame_at` で幅を変え、はみ出しが無いこと（右端ボタンが画面内にあること）を検証する。
- **N29-22 プロジェクト選択に戻る動線がない**［仕様変更（発見可能性）］
  - 現状: `Ctrl+O`（保存して選択画面へ戻る 2 段構成、F-40、§9.8.6）と
    パレット `>open` は在るが、画面上のボタンが無い。プロジェクト名表示も押せない。
  - 判断: (a) ヘッダのプロジェクト名をボタンにし、押すと選択画面へ戻る
    (b) ヘッダ右端に専用アイコンを足す (c) 現状維持（ヘルプへの記載を厚くする）。
    推奨: **(a)**。ただし N29-15 でパンくずもボタン化すると「名前＝作品を変える／
    パンくず＝節を変える」という対応が読み取れる形になる。保存失敗中は現行 `Ctrl+O` と同じく
    戻らずに知らせる規則をそのまま使う。ヘッダ幅（N29-02）への影響も (a) が最も小さい。
  - **決定（所有者、第 13 回）**: ヘッダのプロジェクト名をボタンにし、押したら `Ctrl+O` と同じ規則でプロジェクト選択画面へ戻る。保存失敗中は戻らずに知らせる。
- **N29-24 ショートカット案内をすべてクリック可能に＋網羅＋テスト化**［仕様変更］
  - 現状: F-16 で「ガイドラベルのボタン化」は実施済みとされるが、残存箇所がある。
    (1) ヘルプモーダルのキー一覧は押せないラベルの列である。
    (2) 各画面末尾のキー案内 1 行（アイデア・資料・点検・エージェント・選択画面の
        `*.keys` 文言）は 1 つのラベルのままで、項目ごとに押せない。
    (3) 文言の陳腐化もある。資料画面の案内は今も「Ctrl+3 審査フォーム」と書くが、
        F-36 で `Ctrl+3` は参照ペインフォーカスに統一され、フォーム移動は `Tab` になった。
    一方、パレットの `?` モードはショートカットを検索してそのまま実行でき、
    `>` からも全コマンドが実行できる（F-15・F-25）。つまり「押して実行する」経路は
    パレットに集約されており、案内文言側が追いついていない。
  - 判断 1（網羅の定義）: 「案内」の範囲を (a) ヘルプ一覧と各画面末尾のキー案内に限る
    (b) ホバーのツールチップやステータスバーの補完表示まで含める。推奨: **(a)**。
  - 判断 2（実行の意味）: 案内を押したとき (a) そのコマンドを即実行する
    (b) パレットの `?` にその項目を選んだ状態で開く（確認してから実行）。
    推奨: **(a)** ただし破壊的・確認付きコマンドは既存の確認経路をそのまま通ること。
    ヘルプから実行する場合はヘルプを閉じてから実行する順序を仕様に書く必要がある。
  - 判断 3（テスト化）: (a) 案内の文言キーと実際のコマンド表（`REF:src/app/commands.rs`）を
    突き合わせる契約テスト（案内に書いたキーが実在のコマンドに対応すること）を足す
    (b) 画面ごとにボタンを押す GUI テストを足す。推奨: **(a)＋代表画面の (b)**。
    陳腐化（上記 (3)）は (a) で機械的に捕まえられる。
  - 判断 4（文言の持ち方）: 現在の `*.keys` は 1 本の文字列であり、項目ごとに押せるようにするには
    文言カタログの構造変更（項目単位のキー化）が要る。ja/en 両カタログの整合テスト
    （§12.3）の対象が増えることを実装計画に含める。
  - **決定（所有者、第 14 回）**: クリック可能にする案内は、ヘルプのキー一覧と各画面末尾のキー案内に限り、ツールチップやステータスバーの補完表示は含めない。`*.keys` を項目単位の文言キーへ分け、押したら即実行する。破壊的・確認付きコマンドは既存の確認経路を通し、ヘルプから実行する場合はヘルプを閉じてから実行する。案内の文言キーとコマンド表を突き合わせる契約テストに、代表画面で実際にボタンを押す GUI テストを加える。
- **N29-14 参照ペインの表示情報をクリックして飛べるようにする**［新機能］
  - 現状: 参照ペインで押せるのは、条件一致の資料候補（ダブルクリック／Enter でプロットへ追加）、
    資料の「続き...」、診断からの辞書・キャラ登録ボタン、資料内の URL に限られる
    （`REF:src/ui/widgets/reference_pane.rs` の `Chosen` は 3 種のみ）。
    上位プロット・配役・用語・今の節のプロットは表示専用である。
    コア側のモデル（`REF:src/core/features/reference.rs`）は表示用文字列しか持たず、
    飛び先（ファイル・行・節）の情報を持っていないため、コア→UI の順に拡張が要る。
  - 判断 1（飛び先の対応表）: 節ごとに決める必要がある。素案は次のとおり。
    今の節のプロット＝話プロットの当該節、上位プロット＝章／全体プロットの当該ブロック、
    配役＝配役画面の当該キャラ、用語＝用語の全文表示（現行 `Ctrl+Shift+W` 相当）または定義行、
    五感・空間・挿絵＝話プロットの当該フィールド、資料＝資料画面の当該資料（候補の現行動作とは分ける）、
    診断＝該当箇所の節（点検画面の Enter と同じ）。
  - 判断 2（操作の作法）: (a) シングルクリックで即移動 (b) 現行資料候補と同じ
    ダブルクリック／フォーカス＋Enter。推奨: **(b) に統一**。参照ペインは「読みながら触る」場所であり、
    テキスト選択やスクロールのつもりのクリックで画面が飛ぶ事故を避けるため。
    ただし資料の「候補（追加）」と「明示参照（開く）」は動作の意味が違うので、
    見た目（下線の有無など §9.9.2）で区別できることが条件になる。
  - 判断 3（キーボード完結）: `Ctrl+3` でフォーカスした後の項目移動モデル
    （`↑`/`↓` で項目、`Enter` で移動）を仕様化する必要がある（P3）。
  - 判断 4（ヘッドレス）: 参照ペインの取得操作（`reference.pane`）の戻りに飛び先を含めるか。
    推奨: 含める（コアのモデルに持たせる以上、API にも出すのが §19.1 の単一の真実に合う）。
  - **決定（所有者、第 7 回）**: 行の確定操作はダブルクリック、または `Ctrl+3` でフォーカスして `↑`／`↓` と `Enter` に統一する。シングルクリックでは移動させない。
  - **決定（所有者、第 14 回）**: 飛び先は、今の節のプロット＝話プロットの当該節、上位プロット＝章／全体プロットの当該ブロック、配役＝配役画面の当該キャラ、五感・空間・挿絵＝話プロットの当該フィールド、明示参照の資料＝資料画面の当該資料、診断＝該当箇所の節とする。用語は定義行へ飛ばず、`Ctrl+Shift+W` 相当の全文表示を開く。条件一致の資料候補は現行の「プロットへ追加」を保ち、明示参照の「開く」と見た目で区別する。`reference.pane` の戻りにも各行の飛び先・動作を含める。

#### G4 表示・フォント・ステータスバー

- **N29-03 読者モードでのみ一部の文字が豆腐になる**［バグ］
  - 現状と有力な原因: 読者モードの組版（`REF:src/core/reader.rs`）は縦書き時に
    括弧類を縦書き用互換文字（`「`→`﹁` など CJK 互換形）へ**無条件に置き換える**。
    フォントにその字が無いかを伝えるための注入項目（縦書きグリフの有無）は用意されているが、
    UI 側（`REF:src/ui/workspaces/reader.rs`）は常に空を渡しており、しかも括弧の置換は
    その注入を見ていない。同梱フォント（IPA ゴシック）に互換形が無ければ、
    読者モードの括弧だけが豆腐になる——症状と一致する。仕様 §17.3.3 の表には
    長音・ダッシュの「縦向きグリフ（なければ 90° 回転）」はあるが、括弧の互換形置換そのものは
    仕様に書かれていない（実装が仕様を越えている）。
  - 判断: (a) 置換を「フォントに在る場合だけ」にし、UI から収録字の判定を注入する
    (b) 置換をやめ、括弧は元の字を 90° 回転して描く (c) 互換形を持つフォントを同梱する。
    推奨: **(a)＋フォールバックとして (b)**。(c) は同梱サイズ（現在約 6MB）と
    WASM のサイズ検査へ響くため採らない。あわせて §17.3.3 に括弧の扱いを明記する。
  - 判断（検証）: 豆腐の検出は (a) フォント収録字を数値で確かめる単体テスト
    (b) ヘッドレス描画のスクリーンショット目視。推奨: 両方。（a）は「置換先コードポイントが
    同梱フォントに在るか」をテストにでき、将来フォントを差し替えたときの回帰を防げる。
  - **決定（所有者、第 15 回）**: 括弧の互換形置換は「フォントに在る場合だけ」とし、UI から収録字の判定を注入する。互換形が無い字は元の括弧を 90° 回転して描く。検証は、置換先コードポイントが同梱フォントに在るかを確かめる単体テストと、ヘッドレス描画のスクリーンショット目視の両方を行う。§17.3.3 に括弧の扱いを明記する。
- **N29-11 ステータスバーの省略しきい値が小さすぎる**［バグ］
  - 現状: 表示/省略は**ウィンドウ幅に対する固定しきい値**（パンくず 320px・文字数 410px・
    枚数とセッション 680px・通知 950px）で決まり、実際の文字の描画幅を見ていない
    （`REF:src/ui/app.rs` の `status_bar`）。修飾キーの補完表示も 23 字固定で切る。
    そのため「隙間があるのに `…` になる」状態が起きる。仕様 §9.6.3 は落とす順序だけを
    定めており、判定方法は定めていない。
  - 判断: (a) 各要素の実測幅（レイアウト結果）で「入るか」を判定する方式に変える
    (b) 固定しきい値を実態に合わせて引き直す。推奨: **(a)**。(b) はフォント倍率や
    言語（en は文言が長い）で再びずれる。保存インジケータの予約幅（右端）は現行の考えを維持する。
  - 検証: 幅を変えたフレームで「空きがあるのに `…` が出ていないこと」「保存表示が画面内に
    残ること」をハーネスで検証する。
  - **決定（所有者、第 15 回）**: 表示/省略は各要素の実測幅（レイアウト結果）で判定する方式に変える。固定しきい値の引き直しは採らない。保存インジケータの予約幅（右端）は現行の考えを維持する。
- **N29-12 プロジェクト選択画面が多言語対応していない**［バグ＋仕様変更］
  - 現状: 選択画面の文言自体は言語カタログ経由である（Web・デスクトップとも）。
    問題は選択画面の段階では**言語を切り替えるボタンが無く、選んでも保存先が無い**ことにある。
    言語設定の保存先はプロジェクト内の `.novnow/ui.json` であり（§12.4・§11.4）、
    プロジェクトを開く前は OS／ブラウザのロケールだけで決まる。`Ctrl+Alt+L` は届くが、
    プロジェクト未オープン時は設定変更がどこにも保存されない
    （`REF:src/ui/app.rs` の `settings_changed` はオープン中だけ dirty にする）。
    つまり「OS が英語・本人は日本語で使いたい」初回利用者は、言語を変える術が画面上に無い。
  - 判断 1（切替ボタンの設置）: (a) 選択画面にも言語・テーマの切替ボタンを出す
    (b) 出さない（OS 判定で足りるとする）。推奨: **(a)**。
  - 判断 2（保存先。規則 10 との衝突）: §13.1.1 は設定ファイルをプロジェクト外に置くことを禁じ、
    履歴ファイルの形式も「これ以外のキーを足してはならない」（§13.2.2）と固定されている。
    (a) 保存しない。選択画面で切り替えた言語はそのセッション限りとし、プロジェクトを開いた後は
        そのプロジェクトの `ui.json` に書く（現行の決定順序 §12.4 の範囲内）。
    (b) 履歴ファイルに言語・テーマだけを足すことを認め、規則 10 と §13.2.2 を改訂する。
    (c) Web だけブラウザ側の保存領域（メタ情報ストア）に言語を持つ（デスクトップと非対称になる）。
    推奨: **(a)**。規則 10 はこの製品の核の約束であり、言語 1 個のために例外を増やす利益は小さい。
    (a) でも「選択画面で日本語に変えてから作品を作る」ことは可能になる。
  - 判断 3: Web の入口 HTML の `lang="ja"` 固定とオフライン状態表示の二言語ベタ書きも
    同じ問題の表面であり、併せて扱うか。推奨: 併せて扱う（範囲は `REF:index.html` の範囲内）。
  - **決定（所有者、第 7 回）**: 選択画面に言語・テーマの切替ボタンは置かず、プロジェクトを開くまでの初期表示は現行どおり OS／ブラウザのロケールで決める。推奨した切替ボタンの設置は採らない。保存先を新設しないため、規則 10 と履歴形式は変更しない。
  - **決定（所有者、第 16 回）**: 入口 `REF:index.html` の `lang` 属性とオフライン状態表示の初期文言は、`navigator.language` に応じて動的に切り替える（アプリ本体の初期ロケール判定と揃える）。`lang="ja"` 固定と二言語ベタ書きはやめる。

#### G5 検索

- **N29-25 検索一致の太字が Markdown 記法のまま表示される**［バグ］
  - 現状: 検索が 2 系統ある。(1) `Ctrl+Shift+F` の重ね物は `REF:src/app/assist.rs` の
    文字どおり検索（本文のみ・上限あり）で、一致箇所の強調自体が無い。
    (2) 本文画面の検索タブは `REF:src/core/features/search.rs` の正規表現検索で、
    抜粋文字列に `**一致**` という Markdown の太字記法を埋め込んで作り、
    画面ではそれを平文のリンクとしてそのまま出している。症状は (2) である。
    保存される検索結果資料（S 番号）の本文にも同じ抜粋が入るが、そちらは
    §16.3.1 が Markdown 形式として `**…**` を仕様で定めているため正しい。
    なお実装の保存形式は §16.3.1 の固定形（見出し・クエリ情報・節 ID 付きの箇条書き）と
    食い違っており、これは別途切り分けが要る仕様・実装のずれである。
  - 判断 1（画面表示）: (a) 抜粋を「前文・一致・後文」の構造で返し、画面は一致部分だけを
    太字（または背景色 §11.3 の検索一致）で描く (b) 画面側で `**` を解釈して描く。
    推奨: **(a)**。表示用の記法をコアのデータに埋めるのは層の向きが逆であり、
    一致文字列自体に `*` が含まれる場合に (b) は壊れる。保存形式（Markdown）の生成は
    保存時に組み立てるよう分ける。
  - 判断 2（検索 2 系統の整理）: §9.7.4 は正規表現を `Ctrl+Shift+F` の機能として定めるが、
    実装では重ね物は文字どおり検索、正規表現は本文画面のタブ側にある。
    (a) 重ね物に正規表現・大小文字トグルを寄せ、タブ側は資料化と再検索に専念させる
    (b) 現行の分担を仕様に明記して追認する。推奨: **(a)**。ただし作業量が増えるため、
    M-1 で A を選ぶ場合は「太字表示の修正だけ先行し、統合は次バッチ」と割る余地がある。
  - 判断 3: 重ね物側にも一致の強調（§11.3 の背景塗り）を入れるか。推奨: 入れる（判断 1 の構造化と同時にできる）。
  - **決定（所有者、第 8 回）**: 現行の分担を追認し、重ね物は本文の文字どおり検索、Write 内の検索タブは正規表現検索とする。検索タブの画面表示への `**` の埋め込みはやめ、抜粋は構造化して一致範囲を強調する。保存される検索結果資料では Markdown の `**...**` を維持し、仕様 §9.7.4 と §16.3.1 はこの分担と表示・保存の分離に合わせて更新する。
  - **決定（所有者、第 15 回）**: 重ね物（`Ctrl+Shift+F`）側にも §11.3 の検索一致の背景塗りで一致強調を入れる。保存される検索結果資料の形式は、実装を §16.3.1 の固定形（タイトル見出し・`query`／`regex`／`case_sensitive`／`searched_at`／`hits`・節 ID 付きの結果箇条書き）へ寄せる。現行実装の「タイトル＋パスと byte 範囲＋抜粋」の形へ仕様を寄せる案は採らない。

#### G6 執筆支援の診断

- **N29-17 セリフ内の「。」の後に空白を開けているかを確認する機能**［新機能（診断追加）］
  - 現状: 該当する検査は無い。台詞範囲の判定部品（役割語照合と同じ括弧範囲の抽出）は
    既存であり、台詞比率や連続台詞（W082）と同じ土台で実装できる。
    文体診断の作法は §7.2 に固まっている（警告どまり・実行コスト区分・既定で無効なものは
    文体タブでのみ計算・有効/無効はセッション内の UI トグルのみでファイルに書かない）。
  - 判断 1（何を正とするか）: 流派が割れる事項である。
    (a) 「。」の後に空白（全角）があることを正とし、無い箇所を指摘する
    (b) 空白が無いことを正とし、ある箇所を指摘する
    (c) プロジェクト内で混在している場合だけ、少数派を指摘する（寄せる方向は作者の既存文面から決める）。
    Issue の文言は (a) を指すが、作品全体の既存文面と逆だと全件が指摘になる。
    推奨: **(c) を既定とし、寄せ先を UI トグルで (a)/(b) に固定もできる**形。
    ただし §7.2 の「設定ファイルを増やさない」に従い、固定先もファイルには書かない。
  - **決定（所有者）: (c)**（混在だけを指摘し、作品内の多数派へ寄せる）。寄せ先を固定する UI トグルの要否、多数決の集計単位と同数の場合、対象記号・空白の種類は、仕様化の段階で残り判断として確定する。
  - **決定（所有者、第 6 回）**: 対象は台詞内のみとし、作品全体を集計単位として、`。`・`！`・`？` の記号ごとに多数派を決める。同数または件数が不足する記号については診断しない。空白として数える文字、件数不足のしきい値、寄せ先固定トグルの要否は残り判断とする。
  - 判断 2（対象）: (a) 台詞内のみ（`「…」『…』` の内側。入れ子の扱いも決める）
    (b) 地の文も含める。推奨: **(a)**（Issue の指定どおり）。閉じ括弧直前の「。」の扱い
    （`…。」` の直後には空白を置かないのが通例）と、感嘆符・疑問符（`！？` の後は
    空白を開ける慣習が強い）を含めるかを併せて決める必要がある。推奨: 初版は「。」のみとし、
    `！？` は対象外と明記する（混ぜると流派論が広がるため）。
  - 判断 3（診断としての置き方）: 新規コード（W の空き番号）の追加は、仕様 §7.2・
    日英カタログ・Web サイトの解説ページ・網羅性 CI（§7.2.1）まで一式が要る。
    実行コストは「編集中は編集中の節のみ」、既定は**無効**（文体タブでのみ）とするのが
    §7.2 の前例（W081 等）であり、推奨もそれに沿う。クイックフィックスは
    「空白を入れる／取る」の 1 操作を付ける。
  - 判断 4（空白の種類）: 全角空白か半角空白か。文字数計算は空白除外（§9.6.1）であり、
    エクスポートは本文の空白を保持する。推奨: 全角空白を正とし、半角は「空白あり」とは数えない
    （日本語組版の慣習に合わせる）。英語作品ではこの検査自体を出さない（言語判定は
    プロジェクトの表示言語ではなく本文の文字種で行うかも含め、実装計画で確定する）。
  - **決定（所有者、第 9 回）**: 空白として数えるのは全角空白だけとし、半角空白は「空白なし」として扱う。記号ごとに、対象が合計 5 件以上あり、多数派が少数派より 2 件以上多い場合だけ診断する。寄せ先を固定する切替は初版に入れない。次の文が続かない閉じ括弧直前（`。」` など）は数えない。
  - **決定（所有者、第 15 回）**: 対象言語の判定は表示言語ではなく本文の文字種で行い、日本語主体でない本文ではこの診断を出さない。入れ子の台詞（『…』）も台詞内として数える。新規の W コードとし、実行コストは「編集中は編集中の節のみ」、既定は無効で、`project.toml` の feature 設定（診断コード単位、第 8・10 回決定）で有効化する。クイックフィックスは「空白を入れる／取る」の 1 操作を付ける。
- **N29-39 執筆サポートの拡充と高度な自然言語処理の検討**［検討］
  - 現状: 形態素は軽量な分かち書き部品（未知参照の範囲決め専用、S-09）だけで、
    文体診断はすべて形態素解析なしの決定論ルールである（§9.4.3・§9.4.4 がそれを MUST とする）。
    制約は強い。P4 決定論、コア純粋性（§3.2）、LLM SDK と HTTP クライアントの禁止
    （`REF:tests/no_llm_sdk.rs`）、許可クレート表（`REF:tests/layering.rs`）、
    新規クレートの搬入手順とコスト（`REF:docs/add-dependencies.md`）、WASM のサイズ検査。
    辞書同梱型の形態素解析器はサイズと搬入の両面で重い。
  - 判断: (a) 現行方針の延長で、ルールベースの診断を 1 つずつ足す（N29-17 がその型）
    (b) 分かち書きの精度を上げる部品へ差し替える（サイズ・搬入・決定論の検証が前提）
    (c) 高度な処理は MCP 経由の外部ツールに任せ、本体には入れない。
    推奨: **(a) を既定路線とし、(b) は「何が良くなるかの具体例」が出た時点で個別に判断する**。
    (c) は N29-31 の凍結方針と矛盾しない逃げ道として仕様に一文置く程度に留める。
  - **決定（所有者、第 16 回）**: 「確率・統計モデル」とは実行時に乱数を必要とするものを指す。決定論的であり、人間が中身を把握できる規模の小さな学習済みモデル（TinySegmenter 級）は許容する。ただしアプリ内で学習させてユーザーごとに最適化することは禁止し、独自に訓練した小さなモデルは全ユーザーに同一の学習結果を配布する場合に限る。その上で路線は、決定論ルールの逐次追加を基本としつつ、必要な診断が出た時点で小さな固定モデルの追加を許容する。追加のたびに「何が良くなるか」の具体例、サイズ、依存搬入、決定論（乱数なし）の検証を個別に行う。

#### G7 Web／PWA

- **N29-07 iOS＋PWA＋物理キーボードで画面位置がずれる**［バグ（実機依存）］
  - 現状: Web 版は eframe の隠し入力欄（テキストエージェント）を、入力欄にフォーカスがある間
    毎フレーム同期してフォーカスし直している（`REF:src/ui/web.rs` の `sync_text_agent`。
    キーボード操作で節を移ったときに IME が使えなくなる問題への対処であり、消してはならない）。
    iOS Safari は入力欄へのフォーカス時にその要素が見えるよう画面をスクロールさせるため、
    節移動のたびに再フォーカスが走ると表示位置がずれる、というのが有力な機序である。
    画面スクロール自体は `REF:index.html` で止めているが、視覚ビューポートの移動は別である。
  - 判断（対処の型）: (a) フォーカス時にスクロール抑止を試みる（ブラウザ差が大きく iOS では効かない場合がある）
    (b) 入力欄を常時フォーカスしたままにし、再フォーカス自体を無くす
    (c) フォーカスの直後にスクロール位置を復元する (d) 入力欄の置き場所を固定位置に寄せる
    （デスクトップブラウザの IME 候補窓の位置が悪化する副作用がある）。
    推奨: **(b) を軸に (c) を保険として併用**する方向で実装計画を立てる。
    ただしサンドボックスでは iOS Safari を再現できないため、完了条件は M-6（実機確認は分離）に従い、
    Chromium の Web スモークで「節移動後も入力が続くこと」を回帰として守る。
  - 判断（記録先）: 実機での解消確認が終わるまで `REF:docs/known-issues/ja.md` に載せるか。
    推奨: 公開版の症状として載せ、修正版の公開後に外す（既存運用どおり）。
  - **決定（所有者、第 16 回）**: 対処は、隠し入力欄を常時フォーカスしたままにして再フォーカス自体を無くす方式を軸とし、フォーカス直後にスクロール位置を復元する保険を併用する。Chromium の Web スモークで「節移動後も入力が続くこと」を回帰として守り、実機での解消確認は M-6 どおり分離する。症状は公開版の既知の問題として `REF:docs/known-issues/ja.md` に載せ、修正版の公開後に外す。

#### G8 ファイル形式と機能の管理

- **N29-30 JSON より TOML のような見やすい形式がよいかもしれない**［検討（仕様変更を伴う場合は大）］
  - 現状: JSON を使う面は性質が異なる。(1) 人が手で直す可能性があるのは実質
    `project.json`（§4.5）だけ。(2) `materials/index.json`（§5.8.2）は機械管理が主。
    (3) `agents/mcp.json`（§5.11）は MCP の慣行に合わせた JSON であり、外部ツールとの互換面である。
    (4) `.novnow/` の各 JSON と履歴ファイルは人が読む前提では無い。
    (5) CLI の `--json` 出力と API の値（§19）はファイル形式とは無関係。
    TOML の読み書き部品は文言カタログ用に既に依存へ入っている（`REF:Cargo.toml`）。
  - 判断: (a) 全面を TOML に変える (b) `project.json` だけ TOML 化する
    (c) 形式は変えず、アプリ内から設定を編集できる面を増やして「手で JSON を直す必要」を無くす
    (d) 現行維持（見やすさは整形・キー順の安定化で対応する）。
    推奨: **(c)**。(a) は MCP の慣行と衝突し、(b) は「どのファイルがどの形式か」の
    覚えにくさと二重のパーサ保守を生む。「見やすい形式がほしい」の実需は
    設定を触りたいときに JSON の括弧と格闘することであり、編集 UI 側で解くほうが安い。
    形式変更を行う場合でも M-2 の判断（互換の扱い）が前提になる。
  - 補足: TOML にするとコメントが書ける利点はあるが、現行の未知キー温存
    （パースして書き戻す方式）ではコメントは保存時に消える。コメントを守るには
    無損失編集方式への変更が別途要り、(b) を選ぶ場合はその費用も含めて見積もる必要がある。
  - **決定（所有者、第 2 回→第 3 回で確定）: `project.json` だけを `project.toml` に変更する**（推奨 (c) は不採用）。`materials/index.json`、`agents/mcp.json`、`agents/log/*.jsonl`、`.novnow/*.json`、API/CLI の JSON 出力は維持する。`project.toml` は、コメント・書式・キー順・未知キーを可能な限り保持する無損失編集を前提にする。形式版は `schema_version=2` とし、旧形式の特別検出はせず、必須ファイル不足などの通常エラーとして扱う（第 4 回決定）。
  - **決定（所有者、第 16 回）**: 無損失編集の具体方式は `toml_edit` の文書編集とする。意味の検証は型付き構造で行い、書き戻しは文書へ変更キーだけを反映してコメント・書式・キー順・未知キーを保持する。壊れた TOML は診断して上書きしない。`toml_edit` は現行の `toml` 依存が内包するが、直接依存として明示し、許可クレート表と搬入手順（`REF:docs/add-dependencies.md`）に従って追加する。
- **N29-33 「.txt」でいいのか**［検討］
  - 現状: 本文とプロットが `.txt`、人物・用語・資料などが `.md` であり、この区別には意味がある。
    `.txt` 側では行頭 `# ` がコメント、`.md` 側では見出しである（段階 ② の行コメント仕様）ため、
    拡張子は単なる見た目ではなく字句規則の一部になっている。形式の判定はパスから決まる
    （`REF:src/core/model/document.rs` の `DocumentFormat::of_path`）。
  - 判断: (a) 現行維持（`.txt` はどの環境でも素朴に開ける利点を取る）
    (b) `.md` に寄せる（字句規則の衝突解消と、外部エディタでの見え方の再設計が要る）
    (c) 独自拡張子を新設する（エディタ関連付けと hub での識別には有利だが、
    「ただのテキストとして開ける」性質と移行支援（§14.6）の前提が弱くなる）。
    推奨: **(a)**。「.txt でいいのか」の懸念が指すものが、OS の関連付けなのか
    外部公開時の見え方なのかで結論が変わるため、判断理由を所有者に確認したうえで
    「維持する理由」を仕様 §5.0 に一文で残すことを勧める（決めないままにしない）。
  - **決定（所有者、第 2 回→第 3 回で確定）: 現行 `.txt`（`contents/` と `plots/`）だけを `.cnov` に変更する**（推奨 (a) は不採用）。MD 方言の `.md` は変更しない。`.novnow` は Novnow プロジェクト全体のファイル形式として予約し、実体はプロジェクト全体を格納する Zip とする構想である。第 4 回決定により、`.novnow` は名前の予約に留めず、プロジェクト一式を入れた Zip パッケージ形式として定義し、次段の実装計画で現行 Zip の導入・書き出しと同じ安全規則で扱う。第 9 回決定により、字句規則は現行 `.txt` と同一とし、変更するのは拡張子とパス判定だけとする。旧 `.txt` 名については特別検出をせず、通常の必須ファイル不足として扱う。
  - **決定（所有者、第 17 回）**: 外部エディタ向けの案内として、ユーザーガイドに `.cnov` がただのテキストとして開けることと拡張子関連付けの手順を書き、既存の VS Code 拡張には `.cnov` の言語関連付け（最低限の色付け）を足す。
- **N29-35 feature ベースのリファクタリングと project.json による統一的なオン・オフ**［検討＋仕様変更の可能性］
  - 現状: オン・オフや閾値の置き場所は意図的に分散している。共有設定は `project.json`
    （構造・執筆閾値・エクスポート・エージェント・読者）、個人の表示設定は `.novnow/ui.json`
    （句切れ提案・折りたたみ等）、文体診断の有効/無効は**ファイルに書かずセッション内の
    トグルのみ**（§7.2 が明文でそう決めている）。実装側は段階 ② の追加機能が
    `stage_two` という寄せ集めの置き場所と巨大な単一ファイル（`REF:src/ui/app.rs`）に
    溜まっており、「機能単位で独立に管理したい」という要望の実装側の根拠は実在する
    （X3 で `stage_two` は `REF:src/core/features/` へ移し、UI は画面単位のファイルへ分割済み。`REF:docs/plans.md#issue-29-x3`）。
  - 判断 1（オン・オフの統一面）: (a) `project.json` に機能トグルの名前空間を新設し一元化する
    (b) 置き場所の現在の三分（共有／個人／セッション）を仕様として固定し、新機能は
    そのどれかに分類して増やす (c) 診断だけは現行どおりファイルに書かない原則を守り、
    それ以外を寄せる。推奨: **(b)**。(a) は §7.2 の決定（書いている最中に言われたくない
    指摘の制御を設定ファイルにしない）を覆し、作品を共有したときに相手の画面まで
    変えてしまう副作用がある。分類表を仕様に置くことが「統一的」の実質になる。
  - 判断 2（コード側の feature 分割）: (a) 機能単位のモジュール分割だけを行い、振る舞いは変えない
    (b) 機能の登録表（レジストリ）方式にし、画面タブや診断の出し分けを表駆動にする。
    推奨: **(a) から始める**。`stage_two` の解体と `app.rs` の分割は、仕様変更を伴わない
    内部整理として個別 PR に割るほうが安全であり、本 Issue の他の実装と混ぜない。
  - 判断 3（テスト行列）: 機能トグルを増やす場合は組み合わせの検証範囲を先に決める必要がある
    （全組み合わせは不可能なため、「既定＋全オフ＋個別 1 件ずつ」の型を推奨する）。
  - **決定（所有者、第 8 回）**: `project.toml` は一つの作品を複数人で扱うときに揃えるべき意味・検証を持ち、そこにだけ feature 名前空間を置く。ある指摘をエラーとして扱うかは feature ごとに `project.toml` 側で決める。一方 `.novnow/ui.json` は揃えなくてよい個人の表示好みに限り、エラーを画面に表示するかのような全 feature 共通の粗い設定だけを許可し、feature ごとの設定は置かない。feature の粒度と、現行のセッション内診断トグルをこの二層へ移す範囲は仕様化時に確定する。
  - **決定（所有者、第 10 回）**: feature の粒度は診断コードや操作ごとの細粒度とする。診断コード単位の設定は `project.toml` 側へ置き、現行のセッション内診断トグルのうち作品として揃えるべき有効性・エラー扱いはそちらへ移す。`.novnow/ui.json` には診断コード単位の設定を置かず、全 feature 共通の表示好みだけを置く。設定値の表現、グループ単位の継承、テスト行列は後続の判断で詰める。
  - **決定（所有者、第 17 回）**: 設定値の表現は、診断コード単位では `"off" / "warn" / "error"` の 3 値（出すかどうかとエラー扱いまでを表す）とし、操作系の feature は真偽値とする。グループ（例: 文体診断全体）の既定を置けて個別コードが上書きできる継承を許し、書かないキーは組み込み既定とする。テストは「既定設定・全 feature オフ・個別に 1 件ずつ切り替え」の行列で守る。コード側は機能単位のモジュール分割（`stage_two` の解体と `app.rs` の分割）だけを、振る舞いを変えずに別 PR で行い、レジストリ方式にはしない。
  - **決定（所有者、第 21 回、2026-10-27）**: `app.rs` の分割は独立した塊（プロジェクト選択画面、パレット、ステータスバー等）の切り出しまでとし、`NovnowApp` 本体は残す。実施は別 PR ではなく X1 と同じ PR（#38）で X1 の後に行う。
- **N29-36 テキスト以外のメディア拡張を見据えた拡張性**［検討］
  - 現状: 画像はキャラアイコン（§5.5.3。PNG/JPEG・大きさ上限・`assets/` 配下）だけが実体を持ち、
    挿絵はテキストのメモ（§5.2.5）、空間メモはテキストと外部参照（§5.3.6）である。
    Issue #14 ではメディア対応そのものは独立プロジェクトとし、novnow 形式側には
    拡張性だけを持たせる方針が書かれている。
  - 判断: (a) 今回は仕様に「拡張点の棚卸し」だけを置く（`assets/` の位置づけ、資料の出典種別、
    未知ファイルの温存規則を明記し、将来メディアを足しても壊れないことを保証する）
    (b) 画像資料など最小の実体対応を 1 つ入れる (c) 何も決めない。
    推奨: **(a)**。特に「プロジェクト内に在る未知のファイルを novnow がどう扱うか」
    （Zip 入出力・妥当性判定・外部変更監視での扱い）を明記しておくことが、
    将来のメディア追加と hub 連携の両方の土台になる。
  - **決定（所有者、第 17 回）**: 今回は仕様に拡張点の棚卸しだけを置く。`assets/` の位置づけと資料の出典種別の拡張余地を整理し、未知ファイルについては「novnow は削除・改変せず、Zip 入出力でも保持し、妥当性判定では無視する」という温存規則を明記する。メディアの実体対応は入れない。

#### G9 移行・サンプル・文書・整理

- **N29-41 Word など一般的なエディタからの移行支援を厚くする**［新機能＋仕様変更］
  - 現状: 移行支援（§14.6、L-01）は UTF-8 プレーンテキストのみ・自动分割なし・
    1 ファイルを 1 話 1 場面 1 節へ入れる最小形であり、実装も小さい
    （`REF:src/core/migration.rs`）。Web は対象外と明記されている。
  - 判断 1（入力形式の広げ方）: (a) 文字コードの拡張（Shift_JIS 等）だけを行う
    (b) `.docx` を読む (c) どちらも行う。推奨: 段階を割り、**(a) → (b)** の順とする。
    (b) は Zip 解凍部品の流用はできるが XML の読み取りが新規依存になりやすく、
    搬入手順（`REF:docs/add-dependencies.md`）の費用が乗る。段落・ルビ・傍点などを
    どこまで novnow 記法へ落とすかの変換表も仕様として要る。
  - 判断 2（分割の自動化）: 現行仕様は「自動分割・話者推定はしない」。厚くするなら
    (a) 空行や見出しらしい行で割る候補をプレビューで提示し、確定は人間が行う
    (b) 自動で割って入れる。推奨: **(a)**。現行のプレビュー必須の型を保ち、
    推定の誤りを原稿へ持ち込まない。
  - 判断 3（提供面）: CLI とデスクトップのみか、Web にも広げるか。推奨: 当面現行どおり
    （Web は対象外を維持）とし、需要が見えたら広げる。
  - **決定（所有者、第 8 回）**: 入力形式の拡張は Shift_JIS への対応に限る。貼り付け時にも文字コードを自動変換し、文字化けさせないことを受け入れ条件にする。本題は取り込んだ本文を話・節へどう分けるかであり、具体案を別途提示して決める。`.docx` は今回の対象に含めない。
  - **決定（所有者、第 9 回）**: 分割は候補提示型とし、確定前に必ずプレビューで境界を直せるようにする。候補生成の初期規則は次のとおりとする。
    - 複数ファイルは自然順に並べ、1 ファイルを 1 話、1 フォルダを 1 章として候補化する。単一ファイルは第 1 章・第 1 話から始める。
    - 「第○章」「第○話」等の見出し行では、章・話の境界候補を作る。
    - `＊＊＊` 等の区切り行と連続空行では、場面の境界候補を作る。
    - 段落ブロックの区切りでは、節の境界候補を作る。
    - プレビューでは章・話・場面・節のツリーと件数を示し、各候補を話／場面／節／境界なしへ変更できる。話者推定は行わない。
  - **決定（所有者、第 18 回）**: 見出し候補は単独の短い行だけを対象とし、「第○章」「第○話」（漢数字・算用数字、括弧や空白、末尾タイトル可）と、序章・終章・プロローグ・エピローグ・幕間・閑話・番外編を拾う。見出しの残りは章・話のタイトルとして使い、本文には残さない。番号が飛んだり重なったりしても、構造番号は出現順に振り直す。
  - **決定（所有者、第 18 回）**: 文字コード変換は、ファイル取り込み（CLI／デスクトップ）ではバイト列から判定し、正しい UTF-8 を優先して、読めない場合は Shift_JIS として変換する。曖昧な場合はプレビューで文字コードを切り替えられる。通常の貼り付けは Unicode のまま扱い、文字化けらしい文字列や置換文字がある時だけ、直した候補をプレビューして選ばせる。編集画面での分割は現行どおり明示的な区切りだけとし、Web のファイル移行は対象外を維持する。
  - **決定（所有者、第 21 回、2026-10-27）**: Shift_JIS 変換は `encoding_rs` クレートで実装する（設計書 overview §3.2 の許可クレート表へ追加する）。CLI は候補を編集できず生成した候補のとおりに生成し、`--preview` を事前確認とする。序章・終章・プロローグ・エピローグは章、幕間・閑話・番外編は話の境界候補とする。貼り付けの修復候補は複数行の入力欄すべてを対象にする（実装は貼り付けイベントの横取りで一元化する）。X1 と X3 は同じ PR（#38）で X1 → X3 の順に実施する。
- **N29-42 一つの小説として完成されたサンプルプロジェクト**［コンテンツ追加］
  - 現状: 付録 C のサンプル（`REF:examples/sample-novel/project.toml`）は
    「機能を一通り踏んだ最小例」が MUST であり、本文は 1 話の断片である。
    これは検証用の性質を担っており（テストが埋め込んで使う）、読み物としての完成とは別物である。
  - 判断 1（置き場所と役割分担）: (a) 検証用サンプルとは別に、読み物用を `examples/` へ追加する
    (b) 検証用サンプル自体を完成した短編に育てる。推奨: **(a)**。(b) は仕様の検証要素
    （モード併用・不一致例などを各 1 回含むこと）と物語の自然さが衝突し、テストの期待値も揺れる。
  - 判断 2（分量と作り方）: 短編 1 本（数話・数千字規模を想定）か、長編の冒頭数話か。
    誰が書くか（所有者執筆／エージェント生成／青空文庫等の既存テキストの変換）で
    ライセンス表記と保守の形が変わる。推奨: 所有者の意向確認を必須とする（本節では決めない）。
  - 判断 3（用途）: Web 版の初回体験に読み込ませるか、Web サイトで読めるようにするか、
    リポジトリ内に置くだけか。ガイドモード（§9.10）の段階との接続もここで決める。
  - **決定（所有者、第 5 回）**: 読み物サンプルの本文は所有者が書く。仕様側では検証用サンプルとは分けることを前提に、置き場所・分量・用途を残り判断として確定する。
  - **決定（所有者、第 10 回）**: 検証用とは別に `examples/` 配下へ数話で完結する短編プロジェクトを置き、Web の初回体験や Web サイト／ガイドから開ける読み物サンプルにする。
- **N29-32 ドキュメントの言葉の不自然さを直す**［文書作業］
  - 現状: 読みやすさの機械検査（`REF:docs/script/easy-read.py`）は利用者向け文書だけが対象であり、
    仕様書・設計書・README の文体は人の目に頼っている。仕様書は用語が規範そのものであるため、
    言い換えは意味の変更と紙一重である。
  - 判断: (a) 対象を利用者向け文書と README に限る (b) 仕様書・設計書も含めて全面を見直す。
    推奨: **(a) から始め、仕様書は「用語を変えない範囲の文の推敲」に限る**ことを明記する。
    (b) は差分が巨大になり、レビューで意味の不変を確認できなくなるため、本 Issue の
    実装 PR とは分ける。
  - **決定（所有者、第 18 回）**: 文体の見直しは別 PR の文書作業とし、まず利用者向け文書と README を対象にする。仕様書・設計書は用語と意味を変えない範囲の文の推敲に限り、意味が変わり得る言い換えは対象から外す。
- **N29-27 要らないファイルの整理**［整理作業］
  - 現状の候補: リポジトリ直下に動作確認用のプロジェクトが 2 つ置かれている。
    名前が紛らわしく（片方は `project.json` という名前のディレクトリ）、
    `REF:docs/developers.md` §3 のツリーにもそのまま載っている。生成物として必要なもの
    （サイト生成物・`guide/`・ゴールデン）は検査で存在が守られているため対象外である。
  - 判断: (a) 直下の 2 プロジェクトを削除する (b) `examples/` 配下へ移し名前を正す
    (c) テスト用の固定データ置き場へ移す。推奨: 参照の有無を機械的に確認したうえで
    **(a) または (b)**。あわせて「直下に置いてよいもの」の基準を `REF:docs/developers.md` §3 に
    一文で置き、再発を防ぐ。その他の候補（古いスクリーンショットの鮮度など）は
    §4.6.1 の「人が確かめるもの」に既に挙がっているため、棚卸し表を作ってから決める。
  - **決定（所有者、第 18 回）**: 直下の動作確認用プロジェクト 2 件（`contents/01-01-01-01.txt/` と `project.json/`）は、参照が無いことを機械的に確かめたうえで、分かる名前の 1 件に統合して `examples/` 配下へ移し、重複するもう 1 件は削除する。あわせて、リポジトリ直下に置いてよいものの基準を `REF:docs/developers.md` §3 に一文で置く。
- **N29-34 静的検証できる場所・できない場所の網羅と developers.md の更新**［文書＋検査追加の可能性］
  - 現状: 機械検査は既に厚い。文書規則（`REF:docs/script/duplicate.py`）、
    文書と実装の突き合わせ 12 項目（`REF:docs/script/reference.py`）、層と文字列の AST 検査
    （`REF:tests/layering.rs`）、仕様書を直接読む契約テスト群、ゴールデン、
    `REF:scripts/verify.sh` の一括検証が在る。「機械で見られないもの」は
    `REF:docs/developers.md` §4.6.1 に 5 件挙がっているが、ソースの領域別の
    「何がどの検査で守られているか」の地図は無い。
  - 判断: (a) 領域別（`src/core`・`src/app`・`src/ui`・CLI・LSP・Python・Web・文書・配布）の
    対応表を `REF:docs/developers.md` に作り、空白地帯を明示するに留める
    (b) (a) に加え、空白のうち機械化できるものを新しい検査として足す
    （例: 本節の N29-04 の網羅テスト、N29-24 の案内とコマンド表の突き合わせ）。
    推奨: **(b)**。ただし表の陳腐化を防ぐため、表に書く検査名は実在を `reference.py` の
    新規検査で確かめる形にする（検査一覧そのものを検査する）。
  - **決定（所有者、第 19 回）**: 領域別（`src/core`・`src/app`・`src/ui`・CLI・LSP・Python・Web・文書・配布）の対応表を `REF:docs/developers.md` に作り、空白地帯を明示したうえで、空白のうち機械化できるものを新しい検査として足す。表に書く検査名は `REF:docs/script/reference.py` の新規検査で実在を確かめる（検査一覧そのものを検査する）。

#### G10 配布・CI・運用

- **N29-01 GitHub Actions の最適化**［運用改善］
  - 現状: 既定で走らせない仕組みは既に厚い（`REF:docs/developers.md` §4.7。
    コミットの `[ci skip]` 既定、draft では走らせない、変更フィルタ、同時実行の取消、
    Rust のキャッシュ）。残っている費用源は、CI 本体（`REF:.github/workflows/ci.yml`）の
    ジョブ数（文書・整形・Lint・テスト・WASM・ベンチ・Python）と、周辺ワークフローの行列
    （ホイールの複数 OS、デスクトップ確認、ブラウザ確認、VSCode の実機確認）である。
  - 判断（何を最適化するかで打ち手が変わる）:
    (a) 課金時間の削減が目的: ベンチを通常実行から外す、PR の行列を代表構成に絞る、
        キャッシュの共有キーを寄せる、WASM ジョブ内の重複ビルドを畳む等。
    (b) 待ち時間の短縮が目的: ジョブ分割と並列度の見直し（課金は増えうる）。
    (c) 失敗の切り分けやすさが目的: レポート生成（§4.5）の範囲拡張。
    推奨: まず直近の実行時間の実測（ジョブ別の所要時間）を 1 回取り、(a) を主目的として
    上位の費用源から潰す。推奨の初期候補は「ベンチの分離」と「PR のデスクトップ行列の縮小」。
    あわせて §4.7 の表（変更種別ごとに回すもの）を実測に合わせて更新する。
    なお仕様 §18.4 の継続的検証要件に触れる変更（ベンチの頻度など）は仕様側の記述も直す必要がある。
  - **決定（所有者、第 5 回）: 主目的は待ち時間の短縮 (b)**（課金時間の削減を主目的にはしない）。ジョブ別の壁時計時間とクリティカルパスを実測し、並列度・キャッシュの効き・長いジョブの分割を優先する。課金時間は副次的な監視項目とする。
  - **決定（所有者、第 19 回）**: 具体策は、まずジョブ別の所要時間とクリティカルパスを 1 回実測し、上位の待ち時間源から潰す進め方とする。対策候補は、長いジョブの分割・並列化、キャッシュの効きの見直し、WASM ジョブ内の重複ビルドの畳み込みとし、`REF:docs/developers.md` §4.7 の表は実測に合わせて更新する。ベンチ頻度など仕様 §18.4 の継続的検証要件に触れる変更は、仕様側の記述も直す。
- **N29-26 VSCode Marketplace への自動公開**［運用＋仕様確認］
  - 現状: 仕様 §20.4.5 は「当面は Releases の `.vsix` で配布し、Marketplace 公開は
    遅くとも v1 到達までに開始する」と定めている。拡張の梱包は実装済み
    （`REF:vscode/package.json` に公開スクリプトも在る）だが、市場への公開ジョブは
    リリースワークフローに無い。公開には Azure DevOps 組織と発行者トークンという
    **人間しかできない手続き**が前提になる。
  - 判断: (a) v1 延期に伴い、市場公開も延期する（§20.4.5 の「v1 まで」を次版の節目へ読み替える）
    (b) V0 のうちに市場公開まで行う（手続きを所有者が行い、公開ジョブは承認付きで足す）
    (c) 市場公開は行わず `.vsix` 配布のみに固定する（仕様の改訂）。
    推奨: **(b) を目指すが、本 Issue の PR では「公開ジョブの用意と手順書まで」に留め**、
    実際の初回公開はトークン用意後とする。市場に出した版は取り消せない（§7.3 のリスク）ため、
    自動公開ではなく手動承認付きにする現行方針は維持する。Open VSX への同時公開は任意として分ける。
  - **決定（所有者、第 5 回）**: 今回は公開ジョブと手順の用意までとする。Azure DevOps 登録と発行者トークンの用意は所有者側の後続手続きとし、初回公開はその後に別途行う。
  - **決定（所有者、第 19 回）**: 公開ジョブは VS Code Marketplace と Open VSX の両方を用意する。所有者向けの手順ガイドは作らない。初回公開は両市場とも、登録とトークンの用意が済んだ後に別途行う。
- **N29-37 Contributor 受け入れ体制**［運用・文書］
  - 現状: PR テンプレートは在るが、貢献者向けの案内文書と Issue テンプレートと
    行動規範は無い。Web サイト側には「受け入れ準備が完了次第、受け付け欄を復活させる」
    という依存（Issue #8）がぶら下がっている。ライセンスは MIT である。
  - 判断: (a) 最小セット（貢献案内＋Issue テンプレート 2 種＋開発環境への導線）を作る
    (b) (a) に加えて行動規範と、仕様変更を伴う提案の進め方（承認制であること）まで書く。
    推奨: **(b)**。このリポジトリは「仕様書が正本・変更は承認制」という特殊な進め方であり、
    それを最初に伝えなければ貢献者とのすれ違いが必ず起きる。DCO や CLA の要否は
    所有者の判断として分けて確認する。
  - **決定（所有者、第 5 回）**: 外部コード貢献の受け入れ自体を当面見送る。したがって DCO／CLA は今回決めない。貢献案内を作る場合も、コード PR を受け付けていないことと、Issue／提案の扱いを明確にする。
  - **決定（所有者、第 19 回）**: 貢献案内として、「当面コード PR は受け付けていない」こと、Issue／提案の出し方、仕様変更は承認制であることを書いた案内文書と、Issue テンプレート（不具合報告・提案）を作る。行動規範は、貢献受け入れを再開する時点で判断する。
- **N29-38 実機での網羅的な確認**［運用］
  - 現状: 自動検証は Linux サンドボックスと CI の行列が担い、実機は未検証の切り分けが
    各計画に散在している（例: 本書 §5.6 の Python 検証、§7.2 の配布受け入れ基準の未チェック分）。
  - 判断: (a) 実機チェックリストを 1 か所に集約し、対象環境（Windows・macOS・Linux・
    iOS Safari PWA・Android Chrome を想定）と確認観点（IME・フォント・ファイルダイアログ・
    キー配列を含む）を列挙する (b) リリース手順の一部として配布手順側に置く。
    推奨: **(a)** を作り、(b) とは参照で結ぶ。本節の N29-05（キー配列差）・N29-07（iOS）・
    N29-03（フォント差）の確認項目はこのリストへ入れる。
  - **決定（所有者、第 20 回）**: 実機チェックリストは 1 か所に集約し、対象環境（Windows・macOS・Linux・iOS Safari PWA・Android Chrome を想定）と確認観点（IME・フォント・ファイルダイアログ・キー配列を含む）を列挙する。N29-05（キー配列差）・N29-07（iOS）・N29-03（フォント差）の確認項目はこのリストへ入れ、配布手順側からは参照で結ぶ。

#### G11 エージェントと将来構想

- **N29-31 エージェント関連機能は MCP サーバの枠を超えず現状で凍結する**［方針の明記先の判断］
  - 現状: §15 のエージェント機能は実装完了しており、本体が LLM へ直接つながない原則
    （MCP 経由のみ）はテストでも守られている（`REF:tests/no_llm_sdk.rs`）。
  - 判断: (a) 凍結を仕様 §15.1 の原則として明記する
    (b) 計画文書と履歴にだけ残し、仕様には書かない。
    推奨: **(a)**。将来の開発者（とエージェント）が「もっと賢い内蔵機能」を足し始めるのを
    仕様の段階で止められるのは (a) だけである。文面は「作るなら別ソフトウェアとして作る」
    という Issue の意図が伝わる形にする。なお凍結は「バグ修正をしない」意味ではないことも併記する。
  - **決定（所有者、第 20 回）**: 凍結は仕様 §15.1 の原則として明記する。文面は「作るなら別ソフトウェアとして作る」という意図が伝わる形とし、凍結はバグ修正をしない意味ではないことを併記する。
- **N29-40 novnow-hub のための git ラッパを実装するか（git には依存しないべき）**［別リポジトリ＋仕様確認］
  - 現状: 本体は `.git/` を読まず `git` コマンドも実行しない、という受け入れ基準が
    段階 ② の時点で確定している（本書 §4.5 の 19.14-14）。hub は別リポジトリで
    開発予定とされ（Issue #14）、本体とは「git と GitHub のような関係」と位置づけられている。
  - 判断: (a) 本体には入れない。バージョン管理は hub 側（別ソフトウェア）の責務とし、
    本体は「プロジェクトディレクトリをそのまま版管理できる素朴さ」を守る
    (b) 本体に任意機能として git 連携を足す（上記の受け入れ基準の撤回を伴う）。
    推奨: **(a)**。N29-31 の凍結方針と同じ「本体は小さく保ち、広げるときは別ソフトで」という
    一貫した線になる。本 Issue では「本体に入れない」ことを決めて閉じ、残りは Issue #14 側で扱う。
    その代わり、hub が本体の形式を読むための安定面（§19 の API とファイル形式）が
    何であるかは N29-36 の拡張点の棚卸しに含める。
  - **決定（所有者、第 20 回）**: git ラッパは本体には入れない。本体は `.git/` を読まず `git` コマンドも実行しない現行の受け入れ基準を守り、バージョン管理は hub 側（別ソフトウェア）の責務とする。本 Issue ではこの決定で閉じ、残りは Issue #14 側で扱う。hub が本体の形式を読む安定面（§19 の API とファイル形式）は、N29-36 の拡張点の棚卸しに含める。
- **N29-43 WASM 経由の TypeScript バインディング**［別リポジトリ前提の検討］
  - 現状: ヘッドレス面は Rust API と Python API と LSP と MCP と CLI（§19）であり、
    TypeScript 向けは無い。Web 版のビルドで WASM の土台は使っているが、
    ライブラリとしての公開面は用意されていない。
  - 判断: (a) hub の着手時に合わせて別途計画する（本 Issue では作らない）
    (b) 本体リポジトリで WASM バインディングの最小面から作る。
    推奨: **(a)**。配布形態が 1 つ増える判断（§20 の改訂、検査 12 の数への影響）であり、
    利用側（hub）の要件が固まる前に面を固定すると作り直しになる。
    本 Issue では「§19 の操作カタログが将来のバインディングの正本である」ことだけを確認する。
  - **決定（所有者、第 20 回）**: WASM 経由の TypeScript バインディングは本 Issue では作らず、hub の着手時に合わせて別途計画する。本 Issue では「§19 の操作カタログが将来のバインディングの正本である」ことだけを確認する。

#### G12 第 2 ラウンド（2026-10-27）: unchecked item の棚卸しと執筆支援ラウンド 2

- **N29-44 Issue #29 の unchecked item の棚卸し**［文書作業］
  - 現状: Issue 本文の unchecked item 38 件（うち空の 1 件）を §8.5 の実施記録・仕様書・コードと突き合わせた結果、32 件は解決済みでチェックボックスが stale（うち実装済み 29 件・決定で閉じ 3 件）だった。待っている 5 件は N29-07（実機確認とリリース待ち。コード修正は `REF:src/ui/web.rs` に反映済み）・N29-42（X2、所有者の短編）・N29-26（X5、トークン後）・N29-38（実機網羅、チェックリスト運用）・N29-39（執筆支援の拡充。実装が未計画だった）。
  - 判断: (a) 実装済みの 29 件と決定で閉じた 3 件を Issue 本文でチェックし根拠をコメントに置く (b) Issue は触らず報告のみ。
    推奨: **(a)**。Issue は backlog の正本なので stale な箱が次の棚卸しの邪魔になる。
  - **決定（所有者、2026-10-27 セッション）**: (a)。チェックするのは解決済みの 32 件のみで、所有者待ちの 5 件と空の 1 件は置いたままにする。
  - 実施注記: エージェントのトークンに Issue への書き込み権限が無い（HTTP 403、workflow dispatch と同権限圏）ため、チェック済みの本文と各チェックの根拠は PR 本文に添付し、Issue への反映は所有者が行うこととする。
- **N29-45 執筆支援ラウンド 2: 反復系診断 W097 / W098**［仕様変更］
  - 現状: 反復系の診断は W087（同一段落内で同じ自立語 3 回以上・段落単位）と W088（文頭の接続詞 3 文連続・文頭のみ）の 2 つしかない。文末尾の連続反復（「〜た。〜た。〜た。」）と、場面をまたいだ同語の反復（「言った」の乱用・固有名詞の繰り返し）は既存の 77 種いずれにも該当しない。N29-39 の路線（第 16 回決定: 決定論ルールの逐次追加が基本）に沿い、今回は新規依存なしで 2 件を計画する。TinySegmenter 級の固定モデルは、WASM の 8MiB 検査と依存搬入のコストを踏まえて今回は見送る（第 16 回決定の「必要な診断が出た時点で個別に判断」に留める）。
  - 判断: (a) W097（文末尾の反復）＋W098（場面内の同語反復）を両方計画 (b) W097 のみ (c) W098 のみ (d) V1 に延期。
    推奨: **(a)**。両方とも決定論・新規依存ゼロ・既定無効で、N29-17（W096）と同一のパターンに乗る。(a) を取った場合の既定値: W097 は同一節内で同じ文末尾（句点直前の 2 字）3 文連続で発火（`「…」` の内側は対象外）、W098 は W087 と同じ語抽出で場面内に 5 回以上出現で発火（最後の出現位置に出す）。
  - **決定（所有者、2026-10-27 セッション）**: (a) の両方を計画する（上記の推奨した既定値どおり）。TinySegmenter の導入は行わない。実装は B11（§8.5）。
- **N29-27（残）要らないファイルの最終スキャン**［整理作業］
  - 現状: B9 でリポジトリ直下の動作確認用プロジェクト 2 件を `examples/manual-check/` へ統合済み。リポジトリ全体の最終スキャンは未実施だった。
  - **決定（所有者、2026-10-27 セッション）**: 第 2 ラウンドの PR でリポジトリ全体をスキャンし、不要なもの（参照が無いと機械的に確かめたもの）を整理して本項目を完了させる。
    スキャン結果（2026-10-27）: 不要ファイル 0 件。対象はルート・`scripts/`・`examples/`・`docs/`・`licenses/`・`assets/`・ワークフロー・生成物（`guide/` と `website/public/guide/` は生成規則を持つため対象外）で、未追跡ファイルも無い。B9 の整理で既に整理済みと確認。

### 8.3 判断が要らない（または小さい）事項の確認

- N29-19 は N29-04 に、N29-23 は N29-18 に、N29-13 は N29-09 にそれぞれ統合する。
- N29-08 は仕様どおりへ実装を戻す方向で判断を閉じる（仕様変更なし）。
- 本書冒頭のマイルストーン記述は、この仕様更新で v0 次版の位置づけへ直した。

### 8.4 仕様書・設計書への反映先（確定・反映済み）

判断は次の対応で仕様書・設計書へ反映した。該当する設計書の節は「未実装・実装計画済み」へ落とし、
実装は §8.5 のバッチで行う。

| 対象 | 主な判断 | 反映先 |
|---|---|---|
| モーダルの閉じ方・配置 | M-3・N29-04・N29-16 | `REF:docs/specifications/ui.md` §9.7・§9.8 と対応する設計書 |
| 節の削除・分割 | N29-05・N29-06 | `REF:docs/specifications/ui.md` §9.3・§9.4・§9.8・§10.2、`REF:docs/specifications/editing.md` §8.1、`REF:docs/specifications/interfaces.md` §19.7、`REF:docs/specifications/project.md` §4.4.1（確認設定）と各設計書 |
| 台詞比率バーの形 | N29-08 | 仕様変更なし（実装を §9.4.2 へ戻す） |
| 編集面の左線 | N29-20 | `REF:docs/specifications/ui.md` §9.2・§9.4・§11 と対応する設計書 |
| パレットの履歴・追従 | N29-09・N29-10 | `REF:docs/specifications/ui.md` §9.7 と対応する設計書 |
| パンくずと作品名のボタン化・ヘッダ幅 | N29-02・N29-15・N29-22 | `REF:docs/specifications/ui.md` §9.1・§9.4・§9.6 と対応する設計書 |
| 案内のクリック実行 | N29-24 | `REF:docs/specifications/ui.md` §9.8.7（新設）・§9.7.5（実行可能性の共通規則）と対応する設計書 |
| 参照ペインからの移動 | N29-14 | `REF:docs/specifications/ui.md` §9.5、`REF:docs/specifications/overview.md` §3.4 と各設計書 |
| 読者モードの括弧 | N29-03 | `REF:docs/specifications/workflows.md` §17.3 と対応する設計書 |
| ステータスバーの省略判定 | N29-11 | `REF:docs/specifications/ui.md` §9.6 と対応する設計書 |
| 選択画面の言語 | N29-12 | `REF:docs/specifications/platform.md` §13.4.2（新設。§12・§14.4 は現行どおりで変更なし）と対応する設計書 |
| 検索結果の表示と検索の分担 | N29-25 | `REF:docs/specifications/ui.md` §9.7、`REF:docs/specifications/workflows.md` §16.3 と各設計書 |
| 台詞内の空白診断 | N29-17 | `REF:docs/specifications/diagnostics.md` §7.2、`REF:docs/specifications/ui.md` §10.9 と各設計書（Web サイトの解説も §7.2.1） |
| `project.toml`、`.cnov`、`.novnow` Zip パッケージ | M-2・N29-30・N29-33 | `REF:docs/specifications/project.md` §4.1・§4.5、`REF:docs/specifications/syntax.md` §5.0〜§5.4、`REF:docs/specifications/platform.md` §13.5・§14.2・§14.5 と各設計書 |
| 設定の置き場所の分類 | N29-35 | `REF:docs/specifications/project.md` §4.4・§4.5 と対応する設計書 |
| 未知ファイルと拡張点 | N29-36 | `REF:docs/specifications/project.md` §4.1・§4.3、`REF:docs/specifications/platform.md` §13.5 と各設計書 |
| 移行支援の拡張 | N29-41 | `REF:docs/specifications/platform.md` §14.6 と対応する設計書 |
| エージェント凍結 | N29-31 | `REF:docs/specifications/agent.md` §15.1 と対応する設計書 |
| 市場公開の時期 | N29-26 | `REF:docs/specifications/external.md` §20.4・§20.6 と対応する設計書 |
| 静的検証の地図 | N29-34 | `REF:docs/developers.md` §4.6 へ表を作り、検査名の実在は `REF:docs/script/reference.py` の新規検査で確かめる（仕様書の対象外。B9 で実施） |
| 自然言語処理と固定モデルの方針 | N29-39 | `REF:docs/specifications/overview.md` P4 と対応する設計書 |
| unchecked item の棚卸し | N29-44 | 仕様書の変更なし（棚卸しは本 §8.2 G12 と PR 本文・Issue のコメント。Issue 本文の実装済み 32 件は所有者の判断でチェック済み） |
| 反復系診断 W097 / W098 | N29-45 | `REF:docs/specifications/diagnostics.md` §7.2.3・§7.2.4（新設）と `REF:docs/specifications/ui.md` §9.4.4（算法）。`REF:docs/designs/diagnostics.md` §7.2（§7.2.3 / §7.2.4 を新設し、節の状態を未実装・実装計画済みへ）。§7.2 の表行・ja/en カタログ・サイト解説（§7.2.1）・「77 種」の数値主張は B11 で同時追加（§8.6） |
| 要らないファイルの最終スキャン | N29-27（残） | 仕様書の変更なし（整理作業。結果は本 §8.2 G12 と history） |
| 外部エディタとの付き合い方 | N29-33（第 17 回） | 拡張の `.cnov` 関連付けは `REF:docs/specifications/external.md` §20.4.4、ユーザーガイドへの説明追加は B9 |

### 8.5 実装計画（確定）

実装は次セッション以降に、1 バッチ = 1 PR を原則として行う。推奨順は
**B1 →（B2・B4・B5・B6 は画面系で独立）→ B3 → B10 → B7 → B8・B9** であり、
B7 は B10 の `[features]` 基盤を使うため B10 の後にする。X1〜X5 は Issue #29 の
後続の別 PR であり、順序は任意である（2026-10-27 の所有者指示により、X1 と X3 は
PR #38 で X1 → X3 の順に続けて実施する）。

共通の受け入れ条件（全バッチ）:

- `REF:scripts/verify.sh` を全て通す。UI と Web に触るバッチは `REF:scripts/web-smoke.sh` も通す。
- 対応する設計書の節の状態を「実装完了」へ戻し、`REF:docs/plans.md` と `REF:docs/handoff.md` の SYNC 表を実測値へ更新する。
- 実機での目視確認は M-6 のとおり完了条件から外し、実機チェックリスト（B9）へ回す。

1. <a id="issue-29-b1"></a>**B1 重ね物の土台（実装完了）**（N29-04・N29-19・N29-16）:
   設計は `REF:docs/designs/ui.md` §9.7 のメモに従い、`popup()` へ外クリック閉じを接続し、配置を 2 系統へ寄せ、複雑なフォームの下書きをセッション内に保持する。
   受け入れ条件: 全モーダルの棚卸しに沿った網羅テスト（開く・確定・閉じるの 3 状態）が通り、Esc の意味が表どおりであること。
   実施記録: 分類（ `OverlayKind` ）・外クリックの中央処理・配置 2 系統・共通フッタ・下書き（ `OverlayDrafts` ）を `REF:src/ui/app.rs` に実装し、網羅テスト 3 件を `REF:tests/gui.rs` に追加した。配置変更後の見た目はヘッドレス描画（パレット）で確認した。
2. <a id="issue-29-b2"></a>**B2 節エディタ（実装完了）**（N29-08・N29-18・N29-23・N29-20）:
   比率バーを仕様 §9.4.2 の形へ戻し、行番号・キャレットの整列をベースライン基準にして、複数行エディタの左線を二段表示にする（`accent.faint` の追加を含む）。
   受け入れ条件: 空行と通常行の双方でベースライン差が許容範囲内であることの幾何テストと、両テーマでの比率バーの表示確認。
   実施記録: 共通部品 `ratio_bar`・`field_accent`・`paint_field_accent`（`REF:src/ui/widgets/mod.rs`）、`row_baseline` と幾何テスト（`REF:src/ui/widgets/syntax_text.rs`）、`accent.faint`（`REF:src/ui/theme.rs`、仕様表との一致は `REF:tests/gui.rs`）を実装した。比率バーと左線・行番号の見た目はヘッドレス描画で確認した。
3. <a id="issue-29-b3"></a>**B3 節の操作（実装完了）**（N29-05・N29-06）:
   文脈ビュー／プロット一覧へ削除ボタンを追加し、全入口で「本文と対応プロットの一体削除」へ意味を統一する。分割はカーソルのバイト位置どおりとし（`section.split`）、空節では新たな空節を作らない。`section.split` は、このバッチで仕様 §19.7 の表へ行として取り込み、OPS・各インターフェース・ゴールデン・Web サイトの API 表を同時に更新する。
   受け入れ条件: 分割位置の保存→再読込の往復、対応不明時に何も消さないこと、キー配列互換（実機チェックは B9 のリストへ）。
   実施記録: 分割は `sections::split_body`＋`Workbench::split_section`（ `Ctrl+Enter` と `section.split` の共通経路）で実装し、再解析の往復をテストにした。削除は `Workbench::delete_section_at` の単一経路へ統一し（対応が一意でないときは何も消さない）、文脈ビューとプロット一覧に削除ボタンを置いた。確認は `confirm_section_delete` 個人設定に従う。`section.split` を §19.7 の表へ取り込み 120 操作・カタログ版 1.1 とし、生成物一式を更新した。あわせて `; // コメント` 付き区切りを削除が認識しない既存の不整合を、字句器の分類で判定するよう直した。
4. <a id="issue-29-b4"></a>**B4 パレットとヘッダ（実装完了）**（N29-09・N29-13・N29-10・N29-15・N29-22・N29-02・N29-11）:
   パレットの追従スと「空入力時だけ出す」最近履歴、パンくずと作品名のボタン化、ヘッダの省略とステータスバーの実測幅判定。
   受け入れ条件: キーボード移動だけで選択行が常に見えること、幅を変えたフレームで保存表示が画面内に残ること。
   実施記録: 候補一覧の共通部品（ `candidate_scroll` / `follow_selection`、`REF:src/ui/widgets/mod.rs`）、モード別履歴（最大 10 件）、ヘッダの実測省略とボタン化、ステータスバーの実測幅判定を実装し、`REF:tests/gui.rs` に追従ス・履歴・ヘッダのテストを足した（ステータスバーの既存テストは実測判定の期待値へ更新）。
5. <a id="issue-29-b5"></a>**B5 案内と参照ペイン（実装完了）**（N29-24・N29-14）:
   キー案内の項目実行と、参照ペイン行からの移動（ダブルクリック／Enter）。コアの行モデルへ移動先を持たせ、ヘッドレス `reference.pane` の返却値も同時に更新する。
   受け入れ条件: ゴールデンの差分が移動先フィールドの追加だけであること、シングルクリックで動かないこと。
   実施記録: 行モデルへ `Nav` を追加し（`REF:src/core/features/reference.rs`）、`reference.pane` の返却値へ `nav`・診断の `byte`/`path`（追加のみ）を足した。ペインの行はダブルクリックと `Ctrl+3`+`↑`/`↓`+`Enter` で移動する（シングルクリックでは動かない）。キー案内は項目単位の文言キー＋共通部品 `key_guide` へ分け、ヘルプ項目はコマンド表経由で実行する（契約テスト付き）。資料画面の陳腐化した案内（ `Ctrl+3` ）は `Tab` へ直した。
6. <a id="issue-29-b6"></a>**B6 読者と検索（実装完了）**（N29-03・N29-25）:
   読者モード括弧の豆腐解消（フォント収録判定の実配線）と、全文検索タブの一致強調の構造化（表示と保存の分離）、全体検索の一致塗り。
   受け入れ条件: 収録判定の単体テストとヘッドレス描画の目視確認、保存した資料の書式が §16.3.1 の固定形どおりであること。
   実施記録: 読者は egui の `has_glyph` で互換形の収録を確かめて組版へ渡す実配線にした（同梱 IPA ゴシックには互換形が皆無で、同梱時は元の括弧の回転描画になる）。検索の抜粋は前文・一致・後文へ構造化し、画面は一致だけを §11.3 の背景塗りで強調、資料保存は §16.3.1 の固定形（メタ行＋節 ID 付き箇条書き＋500 件註記）で組み立てる。全体検索（ `Ctrl+Shift+F` ）の結果行にも同じ背景塗りを付けた。
7. <a id="issue-29-b7"></a>**B7 診断の追加（実装完了）**（N29-17）:
   W096（台詞内の句読点直後の空白、記号別多数派・合計 5 件以上かつ差 2 件以上）と、点検側の有効設定の編集面（§10.9.6）。W096 は、このバッチで仕様 §7.2 の表へ行として取り込み、コード・日英カタログ・サイト解説（§7.2.1）を同時に足す。
   前提: B10 の `[features.diagnostics]`。受け入れ条件: 同数・件数不足では出ないこと、集計が文字種で切り分くこと。
   実施記録: W096 を実装し、仕様 §7.2 の表（行・コスト表・既定リスト）・コード（カタログ 77 件）・日英カタログ・サイト解説データを同時に揃えた。生成は記号別の作品全体集計（合計 5 件以上・差 2 件以上で少数派のみ）で、組み込み既定は無効、有効化は `[features.diagnostics]`。日本語主体かは本文の文字種で決める。クイックフィックス（空白を多数派に合わせる 1 操作）と、点検画面の有効設定の編集面（選択中コードの 出さない/警告/エラー扱い を `project.toml` へ無損失保存、§10.9.6）、文体タブの指標への追加まで含む。
8. <a id="issue-29-b8"></a>**B8 選択画面と Web（実装完了）**（N29-12・N29-07）:
   入口ページのロケール追従（切替ボタンは置かない）と、iOS のフォーカスずれ対策（常時フォーカス＋位置復元）。
   受け入れ条件: Web スモークで「節移動後も入力が続くこと」を回帰として持ち、ja/en 両ロケールの初期表示を確認する。実機確認は B9 のリストへ回し、既知の問題の掲載は修正版の公開後に外す。
   実施記録: `REF:index.html` の `lang`・題・オフライン状態表示を `navigator.language` で出し分け、Web スモークに ja/en 両ロケールの初期表示チェックを足した。iOS のフォーカスずれには、隠し入力欄を短い離脱では隠さず再フォーカスを無くす変更と、フォーカス直後のスクロール位置復元を入れ、「節移動後も入力が続くこと」をスモークの回帰にした。症状は既知の問題 KI-002 として掲載し、実機確認は B9 のチェックリストへ回す。
9. <a id="issue-29-b9"></a>**B9 文書・運用（実装完了）**（N29-34・N29-27・N29-38・N29-37・N29-01・N29-26 準備）:
   静的検証の地図（`REF:docs/developers.md` §4.6 の領域別表と、検査名の実在を確かめる新規検査）、リポジトリ直下のファイル整理（動作確認用プロジェクト 2 件を、参照が無いことを確かめて `examples/` へ 1 件へ統合）、実機チェックリストの集約（対象環境と、IME・フォント・ダイアログ・キー配列の観点。N29-03・N29-05・N29-07 の確認項目を含む）、貢献案内と Issue テンプレート（当面コード PR は受け付けない旨を含む）、Actions の待ち時間の実測と短縮（ジョブ別所要時間の測定から。仕様 §18.4 に触れる変更は仕様も同時に直す）、両市場への公開ジョブの用意（初回公開は含まない）。
   実施記録: 検証の地図を `REF:docs/developers.md` §4.6.2 に領域別表として作り、`REF:docs/script/reference.py` に検査 13（地図の検査名の実在）を足した。直下の動作確認用プロジェクト 2 件（同一内容）は参照が無いことを確かめ、現行形式へ直して `examples/manual-check/` の 1 件に統合した。実機チェックリストは `REF:docs/developers.md` §4.9 に集約した。貢献案内 `REF:CONTRIBUTING.md`（当面コード PR は受け付けない旨と、承認制の進め方）と Issue テンプレート 2 種を用意した。Actions はジョブ別に実測し（CI 約 3.3 分、Benchmarks が最長）、`bench` からフレーム予算テストを `perf` へ分けて並行化し、重複ビルドを畳んだ（§18.4 の項目は減らしていない。実測は `REF:docs/developers.md` §4.7）。両市場の公開ジョブ `REF:.github/workflows/vscode-publish.yml`（手動実行＋承認 environment、トークンが無ければ何もしない）を用意した。
10. <a id="issue-29-b10"></a>**B10 形式の更新（実装完了）**（M-2・N29-30・N29-33）:
    `project.json` から `project.toml`（無損失編集、`schema_version = 2`、feature 名前空間を含む）への変更、`contents/`・`plots/` の `.txt` から `.cnov` への改名（文法は不変、`DocumentFormat` の厳密一致化、サンプル・テンプレート・ゴールデンの追随）、未知ファイルの温存の固定、`.novnow` Zip パッケージ対応、VS Code 拡張の `.cnov` 関連付け。仕様 §14.2.2 / §14.2.3 の出力例は不足一覧の実装と同時に差し替え、`REF:tests/project_json.rs` は仕様 §4.5 の TOML の例を読む形へ戻す。他の実装バッチとは混ぜない。
    受け入れ条件: 旧形式プロジェクトが通常エラー（必須不足）になること、コメント付き TOML の無損失往復、壊れた TOML は上書きしないこと。
    実施記録: `project.toml`（schema_version 2）へ移行した。読み取りは TOML の値から型付き検証し、書き戻しは `toml_edit` の無損失編集（変更したキー以外はコメント・書式・キー順・未知キーを保持。壊れた TOML は E025 で開き上書きしない）。`[features.diagnostics]` の解決（個別コード→グループ→default→組み込み既定）を診断生成へ適用し、`"off"` は出さず `"error"` は `check` の終了コード 5 に含める。`contents/`・`plots/` は `.cnov` のみを §5.0.9 の厳密一致で文書とし、旧形式は未知ファイルとして触れない（旧形式プロジェクトは必須不足の通常エラー）。仕様 §14.2.2 / §14.2.3 の出力例と E025 の文言（JSON / TOML 共通化）、§13.1.1 の特例表（`.novnow`）を合わせ、VS Code 拡張へ `.cnov` の関連付けと最低限の色付けを足した。`toml` / `toml_edit` を core の許可クレートへ追加した。
11. **後続の別 PR（Issue #29 の残作業）**:
    - <a id="issue-29-x1"></a>**X1 移行支援の拡張（実装完了）**（N29-41）: Shift_JIS 対応（ファイルはバイト列判定、貼り付けは候補プレビュー）と、候補提示型の章・話・場面・節分割（見出し・区切り行・連続空行・段落）。CLI の複数入力の引数の並びもここで仕様 §14.1 へ取り込む。
      実装プラン（2026-10-27 確定）: (1) `encoding_rs` を依存へ追加する（許可クレート表・`REF:tests/layering.rs`・vendoring の blob 搬入を含む）。(2) `REF:src/core/text/encoding.rs` に読み替え（`decode` / `decode_as`）と貼り付け修復の判定（`mojibake_repair`）を置く。(3) `REF:src/core/migration.rs` を候補生成（`analyze`）・構造確定（`structure`）・描画（`render`）へ作り替え、`TemplateFile` のパスを所有文字列にする。(4) CLI（`REF:src/cli/mod.rs` の解析と `REF:src/cli/migrate.rs`）を複数入力・フォルダ入力・候補どおりの生成とプレビューへ広げる。(5) デスクトップの移行プレビュー（ツリー表示・段変更・文字コード切り替え）を NativePicker へ接続し、複数ファイル選択をポートへ足す。(6) 貼り付け修復をフレーム先頭の横取りで GUI 全体（デスクトップ・Web）へ接続する。
      受け入れ条件: Shift_JIS の原稿（複数ファイル・フォルダ）が文字化けなく章・話・場面・節へ分かれて移行でき、生成したプロジェクトが診断エラー無く開けること。CLI のプレビューが何も作らないこと。候補を全て「境界なし」にすると従来の 1 話 1 場面 1 節と等価になること。貼り付けは、UTF-8 を Shift_JIS で読み違えた文字列でのみ修復候補が出て、通常の日本語・英語では出ないこと。
      実施記録（2026-10-27）: `encoding_rs` 0.8.35（必須依存が cfg-if だけの版に固定）を許可クレートとして追加し、`REF:src/core/text/encoding.rs` に読み替え（`decode` / `decode_as`。UTF-8 優先→Shift_JIS、曖昧の記録）と貼り付け修復の判定（`mojibake_repair`。Shift_JIS 再符号化→UTF-8 読み直し、欠損は置換文字で残し半分以上なら出さない）を置いた。`REF:src/core/migration.rs` を候補生成（`analyze`）・構造確定（`structure`）・描画（`render`）へ作り替え、`TemplateFile` のパスは所有文字列へ変更した。見出し（第○章/第○話・序章等 7 語。番号は出現順に振り直し、タイトルは取り出して本文に残さない）・記号 3 字以上の区切り行・空行 1 つ＝節/2 つ以上＝場面 を候補にし、ファイル境界は話・フォルダ先頭は章。`novnow migrate <入力…> <dir> [--preview]` はファイル・フォルダ混在（フォルダは直下の .txt/.md を自然順）で候補どおりに生成し、プレビューは構造の件数と変換後本文を何も作らず表示する。デスクトップは複数ファイル選択（`Dialog::pick_texts`）と、候補の段変更・曖昧な入力の文字コード切替ができるプレビューを NativePicker に置いた。貼り付け修復はフレーム先頭の横取り（複数行の貼り付けのみ）で候補を提示し、選択した文字列を次のフレームで元の入力欄へ渡す（デスクトップ・Web 共通）。検証は `REF:tests/migration.rs`（読み替え・候補・描画・修復・境界なしで従来の往復と等価）・`REF:tests/cli.rs`（複数入力・フォルダ・Shift_JIS）・`REF:tests/gui.rs`（提示・選択・通常の貼り付けに出ない）。設計書 §14.1・§14.6 を実装完了へ戻した。設計書 §14.1・§14.6 を「実装完了」へ戻す（全 142 節の完了）。
    - <a id="issue-29-x2"></a>**X2 読み物短編サンプル**（N29-42）: 検証用サンプルとは別に、所有者が書いた短編を `examples/` へ置き、Web の初回体験とサイト／ガイドから開けるようにする。
    - <a id="issue-29-x3"></a>**X3 コードの機能単位分割（実装完了）**（N29-35 の後半）: `stage_two` の解体と `app.rs` の分割。振る舞いは変えず、レジストリ方式にはしない。
      実施記録（2026-10-27）: (1) `stage_two` の 6 ファイル（diagnose・local_llm・metadata・metrics・search・stock）を `REF:src/core/features/` へ移し、`stage_two` モジュールを無くした。(2) `src/ui/workspaces/stage_two.rs` を画面単位のファイルへ分割した（タブ枠と共有状態は `REF:src/ui/workspaces/tabs.rs`、読者は `reader.rs`、統計は `stats.rs`、正規表現検索は `regex_search.rs`、宛先是 `addresses.rs`、ローカル LLM は `local_llm.rs`、ストック提案は `proposals.rs`）。(3) `REF:src/ui/app.rs`（8823 行）から独立した塊を切り出し、`NovnowApp` 本体を残した：起動前の画面とデスクトップの Host は `REF:src/ui/picker.rs`、パレットの候補生成は `REF:src/ui/palette_items.rs`、ステータスバーと省略判定とミニヘッダーは `REF:src/ui/status.rs`（6611 行へ）。(4) 設計書 overview §3.3 と `REF:docs/developers.md` のソースツリー、文書中の `REF:` パス（NativePicker・ステータスバー・palette_items 等）を追随させた。全テストは無変更で通り（ゴールデン差分ゼロ）、`stage_two` の名前は `src/` から消えた（`tests/stage_two.rs` のファイル名は歴史の記録として残す）。
      受け入れ条件: 既存テストが無変更で全て通り（ゴールデン差分ゼロ）、`stage_two` の名前が `src/` から消えていること。公開 API（§19）の形と利用者から見える振る舞いを変えないこと。
    - <a id="issue-29-x4"></a>**X4 文書の文体見直し（実装完了）**（N29-32）: 利用者向け文書と README から行う。仕様書・設計書は用語と意味を変えない範囲に限る。
      実施記録（2026-10-27）: 利用者向け文書 10 本と README（日・英）の文体を見直した。誇張・不自然な表現（「次世代的な」タイトル、「English version is HERE」、「研究開発の末、実装する予定です」など）を事実のみの文章へ置き換え、利用者向け文書の用語・手順・キー・リンクは変えていない。README の事実の誤記 3 件を直した（診断 76 種→77 種、英語版の正規操作 119→120、ファイル拡張 `.txt`→`.cnov`。いずれも実装・ゴールデンとの不一致だった）。メディア将来記述は N29-36 の決定（実体対応を入れない・拡張点の明記まで）に合わせ「拡張点は形式の中に用意されている」へ。「v0→v1 移行機能は提供しない」から開発者向け理由（実装コスト）を削除した。`docs/users/errors.md` の `E025` 節を B10 後の実装（JSON/TOML 共通の「読み込めません」）に合わせて書き直し、JSON 特有の直し方（`,` と `"` の数）を削除した。`REF:docs/designs/platform.md` §14.1 の X1 注記の stale な「未実装・実装計画済み」を「実装完了」へ直した（X1 は PR #38 で実装済み）。生成物一式は `REF:scripts/docs-site.py` で再生成し同コミットに含めた。用語・意味の不変を機械で見た範囲: `REF:docs/script/easy-read.py`（利用者向け 10 本の規則）・`REF:docs/script/reference.py`（REF とサイト検査）・サイト生成物のリンク・アンカー検査（`npm run check:site`）は全て通過。
    - <a id="issue-29-x5"></a>**X5 両市場への初回公開**（N29-26）: 所有者が Azure DevOps の登録と発行者トークンの用意を終えた後、B9 で用意したジョブで VS Code Marketplace と Open VSX へ初めて公開する。
12. **作らないと決めたもの**: 本体への git ラッパ（N29-40）、WASM 経由の TypeScript バインディング（N29-43。将来の正本は §19 の操作カタログ）、メディアの実体対応（N29-36。拡張点の明記まで）。
13. <a id="issue-29-b11"></a>**B11 執筆支援ラウンド 2（W097 / W098）**（N29-45）:
   反復系診断を 2 件追加する。W097 は同一節内で同じ文末尾（句点 `。！？` 直前の 2 字）で 3 文連続した場合に 3 文目の位置へ出す（対象は地の文。`「…」` の内側は数えない。改行・段落は連続を切らない。節をまたがない）。W098 は W087 と同じ語の抽出（辞書登録語・漢字 2 字以上連続・カタカナ 3 字以上連続）を集計単位を場面にして適用し、5 回以上出現で最後の出現位置へ出す。両方とも新規依存のない決定論ルールで、組み込み既定は無効（`[features.diagnostics]` で `W097 = "warn"` 等に設定して有効化）。日本語主体の判定は §7.2.2（W096）と同一。
   受け入れ条件: サンプル小説（`examples/sample-novel`）と `examples/manual-check` で既定でも有効化しても発火しないこと（閾値未満の反復が無いことを固定する）。W097 は同じ文末尾 3 連続で発火し 2 連続では発火せず、`「…」` の内側と句点直前 1 字の文（連続を切る）をテストに固定する。W098 は場面内で 5 回で発火し 4 回では発火せず、場面をまたぐ 5 回では集計されないことをテストに固定する。`[features.diagnostics]` で `"off"` のコードは件数にも含まれない（B10 の feature 解決の回帰）。
   実施プラン: 診断 enum（`REF:src/core/diagnostics.rs`）へ W097 / W098 を追加（severity W・既定オフ）。W097 は §9.4.3 と同一の文区切りで読み順に文末尾キーを収集し、3 連続を検出する。W098 は `REF:src/core/features/diagnose.rs` で W087 の語抽出を流用し場面ごとに集計する。クイックフィックスは W097 が連続 3 文を表示（自動置換しない）、W098 が用語辞書で類語を見る（W087 同一）。実装と同時に: 仕様 §7.2 の表行 2 件・ja/en カタログ（`REF:website/src/data/diagnostics.ts`・`diagnostics-en.json`・`diagnostics-examples.json`）・サイト解説（§7.2.1）・「77 種」の数値主張（`REF:docs/specifications/overview.md` §2・`REF:README.md`・`REF:README.en.md`・`REF:docs/users/glossary.md`）を 79 種へ更新する。設計書 `REF:docs/designs/diagnostics.md` §7.2 の状態を「実装完了」へ戻す（全 142 節の完了）。

### 8.6 未決リスト（仕様更新の前に所有者の判断が要るもの）

- M-1〜M-6 と、第 1〜20 回の判断により、§8.2 の推奨案はすべて確定した。未決事項は無い。
- 仕様書・設計書への反映（§8.4）と実装計画の確定（§8.5）は完了した。次段は §8.5 の各バッチの実装であり、本節と各設計書の「Issue #29 / B*」設計メモが入力になる。
- 実装と同時に仕様文へ取り込む後追い 3 か所（`section.split` の §19.7 の表行、W096 の §7.2 の表行とサイト解説、§14.2.2 / §14.2.3 の出力例）は、各バッチで解消済みである。
- 第 2 ラウンドの後追い: W097 / W098 の §7.2 の表行 2 件、ja/en カタログ、サイト解説（§7.2.1）、そして「77 種」の数値主張（overview §2・README 日英・glossary）は、B11 で同時に解消する（W096 を B7 が取り込んだのと同じパターン）。
