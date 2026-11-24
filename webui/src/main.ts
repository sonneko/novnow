// 入口（第 2 の画面実装）。第 1 のブラウザ版（`app/`）とは別の入口である
// （`REF:docs/specifications/webui.md` §21.3.1 MUST: どちらの入口か区別できる）。

import { mount } from 'svelte';
import App from './App.svelte';
import { wasmBridge } from './lib/bridge/wasm';
import { applyTheme, systemMode } from './lib/theme/tokens';
import './app.css';

/** 配色を先に決める（最初の描画でちらつかせない）。 */
applyTheme(systemMode());

const bridge = wasmBridge();
const app = mount(App, {
  target: document.getElementById('app') as HTMLElement,
  props: { bridge },
});

// 保存領域（IndexedDB `novnow-webui`）からプロジェクトを開く。開けなければ画面に理由を出す。
void (app as unknown as { start?: (root?: string | null) => Promise<void> }).start?.();

export default app;
