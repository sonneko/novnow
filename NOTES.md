# cache/rust-toolchain の中身

Arena の開発サンドボックス（Debian 12 / glibc 2.36 / cargo・rustc 無し / crates.io 到達不可）
で Rust を動かすための一式。`.github/workflows/toolchain-export.yml` が生成した。
孤立ブランチなので main にも作業ブランチにも影響しない。いつでも削除してよい:

    git push origin --delete cache/rust-toolchain

## 中身

- `MANIFEST.json` … status / cache_key / 各 tar.gz の sha256 とチャンク一覧
- `BUILD-LOG.txt` … 生成時のログ（Actions のログはサンドボックスから読めないため同梱）
- `ENV-INFO.txt`  … 同梱バイナリの ldd 結果（debian:12 で全部解決できることの確認）
- `SMOKE-RESULT.txt` … 同梱物だけでビルド＆テストした結果と所要時間
- `chunks/sysroot.tar.gz.NNN` … rustc / cargo / rust-std / rustfmt / clippy の sysroot
- `chunks/vendor.tar.gz.NNN`  … `cargo vendor` の出力（vendor-src/ と vendor-config.toml）
- `probe/` … 取り出し経路の検証用（3MB のランダムデータと sha256）

## サンドボックス側での展開

/home/user/build はスナップショット対象外（＝サンドボックス再作成で消える）ので、
消えていたらここからやり直す。

```sh
mkdir -p /home/user/build && cd /home/user/build
curl -sSL -H "Authorization: Bearer $(gh auth token)" -o tc.tgz \
  "https://codeload.github.com/sonneko/novnow/tar.gz/refs/heads/cache/rust-toolchain"
tar xzf tc.tgz && cd novnow-cache-rust-toolchain
cat chunks/sysroot.tar.gz.* > sysroot.tar.gz
cat chunks/vendor.tar.gz.*  > vendor.tar.gz
sha256sum -c SHA256SUMS
mkdir -p /home/user/build/sysroot /home/user/build/cargo-home
tar xzf sysroot.tar.gz -C /home/user/build/sysroot
tar xzf vendor.tar.gz  -C /home/user/build
cat > /home/user/build/cargo-home/config.toml <<'CFG'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "/home/user/build/vendor-src"

[net]
offline = true
CFG
cat > /home/user/build/env.sh <<'ENVS'
export PATH=/home/user/build/sysroot/bin:$PATH
export RUSTC=/home/user/build/sysroot/bin/rustc
export CARGO_HOME=/home/user/build/cargo-home
export CARGO_NET_OFFLINE=true
ENVS
```

そのあと、コマンドごとに次を読む（bash の呼び出しごとに環境は引き継がれないため）:

```sh
. /home/user/build/env.sh && cd /home/user/novnow && cargo test --all-targets
```
