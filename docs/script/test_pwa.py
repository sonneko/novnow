"""Offline sealing must be complete, build-specific, and reproducible."""
import importlib.util
import tempfile
import unittest
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("pwa_assets", ROOT / "scripts/pwa-assets.py")
pwa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pwa)


class OfflineSeal(unittest.TestCase):
    def test_trunk_and_smoke_builder_both_seal_the_output(self):
        hooks = tomllib.loads((ROOT / "Trunk.toml").read_text())["hooks"]
        self.assertIn({"stage": "post_build", "command": "python3", "command_arguments": ["scripts/pwa-assets.py"]}, hooks)
        self.assertIn('scripts/pwa-assets.py" --dir "$DIST"', (ROOT / "scripts/web-smoke.sh").read_text())

    def test_complete_asset_list_and_build_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            original = (ROOT / "sw.js").read_text()
            (path / "sw.js").write_text(original)
            (path / "index.html").write_text("app")
            with self.assertRaises(ValueError):
                pwa.seal(path)
            (path / "app-123.js").write_text("module")
            (path / "app-456.wasm").write_bytes(b"wasm")
            (path / "guide").mkdir()
            (path / "guide/a b.html").write_text("guide")
            pwa.seal(path)
            first = (path / "sw.js").read_text()
            self.assertIn('"./app-456.wasm"', first)
            self.assertIn('"./guide/a%20b.html"', first)
            self.assertNotIn("const PRECACHE = null", first)
            self.assertNotIn("const BUILD = 'unsealed'", first)
            with self.assertRaises(ValueError):
                pwa.seal(path)
            (path / "sw.js").write_text(original)
            pwa.seal(path)
            self.assertEqual((path / "sw.js").read_text(), first)
            (path / "app-456.wasm").write_bytes(b"new build")
            (path / "sw.js").write_text(original)
            pwa.seal(path)
            self.assertNotEqual((path / "sw.js").read_text(), first)
            (path / "app-456.wasm").write_bytes(b"wasm")
            (path / "sw.js").write_text(original + "\n// worker-only update\n")
            pwa.seal(path)
            self.assertNotEqual((path / "sw.js").read_text().split("const PRECACHE")[0], first.split("const PRECACHE")[0])
