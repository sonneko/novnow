// L3 知識（`idea` と `material`）の回帰（Node。`webui/src/lib/workspaces/{idea,material}/` を読む）。
//
// 見るのは 3 つ:
//
// 1. 規則（`ideas.ts` / `materials.ts`）: 絞り込み・並び・次の行・本文の切り出し・昇格先・検索語・
//    審査の差分・出典の有無・キーの写像・仮想化の窓。第 1 の画面（`REF:src/ui/workspaces/idea.rs` /
//    `material.rs`）と同じ結果になることを、正本の規則から写したケースで確かめる。
// 2. 配線の約束（`.svelte`）: 呼ぶ正規操作は `OP_IDS` に実在する。画面の文言は `CATALOGS` の `ja` と `en`
//    の両方に実在する（§12.3）。生色を書かない（§11.1）。
// 3. 凍結の約束: 規則のファイルは実行時の import を持たない（テストが単独で読める形を保つ）。
//
// TypeScript は `tsc` で 1 ファイルずつ書き出してから読む（`keys.test.mjs` と同じ方式）。

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

const WEBUI = new URL('..', import.meta.url).pathname;

/** 書き出し済みの結果（1 ファイル 1 回だけ `tsc` を回す）。 */
const compiled = new Map();

/** `webui/` の相対パスの `.ts` を JS に書き出して読む。 */
function importTs(relative) {
  if (!compiled.has(relative)) {
    compiled.set(relative, compileTs(relative));
  }
  return compiled.get(relative);
}

async function compileTs(relative) {
  const out = await mkdtemp(join(tmpdir(), 'novnow-knowledge-'));
  try {
    execFileSync(
      process.execPath,
      [
        join(WEBUI, 'node_modules', 'typescript', 'bin', 'tsc'),
        join(WEBUI, relative),
        // `tsconfig.json` があると併用できないと断られるので外す。
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
    const name = relative.split('/').pop().replace(/\.ts$/, '.js');
    return await import(join(out, name));
  } finally {
    await rm(out, { recursive: true, force: true });
  }
}

const ideas = () => importTs('src/lib/workspaces/idea/ideas.ts');
const materials = () => importTs('src/lib/workspaces/material/materials.ts');
const bridgeCatalog = () => importTs('src/lib/bridge/catalog.ts');
const i18nCatalog = () => importTs('src/lib/i18n/catalog.ts');

/** 画面のファイル（L3 の所有。`.svelte` と規則のファイル）。 */
const LANE_FILES = [
  'src/lib/workspaces/idea/IdeaWorkspace.svelte',
  'src/lib/workspaces/idea/CaptureIdea.svelte',
  'src/lib/workspaces/idea/ideas.ts',
  'src/lib/workspaces/material/MaterialWorkspace.svelte',
  'src/lib/workspaces/material/materials.ts',
];

/** `ideas.md` の `idea.list` 相当の行（テスト用の作り）。 */
function row(id, index, status, tags = [], lines = [index * 3, index * 3 + 2]) {
  return { id, index, title: `t${index}`, status, tags, lines };
}

// ---------------------------------------------------------------- idea

test('idea: 行は壊れた要素を捨て、状態は大文字小文字を問わず読む', async () => {
  const { parseRows } = await ideas();
  const parsed = parseRows([
    { id: 'I0001', index: 0, title: 'a', status: 'Inbox', tags: ['x', 3, 'y'], lines: [0, 2] },
    { id: 'I0002', index: 1, title: 'b', status: 'DROPPED', tags: [], lines: [3, 5] },
    { id: 'I0003', index: 2, title: 'c', status: 'weird', lines: [6] }, // lines が 1 つ: 捨てる
    null,
    { index: 4 }, // id が無い: 捨てる
  ]);
  assert.deepEqual(
    parsed.map((found) => [found.id, found.status, found.tags]),
    [
      ['I0001', 'inbox', ['x', 'y']],
      ['I0002', 'dropped', []],
    ],
  );
});

test('idea: 既定の絞り込みは dropped を隠し、絞り込みで出せる（§5.7）', async () => {
  const { visibleRows } = await ideas();
  const rows = [row('I1', 0, 'inbox'), row('I2', 1, 'dropped'), row('I3', 2, 'promoted')];
  assert.deepEqual(visibleRows(rows, 'active', '').map((found) => found.id), ['I1', 'I3']);
  assert.deepEqual(visibleRows(rows, 'dropped', '').map((found) => found.id), ['I2']);
  assert.deepEqual(visibleRows(rows, 'inbox', '').map((found) => found.id), ['I1']);
});

test('idea: タグは部分一致で絞り、並びは ideas.md の順のまま', async () => {
  const { visibleRows } = await ideas();
  const rows = [row('I1', 0, 'inbox', ['会話']), row('I2', 1, 'inbox', ['地図', '会話']), row('I3', 2, 'keep', ['景色'])];
  assert.deepEqual(visibleRows(rows, 'active', '会').map((found) => found.id), ['I1', 'I2']);
  assert.deepEqual(visibleRows(rows, 'active', ' 景 ').map((found) => found.id), ['I3']);
  assert.deepEqual(visibleRows(rows, 'active', '').map((found) => found.id), ['I1', 'I2', 'I3']);
});

test('idea: 選んでいる行が見えなければ先頭、見えるものが無ければ null', async () => {
  const { currentRow, visibleRows } = await ideas();
  const rows = [row('I1', 0, 'inbox'), row('I2', 1, 'dropped')];
  const shown = visibleRows(rows, 'active', '');
  assert.equal(currentRow(shown, 'I2')?.id, 'I1');
  assert.equal(currentRow(shown, 'I1')?.id, 'I1');
  assert.equal(currentRow([], 'I1'), null);
});

test('idea: 一覧の移動は端で止まり、見えていない選択からは先頭へ', async () => {
  const { moveSelection } = await ideas();
  const shown = [row('I1', 0, 'inbox'), row('I2', 1, 'inbox'), row('I3', 2, 'inbox')];
  assert.equal(moveSelection(shown, 'I1', 'next'), 'I2');
  assert.equal(moveSelection(shown, 'I2', 'prev'), 'I1');
  assert.equal(moveSelection(shown, 'I1', 'prev'), null);
  assert.equal(moveSelection(shown, 'I3', 'next'), null);
  assert.equal(moveSelection(shown, 'gone', 'next'), 'I1');
  assert.equal(moveSelection(shown, 'I1', 'last'), 'I3');
  assert.equal(moveSelection(shown, 'I3', 'first'), 'I1');
  assert.equal(moveSelection([], 'I1', 'next'), null);
});

test('idea: 捨てたあとは、捨てた位置より後ろの最初の行（無ければ最後）を選ぶ', async () => {
  const { nextAfterDrop } = await ideas();
  const after = [row('I1', 0, 'inbox'), row('I3', 2, 'inbox')];
  assert.equal(nextAfterDrop(after, 1), 'I3');
  assert.equal(nextAfterDrop(after, 5), 'I3');
  assert.equal(nextAfterDrop([row('I1', 0, 'inbox')], 3), 'I1');
  assert.equal(nextAfterDrop([], 0), null);
});

test('idea: 本文は見出しの次の行から、範囲の終わりまで（見出しは含めない）', async () => {
  const { bodyOf } = await ideas();
  const text = '# 一\n- 見出し\nstatus: inbox\n\nの本文\n\n# 二\n本文2\n';
  assert.equal(bodyOf(text, { lines: [0, 5] }), '- 見出し\nstatus: inbox\n\nの本文');
  assert.equal(bodyOf(text, { lines: [0, 1] }), '');
  assert.equal(bodyOf(text, { lines: [6, 99] }), '本文2\n');
});

test('idea: 即時入力の 1 行は改行を空白にし、空なら積まない', async () => {
  const { captureTitle } = await ideas();
  assert.equal(captureTitle('  猫が\n  歩く  '), '猫が 歩く');
  assert.equal(captureTitle('   \n  '), null);
  assert.equal(captureTitle(''), null);
});

test('idea: 昇格先は今の節・人物がある時だけ押せ、資料は未提供（§10.4）', async () => {
  const { promoteChoices } = await ideas();
  const none = promoteChoices('', []);
  assert.deepEqual(
    none.map((choice) => [choice.target, choice.available]),
    [
      ['plot', false],
      ['term', true],
      ['character', false],
      ['material', false],
    ],
  );
  assert.equal(none[0].labelKey, 'idea.promote_plot_none');
  assert.equal(none[2].labelKey, 'idea.no_characters');

  const full = promoteChoices('01-02', [
    { index: 0, name: '太郎' },
    { index: 3, name: '花子' },
  ]);
  assert.equal(full[0].to, '01-02');
  assert.deepEqual(full[0].labelArgs, ['01-02']);
  const people = full.filter((choice) => choice.target === 'character');
  assert.deepEqual(people.map((choice) => choice.to), ['0', '3']);
  assert.ok(people.every((choice) => choice.available));
  assert.equal(full.at(-1)?.available, false, '資料への昇格は押せない');
});

test('idea: キーの写像（この画面が横取りするものだけ）', async () => {
  const { ideaAction } = await ideas();
  assert.equal(ideaAction('Ctrl+Enter'), 'promote');
  assert.equal(ideaAction('Ctrl+Delete'), 'drop');
  assert.equal(ideaAction('Alt+ArrowUp'), 'prev');
  assert.equal(ideaAction('Alt+ArrowDown'), 'next');
  assert.equal(ideaAction('Ctrl+Home'), 'first');
  assert.equal(ideaAction('Ctrl+End'), 'last');
  assert.equal(ideaAction('Ctrl+2'), 'focus-editor');
  // 全体の割当（即時入力・パレット）はこの画面は触らない。
  assert.equal(ideaAction('Ctrl+Shift+I'), null);
  assert.equal(ideaAction('Ctrl+P'), null);
});

test('idea: 仮想化の窓と「見える位置までだけ動く」スクロール（§10.4.1・§9.7.7）', async () => {
  const { ROW_HEIGHT, revealOffset, windowOf } = await ideas();
  assert.deepEqual(windowOf(0, 0, 300), { start: 0, end: 0 });
  // 1000 行・先頭: 表示 300px（11 行）+ 余白 6 行
  assert.deepEqual(windowOf(1000, 0, 300), { start: 0, end: 17 });
  // 途中へ送っても、描く行は表示の前後だけ
  const mid = windowOf(1000, 500 * ROW_HEIGHT, 300);
  assert.equal(mid.start, 494);
  assert.equal(mid.end, 517);
  // 件数が少なければ全部
  assert.deepEqual(windowOf(3, 0, 300), { start: 0, end: 3 });

  // 見えている範囲の中なら動かさない
  assert.equal(revealOffset(5, 0, 300), 0);
  // 下に外れたら、ちょうど下端に合わせる（中央寄せしない）
  assert.equal(revealOffset(20, 0, 300), 21 * ROW_HEIGHT - 300);
  // 上に外れたら、ちょうど上端に合わせる
  assert.equal(revealOffset(2, 10 * ROW_HEIGHT, 300), 2 * ROW_HEIGHT);
});

// ---------------------------------------------------------------- material

test('material: 状態・絞り込み・表示名の写し', async () => {
  const { statusOf, filterToken, filterLabelKey, statusLabelKey, MATERIAL_FILTERS } = await materials();
  assert.equal(statusOf('approved'), 'approved');
  assert.equal(statusOf('Approved'), null, '表に無い値は null（第 1 と同じく大文字は読まない）');
  assert.equal(filterToken('active'), '');
  assert.equal(filterToken('rejected'), '!rejected');
  assert.deepEqual([...MATERIAL_FILTERS], ['active', 'proposed', 'approved', 'rejected']);
  assert.equal(filterLabelKey('active'), 'material.filter_active');
  assert.equal(statusLabelKey(null), 'material.status_unknown');
  assert.equal(statusLabelKey('proposed'), 'material.status_proposed');
});

test('material: 索引に無い資料 MD（W064）だけを取り出し、パスは重複させない', async () => {
  const { unregisteredOf } = await materials();
  const found = unregisteredOf([
    { code: 'W064', path: 'materials/inbox/x.md', message: 'm' },
    { code: 'E001', path: 'a.md' },
    { code: 'W064', path: 'materials/inbox/x.md' },
    { code: 'W064', path: 'materials/docs/y.md' },
  ]);
  assert.deepEqual(
    found.map((entry) => [entry.key, entry.id, entry.title, entry.status]),
    [
      ['materials/inbox/x.md', null, 'inbox/x.md', null],
      ['materials/docs/y.md', null, 'docs/y.md', null],
    ],
  );
});

test('material: 検索の結果は行にする。題名が空なら ID を出す', async () => {
  const { rowsOf } = await materials();
  const rows = rowsOf([
    { id: 'M0001', title: '猫', path: 'materials/docs/a.md', status: 'approved', category: '取材' },
    { id: 'M0002', title: '', path: 'materials/inbox/b.md', status: 'weird', category: '' },
    { title: 'no id' },
  ]);
  assert.deepEqual(
    rows.map((entry) => [entry.key, entry.title, entry.status, entry.category]),
    [
      ['M0001', '猫', 'approved', '取材'],
      ['M0002', 'M0002', null, ''],
    ],
  );
});

test('material: 一覧は既定の絞り込みのときだけ索引外の行を先頭に足す（§16.3・B-134）', async () => {
  const { listOf, rowsOf, unregisteredOf } = await materials();
  const loose = unregisteredOf([{ code: 'W064', path: 'materials/inbox/tiger.md' }]);
  const registered = rowsOf([{ id: 'M0001', title: '猫', path: 'materials/docs/a.md', status: 'approved' }]);
  assert.deepEqual(
    listOf(loose, registered, 'active', '').map((entry) => entry.key),
    ['materials/inbox/tiger.md', 'M0001'],
  );
  // 絞り込み（採用だけ）では索引外は出さない
  assert.deepEqual(listOf(loose, registered, 'approved', '').map((entry) => entry.key), ['M0001']);
  // 検索語はパスの部分一致で索引外も絞る
  assert.deepEqual(listOf(loose, [], 'active', 'zebra'), []);
  assert.deepEqual(listOf(loose, [], 'active', 'TIGER').map((entry) => entry.key), ['materials/inbox/tiger.md']);
});

test('material: 検索語は §16.3 の語に絞り込みの語を足す（既定は足さない）', async () => {
  const { searchText } = await materials();
  assert.equal(searchText('  猫 #取材 ', 'active'), '猫 #取材');
  assert.equal(searchText('', 'proposed'), '!proposed');
  assert.equal(searchText('猫', 'rejected'), '猫 !rejected');
});

test('material: 新しい資料の題名と分類（draft_of の写し。演算子は捨てる）', async () => {
  const { draftOf } = await materials();
  assert.deepEqual(draftOf('#取材 猫 の 話'), { title: '猫 の 話', category: '取材' });
  // 演算子（M0137・@範囲・!状態・~日付・'参照）は題名に入れず、最初の #分類だけを使う。
  assert.deepEqual(draftOf('猫 M0137 @02-03 !approved ~2026 \'名 #文献 #Web'), {
    title: '猫',
    category: '文献',
  });
  assert.deepEqual(draftOf('   '), { title: '', category: '' });
});

test('material: 出典の有無（W065 の条件。kind: search は出典そのもの）', async () => {
  const { hasNoSource } = await materials();
  assert.equal(hasNoSource(undefined), true);
  assert.equal(hasNoSource({ kind: 'custom', text: '  ' }), true);
  assert.equal(hasNoSource({ kind: 'url', url: '', text: '' }), true);
  assert.equal(hasNoSource({ kind: 'url', url: 'https://example.com' }), false);
  assert.equal(hasNoSource({ kind: 'custom', text: '取材 2026-10' }), false);
  assert.equal(hasNoSource({ kind: 'search', query: '' }), false);
});

test('material: 審査の差分と読み（materials.get の review）', async () => {
  const { reviewOf, sameReview } = await materials();
  const base = reviewOf({ title: 't', category: '取材', summary: '', tags: 'a, b', conditions: 'tag: x', source: 's' });
  assert.deepEqual(base, { title: 't', category: '取材', summary: '', tags: 'a, b', conditions: 'tag: x', source: 's' });
  assert.deepEqual(reviewOf(undefined), {
    title: '',
    category: '',
    summary: '',
    tags: '',
    conditions: '',
    source: '',
  });
  assert.equal(sameReview(base, { ...base }), true);
  assert.equal(sameReview(base, { ...base, summary: 'x' }), false);
});

test('material: 却下や移動のあとの選び方（同じ位置、無ければ最後）', async () => {
  const { moveKey, nextKeyAfterRemoval } = await materials();
  const rows = ['M1', 'M2', 'M3'].map((key) => ({
    key,
    id: key,
    title: '',
    path: '',
    status: null,
    category: '',
  }));
  // M2 を却下して消えた（位置 1）→ 同じ位置の行（今は M3）
  assert.equal(nextKeyAfterRemoval([rows[0], rows[2]], 1), 'M3');
  // 最後の行を消した → 新しい最後
  assert.equal(nextKeyAfterRemoval(rows.slice(0, 2), 9), 'M2');
  assert.equal(nextKeyAfterRemoval([], 0), null);
  assert.equal(moveKey(rows, 'M1', 'next'), 'M2');
  assert.equal(moveKey(rows, 'M1', 'prev'), null);
  assert.equal(moveKey(rows, 'M3', 'next'), null);
  assert.equal(moveKey(rows, null, 'next'), 'M1');
});

test('material: キーの写像（採用・却下・新規・フォーカス）', async () => {
  const { materialAction } = await materials();
  assert.equal(materialAction('Ctrl+Enter'), 'approve');
  assert.equal(materialAction('Ctrl+Delete'), 'reject');
  assert.equal(materialAction('Ctrl+Shift+N'), 'new');
  assert.equal(materialAction('Ctrl+Alt+Shift+N'), 'new', 'Web の代替キー（§9.8.6）');
  assert.equal(materialAction('Ctrl+2'), 'focus-editor');
  assert.equal(materialAction('Ctrl+3'), 'focus-form');
  assert.equal(materialAction('Ctrl+M'), null, '検索は全体の割当のまま');
});

test('material: 引用注釈の探し方と分類の候補', async () => {
  const { categoryOptions, citationQuery } = await materials();
  assert.equal(citationQuery('M0137'), '^M0137');
  assert.deepEqual(categoryOptions('取材,文献,Web', '取材'), ['取材', '文献', 'Web']);
  assert.deepEqual(categoryOptions('取材,文献', '独自'), ['独自', '取材', '文献'], '今の値を消さない');
  assert.deepEqual(categoryOptions('', ''), []);
});

// ---------------------------------------------------------------- 配線の約束

test('配線: 画面が呼ぶ正規操作は、カタログ（OP_IDS）に実在する', async () => {
  const { OP_IDS } = await bridgeCatalog();
  const known = new Set(OP_IDS);
  const used = new Set();
  for (const file of LANE_FILES) {
    const text = await readFile(join(WEBUI, file), 'utf8');
    for (const match of text.matchAll(/session\.run\(\s*'([a-z_.]+)'/gu)) {
      used.add(match[1]);
    }
  }
  assert.ok(used.size >= 10, `画面が呼ぶ操作を集める（${[...used].join(', ')}）`);
  for (const op of used) {
    assert.ok(known.has(op), `カタログに無い操作: ${op}`);
  }
});

test('配線: 画面の文言のキーは ja と en の両方に実在する（§12.3）', async () => {
  const { CATALOGS } = await i18nCatalog();
  const keys = new Set();
  for (const file of LANE_FILES) {
    const text = await readFile(join(WEBUI, file), 'utf8');
    for (const match of text.matchAll(/\bt\(\s*'([a-z0-9_.]+)'/gu)) {
      keys.add(match[1]);
    }
  }
  // 規則の関数が返すキー（動的な組み立て）も集める。
  const ideaRules = await ideas();
  const materialRules = await materials();
  for (const filter of ['active', 'inbox', 'keep', 'promoted', 'dropped']) {
    keys.add(ideaRules.filterLabelKey(filter));
  }
  for (const status of ['inbox', 'keep', 'promoted', 'dropped']) {
    keys.add(ideaRules.statusLabelKey(status));
  }
  for (const choice of ideaRules.promoteChoices('01-02', [{ index: 0, name: 'x' }])) {
    keys.add(choice.labelKey);
  }
  for (const choice of ideaRules.promoteChoices('', [])) {
    keys.add(choice.labelKey);
  }
  for (const filter of ['active', 'proposed', 'approved', 'rejected']) {
    keys.add(materialRules.filterLabelKey(filter));
  }
  for (const status of ['proposed', 'approved', 'rejected', null]) {
    keys.add(materialRules.statusLabelKey(status));
  }
  assert.ok(keys.size >= 40, `キーを集める（${keys.size} 件）`);
  for (const locale of ['ja', 'en']) {
    for (const key of keys) {
      assert.ok(
        Object.prototype.hasOwnProperty.call(CATALOGS[locale], key) && CATALOGS[locale][key] !== '',
        `${locale} に無い文言のキー: ${key}`,
      );
    }
  }
});

test('配線: 生色を書かない（§11.1。色は CSS 変数だけ。影の黒だけは既存の書き方に合わせる）', async () => {
  for (const file of LANE_FILES.filter((found) => found.endsWith('.svelte'))) {
    const text = await readFile(join(WEBUI, file), 'utf8');
    const style = text.slice(text.indexOf('<style>'));
    assert.doesNotMatch(style, /#[0-9a-fA-F]{3,8}\b/u, `${file} に 16 進の色`);
    const withoutShadow = style.replace(/rgb\(0 0 0 \/ 32%\)/gu, '');
    assert.doesNotMatch(withoutShadow, /rgba?\(/u, `${file} に rgb の色`);
    assert.doesNotMatch(withoutShadow, /hsla?\(/u, `${file} に hsl の色`);
  }
});

test('配線: 規則のファイルは実行時の import を持たない（単独で読める）', async () => {
  for (const file of ['src/lib/workspaces/idea/ideas.ts', 'src/lib/workspaces/material/materials.ts']) {
    const text = await readFile(join(WEBUI, file), 'utf8');
    assert.doesNotMatch(text, /^import\s/mu, `${file} に import`);
  }
});
