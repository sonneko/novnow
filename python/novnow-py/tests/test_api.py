"""novnow Python API（§19.3）の pytest。

    pip install -e "python/novnow-py[test]" && pytest python/novnow-py
"""

import novnow
import pytest


@pytest.fixture()
def project(tmp_path):
    path = tmp_path / "novel"
    novnow.create(str(path), "星屑の時計")
    return str(path)


def test_create_then_read_the_catalog(project):
    catalog = novnow.operations()
    assert len(catalog) == 119, "§19.7 の全行"
    assert {operation["id"] for operation in catalog} >= {"project.info", "section.list"}


def test_the_documented_shape_of_section_19_3_2(project):
    with novnow.Session(project) as session:
        info = session.project.info()
        assert info["title"] == "星屑の時計"
        assert session.structure.tree()["count"] >= 1
        listed = session.section.list()
        assert listed["count"] >= 1


def test_arguments_can_be_positional_or_named(project):
    with novnow.Session(project) as session:
        listed = session.section.list()
        first = listed["sections"][0]["path"]
        assert session.document.get(path=first)["path"] == first
        assert session.document.get(first)["path"] == first


def test_errors_carry_the_classification(project):
    with novnow.Session(project) as session:
        with pytest.raises(novnow.NotFoundError) as caught:
            session.materials.get(id="M9999")
        error = caught.value
        assert error.class_ == "not_found"
        assert error.message
        assert error.exit_code == 3, "§19.1.7 の写像"
        assert issubclass(novnow.NotFoundError, novnow.NovnowError)


def test_all_nine_exception_types_exist():
    for name in [
        "UsageError",
        "ProjectError",
        "IOError",
        "ValidationError",
        "PolicyError",
        "NotFoundError",
        "ConflictError",
        "TimeoutError",
        "CancelledError",
    ]:
        assert issubclass(getattr(novnow, name), novnow.NovnowError), name


def test_events_are_python_dicts(project):
    with novnow.Session(project) as session:
        seen = []
        session.on_event(seen.append)
        session.document.write(path="contents/01-01.txt", text="書き換え\n")
        events = session.poll_events()
        assert all(isinstance(event, dict) for event in events)
        assert events, "イベントが 1 つも無い"
        # コールバックは起きたときに 1 回、`poll_events()` は同じ列を返す
        # （REF:src/app/session.rs の `push` / `poll_events` と同じ関係）
        assert seen == events


def test_calls_from_several_threads_are_serialised(project):
    """§19.3.4: 複数スレッドからの同時呼び出しはデータ競合でなく直列実行。"""
    import threading

    with novnow.Session(project) as session:
        answers: list[str] = []
        failures: list[BaseException] = []

        def worker() -> None:
            try:
                for _ in range(20):
                    answers.append(session.project.info()["title"])
            except BaseException as error:  # noqa: BLE001 - 失敗は主スレッドで見る
                failures.append(error)

        threads = [threading.Thread(target=worker) for _ in range(4)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()

    assert not failures, failures
    assert len(answers) == 80, "100 往復のうち欠けたものがある"
    assert all(answers), "応答が壊れている"


def test_the_gil_is_released_while_an_operation_runs(project):
    """§19.3.4: 待っているあいだにほかの Python スレッドが動く（GIL の解放）。

    `sys.setswitchinterval` を大きくしておくので、拡張が GIL を握ったままループしたら
    数えるスレッドは 1 歩も進めない。`Python::allow_threads` で待っていれば自由に走る。
    """
    import sys
    import threading
    import time

    original = sys.getswitchinterval()
    sys.setswitchinterval(30.0)
    ticks = 0
    stop = threading.Event()

    def ticker() -> None:
        nonlocal ticks
        while not stop.is_set():
            ticks += 1

    thread = threading.Thread(target=ticker)
    thread.start()
    try:
        with novnow.Session(project) as session:
            deadline = time.monotonic() + 0.5
            while time.monotonic() < deadline:
                session.project.info()
    finally:
        stop.set()
        sys.setswitchinterval(original)
        thread.join(timeout=10.0)

    assert ticks > 100, f"GIL が解放されていない（{ticks} 歩）"
