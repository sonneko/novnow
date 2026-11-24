// Headless Chromium regression audit for the develop nightly-webui build.
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import chromium from '/home/user/build/web-tools/browser/node_modules/@sparticuz/chromium/build/index.js';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const dist = path.join(repo, 'webui/dist');
const logPath = path.join(repo, 'investigation/nightly-webui-browser.json');
const require = createRequire('/home/user/build/web-tools/browser/package.json');
const puppeteer = require('puppeteer-core');
const prefix = '/novnow/nightly-webui';
const port = 5187;
const address = `http://127.0.0.1:${port}${prefix}/`;
const mime = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml', '.png': 'image/png', '.webmanifest': 'application/manifest+json',
  '.wasm': 'application/wasm', '.ico': 'image/x-icon',
};
const evidence = {
  url: address,
  console: [],
  pageErrors: [],
  unhandledRejections: [],
  network: [],
  workspaceSwitches: [],
  initial: null,
  edit: null,
  reload: null,
  result: 'running',
  phase: 'setup',
};
let phase = 'setup';
let server;
let browser;

function writeEvidence() {
  fs.mkdirSync(path.dirname(logPath), { recursive: true });
  fs.writeFileSync(logPath, `${JSON.stringify(evidence, null, 2)}\n`);
}

function startServer() {
  return new Promise((resolve, reject) => {
    server = http.createServer((req, res) => {
      const url = new URL(req.url, `http://127.0.0.1:${port}`);
      if (url.pathname !== prefix && !url.pathname.startsWith(`${prefix}/`)) {
        res.writeHead(404, { 'content-type': 'text/plain' });
        res.end('not found');
        return;
      }
      let relative = decodeURIComponent(url.pathname.slice(prefix.length)).replace(/^\/+/, '');
      if (!relative || relative.endsWith('/')) relative += 'index.html';
      const file = path.resolve(dist, relative);
      if (!file.startsWith(`${path.resolve(dist)}${path.sep}`)) {
        res.writeHead(400); res.end(); return;
      }
      fs.readFile(file, (err, bytes) => {
        if (err) { res.writeHead(404); res.end(); return; }
        res.writeHead(200, { 'content-type': mime[path.extname(file)] || 'application/octet-stream' });
        res.end(bytes);
      });
    });
    server.once('error', reject);
    server.listen(port, '127.0.0.1', resolve);
  });
}

async function readDatabase(page) {
  return page.evaluate(async () => {
    const databases = await indexedDB.databases();
    const rows = [];
    for (const item of databases) {
      if (!item.name) continue;
      const db = await new Promise((resolve, reject) => {
        const request = indexedDB.open(item.name);
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
      });
      const stores = [...db.objectStoreNames];
      const snapshot = { name: db.name, version: db.version, stores, projects: [], files: { count: 0, paths: [] } };
      if (stores.includes('projects')) {
        const projects = await new Promise((resolve, reject) => {
          const tx = db.transaction('projects', 'readonly');
          const req = tx.objectStore('projects').getAll();
          req.onsuccess = () => resolve(req.result);
          req.onerror = () => reject(req.error);
        });
        snapshot.projects = projects.map(({ id, name, origin, schema_version }) => ({ id, name, origin, schema_version }));
      }
      if (stores.includes('files')) {
        const files = await new Promise((resolve, reject) => {
          const tx = db.transaction('files', 'readonly');
          const req = tx.objectStore('files').getAll();
          req.onsuccess = () => resolve(req.result);
          req.onerror = () => reject(req.error);
        });
        snapshot.files = { count: files.length, paths: files.map((row) => row.path).sort() };
      }
      rows.push(snapshot);
      db.close();
    }
    return rows;
  });
}

async function main() {
  if (!fs.existsSync(path.join(dist, 'index.html'))) throw new Error(`Missing build output: ${dist}`);
  await startServer();
  const executablePath = await chromium.executablePath();
  browser = await puppeteer.launch({
    executablePath,
    args: [...chromium.args, '--no-sandbox', '--disable-dev-shm-usage', '--lang=ja-JP'],
    defaultViewport: { width: 1440, height: 1000 },
    headless: true,
  });
  const page = await browser.newPage();
  page.on('console', (message) => evidence.console.push({
    type: message.type(), text: message.text(), location: message.location(),
  }));
  page.on('pageerror', async (error) => {
    let activeWorkspace = null;
    try { activeWorkspace = await page.evaluate(() => document.querySelector('header nav.workspaces button[aria-current="page"]')?.getAttribute('aria-label') ?? null); } catch {}
    evidence.pageErrors.push({ at: new Date().toISOString(), phase, activeWorkspace, name: error.name, message: error.message, stack: error.stack });
  });
  page.on('error', (error) => evidence.pageErrors.push({ event: 'page-crash', name: error.name, message: error.message, stack: error.stack }));
  page.on('request', (request) => evidence.network.push({
    method: request.method(), url: request.url(), type: request.resourceType(), status: null, failure: null,
  }));
  page.on('response', (response) => {
    const request = response.request();
    const found = [...evidence.network].reverse().find((row) => row.url === request.url() && row.method === request.method() && row.status === null && row.failure === null);
    if (found) found.status = response.status();
  });
  page.on('requestfailed', (request) => {
    const found = [...evidence.network].reverse().find((row) => row.url === request.url() && row.method === request.method() && row.status === null && row.failure === null);
    if (found) found.failure = request.failure()?.errorText ?? 'unknown';
  });
  await page.evaluateOnNewDocument(() => {
    window.__nightlyAudit = { rejections: [], saveStates: [] };
    window.addEventListener('unhandledrejection', (event) => {
      const reason = event.reason;
      window.__nightlyAudit.rejections.push({
        reason: reason instanceof Error ? reason.message : String(reason),
        stack: reason instanceof Error ? reason.stack : null,
      });
    });
    const sampleSave = () => {
      const button = document.querySelector('footer.status button.save');
      if (!button) return;
      const state = button.getAttribute('data-state');
      const rows = window.__nightlyAudit.saveStates;
      if (rows.length === 0 || rows[rows.length - 1].state !== state) {
        rows.push({ at: new Date().toISOString(), state, title: button.title, ariaLabel: button.getAttribute('aria-label') });
      }
    };
    setInterval(sampleSave, 25);
  });
  phase = 'initial load';
  evidence.phase = phase;
  await page.goto(address, { waitUntil: 'load', timeout: 60000 });
  await page.waitForFunction(() => document.querySelectorAll('header nav.workspaces button').length === 9, { timeout: 60000 });
  await page.waitForFunction(() => document.querySelector('header button.name')?.textContent?.trim().length > 0, { timeout: 60000 });
  await page.waitForSelector('div.lines[role="textbox"]', { timeout: 60000 });
  await new Promise((resolve) => setTimeout(resolve, 1000));
  evidence.initial = await page.evaluate(() => ({
    title: document.querySelector('header button.name')?.textContent?.trim() ?? null,
    path: location.pathname,
    documentTitle: document.title,
    workspaces: [...document.querySelectorAll('header nav.workspaces button')].map((button) => ({
      label: button.getAttribute('aria-label'), current: button.getAttribute('aria-current') === 'page',
    })),
    saveState: document.querySelector('footer.status button.save')?.getAttribute('data-state') ?? null,
    saveTitle: document.querySelector('footer.status button.save')?.title ?? null,
    editorText: [...document.querySelectorAll('.lines .row .text')].map((node) => node.textContent).join('\n'),
  }));
  evidence.initial.databases = await readDatabase(page);

  const buttons = await page.$$('header nav.workspaces button');
  for (let index = 0; index < buttons.length; index += 1) {
    const label = await buttons[index].evaluate((element) => element.getAttribute('aria-label'));
    phase = `workspace switch: ${label}`;
    evidence.phase = phase;
    await buttons[index].click();
    await page.waitForFunction((wanted) => {
      const current = document.querySelector('header nav.workspaces button[aria-current="page"]');
      return current?.getAttribute('aria-label') === wanted;
    }, { timeout: 20000 }, label);
    await new Promise((resolve) => setTimeout(resolve, 500));
    evidence.workspaceSwitches.push(await page.evaluate(() => {
      const active = document.querySelector('header nav.workspaces button[aria-current="page"]');
      return {
        label: active?.getAttribute('aria-label') ?? null,
        mainText: document.querySelector('main')?.innerText?.slice(0, 500) ?? null,
        activeCount: document.querySelectorAll('header nav.workspaces button[aria-current="page"]').length,
      };
    }));
  }

  phase = 'edit and autosave';
  evidence.phase = phase;
  await page.click('header nav.workspaces button:first-child');
  await page.waitForSelector('div.lines .row', { timeout: 30000 });
  const marker = 'NIGHTLY_WEBUI_BROWSER_SAVE_PROBE_20261124';
  const lastRow = await page.$('div.lines .row:last-child');
  const box = await lastRow.boundingBox();
  await lastRow.click({ position: { x: Math.max(1, box.width - 8), y: Math.max(1, box.height / 2) } });
  await page.keyboard.type(marker, { delay: 5 });
  await new Promise((resolve) => setTimeout(resolve, 30000));
  evidence.edit = await page.evaluate(async (needle) => {
    const button = document.querySelector('footer.status button.save');
    const databases = await indexedDB.databases();
    const loaded = [];
    for (const item of databases) {
      if (!item.name) continue;
      const db = await new Promise((resolve, reject) => {
        const req = indexedDB.open(item.name);
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
      });
      const snapshot = { name: db.name, version: db.version, stores: [...db.objectStoreNames], projects: [], matches: [] };
      if (snapshot.stores.includes('projects')) {
        snapshot.projects = await new Promise((resolve, reject) => {
          const req = db.transaction('projects', 'readonly').objectStore('projects').getAll();
          req.onsuccess = () => resolve(req.result);
          req.onerror = () => reject(req.error);
        });
      }
      if (snapshot.stores.includes('files')) {
        const files = await new Promise((resolve, reject) => {
          const req = db.transaction('files', 'readonly').objectStore('files').getAll();
          req.onsuccess = () => resolve(req.result);
          req.onerror = () => reject(req.error);
        });
        snapshot.matches = files.filter((row) => typeof row.text === 'string' && row.text.includes(needle)).map((row) => ({
          project_id: row.project_id, path: row.path, containsMarker: row.text.includes(needle), updated_at: row.updated_at,
        }));
      }
      loaded.push(snapshot);
      db.close();
    }
    return {
      marker: needle,
      editorContainsMarker: [...document.querySelectorAll('.lines .row .text')].some((node) => node.textContent.includes(needle)),
      state: button?.getAttribute('data-state') ?? null,
      title: button?.title ?? null,
      ariaLabel: button?.getAttribute('aria-label') ?? null,
      databases: loaded,
      saveStates: window.__nightlyAudit.saveStates,
      unhandledRejections: window.__nightlyAudit.rejections,
    };
  }, marker);

  phase = 'reload persistence check';
  evidence.phase = phase;
  await page.reload({ waitUntil: 'load', timeout: 60000 });
  await page.waitForSelector('div.lines[role="textbox"]', { timeout: 60000 });
  await new Promise((resolve) => setTimeout(resolve, 1000));
  evidence.reload = await page.evaluate((needle) => ({
    title: document.querySelector('header button.name')?.textContent?.trim() ?? null,
    editorContainsMarker: [...document.querySelectorAll('.lines .row .text')].some((node) => node.textContent.includes(needle)),
    saveState: document.querySelector('footer.status button.save')?.getAttribute('data-state') ?? null,
    saveTitle: document.querySelector('footer.status button.save')?.title ?? null,
    saveStates: window.__nightlyAudit.saveStates,
    unhandledRejections: window.__nightlyAudit.rejections,
  }), marker);
  evidence.reload.databases = await readDatabase(page);
  evidence.result = 'completed';
}

try {
  await main();
} catch (error) {
  evidence.result = 'failed';
  evidence.fatal = { name: error.name, message: error.message, stack: error.stack };
} finally {
  if (browser) await browser.close().catch(() => {});
  if (server) await new Promise((resolve) => server.close(resolve));
  writeEvidence();
  const failedNetwork = evidence.network.filter((row) => row.failure || (row.status !== null && row.status >= 400));
  console.log(JSON.stringify({
    result: evidence.result,
    logPath,
    fatal: evidence.fatal ?? null,
    initial: evidence.initial,
    workspaceSwitches: evidence.workspaceSwitches,
    edit: evidence.edit,
    reload: evidence.reload,
    console: evidence.console,
    pageErrors: evidence.pageErrors,
    unhandledRejections: evidence.unhandledRejections,
    failedNetwork,
    requestCount: evidence.network.length,
  }, null, 2));
}
if (evidence.result !== 'completed') process.exitCode = 1;
