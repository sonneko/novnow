"""Approved distribution policies: license, Python spelling, publication route."""
import importlib.util
import hashlib
import re
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("release_matrix", ROOT / "scripts/release-matrix.py")
matrix = importlib.util.module_from_spec(spec)
spec.loader.exec_module(matrix)


class ReleasePolicy(unittest.TestCase):
    def test_python_equivalent_versions(self):
        for source, expected in [("0.1.0", "0.1.0"), ("0.1.0-alpha.0", "0.1.0a0"), ("0.1.0-beta.2", "0.1.0b2"), ("0.1.0-rc.12", "0.1.0rc12")]:
            self.assertEqual(matrix.python_version(source), expected)
            self.assertIn(f"novnow-{expected}-cp39-abi3-", matrix.wheel_name(source, matrix.TARGET_NAMES[0]))
        for invalid in ["0.1.0-preview.1", "0.1.0-rc.01", "0.1.0+local", "0.1.0-rc"]:
            with self.assertRaises(ValueError):
                matrix.python_version(invalid)

    def test_prerelease_metadata_uses_pep440(self):
        target = matrix.TARGET_NAMES[0]
        version = "0.1.0-rc.1"
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / matrix.wheel_name(version, target)
            def write(metadata_version):
                with zipfile.ZipFile(path, "w") as wheel:
                    prefix = "novnow-0.1.0rc1.dist-info/"
                    wheel.writestr(prefix + "METADATA", f"Name: novnow\nVersion: {metadata_version}\n")
                    wheel.writestr(prefix + "WHEEL", f"Tag: cp39-abi3-{matrix.target_row(target)['wheel_tag']}\n")
            write("0.1.0rc1")
            matrix.verify_wheel(path, version, target)
            write(version)
            with self.assertRaises(ValueError):
                matrix.verify_wheel(path, version, target)

    def test_develop_scope_keeps_single_lightest_platform(self):
        full_wheels = matrix.wheel_rows("0.1.0")
        develop_wheels = matrix.filter_develop_rows(full_wheels)
        self.assertEqual([row["target"] for row in full_wheels], list(matrix.TARGET_NAMES))
        self.assertEqual([row["target"] for row in develop_wheels], [matrix.DEVELOP_TARGET])
        full_desktop = matrix.desktop_rows("0.1.0")
        develop_desktop = matrix.filter_develop_rows(full_desktop, nogui_only=True)
        self.assertEqual(len(full_desktop), 10)
        self.assertEqual(len(develop_desktop), 1)
        self.assertEqual(develop_desktop[0]["target"], matrix.DEVELOP_TARGET)
        self.assertEqual(develop_desktop[0]["gui"], "nogui")
        self.assertEqual(matrix.python_versions_for_scope("full"), ["3.9", "3.11", "3.13"])
        self.assertEqual(matrix.python_versions_for_scope("develop"), ["3.9", "3.13"])
        with self.assertRaises(ValueError):
            matrix.python_versions_for_scope("release")

    def test_ci_workflows_share_one_scope_expression(self):
        scope = (
            "github.event_name == 'workflow_dispatch' && 'full' ||"
            " (github.event_name == 'pull_request' && (github.base_ref == 'main' && 'full' || 'develop'))"
            " || (github.ref_name == 'main' && 'full' || 'develop')"
        )
        normalized = " ".join(scope.split())
        for workflow in ("wheels.yml", "desktop-check.yml", "pages-check.yml", "vscode.yml"):
            text = (ROOT / ".github/workflows" / workflow).read_text()
            flat = " ".join(text.split())
            self.assertIn(normalized, flat, f"{workflow} の CI_SCOPE 判定が一致しない")

    def test_supplementary_workflow_cannot_publish_to_pypi(self):
        text = (ROOT / ".github/workflows/wheels.yml").read_text()
        self.assertNotIn("publish_pypi", text)
        self.assertNotIn("publish-pypi:", text)
        self.assertIn("repository-url: https://test.pypi.org/legacy/", text)
        self.assertIn("needs: test", text)
        self.assertIn('"$REF_NAME" != "v$version"', text)
        self.assertIn("matching prerelease version tag", text)
        release = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertLess(release.index("Integrated Release supports stable versions only"), release.index("\n  desktop:\n"))

    def test_font_license_is_embedded_and_archived(self):
        license_path = ROOT / "licenses/IPA_Font_License_Agreement_v1.0.txt"
        text = license_path.read_text(encoding="utf-8-sig")
        self.assertIn("IPA Font License Agreement v1.0", text)
        self.assertIn("Article 5", text)
        provenance = (ROOT / "licenses/README.txt").read_text()
        checksum = re.search(r"Font SHA-256: ([0-9a-f]{64})", provenance)[1]
        self.assertEqual(hashlib.sha256((ROOT / "assets/ipag.ttf").read_bytes()).hexdigest(), checksum)
        self.assertIn("IPA_FONT_LICENSE", (ROOT / "src/ui/app.rs").read_text())
        entries = matrix.archive_entries(Path("fake-binary"), None, "0.1.0", matrix.TARGET_NAMES[0], "gui")
        self.assertIn(("licenses/IPA_Font_License_Agreement_v1.0.txt", license_path, False), entries)
