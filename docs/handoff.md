# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 123 節 / 実装中 0 節 / 未実装・実装計画済み 8 節 / 未実装・未計画 1 節 / 実装待ち 2 節 | 全 11 設計書（`docs/designs/*.md`）の `### ` 節（合計 134 節） | `REF:docs/plans.md#planned-fixes-and-features` |
| 仕様変更・修正（F-01〜F-44） | 実装完了（44 件） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装完了（18 件） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 未実装・未計画 | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 未着手（計画待機） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
| 段階 ③（§19 外部インターフェース） | 仕様確定・実装未着手（H0 完了） | Rust API・Python API・LSP・MCP 拡張・CLI 拡張による GUI 全機能のヘッドレス公開 | `REF:docs/plans.md#stage-3` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- **現在のブランチ**: `arena/8981acff-novnow`（`main` = マージ済み PR #21 から分岐）。
- **PR**: 本セッションで作成（番号は作成直後のコミットで追記する）。内容は段階 ③ H0 — 下記「実施内容」のドキュメント一式のみで、ソースコードの変更はない。
- **実施内容**: `REF:docs/specifications/interfaces.md` を新設した（§19.1〜§19.8: Rust API・Python API・LSP・MCP サーバ拡張・CLI 拡張のヘッドレス全機能公開仕様。§19.7 の正規操作カタログは 117 行で、5 インターフェース間の例外と関門を行単位で定義する）。対になる `REF:docs/designs/interfaces.md`（8 節すべて「未実装・実装計画済み」）も新設した。`REF:docs/plans.md` には §5（マイルストーン H0〜H7・受け入れ基準・PyO3 vendoring 手順・テスト戦略・リスク表）を追加した。規則側は `REF:docs/developers.md` §1.1 のファイル対を 11 対へ、§3 ツリーのファイル数を 11 へ改め、`REF:docs/script/duplicate.py` の末尾カウント表示を実ファイル数から動的に生成するようにした。
- **検証**: `python3 docs/script/duplicate.py` はグリーン。ドキュメント専用セッションのため cargo 系検証は走らせていない（`src/`・`tests/`・`Cargo.toml`・`Cargo.lock` はいずれも無変更）。
- **繰越事項**: 前段階の 62 項目（F-01〜F-44・S-01〜S-18、PR #18 → #21 でマージ済み）は完了。macOS と Windows の実機での操作確認、異なる性能環境での測定は未実施のまま（変更なし）。
- **次の作業**: PR マージ後、`REF:docs/plans.md#stage-3` に従い H1（app 層の抽出: 状態機械の `src/app/` への移設と層規則の追加）から着手する。

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
3. `REF:docs/plans.md#stage-3` に従い、段階 ③ を H1（app 層の抽出）から始める。マイルストーン完了ごとに `REF:docs/designs/interfaces.md` の `状態:` 行を「実装中」→「実装完了」へ進め、本書と `REF:docs/plans.md` の SYNC 進捗表（集計行）を同時に更新する。
