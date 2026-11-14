#!/usr/bin/env bash
# 第 2 の画面実装（`REF:webui/`）の検査（`REF:docs/designs/webui.md` §21.6）。
#
# やること:
#   1. 生成物が正本どおりか（`scripts/api-ts.py --check` / `theme-ts.py --check` /
#      `i18n-ts.py --check`）。手で直した状態・正本を変えて生成し忘れた状態のどちらも落とす。
#   2. wasm が組めるか（`scripts/webui-build.sh --check`。`webui/src/wasm/` は git に入れない）。
#   3. `--full` のときだけ: `npm ci`（または既存の node_modules）→ `svelte-check` → `node --test`
#      → `vite build`。CI は `--full`、手元の `scripts/verify.sh` の既定は 1〜3 の軽いほう。
#
# 使い方:
#   bash scripts/webui-check.sh          # 生成物 + wasm のビルド（Rust のビルドが要る）
#   bash scripts/webui-check.sh --full   # さらに型検査・テスト・本番ビルド（node 22 が要る）
#   bash scripts/webui-check.sh --quick  # wasm を組まずに生成物だけ（1 秒）
#
# 環境変数:
#   BUILD_DIR  ツールチェーンの場所（既定 $HOME/build。scripts/bootstrap-toolchain.sh が作る）
#   NPM        npm の場所（既定 PATH の npm）
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$HOME/build}"
FULL=0
WASM=1
for arg in "$@"; do
    case "$arg" in
        --full)  FULL=1 ;;
        --quick) WASM=0 ;;
        -h|--help) sed -n '2,19p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) printf 'error: 未知の引数 %s\n' "$arg" >&2; exit 2 ;;
    esac
done

cd "$ROOT" || exit 2
step() { printf '\n==> %s\n' "$*"; }

# --- 1. 生成物 --------------------------------------------------------------
step "生成物が正本どおりか（api / theme / i18n）"
python3 scripts/api-ts.py --check || exit 1
python3 scripts/theme-ts.py --check || exit 1
python3 scripts/i18n-ts.py --check || exit 1

# --- 2. wasm -----------------------------------------------------------------
if [ "$WASM" = 1 ]; then
    step "wasm が組めるか（scripts/webui-build.sh --check）"
    bash scripts/webui-build.sh --check || exit 1
fi

# --- 3. 型検査・テスト・本番ビルド -------------------------------------------
if [ "$FULL" = 0 ]; then
    echo
    echo "webui-check: ok（--full で型検査・テスト・本番ビルドも回す）"
    exit 0
fi

if ! command -v npm >/dev/null 2>&1; then
    echo "error: npm が無い（node 22 以降が要る。docs/developers.md §2.3）" >&2
    exit 2
fi

step "依存の導入（npm ci）"
if [ -d webui/node_modules ] && [ "${WEBUI_NPM_CI:-0}" != 1 ]; then
    echo "webui/node_modules があるので npm ci を省く（WEBUI_NPM_CI=1 で強制）"
else
    ( cd webui && npm ci --no-audit --no-fund ) || exit 1
fi

# `webui/src/wasm/` は生成物なので、型検査・ビルドの前に必ず作る。
if [ ! -f webui/src/wasm/novnow_wasm.js ]; then
    step "wasm を作る（webui/src/wasm/ が無い）"
    bash scripts/webui-build.sh || exit 1
fi

step "svelte-check（型検査・警告）"
( cd webui && npm run --silent check ) || exit 1

step "node --test（IndexedDB 永続化を含む回帰テスト）"
( cd webui && npm test ) || exit 1

step "vite build（本番ビルド）"
( cd webui && npm run --silent build ) || exit 1

echo
echo "webui-check: ok"
