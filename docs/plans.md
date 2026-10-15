# 開発計画（Plans）

本書は現在の開発状況サマリ、新規に追加された仕様変更・小規模機能追加（全 62 件：5 件実装完了・一部実装中・残りは実装計画済み）の実装計画、比較的大規模な機能追加（1 件：未実装・未計画）、および段階 ②（追加仕様 27 項目）の計画を記述する。完了済みの段階 ① マイルストーン（M0〜M11）の履歴は `REF:docs/history.md` を参照すること。

---

## 1. 全体進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 94 節 / 実装中 28 節 / 未実装・実装計画済み 1 節 / 未実装・未計画 1 節 / 実装待ち 2 節 | 全 10 設計書（`docs/designs/*.md`）の `### ` 節（合計 126 節） | `REF:docs/plans.md#planned-fixes-and-features` |
| 仕様変更・修正（F-01〜F-44） | 実装中（F-01, F-10, F-11, F-23, F-24, F-31, F-36, F-38, F-42, F-43 は完了） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装中（S-01, S-09, S-15 は完了） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 未実装・未計画 | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 未着手（計画待機） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
<!-- SYNC:END stage1-status -->

---

<a id="planned-fixes-and-features"></a>
## 2. 仕様変更・修正および小規模機能追加の実装計画（状態: 一部実装完了）

以下の 62 項目（修正 44 件 `F-01`〜`F-44`、小規模機能追加 18 件 `S-01`〜`S-18`）はすべて仕様書（`docs/specifications/*.md`）および設計書（`docs/designs/*.md`）への反映を完了している。計画策定時に `F-10` / `F-23` / `F-24` / `F-42` / `S-09` が **「実装完了」** だった。その後 `F-01` / `F-11` / `F-31` / `F-36` / `F-38` / `F-43` / `S-01` / `S-15` の実装・検証を完了し、残りは **「実装中」または「未実装・実装計画済み」** である。実装時は依存関係と変更対象モジュールの近接性に基づき、以下の **6 フェーズ（P1〜P6）** の順序で段階的に実装・検証する。

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
### 2.2 修正項目一覧（F-01〜F-44：一部実装中）

<a id="plan-F-01"></a>
- **F-01（実装完了）**: テーマ記号 `◐` / `☾` / `☀` のベジェ描画化
  - 仕様・設計: `REF:docs/designs/ui.md`（§11.4）
  - 変更対象: `REF:src/ui/theme.rs`, `REF:src/ui/app.rs`
  - 計画内容: フォントグリフに存在しない `☾`（U+263E）等の文字描画をやめ、`egui::Painter` のベジェ曲線・円弧で半円（system）・三日月（dark）・太陽（light）を描画する `paint_theme_icon` を実装する。

<a id="plan-F-02"></a>
- **F-02（実装中）**: 節エディタの枠線外へのテキストはみ出し修正
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.4）
  - 変更対象: `REF:src/ui/widgets/section_editor.rs`
  - 計画内容: 節エディタ枠の `inner_margin` と内部 `ScrollArea` / `TextEdit` のクリップ領域を一致させ、文字や下線が枠外へ出ないよう調整する。

<a id="plan-F-03"></a>
- **F-03（実装中）**: カーソル寸法と行番号・文字の垂直位置合わせ
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.4）
  - 変更対象: `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/syntax_text.rs`
  - 計画内容: `Galley` の各行 `rect` から行番号のベースライン Y 座標を算出し、キャレット高さをフォントサイズに比例した自然な高さへ揃える。

<a id="plan-F-04"></a>
- **F-04（実装中）**: 参照ペインで 4 個以上の同じ診断をアコーディオン集約
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.5）
  - 変更対象: `REF:src/ui/widgets/reference_pane.rs`, `REF:src/core/features/reference.rs`
  - 計画内容: 同一 `DiagnosticCode` の診断が 4 件以上ある場合は 1 行の折りたたみヘッダに集約し、クリックまたは `Enter` で展開・折りたたみできるようにする。

<a id="plan-F-05"></a>
- **F-05（実装中）**: `Cmd` / `Ctrl` 押下中のステータスバーへのショートカット補完表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.6, §9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/app.rs`
  - 計画内容: `ctx.input(|i| i.modifiers)` を監視し、`Cmd` 単体や `Cmd+Shift` 等の押下中に、そのプレフィックスから始まるショートカット候補（`Cmd+Shift+X` 等）をステータスバーへリアルタイム表示する。

<a id="plan-F-06"></a>
- **F-06（実装中）**: コマンドパレットの位置を画面水平中央・最上部に調整
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/palette.rs`
  - 計画内容: パレットの表示位置を VS Code と同じく X 座標中央・上端（ヘッダ直下）に固定する。

<a id="plan-F-07"></a>
- **F-07（実装中）**: パレットの選択肢に「パレットを閉じる」を追加
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: パレット候補に常設の「パレットを閉じる（`Esc`）」項目を追加し、選択時にパレットを閉じる。

<a id="plan-F-08"></a>
- **F-08（実装中）**: パレットが節エディタ枠の下に隠れる z-index の修正
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/widgets/section_editor.rs`
  - 計画内容: パレットの `egui::Area` を `Order::Foreground` で最前面に描画し、節エディタの枠線がパレットの上に重ならないようにする。

<a id="plan-F-09"></a>
- **F-09（実装中）**: パレットの `>` でエージェント関連設定をサブ階層化
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`
  - 計画内容: `>` 直後の初期候補ではエージェント設定を `>agent ` の 1 項目にまとめ、`>agent` 入力または選択後に役割別・操作別の候補を展開する。

<a id="plan-F-10"></a>
- **F-10（実装完了: `REF:src/core/features/sections.rs`, `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/workbench.rs`）**: 本文エディタへのペースト時に改行保持＋`;` で自動節分割
  - 仕様・設計: `REF:docs/designs/editing.md`（§8.1）
  - 変更対象: `REF:src/core/features/sections.rs`, `REF:src/ui/workbench.rs`, `REF:src/ui/widgets/section_editor.rs`
  - 計画内容: ペーストされた文字列の改行を維持し、単独 `;` 行または `@A-B-C` 見出し行が含まれる場合は一気に複数の節へ分割して文書へ反映する。

<a id="plan-F-11"></a>
- **F-11（実装完了）**: 「？」「！」「＊」の太さを他文字と統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§11.5）
  - 変更対象: `REF:src/ui/theme.rs`
  - 計画内容: 全角約物（`？` `！` `＊`）が別フォントへフォールバックせず本文 CJK フォントで描画されるようフォントファミリ優先順を調整する。

<a id="plan-F-12"></a>
- **F-12（実装中）**: パレットの横幅固定と長い候補テキストの `...` 省略
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: パレットの幅を固定し、長い候補ラベルは `...` で省略してパレットの位置ズレを防ぐ。

<a id="plan-F-13"></a>
- **F-13（実装中）**: 未入力のパレット下部に接頭辞ルールのガイドを表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: 入力が空のときパレット下部に「`>` コマンド / `@` 節 / `'` 辞書 / `M`（`#`）資料 / `?` ショートカット」のガイドを表示する。

<a id="plan-F-14"></a>
- **F-14（実装中）**: パレットで候補が 0 件のときに「ない」旨を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`
  - 計画内容: 資料が 0 件のときに `M` / `#` を打った場合を含め、候補が 0 件のときはモードに応じた「一致する項目がありません（登録されている資料がありません）」を表示する。

<a id="plan-F-15"></a>
- **F-15（実装中）**: すべてのショートカットキー操作を `>` パレット経由でも実行可能にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7, §9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: 全 `Command` を `>` コマンドパレットに公開し、パレットから全ショートカット機能を実行できるようにする。

<a id="plan-F-16"></a>
- **F-16（実装中）**: ショートカット専用機能の視覚ボタン化（ガイドラベルのボタン化）
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1, §10.0〜§10.9）
  - 変更対象: `REF:src/ui/workspaces/mod.rs`, `REF:src/ui/workspaces/material.rs`, `REF:src/ui/workspaces/idea.rs`, `REF:src/ui/workspaces/plot_episode.rs`, `REF:src/ui/workspaces/plot_outline.rs`, `REF:src/ui/workspaces/cast.rs`, `REF:src/ui/workspaces/agent.rs`, `REF:src/ui/workspaces/review.rs`
  - 計画内容: 「`Ctrl+Shift+N` で新しい資料を作れます」等のガイドラベルをショートカット表記付きの押せるボタンに置き換える。

<a id="plan-F-17"></a>
- **F-17（実装中）**: ブラウザ・OS に奪われるショートカットの非衝突化調整
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/web.rs`
  - 計画内容: ブラウザ予約キーと被らない代替キー割当および `preventDefault` 調整を行う。

<a id="plan-F-18"></a>
- **F-18（実装中）**: 同梱フォント使用通知を左下のモードレストースト（数秒で消去）に変更
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.6, §11.5）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: フォントフォールバック通知をステータスバー常駐から外し、起動後 4 秒間だけ左下に表示されるモードレス通知にする。

<a id="plan-F-19"></a>
- **F-19（実装中）**: コマンドパレットから `agents/mcp.json` 編集モーダルへ移行
  - 仕様・設計: `REF:docs/designs/syntax.md`（§5.11）, `REF:docs/designs/agent.md`（§15.7）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/palette.rs`, `REF:src/ui/workspaces/agent.rs`
  - 計画内容: パレット（`>mcp` / `>agent mcp`）および `agent` 画面から `agents/mcp.json` を編集・検証・保存できるモーダルを追加する。

<a id="plan-F-20"></a>
- **F-20（実装中）**: エラー発生時に `review` ワークスペースアイコンへ視覚変化を付与
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1, §10.9）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: エラーまたは警告が存在するとき、ヘッダの `review` アイコンに色変化とバッジを描画する。

<a id="plan-F-21"></a>
- **F-21（実装中）**: 文脈ビューの節の境目に横線を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 文脈ビューの各節の間に区切り横線を描画する。

<a id="plan-F-22"></a>
- **F-22（実装中）**: 文脈ビューの節番号表示を `A-B-C-D`（4 階層完全形）に統一
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
- **F-25（実装中）**: パレットの `?` でショートカットを選択・実行可能にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: `?` モードをショートカットの検索＋選択実行モードとして実装する。

<a id="plan-F-26"></a>
- **F-26（実装中）**: パレットの `@` 移動を広さ優先の階層ドリルダウン（`@01-* 章タイトル` 等）に変更
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: `@` 入力時に `@01-*`, `@02-*`（章タイトル付き）から始め、話 → 場面 → 節へと広さ優先で段階的に絞り込む。

<a id="plan-F-27"></a>
- **F-27（実装中）**: パレット外クリック時に背面要素を押下判定にせずパレットのみ閉じる
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: パレット背面にフルスクリーンのクリック吸収シールド（`Sense::click()`）を配置し、外側クリックでパレットのみを閉じる。

<a id="plan-F-28"></a>
- **F-28（実装中）**: 参照ペインの各セクション境界に薄めの横線を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.5）
  - 変更対象: `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 上位プロット・配役・用語・資料・診断の各セクション間に 1px の `border` 横線を描画する。

<a id="plan-F-29"></a>
- **F-29（実装中）**: 押せないラベルのテキスト選択（ダブルクリック青選択）を無効化
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: `style.interaction.selectable_labels = false` を設定し、押せないラベルが選択ハイライトされないようにする。

<a id="plan-F-30"></a>
- **F-30（実装中）**: 押せる要素すべてに薄い下線＋ホバー変化を付与しテストで保証
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1, §9.9）
  - 変更対象: `REF:src/ui/widgets/mod.rs`, `REF:tests/gui.rs`
  - 計画内容: 全クリック対象に薄い下線とホバー変化を付与し、`REF:tests/gui.rs` で自動検査する。

<a id="plan-F-31"></a>
- **F-31（実装完了）**: 資料ワークスペースの `Ctrl+Shift+N` をペインフォーカス非依存にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.5）, `REF:docs/designs/workflows.md`（§16.1）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/shell.rs`
  - 計画内容: 左一覧以外にフォーカスがあっても `Ctrl+Shift+N`（および新規ボタン）で新しい資料を作成できるようにする。

<a id="plan-F-32"></a>
- **F-32（実装中）**: 全クリック対象に hover 背景（`bg.selection` 60%）＋ポインタ変更＋24×24px 以上の `interact` 矩形を付与
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.9）
  - 変更対象: `REF:src/ui/widgets/mod.rs`, `REF:src/ui/app.rs`, `REF:src/ui/workspaces/mod.rs`, `REF:tests/gui.rs`
  - 計画内容: `allocate_exact_size` の視覚寸法は変えずに `ui.interact` の当たり判定矩形を最低 24×24px に広げ、hover 時の `bg.selection`（60%）塗りと `CursorIcon::PointingHand` を全クリック対象へ適用する。

<a id="plan-F-33"></a>
- **F-33（実装中）**: Web 版で同じタイトルのプロジェクトを作成・改名できないようにする
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.4）
  - 変更対象: `REF:src/ui/web.rs`, `REF:src/adapters/storage_idb.rs`
  - 計画内容: 既存プロジェクトとタイトルが重複する場合は新規作成・改名を拒否してエラーを表示する。

<a id="plan-F-34"></a>
- **F-34（実装中）**: 参照ペインの要約が途中で切れるときに「`続き...`」の 1 行を表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.5）, `REF:docs/designs/workflows.md`（§16.5）
  - 変更対象: `REF:src/core/features/reference.rs`, `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 行数・文字数上限で切り詰めた要約の末尾にクリック可能な「`続き...`」を表示する。

<a id="plan-F-35"></a>
- **F-35（実装中）**: 全画面で「一覧移動は `↑`/`↓`、編集確定して移動は `Alt+↑`/`Alt+↓`」に統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.8）
  - 変更対象: `REF:src/ui/shell.rs`, `REF:src/ui/workspaces/material.rs`, `REF:src/ui/workspaces/idea.rs`, `REF:src/ui/workspaces/cast.rs`, `REF:src/ui/workspaces/plot_episode.rs`, `REF:src/ui/workspaces/plot_outline.rs`, `REF:src/ui/workspaces/agent.rs`, `REF:src/ui/workspaces/review.rs`
  - 計画内容: エディタにフォーカスがないときは素の `↑`/`↓` をどの画面でも一覧・タイムライン移動に効かせる。

<a id="plan-F-36"></a>
- **F-36（実装完了）**: 資料ワークスペースのフォーカス移動を `Tab` に移し `Ctrl+3` を参照ペイン移動に統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.8, §10.5）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/shell.rs`
  - 計画内容: `material` 内の `Ctrl+3` 独自バインドを撤去して `Tab` に移し、`Ctrl+3` の意味を共通シェルと統一する。

<a id="plan-F-37"></a>
- **F-37（実装中）**: `TextEdit` に `lock_focus(true)` を付け、入力欄外では `Tab` でヘッダボタン等を巡回可能にする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.4, §9.8）
  - 変更対象: `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/slice_editor.rs`, `REF:src/ui/app.rs`
  - 計画内容: 節エディタを 1 つのフォーカス対象にし、入力欄外での `Tab` フォーカス移動と明瞭なフォーカス枠表示を実装する。

<a id="plan-F-38"></a>
- **F-38（実装完了）**: アイデア 1,000 件の一覧で可視行のみを描画し 60fps を維持
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.4）, `REF:docs/designs/workflows.md`（§18.1）
  - 変更対象: `REF:src/ui/workspaces/idea.rs`
  - 計画内容: `ScrollArea::show_rows` による仮想スクロールを導入する。

<a id="plan-F-39"></a>
- **F-39（実装中）**: 文脈ビューの節未保存状態を示すオレンジの点を拡大
  - 仕様・設計: `REF:docs/designs/editing.md`（§8.4）, `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`, `REF:src/ui/app.rs`
  - 計画内容: 未保存ドットの直径を 8〜10px 相当に大きくする。

<a id="plan-F-40"></a>
- **F-40（実装中）**: デスクトップの `Ctrl+O` を「保存してから選択画面（履歴）へ戻る」に統一し 2 段構成にする
  - 仕様・設計: `REF:docs/designs/platform.md`（§14.4）, `REF:docs/designs/ui.md`（§9.8）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: `Ctrl+O` でいきなりフォルダダイアログを開かず、Web 版と同じくプロジェクト選択画面（履歴一覧）へ戻り、そこから「フォルダを開く」で選ぶ 2 段構成に統一する。

<a id="plan-F-41"></a>
- **F-41（実装中）**: ペインフォーカスを左端 3px 縦線、編集中の節を 1px 枠に分離し、文脈ビューのプレースホルダと対で見せる
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
  - 変更対象: `REF:src/ui/workbench.rs`, `REF:src/core/features/project.rs`, `REF:tests/incremental.rs`, `REF:benches/core.rs`
  - 計画内容: 打鍵時の同期解析を編集中の 1 節に限定して他節をデバウンスし、「10 万字のファイルで 1 打鍵 16ms 以内」を計測するテストを追加する。

<a id="plan-F-44"></a>
- **F-44（実装中）**: 資料画面のカテゴリチップ（「この資料のカテゴリ」直下＋選択枠）と条件欄プレースホルダ化・`fg.faint` 統一
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.5）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`
  - 計画内容: 8 個のカテゴリチップを「この資料のカテゴリ」見出し直下に置いて選択中を枠線で示し、条件の入力例は入力欄のプレースホルダに移し、説明文の色を `fg.faint` に統一する。

---

<a id="planned-small-features"></a>
### 2.3 機能追加（小規模：S-01〜S-18：一部実装中）

<a id="plan-S-01"></a>
- **S-01（実装完了）**: LLM エージェントに「例」として指定した話の本文またはプロットをコンテキスト付与
  - 仕様・設計: `REF:docs/designs/agent.md`（§15.4, §15.7）, `REF:docs/designs/syntax.md`（§5.9, §5.10）
  - 変更対象: `REF:src/core/model/task.rs`, `REF:src/core/agents/files.rs`, `REF:src/core/aicontext/packet.rs`, `REF:src/core/syntax/prompt_md.rs`, `REF:src/ui/workspaces/agent.rs`
  - 計画内容: タスクに参考例範囲 `examples` を持たせ、指定した話・節の本文またはプロットをコンテキストパケットおよび `{{examples}}` 変数として渡す。

<a id="plan-S-02"></a>
- **S-02（実装中）**: 初回起動時（履歴が空のとき）のアプリ説明・ドキュメントサイト案内とパレットからの再呼び出し
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.4, §14.4）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/web.rs`, `REF:src/ui/palette.rs`
  - 計画内容: 履歴が空の初回起動時にウェルカム案内を表示し、コマンドパレットからも常時呼び出し可能にする。

<a id="plan-S-03"></a>
- **S-03（実装中）**: ブラウザ版の PWA 化・オフライン動作対応と `README.md` への追記
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.3）
  - 変更対象: `REF:index.html`, `REF:Trunk.toml`, `REF:README.md`
  - 計画内容: Web App Manifest と Service Worker を追加してオフライン動作可能にし、`REF:README.md` に PWA 化可能であることを記載する。

<a id="plan-S-04"></a>
- **S-04（実装中）**: 本文が 0 文字のときに文脈ビューへ 3 行ガイドを薄く表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: 本文が空のとき `Alt+数字=画面 / Alt+矢印=節移動 / Ctrl+P=一覧` の 3 行ガイドを `fg.faint` で描画する。

<a id="plan-S-05"></a>
- **S-05（実装中）**: ヘッダ右に `?` と `Ctrl+P` のボタンを常設
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.1）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: 上部ヘッダ右端に `?` ボタンと `Ctrl+P` ボタンを常設する。

<a id="plan-S-06"></a>
- **S-06（実装中）**: ワークスペース切替時にパンくずの右へ画面名を 1 秒だけ表示
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.0）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: 画面切替から 1 秒間、パンくずの右にワークスペース名を表示する。

<a id="plan-S-07"></a>
- **S-07（実装中）**: 資料ビュワーの URL を外部リンクアイコン＋薄黒下線＋ホバー青色で開けるようにする
  - 仕様・設計: `REF:docs/designs/ui.md`（§10.5）, `REF:docs/designs/workflows.md`（§16.5）
  - 変更対象: `REF:src/ui/workspaces/material.rs`, `REF:src/ui/widgets/reference_pane.rs`
  - 計画内容: 資料内の URL に左下向き（外部リンク）矢印アイコン・薄い下線・ホバー時青色を付け、クリックで開けるようにする。

<a id="plan-S-08"></a>
- **S-08（実装中）**: 閉じ鉤括弧の後の `@` でキャラ名を予測補完
  - 仕様・設計: `REF:docs/designs/syntax.md`（§5.2）, `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/core/features/completion.rs`, `REF:src/ui/assist.rs`
  - 計画内容: `」@` / `』@` 入力時にキャラ名およびモードの補完ポップアップを出す。

<a id="plan-S-09"></a>
- **S-09（実装完了: `REF:src/core/resolve/dictionary.rs`）**: 未知の `'` 用語を形態素解析で区切り赤波線の範囲を決定
  - 仕様・設計: `REF:docs/designs/semantics.md`（§6.2）
  - 変更対象: `REF:src/core/resolve/dictionary.rs`, `REF:tests/dictionary.rs`
  - 計画内容: 未知の `'` 参照の赤波線範囲を形態素解析（文字種境界・助詞切り出し）による 1 形態素に区切る。

<a id="plan-S-10"></a>
- **S-10（実装中）**: `novnow import <zip> <dir>` の追加とデスクトップ版の Zip 導入対応
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.5, §14.1）
  - 変更対象: `REF:src/cli/mod.rs`, `REF:src/adapters/zip.rs`, `REF:src/ui/app.rs`, `REF:tests/cli.rs`
  - 計画内容: CLI `novnow import <zip> <dir>` とデスクトップ GUI での Zip インポートを実装する。

<a id="plan-S-11"></a>
- **S-11（実装中）**: デスクトップのプロジェクト選択画面に日付を表示
  - 仕様・設計: `REF:docs/designs/platform.md`（§14.4）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: `NativePicker` の各履歴行に `opened_at` の日付を表示する。

<a id="plan-S-12"></a>
- **S-12（実装中）**: 文脈ビューを可視範囲だけレイアウトする
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.3）, `REF:docs/designs/workflows.md`（§18.1）
  - 変更対象: `REF:src/ui/widgets/context_view.rs`
  - 計画内容: ビューポート外の節は高さ推定のみでスキップし、可視範囲の節のみレイアウトする。

<a id="plan-S-13"></a>
- **S-13（実装完了: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`, `REF:tests/gui.rs`）**: パレットで個別単位（例: `">ag"` => `">agent "`）の `Tab` 補完
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.7）
  - 変更対象: `REF:src/ui/palette.rs`, `REF:src/ui/app.rs`
  - 計画内容: パレットでの `Tab` キー押下時に次のトークン単位まで補完する。

<a id="plan-S-14"></a>
- **S-14（実装中）**: ステータスバーの幅に応じた省略優先順（保存状態最高優先、`…`＋ホバー詳細）
  - 仕様・設計: `REF:docs/designs/ui.md`（§9.6）
  - 変更対象: `REF:src/ui/app.rs`
  - 計画内容: 幅不足時に「セッション情報 → 原稿用紙枚数 → 文字数」の順で `…` に畳み、ホバーで詳細を出し、保存状態マークを最高優先で残す。

<a id="plan-S-15"></a>
- **S-15（実装完了）**: Web サイトへの全エラー解説の記載と CI での網羅性チェック
  - 仕様・設計: `REF:docs/designs/diagnostics.md`（§7.2）, `REF:docs/designs/workflows.md`（§18.4）
  - 変更対象: `REF:docs/specifications/diagnostics.md`, `REF:tests/diagnostics_catalog.rs`, `REF:.github/workflows/ci.yml`
  - 計画内容: 定義されている全 76 診断コードの原因・具体例・対処（日英）の個別ページを整備し、全コードの解説が存在するかを CI で機械検査する（`REF:website/src/pages/[lang]/diagnostics/[code].astro`, `REF:scripts/diagnostics-site.py`）。

<a id="plan-S-16"></a>
- **S-16（実装中）**: Web⇔デスクトップ双方向移行のヘルプ（Zip 手順）を追加
  - 仕様・設計: `REF:docs/designs/platform.md`（§13.5）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/ui/i18n/ja.toml`, `REF:src/ui/i18n/en.toml`
  - 計画内容: Zip を使った Web→デスクトップおよびデスクトップ→Web 移行手順のヘルプ画面を追加する。

<a id="plan-S-17"></a>
- **S-17（実装中）**: デスクトップ選択画面に「データは消さず履歴のみを消す」ことを明示した履歴削除機能を追加
  - 仕様・設計: `REF:docs/designs/platform.md`（§14.4）
  - 変更対象: `REF:src/ui/app.rs`, `REF:src/core/policy/recent.rs`
  - 計画内容: デスクトップの履歴一覧から、実データを消さずに履歴エントリのみを削除する操作と明示説明を追加する。

<a id="plan-S-18"></a>
- **S-18（実装完了: `REF:src/ui/settings.rs`, `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/widgets/context_view.rs`, `REF:tests/gui.rs`）**: 本文の最大幅を `.novnow/ui.json` の設定にし、`ja` は 44 字、`en` は 70 字前後を既定にする
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

段階 ① で整備した 10 個の拡張点（縫い代、`REF:docs/designs/overview.md` §3.3）を土台として、以下の 27 項目を段階 ② で実装する。

### 4.1 文脈・世界観・プロット拡張（追加仕様 1〜8）

| 番号 | 項目 | 主な変更対象 | 関連する診断・縫い代 |
|---|---|---|---|
| 1 | 物語内時計・タイムライン（`when:` フィールドとタイムラインタブ） | `REF:src/core/syntax/plot_episode.rs`, `REF:src/ui/workspaces/plot_outline.rs` | `E031`, `W088`, `W089`（縫い代 2, 3, 8） |
| 2 | キャラの節単位状態スナップショット（`cast-state:`） | `REF:src/core/syntax/plot_episode.rs`, `REF:src/ui/widgets/reference_pane.rs` | `W090`（縫い代 2, 6） |
| 3 | 読者/キャラの知識境界と劇的アイロニー（`knows:`） | `REF:src/core/syntax/plot_episode.rs`, `REF:src/core/features/project.rs` | `W091`（縫い代 2, 3） |
| 4 | 伏線レジストリの一次市民化（`# F####` と回収率マトリクス） | `REF:src/core/syntax/plot_episode.rs`, `REF:src/ui/workspaces/review.rs` | `W092`, `I053`（縫い代 2, 3） |
| 5 | 五感パレット（`senses:` と参照ペイン五感セクション） | `REF:src/core/features/reference.rs`, `REF:src/ui/widgets/reference_pane.rs` | `W093`（縫い代 2, 6） |
| 6 | 場面の空間・小道具スケッチ（`space:` と参照ペイン空間セクション） | `REF:src/core/features/reference.rs`, `REF:src/ui/widgets/reference_pane.rs` | 縫い代 2, 6 |
| 7 | 挿絵・ビジュアル資料（`S####` と参照ペイン挿絵セクション） | `REF:src/core/model/material.rs`, `REF:src/ui/widgets/reference_pane.rs` | 縫い代 6, 7 |
| 8 | 声紋カード・台詞比率メーター（文長ヒストグラム・一人称検査） | `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/workspaces/cast.rs` | `W094`, `W095`（縫い代 3, 8） |

### 4.2 エディタ・執筆体験拡張（追加仕様 9〜16）

| 番号 | 項目 | 主な変更対象 | 関連する診断・縫い代 |
|---|---|---|---|
| 9 | 文脈ビューの粒度切替（全文 ↔ 1 行要約 ↔ 見出しのみ、`Alt+-` / `Alt+=`） | `REF:src/ui/widgets/context_view.rs`, `REF:src/ui/shell.rs` | `REF:docs/designs/ui.md` §9.3 |
| 10 | 節内ブランチ（別案テイク `=== take: <name> ===` と `Alt+[` / `Alt+]`） | `REF:src/core/text/lexer.rs`, `REF:src/core/syntax/contents.rs` | `I002`（縫い代 4） |
| 11 | インライン下書きメモ（`??` / `？？` と未解決メモ集計） | `REF:src/core/text/lexer.rs`, `REF:src/core/export.rs` | `W080`（縫い代 3, 4） |
| 12 | ゴーストテキスト（プロット要点の薄字オーバーレイ、`Alt+G`） | `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/shell.rs` | `REF:docs/designs/ui.md` §9.4 |
| 13 | タイプライタスクロールとフォーカス調光（`Ctrl+Alt+Z`） | `REF:src/ui/widgets/section_editor.rs`, `REF:src/ui/settings.rs` | 縫い代 1 |
| 14 | 節の結合・移動・章話リナンバ（`Ctrl+Shift+M` 等） | `REF:src/core/features/sections.rs`, `REF:src/ui/workspaces/plot_outline.rs` | `REF:docs/designs/ui.md` §10.3 |
| 15 | 原稿用紙換算・セッション統計・締め切りペース表示 | `REF:src/core/features/project.rs`, `REF:src/ui/app.rs` | 縫い代 1, 9 |
| 16 | スナップショット（マイルストーン凍結 `.novnow/snapshots/`） | `REF:src/core/policy/write_policy.rs`, `REF:src/ui/app.rs` | `REF:docs/designs/editing.md` §8.6 |

### 4.3 日本語組版・文体点検拡張（追加仕様 17〜21）

| 番号 | 項目 | 主な変更対象 | 関連する診断・縫い代 |
|---|---|---|---|
| 17 | リズム・文体ヒートマップ（`review` の「文体」タブ） | `REF:src/core/features/project.rs`, `REF:src/ui/workspaces/review.rs` | `W081`, `W082`, `W085`, `W086`, `W087` |
| 18 | 視点（POV）漏れ検査の精密化 | `REF:src/core/features/project.rs` | `W084` |
| 19 | 表記揺れ・誤用辞書・組版規則の拡張 | `REF:src/core/features/project.rs`, `REF:src/core/model/project.rs` | `W096`, `W097`, `W098`, `W099`（縫い代 1, 3, 8） |
| 20 | ルビ一貫性ポリシー（`first_in_chapter` / `first_in_work` 等） | `REF:src/core/export.rs`, `REF:src/core/features/project.rs` | `I001`（縫い代 1） |
| 21 | 台詞話者の逆引き・無標識会話の検出強化 | `REF:src/core/features/project.rs` | `W083` |

### 4.4 AI エージェント・出力拡張（追加仕様 22〜27）

| 番号 | 項目 | 主な変更対象 | 関連する診断・縫い代 |
|---|---|---|---|
| 22 | 選択範囲インライン指示（`Ctrl+K` と `P####.diff` 即時プレビュー） | `REF:src/ui/app.rs`, `REF:src/ui/agents.rs` | `REF:docs/designs/agent.md` §15.3 |
| 23 | ロールプレイ対話モード（キャラ憑依インタビュー `@char`） | `REF:src/core/agents/runner.rs`, `REF:src/ui/workspaces/agent.rs` | `REF:docs/designs/agent.md` §15.6 |
| 24 | 7 番目の専門ロール `continuity`（設定考証・時系列監査） | `REF:src/core/syntax/prompt_md.rs`, `REF:src/core/template/mod.rs` | 縫い代 10 |
| 25 | コンテキスト予算の可視化とピン留め | `REF:src/core/aicontext/budget.rs`, `REF:src/ui/workspaces/agent.rs` | `REF:docs/designs/agent.md` §15.4 |
| 26 | 投稿サイト別エクスポートプリセット（`kakuyomu` / `narou` / `aozora`） | `REF:src/core/export.rs`, `REF:src/cli/export.rs`, `REF:src/ui/app.rs` | `REF:docs/designs/workflows.md` §17.1 |
| 27 | 縦書き・ページ組版プレビュー（`reader` ワークスペース、`Alt+7`） | `REF:src/ui/workspaces/mod.rs`, `REF:src/ui/app.rs` | `REF:docs/designs/ui.md` §10.7, `REF:docs/designs/workflows.md` §17.3 |
