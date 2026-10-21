# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 126 節 / 実装中 0 節 / 未実装・実装計画済み 0 節 / 未実装・未計画 0 節 / 実装待ち 0 節 | 全 10 設計書（`docs/designs/*.md`）の `### ` 節（合計 126 節） | `REF:docs/plans.md#planned-fixes-and-features` |
| 仕様変更・修正（F-01〜F-44） | 実装完了（44 件） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装完了（18 件） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 実装済み（CLI・デスクトップ） | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 実装済み（#1〜27・最終検証） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- 作業ブランチは `arena/acfdeb42-novnow`、PR は #23。マージは人間が行う。最新のレビュー状態と Actions の結果は GitHub を確認する。
- L-01 は CLI とデスクトップの入力選択・プレビュー・新規先限定作成を接続済み。Web のテキスト移行は初期範囲外。
- 段階②のコア・保存境界・UI を統合。機能別の正本と検収は `REF:docs/plans.md` §4.1 / §4.5、裁定履歴は `REF:docs/history.md` の L01-STAGE2。
- 最終統合中の検証: `REF:scripts/verify.sh` は 1267 件・56 ターゲット成功。release の打鍵・一覧の 16ms 条件と読者組版の 3ms 条件に成功。ブラウザで読者の原稿非変更、設定保存、画像のバイナリ Zip 往復に成功。
- 追加ブラウザ検証が発見した Alt+数字の文字イベント混入を修正し、`REF:tests/stage_two.rs` に回帰テストを追加。最終実行結果は PR の検証欄を参照。

## 3. 継続作業の手順

1. `REF:docs/developers.md` の手順でツールチェーンを復旧する。
2. 変更前に最新ブランチ差分と PR のチェックを確認する。ユーザーの全項目自律実装指示に基づく判断は履歴に記録済み。
3. 変更時は `REF:scripts/verify.sh`、UI/Web の変更時は加えて `REF:scripts/web-smoke.sh` を実行する。PR は自動マージしない。
