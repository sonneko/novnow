#!/usr/bin/env python3
"""Offline regressions for the composite action; use macOS's Bash 3.2 explicitly."""
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("install.sh").resolve()
BASH = "/bin/bash" if sys.platform == "darwin" else "bash"


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
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.capture.read_text().splitlines()[:2],
                         ["check", "a project with spaces"])
        self.assertIn("findings=1", self.outputs.read_text())

    def test_whitespace_only_arguments(self):
        result = self.run_action(INPUT_ARGS="   ")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.capture.read_text().splitlines()[:3],
                         ["check", "a project with spaces", "diagnostics"])

    def test_failure_exit_code_is_not_hidden(self):
        result = self.run_action(FAKE_EXIT_CODE="5")
        self.assertEqual(result.returncode, 5, result.stderr)
        self.assertIn("exit-code=5", self.outputs.read_text())
        self.assertIn("findings=1", self.outputs.read_text())

    def test_export_path_is_preserved(self):
        result = self.run_action(INPUT_COMMAND="export", INPUT_ARGS="--out result.txt")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("export-path=result.txt", self.outputs.read_text())
        self.assertEqual(self.capture.read_text().splitlines(),
                         ["export", "a project with spaces", "--out", "result.txt"])

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
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), hashlib.sha256(file.read_bytes()).hexdigest())


if __name__ == "__main__":
    unittest.main()
