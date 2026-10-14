# 実装担当の引き継ぎ — novnow

> 最終更新: **2026-10-14**（ユーザーのタイムゾーンは Asia/Tokyo）
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
| 作業ブランチ | セッションが指定したもの固定（2026-10-13 のセッションは `arena/cb878908-novnow`）。**PR #12** に M4〜M10 を積んだ（ユーザー指示「同じ PR で続ける・マージはまだしない」）。**マージはユーザー** |
| マイルストーン | **M0〜M6 完了**（PR #12 のマージ待ち）。**M7「UI シェル ＋ `write`」・M8「ワークスペース」・M9「エクスポート」・M10「エージェント」は実装済み・人の確認待ち**（`PLANS.md` §5.8〜§5.11 の完了条件の消し込み状況を参照）。**Web 版も 2026-10-14 に動くようにした**（ユーザーの質問「Web で動くようになってる？」への対応。B-156〜B-158、`bash scripts/web-smoke.sh` で確かめる）。M10 はユーザー指示（2026-10-13「M10を丸ごとお願いします」）で進めた。残りは M11「仕上げ」（§5.12）で、**着手はユーザーの指示を待つ** |
| 仕様書 | **第 2.16 版**（2026-10-14。2.12 は M7 に合わせた補正、2.13 は §11 の色のコントラスト比、2.14 は §17 のエクスポートの細部、2.15 は §10.5・§16・§6.6.2 の M8 の細部、2.16 は §15 の M10 の細部）。診断コード **81 件**（§7:40 + §7.1:14 + §7.2:27） |
| テスト | **1,145 件緑**（`bash scripts/verify.sh`。M7 から既定で `--features ui`）＋ Web の筋書き 19 項目（`scripts/web-smoke.sh`）。CI は `4acd11a`（M10）で 4 ジョブ緑（run 38498620189。PR を一時的に ready にして回し、draft に戻した）。Web の分（`34f361b`）も 4 ジョブ緑（run 38507612752） |
| 未決の裁定 | なし |

M7 で何を入れたか・決めたか（B-111〜B-121）は `PLANS.md` §5.8 の進捗ノートと §9.2、M9（B-122〜B-125）は §5.10、
M8（B-126〜B-137）は §5.9、M10（B-138〜B-155）は §5.11、GUI の作りは `DEVELOPERS.md` §6、エージェントは同 §10。

### 次の一手

1. **ユーザーの実機レビュー（M8・M10）の結果を待つ**。指摘はその場で直して PR #12 に積む。M11 はユーザーの指示があってから。
2. M7 の残り（`PLANS.md` §5.8 の未チェックの完了条件）: 人の確認（2,000 字の執筆・IME（Web の変換候補の位置も）・Windows の
   ダブルクリック・`docs/verify-rule10.md` の GUI での実施）。Web の選択モーダルは B-156〜B-158 で入った。
3. M9 の残り: 1 話（約 6,000 字）を外部の縦書きビューアで通読する人の確認（`PLANS.md` §5.10）。M10 の残り: A6 の実機確認と、
   実際の MCP サーバにつないだ一通りの操作（§5.11）。
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
  （**cargo はオフライン**。vendored sources で動く）・GitHub リリースの asset（trunk 等。要るものは Actions で取って git の blob に
  する＝ `sandbox-tools.yml`、§4.9）。
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

### 2.3 ローカル git が基点まで巻き戻る（2026-10-14 までに 9 回起きた）

サンドボックスの再作成で、**ファイルは残るのに `.git` のブランチポインタだけが古いコミットに戻る**ことがある。
**`git reset --hard` は絶対に使わない**（作業ツリーを壊す）。

```bash
git fetch origin <ブランチ> && git reset --mixed FETCH_HEAD   # ポインタだけ戻す
git status --short   # 本当に新しい差分だけになれば復旧完了
```

shallow clone なので `git diff main...HEAD` が使えないことがある。
巻き戻りと一緒に作業ツリーの外（`$HOME/build` のツールチェーンと `/tmp`）も消える。`bash scripts/bootstrap-toolchain.sh` で戻す（約 30 秒）。

### 2.4 CI（4 ジョブの中身と出力は `DEVELOPERS.md` §1.4・§1.5）

- **CI は `pull_request` でしか走らない**（`on: push` は main だけ）。PR がマージ／クローズされたら、
  次の作業では**新しい PR を立てる**（立てないと CI が一切走らない）。
- **draft PR の間は 4 ジョブが skip される**（2026-10-13 に `ci.yml` を変更）。作業中は draft で push し、
  マイルストーンが終わったら **ready for review にすると CI が 1 回走る**（緑の証跡）。
- `paths-ignore`（`**.md` / `.github/**` 等）は PR 全体の diff で判定される。コードを含む PR に docs だけの
  コミットを足しても CI は走る。`scripts/**` は ignore されていない。
- **CI の Test ジョブはディスクが足りなくなりうる**: GUI 込みのテスト 47 本がそれぞれ egui をリンクするので、既定の debuginfo では
  ランナーのディスクを使い切った（2026-10-14、"No space left on device"。注釈は `ci-report.sh --run <id>` で読める）。
  Test ジョブだけ `CARGO_PROFILE_DEV_DEBUG: 0` にした。テストのバイナリを増やすときは思い出すこと。
- **Actions のログ本体は読めない**（`gh run view --log` は必ず失敗）。読むのは
  `bash scripts/ci-report.sh --run <id>` / `--pr <番号>`（失敗時の PR コメント＝ `.github/report.py` の出力が一番詳しい）。
- `gh pr edit` と `gh pr view --comments` は Projects(classic) の GraphQL エラーで失敗する → REST で:
  ```bash
  python3 -c "import json;print(json.dumps({'title':'…','body':open('/tmp/body.md').read()}))" \
    | gh api -X PATCH repos/sonneko/novnow/pulls/<番号> --input -
  ```
- `gh workflow run`（`workflow_dispatch`）・branch protection の API は **403**。
- Pages（`deploy.yml`）は M7 で `push: branches: [main]` を戻した（`index.html` の `data-cargo-features="ui"`）。
  作業環境に trunk は無い（GitHub のリリースの asset に届かない）。Web の動作は `bash scripts/web-smoke.sh`（wasm-bindgen CLI ＋
  ヘッドレスの Chromium。§4.9）で確かめる。Pages の表示そのものは main に入って deploy が走るまで見られない。

### 2.5 この環境の速さ（予算の目安）

温まったあと: fmt 0.1 秒 / clippy 2〜15 秒 / test（`--features ui`）10〜45 秒 / wasm check 2〜30 秒。
`--features ui` のコールドビルドは約 2 分（egui / eframe / winit のツリー）。release ビルド初回は約 40 秒。
作業環境が 2 CPU / 3GB のときはコールドの `--no-run` ビルドが約 2 分、`verify.sh` 全体が約 2.6 分だった（2026-10-13）。

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
  が egui の出力をソフトウェアで描いて RGBA を書く（`NOVNOW_SHOT_SCENE=inlay|completion|palette|find|help|episode|outline|idea|material|material-search|cast|review`・`NOVNOW_SHOT_LIGHT=1`）。
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
- **新しく出たモーダル（`egui::Area`）は最初のフレームでは大きさを測るだけで描かれない**。画面なしのテストで文言を
  確かめるときは、出した後に 2 フレーム回してから `painted_text` を見る（M9 で踏んだ）。
- `InputState::consume_key(mods, key)` は `matches_logically` で比べるので、**`Modifiers::NONE` は Shift / Alt 付きにも当たる**。
  `Shift+Enter` と `Enter` を分けるときは Shift のほうを先に消費する。`Modifiers::COMMAND` を要求する組み合わせは、テストの
  イベントでも `command: true` を立てる（実機の egui-winit は Windows / Linux の Ctrl で ctrl と command の両方を立てる）。

### 4.9 Web（2026-10-14）で踏んだもの

- **作業環境でブラウザを動かせる**: npm の `@sparticuz/chromium` はバイナリを npm の tarball に同梱しているので届く
  （`scripts/web-smoke.sh` が展開する。`LD_LIBRARY_PATH` に同梱の `lib/` が要る）。wasm-bindgen CLI は GitHub のリリースの
  asset（objects.githubusercontent.com）に届かないので、`sandbox-tools.yml` が git の blob にして PR にコメントする。
  blobs API は `Accept: application/vnd.github.raw` だと 404 のことがあるので、JSON（base64）で取る。
- **IndexedDB を外から覗くときに DB を作らない**: 確かめる側が先に `indexedDB.open('novnow', 1)` すると、空の版 1 ができて
  novnow の `onupgradeneeded` が走らず、ストアが無いまま全部失敗した。`indexedDB.databases()` で有無を見てから開く。
- **eframe 0.27 の Web は、マウスで入力欄をクリックしたときだけ IME の隠れた入力欄を出す**。キーボードで移ると日本語が
  入らなかった（`sync_text_agent` で直した）。`--cfg=web_sys_unstable_apis`（コピー / 切り取りを入れる cfg）は今の web-sys と
  組み合わせると eframe がコンパイルできない。`App::raw_input_hook` は Web では呼ばれない（`update` の頭で `input_mut`）。
- **このヘッドレスの Chromium は日本語を含むダウンロードの名前を `download` にする**（`data:` のページでも同じ。普通の Chrome は
  日本語のまま保存する）。筋書きは `Browser.downloadWillBegin` の名前と中身で見分ける。ダウンロードの置き場はブラウザの CDP
  セッションで設定する（ページのセッションだと名前が崩れた）。ヘッドレスは `--lang` を無視するので `navigator.language` を上書きする。
- **マクロの中の `///` は文字列リテラル**（`#[doc = "…"]`）になり、`tests/layering.rs` の「GUI の文言」の検査に掛かる
  （`thread_local!` の中で踏んだ）。ふつうのコメントにする。

### 4.7 ワークスペース（M8）で踏んだもの

- **`TextEdit` には文字ではない表示（インレイヒント）を挟めない**。ヒントの位置から始まる `LayoutSection` の `leading_space` で
  隙間を空け、`TextEdit` の後に上から描く（`ui/widgets/inlay.rs`、B-137）。epaint 0.27 の `extra_letter_spacing` は同じセクションの
  2 字目以降にしか効かないので使えない。下線・波線の右端は「次の字の左端」ではなく「最後の字の右端」で取る（でないと隙間の下まで伸びる）。
- **フォントに無い字は豆腐になる**: 同梱の IPA ゴシックにも egui の Ubuntu-Light にも `⟨⟩`（U+27E8 / U+27E9）が無い。比例フォントの最後に
  egui 既定の Hack を受け皿として足した（`theme::fonts`）。新しい記号を画面に出すときは、python で `cmap` 表を読んで有無を確かめる。
- **仕様書を `include_str!` で取り込むテストが 7 本ある**。`verify.sh` の実行中に `SPECIFICATIONS.md` を書き換えない（テストのビルドが
  途中の版を拾う）。`PLANS.md` / README / 本書は読まれない。
- **write 以外のワークスペースでは、`NovnowApp::run_workspace` が知らないコマンドを飲み込む**（write 向けのキーが他の画面で誤爆しない
  ように）。どこでも効かせたいコマンドは `run()` の先頭の `match`（言語・テーマ・`Alt+H` など）に置くか、`run_workspace` の素通しの列に足す。
  write 以外の描画は `common_tail` で早く戻るので、ポップアップ（`Ctrl+.`）もそちらで描く。ポップアップの ↑ / ↓ / Enter はエディタを
  描く**前**に `popup_keys` で取る（後だと `TextEdit` に入る）。
- **ボタンだけのオーバーレイ（確認など）は、開いたフレームでキーボードのフォーカスを手放す**。手放さないと後ろの `TextEdit` に
  `Enter` が入る（B-135 の (7)）。

### 4.8 エージェント（M10）で踏んだもの

- **`Alt+8` の直後は 2 フレーム空回ししないと素のキーが効かない**: 描かれたことのない id へのフォーカスは egui 0.27 が 1 フレームの
  猶予の後に外すので、切替のフレームでは列のキー（`R` など）がまだ届かない（`tests/workspaces.rs` の `agent_harness`）。
- **パレットから開くオーバーレイ（`>prompt diff` / `>prompt reset`）は `overlays()` の中で作るので、最初のフレームは大きさを測るだけ**
  （§4.5 のモーダルと同じ）。文言は 1 フレーム後に読む。ワークスペースの描画中に `agent_actions` が作るものは次のフレームで見える。
- **作業環境に日本語フォントが無いので、開いた直後にフォントのフォールバックの通知が出る**。これが I050（既定プロンプトを作った）を
  上書きしていたので「{前} / {フォント}」とつないだ（B-154）。開いた最初の画面の文字で通知を確かめるテストはこれに注意。
- `cargo test` に渡せるテスト名の絞り込みは 1 つだけ（2 つ渡すと「unexpected argument」）。
- **規則 10 の関門は debug で panic する**ので、外を指すシンボリックリンクのある場所に既定のプロンプトを書こうとするとテストが落ちる
  （`tests/cli.rs` の `mcp_leaves_prompts_that_are_links_to_outside_alone` で踏んだ。B-155）。書く前に `exists` / `Host::occupied` を見る。
- `tokio` の process / io-util は vendored に無い。MCP の stdio は `std::process` とスレッドで書いた（`adapters/mcp_stdio.rs`）。
  `serde_json` の `preserve_order` も使わない（`agents/mcp.json` のサーバの定義順は自前で読む、B-147）。
- Slice のエディタのキャレットの矩形は `slice_editor::caret_rect(ctx)`（egui の一時データ）。`Workbench::apply_edit` は Slice の文書を
  変えたら Slice を閉じる。次のフレームで各ワークスペースの `sync` が鍵から開き直す。

### 4.6 core の書き方（検査で落ちるもの。規則の全文は `DEVELOPERS.md` §1.4・§3）

- `src/core/**` に日本語の**文字列リテラル**を置かない（仮名・漢字が対象。`「」` `。` `！` `…` などの約物と全角形は対象外）。
  ファイル形式の語彙だけは `/// i18n: format-vocabulary` を付けた `const` / `static` に置ける。
- 診断コードを変えるときは**先に `SPECIFICATIONS.md` の表**（`tests/diagnostics_catalog.rs` が仕様書を読む）。
  文言の `{n}` は 0 から抜けなく使う（`every_placeholder_index_is_used`）。
- 位置の単位: `syntax` の木は行内相対、`resolve` の診断はファイル単位（B-56）。
