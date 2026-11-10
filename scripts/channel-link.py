#!/usr/bin/env python3
"""Add only an unstable-app link when bootstrapping an older official site."""
import sys
from pathlib import Path

for page in Path(sys.argv[1]).rglob("*.html"):
    if "app" in page.relative_to(sys.argv[1]).parts:
        continue
    text = page.read_text()
    if "/novnow/nightly-app/" not in text:
        text = text.replace('</body>', '<p><a href="/novnow/nightly-app/">Nightly (unstable) / 開発版（不安定）</a></p></body>')
        page.write_text(text)
