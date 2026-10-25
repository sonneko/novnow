// Read-only CI test of the actual merged Astro + Trunk output. No deployment.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import { createRequire } from 'node:module';
const require = createRequire(path.join(process.env.NODE_DIR, 'package.json'));
const engine = process.env.BROWSER || 'chromium';
const playwright = require('playwright');
const root = path.resolve('website/dist');
let offline = false;
let retire = false;
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json' };
const server = http.createServer(async (req, res) => {
  if (offline) { req.socket.destroy(); return; }
  try {
    const url = new URL(req.url, 'http://test');
    if (url.pathname === '/novnow/legacy.html') {
      res.writeHead(200, { 'Content-Type': 'text/html' }); res.end('<title>Legacy worker migration fixture</title>'); return;
    }
    if (url.pathname === '/novnow/legacy-sw.js') {
      res.writeHead(200, { 'Content-Type': 'text/javascript', 'Cache-Control': 'no-store' });
      res.end(retire ? await fs.readFile(path.join(root, 'sw.js')) : `self.addEventListener('install',e=>e.waitUntil(self.skipWaiting()));self.addEventListener('activate',e=>e.waitUntil(self.clients.claim()));`); return;
    }
    assert(url.pathname.startsWith('/novnow/'));
    let file = path.resolve(root, decodeURIComponent(url.pathname.slice('/novnow/'.length)));
    assert(file === root || file.startsWith(root + path.sep));
    if ((await fs.stat(file)).isDirectory()) file = path.join(file, 'index.html');
    res.writeHead(200, { 'Content-Type': mime[path.extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-store' });
    res.end(await fs.readFile(file));
  } catch { res.writeHead(404); res.end('Not found'); }
});
await new Promise(resolve => server.listen(0, '0.0.0.0', resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await playwright[engine].launch(engine === 'chromium' ? { args: ['--enable-unsafe-swiftshader'] } : {});
async function snapshot(page) {
  return JSON.parse(await page.evaluate(async () => {
    const db = await new Promise((resolve, reject) => { const r = indexedDB.open('novnow'); r.onsuccess = () => resolve(r.result); r.onerror = () => reject(r.error); });
    try {
      return JSON.stringify(await Promise.all(['projects', 'files', 'app'].map(store => new Promise((resolve, reject) => {
        const r = db.transaction(store).objectStore(store).getAll(); r.onsuccess = () => resolve(r.result); r.onerror = () => reject(r.error);
      }))));
    } finally { db.close(); }
  }));
}
async function instrument(context) {
  await context.addInitScript(() => {
    for (const name of ['instantiate', 'instantiateStreaming']) {
      const original = WebAssembly[name];
      if (original) WebAssembly[name] = async function (...args) {
        const result = await original.apply(this, args); window.wasmInitialized = true; return result;
      };
    }
  });
}
async function waitUntil(predicate, description) {
  const deadline = Date.now() + 60000;
  while (!(await predicate())) {
    assert(Date.now() < deadline, `Timed out waiting for ${description}`);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
}
async function appReady(page, { offlineReady = true } = {}) {
  await page.waitForFunction(() => window.wasmInitialized === true && document.querySelector('canvas'), null, { timeout: 60000 });
  if (offlineReady) await page.waitForFunction(() => document.getElementById('offline-status')?.dataset.ready === 'true', null, { timeout: 60000 });
  await waitUntil(() => page.evaluate(async () => {
    if (!(await indexedDB.databases()).some(db => db.name === 'novnow')) return false;
    return new Promise(resolve => {
      const request = indexedDB.open('novnow');
      request.onerror = () => resolve(false);
      request.onsuccess = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains('projects')) { db.close(); resolve(false); return; }
        const sample = db.transaction('projects').objectStore('projects').get('sample');
        sample.onsuccess = () => { db.close(); resolve(!!sample.result); };
        sample.onerror = () => { db.close(); resolve(false); };
      };
    });
  }), "sample project to commit");
}
try {
  const context = await browser.newContext();
  const page = await context.newPage();
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 900 });
    for (const route of ['ja/', 'en/', 'ja/manual/start/', 'en/manual/start/', 'ja/api/', 'en/api/']) {
      for (const suffix of ['', '/']) {
        const response = await page.goto(`${origin}/novnow/${route.replace(/\/$/, '')}${suffix}`);
        assert.equal(response.status(), 200);
        assert(await page.locator('link[rel="canonical"]').getAttribute('href'));
        assert.equal(await page.locator('img').evaluateAll(images => images.filter(i => i.complete && i.naturalWidth === 0).length), 0);
      }
    }
  }
  await page.goto(`${origin}/novnow/en/manual/start/`);
  await page.locator('[aria-label="Language"] a').first().click();
  await page.waitForURL('**/novnow/ja/manual/start/');
  await context.close();
  console.log(`${engine}: merged website, both slash forms, desktop/mobile and language switching PASS`);

  const fresh = await browser.newContext({ serviceWorkers: 'allow' });
  await instrument(fresh);
  const app = await fresh.newPage();
  const errors = [];
  app.on('pageerror', error => errors.push(error.message));
  if (engine === 'chromium') {
    const cdp = await fresh.newCDPSession(app); await cdp.send('Network.enable'); await cdp.send('Network.setCacheDisabled', { cacheDisabled: true });
  }
  await app.goto(`${origin}/novnow/app/`);
  await appReady(app);
  const before = await snapshot(app);
  assert(before[0].some(project => project.id === 'sample'), JSON.stringify(before[0]));
  // The server disconnects worker fetches too; page-only offline emulation is insufficient.
  offline = true;
  try {
    await app.reload(); await appReady(app);
    assert.deepEqual(await snapshot(app), before);
  } finally { offline = false; }
  assert.deepEqual(errors, []);
  console.log(`${engine}: real Trunk first-load offline WASM and IndexedDB preservation PASS`);

  // A browser permission prompt must never gate project initialization.
  const permissionPending = await browser.newContext();
  await permissionPending.addInitScript(() => {
    navigator.storage.persist = () => new Promise(() => {});
  });
  await instrument(permissionPending);
  const pendingApp = await permissionPending.newPage();
  await pendingApp.goto(`${origin}/novnow/app/`);
  // This isolates application initialization, not a second offline installation.
  // The real first-install/offline contract is checked above in every engine.
  await appReady(pendingApp, { offlineReady: false });
  assert((await snapshot(pendingApp))[0].some(project => project.id === 'sample'));
  await permissionPending.close();
  console.log(`${engine}: unanswered persistence permission does not block startup PASS`);

  // Retire a legacy root-scoped worker using the actual retirement script.
  // Unload app scripts before registering the legacy root worker: the app's
  // compatibility cleanup must not race with this synthetic old installation.
  const legacy = app;
  await legacy.goto(`${origin}/novnow/legacy.html`);
  await legacy.evaluate(async () => { for (const reg of await navigator.serviceWorker.getRegistrations()) await reg.unregister(); });
  await legacy.evaluate(async () => {
    await navigator.serviceWorker.register('./legacy-sw.js', { scope: './' });
    await navigator.serviceWorker.ready;
  });
  await legacy.waitForFunction(() => navigator.serviceWorker.controller !== null);
  assert.deepEqual(await snapshot(legacy), before);
  assert((await legacy.evaluate(async () => (await navigator.serviceWorker.getRegistrations()).map(r => new URL(r.scope).pathname))).includes('/novnow/'));
  retire = true;
  await legacy.evaluate(async () => {
    const reg = (await navigator.serviceWorker.getRegistrations()).find(r => new URL(r.scope).pathname === '/novnow/');
    // WebKit may already have performed the update and unregistered the worker.
    if (reg) await reg.update();
  });
  await waitUntil(() => legacy.evaluate(async () => (await navigator.serviceWorker.getRegistrations()).length === 0), "legacy worker retirement");
  assert.deepEqual(await snapshot(legacy), before);
  await legacy.goto(`${origin}/novnow/app/`); await appReady(legacy);
  assert.deepEqual(await snapshot(legacy), before);
  const scopes = await legacy.evaluate(async () => (await navigator.serviceWorker.getRegistrations()).map(r => new URL(r.scope).pathname));
  assert.deepEqual(scopes, ['/novnow/app/']);
  await legacy.goto(`${origin}/novnow/en/`);
  assert.equal(await legacy.evaluate(() => navigator.serviceWorker.controller), null);
  console.log(`${engine}: root worker retirement, project preservation, app scope and website isolation PASS`);
  await fresh.close();
} finally {
  offline = false;
  await browser.close();
  await new Promise(resolve => server.close(resolve));
}
