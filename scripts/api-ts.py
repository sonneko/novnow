#!/usr/bin/env python3
"""`tests/golden/api_catalog.json`（§19.7 の正本）から `webui/src/lib/bridge/catalog.ts` を作る。

第 1 の画面実装・LSP・MCP・Python・CLI・第 2 の画面実装は、**同じ 1 つのカタログ**を使う
（`REF:docs/specifications/interfaces.md` §19.7）。第 2 の TS 側は、操作 ID を手書きせず
この生成物を参照する（`REF:docs/plans.md#webui-parallel` §11.11.5「インターフェースの凍結」）。

使い方:

    python3 scripts/api-ts.py            # 書き出す
    python3 scripts/api-ts.py --check    # 一致するかだけ見る（`scripts/verify.sh` 用）

`--check` は差分があれば終了コード 1 を返す。生成物を手で直した状態・カタログを変えて
生成し忘れた状態のどちらも落ちる（`scripts/diagnostics-site.py --check` と同じ考え方）。
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "tests" / "golden" / "api_catalog.json"
TARGET = ROOT / "webui" / "src" / "lib" / "bridge" / "catalog.ts"

HEADER = """\
// このファイルは **生成物**。手で編集しない（`scripts/api-ts.py` が書く）。
//
// 正本は `REF:tests/golden/api_catalog.json`（§19.7）。第 1 の画面実装・LSP・MCP・Python・
// CLI と同じカタログから作るので、操作 ID の綴りと引数の形が 2 系統でずれない
// （`REF:docs/plans.md#webui-w3`・`REF:docs/designs/webui.md` §21.5）。
//
// 変更するときは `python3 scripts/api-ts.py` を実行する（`scripts/verify.sh` の
// `webui-api` が `--check` で一致を強制する）。
"""


def ts_type(schema: Any, depth: int = 0, optional: bool = False) -> str:
    """JSON Schema の小さな部分集合を TS の型にする（カタログの範囲だけを扱う）。

    `optional` は「この object のプロパティを省略可能にするか」。引数はカタログの
    `required` に従うが、**戻り値のプロパティは常に全部ある**（`src/**` の `json!` が
    すべてのキーを書く）ので、戻り値は必須として型にする。
    """
    if not schema:
        return "unknown"
    if isinstance(schema, list):
        return " | ".join(ts_type(item, depth, optional) for item in schema)
    if not isinstance(schema, dict):
        return "unknown"
    kind = schema.get("type")
    if isinstance(kind, list):
        return " | ".join(ts_type({**schema, "type": item}, depth, optional) for item in kind)
    if kind == "string":
        return "string"
    if kind in ("integer", "number"):
        return "number"
    if kind == "boolean":
        return "boolean"
    if kind == "null":
        return "null"
    if kind == "array":
        return f"{ts_type(schema.get('items', {}), depth, True)}[]"
    if kind == "object":
        properties = schema.get("properties") or {}
        if not properties:
            return "Record<string, unknown>"
        required = set(schema.get("required") or [])
        indent = "  " * (depth + 1)
        rows = []
        for name, found in properties.items():
            mark = "?" if optional and name not in required else ""
            rows.append(f"{indent}{json.dumps(name)}{mark}: {ts_type(found, depth + 1, True)};")
        return "{\n" + "\n".join(rows) + "\n" + "  " * depth + "}"
    return "unknown"


def render(catalog: dict[str, Any]) -> str:
    """カタログから TS の全文を作る（決定論。同じ入力なら同じバイト列）。"""
    operations = catalog["operations"]
    lines: list[str] = [HEADER]
    lines.append(f"export const CATALOG_VERSION = {json.dumps(catalog['catalog_version'])};")
    lines.append(f"export const PRODUCT_VERSION = {json.dumps(catalog['version'])};")
    lines.append("/** カタログの操作 ID の全部（件数は `OP_IDS.length` で数える）。 */")
    lines.append("export const OP_IDS = [")
    for found in operations:
        lines.append(f"  {json.dumps(found['id'])}, // {found['description']}")
    lines.append("] as const;")
    lines.append("")
    lines.append("/** 操作 ID の型（`run(op, args)` の第 1 引数）。 */")
    lines.append("export type OpId = (typeof OP_IDS)[number];")
    lines.append("")
    lines.append("/** 操作 1 つの引数の形（`run` の第 2 引数）。 */")
    lines.append("export interface OpArgs {")
    for found in operations:
        properties = found["arguments"].get("properties") or {}
        required = set(found["arguments"].get("required") or [])
        if not properties:
            lines.append(f"  {json.dumps(found['id'])}: Record<string, never>;")
            continue
        lines.append(f"  {json.dumps(found['id'])}: {{")
        for name, schema in properties.items():
            mark = "" if name in required else "?"
            lines.append(f"    {json.dumps(name)}{mark}: {ts_type(schema, 2, True)};")
        lines.append("  };")
    lines.append("}")
    lines.append("")
    lines.append("/** 操作 1 つの戻り値の形（`run` の `ok: true` のときの `result`）。 */")
    lines.append("export interface OpResult {")
    for found in operations:
        lines.append(f"  {json.dumps(found['id'])}: {ts_type(found.get('returns'), 1, False)};")
    lines.append("}")
    lines.append("")
    lines.append("/** 画面から呼べる操作か（関門付きでも `Interface::Rust` なので `role` は要らない）。 */")
    lines.append("export function isOp(id: string): id is OpId {")
    lines.append("  return (OP_IDS as readonly string[]).includes(id);")
    lines.append("}")
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="一致するかだけ見る")
    options = parser.parse_args(argv)
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    expected = render(catalog)
    if options.check:
        if not TARGET.is_file():
            print(f"api-ts: {TARGET.relative_to(ROOT)} が無い（python3 scripts/api-ts.py）", file=sys.stderr)
            return 1
        found = TARGET.read_text(encoding="utf-8")
        if found != expected:
            print(
                f"api-ts: {TARGET.relative_to(ROOT)} がカタログと一致しない"
                "（python3 scripts/api-ts.py で作り直す）",
                file=sys.stderr,
            )
            return 1
        print(f"api-ts: {len(catalog['operations'])} 操作が一致")
        return 0
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    TARGET.write_text(expected, encoding="utf-8")
    print(f"api-ts: {TARGET.relative_to(ROOT)} を書いた（{len(catalog['operations'])} 操作）")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
