#!/usr/bin/env bash
# Release-only verification. This is intentionally not part of verify.sh.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
DIRECTORY="$ROOT/dist/release"
REMOTE=0
ARTIFACTS_ONLY=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --remote) REMOTE=1 ;;
    --artifacts-only) ARTIFACTS_ONLY=1 ;;
    --dir) shift; DIRECTORY="${1:?--dir requires a path}" ;;
    -h|--help)
      echo "usage: bash scripts/release-check.sh [--dir DIR] [--artifacts-only] [--remote]"
      exit 0
      ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$ROOT"
VERSION="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')"
python3 scripts/release-matrix.py --check
if [ "$ARTIFACTS_ONLY" = 1 ]; then
  python3 scripts/release-matrix.py --verify-release "$DIRECTORY" --artifacts-only
else
  python3 scripts/release-matrix.py --verify-release "$DIRECTORY"
fi

# All 10 desktop archives + five wheel files + one VSIX + full source archive are covered by the
# verifier above. Browser and Actions are source-distributed and therefore add
# no binary to SHA256SUMS; the full source archive is included separately.
if [ "$REMOTE" = 1 ]; then
  SHA="$(git rev-parse HEAD)"
  bash scripts/release-tags.sh --check --version "$VERSION" --sha "$SHA"
fi
printf 'OK: release %s is complete%s\n' "$VERSION" "$([ "$REMOTE" = 1 ] && printf ' and tags point to HEAD' || true)"
