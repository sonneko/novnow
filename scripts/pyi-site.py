#!/usr/bin/env python3
"""Generate/check the Python type stub from the normative operation catalog.

`python/novnow-py/python/novnow/__init__.pyi` は §19.3.1 の型スタブ。§19.7 の 120 操作と
`arguments.order`（位置引数の束縛順）から**生成する**（手書きの複製を置かない＝ §19.5.3 と
同じ原則）。入力は `tests/golden/api_catalog.json`（`cargo run --locked --example api_catalog`
の出力。`REF:tests/api_parity.rs` が仕様書の表と突き合わせる）。

    python3 scripts/pyi-site.py            # __init__.pyi を生成
    python3 scripts/pyi-site.py --check    # CI: 生成結果と置いてあるファイルが一致するか
"""

from __future__ import annotations

import ast
import json
import keyword
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOG = ROOT / "tests/golden/api_catalog.json"
OUT = ROOT / "python/novnow-py/python/novnow/__init__.pyi"
IMPLEMENTATION = ROOT / "python/novnow-py/python/novnow/__init__.py"

# §19.1.7 の 9 分類 → §19.3.3 の例外クラス名。`REF:src/app/error.rs` の `Class::as_str` と
# `REF:python/novnow-py/python/novnow/__init__.py` の `TYPES` が正本。
ERRORS = [
    ("usage", "UsageError", "引数の形・型の誤り"),
    ("project", "ProjectError", "プロジェクト構造の不正"),
    ("io", "IOError", "ファイルシステムのエラー"),
    ("validation", "ValidationError", "内容の検証エラー"),
    ("policy", "PolicyError", "権限・関門の拒否（E036 など）"),
    ("not_found", "NotFoundError", "対象 ID の不存在"),
    ("conflict", "ConflictError", "保存状態機の衝突"),
    ("timeout", "TimeoutError", "LLM 応答・処理の時間切れ"),
    ("cancelled", "CancelledError", "利用者が操作を中断した"),
]

# JSON Schema の `type`（`REF:src/app/ops/mod.rs` の `ArgDef::kind`）→ Python の型注釈。
PYTHON_TYPES = {
    "string": "str",
    "integer": "int",
    "number": "float",
    "boolean": "bool",
    "array": "list[Any]",
    "object": "dict[str, Any]",
}


def read_text(target: Path) -> str:
    """UTF-8 as text, without translating newlines (see scripts/api-site.py)."""
    with target.open("r", encoding="utf-8", newline="") as handle:
        return handle.read()


def write_text(target: Path, text: str) -> None:
    """Write UTF-8 with LF endings, regardless of the OS default."""
    with target.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def parameter(name: str) -> str:
    """Python の引数名（予約語と衝突するときは PEP 8 どおり末尾に `_`）。"""
    return f"{name}_" if keyword.iskeyword(name) else name


def python_type(name: str, kind: str) -> str:
    """引数名と JSON Schema の型から Python の型注釈を作る。"""
    if name == "bytes":
        # Zip の導入・書き出しは Python 側で base64 へ変換する（§19.3.2）
        return "bytes | str"
    return PYTHON_TYPES.get(kind, "Any")


def signature(operation: dict) -> str:
    """`def verb(self, a: str, b: int = ...) -> Any` の形（`arguments.order` が束縛順）。

    任意引数の後ろに必須引数が来ることがある（例 `plot.write` は
    `level, text, scene = ..., chapter = ...` の後に**必須**の `role`）。Python では
    既定値の後ろに既定値の無い位置引数を置けないので、そこで keyword-only（`*`）に切る。
    実行時（`REF:python/novnow-py/python/novnow/__init__.py` の `run`）は位置でも受け付けるが、
    `role` は明示的に書くほうが §19.1.5 の意図に合う。
    """
    arguments = operation["arguments"]
    order = list(arguments.get("order") or arguments.get("properties", {}).keys())
    properties = arguments.get("properties", {})
    required = set(arguments.get("required") or [])
    parts = ["self"]
    defaulted = False
    keyword_only = False
    for name in order:
        kind = properties.get(name, {}).get("type", "string")
        annotation = python_type(name, kind)
        shown = parameter(name)
        if name in required:
            if defaulted and not keyword_only:
                parts.append("*")
                keyword_only = True
            parts.append(f"{shown}: {annotation}")
        else:
            defaulted = True
            parts.append(f"{shown}: {annotation} = ...")
    return ", ".join(parts)


def docstring(operation: dict) -> str:
    # 一行 docstring: 説明（§7.4 の辞書）＋操作 ID ＋ 関門・例外の注記。
    text = operation["description"].replace('"', "'").replace("\n", " ")
    notes = []
    if operation.get("gate"):
        notes.append("MCP からは `role` が必要" if not operation.get("gate_arg") else "書き込みのときだけ `role` が必要")
    for column, reason in sorted((operation.get("exceptions") or {}).items()):
        notes.append(f"{column}—（{reason}）")
    renamed = [
        name
        for name in (operation["arguments"].get("order") or [])
        if keyword.iskeyword(name)
    ]
    if renamed:
        notes.append(
            "実引数名は " + "・".join(f"`{name}`" for name in renamed)
            + "（Python の予約語なので型スタブでは `_` を付ける）"
        )
    suffix = f"　[{'; '.join(notes)}]" if notes else ""
    return f'"""{text}（`{operation["id"]}`）{suffix}"""'


def stub(catalog: dict) -> str:
    """型スタブ全文。"""
    operations = catalog["operations"]
    domains: dict[str, list[dict]] = {}
    for operation in operations:
        domains.setdefault(operation["domain"], []).append(operation)

    out: list[str] = []
    out.append('"""novnow の型スタブ（§19.3.1。`py.typed` と一緒に wheel へ同梱する）。')
    out.append("")
    out.append("**生成物**: `python3 scripts/pyi-site.py` が `tests/golden/api_catalog.json`")
    out.append(f"（§19.7 の {catalog['count']} 操作・{len(domains)} ドメイン）から作る。手で編集しないこと。")
    out.append("位置引数の型と並びは `arguments.order`（宣言順）どおり。")
    out.append('"""')
    out.append("")
    out.append("from typing import Any, Callable, Iterator, Protocol")
    out.append("")
    out.append("__version__: str")
    out.append("")
    out.append("")
    out.append("class Record(dict):")
    out.append('    """JSON と同じ構造の `dict`。キーは属性としても読める（§19.3.2）。"""')
    out.append("")
    out.append("    def __getattr__(self, name: str) -> Any: ...")
    out.append("    def __dir__(self) -> list[str]: ...")
    out.append("")
    out.append("")
    out.append("class NovnowError(Exception):")
    out.append('    """すべての novnow エラーの親（§19.3.3）。"""')
    out.append("")
    out.append("    class_: str")
    out.append("    code: str")
    out.append("    message: str")
    out.append("    path: str | None")
    out.append("    exit_code: int")
    for _, name, meaning in ERRORS:
        out.append("")
        out.append("")
        out.append(f"class {name}(NovnowError):")
        out.append(f'    """{meaning}（§19.1.7）。"""')
    out.append("")
    out.append("")
    for domain in sorted(domains):
        out.append(f"class {domain}(Protocol):")
        out.append(f'    """`session.{domain}` のドメイン（§19.3.2）。"""')
        out.append("")
        for operation in domains[domain]:
            verb = operation["verb"]
            out.append(f"    def {verb}({signature(operation)}) -> Any:")
            out.append(f"        {docstring(operation)}")
            out.append("")
        out.append("")
    out.append("class Session:")
    out.append('    """開いたプロジェクト（§19.3.2）。コンテキストマネージャ。"""')
    out.append("")
    out.append("    def __init__(self, path: str) -> None: ...")
    out.append("    def run(self, op: str, *args: Any, **kwargs: Any) -> Any:")
    out.append('        """正規操作を 1 つ実行する。位置引数は `arguments.order` の順。"""')
    out.append("    def command(self, name: str) -> Any: ...")
    out.append("    def poll_events(self) -> list[dict[str, Any]]: ...")
    out.append("    def on_event(self, callback: Callable[[dict[str, Any]], None]) -> None: ...")
    out.append("    def cancel(self, handle: str) -> Any: ...")
    out.append("    def set_locale(self, locale: str) -> None: ...")
    out.append("    @property")
    out.append("    def root(self) -> str | None: ...")
    out.append("    def close(self) -> None: ...")
    out.append("    def __getattr__(self, domain: str) -> Any: ...")
    out.append("    def __enter__(self) -> Session: ...")
    out.append("    def __exit__(self, *exc_info: Any) -> bool: ...")
    out.append("    def __iter__(self) -> Iterator[str]: ...")
    for domain in sorted(domains):
        out.append("    @property")
        out.append(f"    def {domain}(self) -> {domain}: ...")
    out.append("")
    out.append("")
    out.append("def open(path: str) -> Session:")
    out.append('    """既存のプロジェクトを開く（§19.3.2）。"""')
    out.append("def create(path: str, title: str | None = ...) -> Session:")
    out.append('    """新しいプロジェクトを作って開く（`project.new`）。"""')
    out.append("def operations() -> list[dict[str, Any]]:")
    out.append('    """正規操作カタログ（§19.7。`api.list` と同じ形）。"""')
    return "\n".join(out) + "\n"


def exported_names() -> list[str]:
    """`__init__.py` の `__all__`（実装が公開する名前）。"""
    tree = ast.parse(read_text(IMPLEMENTATION))
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "__all__" for target in node.targets
        ):
            return [element.value for element in node.value.elts]
    raise SystemExit(f"{IMPLEMENTATION.relative_to(ROOT)}: __all__ が見つからない")


def main() -> int:
    catalog = json.loads(read_text(CATALOG))
    operations = catalog["operations"]
    if len(operations) != catalog.get("count") or not operations:
        raise SystemExit(f"{CATALOG.name}: count と operations が合わない")
    # `arguments.order` が無いと位置引数の型を出せないので、生成前に止める
    missing = [op["id"] for op in operations if not op["arguments"].get("order") and op["arguments"].get("properties")]
    if missing:
        raise SystemExit(f"arguments.order が無い: {missing[:5]} ({len(missing)})。カタログを作り直すこと")
    result = stub(catalog)
    # 実装が公開する名前（`__all__`）が全部スタブに出るか（片方だけ増える事故を防ぐ）
    missing = [name for name in exported_names() if f"{name}" not in result]
    if missing:
        raise SystemExit(f"{OUT.relative_to(ROOT)}: __all__ の名前が無い: {missing}")
    # 生成したスタブが Python として構文的に正しいこと（mypy を回せない環境でも止まるように）
    try:
        compile(result, str(OUT.relative_to(ROOT)), "exec")
    except SyntaxError as error:
        raise SystemExit(f"{OUT.relative_to(ROOT)}: generated stub is not valid Python: {error}")
    if "--check" in sys.argv:
        current = read_text(OUT) if OUT.exists() else ""
        if current != result:
            raise SystemExit(f"{OUT} is out of date; run `python3 scripts/pyi-site.py`")
        print(f"{OUT.relative_to(ROOT)} covers {len(operations)} operations")
        return 0
    write_text(OUT, result)
    print(f"wrote {OUT.relative_to(ROOT)} ({len(operations)} operations)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
