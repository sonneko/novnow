#!/usr/bin/env bash
# Web 版をヘッドレスの Chromium で実際に動かして確かめる（`PLANS.md` §5.8 の「Web で IndexedDB の接続を
# 一通り動かす」・§9.2 B-156〜B-158）。CI では回さない（作業環境で回す道具）。
#
# 背景: 作業環境（Arena）から GitHub のリリースの asset と Chrome の配布元には届かない。
#   - wasm-bindgen CLI（Cargo.lock の wasm-bindgen と同じ版）は `.github/workflows/sandbox-tools.yml` が
#     git の blob にして PR にコメントしたものを api.github.com から取る（`.github/sandbox-tools.env` を
#     変えて push すると作り直す）。
#   - Chromium は npm の `@sparticuz/chromium`（バイナリを同梱している）と `puppeteer-core`。
#
# 使い方:
#   bash scripts/web-smoke.sh            # 道具の用意 → release の wasm → dist → 筋書き（scripts/web-smoke.mjs）
#   bash scripts/web-smoke.sh --no-build # dist を作り直さずに筋書きだけ
#   bash scripts/web-smoke.sh --shot     # 選択画面を 1 枚撮るだけ（OUT_DIR/00-picker.png）
#
# 環境変数:
#   BUILD_DIR  ツールチェーンの場所（既定 $HOME/build）。道具は $BUILD_DIR/web-tools に置く
#   OUT_DIR    スクリーンショットとダウンロードの置き場（既定 /tmp/novnow-web）
#   PR         sandbox-tools のコメントを探す PR（既定: 今のブランチの PR）
#
# 終了コード: 筋書きが全部通れば 0。
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$HOME/build}"
TOOLS="$BUILD_DIR/web-tools"
OUT_DIR="${OUT_DIR:-/tmp/novnow-web}"
DIST="$OUT_DIR/dist"
BUILD=1
MODE=all
for arg in "$@"; do
    case "$arg" in
        --no-build) BUILD=0 ;;
        --shot) MODE=shot ;;
        -h|--help) sed -n '2,22p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) printf 'error: 未知の引数 %s\n' "$arg" >&2; exit 2 ;;
    esac
done

say() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
mkdir -p "$TOOLS" "$OUT_DIR"

# --- wasm-bindgen CLI（Cargo.lock と同じ版）---
VERSION="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$ROOT/Cargo.lock")"
WB="$TOOLS/wasm-bindgen-$VERSION-x86_64-unknown-linux-musl/wasm-bindgen"
if [ ! -x "$WB" ]; then
    say "wasm-bindgen $VERSION を取る（sandbox-tools の blob）"
    REPO="$(cd "$ROOT" && gh repo view --json nameWithOwner --jq .nameWithOwner)"
    PR="${PR:-$(cd "$ROOT" && gh pr view --json number --jq .number)}"
    LINE="$(gh api "repos/$REPO/issues/$PR/comments" --paginate --jq '.[].body' \
        | grep "^sandbox-tools: wasm-bindgen=$VERSION " | tail -1 || true)"
    if [ -z "$LINE" ]; then
        printf 'error: PR #%s に wasm-bindgen=%s の sandbox-tools のコメントが無い。\n' "$PR" "$VERSION" >&2
        printf '       .github/sandbox-tools.env の SALT を変えて push し、ワークフローを走らせる。\n' >&2
        exit 1
    fi
    BLOB="$(sed -E 's/.* blob=([0-9a-f]+).*/\1/' <<<"$LINE")"
    SUM="$(sed -E 's/.* sha256=([0-9a-f]+).*/\1/' <<<"$LINE")"
    for _ in 1 2 3 4 5; do
        if gh api "repos/$REPO/git/blobs/$BLOB" >"$TOOLS/blob.json" 2>/dev/null; then break; fi
        sleep 3
    done
    python3 - "$TOOLS" <<'PY'
import base64, json, sys
tools = sys.argv[1]
data = json.load(open(f"{tools}/blob.json"))
open(f"{tools}/wasm-bindgen.tar.gz", "wb").write(base64.b64decode(data["content"]))
PY
    rm -f "$TOOLS/blob.json"
    echo "$SUM  $TOOLS/wasm-bindgen.tar.gz" | sha256sum -c -
    tar xzf "$TOOLS/wasm-bindgen.tar.gz" -C "$TOOLS"
fi
"$WB" --version

# --- Chromium（@sparticuz/chromium のバイナリを展開）と puppeteer-core ---
NODE_DIR="$TOOLS/node"
CHROME="$TOOLS/chromium/chromium"
if [ ! -x "$CHROME" ] || [ ! -d "$NODE_DIR/node_modules/puppeteer-core" ]; then
    say "Chromium と puppeteer-core を npm から取る"
    mkdir -p "$NODE_DIR" "$TOOLS/chromium"
    (cd "$NODE_DIR" && [ -f package.json ] || npm init -y >/dev/null)
    (cd "$NODE_DIR" && npm install --no-audit --no-fund --silent @sparticuz/chromium puppeteer-core >/dev/null)
    node - "$NODE_DIR/node_modules/@sparticuz/chromium/bin" "$TOOLS/chromium" <<'JS'
const fs = require('fs'), zlib = require('zlib'), path = require('path');
const [bin, out] = process.argv.slice(2);
fs.writeFileSync(path.join(out, 'chromium'), zlib.brotliDecompressSync(fs.readFileSync(path.join(bin, 'chromium.br'))), { mode: 0o755 });
for (const name of ['al2023', 'swiftshader', 'fonts']) {
  const file = path.join(bin, `${name}.tar.br`);
  if (fs.existsSync(file)) fs.writeFileSync(path.join(out, `${name}.tar`), zlib.brotliDecompressSync(fs.readFileSync(file)));
}
JS
    for name in al2023 swiftshader fonts; do
        [ -f "$TOOLS/chromium/$name.tar" ] && tar xf "$TOOLS/chromium/$name.tar" -C "$TOOLS/chromium" && rm -f "$TOOLS/chromium/$name.tar"
    done
fi

# --- release の wasm → wasm-bindgen → dist（trunk の代わりに index.html の <link data-trunk> を置き換える）---
if [ "$BUILD" = 1 ] || [ ! -f "$DIST/novnow_bg.wasm" ]; then
    say "release の wasm を作る"
    # shellcheck source=/dev/null
    . "$BUILD_DIR/env.sh"
    (cd "$ROOT" && cargo build --release --locked --target wasm32-unknown-unknown --features ui --bin novnow)
    rm -rf "$DIST" && mkdir -p "$DIST"
    "$WB" --target web --no-typescript --out-dir "$DIST" --out-name novnow \
        "$ROOT/target/wasm32-unknown-unknown/release/novnow.wasm"
    cp "$ROOT/sw.js" "$ROOT/manifest.webmanifest" "$ROOT/icon.svg" "$DIST/"
    python3 - "$ROOT/index.html" "$DIST/index.html" <<'PY'
import re, sys
source = open(sys.argv[1], encoding="utf-8").read()
script = '<script type="module">import init from "./novnow.js"; init({ module_or_path: "./novnow_bg.wasm" });</script>'
open(sys.argv[2], "w", encoding="utf-8").write(re.sub(r'<link rel="rust"[^>]*/>', script, source))
PY
fi

say "筋書きを回す（$MODE）"
LD_LIBRARY_PATH="$TOOLS/chromium/lib:$TOOLS/chromium" \
    NODE_DIR="$NODE_DIR" CHROME="$CHROME" DIST="$DIST" OUT_DIR="$OUT_DIR" MODE="$MODE" \
    node "$HERE/web-smoke.mjs"
