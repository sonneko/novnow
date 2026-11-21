#!/usr/bin/env python3
"""リリースノートの定型を検査し、公開時の本文を出す（Issue #61 の提案 E）。

公開された Release の本文は成果物と同じで、あとから読む人が「この版で何が変わり、何が壊れ、
何が未解決か」を知る唯一の場所である。1 行の本文（`novnow 0.1.2 distribution artifacts`）では
その役割を果たさないので、**本文が無い版は公開できない**ようにする。

- 定型は `REF:docs/release-notes/TEMPLATE.md`。必須の 4 節は
  **変更点 / 参照（Issue・PR）/ 互換性 / 既知の問題**。
- 本文は `REF:docs/release-notes/v<版>.md` に置き、公開時にそのまま Release へ貼る
  （`REF:.github/workflows/release.yml` が `--notes-file` で使う）。
- 既知の問題の節は `REF:docs/known-issues/ja.md` へのリンクを**必ず**含む（検査する）。
- 定型より前に公開した版（`LEGACY_VERSIONS`）は公開時の本文をそのまま残し、
  節の検査はしない（書き直すと「公開時に何と書いてあったか」が失われる）。

使い方:
    python3 scripts/release-notes.py --check --version 0.1.1   # 公開前のゲート（release.yml / release-check.sh）
    python3 scripts/release-notes.py --check-all               # 既にある本文と配線の検査（CI / verify.sh）
    python3 scripts/release-notes.py --print --version 0.1.1   # Release へ貼る本文を stdout へ
    python3 scripts/release-notes.py --new --version 0.1.2     # 定型から書きかけを作る
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NOTES_DIR = ROOT / "docs" / "release-notes"
TEMPLATE = NOTES_DIR / "TEMPLATE.md"
GUIDE = ROOT / "docs" / "distribution.md"
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
RELEASE_CHECK = ROOT / "scripts" / "release-check.sh"
CI_WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"
VERIFY = ROOT / "scripts" / "verify.sh"
KNOWN_ISSUES = ROOT / "docs" / "known-issues" / "ja.md"

# 必須の 4 節（この順）と、v0.1.0 の本文に倣った推奨 2 節。
REQUIRED_SECTIONS = ("変更点", "参照（Issue・PR）", "互換性", "既知の問題")
RECOMMENDED_SECTIONS = ("配布物", "検証範囲")
# 既知の問題の節に必須のリンク（GitHub の URL でも `REF:` でもよい）。
KNOWN_ISSUES_TARGET = "docs/known-issues/ja.md"
REFERENCE_RE = re.compile(r"#\d+|https://github\.com/[^\s)〉》]+/(?:issues|pull|releases)/[\w./-]+")
# 定型より前に公開した版。公開時の本文を組み直さない（節の検査もしない）。
LEGACY_VERSIONS = {"0.1.0"}
LEGACY_MARKER = "<!-- legacy:"
GENERIC_PLACEHOLDERS = ("TODO", "TBD", "nnn")
VERSION_FROM_NAME = re.compile(r"^v(.+)\.md$")


def read(path: Path) -> str:
    """UTF-8 のテキスト。無ければ空文字。"""
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return ""


def display(path: Path) -> str:
    """リポジトリからの相対パス（`ROOT` の外にあるときは絶対パスのまま。試験で一時ディレクトリを使うため）。"""
    return path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else str(path)


def notes_path(version: str) -> Path:
    """版 → 本文のパス（`v0.1.1` → `docs/release-notes/v0.1.1.md`）。"""
    return NOTES_DIR / f"v{version}.md"


def headings(text: str) -> list[tuple[int, str]]:
    """見出しの (レベル, 題)。コードブロックの中は見ない。"""
    found: list[tuple[int, str]] = []
    inside = False
    for line in text.splitlines():
        if line.lstrip().startswith("```"):
            inside = not inside
            continue
        if inside:
            continue
        match = re.match(r"^(#{1,6})\s+(.*?)\s*$", line)
        if match:
            found.append((len(match.group(1)), match.group(2)))
    return found


def section_body(text: str, title: str) -> str:
    """`## <title>` の中身（次の同じか上のレベルの見出しまで）。"""
    lines = text.splitlines()
    start = None
    level = 0
    for index, line in enumerate(lines):
        match = re.match(r"^(#{1,6})\s+(.*?)\s*$", line)
        if not match:
            continue
        if start is None:
            if match.group(2).strip() == title:
                start = index + 1
                level = len(match.group(1))
            continue
        if len(match.group(1)) <= level:
            return "\n".join(lines[start:index])
    return "\n".join(lines[start:]) if start is not None else ""


def content_lines(body: str) -> list[str]:
    """空行とコメントを除いた本文の行。"""
    return [
        line.strip()
        for line in body.splitlines()
        if line.strip() and not line.strip().startswith("<!--")
    ]


def template_placeholders() -> set[str]:
    """定型の中の `{…}`（書きかけをそのまま公開しないための照合用）。"""
    return set(re.findall(r"\{[^{}\n]*\}", read(TEMPLATE)))


def placeholders_in(text: str) -> list[str]:
    """本文に残っている書きかけの印。"""
    found = [item for item in template_placeholders() if item in text]
    found += [item for item in GENERIC_PLACEHOLDERS if item in text]
    return sorted(set(found))


def legacy_versions() -> set[str]:
    """`LEGACY_VERSIONS` と、legacy の印が付いた本文の版。"""
    versions = set(LEGACY_VERSIONS)
    for path in sorted(NOTES_DIR.glob("v*.md")):
        if read(path).lstrip().startswith(LEGACY_MARKER):
            match = VERSION_FROM_NAME.match(path.name)
            if match:
                versions.add(match.group(1))
    return versions


def check_notes(version: str, text: str) -> list[str]:
    """1 版分の本文を検査する（`--check` と `--check-all` の共通の実体）。"""
    where = f"docs/release-notes/v{version}.md"
    errors: list[str] = []
    if not text:
        return [f"{where} が無い（リリースノートは必須。`python3 scripts/release-notes.py --new --version {version}` で書き始める）"]

    titles = [title for level, title in headings(text) if level == 1]
    if not titles:
        errors.append(f"{where}: H1（`# novnow v{version}`）が無い")
    elif titles[0] != f"novnow v{version}":
        errors.append(f"{where}: H1 が `# novnow v{version}` でない（実際: `{titles[0]}`）")

    if version in legacy_versions():
        # 公開時の本文をそのまま残す版。节の定型は求めない。
        return errors

    found = [title for level, title in headings(text) if level == 2]
    for title in REQUIRED_SECTIONS:
        if title not in found:
            errors.append(f"{where}: 必須の節 `## {title}` が無い")
    ordered = [title for title in found if title in REQUIRED_SECTIONS]
    if not errors and ordered != list(REQUIRED_SECTIONS):
        errors.append(
            f"{where}: 必須の 4 節の順が違う（期待: {' → '.join(REQUIRED_SECTIONS)}、実際: {' → '.join(ordered)}）"
        )

    leftovers = placeholders_in(text)
    if leftovers:
        errors.append(f"{where}: 定型の書きかけが残っている: {', '.join(leftovers)}")

    changes = content_lines(section_body(text, "変更点"))
    if not any(line.startswith(("-", "*", "1.", "2.")) or line[:1].isdigit() for line in changes):
        errors.append(f"{where}: `## 変更点` に箇条書きが 1 件も無い（何が変わったかを読める形にする）")

    references = section_body(text, "参照（Issue・PR）")
    if not REFERENCE_RE.search(references):
        errors.append(f"{where}: `## 参照（Issue・PR）` に Issue / PR の参照（`#61` や GitHub の URL）が無い")

    compatibility = content_lines(section_body(text, "互換性"))
    if not compatibility:
        errors.append(f"{where}: `## 互換性` が空（変更が無いなら「形式の変更は無い」と明示する）")

    known = section_body(text, "既知の問題")
    if not content_lines(known):
        errors.append(f"{where}: `## 既知の問題` が空（無いなら「掲載中の問題は無い」と書く）")
    if KNOWN_ISSUES_TARGET not in known:
        errors.append(
            f"{where}: `## 既知の問題` に {KNOWN_ISSUES_TARGET} へのリンクが無い"
            "（公開版の問題は同じ場所に集約する）"
        )
    return errors


def wiring_errors() -> list[str]:
    """定型が「公開の手順」と繋がっていること（本文だけあっても公開に使われなければ意味が無い）。"""
    errors: list[str] = []
    if not read(TEMPLATE):
        errors.append("docs/release-notes/TEMPLATE.md が無い（定型の正本）")
    else:
        template = read(TEMPLATE)
        for title in [*REQUIRED_SECTIONS, *RECOMMENDED_SECTIONS]:
            if f"## {title}" not in template:
                errors.append(f"docs/release-notes/TEMPLATE.md: 節 `## {title}` が無い")
        if KNOWN_ISSUES_TARGET not in template:
            errors.append(f"docs/release-notes/TEMPLATE.md: {KNOWN_ISSUES_TARGET} へのリンクが入っていない")

    release = read(RELEASE_WORKFLOW)
    if "scripts/release-notes.py" not in release:
        errors.append(".github/workflows/release.yml: リリースノートを使わずに Release を作っている")
    elif "--notes-file" not in release:
        errors.append(".github/workflows/release.yml: `--notes-file` で本文を渡していない（1 行の本文に戻る）")
    if "release-notes.py --check" not in release:
        errors.append(".github/workflows/release.yml: 公開の前段（prepare）で `--check` を実行していない")

    if "release-notes.py" not in read(RELEASE_CHECK):
        errors.append("scripts/release-check.sh: リリースノートを検査していない（手元のゲートが無い）")
    if "docs/release-notes/" not in read(GUIDE):
        errors.append("docs/distribution.md: リリースノート（docs/release-notes/）へのリンクが無い（手引きから辿れない）")
    if "docs/release-notes/" not in read(KNOWN_ISSUES):
        errors.append("docs/known-issues/ja.md: リリースノートへのリンクが無い（解消の履歴を辿れない）")
    if "release-notes.py --check-all" not in read(CI_WORKFLOW):
        errors.append(".github/workflows/ci.yml: `--check-all` を実行していない（PR で落ちない）")
    if "release-notes.py" not in read(VERIFY):
        errors.append("scripts/verify.sh: `--check-all` を実行していない（手元で CI と同じ判定にならない）")
    return errors


def existing_notes() -> list[tuple[str, Path]]:
    """`docs/release-notes/` の本文（版, パス）。定型は除く。"""
    found: list[tuple[str, Path]] = []
    for path in sorted(NOTES_DIR.glob("v*.md")):
        match = VERSION_FROM_NAME.match(path.name)
        if match:
            found.append((match.group(1), path))
    return found


def print_notes(version: str) -> str:
    """Release へ貼る本文（先頭の HTML コメント＝リポジトリ内の注記は除く）。"""
    text = read(notes_path(version))
    return re.sub(r"\A\s*<!--.*?-->\s*", "", text, flags=re.DOTALL).strip() + "\n"


def new_notes(version: str) -> Path:
    """定型から書きかけを作る（既にある本文は上書きしない）。"""
    path = notes_path(version)
    if path.exists():
        raise SystemExit(f"NG: {display(path)} は既にある（上書きしない）")
    template = read(TEMPLATE)
    if not template:
        raise SystemExit("NG: docs/release-notes/TEMPLATE.md が無い")
    NOTES_DIR.mkdir(parents=True, exist_ok=True)
    path.write_text(template.replace("{version}", version), encoding="utf-8")
    return path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="リリースノートの定型検査と本文の出力")
    parser.add_argument("--version", help="対象の版（Cargo.toml と同じ形。例: 0.1.1 / 0.1.2-rc.1）")
    parser.add_argument("--check", action="store_true", help="1 版分の本文を検査する（公開前のゲート）")
    parser.add_argument("--check-all", action="store_true", help="既にある本文すべてと配線を検査する")
    parser.add_argument("--print", action="store_true", help="Release へ貼る本文を stdout へ出す")
    parser.add_argument("--new", action="store_true", help="定型から書きかけを作る")
    parser.add_argument("--list", action="store_true", help="本文のある版を一覧する")
    args = parser.parse_args(argv)

    if args.list:
        for version, path in existing_notes():
            mark = " (legacy)" if version in legacy_versions() else ""
            print(f"v{version}{mark}: {display(path)}")
        return 0

    if args.new:
        if not args.version:
            parser.error("--new には --version が必要")
        path = new_notes(args.version)
        print(f"OK: {display(path)} を作った（公開前に `--check --version {args.version}` が通るように書く）")
        return 0

    if args.print:
        if not args.version:
            parser.error("--print には --version が必要")
        text = print_notes(args.version)
        if not text.strip():
            print(
                f"NG: docs/release-notes/v{args.version}.md が無い"
                f"（`python3 scripts/release-notes.py --new --version {args.version}`）",
                file=sys.stderr,
            )
            return 1
        sys.stdout.write(text)
        return 0

    if args.check:
        if not args.version:
            parser.error("--check には --version が必要")
        errors = check_notes(args.version, read(notes_path(args.version)))
        return report(errors, f"リリースノート v{args.version}")

    if args.check_all:
        errors: list[str] = []
        for version, path in existing_notes():
            errors.extend(check_notes(version, read(path)))
        errors.extend(wiring_errors())
        return report(errors, f"リリースノート {len(existing_notes())} 版と配線")

    parser.error("--check / --check-all / --print / --new / --list のいずれかが必要")
    return 2


def report(errors: list[str], what: str) -> int:
    if errors:
        for error in errors:
            print(f"NG: {error}", file=sys.stderr)
        print(f"NG: {what} に {len(errors)} 件の不備（scripts/release-notes.py）", file=sys.stderr)
        return 1
    print(f"OK: {what}が定型を満たしている（scripts/release-notes.py）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
