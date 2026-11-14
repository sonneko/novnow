import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

// サイト内検索の索引を**ビルド時に**作る（仕様 §18.5.1）。
//
// 利用者のブラウザでは検索サービスへ問い合わせない。読むのは同じ公開元にある
// `search-index/<lang>.json` だけで、検索語が外部へ送られることは無い。
// 索引は生成物（`website/dist`）から作るので、「公開されているのに検索できないページ」が
// 原理的に起きない。対象漏れは `scripts/website-check.py` が dist と突き合わせて落とす。
export const INDEX_DIRECTORY = 'search-index';
export const LANGUAGES = ['ja', 'en'];
// 検索のページ自身は索引に入れない（自分の説明文が毎回ヒットするノイズになる）。
export const SEARCH_PAGE_RE = /(?:^|\/)(?:ja|en)\/search\/index\.html$/;
// Trunk の出力（ビルド後に `dist/app` として追加される）と、リダイレクトだけのルートは入れない。
export const EXCLUDED_PREFIXES = ['app/', 'nightly-app/', '_astro/'];
export const ROOT_REDIRECT = 'index.html';
// 1 ページあたりの本文の上限（字）。仕様書 API リファレンスの長いページでも収まる値にする。
export const MAX_TEXT_CHARS = 120000;

const ENTITIES = {
  '&amp;': '&', '&lt;': '<', '&gt;': '>', '&quot;': '"', '&#39;': "'", '&apos;': "'",
  '&nbsp;': ' ', '&ndash;': '–', '&mdash;': '—', '&hellip;': '…', '&copy;': '©',
};

/** @param {string} text */
export function decodeEntities(text) {
  return text
    .replace(/&(?:amp|lt|gt|quot|#39|apos|nbsp|ndash|mdash|hellip|copy);/g, match => ENTITIES[match] ?? match)
    .replace(/&#(\d+);/g, (_, code) => String.fromCodePoint(Number(code)))
    .replace(/&#x([0-9a-fA-F]+);/g, (_, code) => String.fromCodePoint(parseInt(code, 16)));
}

/**
 * 本文（`<main>`、無ければ `<body>`）だけを、タグとスクリプト・スタイルを除いた文字列にする。
 * ナビゲーションとフッターは毎ページ同じなので入れない（結果が全部同じ文言で埋まる）。
 * @param {string} html
 */
export function visibleText(html) {
  const withoutComments = html.replace(/<!--[\s\S]*?-->/g, ' ');
  const main = /<main[\s>][\s\S]*<\/main>/i.exec(withoutComments)
    ?? /<body[\s>][\s\S]*<\/body>/i.exec(withoutComments);
  const body = main ? main[0] : withoutComments;
  const text = body
    .replace(/<(script|style|noscript|svg|nav|header|footer)[\s>][\s\S]*?<\/\1>/gi, ' ')
    .replace(/<[^>]*>/g, ' ');
  return decodeEntities(text).replace(/\s+/g, ' ').trim().slice(0, MAX_TEXT_CHARS);
}

/** @param {string} html */
export function pageTitle(html) {
  const title = /<title[^>]*>([\s\S]*?)<\/title>/i.exec(html);
  if (title) return decodeEntities(title[1].replace(/\s*\|\s*NovNow\s*$/i, '')).replace(/\s+/g, ' ').trim();
  const heading = /<h1[^>]*>([\s\S]*?)<\/h1>/i.exec(html);
  return heading ? decodeEntities(heading[1].replace(/<[^>]*>/g, ' ')).replace(/\s+/g, ' ').trim() : '';
}

/** 節の見出し（`<h2>`）。結果の「どの節か」の表示と、見出し一致の重い点数に使う。 @param {string} html */
export function pageHeadings(html) {
  const found = [];
  for (const match of html.matchAll(/<h2[^>]*>([\s\S]*?)<\/h2>/gi)) {
    const text = decodeEntities(match[1].replace(/<[^>]*>/g, ' ')).replace(/\s+/g, ' ').trim();
    if (text) found.push(text);
  }
  return found;
}

/** `<html lang="…">` から言語を決める（ディレクトリ名に依存しない）。 @param {string} html */
export function pageLanguage(html) {
  const match = /<html[^>]*\slang=["']?([A-Za-z-]+)/i.exec(html);
  return match ? match[1].toLowerCase().split('-')[0] : '';
}

/**
 * dist 内の相対パスを公開 URL へ（`ja/manual/start/index.html` → `<base>/ja/manual/start/`）。
 * @param {string} file @param {string} base
 */
export function pageUrl(file, base) {
  const prefix = (base ?? '').replace(/\/$/, '');
  const relative = file.replace(/\\/g, '/');
  return relative.endsWith('/index.html')
    ? `${prefix}/${relative.slice(0, -'index.html'.length)}`
    : `${prefix}/${relative}`;
}

/**
 * 1 ページ分の索引のエントリ。索引に入れないページは `null`。
 * @param {string} file dist 内の相対パス @param {string} html @param {string} base
 */
export function entryFor(file, html, base) {
  const relative = file.replace(/\\/g, '/');
  if (SEARCH_PAGE_RE.test(relative) || relative === ROOT_REDIRECT) return null;
  if (EXCLUDED_PREFIXES.some(prefix => relative.startsWith(prefix))) return null;
  const language = pageLanguage(html);
  if (!LANGUAGES.includes(language)) return null;
  const text = visibleText(html);
  const title = pageTitle(html);
  if (!text && !title) return null;
  return { url: pageUrl(relative, base), lang: language, title, headings: pageHeadings(html), text };
}

/**
 * ページ一式から言語別の索引を作る。
 * @param {Array<{file: string, html: string}>} pages @param {string} base
 * @returns {Record<string, Array<object>>}
 */
export function buildIndex(pages, base) {
  /** @type {Record<string, Array<object>>} */
  const index = {};
  for (const language of LANGUAGES) index[language] = [];
  for (const page of pages) {
    const entry = entryFor(page.file, page.html, base);
    if (entry) index[entry.lang].push(entry);
  }
  for (const language of LANGUAGES) index[language].sort((a, b) => (a.url < b.url ? -1 : a.url > b.url ? 1 : 0));
  return index;
}

export default function searchIndex() {
  let base = '';
  return {
    name: 'novnow-search-index',
    hooks: {
      'astro:config:done': ({ config }) => { base = (config.base ?? '').replace(/\/$/, ''); },
      'astro:build:done': async ({ dir, logger }) => {
        const root = fileURLToPath(dir);
        /** @type {Array<{file: string, html: string}>} */
        const pages = [];
        await visit(root, root, pages);
        const index = buildIndex(pages, base);
        const out = path.join(root, INDEX_DIRECTORY);
        await mkdir(out, { recursive: true });
        for (const language of LANGUAGES) {
          const payload = JSON.stringify({ base, language, pages: index[language] });
          await writeFile(path.join(out, `${language}.json`), `${payload}\n`);
          logger?.info?.(`search index: ${language} ${index[language].length} pages, ${(payload.length / 1024).toFixed(0)} KB`);
        }
      },
    },
  };
}

/** @param {string} root @param {string} folder @param {Array<{file: string, html: string}>} pages */
async function visit(root, folder, pages) {
  for (const entry of await readdir(folder, { withFileTypes: true })) {
    const file = path.join(folder, entry.name);
    if (entry.isDirectory()) await visit(root, file, pages);
    else if (entry.name.endsWith('.html')) {
      pages.push({ file: path.relative(root, file).split(path.sep).join('/'), html: await readFile(file, 'utf8') });
    }
  }
}
