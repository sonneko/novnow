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
| 配布（D-01〜D-20） | 実装済み。ブラウザ版以外は初回リリースまで未提供 | 5 形態・18 通りの成果物、VSCode 拡張、複合アクション、公開・検査ワークフロー | `REF:docs/plans.md#distribution` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- 作業ブランチ: `arena/b97c5fbe-novnow`（`main` の `38c6244` から分岐）。配布の D-01〜D-20 を実装し、PR #28 の検証を進める。マージは人間が行う。
- 段階 ①〜③（M0〜M11・追加仕様 1〜27・L-01・§19 の H1〜H7）は `main` にマージ済み（PR #25 は 2026-10-23 にマージ）。
- 直近の作業: 配布ワークフローへ対象別 Rust キャッシュと `sccache` を追加し、manylinux コンテナの選択、ARM64 ランナーでの Intel macOS ホイール検査、VSCode Electron テストのモジュール境界を修正した。続くホスト検証で見つかった Python 3.9 の版読み取りと Electron の LSP 再起動待機も直した（`REF:.github/workflows/wheels.yml`・`REF:.github/workflows/release.yml`・`REF:vscode/src/extension.ts`・`REF:vscode/src/test/suite/extension.test.ts`）。
- **コミットは既定で `[ci skip]`**（`REF:.githooks/commit-msg`）。CI は必要なときだけ `bash REF:scripts/ci-run.sh ci watch` で回す。今回はユーザーの指示によりホストジョブを再実行しない。
- サンドボックスでの検証は全件通る: `REF:scripts/verify.sh` の全 12 ステップ、cargo test 1372 件（GUI あり）・1186 件（GUI なし）、clippy・fmt・wasm32。配布テスト 10 件、VSCode の型検査・単体テスト 6 件・VSIX 生成、実ホイールの版/タグ検査と pytest（8 件成功・1 件 skip）、Astro 208 ページも通る。Web 実機スモーク（`REF:scripts/web-smoke.sh`）も全項目成功。
- ホスト CI `40049130837` は全ジョブ成功。ホイール `40049130861` は 5 対象すべてのビルド・導入・import と Python 3.11 / 3.13 テストが成功し、Python 3.9 は検査コードの `tomllib` 使用で失敗。VSCode `40049130929` は型・単体・VSIX が成功し、Electron は失敗。3.9 非対応の版読み取りと Electron の再起動待機を修正済み。最新のホスト結果は PR #28 で確認する。
- 3 OS の Action スモーク、実リリース、署名・公証・公開は未実施。ローカル成功とホスト検証を区別する。

## 3. 継続作業の手順

1. ツールチェーン復旧とサンドボックス再起動時の Git 同期は `REF:docs/developers.md` §2.2 / §6.2 に従う。
2. PR の最新状態・チェックと作業ツリーを確認する。段階 ③（`REF:docs/plans.md#stage-3`）は H1〜H7 を実装済み。実装済みの段階②を未着手へ戻さない。
3. 変更後は `REF:scripts/verify.sh`、UI/Web 変更時は加えて `REF:scripts/web-smoke.sh` を実行する。
   ユーザー向け文書（`REF:docs/users/`）を直したら、サイトの手引きとアプリ内のガイドモードの段階も同じにする（`REF:docs/developers.md` §4.8）。設計の状態を変更した場合は文書検査の実測値に従い両文書の SYNC 表を更新する。
