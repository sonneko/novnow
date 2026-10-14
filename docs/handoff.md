# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 8 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 89 節 / 実装中 0 節 / 未実装・実装計画済み 34 節 / 未実装・未計画 1 節 / 実装待ち 2 節 | 全 10 設計書（`docs/designs/*.md`）の `### ` 節（合計 126 節） | `REF:docs/plans.md#planned-fixes-and-features` |
| 仕様変更・修正（F-01〜F-44） | 一部実装完了（F-10, F-23, F-24） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 一部実装完了（S-09） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 未実装・未計画 | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 未着手（計画待機） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- **現在のブランチ**: `arena/0bccdaf9-novnow`
- **オープン中の PR**: PR #17（Draft、base は `main`）
- **本セッションで完了した作業**（Phase ① の 4 件と、その土台作り）:
  1. **形態素解析クレート `tiniestsegmenter` 0.3.0 の導入**（`Cargo.toml` / `Cargo.lock`、隔離環境向けの `REF:scripts/bootstrap-toolchain.sh` と CI の退避ジョブ）。
  2. **`S-09` 未知の `'` 参照の形態素境界と `F-24` 登場人物名の重複抑止**（`REF:src/core/resolve/dictionary.rs`、`REF:src/core/features/completion.rs`、`REF:src/ui/palette.rs`。`E001` のスパンを 1 形態素に限定し、`characters.md` と同名の `words.md` 見出しを一覧から除外）。
  3. **`F-23` 参照ペインの診断からの自動回復**（`REF:src/ui/widgets/reference_pane.rs` の登録ボタンと `REF:src/ui/app.rs` の `register_entry`、`REF:tests/workspaces.rs` のクリックテスト）。
  4. **`F-10` ペースト時の改行保持と `;` / `@A-B-C` による自動節分割**（`REF:src/core/features/sections.rs` の `split_pasted_sections`、`REF:src/ui/workbench.rs` の `paste`、`REF:src/ui/widgets/section_editor.rs` の `apply_paste`）。
  5. **ドキュメント検査 `rule-3` の赤の解消**（`REF:docs/developers.md` §1 の原文を正として `REF:docs/script/duplicate.py` の `VERBATIM_RULES_BLOCK` を揃えた。経緯は `REF:docs/history.md` の `D-017`）。
- **検証状態**: `bash scripts/verify.sh` が全ジョブ緑（docs / fmt / clippy / clippy-ui / test / test-cli / wasm、1150 件超のテストが成功）。
- **次に着手する項目**: Phase ① の残り `F-43`（打鍵時の編集中節のみの同期解析）、`S-08`（`」@` の補完）、`F-42`（`ja.toml` の助詞直前空白）。

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
3. `REF:docs/plans.md#planned-fixes-and-features` の Phase 1〜Phase 6 に従い、`F-01`〜`F-44` および `S-01`〜`S-18` の実装を進め、完了した節の `docs/designs/*.md` の `状態:` を更新していく。
