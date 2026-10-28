/* The post_build hook seals every app asset into this worker. Installation
   succeeds only after the complete first-load offline application is cached. */
const CACHE = 'novnow-offline-0.1.1';
const BUILD = 'unsealed';
const PRECACHE = null;
const CURRENT = `${CACHE}-${BUILD}`;
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
      .filter(key => key.startsWith('novnow-offline-') && key !== CURRENT)
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
