import test from 'node:test';
import assert from 'node:assert/strict';
import { prefixLinks } from './base-links.mjs';

test('prefix handwritten and markdown URLs without double-prefixing Astro assets', () => {
  const html = `<a href="/ja/manual/">Guide</a><img src='/_astro/a.png'><a href="/novnow/en/">EN</a>`;
  assert.equal(prefixLinks(html, '/novnow'), `<a href="/novnow/ja/manual/">Guide</a><img src='/novnow/_astro/a.png'><a href="/novnow/en/">EN</a>`);
});
test('preserve anchors, external URLs and relative app paths', () => {
  const html = `<a href="#x"></a><a href="https://example.com/"></a><a href="//example.com/"></a><a href="./app/"></a>`;
  assert.equal(prefixLinks(html, '/novnow'), html);
});
