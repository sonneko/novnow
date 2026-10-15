# 開発計画（Plans）

本書は現在の開発状況サマリ、新規に追加された仕様変更・小規模機能追加（全 62 件：5 件実装完了・57 件未実装・実装計画済み）の実装計画、比較的大規模な機能追加（1 件：未実装・未計画）、および段階 ②（追加仕様 27 項目）の計画を記述する。完了済みの段階 ① マイルストーン（M0〜M11）の履歴は `REF:docs/history.md` を参照すること。

---

## 1. 全体進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 90 節 / 実装中 0 節 / 未実装・実装計画済み 33 節 / 未実装・未計画 1 節 / 実装待ち 2 節 | 全 10 設計書（`docs/designs/*.md`）の `### ` 節（合計 126 節） | `REF:docs/plans.md#planned-fixes-and-features` |
| 仕様変更・修正（F-01〜F-44） | 一部実装完了（F-10, F-23, F-24, F-42） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 一部実装完了（S-09） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 未実装・未計画 | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 未着手（計画待機） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
<!-- SYNC:END stage1-status -->

---

<a id="planned-fixes-and-features"></a>
## 2. 仕様変更・修正および小規模機能追加の実装計画（状態: 一部実装完了）

以下の 62 項目（修正 44 件 `F-01`〜`F-44`、小規模機能追加 18 件 `S-01`〜`S-18`）はすべて仕様書（`docs/specifications/*.md`）および設計書（`docs/designs/*.md`）への反映を完了している。現時点での実装状態は `F-10` / `F-23` / `F-24` / `F-42` / `S-09` が **「実装完了」**、残りが **「未実装・実装計画済み」** である。実装時は依存関係と変更対象モジュールの近接性に基づき、以下の **6 フェーズ（P1〜P6）** の順序で段階的に実装・検証する。

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
### 2.2 修正項目一覧（F-01〜F-44：すべて未実装・実装計画済み）

<a id="plan-F-01"></a>
- **F-01（未実装・実装計画済み）**: テーマ記号 `◐` / `☾` / `☀` のベジェ描画化
  - 仕様・設計: `REF:docs/designs/ui.md`（§11.4）
  - 変更対象: `REF:src/ui/theme.rs`, `REF:src/ui/app.rs`
  - 計画内容: フォントグリフに存在しない `☾`（U+263E）等の文字描画をやめ、`egui::Painter` のベジェ曲線・円弧で半円（system）・三日月（dark）・太陽（light）を描画する `paint_theme_icon` を実装する。

<a id="plan-F-02"></a>
- **F-02（未実装・実装計画済み）**: 節エディタの枠線外へのテキストはみ出し修正
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.4）
  - 変更対象: `REF:src/ui/widgets/section_editor.rs`
  - 計画内容: 節エディタ枠の `inner_margin` と内部 `ScrollArea` / `TextEdit` のクリップ領域を一致させ、文字や下線が枠外へ出ないよう調整する。

<a id="plan-F-03"></a>
- **F-03（未実装・実装計画済み）**: カーソル寸法と行番号・文字の垂直位置合わせ
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.4）
  - 変更対象: `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/syntax_text.rs`
  - 計画内容: `Galley` の各行 `rect` から行番号のベースライン Y 座標を算出し、キャレット高さをフォントサイズに比例した自然な高さへ揃える。

<a id="plan-F-04"></a>
- **F-04（未実装・実装計画済み）**: 参照ペインで 4 個以上の同じ診断をアコーディオン集約
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.5）
  - 変更対象: `REF:src/ui/widgets/reference_pane.rs`, `REF:src/core/features/reference.rs`
  - 計画内容: 同一 `DiagnosticCode` の診断が 4 件以上ある場合は 1 行の折りたたみヘッダに集約し、クリックまたは `Enter` で展開・折りたたみできるようにする。

<a id="plan-F-05"></a>
- **F-05（未実装・実装計画済み）**: `Cmd` / `Ctrl` 押下中のステータスバーへのショートカット補完表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.6, §9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/app.rs`
  - 計画内容: `ctx.input(|i| i.modifiers)` を監視し、`Cmd` 単体や `Cmd+Shift` 等の押下中に、そのプレフィックスから始まるショートカット候補（`Cmd+Shift+X` 等）をステータスバーへリアルタイム表示する。

<a id="plan-F-06"></a>
- **F-06（未実装・実装計画済み）**: コマンドパレットの位置を画面水平中央・最上部に調整
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/palette.rs`
  - 計画内容: パレットの表示位置を VS Code と同じく X 座標中央・上端（ヘッダ直下）に固定する。

<a id="plan-F-07"></a>
- **F-07（未実装・実装計画済み）**: パレットの選択肢に「パレットを閉じる」を追加
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: パレット候補に常設の「パレットを閉じる（`Esc`）」項目を追加し、選択時にパレットを閉じる。

<a id="plan-F-08"></a>
- **F-08（未実装・実装計画済み）**: パレットが節エディタ枠の下に隠れる z-index の修正
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/widgets/section_editor.rs`
  - 計画内容: パレットの `egui::Area` を `Order::Foreground` で最前面に描画し、節エディタの枠線がパレットの上に重ならないようにする。

<a id="plan-F-09"></a>
- **F-09（未実装・実装計画済み）**: パレットの `>` でエージェント関連設定をサブ階層化
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`
  - 計画内容: `>` 直後の初期候補ではエージェント設定を `>agent ` の 1 項目にまとめ、`>agent` 入力または選択後に役割別・操作別の候補を展開する。

<a id="plan-F-10"></a>
- **F-10（実装完了: `REF:src/core/features/sections.rs`, `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/workbench.rs`）**: 本文エディタへのペースト時に改行保持＋`;` で自動節分割
  - 仕様・設計: `REF:docs/designs/editing.md`（§8.1）
  - 変更対象: `REF:src/core/features/sections.rs`, `REF:src/ui/workbench.rs`, `REF:src/ui/widgets/section_editor.rs`
  - 計画内容: ペーストされた文字列の改行を維持し、単独 `;` 行または `@A-B-C` 見出し行が含まれる場合は一気に複数の節へ分割して文書へ反映する。

<a id="plan-F-11"></a>
- **F-11（未実装・実装計画済み）**: 「？」「！」「＊」の太さを他文字と統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§11.5）
  - 変更対象: `REF:src/ui/theme.rs`
  - 計画内容: 全角約物（`？` `！` `＊`）が別フォントへフォールバックせず本文 CJK フォントで描画されるようフォントファミリ優先順を調整する。

<a id="plan-F-12"></a>
- **F-12（未実装・実装計画済み）**: パレットの横幅固定と長い候補テキストの `...` 省略
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: パレットの幅を固定し、長い候補ラベルは `...` で省略してパレットの位置ズレを防ぐ。

<a id="plan-F-13"></a>
- **F-13（未実装・実装計画済み）**: 未入力のパレット下部に接頭辞ルールのガイドを表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: 入力が空のときパレット下部に「`>` コマンド / `@` 節 / `'` 辞書 / `M`（`#`）資料 / `?` ショートカット」のガイドを表示する。

<a id="plan-F-14"></a>
- **F-14（未実装・実装計画済み）**: パレットで候補が 0 件のときに「ない」旨を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`
  - 計画内容: 資料が 0 件のときに `M` / `#` を打った場合を含め、候補が 0 件のときはモードに応じた「一致する項目がありません（登録されている資料がありません）」を表示する。

<a id="plan-F-15"></a>
- **F-15（未実装・実装計画済み）**: すべてのショートカットキー操作を `>` パレット経由でも実行可能にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7, §9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: 全 `Command` を `>` コマンドパレットに公開し、パレットから全ショートカット機能を実行できるようにする。

<a id="plan-F-16"></a>
- **F-16（未実装・実装計画済み）**: ショートカット専用機能の視覚ボタン化（ガイドラベルのボタン化）
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1, §10.0〜§10.9）
  - 変更対象: `REF:src/ui/workspaces/mod.rs`, `REF:src/ui/workspaces/material.rs`, `REF:src/ui/workspaces/idea.rs`, `REF:src/ui/workspaces/plot_episode.rs`, `REF:src/ui/workspaces/plot_outline.rs`, `REF:src/ui/workspaces/cast.rs`, `REF:src/ui/workspaces/agent.rs`, `REF:src/ui/workspaces/review.rs`
  - 計画内容: 「`Ctrl+Shift+N` で新しい資料を作れます」等のガイドラベルをショートカット表記付きの押せるボタンに置き換える。

<a id="plan-F-17"></a>
- **F-17（未実装・実装計画済み）**: ブラウザ・OS に奪われるショートカットの非衝突化調整
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/web.rs`
  - 計画内容: ブラウザ予約キーと被らない代替キー割当および `preventDefault` 調整を行う。

<a id="plan-F-18"></a>
- **F-18（未実装・実装計画済み）**: 同梱フォント使用通知を左下のモードレストースト（数秒で消去）に変更
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.6, §11.5）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: フォントフォールバック通知をステータスバー常駐から外し、起動後 4 秒間だけ左下に表示されるモードレス通知にする。

<a id="plan-F-19"></a>
- **F-19（未実装・実装計画済み）**: コマンドパレットから `agents/mcp.json` 編集モーダルへ移行
  - 仕様・設計: `REF:docs/designs/syntax.md`（§5.11）, `REF:docs/designs/agent.md`（§15.7）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/palette.rs`, `REF:src/ui/workspaces/agent.rs`
  - 計画内容: パレット（`>mcp` / `>agent mcp`）および `agent` 画面から `agents/mcp.json` を編集・検証・保存できるモーダルを追加する。

<a id="plan-F-20"></a>
- **F-20（未実装・実装計画済み）**: エラー発生時に `review` ワークスペースアイコンへ視覚変化を付与
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1, §10.9）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: エラーまたは警告が存在するとき、ヘッダの `review` アイコンに色変化とバッジを描画する。

<a id="plan-F-21"></a>
- **F-21（未実装・実装計画済み）**: 文脈ビューの節の境目に横線を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 文脈ビューの各節の間に区切り横線を描画する。

<a id="plan-F-22"></a>
- **F-22（未実装・実装計画済み）**: 文脈ビューの節番号表示を `A-B-C-D`（4 階層完全形）に統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 章省略の `B-C-D` 表記をやめ、`@AA-BB-CC-DD` に統一する。

<a id="plan-F-23"></a>
- **F-23（実装完了: `REF:src/ui/widgets/reference_pane.rs`, `REF:src/ui/app.rs`）**: 参照ペインの診断から形態素単位で辞書・キャラ一覧へ追加する自動回復機能
  - 仕様・設計: `REF:docs/designs/semantics.md`（§6.2）, `REF:docs/designs/ui.md`（§9.5）
  - 変更対象: `REF:src/core/resolve/dictionary.rs`, `REF:src/core/features/code_action.rs`, `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 未解決参照診断に対して形態素解析で 1 形態素を切り出し、参照ペイン上のボタンから `words.md` または `characters.md` へ即時登録できるようにする。

<a id="plan-F-24"></a>
- **F-24（実装完了: `REF:src/core/resolve/dictionary.rs`, `REF:src/ui/palette.rs`）**: `characters.md` の名前を透過的に辞書へ自動登録し重複を解消
  - 仕様・設計: `REF:docs/designs/semantics.md`（§6.1）
  - 変更対象: `REF:src/core/resolve/dictionary.rs`, `REF:src/core/features/reference.rs`, `REF:src/ui/palette.rs`
  - 計画内容: `words.md` を書き換えずに `characters.md` の内容を透過的に用語辞書としても扱い、重複表示を抑止する。

<a id="plan-F-25"></a>
- **F-25（未実装・実装計画済み）**: パレットの `?` でショートカットを選択・実行可能にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: `?` モードをショートカットの検索＋選択実行モードとして実装する。

<a id="plan-F-26"></a>
- **F-26（未実装・実装計画済み）**: パレットの `@` 移動を広さ優先の階層ドリルダウン（`@01-* 章タイトル` 等）に変更
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: `@` 入力時に `@01-*`, `@02-*`（章タイトル付き）から始め、話 → 場面 → 節へと広さ優先で段階的に絞り込む。

<a id="plan-F-27"></a>
- **F-27（未実装・実装計画済み）**: パレット外クリック時に背面要素を押下判定にせずパレットのみ閉じる
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: パレット背面にフルスクリーンのクリック吸収シールド（`Sense::click()`）を配置し、外側クリックでパレットのみを閉じる。

<a id="plan-F-28"></a>
- **F-28（未実装・実装計画済み）**: 参照ペインの各セクション境界に薄めの横線を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.5）
  - 変更対象: `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 上位プロット・配役・用語・資料・診断の各セクション間に 1px の `border` 横線を描画する。

<a id="plan-F-29"></a>
- **F-29（未実装・実装計画済み）**: 押せないラベルのテキスト選択（ダブルクリック青選択）を無効化
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: `style.interaction.selectable_labels = false` を設定し、押せないラベルが選択ハイライトされないようにする。

<a id="plan-F-30"></a>
- **F-30（未実装・実装計画済み）**: 押せる要素すべてに薄い下線＋ホバー変化を付与しテストで保証
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1, §9.9）
  - 変更対象: `REF:src/ui/widgets/mod.rs`, `REF:tests/gui.rs`
  - 計画内容: 全クリック対象に薄い下線とホバー変化を付与し、`REF:tests/gui.rs` で自動検査する。

<a id="plan-F-31"></a>
- **F-31（未実装・実装計画済み）**: 資料ワークスペースの `Ctrl+Shift+N` をペインフォーカス非依存にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.5）, `REF:docs/designs/workflows.md`（§16.1）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/shell.rs`
  - 計画内容: 左一覧以外にフォーカスがあっても `Ctrl+Shift+N`（および新規ボタン）で新しい資料を作成できるようにする。

<a id="plan-F-32"></a>
- **F-32（未実装・実装計画済み）**: 全クリック対象に hover 背景（`bg.selection` 60%）＋ポインタ変更＋24×24px 以上の `interact` 矩形を付与
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.9）
  - 変更対象: `REF:src/ui/widgets/mod.rs`, `REF:src/ui/app.rs`, `REF:src/ui/workspaces/mod.rs`, `REF:tests/gui.rs`
  - 計画内容: `allocate_exact_size` の視覚寸法は変えずに `ui.interact` の当たり判定矩形を最低 24×24px に広げ、hover 時の `bg.selection`（60%）塗りと `CursorIcon::PointingHand` を全クリック対象へ適用する。

<a id="plan-F-33"></a>
- **F-33（未実装・実装計画済み）**: Web 版で同じタイトルのプロジェクトを作成・改名できないようにする
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.4）
  - 変更対象: `REF:src/ui/web.rs`, `REF:src/adapters/storage_idb.rs`
  - 計画内容: 既存プロジェクトとタイトルが重複する場合は新規作成・改名を拒否してエラーを表示する。

<a id="plan-F-34"></a>
- **F-34（未実装・実装計画済み）**: 参照ペインの要約が途中で切れるときに「`続き...`」の 1 行を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.5）, `REF:docs/designs/workflows.md`（§16.5）
  - 変更対象: `REF:src/core/features/reference.rs`, `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 行数・文字数上限で切り詰めた要約の末尾にクリック可能な「`続き...`」を表示する。

<a id="plan-F-35"></a>
- **F-35（未実装・実装計画済み）**: 全画面で「一覧移動は `↑`/`↓`、編集確定して移動は `Alt+↑`/`Alt+↓`」に統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/workspaces/material.rs`, `REF:src/ui/workspaces/idea.rs`, `REF:src/ui/workspaces/cast.rs`, `REF:src/ui/workspaces/plot_episode.rs`, `REF:src/ui/workspaces/plot_outline.rs`, `REF:src/ui/workspaces/agent.rs`, `REF:src/ui/workspaces/review.rs`
  - 計画内容: エディタにフォーカスがないときは素の `↑`/`↓` をどの画面でも一覧・タイムライン移動に効かせる。

<a id="plan-F-36"></a>
- **F-36（未実装・実装計画済み）**: 資料ワークスペースのフォーカス移動を `Tab` に移し `Ctrl+3` を参照ペイン移動に統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.8, §10.5）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/shell.rs`
  - 計画内容: `material` 内の `Ctrl+3` 独自バインドを撤去して `Tab` に移し、`Ctrl+3` の意味を共通シェルと統一する。

<a id="plan-F-37"></a>
- **F-37（未実装・実装計画済み）**: `TextEdit` に `lock_focus(true)` を付け、入力欄外では `Tab` でヘッダボタン等を巡回可能にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.4, §9.8）
  - 変更対象: `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/slice_editor.rs`, `REF:src/ui/app.rs`
  - 計画内容: 節エディタを 1 つのフォーカス対象にし、入力欄外での `Tab` フォーカス移動と明瞭なフォーカス枠表示を実装する。

<a id="plan-F-38"></a>
- **F-38（未実装・実装計画済み）**: アイデア 1,000 件の一覧で可視行のみを描画し 60fps を維持
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.4）, `REF:docs/designs/workflows.md`（§18.1）
  - 変更対象: `REF:src/ui/workspaces/idea.rs`
  - 計画内容: `ScrollArea::show_rows` による仮想スクロールを導入する。

<a id="plan-F-39"></a>
- **F-39（未実装・実装計画済み）**: 文脈ビューの節未保存状態を示すオレンジの点を拡大
  - 仕様・設計: `REF:docs/designs/editing.md`（§8.4）, `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`, `REF:src/ui/app.rs`
  - 計画内容: 未保存ドットの直径を 8〜10px 相当に大きくする。

<a id="plan-F-40"></a>
- **F-40（未実装・実装計画済み）**: デスクトップの `Ctrl+O` を「保存してから選択画面（履歴）へ戻る」に統一し 2 段構成にする
  - 仕様・設計: `REF:docs/designs/platform.md`（§14.4）, `REF:docs/designs/ui.md`（§9.8）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: `Ctrl+O` でいきなりフォルダダイアログを開かず、Web 版と同じくプロジェクト選択画面（履歴一覧）へ戻り、そこから「フォルダを開く」で選ぶ 2 段構成に統一する。

<a id="plan-F-41"></a>
- **F-41（未実装・実装計画済み）**: ペインフォーカスを左端 3px 縦線、編集中の節を 1px 枠に分離し、文脈ビューのプレースホルダと対で見せる
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3, §9.4）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 節エディタの全周太青枠をやめ、ペインフォーカスは左端 3px 縦線、編集中節は常時 1px 枠、文脈ビューのプレースホルダにも左端 3px 縦線を付けて対にする。

<a id="plan-F-42"></a>
- **F-42（実装完了: `REF:src/ui/i18n/ja.toml`, `REF:tests/i18n_catalog.rs`）**: `{0}はこの版ではまだ使えません` の空白修正と `ja.toml` 助詞直前空白のテスト検査
  - 仕様・設計: `REF:docs/designs/ui.md`（§12.3）
  - 変更対象: `REF:src/ui/i18n/ja.toml`, `REF:tests/i18n_catalog.rs`, `REF:tests/workspaces.rs`
  - 計画内容: `ja.toml` のテンプレート直後が助詞の箇所の半角空白を削除し、`REF:tests/i18n_catalog.rs` で機械検査する。

<a id="plan-F-43"></a>
- **F-43（未実装・実装計画済み）**: 打鍵時は編集中の節だけ同期解析し、10 万字ファイルで 1 打鍵 16ms 以内を保証・テスト計測
  - 仕様・設計: `REF:docs/designs/semantics.md`（§6.7）, `REF:docs/designs/workflows.md`（§18.1, §18.4）
  - 変更対象: `REF:src/ui/workbench.rs`, `REF:src/core/features/project.rs`, `REF:tests/incremental.rs`, `REF:benches/core.rs`
  - 計画内容: 打鍵時の同期解析を編集中の 1 節に限定して他節をデバウンスし、「10 万字のファイルで 1 打鍵 16ms 以内」を計測するテストを追加する。

<a id="plan-F-44"></a>
- **F-44（未実装・実装計画済み）**: 資料画面のカテゴリチップ（「この資料のカテゴリ」直下＋選択枠）と条件欄プレースホルダ化・`fg.faint` 統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.5）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`
  - 計画内容: 8 個のカテゴリチップを「この資料のカテゴリ」見出し直下に置いて選択中を枠線で示し、条件の入力例は入力欄のプレースホルダに移し、説明文の色を `fg.faint` に統一する。

---

<a id="planned-small-features"></a>
### 2.3 機能追加（小規模：S-01〜S-18：すべて未実装・実装計画済み）

<a id="plan-S-01"></a>
- **S-01（未実装・実装計画済み）**: LLM エージェントに「例」として指定した話の本文またはプロットをコンテキスト付与
  - 仕様・設計: `REF:docs/designs/agent.md`（§15.4, §15.7）, `REF:docs/designs/syntax.md`（§5.9, §5.10）
  - 変更対象: `REF:src/core/model/task.rs`, `REF:src/core/agents/files.rs`, `REF:src/core/aicontext/packet.rs`, `REF:src/core/syntax/prompt_md.rs`, `REF:src/ui/workspaces/agent.rs`
  - 計画内容: タスクに参考例範囲 `examples` を持たせ、指定した話・節の本文またはプロットをコンテキストパケットおよび `{{examples}}` 変数として渡す。

<a id="plan-S-02"></a>
- **S-02（未実装・実装計画済み）**: 初回起動時（履歴が空のとき）のアプリ説明・ドキュメントサイト案内とパレットからの再呼び出し
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.4, §14.4）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/web.rs`, `REF:src/ui/palette.rs`
  - 計画内容: 履歴が空の初回起動時にウェルカム案内を表示し、コマンドパレットからも常時呼び出し可能にする。

<a id="plan-S-03"></a>
- **S-03（未実装・実装計画済み）**: ブラウザ版の PWA 化・オフライン動作対応と `README.md` への追記
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.3）
  - 変更対象: `REF:index.html`, `REF:Trunk.toml`, `REF:README.md`
  - 計画内容: Web App Manifest と Service Worker を追加してオフライン動作可能にし、`REF:README.md` に PWA 化可能であることを記載する。

<a id="plan-S-04"></a>
- **S-04（未実装・実装計画済み）**: 本文が 0 文字のときに文脈ビューへ 3 行ガイドを薄く表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 本文が空のとき `Alt+数字=画面 / Alt+矢印=節移動 / Ctrl+P=一覧` の 3 行ガイドを `fg.faint` で描画する。

<a id="plan-S-05"></a>
- **S-05（未実装・実装計画済み）**: ヘッダ右に `?` と `Ctrl+P` のボタンを常設
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: 上部ヘッダ右端に `?` ボタンと `Ctrl+P` ボタンを常設する。

<a id="plan-S-06"></a>
- **S-06（未実装・実装計画済み）**: ワークスペース切替時にパンくずの右へ画面名を 1 秒だけ表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.0）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: 画面切替から 1 秒間、パンくずの右にワークスペース名を表示する。

<a id="plan-S-07"></a>
- **S-07（未実装・実装計画済み）**: 資料ビュワーの URL を外部リンクアイコン＋薄黒下線＋ホバー青色で開けるようにする
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.5）, `REF:docs/designs/workflows.md`（§16.5）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 資料内の URL に左下向き（外部リンク）矢印アイコン・薄い下線・ホバー時青色を付け、クリックで開けるようにする。

<a id="plan-S-08"></a>
- **S-08（未実装・実装計画済み）**: 閉じ鉤括弧の後の `@` でキャラ名を予測補完
  - 仕様・設計: `REF:docs/designs/syntax.md`（§5.2）, `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/core/features/completion.rs`, `REF:src/ui/assist.rs`
  - 計画内容: `」@` / `』@` 入力時にキャラ名およびモードの補完ポップアップを出す。

<a id="plan-S-09"></a>
- **S-09（実装完了: `REF:src/core/resolve/dictionary.rs`）**: 未知の `'` 用語を形態素解析で区切り赤波線の範囲を決定
  - 仕様・設計: `REF:docs/designs/semantics.md`（§6.2）
  - 変更対象: `REF:src/core/resolve/dictionary.rs`, `REF:tests/dictionary.rs`
  - 計画内容: 未知の `'` 参照の赤波線範囲を形態素解析（文字種境界・助詞切り出し）による 1 形態素に区切る。

<a id="plan-S-10"></a>
- **S-10（未実装・実装計画済み）**: `novnow import <zip> <dir>` の追加とデスクトップ版の Zip 導入対応
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.5, §14.1）
  - 変更対象: `REF:src/cli/mod.rs`, `REF:src/adapters/zip.rs`, `REF:src/ui/app.rs`, `REF:tests/cli.rs`
  - 計画内容: CLI `novnow import <zip> <dir>` とデスクトップ GUI での Zip インポートを実装する。

<a id="plan-S-11"></a>
- **S-11（未実装・実装計画済み）**: デスクトップのプロジェクト選択画面に日付を表示
  - 仕様・設計: `REF:docs/designs/platform.md`（§14.4）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: `NativePicker` の各履歴行に `opened_at` の日付を表示する。

<a id="plan-S-12"></a>
- **S-12（未実装・実装計画済み）**: 文脈ビューを可視範囲だけレイアウトする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）, `REF:docs/designs/workflows.md`（§18.1）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: ビューポート外の節は高さ推定のみでスキップし、可視範囲の節のみレイアウトする。

<a id="plan-S-13"></a>
- **S-13（未実装・実装計画済み）**: パレットで個別単位（例: `">ag"` => `">agent "`）の `Tab` 補完
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: パレットでの `Tab` キー押下時に次のトークン単位まで補完する。

<a id="plan-S-14"></a>
- **S-14（未実装・実装計画済み）**: ステータスバーの幅に応じた省略優先順（保存状態最高優先、`…`＋ホバー詳細）
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.6）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: 幅不足時に「セッション情報 → 原稿用紙枚数 → 文字数」の順で `…` に畳み、ホバーで詳細を出し、保存状態マークを最高優先で残す。

<a id="plan-S-15"></a>
- **S-15（未実装・実装計画済み）**: Web サイトへの全エラー解説の記載と CI での網羅性チェック
  - 仕様・設計: `REF:docs/designs/diagnostics.md`（§7.2）, `REF:docs/designs/workflows.md`（§18.4）
  - 変更対象: `REF:docs/specifications/diagnostics.md`, `REF:tests/diagnostics_catalog.rs`, `REF:.github/workflows/ci.yml`
  - 計画内容: 定義されている全 76 診断コードの解説を整備し、全エラーの解説が存在するかを CI で機械検査する。

<a id="plan-S-16"></a>
- **S-16（未実装・実装計画済み）**: Web⇔デスクトップ双方向移行のヘルプ（Zip 手順）を追加
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.5）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`
  - 計画内容: Zip を使った Web→デスクトップおよびデスクトップ→Web 移行手順のヘルプ画面を追加する。

<a id="plan-S-17"></a>
- **S-17（未実装・実装計画済み）**: デスクトップ選択画面に「データは消さず履歴のみを消す」ことを明示した履歴削除機能を追加
  - 仕様・設計: `REF:docs/designs/platform.md`（§14.4）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/core/policy/recent.rs`
  - 計画内容: デスクトップの履歴一覧から、実データを消さずに履歴エントリのみを削除する操作と明示説明を追加する。

<a id="plan-S-18"></a>
- **S-18（未実装・実装計画済み）**: 本文の最大幅を `.novnow/ui.json` の設定にし、`ja` は 44 字、`en` は 70 字前後を既定にする
  - 仕様・設計: `REF:docs/designs/project.md`（§4.4）, `REF:docs/designs/ui.md`（§11.5）
  - 変更対象: `REF:src/ui/settings.rs`, `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/context_view.rs`
  - 計画内容: `ui.json` に `max_line_chars` を追加し、言語別既定値（`ja`=44, `en`=70）で折り返し幅を制御する。

---

<a id="unplanned-large"></a>
## 3. 機能追加（比較的大規模：未実装・未計画）

- **L-01（未実装・未計画）**: 既存の本文テキストから `novnow` に移行するための手段の提供
  - 対応する仕様・設計: `REF:docs/designs/platform.md`（§14.6）
  - 状態: **未実装・未計画**（ユーザー指示により、現時点では実装計画を立てず「未実装・未計画」として記録する）。

---

<a id="stage-2"></a>
## 4. 段階 ② 実装計画（追加仕様 1〜27）

段階 ① で整備した 10 個の拡張点（縫い代、`REF:docs/designs/overview.md` §3.3）を土台として、第 2 次指示の 26 件と段階 ② へ移管した読者モード（#27）の計 **27 項目**を段階 ② で実装する。

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
| 27 | 読者モード（縦書きプレビュー） | `REF:docs/designs/ui.md`（§10.7）, `REF:docs/designs/workflows.md`（§17.3, §18.1）, `REF:docs/designs/project.md`（§4.4.3, §4.5.1） | `Alt+7`（`reader` ワークスペース）、`core/reader.rs`（純粋組版関数 `layout`）と `ui/workspaces/reader.rs`（段階 ② で新設予定）、`.novnow/reader.json`、`project.json.reader`、`reader.exit` 文言、1 ページ 3ms 以内目標 | §4.5 `19.14-27` |
| U-7 | 既定プロンプトの不変保管と復元 | `REF:docs/designs/agent.md`（§15.5.4） | **段階 ①（M10）で実装完了**（`REF:src/core/agents/prompts.rs`、`>prompt reset` / `>prompt diff`） | §4.5 `19.14-U7` |
| U-8 | エージェントの並列実行 | `REF:docs/designs/agent.md`（§15.7.5） | **段階 ①（M10）で実装完了**（`REF:src/core/agents/runner.rs`、`REF:src/ui/agents.rs`、`Shift+R`） | §4.5 `19.14-U8` |

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
| 全体検索（`Ctrl+Shift+F`） | **通常検索は ①、正規表現は ②** | リテラル全体検索は段階 ① で実装済み（`REF:src/ui/assist.rs`）。§9.7.4 の正規表現モード（`Alt+R`）と 1 秒打ち切り（`W094`）が段階 ②（#19）。 |
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
| 1 | 未知キーの温存 | `project.json`（`REF:src/core/model/project.rs`）・`ui.json`（`REF:src/ui/settings.rs`）・`index.json`（`REF:src/core/model/material.rs`）を `#[serde(flatten)]` で受け、`REF:tests/roundtrip.rs` で検証 | 設定ファイルの前後互換が切れる |
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
- [ ] `Alt+1..9` の切替が 16ms 以内に完了し、ローディング表示・アニメーションが出ない。
- [ ] `Ctrl+Shift+1..9` が §10.0 の対応表どおりの節に着地する。
- [ ] `Ctrl+\` で参照ペインが即座に消え、メインカラムが全幅になる。隠している間も内容の計算が続いている。
- [ ] `Alt+0` で直前のワークスペースに戻る。

**19.14-2 セッション情報**
- [ ] ステータスバーに `+n字 m分` が出る。削ったときマイナス表示になる。
- [ ] 30 分の無操作でセッションが切れ、次の打鍵から新しいセッションが始まる。
- [ ] 既定で「毎分の速度」が表示されていない。

**19.14-3 タイムライン**
- [ ] `when: 405 春分 09:15` / `when: +1` / `when: 12 .. 14` / `when: ?` が仕様どおり解釈される。
- [ ] 作中時間が巻き戻る箇所に W080 が出る。`回想` を含む場合は出ない。
- [ ] `when` が 1 つも無いプロジェクトでタイムラインタブが表示されない。
- [ ] タイムラインビューから `when` を編集できない。

**19.14-4 句切れ提案**
- [ ] 既定でオフ。`Ctrl+Alt+,` でオン。
- [ ] 候補位置に縦線だけが出て、文字が挿入されない。
- [ ] `「…」` の内側に候補が出ない。
- [ ] 同じ入力に対して常に同じ候補が出る（決定論）。

**19.14-5 連続台詞**
- [ ] `max_consecutive_speeches` を超えた連続で W082 が出る。
- [ ] 地の文を 1 行挟むとカウントがリセットされる。

**19.14-6 台詞比率**
- [ ] ミニヘッダーのバーが §9.4.2 の算法どおりの値を示す。
- [ ] 目標から外れても色が変わらない。
- [ ] 話プロットの節リストにも同じバーが出る。

**19.14-7 五感**
- [ ] `senses:` を書いた節だけが集計対象になる。
- [ ] 本文を解析して感覚を推定する処理が存在しない（コードレビュー項目）。
- [ ] 文体タブの文言が「指摘」ではなく「観測」の語で書かれている。

**19.14-8 呼称**
- [ ] `address:` の `=>` 右側に `:` を含めても呼称の一部として読まれる。
- [ ] `address` 未定義のキャラでは W084 が出ない。
- [ ] 呼称マトリクスでセルを編集すると `characters.md` に書き戻る。
- [ ] 登場人物 25 人以上で表形式が自動的に 1 行表示に切り替わる。

**19.14-9 折りたたみ**
- [ ] 章・話・場面の単位で畳める。現在の節を含む単位は畳めない。
- [ ] 状態が `.novnow/ui.json` に保存され、再起動後に復元される。
- [ ] 検索ヒットのある単位が自動展開される。
- [ ] エクスポート結果が折りたたみ状態に影響されない。

**19.14-10 資料引用追跡**
- [ ] `^M0137` が本文文字数に数えられない。
- [ ] 未知の ID で E012 が出る。
- [ ] エクスポート結果に `^M` が残らない（`REF:tests/export_invariants.rs`）。
- [ ] どこからも引用されていない資料が I054 で通知される。

**19.14-11 検索結果の資料化**
- [ ] `S####` が `M####` と同じ `next_id` から採番される。
- [ ] 保存後に本文を書き換えても資料の内容が変わらない。
- [ ] `[再検索]` が差分を表示し、自動では更新しない。

**19.14-12 カスタムロール**
- [ ] `agents.custom_roles` に登録したものだけが UI に出る。
- [ ] 委譲レベル `propose` では、カスタムロールの出力が `agents/proposals/` にしか書かれない。
- [ ] `context:` で `project` と `constraints` を外せない。
- [ ] 既定ロール名と同名で E033 が出る。

**19.14-13 ローカル LLM**
- [ ] 設定すると `agents/mcp.json` に `managed_by: "novnow-local-llm"` 付きのサーバが書かれる。
- [ ] 手書きのサーバ定義が書き換えられない。
- [ ] localhost 以外の URL が拒否される。
- [ ] `REF:Cargo.toml` に LLM プロバイダ SDK が無い（`REF:tests/no_llm_sdk.rs`）。

**19.14-14 Web 解説ドキュメント**
- [ ] `GIT_GUIDE_DOC_URL` / `CICD_GUIDE_DOC_URL` がハードコードされず定数経由である。
- [ ] リンクが外部ブラウザで開く。
- [ ] novnow が `.git/` を読まず、`git` コマンドを実行しない（コードレビュー項目）。

**19.14-15 `# ` 行コメント**
- [ ] `.txt` で行頭 `# ` がコメントになり、`.md` では見出しのままである。
- [ ] `#1` のように空白が無い場合はコメントにならない。
- [ ] エクスポート結果に `# ` 行が残らない。

**19.14-16 提案モード**
- [ ] 提案モード中、本文ファイルが 1 バイトも変わらない。
- [ ] `agents/proposals/H####.md` に `before` / `after` が節全文で書かれる。
- [ ] エディタ上に差分が重ねて表示されない。
- [ ] 提案の文字数が統計・進捗に含まれない。

**19.14-17 あらすじ**
- [ ] `blurb` が本文文字数に含まれない。
- [ ] `export.include_blurb = false` のときエクスポートに現れない。
- [ ] 読者モードの章表紙に表示される。
- [ ] `blurb` が無くても警告が出ない。

**19.14-18 文体・品質分析**
- [ ] 総合点・スコア・評価が表示されない。
- [ ] 各指標を個別にオフにできる。
- [ ] 既定で W081/W087/W088/W089/W090 が編集中に出ない。
- [ ] 各指摘から `Enter` で該当箇所へ飛べる。

**19.14-19 正規表現検索**
- [ ] 1 秒のタイムアウトで中断し、部分結果が出る。
- [ ] 構文エラーが 1 行で説明される。
- [ ] 置換がプレビュー必須で、1 つの undo 単位になる。
- [ ] `.novnow/` と `exports/` が検索対象外である。

**19.14-20 統計ダッシュボード**
- [ ] `.novnow/stats.json` に日次集計のみが書かれ、イベントログが作られない。
- [ ] ファイルを削除しても他の機能に影響が無い。
- [ ] 完成予測・達成率・締切が表示されない。
- [ ] `Ctrl+Alt+S` で統計モードに切り替わり、パンくず・診断・保存表示が消えない。

**19.14-21 空間設計メモ**
- [ ] `space-mode: ascii` で等幅・折り返し無しで表示される。
- [ ] 外部ファイル参照がプロジェクト外を指すと E062 になる。
- [ ] 図の内容を解釈する処理が存在しない（コードレビュー項目）。

**19.14-22 00 章ストック**
- [ ] `@00-01-02-03` が正当な SceneId としてパースされる。
- [ ] ツリーでストック章が最後に表示される。
- [ ] 配属操作が原子的に行われ、途中失敗で全巻き戻しされる。
- [ ] 範囲「全体」のエクスポートにストック章が含まれない。

**19.14-23 挿絵メモ**
- [ ] `! ` 行が本文文字数に数えられない。
- [ ] エクスポートで行ごと消える。`--keep-illustration-memo` では `［挿絵: …］` になる。
- [ ] `! ` 行と `illustration:` が §10.9 の一覧に統合され、出典欄で区別される。

**19.14-24 キャラアイコン**
- [ ] `assets/` 外を指すと E062 になる。
- [ ] 本文エディタ内にアイコンが出ない。
- [ ] `icon` を 1 件も定義していないプロジェクトでアイコン用の余白が無い。
- [ ] ファイルが無くてもエラーにならず W070 が出る。

**19.14-25 キャラ別・節長統計**
- [ ] 話者不明の台詞が `（不明）` として計上され、総和が本文の台詞総量と一致する。
- [ ] 節長のヒストグラムと T スコア外れ値が出る。

**19.14-26 分量偏差値**
- [ ] 母集団が 3 未満のとき計算されず `—` と表示される。
- [ ] 標準偏差 0 のとき全単位が T 50 になる。
- [ ] 文字数 0 の単位が母集団から除外される。
- [ ] `⚠` が赤ではなく橙である。

**19.14-27 読者モード（縦書きプレビュー）**
- [ ] 縦書き（右上から左下へ、行は右から左へ）で表示される。
- [ ] 禁則処理・縦中横・ラテン文字 90° 回転・句読点の右上寄せ・モノルビが §17.3.3 のとおり動く。
- [ ] `core::reader::layout()` が純粋関数であり、フォント実測値は引数で注入されている（§3.2 を破っていない）。
- [ ] 読者モードで注釈・診断・ヒントが一切表示されない。
- [ ] どのキー操作でもファイルが変更されない。
- [ ] 1 ページのレイアウト計算が 3ms 以内（§18.1）。
- [ ] `Alt+7` が読者モードに割り当てられる（段階 ① で欠番にしていた枠が埋まる）。

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
