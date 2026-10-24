// @ts-check
import { defineConfig } from 'astro/config';

// https://astro.build/config
export default defineConfig({
  redirects: {
    '/': '/en',
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
