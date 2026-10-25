#!/usr/bin/env python3
"""Offline regressions for the composite action; use macOS's Bash 3.2 explicitly."""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import shutil
import sys
import tempfile
import tarfile
import zipfile
import unittest

SCRIPT = Path(__file__).with_name("install.sh").resolve()
BASH = "/bin/bash" if sys.platform == "darwin" else "bash"
if sys.platform == "win32":
    # Do not accidentally launch Windows System32/WSL bash instead of Git Bash.
    git = shutil.which("git")
    if git:
        # Git Bash puts mingw64/bin/git.exe before cmd/git.exe on PATH.
        for parent in Path(git).resolve().parents:
            git_bash = parent / "bin/bash.exe"
            if git_bash.is_file():
                BASH = str(git_bash)
                break


class ActionShellTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.outputs = self.root / "outputs"
        self.capture = self.root / "arguments"
        self.binary = self.root / "fake-cli.sh"
        self.binary.write_text(
            '#!/usr/bin/env bash\n'
            'printf "%s\\n" "$@" >> "$CAPTURE_FILE"\n'
            'if [ "$1" = diagnostics ]; then printf \'{"count":1}\\n\'; exit 0; fi\n'
            'exit "${FAKE_EXIT_CODE:-0}"\n', encoding="utf-8", newline="\n"
        )
        self.binary.chmod(0o755)
        self.env = dict(os.environ, NOVNOW_BINARY=str(self.binary),
                        INPUT_PATH="a project with spaces", INPUT_COMMAND="check",
                        CAPTURE_FILE=str(self.capture), GITHUB_OUTPUT=str(self.outputs))
        self.env.pop("INPUT_ARGS", None)

    def run_action(self, **env):
        return subprocess.run([BASH, str(SCRIPT), "--run"],
                              env=dict(self.env, **env), capture_output=True,
                              text=True, encoding="utf-8", timeout=10)

    def test_no_extra_arguments_on_bash_32(self):
        result = self.run_action()
        self.assertEqual(result.returncode, 0, f"{BASH}: {result.stderr}\n{result.stdout}")
        self.assertEqual(self.capture.read_text().splitlines()[:2],
                         ["check", "a project with spaces"])
        self.assertIn("findings=1", self.outputs.read_text())

    def test_whitespace_only_arguments(self):
        result = self.run_action(INPUT_ARGS="   ")
        self.assertEqual(result.returncode, 0, f"{BASH}: {result.stderr}\n{result.stdout}")
        self.assertEqual(self.capture.read_text().splitlines()[:3],
                         ["check", "a project with spaces", "diagnostics"])

    def test_failure_exit_code_is_not_hidden(self):
        result = self.run_action(FAKE_EXIT_CODE="5")
        self.assertEqual(result.returncode, 5, f"{BASH}: {result.stderr}\n{result.stdout}")
        self.assertIn("exit-code=5", self.outputs.read_text())
        self.assertIn("findings=1", self.outputs.read_text())

    def test_export_path_is_preserved(self):
        result = self.run_action(INPUT_COMMAND="export", INPUT_ARGS="--out result.txt")
        self.assertEqual(result.returncode, 0, f"{BASH}: {result.stderr}\n{result.stdout}")
        self.assertIn("export-path=result.txt", self.outputs.read_text())
        self.assertEqual(self.capture.read_text().splitlines(),
                         ["export", "a project with spaces", "--out", "result.txt"])

    def test_install_from_authenticated_cached_archive(self):
        target = "x86_64-pc-windows-msvc" if os.name == "nt" else "x86_64-unknown-linux-gnu"
        executable = "novnow.exe" if os.name == "nt" else "novnow"
        extension = ".zip" if os.name == "nt" else ".tar.gz"
        cache = self.root / "cache" / "0.1.0" / target
        cache.mkdir(parents=True)
        archive = cache / f"novnow-0.1.0-{target}-nogui{extension}"
        payload = b"authenticated offline fixture"
        if os.name == "nt":
            with zipfile.ZipFile(archive, "w") as bundle:
                bundle.writestr(executable, payload)
        else:
            with tarfile.open(archive, "w:gz") as bundle:
                entry = tarfile.TarInfo(executable)
                entry.size = len(payload)
                bundle.addfile(entry, io.BytesIO(payload))
        manifest = self.root / "manifest"
        manifest.write_text(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n")
        tools = self.root / "bin"
        tools.mkdir()
        curl = tools / "curl"
        curl.write_text('#!/usr/bin/env bash\nfor arg in "$@"; do destination="$arg"; done\ncp "$FAKE_MANIFEST" "$destination"\n', newline="\n")
        curl.chmod(0o755)
        env = dict(self.env, NOVNOW_VERSION="0.1.0", NOVNOW_TARGET=target,
                   NOVNOW_CACHE_DIR=str(self.root / "cache"), FAKE_MANIFEST=str(manifest),
                   PATH=str(tools) + os.pathsep + self.env["PATH"])
        result = subprocess.run([BASH, str(SCRIPT), "--install"], env=env,
                                capture_output=True, text=True, encoding="utf-8", timeout=10)
        self.assertEqual(result.returncode, 0, f"{BASH}: {result.stderr}\n{result.stdout}")
        self.assertEqual((cache / executable).read_bytes(), payload)
        self.assertIn("binary=", self.outputs.read_text())

    def test_backslash_path_cannot_escape_the_digest(self):
        # On Windows the directory already has backslashes; on Unix make one.
        file = self.root / ("payload.zip" if os.name == "nt" else "windows\\payload.zip")
        file.write_bytes(b"published release checksum regression")
        result = subprocess.run(
            [BASH, "-c", 'source "$1" --resolve >/dev/null; sha256 "$2"',
             "bash", str(SCRIPT), str(file)],
            env=dict(os.environ, INPUT_VERSION="0.1.0", INPUT_TARGET="x86_64-unknown-linux-gnu"),
            capture_output=True, text=True, encoding="utf-8", timeout=10,
        )
        self.assertEqual(result.returncode, 0, f"{BASH}: {result.stderr}\n{result.stdout}")
        self.assertEqual(result.stdout.strip(), hashlib.sha256(file.read_bytes()).hexdigest())


if __name__ == "__main__":
    unittest.main()
