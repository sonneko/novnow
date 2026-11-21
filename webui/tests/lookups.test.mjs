// 参照の引き口（`materials.attach` / `cast.get` / `dictionary.list` / `dictionary.term_full` /
// `diagnostics.place`）の回帰（Node。`webui/src/lib/reference/` と配線先を読む）。
//
// 見るのは 4 つ:
//
// 1. 規則（`lookups.ts`）: 機能核の戻り値の読み方（`cast` のブロック・辞書の行・用語・どこ）と、
//    `materials.attach` の 2 つの腕（登録 / 添付）の引数の分け方。
// 2. **機能核のソースとの突き合わせ**: `place.*` の表・`cast_entry` の見出し・`materials_attach`
//    の `path` の分岐を、Rust のソースから読んで確かめる（画面が意味を作っていないこと）。
// 3. 配線: 5 つを画面が実際に `run('<操作 ID>')` で呼び、呼び口（参照ペインのダブルクリック・
//    ヘッダのボタン・点検の診断の行・資料の画面のボタン）が繋がっている。
// 4. パリティ判定との一致: `coverage.ts` の `PENDING` から 5 件が外れ、バッチ6 で 0 件になる。
//
// TypeScript は `tsc` で 1 ファイルずつ書き出してから読む（`sections.test.mjs` と同じ方式）。

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

const WEBUI = new URL('..', import.meta.url).pathname;
const REPO = new URL('../../', import.meta.url).pathname;

/** 書き出し済みの結果（1 ファイル 1 回だけ `tsc` を回す）。 */
const compiled = new Map();

function importTs(relative) {
  if (!compiled.has(relative)) {
    compiled.set(relative, compileTs(relative));
  }
  return compiled.get(relative);
}

async function compileTs(relative) {
  const out = await mkdtemp(join(tmpdir(), 'novnow-lookups-'));
  try {
    execFileSync(
      process.execPath,
      [
        join(WEBUI, 'node_modules', 'typescript', 'bin', 'tsc'),
        join(WEBUI, relative),
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

const lookups = () => importTs('src/lib/reference/lookups.ts');

/** `webui/` の相対パスを読む。 */
function read(relative) {
  return readFile(join(WEBUI, relative), 'utf8');
}

/** リポジトリの相対パスを読む。 */
function readRepo(relative) {
  return readFile(join(REPO, relative), 'utf8');
}

// ------------------------------------------------------------------ 1. 規則

test('cast.get: `- name:` で人を分け、知らない見出しも捨てない', async () => {
  const { castEntries } = await lookups();
  const block = [
    '- name: リリア',
    '  mode: B 冷静な観察者 / C 決意',
    '  personality: 呆れながらも見捨てない。',
    '  emotion: 好奇心が警戒心を上回りつつある。',
    '  role: 主人公の暴走を止める者。',
    '  role-language: 「*なのよ」「*あげるわよ」',
    '- name: 葵',
    '  mode: A 日常',
    '  unknown-field: そのまま残す',
  ].join('\n');
  const rows = castEntries(block);
  assert.deepEqual(
    rows.map((row) => row.name),
    ['リリア', '葵'],
  );
  assert.deepEqual(
    rows[0].fields.map((field) => field.key),
    ['name', 'mode', 'personality', 'emotion', 'role', 'role-language'],
  );
  assert.equal(
    rows[0].fields.find((field) => field.key === 'personality')?.value,
    '呆れながらも見捨てない。',
  );
  // 知らない見出しは落とさない（機能核が項目を増やしても画面が黙って消さない）。
  assert.equal(rows[1].fields.find((field) => field.key === 'unknown-field')?.value, 'そのまま残す');
  assert.deepEqual(castEntries(''), []);
  assert.deepEqual(castEntries(undefined), []);
  // 人の前の値は誰のものか決められないので捨てる。
  assert.deepEqual(castEntries('  mode: 迷子'), []);
});

test('cast.get: 見出しの文言は既存のキーへ写し、知らない見出しは空（画面は生の見出しを出す）', async () => {
  const { castFieldLabel } = await lookups();
  assert.equal(castFieldLabel('mode'), 'cast.tab_modes');
  assert.equal(castFieldLabel('personality'), 'pane.personality');
  assert.equal(castFieldLabel('emotion'), 'pane.emotion');
  assert.equal(castFieldLabel('role'), 'pane.role');
  assert.equal(castFieldLabel('role-language'), 'pane.tone');
  assert.equal(castFieldLabel('unknown-field'), '');
});

test('dictionary.list: 壊れた行は捨て、並べ替えない', async () => {
  const { dictionaryEntries } = await lookups();
  const rows = dictionaryEntries([
    { key: 'リリア', kind: 'Character', normalized: 'りりあ' },
    { key: '', kind: 'Word' },
    null,
    { kind: 'Word' },
    { key: '題名', kind: 'MaterialTitle', normalized: 'だいめい' },
  ]);
  assert.deepEqual(
    rows.map((row) => row.key),
    ['リリア', '題名'],
  );
  assert.deepEqual(dictionaryEntries(undefined), []);
});

test('dictionary.list: 種別の文言は既存のキー（`api.op.*` を画面に並べない）', async () => {
  const { dictionaryKindLabel, isDictionaryKind, DICTIONARY_KINDS } = await lookups();
  assert.deepEqual([...DICTIONARY_KINDS], ['Character', 'Address', 'Word', 'Material', 'MaterialTitle']);
  assert.equal(dictionaryKindLabel('Character'), 'pane.cast');
  assert.equal(dictionaryKindLabel('Address'), 'stage.addresses');
  assert.equal(dictionaryKindLabel('Word'), 'pane.words');
  assert.equal(dictionaryKindLabel('Material'), 'pane.materials');
  assert.equal(dictionaryKindLabel('MaterialTitle'), 'pane.materials');
  assert.equal(dictionaryKindLabel('Other'), '');
  assert.equal(isDictionaryKind('Word'), true);
  assert.equal(isDictionaryKind('Other'), false);
});

test('dictionary.list: 絞り込みは全角半角・大文字小文字を問わず、空なら全件', async () => {
  const { filterDictionary } = await lookups();
  const rows = [
    { key: 'リリア', kind: 'Character', normalized: 'りりあ' },
    { key: 'ＡＢＣ', kind: 'Word', normalized: 'abc' },
    { key: '猫', kind: 'Word', normalized: 'ねこ' },
  ];
  assert.deepEqual(
    filterDictionary(rows, '').map((row) => row.key),
    ['リリア', 'ＡＢＣ', '猫'],
  );
  assert.deepEqual(
    filterDictionary(rows, 'りり').map((row) => row.key),
    ['リリア'],
  );
  assert.deepEqual(
    filterDictionary(rows, 'ａｂ').map((row) => row.key),
    ['ＡＢＣ'],
    '全角で打っても半角の正規化に当たる',
  );
  assert.deepEqual(filterDictionary(rows, '  ').length, 3);
});

test('dictionary.term_full: 語と定義（空の定義も残す）', async () => {
  const { termHits } = await lookups();
  const rows = termHits([
    { term: '猫', definition: '主人公の飼い猫。' },
    { term: '犬', definition: '' },
    { term: '' },
    null,
  ]);
  assert.deepEqual(rows, [
    { term: '猫', definition: '主人公の飼い猫。' },
    { term: '犬', definition: '' },
  ]);
  assert.deepEqual(termHits(undefined), []);
});

test('diagnostics.place: `place` の種別ごとに place.* の文言へ写す（機能核の表と同じ）', async () => {
  const { placeRef, placeKey } = await lookups();
  const key = (value) => {
    const found = placeRef(value);
    return found === null ? null : placeKey(found);
  };
  assert.deepEqual(key({ kind: 'contents', id: '01-04-02-03' }), {
    key: 'place.contents',
    args: ['01-04-02-03'],
  });
  assert.deepEqual(key({ kind: 'chapter_plot', id: '' }), { key: 'place.chapter_plot_head', args: [] });
  assert.deepEqual(key({ kind: 'whole_plot', id: '01' }), { key: 'place.whole_plot', args: ['01'] });
  assert.deepEqual(key({ kind: 'character', name: 'リリア' }), {
    key: 'place.character',
    args: ['リリア'],
  });
  assert.deepEqual(key({ kind: 'character', name: 'リリア', mode: 'B 冷静' }), {
    key: 'place.character_mode',
    args: ['リリア', 'B 冷静'],
  });
  assert.deepEqual(key({ kind: 'word', term: '猫' }), { key: 'place.word', args: ['猫'] });
  assert.deepEqual(key({ kind: 'material', id: 'M0137' }), {
    key: 'place.material_id',
    args: ['M0137'],
  });
  assert.deepEqual(key({ kind: 'material', id: 'M0137', title: '猫' }), {
    key: 'place.material',
    args: ['M0137', '猫'],
  });
  assert.deepEqual(key({ kind: 'prompt', role: 'writer', section: 'cast' }), {
    key: 'place.prompt_section',
    args: ['writer', 'cast'],
  });
  assert.deepEqual(key({ kind: 'settings' }), { key: 'place.settings', args: [] });
  // 決まらないもの・知らない種別は null（画面は生のパスを出す。推測で埋めない）。
  assert.equal(placeRef(null), null);
  assert.equal(placeRef({}), null);
  assert.equal(key({ kind: 'unknown' }), null);
});

test('diagnostics.place: 位置が無ければ pos を送らない（0 を送ると先頭を指す）', async () => {
  const { placeArgs } = await lookups();
  assert.deepEqual(placeArgs('contents/01.cnov', 42), { path: 'contents/01.cnov', pos: 42 });
  assert.deepEqual(placeArgs('contents/01.cnov', 0), { path: 'contents/01.cnov', pos: 0 });
  assert.deepEqual(placeArgs('contents/01.cnov', null), { path: 'contents/01.cnov' });
  assert.deepEqual(placeArgs('contents/01.cnov', -1), { path: 'contents/01.cnov' });
  assert.equal(placeArgs('  ', 1), null, '空のパスは送らない（必須引数）');
});

test('materials.attach: 添付（path なし）と登録（path あり）を分ける', async () => {
  const { attachArgs, attachAnchor, attachResult } = await lookups();
  assert.deepEqual(attachArgs('M0001'), { id: 'M0001', role: '' });
  assert.deepEqual(attachArgs('M0001', 'materials/inbox/x.md'), {
    id: 'M0001',
    path: 'materials/inbox/x.md',
    role: '',
  });
  // 空の path は「添付」と同じ（path を送らない）。
  assert.deepEqual(attachArgs('M0001', '   '), { id: 'M0001', role: '' });
  assert.equal(attachArgs(''), null, 'id は必須（空は送らない）');
  assert.equal(attachArgs('  ', 'a.md'), null);
  // 索引に 1 件も無ければ登録の錨が無い（押せない）。
  assert.equal(attachAnchor([null, null]), null);
  assert.equal(attachAnchor([null, 'M0002']), 'M0002');
  assert.deepEqual(attachResult({ attached: 'M0009', path: 'materials/inbox/x.md' }), {
    attached: 'M0009',
    path: 'materials/inbox/x.md',
  });
  assert.deepEqual(attachResult(undefined), { attached: '', path: '' });
});

// ------------------------------------------------------------------ 2. 機能核との突き合わせ

test('機能核: `materials_attach` は path の有無で「登録」と「添付」に分かれる', async () => {
  const source = await readRepo('src/app/ops/knowledge.rs');
  const at = source.indexOf('pub fn materials_attach');
  assert.ok(at > 0, 'materials_attach が見つからない');
  const body = source.slice(at, at + 3000);
  assert.match(body, /if let Some\(path\) = path\.as_deref\(\)/, 'path の分岐がある');
  assert.match(body, /materials_ops::register\(/, '登録の腕が register を呼ぶ');
  assert.match(body, /materials_ops::touch\(/, '添付の腕が touch を呼ぶ');
  // 返す `attached` は登録のとき**新しい ID**（画面はそれを通知に出す）。
  assert.match(body, /"attached": created\.id/);
});

test('機能核: `place` の種別と文言の対応が画面の写しと同じ', async () => {
  const rust = await readRepo('src/ui/i18n/mod.rs');
  // 表は「Plase の種別 → place.* のキー」を 1 か所（`src/ui/i18n/mod.rs` の `place`）に持つ。
  const at = rust.indexOf('pub fn place(locale: Locale');
  assert.ok(at > 0, '`place` が見つからない');
  const body = rust.slice(at, rust.indexOf('/// `{0}`', at));
  for (const [kind, key] of [
    ['Contents', 'place.contents'],
    ['EpisodePlot', 'place.episode_plot'],
    ['ChapterPlot', 'place.chapter_plot'],
    ['WholePlot', 'place.whole_plot'],
    ['Character', 'place.character'],
    ['Word', 'place.word'],
    ['Idea', 'place.idea'],
    ['Material', 'place.material'],
    ['Prompt', 'place.prompt'],
    ['Settings', 'place.settings'],
  ]) {
    assert.match(body, new RegExp(`Place::${kind}`), kind);
    assert.match(body, new RegExp(key.replace('.', '\\.')), `${kind} の ${key}`);
  }
  // 機能核の `place_json` の種別名（`kind` の値）と、画面の `placeRef` が読む名前を突き合わせる。
  const ops = await readRepo('src/app/ops/lang.rs');
  for (const kind of [
    'contents',
    'episode_plot',
    'chapter_plot',
    'whole_plot',
    'character',
    'word',
    'idea',
    'material',
    'prompt',
    'settings',
  ]) {
    assert.match(ops, new RegExp(`"kind": "${kind}"`), `place_json の ${kind} が変わった`);
  }
});

test('機能核: `cast_entry` の見出しは 5 つ（画面の写しと同じ）', async () => {
  const packet = await readRepo('src/core/aicontext/packet.rs');
  const at = packet.indexOf('fn cast_entry');
  assert.ok(at > 0, 'cast_entry が見つからない');
  const body = packet.slice(at, at + 900);
  for (const key of ['mode', 'personality', 'emotion', 'role', 'role-language']) {
    assert.match(body, new RegExp(`push\\("${key}"`), key);
  }
  const { castFieldLabel } = await lookups();
  // 画面の写し（`castFieldLabel`）が、機能核の 5 つをすべて知っている。
  for (const key of ['mode', 'personality', 'emotion', 'role', 'role-language']) {
    assert.notEqual(castFieldLabel(key), '', `${key} の文言が無い`);
  }
});

test('機能核: `dictionary_list` の種別名は §6.1.1 の 5 つ', async () => {
  const dictionary = await readRepo('src/core/resolve/dictionary.rs');
  const at = dictionary.indexOf('pub enum EntryKind');
  assert.ok(at > 0);
  const body = dictionary.slice(at, at + 1200);
  const { DICTIONARY_KINDS } = await lookups();
  for (const kind of DICTIONARY_KINDS) {
    assert.match(body, new RegExp(`\\b${kind},`), `EntryKind に ${kind} が無い`);
  }
});

// ------------------------------------------------------------------ 3. 配線

test('配線: 5 つの操作を画面が実際に run で呼ぶ（コメントだけにしない）', async () => {
  const app = await read('src/App.svelte');
  assert.match(app, /run\('materials\.attach'/);
  assert.match(app, /run\('cast\.get'/);
  assert.match(app, /run\('dictionary\.list'/);
  assert.match(app, /run\('dictionary\.term_full'/);
  assert.match(app, /run\('diagnostics\.place'/);
});

test('配線: 参照ペインのダブルクリックが面へ届く（3 段の受け渡し）', async () => {
  const pane = await read('src/lib/reference/ReferencePane.svelte');
  const write = await read('src/lib/workspaces/write/WriteWorkspace.svelte');
  const shell = await read('src/lib/shell/AppShell.svelte');
  const app = await read('src/App.svelte');
  assert.match(pane, /on:dblclick=\{\(\) => onopen\?\.\(row\.kind, row\.value\)\}/);
  assert.match(write, /\{onopen\}/);
  assert.match(shell, /\{onopen\}/);
  assert.match(app, /onopen=\{openRow\}/);
  // ダブルクリックの行き先は配役（`cast.get`）と用語（`dictionary.term_full`）。
  assert.match(app, /async function openRow/);
  assert.match(app, /if \(kind === 'cast'\) \{\s*await openCast\(\)/);
  assert.match(app, /if \(kind === 'word'\) \{/);
});

test('配線: 辞書の一覧はヘッダのボタンから開ける（キーを知らなくても押せる）', async () => {
  const header = await read('src/lib/shell/Header.svelte');
  const shell = await read('src/lib/shell/AppShell.svelte');
  const app = await read('src/App.svelte');
  assert.match(header, /ondictionary\?\.\(\)/);
  assert.match(shell, /\{ondictionary\}/);
  assert.match(app, /ondictionary=\{\(\) => void openDictionary\(\)\}/);
});

test('配線: 点検の診断の行が機能核の「どこ」を出す（`diagnostics.place`）', async () => {
  const review = await read('src/lib/workspaces/review/ReviewWorkspace.svelte');
  const shell = await read('src/lib/shell/AppShell.svelte');
  const app = await read('src/App.svelte');
  assert.match(review, /export let onplace:/);
  assert.match(review, /await onplace\(row\.path, row\.byte\)/);
  assert.match(review, /placeKeyOf\(where\.place\)/);
  assert.match(shell, /\{onplace\}/);
  assert.match(app, /onplace=\{placeOfDiag\}/);
});

test('配線: 資料の添付は資料の画面のボタンから呼ぶ（索引外の行は登録）', async () => {
  const material = await read('src/lib/workspaces/material/MaterialWorkspace.svelte');
  const shell = await read('src/lib/shell/AppShell.svelte');
  const app = await read('src/App.svelte');
  assert.match(material, /export let onattach:/);
  assert.match(material, /on:click=\{\(\) => void attach\(\)\}/);
  assert.match(material, /target\.id === null \? attachArgs\(anchor, target\.path\) : attachArgs\(target\.id\)/);
  assert.match(shell, /<MaterialWorkspace \{t\} \{session\} \{revision\} \{onattach\} focusId=\{materialFocusId\} onfocusdone=\{onfocusdone\} \/>/);
  assert.match(app, /onattach=\{attachMaterial\}/);
});

test('配線: 9 つのワークスペースをシェルが実際に描く（`write` 以外を空にしない）', async () => {
  const shell = await read('src/lib/shell/AppShell.svelte');
  for (const name of [
    'write',
    'plot-episode',
    'plot-outline',
    'idea',
    'material',
    'cast',
    'reader',
    'agent',
    'review',
  ]) {
    assert.match(
      shell,
      new RegExp(`screen\\.workspace === '${name.replace('-', '\\-')}'`),
      `${name} を描く分岐が無い`,
    );
  }
  // セッションが繋がっていないときだけ「使えません」を出す（黙って空にしない）。
  assert.match(shell, /session === null/);
  assert.match(shell, /workspace\.unavailable/);
});

// ------------------------------------------------------------------ 4. パリティ判定との一致

test('PENDING からバッチ4・5・6 の 24 件が外れ、0 件である', async () => {
  const { PENDING } = await importTs('src/lib/coverage.ts');
  for (const id of [
    'materials.attach',
    'cast.get',
    'dictionary.list',
    'dictionary.term_full',
    'diagnostics.place',
    'lang.tokens',
    'lang.hover',
    'lang.definition',
    'lang.references',
    'lang.symbols',
    'lang.format',
    'lang.rename_preview',
    'lang.rename_apply',
    'lang.folding',
    'agent.audit_list',
    'prompt.list',
    'prompt.get',
    'prompt.diff',
    'prompt.reset',
    'mail.send',
    'session.workspace_get',
    'help.topic',
  ]) {
    assert.ok(!PENDING.includes(id), `${id} がまだ未提供に残っている`);
  }
  assert.equal(PENDING.length, 0, `未提供は 0 件（バッチ6 で全解除）。いまは ${PENDING.length} 件`);
});

test('SPEC_GAPS から §11.3（診断の地名解決）が消えている', async () => {
  const { SPEC_GAPS } = await importTs('src/lib/coverage.ts');
  assert.ok(
    !SPEC_GAPS.some((gap) => gap.section === '11.3'),
    '§11.3 は解消した（`diagnostics.place` の表示が付いた）',
  );
});
