# 実装担当の引き継ぎ — novnow

> 最終更新: **2026-10-13**（ユーザーのタイムゾーンは Asia/Tokyo）
>
> **本書の方針（ユーザー指示 2026-10-13。更新する人も守ること）**
>
> - 本書に書くのは「**次の実装者が必要とする**」かつ「**他の文書に書かれていない**」ことだけ
>   （現在地と次の一手・環境の落とし穴・作業の作法・開発上の知見）。
> - 仕様は [`SPECIFICATIONS.md`](../SPECIFICATIONS.md)、計画・設計判断（A / B 番号）・進捗ノートは
>   [`PLANS.md`](../PLANS.md)、手を動かすときの規則は [`DEVELOPERS.md`](../DEVELOPERS.md) が正本。
>   **そちらにある内容は本書に写さず、節番号で指す**（写すと二重管理になり、更新の手間と食い違いが増える）。
> - 落とし穴を踏んだとき・進捗があったときは**その場で本書を更新する**。他の文書に移した内容は本書から消す。
> - 食い違ったら正本が勝つ。

---

## 0. 現在地

| 項目 | 状態 |
|---|---|
| `main` | `00a5813`（PR #11 マージ済み。M0〜M3 ＋ 本書と `scripts/`） |
| 作業ブランチ | セッションが指定したもの固定（2026-10-13 のセッションは `arena/cb878908-novnow`）。**PR #12**（M4。ready for review にして CI を 1 回回した。**マージはユーザー**） |
| マイルストーン | **M0〜M4 完了**。**M5「ストレージ」進行中**（同じ PR #12 に積む。ユーザー指示 2026-10-13「マージはまだしない」）。済: 仕様 第 2.9 版・E027（A-24）・`core::policy`・ports・`storage_fs`（関門）・履歴・Zip・`project_load`。残り: `storage_idb`・`docs/verify-rule10.md`・文書の更新 |
| 仕様書 | **第 2.9 版**（2026-10-13）。診断コード **80 件**（§7:39 + §7.1:14 + §7.2:27） |
| テスト | **814 件緑**（`bash scripts/verify.sh`） |
| 未決の裁定 | なし（A-24 は 2026-10-13 にユーザー裁定 → E027） |

M4 で何を入れたか・決めたか（B-74〜B-87）は `PLANS.md` §5.5 の進捗ノートと §9.2、使い方は `DEVELOPERS.md` §5.6。

### 次の一手（M5 に入る人へ）

1. `PLANS.md` §5.6（M5 の作業項目と完了条件）を読む。
2. **A-24 をユーザーに裁定してもらう**（`project.json` の `lexicon.word_classes` の正規表現が壊れているとき。
   今は `role_language::Matcher::new` が黙ってパターンを捨てている）。コードを足すなら**仕様書 §7 が先**
   （`tests/diagnostics_catalog.rs` と `tests/i18n_catalog.rs` が仕様書を読むので、表・enum・`ja.toml`・
   `en.toml` を同じコミットで直す）。
3. M5 のストレージは `features::Project` に文書を `insert` する側。辞書の作り直しは `insert` / `remove` が
   自動でやるので、ストレージは「読んだ文書を渡す」だけでよい（`DEVELOPERS.md` §5.6）。

---

## 1. 引き継いで最初にやること

```bash
bash scripts/bootstrap-toolchain.sh   # ツールチェーン復旧（約 40 秒。無ければ取得、あれば何もしない）
bash scripts/verify.sh                # fmt / clippy / test / wasm（コールドで約 2 分）。緑でなければ始めない
bash scripts/ci-report.sh --branch main   # GitHub 側の状態
gh pr list --state open
```

`git status` が「全ファイル未コミット」に見えたら §2.3 の巻き戻り。

---

## 2. 環境の落とし穴

### 2.1 到達できるホスト（実測。セッションごとに変わりうる）

- 到達できる: `api.github.com`（`gh api`）・`codeload.github.com`（tarball 取得＝ツールチェーンの搬入経路）・
  `github.com`（git push / fetch）・`pypi.org`・`registry.npmjs.org`。
- **到達できない**: `objects.githubusercontent.com` / `results-receiver.actions.githubusercontent.com`
  （**Actions のログが読めない**）・`static.rust-lang.org`（rustup 不可）・crates.io 系
  （**cargo はオフライン**。vendored sources で動く）・GitHub リリースの asset（trunk 等）。
- **新しいクレートは仕様書 §3.2 の許可リスト更新が先**（ユーザー指示）。作業環境の cargo はオフラインなので、
  `Cargo.lock` を変えるには `.github/toolchain.env` の `FORCE=1` を push して `toolchain-export.yml` で vendor を
  作り直し、ツールチェーンを取り直す（仕組みは `PLANS.md` §9.3）。

### 2.2 ツールチェーン

- `$HOME/build` に展開される（スナップショット対象外。サンドボックスが作り直されたら §1 をやり直す）。
- **bash の呼び出しごとに環境変数が消える**ので、cargo を直接使うときは毎回
  `. $HOME/build/env.sh && cargo …`。`scripts/verify.sh` はこれを自分でやる（使い方はスクリプト冒頭）。
- `target/` も消えるので、作り直し後はコールドビルド（約 2 分）が 1 回走る。

### 2.3 ローカル git が基点まで巻き戻る（実際に 3 回起きた）

サンドボックスの再作成で、**ファイルは残るのに `.git` のブランチポインタだけが古いコミットに戻る**ことがある。
**`git reset --hard` は絶対に使わない**（作業ツリーを壊す）。

```bash
git fetch origin <ブランチ> && git reset --mixed FETCH_HEAD   # ポインタだけ戻す
git status --short   # 本当に新しい差分だけになれば復旧完了
```

shallow clone なので `git diff main...HEAD` が使えないことがある。

### 2.4 CI（4 ジョブの中身と出力は `DEVELOPERS.md` §1.4・§1.5）

- **CI は `pull_request` でしか走らない**（`on: push` は main だけ）。PR がマージ／クローズされたら、
  次の作業では**新しい PR を立てる**（立てないと CI が一切走らない）。
- **draft PR の間は 4 ジョブが skip される**（2026-10-13 に `ci.yml` を変更）。作業中は draft で push し、
  マイルストーンが終わったら **ready for review にすると CI が 1 回走る**（緑の証跡）。
- `paths-ignore`（`**.md` / `.github/**` 等）は PR 全体の diff で判定される。コードを含む PR に docs だけの
  コミットを足しても CI は走る。`scripts/**` は ignore されていない。
- **Actions のログ本体は読めない**（`gh run view --log` は必ず失敗）。読むのは
  `bash scripts/ci-report.sh --run <id>` / `--pr <番号>`（失敗時の PR コメント＝ `.github/report.py` の出力が一番詳しい）。
- `gh pr edit` と `gh pr view --comments` は Projects(classic) の GraphQL エラーで失敗する → REST で:
  ```bash
  python3 -c "import json;print(json.dumps({'title':'…','body':open('/tmp/body.md').read()}))" \
    | gh api -X PATCH repos/sonneko/novnow/pulls/<番号> --input -
  ```
- `gh workflow run`（`workflow_dispatch`）・branch protection の API は **403**。
- Pages（`deploy.yml`）は M7 まで手動実行だけ（v0 の UI を消した M0 以降 `trunk build` が失敗していた）。

### 2.5 この環境の速さ（予算の目安）

温まったあと: fmt 0.1 秒 / clippy 2〜15 秒 / test 3〜30 秒 / wasm check 1〜26 秒。
`cargo clippy --features ui` の初回は約 1 分（egui ツリー）。release ビルド初回は約 40 秒。

---

## 3. 作業の作法（ユーザーからの指示。守ること）

1. **コミットしたら毎回 push する**。push 先はセッション指定のブランチだけ。**PR は勝手にマージしない**（マージはユーザー）。
2. **push 前にローカルで緑にする**（`bash scripts/verify.sh`）。GitHub Actions の使用時間をできるだけ抑える
   （docs だけのコミットで CI を回さない工夫も含む）。ワークフロー設定はエージェントが変えてよい。
3. `cargo test` は **`--no-fail-fast`**、`cargo clippy` は **`--all-targets -- -D warnings`**。
   整形は `cargo fmt --all`（CI に整形を任せる `autofmt-temp.yml` は復活させない）。
4. **レビューや評価はチャットで伝える**（ファイルに書かない。機械的チェックの一時ファイルは可）。
5. **仕様書の軽微な矛盾は自分で直して後から報告**してよい。**挙動に効く裁定は PR 本文と報告で必ず明示**する。
6. **マイルストーンは丸ごと完了させる**。中断して聞くのは**大量の手戻りがありうる重要判断**のときだけ。
   それ以外は自分で決めて `PLANS.md` §9.2 に B 番号で記録する。
7. 新しいクレートは仕様書 §3.2 の許可リスト更新が先（§2.1）。
8. repo のファイルにサンドボックス固有のパス（`/home/user/...`）を書かない（`$HOME` / `BUILD_DIR` で受ける）。
9. **本書は冒頭の方針どおり最小限に保ち、進捗と落とし穴はその場で反映する**。

---

## 4. 開発上の知見（踏んだ地雷）

### 4.1 Rust

- メソッドを値として渡すと E0615 → クロージャで包む。非 Copy の `Option` を `match self.x` で移動すると E0507 → `.as_ref()`。
- クロージャ内で `Vec` を借りながら `retain` すると E0521 → `while` ＋ `remove`。
- 兄弟モジュールの型の再公開は `pub use self::x::Y`（`self::` が無いと `tests/layering.rs` に落ちる、B-49）。
- clippy はテストにも効く（`bool_assert_comparison` / `needless_borrow` / `useless_format` / `field_reassign_with_default` /
  `doc_lazy_continuation` などで実際に落ちた）。
- **`assert!` のメッセージに `{0}` を書くと書式引数になってコンパイルエラー** → `{{0}}` か言い換え。
- 性能テストは release で測ってから、上限は **debug 前提の緩い値**にする（実測値はコメントと `PLANS.md` に書く）。

### 4.2 テストの期待値（最大の欠陥源は自分の期待値）

- **byte オフセットを手で数えない**（CJK は 1 字 3 byte、全角空白も 3 byte）。`&text[range]` で原文を切って比べる。
- 仕様書のコード例を**そのまま**フィクスチャにする（`PLANS.md` §6.3）。ゴールデンの期待値 JSON は手で書く
  （失敗時は実測 JSON が stderr に出るので、仕様と突き合わせてから貼り替える）。
- B 形式の節番号: 見出し直下の `;` が 1 節目（空）を作り、その次が 2 節目。

### 4.3 シェル

- **`bash scripts/verify.sh | tail` を `&&` でつなぐと、赤でも次へ進む**（パイプの終了コードは `tail` のもの）。
  2026-10-13 に fmt 違反のまま commit / push した。verify はパイプに通さず単独で実行するか、`set -o pipefail` を付ける。

### 4.4 文書を python でパッチするとき（長い CJK は `edit_file` が失敗しやすい）

- 置換ごとに `assert s.count(old) == 1`。**複数の置換を 1 回の書き込みにまとめると、途中の不一致で全部失う**
  （書き込まれないまま終わる）。前の置換で `old` の文字列が変わる組み合わせにも注意。
- `cargo fmt` の後は行が折り返されていることがあるので、`sed -n` で実物を見てから `old` を作る。
- Markdown の表に行を足すとき、前後に空行を入れると表が割れる。挿入後は `grep -n "^| B-"` で順序を目視。
- `PLANS.md` §9.1 の A 表は解決したら名前を `~~**A-n**~~` にし、最後のセルを「済」にする。

### 4.5 core の書き方（検査で落ちるもの。規則の全文は `DEVELOPERS.md` §1.4・§3）

- `src/core/**` に日本語の**文字列リテラル**を置かない（仮名・漢字が対象。`「」` `。` `！` `…` などの約物と全角形は対象外）。
  ファイル形式の語彙だけは `/// i18n: format-vocabulary` を付けた `const` / `static` に置ける。
- 診断コードを変えるときは**先に `SPECIFICATIONS.md` の表**（`tests/diagnostics_catalog.rs` が仕様書を読む）。
  文言の `{n}` は 0 から抜けなく使う（`every_placeholder_index_is_used`）。
- 位置の単位: `syntax` の木は行内相対、`resolve` の診断はファイル単位（B-56）。
