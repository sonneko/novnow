#!/usr/bin/env python3
"""Validate the emitted Pages tree, not just source strings (stdlib only).

--website-only permits exactly the application entry point, which deploy.yml
adds after the Astro build. Without it, the merged app must also exist.
No remote URL is fetched: service availability is a separate release gate.
"""
from __future__ import annotations

import argparse
from html.parser import HTMLParser
from pathlib import Path
import re
import tomllib
from urllib.parse import unquote, urljoin, urlsplit

ORIGIN = "https://sonneko.github.io"
BASE = "/novnow/"
VERSION = tomllib.loads((Path(__file__).resolve().parents[1] / "Cargo.toml").read_text())["package"]["version"]


class Page(HTMLParser):
    def __init__(self, text: str):
        super().__init__(convert_charrefs=True)
        self.ids: set[str] = set()
        self.duplicates: set[str] = set()
        self.references: list[str] = []
        self.images_without_alt = 0
        self.language_links: list[str] = []
        self.switch_depth = 0
        self.in_header = False
        self.header_text = []
        self.has_notice = False
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        identifier = attrs.get("id") or (attrs.get("name") if tag == "a" else None)
        if identifier:
            if identifier in self.ids:
                self.duplicates.add(identifier)
            self.ids.add(identifier)
        if tag == "header":
            self.in_header = True
        if "release-notice" in attrs.get("class", "").split():
            self.has_notice = True
        if tag == "div":
            if self.switch_depth:
                self.switch_depth += 1
            elif attrs.get("aria-label") == "Language":
                self.switch_depth = 1
        if tag == "a" and self.switch_depth and attrs.get("href"):
            self.language_links.append(attrs["href"])
        for key in ("href", "src", "action", "poster"):
            if attrs.get(key):
                self.references.append(attrs[key])
        if attrs.get("srcset"):
            self.references.extend(item.strip().split()[0] for item in attrs["srcset"].split(",") if item.strip())
        if tag == "img" and "alt" not in attrs:
            self.images_without_alt += 1

    def handle_data(self, data):
        if self.in_header:
            self.header_text.append(data)

    def handle_endtag(self, tag):
        if tag == "header":
            self.in_header = False
        if tag == "div" and self.switch_depth:
            self.switch_depth -= 1


def resolve(root: Path, page_url: str, reference: str, website_only: bool):
    """Return (local file, fragment, error); external links return no file."""
    url = urlsplit(urljoin(page_url, reference))
    if url.scheme not in ("http", "https") or url.netloc != "sonneko.github.io":
        return None, "", None
    path = unquote(url.path)
    if not path.startswith(BASE):
        return None, "", f"outside Pages base: {reference}"
    if website_only and path == BASE + "app/" and not url.fragment:
        return None, "", None
    target = root / path[len(BASE):]
    if not target.resolve().is_relative_to(root.resolve()):
        return None, "", f"unsafe path: {reference}"
    if target.is_dir() or (not target.exists() and not target.suffix):
        target /= "index.html"
    if not target.is_file():
        return None, "", f"missing target: {reference}"
    return target, unquote(url.fragment), None


def audit(root: Path, website_only: bool = False) -> tuple[int, list[str]]:
    pages = {p: Page(p.read_text(encoding="utf-8")) for p in root.rglob("*.html")}
    errors = []
    if not pages:
        return 0, [f"no HTML pages in {root}"]
    for path, page in pages.items():
        relative = path.relative_to(root).as_posix()
        page_url = ORIGIN + BASE + relative.removesuffix("index.html")
        for identifier in sorted(page.duplicates):
            errors.append(f"{relative}: duplicate id: {identifier}")
        if page.images_without_alt:
            errors.append(f"{relative}: {page.images_without_alt} images without alt")
        for reference in page.references:
            target, fragment, error = resolve(root, page_url, reference, website_only)
            if error:
                errors.append(f"{relative}: {error}")
            elif fragment and target in pages and fragment not in pages[target].ids:
                errors.append(f"{relative}: missing anchor: {reference}")
        # A language switch must preserve the current article, not silently go
        # back to the catalog. Excludes the separately generated app guide.
        if relative.startswith(("ja/", "en/")):
            if f"v{VERSION}" not in "".join(page.header_text):
                errors.append(f"{relative}: header version differs from Cargo.toml")
            if not page.has_notice:
                errors.append(f"{relative}: compatibility notice missing")
            suffix = relative[3:].removesuffix("index.html").rstrip("/")
            expected = {BASE + language + "/" + suffix for language in ("ja", "en")}
            actual = {urlsplit(urljoin(page_url, ref)).path.rstrip("/") for ref in page.language_links}
            if actual != {ref.rstrip("/") for ref in expected}:
                errors.append(f"{relative}: language switch does not preserve article")
    for path in root.rglob("*.css"):
        page_url = ORIGIN + BASE + path.relative_to(root).as_posix()
        for reference in re.findall(r"url\(\s*['\"]?([^'\"\s)]+)", path.read_text(encoding="utf-8")):
            _, _, error = resolve(root, page_url, reference, website_only)
            if error:
                errors.append(f"{path.relative_to(root)}: {error}")
    return len(pages), errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", type=Path, default=Path("website/dist"))
    parser.add_argument("--website-only", action="store_true")
    args = parser.parse_args()
    count, errors = audit(args.dir, args.website_only)
    if errors:
        for error in errors:
            print(error)
        raise SystemExit(1)
    print(f"OK: {count} HTML files; internal links, anchors, assets, CSS, language switching, IDs and image alt")


if __name__ == "__main__":
    main()
