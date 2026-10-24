#!/usr/bin/env python3
"""Seal a completed Trunk staging directory into one atomic offline application.

Trunk post_build supplies TRUNK_STAGING_DIR. The manual wasm smoke builder uses
--dir. Do not deploy a source (unsealed) sw.js: it intentionally refuses install.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
from urllib.parse import quote


def seal(directory: Path) -> None:
    worker = directory / "sw.js"
    source = worker.read_text(encoding="utf-8")
    if "const PRECACHE = null;" not in source:
        raise ValueError("expected an unsealed worker; rebuild before sealing")
    paths = sorted(directory.rglob("*"))
    if any(p.is_symlink() for p in paths):
        raise ValueError("symlinks in application are forbidden")
    assets = [p for p in paths if p.is_file() and p != worker]
    names = [p.relative_to(directory).as_posix() for p in assets]
    if "index.html" not in names or not any(n.endswith(".js") for n in names) or not any(n.endswith(".wasm") for n in names):
        raise ValueError("offline app requires index.html, JavaScript and WASM")
    digest = hashlib.sha256(source.encode("utf-8"))
    urls = ["./"]
    for path, name in zip(assets, names):
        if path.is_symlink():
            raise ValueError(f"symlink in application: {name}")
        digest.update(name.encode() + b"\0" + hashlib.sha256(path.read_bytes()).digest())
        urls.append("./" + quote(name, safe="/"))
    source = source.replace("const PRECACHE = null;", "const PRECACHE = " + json.dumps(urls) + ";")
    source = source.replace("const BUILD = 'unsealed';", f"const BUILD = '{digest.hexdigest()}';")
    worker.write_text(source, encoding="utf-8")
    print(f"PWA: sealed {len(urls)} URLs, build {digest.hexdigest()[:12]}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", type=Path, default=Path(os.environ.get("TRUNK_STAGING_DIR", "dist")))
    args = parser.parse_args()
    seal(args.dir)
