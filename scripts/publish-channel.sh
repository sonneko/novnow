#!/usr/bin/env bash
# Called only inside the shared Pages publication concurrency group.
# Uses a temporary index and commit-tree: never switches the source checkout.
set -euo pipefail
channel="$1"
payload="$(realpath "$2")"
sha="$3"
run="$4"
previous="$RUNNER_TEMP/previous-pages"
output="$RUNNER_TEMP/published-pages"
mkdir -p "$previous"
parent=()
if git ls-remote --exit-code --heads origin gh-pages > "$RUNNER_TEMP/pages-ref"; then
  git fetch origin refs/heads/gh-pages
  parent=(-p "$(git rev-parse FETCH_HEAD)")
  git archive FETCH_HEAD | tar -xf - -C "$previous"
else
  status=$?
  if [ "$status" != 2 ]; then exit "$status"; fi
fi
# Ignore delayed runs; use publication generation rather than source commit time.
if [ -f "$previous/$channel-build.json" ]; then
  old=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["run"])' "$previous/$channel-build.json")
  if [ "$old" -gt "$run" ]; then echo 'publish=false' >> "$GITHUB_OUTPUT"; exit 0; fi
fi
if [ "$channel" = nightly ]; then
  # Do not let a build queued before a stable reset undo that reset.
  if [ -f "$previous/stable-build.json" ]; then
    old=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["run"])' "$previous/stable-build.json")
    if [ "$old" -gt "$run" ]; then echo 'publish=false' >> "$GITHUB_OUTPUT"; exit 0; fi
  fi
  current=$(git ls-remote origin refs/heads/develop | cut -f1)
  if [ "$current" != "$sha" ]; then echo 'publish=false' >> "$GITHUB_OUTPUT"; exit 0; fi
  # First run bootstraps from a separately built official main snapshot, NOT develop.
  if [ ! -f "$previous/index.html" ]; then
    cp -a "$payload/bootstrap/." "$previous/"
  fi
  python3 - "$payload/release" "$sha" <<'PYVALIDATE'
import json, sys
from pathlib import Path
root = Path(sys.argv[1])
expected = {'novnow-nightly-linux-x86_64-nogui.tar.gz', 'novnow-nightly.vsix', 'novnow-nightly-browser.tar.gz', 'manifest.json', 'SHA256SUMS'}
assert {p.name for p in root.iterdir()} == expected, 'incomplete nightly payload'
assert json.loads((root / 'manifest.json').read_text())['sha'] == sys.argv[2], 'wrong source SHA'
PYVALIDATE
  (cd "$payload/release" && sha256sum --check SHA256SUMS)
  mkdir -p "$RUNNER_TEMP/nightly-browser"
  tar -xzf "$payload/release/novnow-nightly-browser.tar.gz" -C "$RUNNER_TEMP/nightly-browser"
  incoming="$RUNNER_TEMP/nightly-browser"
else
  current=$(git ls-remote origin refs/heads/main | cut -f1)
  if [ "$current" != "$sha" ]; then echo 'publish=false' >> "$GITHUB_OUTPUT"; exit 0; fi
  incoming="$payload/site"
fi
python3 scripts/channel-pages.py "$channel" --incoming "$incoming" --previous "$previous" \
  --output "$output" --sha "$sha" --run "$run"
if [ "$channel" = nightly ]; then
  # Build and validate everything before touching the sole mutable release.
  if gh release view nightly >/dev/null 2>&1; then
    gh release delete nightly --yes
  fi
  if git ls-remote --exit-code --tags origin refs/tags/nightly > /dev/null; then
    gh api "repos/$GITHUB_REPOSITORY/git/refs/tags/nightly" -X PATCH -f sha="$sha" -F force=true
  else
    status=$?
    if [ "$status" != 2 ]; then exit "$status"; fi
    gh api "repos/$GITHUB_REPOSITORY/git/refs" -f ref=refs/tags/nightly -f sha="$sha"
  fi
  gh release create nightly --verify-tag --prerelease --latest=false --title 'Nightly — develop (unstable)' \
    --notes "Source: $sha. Build: $GITHUB_SERVER_URL/$GITHUB_REPOSITORY/actions/runs/$run. VSIX/LSP version matching is the consumer's responsibility." \
    "$payload"/release/*
fi
export GIT_INDEX_FILE="$RUNNER_TEMP/pages-index"
rm -f "$GIT_INDEX_FILE"
git read-tree --empty
git --work-tree="$output" add --all --force
tree=$(git write-tree)
git config user.name github-actions[bot]
git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
commit=$(printf '%s\n' "Publish $channel from $sha (run $run)" | git commit-tree "$tree" "${parent[@]}")
# Fast-forward only. An unexpected external writer fails rather than losing data.
git push origin "$commit:refs/heads/gh-pages"
echo 'publish=true' >> "$GITHUB_OUTPUT"
