// 入口（第 2 の画面実装）。第 1 のブラウザ版（`app/`）とは別の入口である
// （`REF:docs/specifications/webui.md` §21.3.1 MUST: どちらの入口か区別できる）。

import { mount } from 'svelte';
import App from './App.svelte';
import { wasmBridge } from './lib/bridge/wasm';
import { nativeBridge } from './lib/bridge/native';
import { applyTheme, systemMode } from './lib/theme/tokens';
import './app.css';

/** 配色を先に決める（最初の描画でちらつかせない）。 */
applyTheme(systemMode());

const native = typeof window !== 'undefined' && '__TAURI__' in window;
const bridge = native ? nativeBridge() : wasmBridge();
const app = mount(App, {
  target: document.getElementById('app') as HTMLElement,
  props: { bridge },
});

// ブラウザは IndexedDB の最近の作品、Tauri は native file dialog から開く。
void (app as unknown as { start?: (root?: string | null) => Promise<void> }).start?.();

export default app;
