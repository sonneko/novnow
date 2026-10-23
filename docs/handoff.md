# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 9 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 134 節 / 実装中 0 節 / 未実装・実装計画済み 0 節 / 未実装・未計画 0 節 / 実装待ち 0 節 | 全 11 設計書の `### ` 節（合計 134 節） | `REF:docs/designs/` |
| 仕様変更・修正（F-01〜F-44） | 実装完了（44 件） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装完了（18 件） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 実装済み（CLI・デスクトップ） | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 実装済み・検証完了（#1〜27） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
| 段階 ③（§19 外部インターフェース） | 実装済み・検証完了（H1〜H7。Python 拡張もサンドボックスでビルドし pytest まで通る。正式な wheel 検証は CI のホイール行列 `REF:.github/workflows/wheels.yml`。`REF:docs/plans.md` §5.5・§5.6） | Rust API・Python API・LSP・MCP 拡張・CLI 拡張による GUI 全機能のヘッドレス公開 | `REF:docs/plans.md#stage-3` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- 作業ブランチ: `arena/06cb5373-novnow`（`main` の `8360f28` から分岐）。PR #26（文書と実装の同期是正と `REF:docs/script/reference.py` の追加）を作成済み。マージは人間が行う。
- 段階 ①〜③（M0〜M11・追加仕様 1〜27・L-01・§19 の H1〜H7）は `main` にマージ済み（PR #25 は 2026-10-23 にマージ）。
- 直近の作業: ドキュメントとコードの同期監査とその是正（`REF:docs/history.md` の DOCS-SYNC-AUDIT、検査は `REF:docs/developers.md` §4.6）、および Actions の節約（同 §4.7）。
- **コミットは既定で `[ci skip]`**（`REF:.githooks/commit-msg`）。CI は必要なときだけ `bash REF:scripts/ci-run.sh ci watch` で回す。マージ前に 1 回だけ回して緑を確認する。
- サンドボックスでの検証は全件通る: `docs 11 項目 / docs-tests 30 件 / docs-sync 10 項目 / site-check 2 件 / python-check 10 件 / 拡張の pytest 8 passed 1 skipped / cargo test 1176（GUI なし）・1347（GUI あり）/ clippy・fmt・wasm32 すべて ok`。Web 実機スモーク（`REF:scripts/web-smoke.sh`、ヘッドレス Chromium で 28 項目）も通る。
- macOS / Windows の実機操作と、異なる性能環境での測定は未実施。CI のクロスビルドやサンドボックスでの測定とは区別する。

## 3. 継続作業の手順

1. ツールチェーン復旧とサンドボックス再起動時の Git 同期は `REF:docs/developers.md` §2.2 / §6.2 に従う。
2. PR の最新状態・チェックと作業ツリーを確認する。段階 ③（`REF:docs/plans.md#stage-3`）は H1〜H7 を実装済み。実装済みの段階②を未着手へ戻さない。
3. 変更後は `REF:scripts/verify.sh`、UI/Web 変更時は加えて `REF:scripts/web-smoke.sh` を実行する。設計の状態を変更した場合は文書検査の実測値に従い両文書の SYNC 表を更新する。
