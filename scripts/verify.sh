#!/usr/bin/env bash
# ドキュメント検査（duplicate.py）と CI ジョブ（fmt / clippy / test / wasm）を手元で回す。
#
# 背景（docs/developers.md §2・§4）:
#   2026-10-11 以降、作業環境に CI と同一のツールチェーン（rustc 1.99.0）があるので、
#   **CI を待たずに全ジョブをローカルで確認できる**。CI は「緑の証跡」として残す。
#   Actions の使用時間を節約するためにも、push 前は必ずこれで確認すること。
#
# 使い方:
#   bash scripts/verify.sh              # fmt / clippy / test / wasm（既定）
#   bash scripts/verify.sh --quick      # fmt / clippy / test（wasm を省く。wasm は 15〜40 秒かかる）
#   bash scripts/verify.sh --ui         # M7 から既定で GUI 込み（`--features ui`）なので何もしない（互換のため残す）
#   bash scripts/verify.sh --report     # 失敗時に .github/report.py の出力も表示（CI と同じ読み物）
#   bash scripts/verify.sh --fix        # cargo fmt --all を先に実行する（差分を書く）
#
# 環境変数:
#   BUILD_DIR  ツールチェーンの場所（既定 $HOME/build。scripts/bootstrap-toolchain.sh が作る）
#   LOG_DIR    ログの置き場（既定 /tmp/novnow-verify）
#
# 終了コード: すべて緑なら 0、1 つでも赤なら 1。
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$HOME/build}"
LOG_DIR="${LOG_DIR:-/tmp/novnow-verify}"

QUICK=0; UI=0; REPORT=0; FIX=0; WASM=1
for arg in "$@"; do
    case "$arg" in
        --quick)   QUICK=1 ;;
        --ui)      UI=1 ;;
        --report)  REPORT=1 ;;
        --fix)     FIX=1 ;;
        --no-wasm) WASM=0 ;;
        -h|--help) sed -n '2,26p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) printf 'error: 未知の引数 %s（--help を見る）\n' "$arg" >&2; exit 2 ;;
    esac
done
[ "$QUICK" = 1 ] && WASM=0

cd "$ROOT"
mkdir -p "$LOG_DIR"

# --- ツールチェーン ---------------------------------------------------------
if ! command -v cargo >/dev/null 2>&1; then
    if [ -f "$BUILD_DIR/env.sh" ]; then
        # shellcheck disable=SC1091
        . "$BUILD_DIR/env.sh"
    else
        cat >&2 <<MSG
error: cargo が無い。作業環境には Rust ツールチェーンが入っていないので、先に次を実行すること。

    bash scripts/bootstrap-toolchain.sh        # 376MB / 約 40 秒（GitHub への到達が必要）

（docs/developers.md §2.2）
MSG
        exit 2
    fi
fi
command -v cargo >/dev/null 2>&1 || { echo "error: cargo が依然として見つからない" >&2; exit 2; }

green() { printf '\033[1;32m%s\033[0m\n' "$*"; }
red()   { printf '\033[1;31m%s\033[0m\n' "$*"; }
head_() { printf '\n\033[1;34m==>\033[0m %s\n' "$*"; }

names=(); statuses=(); times=()
run() {  # run <名前> <コマンド...>
    local name="$1"; shift
    head_ "$name"
    local started ended status
    started=$(date +%s)
    "$@" >"$LOG_DIR/$name.log" 2>&1
    status=$?
    ended=$(date +%s)
    if [ "$status" = 0 ]; then
        green "ok   $name（$((ended - started)) 秒、ログ $LOG_DIR/$name.log）"
    else
        red   "FAIL $name（exit $status、ログ $LOG_DIR/$name.log）"
        tail -40 "$LOG_DIR/$name.log"
        if [ "$REPORT" = 1 ] && [ -f "$ROOT/.github/report.py" ]; then
            head_ "$name — .github/report.py の出力（CI に出るものと同じ）"
            CI_LOG="$LOG_DIR/$name.log" CI_JOB="$name" CI_KIND="$name" CI_STATUS=failure \
            CI_SHA="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)" \
                python3 "$ROOT/.github/report.py" "$LOG_DIR/$name.md" 2>/dev/null \
                && { cat "$LOG_DIR/$name.md"; } \
                || echo "（レポートの生成に失敗。生ログは $LOG_DIR/$name.log）"
        fi
    fi
    names+=("$name"); statuses+=("$status"); times+=("$((ended - started))")
}

if [ "$FIX" = 1 ]; then
    head_ "cargo fmt --all（差分を書く）"
    cargo fmt --all
fi

# GUI 込み（`--features ui`）と GUI を含まない既定のビルドの双方を clippy とテストで確かめる。
: "$UI"
run docs      python3 "$ROOT/docs/script/duplicate.py"
run fmt       cargo fmt --all --check
run clippy    cargo clippy --locked --all-targets -- -D warnings
run clippy-ui cargo clippy --locked --all-targets --features ui -- -D warnings
run test      cargo test --locked --all-targets --no-fail-fast --features ui
run test-cli  cargo test --locked --test cli --no-fail-fast
if [ "$WASM" = 1 ]; then
    run wasm      cargo check --locked --target wasm32-unknown-unknown --features ui
fi

# --- テスト件数（CI の実行サマリと同じ見せ方） -------------------------------
if [ -f "$LOG_DIR/test.log" ]; then
    passed=$(grep -E '^test result: ok\.' "$LOG_DIR/test.log" | awk -F'[ ;]' '{s+=$4} END {print s+0}')
    failed=$(grep -E '^test result:' "$LOG_DIR/test.log" | awk -F'[ ;]' '{s+=$6} END {print s+0}')
    targets=$(grep -cE '^test result:' "$LOG_DIR/test.log")
    head_ "テスト"
    echo "    $passed passed / $failed failed（$targets ターゲット）"
fi

head_ "まとめ"
exit_code=0
for i in "${!names[@]}"; do
    if [ "${statuses[$i]}" = 0 ]; then
        printf '    \033[1;32mok  \033[0m %-10s %s 秒\n' "${names[$i]}" "${times[$i]}"
    else
        printf '    \033[1;31mFAIL\033[0m %-10s %s 秒（%s）\n' "${names[$i]}" "${times[$i]}" "$LOG_DIR/${names[$i]}.log"
        exit_code=1
    fi
done
if [ "$exit_code" = 0 ]; then
    green "すべて緑。コミットしてよい（push も忘れないこと＝ユーザー指示 2026-10-11）。"
else
    red "赤があるので push しないこと。ログは $LOG_DIR/"
fi
exit "$exit_code"
