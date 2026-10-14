#!/usr/bin/env bash
# novnow の開発用 Rust ツールチェーンを復旧する。
#
# 背景（PLANS.md §9.3 / docs/handoff.md §2）:
#   作業環境（Arena のサンドボックス）には cargo / rustc が無く、crates.io にも static.rust-lang.org
#   にも到達できない。そこで GitHub Actions 側（.github/workflows/toolchain-export.yml）が
#   debian:12 コンテナで rustup の sysroot と `cargo vendor` の出力を作り、45MB ずつに割って
#   孤立ブランチ cache/rust-toolchain に push してある。このスクリプトはそれを取り出して展開する。
#
# 使い方:
#   bash scripts/bootstrap-toolchain.sh              # 無ければ取得、あれば何もしない
#   FORCE=1 bash scripts/bootstrap-toolchain.sh      # 展開済みでも取り直す
#   SMOKE=1 bash scripts/bootstrap-toolchain.sh      # 展開後に `cargo test` まで回して確認する
#
# 環境変数:
#   BUILD_DIR  展開先。既定は $HOME/build。**スナップショット対象外**なので、サンドボックスが
#              再作成されたら消える（＝このスクリプトをもう一度回す）。
#   BRANCH     取り出し元の孤立ブランチ。既定 cache/rust-toolchain。
#   REPO       owner/name。既定は origin のリモート URL から推定。
#   FORCE      1 で展開済みでも取り直す。
#   SMOKE      1 で展開後にリポジトリのテストを回す（コールドビルド込みで約 2 分）。
#
# 所要時間: ダウンロード 376MB で約 15 秒（実測 28MB/s）＋展開約 25 秒。
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$HOME/build}"
BRANCH="${BRANCH:-cache/rust-toolchain}"

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# --- 前提 -------------------------------------------------------------------
for tool in curl tar cat sha256sum gh python3; do
    command -v "$tool" >/dev/null 2>&1 || die "$tool が無い。これ無しでは復旧できない。"
done

if [ -z "${FORCE:-}" ] && [ -x "$BUILD_DIR/sysroot/bin/rustc" ]; then
    if PATH="$BUILD_DIR/sysroot/bin:$PATH" rustc --version >/dev/null 2>&1; then
        if [ -f "$BUILD_DIR/env.sh" ] && [ -f "$BUILD_DIR/cargo-home/config.toml" ]; then
            say "すでにある: $BUILD_DIR（$(PATH="$BUILD_DIR/sysroot/bin:$PATH" rustc --version)）"
            echo "    使うときは次を各コマンドの前に置く: . $BUILD_DIR/env.sh"
            exit 0
        fi
        # sysroot は生きているが設定だけ消えた → 再ダウンロードせずに書き直す
        say "sysroot はあるので env.sh と cargo-home/config.toml だけ書き直す"
        REGENERATE_ONLY=1
    else
        say "$BUILD_DIR/sysroot/bin/rustc が動かないので取り直す"
    fi
fi

TOKEN="$(gh auth token 2>/dev/null || true)"
[ -n "$TOKEN" ] || die "gh の認証が無い（gh auth token が空）。GitHub 接続を確認すること。"

if [ -z "${REPO:-}" ]; then
    url="$(git -C "$ROOT" remote get-url origin 2>/dev/null || true)"
    REPO="$(printf '%s' "$url" | sed -E 's#.*github\.com[:/]##; s#\.git$##')"
fi
case "$REPO" in */*) ;; *) die "REPO を推定できなかった（origin=$url）。REPO=owner/name を指定すること。";; esac

mkdir -p "$BUILD_DIR/sysroot" "$BUILD_DIR/cargo-home"
if [ "${REGENERATE_ONLY:-0}" != 1 ]; then
say "取り出し元: https://codeload.github.com/$REPO/tar.gz/refs/heads/$BRANCH"
say "展開先:     $BUILD_DIR（スナップショット対象外）"
cd "$BUILD_DIR"

# --- 取得（codeload だけが到達できる経路。objects.githubusercontent.com は不可）---
curl -fsSL -H "Authorization: Bearer $TOKEN" -o tc.tgz \
    "https://codeload.github.com/$REPO/tar.gz/refs/heads/$BRANCH" \
    || die "ダウンロードに失敗した。ブランチ $BRANCH が存在するか確認すること（gh api repos/$REPO/branches）。"
say "ダウンロード完了: $(du -h tc.tgz | cut -f1)"

rm -rf ./novnow-"$BRANCH"
tar xzf tc.tgz
work="$(find . -maxdepth 1 -type d -name 'novnow-*' | head -1)"
[ -n "$work" ] || die "tarball の中に展開先ディレクトリが見つからない"
cd "$work"

cat chunks/sysroot.tar.gz.* > sysroot.tar.gz
cat chunks/vendor.tar.gz.*  > vendor.tar.gz
say "sha256 の検証"
sha256sum -c SHA256SUMS || die "sha256 が合わない。ブランチが途中で切れている（再生成すること）。"

mkdir -p "$BUILD_DIR/sysroot" "$BUILD_DIR/cargo-home"
say "sysroot を展開（約 700MB）"
tar xzf sysroot.tar.gz -C "$BUILD_DIR/sysroot"
say "vendor を展開（474 クレート）"
tar xzf vendor.tar.gz -C "$BUILD_DIR"

cat > "$BUILD_DIR/cargo-home/config.toml" <<CFG
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "$BUILD_DIR/vendor-src"

[net]
offline = true
CFG

cat > "$BUILD_DIR/env.sh" <<ENVS
# novnow の開発ツールチェーン（scripts/bootstrap-toolchain.sh が生成）。
# bash の呼び出しごとに環境は引き継がれないので、**毎回**これを読むこと:
#   . $BUILD_DIR/env.sh && cargo test --locked --all-targets --no-fail-fast
export PATH=$BUILD_DIR/sysroot/bin:\$PATH
export RUSTC=$BUILD_DIR/sysroot/bin/rustc
export CARGO_HOME=$BUILD_DIR/cargo-home
export CARGO_NET_OFFLINE=true
# M7（2026-10-13）: GUI 込み（--features ui）のテストバイナリは debuginfo 付きだと 1 本 200MB を超え、
# 40 本で作業環境のディスク（21GB）を使い切った。手元では行番号だけにし、incremental も切る（CI には効かない）。
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
ENVS

# 中間生成物はディスクを食うので消す（376MB の tarball ＋ 連結した 2 つの tar.gz）
rm -rf "$BUILD_DIR/tc.tgz" "$BUILD_DIR/$work"
fi  # REGENERATE_ONLY

say "確認"
# shellcheck disable=SC1091
. "$BUILD_DIR/env.sh"
cd "$ROOT"
rustc --version
cargo --version
cargo fmt --version
cargo clippy --version
# ここで cargo check を回すとコールドビルド（約 2 分）が走るので回さない。
# 動作確認は SMOKE=1 か scripts/verify.sh で行う。

if [ "${SMOKE:-0}" = "1" ]; then
    say "SMOKE=1: cargo test を回す（コールドビルド込み約 2 分）"
    cargo test --locked --all-targets --no-fail-fast
fi

say "復旧完了。各コマンドの前に次を置くこと:"
echo "    . $BUILD_DIR/env.sh && cd $ROOT && cargo test --locked --all-targets --no-fail-fast"
