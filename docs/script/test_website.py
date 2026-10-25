"""Regression coverage for the build-output website auditor."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "website_check", Path(__file__).resolve().parents[2] / "scripts/website-check.py")
website = importlib.util.module_from_spec(spec)
spec.loader.exec_module(website)


class WebsiteTests(unittest.TestCase):
    def test_parser_checks_ids_images_and_language_switch(self):
        page = website.Page('<div aria-label="Language"><a href="/ja/">JA</a></div>'
                            '<img src="a.png"><p id="x"></p><p id="x"></p>')
        self.assertEqual(page.language_links, ["/ja/"])
        self.assertEqual(page.duplicates, {"x"})
        self.assertEqual(page.images_without_alt, 1)

    def test_missing_anchor_and_asset_are_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "index.html").write_text('<a href="#missing">x</a><img alt="" src="lost.png">')
            count, errors = website.audit(root)
            self.assertEqual(count, 1)
            self.assertTrue(any("missing anchor" in error for error in errors))
            self.assertTrue(any("missing target" in error for error in errors))

    def test_app_exception_is_narrow_and_only_for_website_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            url = "https://sonneko.github.io/novnow/"
            self.assertIsNone(website.resolve(root, url, "/novnow/app/", True)[2])
            self.assertIsNotNone(website.resolve(root, url, "/novnow/app/", False)[2])
            self.assertIsNotNone(website.resolve(root, url, "/novnow/app/lost.js", True)[2])

    def test_outside_base_is_rejected_and_external_urls_are_not_fetched(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            url = "https://sonneko.github.io/novnow/"
            self.assertIsNotNone(website.resolve(root, url, "/ja/", False)[2])
            self.assertEqual(website.resolve(root, url, "https://example.com/x", False), (None, "", None))

    def test_language_switch_must_keep_article(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for language in ("ja", "en"):
                folder = root / language / "manual" / "start"
                folder.mkdir(parents=True)
                (folder / "index.html").write_text('<div aria-label="Language"></div>')
            self.assertEqual(sum("language switch" in e for e in website.audit(root)[1]), 2)
