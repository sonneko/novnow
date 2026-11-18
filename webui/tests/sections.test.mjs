// 節の操作と衝突の解決（`section.split` / `section.propose` / `plot.set_state` /
// `save.keep_mine` / `save.reload_external`）の回帰（Node。`webui/src/lib/sections/` を読む）。
//
// 見るのは 4 つ:
//
// 1. 規則（`sectionOps.ts`）: 分割位置の**バイト換算**（和文・絵文字）・提案の検査・
//    `state:` の読み書き・衝突の一覧と差分。
// 2. **単位を取り違えない**: `section.split` の `at` はバイト位置、キャレットは UTF-16。
//    機能核のソースを読んで、`at` がバイトとして扱われていることを確かめる。
// 3. 配線: 5 つを画面が実際に `run('<操作 ID>')` で呼ぶ。`Ctrl+Enter` が分割へ届く。
// 4. パリティ判定との一致: `coverage.ts` の `PENDING` から 5 件が外れている。

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

const WEBUI = new URL('..', import.meta.url).pathname;
const REPO = new URL('../../', import.meta.url).pathname;

const compiled = new Map();

function importTs(relative) {
  if (!compiled.has(relative)) {
    compiled.set(relative, compileTs(relative));
  }
  return compiled.get(relative);
}

async function compileTs(relative) {
  const out = await mkdtemp(join(tmpdir(), 'novnow-sections-'));
  try {
    execFileSync(
      process.execPath,
      [
        join(WEBUI, 'node_modules', 'typescript', 'bin', 'tsc'),
        join(WEBUI, relative),
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

const ops = () => importTs('src/lib/sections/sectionOps.ts');

// ------------------------------------------------------------------ 1. 規則

test('分割の位置: キャレット（UTF-16）をバイト位置へ換算する', async () => {
  const { byteOffset, splitArgs } = await ops();
  // 和文は 1 文字 3 バイト。換算しないと 3 倍ずれた場所で節が切れる。
  assert.equal(byteOffset('あいう', 2), 6);
  assert.equal(byteOffset('abc', 2), 2);
  assert.equal(byteOffset('あaい', 2), 4);
  // 絵文字（サロゲートペア）も 1 文字 = 4 バイト。
  assert.equal(byteOffset('😀x', 2), 4);
  assert.equal(byteOffset('😀x', 3), 5);
  // 範囲外は端へ丸める（機能核の `at.min(buffer.len())` と同じ規則）。
  assert.equal(byteOffset('あい', 99), 6);
  assert.equal(byteOffset('あい', -5), 0);
  assert.equal(byteOffset('', 3), 0);

  const args = splitArgs('01-02-01-01', 'あいう', 1);
  assert.deepEqual(args, { at: 3, role: '', scene: '01-02-01-01' });
  // 末尾は本文の長さ（機能核が `at >= len` で「確定して次へ」に回す）。
  assert.equal(splitArgs('s', 'あいう', 3).at, 9);
});

test('提案: 空の本文・空の節は断る（押しても効かない送信を置かない）', async () => {
  const { proposalArgs } = await ops();
  assert.deepEqual(proposalArgs({ scene: '', text: 'あ', note: '' }), {
    ok: false,
    reason: 'no-scene',
  });
  assert.deepEqual(proposalArgs({ scene: 's', text: '   ', note: '' }), {
    ok: false,
    reason: 'empty',
  });
  // 覚え書きは空なら送らない（`note` は任意引数）。
  assert.deepEqual(proposalArgs({ scene: ' s ', text: '本文', note: '  ' }), {
    ok: true,
    args: { scene: 's', text: '本文' },
  });
  assert.deepEqual(proposalArgs({ scene: 's', text: '本文', note: '理由' }), {
    ok: true,
    args: { scene: 's', text: '本文', note: '理由' },
  });
});

test('プロットの確認状態: state: を読み、反転した真偽を送る', async () => {
  const { stateOf, toggleState, setStateArgs } = await ops();
  // `state:` の行から読む（無ければ draft = 機能核の既定）。
  assert.equal(stateOf('@01-02-01 場面\nstate: fixed'), 'fixed');
  assert.equal(stateOf('@01-02-01 場面\nstate: draft'), 'draft');
  assert.equal(stateOf('@01-02-01 場面'), 'draft');
  assert.equal(stateOf(''), 'draft');
  // 反転。
  assert.equal(toggleState('fixed'), 'draft');
  assert.equal(toggleState('draft'), 'fixed');
  assert.equal(toggleState('なにか'), 'fixed');
  // 機能核は `bool` を取り、`true` → `fixed`。
  assert.deepEqual(setStateArgs('01-02-01-01', 'fixed'), {
    role: '',
    scene: '01-02-01-01',
    state: true,
  });
  assert.deepEqual(setStateArgs('01-02-01-01', 'draft'), {
    role: '',
    scene: '01-02-01-01',
    state: false,
  });
});

test('衝突: 一覧はパスの配列だけを受け取り、形が違えば空にする', async () => {
  const { conflictPaths, resolveArgs, conflictSurface } = await ops();
  assert.deepEqual(conflictPaths({ conflicts: ['a.cnov', 'b.cnov'] }), ['a.cnov', 'b.cnov']);
  // 空文字・文字列でない要素は落とす。
  assert.deepEqual(conflictPaths({ conflicts: ['a.cnov', '', 3, null] }), ['a.cnov']);
  for (const found of [null, {}, { conflicts: 'a' }, { conflicts: null }]) {
    assert.deepEqual(conflictPaths(found), [], JSON.stringify(found));
  }
  // `path` を省略すると機能核は**すべて**解決する。画面は 1 件ずつ指定する。
  assert.deepEqual(resolveArgs('a.cnov'), { path: 'a.cnov' });
  assert.deepEqual(resolveArgs(''), {});
  // ブラウザは外部変更を監視しない（§13.3）。面は理由を書いて出る。
  assert.equal(conflictSurface({ native: true, conflicts: 0 }), 'hidden');
  assert.equal(conflictSurface({ native: false, conflicts: 0 }), 'unsupported');
  assert.equal(conflictSurface({ native: true, conflicts: 1 }), 'shown');
  assert.equal(conflictSurface({ native: false, conflicts: 1 }), 'shown');
});

test('衝突: 差分は読むための形に直す（形の違う行は落とす）', async () => {
  const { conflictDiffs } = await ops();
  const diffs = conflictDiffs({
    diffs: [
      {
        path: 'contents/01.cnov',
        mine_chars: 10,
        external_chars: 12,
        lines: [
          { line: 1, side: 'mine', text: '自分' },
          { line: 2, side: 'external', text: '外部' },
          { line: 3, side: 'other', text: '捨てる' },
          { line: 4, side: 'mine' },
        ],
      },
      { path: '' },
      { mine_chars: 1 },
    ],
  });
  assert.equal(diffs.length, 1);
  assert.equal(diffs[0].path, 'contents/01.cnov');
  assert.equal(diffs[0].mine_chars, 10);
  assert.deepEqual(
    diffs[0].lines.map((line) => line.side),
    ['mine', 'external'],
  );
  // 形が違えば空。
  for (const found of [null, {}, { diffs: 'x' }]) {
    assert.deepEqual(conflictDiffs(found), []);
  }
});

// -------------------------------------------------- 2. 単位を取り違えない

/** Rust の関数本体（波括弧の対応が閉じるまで）。 */
function bodyOf(text, name) {
  const start = text.indexOf(`fn ${name}(`);
  assert.ok(start >= 0, `${name} がソースに無い`);
  const open = text.indexOf('{', start);
  let depth = 0;
  for (let index = open; index < text.length; index += 1) {
    if (text[index] === '{') {
      depth += 1;
    } else if (text[index] === '}') {
      depth -= 1;
      if (depth === 0) {
        return text.slice(open, index + 1);
      }
    }
  }
  throw new Error(`${name} の本体が閉じない`);
}

test('section.split の at はバイト位置である（画面は換算して渡す）', async () => {
  const editing = await readFile(join(REPO, 'src/app/ops/editing.rs'), 'utf8');
  // 画面から来た `at` をそのまま `split_section` へ渡している（換算は画面の責任）。
  assert.match(bodyOf(editing, 'section_split'), /session\.bench\.split_section\(at/);
  const workbench = await readFile(join(REPO, 'src/app/workbench.rs'), 'utf8');
  const split = bodyOf(workbench, 'split_section');
  // 機能核は **char 境界**（= バイト位置）として扱う。UTF-16 の添字ではない。
  assert.match(split, /is_char_boundary\(at\)/);
  assert.match(split, /at\.min\(self\.buffer\.len\(\)\)/);
  assert.doesNotMatch(split, /chars\(\)\.nth|enumerate\(\)/, '文字数で数えている（バイトでない）');
  // 仕様もバイト位置と書いている（画面のコメントだけを信じない）。
  const spec = await readFile(join(REPO, 'docs/specifications/editing.md'), 'utf8');
  assert.match(spec, /カーソルのバイト位置/);
});

test('Ctrl+Enter は節の分割（§8.1.1・§9.8.3）で、表の confirm-next とは別経路', async () => {
  const editor = await readFile(join(WEBUI, 'src/lib/editor/SectionEditor.svelte'), 'utf8');
  // エディタは `Ctrl+Enter` を `onsplit` へ渡す（`onkey` へ流さない）。
  assert.match(editor, /if \(onsplit\) \{/);
  assert.match(editor, /onsplit\(caret\)/);
  // 表の `Ctrl+Enter` は `confirm-next` のまま（キーの意味を変えない）。
  const keys = await readFile(join(WEBUI, 'src/lib/keyboard/keys.ts'), 'utf8');
  assert.match(keys, /'Ctrl\+Enter': 'confirm-next'/);
  // `confirm-next` は `session.command` から引ける（表に名前がある）。
  const commands = await readFile(join(REPO, 'src/app/commands.rs'), 'utf8');
  assert.match(commands, /"confirm-next"/);
});

test('plot.set_state の state は真偽（true = fixed）', async () => {
  const editing = await readFile(join(REPO, 'src/app/ops/editing.rs'), 'utf8');
  const body = bodyOf(editing, 'plot_set_state');
  assert.match(body, /bool_or\(args, "state", true\)/);
  assert.match(body, /if fixed \{ "fixed" \} else \{ "draft" \}/);
});

test('衝突の 2 つは利用者の選択でだけ動く（画面が勝手に決めない）', async () => {
  const editing = await readFile(join(REPO, 'src/app/ops/editing.rs'), 'utf8');
  // 自分の版を残す。
  assert.match(bodyOf(editing, 'keep_mine'), /keep_mine\(path/);
  // 外部の版を読み込む。
  assert.match(bodyOf(editing, 'reload_external'), /take_external\(path/);
  // 衝突が無ければ `Class::Conflict` を返す（画面はそれをそのまま出す）。
  assert.match(bodyOf(editing, 'keep_mine'), /Class::Conflict/);
  assert.match(bodyOf(editing, 'reload_external'), /Class::Conflict/);
  // 画面は両方のボタンを出す（片方だけにしない）。
  const dialog = await readFile(join(WEBUI, 'src/lib/save/ConflictDialog.svelte'), 'utf8');
  assert.match(dialog, /onkeep\?\.\(path\)/);
  assert.match(dialog, /onreload\?\.\(path\)/);
});

// ---------------------------------------------------------------- 3. 配線

test('配線: 5 つの操作を画面が実際に run で呼ぶ（コメントだけにしない）', async () => {
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  for (const op of [
    'section.split',
    'section.propose',
    'plot.set_state',
    'save.keep_mine',
    'save.reload_external',
  ]) {
    assert.match(app, new RegExp(`run\\('${op.replace('.', '\\.')}'`), `${op} を呼んでいない`);
  }
});

test('配線: 節の分割と提案はキーを知らなくても押せる（§21.2.1）', async () => {
  const editor = await readFile(join(WEBUI, 'src/lib/editor/SectionEditor.svelte'), 'utf8');
  const write = await readFile(join(WEBUI, 'src/lib/workspaces/write/WriteWorkspace.svelte'), 'utf8');
  const shell = await readFile(join(WEBUI, 'src/lib/shell/AppShell.svelte'), 'utf8');
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  // エディタのミニバーに 2 つのボタンがある（キーの案内ではなく、押せるボタン）。
  assert.match(editor, /on:click=\{\(\) => onsplit\?\.\(caret\)\}/);
  assert.match(editor, /on:click=\{\(\) => onpropose\?\.\(\)\}/);
  // 3 段の受け渡しが繋がっている（切れていると押しても何も起きない）。
  assert.match(write, /\{onsplit\}/);
  assert.match(write, /\{onpropose\}/);
  assert.match(shell, /onsplit=\{onsplit\}/);
  assert.match(shell, /onpropose=\{onpropose\}/);
  assert.match(app, /onsplit=\{\(position\) => void splitSection\(position\)\}/);
  assert.match(app, /onpropose=\{startProposal\}/);
});

test('配線: 衝突の面はステータスバーと App から開ける', async () => {
  const status = await readFile(join(WEBUI, 'src/lib/shell/StatusBar.svelte'), 'utf8');
  const shell = await readFile(join(WEBUI, 'src/lib/shell/AppShell.svelte'), 'utf8');
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  // 衝突している件数が出る（`save.state` の `conflicts`）。
  assert.match(status, /\{#if conflicts.length > 0\}/);
  assert.match(status, /onconflict\?\.\(\)/);
  assert.match(shell, /onconflict=\{onconflict\}/);
  assert.match(app, /onconflict=\{openConflicts\}/);
  assert.match(app, /\{#if conflicting\}/);
  // 一覧は `save.state` から取る（押したときに推測しない）。
  assert.match(app, /conflicts = conflictPaths\(/);
});

test('配線: プロットの確認状態は切替パネルから切り替えられる', async () => {
  const switcher = await readFile(join(WEBUI, 'src/lib/nav/Switcher.svelte'), 'utf8');
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  assert.match(switcher, /on:click=\{\(\) => ontogglestate\?\.\(\)\}/);
  assert.match(app, /ontogglestate=\{\(\) => void togglePlotState\(screen.scene\)\}/);
  assert.match(app, /plotState=\{stateOf\(/);
});

test('配線: 文言は生成物のカタログに実在する（新しいキーを足していない）', async () => {
  const catalog = await readFile(join(WEBUI, 'src/lib/i18n/catalog.ts'), 'utf8');
  for (const path of [
    'src/lib/sections/ProposalDialog.svelte',
    'src/lib/save/ConflictDialog.svelte',
  ]) {
    const source = await readFile(join(WEBUI, path), 'utf8');
    const keys = new Set([...source.matchAll(/\bt\(\s*'([a-z0-9_.]+)'/g)].map((found) => found[1]));
    assert.ok(keys.size >= 4, `${path} のキーが少なすぎる（${keys.size}）`);
    for (const key of keys) {
      assert.match(
        catalog,
        new RegExp(`'${key.replace(/\./g, '\\.')}':`),
        `${path}: ${key} がカタログに無い`,
      );
    }
  }
});

// ---------------------------------------------------- 4. パリティ判定との一致

test('PENDING から 5 件が外れ、parity の判定と一致する', async () => {
  const { PENDING } = await importTs('src/lib/coverage.ts');
  for (const id of [
    'section.split',
    'section.propose',
    'plot.set_state',
    'save.keep_mine',
    'save.reload_external',
  ]) {
    assert.ok(!PENDING.includes(id), `${id} がまだ未提供に残っている`);
  }
  // バッチ 3 の時点では 24 件。**件数はバッチごとに減る**ので、ここでは「24 以下で、
  // このバッチの 5 件が入っていない」ことだけを見る（件数の正本は `webui/tests/parity.mjs`）。
  assert.ok(PENDING.length <= 24, `未提供が増えている（${PENDING.length} 件）`);
});
