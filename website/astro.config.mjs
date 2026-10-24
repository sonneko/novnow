// @ts-check
import { defineConfig } from 'astro/config';
import baseLinks from './integrations/base-links.mjs';

// https://astro.build/config
export default defineConfig({
  site: 'https://sonneko.github.io',
  base: '/novnow',
  // Existing links use both forms; neither should become a 404.
  trailingSlash: 'ignore',
  integrations: [baseLinks()],
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
