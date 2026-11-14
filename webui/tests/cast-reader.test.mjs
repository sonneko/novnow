// `cast` / `reader` ワークスペースの回帰（Node。ブラウザを使わずに純粋な部分を確かめる）。
//
// 見るのは 4 つ:
//
// 1. **組版と呼称表の純粋関数が仕様どおり**（`reader.ts` の禁則処理・`cast.ts` の `reading` 順・
//    対角線）。同じ入力から同じ結果になる（決定論。§17.3.3・§10.6.1）。
// 2. **操作 ID は凍結カタログに実在する**（`catalog.ts` の `OP_IDS`）。綴りを写し間違えると
//    操作が黙って効かない（§11.11.5）。
// 3. **使う i18n キーは生成物 `i18n/catalog.ts` に実在する**（§12.3。文字列リテラル禁止）。
// 4. **使う CSS 変数は生成物 `theme/tokens.ts` に実在する**（§11.1。生の色を書かない）。
//
// TypeScript を Node で読むため、`tsc` で 1 度だけ書き出してから読む（型検査は `npm run check`）。

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

const ROOT = new URL('..', import.meta.url);

/** `webui/src/lib/workspaces/cast|reader/*.ts` を JS へ書き出して読む（1 ファイル 1 回）。 */
const compiled = new Map();

async function importTs(relative) {
  if (compiled.has(relative)) {
    return compiled.get(relative);
  }
  const webui = new URL('.', ROOT).pathname;
  const out = await mkdtemp(join(tmpdir(), 'novnow-ts-'));
  process.on('exit', () => {
    rm(out, { recursive: true, force: true }).catch(() => {});
  });
  execFileSync(
    process.execPath,
    [
      join(webui, 'node_modules', 'typescript', 'bin', 'tsc'),
      join(webui, relative),
      '--ignoreConfig',
      '--target',
      'es2022',
      '--module',
      'es2022',
      '--moduleResolution',
      'bundler',
      '--outDir',
      out,
    ],
    { stdio: 'pipe' },
  );
  const found = await import(join(out, relative.split('/').pop().replace(/\.ts$/, '.js')));
  compiled.set(relative, found);
  return found;
}

/** `reader.ts`（縦書き組版）。 */
function reader() {
  return importTs('src/lib/workspaces/reader/reader.ts');
}

/** `cast.ts`（呼称表の補助）。 */
function cast() {
  return importTs('src/lib/workspaces/cast/cast.ts');
}

// ============================================================================ 1. 純粋関数

test('reader 組版: 行は右から左へ、行内は上から下へ進む（§17.3.2）', async () => {
  const { layout } = await reader();
  const pages = layout('あいうえお', { columns: 2, rows: 42 });
  assert.equal(pages.length, 1, '3 行は 1 ページ（行数 42）に収まる');
  assert.deepEqual(
    pages[0].lines.map((line) => line.map((cell) => cell.ch).join('')),
    ['あい', 'うえ', 'お'],
    '行の並びは右から左へ（行 0 = 最右 = 先頭）',
  );
});

test('reader 組版: 行末禁則の括弧は次の行へ追い出す（§17.3.3）', async () => {
  const { layout, NO_END } = await reader();
  assert.ok(NO_END.includes('（'), '行末禁則に括弧を含む');
  const pages = layout('あ（い', { columns: 2, rows: 42 });
  // あ （ で 1 行満了。 （ は行末禁則なので次の行へ。结果是 [あ] / [（い]
  assert.deepEqual(
    pages[0].lines.map((line) => line.map((cell) => cell.ch).join('')),
    ['あ', '（い'],
  );
});

test('reader 組版: 行頭禁則の句読点は前の行へ戻す（§17.3.3）', async () => {
  const { layout, NO_START } = await reader();
  assert.ok(NO_START.includes('、'), '行頭禁則に読点を含む');
  const pages = layout('あい、う', { columns: 2, rows: 42 });
  // あ い で 1 行満了。次は 、（行頭禁則）→ 行頭に置けないので い を次の行へ連れ、
  // 、 が行頭に来ないようにする。結果は [あ] / [い、] / [う]。
  assert.deepEqual(
    pages[0].lines.map((line) => line.map((cell) => cell.ch).join('')),
    ['あ', 'い、', 'う'],
  );
});

test('reader 組版: ルビは親文字に添えて残す（§17.3.3）', async () => {
  const { layout } = await reader();
  const pages = layout('《ら》字', { columns: 8, rows: 42 });
  const cells = pages[0].lines.flat();
  const target = cells.find((cell) => cell.ch === '字');
  assert.ok(target, '親文字がある');
  assert.equal(target.ruby, 'ら', 'ルビが親文字に添えられる');
});

test('cast 呼称表: 対角線は常に `—`（§10.6.1 MUST）', async () => {
  const { buildMatrix, DIAGONAL } = await cast();
  const names = [
    { name: 'あおい', reading: 'あおい' },
    { name: '恵', reading: 'めぐみ' },
  ];
  const matrix = buildMatrix(names, new Map());
  assert.equal(matrix[0][0].value, DIAGONAL, '自分自身は —');
  assert.equal(matrix[1][1].value, DIAGONAL, '自分自身は —');
});

test('cast 呼称表: 行・列は `reading` 順（§10.6）', async () => {
  const { sortByReading } = await cast();
  const rows = [
    { name: '恵', reading: 'めぐみ' },
    { name: 'あおい', reading: 'あおい' },
  ];
  const sorted = sortByReading(rows);
  assert.deepEqual(
    sorted.map((row) => row.name),
    ['あおい', '恵'],
    'あおい（あおい）が先、恵（めぐみ）が後',
  );
});

// ============================================================================ 2. 操作 ID

test('cast / reader が使う操作 ID は凍結カタログに実在する（§11.11.5）', async () => {
  const catalog = await readFile(new URL('src/lib/bridge/catalog.ts', ROOT), 'utf8');
  const ids = new Set([...catalog.matchAll(/^\s*"([a-z][a-z0-9_.]+)":/gm)].map((found) => found[1]));
  // `cast` ワークスペース
  for (const op of [
    'cast.list',
    'cast.timeline',
    'cast.scan',
    'dictionary.register_character',
    'dictionary.modes_list',
    'document.get',
    'document.write',
  ]) {
    assert.ok(ids.has(op), `cast が使う ${op} がカタログに無い`);
  }
  // `reader` ワークスペース
  for (const op of [
    'structure.tree',
    'document.get',
    'export.text',
    'session.command',
  ]) {
    assert.ok(ids.has(op), `reader が使う ${op} がカタログに無い`);
  }
});

test('cast / reader の Svelte から参照する操作 ID はすべてカタログにある', async () => {
  const catalog = await readFile(new URL('src/lib/bridge/catalog.ts', ROOT), 'utf8');
  const ids = new Set([...catalog.matchAll(/^\s*"([a-z][a-z0-9_.]+)":/gm)].map((found) => found[1]));
  for (const file of [
    'src/lib/workspaces/cast/CastWorkspace.svelte',
    'src/lib/workspaces/reader/ReaderWorkspace.svelte',
  ]) {
    const text = await readFile(new URL(file, ROOT), 'utf8');
    for (const found of text.matchAll(/run\(\s*'([a-z][a-z0-9_.]+)'/g)) {
      assert.ok(ids.has(found[1]), `${file} の ${found[1]} がカタログに無い`);
    }
  }
});

// ============================================================================ 3. i18n

test('cast / reader が使う i18n キーは生成物に実在する（§12.3）', async () => {
  const catalog = await readFile(new URL('src/lib/i18n/catalog.ts', ROOT), 'utf8');
  const keys = new Set([...catalog.matchAll(/^\s*'([a-z0-9_.]+)':/gm)].map((found) => found[1]));
  for (const file of [
    'src/lib/workspaces/cast/CastWorkspace.svelte',
    'src/lib/workspaces/reader/ReaderWorkspace.svelte',
  ]) {
    const text = await readFile(new URL(file, ROOT), 'utf8');
    for (const found of text.matchAll(/\bt\('([a-z0-9_.]+)'/g)) {
      assert.ok(keys.has(found[1]), `${file} の i18n キー ${found[1]} がカタログに無い`);
    }
  }
});

// ============================================================================ 4. CSS トークン

test('cast / reader が使う CSS 変数は生成物 tokens.ts に実在する（§11.1）', async () => {
  const tokens = await readFile(new URL('src/lib/theme/tokens.ts', ROOT), 'utf8');
  const names = new Set(
    [...tokens.matchAll(/'([a-z][a-z0-9_.]*)':\s*'--/g)].map((found) => found[1]),
  );
  // CSS 変数名（`--accent` など）をトークン名へ戻す
  const toToken = (cssName) => {
    const found = [...tokens.matchAll(/'([a-z][a-z0-9_.]*)':\s*'(--[a-z0-9-]+)'/g)].find(
      ([, , name]) => name === cssName,
    );
    return found?.[1];
  };
  for (const file of [
    'src/lib/workspaces/cast/CastWorkspace.svelte',
    'src/lib/workspaces/reader/ReaderWorkspace.svelte',
  ]) {
    const text = await readFile(new URL(file, ROOT), 'utf8');
    for (const found of text.matchAll(/var\((--[a-z0-9-]+)\)/g)) {
      const token = toToken(found[1]);
      assert.ok(token, `${file} の CSS 変数 ${found[1]} が tokens.ts に無い`);
      assert.ok(names.has(token), `${found[1]} はトークン ${token} に無い`);
    }
  }
});
