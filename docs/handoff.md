# セッション引き継ぎメモ（Handoff）

本書は開発を始めるにあたって必要な最小限の流動的情報（現在の進捗、作業ブランチ、直近の検証状態）を記録する。毎セッション終了時に更新する。
固定の開発環境構築手順・ドキュメント管理規則・ArenaAgent の仕様・技術的注意点は `REF:docs/developers.md`、今後の実装計画は `REF:docs/plans.md`、完了済みの履歴は `REF:docs/history.md` を参照すること。

---

## 1. 現在の進捗サマリ

<!-- SYNC:START stage1-status -->
| 区分 | 状態 | 対象範囲 | 参照 |
|---|---|---|---|
| 段階 ①（M0〜M11） | 完了 | コア仕様・全 9 ワークスペース・MCP・CLI・Web 版・ベンチマーク・サンプル一式 | `REF:docs/history.md` |
| 設計書セクション実装状況 | 実装完了 142 節 / 実装中 0 節 / 未実装・実装計画済み 5 節 / 未実装・未計画 0 節 / 実装待ち 0 節 | 全 13 設計書の `### ` 節（合計 147 節） | `REF:docs/designs/` |
| 仕様変更・修正（F-01〜F-44） | 実装完了（44 件） | UI・エディタ・パレット・辞書・ショートカット・性能等の修正 44 項目 | `REF:docs/plans.md#planned-fixes` |
| 機能追加（小規模：S-01〜S-18） | 実装完了（18 件） | エージェント例指定・PWA・Zip 導入・形態素波線等の小規模機能追加 18 項目 | `REF:docs/plans.md#planned-small-features` |
| 機能追加（比較的大規模：L-01） | 実装済み（CLI・デスクトップ） | 既存の本文テキストから `novnow` に移行するための手段の提供 | `REF:docs/plans.md#unplanned-large` |
| 段階 ②（追加仕様 1〜27） | 実装済み・検証完了（#1〜27） | 予約済みの追加機能 27 項目（10 個の縫い代で受け入れ準備済み） | `REF:docs/plans.md#stage-2` |
| 段階 ③（§19 外部インターフェース） | 実装済み・検証完了（H1〜H7。Python 拡張もサンドボックスでビルドし pytest まで通る。正式な wheel 検証は CI のホイール行列 `REF:.github/workflows/wheels.yml`。`REF:docs/plans.md` §5.5・§5.6） | Rust API・Python API・LSP・MCP 拡張・CLI 拡張による GUI 全機能のヘッドレス公開 | `REF:docs/plans.md#stage-3` |
| 配布（D-01〜D-20） | v0.1.0公開済み。既知の問題は公開版一覧で管理 | 5 形態・17 成果物とSHA256SUMS、公開・検査ワークフロー | `REF:docs/plans.md#distribution` |
| 第 2 の画面実装（仕様 §21） | 計画確定・未着手（W1 の文書整備だけ完了） | Svelte + TypeScript の画面と Tauri のネイティブ実行を、第 1 の画面と並行して作る | `REF:docs/plans.md#webui` |
<!-- SYNC:END stage1-status -->

---

## 2. 現在のブランチ・PR・直近の作業状態

- 2026-11-14（第 2 の画面実装の並列開発戦略）: 複数エージェントが同時に W2〜W7 を進めても衝突が小さくなるよう、レーン分割（L0 基盤・L1〜L5 の画面グループ・L6 Tauri・L7 配線）と共有ファイルの所有者表を `REF:docs/plans.md#webui-parallel` に追記した。要点は「1 ファイル 1 所有者」「生成物は生成器だけが書く」「共有ファイルは小さな PR で先に着地」。W2 は全レーンの前提なので 1 本で仕上げる。
- 2026-11-14（第 2 の画面実装の計画）: 所有者指示により、画面を Svelte + TypeScript で作り、Tauri でネイティブ化する第 2 の系統を、egui の第 1 と**並行**して作る方針を仕様・設計・計画に起こした。仕様 §21（`REF:docs/specifications/webui.md`）と設計 §21（`REF:docs/designs/webui.md`）を新設し、計画は `REF:docs/plans.md#webui`（W1〜W7）。既存の記述は削除せず追記のみ。文書だけの変更で、`src/` には触れていない（W1 の完了分）。反映先は `REF:docs/developers.md` §1.1（13 ファイル対）・§2.1（Node / wasm-pack / Tauri の前提）・§3（`crates/`・`webui/` の予定ツリー）、SYNC 表（13 設計書・147 節）、`REF:docs/script/reference.py` の章番号表と `REF:docs/script/test_reference.py`、`REF:scripts/docs-site.py` の並び順、README（日・英）・`REF:GUIDE.md`・サイトの「13 冊」表記。
- 2026-11-10（my-novels 用 nightly 更新スクリプト）: コピーして単独実行できる `REF:scripts/update-novnow-nightly.py` と手順 `REF:scripts/update-novnow-nightly.md` を追加。Linux x64 の VSIX・nogui を asset ID 指定で取得し、照合・導入後に current を切り替える。利用側の設定ファイルは上書きしない。オフライン回帰 `REF:docs/script/test_nightly_update.py` 7 件成功。

- 2026-11-10（nightly 配布経路）: 所有者承認の 3 成果物・gh-pages 共存方式を実装。ローカル一括検証（Rust 1,480 件）・Web smoke・サイト検査成功。VSIX の取得・版管理は対象外。現行仕様は `REF:docs/specifications/external.md` §20.6.4、公開環境の要設定事項は `REF:docs/distribution.md` §11。

- 2026-11-10（Windows LSP URI 修正）: `REF:src/lsp/methods.rs` でネイティブパスの区切り・ドライブ文字・verbatim prefix と file URI の表記差を正規化。`REF:tests/lsp.rs` に Windows の本文取得・トークン生成と UNC 往復・ルート境界の回帰を追加。`REF:.github/workflows/vscode.yml` は develop 向けでも Windows Electron を実行する。

- 2026-11-10（PR #51 マージ前検証）: 最新 develop を取り込み、CI を起動。Electron 回帰のサンプル参照先を実在する本文へ修正（`REF:vscode/src/test/suite/extension.test.ts`）。手動 dispatch は権限不足のため、PR 更新による起動を使用。

- 2026-11-10（VSCode Semantic Token 修正）: `REF:vscode/package.json` に novnow 限定の有効化と全トークンのテーマスコープを追加。`REF:src/lsp/methods.rs` は同一行の差分を前トークンの開始位置基準へ修正し、LSP の種別名のドットをハイフンへ変換（VSCode の修飾子区切りとの衝突回避、コア API は不変）。`REF:vscode/syntaxes/novnow.tmLanguage.json` は辞書依存の参照終端を推測せずシジルのみをフォールバック着色する。回帰は `REF:tests/lsp.rs`・`REF:vscode/test/highlighting.test.cjs`・`REF:vscode/src/test/suite/extension.test.ts`。検証: `REF:scripts/verify.sh` 全項目成功（Rust 1,472 passed）、拡張 check・単体 8 件・Electron テストの型検査成功。Electron 実行は VSCode 取得先が通信許可対象外のため未実施。

- PR #38（X1・X3・CI スコープ・CI 修正 2 件）は 2026-10-27 に所有者が `develop` へマージ済み（merge commit `6b5853b`。ruleset `protect-develop` の必須チェック 11 件へは所有者が同日更新済み）。
- 2026-10-27（CI スコープ）: `browsers (webkit)` の偽陽性で Checks が赤くなった件をきっかけに、各プラットフォームのビルドを最小化した（所有者指示: 開発速度を第一に）。`CI_SCOPE` で対象ブランチごとに行列を切り替え、develop 向け PR は 1 プラットフォーム（Linux x64・desktop nogui / wheel / chromium / ubuntu Electron / Python 3.9・3.13）、main 向け PR・main push・手動実行（`scope` 入力、既定 full）は full 行列。正本は `REF:docs/developers.md` §4.7 と `REF:docs/distribution.md` §8。
- 2026-10-27（CI 不整合の修正）: マージ commit の実測で develop への push が必須 11 件のうち平台 2 行（wheels・desktop の Linux x64）を報告しないことが分かったので、wheels / desktop-check に `develop` への push トリガーを追加（wheels は PR 同一 `paths`）、4 ワークフローの手動実行に `scope` 入力を追加した。**PR #39**（develop 向け、`[ci run]` 付き）として提出済み。マージは所有者が行う。マージ後の develop push で発火を確認すること。
- 2026-10-27（X4 文書の文体見直し）: 利用者向け文書 10 本と README（日・英）の文体を見直し、README の事実誤記 3 件（診断 76→77 種・英語版の正規操作 119→120・拡張 `.txt`→`.cnov`）と `errors.md` の `E025` 節（B10 前の JSON 前提だった）を直し、`REF:docs/designs/platform.md` §14.1 の X1 注記の stale な「未実装・実装計画済み」を「実装完了」へ直した。doc-only のため doc-site 生成物（guide HTML・website content）を同 PR に含める。PR #39 に同梱してマージ済み（`cf6385c`）。
- 2026-10-27（Issue #29 第 2 ラウンド: 棚卸し＋執筆支援の計画）: unchecked item 38 件を全部実装・仕様・コードと突き合わせて棚卸しした（`REF:docs/plans.md` §8.2 G12）。32 件は解決済みでチェックボックスが stale だった（実装済み 29 件・決定で閉じ 3 件）。Issue 本文へのチェックと根拠コメントは所有者の判断で代理編集するはずだったが、エージェントのトークンに Issue 書き込み権限が無い（HTTP 403）ため、チェック済みの本文と各チェックの根拠を PR 本文に添付して所有者が反映する形にした。待っている 5 件: N29-07（実機確認とリリース待ち）・X2（所有者の短編）・X5（トークン後）・N29-38（実機網羅チェックリスト）・執筆支援（→ B11 に計画済み）。執筆支援ラウンド 2 は W097（文末尾の反復）・W098（場面内の同語反復）の 2 件の決定論診断を計画した（新規依存なし・既定無効。仕様 `REF:docs/specifications/diagnostics.md` §7.2.3 / §7.2.4・`REF:docs/specifications/ui.md` §9.4.4・設計書 `REF:docs/designs/diagnostics.md` §7.2（§7.2.3 / §7.2.4 を新設・状態は未実装・実装計画済み）・実装計画 B11）。要らないファイルの最終スキャン（N29-27 残）は不要 0 件で完了。
- 2026-10-27（Issue #29 第 3 ラウンド: 執筆支援 W099–W106 の計画）: 所有者が TinySegmenter（`tiniestsegmenter`）の導入済みを見直し指摘 → 「固定モデルは見送り」の誤記 4 箇所（plans / designs / history）を訂正した。副発見: W087 の仕様（文字種抽出・「形態素解析なし」）と実装（`tiniestsegmenter` の 2 字以上トークン・動作テストなし）が食い違っていた → 所有者が**実装を規範**に決定（第 16 回決定の許容範囲・§6.2.6 が既に同じエンジンを規範化）し、`ui.md` §9.4.4 の規範テキストを直した。W097 の文末尾も同セッションで「句点直前 2 字」から「末尾の助詞を除いた最後の自立語」に細分（「静かに。ちかちかに。」の偽陽性解消）。新規診断 **W099–W106 の 8 件を全採用**（所有者決定、N29-46）: W099 隣接同語反復・W100 文末助詞の反復・W101 文頭語の反復・W102 波ダッシュ多用・W103 やわらか・不確か表現の多用（固定 8 語）・W104 同一動詞語幹の反復（固定の活用接尾辞リスト）・W105 感嘆符の多用・W106 ダッシュの多用。全て決定論・新規依存なし（語抽出は導入済みの `tiniestsegmenter`）・既定無効。規範テキストは `REF:docs/specifications/diagnostics.md` §7.2.5–7.2.12（新設）・`REF:docs/specifications/ui.md` §9.4.4（8 行追加）・`REF:docs/designs/diagnostics.md` §7.2.5–7.2.12（新設・状態は B11 / B12 まで未実装・実装計画済み）。実装計画は B12（§8.5）。後追い（表行 8 件・カタログ・サイト解説・トリガー表・「87 種」化）は B12 で同時。
- 2026-10-28（執筆支援ラウンド 2・3 の実装）: B11（W097 文末尾の反復・W098 場面内の同語反復）と B12（W099–W106）を実装し、診断は 77 種から **87 種** になった（設計書 142 節すべて実装完了）。語の抽出は導入済みの `tiniestsegmenter`（§6.2.6）と固定の語彙リスト。節単位の診断は節内の改行をまたいで数え（`SectionText` で本文 byte に戻す）、場面単位は場面の全節を集計する。実装中に分かった仕様の前提違い 2 件（セグメンタが活用・標点を別トークンに切ること）を仕様 §7.2.10・§9.4.4 にも反映した。全て組み込み既定は無効（`[features.diagnostics]` で有効化、文体タブの明示操作では有効化なしでも計算）。
- v0.1.0は公開済み。既存の版タグ・可動v0・配布バイナリは変更しない。Issue #29 の方針は「V1 延期、開発は V0 で継続」である。
- 既知の問題の原本は `REF:docs/known-issues/ja.md` と `REF:docs/known-issues/en.md`。サイトで直接表示し、修正版公開後に削除する運用は `REF:docs/developers.md` §9に従う。READMEに不具合本文を重複させない。
- 2026-11-06（性能改善 B13 の実装完了）: 5 つの既知の問題と調査で見つけた 4 件（執筆支援診断の全走査、UI の毎フレーム再パース、辞書供給元の多重パース、文字数の再計算）を B13-1〜B13-6 の 6 段で実装した。仕様書は変更していない。設計書 4 節（`REF:docs/designs/overview.md` §3.5・`REF:docs/designs/diagnostics.md` §7.0・`REF:docs/designs/semantics.md` §6.1・`REF:docs/designs/workflows.md` §18.1）に実装の形と実測を、`REF:docs/plans.md` §9.8 に実施記録と計画からの逸脱（`sections::all` / `metadata::records` の戻り値を `Arc<Vec<_>>` に変更、診断 4,000 件の目標を 12ms へ）を書いた。7 本のベンチは `REF:benches/core.rs`、キャッシュの差分一致と `Project: Send` は `REF:tests/project_cache.rs`。所有者の指示で途中レビューは省略し、最終レビューだけを挟む（マージは所有者）。
- 所有者指示: この作業は `develop` へマージすることを目指し、マージは所有者自身が行う。

## 3. 継続作業の手順

1. ツールチェーン復旧とサンドボックス再起動時の Git 同期は `REF:docs/developers.md` §2.2 / §6.2 に従う。
2. PR の最新状態・チェックと作業ツリーを確認する。段階 ③（`REF:docs/plans.md#stage-3`）は H1〜H7 を実装済み。実装済みの段階②を未着手へ戻さない。
3. 変更後は `REF:scripts/verify.sh`、UI/Web 変更時は加えて `REF:scripts/web-smoke.sh` を実行する。
   ユーザー向け文書（`REF:docs/users/`）を直したら、サイトの手引きとアプリ内のガイドモードの段階も同じにする（`REF:docs/developers.md` §4.8）。設計の状態を変更した場合は文書検査の実測値に従い両文書の SYNC 表を更新する。
