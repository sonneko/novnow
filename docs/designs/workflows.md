# novnow 設計書 — 資料収集・エクスポート・性能品質

本書は `REF:docs/specifications/workflows.md` の各節に対応する実装設計を記述する。

## 16. 資料収集機能

### 16.1 ワークフロー

状態: 実装完了（REF:src/core/features/reference.rs, REF:src/ui/workspaces/material.rs, REF:tests/materials_ops.rs, REF:docs/plans.md#plan-F-31）

- 資料の新規作成（提案 `proposed` / 承認 `approved`）、採番（`M####`）、索引登録、MD テンプレート生成は `core::features::reference`（`REF:src/core/features/reference.rs`）および `REF:src/ui/workspaces/material.rs` に実装する。

- **[F-31] 新規資料作成の全域有効化とボタン設置（実装完了: `REF:docs/plans.md#plan-F-31`）**:
  - `REF:src/ui/workspaces/material.rs` でペインフォーカスにかかわらず `Ctrl+Shift+N` と画面上のボタンで新規資料を作成可能にする。

### 16.2 インデックス（§5.8.2 の運用規則）

状態: 実装完了（REF:src/core/syntax/materials_index.rs, REF:src/core/features/project.rs, REF:tests/materials_ops.rs）

- `materials/docs/` 内の未登録ファイル検出（`W064`）と索引への自動追加クイックフィックス、および索引登録済みで実ファイルが欠落している項目の検出（`W065`）は `REF:src/core/features/project.rs` と `REF:src/core/features/reference.rs` に実装する（検証: `REF:tests/materials_ops.rs`）。

### 16.3 検索 UX

状態: 実装完了（REF:src/core/features/reference.rs, REF:src/ui/app.rs, REF:tests/materials_ops.rs, REF:tests/workspaces.rs）

- `Ctrl+M` 資料検索モーダルのクエリ解析（`tag:`, `cat:`, `id:`, `scene:`, `status:`, `!status:`）と決定論的スコアリング（ID 完全一致 +500、タイトル先頭一致 +400、部分一致 +250、タグ完全一致 +200、要約一致 +100、本文一致 +50）、および `Enter`（参照挿入）/ `Shift+Enter`（`material` で開く）/ `Ctrl+Enter`（新規提案作成）は `REF:src/core/features/reference.rs` と `REF:src/ui/app.rs` に実装する。

- **[Issue #29 / B6] 検索結果の表示と保存の分離（実装完了: `REF:docs/plans.md#issue-29-b6`）**:
  - 正規表現検索（`REF:src/core/features/search.rs`）の抜粋は「前文・一致・後文」の構造（ `Hit.before` / `matched` / `after` ）で返し、画面は一致範囲だけを強調して描く。資料として保存する MD は §16.3.1 の固定形（タイトル見出し＋ `<!-- novnow:S#### -->`、`query` / `regex` / `case_sensitive` / `searched_at` / `hits`、節 ID 付きの結果箇条書き、500 件上限の註記）を `snapshot_body` で組み立てる。表示用の記法（ `**…**` ）は保存時にだけ使う（検証: `REF:tests/stage_two.rs` の `search_excerpts_are_structured_and_snapshots_use_the_fixed_form`）。

### 16.4 「必要になる条件」と自動提示

状態: 実装完了（REF:src/core/resolve/materials.rs, REF:src/core/features/reference.rs, REF:tests/materials.rs, REF:tests/materials_ops.rs）

- `scenes` 範囲一致・`characters` 登場一致・`keywords` 出現一致による自動提示候補の抽出と、参照ペインの `[候補]` 表示および `Enter` による話プロット `material:` 行への昇格は `REF:src/core/resolve/materials.rs` と `REF:src/core/features/reference.rs` に実装する。

### 16.5 プロットからの参照と表示

状態: 実装完了（REF:src/core/features/reference.rs, REF:src/ui/widgets/reference_pane.rs, REF:src/ui/widgets/mod.rs, REF:tests/context_rules.rs, REF:tests/gui.rs）

- 話プロット・章プロットの `material:` フィールドから解決された資料のタイトル・要点（最大 5 行）の抽出は `REF:src/core/features/reference.rs` が担い、`REF:src/ui/widgets/reference_pane.rs` が描画する。

- **[F-34 / S-07] 要約の「続き...」表示と資料 URL の外部リンク化（実装完了: `REF:src/ui/widgets/reference_pane.rs`, `REF:src/ui/widgets/mod.rs`, `REF:tests/gui.rs`）**:
  - `REF:src/ui/widgets/reference_pane.rs` と `REF:src/ui/workspaces/material.rs` に切り詰め時の「`続き...`」行表示（F-34）および URL の外部リンクアイコン＋下線＋ホバー青色表示（S-07）を設ける。両方とも `REF:tests/gui.rs` で表示・操作を検証済み。

### 16.6 資料 MD の編集

状態: 実装完了（REF:src/core/text/sha256.rs, REF:src/core/features/reference.rs, REF:src/ui/workspaces/material.rs, REF:tests/materials_ops.rs）

- 資料 MD 保存時の SHA-256 チェックサム計算（`REF:src/core/text/sha256.rs`）と `materials/index.json` の `checksum` / `updated_at` 自動更新は `REF:src/core/features/reference.rs` に実装する。

### 16.7 エージェントとの連携

状態: 実装完了（REF:src/core/agents/server.rs, REF:src/core/agents/apply.rs, REF:tests/mcp_stdio.rs, REF:tests/agents_files.rs）

- `researcher` ロールおよび MCP `novnow.material.propose` からの資料提案登録（`status: proposed`）と `material` ワークスペースでの承認・却下は `REF:src/core/agents/server.rs` と `REF:src/ui/workspaces/material.rs` に実装する。

## 17. エクスポート

### 17.1 アノテーション除去パイプライン

状態: 実装完了（REF:src/core/export.rs, REF:tests/export.rs, REF:tests/export_invariants.rs）

- 14 段のアノテーション除去パイプライン（対象節収集、構造見出し除去/挿入、コメント除去、行末話者注釈除去、`'` 参照の平文化、資料参照の完全除去、エスケープ解除、ルビ処理、節区切り変換、連続空行圧縮、行頭字下げ、改行コード変換、末尾整形）は純粋関数 `core::export::run`（`REF:src/core/export.rs`）に実装する（`B-98`, `B-99`）。
- エクスポート不変条件（出力に `@` 見出し・`//`・`/* */`・解決済み `'`・資料 ID・単独 `;` 行が残らないこと）は `REF:tests/export_invariants.rs` で検証する。

### 17.2 範囲指定と出力

状態: 実装完了（REF:src/core/export.rs, REF:src/cli/export.rs, REF:src/ui/app.rs, REF:tests/export.rs, REF:tests/cli.rs）

- 範囲式（`@` 省略形を含む `parse_range`）と確定度フィルタ（`effective_state` による `draft` / `review` / `fixed` 絞り込み）、既定ファイル名生成 `file_name` は `REF:src/core/export.rs` に実装し、CLI `novnow export`（`REF:src/cli/export.rs`）および GUI `>export` モーダル（`REF:src/ui/app.rs`）から呼び出す（`B-100`, `B-101`）。

### 17.3 読者モード（`reader` ワークスペース）

状態: 実装完了（REF:src/core/reader.rs, REF:src/ui/workspaces/reader.rs, REF:tests/stage_two.rs）

- 組版は `REF:src/core/reader.rs` の純粋関数 `layout(text, options, metrics)`。実測送り幅と縦書き字形の有無を引数として注入し、禁則・縦中横・Unicode 縦字形・回転・モノルビを座標列へ変換する。
- 描画とページ操作・章表紙・目次・しおりは `REF:src/ui/workspaces/reader.rs`。原稿と章プロットの指紋および組版設定をキーにページをキャッシュし、ページ送りで再組版しない。
- 読書位置と表示設定はローカル状態のみ。未知キーを保持し、原稿の変更経路とは分離する。

- **[Issue #29 / B6] 縦書き括弧の豆腐解消（実装完了: `REF:docs/plans.md#issue-29-b6`）**:
  - 括弧の互換形置換は、使用フォントに置換先の字が収録されている場合だけ行う。UI（`REF:src/ui/workspaces/reader.rs`）は組版のキャッシュ再構築時に egui の `Fonts::has_glyph` で `VERTICAL_FORMS` の互換形を確かめ、`Metrics.vertical_forms` へ渡す。組版（`REF:src/core/reader.rs`）は収録のある字だけ互換形へ置換し、無い字は元の括弧を 90° 回転して描くグリフにする。同梱の IPA ゴシックには互換形が 1 字も無い（実測）ため、同梱フォント使用時は常に回転描画になる。
  - 検証は `REF:tests/stage_two.rs` の `reader_brackets_substitute_only_when_the_font_has_the_form`（収録集合に対する組版の分岐）と `REF:tests/gui.rs` の `bundled_font_bracket_coverage_drives_reader_substitution`（同梱フォントの実収録と組版結果の一致）で行う。

## 18. 性能・品質要件

### 18.1 性能目標（既定のリファレンス環境: 4 コア / 8GB / 統合 GPU）

状態: 実装完了（REF:benches/core.rs, REF:tests/incremental.rs, REF:tests/role_language.rs, REF:.github/workflows/ci.yml, REF:docs/plans.md#plan-F-43）

- 主要 5 処理の `criterion` ベンチマークは `REF:benches/core.rs` に実装している（リリース実測値: `single_line_relex` 約 462ns、`role_language_10_patterns` 約 1.89µs、`dictionary_trie_5000` 約 1.14ms、`reference_pane_rebuild` 約 38.7µs、`export_100k_chars` 約 7.37ms、`B-155`）。
- Web リリースビルドの gzip 後 wasm サイズ（上限 8MiB、実測約 6.75MiB）は `REF:.github/workflows/ci.yml` の `wasm` ジョブで検査する。

- **[F-38 / F-43 / S-12] 10 万字打鍵 16ms 以内・アイデア 1,000 件 60fps・文脈ビュー可視範囲レイアウト（実装完了: `REF:docs/plans.md#plan-F-43`）**:
  - 打鍵時の編集中節限定解析（F-43）、一覧の `show_rows` 仮想スクロール（F-38）、文脈ビューのビューポートカリング（S-12）を実装し、`REF:tests/editor_performance.rs`（release で画面全体の打鍵 16ms とアイデア 1,000 件の一覧移動 16ms を確認）、`REF:tests/incremental.rs` と `REF:benches/core.rs` で計測・検証する。

### 18.2 メモリ

状態: 実装完了（REF:src/core/text/rope.rs, REF:src/core/text/incremental.rs）

- 本文バッファは `ropey::Rope` ラッパ `TextBuffer`（`REF:src/core/text/rope.rs`）で保持し、行種別インデックス `LineIndex`（`REF:src/core/text/incremental.rs`）は 1 行 1 バイトの `LineTag` のみを保持する。

### 18.3 堅牢性

状態: 実装完了（REF:src/main.rs, REF:src/adapters/env_system.rs, REF:tests/layering.rs, REF:tests/roundtrip.rs）

- 全パーサは不正入力でもパニックせず部分木と診断を返す（`REF:tests/roundtrip.rs`, `REF:tests/golden.rs`）。
- `unsafe` ブロックは Windows FFI の 2 箇所（`REF:src/main.rs` の `AttachConsole`、`REF:src/adapters/env_system.rs` の `GetUserDefaultLocaleName`）のみに限定し、それ以外の `unsafe` がリポジトリ内に存在しないことを `REF:tests/layering.rs`（`unsafe_is_restricted_to_the_two_windows_ffi_sites`）で機械検査する。

### 18.4 継続的検証要件

状態: 実装完了（REF:tests/layering.rs, REF:.github/workflows/ci.yml, REF:scripts/verify.sh, REF:docs/script/duplicate.py, REF:docs/plans.md#plan-S-15）

- CI ワークフロー（`REF:.github/workflows/ci.yml`）の 5 ジョブ（`fmt`, `clippy`, `test`, `wasm`, `bench`）およびローカル検証スクリプト `REF:scripts/verify.sh`、ドキュメント規約検査スクリプト `REF:docs/script/duplicate.py` により全変更を機械的に検証する。

- **[F-30 / F-32 / F-42 / F-43 / S-15] 追加の CI・自動テスト検証項目（実装完了: `REF:docs/plans.md#plan-S-15`）**:
  - `REF:tests/gui.rs`（押せる要素の下線・ホバー・24×24px 当たり判定検査）、`REF:tests/i18n_catalog.rs`（`ja.toml` 助詞前空白検査）、`REF:tests/editor_performance.rs`（10 万字での画面全体の 1 打鍵 16ms 検査。CI の bench ジョブは release で実行）、および `REF:scripts/diagnostics-site.py`（全診断コードの日英解説と具体例の網羅性 CI 検査）を追加する。

### 18.5 ドキュメント公開（仕様書とユーザー向けガイド）

状態: 実装完了（REF:scripts/docs-site.py, REF:docs/script/reference.py, REF:src/core/model/project.rs, REF:docs/plans.md#plan-docs-site, REF:docs/users/, REF:guide/）

- **正本は 1 か所だけ**: `REF:docs/users/*.md` と `REF:docs/specifications/*.md` が正本で、サイトのページは
  その生成物。Markdown を書き写した複製を `REF:website/` に置かない。
- **生成器**: `REF:scripts/docs-site.py` が正本を `REF:website/src/content/docs/users/` と
  `REF:website/src/content/docs/specifications/` へ、先頭に `title`・`group`・`order`・`source` の
  フロントマターを付けて写す。あわせて目次 `REF:website/src/data/docs-nav.ts`（`manualNav`・
  `specificationNav`）を書き出す。`REF:scripts/diagnostics-site.py` と同じ形（既定は生成、
  `--check` は再生成して差分があれば終了コード 1、余剰の生成物は削除）にそろえてある。
- **URL**: ユーザー向けガイドは `/ja/manual/<slug>`（`README.md` は `/ja/manual/`）、仕様書は
  `/ja/specifications/<name>`（`README.md` は `/ja/specifications/`）。slug はファイル名から `.md` を
  外したもの。ページの描画は `REF:website/src/components/DocShell.astro`（左に目次、本文、
  「このページは `<source>` から自動生成」の注記）と、`REF:website/src/pages/ja/manual/[slug].astro`・
  `REF:website/src/pages/ja/specifications/[slug].astro` が担う。
- **リンクの書き換え**: `./x.md` → `/ja/manual/<slug>`、`./README.md` → `/ja/manual/`、
  `../specifications/x.md` → `/ja/specifications/<slug>`。外部 URL と `#アンカー` はそのまま残す。
  これで「ガイドから仕様書へ」の向きだけが生まれる。
- **アンカー**: 見出しの文字列から GitHub と同じ規則でアンカーを作る。日本語の見出しは URL に
  パーセントエンコードされて読みにくく、環境によっても変わるため、飛び先には
  `<a id="install">` のような英字 ID を置き、本文のリンクもその ID を使う（`start.md` の
  `#install`・`#write-first-section`・`#export`）。生成器は見出しと `<a id>` の両方からアンカー集合を
  作り、**存在しないアンカーを指すリンクがあればエラーにする**。
- **英語**: ガイドと仕様書の本文は日本語のみ。`/en/manual/<slug>` と `/en/specifications/<slug>` は
  「日本語で書かれている」ことと日本語版へのリンクを出す短いページにして、英語話者を空振りさせない。
- **検査**: `REF:scripts/verify.sh` の `site-check` が `REF:scripts/docs-site.py` を `--check` 付きで
  実行する。docs を直して生成し忘れた状態と、生成物を手で編集した状態の両方が落ちる。
- **アプリ内の解説ページ**: 同じ生成器が `docs/users/*.md` から `REF:guide/*.html`（8 ページ）を作る。
  `REF:index.html` の `copy-dir` で trunk の dist に載るので、仕様 §14.2.1 の 3 URL
  （`guide/project_structure.html`・`guide/git.html`・`guide/publish.html`）が公開物に実在する。
  公式サイト側でも同じ URL で開けるよう、`REF:website/public/guide/` にも同じ HTML を置く。
- **検査 11**: `REF:docs/script/reference.py` の 11 番目が、(1) 3 URL の実体と `copy-dir` の指定、
  (2) 仕様 §14.2.1・`REF:src/core/model/project.rs`・`REF:guide/` の URL の一致、(3) `docs/users` の
  相対リンクが仕様書か自分自身だけを指すこと、(4) `REF:src/ui/guide.rs` の段階と
  `REF:docs/users/start.md` の見出しが 1〜9 で揃うこと、(5) 生成した HTML どうしのリンクの実在を見る。
- **まだ実装していないこと**: 英語の本文（`docs/users/*.en.md` と `/en` の生成）。Phase 5 として残す。
