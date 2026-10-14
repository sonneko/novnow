# 外部依存・外部ツールの導入手順

本書は、Arena サンドボックス環境（通信制限あり）で開発している `novnow` リポジトリにおいて、新たに**① 開発・検証用ツール（CLI バイナリ・ベンチマーク/E2E 補助ツール等）**または**② 本体クレート依存（`[dependencies]` / `[dev-dependencies]`）**を追加する際の手順をまとめる。

---

## 0. 前提：サンドボックスの通信制限と 2 つの搬入経路

Arena サンドボックスから外部へ直接通信できるホストは以下の 6 つに限定されている（`REF:docs/developers.md` §6.1）。

- `github.com` / `codeload.github.com` / `api.github.com`
- `registry.npmjs.org`
- `pypi.org` / `files.pythonhosted.org`

一方、Rust の公式配布元やクレートレジストリ（`static.rust-lang.org`, `crates.io`, `static.crates.io`）および GitHub Releases のバイナリ配布先（`objects.githubusercontent.com`）には**サンドボックスから直接到達できない**。
そのため、npm / PyPI で取得できるもの以外は **GitHub Actions 上で取得し、リポジトリの Git オブジェクト（Git Blob または `cache/rust-toolchain` 孤立ブランチ）を経由してサンドボックスへ搬入する**。

| 導入対象 | 推奨する搬入経路 | 代表例 |
|---|---|---|
| **A-1. 外部 CLI バイナリ / 少数クレートの追加** | `.github/workflows/sandbox-tools.yml` で取得 → **Git Blob API** で保存 → スクリプトで展開 | `wasm-bindgen-cli`, `criterion`（19 クレート） |
| **A-2. Node.js / Python ベースの検証ツール** | サンドボックスから `registry.npmjs.org` / `pypi.org` へ直接 `npm install` / `pip install` | `@sparticuz/chromium`, `puppeteer-core` |
| **B-1. 本体依存クレートの追加（少数・差分追加）** | Git Blob 方式で `.crate` を `/home/user/build/vendor-src/` に追加展開 | 小規模なクレート追加 |
| **B-2. 本体依存クレートの追加（大規模・ロックファイル全面更新）** | `.github/workflows/toolchain-export.yml` で `cache/rust-toolchain` ブランチの `vendor.tar.gz` を再生成 | 多数の推移的依存を伴う更新 |

---

## 1. ツールとしての導入手順（CLI バイナリ・ベンチマーク/テスト補助ツール）

本体バイナリ（`src/`）にはリンクされない開発ツール（`wasm-bindgen-cli` のような単体バイナリや、`criterion` のようなベンチマーク専用ハーネス、ブラウザ自動テスト道具など）を導入する手順。

### 1.1 パターン A：GitHub Releases バイナリ等の搬入（Git Blob 方式）

GitHub Releases の tarball（裏側で `objects.githubusercontent.com` にリダイレクトされるためサンドボックスから直接 `curl` できないもの）を導入する場合の手順。

1. **`.github/workflows/sandbox-tools.yml` に取得ジョブを追加する**
   - Actions ランナー（`ubuntu-latest`）上で配布元 URL から `curl -fsSL` でアーカイブを取得する。
   - 取得したアーカイブの `sha256sum` を計算する。
   - アーカイブを base64 エンコードし、`gh api -X POST "repos/${GITHUB_REPOSITORY}/git/blobs" --input blob.json --jq .sha` でリポジトリに **Git Blob** として登録する（どのブランチの履歴も汚さない）。
   - 生成された `blob=<40桁SHA>` と `sha256=<64桁ハッシュ>` を `$GITHUB_STEP_SUMMARY` および PR コメントに出力する。
2. **`.github/sandbox-tools.env` を `ENABLED=1`（および `SALT` を +1）にして push する**
   - ワークフローが走り、PR コメントに `blob=... sha256=...` が記録される。
   - 取得完了後は無駄な Actions 実行を防ぐため `.github/sandbox-tools.env` を `ENABLED=0` に戻す。
3. **サンドボックス側のスクリプト（`scripts/web-smoke.sh` 等）で取り出す**
   - サンドボックスからは `api.github.com` に到達できるため、以下のコマンドで `/home/user/build/` 配下（スナップショット対象外）に展開する:
     ```bash
     gh api "repos/$REPO/git/blobs/$BLOB_SHA" --jq .content | base64 -d > "$BUILD_DIR/tool.tar.gz"
     echo "$EXPECTED_SHA256  $BUILD_DIR/tool.tar.gz" | sha256sum -c -
     tar xzf "$BUILD_DIR/tool.tar.gz" -C "$BUILD_DIR/web-tools"
     ```

### 1.2 パターン B：ベンチマーク・テスト専用クレート（`[dev-dependencies]`）の搬入（`criterion` の実例）

`criterion` のように `Cargo.toml` の `[dev-dependencies]` に追加するツールクレートを導入する場合、サンドボックスの `/home/user/build/vendor-src/` に不足分の `.crate` ファイルを補充する必要がある。

1. **必要な推移的依存クレート一覧の特定**
   - 既存の `/home/user/build/vendor-src/` に入っていない差分クレートとバージョンを洗い出す（`criterion 0.5.1` の場合は `criterion`, `criterion-plot`, `ciborium`, `clap`, `itertools`, `oorandom`, `tinytemplate`, `anes`, `cast`, `crunchy`, `half` 等 19 クレート）。
   - ※注意：サンドボックスの Rust が `1.99.0`（`Cargo.lock` v4）である場合、Actions の `dtolnay/rust-toolchain@stable` が古いと `cargo vendor` が `lock file version 4` エラーで失敗する。そのため Actions 上では `cargo` を回さず、`https://static.crates.io/crates/${name}/${name}-${ver}.crate` から直接 `.crate` ファイルを `curl` で取得して `tar.gz` にまとめるのが最も確実で速い。
2. **`.github/workflows/sandbox-tools.yml` で `.crate` 一式を Git Blob 化する**
   ```bash
   mkdir -p crates
   for item in criterion:0.5.1 criterion-plot:0.5.0 ...; do
     name="${item%:*}"
     ver="${item#*:}"
     curl -fsSL --retry 3 -o "crates/${name}-${ver}.crate" \
       "https://static.crates.io/crates/${name}/${name}-${ver}.crate"
   done
   tar czf extra-crates.tar.gz crates
   ```
   これを Git Blob として POST し、`blob` SHA と `sha256` を控える。
3. **`scripts/bootstrap-toolchain.sh` で `/home/user/build/vendor-src/` に展開し `.cargo-checksum.json` を生成する**
   - Cargo のディレクトリ vendor（`[source.vendored-sources]`）では、各クレートディレクトリ直下に `.cargo-checksum.json`（パッケージ全体の SHA-256 と各ファイルの SHA-256 マップ）が必須となる。
   - `scripts/bootstrap-toolchain.sh` 内の Python スニペット（`REF:scripts/bootstrap-toolchain.sh` 130〜162 行目）と同じ手順で `.crate`（実体は `.tar.gz`）を `vendor-src/<crate_name>/` に展開し、`.cargo-checksum.json` を書き出す。
4. **`Cargo.lock` の更新と `[dev-dependencies]` 許可リストの更新**
   - `. /home/user/build/env.sh && cargo check --benches`（または `--tests`）を実行して `Cargo.lock` を更新する。
   - `tests/layering.rs` の `ALLOWED_DEV_DEPS` および設計書（`REF:docs/designs/overview.md` §3.2）の許可リストに対象クレートを追加する（仕様・設計の変更を伴うため事前に人間の承認を得ること）。

### 1.3 パターン C：npm / PyPI パッケージとして提供されているツールの導入

`registry.npmjs.org` と `pypi.org` にはサンドボックスから直接通信できるため、GitHub Actions を経由する必要はない。

- **格納場所の原則**: リポジトリのターン終了時スナップショット上限（128MB / 10,000 ファイル）を圧迫しないよう、`node_modules` や仮想環境は必ず **`/home/user/build/` 配下**（例: `/home/user/build/web-tools/node`）に作成する（`REF:scripts/web-smoke.sh` 参照）。

---

## 2. 本体依存クレート（`[dependencies]`）としての導入手順

アプリケーション本体（`src/core/`, `src/ports/`, `src/adapters/`, `src/ui/`, `src/cli/`）が使用する外部クレートを `Cargo.toml` の `[dependencies]` に追加する場合の手順。

### 2.1 アーキテクチャ規則と許可リストの確認（必須）

`novnow` では層の純粋性と供給網の最小化のため、`REF:tests/layering.rs` によって使用可能なクレートと配置層が厳格に検査されている。本体依存を追加する際は、まず以下を確認し**人間の承認**を得る。

1. **`src/core/**`（ドメイン層）で使うクレートの場合**:
   - I/O・非同期ランタイム・GUI・システム時刻・乱数を含まない純粋な計算クレートであること。
   - `REF:docs/designs/overview.md` §3.2 の許可クレート表、および `REF:tests/layering.rs` の許可リストを更新する必要がある。
   - `src/core/` から無関係なモジュールで誤用されないよう、`tests/layering.rs` で「どのサブモジュールからのみ利用を許可するか」を明記する。
2. **`src/ui/**`（GUI 層）専用のクレートの場合**:
   - `Cargo.toml` で必ず `optional = true` を付け、`[features] ui = [...]` のリストに含めること（CLI 専用ビルドに GUI 依存を混入させないため。`REF:tests/layering.rs` の feature gate 検査で強制される）。
3. **ターゲット固有クレート（デスクトップ専用 / WASM 専用）の場合**:
   - デスクトップ専用（例: `rfd`, `directories`, `fontdb`, `tokio`）は `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`、Web 専用（例: `wasm-bindgen`, `web-sys`, `js-sys`, `eframe` の web feature）は `[target.'cfg(target_arch = "wasm32")'.dependencies]` に配置する。
4. **LLM / AI SDK の禁止**:
   - OpenAI / Anthropic 等の商用 LLM クライアント SDK の追加は全面禁止されている（`REF:tests/no_llm_sdk.rs` で常時検査）。

### 2.2 クレートソースの搬入（2 つの方法）

#### 方法 1：差分クレートが少数の場合（Git Blob による差分追加 — 高速・推奨）

追加されるクレート（推移的依存を含む）が数個〜20 個程度であれば、§1.2（パターン B）と同じく `.github/workflows/sandbox-tools.yml` で `.crate` ファイルを Git Blob 化し、`scripts/bootstrap-toolchain.sh` で `/home/user/build/vendor-src/` に展開する。所要時間は約 1〜2 分で完了する。

#### 方法 2：依存ツリーが大きく変わる場合（`cache/rust-toolchain` ブランチの再生成）

大型クレート（新しい GUI バックエンド等）の追加や既存クレートの一括メジャーアップデートで数十個以上のクレートが変化する場合は、`.github/workflows/toolchain-export.yml` を使って `cache/rust-toolchain` ブランチの `vendor.tar.gz` 自体を再生成する。

1. `Cargo.toml` に追加したい依存を記述する。
2. `.github/toolchain.env` を編集する:
   ```dotenv
   ENABLED=1
   SALT=<現在の値 + 1>
   ```
3. `.github/workflows/toolchain-export.yml` の `06 vendor` ステップで `cargo vendor` が実行され、新しい `Cargo.lock` と全依存クレートをまとめた `vendor.tar.gz`（45MB チャンク分割）が孤立ブランチ `cache/rust-toolchain` に push される。
4. 完了後、`.github/toolchain.env` を `ENABLED=0` に戻し、サンドボックス側で次を実行して新しいツールチェーン・vendor を展開する:
   ```bash
   FORCE=1 bash scripts/bootstrap-toolchain.sh
   ```

### 2.3 導入後の必須検証チェックリスト

外部依存（ツールまたは本体クレート）を追加した後は、コミット・プッシュ前に必ず以下を実行して全検査が通ることを確認する。

```bash
. /home/user/build/env.sh && bash scripts/verify.sh
```

1. `python3 docs/script/duplicate.py`（設計書と計画書の `REF:`・整合性検査）
2. `cargo test --locked --test layering --test no_llm_sdk`（層境界・optional feature gate・禁止 SDK 検査）
3. `cargo clippy --locked --all-targets -- -D warnings`（GUI なしビルドの静的解析）
4. `cargo clippy --locked --all-targets --features ui -- -D warnings`（GUI ありビルドの静的解析）
5. `cargo check --locked --target wasm32-unknown-unknown --features ui`（WASM ターゲットの型検査）
