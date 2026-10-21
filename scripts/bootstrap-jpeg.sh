#!/usr/bin/env bash
# PNG/JPEG icon decoding: pinned upstream source for the offline Cargo directory source.
set -euo pipefail
BUILD_DIR="${BUILD_DIR:-/home/user/build}"
DEST="$BUILD_DIR/vendor-src/jpeg-decoder"
if [ -f "$DEST/.cargo-checksum.json" ]; then exit 0; fi
ARCHIVE="$BUILD_DIR/jpeg-decoder.tar.gz"
curl -fL --retry 3 https://codeload.github.com/image-rs/jpeg-decoder/tar.gz/c1a1fe04cc54a5446e57a71ea856afd07cd374b2 -o "$ARCHIVE"
echo "7a7e6b683f528e4a10ecd1cd4236085bdc2d2c3232beaf0db377efa4ffed04b8  $ARCHIVE" | sha256sum -c -
mkdir -p "$DEST"
tar xzf "$ARCHIVE" -C "$DEST" --strip-components=1
python3 - "$DEST" <<'PY'
import pathlib,sys,json,hashlib
root=pathlib.Path(sys.argv[1])
files={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*') if p.is_file() and p.name!='.cargo-checksum.json'}
(root/'.cargo-checksum.json').write_text(json.dumps({'files':files,'package':'f5d4a7da358eff58addd2877a45865158f0d78c911d43a5784ceb7bbf52833b0'}))
PY
