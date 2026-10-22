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
#   PYO3_BRANCH pyo3 vendor の孤立ブランチ。既定 cache/pyo3-crates（無ければ警告して続く）。
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
        if [ -f "$BUILD_DIR/env.sh" ] && [ -f "$BUILD_DIR/cargo-home/config.toml" ] \
            && [ -d "$BUILD_DIR/vendor-src/criterion" ] && [ -d "$BUILD_DIR/vendor-src/tiniestsegmenter" ]; then
            say "すでにある: $BUILD_DIR（$(PATH="$BUILD_DIR/sysroot/bin:$PATH" rustc --version)）"
            echo "    使うときは次を各コマンドの前に置く: . $BUILD_DIR/env.sh"
            exit 0
        fi
        # sysroot は生きているが設定または追加クレート vendor だけ消えた → 再ダウンロードせずに書き直す
        say "sysroot はあるので env.sh / cargo-home/config.toml / 追加クレート vendor だけ補う"
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

# --- 追加クレート（git blob）の展開 ------------------------------------------
# docs/add-dependencies.md §1.2 パターン B。`$BUILD_DIR/crates/*.crate` を
# `$BUILD_DIR/vendor-src/<name>/` に展開し、ディレクトリ vendor に必須の
# `.cargo-checksum.json`（package = .crate の sha256、files = 展開後ファイルの sha256）を書く。
install_extra_crates() {
    BUILD_DIR="$BUILD_DIR" python3 - <<'PY'
import glob, hashlib, json, os, shutil, tarfile
build_dir = os.environ["BUILD_DIR"]
vendor_root = os.path.join(build_dir, "vendor-src")
for crate_path in sorted(glob.glob(os.path.join(build_dir, "crates", "*.crate"))):
    base = os.path.basename(crate_path)[:-6]
    name, _ = base.rsplit("-", 1)
    pkg_sha = hashlib.sha256(open(crate_path, "rb").read()).hexdigest()
    dest = os.path.join(vendor_root, name)
    if os.path.exists(dest):
        shutil.rmtree(dest)
    os.makedirs(dest, exist_ok=True)
    with tarfile.open(crate_path, "r:gz") as tf:
        for member in tf.getmembers():
            parts = member.name.split("/", 1)
            if len(parts) == 2 and parts[1]:
                member.name = parts[1]
                tf.extract(member, dest)
    files = {}
    for root, _, fnames in os.walk(dest):
        for fn in fnames:
            if fn == ".cargo-checksum.json":
                continue
            full = os.path.join(root, fn)
            rel = os.path.relpath(full, dest).replace(os.sep, "/")
            files[rel] = hashlib.sha256(open(full, "rb").read()).hexdigest()
    with open(os.path.join(dest, ".cargo-checksum.json"), "w") as f:
        json.dump({"files": files, "package": pkg_sha}, f, separators=(",", ":"))
PY
}

# blob を取り出し、sha256 を検証してから `install_extra_crates` に渡す。
#   $1 = 表示名, $2 = blob sha, $3 = tar.gz の sha256, $4 = tar.gz のファイル名
install_crates_blob() {
    say "$1 を git blob から展開"
    local blob="$2" sha="$3" file="$4"
    gh api "repos/$REPO/git/blobs/$blob" --jq .content | base64 -d > "$BUILD_DIR/$file"
    echo "$sha  $BUILD_DIR/$file" | sha256sum -c - >/dev/null || die "$file の sha256 が合わない"
    tar xzf "$BUILD_DIR/$file" -C "$BUILD_DIR"
    install_extra_crates
    rm -rf "$BUILD_DIR/$file" "$BUILD_DIR/crates"
}

# M11（B-25）: criterion 0.5.1 とその推移的依存 19 クレート（ベンチマーク用 dev-dependency）
if [ ! -d "$BUILD_DIR/vendor-src/criterion" ]; then
    install_crates_blob "criterion vendor（19 クレート）" \
        74b3e210ac964dae4e37f5c03a9b1b129e5fee9f \
        f4592ca89324ef7dad5a3f39421efac078f9a9d77fb2d1c3e3454be5f34dd568 \
        criterion-crates.tar.gz
fi

# S-09 / F-23: tiniestsegmenter 0.3.0（`'` 未解決参照の形態素境界。本体依存）
if [ ! -d "$BUILD_DIR/vendor-src/tiniestsegmenter" ]; then
    install_crates_blob "tiniestsegmenter vendor（1 クレート）" \
        ab68047ca32a40bc5cb4a678b7f6fd62f8711e04 \
        557df849c79f8087211ee5f18c335ca69f6a6a501a2aa088fd209b0947578aaa \
        japanese-crates.tar.gz
fi

# --- pyo3（§19.3 Python API。`python/novnow-py` のビルドに要る）-----------------
# `python/novnow-py` は root の workspace から外してあるので、pyo3 系だけ別の vendor
# （孤立ブランチ cache/pyo3-crates。`REF:.github/workflows/sandbox-tools.yml` の
# `pyo3-crates` ジョブが作る）から取り出して、同じ $BUILD_DIR/vendor-src へ合わせる。
# ブランチが無い（＝まだ harvest を走らせていない）ときは**警告だけ**して続ける:
# 本体のビルドには pyo3 が要らない。
PYO3_BRANCH="${PYO3_BRANCH:-cache/pyo3-crates}"
install_pyo3_vendor() {
    if [ -d "$BUILD_DIR/vendor-src/pyo3" ]; then
        say "pyo3 vendor はすでにある: $BUILD_DIR/vendor-src/pyo3"
        return 0
    fi
    say "pyo3 vendor を取り出す: https://codeload.github.com/$REPO/tar.gz/refs/heads/$PYO3_BRANCH"
    local tmp="$BUILD_DIR/pyo3-fetch"
    rm -rf "$tmp"; mkdir -p "$tmp"
    if ! curl -fsSL -H "Authorization: Bearer $TOKEN" -o "$tmp/src.tgz" \
        "https://codeload.github.com/$REPO/tar.gz/refs/heads/$PYO3_BRANCH"; then
        printf '\033[1;33mwarning:\033[0m %s\n' \
            "ブランチ $PYO3_BRANCH が無い。python/novnow-py はビルドできません。" >&2
        printf '\033[1;33mwarning:\033[0m %s\n' \
            "用意するには GitHub Actions の sandbox-tools を workflow_dispatch で回すこと" \
            "（.github/workflows/sandbox-tools.yml の pyo3-crates ジョブ）。" >&2
        rm -rf "$tmp"
        return 0
    fi
    tar xzf "$tmp/src.tgz" -C "$tmp"
    local work
    work="$(find "$tmp" -maxdepth 1 -type d -name 'novnow-*' | head -1)"
    [ -n "$work" ] || die "pyo3 vendor の tarball の中に展開先ディレクトリが見つからない"
    ( cd "$work" && sha256sum -c SHA256SUMS ) || die "pyo3 vendor の sha256 が合わない"
    say "pyo3 vendor を $BUILD_DIR/vendor-src へ合わせる（$(cat "$work/PYO3_VERSION")）"
    BUILD_DIR="$BUILD_DIR" python3 - "$work/pyo3-crates.tar.gz" <<'PYEOF'
import os
import sys
import tarfile

build_dir = os.environ["BUILD_DIR"]
target = os.path.join(build_dir, "vendor-src")
staging = os.path.join(build_dir, "pyo3-src")
os.makedirs(target, exist_ok=True)
with tarfile.open(sys.argv[1]) as archive:
    archive.extractall(staging)
added, skipped = [], []
for name in sorted(os.listdir(staging)):
    source = os.path.join(staging, name)
    if not os.path.isdir(source):
        continue
    if os.path.exists(os.path.join(target, name)):
        skipped.append(name)
        continue
    os.rename(source, os.path.join(target, name))
    added.append(name)
print("    追加 %d: %s" % (len(added), " ".join(added) or "(なし)"))
print("    既存 %d: %s" % (len(skipped), " ".join(skipped) or "(なし)"))
PYEOF
    rm -rf "$tmp"
}
install_pyo3_vendor

bash "$ROOT/scripts/bootstrap-jpeg.sh"

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
