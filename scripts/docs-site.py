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
            title = title_of(rewritten, path.stem)
            name = map_slug(path.name)
            outputs[OUT / group / f"{name}.md"] = (
                frontmatter(title, group, order.get(path.name, 99), rel) + body
            )
            nav[group].append((order.get(path.name, 99), name, title))

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
    for group in ("users", "specifications"):
        folder = OUT / group
        if not folder.is_dir():
            continue
        for path in folder.glob("*.md"):
            if path not in expected:
                changed += 1
                if not check:
                    path.unlink()

    if check and changed:
        print(f"NG: サイトのページが docs と一致しません（{changed} 件の差）。python3 scripts/docs-site.py を実行すること")
        return 1
    print(
        f"{'OK' if check else '生成'}: {len(nav['users'])} ガイド / {len(nav['specifications'])} 仕様書"
        + ("（docs と一致）" if check else f"（{changed} 件を更新）")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
