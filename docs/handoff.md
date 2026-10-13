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
| 作業ブランチ | セッションが指定したもの固定（2026-10-13 のセッションは `arena/cb878908-novnow`）。**PR #12** に M4〜M7 を積んだ（ユーザー指示「同じ PR で続ける・マージはまだしない」）。**マージはユーザー** |
| マイルストーン | **M0〜M6 完了**（PR #12 のマージ待ち）。**M7「UI シェル ＋ `write`」は実装済み・人の確認待ち**（`PLANS.md` §5.8 の完了条件の消し込み状況を参照。Web の選択モーダルは未接続＝ B-118） |
| 仕様書 | **第 2.13 版**（2026-10-13。2.12 は M7 に合わせた補正、2.13 は §11 の色のコントラスト比）。診断コード **80 件**（§7:39 + §7.1:14 + §7.2:27） |
| テスト | **947 件緑**（`bash scripts/verify.sh`。M7 から既定で `--features ui`） |
| 未決の裁定 | なし |

M7 で何を入れたか・決めたか（B-111〜）は `PLANS.md` §5.8 の進捗ノートと §9.2、GUI の作りは `DEVELOPERS.md` §6。

### 次の一手

1. M7 の残り（`PLANS.md` §5.8 の未チェックの完了条件）: 人の確認（2,000 字の執筆・IME・Windows のダブルクリック・
   `docs/verify-rule10.md` の GUI での実施）と、Web の選択モーダル（§13.4、B-118）・`Alt+H`（B-119）。
2. **M9（エクスポート）を M8 より先にやる**（`PLANS.md` §9.2 B-120）。
3. M8（ワークスペース、`PLANS.md` §5.9）: `ui::app::NovnowApp` に「ワークスペース」の層が無い（M7 は `write` 1 つ）。
   `Alt+1..9` の切替は `ui::shell::command_of` に足し、`Stage::Open` の中で描き分ける。点検ワークスペースの「位置」は
   `place::at` ＋ `i18n::place`（`novnow check` と同じ表記。B-110）。
4. 外に書けるのは `FsGate::write_chosen_export(ChosenPath)` だけ（Zip は `Dialog::save_zip` → `zip::export_to_chosen`、テキストは
   `Dialog::save_text` / `novnow export --out` → `ChosenPath::from_command_line(.., ExportKind::Text)`。B-99・B-109）。

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
- `strace` は無く、`apt` のミラーにも届かない。規則 10 の確認はファイル差分で代用する（`docs/verify-rule10.md` §3.2・§5）。
- **ツールチェーンに Windows の std が無い**（`x86_64-linux` と `wasm32` だけ）ので `cargo check --target *-windows-*` は
  手元で回せない。`#[cfg(windows)]` のコードは CI の wasm ジョブ（M6 で Windows の型検査を足した）が見る。手元で型だけ
  確かめたいときは、その関数を cfg なしで一時的な小さなクレートに写して `cargo check`（リンクしないので通る）。
- **新しいクレートは仕様書 §3.2 の許可リスト更新が先**（ユーザー指示）。作業環境の cargo はオフラインなので、
  `Cargo.lock` を変えるには `.github/toolchain.env` の `FORCE=1` を push して `toolchain-export.yml` で vendor を
  作り直し、ツールチェーンを取り直す（仕組みは `PLANS.md` §9.3）。

### 2.2 ツールチェーン

- `$HOME/build` に展開される（スナップショット対象外。サンドボックスが作り直されたら §1 をやり直す）。
- vendored のソースは `$HOME/build/vendor-src/<crate>`（API の確認に使える）。**web-sys の feature を足しても
  `Cargo.lock` は変わらない**（vendored に全 feature のソースがある）ので、オフラインのまま足せる。
- **bash の呼び出しごとに環境変数が消える**ので、cargo を直接使うときは毎回
  `. $HOME/build/env.sh && cargo …`。`scripts/verify.sh` はこれを自分でやる（使い方はスクリプト冒頭）。
- `target/` も消えるので、作り直し後はコールドビルド（約 2 分）が 1 回走る。
- **ディスクは 21GB しか無い**。M7 で GUI 込みのテスト 40 本を debuginfo 付きで作ったら `target/` が 18GB になり、リンカが
  `Bus error` で落ちた（ディスク満杯の症状）。`env.sh` で `CARGO_PROFILE_DEV_DEBUG=line-tables-only` と
  `CARGO_INCREMENTAL=0` にしている（`scripts/bootstrap-toolchain.sh` が書く）。それでも溢れたら
  `find target/debug/deps -maxdepth 1 -type f -perm -u+x -delete` と `rm -rf target/debug/incremental`（作り直しはリンクだけ）。

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
- Pages（`deploy.yml`）は M7 で `push: branches: [main]` を戻した（`index.html` の `data-cargo-features="ui"`）。
  作業環境に trunk もブラウザも無いので、Web の GUI は `cargo check --target wasm32-unknown-unknown --features ui` までしか確かめていない。

### 2.5 この環境の速さ（予算の目安）

温まったあと: fmt 0.1 秒 / clippy 2〜15 秒 / test（`--features ui`）10〜45 秒 / wasm check 2〜30 秒。
`--features ui` のコールドビルドは約 2 分（egui / eframe / winit のツリー）。release ビルド初回は約 40 秒。

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
- **CI の clippy は `#[cfg(target_arch = "wasm32")]` の中を見ない**（ネイティブでしか回さない）。Web 専用のコードを
  触ったら手元で `cargo clippy --locked --target wasm32-unknown-unknown -- -D warnings` も回す。
- clippy で実際に落ちたもの（M5）: `assert!(cfg!(debug_assertions))`（`assertions_on_constants` → `if` ＋ `panic!`）、
  `(|| { … })()`（`redundant_closure_call` → 関数に出す）。trait に `async fn` を書くと `async_fn_in_trait`
  （→ 宣言は `fn … -> impl Future`、実装は `async fn` でよい）。M6: `fn f<'a>(x: &'a str) -> &'a str`（`needless_lifetimes`）、
  `&[x.clone()]`（`cloned_ref_to_slice_refs` → `std::slice::from_ref(&x)`）。
- **rustc 1.99 の `float_literal_f32_fallback`**（将来エラーになる警告）: `Stroke::new(1.0, …)` のように `impl Into<f32>` へ
  浮動小数のリテラルを渡すと出る → `1.0_f32`。clippy 1.99 の `manual_is_multiple_of`（`n % 2 == 0` → `n.is_multiple_of(2)`）。
- **`main.rs` は wasm でもビルドされる**（Web も同じ実行ファイル `data-bin="novnow"` を入口にする）。ネイティブ専用の
  コード（`novnow::cli` は `cfg(not(wasm32))`）は `main.rs` の中で `#[cfg(not(target_arch = "wasm32"))]` に入れる。

### 4.2 テストの期待値（最大の欠陥源は自分の期待値）

- **byte オフセットを手で数えない**（CJK は 1 字 3 byte、全角空白も 3 byte）。`&text[range]` で原文を切って比べる。
- 仕様書のコード例を**そのまま**フィクスチャにする（`PLANS.md` §6.3）。ゴールデンの期待値 JSON は手で書く
  （失敗時は実測 JSON が stderr に出るので、仕様と突き合わせてから貼り替える）。
- B 形式の節番号: 見出し直下の `;` が 1 節目（空）を作り、その次が 2 節目。
- CLI のテストは `novnow::cli::run` を `Cli::new(&FakeEnv, &mut Vec<u8>, …)` でプロセス内で走らせる（環境変数も
  カレントディレクトリも書き換えない）。仕様書の出力例（§14.2.2〜§14.2.4）は `tests/cli.rs` の `spec_block` で読み直して
  一字一句比べる（コードブロックの開きは「```」の後に言語名があるので、行末まで飛ばしてから本文を取る）。
- 規則 10 の違反は **debug ビルドで panic** する（§13.1.2）。拒否を確かめるテストは `catch_unwind` で受ける
  （`tests/storage_policy.rs` の `rejected`。panic の表示は hook で黙らせている）。release でも一度
  `cargo test --release --test storage_policy` を回す（E060 を返す側。初回は約 30 秒）。

### 4.3 シェル

- **`bash scripts/verify.sh | tail` を `&&` でつなぐと、赤でも次へ進む**（パイプの終了コードは `tail` のもの）。
  2026-10-13 に fmt 違反のまま commit / push した。verify はパイプに通さず単独で実行するか、`set -o pipefail` を付ける。

### 4.4 文書を python でパッチするとき（長い CJK は `edit_file` が失敗しやすい）

- 置換ごとに `assert s.count(old) == 1`。**複数の置換を 1 回の書き込みにまとめると、途中の不一致で全部失う**
  （書き込まれないまま終わる）。前の置換で `old` の文字列が変わる組み合わせにも注意。
- `cargo fmt` の後は行が折り返されていることがあるので、`sed -n` で実物を見てから `old` を作る。
- Markdown の表に行を足すとき、前後に空行を入れると表が割れる。挿入後は `grep -n "^| B-"` で順序を目視。
- `PLANS.md` §9.1 の A 表は解決したら名前を `~~**A-n**~~` にし、最後のセルを「済」にする。

### 4.5 GUI（M7）で踏んだもの

- **画面が無くても見た目を確かめられる**: `NOVNOW_SHOT=/tmp/shot.rgba cargo test --features ui --test gui screenshot -- --ignored`
  が egui の出力をソフトウェアで描いて RGBA を書く（`NOVNOW_SHOT_SCENE=completion|palette|find|help`・`NOVNOW_SHOT_LIGHT=1`）。
  PNG にするのは標準ライブラリの zlib だけで足りる（`DEVELOPERS.md` §6.4 に 10 行のスクリプト）。作業環境には CJK フォントが無いので、
  画面下に「同梱のフォントを使います」が出るのは正しい。
- フォントのテクスチャは**最初のフレームにだけ全体が来る**（以降は差分）。最初のフレームの出力を捨てると描けない。
- `TextFormat::underline` は行の**下端**に引くので、行間 1.8〜1.9 では文字から離れる。下線と波線は `Glyph::pos.y`（ベースライン）
  から自分で描く（`ui/widgets/syntax_text.rs`）。
- **ファイル末尾（と場面の末尾）の `;` の後は、本文が無いと節にならない**（§5.2.2）。`Ctrl+Enter` / `Ctrl+;` で末尾に作った節は、
  書き始めるまで文書に無い節として `Workbench` が持つ（B-112）。
- ショートカットのイベントは `ui::shell::take_commands` が `TextEdit` より先に**消費する**。`Ctrl+Home` は §9.8.2 の
  「ファイル先頭の節」なので、`TextEdit` の「先頭へ」は効かない（テストで踏んだ）。
- 画面なしのテストで `NovnowApp` を回すときは `RawInput { focused: true, time: Some(..), .. }` を毎フレーム渡す
  （`time` を進めないと 400ms の自動保存が来ない）。

### 4.6 core の書き方（検査で落ちるもの。規則の全文は `DEVELOPERS.md` §1.4・§3）

- `src/core/**` に日本語の**文字列リテラル**を置かない（仮名・漢字が対象。`「」` `。` `！` `…` などの約物と全角形は対象外）。
  ファイル形式の語彙だけは `/// i18n: format-vocabulary` を付けた `const` / `static` に置ける。
- 診断コードを変えるときは**先に `SPECIFICATIONS.md` の表**（`tests/diagnostics_catalog.rs` が仕様書を読む）。
  文言の `{n}` は 0 から抜けなく使う（`every_placeholder_index_is_used`）。
- 位置の単位: `syntax` の木は行内相対、`resolve` の診断はファイル単位（B-56）。
