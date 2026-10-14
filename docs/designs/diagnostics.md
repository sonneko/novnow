# novnow 設計書 — 診断カタログ

本書は `REF:docs/specifications/diagnostics.md` の各節に対応する実装設計を記述する。

## 7. 診断カタログ

### 7.0 コア・構文・解決・整合の診断

状態: 実装完了（REF:src/core/diagnostics.rs, REF:src/core/features/project.rs, REF:tests/diagnostics_catalog.rs, REF:tests/diagnostics_document.rs）

- コア・構文・解決・整合の 39 コード（`E001`〜`E030`, `W001`〜`W015`, `I001`〜`I002`）は `DiagnosticCode`（`REF:src/core/diagnostics.rs`）に列挙し、`CATALOG` 定数で重大度・グループ・引数個数を管理する。
- 文書単位およびプロジェクト横断の診断収集は `core::features::diagnostics`（`REF:src/core/features/project.rs`）が担う。
- `REF:tests/diagnostics_catalog.rs` が `REF:docs/specifications/diagnostics.md` を `include_str!` で直接パースし、全コードの個数・重大度・引数プレースホルダ・variant の doc コメントの一致を機械的に検証する。

### 7.1 ストレージ・エージェント系（文書位置を持たない通知）

状態: 実装完了（REF:src/core/diagnostics.rs, REF:src/adapters/storage_fs.rs, REF:src/adapters/storage_idb.rs, REF:tests/diagnostics_catalog.rs）

- ストレージ・エージェント系の 10 コード（`E060`〜`E063`, `W060`〜`W065`, `I050`〜`I052`）は `Anchor::Status`（文書位置を持たないステータスバー通知）として `STATUS_ANCHOR`（`REF:src/core/diagnostics.rs`）に定義する。
- 規則 10 違反（`E060`）、IndexedDB 利用不可（`E062`）、保存失敗（`E063`）、容量逼迫（`W063`）、資料変更検知（`W064` / `W065`）、プロンプト初期化（`I050`）などを各アダプタ・ワークスペースから発行する。

### 7.2 執筆支援系の診断

状態: 実装完了（REF:src/core/diagnostics.rs, REF:src/core/features/project.rs, REF:src/core/features/project.rs, REF:tests/diagnostics_document.rs）

- 執筆支援系の 27 コード（`W080`〜`W099`, `E032`〜`E036`, `E064`, `I054`）は `Group::Writing` として `REF:src/core/diagnostics.rs` に定義する。
- 既定の有効（`TOGGLE_ON`: 11 件）/ 無効（`TOGGLE_OFF`: 7 件）および実行コスト制御テーブル（`COST`: 16 件の `Trigger` × `Scope`）は `REF:src/core/diagnostics.rs` に定数表として持ち、`REF:tests/diagnostics_catalog.rs` で仕様表との完全一致を検証する。
- 段階 ① 対象の執筆支援診断（`W081` 文長、`W082` 連続文末、`W083` 話者不明会話、`W084` 非定義視点、`W085` 助詞連続、`W086` 指示語過多、`W087` 漢字率、`W094` 台詞長、`W098` 表記揺れ、`E032`〜`E036` / `E064` / `I054` エージェント系）は `REF:src/core/features/project.rs` および `REF:src/core/agents/runner.rs` で実装済みであり、段階 ② の構文（`when:` / `senses:` / `space:` 等）に依存するコードは縫い代 3 に従いカタログ定義と設定トグルを保持している。
