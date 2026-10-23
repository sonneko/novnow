# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 134 節 / 実装中 0 節 / 未実装・実装計画済み 0 節 / 未実装・未計画 0 節 / 実装待ち 0 節 | 全 11 設計書の `### ` 節（合計 134 節） | `REF:docs/designs/` |
| 仕様変更・修正（F-01〜F-44） | 実装完了（44 件） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装完了（18 件） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 実装済み（CLI・デスクトップ） | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 実装済み・検証完了（#1〜27） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
| 段階 ③（§19 外部インターフェース） | 実装済み・検証完了（H1〜H7。Python 拡張もサンドボックスでビルドし pytest まで通る。正式な wheel 検証は CI のホイール行列 `REF:.github/workflows/wheels.yml`。`REF:docs/plans.md` §5.5・§5.6） | Rust API・Python API・LSP・MCP 拡張・CLI 拡張による GUI 全機能のヘッドレス公開 | `REF:docs/plans.md#stage-3` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- 作業ブランチ: `arena/10a83ca3-novnow`。PR #25 は **Draft**（CI を走らせないため）。マージと Ready 化は人間が行う。
- 今回は段階 ③（§19 外部インターフェース）の H1〜H7 を実装した。実装の記録は `REF:docs/history.md` の STAGE-3、受け入れ基準の達成状況は `REF:docs/plans.md#stage-3` §5.4 を参照。L-01・段階②の検証は L01-STAGE2-COMPLETE、統合文書の修正理由は DOCS-MERGE-STAGE23。
- 未検証のまま残るのは Python 拡張のビルドと pytest（サンドボックスから crates.io に到達できない）。初回の検証は CI のホイール行列 `REF:.github/workflows/wheels.yml`（5 構成 × Python 3.9 / 3.11 / 3.13）が行う。pyo3 系クレートの vendoring が必要になったら `REF:docs/add-dependencies.md` §2.2 と `REF:docs/plans.md#stage-3` §5.5 に従う（人間の承認が前提）。
- macOS / Windows の実機操作と、異なる性能環境での測定は未実施。CI のクロスビルドやサンドボックスでの測定とは区別する。

## 3. 継続作業の手順

1. ツールチェーン復旧とサンドボックス再起動時の Git 同期は `REF:docs/developers.md` §2.2 / §6.2 に従う。
2. PR の最新状態・チェックと作業ツリーを確認する。段階 ③（`REF:docs/plans.md#stage-3`）は H1〜H7 を実装済み。実装済みの段階②を未着手へ戻さない。
3. 変更後は `REF:scripts/verify.sh`、UI/Web 変更時は加えて `REF:scripts/web-smoke.sh` を実行する。設計の状態を変更した場合は文書検査の実測値に従い両文書の SYNC 表を更新する。
