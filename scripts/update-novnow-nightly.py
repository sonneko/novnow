#!/usr/bin/env python3
"""Install the official Linux x64 nightly VSIX/LSP pair from a my-novels checkout.

Requirements: Linux x86_64, Python >= 3.9, authenticated GitHub CLI (gh), VS Code
CLI (code). Run from the my-novels root; no project settings are overwritten.
Use --download-only to verify/download without installing or changing current.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import uuid
import zipfile

REPOSITORY = "sonneko/novnow"
BINARY_ASSET = "novnow-nightly-linux-x86_64-nogui.tar.gz"
VSIX_ASSET = "novnow-nightly.vsix"
ASSETS = ("SHA256SUMS", "manifest.json", BINARY_ASSET, VSIX_ASSET)


class UpdateError(Exception):
    pass


class GitHub:
    def get(self, endpoint, output=None):
        command = ["gh", "api", "--hostname", "github.com", f"repos/{REPOSITORY}/{endpoint}"]
        if output is not None:
            command += ["-H", "Accept: application/octet-stream"]
            with output.open("wb") as stream:
                subprocess.run(command, stdout=stream, check=True, timeout=180)
            return None
        return json.loads(subprocess.check_output(command, timeout=60))

    def release(self):
        return self.get("releases/tags/nightly")

    def download(self, asset_id, output):
        self.get(f"releases/assets/{asset_id}", output)


def selected_assets(release):
    if release.get("tag_name") != "nightly" or release.get("draft"):
        raise UpdateError("nightly is not a published release")
    selected = {}
    for asset in release.get("assets", []):
        if asset.get("name") in ASSETS:
            name = asset["name"]
            if name in selected or not isinstance(asset.get("id"), int):
                raise UpdateError(f"Invalid/duplicate release asset: {name}")
            selected[name] = asset
    if set(selected) != set(ASSETS):
        raise UpdateError("nightly is being published or is missing required assets")
    return selected


def fingerprint(release):
    selected = selected_assets(release)
    return (release["id"], tuple((name, selected[name]["id"], selected[name].get("size"),
                                selected[name].get("updated_at")) for name in ASSETS))


def verify_checksums(directory):
    hashes = {}
    for line in (directory / "SHA256SUMS").read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        match = re.fullmatch(r"([a-fA-F0-9]{64})  ([^/\\]+)", line)
        if not match or match[2] in hashes:
            raise UpdateError("Invalid or duplicate SHA256SUMS entry")
        hashes[match[2]] = match[1].lower()
    for name in ASSETS[1:]:
        digest = hashlib.sha256((directory / name).read_bytes()).hexdigest()
        if hashes.get(name) != digest:
            raise UpdateError(f"Checksum mismatch or missing entry: {name}")


def extract_binary(directory):
    """Never extract paths or links supplied by the archive into the filesystem."""
    with tarfile.open(directory / BINARY_ASSET, "r:gz") as archive:
        seen = set()
        binary = None
        for member in archive:
            name = member.name
            path = PurePosixPath(name)
            if (path.is_absolute() or ".." in path.parts or "\\" in name
                    or name in seen or not member.isfile()):
                raise UpdateError(f"Unsafe archive member: {name}")
            seen.add(name)
            if name == "novnow":
                binary = member
        if binary is None or not 0 < binary.size <= 200 * 1024 * 1024:
            raise UpdateError("Archive must contain one regular novnow executable")
        with archive.extractfile(binary) as source, (directory / "novnow").open("wb") as target:
            shutil.copyfileobj(source, target)
    (directory / "novnow").chmod(0o755)


def verify_vsix(directory):
    with zipfile.ZipFile(directory / VSIX_ASSET) as archive:
        names = archive.namelist()
        if names.count("extension/package.json") != 1:
            raise UpdateError("VSIX must contain a single extension/package.json")
        manifest = json.loads(archive.read("extension/package.json"))
    if manifest.get("publisher") != "sonneko" or manifest.get("name") != "novnow":
        raise UpdateError("Unexpected VSIX extension identity")
    version = manifest.get("version", "")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][\w.+-]+)?", version):
        raise UpdateError("Invalid VSIX version")
    return version


def fetch_bundle(directory, github):
    release = github.release()
    assets = selected_assets(release)
    # Asset IDs, not moving /download/nightly URLs, pin this download generation.
    for name in ASSETS:
        size = assets[name].get("size", 0)
        if not isinstance(size, int) or not 0 < size <= 250 * 1024 * 1024:
            raise UpdateError(f"Invalid asset size: {name}")
        github.download(assets[name]["id"], directory / name)
        if (directory / name).stat().st_size != size:
            raise UpdateError(f"Incomplete download: {name}")
    verify_checksums(directory)
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest.get("channel") != "nightly" or not re.fullmatch(r"[0-9a-f]{40}", manifest.get("sha", "")):
        raise UpdateError("Invalid nightly source manifest")
    if fingerprint(github.release()) != fingerprint(release):
        raise UpdateError("nightly changed during download; retrying the whole set")
    extract_binary(directory)
    verify_vsix(directory)
    (directory / "installed.json").write_text(json.dumps({
        "repository": REPOSITORY, "release_id": release["id"],
        "sha": manifest["sha"], "run": manifest.get("run"),
        "asset_ids": {name: assets[name]["id"] for name in ASSETS},
    }, indent=2) + "\n")
    return manifest["sha"]


def replace_link(link, target):
    temporary = link.with_name(f".link-{uuid.uuid4().hex}")
    try:
        temporary.symlink_to(target)
        os.replace(temporary, link)
    finally:
        temporary.unlink(missing_ok=True)


def activate(root, bundle, code_command, profile=None, run=subprocess.run):
    current = root / "current"
    previous = root / "previous"
    for link in (current, previous):
        if link.exists() and not link.is_symlink():
            raise UpdateError(f"Refusing to replace a non-symlink: {link}")
    old = os.readlink(current) if current.is_symlink() else None
    command = [code_command, "--install-extension", str(bundle / VSIX_ASSET), "--force"]
    if profile:
        command += ["--profile", profile]
    # If this fails, the old binary selection is untouched. VSCode installation
    # itself is not transactional; keep both downloaded VSIX files for recovery.
    run(command, check=True)
    if old is not None:
        replace_link(previous, old)
    replace_link(current, os.path.relpath(bundle, root))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install-dir", type=Path, default=Path(".tools/novnow"),
                        help="destination relative to cwd (default: .tools/novnow)")
    parser.add_argument("--code", default="code", help="VS Code CLI command/path, e.g. code-insiders")
    parser.add_argument("--profile", help="VS Code profile for VSIX installation")
    parser.add_argument("--download-only", action="store_true", help="verify/download only; do not install or switch current")
    args = parser.parse_args(argv)
    if platform.system() != "Linux" or platform.machine().lower() not in ("x86_64", "amd64"):
        raise UpdateError("This nightly binary is for Linux x86_64 only (including x64 WSL/containers).")
    if shutil.which("gh") is None:
        raise UpdateError("Install GitHub CLI (gh) and authenticate it first.")
    if not args.download_only and shutil.which(args.code) is None:
        raise UpdateError(f"VS Code CLI not found: {args.code}; use --code or --download-only")
    root = args.install_dir.resolve()
    root.mkdir(parents=True, exist_ok=True)
    with (root / ".update.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise UpdateError("Another nightly update is running for this directory.") from error
        for name in ("current", "previous"):
            link = root / name
            if link.exists() and not link.is_symlink():
                raise UpdateError(f"Refusing to replace a non-symlink: {link}")
        versions = root / "versions"
        versions.mkdir(exist_ok=True)
        for attempt in range(3):
            staging = Path(tempfile.mkdtemp(prefix=".download-", dir=versions))
            try:
                sha = fetch_bundle(staging, GitHub())
            except (UpdateError, subprocess.SubprocessError, OSError, ValueError,
                    tarfile.TarError, zipfile.BadZipFile) as error:
                shutil.rmtree(staging)
                if attempt == 2:
                    raise UpdateError(f"Download failed; current was not changed: {error}") from error
                print(f"Attempt {attempt + 1} failed: {error}; retrying in 5s", file=sys.stderr)
                time.sleep(5)
                continue
            bundle = versions / f"{sha[:12]}-{uuid.uuid4().hex[:8]}"
            staging.rename(bundle)
            break
        print(f"Verified source: {sha}\nDownloaded set: {bundle}", flush=True)
        if args.download_only:
            print("Download only: VSIX installation and current are unchanged.")
            return 0
        # Only execute after checksums and archive/VSIX validation.
        version = subprocess.check_output([str(bundle / "novnow"), "--version"], text=True, timeout=30).strip()
        expected = verify_vsix(bundle)
        if version != f"novnow {expected}":
            raise UpdateError(f"Binary/VSIX version mismatch: {version!r} vs {expected!r}; current unchanged")
        activate(root, bundle, args.code, args.profile)
        print(f"Updated: {version}\nCLI/LSP: {root / 'current/novnow'}")
        print("Set these VS Code workspace settings once (existing settings are not edited):")
        print(json.dumps({"novnow.executablePath": str(root / "current/novnow"), "novnow.autoDownload": False}, indent=2))
        print("Reload the VS Code window to load the updated extension and restart the LSP.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (UpdateError, subprocess.SubprocessError, OSError, ValueError, zipfile.BadZipFile) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        sys.exit(1)
