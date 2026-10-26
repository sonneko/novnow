# novnow 設計書 — プロジェクト構造

本書は `REF:docs/specifications/project.md` の各節に対応する実装設計を記述する。

## 4. プロジェクト構造

### 4.1 ディレクトリレイアウト

状態: 未実装・実装計画済み（REF:src/core/model/project.rs, REF:src/core/policy/write_policy.rs, REF:src/core/template/mod.rs, REF:docs/plans.md#issue-29-b10）

- プロジェクト内の相対パス表現は `ProjectPath`（`REF:src/core/policy/write_policy.rs`）で正規化し、`..` や絶対パスを型レベルで排除する。
- 新規プロジェクトのディレクトリ・初期ファイル構成は `core::template::new_project`（`REF:src/core/template/mod.rs`）で生成する。

- **[Issue #29 / B10] レイアウトの改名（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b10`）**:
  - `project.json` → `project.toml`、`contents/`・`plots/` の `.txt` → `.cnov` への改名は仕様側へ反映済み。テンプレート・パス判定・サンプルの追随は B10 で一括して行う（他の節の `project.toml` / `.cnov` 言及もすべてこの改名に伴い、B10 が実装の受け持ちである）。

### 4.2 必須 / 任意の判定表

状態: 未実装・実装計画済み（REF:src/core/model/project.rs, REF:tests/model.rs, REF:docs/plans.md#issue-29-b10）

- 必須項目（`project.json`, `contents/` かつ `.txt` 1 個以上, `plots/all.txt`, `characters.md`, `words.md`, `materials/index.json`）の検査は `Listing` と `MissingItem`（`REF:src/core/model/project.rs`）で表現し、不足項目を固定順で返す（検証: `REF:tests/model.rs`）。

- **[Issue #29 / B10] 必須表の改名追随（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b10`）**:
  - 必須パスの定数（`REF:src/core/model/project.rs`）を新しい名前へ差し替える。旧形式のプロジェクトは、特別扱いなしに必須ファイル不足の通常エラーになることをテストで固定する。

### 4.3 プロジェクト妥当性の判定

状態: 未実装・実装計画済み（REF:src/core/model/project.rs, REF:src/adapters/project_load.rs, REF:tests/project_load.rs, REF:docs/plans.md#issue-29-b10）

- `core::model::project::validate`（`REF:src/core/model/project.rs`）が `Listing` を受け取り、`ProjectStatus`（`Valid` / `Incomplete` / `InvalidProjectJson` / `NewerSchema`）を決定論的に返す。
- ストレージからの一括読み込みと UTF-8 検査・自動作成ディレクトリの補完は `adapters::project_load::load`（`REF:src/adapters/project_load.rs`）が担う（検証: `REF:tests/project_load.rs`）。

- **[Issue #29 / B10] 未知ファイルの扱いの明文化（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b10`）**:
  - 仕様が定義しないファイルは妥当性判定の対象外であり、削除・改名・上書きをしない。読み込み側は §5.0.9 の文書判定で既にこの形に近く、B10 で「未知の `.cnov` や旧 `.txt` を文書にしない」ことを `REF:tests/project_load.rs` のケースにして固定する。

### 4.4 `.novnow/`（ローカル状態）

状態: 実装完了（REF:src/app/settings.rs, REF:src/ui/widgets/section_editor.rs, REF:src/ui/widgets/context_view.rs, REF:tests/gui.rs）

- `.novnow/ui.json` の読み書きと未知キー温存・数値範囲のクランプ（`font_scale`, `main_ref_ratio`, `context_editor_ratio`, `autosave_ms`）は `ui::settings::UiSettings`（`REF:src/app/settings.rs`）が担う（検証: `REF:tests/gui.rs`）。
- **[Issue #29 / B3] `confirm_section_delete` の消費（実装完了: `REF:docs/plans.md#issue-29-b3`）**: 空でない節の削除確認の有無は `UiSettings.confirm_section_delete`（`REF:src/app/settings.rs`、既定 `true`）で読み書きし、`NovnowApp::request_delete`（`REF:src/ui/app.rs`）が消費する。

- **[S-18] 本文最大行幅設定 `max_line_chars`（実装完了: `REF:docs/plans.md#plan-S-18`）**:
  - `REF:src/app/settings.rs` の `UiSettings` 構造体に `pub max_line_chars: Option<usize>` を追加し、`clamp` 時に `20..=120` に丸める。
  - 実効幅取得メソッド `line_chars(english: bool) -> usize` を設け、未設定時は `Locale::Ja => 44`、`Locale::En => 70` を返す。

### 4.5 `project.toml` スキーマ（schema_version 2）

状態: 未実装・実装計画済み（REF:src/core/model/project.rs, REF:src/core/syntax/project_json.rs, REF:tests/project_json.rs, REF:docs/plans.md#issue-29-b10）

- スキーマ構造体 `Project`（`Settings`）は `REF:src/core/model/project.rs` に定義し、`#[serde(flatten)] pub unknown: BTreeMap<String, Value>` で未知キーを無損失保持する。
- JSON パースエラー（`E025`）および `lexicon.word_classes` の不正正規表現（`E027`）の位置特定と診断生成は `core::syntax::project_json::parse`（`REF:src/core/syntax/project_json.rs`）が担う。仕様書 §4.5 の JSON 例との一致は `REF:tests/project_json.rs` が `include_str!` で直接検証する。

## 付録 C: `sample` プロジェクト

- **[Issue #29 / B10] `project.toml` への移行と `[features]`（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b10`）**:
  - 読み取りは TOML の型付き検証で意味を取り、書き戻しは `toml_edit` の文書編集で変更したキーだけを反映し、コメント・書式・キー順・未知キーを保持する。壊れた TOML は E025 を出して既定値で開き、上書きしない。
  - `toml_edit` は既存の `toml` 依存が内包するが、直接依存として明示する。追加は `REF:docs/add-dependencies.md` の搬入手順に従う（許可クレート表への追記を含む）。
  - `schema_version = 2` とし、旧形式の変換は作らない（開けなければ通常エラー。所有者決定 M-2）。
  - `[features.diagnostics]` の解決は「個別コード → グループ（`structure` / `storage` / `writing`）→ `default` → 組み込み既定」の順。検証は「既定設定・全 feature オフ・個別に 1 件ずつ切り替え」の行列で行う。操作系 feature は `[features]` 直下の真偽値とし、キー定義は各機能の節が持つ。
  - 仕様 §4.4 の設定分類（feature 単位の設定は `.novnow/` に置かない）と `confirm_section_delete` のキーは §4.4 側の変更であり、削除確認の消費は B3 で先行してよい（`REF:docs/designs/editing.md` §8.1）。
  - `REF:tests/project_json.rs` は当面、ファイル内に埋め込んだ旧 JSON の例で現行パーサを守る。B10 で §4.5 の TOML の例を仕様書から読む形へ戻す。

### C.1 サンプルプロジェクト構成と検証要素

状態: 未実装・実装計画済み（REF:src/core/template/mod.rs, REF:src/ui/web.rs, REF:tests/template.rs, REF:docs/plans.md#issue-29-b10）

- 付録 C のサンプルプロジェクト一式は `examples/sample-novel/` に配置し、`core::template::sample_project`（`REF:src/core/template/mod.rs`）が `include_str!` でバイナリに埋め込む。
- Web 版の初回起動時は `REF:src/ui/web.rs` の `seed_if_empty` が `template::sample_project()` を IndexedDB に投入する。
- サンプルが全検証要素を含み、かつエラー診断が 0 件であることは `REF:tests/template.rs` の `sample_project_covers_appendix_c_with_no_diagnostics` で検証する。

- **[Issue #29 / B10] サンプルの改名と `project.toml` 化（未実装・実装計画済み: `REF:docs/plans.md#issue-29-b10`）**:
  - 埋め込みサンプルと `examples/` の実ファイルは、新しい名前へ揃えてから検証要素（診断 0 件など）が不変であることを `REF:tests/template.rs` で確かめる。
