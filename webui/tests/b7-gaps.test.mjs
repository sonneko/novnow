// バッチ7の画面経路・残存制限の回帰。
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

const ROOT = new URL('../src/', import.meta.url);
const TEST_ROOT = new URL('./', import.meta.url);

async function text(url) { return readFile(url, 'utf8'); }

test('検索キーは入力面を開き、全文検索タブも同じ画面へつながる', async () => {
  const app = await text(new URL('App.svelte', ROOT));
  const shell = await text(new URL('lib/shell/AppShell.svelte', ROOT));
  const write = await text(new URL('lib/workspaces/write/WriteWorkspace.svelte', ROOT));
  const dialog = await text(new URL('lib/search/SearchDialog.svelte', ROOT));
  assert.match(app, /found === 'find' \|\| found === 'replace' \|\| found === 'search'/);
  assert.match(app, /scope: found === 'search' \? 'project' : 'file'/);
  assert.match(shell, /onsearch=\{onsearch\}/);
  assert.match(write, /onsearch\?\.\(\)/);
  assert.match(dialog, /session\.run\('search\.file'/);
  assert.match(dialog, /session\.projectSearch\(/);
  assert.match(dialog, /Alt\+R/);
  assert.match(dialog, /Alt\+C/);
  assert.match(dialog, /case_sensitive: caseSensitive/);
  assert.match(dialog, /timedOut \? ' · 時間上限のため部分結果'/);
  assert.match(dialog, /makePreview/);
  assert.match(dialog, /確認して置換/);
});

test('第2画面の全体置換はプレビュー後にWorkspaceEdit適用APIを呼ぶ', async () => {
  const dialog = await text(new URL('lib/search/SearchDialog.svelte', ROOT));
  const bridge = await text(new URL('lib/bridge/types.ts', ROOT));
  assert.match(dialog, /session\.projectSearch\(\{ query, regex, case_sensitive: caseSensitive, replacement \}\)/);
  assert.match(dialog, /await session\.run\('save\.now'\)/);
  assert.match(bridge, /projectSearch\?\(args: Record<string, unknown>\)/);
});

test('reference.pane のキャラ・明示資料 Nav は workspace switch 後の選択へ到達する', async () => {
  const app = await text(new URL('App.svelte', ROOT));
  const shell = await text(new URL('lib/shell/AppShell.svelte', ROOT));
  const cast = await text(new URL('lib/workspaces/cast/CastWorkspace.svelte', ROOT));
  const material = await text(new URL('lib/workspaces/material/MaterialWorkspace.svelte', ROOT));
  const api = await text(new URL('../../tests/golden/api_catalog.json', import.meta.url));
  assert.match(app, /nav\?\.kind === 'character'[\s\S]*?castFocusIndex = nav\.index[\s\S]*?setWorkspace\('cast'\)/);
  assert.match(app, /nav\?\.kind === 'material_open'[\s\S]*?materialFocusId = nav\.id[\s\S]*?setWorkspace\('material'\)/);
  assert.match(shell, /focusIndex=\{castFocusIndex\}/);
  assert.match(shell, /focusId=\{materialFocusId\}/);
  assert.match(shell, /\{revision\} run=\{run\}/);
  assert.match(cast, /focusIndex !== null && characters\.some\(\(row\) => row\.index === focusIndex\)/);
  assert.match(cast, /onfocusdone\?\.\(\)/);
  assert.match(material, /session\.run\('materials\.get', \{ id: focusId \}\)/);
  assert.match(material, /select\(item\.id\)/);
  const switchOp = JSON.parse(api).operations.find((row) => row.id === 'session.workspace_switch');
  assert.deepEqual(switchOp.arguments.required, ['workspace']);
  assert.deepEqual(Object.keys(switchOp.arguments.properties), ['workspace']);
});

test('Ctrl+K の2打鍵は2秒以内で、上位プロットキーの写像がある', async () => {
  const write = await text(new URL('lib/workspaces/write/WriteWorkspace.svelte', ROOT));
  const outline = await text(new URL('lib/workspaces/plot-outline/PlotOutlineWorkspace.svelte', ROOT));
  const keys = await text(new URL('lib/keyboard/keys.ts', ROOT));
  assert.match(keys, /'Ctrl\+Shift\+B': 'up'/);
  for (const source of [write, outline]) {
    assert.match(source, /chordPending/);
    assert.match(source, /2000\)/);
    assert.match(source, /event\.stopPropagation\(\)/);
  }
  assert.match(outline, /foldAll\(\)/);
  assert.match(outline, /unfoldAll\(\)/);
});
