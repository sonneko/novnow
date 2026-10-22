"""novnow — 長編小説・シナリオ執筆 IDE の Python API（§19.3）。

ネイティブ拡張 `novnow._core` が Rust と同じ機能核（§19.2）をインプロセスで呼ぶ。
このファイルは §19.3.2 の表（`Session` のコンテキストマネージャ、`session.project.info()`
の形のドメイン呼び出し）と §19.3.3 の 9 例外を組むだけ。

    import novnow

    with novnow.Session("my-novel") as session:
        info = session.project.info()
        session.section.write("@03:01", "新しい本文\\n")
        session.save.now()
"""

from __future__ import annotations

import base64
from typing import Any, Callable, Iterator

from . import _core

__version__ = _core.__version__
__all__ = [
    "NovnowError",
    "UsageError",
    "ProjectError",
    "IOError",
    "ValidationError",
    "PolicyError",
    "NotFoundError",
    "ConflictError",
    "TimeoutError",
    "CancelledError",
    "Session",
    "open",
    "create",
    "operations",
    "__version__",
]


class NovnowError(_core.NovnowError):
    """すべての novnow エラーの親（§19.3.3）。属性: class / code / message / path。"""

    class_: str = ""
    code: str = ""
    message: str = ""
    path: str | None = None
    exit_code: int = 0


class UsageError(NovnowError):
    """引数の形・型の誤り（§19.1.7 `usage`）。"""


class ProjectError(NovnowError):
    """プロジェクト構造の不正（`project`）。"""


class IOError(NovnowError):  # noqa: A001 - §19.3.3 がこの名前を指定する
    """ファイルシステムのエラー（`io`）。"""


class ValidationError(NovnowError):
    """内容の検証エラー（`validation`）。"""


class PolicyError(NovnowError):
    """権限・関門の拒否（`policy`。E036 など）。"""


class NotFoundError(NovnowError):
    """対象 ID の不存在（`not_found`）。"""


class ConflictError(NovnowError):
    """保存状態機の衝突（`conflict`）。"""


class TimeoutError(NovnowError):  # noqa: A001 - §19.3.3 がこの名前を指定する
    """LLM 応答・処理の時間切れ（`timeout`）。"""


class CancelledError(NovnowError):  # noqa: A001 - §19.3.3 がこの名前を指定する
    """利用者が操作を中断した（`cancelled`）。"""


TYPES: dict[str, type[NovnowError]] = {
    "usage": UsageError,
    "project": ProjectError,
    "io": IOError,
    "validation": ValidationError,
    "policy": PolicyError,
    "not_found": NotFoundError,
    "conflict": ConflictError,
    "timeout": TimeoutError,
    "cancelled": CancelledError,
}


def _retype(error: _core.NovnowError) -> NovnowError:
    """`_core.NovnowError` を §19.1.7 の分類ごとの型に写す。"""
    kind = TYPES.get(getattr(error, "class", "") or "", NovnowError)
    typed = kind(getattr(error, "message", "") or str(error))
    typed.class_ = getattr(error, "class", "") or ""
    typed.code = getattr(error, "code", "") or ""
    typed.message = getattr(error, "message", "") or str(error)
    typed.path = getattr(error, "path", None)
    typed.exit_code = getattr(error, "exit_code", 0)
    return typed


def _arguments(kwargs: dict[str, Any]) -> dict[str, Any]:
    """`bytes` の引数は base64 文字列にする（Zip 導入・書き出し。§19.3.2）。"""
    return {
        key: base64.b64encode(value).decode("ascii") if isinstance(value, (bytes, bytearray)) else value
        for key, value in kwargs.items()
    }


class _Domain:
    """`session.project` のようなドメイン（動詞を呼ぶと正規操作になる）。"""

    __slots__ = ("_session", "_domain")

    def __init__(self, session: "Session", domain: str) -> None:
        self._session = session
        self._domain = domain

    def __getattr__(self, verb: str) -> Callable[..., Any]:
        if verb.startswith("_"):
            raise AttributeError(verb)
        return lambda *args, **kwargs: self._session.run(f"{self._domain}.{verb}", *args, **kwargs)

    def __dir__(self) -> list[str]:
        prefix = f"{self._domain}."
        return sorted(
            operation["id"][len(prefix):].replace(".", "_")
            for operation in operations()
            if operation["id"].startswith(prefix)
        )


class Session:
    """開いたプロジェクト（§19.3.2）。コンテキストマネージャ。"""

    def __init__(self, path: str) -> None:
        self._session = _core.open(path)

    # -- 正規操作（§19.7） ------------------------------------------------
    def run(self, op: str, *args: Any, **kwargs: Any) -> Any:
        """正規操作を 1 つ実行する。位置引数は宣言順の引数、キーワード引数は名前指定。"""
        arguments = _arguments(kwargs)
        names = _argument_names(op)
        if len(args) > len(names):
            raise TypeError(
                f"{op}() takes {len(names)} positional argument(s) "
                f"({', '.join(names) or 'none'}) but {len(args)} were given"
            )
        for index, value in enumerate(args):
            arguments[names[index]] = value
        try:
            return self._session.run(op, arguments)
        except _core.NovnowError as error:
            raise _retype(error) from None

    def command(self, name: str) -> Any:
        """パレットのコマンドを実行する（§19.2.3）。"""
        try:
            return self._session.command(name)
        except _core.NovnowError as error:
            raise _retype(error) from None

    # -- イベントと中断（§19.3.5） ---------------------------------------
    def poll_events(self) -> list[dict[str, Any]]:
        return list(self._session.poll_events())

    def on_event(self, callback: Callable[[dict[str, Any]], None]) -> None:
        self._session.on_event(callback)

    def cancel(self, handle: str) -> Any:
        try:
            return self._session.cancel(handle)
        except _core.NovnowError as error:
            raise _retype(error) from None

    def set_locale(self, locale: str) -> None:
        """文言の言語（§19.1.8。`ja` / `en`）。"""
        self._session.set_locale(locale)

    @property
    def root(self) -> str | None:
        return self._session.root()

    def close(self) -> None:
        self._session.close()

    def __getattr__(self, domain: str) -> _Domain:
        if domain.startswith("_"):
            raise AttributeError(domain)
        return _Domain(self, domain)

    def __enter__(self) -> "Session":
        return self

    def __exit__(self, *exc_info: Any) -> bool:
        self.close()
        return False

    def __iter__(self) -> Iterator[str]:
        return iter(_domain_names())


def _domain_names() -> list[str]:
    seen: list[str] = []
    for operation in operations():
        if operation["domain"] not in seen:
            seen.append(operation["domain"])
    return seen


def _argument_names(op: str) -> list[str]:
    """位置引数の束縛順（§19.3.2）。

    カタログの `arguments.order`（宣言順）を使う。`properties` は JSON オブジェクトなので
    アルファベット順に並んでおり、こちらを使うと 31 操作で束縛がずれる
    （例 `section.write(scene, text)` が `role`・`scene` に入る）。
    """
    for operation in operations():
        if operation["id"] == op:
            order = operation["arguments"].get("order")
            if order:
                return list(order)
            return list(operation["arguments"].get("properties", {}).keys())
    return []


def open(path: str) -> Session:  # noqa: A001 - §19.3.2 がこの名前を指定する
    """既存のプロジェクトを開く。"""
    return Session(path)


def create(path: str, title: str | None = None) -> Session:
    """新しいプロジェクトを作って開く（`project.new`）。"""
    # 作ったプロジェクトをそのまま使う（開き直さない）
    session = Session.__new__(Session)
    session._session = _core.create(path, title)
    return session


def operations() -> list[dict[str, Any]]:
    """正規操作カタログ（§19.7。`api.list` と同じ形）。"""
    return list(_core.OPERATIONS["operations"])
