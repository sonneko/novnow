# novnow 設計書 — 画面・ワークスペース・テーマ・国際化

本書は `REF:docs/specifications/ui.md` の各節に対応する実装設計を記述する。

## 9. 画面仕様（共通シェル）

### 9.1 全体レイアウト

状態: 実装中（REF:src/ui/app.rs, REF:src/ui/settings.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-16）

- 共通シェル（上端 28px アイコンバー、下端 22px ステータスバー、中央のメインカラム＋参照ペイン 2 カラム分割）は `NovnowApp`（`REF:src/ui/app.rs`）が描画する。
- メイン/参照の比率（既定 `62 : 38`、`Ctrl+Alt+Shift+←/→`）と文脈/節エディタの比率（既定 `45 : 55`、`Ctrl+Alt+↑/↓`）は `UiSettings`（`REF:src/ui/settings.rs`）に保持する。

- **[S-05] ヘッダ右端への `?` と `Ctrl+P` ボタン常設（未実装・実装計画済み: `REF:docs/plans.md#plan-S-05`）**:
  - `REF:src/ui/app.rs` のトップバー右側に `Ctrl+P` ボタンと `?` ボタンを常設する。
- **[F-20] エラー・警告時の `review` アイコン変化（未実装・実装計画済み: `REF:docs/plans.md#plan-F-20`）**:
  - `REF:src/ui/app.rs` でエラーまたは警告が 1 件以上あるとき、`WorkspaceId::Review` アイコンに `state.error` / `state.warn` 色のドットバッジと強調色を付与する。
- **[F-16 / F-29 / F-30] 視覚的アクションボタン・押せる要素の下線・静的ラベルの非選択化（未実装・実装計画済み: `REF:docs/plans.md#plan-F-16`, `REF:docs/plans.md#plan-F-29`, `REF:docs/plans.md#plan-F-30`）**:
  - `REF:src/ui/app.rs` で `style.interaction.selectable_labels = false` を設定し、押せないラベルのダブルクリック青選択を防ぐ。
  - `REF:src/ui/widgets/mod.rs` に共通クリック部品を設けて全ボタン・クリック対象に薄い下線＋ホバー背景＋ショートカット併記を適用する。

### 9.2 ラベル撤廃規則

状態: 実装完了（REF:src/ui/app.rs, REF:tests/layering.rs, REF:tests/gui.rs）

- ペイン見出しラベルや装飾枠線を一切置かず、余白とタイポグラフィで構造を表現する。
- 禁止ラベル文字列（「ファイル」「エディタ」「情報」等）および `Frame::group` が `src/ui/**` に存在しないことを `REF:tests/layering.rs` と `REF:tests/gui.rs`（`no_pane_is_labelled_files_editor_or_information`）で常時検査する。

### 9.3 文脈ビュー（メインカラム上段）

状態: 実装中（REF:src/ui/widgets/context_view.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-21）

- 前後最大 3 節の読み取り専用表示と、編集中の節の位置に本文を重複描画せず 1 行プレースホルダ `─── @01-04-02-03 ───` のみを置く処理は `ui::widgets::context_view`（`REF:src/ui/widgets/context_view.rs`）に実装する（検証: `REF:tests/gui.rs` の `the_context_view_never_paints_the_section_being_edited`）。
- ※§9.3.3 の折りたたみ UI は段階 ②（追加仕様 9、`REF:docs/plans.md#stage-2`）対象。

- **[F-21 / F-22 / F-39 / F-41 / S-04 / S-12] 文脈ビューの視認性・ガイド・可視範囲レイアウト改善（未実装・実装計画済み: `REF:docs/plans.md#plan-F-21`）**:
  - `REF:src/ui/widgets/context_view.rs` で各節境界に `border` 色の横線を描画し（F-21）、節見出しを章省略なしの `@AA-BB-CC-DD`（`A-B-C-D`）に統一する（F-22）。
  - 編集中プレースホルダ行の左端に 3px の `accent` 縦線を描画して節エディタと対にし（F-41）、未保存ドットを直径 8〜10px に拡大する（F-39）。
  - 本文が 0 文字のときは `fg.faint` で 3 行ガイド（`Alt+数字=画面 / Alt+矢印=節移動 / Ctrl+P=一覧`）を描画し（S-04）、スクロール可視範囲内の節のみ `Galley` レイアウトを行う（S-12）。

### 9.4 節エディタ（メインカラム下段）

状態: 未実装・実装計画済み（REF:src/ui/widgets/section_editor.rs, REF:src/ui/widgets/syntax_text.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-02）

- 1 行ミニヘッダー（パンくず `01 › 04 › 02 › 03` と有効モードバッジ）および構文ハイライト・インレイヒント・ベースライン基準の下線/波線描画を伴う複数行エディタは `REF:src/ui/widgets/section_editor.rs` と `REF:src/ui/widgets/syntax_text.rs` に実装する。
- ※§9.4.2 の台詞比率バー、§9.4.3 のゴーストテキスト、§9.4.4 のタイプライタスクロールは段階 ②（`REF:docs/plans.md#stage-2`）対象。

- **[F-02 / F-03 / F-37 / F-41] 節エディタの枠内パディング・カーソル/行番号調整・フォーカス表現分離・`lock_focus(true)`（未実装・実装計画済み: `REF:docs/plans.md#plan-F-02`）**:
  - `REF:src/ui/widgets/section_editor.rs` で枠内パディングとクリップ矩形を整えてテキストのはみ出しを解消し（F-02）、行番号の Y 座標とキャレット高さを `Galley` の行メトリクスに一致させる（F-03）。
  - ペインのフォーカス表示は左端 3px 縦線、編集中の節は常時 1px 枠線に分離する（F-41）。
  - `TextEdit::multiline` に `.lock_focus(true)` を付け、入力欄外での `Tab` 巡回と 2px フォーカス枠描画を実装する（F-37）。

### 9.5 参照ペイン

状態: 実装中（REF:src/core/features/reference.rs, REF:src/ui/widgets/reference_pane.rs, REF:tests/context_rules.rs, REF:docs/plans.md#plan-F-04）

- 段階 ① の 6 セクション（プロット・上位プロット・配役・用語・資料・診断）のデータ組み立ては `core::features::reference::build`（`REF:src/core/features/reference.rs`）が `Vec<Section>` として返し（縫い代 6）、`REF:src/ui/widgets/reference_pane.rs` が枠線なしで描画する（80ms デバウンス）。
- ※五感・空間・挿絵の 3 セクションは段階 ②（`REF:docs/plans.md#stage-2`）対象。

- **[F-04 / F-28 / F-34] 参照ペインのアコーディオン・境界線・「続き...」表示（未実装・実装計画済み: `REF:docs/plans.md#plan-F-04`）**:
  - `REF:src/ui/widgets/reference_pane.rs` で同一診断コードが 4 件以上のときは 1 行アコーディオンに集約する（F-04）。
  - 各セクション間に 1px の薄い横線を引き（F-28）、要約切り詰め時は末尾にクリック可能な「`続き...`」行を表示する（F-34）。

- **[F-23] 参照ペインの診断からの自動回復（実装完了: `REF:src/ui/widgets/reference_pane.rs`, `REF:src/ui/app.rs`）**:
  - 未解決参照診断には、形態素解析で区切った 1 形態素を `words.md` / `characters.md` へ登録するボタンを配置する。

### 9.6 ステータスバー

状態: 実装中（REF:src/ui/app.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-05）

- 左側（位置 `01›04›02›03`、節/話/全体の空白除外文字数、400 字詰め原稿用紙換算）と右側（エラー・警告・情報件数、エージェント実行状態、保存状態アイコン）は `NovnowApp`（`REF:src/ui/app.rs`）に実装する（検証: `REF:tests/gui.rs` の `counts_are_section_episode_and_project_without_whitespace`）。
- ※§9.6.2 のセッション統計モードは段階 ②（`REF:docs/plans.md#stage-2`）対象。

- **[F-05 / F-18 / S-14] 修飾キー補完表示・起動通知の左下トースト化・幅に応じた省略優先順（未実装・実装計画済み: `REF:docs/plans.md#plan-F-05`）**:
  - `REF:src/ui/shell.rs` と `REF:src/ui/app.rs` で、`Cmd`/`Ctrl`/`Alt`/`Shift` 押下中に次に入力可能なショートカット候補をステータスバーに表示する（F-05）。
  - 同梱フォント使用通知はステータスバーから外し、起動後数秒間のみ左下に表示するモードレストーストへ移す（F-18）。
  - ステータスバーの幅不足時は「セッション情報 → 原稿用紙枚数 → 文字数」の順に `…` へ畳んでホバー詳細とし、保存状態マークを最高優先で常時表示する（S-14）。

### 9.7 ナビゲーション

状態: 実装中（REF:src/ui/palette.rs, REF:src/ui/assist.rs, REF:src/ui/app.rs, REF:tests/gui.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-06）

- 統一パレット（`Ctrl+P` / `Ctrl+G` / `Ctrl+Shift+P`）の先頭文字によるモード切替（通常 / `@` 構造 / `'` 辞書 / `#` 資料 / `>` コマンド）と決定論的ファジースコアリング（完全一致 +1000、先頭一致 +500、連続一致 +200、頭文字 +100、50 件上限）は `ui::palette`（`REF:src/ui/palette.rs`）に実装する。
- `'名前` パレットでの `Enter`（`core::features::definition::at` による定義ジャンプ）と `Shift+Enter`（`core::features::references::find` による使用箇所一覧）、および入力中補完ポップアップ（`REF:src/ui/assist.rs`）は `REF:src/ui/app.rs` で接続されている（検証: `REF:tests/gui.rs`, `REF:tests/workspaces.rs`）。

- **[F-06〜F-09 / F-12〜F-15 / F-25〜F-27 / S-08 / S-13] パレットの UX 全面改善（未実装・実装計画済み: `REF:docs/plans.md#plan-F-06`）**:
  - `REF:src/ui/app.rs` と `REF:src/ui/palette.rs` でパレットを画面水平中央・上端に固定し（F-06）、`Order::Foreground` で青枠より前面に描画し（F-08）、背面シールドでパレット外クリック時は背面要素を発火させずにパレットのみを閉じる（F-27）。
  - パレット幅を固定して長文候補を `...` で省略し（F-12）、候補に「パレットを閉じる」を追加し（F-07）、未入力時の下部ガイド（F-13）と 0 件時の「ない」表示（F-14）を追加する。
  - `>` でエージェント系を `>agent ` 階層にまとめ（F-09）、全ショートカット操作を `>` から実行可能にし（F-15）、`?` をショートカット選択実行モードにし（F-25）、`@` を広さ優先の階層ドリルダウン（`@01-* 章タイトル` → `@01-01-* 話タイトル` → …）に変更し（F-26）、`Tab` によるトークン単位補完（S-13）を実装する。

### 9.8 キーボード一覧（v1 確定版）

状態: 未実装・実装計画済み（REF:src/ui/shell.rs, REF:src/ui/web.rs, REF:tests/gui.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-17）

- 全ショートカットの解釈と macOS での `Alt` → `Ctrl+Alt` 読み替えは `ui::shell`（`REF:src/ui/shell.rs`）に実装する。
- **egui 0.27.2 における IME 実装規約**:
  - `egui::TextEditState::has_ime` は非公開のため、`ui::shell::ImeState`（`REF:src/ui/shell.rs`）で `egui::Event::CompositionStart` / `CompositionUpdate` / `CompositionEnd` を追跡する。
  - 1 フレームの処理順序は `ImeState::update() → composing なら Event::Key をすべて破棄 → ショートカット解釈 → テキスト編集` とする。
  - フェイルセーフとして `Event::WindowFocused(false)` または最後の変換イベントから 2 秒無入力で `composing = false` に戻す。
  - デスクトップで IME を有効化するため、編集領域フォーカス中は毎フレーム `PlatformOutput.ime = Some(IMEOutput { rect, cursor_rect })` を出力する。Web 版でキーボード移動時にも IME hidden input を同期する処理は `sync_text_agent`（`REF:src/ui/web.rs`）が担う（検証: `REF:tests/gui.rs`, `REF:tests/gui.rs`）。

- **[F-17 / F-35 / F-36 / F-37 / F-40] キー割当の統一とブラウザ非衝突化（実装中: `REF:docs/plans.md#plan-F-17`）**:
  - `REF:src/ui/shell.rs` は Web の予約キーに `Ctrl+Alt+Shift+I/N/T/W` を代替割当にし、ブラウザの開発者ツール等を妨げない。`REF:src/ui/web.rs` の key guard と `REF:tests/gui.rs` で確認する。
  - `REF:src/ui/shell.rs` と各ワークスペースで「一覧選択は素の `↑`/`↓`、確定して節移動は `Alt+↑`/`Alt+↓`」に統一し（F-35）、`Ctrl+3` を全画面で「参照ペインへフォーカス」に統一して資料内の移動は `Tab` に移し（F-36）、デスクトップの `Ctrl+O` を「保存して一覧へ戻る」に統一する（F-40）。

### 9.9 アクセシビリティ

状態: 実装中（REF:src/ui/theme.rs, REF:src/ui/app.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-30）

- アイコンボタンへの i18n 済みツールチップ/アクセシブル名付与、色＋形状の併用、WCAG コントラスト比基準（本文 4.5:1 以上、補助テキスト 3.0:1 以上）は `REF:src/ui/theme.rs` と `REF:src/ui/app.rs` で実装し、`REF:tests/gui.rs` の `every_token_meets_the_contrast_minimums` で全カラートークンのコントラスト比を機械検証する。

- **[F-30 / F-32] 全クリック対象の下線・ホバー背景・ポインタ・24×24px 当たり判定と自動テスト（未実装・実装計画済み: `REF:docs/plans.md#plan-F-30`, `REF:docs/plans.md#plan-F-32`）**:
  - `REF:src/ui/widgets/mod.rs` の統一クリックヘルパで、視覚寸法（`allocate_exact_size` 等）はそのままに `ui.interact` に渡す矩形を最低 `24.0 x 24.0` へ拡張し、薄い下線描画＋ホバー時 `bg.selection`（60%）塗りと `CursorIcon::PointingHand` を全クリック対象に適用する。`REF:tests/gui.rs` で全画面のクリック対象がこれを満たすことを検証する。

## 10. ワークスペース

### 10.0 切替の挙動

状態: 実装中（REF:src/ui/workspaces/mod.rs, REF:src/ui/app.rs, REF:tests/workspaces.rs, REF:docs/plans.md#plan-S-06）

- ワークスペース識別子 `WorkspaceId` は `REF:src/ui/workspaces/mod.rs` に定義し、各ワークスペースのスクロール・選択状態は破棄せず `Session`（`REF:src/ui/app.rs`）に常時保持する（縫い代 5）。切替時も現在の `SceneId` を維持する。

- **[S-06] 切替時の画面名 1 秒表示（未実装・実装計画済み: `REF:docs/plans.md#plan-S-06`）**:
  - `REF:src/ui/app.rs` でワークスペース切替時刻を記録し、切替後 1.0 秒間はパンくずの右に画面名バッジを描画する。

### 10.1 `write` — 本文執筆（§9 の基本レイアウトそのもの）

状態: 実装完了（REF:src/ui/workspaces/write.rs, REF:src/ui/app.rs, REF:tests/gui.rs）

- 共通シェル（文脈ビュー＋節エディタ＋参照ペイン）による本文執筆ワークスペースは `REF:src/ui/workspaces/write.rs` および `REF:src/ui/app.rs` に実装する。

### 10.2 `plot-episode` — 話プロット

状態: 実装完了（REF:src/core/features/plots.rs, REF:src/ui/workspaces/plot_episode.rs, REF:tests/plots.rs, REF:tests/workspaces.rs）

- 場面・節のカード一覧と本文冒頭 1 行プレビュー・文字数集計・状態バッジは `core::features::plots`（`REF:src/core/features/plots.rs`）が計算し、`REF:src/ui/workspaces/plot_episode.rs` が描画・編集（`Enter` で `write` へ移動、`Space` で `state` 巡回、`M` でモード選択）を担う。

### 10.3 `plot-outline` — 全体 / 章プロット

状態: 実装完了（REF:src/core/features/plots.rs, REF:src/core/features/folding.rs, REF:src/ui/workspaces/plot_outline.rs, REF:tests/workspaces.rs）

- 左ペインの構成ツリー（章→話展開に `core::features::folding::ranges` を使用）と中央ペインのブロックエディタ・進捗バーは `REF:src/ui/workspaces/plot_outline.rs` に実装する。
- ※タイムライン・伏線マトリクス・章話番号リナンバは段階 ②（`REF:docs/plans.md#stage-2`）対象。

### 10.4 `idea` — アイデア

状態: 実装中（REF:src/core/features/ideas.rs, REF:src/ui/workspaces/idea.rs, REF:tests/ideas_ops.rs, REF:tests/workspaces.rs, REF:docs/plans.md#plan-F-38）

- タグ・状態フィルタ、一覧、MD エディタ、新規採番（`N`）、状態変更（`1`〜`3`）、プロットへの昇格（`P`）は `REF:src/core/features/ideas.rs` と `REF:src/ui/workspaces/idea.rs` に実装する。

- **[F-38] アイデア 1,000 件時の可視行仮想スクロール描画（未実装・実装計画済み: `REF:docs/plans.md#plan-F-38`）**:
  - `REF:src/ui/workspaces/idea.rs` の一覧を `ScrollArea::show_rows` による可視行限定描画に変更し、1,000 件でも 60fps を維持する。

### 10.5 `material` — 資料（§16 に詳細）

状態: 実装中（REF:src/core/features/reference.rs, REF:src/ui/workspaces/material.rs, REF:tests/materials_ops.rs, REF:tests/workspaces.rs, REF:docs/plans.md#plan-F-31）

- 分類ツリー・資料一覧・MD 本文エディタ・インデックスメタデータ表示・状態変更（`A` 承認 / `X` 却下）は `REF:src/ui/workspaces/material.rs` に実装する。

- **[F-31 / F-36 / F-44 / S-07] 資料ワークスペースのショートカット全域化・カテゴリ/条件欄整理・URL 外部リンク（未実装・実装計画済み: `REF:docs/plans.md#plan-F-31`, `REF:docs/plans.md#plan-F-44`, `REF:docs/plans.md#plan-S-07`）**:
  - `REF:src/ui/workspaces/material.rs` で `Ctrl+Shift+N` をペインフォーカス非依存にし、ガイドラベルをクリック可能なボタンにする（F-31, F-16）。
  - フォーム間のフォーカス移動を `Tab` に移し（F-36）、右ペインのカテゴリチップを「この資料のカテゴリ」見出し直下に置いて選択中を枠線で示し、条件の入力例は `TextEdit::hint_text` に移して説明文を `fg.faint` に統一する（F-44）。
  - 資料内の URL に外部リンクアイコン＋薄い下線＋ホバー時青色を付け、クリックで開けるようにする（S-07）。

### 10.6 `cast` — 配役・モード・役割語

状態: 実装完了（REF:src/core/features/cast.rs, REF:src/ui/workspaces/cast.rs, REF:tests/cast.rs, REF:tests/workspaces.rs）

- キャラ一覧、属性・モード一覧・役割語パターン表示、および役割語テスター（入力文に対する `role_language::MatcherSet` のリアルタイム緑下線プレビュー）と節別モード割当マトリクスは `REF:src/core/features/cast.rs` と `REF:src/ui/workspaces/cast.rs` に実装する。

### 10.7 `reader` — 読者モード（§17.3 に詳細）

状態: 実装待ち（段階 ② 対象: REF:docs/plans.md#stage-2）

- 縦書き読者モードは追加仕様 27（段階 ②）として予約されており、`Alt+7` は段階 ① では無操作とする（`REF:src/ui/workspaces/mod.rs`）。

### 10.8 `agent` — エージェント管制（§15.7 に詳細。委譲レベルの確認・変更は §15.7.6）

状態: 実装完了（REF:src/ui/workspaces/agent.rs, REF:src/ui/agents.rs, REF:tests/workspaces.rs）

- 4 ペイン構成（タスク一覧・コンテキストプレビュー・結果/提案差分レビュー・委譲レベル管制と MCP 状態）は `REF:src/ui/workspaces/agent.rs` と `REF:src/ui/agents.rs` に実装する。

### 10.9 `review` — 点検

状態: 実装中（REF:src/core/features/project.rs, REF:src/ui/workspaces/review.rs, REF:tests/workspaces.rs, REF:docs/plans.md#plan-F-20）

- 診断一覧タブ（重大度・コード・位置・クイックフィックス適用）と進捗ツリータブ（章・話・節ごとの `state` と文字数集計）は `REF:src/core/features/project.rs` と `REF:src/ui/workspaces/review.rs` に実装する。
- ※伏線一覧・文体・統計の 3 タブは段階 ②（`REF:docs/plans.md#stage-2`）対象。

- **[F-20] エラー・警告発生時の `review` アイコン変化（未実装・実装計画済み: `REF:docs/plans.md#plan-F-20`）**:
  - `REF:src/ui/app.rs` のヘッダアイコン描画でエラー/警告件数に応じたバッジ・色変化を表示する。

## 11. テーマ・配色・タイポグラフィ

### 11.1 セマンティックカラートークン

状態: 実装完了（REF:src/ui/theme.rs, REF:tests/gui.rs）

- `Palette`（`REF:src/ui/theme.rs`）が Dark / Light の全セマンティックカラートークンを保持する。仕様表の 16 進カラーコードとの完全一致は `REF:tests/gui.rs` の `theme_colors_are_the_spec_tables` で検証する。

### 11.2 構文ハイライト割当

状態: 実装完了（REF:src/ui/theme.rs, REF:tests/gui.rs）

- `token_style`（`REF:src/ui/theme.rs`）が `TokenType` ごとの Dark / Light 配色と下線有無を返す（検証: `REF:tests/gui.rs`）。

### 11.3 下線の描画仕様

状態: 実装完了（REF:src/ui/widgets/syntax_text.rs, REF:tests/gui.rs）

- 役割語一致（1px 実線）、Error（2px 波線）、Warning（1.5px 波線）、Info（1px 点線）は `egui::TextFormat::underline` を使わず、`Galley` のグリフベースライン座標から `REF:src/ui/widgets/syntax_text.rs` が直接ペイントする。

### 11.4 テーマ切替

状態: 実装中（REF:src/ui/theme.rs, REF:src/ui/settings.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-01）

- `ThemePreference`（`System` / `Dark` / `Light`）は `REF:src/ui/settings.rs` で `.novnow/ui.json` に永続化し、`Ctrl+Alt+T` または `>theme` で即時切り替える。

- **[F-01] テーマ記号（`◐` / `☾` / `☀`）のベジェ・ベクター描画化（未実装・実装計画済み: `REF:docs/plans.md#plan-F-01`）**:
  - `REF:src/ui/theme.rs` と `REF:src/ui/app.rs` でフォント文字 `"☾"` 等を使わず、`egui::Painter` のベジェ曲線と円弧による自前描画関数 `paint_theme_icon` に置き換える。

### 11.5 タイポグラフィ

状態: 実装中（REF:src/ui/theme.rs, REF:tests/gui.rs, REF:docs/plans.md#plan-F-11）

- デスクトップでは `fontdb` でシステムの CJK フォントを探索し、見つからない場合および Web 版では `assets/ipag.ttf`（同梱 IPA ゴシック）にフォールバックする（`REF:src/ui/theme.rs`）。また `⟨⟩`（U+27E8 / U+27E9）表示のため egui 既定の Hack フォントをフォールバック末尾に連結する。

- **[F-11 / S-18] 全角「？」「！」「＊」の太さ統一と `max_line_chars` 対応（未実装・実装計画済み: `REF:docs/plans.md#plan-F-11`, `REF:docs/plans.md#plan-S-18`）**:
  - `REF:src/ui/theme.rs` のフォント設定で全角約物が CJK フォントからグリフ解決される順序を保証し（F-11）、`REF:src/ui/settings.rs` の `max_line_chars`（`ja`=44, `en`=70）に応じて本文折り返し幅を計算する（S-18）。

## 12. 国際化

### 12.1 対象言語

状態: 実装完了（REF:src/ui/i18n/mod.rs, REF:src/ui/i18n/ja.toml, REF:src/ui/i18n/en.toml）

- `Locale::Ja` と `Locale::En` を `REF:src/ui/i18n/mod.rs` に定義し、全 UI・CLI・診断メッセージを 2 言語で提供する。

### 12.2 ドメイン処理と表示文言の分離（MUST）

状態: 実装完了（REF:src/core/diagnostics.rs, REF:src/ui/i18n/mod.rs, REF:tests/layering.rs）

- `src/core/**`・`src/cli/**`・`src/main.rs`・`src/ui/**` に日本語文字列リテラルが存在しないことを `REF:tests/layering.rs`（`core_has_no_user_facing_text`, `cli_has_no_user_facing_text`, `gui_has_no_user_facing_text`）で AST 検査する。

### 12.3 カタログ

状態: 実装完了（REF:src/ui/i18n/ja.toml, REF:src/ui/i18n/en.toml, REF:docs/specifications/diagnostics.md, REF:tests/i18n_catalog.rs）

- `REF:src/ui/i18n/ja.toml` と `REF:src/ui/i18n/en.toml` を `include_str!` で埋め込み、キー集合・プレースホルダ番号・ソースコード内の参照キーとの過不足ゼロを `REF:tests/i18n_catalog.rs` で検証する。

- **[F-42] `ja.toml` のプレースホルダ直後の助詞空白禁止と検査テスト（実装完了: `REF:src/ui/i18n/ja.toml`, `REF:tests/i18n_catalog.rs`）**:
  - `REF:src/ui/i18n/ja.toml` の `{0} はこの版ではまだ使えません` 等を `{0}はこの版ではまだ使えません` に修正し、`REF:tests/i18n_catalog.rs` に「`}` の直後に半角空白＋ひらがな助詞が続かないこと」を検証するテストを追加する。
  - 同じ規則を診断カタログ（`REF:docs/specifications/diagnostics.md` §7）の日本語列にも適用した（人間承認 2026-10-15。経緯は `REF:docs/history.md` の `v2.21`）。

### 12.4 言語の決定順序

状態: 実装完了（REF:src/ui/i18n/mod.rs, REF:src/adapters/env_system.rs, REF:tests/i18n_catalog.rs）

- `.novnow/ui.json` → `NOVNOW_LANG` → OS/ブラウザロケール（`REF:src/adapters/env_system.rs`）→ 既定 `ja` の優先順位は `Locale::resolve`（`REF:src/ui/i18n/mod.rs`）に実装する（検証: `REF:tests/i18n_catalog.rs`）。

### 12.5 言語切替 UI

状態: 実装完了（REF:src/ui/app.rs, REF:tests/gui.rs）

- ヘッダ右端の `あ` / `A` トグルボタンとアクセシブル名は `NovnowApp`（`REF:src/ui/app.rs`）に実装する。

### 12.6 翻訳しないもの

状態: 実装完了（REF:src/core/text/lexer.rs, REF:tests/lexer.rs）

- 原稿データおよびファイル形式のフィールドキー別名（`REF:src/core/text/lexer.rs`）は UI 言語設定に依存せず常に同一に処理する。

### 12.7 主要文言表

状態: 実装完了（REF:src/ui/i18n/ja.toml, REF:src/ui/i18n/en.toml, REF:tests/i18n_catalog.rs）

- 仕様書 §12.7 の文言表と `ja.toml` / `en.toml` の完全一致は `REF:tests/i18n_catalog.rs` の `the_cli_rows_of_the_spec_table_are_copied_verbatim` で検証する。
