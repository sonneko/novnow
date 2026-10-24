#!/usr/bin/env bash
# Publish or verify the immutable version tag and the moving v0 action tag.
set -euo pipefail

MODE="${1:-}"
VERSION=""
SHA=""
REPOSITORY="${GITHUB_REPOSITORY:-sonneko/novnow}"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --publish|--check) MODE="$1" ;;
    --version) shift; VERSION="${1:-}" ;;
    --sha) shift; SHA="${1:-}" ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done
case "$MODE" in --publish|--check) ;; *) echo "usage: release-tags.sh --publish|--check --version X.Y.Z --sha COMMIT" >&2; exit 2 ;; esac
if ! printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
  echo "invalid version: $VERSION" >&2
  exit 2
fi
if ! printf '%s' "$SHA" | grep -Eq '^[0-9a-f]{40}$'; then
  echo "a full commit SHA is required" >&2
  exit 2
fi
command -v gh >/dev/null 2>&1 || { echo "gh is required" >&2; exit 2; }

commit_for_tag() {
  local tag="$1"
  gh api "repos/$REPOSITORY/commits/$tag" --jq '.sha' 2>/dev/null
}

tag_exists() {
  gh api "repos/$REPOSITORY/git/ref/tags/$1" >/dev/null 2>&1
}

fixed="v$VERSION"
if ! tag_exists "$fixed"; then
  if [ "$MODE" = --check ]; then
    echo "missing tag: $fixed" >&2
    exit 1
  fi
  gh api "repos/$REPOSITORY/git/refs" --method POST -f "ref=refs/tags/$fixed" -f "sha=$SHA" >/dev/null
fi
actual="$(commit_for_tag "$fixed")"
if [ "$actual" != "$SHA" ]; then
  echo "$fixed points to $actual, expected $SHA; an immutable tag is never moved" >&2
  exit 1
fi
printf 'ok: %s -> %s\n' "$fixed" "$SHA"

# Release candidates have their own fixed tags but do not move the stable
# major-channel tag. The current action's stable channel is v0 (§20.5.4).
case "$VERSION" in
  *-*) printf 'ok: prerelease does not move v0\n'; exit 0 ;;
esac
major="v${VERSION%%.*}"
if [ "$MODE" = --publish ]; then
  if tag_exists "$major"; then
    gh api "repos/$REPOSITORY/git/refs/tags/$major" --method PATCH -f "sha=$SHA" -F force=true >/dev/null
  else
    gh api "repos/$REPOSITORY/git/refs" --method POST -f "ref=refs/tags/$major" -f "sha=$SHA" >/dev/null
  fi
fi
actual="$(commit_for_tag "$major")"
if [ "$actual" != "$SHA" ]; then
  echo "$major points to $actual, expected $SHA" >&2
  exit 1
fi
printf 'ok: %s -> %s\n' "$major" "$SHA"
