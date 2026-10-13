# 実装担当の引き継ぎ — novnow

> 引き継ぎ日: **2026-10-12**（ユーザーのタイムゾーンは Asia/Tokyo）
> 対象: novnow の実装を引き継ぐ人（または別セッションの自分）。
>
> **正本は [`SPECIFICATIONS.md`](../SPECIFICATIONS.md)（仕様）と [`PLANS.md`](../PLANS.md)（計画と判断の記録）。**
> 本書は「現在地と次の一手、そして環境の地雷」だけを持つ。食い違ったら正本が勝つ
> （[`DEVELOPERS.md`](../DEVELOPERS.md) 冒頭と同じ規則）。
>
> 読み方の順序: **本書 §1 → §2 → `PLANS.md` §5.5（M4）→ §9.1（未決事項）→ `DEVELOPERS.md` §5**。

---

## 0. 30 秒で分かる現在地

| 項目 | 状態 |
|---|---|
| `main` | **`8d32542`**（PR #10 マージ済み。M0〜M3 の全部が入っている） |
| マイルストーン | **M0・M1・M2・M3 完了**。**次は M4「診断と features」**（`PLANS.md` §5.5、10 日） |
| 仕様書 | **第 2.7 版**（2026-10-11）。診断コード **78 件**（§7:37 + §7.1:14 + §7.2:27） |
| テスト | **641 件緑**（`cargo test --locked --all-targets --no-fail-fast`） |
| CI | 4 ジョブ（fmt / clippy / test / wasm）**緑**（run 38187540789 = `baa603f`） |
| 実装済み | `core/id.rs`・`core/text/*`・`core/diagnostics.rs`（78 コード）・`core/syntax/*`（**8 形式**）・`core/model/*`（10 型）・**`core/resolve/*`（5 本すべて）** |
| 空のまま | `core/features/*`（**M4**）・`core/aicontext/*`（M10）・`ports/**`・`adapters/**`（M5/M6）・`ui/**`（M7 以降。`ui` feature の後ろ） |
| PR | #9・#10 とも**マージ済み**。次の作業は新しい PR を立てる（**CI は `pull_request` でしか走らない**、§4.3） |

マイルストーン別のコミット（新しい順）:

| コミット | 内容 |
|---|---|
| `baa603f` | **仕様書 第 2.7 版**（A-19〜A-23 の解消） |
| `9ad7d50` | M3-5 `context_rules.rs`（§9.5.2 / §15.4 の共用ルール）＋21 テスト |
| `df00071` | M3-4 `materials.rs`（§6.6）＋24 テスト |
| `1fd1f70` | M3-3 `role_language.rs`（§6.5 / 付録 B）＋32 テスト |
| `627547a` | M3-2 `modes.rs`（§6.4 の 8 段）＋53 テスト |
| `52c3bd6` | M3-1 `dictionary.rs`（§6.1 / §6.2 の最短一致）＋42 テスト |
| `321e5a7` | CI の失敗レポート 3 面構成（`.github/report.py`） |
| `5b4ac86` | GUI クレートを `ui` feature に分離（既定ビルドから egui / eframe を外す） |
| `bc8457e` | 仕様書 第 2.6 版 |
| `356bd12` | **M2 完了**（8 形式のパーサ＋model 10 型＋ゴールデン） |
| `f4c969c` | **M1 完了**（字句・位置・rope・増分） |
| `a3311d9` | **M0 完了**（層検査・CI・v0 削除） |

---

## 1. 引き継いで最初にやること（この順で）

1. **本書と `scripts/` を push する。** 書いた時点ではセッションが閉じていて push できていない
   （コミットはローカルにある）。新しいセッションで:
   ```bash
   git log --oneline -2                       # 引き継ぎコミットがあるか
   git push origin <そのセッションのブランチ>   # ブランチ名はセッション指定のものを使う
   ```
2. **ツールチェーンを復旧する**（約 40 秒＋展開 25 秒。§2.3）:
   ```bash
   bash scripts/bootstrap-toolchain.sh
   ```
3. **緑であることを確認する**（コールドビルド込み約 2 分、2 回目以降は test 3 秒）:
   ```bash
   bash scripts/verify.sh
   ```
   → `641 passed / 0 failed` が出れば環境は正常。**これが出なければ実装を始めてはいけない。**
4. **GitHub 側の状態を確認する**: `main` の位置、開いている PR、`cache/rust-toolchain` の存在
   （`bash scripts/ci-report.sh --branch main`）。
5. **`PLANS.md` §5.5（M4 の作業項目と完了条件）と §9.1（未決の要補正点）を読む。**
6. **§5 の未決裁定をユーザーに確認する**（特に **A-23**。実装は済んでいるが仕様解釈の裁定なので
   追認をもらう。M4 の診断文言に効く **A-7・A-8・A-18** も M4 中に裁定が要る）。

---

## 2. 環境（知らないと半日溶ける）

### 2.1 作業環境の前提

Arena のサンドボックス（Debian 12 / glibc 2.36 / kernel 6.17-azure）。**2 CPU / 4GB RAM 前後**、
ディスクは 20GB 前後（セッションごとに変わる）。`cargo` / `rustc` は**入っていない**。
`pip install` は `--break-system-packages` が要る（python3 3.11）。

### 2.2 到達できるホスト／できないホスト（**実測**。セッションごとに変わりうる）

| 到達できる | 用途 |
|---|---|
| `api.github.com` | `gh api` 全般（Contents API・PR・issue コメント・check-run の注釈） |
| `codeload.github.com` | **ブランチの tarball 一括取得**（ツールチェーンの搬入経路。実測 28MB/s） |
| `github.com`（git over https） | `git push` / `git fetch`、`gh pr` 系 |
| `pypi.org` / `files.pythonhosted.org` | python パッケージ（`pyyaml` 等） |
| `registry.npmjs.org` | （使っていない） |

| **到達できない** | 帰結 |
|---|---|
| `objects.githubusercontent.com` / `results-receiver.actions.githubusercontent.com` | **Actions のログが読めない**。`gh run view --log` は必ず失敗する（§4.2） |
| `static.rust-lang.org` | rustup でツールチェーンを入れられない → `scripts/bootstrap-toolchain.sh` を使う |
| `index.crates.io` / `static.crates.io` / `crates.io` | `cargo add` も `cargo build`（オンライン）もできない → **vendored sources ＋ `CARGO_NET_OFFLINE=true`** |
| GitHub リリースの asset | `trunk` などを落とせない |

**新しいクレートを足すときは注意**: 作業環境では `cargo` がオフラインなので、`Cargo.lock` を更新するには
`cache/rust-toolchain` の vendor を作り直す必要がある（`.github/toolchain.env` の `FORCE=1` を push して
`toolchain-export.yml` を回す → 新ブランチを再取得）。**仕様書 §3.2 の許可リスト更新が先**（ユーザー指示）。

### 2.3 ツールチェーンの復旧

```bash
bash scripts/bootstrap-toolchain.sh          # 無ければ取得、あれば何もしない
FORCE=1 bash scripts/bootstrap-toolchain.sh  # 取り直す
SMOKE=1 bash scripts/bootstrap-toolchain.sh  # 展開後に cargo test まで回す
```

中身は `.github/workflows/toolchain-export.yml` が生成した **rustc 1.99.0（CI と同一）の sysroot** ＋
**`cargo vendor` の出力（474 クレート）**で、孤立ブランチ `cache/rust-toolchain` に 45MB ずつ分割してある
（`MANIFEST.json` に sha256、`NOTES.md` に展開手順、`BUILD-LOG.txt` に生成ログ）。

**bash の呼び出しごとに環境変数は引き継がれない**ので、cargo を使うコマンドは毎回こう書く:

```bash
. $HOME/build/env.sh && cd <repo> && cargo test --locked --all-targets --no-fail-fast
```

`scripts/verify.sh` はこれを自动でやる（`BUILD_DIR` は既定 `$HOME/build`）。

### 2.4 **ローカル git が基点まで巻き戻る**（実際に 3 回起きた。最重要の地雷）

サンドボックスが再作成されると、**ファイルは全部残っているのに `.git` のブランチポインタだけが
最初のコミット（`4fb2759`）に戻る**。`git status` が「全ファイルが未コミット」に見えて驚くが、
**作業は失われていない**（GitHub 側が正本）。

復旧（**`--hard` は絶対に使わない**。作業ツリーを壊す）:

```bash
git fetch origin <ブランチ>            # GitHub に到達できる場合
git reset --mixed FETCH_HEAD          # ポインタだけ戻す（ファイルは無傷）

# fetch できない（セッションが閉じている）ときは、ローカルの main が使える:
git log --oneline -1 main             # 例: 8d32542 Merge pull request #10 …
git reset --mixed main
git status --short                    # 差分が「本当に新しいファイルだけ」になれば復旧完了
```

`git reset --mixed` のあと `git status` が空（または新規ファイルだけ）なら、**作業ツリーは
そのコミットと byte 単位で一致している**ことの証明になる。引き継ぎ時はこれで確認した
（`main` = `8d32542` と一致）。

### 2.5 スナップショット対象外（＝再作成で消える）

`target/`（2.9GB まで育つ）・`$HOME/build`（ツールチェーン 1.8GB）・`.venv` / `node_modules` /
`dist` / `build` / 各種キャッシュ。**リポジトリの外（`/home/user/build` 等）に置いたものは消える**前提で、
消えたら §2.3 をやり直す。`/home/user/novnow`（リポジトリ）は残る。

### 2.6 実測の性能（この環境の予算感）

| 作業 | 時間 |
|---|---|
| ツールチェーン復旧（376MB 取得＋展開） | 約 40 秒 |
| コールドビルド込み `cargo test --all-targets` | 約 2 分 |
| 温まったあとの `cargo fmt --all --check` | 0.1 秒 |
| 温まったあとの `cargo clippy --all-targets` | 2.5 秒 |
| 温まったあとの `cargo test --all-targets --no-fail-fast`（641 件） | 3〜13 秒 |
| `cargo check --target wasm32-unknown-unknown` | 1.3 秒（温）/ 25 秒（半コールド） |
| `cargo clippy --features ui`（初回、egui ツリー） | 57 秒 |
| release ビルド初回 | 約 39 秒 |

---

## 3. 検証（コミット前の 4 つ）

```bash
bash scripts/verify.sh            # fmt / clippy / test / wasm
bash scripts/verify.sh --quick    # wasm を省く（速い）
bash scripts/verify.sh --ui       # clippy --features ui も回す（M7 以降は必須）
bash scripts/verify.sh --fix      # 先に cargo fmt --all（差分を書いてから検査）
bash scripts/verify.sh --report   # 失敗時に .github/report.py の出力も表示（CI と同じ読み物）
```

**期待値**: `fmt ok / clippy ok / test ok / wasm ok` ＋ `641 passed / 0 failed`。

守ること:

- `cargo test` は必ず **`--no-fail-fast`**。既定だと統合テストがアルファベット順の**最初の失敗
  ターゲットで止まる**ので、他の形式の失敗が一度に見えない。
- `cargo clippy` は **`--all-targets -- -D warnings`**。テストコードにも lint が効く
  （`needless_borrow` / `useless_format` / `bool_assert_comparison` / `field_reassign_with_default` /
  `dead_code` / `unnecessary_lazy_evaluations` / `manual_clamp` / `manual_ignore_case_cmp` /
  `doc_lazy_continuation` で実際に落ちた）。
- **push 前にローカルで緑にする**（Actions の使用時間を節約する、というユーザー指示）。
  CI は「緑の証跡」として残す位置づけ（`PLANS.md` §9.3）。
- 性能を測るときは `cargo test --release` で測ってから、テスト自体の上限は **debug 前提の緩い値**にする
  （例: 辞書構築 5,000 件 = release 5.3ms / debug 20.3ms → 上限は `< 500ms`。役割語 10 パターン =
  release 0.68µs/行 → 上限は 2,000 回で `< 2000ms`）。実測値はコメントと `PLANS.md` に書く。

---

## 4. CI と「失敗の読み方」

### 4.1 4 ジョブ

`.github/workflows/ci.yml`: **Rustfmt / Clippy / Test / WASM Build Check**。
各ジョブは cargo の出力を `/tmp/ci.log` に tee し、`.github/report.py` が生ログから
「どれが・どこで・何と言って失敗したか」を抜き出して **3 つ**を書く（`DEVELOPERS.md` §1.5）:

1. **ワークフロー注釈**（`::error file=…,line=…::`）— PR の Files changed にインラインで出る
2. **実行サマリ**（`$GITHUB_STEP_SUMMARY`）— **成功時も**書く（テスト件数の内訳＝緑の証跡）
3. **PR コメント**（失敗時のみ）— **これが一番詳しい**

レポート生成は CI の合否に影響しない（`|| echo "::warning::…"`）。

### 4.2 **Actions のログ本体は読めない**

`gh run view --log` / `--log-failed` はすべて失敗する（§2.2）。読む手段は 3 つだけ:

```bash
bash scripts/ci-report.sh                    # 最新 run の job / step の状態
bash scripts/ci-report.sh --run <id>         # 失敗ステップ＋ check-run の注釈
bash scripts/ci-report.sh --pr <番号>        # PR のチェック＋ report.py のコメント本文
```

### 4.3 発火の条件（罠）

- `on: push` は **`main` / `master` だけ**。作業ブランチに push しても **CI は走らない**。
  → **PR を立てて初めて `pull_request` で走る**。PR がマージ／クローズされたら、
  次の作業では**新しい PR を立てること**（立てないと CI が一切走らない）。
- `paths-ignore`（`**.md` / `.github/**` / `assets/**` 等）は **`pull_request` では PR 全体の diff で
  判定される**。つまりコード変更を含む PR に docs だけのコミットを足しても **CI は走る**。
- `concurrency` は PR のときだけ `cancel-in-progress: true`（main への push は最後まで走らせる）。
- `paths-ignore` に `scripts/**` は**入っていない**ので、スクリプトだけのコミットでも CI が走る
  （4 ジョブで約 1 分半）。CI は Rust しか検査しないので、Actions 時間を削りたいなら
  `ci.yml` の `paths-ignore` 2 か所に `scripts/**` を足す（**両方に足すこと**。YAML アンカーは
  GitHub Actions で使えないので同じ一覧を 2 か所に書いてある）。
- `workflow_dispatch` は**この環境からは叩けない**（403）。発火させたいときは `on: push` ＋ `paths:` で。
- branch protection / rulesets は **403**（"Upgrade to GitHub Pro"）なので required checks の有無は不明。

### 4.4 `main` の Pages ワークフローが赤（**既存の問題**）

`.github/workflows/deploy.yml`（Deploy to GitHub Pages）が `Build with Trunk` で失敗する。
**M3 のマージより前から赤**（`e94c031` のときも失敗）。`main.rs` は `ui` feature を使っていないので
feature gate が原因ではない。ログが読めないため原因は未特定。

そもそも **GUI は M7 まで無い**（`PLANS.md` §3 I3）し、`trunk build` を CI に入れるのは M7 の判断
（§9.2 B-16）なので、**M7 まで `workflow_dispatch` だけにする**（＝ main の push では走らせない）のが筋。
ユーザーには 2026-10-12 に報告済みで、**返答はまだ**（§5）。Actions 時間の節約にもなる。

---

## 5. 未決の裁定（ユーザーに聞くこと）

`PLANS.md` §9.1 が正本。M4 の前に確認したいもの:

| # | 内容 | 現状 | いつ要る |
|---|---|---|---|
| **A-23** | 役割語の `^` / `$` が指す範囲。**§6.5.2 の「文単位」**と**付録 B.3 の 1・2 行目**が両立しないので、B.3 を正として**発話全体**に確定した（仕様書 第 2.7 版で修正済み） | 実装・テスト・仕様すべて「発話全体」で一致済み。**副作用**: `「分かってるよ。でも、これを見てくれ」` に `*よ。` は当たらない（途中の文末に当てるには `*よ。*` と書く） | **引き継ぎ直後の追認**。文単位に変えるなら `role_language.rs` の `anchored` 1 関数＋テスト 2 件＋ B.3 の例 2 行 |
| **A-24** | §7 に「語クラスの正規表現が不正」のコードが無い。今は `Matcher::new` が `None` を返して**そのパターンを黙って捨てる** | 実装済み（`PLANS.md` §9.2 B-69） | M5（`project.json` の読み込み）まで |
| **A-18** | E026 の ja 文言が「用語に修飾子は付けられません」で、資料タイトルに付いたときに合わない | 実装は §6.1.1 の表を優先して E026（B-63） | M4（i18n カタログ） |
| **A-7** | E004 の文言が `{0}` を使わないのに引数は 4 つ（`args[0]` は捨てられる） | 実装は 4 引数で `args[0]` にキャラ名を予約 | M4（診断の文言） |
| **A-8** | §7.2 の冒頭「すべて W または I」と E032〜E036・E064 の矛盾 | コードは定義済み（検査は ②） | M4 |
| **A-9 / A-10** | M2 の字句まわりの要補正点 | 実装は確定済み | M4 |
| Pages | §4.4 の赤いワークフローを止めてよいか | 報告済み・**返答待ち** | いつでも |

---

## 6. 作業の作法（ユーザーからの指示。守ること）

1. **コミットしたら毎回 push する**（2026-10-11 の指示）。
2. **レビューや評価はチャットで伝える**。ファイルに書き込まない（機械的チェックのために
   一時ファイルを作るのは可）。
3. **仕様書の矛盾など軽微なものは勝手に直して、後から報告する形でよい**（2026-10-11）。
   → 実際に第 2.6 版・第 2.7 版をこの方針で作った。**挙動に効く裁定は PR 本文と報告で必ず明示する**。
4. **マイルストーンは丸ごと完了させる**（セッションを跨いで止まらないようにする）。
5. **大量の手戻りが発生しうる重要判断のときだけ中断して報告する**。それ以外は自分で決めて
   `PLANS.md` §9.2 に B 番号で記録する（例: B-67 の E005 分担、A-23 の `^`/`$`）。
6. 指示がないと進められない状態になったら中断してよい。
7. **PR は勝手にマージしない**（マージはユーザーがする。2026-10-12 に #9・#10 をマージしたのは
   ユーザー）。ブランチは**セッションが指定したもの固定**（今回は `arena/acb80002-novnow`）。
   他のブランチに push しない。
8. **新しいクレートを足すときは仕様書 §3.2 の更新が先**（許可リストに無いものは入れない）。
9. **GitHub Actions の使用時間をできるだけ抑える**（push 前にローカルで緑にする、docs だけの
   コミットで CI を回さない工夫、`cache_key` が一致すれば toolchain-export は何もしない）。
10. **ワークフロー設定はエージェントが操作してよい**（許可済み）。ただし §9 の節約を優先。
11. rustfmt は**実際に使える**ので `cargo fmt --all` を使う（CI に整形を任せる `autofmt-temp.yml` は
    復活させない）。

---

## 7. コードの約束（検査で落ちる／壊れる）

| 約束 | 検査 | 備考 |
|---|---|---|
| **core は純粋**（§3.2）: egui / eframe / rfd・`std::fs`・`std::path` の I/O・web_sys / js_sys / wasm_bindgen・tokio / `std::thread`・全局時計 / RNG を `src/core/**` に置かない | `tests/layering.rs` の AST（`syn`）検査 | `use` 文・`extern crate`・式／型／パターンの位置・属性・マクロ本体まで見る。**落ちたらマージしない** |
| **core にユーザー向け文言（日本語リテラル）を置かない**（§12.2） | 同上 | 許されるのは doc コメント・`//` コメント・char リテラル・`#[cfg(test)]` の中、そして**字句データ**（`/// i18n: format-vocabulary` という doc 行を付けた `const` / `static` の中だけ）。例: `lexer::FIELD_KEYS`、`role_language::DEFAULT_WORD_CLASSES` |
| **診断コードを変えるときは先に `SPECIFICATIONS.md` の表** | `tests/diagnostics_catalog.rs` が **`include_str!("../SPECIFICATIONS.md")` で仕様書そのものを読む** | §7 / §7.1 / §7.2 の表と enum を件数・コード文字列・重大度・引数の数・位置の有無・既定の有効/無効・doc コメントまで突き合わせる。**表に無いコードを足すと落ちる** |
| **位置の単位を混ぜない**（B-56） | — | `syntax` の木の範囲は**行内相対**（`Field.value_at` / `CharRef.range` / `Speaker.range` / `Pattern.at`）、`resolve` が返す診断は**ファイル単位**（`line_start` を足す）。`context_rules::Layout` は**行 index** |
| **意味のかたまりは `model` の型で返す**（B-56） | — | 解決層は `syntax` の木を入力にし、出口で `model` を組む（`modes::to_model`、`RolePattern`、`MaterialIndex`）。round-trip と位置は `syntax` が持つ |
| `pub use self::module::Type;`（B-49） | `tests/layering.rs` | 同一クレート内でも `self::` を付けないと検査に落ちる |
| GUI は `ui` feature の後ろ（B-54） | `tests/layering.rs` の 2 テスト | `default = []`。`src/lib.rs` の `pub mod ui;` と `src/adapters/mod.rs` の `pub mod dialog_rfd;` に `#[cfg(feature = "ui")]`。**M7 で CI に `--features ui` を足す**（`ci.yml` に TODO あり） |
| ② の追加仕様に手を出さない | `PLANS.md` §1.2 の除外リスト | 枠だけ残すのは可（`EntryKind::Address`、`S` 接頭辞、`PacketBlock`）。**検査は実装しない** |

---

## 8. 設計判断の索引（M3 のぶん。全文は `PLANS.md` §9.2）

| # | 一言 | 実装 |
|---|---|---|
| B-56 | resolve の入力は `core/syntax` の木、model 型は出口で組む。位置の単位を混ぜない | `resolve/*` 全体 |
| B-57 | 正規化順は **長音統一 → NFKC → 小文字化 → 空白除去**（仕様書の旧順だと長音統一が発火しない＝ A-19）。96 byte は正規化後 | `dictionary::normalize_scanning` |
| B-58 | `alias:` の区切りは `,` `、` `，` | `dictionary::alias_entries` |
| B-59 | 修飾子は**半角 `:` のみ**、範囲は種別別（Material は空白を許す＝ A-20）、後ろが空なら修飾子でない | `dictionary::parse_modifier` |
| B-60 | `'` の開始条件（直前が ASCII 英数字なら参照でない。`don't` / `リリア's` を E001 の山にしない） | `dictionary::scan_line` |
| B-61 | 話者注釈は**完全一致**・`Character` 種別のみ（§6.1.1 MUST） | `dictionary::resolve_speaker` |
| B-62 | 索引は**トライ＋衝突キーだけの表**（全キー BTreeMap は 11.5ms で §18.1 の 10ms 超） | `dictionary::Dictionary` |
| B-63 | `MaterialTitle` の修飾子は §6.1.1 の表優先で **E026** | 同上 |
| B-64 | モード ID の比較は **ASCII 大小無視** | `modes::definitions_with_id` |
| B-65 | `mode: 'リリア`（モードなし）は**その段で見つからなかった**扱い。`default-mode: A+B` は受け付ける（＝ A-22） | `modes::from_fields` / `from_default_mode` |
| B-66 | 壊れた `range:` は**全範囲扱い**（E004 を重ねない） | `modes::Definition::range_expr` |
| **B-67** | **E005 は「その形を初めて読んだ層」だけが出す**。B 形式 `mode:` → `plot_episode`／1 トークンの 3 つ以上 → `dictionary`／`cast:` の重複と `default-mode:` の 3 つ以上 → `modes`。波線は**トークン全体**に統一 | `modes::finish`・`dictionary::token_diagnostics` |
| B-68 | 下線は「**下線に含めるアトムの連続**」をキャプチャグループで囲む。隣接・重なりは 1 本にマージ | `role_language::render` / `union` |
| B-69 | 未知の語クラス `(ほげ)` はリテラル。壊れた語クラス正規表現はそのパターンだけ捨てる（＝ A-24） | `role_language::atoms` / `Matcher::new` |
| B-70 | `dialogue_spans` は文字列から `「」`/`『』` の内側を取る。**入れ子は内側と外側の両方**、閉じなければ行末まで | `role_language::dialogue_spans` |
| **B-71** | **E007 と E001 を重ねない**。M4 の入口は **`materials::scan_line`**（辞書と資料の診断を 1 か所で組む） | `materials::scan_line` |
| B-72 | インレイヒントとクイックフィックスは**中身だけ**返す（文言は §12.2 なので UI/i18n）。24 字 / 23 字 + `…` は core | `materials::Hint` / `TitleFixes` |
| **B-73** | §15.4 の MUST（UI と LLM で情報源と優先順位を一致させる）は **1 枚の対応表**で担保する。`Layout` も共用し、ペインは「同一話」、パケットはブロックごとの上限を渡す | `context_rules::PaneSection::packet_blocks` |

---

## 9. M4 の着手メモ（次の担当へ）

**目標**（`PLANS.md` §5.5）: 診断を**出す**側と、LSP 互換の **11 機能**を純粋関数で揃える。
`core/diagnostics.rs`（78 コード）は M2 で定義済みなので、残りは「検査の実装」と「機能」。

### 9.1 M3 が用意した受け皿（**これを呼ぶだけでよい**）

| 欲しいもの | 呼ぶもの |
|---|---|
| 1 行の `'` 参照の解決と診断 | **`materials::scan_line(line, line_start, dictionary, index, in_body)`**（B-71。辞書と資料の診断が一度に出る。`dictionary::scan_line` ＋ `token_diagnostics` を**別に呼ばない**こと＝E001 と E007 が重なる） |
| ある行だけの辞書走査 | `dictionary::scan_line` → `RefToken`（範囲は**行内相対**） |
| トークン 1 件の診断 | `dictionary::token_diagnostics`（`materials::scan_line` が内部で呼ぶ） |
| 話者注釈の解決 / 診断 | `dictionary::resolve_speaker` / `speaker_diagnostic`（**E012**） |
| `## relations` の解決不能 | `dictionary::relation_diagnostics`（**E013**） |
| 本文中の用語出現（§6.1.2） | `dictionary::Dictionary::occurrences`（Aho-Corasick、初回だけ構築） |
| 節のモード | `modes::resolve(&Input, &Target)` → `Assignment { step, ids, definitions, diagnostics }`。**`definitions` が §6.4.6 の「有効モード」** |
| 参照ペインの配役・口調 | `modes::applicable_patterns`（**和集合**）→ `role_language::MatcherSet::new` |
| 役割語の下線 | `MatcherSet::underlines(utterance)`（`dialogue_spans` で台詞だけ渡す） |
| 資料のインレイヒント | `materials::resolve(...).hint`（位置・タイトル・status。**文言は UI**、B-72） |
| ペインの並び・パケットとの対応 | `context_rules::{Scope, Layout, PaneSection, PacketBlock}`（**M10 もこれを引く**、B-73） |
| 診断の並び（§9.5.2 の 9） | `context_rules::sort_diagnostics` |

### 9.2 診断の**分担表**（2 重に出さないために。M4 で最も壊しやすいところ）

| コード | 出す層 | 備考 |
|---|---|---|
| E001 / E002 / E026 / W029 | `dictionary::token_diagnostics` | E001 は**資料 ID の形をしたトークンには出さない**（`materials::scan_line` が E007 に置き換える、B-71） |
| E005 | B 形式 `mode:` → `syntax::plot_episode`／1 トークンの 3 つ以上 → `dictionary`／`cast:` の重複・`default-mode:` の 3 つ以上 → `modes` | **B-67**。同じ形に 2 層が波線を引かない |
| E003 / E004 / W006 | `modes` | E004 の引数は**4 つ**で `args[0]` は予約（A-7） |
| E006 / E007 / W031 | `materials` | W031 は `in_body` のときだけ（ファイルの形式は呼び出し側が知る） |
| E008 / E016 / E017 / E020 / E021 / E024 / E025 / W011 / W012 / W002 | `syntax/*`（M2） | 形式ごとのパーサが持つ |
| E012 / E013 / E028 / W027 | `dictionary`（構築時と解決時） | |
| **未実装（M4 で出す側が要る）** | `diagnostics::document(doc, project)` のような**文書単位のまとめ役** | 上の層を 1 本の関数から呼んで、文書全体の診断列を作る。§9.5.2 の 9 はこれで絞る |

### 9.3 M4 の作業順の提案

1. **`ui/i18n` の文言カタログ**（`diag.<コード>` を 78 件 × ja/en）。`PLANS.md` §3 の I6 で
   「M4 の最初に作る」と決めてある。§12.2 により**文言は core に入れない**。
   このときに A-7・A-8・A-18 の裁定をユーザーに求める。
2. `features/semantic_tokens.rs`（§6.3 のトークン種別。`dictionary::RefToken::token_kind` が既にある）
3. `features/inlay_hint.rs`（§6.6.2。`materials::Hint` を並べるだけ。**選択・コピーの対象にしない**は UI 側）
4. `features/hover.rs` / `definition.rs` / `references.rs`（辞書の `Origin` と `conflicts` を使う）
5. `features/completion.rs`（§9.7.3 の `'` 補完・モード補完・資料 ID 補完。`modes::definitions` と
   `Dictionary::entries` が材料）
6. `features/code_action.rs`（§7 のクイックフィックス。**`TitleFixes`・`RefToken::explicit_form`・
   `normalize_to_id` は既に中身がある**ので、テキスト編集に変換するだけ）
7. `document_symbol.rs` / `folding.rs` / `formatting.rs`（§5.12 の正規化）/ `rename.rs`
8. シグネチャは **§3.4 の表に厳密に従う**（`(&Project, &Document, Position)` → プレーン型）。
   完了条件: 「11 機能すべてがそのシグネチャを持つ」「core に日本語リテラルが無い」。

---

## 10. 踏んだ地雷（同じ轍を踏まないために）

### 10.1 Rust

- メソッドを値として渡すと **E0615**（`outline.field`）→ クロージャで包む（`|line| outline.field(line)`）。
- `Option<RangeValue>`（非 Copy）を `match self.node.range` で移動すると **E0507** → `.as_ref()`。
- クロージャ内で `Vec` を borrow しながら `retain` すると **E0521** → `while` ＋ `remove` ループ。
- 兄弟モジュールの型は `use` だけでは `pub` にならない（**E0603**）→ `pub use self::x::Y`。
- `Option::filter` のクロージャ引数は `&&str` になるが、関数呼び出しの deref coercion で通ることがある
  （通らないときは `|&(a, b)|`）。
- clippy はテストにも `-D warnings`（§3 のリスト）。`assert_eq!(x.is_some(), true)` は
  `bool_assert_comparison` で落ちる → `assert!(x.is_some())`。
- **`assert!` のメッセージ内の `{0}` は書式引数として解釈されてコンパイルエラー** → `{{0}}` か言い換え。
- rustfmt は doc コメントを reflow しないが、**自分のパッチ対象を壊すことがある**（§10.4）。
- 文字列リテラルは**複数行にまたがって書ける**（`tests/dictionary.rs` のフィクスチャがそう）。
  行頭カラム 0 から書けば余分な空白は入らない。

### 10.2 テストの期待値（**最大の欠陥源は自分の期待値**）

- **byte オフセットを手で数えない**。CJK は 1 字 3 byte、全角空白も 3 byte。
  → `&text[range]` で**原文を切って比べる**（`tests/modes.rs` の `slice_of`、
  `tests/materials.rs` の `diags_tag` がこの流儀）。
- 末尾改行があると `split('\n')` の最後の要素は空文字列。
- B 形式の節番号: 見出し直下の `;` が **1 節目（空）**を作り、その次が **2 節目**。
  場面フィールドは「見出し〜最初の `;` の直前」。
- 仕様書のコード例を**そのまま**フィクスチャにする（`PLANS.md` §6.3「仕様が正本であることを
  機械的に担保する」）。名前だけ差し替えるときは「差し替えた」とコメントに書く。
- 1 行にまとめて assert する `tag()` 関数を作ると差分が読める（`diags_tag` / `scenes_tag` 等）。

### 10.3 Markdown とドキュメント

- 既存の表に行を足すとき、**前後に空行を入れると表が分裂する**。
- 挿入アンカーを行頭にすると**順序が入れ替わることがある**（実際に B-53 / B-54 / B-55 で起きた）
  → 挿入後に `grep -n "^| B-"` で目視確認。
- `PLANS.md` §9.1 の A 表は「未解決 → 解決済み（打消し線 `~~**A-19**~~`）」の順で並んでいるので、
  新規の A は**未解決の末尾**に挿入する。
- 解決した A は行の最後のセルを `| **解消（日付 / 仕様 第 x.y 版）** |` に置き換える。

### 10.4 python でパッチするとき（長い CJK は `edit_file` が失敗しやすい）

- `rep(old, new)` は **`assert s.count(old) == 1`** を入れて、**毎回即 `write_text`**
  （最後にまとめて書くと、後の不一致で全編集を失う）。
- **`cargo fmt` の後にパッチするなら、必ず `sed -n` で該当箇所を実確認してから**置く
  （fmt が行を折り返して `old` が見つからなくなることがある）。
- doc コメントの `**太字**` の位置は見た目と違うことがある（`**先頭 2 つで続行**する` vs
  `**先頭 2 つで続行する**`）。**実際にファイルから取った文字列を `old` にする**。

### 10.5 git / GitHub

- §2.4 の**巻き戻り**。`git reset --hard` は禁止。
- shallow clone（`.git/shallow` に grafts）なので `git diff main...HEAD` が使えないことがある。
- **`gh pr view --comments` と `gh pr edit` は Projects(classic) の GraphQL エラーで失敗する**。
  → PR の title / body は REST で:
  ```bash
  python3 -c "import json;print(json.dumps({'title':'…','body':open('/tmp/body.md').read()}))" \
    | gh api -X PATCH repos/<owner>/<repo>/pulls/<番号> --input -
  ```
- `gh run view --log` は全滅（§4.2）。`gh workflow run` / `actions/permissions` は **403**。
- 大きいファイルの push: **100MB 超は拒否**、365MB を 1 pack で送ると
  `RPC failed; curl 55 Send failure: Broken pipe` → **45MB チャンク**（`toolchain-export.yml` がやっている）。
- **repo のファイルにサンドボックス固有のパス（`/home/user/...`）を書かない**
  （`.github/report.py` の再現コマンドから削除した経緯あり）。スクリプトは `$HOME` / `BUILD_DIR` で受ける。

---

## 11. ファイルの地図

| 読むもの | 何が決まっているか |
|---|---|
| `SPECIFICATIONS.md` | **仕様の正本**。§3.2（層と許可依存）・§5（8 形式の文法）・§6（解決）・§7（診断 78）・§9.5.2（参照ペイン）・§12.2（core に文言を置かない）・§15.4（ContextPacket）・§18.1（性能予算）・付録 B（役割語） |
| `PLANS.md` | **計画と判断の記録**。§1.2（② の除外）・§1.5（縫い代 10 項目）・§5（マイルストーン別 作業項目・完了条件・**進捗ノート**）・§6.3（仕様 → ゴールデン → 実装）・§9.1（**A: 仕様の要補正点**）・§9.2（**B: 設計判断**）・§9.3（環境）・§10（進捗管理） |
| `DEVELOPERS.md` | **手を動かすときに開く頁**。§1（ビルド・テスト・CI の出力）・§3（層）・§5.1〜5.5（`core/resolve/*` の 5 本の規則表）・§6（IME）・§11（作業を始める前に） |
| `README.md` | ユーザー向けの現状（テスト件数・できること・仕様書の版） |
| `tests/golden/**` | 8 形式のゴールデン（仕様書の例をそのまま入力にした期待値 JSON） |
| `scripts/` | **本書と一緒に追加した開発スクリプト**（§2.3・§3・§4.2） |
| `.github/report.py` | CI の失敗レポート生成（stdlib のみ） |
| `.github/toolchain.env` | ツールチェーン生成の設定（`ENABLED` / `FORCE` / `SMOKE` 等） |

---

## 12. 引き継ぎの完了条件（次の担当がこれを満たしたら引き継ぎ成功）

- [ ] `bash scripts/bootstrap-toolchain.sh` でツールチェーンが入り、`bash scripts/verify.sh` が
      **641 passed / 0 failed** と 4 ジョブ緑を出す。
- [ ] `git log --oneline -1` が `main` のマージコミット（`8d32542` 以降）を指し、
      `git status` がクリーン（＝巻き戻りの復旧ができている）。
- [ ] 本書と `scripts/` が**リモートに push されている**。
- [ ] `PLANS.md` §9.1 の **A-23** をユーザーに確認した（追認 or 変更の指示）。
- [ ] §4.4 の Pages ワークフローの扱いが決まっている（止める / 直す）。
- [ ] `PLANS.md` §5.5 の M4 作業項目 1（i18n カタログ）に着手し、A-7 / A-8 / A-18 の裁定を求めた。
