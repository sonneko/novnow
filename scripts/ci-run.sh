#!/usr/bin/env bash
# 必要なときだけ Actions を回すための入口。
#
# このリポジトリのコミットは既定で `[ci skip]` が付く（`.githooks/commit-msg`）ので、
# push しても CI は走らない。**CI を回したいときだけ**これを実行する。
#
#   bash scripts/ci-run.sh            # CI（ci.yml）を今のブランチで回す
#   bash scripts/ci-run.sh wheels     # Python ホイール（wheels.yml）
#   bash scripts/ci-run.sh ci watch   # 回して結果を待つ
#
# `workflow_dispatch` はブランチを指定して回せるので、push し直す必要はない
# （ci.yml は dispatch のとき `rust_changed=true` として全ジョブを走らせる）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

workflow="${1:-ci}"
action="${2:-}"

case "$workflow" in
    ci|wheels) ;;
    *) echo "使い方: $0 [ci|wheels] [watch]" >&2; exit 2 ;;
esac

branch="$(git rev-parse --abbrev-ref HEAD)"
if [ "$branch" = "HEAD" ]; then
    echo "エラー: detached HEAD では回せない（ブランチを指定する）" >&2
    exit 2
fi

# 未 push のコミットがあると、回るのはリモートの HEAD（古い方）になる。
if ! git rev-parse --verify --quiet "origin/$branch" >/dev/null; then
    echo "エラー: origin/$branch が無い。先に git push する" >&2
    exit 2
fi
if [ -n "$(git rev-list "origin/$branch..HEAD" 2>/dev/null)" ]; then
    echo "警告: 未 push のコミットがある。CI は origin/$branch の内容で走る" >&2
fi

echo "==> gh workflow run ${workflow}.yml --ref $branch"
gh workflow run "${workflow}.yml" --ref "$branch"

if [ "$action" = "watch" ]; then
    sleep 5
    run_id="$(gh run list --workflow "${workflow}.yml" --branch "$branch" --limit 1 --json databaseId --jq '.[0].databaseId')"
    echo "==> gh run watch $run_id"
    gh run watch "$run_id" --exit-status
else
    sleep 3
    gh run list --workflow "${workflow}.yml" --branch "$branch" --limit 3 \
        --json databaseId,status,conclusion,displayTitle \
        --template '{{range .}}{{.databaseId}}  {{.status}}  {{.conclusion}}  {{.displayTitle}}{{"\n"}}{{end}}'
fi
