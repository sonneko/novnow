#!/usr/bin/env bash
# CI の結果を読む（GitHub Actions のログ**本体**は作業環境から取得できないため）。
#
# なぜこれが要るか（PLANS.md §9.3、DEVELOPERS.md §1.5）:
#   Actions のログは objects.githubusercontent.com / results-receiver.actions.githubusercontent.com
#   にあり、作業環境からは**到達できない**。`gh run view --log` はすべて失敗する。
#   読めるのは次の 3 つだけ:
#     (1) run / job / step の**状態**（`gh run view --json`、api.github.com 経由）
#     (2) check-run の**注釈**（`::error file=…,line=…::`。コンパイルエラーの 1 行目が入る）
#     (3) `.github/report.py` が**失敗時に PR へ投稿するコメント**（これが一番詳しい）
#   なので、失敗の中身を読むには (3) を取るのが正解。
#
# 使い方:
#   bash scripts/ci-report.sh                     # 今のブランチの最新 run の状態
#   bash scripts/ci-report.sh --branch main       # ブランチを指定
#   bash scripts/ci-report.sh --run 38187540789   # run を指定（失敗ステップと注釈を出す）
#   bash scripts/ci-report.sh --pr 10             # PR の CI 状態＋最新のレポートコメント
#   bash scripts/ci-report.sh --watch             # 最新 run が完了するまで待つ（60 秒おき）
#
# 注意: もちろん **push 前に scripts/verify.sh を通していれば、CI は緑の証跡でしかない**。
#       このスクリプトは「赤くなった理由を読む」ためのもので、日常は verify.sh を使う。
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
cd "$ROOT"

BRANCH=""; RUN=""; PR=""; WATCH=0
while [ $# -gt 0 ]; do
    case "$1" in
        --branch) BRANCH="${2:-}"; shift 2 ;;
        --run)    RUN="${2:-}"; shift 2 ;;
        --pr)     PR="${2:-}"; shift 2 ;;
        --watch)  WATCH=1; shift ;;
        -h|--help) sed -n '2,26p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) printf 'error: 未知の引数 %s\n' "$1" >&2; exit 2 ;;
    esac
done

command -v gh >/dev/null 2>&1 || { echo "error: gh が無い" >&2; exit 2; }
REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || true)"
[ -n "$REPO" ] || REPO="$(git remote get-url origin 2>/dev/null | sed -E 's#.*github\.com[:/]##; s#\.git$##')"
[ -n "$BRANCH" ] || BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo main)"

strip_ansi() { python3 -c 'import sys,re; sys.stdout.write(re.sub(r"\x1b\[[0-9;]*m","",sys.stdin.read()))'; }

head_() { printf '\n\033[1;34m==>\033[0m %s\n' "$*"; }

# --- run / job / step の状態 -------------------------------------------------
show_run() {  # show_run <run id>
    gh run view "$1" --json databaseId,workflowName,headSha,status,conclusion,event,jobs \
        --jq '"run \(.databaseId)  \(.workflowName)  \(.headSha[0:8])  \(.event)  \(.status)  \(.conclusion // "-")",
              (.jobs[] | "  job: \(.name)  \(.status)  \(.conclusion // "-")",
                        (.steps[] | select(.conclusion != "success" and .conclusion != "skipped")
                                | "      step \(.number) \(.name): \(.conclusion)  [id \(.databaseId // 0)]"))'
}

if [ -n "$RUN" ]; then
    head_ "run $RUN"
    show_run "$RUN"
    # 失敗ステップの注釈（コンパイルエラーの 1 行目など）
    for job in $(gh run view "$RUN" --json jobs --jq '.jobs[] | select(.conclusion=="failure") | .databaseId' 2>/dev/null); do
        head_ "job $job の注釈（check-run annotations）"
        gh api "repos/$REPO/check-runs/$job/annotations" \
            --jq '.[] | "  [\(.annotation_level)] \(.path // "-"):\(.start_line // "-")  \(.message)"' 2>/dev/null \
            | strip_ansi || echo "  （注釈を取得できなかった）"
    done
    exit 0
fi

if [ -n "$PR" ]; then
    head_ "PR #$PR のチェック"
    gh pr view "$PR" --json state,mergeable,headRefName,headRefOid \
        --jq '"  \(.headRefName) @ \(.headRefOid[0:8])  state=\(.state)  mergeable=\(.mergeable)"' 2>/dev/null \
        || echo "  （PR の情報を取得できなかった。gh pr view は Projects(classic) の GraphQL エラーで失敗することがあるので、次も試す）"
    gh api "repos/$REPO/commits/$(gh pr view "$PR" --json headRefOid -q .headRefOid 2>/dev/null)/check-runs" \
        --jq '.check_runs[] | "  \(.name): \(.status) \(.conclusion // "-")"' 2>/dev/null | head -10

    head_ "PR #$PR のレポートコメント（.github/report.py が失敗時に投稿したもの）"
    # 失敗が無ければ投稿されないので「見つからない」で正常
    gh api "repos/$REPO/issues/$PR/comments" --paginate \
        --jq '.[] | select(.body | test("失敗|FAIL|error\\[|レポート")) | "--- \(.created_at) (\(.id)) ---\n\(.body)"' 2>/dev/null \
        | strip_ansi | tail -120 || echo "  （コメントを取得できなかった）"
    exit 0
fi

head_ "ブランチ $BRANCH の run（新しい順）"
gh run list --branch "$BRANCH" --limit 6 \
    --json databaseId,headSha,workflowName,status,conclusion,event,displayTitle \
    --jq '.[] | "  \(.databaseId)  \(.headSha[0:8])  \(.workflowName)  \(.event)  \(.status)  \(.conclusion // "-")"' 2>/dev/null \
    || { echo "  （run を取得できなかった）"; exit 1; }

latest="$(gh run list --branch "$BRANCH" --limit 1 --json databaseId -q '.[0].databaseId' 2>/dev/null || true)"
if [ -n "$latest" ] && [ "$WATCH" = 1 ]; then
    while :; do
        state="$(gh run view "$latest" --json status,conclusion --jq '.status + " " + (.conclusion // "-")' 2>/dev/null || echo "unknown -")"
        printf '    run %s: %s\n' "$latest" "$state"
        case "$state" in completed*) break ;; esac
        sleep 60
    done
fi
[ -n "$latest" ] && show_run "$latest"

cat <<'MSG'

次の一歩:
  * 全部 success なら何もしなくてよい（push 前に scripts/verify.sh を回していれば当然の結果）。
  * 失敗ステップがあるなら、上の注釈と **PR のレポートコメント**を読む:
        bash scripts/ci-report.sh --pr <PR 番号>
  * ログ本体（gh run view --log）は**この環境からは取れない**。取ろうとしないこと。
MSG
