import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

const ROOT = new URL('..', import.meta.url);

test('ステータスバー: clean/idle の保存ボタンは「保存中」ではなく保存操作を案内する', async () => {
  const component = await readFile(new URL('src/lib/shell/StatusBar.svelte', ROOT), 'utf8');
  const catalog = await readFile(new URL('src/lib/i18n/catalog.ts', ROOT), 'utf8');

  assert.match(
    component,
    /aria-label=\{detail \|\| t\('api\.op\.save\.now'\)\}/,
    '詳細文言のない clean/idle 状態では保存操作の名前を使う',
  );
  assert.match(catalog, /'api\.op\.save\.now': '保存'/, '日本語の保存操作名がある');
  assert.match(catalog, /'api\.op\.save\.now': 'Save now'/, '英語の保存操作名がある');
});
