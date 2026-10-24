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

- 作業ブランチ: `arena/cd84f800-novnow`（`main` の `c00cf2d` から分岐）。今回の目標は v0.1.0 リリース準備。公開・タグ作成・マージは未実施。ユーザーの指示により、準備変更をこの作業ブランチへコミット・push する。
- 段階①〜③・配布整備はすべて v0.1.0 向け。v1 到達前の形式非互換・v0→v1 移行機能を提供しない方針、GitFlow と main の版更新限定運用を反映した。
- Release に crates.io 公開と全ソース添付を追加。タグ実行は環境承認付きで全公開する。Pages は Astro を `/novnow/`、Trunk を `/novnow/app/` にまとめる。旧ルートの Service Worker は登録解除用に置換する。
- 検証: `REF:scripts/verify.sh` 全項目成功（GUI あり 1372 件・GUI なし 1186 件）、文書検査器の単体 38 件、Astro 208 ページ、リンク補正の Node テスト 2 件、生成 HTML のルート相対リンク検査が成功。Cargo のパッケージ対象一覧も確認した。GitHub Actions YAML とジョブ依存の構造検証は成功したが、actionlint の取得は通信制約で失敗した。
- 外部状態と公開前ゲートは `REF:docs/distribution.md` §8〜§10。公開用の 4 environment は未作成。Pages / secrets 設定は API 権限不足で未確認。ホスト CI、実際の cargo publish dry-run、各レジストリ公開、署名・公証、Pages 実デプロイと新配置のブラウザ E2E は未実施。
- コミットの `[ci skip]` とタグ実行・必須チェックの関係に注意する。初回本番を実行する前に、人間が外部設定・ブランチ保護・main へのリリース PR を確認する。

## 3. 継続作業の手順

1. ツールチェーン復旧とサンドボックス再起動時の Git 同期は `REF:docs/developers.md` §2.2 / §6.2 に従う。
2. PR の最新状態・チェックと作業ツリーを確認する。段階 ③（`REF:docs/plans.md#stage-3`）は H1〜H7 を実装済み。実装済みの段階②を未着手へ戻さない。
3. 変更後は `REF:scripts/verify.sh`、UI/Web 変更時は加えて `REF:scripts/web-smoke.sh` を実行する。
   ユーザー向け文書（`REF:docs/users/`）を直したら、サイトの手引きとアプリ内のガイドモードの段階も同じにする（`REF:docs/developers.md` §4.8）。設計の状態を変更した場合は文書検査の実測値に従い両文書の SYNC 表を更新する。
