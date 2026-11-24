# bridge

画面と橋渡しのあいだの型（`REF:docs/designs/webui.md` §21、`REF:docs/plans.md#webui-w2`）。

| ファイル | 由来 | 手で書くか |
|---|---|---|
| `types.ts` | W2 で凍結した口（`open` / `run` / `poll_events` / `flush` / イベント形） | **手で書く。凍結** |
| `catalog.ts` | `tests/golden/api_catalog.json`（§19.7 のカタログ） | 書かない（`scripts/api-ts.py` が生成） |
| `wasm.ts` | `crates/novnow-wasm` の wasm-bindgen の生成物を包む | 手で書く |
| `memory.ts` | ブラウザの外（Node のテスト）で同じ口を使う実装 | 手で書く |

`types.ts` を変えてよいのは L0（基盤）だけである。以降のレーンは読むだけにする
（`REF:docs/plans.md#webui-parallel` §11.11.5）。
