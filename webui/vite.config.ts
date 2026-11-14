// Vite の設定（`REF:docs/designs/webui.md` §21.3 のビルド）。
//
// - 入口は 1 つ（`index.html`）。第 1 のブラウザ版（Trunk の `app/`）とは置き場所が違う。
// - `src/wasm/` は wasm-bindgen の生成物（git に入れない）。`npm run wasm` が作る。
// - `base` は Pages の `webui/` の下に出しても動くよう相対にする（`REF:index.html` の導線）。

import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [svelte()],
  base: './',
  build: {
    outDir: 'dist',
    target: 'es2022',
    assetsInlineLimit: 0,
  },
  server: {
    // ライブプレビュー（プロキシされたホスト）からも開けるようにする。開発サーバーは
    // 既定で `localhost` 以外の Host を拒む（DNS リバインディング対策）ので、
    // 作業環境のプレビュー用ホストを明示して許可する。
    host: '0.0.0.0',
    port: 5273,
    strictPort: true,
    allowedHosts: ['.e2b.app', '.arena.ai', 'localhost', '127.0.0.1'],
    fs: { strict: false },
  },
  preview: {
    host: '0.0.0.0',
    port: 5273,
    allowedHosts: ['.e2b.app', '.arena.ai', 'localhost', '127.0.0.1'],
  },
});
