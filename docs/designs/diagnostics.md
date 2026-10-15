# novnow 設計書 — 診断カタログ

本書は `REF:docs/specifications/diagnostics.md` の各節に対応する実装設計を記述する。

## 7. 診断カタログ

### 7.0 コア・構文・解決・整合の診断

状態: 実装完了（REF:src/core/diagnostics.rs, REF:src/core/features/project.rs, REF:tests/diagnostics_catalog.rs, REF:tests/diagnostics_document.rs）

- コア・構文・解決・整合の 39 コード（`E001`〜`E030`, `W001`〜`W015`, `I001`〜`I002`）は `DiagnosticCode`（`REF:src/core/diagnostics.rs`）に列挙し、`CATALOG` 定数で重大度・グループ・引数個数を管理する。
- 文書単位およびプロジェクト横断の診断収集は `core::features::diagnostics`（`REF:src/core/features/project.rs`）が担う。
- `REF:tests/diagnostics_catalog.rs` が `REF:docs/specifications/diagnostics.md` を `include_str!` で直接パースし、全コードの個数・重大度・引数プレースホルダ・variant の doc コメントの一致を機械的に検証する。
- **診断生成・整列・横断検査の内部規約（`REF:docs/history.md` の `A-22`, `B-25`〜`B-30`, `B-72`〜`B-77`）**:
  - `core` は診断メッセージ文字列や `Display` を持たず、`DiagnosticCode` と構造化引数 `Vec<String>` のみを返し、日本語メッセージへの整形は `ui::i18n`（`REF:src/ui/i18n/mod.rs`）が担う（`B-29`）。
  - `Diagnostic::range` はファイル先頭からの byte offset の半開区間とし、1 基点の `(line, col)`（`col` は UTF-8 文字数＝`chars().count() + 1`）への変換は `LineIndex::line_col` で行う（`B-28`）。
  - A 形式（`plots/all.txt` と `plots/AA.txt`）の両方に同じ話ブロック `@AA-BB` がある場合は、より詳細な章別ファイル `plots/AA.txt` を正とし、`E018` / `W002` の related 位置もそちらを指す（`B-72`）。
  - 会話の話者・役割語検査（`W003` / `W004` / `W005`）では、単一引用符 `'…'`（改行を跨がない閉じたペア）を地の文から除外した上で括弧の開閉を走査し、話者が特定された場合は全定義パターンとの不一致で `W004`、話者未指定の場合は `pov`（なければその節の `mode:` に登場するキャラ集合）を候補として `W003` / `W005` を判定する（`B-73`）。
  - 伏線グラフの `I001`（伏線と回収のペア成立）は伏線側・回収側の双方で参照でき、かつ診断一覧で件数が倍増しないよう、回収側に 1 件のみ出して `related` に伏線側の位置を入れる（`A-22`）。
  - `cli::check` の JSON 出力と text 出力はともに `(path, range.start, code)` の辞書順でソートして決定性を保証し（`B-76`）、`--fix` は安全フラグ付きの `QuickFix` のみを末尾側から自動適用する（`B-86`, `B-102`）。

### 7.1 ストレージ・エージェント系（文書位置を持たない通知）

状態: 実装完了（REF:src/core/diagnostics.rs, REF:src/adapters/storage_fs.rs, REF:src/adapters/storage_idb.rs, REF:tests/diagnostics_catalog.rs）

- ストレージ・エージェント系の 10 コード（`E060`〜`E063`, `W060`〜`W065`, `I050`〜`I052`）は `Anchor::Status`（文書位置を持たないステータスバー通知）として `STATUS_ANCHOR`（`REF:src/core/diagnostics.rs`）に定義する。
- 規則 10 違反（`E060`）、IndexedDB 利用不可（`E062`）、保存失敗（`E063`）、容量逼迫（`W063`）、資料変更検知（`W064` / `W065`）、プロンプト初期化（`I050`）などを各アダプタ・ワークスペースから発行する。
- `WritePolicy::check`（`REF:src/core/policy/write_policy.rs`）はデバッグビルドでは `debug_assert!` で即座にパニックし、リリースビルドでは `Err(WriteViolation::ForbiddenTarget)`（`E060`）を返してシステムコールを発行しない（`B-20`, `B-107`）。

### 7.2 執筆支援系の診断

状態: 未実装・実装計画済み（REF:src/core/diagnostics.rs, REF:src/core/features/project.rs, REF:tests/diagnostics_document.rs, REF:docs/plans.md#plan-S-15）

- 執筆支援系の 27 コード（`W080`〜`W099`, `E032`〜`E036`, `E064`, `I054`）は `Group::Writing` として `REF:src/core/diagnostics.rs` に定義する。
- 既定の有効（`TOGGLE_ON`: 11 件）/ 無効（`TOGGLE_OFF`: 7 件）および実行コスト制御テーブル（`COST`: 16 件の `Trigger` × `Scope`）は `REF:src/core/diagnostics.rs` に定数表として持ち、`REF:tests/diagnostics_catalog.rs` で仕様表との完全一致を検証する。
- 段階 ① 対象の執筆支援診断（`W081` 文長、`W082` 連続文末、`W083` 話者不明会話、`W084` 非定義視点、`W085` 助詞連続、`W086` 指示語過多、`W087` 漢字率、`W094` 台詞長、`W098` 表記揺れ、`E032`〜`E036` / `E064` / `I054` エージェント系）は `REF:src/core/features/project.rs` および `REF:src/core/agents/runner.rs` で実装済みであり、段階 ② の構文（`when:` / `senses:` / `space:` 等）に依存するコードは縫い代 3 に従いカタログ定義と設定トグルを保持している（`REF:docs/plans.md#stage-2`）。
- 執筆支援診断の算出規約（`REF:docs/history.md` の `B-78`, `B-79`）:
  - `W082`（連続文末）は段落内の文ごとに末尾の句点・感嘆符・閉じ括弧を除いた最後の 2 文字（ひらがな・カタカナの語尾）を比較し 3 文連続一致で警告する。
  - `W085`（助詞連続）は文内の平仮名 1 文字助詞（`の` / `が` / `を` / `に` / `で` / `と` / `は`）の出現間隔と回数を検査し、`W087`（漢字率）は段落単位（50 字以上）の CJK 統合漢字比率が `project.json` の範囲（既定 20%〜45%）を外れた場合に報告する。

- **[S-15] 全診断コードの Web サイト解説と CI 網羅検査（未実装・実装計画済み: `REF:docs/plans.md#plan-S-15`）**:
  - `CATALOG`（`REF:src/core/diagnostics.rs`）の全 76 コードそれぞれに対する原因・例・対処法の解説ドキュメントを整備し、`REF:tests/diagnostics_catalog.rs` および `REF:.github/workflows/ci.yml` で全コードの解説が存在することを機械検査する。
