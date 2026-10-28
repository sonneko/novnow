#!/usr/bin/env python3
"""Generate/check the website API reference from the normative catalog table (§19.7).

仕様書 `docs/specifications/interfaces.md` の §19.7 が正本（`tests/api_parity.rs` が実装と突き合わせる）。
説明文は §7.4 の辞書（`src/ui/i18n/{ja,en}.toml` の `api.op.<id>`）から取る。CLI・LSP・MCP と
同じ文言をウェブサイトでも使う（書き分けると片方だけ古くなる）。

    python3 scripts/api-site.py            # website/src/data/api.ts を生成
    python3 scripts/api-site.py --check    # CI: 生成結果と置いてあるファイルが一致するか
"""
import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / "docs/specifications/interfaces.md"
OUT = ROOT / "website/src/data/api.ts"
# 引数は §19.8.2 の `api.list`（ゴールデン）から取る。仕様書の表は引数を持たず、
# `api.list` の `arguments`（JSON Schema + `order`）が正本（§19.8.2）。
CATALOG = ROOT / "tests/golden/api_catalog.json"
DICTIONARIES = {
    "ja": ROOT / "src/ui/i18n/ja.toml",
    "en": ROOT / "src/ui/i18n/en.toml",
}


def catalog_rows(text: str) -> list[dict]:
    """§19.7 の表を読む（`| 正規操作 | 説明・GUI 経路 | LSP 標準メソッド | 例外 |`）。"""
    rows: list[dict] = []
    inside = False
    for line in text.splitlines():
        if line.startswith("### 19.7"):
            inside = True
            continue
        if inside and line.startswith("### "):
            break
        if not inside or not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) < 4:
            continue
        head = cells[0]
        if not head or head == "正規操作" or head.startswith("-"):
            continue
        for op in head.split("/"):
            op = op.strip()
            if op:
                rows.append(
                    dict(
                        id=op,
                        path=cells[1],
                        lsp=cells[2],
                        exceptions=cells[3],
                    )
                )
    return rows


def read_text(target: Path) -> str:
    """UTF-8 as text, without translating newlines.

    `Path.read_text()` uses the locale encoding and universal newlines, so on
    Windows it both mangles the Japanese text and silently absorbs CRLF -- which
    makes `--check` accept a corrupted file (a false negative for drift).
    """
    with target.open("r", encoding="utf-8", newline="") as handle:
        return handle.read()


def write_text(target: Path, text: str) -> None:
    """Write UTF-8 with LF endings, regardless of the OS default."""
    with target.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def descriptions(language: str) -> dict[str, str]:
    """`api.op.<id>` の文言（§7.4 の辞書）。"""
    with DICTIONARIES[language].open("rb") as handle:
        table = tomllib.load(handle)
    # 辞書は `[api.op.<ドメイン>]` の `<動詞>`（`api.op.project.info`）。
    out: dict[str, str] = {}
    for domain, verbs in table.get("api", {}).get("op", {}).items():
        for verb, text in verbs.items():
            out[f"{domain}.{verb}"] = text
    return out


def arguments_by_id() -> dict[str, list[dict]]:
    """`api.list` の引数を、表に出す形（名前・型・必須）に落とす。"""
    catalog = json.loads(read_text(CATALOG))
    out: dict[str, list[dict]] = {}
    for operation in catalog.get("operations", []):
        schema = operation.get("arguments", {})
        properties = schema.get("properties", {})
        required = set(schema.get("required", []))
        order = schema.get("order") or list(properties)
        out[operation["id"]] = [
            dict(
                name=name,
                type=properties.get(name, {}).get("type", ""),
                required=name in required,
            )
            for name in order
            if name in properties
        ]
    return out


# 例に使う見本の値（引数名 → 値）。名前で意味が決まるものだけ具体的にし、残りは型の既定に落とす。
SAMPLES = {
    "scene": "@01-01-01-01",
    "after": "@01-01-01-01",
    "before": "@01-01-01-01",
    "path": "contents/01-01.cnov",
    "text": "本文",
    "query": "リリア",
    "role": "writer",
    "name": "writer",
    "id": "@01-01-01-01",
    "title": "星屑の時計",
    "summary": "要約",
    "key": "theme",
    "value": "dark",
    "workspace": "write",
    "topic": "writing",
    "format": "text",
    "out": "out.txt",
    "to": "writer",
    "from": "writer",
    "body": "本文",
    "level": "episode",
    "chapter": 1,
}


# 操作ごとの上書き（引数名だけでは決まらないもの。ID の接頭辞や、同じ名前でも意味が違うもの）
OP_SAMPLES = {
    "project.new": {"path": "my-novel"},
    "project.recent_remove": {"path": "my-novel"},
    "materials.get": {"id": "M0001"},
    "materials.approve": {"id": "M0001"},
    "materials.reject": {"id": "M0001"},
    "materials.update": {"id": "M0001"},
    "materials.attach": {"id": "M0001"},
    "idea.promote": {"id": "I0001", "target": "materials"},
    "idea.drop": {"id": "I0001"},
    "idea.write": {"id": "I0001"},
    "agent.task_get": {"id": "T0001"},
    "agent.task_run": {"id": "T0001"},
    "agent.task_dry_run": {"id": "T0001"},
    "agent.task_stop": {"id": "T0001"},
    "agent.thread_get": {"id": "T0001"},
    "agent.proposal_get": {"id": "P0001"},
    "agent.proposal_apply": {"id": "P0001"},
    "agent.proposal_reject": {"id": "P0001", "comment": "理由"},
    "agent.delegation_set": {"level": "propose"},
    "document.rename": {"from": "contents/01-01.cnov", "to": "contents/02-01.cnov"},
    "search.replace_file": {"replace": "リオ"},
    "dictionary.register_term": {"term": "魔導"},
    "dictionary.register_character": {"name": "リリア"},
    "session.command": {"name": "view.palette"},
    "lang.rename_apply": {"to": "リオ"},
    "agent.mcp_config_set": {"text": '{ "servers": {} }'},
    "prompt.get": {"name": "write"},
    "prompt.diff": {"name": "write"},
    "prompt.reset": {"name": "write"},
}


# 既存サブコマンドを持つ操作は、その形を例に出す（§19.6.1 の対応表。両方の形を受け付けるが、
# 利用者の手元にあるのは既存形なので、そちらを見せる）
CLI_OVERRIDES = {
    "project.new": "novnow new my-novel",
    "project.validate": "novnow check my-novel",
    "project.import_zip": "novnow import my-novel.zip",
    "export.text": "novnow export my-novel",
}


def sample_value(name: str, kind: str) -> object:
    """引数 1 つの見本の値（例の生成用。決定的）。"""
    if name in SAMPLES:
        found = SAMPLES[name]
        if (kind == "integer" and isinstance(found, int)) or kind == "string":
            return found
    if kind == "integer":
        return 1
    if kind == "number":
        return 1.0
    if kind == "boolean":
        return True
    if kind == "array":
        return []
    if kind == "object":
        return {}
    return f"<{name}>"


def quote(value: object) -> str:
    """CLI の例に置く値（空白を含むものは引用する）。"""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, (list, dict)):
        return "'" + json.dumps(value, ensure_ascii=False) + "'"
    text = str(value)
    return f"'{text}'" if any(ch in text for ch in " <>[]{}&'") else text


def examples_for(row, schema: dict) -> dict:
    """1 操作ぶんの例（CLI・Rust・Python・MCP）。引数は必須のものだけを載せる。"""
    properties = schema.get("properties", {})
    required = list(schema.get("required", []))
    overrides = OP_SAMPLES.get(row["id"], {})
    values = {
        name: overrides.get(name, sample_value(name, properties.get(name, {}).get("type", "string")))
        for name in required
    }
    op = row["id"]
    domain, verb = op.split(".", 1)
    rendered = json.dumps(values, ensure_ascii=False)
    cli = f"novnow {domain} {verb} my-novel"
    for name, value in values.items():
        cli += f" --{name} {quote(value)}"
    if not values:
        cli += " --json"
    if op not in CLI_OVERRIDES:
        for name in values:
            if f"--{name} " not in cli:
                raise SystemExit(f"例に必須の引数が無い: {op} / {name}")
    rust_args = ", ".join(
        f'"{name}": {json.dumps(value, ensure_ascii=False)}' for name, value in values.items()
    )
    python_args = ", ".join(f"{name}={value!r}" for name, value in values.items())
    return {
        "cli": CLI_OVERRIDES.get(op, cli),
        "rust": f'session.run("{op}", json!({{{rust_args}}}))?',
        "python": f"session.{domain}.{verb}({python_args})",
        "mcp": f'tools/call {{"name": "novnow.{op}", "arguments": {rendered}}}',
    }


def schema_by_id() -> dict[str, dict]:
    """`api.list` の `arguments`（JSON Schema）を操作 ID で引けるようにする。"""
    catalog = json.loads(read_text(CATALOG))
    return {
        operation["id"]: operation.get("arguments", {})
        for operation in catalog.get("operations", [])
    }


def returns_by_id() -> dict[str, list[str]]:
    """`api.list` の `returns`（戻り値の形）から、表に出すキー名の並びを作る。"""
    catalog = json.loads(read_text(CATALOG))
    return {
        operation["id"]: list(operation.get("returns", {}).get("properties", {}))
        for operation in catalog.get("operations", [])
    }


def main() -> int:
    rows = catalog_rows(read_text(SPEC))
    if len(rows) != 121 or len({row["id"] for row in rows}) != len(rows):
        raise SystemExit(f"expected 121 unique operations, found {len(rows)}")
    words = {language: descriptions(language) for language in DICTIONARIES}
    for language, table in words.items():
        missing = [row["id"] for row in rows if row["id"] not in table]
        if missing:
            raise SystemExit(f"{language}: missing api.op.* for {missing[:5]} ({len(missing)})")
    args = arguments_by_id()
    returns = returns_by_id()
    schemas = schema_by_id()
    missing = [row["id"] for row in rows if row["id"] not in args or row["id"] not in returns]
    if missing:
        raise SystemExit(f"{CATALOG} に無い操作がある: {missing[:5]}（{len(missing)}）")
    payload = [
        dict(
            id=row["id"],
            domain=row["id"].split(".")[0],
            ja=words["ja"][row["id"]],
            en=words["en"][row["id"]],
            lsp=row["lsp"],
            exceptions=row["exceptions"],
            args=args[row["id"]],
            returns=returns[row["id"]],
            examples=examples_for(row, schemas[row["id"]]),
        )
        for row in rows
    ]
    result = "// Generated by scripts/api-site.py from docs/specifications/interfaces.md §19.7\n"
    result += "// and src/ui/i18n/{ja,en}.toml. Run `python3 scripts/api-site.py --check` to verify.\n"
    result += "export const operations = " + json.dumps(payload, ensure_ascii=False, indent=2) + " as const;\n"
    if "--check" in sys.argv:
        current = read_text(OUT) if OUT.exists() else ""
        if current != result:
            raise SystemExit(f"{OUT} is out of date; run `python3 scripts/api-site.py`")
        print(f"{OUT} covers {len(payload)} operations")
        return 0
    write_text(OUT, result)
    print(f"wrote {OUT} ({len(payload)} operations)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
