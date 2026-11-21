// 話プロット・構成（W4 / L2）の回帰（Node。`webui/src/lib/workspaces/plot-*/` の純粋な計算をそのまま読む）。
//
// 見るのは 5 つ:
//
// 1. A 形式のブロックの読み取り（壊れた値は捨てる。章の ID と話の ID を区別する）。
// 2. 構成のツリー（ストック `00` は必ず最後。話は ID の順。畳んだ集合は core の返り値を使う）。
// 3. 話の節の並び（話の外の節を混ぜない。選んでいた節が消えたら同じ場面へ寄せる）。
// 4. 台詞比率（§9.4.2。鉤括弧の内側だけ、空白を除く。分母 0 は描かない）。
// 5. 行の範囲の差し替え（範囲が文書の外へはみ出しても他の行を壊さない）。
//
// TypeScript は `tsc` で 1 度だけ書き出してから読む（型検査は `npm run check`）。
// 画面の見た目と操作は Svelte の型検査と、`scripts/webui-check.sh` の範囲で確かめる。

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

const WEBUI = new URL('..', import.meta.url).pathname;

/**
 * 4 つの `.ts` を 1 つの出力先へ書き出し、読み込む。
 * 相対 import（`../plot-outline/blocks`）を解決するため、書き出し先は元の階層を保つ。
 */
let loaded = null;

async function load() {
  if (loaded) {
    return loaded;
  }
  const out = await mkdtemp(join(tmpdir(), 'novnow-plot-'));
  process.on('exit', () => {
    rm(out, { recursive: true, force: true }).catch(() => {});
  });
  // 書き出しは `bundler`（画面と同じ解決）。拡張子なしの相対 import は、読み込み前に `.js` を補う。
  execFileSync(
    process.execPath,
    [
      join(WEBUI, 'node_modules', 'typescript', 'bin', 'tsc'),
      join(WEBUI, 'src/lib/workspaces/plot-outline/blocks.ts'),
      join(WEBUI, 'src/lib/workspaces/plot-outline/model.ts'),
      join(WEBUI, 'src/lib/workspaces/plot-episode/model.ts'),
      '--ignoreConfig',
      '--target',
      'es2022',
      '--module',
      'es2022',
      '--moduleResolution',
      'bundler',
      '--rootDir',
      join(WEBUI, 'src/lib/workspaces'),
      '--outDir',
      out,
    ],
    { stdio: 'pipe' },
  );
  await writeFile(join(out, 'package.json'), '{"type":"module"}\n');
  // 書き出された相対 import に拡張子を補う（`from './blocks'` → `from './blocks.js'`）
  for (const dir of ['plot-outline', 'plot-episode']) {
    for (const file of ['blocks.js', 'model.js']) {
      const path = join(out, dir, file);
      try {
        const source = await readFile(path, 'utf8');
        await writeFile(path, source.replace(/from '(\.\.?\/[^']+?)'/gu, (_, spec) => `from '${spec}.js'`));
      } catch {
        // そのファイルが無ければ飛ばす（書き出しの有無は下の import で確かめる）
      }
    }
  }
  const blocks = await import(join(out, 'plot-outline', 'blocks.js'));
  const outline = await import(join(out, 'plot-outline', 'model.js'));
  const episode = await import(join(out, 'plot-episode', 'model.js'));
  loaded = { blocks, outline, episode };
  return loaded;
}

/** A 形式のブロックの形を作る（`plot.get` の戻り値の 1 要素）。 */
function block(id, title, lines, extra = {}) {
  return {
    id,
    path: id.includes('-') ? `plots/${id.slice(0, 2)}.cnov` : 'plots/all.cnov',
    lines,
    end: lines[1],
    title,
    summary: '',
    state: null,
    ...extra,
  };
}

test('A 形式のブロック: 壊れた値を捨て、正しい形だけ読む', async () => {
  const { blocks } = await load();
  const good = block('01', '第一章', [0, 5]);
  const result = blocks.blocksOf({
    level: 'all',
    blocks: [good, { id: 3 }, null, { ...good, lines: [0] }, 'x'],
  });
  assert.deepEqual(result, [good]);
  assert.deepEqual(blocks.blocksOf(null), []);
  assert.deepEqual(blocks.blocksOf({ level: 'all' }), []);
});

test('章の ID と話の ID を区別する', async () => {
  const { blocks } = await load();
  assert.equal(blocks.chapterNumber('01'), 1);
  assert.equal(blocks.chapterNumber('01-04'), 1);
  assert.equal(blocks.chapterNumber('00'), 0);
  assert.equal(blocks.chapterNumber('x'), null);
  assert.equal(blocks.isEpisodeId('01-04'), true);
  assert.equal(blocks.isEpisodeId('01'), false);
});

test('行の範囲の差し替え: 範囲の中だけを置き換え、外は保つ', async () => {
  const { blocks } = await load();
  const text = 'a\nb\nc\nd';
  assert.equal(blocks.replaceLines(text, [1, 3], 'X\nY\nZ'), 'a\nX\nY\nZ\nd');
  assert.equal(blocks.linesOf(text, [1, 3]), 'b\nc');
});

test('行の範囲が文書の外へはみ出しても、丸めて壊さない', async () => {
  const { blocks } = await load();
  const text = 'a\nb';
  // 終わりが文書の外: 末尾まで
  assert.equal(blocks.replaceLines(text, [1, 99], 'X'), 'a\nX');
  // 始まりが文書の外: 末尾に足すだけ（既存の行は消さない）
  assert.equal(blocks.replaceLines(text, [99, 120], 'X'), 'a\nb\nX');
  // 逆転した範囲: 空の範囲として扱い、既存の行は消さない
  assert.equal(blocks.replaceLines(text, [2, 1], 'X'), 'a\nb\nX');
});

test('構成のツリー: ストック（00）は最後、話は ID の順', async () => {
  const { outline } = await load();
  const all = [
    block('00', 'ストック', [0, 1]),
    block('02', '第二章', [1, 2]),
    block('01', '第一章', [0, 1]),
  ];
  const episodes = new Map([
    [1, [block('01-02', '二話', [3, 4]), block('01-01', '一話', [0, 3])]],
    [2, [block('02-01', '別の一話', [0, 2])]],
  ]);
  const tree = outline.buildTree(all, episodes);
  assert.deepEqual(
    tree.map((chapter) => chapter.number),
    [1, 2, 0],
    'ストックは番号順では先頭だが、ツリーでは最後',
  );
  assert.deepEqual(
    tree[0].episodes.map((item) => item.id),
    ['01-01', '01-02'],
  );
});

test('構成のツリー: 章のブロックが無い章も、話があれば出す', async () => {
  const { outline } = await load();
  const tree = outline.buildTree([], new Map([[3, [block('03-01', '三話', [0, 2])]]]));
  assert.equal(tree.length, 1);
  assert.equal(tree[0].number, 3);
  assert.equal(tree[0].block, null);
  assert.equal(tree[0].episodes.length, 1);
});

test('畳む: 起動直後は全章を畳み、畳んだ集合は core の返り値に従う', async () => {
  const { outline } = await load();
  const tree = outline.buildTree([block('01', 'a', [0, 1]), block('02', 'b', [1, 2])], new Map());
  assert.deepEqual(outline.allFolded(tree), [1, 2]);
  // 1 章だけ開く
  assert.deepEqual(outline.toggleFold([1, 2], 1, false), [2]);
  // 同じ章を畳む（重複させない）
  assert.deepEqual(outline.toggleFold([2], 2, true), [2]);
  assert.deepEqual(outline.toggleFold([], 3, true), [3]);
});

test('レベルで表示するブロックが決まる（全体は章、章は話）', async () => {
  const { outline } = await load();
  const tree = outline.buildTree(
    [block('01', 'a', [0, 1]), block('02', 'b', [1, 2])],
    new Map([[1, [block('01-01', 'x', [0, 1])]]]),
  );
  assert.deepEqual(
    outline.blocksAt(tree, { kind: 'all' }).map((item) => item.id),
    ['01', '02'],
  );
  assert.deepEqual(
    outline.blocksAt(tree, { kind: 'chapter', chapter: 1 }).map((item) => item.id),
    ['01-01'],
  );
  assert.deepEqual(outline.blocksAt(tree, { kind: 'chapter', chapter: 9 }), []);
});

test('タブ: 既定は基本。端では循環する', async () => {
  const { outline } = await load();
  assert.equal(outline.TABS[0], 'base');
  assert.equal(outline.moveTab('base', -1), 'stock');
  assert.equal(outline.moveTab('stock', 1), 'base');
  assert.equal(outline.moveTab('base', 1), 'timeline');
  assert.equal(outline.tabState('base'), 'ready');
  assert.equal(outline.tabState('timeline'), 'unavailable');
});

test('話の節: 話の外の節は混ぜず、ID の順に並べる', async () => {
  const { episode } = await load();
  const all = [
    { id: '01-04-02-03', path: 'contents/01-04.cnov', lines: [9, 12], summary: 'c' },
    { id: '01-05-01-01', path: 'contents/01-05.cnov', lines: [0, 3], summary: 'x' },
    { id: '01-04-01-01', path: 'contents/01-04.cnov', lines: [0, 3], summary: 'a' },
  ];
  const rows = episode.sectionsInEpisode(all, '01-04');
  assert.deepEqual(
    rows.map((row) => row.id),
    ['01-04-01-01', '01-04-02-03'],
  );
});

test('選んでいた節が消えたら、同じ場面の先頭へ寄せ、無ければ先頭', async () => {
  const { episode } = await load();
  const rows = [
    { id: '01-04-01-01', path: 'p', lines: [0, 1], summary: '' },
    { id: '01-04-02-01', path: 'p', lines: [1, 2], summary: '' },
  ];
  assert.equal(episode.keepSelection(rows, '01-04-01-01'), '01-04-01-01');
  assert.equal(episode.keepSelection(rows, '01-04-01-09'), '01-04-01-01', '同じ場面の別の節');
  assert.equal(episode.keepSelection(rows, '01-04-09-01'), '01-04-01-01', '場面も無ければ先頭');
  assert.equal(episode.keepSelection([], '01-04-01-01'), null);
});

test('台詞比率: 鉤括弧の内側だけを数え、鉤括弧と空白は数えない（§9.4.2）', async () => {
  const { episode } = await load();
  // 台詞: あ・い = 2 字。地の文: と・言・っ・た・。 = 5 字。鉤括弧 4 つは数えない。
  const found = episode.dialogueRatio('「あ」と言った。\n『い』');
  assert.equal(found.dialogue, 2);
  assert.equal(found.narration, 5);
  assert.equal(found.ratio, 2 / 7);
});

test('台詞比率: 分母 0 は null（バーを描かない）', async () => {
  const { episode } = await load();
  assert.equal(episode.dialogueRatio('').ratio, null);
  assert.equal(episode.dialogueRatio('   \n  ').ratio, null);
  assert.equal(episode.percent(null), '');
  assert.equal(episode.percent(0.345), '35%');
});

test('台詞比率: 字句の注釈（コメント・行頭の # と !・資料引用）は数えない', async () => {
  const { episode } = await load();
  const plain = episode.dialogueRatio('「あ」');
  const noisy = episode.dialogueRatio('「あ」// 注\n# 見出し\n! 挿絵\n/* 注 */「あ」^M0137');
  assert.equal(noisy.dialogue, plain.dialogue * 2);
  assert.equal(noisy.narration, 0);
});

test('節の ID の話・場面の部分', async () => {
  const { episode } = await load();
  assert.equal(episode.episodeOf('01-04-02-03'), '01-04');
  assert.equal(episode.sceneOf('01-04-02-03'), '01-04-02');
  assert.equal(episode.episodeOf('01'), null);
});

/** キーの組（`strokeOf` の形。`code` は物理キー）。 */
function stroke(key, code, { ctrl = false, shift = false, alt = false } = {}) {
  return { key, code, ctrl, shift, alt };
}

test('構成のキー: §9.8 の割当を写す（記号は物理キーで決める）', async () => {
  const { outline } = await load();
  assert.equal(outline.outlineKey(stroke('ArrowRight', 'ArrowRight', { ctrl: true, shift: true })), 'tab-next');
  assert.equal(outline.outlineKey(stroke('ArrowLeft', 'ArrowLeft', { ctrl: true, shift: true })), 'tab-previous');
  // 配列で `[` が `{` に変わっても、物理キーで fold と判定する
  assert.equal(outline.outlineKey(stroke('{', 'BracketLeft', { ctrl: true, shift: true })), 'fold');
  assert.equal(outline.outlineKey(stroke('}', 'BracketRight', { ctrl: true, shift: true })), 'unfold');
  assert.equal(outline.outlineKey(stroke('N', 'KeyN', { ctrl: true, shift: true })), 'new-section');
  assert.equal(outline.outlineKey(stroke('B', 'KeyB', { ctrl: true, shift: true })), 'up-level');
  assert.equal(outline.outlineKey(stroke('Enter', 'Enter', { ctrl: true })), 'down-level');
  assert.equal(outline.outlineKey(stroke('ArrowUp', 'ArrowUp')), 'up');
  assert.equal(outline.outlineKey(stroke('ArrowDown', 'ArrowDown')), 'down');
});

test('構成のキー: Alt を含む組は扱わず、親へ流す（Alt は別の割当）', async () => {
  const { outline } = await load();
  assert.equal(outline.outlineKey(stroke('ArrowUp', 'ArrowUp', { alt: true })), null);
  assert.equal(outline.outlineKey(stroke('ArrowUp', 'ArrowUp', { ctrl: true })), null);
  assert.equal(outline.outlineKey(stroke('x', 'KeyX')), null);
});

test('話のキー: §9.8 の割当を写し、Alt+Shift+↑↓ は入れ替えとして拾う', async () => {
  const { episode } = await load();
  assert.equal(episode.episodeKey(stroke('N', 'KeyN', { ctrl: true, shift: true })), 'new-section');
  assert.equal(episode.episodeKey(stroke('Enter', 'Enter', { ctrl: true })), 'to-write');
  assert.equal(episode.episodeKey(stroke('D', 'KeyD', { ctrl: true })), 'mode');
  assert.equal(episode.episodeKey(stroke('M', 'KeyM', { ctrl: true })), 'material');
  assert.equal(episode.episodeKey(stroke('ArrowUp', 'ArrowUp', { alt: true, shift: true })), 'swap-up');
  assert.equal(episode.episodeKey(stroke('ArrowDown', 'ArrowDown', { alt: true, shift: true })), 'swap-down');
  assert.equal(episode.episodeKey(stroke('ArrowUp', 'ArrowUp')), 'up');
  assert.equal(episode.episodeKey(stroke('x', 'KeyX')), null);
});
