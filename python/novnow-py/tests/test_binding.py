"""Python 束縛（§19.3.2）の検査。**ネイティブ拡張をビルドせずに**走る。

`novnow._core` を偽物に差し替えて `python/novnow/__init__.py` のロジックだけを検査するので、
`maturin build` が要らない（CI で Rust なしでも回せる）。カタログは正本の
`tests/golden/api_catalog.json`（`REF:tests/api_parity.rs` が仕様書の表と突き合わせる）。

    python3 -m unittest discover -s python/novnow-py/tests -t python/novnow-py/tests
    pytest python/novnow-py/tests/test_binding.py      # pytest でも走る
"""

from __future__ import annotations

import base64
import importlib.util
import json
import sys
import types
import unittest
from pathlib import Path

TESTS = Path(__file__).resolve().parent
PACKAGE_PARENT = TESTS.parent / "python"
CATALOG = TESTS.parent.parent.parent / "tests/golden/api_catalog.json"


def load_catalog() -> dict:
    with CATALOG.open("r", encoding="utf-8", newline="") as handle:
        return json.load(handle)


class FakeCoreError(Exception):
    """`_core.NovnowError` の偽物（属性の形は §19.3.3）。"""

    def __init__(self, message: str, **attributes):
        super().__init__(message)
        self.class_ = attributes.get("class_", "")
        self.code = attributes.get("code", "")
        self.message = message
        self.path = attributes.get("path", None)
        self.exit_code = attributes.get("exit_code", 0)

    def __getattr__(self, name):
        # `__init__.py` は `getattr(error, "class", "")` で分類を読む
        if name == "class":
            return self.class_
        raise AttributeError(name)


class FakeSession:
    """`_core` のセッション偽物。呼び出しを記録するだけ。"""

    def __init__(self, path):
        self.path = path
        self.calls = []
        self.locale = "ja"

    def run(self, op, arguments):
        self.calls.append((op, dict(arguments)))
        return {"op": op, "arguments": dict(arguments)}

    def command(self, name):
        self.calls.append(("command", name))
        return {"command": name}

    def poll_events(self, limit=None):
        events = list(getattr(self, "events", []))
        if limit is None:
            self.events = []
            return events
        self.events = events[limit:]
        return events[:limit]

    def on_event(self, callback):
        self.callback = callback

    def cancel(self, handle):
        return {"cancelled": handle}

    def set_locale(self, locale):
        self.locale = locale

    def root(self):
        return self.path

    def close(self):
        self.closed = True


def real_core_available() -> bool:
    """本物の `novnow._core`（maturin でビルドした拡張）が入っているか。"""
    try:
        return importlib.util.find_spec("novnow._core") is not None
    except (ImportError, ValueError):
        return False


# wheel が入っている環境（`.github/workflows/wheels.yml` の test ジョブ）では
# `sys.modules` を汚さず、`test_api.py` が本物の拡張を検査する。
if real_core_available():
    raise unittest.SkipTest(
        "novnow._core がビルド済み。束縛の検査は test_api.py が本物で行う"
    )


def install_fake_core() -> types.ModuleType:
    """`novnow._core` の偽物を `sys.modules` へ入れて `novnow` を import する。"""
    core = types.ModuleType("novnow._core")
    core.__version__ = "0.1.0-test"
    core.NovnowError = FakeCoreError
    core.OPERATIONS = load_catalog()
    created = []

    def open_(path):
        session = FakeSession(path)
        created.append(session)
        return session

    core.open = open_
    core.create = lambda path, title=None: open_(path)
    core.created = created
    sys.modules["novnow._core"] = core
    sys.path.insert(0, str(PACKAGE_PARENT))
    return core


CORE = install_fake_core()
import novnow  # noqa: E402  - 偽物の `_core` を入れた後でないと import できない


def new_session() -> "novnow.Session":
    return novnow.Session("novel")


def last_arguments() -> dict:
    return CORE.created[-1].calls[-1][1]


class ArgumentBinding(unittest.TestCase):
    """§19.3.2 の位置引数は**宣言順**（`arguments.order`）で束縛する。"""

    def test_positional_arguments_follow_the_declaration_order(self):
        with new_session() as session:
            # `section.write` の宣言順は scene, text, role。JSON の `properties` は
            # アルファベット順（role, scene, text）なので、そちらを使うとずれる
            session.section.write("@01", "新しい本文")
        self.assertEqual(
            last_arguments(),
            {"scene": "@01", "text": "新しい本文"},
            "位置引数が宣言順に束縛されていない",
        )

    def test_named_and_positional_arguments_agree(self):
        with new_session() as session:
            session.section.write(scene="@01", text="新しい本文")
        named = last_arguments()
        with new_session() as session:
            session.section.write("@01", "新しい本文")
        self.assertEqual(named, last_arguments())

    def test_every_operation_binds_its_declared_order(self):
        """121 操作すべてで「位置引数＝宣言順」を確認する（網羅）。"""
        catalog = load_catalog()["operations"]
        self.assertEqual(len(catalog), 121, "§19.7 の全行")
        for operation in catalog:
            order = operation["arguments"].get("order") or []
            if not order:
                continue
            with self.subTest(op=operation["id"]):
                with new_session() as session:
                    session.run(operation["id"], *order)
                self.assertEqual(list(last_arguments()), order)

    def test_too_many_positional_arguments_are_a_type_error(self):
        with new_session() as session:
            with self.assertRaises(TypeError) as caught:
                session.section.write("@01", "本文", "writer", "余分")
        self.assertIn("section.write", str(caught.exception))

    def test_bytes_arguments_are_base64_encoded(self):
        expected = base64.b64encode(b"PK\x03\x04").decode("ascii")
        # キーワード引数
        with new_session() as session:
            session.project.import_zip(bytes=b"PK\x03\x04")
        self.assertEqual(last_arguments()["bytes"], expected)
        # 宣言順（path, bytes）の位置引数
        with new_session() as session:
            session.project.import_zip(None, b"PK\x03\x04")
        self.assertEqual(last_arguments()["bytes"], expected)
        # 単一位置引数（bytes のみ）
        with new_session() as session:
            session.project.import_zip(b"PK\x03\x04")
        self.assertEqual(last_arguments(), {"bytes": expected})

    def test_export_zip_decodes_base64_bytes_to_python_bytes(self):
        payload = b"PK\x03\x04archive-bytes"
        encoded = base64.b64encode(payload).decode("ascii")
        with new_session() as session:
            session._session.run = lambda op, arguments: {
                "bytes": encoded,
                "name": "novel.zip",
                "size": len(payload),
            }
            exported = session.project.export_zip()
        self.assertIsInstance(exported["bytes"], bytes)
        self.assertIsInstance(exported.bytes, bytes)
        self.assertEqual(exported.bytes, payload)
        self.assertEqual(exported.size, len(payload))

    def test_domain_calls_reach_the_canonical_operation_id(self):
        with new_session() as session:
            session.structure.tree()
            session.export.text()
        self.assertEqual(
            [call[0] for call in CORE.created[-1].calls],
            ["structure.tree", "export.text"],
        )


class Record(unittest.TestCase):
    """結果は辞書の派生で、キーは属性としても読める（§19.3.2）。"""

    def test_results_are_dicts_with_attribute_access(self):
        with new_session() as session:
            result = session.project.info()
        self.assertIsInstance(result, dict)
        self.assertEqual(result["op"], "project.info")
        self.assertEqual(result.op, "project.info")
        self.assertEqual(result.arguments, result["arguments"])
        self.assertIsInstance(result.arguments, dict)

    def test_a_missing_key_raises_attribute_error(self):
        with new_session() as session:
            result = session.project.info()
        with self.assertRaises(AttributeError):
            _ = result.nope

    def test_lists_of_records_are_converted_too(self):
        with new_session() as session:
            events = session.poll_events()
        self.assertEqual(events, [])

    def test_poll_events_honors_limit_in_fifo_order(self):
        with new_session() as session:
            session._session.events = [
                {"kind": "saved", "seq": 1},
                {"kind": "saved", "seq": 2},
                {"kind": "saved", "seq": 3},
            ]
            first = session.poll_events(2)
            second = session.poll_events(2)
            third = session.poll_events(2)
            with self.assertRaises(novnow.UsageError):
                session.poll_events(-1)
        self.assertEqual([e.seq for e in first], [1, 2])
        self.assertEqual([e.seq for e in second], [3])
        self.assertEqual(third, [])


class ErrorClassification(unittest.TestCase):
    """§19.3.3 の 9 例外へ写る。"""

    def test_the_nine_classes_are_exported_and_mapped(self):
        for name in (
            "UsageError",
            "ProjectError",
            "IOError",
            "ValidationError",
            "PolicyError",
            "NotFoundError",
            "ConflictError",
            "TimeoutError",
            "CancelledError",
        ):
            self.assertTrue(issubclass(getattr(novnow, name), novnow.NovnowError), name)
        self.assertEqual(len(novnow.TYPES), 9, "§19.1.7 の 9 分類")

    def test_a_core_error_is_retyped_by_its_class(self):
        table = [
            ("usage", novnow.UsageError, "E001", 2),
            ("project", novnow.ProjectError, "S021", 3),
            ("io", novnow.IOError, "S030", 4),
            ("validation", novnow.ValidationError, "E010", 5),
            ("policy", novnow.PolicyError, "E036", 5),
            ("not_found", novnow.NotFoundError, "", 3),
            ("conflict", novnow.ConflictError, "", 4),
            ("timeout", novnow.TimeoutError, "", 4),
            ("cancelled", novnow.CancelledError, "", 0),
        ]
        for class_name, exc_type, kind, exit_code in table:
            with self.subTest(class_name=class_name):
                with new_session() as session:
                    session._session.run = lambda op, arguments, c=class_name, k=kind, e=exit_code: (
                        _ for _ in ()
                    ).throw(
                        FakeCoreError(
                            f"msg-{c}",
                            class_=c,
                            code=k,
                            path="contents/01-01.cnov",
                            exit_code=e,
                        )
                    )
                    with self.assertRaises(exc_type) as caught:
                        session.section.write("@01", "本文")
                self.assertEqual(caught.exception.class_, class_name)
                self.assertEqual(caught.exception.code, kind)
                self.assertEqual(caught.exception.exit_code, exit_code)
                self.assertEqual(caught.exception.path, "contents/01-01.cnov")


class ModuleSurface(unittest.TestCase):
    """公開面（`__all__`）と型スタブ（§19.3.1）が揃っている。"""

    def test_every_name_in_all_exists(self):
        for name in novnow.__all__:
            self.assertTrue(hasattr(novnow, name), f"{name} が無い")

    def test_the_stub_covers_every_operation_of_the_catalog(self):
        stub = (PACKAGE_PARENT / "novnow" / "__init__.pyi").read_text(encoding="utf-8")
        for operation in load_catalog()["operations"]:
            domain, verb = operation["id"].split(".", 1)
            self.assertIn(f"class {domain}(Protocol):", stub, operation["id"])
            self.assertIn(f"    def {verb}(", stub, operation["id"])
        self.assertTrue(
            (PACKAGE_PARENT / "novnow" / "py.typed").exists(),
            "py.typed が無い（PEP 561）",
        )


if __name__ == "__main__":
    unittest.main()
