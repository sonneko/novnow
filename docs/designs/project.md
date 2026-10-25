# novnow 設計書 — プロジェクト構造

本書は `REF:docs/specifications/project.md` の各節に対応する実装設計を記述する。

## 4. プロジェクト構造

### 4.1 ディレクトリレイアウト（v0.1.0）

状態: 実装完了（REF:src/core/model/project.rs, REF:src/core/policy/write_policy.rs, REF:src/core/template/mod.rs）

- プロジェクト内の相対パス表現は `ProjectPath`（`REF:src/core/policy/write_policy.rs`）で正規化し、`..` や絶対パスを型レベルで排除する。
- 新規プロジェクトのディレクトリ・初期ファイル構成は `core::template::new_project`（`REF:src/core/template/mod.rs`）で生成する。

### 4.2 必須 / 任意の判定表

状態: 実装完了（REF:src/core/model/project.rs, REF:tests/model.rs）

- 必須項目（`project.json`, `contents/` かつ `.txt` 1 個以上, `plots/all.txt`, `characters.md`, `words.md`, `materials/index.json`）の検査は `Listing` と `MissingItem`（`REF:src/core/model/project.rs`）で表現し、不足項目を固定順で返す（検証: `REF:tests/model.rs`）。

### 4.3 プロジェクト妥当性の判定

状態: 実装完了（REF:src/core/model/project.rs, REF:src/adapters/project_load.rs, REF:tests/project_load.rs）

- `core::model::project::validate`（`REF:src/core/model/project.rs`）が `Listing` を受け取り、`ProjectStatus`（`Valid` / `Incomplete` / `InvalidProjectJson` / `NewerSchema`）を決定論的に返す。
- ストレージからの一括読み込みと UTF-8 検査・自動作成ディレクトリの補完は `adapters::project_load::load`（`REF:src/adapters/project_load.rs`）が担う（検証: `REF:tests/project_load.rs`）。

### 4.4 `.novnow/`（ローカル状態）

状態: 実装完了（REF:src/app/settings.rs, REF:src/ui/widgets/section_editor.rs, REF:src/ui/widgets/context_view.rs, REF:tests/gui.rs）

- `.novnow/ui.json` の読み書きと未知キー温存・数値範囲のクランプ（`font_scale`, `main_ref_ratio`, `context_editor_ratio`, `autosave_ms`）は `ui::settings::UiSettings`（`REF:src/app/settings.rs`）が担う（検証: `REF:tests/gui.rs`）。

- **[S-18] 本文最大行幅設定 `max_line_chars`（実装完了: `REF:docs/plans.md#plan-S-18`）**:
  - `REF:src/app/settings.rs` の `UiSettings` 構造体に `pub max_line_chars: Option<usize>` を追加し、`clamp` 時に `20..=120` に丸める。
  - 実効幅取得メソッド `line_chars(english: bool) -> usize` を設け、未設定時は `Locale::Ja => 44`、`Locale::En => 70` を返す。

### 4.5 `project.json` スキーマ（v0.1.0）

状態: 実装完了（REF:src/core/model/project.rs, REF:src/core/syntax/project_json.rs, REF:tests/project_json.rs）

- スキーマ構造体 `Project`（`Settings`）は `REF:src/core/model/project.rs` に定義し、`#[serde(flatten)] pub unknown: BTreeMap<String, Value>` で未知キーを無損失保持する。
- JSON パースエラー（`E025`）および `lexicon.word_classes` の不正正規表現（`E027`）の位置特定と診断生成は `core::syntax::project_json::parse`（`REF:src/core/syntax/project_json.rs`）が担う。仕様書 §4.5 の JSON 例との一致は `REF:tests/project_json.rs` が `include_str!` で直接検証する。

## 付録 C: `sample` プロジェクト

### C.1 サンプルプロジェクト構成と検証要素

状態: 実装完了（REF:src/core/template/mod.rs, REF:src/ui/web.rs, REF:tests/template.rs）

- 付録 C のサンプルプロジェクト一式は `examples/sample-novel/` に配置し、`core::template::sample_project`（`REF:src/core/template/mod.rs`）が `include_str!` でバイナリに埋め込む。
- Web 版の初回起動時は `REF:src/ui/web.rs` の `seed_if_empty` が `template::sample_project()` を IndexedDB に投入する。
- サンプルが全検証要素を含み、かつエラー診断が 0 件であることは `REF:tests/template.rs` の `sample_project_covers_appendix_c_with_no_diagnostics` で検証する。
