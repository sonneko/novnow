#!/usr/bin/env bash
# 週次の依存監査で見つかった指摘を Issue にする（docs/developers.md §4.10）。
#
# 同じ週次監査の Issue が開いていれば**追記**し、無ければ**作る**。毎週新しい Issue を
# 増やさないため、検索はタイトル前方の「依存監査（週次）」と `dependencies` ラベルで行う。
#
# 使い方:
#   GH_TOKEN=... bash scripts/action/dependency-audit-issue.sh <本文ファイル> [タイトル]
#
# 本文ファイルが空（または無い）なら何もしない（＝指摘が無かった run で Issue を作らない）。
set -euo pipefail

body="${1:?本文ファイルのパスが必要（例: dependency-audit-issue.md）}"
title="${2:-依存監査（週次）: 対応が必要な指摘}"
search_prefix="依存監査（週次）"

if [ ! -s "$body" ]; then
  echo "本文が無いので Issue は作らない: $body"
  exit 0
fi

if [ -z "${GH_TOKEN:-}" ]; then
  echo "::error::GH_TOKEN が無い（Issue を作れない）" >&2
  exit 1
fi

# ラベルは Dependabot も使う名前。無いリポジトリでも動くように作る（既にあれば何もしない）。
gh label create dependencies --description "依存の更新と監査" --color "0366d6" >/dev/null 2>&1 || true

number=$(gh issue list --state open --label dependencies \
  --search "in:title ${search_prefix}" --json number --jq '.[0].number // empty')

if [ -n "$number" ]; then
  gh issue comment "$number" --body-file "$body"
  echo "既存の Issue #${number} へ追記した"
else
  gh issue create --title "$title" --label dependencies --body-file "$body"
  echo "Issue を作った: $title"
fi
