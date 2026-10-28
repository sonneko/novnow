# novnow 設計書 — ファイル形式（構文仕様）

本書は `REF:docs/specifications/syntax.md` の各節に対応する実装設計を記述する。

## 5. ファイル形式（構文仕様）

### 5.0 共通字句規則

状態: 実装完了（REF:src/core/text/lexer.rs, REF:tests/lexer.rs）

- 行頭シジル判定（`SIGIL_TABLE`）・10 種のエスケープ解除・フィールド行の 7 段判定・継続行インデント判定（2 空白以上または先頭タブ 1 文字以上）・BOM/CRLF 正規化は `core::text::lexer`（`REF:src/core/text/lexer.rs`）に実装する。
- 日本語フィールドキー別名表は `/// i18n: format-vocabulary` を付与した定数配列として保持し、URI スキームとの非交差（`no_field_key_collides_with_a_url_scheme`）と双方向 1:1 対応（`the_alias_table_is_one_to_one_in_both_directions`）を `REF:tests/lexer.rs` で検証する。
- **字句解析の内部不変条件（`REF:docs/history.md` の `B-18`, `B-21`〜`B-24`, `B-36`, `B-37`）**:
  - `SIGIL_TABLE: &[SigilSpec]` は `(prefix, sigil, requires_space, whole_line, formats, stage)` を保持し、配列の並び順そのものが優先順位を表す（`match` 直書き禁止）。
  - インデント 0〜1 文字の行のみシジルおよびフィールド行として判定し、2 文字以上の空白または 1 文字以上のタブで始まる行は継続行（C 形式では段落字下げの通常本文）を優先する（`B-18`, `B-21`, `B-31`）。
  - 半角コロンのフィールド行判定は §5.0.6 のキー表を許可リストとして用い、表にない ASCII キーは `:` 直後の空白を必須とすることで URL（`https://...`）・時刻（`10:30`）・Windows パスを誤認しない。また直前が `:` の `//` は行内コメントにしない（`B-24`）。
  - フィールド値の後ろに書かれた `//` は無損失往復変換を保つため値の一部として保持し、数値等として解釈する層が行末コメントを除去する（`A-14`, `B-36`）。
  - 空文書は保存時に改行 1 個を追加せず 0 バイトのまま保持する（`B-22`）。

- L-01 の本文記号保護に `\#`・`\!`・`\^` を含め、`# ` コメントは `.cnov` 3 形式で有効化する。検証: `REF:tests/migration.rs`, `REF:tests/stage_two_comments.rs`。

- **[Issue #29 / B10] 文書として認識する範囲（§5.0.9）の実装（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - 形式の判定（`DocumentFormat::of_path`、`REF:src/core/model/document.rs`）を「置き場所と拡張子の組の厳密一致」に揃えた。`contents/<章>-<話>.cnov` のみ本文、`plots/all.cnov` と `plots/<章>.cnov` は A 形式、`plots/<章>-<話>.cnov` は B 形式とし、形が一致しない `.cnov` / `.md` / `.txt` は文書にしない（未知ファイルとして触れない）。字句規則そのものは変更していない（検証: `REF:tests/model.rs` の形式表）。

### 5.1 SceneId と範囲式

状態: 実装完了（REF:src/core/id.rs, REF:tests/id_range.rs）

- `SceneId`・`PartialSceneId`・`RangeTerm`・`RangeExpr` のパース、階層比較、閉区間判定（`contains`）、逆転範囲や予約番号 `00` の検出（`E016` / `E020` / `E022`）は `REF:src/core/id.rs` に実装し、`REF:tests/id_range.rs` で検証する。
- `IdError` / `RangeError` は `core` に文言を持たせないため `Display` を実装しない純粋な型 enum とし、`REF:src/core/diagnostics.rs` で診断コードへ写像する（`B-17`）。

### 5.2 本文形式（C 形式）— `contents/AA-BB.cnov`

状態: 実装完了（REF:src/core/syntax/contents.rs, REF:src/core/features/completion.rs, REF:tests/contents.rs, REF:tests/golden.rs, REF:tests/features_edit.rs）

- 場面見出し `@A-B-C[-D]`、節区切り `;` による節番号 `D` の自動採番、空節の `W012`、行末話者注釈 `@char[:mode]`、会話スパン `「」` / `『』`、ルビ 2 形式、コメント範囲の解析と無損失 `to_text()` は `core::syntax::contents`（`REF:src/core/syntax/contents.rs`）に実装する（検証: `REF:tests/contents.rs`, `REF:tests/golden.rs`）。
- **C 形式パーサの境界規則（`REF:docs/history.md` の `B-32`〜`B-35`, `B-40`）**:
  - 最初の場面見出しより前の行はどの節にも属させず `Contents::lines` にのみ保持して無損失往復を保つ（`B-32`）。
  - ルビの親文字 `kanji_run` は CJK 統合漢字・拡張 A・互換漢字および反復記号 `々`（U+3005）・`〻`（U+303B）とし、話者注釈の `char_name` は空白と `:` を含まない 1 文字以上とする（`B-33`）。
  - 空の節（`W012`）の診断スパンはクイックフィックスで削除できるよう「節を閉じた `;` 行または次の見出し行」を指し、ファイル末尾の `;` の後ろに本文がない場合は空節を作らない（`B-35`）。
  - `.cnov` 3 形式の見出し・コメント・区切り行の共通ヘルパは `REF:src/core/syntax/contents.rs` から `pub(crate)` で共有する（`B-40`）。

- **[S-08] 会話閉じ括弧直後の `@` による話者予測補完（実装完了: `REF:src/core/features/completion.rs`, `REF:tests/features_edit.rs`）**:
  - `REF:src/core/features/completion.rs` のトリガー判定に「同一行のカーソル前に `」` または `』` があり、その後に `@`（および入力途中のキャラ名/モード）が続く」ケースを追加し、`characters.md` の登場人物名と `:モードID` を候補として返す。

### 5.3 アウトライン形式（A 形式）— `plots/all.cnov`, `plots/AA.cnov`

状態: 実装完了（REF:src/core/syntax/plot_outline.rs, REF:tests/plot_outline.rs, REF:tests/golden.rs）

- 章ブロック `@AA` / 話ブロック `@AA-BB`、フィールド行と継続行（`drop_indent` で先頭タブ 1 文字または空白 2 文字を除去）、未知フィールド温存、無損失往復変換 `Outline::to_text` は `core::syntax::plot_outline`（`REF:src/core/syntax/plot_outline.rs`）に実装する（検証: `REF:tests/plot_outline.rs`, `REF:tests/roundtrip.rs`）。
- ブロック外（`;` の後から次の見出しの前）の行はどのブロックにも入れず `lines` に保持して `W011` / `W019` のみ検査し、`state` や `length` が不正形式のときは診断を出さず `Field::number()` 等が `None` を返して原文を温存する（`B-38`, `B-39`）。

### 5.4 話プロット形式（B 形式）— `plots/AA-BB.cnov`

状態: 実装完了（REF:src/core/syntax/plot_episode.rs, REF:tests/plot_episode.rs, REF:tests/golden.rs）

- 場面見出し `@A-B-C` と `;` で区切られる節ブロック、場面レベル・節レベルのフィールド（`pov`, `mode`, `state`, `material`, `foreshadow`, `payoff`, `note`）、自由記述行、無損失往復変換 `EpisodePlot::to_text` は `core::syntax::plot_episode`（`REF:src/core/syntax/plot_episode.rs`）に実装する（検証: `REF:tests/plot_episode.rs`, `REF:tests/roundtrip.rs`）。
- 場面レベルフィールド（`scene_field`）と最初の節（`section`）の境界は「最初の `point_line`（自由記述の要点行）が現れた位置（1 行もなければ最初の `;`）」で切る（コメント行と空行は場面フィールド領域を終わらせない、`B-41`）。

- **[Issue #29 / B10] 拡張子の改名のみ（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - A / B / C 形式の文法は一切変えず、パス判定とファイル名だけを `.txt` から `.cnov` へ変えた。

### 5.5 `characters.md`

状態: 実装完了（REF:src/core/syntax/characters_md.rs, REF:tests/characters_md.rs, REF:tests/golden.rs）

- H1 キャラ定義、固定 5 セクション（`attributes`, `tone`, `modes`, `relations`, `notes`）、H3 モード定義（`### <mode_id> <label>`）と同名 `mode_id` の `range:` 変種、役割語パターン行 `「...」` の抽出と無損失往復変換は `core::syntax::characters_md`（`REF:src/core/syntax/characters_md.rs`）に実装する（検証: `REF:tests/characters_md.rs`）。
- MD 方言 4 形式（`characters.md`, `words.md`, `ideas.md`, `prompts/*.md`）の共通パーサ部品は `REF:src/core/syntax/characters_md.rs` を正本として `pub(crate)` で共有する（`B-42`）。入れ子の外にある見出しやモード内の箇条書きは診断を出さず `lines` に保持し、`range:` の検査は「行順の `W011` → モード順の `E002` / `E016` → 最後に重複 `E017`」の順で行う（`B-43`, `B-44`）。

### 5.6 `words.md`

状態: 実装完了（REF:src/core/syntax/words_md.rs, REF:tests/words_md.rs, REF:tests/golden.rs）

- H1 用語定義、フィールド行（`reading`, `alias`, `category`, `first`, `note`, `color`）、自由記述、重複定義 `E015` の検出と無損失往復変換は `core::syntax::words_md`（`REF:src/core/syntax/words_md.rs`）に実装する（検証: `REF:tests/words_md.rs`）。

### 5.7 `ideas.md`

状態: 実装完了（REF:src/core/syntax/ideas_md.rs, REF:src/core/features/ideas.rs, REF:tests/ideas_md.rs, REF:tests/ideas_ops.rs）

- `# I#### タイトル` の解析・ID 重複検査（`E026`）・無損失往復変換は `core::syntax::ideas_md`（`REF:src/core/syntax/ideas_md.rs`）が担い、採番・状態変更・プロットへの昇格操作は `core::features::ideas`（`REF:src/core/features/ideas.rs`）が担う（検証: `REF:tests/ideas_md.rs`, `REF:tests/ideas_ops.rs`）。

### 5.8 資料

状態: 実装完了（REF:src/core/model/material.rs, REF:src/core/syntax/materials_index.rs, REF:tests/materials_index.rs）

- `MaterialId`（接頭辞 `M` / `S` + 4 桁番号）と `MaterialRecord` は `REF:src/core/model/material.rs` に定義し、`materials/index.json` のパース・未知キー温存・重複 ID 検査（`E026`）・整形出力は `core::syntax::materials_index`（`REF:src/core/syntax/materials_index.rs`）が担う（検証: `REF:tests/materials_index.rs`）。
- `serde_json` の `Value` はバイト位置を持たないため、`E024`（`path` の範囲外）や `E008`（重複 `id`）の診断位置は原文 JSON からキー名と値の文字列リテラルを検索する `literal_ranges` で算出し、閉じた集合の値（`status` 等）は未知値でデシリアライズ全体が失敗しないよう `String` で受けて `*_kind()` メソッドで enum へ変換する（`B-45`, `B-46`）。

### 5.9 `prompts/*.md`

状態: 実装完了（REF:src/core/syntax/prompt_md.rs, REF:src/core/agents/prompts.rs, REF:tests/prompt_md.rs, REF:docs/plans.md#plan-S-01）

- H1 ロール名、フィールド行（`version`）、H2 セクション（`system`, `task`, `output`）、`{{include:...}}` と 19 個の閉じた変数 `{{...}}` の検査（`W015` / `E029` / `E030`）は `core::syntax::prompt_md`（`REF:src/core/syntax/prompt_md.rs`）および `core::agents::prompts`（`REF:src/core/agents/prompts.rs`）に実装する（検証: `REF:tests/prompt_md.rs`）。
- `{{variable}}` の走査は行コメント（`//`）と空行を除く全行を対象とし、AST 上の `Variable::range` は行内相対、診断 `E031` のスパンは `line_offsets` を加算したファイル内絶対 byte range とする（`B-47`）。

- **[S-01] 参考例展開変数 `{{examples}}`（実装完了: `REF:docs/plans.md#plan-S-01`）**:
  - `REF:src/core/syntax/prompt_md.rs` の `VARIABLES` 定数配列に `"examples"` を追加し、`REF:src/core/agents/prompts.rs` の変数展開でタスクの参考例テキストを注入する。

### 5.10 `agents/` のファイル形式

状態: 実装完了（REF:src/core/agents/files.rs, REF:tests/agents_files.rs, REF:docs/plans.md#plan-S-01）

- タスク（`agents/tasks/T####.md`）、結果（`agents/results/T####.result.md`）、提案（`agents/proposals/P####.diff`、および人間用 `H####.diff`）、スレッド（`agents/threads/TH####.md`）、JSON Lines 監査ログ（`agents/log/YYYY-MM-DD.jsonl`）のパース・直列化・単調増加 ID 採番は `core::agents::files`（`REF:src/core/agents/files.rs`）および `core::features::proposals`（`REF:src/core/agents/files.rs`）に実装する（検証: `REF:tests/agents_files.rs`, `REF:tests/workspaces.rs`）。

- **[S-01] タスクの `examples:` フィールド（実装完了: `REF:docs/plans.md#plan-S-01`）**:
  - `REF:src/core/model/task.rs` の `Task` および `REF:src/core/agents/files.rs` のタスクパーサ・シリアライザに `examples: Vec<ExampleSpec>`（範囲式＋`contents`/`plot`/`both`）を追加する。

### 5.11 `agents/mcp.json`

状態: 実装完了（REF:src/core/agents/mcp_config.rs, REF:src/ui/app.rs, REF:tests/agents_files.rs, REF:tests/gui.rs）

- `agents/mcp.json` のパース、サーバ定義順の保持、`env` 内の `${VAR}` 環境変数展開と未定義エラー（`E030`）、リテラルの秘密情報の警告（`W033`）は `core::agents::mcp_config`（`REF:src/core/agents/mcp_config.rs`）に実装する（検証: `REF:tests/agents_files.rs`）。

- **[F-19] アプリ内 `mcp.json` 編集モーダル（実装完了: `REF:docs/plans.md#plan-F-19`）**:
  - `REF:src/ui/app.rs` の `Overlay` に `McpConfig` を追加し、`REF:src/core/agents/mcp_config.rs` の `parse` を用いて編集中テキストをリアルタイム検証した上で `agents/mcp.json` へ原子的に保存する。

### 5.12 フォーマッタ（正規化）仕様

状態: 実装完了（REF:src/core/features/formatting.rs, REF:src/ui/app.rs, REF:tests/features.rs）

- 行末空白除去、構造見出し直後の `1` 半角スペース正規化、フィールド行の `: ` 正規化（全角 `：` は保持）、連続空行の最大 2 行への圧縮、末尾 LF 1 個の保証は `core::features::formatting::format`（`REF:src/core/features/formatting.rs`）に実装し、コマンドパレット `>format`（`REF:src/ui/app.rs`）から呼び出す（検証: `REF:tests/features.rs`, `REF:tests/workspaces.rs`）。
