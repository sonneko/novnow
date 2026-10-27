# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 9 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 142 節 / 実装中 0 節 / 未実装・実装計画済み 0 節 / 未実装・未計画 0 節 / 実装待ち 0 節 | 全 12 設計書の `### ` 節（合計 142 節） | `REF:docs/designs/` |
| 仕様変更・修正（F-01〜F-44） | 実装完了（44 件） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装完了（18 件） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 実装済み（CLI・デスクトップ） | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 実装済み・検証完了（#1〜27） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
| 段階 ③（§19 外部インターフェース） | 実装済み・検証完了（H1〜H7。Python 拡張もサンドボックスでビルドし pytest まで通る。正式な wheel 検証は CI のホイール行列 `REF:.github/workflows/wheels.yml`。`REF:docs/plans.md` §5.5・§5.6） | Rust API・Python API・LSP・MCP 拡張・CLI 拡張による GUI 全機能のヘッドレス公開 | `REF:docs/plans.md#stage-3` |
| 配布（D-01〜D-20） | v0.1.0公開済み。既知の問題は公開版一覧で管理 | 5 形態・17 成果物とSHA256SUMS、公開・検査ワークフロー | `REF:docs/plans.md#distribution` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- PR #38（X1・X3・CI スコープ・CI 修正 2 件）は 2026-10-27 に所有者が `develop` へマージ済み（merge commit `6b5853b`。ruleset `protect-develop` の必須チェック 11 件へは所有者が同日更新済み）。
- 2026-10-27（CI スコープ）: `browsers (webkit)` の偽陽性で Checks が赤くなった件をきっかけに、各プラットフォームのビルドを最小化した（所有者指示: 開発速度を第一に）。`CI_SCOPE` で対象ブランチごとに行列を切り替え、develop 向け PR は 1 プラットフォーム（Linux x64・desktop nogui / wheel / chromium / ubuntu Electron / Python 3.9・3.13）、main 向け PR・main push・手動実行（`scope` 入力、既定 full）は full 行列。正本は `REF:docs/developers.md` §4.7 と `REF:docs/distribution.md` §8。
- 2026-10-27（CI 不整合の修正）: マージ commit の実測で develop への push が必須 11 件のうち平台 2 行（wheels・desktop の Linux x64）を報告しないことが分かったので、wheels / desktop-check に `develop` への push トリガーを追加（wheels は PR 同一 `paths`）、4 ワークフローの手動実行に `scope` 入力を追加した。Issue #29 の残る後続は X2（所有者の短編）・X4（文書文体）・X5（両市場の初回公開、トークン後）。
- v0.1.0は公開済み。既存の版タグ・可動v0・配布バイナリは変更しない。Issue #29 の方針は「V1 延期、開発は V0 で継続」である。
- 既知の問題の原本は `REF:docs/known-issues/ja.md` と `REF:docs/known-issues/en.md`。サイトで直接表示し、修正版公開後に削除する運用は `REF:docs/developers.md` §9に従う。READMEに不具合本文を重複させない。
- 所有者指示: この作業は `develop` へマージすることを目指し、マージは所有者自身が行う。

## 3. 継続作業の手順

1. ツールチェーン復旧とサンドボックス再起動時の Git 同期は `REF:docs/developers.md` §2.2 / §6.2 に従う。
2. PR の最新状態・チェックと作業ツリーを確認する。段階 ③（`REF:docs/plans.md#stage-3`）は H1〜H7 を実装済み。実装済みの段階②を未着手へ戻さない。
3. 変更後は `REF:scripts/verify.sh`、UI/Web 変更時は加えて `REF:scripts/web-smoke.sh` を実行する。
   ユーザー向け文書（`REF:docs/users/`）を直したら、サイトの手引きとアプリ内のガイドモードの段階も同じにする（`REF:docs/developers.md` §4.8）。設計の状態を変更した場合は文書検査の実測値に従い両文書の SYNC 表を更新する。
