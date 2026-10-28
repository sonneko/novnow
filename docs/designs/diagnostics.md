# novnow 設計書 — 診断カタログ

本書は `REF:docs/specifications/diagnostics.md` の各節に対応する実装設計を記述する。

## 7. 診断カタログ

- **`W041`（読み取り専用）を実装した**: 読み取り専用の判定は `REF:src/ports/storage.rs` の `Storage::writable()`（既定 `true`）と `REF:src/app/host.rs` の `Host::writable()`（既定 `true`）が持ち、`REF:src/adapters/storage_fs.rs`（と `REF:src/app/fs_host.rs`）が「プロジェクトの中に空の `.novnow/.write-probe` を作って消す」1 回の試し書きで決める（プロセスごとに 1 回だけ。`std::sync::OnceLock`）。結果は `REF:src/app/session.rs` の `Session::read_only()` が覚え、次の 3 か所で W041 になる: `REF:src/cli/check.rs` の 1 行（`<ルート>: W041 文言`）、`REF:src/app/ops/project.rs` の `validate` と `REF:src/app/ops/lang.rs` の `diagnostics_run`（プロジェクト全体のとき、`path` は空・`line` / `column` は 0 の 1 件）、`REF:src/ui/app.rs` の `prepare_agents`（ステータスバーの通知）。読み取り専用のときは既定プロンプトの書き出し（I050）もしない。読み取り専用の判定は `.novnow/` の中だけを触り、失敗しても何も残らない（§13.1）。
- `I051`（未保存の変更がある）と `I058`（検索結果を資料化した）は**廃止**した。どちらも画面にすでに出ている事実（保存状態のインジケータ、資料一覧の新しい行）の写しで、診断として二重に持つ意味が無かった。`DiagnosticCode` から variant を消し、日英カタログ・サイト解説データからも消した（番号は再利用しない）。
- 現在、カタログに定義があり実装が発行しないコードは無い。`W022`（用語の `range`）・`W030`（話プロットと本文の節数）は `REF:src/core/diagnostics.rs` の `word_ranges` / `section_counts` が、`W025`・`W064`（資料 MD と索引）は索引の突き合わせが発行する（`REF:tests/diagnostics_document.rs` が W022 / W025 / W030 を、`REF:tests/materials_ops.rs` が W064 を見る）。

### 7.0 コア・構文・解決・整合の診断

状態: 実装完了（REF:src/core/diagnostics.rs, REF:src/core/features/project.rs, REF:tests/diagnostics_catalog.rs, REF:tests/diagnostics_document.rs）

- コア・構文・解決・整合の 39 コード（`E001`〜`E030`, `W001`〜`W015`, `I001`〜`I002`）は `DiagnosticCode`（`REF:src/core/diagnostics.rs`）に列挙し、`CATALOG` 定数で重大度・グループ・引数個数を管理する。
- 文書単位およびプロジェクト横断の診断収集は `core::features::diagnostics`（`REF:src/core/features/project.rs`）が担う。
- `REF:tests/diagnostics_catalog.rs` が `REF:docs/specifications/diagnostics.md` を `include_str!` で直接パースし、全コードの個数・重大度・引数プレースホルダ・variant の doc コメントの一致を機械的に検証する。
- **診断生成・整列・横断検査の内部規約（`REF:docs/history.md` の `A-22`, `B-25`〜`B-30`, `B-72`〜`B-77`）**:
  - `core` は診断メッセージ文字列や `Display` を持たず、`DiagnosticCode` と構造化引数 `Vec<String>` のみを返し、日本語メッセージへの整形は `ui::i18n`（`REF:src/ui/i18n/mod.rs`）が担う（`B-29`）。
  - `Diagnostic::range` はファイル先頭からの byte offset の半開区間とし、1 基点の `(line, col)`（`col` は UTF-8 文字数＝`chars().count() + 1`）への変換は `LineIndex::line_col` で行う（`B-28`）。
  - A 形式（`plots/all.cnov` と `plots/AA.cnov`）の両方に同じ話ブロック `@AA-BB` がある場合は、より詳細な章別ファイル `plots/AA.cnov` を正とし、`E018` / `W002` の related 位置もそちらを指す（`B-72`）。
  - 会話の話者・役割語検査（`W003` / `W004` / `W005`）では、単一引用符 `'…'`（改行を跨がない閉じたペア）を地の文から除外した上で括弧の開閉を走査し、話者が特定された場合は全定義パターンとの不一致で `W004`、話者未指定の場合は `pov`（なければその節の `mode:` に登場するキャラ集合）を候補として `W003` / `W005` を判定する（`B-73`）。
  - 伏線グラフの `I001`（伏線と回収のペア成立）は伏線側・回収側の双方で参照でき、かつ診断一覧で件数が倍増しないよう、回収側に 1 件のみ出して `related` に伏線側の位置を入れる（`A-22`）。
  - `cli::check` の JSON 出力と text 出力はともに `(path, range.start, code)` の辞書順でソートして決定性を保証し（`B-76`）、`--fix` は安全フラグ付きの `QuickFix` のみを末尾側から自動適用する（`B-86`, `B-102`）。

- **[Issue #29 / B10] 有効性とエラー扱いの上書き（実装完了: `REF:docs/plans.md#issue-29-b10`）**:
  - 組み込み既定（表の深刻度と既定）は現行定数のままにし、`diagnostics::document` の末尾で `[features.diagnostics]`（仕様 §4.5.2）の解決結果を適用する。解決は `level_for`（`REF:src/core/diagnostics.rs`: 個別コード → グループ `structure` / `storage` / `writing` → `default`、値でない文字列は次へ進む）で、`"off"` は落とし、`"warn"` / `"error"` は `Diagnostic.severity_override` に写す（`severity()` が実効重大度を返す）。コアは解決済みの値だけを見て、設定ファイルの書式は知らない。
  - `"error"` の診断は `check` の終了コード 5 の対象に含まれ（`found.severity()` で数える）、`"off"` の診断は生成しない（件数にも含めない）（検証: `REF:tests/diagnostics_document.rs` の `features_diagnostics_overrides_follow_4_5_2`）。

### 7.1 ストレージ・エージェント系（文書位置を持たない通知）

状態: 実装完了（REF:src/core/diagnostics.rs, REF:src/adapters/storage_fs.rs, REF:src/adapters/storage_idb.rs, REF:tests/diagnostics_catalog.rs）

- ストレージ・エージェント系の 10 コード（`E060`〜`E063`, `W060`〜`W065`, `I050`〜`I052`）は `Anchor::Status`（文書位置を持たないステータスバー通知）として `STATUS_ANCHOR`（`REF:src/core/diagnostics.rs`）に定義する。
- 規則 10 違反（`E060`）、IndexedDB 利用不可（`E062`）、保存失敗（`E063`）、容量逼迫（`W063`）、資料変更検知（`W064` / `W065`）、プロンプト初期化（`I050`）などを各アダプタ・ワークスペースから発行する。
- `WritePolicy::check`（`REF:src/core/policy/write_policy.rs`）はデバッグビルドでは `debug_assert!` で即座にパニックし、リリースビルドでは `Err(WriteViolation::ForbiddenTarget)`（`E060`）を返してシステムコールを発行しない（`B-20`, `B-107`）。

### 7.2 執筆支援系の診断

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:src/core/diagnostics.rs, REF:tests/diagnostics_document.rs, REF:docs/plans.md#issue-29-b11, REF:docs/plans.md#issue-29-b12）

- 執筆支援系の 37 コード（`W080`〜`W106`, `E032`〜`E036`, `E064`, `I054`。`I058` は廃止）は `Group::Writing` として `REF:src/core/diagnostics.rs` に定義する。
- 既定の有効（`TOGGLE_ON`: 11 件）/ 無効（`TOGGLE_OFF`: 18 件）および実行コスト制御テーブル（`COST`: 27 件の `Trigger` × `Scope`）は `REF:src/core/diagnostics.rs` に定数表として持ち、`REF:tests/diagnostics_catalog.rs` で仕様表との完全一致を検証する。
- 執筆支援診断（`W081` 文長、`W082` 連続文末、`W083` 話者不明会話、`W084` 非定義視点、`W085` 助詞連続、`W086` 指示語過多、`W087` 漢字率、`W094` 台詞長、`W098` 表記揺れ、`E032`〜`E036` / `E064` / `I054` エージェント系）は `REF:src/core/features/diagnose.rs` と `REF:src/core/agents/runner.rs` で発行する。`when:` / `senses:` / `space:` などに依存するコード（`W068`〜`W070`, `W084`, `W085`, `W092`）も同じ経路で発行する。
- 執筆支援診断の算出規約（`REF:docs/history.md` の `B-78`, `B-79`）:
  - `W082`（連続文末）は段落内の文ごとに末尾の句点・感嘆符・閉じ括弧を除いた最後の 2 文字（ひらがな・カタカナの語尾）を比較し 3 文連続一致で警告する。
  - `W085`（助詞連続）は文内の平仮名 1 文字助詞（`の` / `が` / `を` / `に` / `で` / `と` / `は`）の出現間隔と回数を検査し、`W087`（漢字率）は段落単位（50 字以上）の CJK 統合漢字比率が `project.toml` の範囲（既定 20%〜45%）を外れた場合に報告する。

- **[S-15] 全診断コードの Web サイト解説と CI 網羅検査（実装完了: `REF:website/src/pages/[lang]/diagnostics/[code].astro`, `REF:scripts/diagnostics-site.py`）**:
  - `REF:website/src/data/diagnostics.ts` は仕様表から生成し、`REF:website/src/data/diagnostics-en.json` と `REF:website/src/data/diagnostics-examples.json` に全 85 コードの日英の原因・例・対処法を置く。`REF:website/src/pages/[lang]/diagnostics/[code].astro` でコードごとの解説ページを生成し、`REF:scripts/diagnostics-site.py` の `--check` を CI（`REF:.github/workflows/ci.yml`）で実行して仕様表とのコード集合一致と内容の非空性を検査する。

- **[Issue #29 / B11・B12] 執筆支援ラウンド 2・3（W097〜W106、実装完了）**:
  - W097（文末尾の反復）・W098（場面内の同語反復）・W099（隣接同語反復）・W100（文末助詞の反復）・W101（文頭語の反復）・W102（波ダッシュの多用）・W103（不確か表現の多用）・W104（同一動詞語幹の反復）・W105（感嘆符の多用）・W106（ダッシュの多用）を `REF:src/core/features/diagnose.rs` に追加した。
  - 語の抽出は導入済みの `tiniestsegmenter`（§6.2.6）のトークンと、固定の語彙リスト（文末除外リスト・文末助詞・不確か表現・活用接尾辞）で行う。core の日本語リテラルは語彙マーカー（`/// i18n: format-vocabulary`）を付けた `const` に閉じる（§12.2）。
  - 節単位の診断（W097 / W099 / W100 / W101 / W102 / W106）は節内の複数行を改行で連結して数え、行位置は `SectionText` で本文の byte 位置へ戻す。場面単位（W098 / W103 / W104 / W105）は場面の全節を集計する。日本語主体の判定は §7.2.2 と同じ（文書ごとに 1 回）。
  - 有効化は `[features.diagnostics]`（§4.5.2）で行い、組み込み既定は無効。文体タブの明示操作（`explicit`）では有効化なしでも計算する（W096 と同一）。受け入れ条件は `REF:tests/diagnostics_document.rs`（閾値の境界・集計単位・台詞の除外・1 字除外・サンプルで発火しないこと）で固定した。

- **[Issue #29 / B7] トグルの feature 化と W096 の追加（実装完了: `REF:docs/plans.md#issue-29-b7`）**:
  - セッション内トグルだった有効/無効は「組み込み既定」（`TOGGLE_ON` / `TOGGLE_OFF` 定数）として残し、作者の上書きは `[features.diagnostics]`（§7.0、B10）が持つ。点検画面の編集面は §10.9（`REF:docs/designs/ui.md`）。
  - W096（台詞内の句読点直後の空白、§7.2.2）を追加し、仕様 §7.2 の表・コード・日英カタログ・サイト解説データ・コスト/トグルの定数表を同時に揃えた（77 件）。生成は `features::diagnose::dialogue_spacing`（`REF:src/core/features/diagnose.rs`）で、記号（`。`・`！`・`？`）ごとに作品全体から集計し、合計 5 件以上かつ差 2 件以上のときだけ少数派の箇所へ出す。組み込み既定は無効で、有効化は `[features.diagnostics]`。文体タブの明示操作では未有効化でも計算する。対象判定（日本語主体か）は本文の文字種だけで決める。クイックフィックスは空白を入れる／取る 1 操作（`MatchDialogueSpacing`、`REF:src/core/features/code_action.rs`）。

#### 7.2.3 文末尾の反復（W097）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- 文を導入済みの `tiniestsegmenter` で分詞した最後の自立語（末尾の助詞を固定の除外リストで除く）を文末尾とし、同一節内で同じ文末尾が 3 文連続（改行・段落は連続を切らない、節をまたがない。`「…」` の内側は数えない）で、3 文目の位置に 1 件（仕様 `REF:docs/specifications/diagnostics.md` §7.2.3・算法 `REF:docs/specifications/ui.md` §9.4.4）。
- 診断 enum（`REF:src/core/diagnostics.rs`）へ追加（severity W・既定オフ）。§9.4.3 と同一の文区切りで読み順に文末尾キーを収集する。クイックフィックスは連続 3 文を表示（自動置換しない）。日本語主体の判定は §7.2.2（W096）と同一。

#### 7.2.4 場面内の同語反復（W098）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- W087 と同じ語の抽出（`tiniestsegmenter` の 2 字以上トークン）を集計単位を場面（その場面内の全節）にして適用し、5 回以上で最後の出現位置に 1 件（仕様 `REF:docs/specifications/diagnostics.md` §7.2.4・`REF:docs/specifications/ui.md` §9.4.4）。
- `REF:src/core/features/diagnose.rs` で W087 の語抽出を流用し場面ごとに集計する。クイックフィックスは用語辞書で類語を見る（W087 同一）。日本語主体の判定は §7.2.2（W096）と同一。

#### 7.2.5 隣接同語反復（W099）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- `REF:src/core/features/diagnose.rs` のトークン列（W087 と同じ `tiniestsegmenter` 呼び出し）で、同じ語が助詞・句読点のみを挟んで 2 回連続で現れたら 2 回目の位置に 1 件（同じ連続区間に追加はしない）。1 字のトークンは固定の 1 字助詞リストに無いもののみ対象（仕様 §7.2.5）。

#### 7.2.6 文末助詞の反復（W100）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- W097 と同じ文区切り（§9.4.3）で各文の末尾を固定の 1 字 / 2 字助詞リストで分類し、同じ助詞の文が 3 連続で 3 文目の位置に 1 件（仕様 §7.2.6）。

#### 7.2.7 文頭語の反復（W101）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- 各文の先頭トークン（2 字以上）を取り、同じ語が 3 文連続で 3 文目の位置に 1 件（仕様 §7.2.7）。文区切りは W097 と、セグメンタ呼び出しは W087 とを流用する。

#### 7.2.8 波ダッシュの多用（W102）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- 1 節内の `……`（U+2026 を 2 つ連続）の非重複出現を数え、3 回以上で 3 回目の出現位置に 1 件（仕様 §7.2.8）。

#### 7.2.9 やわらか・不確か表現の多用（W103）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- 固定リスト 8 語（仕様 §7.2.9）を表記の完全一致で数え、同じ語が 1 場面で 3 回以上で語ごとに最後の出現位置へ 1 件。場面単位の集計は W098 の経路を流用する。

#### 7.2.10 同一動詞語幹の反復（W104）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- セグメンタの各トークンから固定の活用接尾辞リスト（仕様 §7.2.10）を末尾から最長一致で取り除いて語幹（2 字以上）を得、同じ語幹が 1 場面で 5 回以上で最後の出現位置に 1 件。リストは小さく保って決定論を保つ。

#### 7.2.11 感嘆符の多用（W105）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- 1 場面内の全角 / 半角 `!` を数え、5 回以上で 5 回目の出現位置に 1 件（仕様 §7.2.11）。

#### 7.2.12 ダッシュの多用（W106）

状態: 実装完了（REF:src/core/features/diagnose.rs, REF:tests/diagnostics_document.rs）

- 1 節内の `——`（U+2014 を 2 つ連続）の非重複出現を数え、4 回以上で 4 回目の出現位置に 1 件（仕様 §7.2.12）。

共通: 新規依存のない決定論ルールで、組み込み既定は無効（有効化は `[features.diagnostics]`）。実装と同時に、仕様 §7.2 の表行・ja/en カタログ・サイト解説（§7.2.1）・「77 種」の数値主張（`REF:docs/specifications/overview.md` §2・`REF:README.md`・`REF:README.en.md`・`REF:docs/users/glossary.md`）を 79 種へ。なお `tiniestsegmenter`（TinySegmenter）は `REF:docs/specifications/semantics.md` §6.2.6 とともに導入済みの依存であり（W087 の実装が使用中）、今回は新規依存を足さないだけ（見送りの根拠だった WASM 検査・依存搬入のコストは発生の余地がなかった。N29-39 第 16 回決定の許容範囲内）。W099–W106（B12）も同じく決定論で新規依存なし（語抽出は導入済みの `tiniestsegmenter` を再利用）。実装と同時に、仕様 §7.2 の表行 8 件・ja/en カタログ・サイト解説・トリガー表・「79 種」の数値主張を 87 種へ更新し、本節の状態を「実装完了」へ戻す（全 142 節の完了）。