#!/usr/bin/env python3
"""Assemble a complete Pages tree without allowing nightly to replace stable files."""
import argparse
import json
import shutil
from pathlib import Path


def copy_tree(source, destination):
    if not (source / "index.html").is_file():
        raise ValueError(f"missing index.html: {source}")
    if any(p.is_symlink() for p in source.rglob("*")):
        raise ValueError("symlinks are not allowed in Pages artifacts")
    shutil.copytree(source, destination, dirs_exist_ok=True)


def assemble(channel, incoming, previous, output, sha, run, incoming_webui=None):
    if output.exists():
        raise ValueError("output must be a fresh directory")
    if channel == "stable":
        copy_tree(incoming, output)
        if (output / "nightly-app").exists():
            shutil.rmtree(output / "nightly-app")
        copy_tree(output / "app", output / "nightly-app")
        if (output / "webui" / "index.html").is_file():
            if (output / "nightly-webui").exists():
                shutil.rmtree(output / "nightly-webui")
            copy_tree(output / "webui", output / "nightly-webui")
    else:
        if not (previous / "app/index.html").is_file():
            raise ValueError("initialize gh-pages with the stable site before nightly publication")
        copy_tree(previous, output)
        if (output / "nightly-app").exists():
            shutil.rmtree(output / "nightly-app")
        copy_tree(incoming, output / "nightly-app")
        if incoming_webui is not None:
            if (output / "nightly-webui").exists():
                shutil.rmtree(output / "nightly-webui")
            copy_tree(incoming_webui, output / "nightly-webui")
    (output / ".nojekyll").touch()
    # Separate markers: nightly never changes the last stable publication identity.
    (output / f"{channel}-build.json").write_text(
        json.dumps({"sha": sha, "run": int(run)}, indent=2) + "\n"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("channel", choices=["stable", "nightly"])
    for name in ("incoming", "previous", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--incoming-webui", type=Path, default=None)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--run", required=True)
    args = parser.parse_args()
    assemble(args.channel, args.incoming, args.previous, args.output, args.sha, args.run, args.incoming_webui)
