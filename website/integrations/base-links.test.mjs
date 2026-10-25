import test from 'node:test';
import assert from 'node:assert/strict';
import { prefixLinks } from './base-links.mjs';
import config from '../astro.config.mjs';

test('prefix handwritten and markdown URLs without double-prefixing Astro assets', () => {
  const html = `<a href="/ja/manual/">Guide</a><img src='/_astro/a.png'><a href="/novnow/en/">EN</a>`;
  assert.equal(prefixLinks(html, '/novnow'), `<a href="/novnow/ja/manual/">Guide</a><img src='/novnow/_astro/a.png'><a href="/novnow/en/">EN</a>`);
});
test('preserve anchors, external URLs and relative app paths', () => {
  const html = `<a href="#x"></a><a href="https://example.com/"></a><a href="//example.com/"></a><a href="./app/"></a>`;
  assert.equal(prefixLinks(html, '/novnow'), html);
});

test('base root URLs with queries and fragments remain idempotent', () => {
  const html = '<a href="/novnow?lang=ja"></a><a href="/novnow#top"></a>';
  assert.equal(prefixLinks(html, '/novnow/'), html);
});

test('development keeps the root Vite HMR endpoint while prefixing page links', () => {
  assert.equal(prefixLinks('<script src="/@vite/client"></script><a href="/ja/">JA</a>', '/novnow', true),
    '<script src="/@vite/client"></script><a href="/novnow/ja/">JA</a>');
});

test('routing accepts both trailing slash variants', () => {
  assert.equal(config.trailingSlash, 'ignore');
});

// Built-output regressions: direct assets and document rendering are public contracts.
import { readFileSync } from 'node:fs';
test('published download links cover all ten native archives and VSIX', () => {
  for (const lang of ['ja', 'en']) {
    const html = readFileSync(new URL(`../dist/${lang}/index.html`, import.meta.url), 'utf8');
    const release = readFileSync(new URL('../src/data/release.ts', import.meta.url), 'utf8');
    const version = release.match(/export const version = "([^"]+)"/)[1];
    const files = [...html.matchAll(/https:\/\/github\.com\/sonneko\/novnow\/releases\/download\/v[^/]+\/([^"<>\s]+)/g)].map(m => m[1]);
    for (const target of ['x86_64-pc-windows-msvc', 'aarch64-apple-darwin', 'x86_64-apple-darwin', 'aarch64-unknown-linux-gnu', 'x86_64-unknown-linux-gnu']) {
      for (const flavor of ['gui', 'nogui']) {
        assert.ok(files.includes(`novnow-${version}-${target}-${flavor}${target.includes('windows') ? '.zip' : '.tar.gz'}`));
      }
    }
    assert.equal(new Set(files.filter(f => /-(gui|nogui)\.(zip|tar\.gz)$/.test(f))).size, 10);
    assert.ok(files.includes(`novnow-${version}.vsix`));
    assert.ok(files.includes('SHA256SUMS'));
    assert.ok(html.includes(`/novnow/${lang}/known-issues`));
  }
});
test('known issues are rendered from the document originals', () => {
  for (const lang of ['ja', 'en']) {
    const source = readFileSync(new URL(`../../docs/known-issues/${lang}.md`, import.meta.url), 'utf8');
    const html = readFileSync(new URL(`../dist/${lang}/known-issues/index.html`, import.meta.url), 'utf8');
    for (const heading of source.split('\n').filter(line => /^#{1,2} /.test(line))) {
      assert.ok(html.includes(heading.replace(/^#+ /, '')));
    }
    assert.deepEqual(new Set(html.match(/KI-\d+/g) ?? []), new Set(source.match(/KI-\d+/g) ?? []));
    assert.ok(html.includes(source.split('\n\n')[1]));
  }
});
