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

    def test_header_must_show_the_published_version(self):
        """版更新 PR の途中（Cargo.toml だけ 0.1.1 / 公開は 0.1.0）でも合格する。

        見出しは「公開済みの版」を出す（`website/src/data/release.ts`。未公開の版を公開済みとして
        案内しない）。検査は `Cargo.toml` ではなくそちらと比べる。
        """
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for language in ("ja", "en"):
                folder = root / language / "manual" / "start"
                folder.mkdir(parents=True)
                (folder / "index.html").write_text(
                    '<header>novnow v0.1.0</header>'
                    '<aside class="release-notice">notice</aside>'
                    f'<div aria-label="Language"><a href="/novnow/ja/manual/start/">JA</a>'
                    f'<a href="/novnow/en/manual/start/">EN</a></div>'
                )
            self.assertTrue(website.published_version().startswith("0."))
            self.assertGreaterEqual(website.version_key(website.DEV_VERSION),
                                    website.version_key(website.published_version()))
            errors = website.audit(root, version="0.1.0")[1]
            self.assertFalse([e for e in errors if "header version" in e], errors)
            errors = website.audit(root, version="0.1.1")[1]
            self.assertEqual(sum("header version" in e for e in errors), 2, errors)

    def test_site_may_not_claim_a_version_ahead_of_development(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "index.html").write_text("<p>ok</p>")
            errors = website.audit(root, version="99.0.0")[1]
            self.assertTrue(any("より新しい" in e for e in errors), errors)

    def test_language_switch_must_keep_article(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for language in ("ja", "en"):
                folder = root / language / "manual" / "start"
                folder.mkdir(parents=True)
                (folder / "index.html").write_text('<div aria-label="Language"></div>')
            self.assertEqual(sum("language switch" in e for e in website.audit(root)[1]), 2)


class SearchIndexTests(unittest.TestCase):
    """サイト内検索の索引（仕様 §18.5.1）: 対象漏れ・言語別・外部へ送らない。"""

    def index(self, language, urls, **extra):
        return {"base": "/novnow", "language": language,
                "pages": [{"url": url, "lang": language, "title": "題", "headings": [], "text": "本文"}
                          for url in urls], **extra}

    def build(self, root, *, languages=("ja", "en"), search_pages=True, index_urls=None):
        """最小の公開サイトの木（各言語 1 ページ＋検索のページ＋索引）。"""
        import json
        urls = index_urls or {}
        for language in languages:
            folder = root / language / "manual" / "start"
            folder.mkdir(parents=True, exist_ok=True)
            (folder / "index.html").write_text(
                f'<html lang="{language}"><head><title>t</title></head><body><main>x</main></body></html>',
                encoding="utf-8")
            if search_pages:
                search = root / language / "search"
                search.mkdir(parents=True, exist_ok=True)
                (search / "index.html").write_text(
                    f'<html lang="{language}"><body><main>'
                    '<input data-search-input>'
                    f'<div data-search-root data-index-url="/novnow/search-index/{language}.json">'
                    '<p data-search-status></p><ul data-search-results></ul>'
                    '<script type="module">fetch("/novnow/search-index/ja.json",{credentials:"omit"})</script>'
                    '</main></body></html>', encoding="utf-8")
        (root / "index.html").write_text('<html><body>redirect</body></html>', encoding="utf-8")
        directory = root / "search-index"
        directory.mkdir(exist_ok=True)
        for language in languages:
            (directory / f"{language}.json").write_text(
                json.dumps(urls.get(language, self.index(language, [f"/novnow/{language}/manual/start/"]))),
                encoding="utf-8")

    def audit(self, root):
        pages = {path: website.Page(path.read_text(encoding="utf-8")) for path in root.rglob("*.html")}
        return website.audit_search_index(root, pages)

    def test_a_complete_index_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.build(root)
            self.assertEqual(self.audit(root), [])

    def test_a_fixture_without_site_pages_is_skipped(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "index.html").write_text("<p>ok</p>", encoding="utf-8")
            self.assertEqual(self.audit(root), [])

    def test_a_published_page_missing_from_the_index_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.build(root, index_urls={"ja": self.index("ja", []), "en": self.index("en", ["/novnow/en/manual/start/"])})
            errors = self.audit(root)
            self.assertEqual(len(errors), 1, errors)
            self.assertIn("ja/manual/start/index.html", errors[0])
            self.assertIn("検索できない", errors[0])

    def test_a_stale_index_entry_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.build(root, index_urls={
                "ja": self.index("ja", ["/novnow/ja/manual/start/", "/novnow/ja/gone/"]),
                "en": self.index("en", ["/novnow/en/manual/start/"])})
            errors = self.audit(root)
            self.assertTrue(any("索引が古い" in error for error in errors), errors)

    def test_a_missing_index_file_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.build(root)
            (root / "search-index" / "ja.json").unlink()
            errors = self.audit(root)
            self.assertTrue(any("索引が無い" in error for error in errors), errors)

    def test_mixing_languages_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            mixed = self.index("ja", ["/novnow/ja/manual/start/"])
            mixed["pages"][0]["lang"] = "en"
            self.build(root, index_urls={"ja": mixed, "en": self.index("en", ["/novnow/en/manual/start/"])})
            errors = self.audit(root)
            self.assertTrue(any("混ぜない" in error for error in errors), errors)

    def test_an_external_index_url_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.build(root)
            page = root / "ja" / "search" / "index.html"
            page.write_text(page.read_text(encoding="utf-8").replace(
                'data-index-url="/novnow/search-index/ja.json"',
                'data-index-url="https://search.example.com/novnow/ja"'), encoding="utf-8")
            errors = self.audit(root)
            self.assertTrue(any("同じ公開元からだけ読む" in error for error in errors), errors)

    def test_an_external_script_or_fetch_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.build(root)
            page = root / "ja" / "search" / "index.html"
            page.write_text(page.read_text(encoding="utf-8").replace(
                '<script type="module">fetch("/novnow/search-index/ja.json",{credentials:"omit"})</script>',
                '<script type="module">fetch("https://search.example.com/?q="+q)</script>'),
                encoding="utf-8")
            errors = self.audit(root)
            self.assertTrue(any("外部 URL がある" in error for error in errors), errors)

    def test_a_form_on_the_search_page_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.build(root)
            page = root / "en" / "search" / "index.html"
            page.write_text(page.read_text(encoding="utf-8").replace(
                "<input data-search-input>", '<form action="/novnow/en/search/"><input data-search-input></form>'),
                encoding="utf-8")
            errors = self.audit(root)
            self.assertTrue(any("form がある" in error for error in errors), errors)

    def test_missing_keyboard_hooks_are_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.build(root)
            page = root / "ja" / "search" / "index.html"
            page.write_text(page.read_text(encoding="utf-8").replace("data-search-results", "data-list"),
                            encoding="utf-8")
            errors = self.audit(root)
            self.assertTrue(any("data-search-results" in error for error in errors), errors)

    def test_the_app_tree_is_not_indexed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.build(root)
            app = root / "app"
            app.mkdir()
            (app / "index.html").write_text('<html lang="ja"><body><canvas></canvas></body></html>', encoding="utf-8")
            (root / "nightly-app").mkdir()
            (root / "nightly-app" / "index.html").write_text('<html lang="ja"><body></body></html>', encoding="utf-8")
            self.assertEqual(self.audit(root), [])

    def test_page_language_comes_from_the_html_tag(self):
        self.assertEqual(website.Page('<html lang="ja-JP"><body>x</body></html>').language, "ja")
        self.assertEqual(website.Page("<html><body>x</body></html>").language, "")

    def test_the_repository_index_covers_the_built_site(self):
        dist = website.ROOT / "website" / "dist"
        if not (dist / "search-index").is_dir():
            self.skipTest("website/dist がまだ無い（npm run check:site --prefix website を先に回す）")
        pages = {path: website.Page(path.read_text(encoding="utf-8")) for path in dist.rglob("*.html")}
        self.assertEqual(website.audit_search_index(dist, pages), [])


class PublishedClaimTests(unittest.TestCase):
    """生成物の中の「書き写された数・版・コード表記」（#61 追加監査 8・9・10）。

    いずれも実際の公開ページで確認された矛盾: 同じページに v0.1.0 と v0.1.2 が並ぶ（8）、
    トップだけ診断数が 76（9）、E001 の発生条件が `` `' の後が辞書に一致しない`` と
    釣り合いの崩れた文字列になる（10）。
    """

    def test_top_page_must_derive_the_diagnostic_count(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "ja").mkdir()
            stale = root / "ja" / "index.html"
            stale.write_text('<div class="fact-value" data-fact="diagnostics">76</div>', encoding="utf-8")
            errors = website.audit_published_claims(root, {stale: website.Page(stale.read_text())}, "0.1.1")
            self.assertEqual(len(errors), 1, errors)
            self.assertIn("診断数 76", errors[0])
            self.assertIn(str(len(website.diagnostics_rows())), errors[0])

    def test_top_page_without_the_count_hook_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "en").mkdir()
            page = root / "en" / "index.html"
            page.write_text('<div class="fact-value">76</div>', encoding="utf-8")
            errors = website.audit_published_claims(root, {page: website.Page(page.read_text())}, "0.1.1")
            self.assertTrue(any("data-fact" in error for error in errors), errors)

    def test_announced_version_must_match_the_published_release(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            page = root / "index.html"
            page.write_text('<span data-announcement="version">v0.1.0 公開</span>', encoding="utf-8")
            errors = website.audit_published_claims(root, {page: website.Page(page.read_text())}, "0.1.2")
            self.assertEqual(len(errors), 1, errors)
            self.assertIn("v0.1.0 公開", errors[0])
            page.write_text('<span data-announcement="version">v0.1.2 公開</span>', encoding="utf-8")
            self.assertEqual(website.audit_published_claims(root, {page: website.Page(page.read_text())}, "0.1.2"), [])

    def test_diagnostic_page_must_render_inline_code(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            folder = root / "ja" / "diagnostics" / "E001"
            folder.mkdir(parents=True)
            page = folder / "index.html"
            page.write_text("<p><code>&#39;</code> の後が辞書に一致しない</p>", encoding="utf-8")
            parsed = {page: website.Page(page.read_text(encoding="utf-8"))}
            self.assertEqual(website.audit_published_claims(root, parsed, "0.1.1"), [])

    def test_diagnostic_page_may_not_show_a_stray_backtick(self):
        """公開ページで実際に出ていた表示（`` `' の後が…``）を落とすこと。"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            folder = root / "ja" / "diagnostics" / "E001"
            folder.mkdir(parents=True)
            page = folder / "index.html"
            page.write_text("<p>'` の後が辞書に一致しない</p>", encoding="utf-8")
            errors = website.audit_published_claims(root, {page: website.Page(page.read_text(encoding="utf-8"))}, "0.1.1")
            self.assertTrue(any("バッククォート" in error for error in errors), errors)
            self.assertTrue(any("<code> で描画されていない" in error for error in errors), errors)

    def test_generated_catalog_matches_the_specification_table(self):
        rows = website.diagnostics_rows()
        spec = (website.ROOT / "docs" / "specifications" / "diagnostics.md").read_text(encoding="utf-8")
        table = [line for line in spec.splitlines() if website.re.match(r"^\| [EWI]\d{3} \|", line)]
        self.assertEqual(len(rows), len(table))
        self.assertEqual([row["code"] for row in rows],
                         [line.split("|")[1].strip() for line in table])

    def test_generated_catalog_keeps_inner_code_spans_balanced(self):
        by_code = {row["code"]: row for row in website.diagnostics_rows()}
        # 生成時に前後のバッククォートを一律に削ると、内部スパンが崩れる（監査の再現）
        self.assertEqual(by_code["E001"]["condition"], "`'` の後が辞書に一致しない")
        self.assertEqual(by_code["E013"]["condition"], "`relations` 等の参照不能")
        self.assertEqual(by_code["E024"]["condition"], "`materials` 外を指す `path`")
        for row in by_code.values():
            for field in ("condition", "ja", "en", "action"):
                self.assertEqual(row[field].count("`") % 2, 0, f"{row['code']} {field}: {row[field]}")
                self.assertNotIn("``", row[field])

    def test_visible_text_skips_style_and_script_and_collects_code(self):
        page = website.Page(
            "<style>.a { --tick: '`'; }</style><script>var s = '`';</script>"
            "<p>本文 <code>range</code> の外</p>"
        )
        # <code> の中身も利用者に見える本文（そこにバッククォートが残っても落とするため）
        self.assertEqual("".join(page.visible_text), "本文 range の外")
        self.assertEqual(page.code_texts, {"range"})
