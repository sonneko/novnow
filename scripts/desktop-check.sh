#!/usr/bin/env bash
# Read-only release rehearsal. TARGET/GUI come from release-matrix.py.
set -euo pipefail
: "${TARGET:?TARGET is required}"
: "${GUI:?GUI is required}"
command=(cargo build --locked --release --target "$TARGET")
case "$GUI" in gui) command+=(--features ui);; nogui) command+=(--no-default-features);; *) exit 2;; esac
"${command[@]}"
binary="target/$TARGET/release/novnow"
if [[ "$TARGET" == *pc-windows-msvc ]]; then binary+=.exe; fi
artifact=$(python3 scripts/release-matrix.py --artifact --target "$TARGET" --gui "$GUI")
input=(--binary "$binary")
if [[ "$TARGET" == *apple-darwin && "$GUI" == gui ]]; then
  app="$RUNNER_TEMP/novnow.app"
  python3 scripts/release-matrix.py --bundle-macos --binary "$binary" --output "$app"
  input=(--app "$app")
fi
python3 scripts/release-matrix.py --package --target "$TARGET" --gui "$GUI" "${input[@]}" --output "release-check/$artifact"
python3 scripts/release-matrix.py --verify-archive "release-check/$artifact" --target "$TARGET" --gui "$GUI"
export ARTIFACT="release-check/$artifact"
python3 - <<'PY'
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

artifact = Path(os.environ['ARTIFACT'])
version = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    if artifact.suffix == '.zip':
        with zipfile.ZipFile(artifact) as archive:
            archive.extractall(root)
        binary = root / 'novnow.exe'
    else:
        with tarfile.open(artifact) as archive:
            archive.extractall(root, filter='data')
        binary = root / ('novnow.app/Contents/MacOS/novnow' if 'apple-darwin' in os.environ['TARGET'] and os.environ['GUI'] == 'gui' else 'novnow')
    result = subprocess.run([str(binary), '--version'], check=True, capture_output=True, text=True, encoding="utf-8", timeout=60)
    assert result.stdout.strip() == f'novnow {version}', result
    result = subprocess.run([str(binary), 'check', 'examples/sample-novel'], check=True, capture_output=True, text=True, encoding="utf-8", timeout=60)
    print(result.stdout)
    print(f'{artifact.name}: unpacked executable --version and sample check PASS')
PY
