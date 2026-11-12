/* The post_build hook seals every app asset into this worker. Installation
   succeeds only after the complete first-load offline application is cached. */
const CACHE = 'novnow-offline-0.1.1';
const BUILD = '5f2ba8df3896e74468ba8ace71952bbf308716b0b266965cbbdf60b5b34ca065';
const PRECACHE = ["./", "./guide/cheatsheet.html", "./guide/errors.html", "./guide/faq.html", "./guide/git.html", "./guide/glossary.html", "./guide/index.html", "./guide/install.html", "./guide/project_structure.html", "./guide/publish.html", "./guide/start.html", "./icon.svg", "./index.html", "./licenses/IPA_Font_License_Agreement_v1.0.txt", "./licenses/README.txt", "./manifest.webmanifest", "./novnow-193c0f551547c892.js", "./novnow-193c0f551547c892_bg.wasm"];
const SCOPE_CACHE = `novnow-offline-scope-${encodeURIComponent(self.registration.scope)}-`;
const CURRENT = `${SCOPE_CACHE}${CACHE}-${BUILD}`;
self.addEventListener('install', event => {
  event.waitUntil((async () => {
    if (!PRECACHE) throw new Error('Unsealed offline application; run the Trunk post_build hook');
    try {
      const cache = await caches.open(CURRENT);
      // Bypass the HTTP cache so a same-name HTML file cannot mix two builds.
      await cache.addAll(PRECACHE.map(url => new Request(url, { cache: 'reload' })));
    } catch (error) {
      await caches.delete(CURRENT);
      throw error; // Keep the previous working worker and cache on failed updates.
    }
    await self.skipWaiting();
  })());
});
self.addEventListener('activate', event => {
  event.waitUntil((async () => {
    await self.clients.claim();
    await Promise.all((await caches.keys())
      .filter(key => key.startsWith(SCOPE_CACHE) && key !== CURRENT)
      .map(key => caches.delete(key)));
  })());
});
self.addEventListener('fetch', event => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== 'GET' || url.origin !== self.location.origin || !url.href.startsWith(self.registration.scope)) return;
  event.respondWith((async () => {
    const cache = await caches.open(CURRENT);
    // The app shell and hashed assets must belong to the same sealed build.
    const hit = await cache.match(request, { ignoreSearch: true });
    if (hit) return hit;
    try {
      return await fetch(request);
    } catch (error) {
      if (request.mode === 'navigate') return (await cache.match('./index.html')) || Response.error();
      throw error;
    }
  })());
});
