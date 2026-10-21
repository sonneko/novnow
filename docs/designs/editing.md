# novnow 設計書 — 編集モデルと保存

本書は `REF:docs/specifications/editing.md` の各節に対応する実装設計を記述する。

## 8. 編集モデルと保存

### 8.1 節単位編集

状態: 実装完了（REF:src/core/features/sections.rs, REF:src/app/workbench.rs, REF:src/ui/widgets/section_editor.rs, REF:tests/sections.rs, REF:tests/gui.rs）

- 文書内の各節のスライス抽出と単一節の差し替え・分割（`Ctrl+Enter` / `Ctrl+;`）・削除（`Ctrl+Shift+;`）は `core::features::sections`（`REF:src/core/features/sections.rs`）と `Workbench`（`REF:src/app/workbench.rs`）が担う。
- 節エディタウィジェット（`REF:src/ui/widgets/section_editor.rs`）は選択中の 1 節のテキストのみを `egui::TextEdit::multiline` に渡し、コミット時にファイル全体のテキストを再構成する（検証: `REF:tests/gui.rs`）。
- **節スライス再構成と末尾 `;` の内部規約（`REF:docs/history.md` の `B-112`, `B-113`）**:
  - 節スライスの差し替え（`sections::replace`）は AST 全体の再構築を行わず、該当節の byte range のみを置換して前後のコメント・空行・未知構文を 1 バイトも変えずに保持する（`B-113`）。
  - C 形式のパーサはファイル末尾の `;` の後ろに本文がない場合に空節を作らないため（`B-35`）、エディタで末尾で `Ctrl+Enter`（節分割）を押した直後に新節へフォーカスできるよう、`sections::slices` が「末尾の非空行が `;` のときは末尾オフセットに仮想の空節スライスを 1 個追加する」ことで GUI の節追加を実現する（`B-112`）。

- **[F-10] 複数行および `;` を含むペースト時の自動節分割（実装完了: `REF:src/core/features/sections.rs`, `REF:src/ui/widgets/section_editor.rs`）**:
  - `REF:src/core/features/sections.rs` に `split_pasted_sections(current: &str, cursor: usize, pasted: &str)` を追加し、`REF:src/ui/widgets/section_editor.rs` / `REF:src/app/workbench.rs` で `egui::Event::Paste` を処理する際、改行を保持するとともに単独 `;` 行や `@A-B-C` 行が含まれていれば自動的に複数節へ分割・一括挿入する。

### 8.2 保存ステートマシン

状態: 実装完了（REF:src/app/save_machine.rs, REF:src/ui/app.rs, REF:tests/gui.rs）

- 保存状態 `State`（`Clean`, `Dirty`, `Saving`, `Failed`）とデバウンスタイマー（既定 400ms、`200..=2000ms` にクランプ）、指数バックオフ再試行（1s → 2s → 4s → 最大 8s）は `SaveMachine`（`REF:src/app/save_machine.rs`）に純粋な状態機械として実装する。
- 節移動・ワークスペース切替・ウィンドウフォーカス喪失・`Ctrl+S` では `request_now()` で即時保存を要求する（検証: `REF:tests/gui.rs`）。
- `SaveMachine` は時刻を引数 `now_ms: u64` で受け取る決定論的ステートマシンとし、テストからは実時間スリープなしでミリ秒単位の遷移を検証できるようにする（`B-115`）。

### 8.3 原子的書き込み

状態: 実装完了（REF:src/adapters/storage_fs.rs, REF:src/adapters/storage_idb.rs, REF:tests/storage_policy.rs, REF:tests/storage_idb.rs）

- デスクトップ版（`REF:src/adapters/storage_fs.rs`）は同一ディレクトリ内に一時ファイル `.~novnow-tmp-<name>` を作成して `write_all` + `sync_all` の後に `fs::rename` で原子的に置換する。
- Web 版（`REF:src/adapters/storage_idb.rs`）は単一の IndexedDB `readwrite` トランザクション内で `files` ストアと `projects.updated_at` を同時更新し、`Saving` / `Dirty` 中は `beforeunload` リスナ（`REF:src/ui/web.rs`）を有効化する。

### 8.4 保存状態の表示

状態: 実装完了（REF:src/ui/app.rs, REF:src/ui/widgets/context_view.rs, REF:src/ui/theme.rs, REF:tests/gui.rs）

- ステータスバー右端の保存状態インジケータ（`Clean` 無表示 / `Dirty` 琥珀色 `●` / `Saving` 点滅 `●` / `Failed` 赤 `✕` + 通知）は `NovnowApp`（`REF:src/ui/app.rs`）が `SaveMachine::state()` を参照して描画する。

- **[F-39] 未保存ドット（オレンジ点）のサイズ拡大（実装完了: `REF:src/ui/widgets/context_view.rs`, `REF:tests/gui.rs`）**:
  - `REF:src/ui/widgets/context_view.rs` の編集中プレースホルダの未保存ドットは半径 5px（直径 10px）。ステータスバーの保存インジケータは半径 4px（直径 8px、`REF:src/ui/app.rs`）。`REF:tests/gui.rs` で描画形状を確認する。

### 8.5 外部変更との競合

状態: 実装完了（REF:src/core/policy/external_change.rs, REF:src/ui/app.rs, REF:tests/gui.rs）

- ファイルスタンプ（更新時刻・サイズ・ハッシュ）の比較と `Clean` 時の自動再読み込み（`I052`）または `Dirty` 時の競合検出（`W061`）は `core::policy::external_change`（`REF:src/core/policy/external_change.rs`）が判定し、競合時は `Overlay::Conflict` モーダル（`REF:src/ui/app.rs`）で「自分の変更を残す / 外部の変更を採用 / 差分を見る」を選択させる（検証: `REF:tests/gui.rs`）。
- 自分自身の保存直後に OS のファイル監視イベントやポーリングが遅延到着して誤検知しないよう、保存完了時に記録した `FileStamp`（サイズ＋FNV-1a ハッシュ）と一致する外部通知は無視する（`B-116`）。

### 8.6 Undo / Redo

状態: 実装完了（REF:src/app/workbench.rs, REF:src/ui/widgets/section_editor.rs, REF:tests/gui.rs）

- 節内の編集履歴と節をまたぐ操作履歴は `Workbench`（`REF:src/app/workbench.rs`）および `egui::TextEdit` が保持し、自動保存によって Undo スタックが消去されない設計としている（`B-118`）。

### 8.7 提案モード（人間用トラックチェンジ）

状態: 実装完了（REF:src/core/agents/files.rs, REF:src/ui/app.rs, REF:tests/workspaces.rs）

- 人間用の変更提案 `H####.diff` の採番・作成・一覧・適用・破棄・差分比較（`Ctrl+Shift+D`）は `core::features::proposals`（`REF:src/core/agents/files.rs`）および `NovnowApp`（`REF:src/ui/app.rs`）に実装する（`B-120`、検証: `REF:tests/workspaces.rs`）。
