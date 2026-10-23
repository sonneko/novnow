#!/usr/bin/env bash
# Python 側の検証（§19.3 の Python API とその生成物）。
#
# **常に走る**（Rust のビルド不要・1 秒）:
#   1. 型スタブ（§19.3.1）がカタログ（§19.7）から生成したとおりか（`scripts/pyi-site.py --check`）
#   2. Python のソースが構文的に正しいか（`compileall`。`-B` なので `__pycache__` は作らない）
#   3. 位置引数の束縛（§19.3.2）と 9 例外（§19.3.3）の検査
#      （`novnow._core` を偽物に差し替えるので、拡張をビルドせずに走る）
#
# **pyo3 の vendor があるときだけ走る**（`$BUILD_DIR/vendor-src/pyo3`。約 2 分）:
#   4. `python/novnow-py` の clippy（`-D warnings`）と release ビルド
#   5. 成果物の `.so` を `novnow/_core.so` へ置いて本物の拡張に対する pytest
#      （`REF:.github/workflows/wheels.yml` の test ジョブと同じ検査を手元で）
#
# vendor は `REF:scripts/bootstrap-toolchain.sh` の `install_pyo3_vendor` が
# 孤立ブランチ `cache/pyo3-crates`（`REF:.github/workflows/sandbox-tools.yml` の
# `pyo3-crates` ジョブが生成）から取り出す。無ければ 4・5 は警告して飛ばす。
#
#   bash scripts/python-check.sh                     # 1〜3 だけ（約 1 秒）
#   bash scripts/python-check.sh --wheel dist/x.whl  # ホイールの版・abi3 タグも検査
#   bash scripts/python-check.sh --full              # 4・5 も回す（約 2 分半。`PYO3_BUILD=1` でも可）
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$HOME/build}"
PY="$ROOT/python/novnow-py"
FULL="${PYO3_BUILD:-0}"
WHEEL=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --full) FULL=1 ;;
        --wheel) shift; WHEEL="${1:?--wheel にはファイルが要る}" ;;
        -h|--help) sed -n '2,26p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) printf 'error: 未知の引数 %s\n' "$1" >&2; exit 2 ;;
    esac
    shift
done
cd "$ROOT"

echo "==> 型スタブ（§19.3.1）がカタログ（§19.7）から生成したとおりか"
python3 -B scripts/pyi-site.py --check

echo "==> Python ソースの構文"
python3 -B -m compileall -q python/novnow-py scripts

echo "==> 束縛と例外の検査（\`_core\` は偽物）"
python3 -B -m unittest discover -s python/novnow-py/tests -t python/novnow-py/tests -p 'test_binding.py'

if [ -n "$WHEEL" ]; then
    echo "==> ホイールの版・abi3 タグを Cargo.toml と照合"
    python3 -B scripts/release-matrix.py --verify-wheel "$WHEEL"
fi

# --- 拡張のビルドと本物の pytest（--full で、かつ vendor があるときだけ） --------
if [ "$FULL" != 1 ]; then
    echo "==> 拡張のビルドと pytest は --full で走る（約 2 分半）"
    exit 0
fi
if [ ! -d "$BUILD_DIR/vendor-src/pyo3" ]; then
    printf '\033[1;33mskip:\033[0m %s\n' \
        "$BUILD_DIR/vendor-src/pyo3 が無いので拡張のビルドと pytest は飛ばす" \
        "（bash scripts/bootstrap-toolchain.sh で cache/pyo3-crates を展開できる）"
    exit 0
fi
if [ ! -f "$BUILD_DIR/env.sh" ]; then
    printf '\033[1;33mskip:\033[0m %s\n' "$BUILD_DIR/env.sh が無い（ツールチェーン未復旧）"
    exit 0
fi
# shellcheck disable=SC1091
. "$BUILD_DIR/env.sh"

echo "==> python/novnow-py の clippy（-D warnings）"
( cd "$PY" && cargo clippy --offline --all-targets -- -D warnings )

echo "==> python/novnow-py の release ビルド"
( cd "$PY" && cargo build --release --offline )

echo "==> 本物の拡張に対する pytest（§19.3.2 / §19.3.3 / §19.3.5）"
cp "$PY/target/release/lib_core.so" "$PY/python/novnow/_core.so"
if python3 -c "import pytest" >/dev/null 2>&1; then
    ( cd "$PY" && PYTHONPATH=python python3 -m pytest tests -q )
else
    printf '\033[1;33mskip:\033[0m %s\n' \
        "pytest が無いので本物の拡張の検査は飛ばす（pip install pytest）" \
        "。import と操作数だけ確認する"
    ( cd "$PY" && PYTHONPATH=python python3 -c \
        'import novnow; assert len(novnow.operations()) == 119; print("novnow", novnow.__version__, "119 operations")' )
fi
