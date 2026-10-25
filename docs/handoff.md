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

- 作業ブランチ: `arena/cd84f800-novnow`、PR #31。v0.1.0公開準備。公開・タグ作成・マージ・外部設定変更は行っていない。承認済みD-01〜D-06の実装を維持する。
- 現行方針はv0.1.0からの番号付き配布、v1前の非互換許容・v0→v1移行保証なし、GitFlow、mainは版更新限定。既存本文のimport/migrate機能の廃止とは異なる。統合Releaseは通常版だけ。VSIXはGitHub Releasesで配布し、Marketplace公開は遅くともv1到達までに開始する。Windows/macOSの配布元署名・macOS公証は行わない。Pythonの試験版はPEP 440等価表現で補助wheels→TestPyPIへ公開する。
- サイトはAstroを `/novnow/`、実Trunkを `/novnow/app/` に結合。末尾スラッシュは両表記を許可する。完全precacheと旧root workerの登録解除はIndexedDBを消さない。
- 2026-10-25のローカル `REF:scripts/verify.sh` は全項目成功（GUIあり1374件/なし1188件、文書単体52件）。VSCodeの型検査・Electron用コンパイル・Node単体6件も成功した。Marketplace/PATなしのローカルVSIX生成も成功（8 files）。
- 公開リポジトリでホストCIを実行した。通常CI全ジョブ、5ターゲットwheel作成とPython 3.9/3.11/3.13テスト、3 OSの実VSCode Electronテスト、実クレートのpublish dry-run、実Trunk/Astroの結合buildの成功を確認。macOS VSCodeのbundle実行ファイル名、Windowsのexe/UTF-8、macOS既定bashの空配列を修正した。
- `REF:.github/workflows/pages-check.yml` / `REF:scripts/pages-e2e.mjs` は結合サイト・言語切替・初回オフラインWASM・旧worker登録解除と作品保持・新scope分離を3エンジンで検査する。Chromium/WebKitのシナリオは成功し、Firefox用の非同期条件の再評価を修正した。最終SHAの結果はPR #31のChecks/対応コメントを確認する。Playwright WebKitは実Apple Safariそのものの動作保証ではない。
- `REF:.github/workflows/desktop-check.yml` / `REF:scripts/desktop-check.sh` は5ターゲット×GUI有無の10実アーカイブを作り、ライセンス・構造・展開後の実行ファイルの版とサンプル診断を検査する。署名・公証・GUI画面/IME/ファイルダイアログの実機検証とは区別する。公開処理はない。
- 外部状態は `REF:docs/distribution.md` §10.1。APIでmain/developのRestrict updates・linear history解除、mergeのみ許可、必要承認数0を確認した。必須チェックは現行名に一致し、公開用environmentのtag型/reviewerも確認済み。crates.io/PyPI/TestPyPIの設定は所有者確認済み（実公開成功とは別）。今回はVSCE_PAT不要。外部設定は変更していない。
- Firefox描画環境追補: 3ba7150の失敗ではconsoleにWebGL未対応、IDB履歴は空だった。保存不具合と断定できない。CIをXvfb上のheaded Firefox + Mesa llvmpipeへ固定し、rendererの事前確認とアプリ起動エラーの即時報告を追加。チェック削除や自動retryは行わず、新規profile5回の検証を維持する。
- ブラウザ追補: WebKitの独立permission fixtureにStorageManager不在のガードを追加。Firefoxの作品初期化はサンプルの重複名確認後の書き込みをIDBリクエスト成功イベント内へ移し、トランザクション寿命へのawait依存を除去した。Firefoxは新規profileで5回繰り返す。Desktop検査のGUI行にui feature指定漏れも修正し、以前の行列成功を実GUIビルドの保証に使わない。最終結果はPRを確認する。
- 未実施: 実公開・本番Pagesデプロイ・レジストリ認証の実動確認、対象実機でのGUI/IME/Zip往復。署名/公証は行わず、Marketplace公開は延期。テスト成功を公開完了と表現しない。

## 3. 継続作業の手順

1. ツールチェーン復旧とサンドボックス再起動時の Git 同期は `REF:docs/developers.md` §2.2 / §6.2 に従う。
2. PR の最新状態・チェックと作業ツリーを確認する。段階 ③（`REF:docs/plans.md#stage-3`）は H1〜H7 を実装済み。実装済みの段階②を未着手へ戻さない。
3. 変更後は `REF:scripts/verify.sh`、UI/Web 変更時は加えて `REF:scripts/web-smoke.sh` を実行する。
   ユーザー向け文書（`REF:docs/users/`）を直したら、サイトの手引きとアプリ内のガイドモードの段階も同じにする（`REF:docs/developers.md` §4.8）。設計の状態を変更した場合は文書検査の実測値に従い両文書の SYNC 表を更新する。
