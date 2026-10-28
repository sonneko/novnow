# novnow 仕様書 — プロジェクト構造

## 4. プロジェクト構造

### 4.1 ディレクトリレイアウト

```
[title]/
├── project.toml              必須。機械可読のプロジェクト設定（§4.5）
├── characters.md             必須。キャラ・モード・役割語（§5.5）
├── words.md                  必須。用語辞書（§5.6）
├── ideas.md                  任意（無ければ自動生成）。アイデア（§5.7）
├── contents/                 必須（空でも可）。本文 = C 形式
│   ├── 00-01.cnov            「00 章」= 未配属ストック（§5.1.2）
│   ├── 01-00.cnov            「00 話」= 章あらすじの本文（§5.1.2）
│   ├── 01-01.cnov
│   └── 01-04.cnov
├── plots/                    必須
│   ├── all.cnov              必須。全体プロット = A 形式（章を記述）
│   ├── 00.cnov               ストック用プロット = A 形式（任意）
│   ├── 01.cnov               章プロット = A 形式（話を記述。先頭に @01-00 あらすじブロック）
│   └── 01-04.cnov            話プロット = B 形式（場面・節を記述）
├── materials/                必須
│   ├── index.json            必須。資料インデックス（§5.8）
│   ├── docs/
│   │   ├── M0137-science-koshien.md
│   │   └── S0042-search.md   検索結果の保存（§16.3.1）
│   └── inbox/                提案中の資料
├── assets/                   任意。プロジェクト内の小さな付随ファイル
│   └── icons/megumi.png      キャラアイコン（§5.5.3）。デスクトップ版のみ
├── prompts/                  任意（無ければ既定を自動生成）。AI プロンプト（§5.9）
│   ├── writer.md  plotter.md  ideator.md
│   ├── researcher.md  stylist.md  logician.md  integrator.md
│   ├── custom/               任意。ユーザー定義ロール（§5.9.2）
│   │   └── weapon-advisor.md
│   └── _shared/style.md  _shared/output-contract.md
├── agents/                   任意（無ければ自動生成）。AI 作業領域（§5.10）
│   ├── mcp.json              MCP サーバ設定（§5.11）
│   ├── tasks/     T0042.md
│   ├── results/   T0042.md
│   ├── threads/   TH0007.md
│   ├── proposals/ P0003.md / H0044.md（H = 人間の提案、§8.7）
│   └── log/       2026-10-09.jsonl
├── exports/                  任意。エクスポート出力先（自動生成）
├── .gitignore                `novnow new` が生成。`.novnow/` と `exports/` を無視
└── .novnow/                  任意。ローカル UI 状態・キャッシュ（§4.4、VCS 無視推奨）
    ├── ui.json
    ├── reader.json            追加仕様 27
    ├── stats.json            執筆統計の日次集計のみ（§4.4.2）
    └── cache/
```

拡張点: `assets/` は novnow の文書形式の外にある付随ファイルの置き場所であり、画像などの将来の拡張はここを土台にする。資料の出典種別（§5.8）などの拡張余地は各節が明示する。プロジェクト内の未知ファイルを壊さない規則（§4.3、§13.5）が、こうした拡張をあとから入れるための前提である。外部のソフトウェア（連携ツールなど）が読む安定面は、このファイル形式と §19 の API である。本体はバージョン管理そのものは実装せず、プロジェクトを通常のフォルダとして扱う素朴さを守る。

### 4.2 必須 / 任意の判定表

起動時スキャン（§14）での扱い。

| パス | 種別 | 欠けている場合 |
|---|---|---|
| `project.toml` | 必須ファイル | **E 起動失敗** |
| `characters.md` | 必須ファイル | **E 起動失敗** |
| `words.md` | 必須ファイル | **E 起動失敗** |
| `materials/index.json` | 必須ファイル | **E 起動失敗** |
| `plots/all.cnov` | 必須ファイル | **E 起動失敗** |
| `contents/` | 必須ディレクトリ | **E 起動失敗** |
| `plots/` | 必須ディレクトリ | **E 起動失敗** |
| `materials/docs/` | 任意ディレクトリ | 無言で作成 |
| `ideas.md` | 任意 | 無言で作成（空テンプレート） |
| `prompts/` | 任意 | 既定プロンプト一式を生成し、I050 を通知 |
| `agents/` | 任意 | 無言で作成（`mcp.json` は空の `{"schema_version":1,"servers":{}}`） |
| `exports/`, `.novnow/` | 任意 | 必要時に作成 |

「無言で作成」も **書き込みはプロジェクトルート配下に限る**（§13.1）。読み取り専用メディア上では作成を省略し、W041 を出して続行する。

**実装状況の注記**: いま自動作成を行うのは `novnow new`（テンプレート一式）と、`prompts/` の欠けた既定プロンプト（開いたときと `novnow mcp` の起動時。§15.5.4。I050 を通知）だけである。既存プロジェクトを開いたときの `ideas.md` / `materials/docs/` / `agents/` の「無言で作成」と、読み取り専用時の W041 は**未実装**（§7 冒頭の表）。無い任意のパスは「無い」まま動く（`ideas.md` が無ければ着想は空、`materials/docs/` が無ければ資料ファイルを置く場所が無いだけ）。

### 4.3 プロジェクト妥当性の判定

1. 必須パスの存在確認（上表）。欠落が 1 件でもあれば `ProjectError::Incomplete { missing: Vec<PathBuf> }`。
2. `project.toml` の `schema_version` 検査。
   - `2`（または省略）→ OK
   - `>2` → `ProjectError::TooNew { found }`（novnow 本体の更新が必要）
3. 文字コード検査: 文書として認識するすべてのファイル（§5.0.9）が UTF-8 として妥当。不正バイトがあれば該当ファイルを列挙して `ProjectError::Encoding`。
4. 以降のパース失敗（構文エラー）は **起動失敗にしない**。診断として表示し（**JSON / TOML がパースできないときは E025**、§7）、該当ファイルは「部分的に壊れた文書」として開く（P4: 作家の原稿を人質に取らない）。**原文は書き換えない**（MUST）。
5. 仕様が定義しないファイル・ディレクトリ（§5.0.9 が文書として認識しないものを含む）は妥当性判定の対象にしない。novnow が書くのは仕様が定義するパスだけで、未知のものを削除・改名・上書きしない（MUST）。旧形式のファイル（`project.json`、`contents/*.txt`、`plots/*.txt`）も未知のファイルとして扱うため、旧形式のプロジェクトを開くと必須ファイルの不足として通常のエラーになる。旧形式の特別検出や自動変換は行わない。

補足:

- `project.toml` が TOML として読めない（または `schema_version` が数でない）ときは 2. を飛ばす。版が読めないことは起動失敗にせず、4. のとおり E025 を出して既定値で開く。壊れた `project.toml` は上書きしない（§4.5）。
- core は I/O を持たない（§3.2）。存在するパスの一覧・`project.toml` の中身・UTF-8 として不正なファイルの一覧はストレージ層（adapters）が集めて渡し、`validate` は判定だけを行う。Web（IndexedDB、§13.3）にはディレクトリが無いので、必須ディレクトリは常に存在するものとして扱う。

### 4.4 `.novnow/`（ローカル状態）

プロジェクト内に置くが、VCS へのコミットは推奨しない（`novnow new` が生成する `.gitignore` に `.novnow/` と `exports/` を含める）。
`.novnow/` に置くのは、揃える必要のない個人の表示・操作の好みと、全 feature に共通する粗い表示設定に限る。診断の有効性やエラー扱いなどの feature 単位の設定は、作品として揃える設定として `project.toml` の `[features]`（§4.5.2）に置き、ここには置かない。

#### 4.4.1 `.novnow/ui.json`

```json
{
  "schema_version": 1,
  "theme": "system",
  "language": "system",
  "reference_pane_side": "right",
  "workspace": "write",
  "last_scene": "01-04-02-03",
  "pane_ratio": { "main": 0.62, "context_view": 0.55 },
  "font_scale": 1.0,
  "inlay_hints": true,
  "suggest_comma": false,
  "status_stats_mode": false,
  "confirm_section_delete": true,
  "context_sections": 3,
  "autosave_ms": 400,
  "folded": { "outline": ["@02", "@03"], "context_view": [] }
}
```
`theme`: `"system" | "dark" | "light"` / `language`: `"system" | "ja" | "en"`。
`autosave_ms`: 自動保存までの打鍵の間隔（§8.2、200–2000、既定 400。範囲外は丸める）。
`suggest_comma`: 自動句切れ提案（§9.4.3、既定 `false`）。
`status_stats_mode`: ステータスバーの統計モード（§9.6.3、既定 `false`）。
`confirm_section_delete`: 空でない節の削除前に確認するか（§8.1.2、既定 `true`）。
`folded`: 折りたたみ状態（§9.3.3）。2 つの配列を持つ。

| キー | 内容 |
|---|---|
| `outline` | 構成ツリー（§10.3.1）で**話を展開している章**の鍵（`@NN`）。構成ツリーは既定で畳んだ状態から始まるので、開いている章を並べる（§9.3.3 の注記） |
| `context_view` | 文脈ビュー（§9.3）で**畳んでいる節**の鍵（`@AA-BB-CC-DD`）。現在の節を含む単位（鍵が現在の節 ID の接頭辞）は保存しない（§9.3.3「現在の節を含む単位は畳めない」） |

- 既に無い章・節の鍵は、描画で参照されないだけなので害は無い（起動時に消しには行かない。ファイルを書き換えない、§4.5）。
- 旧名 `folded.context` は**読むときだけ**受け付ける（書き出しは常に `context_view`）。
**このファイルが存在しない／壊れている場合は既定値で起動し、エラーを出さない**（MUST）。

#### 4.4.2 `.novnow/stats.json`（執筆統計の最小メタデータ）

§9.6.2 のセッション情報と §10.9.5 の統計タブのために、**日次の集計だけ** を保持する。

```json
{
  "schema_version": 1,
  "days": {
    "2026-10-09": { "chars": 3800, "hours": [9, 10, 22, 23] }
  }
}
```

| 規則 | 内容 |
|---|---|
| 粒度 | 1 日 1 レコード。`chars` は当日の増加文字数（空白除外）、`hours` は執筆のあった時刻（0–23）の集合 |
| **イベントログを作らない**（MUST） | `*.jsonl` 形式の打鍵・保存ログは一切作らない。内容・文面・ファイル名は記録しない |
| 保持期間 | 直近 400 日。超えた分は削除する |
| サイズ上限 | 64KB。超えたら古い日から削る |
| 欠損耐性 | 削除・破損しても **統計の履歴が消えるだけ**。他の機能に影響してはならない（MUST） |
| 書き込み頻度 | セッション終了時と 10 分ごと。打鍵ごとには書かない |

#### 4.4.3 `.novnow/reader.json`

読者モードの読書位置・文字サイズ・列数・しおりを保持する。原稿・共有設定とは独立したローカル状態であり、削除しても読書位置が初期化されるだけである。未知のキーは保持する。

#### 4.4.4 `.novnow/cache/`

解析キャッシュ（再生成可能。削除しても動作に影響してはならない）。

#### 4.4.5 本文の 1 行最大文字幅（`max_line_chars`）〔状態: 実装完了〕

- `.novnow/ui.json` に任意キー `max_line_chars`（整数、有効範囲 `20..=120`、範囲外はクランプ）を追加する。
- 未指定時の既定値は、日本語表示（`ja`）では全角 **44 字**相当、英語表示（`en`）では半角 **70 字**前後相当とし、言語ごとの字面幅に応じて右側に不自然な余白が残らないようにする。コマンドパレットからも変更できる。

### 4.5 `project.toml` スキーマ（schema_version 2）

```toml
schema_version = 2
title = "星屑の時計"
author = ""
content_encoding = "utf-8"
newline = "lf"

[structure]
scene_id_format = "A-B-C-D"
chapter_digits = 2
episode_digits = 2
scene_digits = 2
section_digits = 2
allow_chapter_zero = true

[writing]
section_target_chars = 400
section_warn_chars = 1200
episode_target_chars = 6000
manuscript_page_chars = 400
max_consecutive_speeches = 3
dialogue_ratio_target = 0.3
long_sentence_chars = 120
particle_run_threshold = 4

[lexicon.word_classes]
"体言" = '[\p{Han}\p{Katakana}\p{Hiragana}A-Za-z0-9ー]{1,24}'

[export]
paragraph_break = "blank-line"
indent_first_line = false
keep_ruby = true
insert_headings = false
include_blurb = false
newline = "crlf"

[reader]
columns = 18
chars_per_column = 42
ruby = true

[agents]
enabled = true
delegation = "propose"
max_chars_per_request = 12000
integrator = "human"
max_parallel = 2
custom_roles = []

[agents.roles]

[agents.limits]
max_tasks_per_session = 100
max_chain_hops = 10
max_input_chars_per_session = 2000000

[agents.local_llm]
enabled = false
url = "http://localhost:11434"
model = ""
```

- 形式は TOML とし、スキーマの版は `schema_version = 2`（整数）。コメント（`#`）を書いてよい。
- 書き戻しは **無損失編集**とする。アプリが変更したキー以外は、コメント・書式・キー順・未知キーとその値をそのまま保持する（MUST）。未知キーは保存時に温存し、novnow は未知キーを削除してはならない（MUST）。
- TOML として読めない場合は E025 を出して既定値で開き、そのファイルは **上書きしない**（MUST、§4.3）。
- `title` が空文字のときは、表示上のタイトルにディレクトリ名を使う。
- `newline` は保存時の改行コード。`"lf" | "crlf"`。既定 `crlf`（上の例のとおり。`[export]` を書かないプロジェクトは `crlf` になる）。

#### 4.5.1 追加キーの意味

| キー | 既定 | 範囲 | 参照 |
|---|---|---|---|
| `structure.allow_chapter_zero` | `true` | bool | §5.1.2（00 章ストック） |
| `writing.max_consecutive_speeches` | `3` | 1–9 | §7.2 W082 |
| `writing.dialogue_ratio_target` | `0.3` | 0.0–1.0（台詞の割合の上限目安） | §9.4.2 |
| `writing.long_sentence_chars` | `120` | 40–400 | §9.4.3, §7.2 W089 |
| `writing.particle_run_threshold` | `4` | 2–10 | §9.4.3 |
| `export.include_blurb` | `false` | bool | §17.2（章あらすじの挿入） |
| `agents.delegation` | `"propose"` | `"off" \| "propose" \| "apply" \| "autonomous"` | §15.1.1（委譲レベルの全体既定） |
| `agents.roles` | `{}` | 役割名 → `"off" \| "propose" \| "apply" \| "autonomous"` | §15.1.1（役割ごとの上書き） |
| `agents.limits` | 下記参照 | オブジェクト | §15.1.1 A8（自律の上限） |
| `agents.integrator` | `"human"` | `"human" \| "agent"` | §15.2。`"human"` は `roles.integrator = "off"` と同義 |
| `agents.max_parallel` | `2` | 1–8 | §15.7.5（並列実行） |
| `agents.custom_roles` | `[]` | ロール名の配列 | §15.2.1 |
| `agents.local_llm` | 無効 | — | §15.8.4 |
| `reader` | `{columns:18, chars_per_column:42, ruby:true}` | — | §17.3 の既定の表示設定。読書位置・しおり・利用者の表示調整はローカル状態が優先 |

`agents.limits` のキー（すべて任意。超過したら自律タスクは自動停止し W095 を出す、A8）。
上限を見るのは**自動で起こす直前だけ**で、人間が起こすタスクは止めない。`0` は無制限:

| キー | 既定 | 意味 |
|---|---|---|
| `max_tasks_per_session` | `100` | 1 セッションに**自動で**起動できるタスク数の上限 |
| `max_chain_hops` | `10` | エージェント間連鎖のホップ数上限（§15.6）。人間が起こしたタスクから数え、自動で起こすたびに 1 |
| `max_input_chars_per_session` | `2,000,000` | 1 セッションの入力文字数上限（コストの暴走止め） |

#### 4.5.2 `[features]`（feature 単位の設定）

作品として揃えるべき意味・検証のオン・オフは、すべて `project.toml` の `[features]` に置く。個人の表示設定（§4.4）には置かない。

**診断（`[features.diagnostics]`）**: 値は `"off"`（出さない）/ `"warn"`（警告として出す）/ `"error"`（エラーとして扱う）の 3 値。診断コードごとに、次の順で最初に見つかった値を適用する。

1. 個別の診断コード（例: `W083 = "off"`）
2. グループ: `structure`（§7）/ `storage`（§7.1）/ `writing`（§7.2）
3. `default`

どれも無ければ、各コードの組み込み既定（§7〜§7.2 の各表の深刻度と既定）に従う。`"off"` の診断は件数にも含めない。この設定が変えるのは診断の有効性とエラー扱いだけであり、文法・名前解決・保存の規則は変えない。

**操作系 feature**: `[features]` の直下に真偽値で置く。個々のキー名と既定値は、それぞれの機能を定義する節が定める。書かないキーは組み込み既定である。

---


## 付録 C: `sample` プロジェクト

### C.1 サンプルプロジェクト構成と検証要素

```
sample/
  project.toml
  characters.md          アルス（モード A/B）、リリア（モード A/B/C）、学院長（モード A）
  words.md               星屑の時計 / 魔法学院 / 旧校舎
  ideas.md               I0001 学院長の動機（inbox）
  contents/01-04.cnov    既存の本文（`'『…』` は `『'…』` に直した v0.1.0 版）
  plots/all.cnov         @01, @02 の 2 ブロック（summary 必須）
  plots/01.cnov          @01-04 を含む話ブロック
  plots/01-04.cnov       @01-04-01, @01-04-02（mode: と check: と material: の実例つき）
  materials/index.json   M0001 を 1 件（source.kind = custom）
  materials/docs/M0001-kyukousha.md
  prompts/               既定 7 本
  agents/mcp.json        servers: {}
```

サンプルは「**機能を一通り踏んだ最小例**」であること（MUST）。具体的には次をすべて 1 回ずつ含む:
モードの併用（`'リリア:B+C`）、モードの range 変種、役割語の一致例と不一致例、資料 ID 参照、
条件一致による資料候補、未回収の伏線 1 件、`state: fixed` の節 1 つ、アイデア 1 件。

---
