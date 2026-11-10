#!/usr/bin/env bash
# Build relocatable app output so the same bytes work at app/ and nightly-app/.
set -euo pipefail
trunk build --release --public-url './'
test -f dist/index.html
test -f dist/sw.js
! grep -q 'const PRECACHE = null' dist/sw.js
