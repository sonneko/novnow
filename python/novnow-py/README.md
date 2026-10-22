# novnow (Python)

`novnow` の Python API（仕様書 §19.3）。Rust と同じ機能核を **インプロセス** で呼ぶ
ネイティブ拡張（`novnow._core`）で、サブプロセスや JSON ソケットのラッパーではありません。

```python
import novnow

with novnow.Session("my-novel") as session:      # open。with 終了で閉じる
    info = session.project.info()                 # dict（JSON と同じ構造）
    session.section.write("@03:01", "新しい本文\n")
    session.save.now()

session = novnow.create("new-novel")              # project.new
```

- 対応 Python: CPython 3.9 以降（abi3。1 つの wheel が 3.9+ の全版で動く）。
- 対応プラットフォーム: Linux x86_64 / aarch64、macOS x86_64 / arm64、Windows x86_64。
- 型スタブ（`__init__.pyi`）と `py.typed` を同梱。

## ビルドとテスト

```bash
pip install maturin pytest
cd python/novnow-py
maturin develop            # 開発用ビルド（venv に入れる）
pytest                     # tests/test_api.py
```

## 実装メモ

- 表の API（`Session`・9 例外・ドメイン呼び出し）は `python/novnow/__init__.py`。
  ネイティブ側（`src/lib.rs`）は `run` / `command` / `poll_events` / `on_event` / `cancel`
  だけを公開し、**正規操作カタログ（§19.7）が唯一の実装点**（§19.1.2）。
- 直列化（§19.3.4）: 1 つの `Session` は `Mutex` で守る。
- **GIL の解放（§19.3.4）は未実装**: `novnow::app::Session` が `Send` でない
  （`Host` と `on_event` のコールバックに `Send` が付いていない）ため、
  `Python::allow_threads` で包めない。`Host: Send` + `Callback: Send` が前提作業
  （`REF:docs/plans.md#stage-3` H6-AC-3）。
- pyo3 系のクレートは vendoring が必要（`REF:docs/add-dependencies.md` §2.2・パターン B-1。
  対象一覧は `REF:docs/plans.md` §5.5）。vendoring が済むまで root の workspace 解決から
  外してある（root `Cargo.toml` の `exclude`）。
