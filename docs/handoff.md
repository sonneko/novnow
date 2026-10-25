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

- 作業ブランチ: `arena/35b388c5-novnow`。前のPR #31は通常merge済み。公開対象はmainの `25725ba6c01c9b8294c83ccfc3c58093004a4eaa`、タグ `v0.1.0` と可動 `v0` は同じコミットを指す。
- Release run 40159636238は全ジョブ成功。デスクトップ10・wheel5・VSIX1・全ソース1とSHA256SUMSを添付し、crates.io/TestPyPI/PyPIの公開も成功。本番Pages deployも成功。Marketplaceは延期（遅くともv1到達までに開始）。Windows/macOSの配布元署名・macOS公証は行わない。
- 公開後のComposite Action Smoke run 40161112987はLinux成功、macOS/Windows失敗。**全形態が正常という判定ではない**。不変のv0.1.0タグ/成果物は変更しない。リリースノートに失敗を明記した。
- 公開物再取得の監査run 40161414239: 17成果物全件のSHA256SUMS/構造/版、ソースtarと版タグの内容一致、両Python registryの5wheelのhash一致、両registryからの実install、公開crates.ioからのcargo installとCLI実行、本番Pages/WASM取得は成功。
- サンドボックスでも実PyPI wheelを隔離venvへinstallし、実API pytest 9件成功・1件skip（150.48秒）。ネットワーク制約でGitHub release-assetsとcrates.ioへ直接接続できない部分は、公開物を取得するread-only CIで検証した。
- Actions失敗の診断: macOSは標準Bash 3.2+nounsetで空配列展開がエラー。WindowsはWindows式パスのバックスラッシュでsha256sumが出力先頭をエスケープし、hash比較が誤判定する。公開ZIPのhash自体は監査で正常。前者は常に非空のargv、後者はstdin経由のhash計算で修正候補を準備した。
- `REF:.github/workflows/release-audit.yml` は既存公開物の検証と**未公開の修正候補**の3 OS実行を分ける。`REF:scripts/action/test_install.py` の回帰5件を `REF:scripts/verify.sh` へ追加。修正候補の最終CI結果はこのブランチのChecksを確認する。
- v0.1.1等の追加公開・版更新・mainへのマージ・タグ変更は未承認。修正候補が成功しても公開済みv0.1.0が直ったとは言わない。利用者向けの「未提供」解除は、部分失敗の方針を所有者と確認してから行う。

## 3. 継続作業の手順

1. ツールチェーン復旧とサンドボックス再起動時の Git 同期は `REF:docs/developers.md` §2.2 / §6.2 に従う。
2. PR の最新状態・チェックと作業ツリーを確認する。段階 ③（`REF:docs/plans.md#stage-3`）は H1〜H7 を実装済み。実装済みの段階②を未着手へ戻さない。
3. 変更後は `REF:scripts/verify.sh`、UI/Web 変更時は加えて `REF:scripts/web-smoke.sh` を実行する。
   ユーザー向け文書（`REF:docs/users/`）を直したら、サイトの手引きとアプリ内のガイドモードの段階も同じにする（`REF:docs/developers.md` §4.8）。設計の状態を変更した場合は文書検査の実測値に従い両文書の SYNC 表を更新する。
