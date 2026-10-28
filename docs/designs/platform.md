# novnow 設計書 — ストレージと起動仕様

本書は `REF:docs/specifications/platform.md` の各節に対応する実装設計を記述する。

## 13. ストレージ

### 13.1 規則 10 — プロジェクト外に出してよい情報

状態: 実装完了（REF:src/core/policy/write_policy.rs, REF:src/adapters/storage_fs.rs, REF:src/ports/dialog.rs, REF:tests/storage_policy.rs, REF:tests/layering.rs）

- **境界判定（`WritePolicy`）**: `REF:src/core/policy/write_policy.rs` がシンボリックリンク解決後の絶対パスを受け取り、現在開いている `project_root` 配下または `history_file` 1 個のみを許可する（`B-54`）。
- **明示エクスポート先の特例（`ChosenPath` 関門、`B-108`）**: `ChosenPath`（`REF:src/ports/dialog.rs`、プライベートフィールドにより保存ダイアログまたは CLI の `--out` 引数解析以外での構築を型レベルで禁止）と `WritePolicy::check_chosen_export` / `FsGate::write_chosen_export`（`REF:src/adapters/storage_fs.rs`）のみが、ユーザーが明示指定した拡張子一致ファイル 1 個への単発書き込みを許可する。
- **単一関門の強制と違反時の挙動（`B-20`, `B-107`）**: `src/` 全体でファイル書き込み API を呼んでよいのは `REF:src/adapters/storage_fs.rs` のみであり、`REF:tests/layering.rs`（`only_the_write_gate_touches_the_file_system_for_writing`）と `REF:tests/storage_policy.rs` で機械的に強制する。ポリシー違反時はデバッグビルドでは `debug_assert!` で即座にパニックし、リリースビルドでは `Err(WriteViolation::ForbiddenTarget)`（診断 `E060`）を返してシステムコールを一切発行しない。
- 配布物取得の限定例外は `REF:vscode/src/binary.ts` と `REF:scripts/action/install.sh` の専用保存領域で扱い、作品の `FsGate` / `WritePolicy` を緩めない。原稿・設定・ログを取得キャッシュへ保存しない。
- **外部シンボリックリンクの遮断**: プロジェクト内から外部を指すシンボリックリンクは `REF:src/adapters/storage_fs.rs` の列挙・読み取りで除外する。

### 13.2 デスクトップ: パス履歴ファイル

状態: 実装完了（REF:src/core/policy/recent.rs, REF:src/adapters/recent_file.rs, REF:tests/recent_file.rs）

- 履歴 JSON スキーマ（`schema_version: 1`, `items: [{ path, opened_at }]`、上限 32 件、重複統合）は `RecentProjects`（`REF:src/core/policy/recent.rs`）に純粋データ構造として実装し、OS 別の保存先解決と `FsGate` 経由の直接上書きは `REF:src/adapters/recent_file.rs` が担う（検証: `REF:tests/recent_file.rs`）。

### 13.3 Web: IndexedDB

状態: 実装完了（REF:src/adapters/storage_idb.rs, REF:tests/storage_idb.rs, REF:index.html, REF:sw.js, REF:manifest.webmanifest, REF:scripts/web-smoke.mjs）

- DB 名 `novnow`（version 1）の 3 オブジェクトストア（`projects`, `files`, `app`）と `APP_KEYS`（`recent_projects` のみ許可）、残容量 10MB 未満の `W063`、IndexedDB 利用不可時の `E062`、初回 `sample` プロジェクト投入は `REF:src/adapters/storage_idb.rs` に実装する。
- スキーマ定数と仕様表の一致は `REF:tests/storage_idb.rs` が `include_str!` で検証し、ブラウザ実機動作は `REF:scripts/web-smoke.mjs` で検証する。

- **[S-03] Web 版の PWA 化とオフライン動作（実装完了: `REF:index.html`, `REF:sw.js`, `REF:README.md`, `REF:scripts/web-smoke.mjs`）**:
  - `REF:index.html` に Web App Manifest（`REF:manifest.webmanifest`）と Service Worker（`REF:sw.js`）登録を設け、オフラインキャッシュと `REF:README.md` への PWA 案内を実装する。`REF:scripts/pwa-assets.py` を Trunk post_build で実行し、全資源の一覧と内容ハッシュを worker に封入する。`REF:sw.js` は全件キャッシュが成功したときだけ activate する。旧キャッシュを install 中に消さない。`REF:scripts/web-smoke.mjs` では追加のオンライン再読み込みを挟まず、オフライン再起動と IndexedDB の継続性を確認する。

### 13.4 プロジェクト選択モーダル（Web）

状態: 実装完了（REF:src/ui/web.rs, REF:src/adapters/storage_idb.rs, REF:scripts/web-smoke.mjs, REF:tests/gui.rs）

- Web 起動時および `Ctrl+O` 時のプロジェクト選択モーダル（一覧・検索 `/`・新規作成 `Ctrl+N`・Zip 取込 `Ctrl+I`・改名 `F2`・削除 `Delete`・単一タブ利用の注記 `project.pick.single_tab`）は `WebPicker`（`REF:src/ui/web.rs`）に実装する（検証: `REF:scripts/web-smoke.mjs`）。

- **[F-33 / S-02] Web 版の同名プロジェクト作成禁止と初回起動ウェルカム案内（実装完了: `REF:docs/plans.md#plan-F-33`, `REF:docs/plans.md#plan-S-02`）**:
  - `REF:src/ui/web.rs` と `REF:src/adapters/storage_idb.rs` で既存プロジェクトと同一タイトルの作成・改名を拒否する（F-33 は実装完了。`REF:tests/storage_idb.rs` と実ブラウザ `REF:scripts/web-smoke.mjs` で検証済み）。履歴が空の初回起動時に案内を表示し、`>welcome` パレットからウェルカム案内モーダルを再表示する（S-02 は `REF:src/ui/app.rs`, `REF:src/ui/web.rs`, `REF:tests/gui.rs` で確認済み）。

- **[Issue #29 / B8] 入口ページの初期言語（実装完了: `REF:docs/plans.md#issue-29-b8`）**:
  - `REF:index.html` の冒頭のインラインスクリプトが `navigator.language` で `lang` 属性・題・オフライン状態表示の初期文言を出し分け、本体読み込み前の静的な表示もロケールに従う。選択画面に言語切替ボタンは置かない（§13.4.2）。検証は `REF:scripts/web-smoke.mjs` の ja/en 両ロケールの初期表示チェック。あわせて iOS のフォーカスずれ（N29-07）へ、隠し入力欄への再フォーカスを減らす（短い離脱で隠さない）＋フォーカス直後のスクロール位置復元を `sync_text_agent`（`REF:src/ui/web.rs`）に実装し、「節移動後も入力が続くこと」をスモークの回帰にした。症状は `REF:docs/known-issues/ja.md`（KI-002）に載せ、修正版の公開後に外す。

### 13.5 Zip インポート / エクスポート

状態: 実装完了（REF:src/core/policy/archive.rs, REF:src/adapters/zip.rs, REF:src/cli/import.rs, REF:src/ui/app.rs, REF:tests/zip_roundtrip.rs, REF:tests/cli.rs）

- アーカイブ対象の選別（ドット始まりファイルの除外と `.gitignore` / `.gitkeep` の例外許可、パストラバーサル `..` の拒否）は `core::policy::archive`（`REF:src/core/policy/archive.rs`）が担い、Deflate 圧縮・展開は `adapters::zip`（`REF:src/adapters/zip.rs`）が担う（検証: `REF:tests/zip_roundtrip.rs`）。

- **[S-10 / S-16] デスクトップ Zip インポートと Web⇔デスクトップ移行ヘルプ（実装完了: `REF:src/cli/import.rs`, `REF:src/ui/app.rs`, `REF:tests/cli.rs`, `REF:tests/gui.rs`）**:
  - `REF:src/cli/mod.rs` と `REF:src/ui/app.rs` にデスクトップでの Zip 導入を追加し（S-10 は `REF:tests/cli.rs`, `REF:tests/zip_roundtrip.rs` で安全な復元を検証）、Zip を使った Web⇔デスクトップ移行手順のヘルプ画面を追加する（S-16 は `REF:src/ui/app.rs`, `REF:tests/gui.rs` で検証済み）。

## 14. 起動仕様

- **[Issue #29 / B10] `.novnow` パッケージ（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - `.novnow` は実体が通常の Zip で、`.zip` と同じ安全規則（パス検証・未知ファイルの保持・上書き禁止）で扱う。書き出し側は `ExportKind::Zip` の拡張子（`REF:src/core/policy/write_policy.rs`）とダイアログのフィルタ（`REF:src/adapters/dialog_rfd.rs`, `REF:src/ui/web.rs`）へ `novnow` を足し、インポートは拡張子を問わず同じ計画（`plan_import`）で未知ファイルを落とさない（検証: `REF:tests/storage_policy.rs`, `REF:tests/zip_roundtrip.rs` の `unknown_files_are_kept_on_import`）。

### 14.1 CLI 文法

状態: 実装完了（REF:src/cli/mod.rs, REF:src/main.rs, REF:tests/cli.rs, REF:docs/plans.md#issue-29-x1）

- 引数解析とサブコマンド（`<dir>`, `new`, `check`, `export`, `mcp`, `import`, `migrate`）およびフラグ（`--gui`, `-h`/`--help`, `-V`/`--version`）の振り分けは `REF:src/cli/mod.rs` に実装する（検証: `REF:tests/cli.rs`）。

- **[S-10] `novnow import <zip> <dir>` サブコマンド（実装完了: `REF:docs/plans.md#plan-S-10`）**:
  - `REF:src/cli/mod.rs` に `import <zip> <dir>` を追加し、`REF:src/adapters/zip.rs` で展開・検証を行う。

- **[Issue #29 / X1] `migrate` の複数入力（実装完了: `REF:docs/plans.md#issue-29-x1`）**:
  - `migrate` の引数は、末尾の `--preview` を除いた最後の引数を移行先ディレクトリとし、それより前をすべて入力（ファイルとフォルダの混在可）とする。入力が 1 個も無い形式はエラーにする。既存の `migrate <text> <dir>` の並びはそのまま使える。入力の展開・読み替え・候補生成は §14.6。

### 14.2 起動時のスキャンとエラー出力

状態: 実装完了（REF:src/cli/open.rs, REF:src/cli/check.rs, REF:src/core/features/place.rs, REF:tests/cli.rs）

- 起動前スキャン、不足項目一覧の表示、`novnow new` の案内、作家の言葉での診断位置表記 `[本文 @01-04-02-03]`（`REF:src/core/features/place.rs`）、および終了コード `0`〜`5` の制御は `REF:src/cli/open.rs` と `REF:src/cli/check.rs` に実装する。
- 仕様書 §14.2.2・§14.2.3 の出力例との一字一句の一致は `REF:tests/cli.rs` で検証する。
- §14.2.1 の 3 つの URL は `REF:src/core/model/project.rs` の定数だけが持ち、`REF:src/cli/mod.rs`・
  `REF:src/cli/open.rs`・`REF:src/ui/app.rs` は定数を参照する（直書きしない）。3 URL の公開物の実在は
  `REF:docs/script/reference.py` の検査 11 が照合する（対応するページは `guide/` 配下に生成する）。
- 案内文は「何が足りないか」の列挙で終わらせず、次にすること（`novnow new` / フォルダを開く）を
  最後の 1 文に必ず置く（利用者向けガイドと同じ言い回しにする）。

- **[Issue #29 / B10] 出力例の新名称への差し替え（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - 仕様 §14.2.2 / §14.2.3 の出力ブロックを不足一覧の実装（`project.toml` / `plots/all.cnov`）と同時に差し替えた。照合は `REF:tests/cli.rs` の `spec_block` が引き続き担う。

### 14.3 GUI 直接起動の判定

状態: 実装完了（REF:src/cli/mod.rs, REF:src/main.rs, REF:tests/cli.rs）

- 優先順位（`--help`/`--version` → サブコマンド → `--gui` → 位置引数 `<dir>` → Web → 非 TTY → TTY 引数なしは終了コード 2）は `REF:src/cli/mod.rs` に実装する。
- Windows の `AttachConsole(ATTACH_PARENT_PROCESS)` は `REF:src/main.rs` の `#[cfg(windows)]` ブロックにのみ置く（`REF:tests/layering.rs` で検査）。

### 14.4 GUI 起動時のプロジェクト選択モーダル（デスクトップ）

状態: 実装完了（REF:src/ui/app.rs, REF:tests/gui.rs）

- デスクトップで `<dir>` なしに GUI 起動した際のプロジェクト選択画面（`recent.json` の履歴一覧、存在しないパスの灰色表示と履歴削除、フォルダ選択ダイアログ、新規作成）は `NativePicker`（`REF:src/ui/picker.rs`）に実装する。

- **[F-40 / S-11 / S-17] デスクトップ選択画面の `Ctrl+O` 2 段化・日付表示・履歴削除（実装完了: `REF:docs/plans.md#plan-F-40`, `REF:docs/plans.md#plan-S-11`, `REF:docs/plans.md#plan-S-17`）**:
  - `REF:src/ui/picker.rs` の `NativePicker` で、`Ctrl+O` 押下時は保存して履歴一覧画面へ戻り、そこから「フォルダを開く」を選ぶ 2 段構成に統一する（F-40）。
  - 各履歴項目に最終利用日付 `opened_at` を表示する（S-11、`REF:src/ui/app.rs` の native picker テストで確認）。「データは消さず履歴のみを一覧から削除する」旨を明示した履歴削除機能を提供する（S-17、`REF:tests/recent_file.rs` で実データ不変を確認）。

### 14.5 `novnow new` の生成物

状態: 実装完了（REF:src/core/template/mod.rs, REF:src/cli/new.rs, REF:tests/template.rs, REF:tests/cli.rs）

- 初期ファイル一式の生成と既存ファイル保護（上書きせず `kept` / `created` を表示）、生成直後の `novnow check` 診断 0 件保証は `REF:src/core/template/mod.rs` と `REF:src/cli/new.rs` に実装し、`REF:tests/template.rs` と `REF:tests/cli.rs` で検証する。

- **[Issue #29 / B10] 生成物の改名（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - `novnow new` の生成物を新しい名前（`project.toml` / `.cnov`、`schema_version = 2`）で作るよう生成側（`template::project_toml` とファイル一覧）を合わせた。

### 14.6 既存本文テキストからの移行支援

状態: 実装完了（REF:src/core/migration.rs, REF:src/core/text/encoding.rs, REF:src/cli/migrate.rs, REF:src/adapters/storage_fs.rs, REF:src/ui/app.rs, REF:tests/migration.rs, REF:tests/cli.rs, REF:tests/gui.rs, REF:docs/plans.md#issue-29-x1）

- 純粋な変換は `REF:src/core/migration.rs` に置き、I/O は持たせない。役割は分割候補の生成（`analyze`）、候補からの構造確定（`structure`）、プロジェクトファイルの組み立て（`render`）に分ける。入力は `SourceUnit`（ファイル名、読み替え済み本文、章開始の印）の列とし、全入力を 1 本の行列として連結して走査する。ファイル境界は話、フォルダ先頭ファイルの境界は章の初期候補になる。既存の単一入力用 `convert` は `analyze`＋`structure`＋`render` の組み合わせへ置き換える。
- バイト列の読み替えは `REF:src/core/text/encoding.rs` の `decode` / `decode_as` が担う（許可クレート `encoding_rs`、§3.2）。判定は「正しい UTF-8（BOM は除く）ならそのまま、読めなければ Shift_JIS として変換し、変換でも化ける（置換文字が出る）入力は拒否」の決定的な順序で行う。UTF-8 としても Shift_JIS としても読める場合は「曖昧」として記録し、デスクトップのプレビューでファイルごとに切り替えられるようにする。空白だけの入力、NUL を含む入力は拒否する。
- 候補生成（`analyze`）の規則は決定的に次のとおりとする。境界候補は行と行の間の点に段（章 / 話 / 場面 / 節）を持つ。候補が重なった場合はそのまま積み、構造確定で内容を持たない区間を畳むことで、実質的に最上位の段と見出しのタイトルが残る（「第一章」の直後の「第一話」のように、章と話のタイトルを両方生かすため）。見出し行と区切り行は境界が有効な間は本文へ残らず、境界を取り消すと本文へ戻る。
  - 見出し行: 前の行が空行・入力先頭・見出し行のいずれかで、40 文字以内の単独の短い行だけを対象とする。先頭の空白と括弧類を除いて「第○章」「第○話」（○ は算用数字の列または漢数字の列。値は構造番号に使わない）で始まれば章 / 話の候補とし、序章・終章・プロローグ・エピローグで始まれば章、幕間・閑話・番外編（「その○」の続きも可）で始まれば話の候補とする。語の直後が区切り（空白・句読点・各種ダッシュ等）か行末のときだけ見出しとし、残りをタイトルとして取り出す（章・話のタイトルに使い、本文へ残さない）。
  - 区切り行: 空白を取り除いて 3 文字以上がすべて区切り記号（`＊` `*` `※` `＃` `#` `―` `－` `-` `=` `＝` `◇` `◆` `○` `●` `△` `▲` `□` `■` `☆` `★` 等の定数集合）で構成される行を場面の候補とする。三点リーダや読点だけの行は区切り行にしない。
  - 空行: 空行 1 つだけの箇所は節、2 つ以上続く箇所は場面の候補とする。境界が有効な空行は本文へ残らず、取り消すと本文の空行として戻る。
  - 構造確定（`structure`）では、境界で区切られた区間のうち内容行を持たないものを捨て、章・話・場面・節の番号を出現順に振り直す。話者推定はしない。
- ファイルの組み立て（`render`）は既定雛形（`REF:src/core/template/mod.rs`）からサンプルの本文・プロットを取り除き、生成した `contents/CC-EE.cnov`（場面見出しと `;` 区切りの節）と `plots/all.cnov` / `plots/CC.cnov` / `plots/CC-EE.cnov`（章・話・場面のブロック。要約は空、タイトルは見出しから取る。本文の節と対応する節区切りを持つ）へ置き換える。本文とタイトルの novnow 記号はエスケープして保持する。生成パスが動的になるため `TemplateFile` のパスは所有する文字列へ変更する。
- CLI の入力展開・プレビュー・書き込みは `REF:src/cli/migrate.rs`。ファイルはそのまま、フォルダは直下の `.txt` / `.md` だけを自然順（`natural_cmp`）で読み、読み替えと検証をすべて済ませてから候補を生成する。プレビューは生成ファイル一覧・構造の件数・変換後の本文を表示して何も作らない。確定時は移行先を排他的に作成する `FsStorage::create_fresh`（`REF:src/adapters/storage_fs.rs`）で既存先を拒否し、書き込みは既存の単一関門を使用する。題名は移行先ディレクトリ名とする。
- デスクトップは `REF:src/ui/picker.rs` の NativePicker とダイアログポート（複数ファイル選択の追加を含む）から、入力選択→プレビュー→新規先へ作成→開く、を接続する。プレビューでは章・話・場面・節のツリーと件数を示し、各候補の段変更と、曖昧な入力の文字コード切り替え（切り替え時は候補を生成し直す）ができる。Web はファイル移行の対象外とする。
- 貼り付けの修復候補は `REF:src/core/text/encoding.rs` の純粋関数 `mojibake_repair` が判定する。貼り付けた文字列を Shift_JIS へ符号化し（復元できない文字は 1 文字 1 バイトの欠損として扱う）、得られたバイト列を UTF-8 として読み直す。文字化けらしき文字列（仕様 §14.6.1 の固定 17 字のいずれかを含む、または半角カタカナが 3 文字以上）で、4 文字以上・ASCII だけではない・欠損（置換文字）が候補の半分以下・候補にかなを含むときだけ、直した候補を返す。GUI はフレームの先頭で複数行の貼り付け（複数行入力欄へのもの）を横取りし、候補があるときだけ元の先頭と直した先頭を見せて選ばせ、選択した文字列を次のフレームの頭で貼り付けイベントとして渡し直す（フォーカスは元の入力欄へ戻す）。候補を作れない場合は、元のまま貼り付ける。対象はデスクトップと Web の双方で、節エディタの分割（§8.1.3）はその後段にそのまま効く。
- 検証は `REF:tests/migration.rs`（読み替え、候補生成、構造確定、描画したプロジェクトが開けることと本文の往復）、`REF:tests/cli.rs`（複数入力・フォルダ入力・プレビュー非変更・入力不正・既存先拒否・入力元保持）、`REF:tests/gui.rs`（プレビューの段変更と確定、貼り付け修復の提示と選択）。
