// @ts-check
import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';
import baseLinks from './integrations/base-links.mjs';
import searchIndex from './integrations/search-index.mjs';

// https://astro.build/config
export default defineConfig({
  site: 'https://sonneko.github.io',
  base: '/novnow',
  // Existing links use both forms; neither should become a 404.
  trailingSlash: 'ignore',
  // search-index は base-links のあと（リンクの base 付与が終わった dist を読む）。
  integrations: [baseLinks(), searchIndex(), sitemap()],
  redirects: {
    '/': '/novnow/en/',
  },
  devToolbar: {
    enabled: false,
  },
  markdown: {
    shikiConfig: {
      theme: 'github-light',
    },
  },
  vite: {
    server: {
      allowedHosts: true,
      fs: {
        allow: ['..'],
      },
    },
    preview: {
      allowedHosts: true,
    },
  },
});
