// L5 管制・点検（`agent` と `review`）の回帰（Node。`webui/src/lib/workspaces/{agent,review}/` を読む）。
//
// 見るのは 3 つ:
//
// 1. 規則（`agent.ts` / `review.ts`）: 4 列のキー写像・タスクの行と選択・コンテキストのブロックと
//    予算（§15.4.2）・提案の行と行差分・委譲レベル・まとめて実行の確認・点検の 9 タブ・診断一覧の
//    形と場所（place.*）・進捗ツリー・T スコア（§10.3.4）・文体観察の指標・挿絵メモ。
//    第 1 の画面（`REF:src/ui/workspaces/{agent.rs,review.rs,tabs.rs,proposals.rs}`）と同じ結果に
//    なることを、正本の規則から写したケースで確かめる。
// 2. 配線の約束（`.svelte`）: 呼ぶ正規操作は `OP_IDS` に実在する。画面の文言は `CATALOGS` の `ja`
//    と `en` の両方に実在する（§12.3）。生色を書かない（§11.1）。CSS 変数は `tokens` のものだけ。
// 3. 凍結の約束: 規則のファイルは実行時の import を持たない（テストが単独で読める形を保つ）。
//
// TypeScript は `tsc` で 1 ファイルずつ書き出してから読む（`knowledge.test.mjs` と同じ方式）。

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
  const out = await mkdtemp(join(tmpdir(), 'novnow-l5-'));
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

const agent = () => importTs('src/lib/workspaces/agent/agent.ts');
const review = () => importTs('src/lib/workspaces/review/review.ts');
const bridgeCatalog = () => importTs('src/lib/bridge/catalog.ts');
const i18nCatalog = () => importTs('src/lib/i18n/catalog.ts');
const tokens = () => importTs('src/lib/theme/tokens.ts');

/** 画面のファイル（L5 の所有。`.svelte` と規則のファイル）。 */
const LANE_FILES = [
  'src/lib/workspaces/agent/agent.ts',
  'src/lib/workspaces/agent/AgentWorkspace.svelte',
  'src/lib/workspaces/review/review.ts',
  'src/lib/workspaces/review/ReviewWorkspace.svelte',
];

/** 簡単な文言（記録用）。 */
const t = (key, args) => `${key}(${(args ?? []).join(',')})`;

// ---------------------------------------------------------------- agent.ts

test('agent: 4 列のキー写像（§15.7 の表）。列の外の文字キーは効かず、Ctrl+Shift+X は列を問わない', async () => {
  const rules = await agent();
  assert.equal(rules.agentAction('N', 'tasks'), 'new');
  assert.equal(rules.agentAction('R', 'tasks'), 'run');
  assert.equal(rules.agentAction('X', 'tasks'), 'stop');
  assert.equal(rules.agentAction('D', 'tasks'), 'dry-run');
  assert.equal(rules.agentAction('Space', 'tasks'), 'select');
  assert.equal(rules.agentAction('Ctrl+A', 'tasks'), 'select-all');
  assert.equal(rules.agentAction('Shift+R', 'tasks'), 'run-batch');
  assert.equal(rules.agentAction('Enter', 'tasks'), 'detail');
  assert.equal(rules.agentAction('Space', 'context'), 'toggle-exclude');
  assert.equal(rules.agentAction('D', 'context'), 'dry-run');
  assert.equal(rules.agentAction('A', 'review'), 'accept');
  assert.equal(rules.agentAction('P', 'review'), 'partial');
  assert.equal(rules.agentAction('X', 'review'), 'reject');
  assert.equal(rules.agentAction('M', 'review'), 'approve-messages');
  assert.equal(rules.agentAction('1', 'delegation'), 'level-off');
  assert.equal(rules.agentAction('2', 'delegation'), 'level-propose');
  assert.equal(rules.agentAction('3', 'delegation'), 'level-apply');
  assert.equal(rules.agentAction('4', 'delegation'), 'level-autonomous');
  assert.equal(rules.agentAction('5', 'delegation'), null);
  assert.equal(rules.agentAction('Ctrl+Shift+X', 'tasks'), 'stop-all');
  assert.equal(rules.agentAction('Ctrl+Shift+X', 'context'), 'stop-all');
  assert.equal(rules.agentAction('Ctrl+Shift+X', 'review'), 'stop-all');
  assert.equal(rules.agentAction('ArrowLeft', 'tasks'), 'left');
  assert.equal(rules.agentAction('ArrowRight', 'review'), 'right');
  assert.equal(rules.agentAction('ArrowUp', 'tasks'), 'prev');
  assert.equal(rules.agentAction('ArrowDown', 'tasks'), 'next');
  assert.equal(rules.agentAction('Home', 'context'), 'first');
  assert.equal(rules.agentAction('End', 'context'), 'last');
  assert.equal(rules.agentAction('N', 'context'), null);
  assert.equal(rules.agentAction('X', 'delegation'), null);
  assert.equal(rules.agentAction('A', 'tasks'), null);
  assert.equal(rules.moveColumn('tasks', 1), 'context');
  assert.equal(rules.moveColumn('tasks', -1), 'delegation');
  assert.equal(rules.levelOfKey('3'), 'apply');
  assert.equal(rules.levelOfKey('9'), null);
});

test('agent: タスクの行・状態の見た目・選択と移動（§15.7.1）', async () => {
  const rules = await agent();
  const rows = rules.taskRowsOf({
    tasks: [
      { id: 'T0001', role: 'writer', scope: '@01-01-01-01', status: 'running', intent: '書く', initiator: 'agent' },
      { id: 'T0002', role: 'writer', scope: '@01-01-01-02', status: 'ready', intent: '' },
      { broken: true },
    ],
  });
  assert.deepEqual(rows.map((row) => row.id), ['T0001', 'T0002']);
  assert.equal(rows[1].initiator, 'human');
  assert.equal(rules.statusTone('running'), 'run');
  assert.equal(rules.statusTone('ready'), 'wait');
  assert.equal(rules.statusTone('applied'), 'done');
  assert.equal(rules.statusTone('nope'), 'idle');
  assert.ok(rules.startedByAgent(rows[0]));
  assert.ok(!rules.startedByAgent(rows[1]));
  assert.deepEqual(rules.readyOf(rows).map((row) => row.id), ['T0002']);
  assert.deepEqual(rules.toggleSelect(['T0001'], 'T0002'), ['T0001', 'T0002']);
  assert.deepEqual(rules.toggleSelect(['T0001', 'T0002'], 'T0001'), ['T0002']);
  assert.equal(rules.moveInColumn(rows, 'T0001', 1), 'T0002');
  assert.equal(rules.moveInColumn(rows, 'T0002', 1), 'T0002');
  assert.equal(rules.moveInColumn(rows, 'T0002', 'last'), 'T0002');
  assert.equal(rules.moveInColumn(rows, null, 1), 'T0001');
  assert.deepEqual(rules.progressCounts(rows), { running: 1, waiting: 1, done: 0 });
  assert.equal(rules.sceneOfScope('@01-02-03-04'), '01-02-03-04');
  assert.equal(rules.sceneOfScope('01-02-03-04'), '01-02-03-04');
  assert.equal(rules.sceneOfScope('@01-02'), null);
});

test('agent: まとめて実行の確認（件数・合計入力・サーバ）。1 件でも作れなければ合計は出さない', async () => {
  const rules = await agent();
  const both = rules.runConfirm(['a', 'b'], new Map([['a', 100], ['b', 230]]));
  assert.equal(both.count, 2);
  assert.equal(both.chars, 330);
  assert.deepEqual(both.withoutPacket, []);
  const some = rules.runConfirm(['a', 'x'], new Map([['a', 100]]));
  assert.equal(some.chars, null);
  assert.deepEqual(some.withoutPacket, ['x']);
});

test('agent: コンテキストのブロック（§15.4.1）。本文の文字数と「…（N 文字省略）」の数え', async () => {
  const rules = await agent();
  const packet =
    'Novnow のコンテキスト\nscene: 01-02-03\n\n' +
    '## 1 project\nproject 本文\n## 2 outline\n## 3 plot\n## 4 target\n## 5 cast\n' +
    '## 6 lexicon\n## 7 materials\n…（12 文字省略）\n## 8 constraints\n## 9 history\nhistory 本文';
  const blocks = rules.packetBlocks(packet);
  assert.deepEqual(blocks.map((block) => [block.order, block.name]), [
    [1, 'project'],
    [2, 'outline'],
    [3, 'plot'],
    [4, 'target'],
    [5, 'cast'],
    [6, 'lexicon'],
    [7, 'materials'],
    [8, 'constraints'],
    [9, 'history'],
  ]);
  // 見出しより前（タイトル・scene:）はブロックにしない
  assert.ok(blocks.every((block) => !block.body.includes('Novnow') && !block.body.includes('scene:')));
  assert.equal(blocks.find((block) => block.name === 'materials')?.omitted, 12);
  assert.equal(blocks.find((block) => block.name === 'project')?.chars, [...'project 本文'].length);
});

test('agent: ブロック別の文字数予算（§15.4.2 の既定表）。cap で比率のまま伸び縮み、外せないブロックがある', async () => {
  const rules = await agent();
  const totals = Object.fromEntries(
    Object.entries(rules.BUDGETS).map(([role, list]) => [role, list.reduce((sum, value) => sum + value, 0)]),
  );
  assert.deepEqual(totals, {
    writer: 7900,
    plotter: 7000,
    stylist: 6600,
    logician: 9500,
    researcher: 5600,
    ideator: 5200,
    integrator: 6000,
  });
  // 既定（cap 0）は表の数字そのまま
  assert.equal(rules.blockBudget('writer', 4, 0), 2400);
  // cap があるときは比率で配分（writer の合計 7900 に対し target 2400）
  assert.equal(rules.blockBudget('writer', 4, 7900), 2400);
  assert.equal(rules.blockBudget('writer', 4, 15800), 4800);
  // 表に無い役割は writer の比率
  assert.equal(rules.blockBudget('custom', 4, 7900), 2400);
  assert.equal(rules.budgetRatio(50, 100), 0.5);
  assert.equal(rules.budgetRatio(150, 100), 1);
  assert.equal(rules.budgetRatio(10, 0), 0);
  assert.ok(!rules.canExclude('project'));
  assert.ok(!rules.canExclude('constraints'));
  assert.ok(rules.canExclude('history'));
});

test('agent: 提案の行差分は行単位（§8.7.2 の全節スナップショット）。前だけ・後だけで並べる', async () => {
  const rules = await agent();
  const rows = rules.diffRows('a\nb\nc', 'a\nx\nc');
  assert.deepEqual(rows.map((row) => [row.kind, row.before, row.after]), [
    ['same', 'a', 'a'],
    ['before', 'b', null],
    ['after', null, 'x'],
    ['same', 'c', 'c'],
  ]);
  const same = rules.diffRows('x\ny', 'x\ny');
  assert.ok(same.every((row) => row.kind === 'same'));
  assert.deepEqual(rules.diffRows('', 'n'), [{ kind: 'after', before: null, after: 'n' }]);
});

test('agent: 提案の形・決着（§8.7.2）。却下はコメント必須', async () => {
  const rules = await agent();
  const rows = rules.proposalRowsOf({
    proposals: [
      { id: 'P1', status: 'open', target: '@01-01-01-01', note: '直し', comment: '', origin: 'human', drifted: true, partial: true, before: 'a', after: 'b' },
      { broken: true },
    ],
  });
  assert.equal(rows.length, 1);
  assert.equal(rows[0].target, '@01-01-01-01');
  assert.equal(rows[0].drifted, true);
  assert.equal(rows[0].partial, true);
  assert.ok(rules.commentUsable('x'));
  assert.ok(!rules.commentUsable('  '));
  assert.ok(rules.settled('applied'));
  assert.ok(rules.settled('accepted'));
  assert.ok(!rules.settled('open'));
});

// ---------------------------------------------------------------- review.ts

test('review: 9 タブの並びと提供状態（§10.9.1）。時系列は when: が無いと出さない', async () => {
  const rules = await review();
  assert.deepEqual([...rules.REVIEW_TABS], [
    'base',
    'timeline',
    'speakers',
    'histogram',
    'observations',
    'stats',
    'senses',
    'illustrations',
    'proposals',
  ]);
  assert.ok(!rules.visibleTabs(false).includes('timeline'));
  assert.ok(rules.visibleTabs(true).includes('timeline'));
  assert.equal(rules.tabState('base'), 'ready');
  assert.equal(rules.tabState('speakers'), 'partial');
  assert.equal(rules.tabState('histogram'), 'partial');
  assert.equal(rules.tabState('observations'), 'partial');
  assert.equal(rules.tabState('stats'), 'unavailable');
  assert.equal(rules.tabState('senses'), 'unavailable');
  assert.equal(rules.tabState('timeline'), 'unavailable');
  // 端で循環
  const visible = rules.visibleTabs(false);
  assert.equal(rules.moveTab('proposals', 1, visible), 'base');
  assert.equal(rules.moveTab('base', -1, visible), 'proposals');
});

test('review: 診断一覧の形（index は操作の並びのまま）、絞込・並べ替え（§10.9.2）', async () => {
  const rules = await review();
  const rows = rules.rowsOf({
    diagnostics: [
      { code: 'W087', path: 'contents/01.cnov', line: 10, column: 1, message: 'a', byte: 3, severity: 'warning' },
      { code: 'E001', path: 'contents/01.cnov', line: 2, column: 1, message: 'b', byte: 1, severity: 'error' },
      { code: 'W087', path: 'contents/02.cnov', line: 1, column: 1, message: 'c', byte: 9, severity: 'warning' },
      { broken: true },
    ],
  });
  assert.deepEqual(rows.map((row) => row.index), [0, 1, 2]);
  assert.deepEqual(rules.sortRows(rows, 'position').map((row) => row.message), ['b', 'a', 'c']);
  assert.deepEqual(rules.sortRows(rows, 'severity').map((row) => row.message), ['b', 'a', 'c']);
  assert.deepEqual(rules.sortRows(rows, 'code').map((row) => row.code), ['E001', 'W087', 'W087']);
  assert.deepEqual(rules.filterRows(rows, 'W08').map((row) => row.code), ['W087', 'W087']);
  assert.equal(rules.filterRows(rows, '').length, 3);
  assert.deepEqual(rules.severityCounts(rows), { error: 1, warning: 2, info: 0 });
});

test('review: 「作家の言葉でのどこ」（place.*）。§10.9.2 の場所列', async () => {
  const rules = await review();
  const index = {
    sections: [{ id: '01-02-03-04', path: 'contents/01.cnov', lines: [10, 20] }],
    blocks: [{ id: 'H2-3', path: 'plots/all.cnov', lines: [4, 8] }],
    materials: [
      { id: 'M1', title: '時刻表', path: 'materials/timetable.md' },
      { id: 'M2', title: '', path: 'materials/blank.md' },
    ],
  };
  // 節の行は 0 始まり・終端排他（`section.list`）。1 始まりの行は [start + 1, end]
  assert.deepEqual(rules.placeOf('contents/01.cnov', 12, index), { key: 'place.contents', args: ['01-02-03-04'] });
  assert.equal(rules.placeOf('contents/01.cnov', 5, index), null);
  assert.deepEqual(rules.placeOf('plots/all.cnov', 6, index), { key: 'place.whole_plot', args: ['H2-3'] });
  assert.deepEqual(rules.placeOf('plots/all.cnov', 99, index), { key: 'place.whole_plot_head', args: [] });
  assert.deepEqual(rules.placeOf('plots/02.cnov', 3, index), { key: 'place.chapter_plot_head', args: [] });
  assert.deepEqual(rules.placeOf('plots/02-03.cnov', 3, index), { key: 'place.episode_plot', args: ['02-03'] });
  assert.deepEqual(rules.placeOf('project.toml', 1, index), { key: 'place.settings', args: [] });
  assert.deepEqual(rules.placeOf('agents/prompts/writer.md', 1, index), { key: 'place.prompt', args: ['writer'] });
  assert.deepEqual(rules.placeOf('materials/timetable.md', 1, index), {
    key: 'place.material',
    args: ['M1', '時刻表'],
  });
  assert.deepEqual(rules.placeOf('materials/blank.md', 1, index), { key: 'place.material_id', args: ['M2'] });
  assert.equal(rules.placeOf('notes.txt', 1, index), null);
  assert.equal(
    rules.placeText(t, { key: 'place.contents', args: ['01-02-03-04'] }, { path: 'x', line: 1, column: 1 }),
    'place.contents(01-02-03-04)',
  );
  assert.equal(rules.placeText(t, null, { path: 'x', line: 2, column: 3 }), 'x:2:3');
  assert.equal(rules.placeText(t, null, { path: 'x', line: 0, column: 0 }), 'x');
});

test('review: 分量の T スコア（§10.3.4）。母集団標準偏差、0 字は数えない、3 未満は計算しない', async () => {
  const rules = await review();
  const score = rules.tScores([10, 20, 30]);
  const sigma = Math.sqrt(200 / 3);
  assert.deepEqual(score, [Math.round(50 - 100 / sigma), 50, Math.round(50 + 100 / sigma)]);
  assert.deepEqual(rules.tScores([100, 100, 100, 100]), [50, 50, 50, 50]);
  assert.deepEqual(rules.tScores([100, 200]), [null, null]);
  // 0 字は母集団から外し、位置は null のまま（「短い」と言わない）
  const withEmpty = rules.tScores([10, 0, 30, 50]);
  assert.equal(withEmpty[1], null);
  assert.equal(withEmpty.filter((value) => value !== null).length, 3);
  assert.equal(rules.isOutlier(34), true);
  assert.equal(rules.isOutlier(66), true);
  assert.equal(rules.isOutlier(50), false);
  assert.equal(rules.isOutlier(null), false);
});

test('review: 節の字数・ヒストグラム・代表値（§9.6.1・§10.9.5）。行の範囲は 0 始まり・終端排他', async () => {
  const rules = await review();
  assert.equal(rules.countChars('あ い\nう'), 3);
  assert.equal(rules.sectionChars('abcde\nfghij\nklmno', [1, 2]), 5);
  assert.equal(rules.sectionChars('abcde\nfghij\nklmno', [1, 3]), 10);
  const bins = rules.histogram([5, 150, 160, 320], 100);
  assert.deepEqual(bins.map((bin) => [bin.from, bin.to, bin.count]), [
    [0, 99, 1],
    [100, 199, 2],
    [300, 399, 1],
  ]);
  const summary = rules.lengthSummary([10, 20, 30, 40]);
  assert.equal(summary.median, 25);
  assert.equal(summary.mean, 25);
  assert.equal(summary.max, 40);
  assert.equal(summary.min, 10);
});

test('review: 進捗ツリー（章 → 話 → 場面 → 節）。文字数は下の合計、ID は数値順', async () => {
  const rules = await review();
  const rows = rules.treeRows(
    new Map([
      ['01', '第 1 章'],
      ['01-01', '第 1 話'],
      ['01-01-01-01', '冒頭'],
      ['01-01-01-02', '続き'],
      ['02', '第 2 章'],
    ]),
    new Map([
      ['01-01-01-01', 100],
      ['01-01-01-02', 250],
    ]),
    new Map([['01', '起']]),
  );
  assert.deepEqual(rows.map((row) => row.id), ['01', '01-01', '01-01-01', '01-01-01-01', '01-01-01-02', '02']);
  assert.equal(rows[0].chars, 350);
  assert.equal(rows[0].state, '起');
  assert.equal(rows[4].chars, 250);
  assert.equal(rows[5].state, null);
});

test('review: 文体観察の指標（§10.9.4）。既定で ON は W082 と役割語未検出だけ、スコア語は出さない', async () => {
  const rules = await review();
  const defaults = rules.defaultOnGroups();
  assert.deepEqual([...defaults].sort(), ['cast.tab_unmatched', 'diag.W082']);
  for (const group of rules.OBSERVATION_GROUPS) {
    assert.ok(!/score|点数|評価|改善/u.test(group.labelKey), `スコア語のキー: ${group.labelKey}`);
  }
  const rows = rules.rowsOf({
    diagnostics: [
      { code: 'W082', path: 'a', line: 1, column: 1, message: 'x', byte: 0, severity: 'warning' },
      { code: 'W087', path: 'a', line: 2, column: 1, message: 'y', byte: 0, severity: 'warning' },
    ],
  });
  const grouped = rules.groupRows(rows, new Set(defaults));
  assert.deepEqual(grouped.map((group) => [group.labelKey, group.rows.length]), [
    ['diag.W082', 1],
    ['cast.tab_unmatched', 0],
  ]);
});

test('review: 挿絵メモ（追加仕様 23）。`! ` 行と `illustration:` 行だけ、SceneId 順', async () => {
  const rules = await review();
  const sections = [
    { id: '01-01-01-01', path: 'contents/01.cnov', lines: [0, 10] },
    { id: '01-01-01-02', path: 'contents/01.cnov', lines: [10, 20] },
  ];
  const rows = rules.memoRowsOf(
    [
      { path: 'contents/01.cnov', line: 12, text: '! あられの丘から町を見下ろす' },
      { path: 'contents/01.cnov', line: 3, text: 'illustration: 夕暮れの街' },
      { path: 'contents/01.cnov', line: 5, text: 'ただの文' },
    ],
    sections,
  );
  assert.deepEqual(rows.map((row) => [row.scene, row.line]), [
    ['01-01-01-01', 3],
    ['01-01-01-02', 12],
  ]);
  assert.ok(rules.isMemo('! 絵'));
  assert.ok(rules.isMemo('illustration: 絵'));
  assert.ok(!rules.isMemo('！！ これは台詞'));
});

test('review: キーの写像（この画面が横取りするものだけ）', async () => {
  const rules = await review();
  assert.equal(rules.reviewAction('Ctrl+Shift+ArrowLeft'), 'tab-prev');
  assert.equal(rules.reviewAction('Ctrl+Shift+ArrowRight'), 'tab-next');
  assert.equal(rules.reviewAction('Ctrl+Shift+E'), 'rerun');
  assert.equal(rules.reviewAction('Enter'), 'open');
  assert.equal(rules.reviewAction('ArrowUp'), 'prev');
  assert.equal(rules.reviewAction('ArrowDown'), 'next');
  assert.equal(rules.reviewAction('Home'), 'first');
  assert.equal(rules.reviewAction('End'), 'last');
  assert.equal(rules.reviewAction('x'), null);
  assert.ok(rules.isBaseView('diagnostics'));
  assert.ok(!rules.isBaseView('diagnostics2'));
});

// ---------------------------------------------------------------- 配線

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
  assert.ok(used.size >= 15, `画面が呼ぶ操作を集める（${[...used].join(', ')}）`);
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
  const agentRules = await agent();
  const reviewRules = await review();
  for (const column of agentRules.COLUMNS) {
    keys.add(agentRules.COLUMN_LABEL[column]);
  }
  for (const place of [
    { key: 'place.contents', args: [] },
    { key: 'place.whole_plot', args: [] },
    { key: 'place.whole_plot_head', args: [] },
    { key: 'place.chapter_plot', args: [] },
    { key: 'place.chapter_plot_head', args: [] },
    { key: 'place.episode_plot', args: [] },
    { key: 'place.material', args: [] },
    { key: 'place.material_id', args: [] },
    { key: 'place.prompt', args: [] },
    { key: 'place.settings', args: [] },
  ]) {
    keys.add(place.key);
  }
  for (const key of Object.values(reviewRules.TAB_LABEL)) {
    keys.add(key);
  }
  for (const key of Object.values(reviewRules.BASE_VIEW_LABEL)) {
    keys.add(key);
  }
  for (const group of reviewRules.OBSERVATION_GROUPS) {
    keys.add(group.labelKey);
  }
  assert.ok(keys.size >= 60, `キーを集める（${keys.size} 件）`);
  for (const locale of ['ja', 'en']) {
    for (const key of keys) {
      assert.ok(
        Object.prototype.hasOwnProperty.call(CATALOGS[locale], key) && CATALOGS[locale][key] !== '',
        `${locale} に無い文言のキー: ${key}`,
      );
    }
  }
});

test('配線: 生色を書かない（§11.1）。色は CSS 変数だけ', async () => {
  for (const file of LANE_FILES.filter((found) => found.endsWith('.svelte'))) {
    const text = await readFile(join(WEBUI, file), 'utf8');
    const style = text.slice(text.indexOf('<style>'));
    assert.doesNotMatch(style, /#[0-9a-fA-F]{3,8}\b/u, `${file} に 16 進の色`);
    const withoutShadow = style.replace(/rgb\(0 0 0 \/ 32%\)/gu, '');
    assert.doesNotMatch(withoutShadow, /rgba?\(/u, `${file} に rgb の色`);
    assert.doesNotMatch(withoutShadow, /hsla?\(/u, `${file} に hsl の色`);
  }
});

test('配線: CSS 変数は app.css と theme/tokens のものだけ（§11.1）', async () => {
  const { CSS_VARIABLES } = await tokens();
  // 変数の素は 2 つ: `app.css` の定義と、`CSS_VARIABLES`（トークン名 → `--…`）
  const app = await readFile(join(WEBUI, 'src/app.css'), 'utf8');
  const known = new Set(Object.values(CSS_VARIABLES));
  for (const match of app.matchAll(/(--[a-z0-9-]+)\s*:/gu)) {
    known.add(match[1]);
  }
  for (const file of LANE_FILES.filter((found) => found.endsWith('.svelte'))) {
    const text = await readFile(join(WEBUI, file), 'utf8');
    for (const match of text.matchAll(/var\((--[a-z0-9-]+)/gu)) {
      assert.ok(known.has(match[1]), `${file} に無い CSS 変数: ${match[1]}`);
    }
  }
});

test('配線: 規則のファイルは実行時の import を持たない（単独で読める）', async () => {
  for (const file of ['src/lib/workspaces/agent/agent.ts', 'src/lib/workspaces/review/review.ts']) {
    const text = await readFile(join(WEBUI, file), 'utf8');
    assert.doesNotMatch(text, /^import\s/mu, `${file} に import`);
  }
});
