#!/usr/bin/env bash
# リポジトリ同梱の git フック（`.githooks/`）を有効にする。何度実行してもよい。
#
# 有効になるもの:
#   commit-msg — 既定で `[ci skip]` を付ける（`.githooks/commit-msg` の説明を参照）。
#
# フックは clone ごとの設定なので、開発環境を作った直後に 1 回だけ実行する
# （`scripts/bootstrap-toolchain.sh` からも呼ばれる）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

git config core.hooksPath .githooks
chmod +x .githooks/* 2>/dev/null || true

echo "ok: core.hooksPath = $(git config core.hooksPath)"
LS="$(git ls-files -s .githooks | awk '{print $1, $4}')"
echo "$LS"
