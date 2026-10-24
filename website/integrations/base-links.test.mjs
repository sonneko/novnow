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
