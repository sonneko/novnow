# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 91 節 / 実装中 31 節 / 未実装・実装計画済み 1 節 / 未実装・未計画 1 節 / 実装待ち 2 節 | 全 10 設計書（`docs/designs/*.md`）の `### ` 節（合計 126 節） | `REF:docs/plans.md#planned-fixes-and-features` |
| 仕様変更・修正（F-01〜F-44） | 実装中（F-10, F-23, F-24, F-42 は既に完了） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装中（S-09 は既に完了） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 未実装・未計画 | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 未着手（計画待機） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- **現在のブランチ**: `arena/75c027a6-novnow`。
- **PR**: PR #18（Draft、base は前セッションの Draft PR #17 の作業ブランチ）。
- **現在の状態**: `REF:docs/plans.md#planned-fixes-and-features` の 62 項目を実装中。実装中という表示は達成を意味しない。未着手・未完了項目（特に F-16 の残り, F-17 の OS 別検証, F-35 の全ワークスペースのフォーカス/一覧/タイムライン動作, F-36 の実機操作, F-43 の画面全体の打鍵計測）と横断的な検査が残っている。
- **本ブランチの作業**: `REF:src/ui/app.rs` ほかのパレット・共通シェル・参照ペイン、`REF:src/cli/import.rs` の CLI Zip 導入、`REF:src/adapters/storage_idb.rs` の Web 同名制限、`REF:src/core/aicontext/packet.rs` の参考例、`REF:sw.js` の PWA、`REF:website/src/data/diagnostics.ts` の診断一覧を実装中。
- **検証**: `bash scripts/verify.sh` は 1174 件のテストで全ジョブ成功。`REF:scripts/web-smoke.sh` は Web の既存シナリオと PWA のオフライン再起動を含め成功。幅 620px の画面を撮り、F-02/F-03 のキャレット・本文の収まりと S-14 の右端の未保存マークを視覚確認（`REF:scripts/web-smoke.mjs` の `02b` / `02c`）。。`npm run build`（`website/`）も成功。
- **直近の追加作業**: F-35 の一覧専用 `↑`/`↓` を plot-episode / plot-outline / idea / material / cast に追加。編集中の Slice を移動・保存せず、Alt 矢印は従来どおり確定して移動する。各画面のフォーカスと長い一覧のスクロールは実機確認が残る。
- **追加作業**: S-13 の `>agent ` トークン単位 Tab 補完を GUI テスト、F-36 の資料専用 `Ctrl+3` 上書きを廃止して `Tab` によるフォーム移動を GUI テスト、S-18 のロケール別最大行幅設定をテスト。S-12 は可視範囲外の節の高さを概算して Galley 構築を省く処理とユニットテストを追加（スクロール中の視覚検証は残る）。
- **追加作業**: F-02 のテキスト/ガター描画クリップ、F-03 のキャレット、F-16 の資料/アイデア/エージェントのボタン化、F-17 の Web 代替キー、F-40 の一覧へ戻る 2 段導線と履歴 Delete、S-07 の出典リンク、S-14 の幅不足時のステータス省略。その他の画面や OS 別の視覚検証は残る。
- **留意点**: F-43 は 10 万字の節単位解析の予算テストのみで、egui の 1 打鍵全体時間の測定はまだない。S-01 は参考例を上限 8,000 字に切り、パケット上限で切り詰めたときの `{{examples}}` との整合をテスト済み。章プロットの抽出と UI 経路は追加確認が必要。S-07 は資料フォームと参照ペインの URL リンクを追加済み。S-15 の英語サイトでは仕様表の「発生条件」「対応」は日本語のまま。完了と扱わないこと。

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
