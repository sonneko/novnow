// Svelte 5 の設定（プリプロセッサは使わない。CSS は素のまま書く）。
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  preprocess: vitePreprocess(),
};
