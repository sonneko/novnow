#!/usr/bin/env python3
"""Validate the emitted Pages tree, not just source strings (stdlib only).

--website-only permits exactly the application entry point, which deploy.yml
adds after the Astro build. Without it, the merged app must also exist.
No remote URL is fetched: service availability is a separate release gate.
"""
from __future__ import annotations

import argparse
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import tomllib
from urllib.parse import unquote, urljoin, urlsplit

ORIGIN = "https://sonneko.github.io"
BASE = "/novnow/"
ROOT = Path(__file__).resolve().parents[1]
# 開発版の版（`Cargo.toml`）と、サイトが名乗る版（公開済みの版）は別物である。
# `docs/developers.md` の規則どおり `REF:website/src/data/release.ts` は公開物を確認してから
# 上げるので、版更新 PR から公開までの間は 2 つが食い違う。見出しは公開済みの版を出すのが
# 正しいので（未公開の版を「公開済み」として案内しない）、検査もそちらと比べる。
DEV_VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
RELEASE_DATA = ROOT / "website/src/data/release.ts"
DIAGNOSTICS_DATA = ROOT / "website/src/data/diagnostics.ts"
DIAGNOSTICS_EN = ROOT / "website/src/data/diagnostics-en.json"
DIAGNOSTICS_EXAMPLES = ROOT / "website/src/data/diagnostics-examples.json"
# 生成物のセルに残る inline `code`（仕様書の表由来）。表示側は <code> へ変換する。
CODE_SPAN = re.compile(r"`([^`]+)`")
# 数や版を書き写さないためのフック（CSS Modules のクラス名は hash されるので data 属性で置く）。
FACT_RE = re.compile(r'data-fact="([^"]+)"[^>]*>([^<]*)<')
ANNOUNCEMENT_RE = re.compile(r'data-announcement="([^"]+)"[^>]*>([^<]*)<')


def published_version() -> str:
    """Return the version the site announces (the published release, from release.ts)."""
    found = re.search(r'export const version = "([^"]+)"', RELEASE_DATA.read_text(encoding="utf-8"))
    if not found:
        raise SystemExit(f"{RELEASE_DATA.name} に公開済みの版（export const version）が無い")
    return found.group(1)


def diagnostics_rows() -> list[dict[str, str]]:
    """The catalog the site is built from (scripts/diagnostics-site.py generates it)."""
    text = DIAGNOSTICS_DATA.read_text(encoding="utf-8")
    body = text.split("export const diagnostics = ", 1)[1].rsplit(" as const;", 1)[0]
    return json.loads(body)


def diagnostic_data() -> tuple[list[dict[str, str]], dict[str, list[str]], dict[str, list[str]]]:
    """カタログと、人が書いた英語解説・具体例（いずれも 85 コードぶん）。"""
    return (
        diagnostics_rows(),
        json.loads(DIAGNOSTICS_EN.read_text(encoding="utf-8")),
        json.loads(DIAGNOSTICS_EXAMPLES.read_text(encoding="utf-8")),
    )


def diagnostic_cells(
    language: str,
    detail: bool,
    row: dict[str, str],
    english: dict[str, list[str]],
    examples: dict[str, list[str]],
) -> list[str]:
    """そのページが実際に描画するセル（一覧と詳細、日英で中身が違う）。

    描かないセルまで要求すると、後から inline `code` を足したときに誤って落ちる。
    """
    if language == "ja":
        cells = [row["condition"], row["ja"], row["action"]]
        if detail:
            cells.append(examples[row["code"]][0])
        return cells
    cells = [row["en"], *english[row["code"]]]
    if detail:
        cells.append(examples[row["code"]][1])
    return cells


def version_key(version: str) -> tuple[int, ...]:
    """Numeric key, so 'the site is ahead of the development version' is detectable."""
    return tuple(int(part) for part in re.split(r"[.+-]", version) if part.isdigit())


VERSION = published_version()


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
        # 利用者に見える本文（style / script の中身は除く）と、<code> として出た文字列。
        # 生成データの inline `code` が記号のまま出ていないか、逆にコードとして描画されたかを
        # 生成物側で見るために集める。
        self.visible_text: list[str] = []
        self.code_texts: set[str] = set()
        self.facts: dict[str, str] = dict(FACT_RE.findall(text))
        self.announcements: dict[str, str] = dict(ANNOUNCEMENT_RE.findall(text))
        self._skip_depth = 0
        self._code_buffer: list[str] | None = None
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag in ("style", "script"):
            self._skip_depth += 1
        if tag == "code" and self._code_buffer is None:
            self._code_buffer = []
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
        if self._skip_depth:
            return
        self.visible_text.append(data)
        if self._code_buffer is not None:
            self._code_buffer.append(data)

    def handle_endtag(self, tag):
        if tag in ("style", "script") and self._skip_depth:
            self._skip_depth -= 1
        if tag == "code" and self._code_buffer is not None:
            self.code_texts.add("".join(self._code_buffer).strip())
            self._code_buffer = None
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
    if website_only and path in (BASE + "app/", BASE + "nightly-app/") and not url.fragment:
        return None, "", None
    target = root / path[len(BASE):]
    if not target.resolve().is_relative_to(root.resolve()):
        return None, "", f"unsafe path: {reference}"
    if target.is_dir() or (not target.exists() and not target.suffix):
        target /= "index.html"
    if not target.is_file():
        return None, "", f"missing target: {reference}"
    return target, unquote(url.fragment), None


def audit_published_claims(root: Path, pages: dict[Path, "Page"], version: str) -> list[str]:
    """書き写された数・版・コード表記が、生成物の側でも正本と一致すること。

    - トップの診断数は診断カタログの件数から（#61 追加監査 9: トップだけ 76 のままだった）。
    - 告知の版は `release.ts` の公開版（同 8: 同じページに v0.1.0 と v0.1.2 が出ていた）。
    - 診断ページの inline `code` は <code> として描画し、記号を見せない（同 10）。
    """
    errors: list[str] = []
    rows, english, examples = diagnostic_data()
    by_code = {row["code"]: row for row in rows}
    for path, page in pages.items():
        relative = path.relative_to(root).as_posix()
        if relative in ("ja/index.html", "en/index.html"):
            shown = page.facts.get("diagnostics")
            if shown is None:
                errors.append(f'{relative}: 診断数のフック（data-fact="diagnostics"）が無い')
            elif shown != str(len(rows)):
                errors.append(
                    f"{relative}: トップの診断数 {shown} がカタログ（{DIAGNOSTICS_DATA.name}）の"
                    f" {len(rows)} 件と違う"
                )
        for key, text in page.announcements.items():
            if key == "version" and f"v{version}" not in text:
                errors.append(
                    f"{relative}: 告知が {text.strip()!r}（{RELEASE_DATA.name} の公開版は v{version}）"
                )
        match = re.match(r"^(ja|en)/diagnostics/(?:([EWI]\d{3})/)?index\.html$", relative)
        if not match:
            continue
        language, code = match.groups()
        stray = next((chunk for chunk in page.visible_text if "`" in chunk), None)
        if stray is not None:
            errors.append(f"{relative}: 本文にバッククォートが残っている: {stray.strip()[:60]!r}")
        for row in ([by_code[code]] if code else rows):
            for cell in diagnostic_cells(language, code is not None, row, english, examples):
                for span in CODE_SPAN.findall(cell):
                    if span not in page.code_texts:
                        errors.append(
                            f"{relative}: {row['code']} の `{span}` が <code> で描画されていない"
                        )
    return errors


def audit(root: Path, website_only: bool = False, version: str | None = None) -> tuple[int, list[str]]:
    pages = {p: Page(p.read_text(encoding="utf-8")) for p in root.rglob("*.html")}
    errors = []
    version = version or VERSION
    if version_key(version) > version_key(DEV_VERSION):
        errors.append(
            f"{RELEASE_DATA.name}: 公開済みの版 {version} が開発版 Cargo.toml {DEV_VERSION} より新しい"
        )
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
            if f"v{version}" not in "".join(page.header_text):
                errors.append(f"{relative}: header version differs from {RELEASE_DATA.name}")
            if not page.has_notice:
                errors.append(f"{relative}: compatibility notice missing")
            suffix = relative[3:].removesuffix("index.html").rstrip("/")
            expected = {BASE + language + "/" + suffix for language in ("ja", "en")}
            actual = {urlsplit(urljoin(page_url, ref)).path.rstrip("/") for ref in page.language_links}
            if actual != {ref.rstrip("/") for ref in expected}:
                errors.append(f"{relative}: language switch does not preserve article")
    errors.extend(audit_published_claims(root, pages, version))
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
    print(
        f"OK: {count} HTML files; internal links, anchors, assets, CSS, language switching, IDs,"
        " image alt, published version, diagnostic count and inline code"
    )


if __name__ == "__main__":
    main()
