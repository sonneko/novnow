# novnow 設計書 — 意味解決エンジン

本書は `REF:docs/specifications/semantics.md` の各節に対応する実装設計を記述する。

## 6. 意味解決エンジン

### 6.1 辞書

状態: 未実装・実装計画済み（REF:src/core/resolve/dictionary.rs, REF:tests/dictionary.rs, REF:benches/core.rs, REF:docs/plans.md#plan-F-24）

- `characters.md`・`words.md`・`materials/index.json` の 3 供給元から `Dictionary`（`REF:src/core/resolve/dictionary.rs`）を構築する。
- 正規化関数 `normalize` は長音記号（`ー` `−` `―` `─` `ｰ` → `ー`）の統一を行った後に Unicode NFKC 正規化（`unicode-normalization`）を適用し、正規化後文字列から原文バイト位置への対応表を用いてトライ木（`Trie`）と `aho-corasick` オートマトンを構築する。
- 同名衝突の診断（`E014` / `E015` / `W027`）と長すぎるキー（`E028`）は辞書構築時に生成する（検証: `REF:tests/dictionary.rs`、ベンチマーク: `REF:benches/core.rs` の `dictionary_trie_5000`）。

- **[F-24] 登場人物の透過的な辞書統合と重複抑止（未実装・実装計画済み: `REF:docs/plans.md#plan-F-24`）**:
  - `REF:src/core/resolve/dictionary.rs` で `characters.md` の見出し名と `alias:` を無条件に統合辞書へ登録し（`words.md` ファイル自体は書き換えない）、`words.md` に同名の見出しが存在する場合は `Character` 定義へマージして `REF:src/core/features/reference.rs` や `REF:src/ui/palette.rs` に重複エントリが出ないようにする。

### 6.2 `'` の最短一致

状態: 未実装・実装計画済み（REF:src/core/resolve/dictionary.rs, REF:tests/dictionary.rs, REF:docs/plans.md#plan-S-09）

- `core::resolve::dictionary`（`REF:src/core/resolve/dictionary.rs`）が `'` トークンを走査し、明示境界 `'{…}` と通常形 `'名前[:修飾子]` を解析する。
- 通常形ではトライ木を 1 文字ずつ辿り、**最初に終端に達したキー**で確定する（最短一致）。終端に一度も達しない場合は助詞・句読点・空白・括弧・改行までを未解決キーとして切り出し、`E001`（または資料 ID 形なら `E012`）を生成する（検証: `REF:tests/dictionary.rs`）。

- **[S-09 / F-23] 未知の `'` 参照の形態素解析境界と自動回復（未実装・実装計画済み: `REF:docs/plans.md#plan-S-09`, `REF:docs/plans.md#plan-F-23`）**:
  - `REF:src/core/resolve/dictionary.rs` に文字種遷移（漢字・カタカナ・ひらがな・ASCII 英数字）および日本語助詞テーブルに基づく簡易形態素境界関数 `first_morpheme_end(text: &str) -> usize` を実装し、未解決参照 `E001` のスパン（赤波線範囲）を先頭 1 形態素に限定する。
  - `REF:src/core/features/code_action.rs` と `REF:src/ui/widgets/reference_pane.rs` で、切り出された 1 形態素を `words.md`（用語）または `characters.md`（登場人物）へワンクリック追加するアクションを提供する。

### 6.3 トークン種別（セマンティックトークン）

状態: 実装完了（REF:src/core/features/semantic_tokens.rs, REF:src/ui/theme.rs, REF:tests/features.rs）

- 文書全体のトークン分類は `core::features::semantic_tokens::tokenize`（`REF:src/core/features/semantic_tokens.rs`）が行い、`TokenType`（構造見出し、節区切り、キャラ参照、別名参照、用語参照、資料参照、未解決参照、モード修飾、会話、話者注釈、役割語一致下線、コメント、フィールドキー、ルビ）の列を返す。
- 各 `TokenType` から色・スタイルへの変換は `ui::theme::token_style`（`REF:src/ui/theme.rs`）が担う（検証: `REF:tests/features.rs`, `REF:tests/gui.rs`）。

### 6.4 モード解決

状態: 実装完了（REF:src/core/resolve/modes.rs, REF:tests/modes.rs）

- 優先順位（行内明示 `'キャラ:モード` > 行末 `@キャラ:モード` > B 形式節 `mode:` > B 形式場面 `mode:` > A 形式話 `mode:` > A 形式章 `mode:` > `characters.md` の `default-mode:`）と同名 `mode_id` の `range:` 変種選択は `core::resolve::modes`（`REF:src/core/resolve/modes.rs`）に実装する。
- 未定義モード（`E003`）、範囲外モード（`E004`）、3 個以上のモード併用または同一節 `mode:` での同一キャラ重複指定（`E005`）、複数変種重複一致（`E017`）、全変種範囲外（`W006`）を検出する（検証: `REF:tests/modes.rs`）。

### 6.5 役割語の照合

状態: 実装完了（REF:src/core/model/role_pattern.rs, REF:src/core/resolve/role_language.rs, REF:tests/role_language.rs, REF:benches/core.rs）

- パターン文字列とアンカー種別（`SentenceEnd`, `SentenceStart`, `Anywhere`, `Exact`）は `RolePattern`（`REF:src/core/model/role_pattern.rs`）が保持する。
- 5 アトム（`*`/`〜`, `…`/`...`/`‥`, `(語クラス)`, `?`, リテラル）から正規表現および下線算出用キャプチャグループ付き正規表現へのコンパイルと、`RegexSet` を用いた高速一括照合は `Matcher` / `MatcherSet` / `Cache`（`REF:src/core/resolve/role_language.rs`）に実装する（検証: `REF:tests/role_language.rs`、ベンチマーク: `REF:benches/core.rs` の `role_language_10_patterns`）。

### 6.6 資料 ID の解決とインレイヒント

状態: 実装完了（REF:src/core/resolve/materials.rs, REF:src/core/features/inlay_hint.rs, REF:src/ui/widgets/inlay.rs, REF:tests/materials.rs）

- 資料参照（`'M0137` / `'M0137:ラベル` / `'{資料タイトル}`）の解決と未解決診断（`E012`）は `core::resolve::materials`（`REF:src/core/resolve/materials.rs`）が担う。
- インレイヒント（`⟨タイトル⟩` の 12 字省略表示およびモード表示名 `:⟨日常⟩`）の算出は `core::features::inlay_hint::inlay_hints`（`REF:src/core/features/inlay_hint.rs`）が行い、エディタ上の隙間確保とオーバーレイ描画は `ui::widgets::inlay`（`REF:src/ui/widgets/inlay.rs`）が担う（検証: `REF:tests/materials.rs`, `REF:tests/gui.rs`）。

### 6.7 インクリメンタル再解析

状態: 未実装・実装計画済み（REF:src/core/text/incremental.rs, REF:tests/incremental.rs, REF:benches/core.rs, REF:docs/plans.md#plan-F-43）

- ファイル全体の行種別キャッシュ `LineIndex` と 1 行編集時の再解析 `relex_line` / `relex_text` は `core::text::incremental`（`REF:src/core/text/incremental.rs`）に実装する。
- 構造見出し `@` または節区切り `;` に関わる変更のみ `Renumber::From(line)` を返し、10 万行規模でも O(1) で 1 行再字句解析を完了する（検証: `REF:tests/incremental.rs`、ベンチマーク: `REF:benches/core.rs` の `single_line_relex`）。

- **[F-43] 打鍵時の編集中節のみの同期解析と他節デバウンス（未実装・実装計画済み: `REF:docs/plans.md#plan-F-43`）**:
  - `REF:src/ui/workbench.rs` と `REF:src/core/features/project.rs` で、打鍵フレーム内では編集中の 1 節スライスのみを対象にトークン化・ローカル診断・インレイヒント計算を行い、ファイル内他節の再構築とプロジェクト横断診断は 300ms デバウンス後に実行する。
  - `REF:tests/incremental.rs` および `REF:benches/core.rs` に「10 万字のファイルで 1 打鍵 16ms 以内」を検査するテストを追加する。

## 付録 B: 役割語パターン言語と、提示された例の変換表

### B.1 変換規則（§6.5.1 の再掲）

状態: 実装完了（REF:src/core/resolve/role_language.rs, REF:tests/role_language.rs）

- `WILDCARD_REGEX`（`[^
]*?`）、`ELLIPSIS_REGEX`（`[…‥.。]{1,6}`）、`ANY_CHAR_REGEX`（`[^
]`）、および `Lexicon::word_classes` の展開は `core::resolve::role_language::to_regex`（`REF:src/core/resolve/role_language.rs`）に実装する。

### B.2 提示された役割語の v1 表記

状態: 実装完了（REF:src/core/syntax/characters_md.rs, REF:tests/role_language.rs）

- 14 キャラクターの役割語パターン表記は `characters_md` パーサと `role_language::MatcherSet` でそのまま解釈され、`REF:tests/role_language.rs` で検証している。

### B.3 変換結果の例（テストの期待値）

状態: 実装完了（REF:tests/role_language.rs）

- 表の 8 行すべての変換後正規表現・一致例・下線部分・不一致例は `REF:tests/role_language.rs` で一字一句固定して検証している。

### B.4 下線範囲の決定

状態: 実装完了（REF:src/core/resolve/role_language.rs, REF:tests/role_language.rs）

- ワイルドカード（`*` / `〜`）以外の連続アトムをキャプチャグループで囲んだ正規表現を内部生成し、複数パターン一致時は区間和集合（マージ済み `Vec<Range<usize>>`）を `MatcherSet::underlines`（`REF:src/core/resolve/role_language.rs`）が返す。
