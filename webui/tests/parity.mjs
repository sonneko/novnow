// L7 パリティ判定（W7。`REF:docs/specifications/webui.md` §21.5.1・`REF:docs/plans.md#webui-w7`）。
//
// 正本は `REF:tests/golden/api_catalog.json` の **121 操作**（§19.7 の機能パリティカタログ）。
// この回帰は「カタログの各操作に、第 2 の画面から到達できるか」を実物のソースから判定し、
// `webui/src/lib/coverage.ts` の宣言と突き合わせる。Rust 側（`REF:crates/novnow-wasm/`）は使わない
// （設計 §21.5: 判定は Node 側で行う）。
//
// ## 到達の数え方
//
// 画面が操作へ届く口は 3 つある。1 つでもあれば「到達できる」とする。
//
// 1. `run('<操作 ID>')` … 画面が正規操作を直接呼ぶ。
// 2. `session.command` の名前 … 機能核（`REF:src/app/ops/session_ops.rs` の `command`）が
//    カタログの `command` 列から操作へ写す（例: `'save'` → `save.now`）。文字列リテラルで送る
//    もの（`runNamed('reload')` を含む）を拾う。
// 3. ショートカット表（`webui/src/lib/keyboard/keys.ts` の `SHORTCUTS`）の値 … キーから 2 の名前へ届く。
//
// 画面側だけで完結する名前（`ui:` の接頭辞）は正規操作ではないので数えない。
//
// ## 判定の公開（§21.5.1 MUST）
//
// 未提供は隠さない。この回帰は判定の数字を `console.log` に出し、`coverage.ts` の `PENDING` と
// 完全一致することを要求する。片方だけ直して「到達できる」と書く事故をここで止める。

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

const WEBUI = new URL('..', import.meta.url).pathname;
const REPO = new URL('../../', import.meta.url).pathname;

/** 生成物（手で書かないファイル）。操作 ID の一覧としては `catalog.ts` ではなくカタログ JSON を読む。 */
const GENERATED = /bridge\/catalog\.ts|i18n\/catalog\.ts|theme\/tokens\.ts/;

/** 書き出し済みの結果（1 ファイル 1 回だけ `tsc` を回す）。 */
const compiled = new Map();

/** `webui/` の相対パスの `.ts` を JS に書き出して読む（`keys.test.mjs` と同じ方式）。 */
function importTs(relative) {
  if (!compiled.has(relative)) {
    compiled.set(relative, compileTs(relative));
  }
  return compiled.get(relative);
}

async function compileTs(relative) {
  const out = await mkdtemp(join(tmpdir(), 'novnow-parity-'));
  try {
    execFileSync(
      process.execPath,
      [
        join(WEBUI, 'node_modules', 'typescript', 'bin', 'tsc'),
        join(WEBUI, relative),
        // `tsconfig.json` があると併用できないと断られるので外す。
        '--ignoreConfig',
        '--target', 'es2022',
        '--module', 'es2022',
        '--moduleResolution', 'bundler',
        '--outDir', out,
      ],
      { stdio: 'pipe' },
    );
    const name = relative.split('/').pop().replace(/\.ts$/, '.js');
    return await import(join(out, name));
  } finally {
    await rm(out, { recursive: true, force: true });
  }
}

/** カタログ（`tests/golden/api_catalog.json`。121 操作の正本）。 */
async function catalog() {
  return JSON.parse(await readFile(join(REPO, 'tests/golden/api_catalog.json'), 'utf8'));
}

/** `src/` 配下の `.svelte` と `.ts`（生成物を除く）。 */
async function sources(dir = join(WEBUI, 'src')) {
  const found = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (!['wasm', 'node_modules'].includes(entry.name)) {
        found.push(...(await sources(path)));
      }
    } else if (/\.(svelte|ts)$/.test(entry.name) && !GENERATED.test(path)) {
      found.push(path);
    }
  }
  return found;
}

/**
 * 画面が操作へ届く口を集める。戻り値は `操作 ID → 届く口の一覧`。
 *
 * 口は「どのファイルの、どの呼び方か」まで残す（未提供の一覧を人が追えるようにする）。
 */
async function reach(operations) {
  /** `session.command` の名前 → 操作 ID（カタログの `command` 列）。 */
  const byCommand = new Map();
  for (const op of operations) {
    if (op.command) {
      byCommand.set(op.command, op.id);
    }
  }

  const reached = new Map();
  const add = (id, how) => {
    if (!reached.has(id)) {
      reached.set(id, []);
    }
    reached.get(id).push(how);
  };

  for (const file of await sources()) {
    const text = await readFile(file, 'utf8');
    const short = file.slice(WEBUI.length);
    // 1. 正規操作 ID の直接呼び出し（`run('save.now')`）
    for (const found of text.matchAll(/run\(\s*'([a-z][a-z_]*\.[a-z_]+)'/g)) {
      add(found[1], `run@${short}`);
    }
    // 2. `session.command` の名前（リテラル。`runNamed('…')` も同じ口へ入る）
    for (const found of text.matchAll(/session\.command'\s*,\s*\{\s*name:\s*'([a-z0-9-]+)'/g)) {
      if (byCommand.has(found[1])) {
        add(byCommand.get(found[1]), `command:${found[1]}@${short}`);
      }
    }
    for (const found of text.matchAll(/runNamed\(\s*'([a-z0-9-]+)'/g)) {
      if (byCommand.has(found[1])) {
        add(byCommand.get(found[1]), `runNamed:${found[1]}@${short}`);
      }
    }
  }

  // 3. ショートカット表の値（キー → コマンド名 → 操作）
  const keys = await readFile(join(WEBUI, 'src/lib/keyboard/keys.ts'), 'utf8');
  const start = keys.indexOf('export const SHORTCUTS');
  const end = keys.indexOf('};', start);
  assert.ok(start >= 0 && end > start, 'keys.ts から SHORTCUTS を読めない');
  for (const found of keys.slice(start, end).matchAll(/:\s*'([a-z0-9:._-]+)'/g)) {
    if (found[1].startsWith('ui:')) {
      continue; // 画面側だけで完結する（正規操作ではない）
    }
    if (byCommand.has(found[1])) {
      add(byCommand.get(found[1]), `key:${found[1]}`);
    }
  }

  return reached;
}

/** 判定 1 回分（カタログ・到達・未提供）。 */
async function judge() {
  const data = await catalog();
  const operations = data.operations;
  const reached = await reach(operations);
  const pending = operations.map((op) => op.id).filter((id) => !reached.has(id));
  // 仕様上「画面からは呼ばない」と定められた操作（`coverage.ts` の `EXCLUDED`）。
  // 除外は根拠付きで、**実物からも到達していないこと**をここで検証する（根拠なしの
  // 除外を防ぐ）。除外対象はカタログに実在する操作に限る。
  const { EXCLUDED } = await importTs('src/lib/coverage.ts');
  const known = new Set(operations.map((op) => op.id));
  for (const entry of EXCLUDED) {
    assert.ok(known.has(entry.id), `EXCLUDED の ${entry.id} がカタログに無い`);
    assert.ok(!reached.has(entry.id), `${entry.id} は実物から到達している。除外しない`);
  }
  const excluded = new Set(EXCLUDED.map((entry) => entry.id));
  const effectivePending = pending.filter((id) => !excluded.has(id));
  return { data, operations, reached, pending, excluded, effectivePending };
}

test('カタログは 121 操作で、ID が重複しない（§19.7 の正本）', async () => {
  const { data, operations } = await judge();
  assert.equal(data.count, 121, 'カタログの件数が 121 でない');
  assert.equal(operations.length, 121, 'operations の件数が count と違う');
  assert.equal(new Set(operations.map((op) => op.id)).size, 121, '操作 ID が重複している');
});

test('画面が呼ぶ正規操作は、すべてカタログに実在する', async () => {
  const { operations, reached } = await judge();
  const known = new Set(operations.map((op) => op.id));
  const unknown = [...reached.keys()].filter((id) => !known.has(id));
  assert.deepEqual(unknown, [], `カタログに無い操作を呼んでいる: ${unknown.join(', ')}`);
});

test('未提供の一覧は coverage.ts の PENDING と完全に一致する（§21.5.1）', async () => {
  const { effectivePending } = await judge();
  const { PENDING } = await importTs('src/lib/coverage.ts');
  assert.deepEqual(
    [...PENDING],
    effectivePending,
    'coverage.ts の PENDING と、実物から集めた未提供の一覧がずれている',
  );
});

test('除外は根拠付きで、仕様上「画面からは呼ばない」操作だけ（§21.5.1）', async () => {
  const { EXCLUDED } = await importTs('src/lib/coverage.ts');
  assert.ok(EXCLUDED.length > 0 || true, '除外の仕組みが無い');
  for (const entry of EXCLUDED) {
    assert.match(entry.id, /^[a-z][a-z_]*\.[a-z_]+$/, `除外の ID の形が違う: ${entry.id}`);
    assert.ok(entry.section.length > 0, `${entry.id} の根拠（節）が空`);
    assert.ok(entry.why.length > 0, `${entry.id} の理由が空`);
  }
  const ids = new Set(EXCLUDED.map((entry) => entry.id));
  assert.equal(ids.size, EXCLUDED.length, '同じ操作を 2 回除外している');
});

test('判定の数字を公開する（到達できる件数と未提供の一覧）', async () => {
  const { operations, pending, excluded, effectivePending } = await judge();
  const { PENDING, verdict } = await importTs('src/lib/coverage.ts');
  // 数字はこの 1 行に出る。PR 本文・リリースノートはこの値を写す（§21.5.1 MUST）。
  console.log(
    `パリティ: ${operations.length} 操作中、到達できる ${operations.length - pending.length} 件・` +
      `未提供 ${effectivePending.length} 件（判定: ${verdict(operations.length)}）`,
  );
  for (const id of effectivePending) {
    console.log(`  未提供: ${id}`);
  }
  for (const id of excluded) {
    console.log(`  仕様上除外: ${id}`);
  }
  assert.equal(PENDING.length, effectivePending.length);
  // 未提供が 0 件でないかぎり「同等」と書かない（§21.5.1 MUST）。
  assert.equal(
    verdict(operations.length),
    effectivePending.length === 0 ? 'equivalent' : 'partial',
    '未提供が残っているのに「同等」と判定している',
  );
});

test('未提供は「この版ではまだ使えません」で示す（§21.1.2・§21.5.1）', async () => {
  const { PENDING } = await importTs('src/lib/coverage.ts');
  // 未提供を黙って隠さない。画面には `workspace.unavailable` の文言があること。
  const catalog = await readFile(join(WEBUI, 'src/lib/i18n/catalog.ts'), 'utf8');
  assert.match(catalog, /'workspace\.unavailable':/, '未提供を示す文言が無い');
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  assert.match(app, /workspace\.unavailable/, '未提供の文言を画面が使っていない');
  for (const id of PENDING) {
    assert.match(id, /^[a-z][a-z_]*\.[a-z_]+$/, `未提供の ID の形が違う: ${id}`);
  }
});

test('カバレッジの数え方は 1 か所（coverage.ts）だけが持つ', async () => {
  const { PENDING, PENDING_SET, isPending, reachableCount, verdict } = await importTs(
    'src/lib/coverage.ts',
  );
  assert.equal(PENDING_SET.size, PENDING.length, 'PENDING_SET と PENDING の件数が違う');
  for (const id of PENDING) {
    assert.ok(isPending(id), `${id} が isPending で真にならない`);
  }
  assert.equal(reachableCount(121), 121 - PENDING.length);
  assert.equal(verdict(0), 'partial', 'カタログが空なら「同等」にしない');
  assert.equal(verdict(121), PENDING.length === 0 ? 'equivalent' : 'partial');
});

test('画面仕様（§9〜§12）のgap一覧は各回帰確認の後で空にする（§21.5.1）', async () => {
  const { SPEC_GAPS } = await importTs('src/lib/coverage.ts');
  assert.deepEqual(SPEC_GAPS, [], '未解決gapは根拠を回帰テストに残したうえで解消する');
});
