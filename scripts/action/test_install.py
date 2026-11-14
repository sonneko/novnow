#!/usr/bin/env python3
"""Offline regressions for the composite action; use macOS's Bash 3.2 explicitly.

公開後 smoke（`.github/workflows/action-smoke.yml`）が負例を作る手順も含む（#61 追加監査 7）。
"""
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
PREPARE = Path(__file__).with_name("prepare-invalid-project.sh").resolve()
REPOSITORY = Path(__file__).resolve().parents[2]
SAMPLE_PROJECT = REPOSITORY / "examples" / "sample-novel"
SMOKE_WORKFLOW = REPOSITORY / ".github" / "workflows" / "action-smoke.yml"
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
        manifest.write_text(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n", newline="\n")
        # Override only network I/O in the sourced helper. Git's bash launcher
        # may prepend its own tools to PATH, defeating a fake curl executable.
        env = dict(self.env, NOVNOW_VERSION="0.1.0", NOVNOW_TARGET=target,
                   INPUT_VERSION="0.1.0", INPUT_TARGET=target,
                   NOVNOW_CACHE_DIR=str(self.root / "cache"), FAKE_MANIFEST=str(manifest))
        command = '''
          source "$1" --resolve >/dev/null
          fetch() {
            [ -f "$2" ] && return 0
            case "$1" in
              */SHA256SUMS) cp "$FAKE_MANIFEST" "$2" ;;
              *) echo "offline fixture archive cache miss: $2" >&2; return 91 ;;
            esac
          }
          install_release
        '''
        result = subprocess.run([BASH, "-c", command, "bash", str(SCRIPT)], env=env,
                                capture_output=True, text=True, encoding="utf-8", timeout=10)
        self.assertEqual(result.returncode, 0, f"{BASH}: {result.stderr}\n{result.stdout}")
        self.assertEqual((cache / executable).read_bytes(), payload)
        self.assertIn("binary=", self.outputs.read_text())

    def test_backslash_path_cannot_escape_the_digest(self):
        # On Windows the directory already has backslashes; on Unix make one.
        file = self.root / ("payload.zip" if os.name == "nt" else "windows\\payload.zip")
        file.write_bytes(b"published release checksum regression\r\n\x1a\x00\xff")
        result = subprocess.run(
            [BASH, "-c", 'source "$1" --resolve >/dev/null; sha256 "$2"',
             "bash", str(SCRIPT), str(file)],
            env=dict(os.environ, INPUT_VERSION="0.1.0", INPUT_TARGET="x86_64-unknown-linux-gnu"),
            capture_output=True, text=True, encoding="utf-8", timeout=10,
        )
        self.assertEqual(result.returncode, 0, f"{BASH}: {result.stderr}\n{result.stdout}")
        self.assertEqual(result.stdout.strip(), hashlib.sha256(file.read_bytes()).hexdigest())


class PrepareInvalidProjectTests(unittest.TestCase):
    """公開後 smoke が「E 診断が出る作品」を作れること（#61 追加監査 7）。

    本文を `-name '*.txt'` で決め打ちしていたため、サンプルの本文が `.cnov` へ変わったあと
    3 OS とも準備の段階で失敗し、後段の「E 診断で exit code 5」が一度も走らなかった。
    """

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def prepare(self, source, target):
        return subprocess.run([BASH, str(PREPARE), str(source), str(target)],
                              capture_output=True, text=True, encoding="utf-8", timeout=60)

    def make_project(self, name, manuscripts):
        """`contents/` の中身だけを指定した最小の作品を作る。"""
        project = self.root / name
        (project / "contents").mkdir(parents=True)
        (project / "project.toml").write_text("[project]\n", encoding="utf-8", newline="\n")
        for manuscript in manuscripts:
            path = project / "contents" / manuscript
            path.write_text("@01-01-01 屋根裏\n本文。\n", encoding="utf-8", newline="\n")
        return project

    def test_the_sample_project_yields_an_injected_reference(self):
        before = {path.relative_to(SAMPLE_PROJECT).as_posix(): path.read_bytes()
                  for path in sorted(SAMPLE_PROJECT.rglob("*")) if path.is_file()}
        target = self.root / "novnow-invalid"
        result = self.prepare(SAMPLE_PROJECT, target)
        self.assertEqual(result.returncode, 0, f"{BASH}: {result.stderr}\n{result.stdout}")
        manuscript = Path(result.stdout.strip())
        self.assertTrue(manuscript.is_file(), result.stdout)
        self.assertEqual(manuscript.parent, target / "contents")
        self.assertIn("'action_unknown_term", manuscript.read_text(encoding="utf-8"))
        # 元の作品（追跡対象）は書き換えない
        after = {path.relative_to(SAMPLE_PROJECT).as_posix(): path.read_bytes()
                 for path in sorted(SAMPLE_PROJECT.rglob("*")) if path.is_file()}
        self.assertEqual(before, after)

    def test_the_manuscript_is_not_resolved_by_extension(self):
        """本文の拡張子が変わっても準備できる（決め打ちへ戻すとここで落ちる）。"""
        candidates = sorted(path.name for path in (SAMPLE_PROJECT / "contents").iterdir()
                            if path.is_file() and not path.name.startswith("."))
        self.assertTrue(candidates, "examples/sample-novel/contents に本文が無い")
        target = self.root / "resolved"
        result = self.prepare(SAMPLE_PROJECT, target)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(result.stdout.strip()).name, candidates[0])
        # サンプルと違う拡張子でも同じ手順で足りる
        for extension in (".txt", ".cnov", ".md"):
            project = self.make_project(f"project{extension.replace('.', '_')}", [f"01-01{extension}"])
            outcome = self.prepare(project, self.root / f"out{extension.replace('.', '_')}")
            self.assertEqual(outcome.returncode, 0, f"{extension}: {outcome.stderr}")
            self.assertEqual(Path(outcome.stdout.strip()).name, f"01-01{extension}")

    def test_a_project_without_a_manuscript_fails_loudly(self):
        """空の結果を曖昧なリダイレクトで先へ送らず、理由を出して止まる。"""
        empty = self.make_project("empty", [])
        (empty / "contents" / ".novnow-local").write_text("local state\n", encoding="utf-8", newline="\n")
        result = self.prepare(empty, self.root / "empty-out")
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("::error::", result.stderr)
        self.assertIn("本文ファイルが無い", result.stderr)

    def test_a_project_without_contents_is_rejected(self):
        project = self.root / "no-contents"
        project.mkdir()
        result = self.prepare(project, self.root / "no-contents-out")
        self.assertEqual(result.returncode, 1)
        self.assertIn("本文のディクトリが無い", result.stderr)

    def test_a_missing_source_or_argument_is_rejected(self):
        self.assertEqual(self.prepare(self.root / "absent", self.root / "out").returncode, 1)
        without_arguments = subprocess.run([BASH, str(PREPARE)], capture_output=True,
                                           text=True, encoding="utf-8", timeout=30)
        self.assertEqual(without_arguments.returncode, 2)
        self.assertIn("usage:", without_arguments.stderr)

    def test_the_smoke_workflow_delegates_to_the_shared_script(self):
        lines = SMOKE_WORKFLOW.read_text(encoding="utf-8").splitlines()
        # 経緯を説明するコメントを除いた**実行される部分**が、本文を拡張子で決め打っていないこと。
        # 決めうちへ戻すと空振りしても気付けなくなる（#61 追加監査 7 の再現）。
        code = "\n".join(line for line in lines if not line.lstrip().startswith("#"))
        self.assertIn("scripts/action/prepare-invalid-project.sh", code)
        self.assertNotIn("-name '*.", code)
        self.assertNotIn(".txt", code)
        # 負例の確認（E 診断で exit code 5）はこれまでどおり後段が担う
        self.assertIn('[ "$INVALID_CODE" = 5 ]', code)


if __name__ == "__main__":
    unittest.main()
