// Web 版の筋書き（scripts/web-smoke.sh から呼ぶ）。ヘッドレスの Chromium で dist を開き、
// 画面（canvas）はスクリーンショットで、保存の結果は IndexedDB を直接読んで確かめる。
//
// 環境変数: NODE_DIR（puppeteer-core の node_modules がある場所）・CHROME（実行ファイル）・DIST・OUT_DIR・MODE（all / shot）
import { createRequire } from 'node:module';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';

const require = createRequire(path.join(process.env.NODE_DIR, 'package.json'));
const puppeteer = require('puppeteer-core');

const DIST = process.env.DIST;
const OUT = process.env.OUT_DIR;
const DOWNLOADS = path.join(OUT, 'downloads');
const MODE = process.env.MODE || 'all';
const PORT = 8765;
const ADDRESS = `http://127.0.0.1:${PORT}/`;

const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.wasm': 'application/wasm' };
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

let failures = 0;
function check(ok, label) {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}`);
  if (!ok) failures += 1;
}

function serve() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    const file = path.join(DIST, url.pathname === '/' ? 'index.html' : decodeURIComponent(url.pathname));
    fs.readFile(file, (err, data) => {
      if (err) {
        res.writeHead(404);
        res.end();
        return;
      }
      res.writeHead(200, { 'Content-Type': MIME[path.extname(file)] || 'application/octet-stream' });
      res.end(data);
    });
  });
  return new Promise((resolve) => server.listen(PORT, '127.0.0.1', () => resolve(server)));
}

async function launch(profile) {
  const browser = await puppeteer.launch({
    executablePath: process.env.CHROME,
    headless: true,
    userDataDir: profile,
    defaultViewport: { width: 1280, height: 800 },
    args: [
      '--headless=new', '--no-sandbox', '--no-zygote', '--disable-setuid-sandbox', '--use-gl=angle',
      '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--in-process-gpu',
      '--lang=ja-JP', '--font-render-hinting=none',
    ],
  });
  // ダウンロードは GUID の名前で置き、ページが付けた名前（suggestedFilename）は事件で受ける
  // （このヘッドレスの Chromium は日本語の名前を `download` にしてしまうため）
  const cdp = await browser.target().createCDPSession();
  await cdp.send('Browser.setDownloadBehavior', { behavior: 'allowAndName', downloadPath: DOWNLOADS, eventsEnabled: true });
  cdp.on('Browser.downloadWillBegin', (event) => downloads.set(event.guid, { name: event.suggestedFilename, done: false }));
  cdp.on('Browser.downloadProgress', (event) => {
    const found = downloads.get(event.guid);
    if (found && event.state === 'completed') found.done = true;
  });
  return browser;
}

/** ダウンロード（GUID → ページが付けた名前と、終わったか）。 */
const downloads = new Map();

async function page(browser, setup) {
  const tab = await browser.newPage();
  tab.on('console', (message) => {
    if (message.type() === 'error') console.log(`  [console.error] ${message.text().slice(0, 300)}`);
  });
  tab.on('pageerror', (error) => console.log(`  [pageerror] ${error.message.slice(0, 300)}`));
  // 画面の言語（novnow は navigator.language を見る。ヘッドレスは --lang を無視するので上書きする）
  await tab.evaluateOnNewDocument((language) => {
    Object.defineProperty(navigator, 'language', { get: () => language });
  }, process.env.UI_LANG || 'ja-JP');
  if (setup) await tab.evaluateOnNewDocument(setup);
  await tab.goto(ADDRESS, { waitUntil: 'load' });
  return tab;
}

async function shot(tab, name) {
  await sleep(400);
  await tab.screenshot({ path: path.join(OUT, `${name}.png`) });
}

// IndexedDB を読む（ページの中で）。DB がまだ無ければ作らない（版 1 の空の DB を先に作ると、novnow の
// onupgradeneeded が走らずストアが無いままになる）
async function db(tab) {
  return tab.evaluate(
    () =>
      new Promise(async (resolve) => {
        const known = await indexedDB.databases();
        if (!known.some((entry) => entry.name === 'novnow')) {
          resolve({ projects: [], files: [], app: [] });
          return;
        }
        const open = indexedDB.open('novnow');
        open.onerror = () => resolve(null);
        open.onsuccess = () => {
          const database = open.result;
          const names = Array.from(database.objectStoreNames);
          if (!names.includes('projects')) {
            resolve({ projects: [], files: [], app: [] });
            return;
          }
          const tx = database.transaction(['projects', 'files', 'app'], 'readonly');
          const out = {};
          let pending = 3;
          for (const store of ['projects', 'files', 'app']) {
            const request = tx.objectStore(store).getAll();
            request.onsuccess = () => {
              out[store] = request.result;
              pending -= 1;
              if (pending === 0) {
                database.close();
                resolve(out);
              }
            };
          }
        };
      }),
  );
}

async function until(label, probe, timeout = 15000) {
  const start = Date.now();
  for (;;) {
    const value = await probe();
    if (value) return value;
    if (Date.now() - start > timeout) {
      check(false, `${label}（${timeout}ms 待った）`);
      return null;
    }
    await sleep(250);
  }
}

const file = (state, project, filePath) =>
  state?.files.find((entry) => entry.project_id === project && entry.path === filePath)?.text ?? null;

async function key(tab, combo) {
  const parts = combo.split('+');
  const main = parts.pop();
  for (const modifier of parts) await tab.keyboard.down(modifier);
  await tab.keyboard.press(main);
  for (const modifier of parts.reverse()) await tab.keyboard.up(modifier);
  await sleep(300);
}

// 終わったダウンロードのうち、まだ受け取っていない `suffix` のもの。このヘッドレスの Chromium は日本語を含む
// 名前を `download` にしてしまう（普通の Chrome は日本語の名前のまま保存する）ので、そのときは中身で見分ける
const taken = new Set();
async function downloaded(suffix, timeout = 10000) {
  return until(`ダウンロード *${suffix}`, () => {
    for (const [guid, entry] of downloads) {
      if (taken.has(guid) || !entry.done) continue;
      const file = path.join(DOWNLOADS, guid);
      const zip = fs.readFileSync(file).subarray(0, 2).toString() === 'PK';
      const fallback = entry.name === 'download' && zip === (suffix === '.zip');
      if (entry.name.endsWith(suffix) || fallback) {
        taken.add(guid);
        console.log(`  ダウンロード: ${entry.name}`);
        // file chooser に渡すときに拡張子が要るので、名前を付けて写す
        const target = path.join(DOWNLOADS, `copy-${guid}${suffix}`);
        fs.copyFileSync(file, target);
        return target;
      }
    }
    return null;
  }, timeout);
}

async function main() {
  fs.rmSync(OUT + '/profile', { recursive: true, force: true });
  fs.rmSync(DOWNLOADS, { recursive: true, force: true });
  fs.mkdirSync(DOWNLOADS, { recursive: true });
  const server = await serve();
  const browser = await launch(path.join(OUT, 'profile'));
  try {
    let tab = await page(browser);
    // 1. 初回: sample が 1 つだけでき、選択画面が出る（自動では開かない、§13.3.5）
    const first = await until('sample の投入', async () => {
      const state = await db(tab);
      return state && state.projects.length > 0 ? state : null;
    });
    await sleep(1500);
    await shot(tab, '00-picker');
    if (MODE === 'shot') return;
    check(first?.projects.length === 1 && first.projects[0].id === 'sample', 'projects は sample の 1 件');
    if (!first) throw new Error('起動しなかった（00-picker.png を見る）');
    check((first?.app ?? []).length === 0, '開く前の app ストアは空');

    // 2. Enter で開く → 本文を打つ → IndexedDB に保存される
    await key(tab, 'Enter');
    await sleep(1500);
    await shot(tab, '01-opened');
    const opened = await db(tab);
    check(opened.app.length === 1 && opened.app[0].key === 'recent_projects', 'app ストアのキーは recent_projects だけ（§13.3.4）');
    check(opened.app[0]?.value?.items?.[0]?.id === 'sample', '履歴の先頭が sample');
    const before = file(opened, 'sample', 'contents/01-04.txt');
    check(before !== null, 'sample の本文 contents/01-04.txt がある');
    // F-01: photograph the glyph-free dark crescent and return to the
    // original system preference before checking the remaining workflows.
    await key(tab, 'Control+Alt+t');
    await shot(tab, '01b-dark-theme-icon');
    await key(tab, 'Control+Alt+t');
    await key(tab, 'Control+Alt+t');
    // F-17: exercise the actual installed Web keydown listener, not just the
    // Rust shortcut table. DevTools' reserved chord stays free; its alternate
    // and ordinary app navigation are prevented before the browser handles them.
    const blocked = await tab.evaluate(() => {
      const dispatch = (key, options) => {
        const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...options });
        document.dispatchEvent(event);
        return event.defaultPrevented;
      };
      return {
        devtools: dispatch('I', { ctrlKey: true, shiftKey: true }),
        alternate: dispatch('I', { ctrlKey: true, altKey: true, shiftKey: true }),
        workspace: dispatch('1', { altKey: true }),
        text: dispatch('あ', {}),
      };
    });
    check(!blocked.devtools && blocked.alternate && blocked.workspace && !blocked.text,
      'F-17: Web 予約キーは維持し、代替キーと移動キーだけ preventDefault');
    await key(tab, 'Control+2');
    await key(tab, 'End');
    await key(tab, 'Enter');
    await tab.keyboard.type('Web で書き足した一文。');
    const saved = await until('打った本文の保存', async () => {
      const text = file(await db(tab), 'sample', 'contents/01-04.txt');
      return text && text.includes('Web で書き足した一文。') ? text : null;
    });
    check(saved !== null, '打った本文が IndexedDB に保存された（§8.3）');
    // IME の変換（compositionstart / update / end）。キーボードで節エディタに移っただけで受けられる（B-158）
    const ime = await tab.target().createCDPSession();
    await ime.send('Input.imeSetComposition', { text: 'へんかん', selectionStart: 4, selectionEnd: 4 });
    await sleep(300);
    await shot(tab, '02a-composing');
    await ime.send('Input.insertText', { text: '変換した。' });
    const composed = await until('IME で確定した字の保存', async () => {
      const text = file(await db(tab), 'sample', 'contents/01-04.txt');
      return text && text.includes('一文。変換した。') ? text : null;
    });
    check(composed !== null && !composed.includes('へんかん'), 'IME の確定だけが本文に入った（変換中の字は入らない、§9.8.5）');
    await shot(tab, '02-typed');
    // S-14/F-02/F-03: capture a narrow layout for manual caret and status-bar inspection.
    await tab.setViewport({ width: 620, height: 720 });
    await shot(tab, '02b-narrow');
    await tab.keyboard.type('追');
    await sleep(70); // before the 400ms autosave, while the status indicator is unsaved
    await tab.screenshot({ path: path.join(OUT, '02c-narrow-unsaved.png') });
    await tab.setViewport({ width: 1280, height: 800 });
    await until('狭幅で打った字の保存', async () => {
      const text = file(await db(tab), 'sample', 'contents/01-04.txt');
      return text?.includes('変換した。追') ? text : null;
    });
    // F-36: on a real browser canvas, Ctrl+3 exposes the shared reference
    // pane from material; Tab remains available to cycle the form controls.
    const materialBefore = file(await db(tab), 'sample', 'materials/index.json');
    await key(tab, 'Alt+5');
    await shot(tab, '02d-material');
    await key(tab, 'Control+3');
    await shot(tab, '02e-material-reference');
    await key(tab, 'Tab');
    await shot(tab, '02f-material-tab');
    check(file(await db(tab), 'sample', 'materials/index.json') === materialBefore,
      'F-36: 資料で Ctrl+3 と Tab を使っても内容は変更されない');
    await key(tab, 'Alt+1');

    // 3. 読み直しても残る・Ctrl+O で選択画面へ戻る（§13.4）
    await tab.reload({ waitUntil: 'load' });
    await sleep(2500);
    await key(tab, 'Enter');
    await sleep(1500);
    await shot(tab, '03-reloaded');
    // PWA: a controlled online reload caches the hashed JS/WASM. The next
    // navigation must boot from the service worker with the network disabled.
    await tab.evaluate(() => navigator.serviceWorker.ready);
    await tab.waitForFunction(() => navigator.serviceWorker.controller !== null, { timeout: 15000 });
    await tab.reload({ waitUntil: 'load' });
    await sleep(2500);
    const cached = await tab.evaluate(async () => {
      const keys = await caches.keys();
      const urls = (await Promise.all(keys.map(async (key) => (await (await caches.open(key)).keys()).map((req) => req.url)))).flat();
      return urls.some((url) => url.endsWith('.wasm')) && urls.some((url) => url.endsWith('.js'));
    });
    check(cached, 'PWA の JS と WASM がキャッシュされた');
    const network = await tab.target().createCDPSession();
    await network.send('Network.enable');
    await network.send('Network.emulateNetworkConditions', { offline: true, latency: 0, downloadThroughput: 0, uploadThroughput: 0 });
    try {
      await tab.reload({ waitUntil: 'load' });
      const offlineStarted = await until('オフライン起動', async () =>
        tab.evaluate(() => document.querySelector('canvas')?.width > 0).catch(() => false), 15000);
      check(!!offlineStarted && (await db(tab))?.projects?.some((p) => p.id === 'sample'),
        'オフライン再起動でも IndexedDB の作品が残る');
    } finally {
      await network.send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
      await network.detach();
    }
    await key(tab, 'Enter');
    await sleep(900);
    await key(tab, 'Control+o');
    await sleep(1200);
    await shot(tab, '04-back-to-list');

    // F-33: the title check must run against IndexedDB, not just the
    // currently filtered picker rows. A failed create must not open a project.
    await key(tab, 'Control+n');
    await tab.keyboard.type(first.projects[0].name);
    await key(tab, 'Enter');
    await sleep(900);
    const afterDuplicateCreate = await db(tab);
    check(afterDuplicateCreate.projects.length === 1 &&
      afterDuplicateCreate.projects[0].id === 'sample',
      'F-33: 既存タイトルの新規作成を拒否し IndexedDB を保持');

    // 4. Ctrl+N で新規 → 名前 → 開く
    await key(tab, 'Control+n');
    await tab.keyboard.type('星屑の時計');
    await key(tab, 'Enter');
    const created = await until('新規作成', async () => {
      const state = await db(tab);
      return state.projects.find((project) => project.name === '星屑の時計') ?? null;
    });
    check(created?.origin === 'new', '新しいプロジェクトのレコード（origin: new）');
    await sleep(2000);
    await shot(tab, '05-created');
    const createdState = await db(tab);
    check(file(createdState, created?.id, 'project.json')?.includes('星屑の時計'), 'project.json の題が名前');

    // 5. Zip でエクスポート（パレットの >zip）→ ダウンロード
    await key(tab, 'Control+p');
    await tab.keyboard.type('>zip');
    await sleep(300);
    await key(tab, 'Enter');
    const zip = await downloaded('.zip');
    check(zip && fs.readFileSync(zip).subarray(0, 2).toString() === 'PK', 'Zip がダウンロードされた（§13.5）');
    await shot(tab, '06-zip-exported');

    // 6. テキストのエクスポート（>export → Shift+Enter の名前を付けて保存 = ダウンロード）
    await key(tab, 'Control+p');
    await tab.keyboard.type('>export');
    await sleep(300);
    await key(tab, 'Enter');
    await sleep(500);
    await shot(tab, '07-export-modal');
    await key(tab, 'Shift+Enter');
    const text = await downloaded('.txt');
    check(text && fs.readFileSync(text, 'utf8').length > 0, 'テキストのエクスポートがダウンロードされた（§17.2）');

    // 7. agent: Web は LLM を呼べない
    await key(tab, 'Alt+8');
    await sleep(800);
    await shot(tab, '08-agent');
    await key(tab, 'Alt+1');

    // 8. 選択画面: F2 で名前の変更・Zip の取り込み・Delete で削除
    await key(tab, 'Control+o');
    await sleep(1200);
    await key(tab, 'F2');
    await key(tab, 'Control+a');
    await tab.keyboard.type(first.projects[0].name);
    await key(tab, 'Enter');
    await sleep(900);
    const afterDuplicateRename = await db(tab);
    check(afterDuplicateRename.projects.find((project) => project.id === created?.id)?.name === '星屑の時計' &&
      afterDuplicateRename.projects.length === 2,
      'F-33: 既存タイトルへの改名を拒否し IndexedDB を保持');
    await key(tab, 'F2');
    await key(tab, 'Control+a');
    await tab.keyboard.type('改名した');
    await key(tab, 'Enter');
    const renamed = await until('名前の変更', async () => {
      const state = await db(tab);
      return state.projects.find((project) => project.name === '改名した') ?? null;
    });
    check(renamed?.id === created?.id, 'F2 で名前を変えた（§13.4）');
    const [chooser] = await Promise.all([tab.waitForFileChooser({ timeout: 5000 }), key(tab, 'Control+i')]);
    await chooser.accept([zip]);
    const imported = await until('Zip の取り込み', async () => {
      const state = await db(tab);
      return state.projects.find((project) => project.origin === 'import') ?? null;
    });
    check(imported !== null, 'Zip を新しいプロジェクトとして取り込んだ（§13.5）');
    const importedState = await db(tab);
    check(file(importedState, imported?.id, 'project.json')?.includes('星屑の時計'), '取り込んだ中身');
    await sleep(800);
    await shot(tab, '09-imported');
    // 取り込んだもの（選ばれている）を消す
    await key(tab, 'Delete');
    await tab.keyboard.type(imported?.name ?? '');
    await shot(tab, '10-delete-confirm');
    await key(tab, 'Enter');
    const deleted = await until('削除', async () => {
      const state = await db(tab);
      return state.projects.some((project) => project.id === imported?.id) ? null : state;
    });
    check(deleted !== null && !deleted.files.some((entry) => entry.project_id === imported?.id), 'Delete で消した（ファイルも）');
    await shot(tab, '11-after-delete');

    // 9. 保存の失敗（QuotaExceededError、§13.3.7）→ 導線のモーダル → 直ったら書ける
    await key(tab, 'ArrowDown');
    await key(tab, 'ArrowUp');
    await key(tab, 'Enter');
    await sleep(1500);
    await tab.evaluate(() => {
      const put = IDBObjectStore.prototype.put;
      window.__novnowPut = put;
      IDBObjectStore.prototype.put = function () {
        throw new DOMException('quota', 'QuotaExceededError');
      };
    });
    await key(tab, 'Control+2');
    await key(tab, 'End');
    await key(tab, 'Enter');
    await tab.keyboard.type('容量不足のあいだの一文。');
    await sleep(2000);
    await shot(tab, '12-quota');
    await tab.evaluate(() => {
      IDBObjectStore.prototype.put = window.__novnowPut;
    });
    const recovered = await until('失敗の後の再試行', async () => {
      const state = await db(tab);
      return state.files.some((entry) => entry.text.includes('容量不足のあいだの一文。')) ? state : null;
    }, 30000);
    check(recovered !== null, '失敗の後、再試行で保存された（§8.3）');

    // 10. IndexedDB が使えない（E062。LocalStorage へフォールバックしない）
    await tab.close();
    tab = await page(browser, () => {
      Object.defineProperty(window, 'indexedDB', { value: undefined, configurable: true });
    });
    await sleep(2500);
    await shot(tab, '13-unavailable');
    const localKeys = await tab.evaluate(() => Object.keys(localStorage));
    check(localKeys.length === 0, 'IndexedDB が無くても LocalStorage に書かない（§13.3.7）');
    await tab.close();
  } finally {
    await browser.close();
    server.close();
  }
  console.log(failures === 0 ? 'すべて通った' : `${failures} 件失敗`);
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
