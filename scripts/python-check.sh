#!/usr/bin/env bash
# Python 側の検証（**Rust のビルド不要**。§19.3 の Python API とその生成物）。
#
#   1. 型スタブ（§19.3.1）がカタログ（§19.7）から生成したとおりか（`scripts/pyi-site.py --check`）
#   2. Python のソースが構文的に正しいか（`compileall`。`-B` なので `__pycache__` は作らない）
#   3. 位置引数の束縛（§19.3.2）と 9 例外（§19.3.3）の検査
#      （`novnow._core` を偽物に差し替えるので、拡張をビルドせずに走る）
#
# 本物の拡張を要する検査（`maturin build` ＋ `pytest python/novnow-py`）は
# `REF:.github/workflows/wheels.yml` の test ジョブが担う。そこでは `test_binding.py` が
# 自動的に skip され、`test_api.py` が本物の拡張を検査する。
#
#   bash scripts/python-check.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
cd "$ROOT"

echo "==> 型スタブ（§19.3.1）がカタログ（§19.7）から生成したとおりか"
python3 -B scripts/pyi-site.py --check

echo "==> Python ソースの構文"
python3 -B -m compileall -q python/novnow-py scripts

echo "==> 束縛と例外の検査（`_core` は偽物）"
python3 -B -m unittest discover -s python/novnow-py/tests -t python/novnow-py/tests -p 'test_binding.py'
