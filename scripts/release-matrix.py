#!/usr/bin/env python3
"""Single source of truth for novnow's release matrices and artifact names.

The script deliberately uses only Python's standard library so release jobs and
local verification need no bootstrap step.  It also owns version propagation,
desktop archive construction, SHA-256 manifests, and release-directory checks.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import stat
import sys
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]

# This is the only target table.  Workflows, tests, VSCode artifact names, and
# release verification all derive their values from these five rows.
TARGETS = (
    {
        "target": "x86_64-unknown-linux-gnu",
        "os": "ubuntu-latest",
        "wheel_tag": "manylinux_2_28_x86_64",
        "python_arch": "x64",
    },
    {
        "target": "aarch64-unknown-linux-gnu",
        "os": "ubuntu-24.04-arm",
        "wheel_tag": "manylinux_2_28_aarch64",
        "python_arch": "arm64",
    },
    {
        "target": "x86_64-apple-darwin",
        "os": "macos-latest",
        "wheel_tag": "macosx_11_0_x86_64",
        "python_arch": "x64",
    },
    {
        "target": "aarch64-apple-darwin",
        "os": "macos-latest",
        "wheel_tag": "macosx_11_0_arm64",
        "python_arch": "arm64",
    },
    {
        "target": "x86_64-pc-windows-msvc",
        "os": "windows-latest",
        "wheel_tag": "win_amd64",
        "python_arch": "x64",
    },
)
TARGET_NAMES = tuple(row["target"] for row in TARGETS)
VERSION_RE = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)


def version() -> str:
    """Read the sole human-maintained version from the root Cargo manifest."""
    with (ROOT / "Cargo.toml").open("rb") as source:
        manifest = tomllib.load(source)
    value = manifest["package"]["version"]
    validate_version(value)
    return value


def validate_version(value: str) -> str:
    if not VERSION_RE.fullmatch(value):
        raise ValueError(f"not a semantic version: {value!r}")
    return value


def target_row(target: str) -> dict[str, str]:
    for row in TARGETS:
        if row["target"] == target:
            return dict(row)
    raise ValueError(f"unsupported target: {target}")


def normalize_gui(gui: str | bool) -> str:
    if gui is True or gui == "gui":
        return "gui"
    if gui is False or gui == "nogui":
        return "nogui"
    raise ValueError("gui must be 'gui' or 'nogui'")


def artifact_name(release_version: str, target: str, gui: str | bool) -> str:
    """The sole implementation of the desktop artifact naming rule."""
    validate_version(release_version)
    target_row(target)
    mode = normalize_gui(gui)
    extension = ".zip" if target.endswith("pc-windows-msvc") else ".tar.gz"
    return f"novnow-{release_version}-{target}-{mode}{extension}"


def python_version(release_version: str) -> str:
    """PEP 440 spelling of the same logical product version (not a new release)."""
    validate_version(release_version)
    match = re.fullmatch(r"(\d+\.\d+\.\d+)(?:-(alpha|beta|rc)\.(0|[1-9]\d*))?", release_version)
    if not match:
        raise ValueError("Python distribution supports stable or alpha.N/beta.N/rc.N versions only (no build metadata)")
    base, stage, number = match.groups()
    return base if stage is None else base + {"alpha": "a", "beta": "b", "rc": "rc"}[stage] + number


def wheel_name(release_version: str, target: str) -> str:
    validate_version(release_version)
    row = target_row(target)
    return f"novnow-{python_version(release_version)}-cp39-abi3-{row['wheel_tag']}.whl"


def vsix_name(release_version: str) -> str:
    return f"novnow-{validate_version(release_version)}.vsix"


def desktop_rows(release_version: str) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for target in TARGETS:
        for mode in ("gui", "nogui"):
            rows.append(
                {
                    **target,
                    "version": release_version,
                    "gui": mode,
                    "features": "ui" if mode == "gui" else "",
                    "artifact": artifact_name(release_version, target["target"], mode),
                }
            )
    return rows


def wheel_rows(release_version: str) -> list[dict[str, object]]:
    return [
        {
            **target,
            "version": release_version,
            "artifact": wheel_name(release_version, target["target"]),
            "compatibility": (
                "manylinux_2_28" if "linux" in target["target"] else "auto"
            ),
        }
        for target in TARGETS
    ]


def matrix(rows: list[dict[str, object]]) -> str:
    return json.dumps({"include": rows}, ensure_ascii=False, separators=(",", ":"))


def replace_manifest_version(text: str, new_version: str) -> str:
    """Replace [package].version without reformatting the rest of a manifest."""
    lines = text.splitlines(keepends=True)
    in_package = False
    replaced = False
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_package = stripped == "[package]"
            continue
        if in_package and line.startswith("version = "):
            newline = "\r\n" if line.endswith("\r\n") else "\n" if line.endswith("\n") else ""
            lines[index] = f'version = "{new_version}"{newline}'
            replaced = True
            break
    if not replaced:
        raise ValueError("[package].version was not found")
    return "".join(lines)


def replace_lock_versions(text: str, new_version: str, names: set[str]) -> str:
    pattern = re.compile(
        r'(?ms)^(\[\[package\]\]\nname = "([^"]+)"\nversion = ")[^"]+(")'
    )

    def replace(match: re.Match[str]) -> str:
        if match.group(2) in names:
            return match.group(1) + new_version + match.group(3)
        return match.group(0)

    return pattern.sub(replace, text)


def replace_required(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE)
    if count != 1:
        raise ValueError(f"version marker was not found: {label}")
    return updated


def versioned_files(new_version: str) -> dict[Path, str]:
    """Return every derived file as it should look for ``new_version``."""
    files: dict[Path, str] = {}

    root_manifest = ROOT / "Cargo.toml"
    files[root_manifest] = replace_manifest_version(
        root_manifest.read_text(encoding="utf-8"), new_version
    )
    root_lock = ROOT / "Cargo.lock"
    files[root_lock] = replace_lock_versions(
        root_lock.read_text(encoding="utf-8"), new_version, {"novnow"}
    )

    py_manifest = ROOT / "python/novnow-py/Cargo.toml"
    files[py_manifest] = replace_manifest_version(
        py_manifest.read_text(encoding="utf-8"), new_version
    )
    py_lock = ROOT / "python/novnow-py/Cargo.lock"
    files[py_lock] = replace_lock_versions(
        py_lock.read_text(encoding="utf-8"), new_version, {"novnow", "novnow-py"}
    )

    package = ROOT / "vscode/package.json"
    if package.exists():
        package_data = json.loads(package.read_text(encoding="utf-8"))
        package_data["version"] = new_version
        package_data.setdefault("novnow", {})["artifact"] = {
            target: artifact_name(new_version, target, "nogui")
            for target in TARGET_NAMES
        }
        files[package] = json.dumps(package_data, ensure_ascii=False, indent=2) + "\n"

    package_lock = ROOT / "vscode/package-lock.json"
    if package_lock.exists():
        lock_data = json.loads(package_lock.read_text(encoding="utf-8"))
        lock_data["version"] = new_version
        if "" in lock_data.get("packages", {}):
            lock_data["packages"][""]["version"] = new_version
        files[package_lock] = json.dumps(lock_data, ensure_ascii=False, indent=2) + "\n"

    action = ROOT / "action.yml"
    if action.exists():
        files[action] = replace_required(
            action.read_text(encoding="utf-8"),
            r'NOVNOW_ACTION_VERSION:\s*"[^"]+"',
            f'NOVNOW_ACTION_VERSION: "{new_version}"',
            "action.yml NOVNOW_ACTION_VERSION",
        )

    index = ROOT / "index.html"
    files[index] = replace_required(
        index.read_text(encoding="utf-8"),
        r'<meta name="novnow-version" content="[^"]+">',
        f'<meta name="novnow-version" content="{new_version}">',
        "index.html novnow-version",
    )
    service_worker = ROOT / "sw.js"
    files[service_worker] = replace_required(
        service_worker.read_text(encoding="utf-8"),
        r"const CACHE = 'novnow-offline-[^']+';",
        f"const CACHE = 'novnow-offline-{new_version}';",
        "sw.js cache version",
    )
    return files


def sync_versions(new_version: str | None, check: bool) -> int:
    selected = validate_version(new_version or version())
    files = versioned_files(selected)
    different = [
        path
        for path, content in files.items()
        if not path.exists() or path.read_text(encoding="utf-8") != content
    ]
    if check:
        if different:
            for path in different:
                print(f"out of sync: {path.relative_to(ROOT)}", file=sys.stderr)
            print("run: python3 scripts/release-matrix.py --bump", file=sys.stderr)
            return 1
        print(f"OK: all distribution versions and artifact names are {selected}")
        return 0
    for path in different:
        path.write_text(files[path], encoding="utf-8")
        print(f"updated {path.relative_to(ROOT)}")
    print(f"OK: distribution version {selected}")
    return 0


def readme_text(release_version: str, target: str, gui: str) -> bytes:
    return (
        f"novnow {release_version} ({target}, {gui})\n"
        "Usage: novnow --help; novnow <project-directory> (GUI builds only).\n"
        "All resources are embedded. See LICENSE for the MIT license.\n"
    ).encode("utf-8")


def bundle_macos(binary: Path, app: Path, release_version: str) -> None:
    """Create a minimal novnow.app without copying any external resources."""
    if app.exists():
        raise ValueError(f"refusing to overwrite existing app bundle: {app}")
    macos = app / "Contents/MacOS"
    macos.mkdir(parents=True)
    executable = macos / "novnow"
    shutil.copyfile(binary, executable)
    executable.chmod(0o755)
    info = {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleExecutable": "novnow",
        "CFBundleIdentifier": "com.sonneko.novnow",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": "novnow",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": release_version,
        "CFBundleVersion": release_version,
        "LSMinimumSystemVersion": "11.0",
        "NSHighResolutionCapable": True,
    }
    with (app / "Contents/Info.plist").open("wb") as destination:
        plistlib.dump(info, destination, sort_keys=True)


def tar_info(name: str, data: bytes, executable: bool = False) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mtime = 0
    info.mode = 0o755 if executable else 0o644
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    return info


def zip_info(name: str, executable: bool = False) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = ((0o755 if executable else 0o644) | stat.S_IFREG) << 16
    return info


def archive_entries(
    binary: Path | None,
    app: Path | None,
    release_version: str,
    target: str,
    gui: str,
) -> list[tuple[str, Path | bytes, bool]]:
    """Build a closed list of files permitted in a desktop archive."""
    entries: list[tuple[str, Path | bytes, bool]] = []
    if target.endswith("apple-darwin") and gui == "gui":
        if app is None:
            raise ValueError("macOS GUI packaging requires --app or --binary")
        if app is not None:
            for path in sorted(app.rglob("*")):
                if path.is_file():
                    name = f"{app.name}/{path.relative_to(app).as_posix()}"
                    entries.append((name, path, path.parent.name == "MacOS"))
    else:
        if binary is None:
            raise ValueError("desktop packaging requires --binary")
        name = "novnow.exe" if target.endswith("pc-windows-msvc") else "novnow"
        entries.append((name, binary, not name.endswith(".exe")))
    entries.append(("LICENSE", ROOT / "LICENSE", False))
    entries.append(("licenses/IPA_Font_License_Agreement_v1.0.txt", ROOT / "licenses/IPA_Font_License_Agreement_v1.0.txt", False))
    entries.append(("licenses/README.txt", ROOT / "licenses/README.txt", False))
    entries.append(("README.txt", readme_text(release_version, target, gui), False))
    return entries


def package_desktop(
    binary: Path | None,
    app: Path | None,
    output: Path,
    release_version: str,
    target: str,
    gui: str,
) -> None:
    expected = artifact_name(release_version, target, gui)
    if output.name != expected:
        raise ValueError(f"output must be named {expected}")
    entries = archive_entries(binary, app, release_version, target, gui)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        prefix=output.name + ".", dir=output.parent, delete=False
    ) as temporary:
        temporary_path = Path(temporary.name)
    try:
        if target.endswith("pc-windows-msvc"):
            with zipfile.ZipFile(temporary_path, "w") as archive:
                for name, source, executable in entries:
                    data = source if isinstance(source, bytes) else source.read_bytes()
                    archive.writestr(zip_info(name, executable), data)
        else:
            with temporary_path.open("wb") as raw:
                with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as gz:
                    with tarfile.open(fileobj=gz, mode="w", format=tarfile.PAX_FORMAT) as archive:
                        for name, source, executable in entries:
                            data = source if isinstance(source, bytes) else source.read_bytes()
                            archive.addfile(tar_info(name, data, executable), io.BytesIO(data))
        temporary_path.replace(output)
    finally:
        temporary_path.unlink(missing_ok=True)
    verify_archive(output, release_version, target, gui)


def archive_names(path: Path) -> list[str]:
    if path.name.endswith(".zip"):
        with zipfile.ZipFile(path) as archive:
            return archive.namelist()
    with tarfile.open(path, "r:gz") as archive:
        return archive.getnames()


def verify_archive(path: Path, release_version: str, target: str, gui: str) -> None:
    if path.name != artifact_name(release_version, target, gui):
        raise ValueError(f"unexpected archive name: {path.name}")
    names = archive_names(path)
    if any(name.startswith(("/", "../")) or "/../" in name for name in names):
        raise ValueError(f"unsafe path in archive: {path}")
    files = {name.rstrip("/") for name in names}
    if "LICENSE" not in files or "README.txt" not in files:
        raise ValueError(f"LICENSE or README.txt is missing: {path}")
    if "licenses/IPA_Font_License_Agreement_v1.0.txt" not in files:
        raise ValueError(f"IPA Font license is missing: {path}")
    if target.endswith("apple-darwin") and gui == "gui":
        required = "novnow.app/Contents/MacOS/novnow"
        if required not in files or "novnow.app/Contents/Info.plist" not in files:
            raise ValueError(f"incomplete macOS app bundle: {path}")
    else:
        required = "novnow.exe" if target.endswith("pc-windows-msvc") else "novnow"
        if required not in files:
            raise ValueError(f"executable is missing: {path}")
    forbidden = {"assets/ipag.ttf", "ipag.ttf", "examples/sample-novel"}
    if any(any(name == item or name.startswith(item + "/") for item in forbidden) for name in files):
        raise ValueError(f"external bundled resources are forbidden: {path}")


def expected_artifacts(release_version: str, include_vsix: bool) -> list[str]:
    names = [str(row["artifact"]) for row in desktop_rows(release_version)]
    names += [str(row["artifact"]) for row in wheel_rows(release_version)]
    if include_vsix:
        names.append(vsix_name(release_version))
        names.append(f"novnow-{release_version}-source.tar.gz")
    return names


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_checksums(directory: Path, signature_note: str | None) -> None:
    version_files = sorted(
        path
        for path in directory.iterdir()
        if path.is_file()
        and path.name != "SHA256SUMS"
        and (
            path.name.endswith((".tar.gz", ".zip", ".whl", ".vsix"))
        )
    )
    if not version_files:
        raise ValueError(f"no release artifacts in {directory}")
    lines: list[str] = []
    if signature_note:
        lines.extend(f"# {line}" for line in signature_note.splitlines())
    lines.extend(f"{sha256(path)}  {path.name}" for path in version_files)
    temporary = directory / ".SHA256SUMS.tmp"
    temporary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    temporary.replace(directory / "SHA256SUMS")
    print(f"wrote {directory / 'SHA256SUMS'} ({len(version_files)} artifacts)")


def verify_wheel(path: Path, release_version: str, target: str) -> None:
    if path.name != wheel_name(release_version, target):
        raise ValueError(f"unexpected wheel filename: {path.name}")
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        metadata_name = next(
            (name for name in names if re.fullmatch(r"novnow-[^/]+\.dist-info/METADATA", name)),
            None,
        )
        wheel_name_in_archive = next(
            (name for name in names if re.fullmatch(r"novnow-[^/]+\.dist-info/WHEEL", name)),
            None,
        )
        if not metadata_name or not wheel_name_in_archive:
            raise ValueError(f"wheel metadata is missing: {path.name}")
        metadata = archive.read(metadata_name).decode("utf-8")
        wheel = archive.read(wheel_name_in_archive).decode("utf-8")
    if not re.search(r"^Name: novnow$", metadata, re.MULTILINE):
        raise ValueError(f"wheel package name is not novnow: {path.name}")
    if not re.search(rf"^Version: {re.escape(python_version(release_version))}$", metadata, re.MULTILINE):
        raise ValueError(f"wheel version differs from Cargo.toml: {path.name}")
    expected_tag = f"cp39-abi3-{target_row(target)['wheel_tag']}"
    if not re.search(rf"^Tag: {re.escape(expected_tag)}$", wheel, re.MULTILINE):
        raise ValueError(f"wheel tag must be {expected_tag}: {path.name}")


def verify_vsix(path: Path, release_version: str) -> None:
    with zipfile.ZipFile(path) as archive:
        try:
            manifest = json.loads(archive.read("extension/package.json"))
        except KeyError as error:
            raise ValueError(f"VSIX package.json is missing: {path.name}") from error
    if manifest.get("publisher") != "sonneko" or manifest.get("name") != "novnow":
        raise ValueError(f"VSIX identifier must be sonneko.novnow: {path.name}")
    if manifest.get("version") != release_version:
        raise ValueError(f"VSIX version differs from Cargo.toml: {path.name}")
    if any(re.search(r"(?:^|/)novnow(?:\.exe)?$", name) for name in archive.namelist()):
        raise ValueError(f"VSIX must not bundle the novnow executable: {path.name}")


def verify_source(path: Path, release_version: str) -> None:
    prefix = f"novnow-{release_version}/"
    with tarfile.open(path, "r:gz") as archive:
        names = set(archive.getnames())
        required = {prefix + name for name in (
            "Cargo.toml", "Cargo.lock", "README.md", "LICENSE",
            "docs/developers.md", "docs/specifications/external.md", "src/lib.rs",
        )}
        if not required <= names:
            raise ValueError("source archive must include source, lockfile, license and documentation")
        if any((name != prefix.rstrip("/") and not name.startswith(prefix)) or ".." in Path(name).parts
               or ".git" in Path(name).parts for name in names):
            raise ValueError("source archive contains an unsafe or non-source path")
        manifest_file = archive.extractfile(prefix + "Cargo.toml")
        if manifest_file is None or tomllib.loads(manifest_file.read().decode())["package"]["version"] != release_version:
            raise ValueError("source archive version differs from release")


def verify_release(directory: Path, release_version: str, artifacts_only: bool) -> None:
    include_vsix = not artifacts_only
    expected = set(expected_artifacts(release_version, include_vsix))
    missing = sorted(name for name in expected if not (directory / name).is_file())
    if missing:
        raise ValueError("missing release artifacts: " + ", ".join(missing))
    checksum_file = directory / "SHA256SUMS"
    if not checksum_file.is_file():
        raise ValueError("SHA256SUMS is missing")
    rows: dict[str, str] = {}
    for line in checksum_file.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        if not match:
            raise ValueError(f"invalid SHA256SUMS line: {line!r}")
        if match.group(2) in rows:
            raise ValueError(f"duplicate SHA256SUMS entry: {match.group(2)}")
        rows[match.group(2)] = match.group(1)
    if set(rows) != expected:
        raise ValueError(
            "SHA256SUMS entries differ from artifacts: "
            f"missing={sorted(expected - set(rows))}, extra={sorted(set(rows) - expected)}"
        )
    for name, expected_hash in rows.items():
        actual = sha256(directory / name)
        if actual != expected_hash:
            raise ValueError(f"SHA-256 mismatch: {name}")
    for row in desktop_rows(release_version):
        verify_archive(
            directory / str(row["artifact"]), release_version, str(row["target"]), str(row["gui"])
        )
    for row in wheel_rows(release_version):
        verify_wheel(
            directory / str(row["artifact"]), release_version, str(row["target"])
        )
    if include_vsix:
        verify_vsix(directory / vsix_name(release_version), release_version)
        verify_source(directory / f"novnow-{release_version}-source.tar.gz", release_version)
    print(
        f"OK: {len(expected)} artifacts, SHA256SUMS, versions, and desktop archive contents"
    )


def release_status(directory: Path | None, release_version: str) -> str:
    """Report local release readiness without claiming that a publish happened."""
    forms: list[dict[str, object]] = [
        {
            "id": "browser",
            "choices": 1,
            "status": "published",
            "url": "https://sonneko.github.io/novnow/app/",
        }
    ]
    groups = (
        ("desktop", 10, expected_artifacts(release_version, False)[:10]),
        ("python", 5, expected_artifacts(release_version, False)[10:]),
        ("vscode", 1, [vsix_name(release_version)]),
    )
    for ident, choices, names in groups:
        ready = directory is not None and all((directory / name).is_file() for name in names)
        forms.append(
            {
                "id": ident,
                "choices": choices,
                "status": "ready" if ready else "not-provided",
            }
        )
    forms.append(
        {
            "id": "actions",
            "choices": 1,
            "status": "ready" if (ROOT / "action.yml").is_file() else "not-provided",
        }
    )
    return json.dumps(
        {"version": release_version, "forms": forms},
        ensure_ascii=False,
        indent=2,
    )


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    actions = result.add_mutually_exclusive_group(required=True)
    actions.add_argument("--desktop", action="store_true", help="print the 10-row desktop matrix")
    actions.add_argument("--wheels", action="store_true", help="print the 5-row wheel matrix")
    actions.add_argument("--artifact", action="store_true", help="print one desktop artifact name")
    actions.add_argument("--bump", action="store_true", help="propagate Cargo.toml's version (or an optional new version)")
    actions.add_argument("--check", action="store_true", help="check version propagation without writing")
    actions.add_argument("--status", action="store_true", help="report local release readiness as JSON")
    actions.add_argument("--package", action="store_true", help="build and verify one desktop archive")
    actions.add_argument("--bundle-macos", action="store_true", help="build a bare novnow.app")
    actions.add_argument("--verify-archive", metavar="PATH", help="verify one desktop archive")
    actions.add_argument("--verify-wheel", metavar="PATH", help="verify one wheel's filename and metadata")
    actions.add_argument("--checksums", metavar="DIR", help="write SHA256SUMS for artifacts in DIR")
    actions.add_argument("--verify-release", metavar="DIR", help="verify a complete release directory")
    result.add_argument("new_version", nargs="?", help="optional version used with --bump")
    result.add_argument("--release-version", help="version override (default: root Cargo.toml)")
    result.add_argument("--target", choices=TARGET_NAMES)
    result.add_argument("--gui", choices=("gui", "nogui"))
    result.add_argument("--binary", type=Path)
    result.add_argument("--app", type=Path)
    result.add_argument("--output", type=Path)
    result.add_argument("--release-dir", type=Path)
    result.add_argument("--signature-note")
    result.add_argument("--artifacts-only", action="store_true", help="verify 10 desktop + 5 wheels, before VSCode")
    return result


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        selected = validate_version(args.release_version or version())
        if args.desktop:
            print(matrix(desktop_rows(selected)))
        elif args.wheels:
            print(matrix(wheel_rows(selected)))
        elif args.artifact:
            if not args.target or not args.gui:
                raise ValueError("--artifact requires --target and --gui")
            print(artifact_name(selected, args.target, args.gui))
        elif args.bump:
            return sync_versions(args.new_version, check=False)
        elif args.check:
            if args.new_version:
                raise ValueError("--check does not take a new version")
            return sync_versions(None, check=True)
        elif args.status:
            print(release_status(args.release_dir, selected))
        elif args.bundle_macos:
            if not args.binary or not args.output:
                raise ValueError("--bundle-macos requires --binary and --output")
            bundle_macos(args.binary, args.output, selected)
        elif args.package:
            if not args.target or not args.gui or not args.output:
                raise ValueError("--package requires --target, --gui, and --output")
            if not args.binary and not args.app:
                raise ValueError("--package requires --binary or --app")
            package_desktop(
                args.binary,
                args.app,
                args.output,
                selected,
                args.target,
                normalize_gui(args.gui),
            )
        elif args.verify_archive:
            if not args.target or not args.gui:
                raise ValueError("--verify-archive requires --target and --gui")
            verify_archive(Path(args.verify_archive), selected, args.target, normalize_gui(args.gui))
            print(f"OK: {args.verify_archive}")
        elif args.verify_wheel:
            wheel_path = Path(args.verify_wheel)
            target = args.target
            if not target:
                target = next(
                    (
                        str(row["target"])
                        for row in wheel_rows(selected)
                        if row["artifact"] == wheel_path.name
                    ),
                    None,
                )
            if not target:
                raise ValueError("--verify-wheel needs --target for a nonstandard filename")
            verify_wheel(wheel_path, selected, target)
            print(f"OK: {wheel_path}")
        elif args.checksums:
            write_checksums(Path(args.checksums), args.signature_note)
        elif args.verify_release:
            verify_release(Path(args.verify_release), selected, args.artifacts_only)
        else:  # pragma: no cover - argparse's required group prevents this
            raise ValueError("no action")
    except (OSError, ValueError, KeyError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
