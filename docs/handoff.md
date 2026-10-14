# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 86 節 / 実装中 0 節 / 未実装・実装計画済み 37 節 / 未実装・未計画 1 節 / 実装待ち 2 節 | 全 10 設計書（`docs/designs/*.md`）の `### ` 節（合計 126 節） | `REF:docs/plans.md#planned-fixes-and-features` |
| 仕様変更・修正（F-01〜F-44） | 未実装・実装計画済み | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 未実装・実装計画済み | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 未実装・未計画 | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 未着手（計画待機） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- **現在のブランチ**: `arena/96cfe43e-novnow`
- **オープン中の PR**: PR #16（Ready for review）
- **本セッションで完了した作業**:
  1. **M11 最終仕上げと v0→v1 移行機能の完全撤廃**（詳細は `REF:docs/history.md` の `M11` エントリ参照）。
  2. **ドキュメント再編成（`docs/` への移行）と GitHub Actions 最適化**。
  3. **新規の仕様変更・修正（44 件 `F-01`〜`F-44`）・小規模機能追加（18 件 `S-01`〜`S-18`）・大規模機能追加（1 件 `L-01`）の仕様書・設計書反映と実装計画策定**:
     - `docs/specifications/*.md` および `docs/designs/*.md` の該当節すべてに仕様・設計を追記し、`F-01`〜`F-44` と `S-01`〜`S-18` を「未実装・実装計画済み」、`L-01`（既存本文テキストからの移行手段）を「未実装・未計画」として記録。
     - `REF:docs/plans.md#planned-fixes-and-features` に Phase 1〜Phase 6 の具体的な実装計画を策定（コード実装は未着手）。

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
