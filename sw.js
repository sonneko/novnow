// Retire the legacy /novnow/ app worker. The app now owns /novnow/app/sw.js.
// IndexedDB projects are intentionally untouched.
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', event => {
  event.waitUntil((async () => {
    await self.registration.unregister();
    await self.clients.claim();
  })());
});
