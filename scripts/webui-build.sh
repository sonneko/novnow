#!/usr/bin/env bash
# 第 2 の画面実装（`REF:webui/`）の wasm を組む（`REF:docs/designs/webui.md` §21.3）。
#
# やること:
#   1. `crates/novnow-wasm` を wasm32 向けに release で組む。
#   2. `wasm-bindgen --target web` で `webui/src/wasm/` へ出す（git には入れない）。
#
# 使い方:
#   bash scripts/webui-build.sh            # 組む（既にあれば --force が無い限り作り直さない）
#   bash scripts/webui-build.sh --force    # 必ず作り直す
#   bash scripts/webui-build.sh --check    # 組めるかだけ見る（CI 用。`verify.sh` から呼ぶ）
#
# 環境変数:
#   BUILD_DIR    ツールチェーンの場所（既定 $HOME/build。scripts/bootstrap-toolchain.sh が作る）
#   WASM_BINDGEN wasm-bindgen の場所（既定: $BUILD_DIR/web-tools の展開先 → PATH の順）
#
# wasm-pack を使わない理由: wasm-pack はインストール時に GitHub の release から
# バイナリを落とすので、ネットワークの限られた作業環境（`docs/developers.md` §2.2）では
# 入らない。`wasm-pack build --target web` と同じ成果物を、cargo + wasm-bindgen で作る。
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$HOME/build}"
OUT="$ROOT/webui/src/wasm"
FORCE=0
CHECK=0
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=1 ;;
        --check) CHECK=1 ;;
        -h|--help) sed -n '2,22p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) printf 'error: 未知の引数 %s\n' "$arg" >&2; exit 2 ;;
    esac
done

if ! command -v cargo >/dev/null 2>&1; then
    if [ -f "$BUILD_DIR/env.sh" ]; then
        # shellcheck disable=SC1091
        . "$BUILD_DIR/env.sh"
    else
        echo "error: cargo が無い（先に bash scripts/bootstrap-toolchain.sh）" >&2
        exit 2
    fi
fi

find_bindgen() {
    if [ -n "${WASM_BINDGEN:-}" ] && [ -x "$WASM_BINDGEN" ]; then
        printf '%s\n' "$WASM_BINDGEN"
        return 0
    fi
    if command -v wasm-bindgen >/dev/null 2>&1; then
        command -v wasm-bindgen
        return 0
    fi
    if [ -d "$BUILD_DIR/web-tools" ]; then
        local found
        found="$(find "$BUILD_DIR/web-tools" -maxdepth 3 -type f -name wasm-bindgen -perm -u+x 2>/dev/null | head -1)"
        if [ -n "$found" ]; then
            printf '%s\n' "$found"
            return 0
        fi
    fi
    return 1
}

cd "$ROOT" || exit 2
echo "webui-build: cargo build --locked --release --target wasm32-unknown-unknown -p novnow-wasm"
# `--offline` は付けない。手元（`scripts/bootstrap-toolchain.sh` の vendor）は
# `$CARGO_HOME/config.toml` の `[net] offline = true` で既にオフラインになり、CI は
# registry から取る。ここで固定すると、CI で「no matching package」になる。
cargo build --locked --release --target wasm32-unknown-unknown -p novnow-wasm >/dev/null || {
    echo "error: wasm のビルドに失敗した（cargo build --locked --release --target wasm32-unknown-unknown -p novnow-wasm）" >&2
    exit 1
}
WASM="$ROOT/target/wasm32-unknown-unknown/release/novnow_wasm.wasm"
[ -f "$WASM" ] || { echo "error: $WASM が無い" >&2; exit 1; }

if [ "$CHECK" = 1 ]; then
    echo "webui-build: ビルドできた（$WASM）"
    exit 0
fi

BINDGEN="$(find_bindgen)" || {
    version="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |\"/, ""); print; exit }' Cargo.lock)"
    cat >&2 <<MSG
error: wasm-bindgen が見つからない（Cargo.lock の版は ${version}）。

    bash scripts/bootstrap-toolchain.sh     # 手元のツールチェーンと一緒に入る
    # CI は .github/workflows/ci.yml の webui ジョブが次の形で入れる:
    #   curl -fsSL https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${version}/\
    #     wasm-bindgen-${version}-x86_64-unknown-linux-musl.tar.gz | tar xzf - -C ~/.cargo/bin
    # または WASM_BINDGEN=/path/to/wasm-bindgen bash scripts/webui-build.sh

（wasm-bindgen の版は Cargo.lock の wasm-bindgen と同じにする。\`docs/developers.md\` §8.10 の 33）
MSG
    exit 2
}

if [ "$FORCE" = 0 ] && [ -f "$OUT/novnow_wasm_bg.wasm" ] && [ "$OUT/novnow_wasm.js" -nt "$WASM" ]; then
    echo "webui-build: 最新（--force で作り直す）"
    exit 0
fi

mkdir -p "$OUT"
echo "webui-build: wasm-bindgen --target web → ${OUT#$ROOT/}"
"$BINDGEN" --target web --out-dir "$OUT" --out-name novnow_wasm "$WASM" || {
    echo "error: wasm-bindgen に失敗した" >&2
    exit 1
}
# 版を記録する（CI の wasm-bindgen と食い違ったときの手掛かり。§8.10 の 33）
"$BINDGEN" --version >"$OUT/VERSION" 2>/dev/null || true
echo "webui-build: できた（$(du -h "$OUT/novnow_wasm_bg.wasm" | cut -f1)）"
