#!/usr/bin/env python3
"""`REF:docs/specifications/ui.md` §11.1・§11.2 の表から `webui/src/lib/theme/tokens.ts` を作る。

色の**正本は仕様書の表**である（`REF:docs/specifications/ui.md` §11.1 MUST: すべての描画は
セマンティックカラートークン経由で色を決める）。第 1 の画面実装は `REF:src/ui/theme.rs` の
`palette()` が同じ表を持つ。第 2 はこの生成物を持つので、表を直せば両方に同じ値が入る
（`REF:docs/plans.md#webui-w3`）。

使い方:

    python3 scripts/theme-ts.py            # 書き出す
    python3 scripts/theme-ts.py --check    # 一致するかだけ見る（`scripts/verify.sh` 用）
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs" / "specifications" / "ui.md"
TARGET = ROOT / "webui" / "src" / "lib" / "theme" / "tokens.ts"
RUST = ROOT / "src" / "ui" / "theme.rs"

# §11.1 の表の行（`| \`bg.base\` | \`#15171B\` | \`#FBFBF9\` | 用途 |`）
ROW = re.compile(r"^\|\s*`([a-z][a-z0-9_.]*)`\s*\|\s*`(#[0-9A-Fa-f]{6})`\s*\|\s*`(#[0-9A-Fa-f]{6})`\s*\|")
# 第 1 の `palette()` の対応（Rust のフィールド名 → トークン名）
RUST_FIELD = {
    "bg_base": "bg.base",
    "bg_raised": "bg.raised",
    "bg_inset": "bg.inset",
    "bg_selection": "bg.selection",
    "fg_primary": "fg.primary",
    "fg_muted": "fg.muted",
    "fg_faint": "fg.faint",
    "border": "border",
    "accent": "accent",
    "accent_faint": "accent.faint",
    "unsaved": "state.unsaved",
    "error": "state.error",
    "warn": "state.warn",
    "ok": "state.ok",
}

HEADER = """\
// このファイルは **生成物**。手で編集しない（`scripts/theme-ts.py` が書く）。
//
// 正本は `REF:docs/specifications/ui.md` §11.1・§11.2 の表。第 1 の画面実装は
// `REF:src/ui/theme.rs` の `palette()` が同じ表から作っている。第 2 の画面実装はこの
// 生成物だけを使い、コンポーネントに生の色を書かない（§11.1 MUST）。
//
// 変更するときは `python3 scripts/theme-ts.py` を実行する（`scripts/verify.sh` の
// `webui-theme` が `--check` で一致を強制する）。
"""


def section(text: str, heading: str) -> str:
    """`### 11.1 …` から次の `### ` までを切り出す。"""
    start = text.find(heading)
    if start < 0:
        return ""
    after = text.find("\n### ", start + len(heading))
    return text[start : after if after >= 0 else len(text)]


def parse() -> tuple[list[tuple[str, str, str]], list[tuple[str, str, str]]]:
    """§11.1 と §11.2 の表を読む（(トークン, dark, light) の並び）。"""
    text = SPEC.read_text(encoding="utf-8")
    base: list[tuple[str, str, str]] = []
    for line in section(text, "### 11.1").splitlines():
        found = ROW.match(line)
        if found:
            base.append((found.group(1), found.group(2), found.group(3)))
    syntax: list[tuple[str, str, str]] = []
    for line in section(text, "### 11.2").splitlines():
        found = ROW.match(line)
        if found:
            syntax.append((found.group(1), found.group(2), found.group(3)))
    return base, syntax


def rust_check(base: list[tuple[str, str, str]]) -> list[str]:
    """第 1 の `theme.rs` と同じ値か（2 系統で色が食い違わないことの確認）。"""
    source = RUST.read_text(encoding="utf-8") if RUST.is_file() else ""
    wanted = {token: (dark, light) for token, dark, light in base}
    errors: list[str] = []
    for field, token in RUST_FIELD.items():
        if token not in wanted:
            continue
        found = re.findall(rf"{field}:\s*hex\(0x([0-9A-Fa-f]{{6}})\)", source)
        if not found:
            errors.append(f"theme-ts: theme.rs に {field} が無い（{token}）")
            continue
        dark, light = wanted[token]
        if len(found) < 2:
            errors.append(f"theme-ts: theme.rs の {field} が dark / light の 2 つでない")
            continue
        if f"#{found[0].upper()}" != dark.upper() or f"#{found[1].upper()}" != light.upper():
            errors.append(
                f"theme-ts: {token} が仕様（{dark} / {light}）と theme.rs"
                f"（#{found[0]} / #{found[1]}）で違う"
            )
    return errors


def css_name(token: str) -> str:
    """トークン名を CSS 変数名にする（`state.unsaved` → `--state-unsaved`）。"""
    return "--" + token.replace(".", "-").replace("_", "-")


def render(
    base: list[tuple[str, str, str]], syntax: list[tuple[str, str, str]]
) -> str:
    lines: list[str] = [HEADER]
    lines.append("/** 配色（§11.4: OS の設定が分からなければ dark）。 */")
    lines.append("export type Mode = 'dark' | 'light';")
    lines.append("")
    lines.append("/** §11.1 のセマンティックカラートークン。 */")
    lines.append("export const COLORS: Record<Mode, Record<string, string>> = {")
    for mode, index in (("dark", 1), ("light", 2)):
        lines.append(f"  {mode}: {{")
        for row in base + syntax:
            lines.append(f"    {json_key(row[0])}: {json_string(row[index])},")
        lines.append("  },")
    lines.append("};")
    lines.append("")
    lines.append("/** `:root` に置く CSS 変数（コンポーネントはこの名前だけを参照する）。 */")
    lines.append("export const CSS_VARIABLES: Record<string, string> = {")
    for row in base + syntax:
        lines.append(f"  {json_key(row[0])}: {json_string(css_name(row[0]))},")
    lines.append("};")
    lines.append("")
    lines.append("/** トークン名を CSS 変数の参照（`var(--bg-base)`）にする。 */")
    lines.append("export function css(token: string): string {")
    lines.append("  return `var(${CSS_VARIABLES[token] ?? token})`;")
    lines.append("}")
    lines.append("")
    lines.append("/** いまの配色を `<html>` の `data-theme` に反映する（§11.4）。 */")
    lines.append("export function applyTheme(mode: Mode): void {")
    lines.append("  const root = document.documentElement;")
    lines.append("  root.dataset.theme = mode;")
    lines.append("  for (const [token, name] of Object.entries(CSS_VARIABLES)) {")
    lines.append("    const value = COLORS[mode][token];")
    lines.append("    if (value) {")
    lines.append("      root.style.setProperty(name, value);")
    lines.append("    }")
    lines.append("  }")
    lines.append("  root.style.colorScheme = mode;")
    lines.append("}")
    lines.append("")
    lines.append("/** OS の設定から配色を決める（`system` のとき。§11.4）。 */")
    lines.append("export function systemMode(): Mode {")
    lines.append("  return typeof matchMedia === 'function' &&")
    lines.append("    matchMedia('(prefers-color-scheme: light)').matches")
    lines.append("    ? 'light'")
    lines.append("    : 'dark';")
    lines.append("}")
    return "\n".join(lines) + "\n"


def json_key(token: str) -> str:
    return "'" + token + "'"


def json_string(text: str) -> str:
    return "'" + text + "'"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="一致するかだけ見る")
    options = parser.parse_args(argv)
    base, syntax = parse()
    if not base:
        print("theme-ts: 仕様 §11.1 の表を読めなかった", file=sys.stderr)
        return 1
    errors = rust_check(base)
    if errors:
        for message in errors:
            print(message, file=sys.stderr)
        return 1
    expected = render(base, syntax)
    if options.check:
        if not TARGET.is_file():
            print(f"theme-ts: {TARGET.relative_to(ROOT)} が無い（python3 scripts/theme-ts.py）", file=sys.stderr)
            return 1
        if TARGET.read_text(encoding="utf-8") != expected:
            print(
                f"theme-ts: {TARGET.relative_to(ROOT)} が仕様と一致しない"
                "（python3 scripts/theme-ts.py で作り直す）",
                file=sys.stderr,
            )
            return 1
        print(f"theme-ts: {len(base) + len(syntax)} トークンが一致")
        return 0
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    TARGET.write_text(expected, encoding="utf-8")
    print(f"theme-ts: {TARGET.relative_to(ROOT)} を書いた（{len(base) + len(syntax)} トークン）")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
