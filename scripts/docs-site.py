#!/usr/bin/env python3
"""scripts/docs-site.py — `docs/` をそのままサイトのページにする生成器。

GitHub の Markdown を読ませるのではなく、**サイト上で読める形**にするための道具。
本文は複製せず、`docs/users/*.md` と `docs/specifications/*.md` をそのまま
Astro のコンテンツ（`website/src/content/docs/`）へ写し、リンクだけをサイト内の URL に直す。

    python3 scripts/docs-site.py            # 生成する
    python3 scripts/docs-site.py --check    # 生成し直して差分があれば終了コード 1（CI 用）

生成物（手で編集しない。`--check` が一致を見張る）:

    website/src/content/docs/users/*.md
    website/src/content/docs/specifications/*.md
    website/src/data/docs-nav.ts
    guide/*.html            （アプリと同じ公開サイト。`index.html` に `copy-dir` で載せる）

リンクの規則:

    ./start.md                → /ja/manual/start
    ./README.md               → /ja/manual/
    ../specifications/x.md    → /ja/specifications/x
    外部 URL・`#アンカー` はそのまま

アンカー（`#節` など）が実際の見出しに無ければエラーにする（サイトで 404 にしないため）。
"""

from __future__ import annotations

import json
import re
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
OUT = ROOT / "website" / "src" / "content" / "docs"
NAV = ROOT / "website" / "src" / "data" / "docs-nav.ts"
# アプリ（GitHub Pages）の隣に置く手引きの HTML。仕様 §14.2.1 の 3 URL がここに実在する。
GUIDE = ROOT / "guide"
# 公式サイト（Astro）の `public/` にも同じ HTML を置く。アプリのドメインと公式サイトのドメインの
# どちらから来ても、仕様 §14.2.1 の 3 URL が開ける（中身は同じ生成物）。
GUIDE_PUBLIC = ROOT / "website" / "public" / "guide"
# ファイル名 → 公開する HTML の名前（`project-structure.md` は仕様の URL に合わせて下線）。
HTML_NAME = {"README.md": "index", "project-structure.md": "project_structure"}

# 表示順（docs/users の並び）。ここに無いファイルは末尾に名前順で並ぶ。
USER_ORDER = {
    "README.md": 0,
    "start.md": 1,
    "cheatsheet.md": 2,
    "glossary.md": 3,
    "faq.md": 4,
    "project-structure.md": 5,
    "git.md": 6,
    "publish.md": 7,
}

SPEC_ORDER = {
    "overview.md": 0,
    "project.md": 1,
    "syntax.md": 2,
    "semantics.md": 3,
    "diagnostics.md": 4,
    "editing.md": 5,
    "ui.md": 6,
    "platform.md": 7,
    "agent.md": 8,
    "workflows.md": 9,
    "interfaces.md": 10,
}

LINK = re.compile(r"\]\(([^)\s]+)\)")


def slug(text: str) -> str:
    """GitHub / Astro の見出しアンカー（日本語はそのまま、記号は落とす）。"""
    lowered = text.strip().lower()
    cleaned = re.sub(r"[^\w\- ]", "", lowered)
    return cleaned.replace(" ", "-")


def heading_slugs(markdown: str) -> set[str]:
    """その文書で飛べるアンカーの集合（見出しと `<a id>`）。"""
    found = set(re.findall(r'<a id="([^"]+)"', markdown))
    for line in markdown.splitlines():
        if line.startswith("#"):
            found.add(slug(line.lstrip("#").strip()))
    return found


def title_of(markdown: str, fallback: str) -> str:
    for line in markdown.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return fallback


def map_slug(name: str) -> str:
    return "index" if name == "README.md" else Path(name).stem


def rewrite_links(text: str, kind: str, anchors: dict[str, set[str]], where: str) -> tuple[str, list[str]]:
    """リンクをサイト内 URL に直し、アンカーが無ければエラーにする。"""
    errors: list[str] = []

    def repl(match: re.Match[str]) -> str:
        target = match.group(1)
        if target.startswith(("http://", "https://", "mailto:", "#")):
            return match.group(0)
        path, _, anchor = target.partition("#")
        if not path.endswith(".md") and not path.endswith("/"):
            return match.group(0)

        if path.startswith("../specifications/"):
            name = path[len("../specifications/") :]
            page = "/ja/specifications/" if name in ("",) else f"/ja/specifications/{map_slug(name)}"
        elif path.startswith("./"):
            name = path[2:]
            page = "/ja/manual/" if name in ("", "README.md") else f"/ja/manual/{map_slug(name)}"
        elif path in ("../specifications", "../specifications/"):
            page = "/ja/specifications/"
        else:
            return match.group(0)

        if anchor:
            known = anchors.get(page)
            if known is None or anchor not in known:
                errors.append(f"{where}: アンカー {target} が生成先に存在しない")
        return f"]({page}{('#' + anchor) if anchor else ''})"

    return LINK.sub(repl, text), errors



# ============================================================ 手引きの HTML（§14.2.1・§18.5）
#
# アプリは GitHub Pages（`https://sonneko.github.io/novnow/`）に配るので、同じ場所の
# `guide/*.html` が公開サイトになる。本文の正本は `docs/users/*.md` のままで、ここは
# **同じ 1 か所から HTML を作るだけ**（書き写さない）。

HTML_HEAD = """<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — novnow</title>
<style>
:root {{ color-scheme: dark; }}
* {{ box-sizing: border-box; }}
body {{
  margin: 0; padding: 0 1.25rem 5rem;
  background: #16161a; color: #e6e6ea;
  font-family: system-ui, -apple-system, "Hiragino Sans", "Noto Sans JP", sans-serif;
  line-height: 1.85; font-size: 16px;
}}
main {{ max-width: 46rem; margin: 0 auto; }}
header.bar {{
  max-width: 46rem; margin: 0 auto; padding: 1rem 0 0.5rem;
  display: flex; gap: 1rem; align-items: baseline; flex-wrap: wrap;
  border-bottom: 1px solid #2c2c33; margin-bottom: 2rem;
}}
header.bar a {{ color: #b9a7ff; text-decoration: none; }}
header.bar .brand {{ font-weight: 700; color: #e6e6ea; }}
header.bar .spacer {{ flex: 1; }}
h1 {{ font-size: 1.7rem; line-height: 1.4; margin: 1.5rem 0 0.75rem; }}
h2 {{ font-size: 1.3rem; margin: 2.2rem 0 0.6rem; border-bottom: 1px solid #2c2c33; padding-bottom: 0.3rem; }}
h3 {{ font-size: 1.1rem; margin: 1.8rem 0 0.5rem; }}
a {{ color: #b9a7ff; }}
code {{ background: #232329; padding: 0.1em 0.35em; border-radius: 4px; font-size: 0.92em; }}
pre {{ background: #232329; padding: 0.9rem 1rem; border-radius: 8px; overflow-x: auto; }}
pre code {{ background: none; padding: 0; }}
blockquote {{ margin: 1rem 0; padding: 0.2rem 1rem; border-left: 3px solid #4a4a58; color: #b9b9c4; }}
table {{ border-collapse: collapse; width: 100%; margin: 1.2rem 0; font-size: 0.94rem; }}
th, td {{ border: 1px solid #2c2c33; padding: 0.45rem 0.7rem; text-align: left; vertical-align: top; }}
th {{ background: #1e1e24; }}
hr {{ border: none; border-top: 1px solid #2c2c33; margin: 2rem 0; }}
footer {{ max-width: 46rem; margin: 3rem auto 0; padding-top: 1rem; border-top: 1px solid #2c2c33;
  color: #85858f; font-size: 0.85rem; }}
</style>
</head>
<body>
<header class="bar">
  <a class="brand" href="index.html">novnow ユーザーガイド</a>
  <a href="start.html">はじめての手引き</a>
  <a href="cheatsheet.html">早見表</a>
  <a href="glossary.html">用語集</a>
  <a href="faq.html">よくある質問</a>
  <span class="spacer"></span>
  <a href="https://novnow.sonneko.com/app/">アプリを開く</a>
</header>
<main>
"""

HTML_FOOT = """</main>
<footer>
  このページは <code>{source}</code> から自動生成しています。直すときはリポジトリの
  <code>docs/users/</code> を編集してください（<code>python3 scripts/docs-site.py</code> で作り直します）。
</footer>
</body>
</html>
"""


def html_name(name: str) -> str:
    """`docs/users` のファイル名 → 公開する HTML の名前。"""
    return HTML_NAME.get(name, Path(name).stem)


def inline_html(text: str, link: "callable[[str], str]") -> str:
    """1 行の Markdown（`code` / **強調** / [リンク](url)）を HTML にする。"""
    out = []
    index = 0
    pattern = re.compile(r"`([^`]+)`|\*\*([^*]+)\*\*|\[([^\]]+)\]\(([^)\s]+)\)")
    for match in pattern.finditer(text):
        out.append(escape(text[index : match.start()]))
        code, strong, label, href = match.groups()
        if code is not None:
            out.append(f"<code>{escape(code)}</code>")
        elif strong is not None:
            out.append(f"<strong>{escape(strong)}</strong>")
        else:
            out.append(f'<a href="{escape(link(href))}">{inline_html(label, link)}</a>')
        index = match.end()
    out.append(escape(text[index:]))
    return "".join(out)


def escape(text: str) -> str:
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def markdown_to_html(markdown: str, link: "callable[[str], str]") -> str:
    """手引きに使う範囲の Markdown を HTML にする（見出し・箇条書き・表・コード・引用・段落）。"""
    lines = markdown.splitlines()
    out: list[str] = []
    paragraph: list[str] = []
    index = 0

    def flush() -> None:
        if paragraph:
            joined = " ".join(part.strip() for part in paragraph)
            out.append(f"<p>{inline_html(joined, link)}</p>")
            paragraph.clear()

    while index < len(lines):
        line = lines[index]
        stripped = line.strip()
        if stripped.startswith("```"):
            flush()
            index += 1
            block: list[str] = []
            while index < len(lines) and not lines[index].strip().startswith("```"):
                block.append(lines[index])
                index += 1
            language = stripped[3:].strip()
            code = escape("\n".join(block))
            css = f' class="language-{language}"' if language else ""
            out.append(f"<pre><code{css}>{code}</code></pre>")
            index += 1
            continue
        heading = re.match(r"^(#{1,6})\s+(.*)$", line)
        if heading:
            flush()
            level = len(heading.group(1))
            text = heading.group(2).strip()
            out.append(f'<h{level} id="{slug(text)}">{inline_html(text, link)}</h{level}>')
            index += 1
            continue
        if re.match(r'^<a id="[^"]+"></a>$', stripped) or re.match(r'^<a id="[^"]+">$', stripped):
            flush()
            out.append(stripped)
            index += 1
            continue
        if stripped in ("---", "***", "___"):
            flush()
            out.append("<hr>")
            index += 1
            continue
        if stripped.startswith("|") and stripped.endswith("|"):
            flush()
            rows: list[list[str]] = []
            while index < len(lines) and lines[index].strip().startswith("|"):
                cells = [cell.strip() for cell in lines[index].strip().strip("|").split("|")]
                if not all(re.fullmatch(r":?-{2,}:?", cell) for cell in cells):
                    rows.append(cells)
                index += 1
            if rows:
                head, *body = rows
                out.append("<table><thead><tr>")
                out.extend(f"<th>{inline_html(cell, link)}</th>" for cell in head)
                out.append("</tr></thead><tbody>")
                for row in body:
                    out.append("<tr>")
                    out.extend(f"<td>{inline_html(cell, link)}</td>" for cell in row)
                    out.append("</tr>")
                out.append("</tbody></table>")
            continue
        bullet = re.match(r"^(\s*)[-*]\s+(.*)$", line)
        numbered = re.match(r"^(\s*)\d+\.\s+(.*)$", line)
        if bullet or numbered:
            flush()
            ordered = bool(numbered)
            match = numbered or bullet
            assert match is not None
            out.append("<ol>" if ordered else "<ul>")
            while index < len(lines):
                current = lines[index]
                item = re.match(r"^(\s*)[-*]\s+(.*)$", current) or re.match(
                    r"^(\s*)\d+\.\s+(.*)$", current
                )
                if not item or bool(re.match(r"^\s*\d+\.", current)) != ordered:
                    break
                out.append(f"<li>{inline_html(item.group(2).strip(), link)}</li>")
                index += 1
            out.append("</ol>" if ordered else "</ul>")
            continue
        if stripped.startswith(">"):
            flush()
            quote: list[str] = []
            while index < len(lines) and lines[index].strip().startswith(">"):
                quote.append(lines[index].strip().lstrip(">").strip())
                index += 1
            out.append(f"<blockquote>{inline_html(' '.join(quote), link)}</blockquote>")
            continue
        if not stripped:
            flush()
            index += 1
            continue
        paragraph.append(line)
        index += 1
    flush()
    return "\n".join(out)


def guide_link(href: str, names: set[str]) -> str:
    """手引きの HTML の中のリンク。同じフォルダのページは HTML へ、仕様書はリポジトリへ。"""
    if href.startswith(("http://", "https://", "mailto:")):
        return href
    path, _, anchor = href.partition("#")
    suffix = f"#{anchor}" if anchor else ""
    if path.startswith("./"):
        name = path[2:]
        if name in names:
            return f"{html_name(name)}.html{suffix}"
        return f"{path}{suffix}"
    if path.startswith("../specifications/"):
        name = path[len("../specifications/") :]
        return f"https://github.com/sonneko/novnow/blob/main/docs/specifications/{name}{suffix}"
    return href


def frontmatter(title: str, group: str, order: int, source: str) -> str:
    escaped = title.replace('"', '\\"')
    return (
        "---\n"
        f'title: "{escaped}"\n'
        f'group: "{group}"\n'
        f"order: {order}\n"
        f'source: "{source}"\n'
        "---\n\n"
    )


def main() -> int:
    check = "--check" in sys.argv[1:]
    sources = [
        ("users", sorted((DOCS / "users").glob("*.md")), USER_ORDER, "/ja/manual/"),
        ("specifications", sorted((DOCS / "specifications").glob("*.md")), SPEC_ORDER, "/ja/specifications/"),
    ]

    # 先に見出しを全部集める（リンク先のアンカーを検証するため）
    anchors: dict[str, set[str]] = {"/ja/manual/": set(), "/ja/specifications/": set()}
    for group, files, order, base in sources:
        for path in files:
            page = base if path.name == "README.md" else f"{base}{map_slug(path.name)}"
            anchors[page] = heading_slugs(path.read_text(encoding="utf-8"))

    errors: list[str] = []
    outputs: dict[Path, str] = {}
    nav: dict[str, list[tuple[int, str, str]]] = {"users": [], "specifications": []}

    for group, files, order, base in sources:
        for path in files:
            text = path.read_text(encoding="utf-8")
            rel = path.relative_to(ROOT).as_posix()
            rewritten, problems = rewrite_links(text, group, anchors, rel)
            errors.extend(problems)
            # 先頭の H1 はページの見出しに使うので、本文からは外す（二重見出しを避ける）
            body = re.sub(r"\A# .*\n", "", rewritten, count=1).lstrip("\n")
            # HTML 版は**元の** Markdown から作る（サイト内 URL ではなく、同じフォルダの HTML へ張る）
            raw_body = re.sub(r"\A# .*\n", "", text, count=1).lstrip("\n")
            title = title_of(rewritten, path.stem)
            name = map_slug(path.name)
            outputs[OUT / group / f"{name}.md"] = (
                frontmatter(title, group, order.get(path.name, 99), rel) + body
            )
            nav[group].append((order.get(path.name, 99), name, title))
            if group == "users":
                seen = {item.name for item in files}
                page = markdown_to_html(
                    raw_body, lambda href, names=seen: guide_link(href, names)
                )
                document = (
                    HTML_HEAD.format(title=escape(title))
                    + f"<h1>{inline_html(title, lambda href: href)}</h1>\n"
                    + page
                    + "\n"
                    + HTML_FOOT.format(source=escape(rel))
                )
                name_html = f"{html_name(path.name)}.html"
                outputs[GUIDE / name_html] = document
                outputs[GUIDE_PUBLIC / name_html] = document

    def nav_ts() -> str:
        lines = [
            "// Generated by scripts/docs-site.py from docs/users/** and docs/specifications/**.",
            "// Run `python3 scripts/docs-site.py --check` to verify.",
            "",
            "export interface DocEntry {",
            "  slug: string;",
            "  title: string;",
            "}",
            "",
            "export const manualNav: DocEntry[] = [",
        ]
        for _, name, title in sorted(nav["users"]):
            lines.append(f'  {{ slug: "{name}", title: {json.dumps(title, ensure_ascii=False)} }},')
        lines += ["];", "", "export const specificationNav: DocEntry[] = ["]
        for _, name, title in sorted(nav["specifications"]):
            lines.append(f'  {{ slug: "{name}", title: {json.dumps(title, ensure_ascii=False)} }},')
        lines += ["];", ""]
        return "\n".join(lines)

    outputs[NAV] = nav_ts()

    if errors:
        for error in errors:
            print(f"NG: {error}")
        return 1

    changed = 0
    for path, content in outputs.items():
        old = path.read_text(encoding="utf-8") if path.exists() else None
        if old == content:
            continue
        changed += 1
        if not check:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")

    # 元が消えたページの生成物を消す
    expected = set(outputs)
    for folder, pattern in (
        (OUT / "users", "*.md"),
        (OUT / "specifications", "*.md"),
        (GUIDE, "*.html"),
        (GUIDE_PUBLIC, "*.html"),
    ):
        if not folder.is_dir():
            continue
        for path in folder.glob(pattern):
            if path not in expected:
                changed += 1
                if not check:
                    path.unlink()

    if check and changed:
        print(f"NG: サイトのページが docs と一致しません（{changed} 件の差）。python3 scripts/docs-site.py を実行すること")
        return 1
    print(
        f"{'OK' if check else '生成'}: {len(nav['users'])} ガイド / {len(nav['specifications'])} 仕様書"
        + f" / {len(nav['users'])} HTML"
        + ("（docs と一致）" if check else f"（{changed} 件を更新）")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
