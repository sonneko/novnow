import { readdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

// Astro prefixes generated assets, but not handwritten or Markdown root links.
// Normalize the emitted HTML so both content sources use the configured base.
/** @param {string} html @param {string} base @param {boolean} [development] */
export function prefixLinks(html, base, development = false) {
  const prefix = base.replace(/\/$/, '');
  return html.replace(/\b(href|src|action)=(['"])(\/(?!\/)[^'"\s]*)\2/g,
    (match, attr, quote, url) => (development && url.startsWith('/@vite/')) || url.split(/[?#]/, 1)[0] === prefix || url.startsWith(`${prefix}/`)
      ? match : `${attr}=${quote}${prefix}${url}${quote}`);
}

export default function baseLinks() {
  let base;
  return {
    name: 'novnow-base-links',
    hooks: {
      'astro:config:done': ({ config }) => { base = config.base; },
      'astro:build:done': async ({ dir }) => {
        async function visit(folder) {
          for (const entry of await readdir(folder, { withFileTypes: true })) {
            const file = path.join(folder, entry.name);
            if (entry.isDirectory()) await visit(file);
            else if (entry.name.endsWith('.html')) {
              const html = await readFile(file, 'utf8');
              await writeFile(file, prefixLinks(html, base));
            }
          }
        }
        await visit(fileURLToPath(dir));
      },
    },
  };
}
