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

- 作業ブランチ: `arena/cd84f800-novnow`（`main` の `c00cf2d` から分岐）。今回の目標は v0.1.0 リリース準備。公開・タグ作成・マージは未実施。準備変更は `0dad09e` として push 済み。続く文書監査では、配布・API 設計と手順の実装追従漏れを訂正した。
- 段階①〜③・配布整備はすべて v0.1.0 向け。v1 到達前の形式非互換・v0→v1 移行機能を提供しない方針、GitFlow と main の版更新限定運用を反映した。
- Release に crates.io 公開と全ソース添付を追加。タグ実行は環境承認付きで全公開する。Pages は Astro を `/novnow/`、Trunk を `/novnow/app/` にまとめる。旧ルートの Service Worker は登録解除用に置換する。
- 検証: `REF:scripts/verify.sh` 全項目成功（GUI あり 1372 件・GUI なし 1186 件）、文書検査器の単体 46 件（サイト監査の回帰 5 件を追加）、Astro 208 ページ、リンク補正・設定の Node テスト 5 件、生成 HTML のルート相対リンク検査が成功。Cargo のパッケージ対象一覧も確認した。GitHub Actions YAML とジョブ依存の構造検証は成功したが、actionlint の取得は通信制約で失敗した。
- Web サイト追加監査: 製品版・互換性/移行説明・手引きアンカー・詳細記事の言語切替を訂正。静的 HTML 全 219 ファイルと CSS/画像を検査し、Astro 7.3.8 の型検査はエラー・警告・ヒント 0 件。`REF:website/README.md` に再現手順、PR と Pages 結合前後に検査を追加した。型検査・Chromium の追加パッケージはローカル検証用だけで、プロジェクト依存は追加していない。
- 監査範囲: 文書規約・参照/アンカー・SYNC・仕様/設計見出し・文書と実装の突き合わせ・生成ガイド/サイト・読みやすさ・配布手順とワークフロー・API/VSCode/Actions の入口を確認した。仕様の新設や互換性方針の変更はしていない。実サービス上の動作や全機能の散文の意味まで完全に保証する検査ではない。
- 外部状態と公開前ゲートは `REF:docs/distribution.md` §8〜§10。公開用の 4 environment は未作成。Pages / secrets 設定は API 権限不足で未確認。ホスト CI、実際の cargo publish dry-run、各レジストリ公開、署名・公証、Pages 実デプロイと Web アプリの新配置での E2E は未実施。公式サイトはローカルで本番相当 24 回・開発モード 8 回の Chromium 表示と実際の言語切替を確認済み（幅 1440 / 390）。
- 末尾スラッシュ修正: Astro の `trailingSlash` を `ignore` に変更し、canonical / hreflang のみ末尾 `/` に統一。209 経路 × `/` 有無 × preview/dev の 836 HTTP 要求がすべて 200。その後、承認された6領域の横断監査を実施。PyPI公開のテスト成功ゲート、Python/GIL/API例、導入手順、LSPヘルプの明白な誤りを修正した。
- 横断監査のD-01〜D-06はユーザー承認に基づき反映した。外部ライセンス文書・配布物取得専用領域の例外を仕様へ明示し、IPA許諾の添付/ヘルプ表示、初回の全資源precache、CLI規則統合、Python版のPEP 440表現、本番PyPI公開経路の一本化を実装した。詳細と検証はPR #31の対応コメントに集約する。
- 最新検証は全verify成功（GUIあり1373件、GUIなし1187件、文書単体52件）。実release WASMを手動wasm-bindgen builderで `/novnow/app/` 相当に配信し、新規profile/HTTP cache無効/追加オンラインreloadなし/サーバ接続遮断でもWASM再起動・作品保持、更新失敗時の旧cache保持と既存のUI/Zip/保存の全smokeを確認した。これはTrunkそのものとAstroを結合した本番Pagesの検証ではない。Trunk取得は配布assetへの接続制約で失敗し、post_buildフックの構造・同じsealerの実行を検証した。
- Python試験版はリポジトリ外の隔離コピーを0.1.0-rc.1として実wheelをbuildし、wheel名/METADATAの0.1.0rc1とimport後の製品版0.1.0-rc.1を確認した（linux_x86_64でありmanylinux適合性の検証ではない）。Marketplaceを含む全形態のプレリリース公開は未対応のため、統合Releaseは通常版以外を公開前に安全停止する。Marketplace向け試験版の対応規則は別途必要であり、承認済みの製品SemVerを別番号へ勝手に変えていない。
- 公開用environment/registry所有権・署名/公証・旧root workerの実データ付き移行・Firefox/Safari・各OS成果物・実デプロイは引き続き未検証。公開・タグ作成・マージ・外部設定変更は行っていない。
- コミットの `[ci skip]` とタグ実行・必須チェックの関係に注意する。初回本番を実行する前に、人間が外部設定・ブランチ保護・main へのリリース PR を確認する。

## 3. 継続作業の手順

1. ツールチェーン復旧とサンドボックス再起動時の Git 同期は `REF:docs/developers.md` §2.2 / §6.2 に従う。
2. PR の最新状態・チェックと作業ツリーを確認する。段階 ③（`REF:docs/plans.md#stage-3`）は H1〜H7 を実装済み。実装済みの段階②を未着手へ戻さない。
3. 変更後は `REF:scripts/verify.sh`、UI/Web 変更時は加えて `REF:scripts/web-smoke.sh` を実行する。
   ユーザー向け文書（`REF:docs/users/`）を直したら、サイトの手引きとアプリ内のガイドモードの段階も同じにする（`REF:docs/developers.md` §4.8）。設計の状態を変更した場合は文書検査の実測値に従い両文書の SYNC 表を更新する。
