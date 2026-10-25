import { defineMiddleware } from 'astro:middleware';
import { prefixLinks } from '../integrations/base-links.mjs';

// Production HTML (including copied guides) is normalized by the build hook.
// Apply the same rule in development, where that hook never runs.
export const onRequest = defineMiddleware(async (_context, next) => {
  const response = await next();
  if (!import.meta.env.DEV || !response.headers.get('content-type')?.includes('text/html')) return response;
  const headers = new Headers(response.headers);
  headers.delete('content-length');
  return new Response(prefixLinks(await response.text(), import.meta.env.BASE_URL, true), {
    status: response.status, statusText: response.statusText, headers,
  });
});
