# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 123 節 / 実装中 0 節 / 未実装・実装計画済み 0 節 / 未実装・未計画 1 節 / 実装待ち 2 節 | 全 10 設計書（`docs/designs/*.md`）の `### ` 節（合計 126 節） | `REF:docs/plans.md#planned-fixes-and-features` |
| 仕様変更・修正（F-01〜F-44） | 実装完了（44 件） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装完了（18 件） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 未実装・未計画 | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 未着手（計画待機） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- **現在のブランチ**: `arena/75c027a6-novnow`。
- **PR**: PR #18（レビュー待ち、base は前セッションの Draft PR #17 の作業ブランチ）。
- **現在の状態**: `REF:docs/plans.md#planned-fixes-and-features` の計画済み 62 項目（F-01〜F-44、S-01〜S-18）を実装・自動検証済み。L-01（未計画）と段階 ②（計画待機）は対象外。macOS/Windows 実機での操作および異なる性能環境での測定はこの Linux サンドボックスでは行っていない。
- **本ブランチの作業**: `REF:src/ui/app.rs` ほかのパレット・共通シェル・参照ペイン、`REF:src/cli/import.rs` の CLI Zip 導入、`REF:src/adapters/storage_idb.rs` の Web 同名制限、`REF:src/core/aicontext/packet.rs` の参考例、`REF:sw.js` の PWA、`REF:website/src/data/diagnostics.ts` の診断一覧を実装済み。
- **検証**: `bash scripts/verify.sh` は 1232 件のテストで全ジョブ成功（53 ターゲット）。`REF:scripts/web-smoke.sh` は Web の既存シナリオと PWA のオフライン再起動を含め成功。幅 620px の画面を撮り、F-02/F-03 のキャレット・本文の収まりと S-14 の右端の未保存マークを視覚確認（`REF:scripts/web-smoke.mjs` の `02b` / `02c`）。`npm run build`（`website/`、診断の個別ページ 152 件を含む 160 ページ）と `python3 scripts/diagnostics-site.py --check` も成功。
- **最終追加作業**: F-16 の Idea ガイドをボタンから起動、F-17 の macOS 代替割当と Web の予約キー、F-35 のヘッダーボタンフォーカス時の一覧移動を GUI テストで確認。F-30/F-32 は共通クリック部品・行・Web 選択・個別アイコン・タイムラインを監査し、hover 背景、ポインタ、下線、24px 当たり判定の GUI テストを追加（`REF:src/ui/workspaces/cast.rs`, `REF:tests/gui.rs`, `REF:tests/workspaces.rs`）。`PR=17 bash scripts/web-smoke.sh` は Web 予約キー、編集・Zip・PWA 再起動を含め成功。
- **直近の追加作業**: F-02/F-03 の節本文と装飾の枠内クリップ、論理行番号とフォント大キャレットの縦位置を 620/380px GUI 描画形状で検証（`REF:src/ui/widgets/section_editor.rs`, `REF:tests/gui.rs`）。
- **直近の追加作業**: F-37 の `TextEdit::lock_focus(true)` と入力欄外の Tab 巡回を GUI で確認。F-40 は `Ctrl+O` で保存後に一覧へ戻ることと、保存失敗時には原稿を捨てず画面を維持することを GUI で確認。S-12 は文脈ビューの不可視節の推定高さと可視節のみの Galley 生成をユニットテストで確認し、不可視節のハイライト抜粋も省略（`REF:src/ui/widgets/context_view.rs`, `REF:tests/gui.rs`）。
- **直近の追加作業**: F-15/F-25 の `>` / `?` でのショートカット検索・実行と F-26 の章→話→場面→節ドリルダウンを GUI で確認。F-19 は `>mcp` と agent 画面からの編集、編集中の JSON 構文/未対応トランスポートのエラー、保存時の原稿反映を確認。F-20 は点検アイコンの警告色/エラー色とバッジを描画検証（`REF:src/ui/app.rs`, `REF:src/ui/workspaces/mod.rs`, `REF:tests/gui.rs`）。
- **直近の追加作業**: F-06/F-08 はパレットを画面全幅の中央・ヘッダ直下の前面に固定。F-07/F-13/F-14 は閉じる候補・未入力ガイド・0 件時の案内を GUI で確認。F-09 は `>agent` 入力時の展開、F-12 は長い CJK 候補の描画幅単位の省略、F-27 は背面へのクリックを防ぐシールドを GUI で確認（`REF:src/ui/app.rs`, `REF:tests/gui.rs`）。
- **直近の追加作業**: F-05 の修飾キー別ステータスガイドを GUI と macOS 用 `Cmd` 文言のユニットテストで確認。F-18 の同梱フォント通知は画面左下に起動後 4 秒だけ表示されることを GUI で確認。S-14 の 1280/620/380px 幅での段階的 `…` 表示と未保存マークの保持を GUI で確認（`REF:src/ui/app.rs`, `REF:tests/gui.rs`）。
- **直近の追加作業**: F-44 は 8 個のカテゴリチップを「この資料のカテゴリ」直下に配置し、選択枠と条件入力欄のヒントを `REF:tests/workspaces.rs` で確認（`REF:src/ui/workspaces/material.rs`）。
- **直近の追加作業**: S-08 は `」@` / `』@`、空白後の `@`、打ちかけの名前および `:モード` の補完が `REF:tests/features_edit.rs` で検証済みであることを確認。
- **直近の追加作業**: F-41 のエディタの 1px 枠と左端 3px フォーカス縦線、文脈ビューの同幅プレースホルダ縦線を `REF:tests/gui.rs` で描画形状として検証。
- **直近の追加作業**: S-10 は CLI `import <zip> <dir>` の復元・既存ファイル非上書き・不正 Zip の拒否を `REF:tests/cli.rs` と `REF:tests/zip_roundtrip.rs` で確認。デスクトップ選択画面から Zip と導入先を選ぶ経路は `REF:src/ui/app.rs` に実装済み（ネイティブ GUI の実機視覚確認は残る）。
- **直近の追加作業**: S-11 はデスクトップ履歴の最終利用日付表示を native picker のテストで確認。S-17 は履歴エントリだけ削除し原稿ファイルを保持する統合テストと、「データは消えない」ツールチップを確認（`REF:src/ui/app.rs`, `REF:tests/recent_file.rs`）。
- **直近の追加作業**: S-02/S-16 は初回のウェルカム案内と `>welcome` / `>migration` のパレットから開く経路を確認。Zip を使う Web→デスクトップ・デスクトップ→Web の両方向のヘルプが表示されることを `REF:tests/gui.rs` で検証。
- **直近の追加作業**: F-33 は実 Chromium の新規作成・F2 改名で既存タイトルと重複させ、いずれも IndexedDB の作品数・名前が変わらないことを `REF:scripts/web-smoke.mjs` で確認。
- **直近の追加作業**: S-03 の PWA Manifest・Service Worker と `REF:README.md` の案内を確認。`REF:scripts/web-smoke.mjs` の実ブラウザ・オフライン再起動で IndexedDB 内容を保持したまま起動できることを検証済み。
- **直近の追加作業**: S-05/S-06 はヘッダの `?`/`Ctrl+P` クリックと、切替後 1 秒以内にだけ画面名がヘッダに出ることを `REF:tests/gui.rs` で確認。
- **直近の追加作業**: F-04 は同じ診断が 4 件以上のときの開閉と 3 件以下の非集約を GUI で検証。F-34 は要約の 120 文字または 5 行の上限で「続き...」を表示し、クリックで資料を開く経路を GUI で検証（`REF:src/ui/widgets/reference_pane.rs`, `REF:tests/gui.rs`）。
- **直近の追加作業**: S-07 は URL 外部リンクを資料フォームと参照ペインに設置し、24px 当たり判定・下線・青ホバー・ブラウザへの open_url を GUI テストで確認（`REF:src/ui/widgets/mod.rs`, `REF:tests/gui.rs`）。
- **直近の追加作業**: F-21/F-22/S-04 の文脈ビュー節間横線・4階層節ID・空本文の3行ガイドと F-28 の参照ペイン各セクション間の 1px 横線、F-29 の静的ラベル非選択化、F-39 の未保存ドット直径 10px を GUI テストで確認。F-30/F-32 は共通アクションボタン・選択アクション・リンク・ステータスラベル・各一覧行・Web 選択画面に下線、ホバー背景、ポインターと最小 24px 当たり判定を広げ、GUI で各共通部品を検証（`REF:src/ui/widgets/mod.rs`, `REF:src/ui/workspaces/mod.rs`, `REF:src/ui/web.rs`, `REF:tests/gui.rs`）。全クリック対象の網羅的な画面検査は継続する。Web スモークは PR #17 の sandbox-tools blob を使用して成功。
- **直近の追加作業**: F-35 の一覧専用 `↑`/`↓` を plot-episode / plot-outline / idea / material / cast に追加。編集中の Slice を移動・保存せず、Alt 矢印は従来どおり確定して移動する。1,000 件のアイデア一覧スクロールと Cast タイムラインのキー競合を GUI テスト済み。その他の画面の実機確認は残る。
- **追加作業**: S-13 の `>agent ` トークン単位 Tab 補完を GUI テスト、F-36 の資料専用 `Ctrl+3` 上書きを廃止して `Tab` によるフォーム移動を GUI テスト、S-18 のロケール別最大行幅設定をテスト。S-12 は可視範囲外の節の高さを概算して Galley 構築を省く処理とユニットテストを追加（スクロール中の視覚検証は残る）。
- **追加作業**: F-17 は Chromium の実際の keydown listener に対し予約キー/代替キーの `preventDefault` を検証。F-01 は Bézier 曲線によるフォント非依存の月/半月を Web のダークテーマ画面で視覚確認、F-11 は CJK フォント優先順を検査。F-31 は資料フォームをフォーカスした状態での新規ショートカットとボタンを GUI テスト済み。F-36 は Web 上で資料画面の Ctrl+3・Tab を操作・撮影し、長いガイドが中央幅を押し広げて右フォームを画面外へ追いやる不具合を修正して可視性を再確認。
- **追加作業**: F-16 の話・章プロット新規作成、review 再実行、cast 走査、agent 総停止・委譲・実行/ドライラン/中止のボタンと GUI 操作テスト。
- **追加作業**: F-02 のテキスト/ガター描画クリップ、F-03 のキャレット、F-16 の資料/アイデア/エージェントのボタン化、F-17 の Web 代替キー、F-40 の一覧へ戻る 2 段導線と履歴 Delete、S-07 の出典リンク、S-14 の幅不足時のステータス省略。その他の画面や OS 別の視覚検証は残る。
- **留意点**: F-38 はアイデア 1,000 件で Alt+↓ / ↓ の 160 フレームを release ビルドで計測し、再測定時の最長約 6.94ms で 16ms 目標を通過。F-43 は 10 万字の節限定解析に加えて egui 画面全体の打鍵フレームを release テストで計測し、8 打鍵の最長 270µs で 16ms 上限を通過。CI bench ジョブにも追加した（実環境差の検証は継続）。S-01 は本文・章/話プロットの抽出、8,000 字上限・`{{examples}}` の整合、起票モーダル→ドライランの UI 経路をテスト済み。S-15 は 76 診断コードの日英の条件・具体例・対処の個別ページを生成し、CI で網羅性検査する。

---

## 3. 次セッション開始時の手順

1. `/home/user/build/env.sh` が存在しない場合（コンテナ再起動後）は次を実行する（詳細は `REF:docs/developers.md` §2.2, §6.2）:
   ```bash
   bash scripts/bootstrap-toolchain.sh
   git fetch origin <現在のブランチ名>
   git reset --mixed FETCH_HEAD
   ```
2. ドキュメント規則とテスト一式を確認する:
   ```bash
   . /home/user/build/env.sh && bash scripts/verify.sh
   ```
3. `REF:docs/plans.md#planned-fixes-and-features` の Phase 1〜Phase 6 に従い、`F-01`〜`F-44` および `S-01`〜`S-18` の実装を進め、完了した節の `docs/designs/*.md` の `状態:` を更新していく。
