#!/usr/bin/env python3
"""`REF:src/ui/i18n/ja.toml` と `en.toml` から `webui/src/lib/i18n/catalog.ts` を作る。

文言の**正本は `REF:src/ui/i18n/*.toml` の 2 つだけ**である（`REF:docs/specifications/ui.md`
§12.3 MUST: 第 2 のために別の文言集を持たない）。第 2 の画面実装はこの生成物を引き、
手で文言を書かない（`REF:docs/plans.md#webui-w3`）。

`{0}` のプレースホルダは**そのまま**残す（差し込みは `src/lib/i18n/index.ts` の `t()`）。
キーの集合は 2 つの TOML で同じでなければならない（片方だけに足すと画面が空になる）。

使い方:

    python3 scripts/i18n-ts.py            # 書き出す
    python3 scripts/i18n-ts.py --check    # 一致するかだけ見る（`scripts/verify.sh` 用）
"""

from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCES = {"ja": ROOT / "src" / "ui" / "i18n" / "ja.toml", "en": ROOT / "src" / "ui" / "i18n" / "en.toml"}
TARGET = ROOT / "webui" / "src" / "lib" / "i18n" / "catalog.ts"

HEADER = """\
// このファイルは **生成物**。手で編集しない（`scripts/i18n-ts.py` が書く）。
//
// 正本は `REF:src/ui/i18n/ja.toml` と `REF:src/ui/i18n/en.toml`（§12.3）。第 1 の画面
// 実装と同じ文言をそのまま持ち、第 2 用の文言集を作らない（§21.2.4 MUST）。
// プレースホルダは `{0}` の形のまま残る（差し込みは `src/lib/i18n/index.ts` の `t()`）。
//
// 変更するときは `python3 scripts/i18n-ts.py` を実行する（`scripts/verify.sh` の
// `webui-i18n` が `--check` で一致を強制する）。
"""


def flatten(prefix: str, value: object, out: dict[str, str]) -> None:
    """TOML の表を `[cli.error] incomplete` → `cli.error.incomplete` に平らにする。"""
    if isinstance(value, dict):
        for key, child in value.items():
            flatten(f"{prefix}.{key}" if prefix else str(key), child, out)
        return
    if not isinstance(value, str):
        raise ValueError(f"{prefix} is not a string")
    out[prefix] = value


def load() -> dict[str, dict[str, str]]:
    catalogs: dict[str, dict[str, str]] = {}
    for locale, path in SOURCES.items():
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        flat: dict[str, str] = {}
        flatten("", data, flat)
        catalogs[locale] = flat
    return catalogs


def check_keys(catalogs: dict[str, dict[str, str]]) -> list[str]:
    """2 つのカタログのキーが同じか（片方だけにあるキーは画面で空文字になる）。"""
    ja = set(catalogs["ja"])
    en = set(catalogs["en"])
    errors = [f"i18n-ts: en.toml に無いキー: {key}" for key in sorted(ja - en)]
    errors += [f"i18n-ts: ja.toml に無いキー: {key}" for key in sorted(en - ja)]
    return errors


def quote(text: str) -> str:
    """TS の単一引用符の文字列にする（`'` と `\\` を逃がす）。"""
    escaped = text.replace("\\", "\\\\").replace("'", "\\'")
    return f"'{escaped}'"


def render(catalogs: dict[str, dict[str, str]]) -> str:
    lines: list[str] = [HEADER]
    lines.append("/** 対応する言語（§12.2）。 */")
    lines.append("export const LOCALES = ['ja', 'en'] as const;")
    lines.append("export type Locale = (typeof LOCALES)[number];")
    lines.append("")
    lines.append("/** 文言カタログ（キー → 文）。プレースホルダは `{0}` の形。 */")
    lines.append("export type Catalog = Record<string, string>;")
    lines.append("")
    lines.append("export const CATALOGS: Record<Locale, Catalog> = {")
    for locale in ("ja", "en"):
        entries = catalogs[locale]
        lines.append(f"  {locale}: {{")
        for key in sorted(entries):
            lines.append(f"    {quote(key)}: {quote(entries[key])},")
        lines.append("  },")
    lines.append("};")
    lines.append("")
    lines.append("/** キーの一覧（`ja` と `en` で同じ。`?` のヘルプとパレットの候補に使う）。 */")
    lines.append("export const KEYS: readonly string[] = Object.keys(CATALOGS.ja);")
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="一致するかだけ見る")
    options = parser.parse_args(argv)
    catalogs = load()
    errors = check_keys(catalogs)
    if errors:
        for message in errors:
            print(message, file=sys.stderr)
        return 1
    expected = render(catalogs)
    if options.check:
        if not TARGET.is_file():
            print(f"i18n-ts: {TARGET.relative_to(ROOT)} が無い（python3 scripts/i18n-ts.py）", file=sys.stderr)
            return 1
        if TARGET.read_text(encoding="utf-8") != expected:
            print(
                f"i18n-ts: {TARGET.relative_to(ROOT)} が TOML と一致しない"
                "（python3 scripts/i18n-ts.py で作り直す）",
                file=sys.stderr,
            )
            return 1
        print(f"i18n-ts: {len(catalogs['ja'])} キーが一致")
        return 0
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    TARGET.write_text(expected, encoding="utf-8")
    print(f"i18n-ts: {TARGET.relative_to(ROOT)} を書いた（{len(catalogs['ja'])} キー）")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
