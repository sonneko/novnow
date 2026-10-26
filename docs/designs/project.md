# novnow 設計書 — プロジェクト構造

本書は `REF:docs/specifications/project.md` の各節に対応する実装設計を記述する。

## 4. プロジェクト構造

### 4.1 ディレクトリレイアウト

状態: 実装完了（REF:src/core/model/project.rs, REF:src/core/policy/write_policy.rs, REF:src/core/template/mod.rs）

- プロジェクト内の相対パス表現は `ProjectPath`（`REF:src/core/policy/write_policy.rs`）で正規化し、`..` や絶対パスを型レベルで排除する。
- 新規プロジェクトのディレクトリ・初期ファイル構成は `core::template::new_project`（`REF:src/core/template/mod.rs`）で生成する。

- **[Issue #29 / B10] レイアウトの改名（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - `project.toml` と `contents/`・`plots/` の `.cnov` へ一括して移行した。パス判定は `DocumentFormat::of_path`（`REF:src/core/model/document.rs`、§5.0）、必須パスの定数は `REF:src/core/model/project.rs`、テンプレートとサンプルは `REF:src/core/template/mod.rs` と `examples/sample-novel/` が追随する。

### 4.2 必須 / 任意の判定表

状態: 実装完了（REF:src/core/model/project.rs, REF:tests/model.rs）

- 必須項目（`project.toml`, `characters.md`, `words.md`, `materials/index.json`, `plots/all.cnov` と必須ディレクトリ）の検査は `REQUIRED_FILES` / `REQUIRED_DIRS` と `Listing`（`REF:src/core/model/project.rs`）で表現し、不足項目を表の順で全部返す（検証: `REF:tests/model.rs`, `REF:tests/project_load.rs`）。

- **[Issue #29 / B10] 必須表の改名追随（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - 必須パスの定数を新しい名前へ差し替えた。旧形式のプロジェクトは特別扱いなしに必須ファイル不足の通常エラーになる（検証: `REF:tests/project_load.rs` の `an_old_format_project_is_simply_incomplete`）。

### 4.3 プロジェクト妥当性の判定

状態: 実装完了（REF:src/core/model/project.rs, REF:src/adapters/project_load.rs, REF:tests/project_load.rs）

- `core::model::project::validate`（`REF:src/core/model/project.rs`）が `Listing` を受け取り、`ProjectError`（`Incomplete` / `TooNew` / `Encoding`）を決定論的に返す。`schema_version` の読みは TOML の整数だけを見て、読めなければ検査を飛ばす（`schema_of`）。
- ストレージからの一括読み込みと UTF-8 検査・自動作成ディレクトリの補完は `adapters::project_load::load`（`REF:src/adapters/project_load.rs`）が担う（検証: `REF:tests/project_load.rs`）。

- **[Issue #29 / B10] 未知ファイルの扱いの明文化（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - 仕様が定義しないファイルは妥当性判定の対象外であり、削除・改名・上書きをしない。文書判定（§5.0.9）が形の合わない `.cnov` と旧 `.txt` を文書にせず、UTF-8 検査の対象も文書として認識するファイルだけにした（検証: `REF:tests/model.rs` の形式表、`REF:tests/project_load.rs`、`REF:tests/zip_roundtrip.rs` の `unknown_files_are_kept_on_import`）。

### 4.4 `.novnow/`（ローカル状態）

状態: 実装完了（REF:src/app/settings.rs, REF:src/ui/widgets/section_editor.rs, REF:src/ui/widgets/context_view.rs, REF:tests/gui.rs）

- `.novnow/ui.json` の読み書きと未知キー温存・数値範囲のクランプ（`font_scale`, `main_ref_ratio`, `context_editor_ratio`, `autosave_ms`）は `ui::settings::UiSettings`（`REF:src/app/settings.rs`）が担う（検証: `REF:tests/gui.rs`）。
- **[Issue #29 / B3] `confirm_section_delete` の消費（実装完了: `REF:docs/plans.md#issue-29-b3`）**: 空でない節の削除確認の有無は `UiSettings.confirm_section_delete`（`REF:src/app/settings.rs`、既定 `true`）で読み書きし、`NovnowApp::request_delete`（`REF:src/ui/app.rs`）が消費する。

- **[S-18] 本文最大行幅設定 `max_line_chars`（実装完了: `REF:docs/plans.md#plan-S-18`）**:
  - `REF:src/app/settings.rs` の `UiSettings` 構造体に `pub max_line_chars: Option<usize>` を追加し、`clamp` 時に `20..=120` に丸める。
  - 実効幅取得メソッド `line_chars(english: bool) -> usize` を設け、未設定時は `Locale::Ja => 44`、`Locale::En => 70` を返す。

### 4.5 `project.toml` スキーマ（schema_version 2）

状態: 実装完了（REF:src/core/model/project.rs, REF:src/core/syntax/project_toml.rs, REF:tests/project_json.rs）

- スキーマ構造体 `Project`（`Settings`）は `REF:src/core/model/project.rs` に定義し、`#[serde(flatten)] pub unknown: BTreeMap<String, Value>` で未知キーを保持する（書き戻しの温存は `toml_edit` 側が担う。付録 C 冒頭の B10 を参照）。
- TOML パースエラー（`E025`）および `lexicon.word_classes` の不正正規表現（`E027`）の位置特定と診断生成は `core::syntax::project_toml::parse`（`REF:src/core/syntax/project_toml.rs`）が担う。読み取りは TOML の値を JSON の値へ写してから型付き検証し（日時は文字列として保持）、仕様書 §4.5 の TOML 例との一致は `REF:tests/project_json.rs` が `include_str!` で直接検証する。

## 付録 C: `sample` プロジェクト

- **[Issue #29 / B10] `project.toml` への移行と `[features]`（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - 読み取りは `project_toml::parse` の型付き検証、書き戻しは `project_toml::edit` / `set_string`（`REF:src/core/syntax/project_toml.rs`）の無損失編集で、変更したキー以外はコメント・書式・キー順・未知キー（変更した値の行末コメントを含む）を保持する。設定を書き換えるのは委譲レベルの変更（`REF:src/core/agents/runner.rs`）とローカル LLM 設定（`REF:src/ui/workspaces/stage_two.rs`）で、どちらもこの経路を通る。壊れた TOML は E025 を出して既定値で開き、上書きしない（検証: `REF:tests/project_json.rs` の `lossless_edit_keeps_comments_order_and_unknown_keys` と `broken_toml_is_never_overwritten`）。
  - `toml_edit` を直接依存として明示し、`toml` とあわせて core の許可クレートへ足した（`REF:Cargo.toml`、`REF:tests/layering.rs`、§3.2）。
  - `schema_version = 2` とし、旧形式の変換は作らない（所有者決定 M-2）。
  - `[features.diagnostics]` は `Settings.features.diagnostics`（`REF:src/core/model/project.rs`）に読み、解決と適用は §7.0（`REF:docs/designs/diagnostics.md`）が担う。操作系 feature は `[features]` 直下に置き、`Features.rest` が温存する。
  - `REF:tests/project_json.rs` は §4.5 の TOML の例を仕様書から読む形へ戻した。

### C.1 サンプルプロジェクト構成と検証要素

状態: 実装完了（REF:src/core/template/mod.rs, REF:src/ui/web.rs, REF:tests/template.rs）

- 付録 C のサンプルプロジェクト一式は `examples/sample-novel/` に配置し、`core::template::sample_project`（`REF:src/core/template/mod.rs`）が `include_str!` でバイナリに埋め込む。
- Web 版の初回起動時は `REF:src/ui/web.rs` の `seed_if_empty` が `template::sample_project()` を IndexedDB に投入する。
- サンプルが全検証要素を含み、かつエラー診断が 0 件であることは `REF:tests/template.rs` の `sample_project_covers_appendix_c_with_no_diagnostics` で検証する。

- **[Issue #29 / B10] サンプルの改名と `project.toml` 化（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - 埋め込みサンプルと `examples/sample-novel/` の実ファイルを新しい名前へ揃えた（内容は同一）。検証要素（診断 0 件など）の不変は `REF:tests/template.rs` がそのまま確かめている。
