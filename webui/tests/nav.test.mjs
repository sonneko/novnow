// ナビゲーション（`nav.*` 5 つ + `document.rename`）の回帰（Node。`webui/src/lib/nav/` を読む）。
//
// 見るのは 4 つ:
//
// 1. 規則（`moves.ts`）: 切替パネルの行の組み立て（`nav.switcher` の戻り値から作る・番号を
//    写す・`previous` を勝手に決めない）・リネームの入力の検査（機能核の判定を先回りしない）。
// 2. **意味を機能核と取り違えない**: カタログの `nav.prev` / `nav.next` は「前 / 次の位置」＝
//    **移動履歴**（`src/app/ops/editing.rs` の `step_history`）であって、仕様 §9.8.2 の
//    「前 / 次の節」（`previous-section` / `next-section` → `Command::Nav`）ではない。
//    Rust のソースを読んで突き合わせる（画面側のコメントだけを信じない）。
// 3. 配線: 6 つを画面が実際に `run('<操作 ID>')` で呼ぶ。`Ctrl+K` / `F2` / `Ctrl+[` / `Ctrl+]` が
//    写像にあること。**`Ctrl+H` を奪っていない**こと（表の `replace` の唯一の口）。
// 4. パリティ判定との一致: `coverage.ts` の `PENDING` から 6 件が外れている。

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
  const out = await mkdtemp(join(tmpdir(), 'novnow-nav-'));
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

const moves = () => importTs('src/lib/nav/moves.ts');

// ------------------------------------------------------------------ 1. 規則

test('切替パネル: 移動の 4 行 + ワークスペース 9 行を nav.switcher の戻り値から作る', async () => {
  const { switcherRows, switcherCurrent, MOVE_COUNT } = await moves();
  const rows = switcherRows({
    current: 'idea',
    previous: 'write',
    workspaces: [
      { name: 'write', number: 1, current: false },
      { name: 'idea', number: 4, current: true },
      { name: 'review', number: 9, current: false },
    ],
  });
  assert.equal(MOVE_COUNT, 4);
  // 先頭は移動の 4 行（§9.8 の並び）。
  assert.deepEqual(
    rows.slice(0, 4).map((row) => row.op),
    ['nav.prev', 'nav.next', 'nav.back', 'nav.forward'],
  );
  const workspaces = rows.filter((row) => row.kind === 'workspace');
  assert.deepEqual(
    workspaces.map((row) => row.args.workspace),
    ['write', 'idea', 'review'],
  );
  // 番号は機能核の値を写す（画面で数え直さない。`Alt+<n>` と食い違わせない）。
  assert.deepEqual(workspaces.map((row) => row.number), [1, 4, 9]);
  assert.deepEqual(workspaces.map((row) => row.current), [false, true, false]);
  // `previous` は `nav.switcher` の値だけを信じる。
  assert.deepEqual(workspaces.map((row) => row.previous), [true, false, false]);
  // ラベルは生成物に実在する形（`header.workspace.<name>`）。
  for (const row of workspaces) {
    assert.equal(row.label, `header.workspace.${row.args.workspace}`);
  }
  assert.equal(switcherCurrent({ current: 'idea' }), 'idea');
});

test('切替パネル: 一覧が無くても移動の行は出す（空の一覧を白紙にしない）', async () => {
  const { switcherRows } = await moves();
  for (const found of [null, {}, { workspaces: 'x' }, { workspaces: [] }]) {
    const rows = switcherRows(found);
    assert.equal(rows.length, 4, JSON.stringify(found));
    assert.ok(rows.every((row) => row.kind === 'move'));
  }
  // 名前の無い行は落とす（空のボタンを並べない）。
  const rows = switcherRows({ workspaces: [{ name: '', number: 1 }, { number: 2 }, 3] });
  assert.equal(rows.length, 4);
});

test('リネーム: 入力を画面で先回りしない（形だけを見る）', async () => {
  const { renamePlan } = await moves();
  assert.deepEqual(renamePlan(' contents/01.cnov ', ' contents/02.cnov '), {
    ok: true,
    from: 'contents/01.cnov',
    to: 'contents/02.cnov',
  });
  // 空白だけ・同じ名前・外へ出る名前・制御文字は断る。
  assert.deepEqual(renamePlan('a', '   '), { ok: false, reason: 'empty' });
  assert.deepEqual(renamePlan('a', 'a'), { ok: false, reason: 'same' });
  assert.deepEqual(renamePlan('a', '../b'), { ok: false, reason: 'separator' });
  assert.deepEqual(renamePlan('a', '/b'), { ok: false, reason: 'separator' });
  assert.deepEqual(renamePlan('a', 'b/'), { ok: false, reason: 'separator' });
  assert.deepEqual(renamePlan('a', 'b//c'), { ok: false, reason: 'separator' });
  assert.deepEqual(renamePlan('a', 'b\u0000c'), { ok: false, reason: 'invalid' });
  // 中へ移すのは許す（`contents/` の中の移動は利用者の領分）。
  assert.equal(renamePlan('contents/01.cnov', 'plots/01.cnov').ok, true);
});

test('リネーム: 失敗の文言は生成物に実在するキーを引く', async () => {
  const { RENAME_MESSAGES } = await moves();
  const catalog = await readFile(join(WEBUI, 'src/lib/i18n/catalog.ts'), 'utf8');
  assert.ok(Object.keys(RENAME_MESSAGES).length >= 3);
  for (const key of Object.values(RENAME_MESSAGES)) {
    assert.match(catalog, new RegExp(`'${key.replace(/\./g, '\\.')}':`), `${key} がカタログに無い`);
  }
});

test('移動の結果: moved=false は失敗ではない（端で黙って止まる）', async () => {
  const { movedScene, didMove } = await moves();
  assert.equal(movedScene({ moved: false }), '');
  assert.equal(didMove({ moved: false }), false);
  assert.equal(movedScene(null), '');
  assert.equal(movedScene({ moved: true, scene: '01-02-01-01' }), '01-02-01-01');
  assert.equal(didMove({ moved: true }), true);
  // `scene` を返さない実装でも「動いた」は見る。
  assert.equal(movedScene({ moved: true }), '');
  assert.equal(didMove({ moved: true }), true);
});

test('キーの写像: Ctrl+[ / Ctrl+] は nav.prev / nav.next、Ctrl+H は写さない', async () => {
  const { moveOf } = await moves();
  // `App.svelte` のキー処理は `LOCAL` の名前（`ui:nav-prev`）から `move('nav.prev')` を呼ぶ。
  // `moveOf` は「キー → 操作」の規則を 1 か所に置くための表で、テストが中身を確かめる。
  assert.equal(moveOf('Ctrl+BracketLeft'), 'nav.prev');
  assert.equal(moveOf('Ctrl+BracketRight'), 'nav.next');
  // 表の `Ctrl+H` は `replace`（仕様 §9.8.1 の検索・置換）の唯一の口。奪わない。
  assert.equal(moveOf('Ctrl+H'), null);
  assert.equal(moveOf('Ctrl+Shift+H'), null);
  assert.equal(moveOf('F2'), null);
});

// ------------------------------------------- 2. 意味を機能核と取り違えない

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

test('nav.prev / nav.next は「移動履歴」である（構造移動ではない）', async () => {
  const editing = await readFile(join(REPO, 'src/app/ops/editing.rs'), 'utf8');
  // 機能核: 履歴を 1 つ進む / 戻る（`step_history`）。
  assert.match(bodyOf(editing, 'nav_prev'), /step_history\(session, false\)/);
  assert.match(bodyOf(editing, 'nav_next'), /step_history\(session, true\)/);
  const step = bodyOf(editing, 'step_history');
  assert.match(step, /session\.history\[index\]/, '履歴を使っていない');
  assert.doesNotMatch(step, /Workbench::go|\.bench\.go\(/, '構造移動になっている');
  // 仕様 §9.8.2 の「前 / 次の節」は別のコマンド（`previous-section` → `Command::Nav`）。
  const session = await readFile(join(REPO, 'src/app/session.rs'), 'utf8');
  assert.match(bodyOf(session, 'nav_of'), /"prev-section" => Nav::PrevSection/);
  // 画面のコメントも同じ意味で書く（取り違えたコメントを残さない）。
  const source = await readFile(join(WEBUI, 'src/lib/nav/moves.ts'), 'utf8');
  assert.match(source, /移動履歴/);
  assert.doesNotMatch(source, /nav\.prev.*前の節/s);
});

test('nav.back / nav.forward は親へ / 子の先頭へ（interfaces.md の定義どおり）', async () => {
  const editing = await readFile(join(REPO, 'src/app/ops/editing.rs'), 'utf8');
  // 親が無ければ `moved: false`（エラーにしない）。画面もそう扱う。
  assert.match(bodyOf(editing, 'nav_back'), /moved.*false/);
  assert.match(bodyOf(editing, 'nav_forward'), /moved.*false/);
});

test('nav.switcher は 9 ワークスペースを番号つきで返す（画面の一覧の正本）', async () => {
  const editing = await readFile(join(REPO, 'src/app/ops/editing.rs'), 'utf8');
  const body = bodyOf(editing, 'nav_switcher');
  // 一覧は機能核が作る（画面で並べ直さない）。`number` と `current` を返す。
  assert.match(body, /"number": workspace\.number\(\)/);
  assert.match(body, /"current": \*workspace == session\.switcher\.current\(\)/);
  assert.match(body, /"previous": session\.switcher\.previous\(\)/);
  const catalog = JSON.parse(await readFile(join(REPO, 'tests/golden/api_catalog.json'), 'utf8'));
  const op = catalog.operations.find((found) => found.id === 'nav.switcher');
  assert.deepEqual(Object.keys(op.returns.properties).sort(), ['current', 'previous', 'workspaces']);
});

// ---------------------------------------------------------------- 3. 配線

test('配線: 6 つの操作を画面が実際に run で呼ぶ（コメントだけにしない）', async () => {
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  for (const op of [
    'nav.prev',
    'nav.next',
    'nav.back',
    'nav.forward',
    'nav.switcher',
    'document.rename',
  ]) {
    assert.match(
      app,
      new RegExp(`run\\('${op.replace('.', '\\.')}'`),
      `${op} を呼んでいない`,
    );
  }
});

test('配線: Ctrl+K / Ctrl+[ / Ctrl+] が切替パネルと移動へ届く', async () => {
  const { LOCAL, command } = await importTs('src/lib/keyboard/keys.ts');
  assert.equal(LOCAL['Ctrl+K'], 'ui:switcher');
  // 文書のリネームのキーは足さない（仕様 §9.8 に無い。`rename` は別の意味で表にある）。
  assert.equal(LOCAL.F2, undefined);
  // 表記は `chord()` と同じ（`event.key` の 1 文字を大文字にする）。`event.code` ではない。
  assert.equal(LOCAL['Ctrl+['], 'ui:nav-prev');
  assert.equal(LOCAL['Ctrl+]'], 'ui:nav-next');
  // `Ctrl+H` は表の `replace` のまま（画面側で奪っていない）。
  assert.equal(LOCAL['Ctrl+H'], undefined);
  const key = (over) => ({
    key: 'x',
    code: 'KeyX',
    ctrl: false,
    shift: false,
    alt: false,
    meta: false,
    ...over,
  });
  assert.equal(command(key({ key: 'k', ctrl: true }), { mac: false }), 'ui:switcher');
  assert.notEqual(command(key({ key: 'F2' }), { mac: false }), 'ui:rename');
  assert.equal(command(key({ key: '[', code: 'BracketLeft', ctrl: true }), { mac: false }), 'ui:nav-prev');
  assert.equal(command(key({ key: ']', code: 'BracketRight', ctrl: true }), { mac: false }), 'ui:nav-next');
});

test('配線: 画面側で完結する名前は ui: を付け、表の名前と混ぜない', async () => {
  const { LOCAL } = await importTs('src/lib/keyboard/keys.ts');
  const commands = await readFile(join(REPO, 'src/app/commands.rs'), 'utf8');
  // 画面側の名前はすべて `ui:` で始まる（`session.command` へ流さない）。
  for (const value of Object.values(LOCAL)) {
    assert.match(value, /^ui:/, `${value} に ui: が付いていない`);
  }
  // このバッチで足した点は、**機能核の同じ名前のコマンドを指していない**こと。
  // （`help` / `rename` は表に別の意味で存在するので、`ui:` の接頭辞で区別している。
  //   仕様 §9.8.1 の `?` は表の `help`（機能核のパレット）ではなく画面の重ね物。）
  for (const name of ['switcher', 'nav-prev', 'nav-next']) {
    assert.doesNotMatch(
      commands,
      new RegExp(`"${name}"`),
      `${name} が表にある（画面側で取ると意味が変わる）`,
    );
  }
  // 逆に、表にある `replace` を画面側で取っていない（`search.replace_file` の唯一の口）。
  assert.equal(LOCAL['Ctrl+H'], undefined);
  assert.match(commands, /"replace"/);
});

test('配線: `>` に載せるコマンドは session.command が引ける名前だけ', async () => {
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  const commands = await readFile(join(REPO, 'src/app/commands.rs'), 'utf8');
  const catalog = JSON.parse(await readFile(join(REPO, 'tests/golden/api_catalog.json'), 'utf8'));
  const table = new Set([...commands.matchAll(/^\s*\("([a-z0-9-]+)"/gm)].map((found) => found[1]));
  assert.ok(table.size > 30, `表から名前を集める（${table.size} 件）`);
  // `PALETTE_ROWS` の `name:` を集めて、すべて表に実在することを確かめる。
  const start = app.indexOf('const PALETTE_ROWS');
  const end = app.indexOf('];', start);
  assert.ok(start >= 0 && end > start, 'App.svelte から PALETTE_ROWS を読めない');
  for (const found of app.slice(start, end).matchAll(/name: '([a-z0-9-]+)'/g)) {
    assert.ok(table.has(found[1]), `>${found[1]} が commands.rs の表に無い（usage エラーになる）`);
  }
  // `document.rename` のカタログ名 `rename-document` は表に**無い**。だから `>` には載せず、
  // 画面の面（ヘッダ・切替パネル）から開く。この前提が崩れたら気づけるようにする。
  assert.equal(
    catalog.operations.find((op) => op.id === 'document.rename').command,
    'rename-document',
  );
  assert.ok(!table.has('rename-document'), '表に入ったなら `>` に載せてよい（このテストを見直す）');
  // `>` の一覧は `oncommand` で実行される（飾りではない）。
  const overlay = await readFile(join(WEBUI, 'src/lib/shell/Overlay.svelte'), 'utf8');
  assert.match(overlay, /on:click=\{\(\) => oncommand\?\.\(row\.name\)\}/);
});

test('配線: 画面は切替パネルとリネームの面を持つ（押せる導線）', async () => {
  const switcher = await readFile(join(WEBUI, 'src/lib/nav/Switcher.svelte'), 'utf8');
  const rename = await readFile(join(WEBUI, 'src/lib/nav/RenameDialog.svelte'), 'utf8');
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  const header = await readFile(join(WEBUI, 'src/lib/shell/Header.svelte'), 'utf8');
  // 一覧は `nav.switcher` の戻り値から作る。
  assert.match(switcher, /switcherRows\(found\)/);
  assert.match(switcher, /onpick\?\.\(row\)/);
  // リネームは 1 つの入力欄と `Esc` / 外側クリックで閉じる（§9.7.5）。
  assert.match(rename, /renamePlan\(from, value\)/);
  assert.match(rename, /event\.key === 'Escape'/);
  // ヘッダから開ける（キーを知らなくても届く）。
  assert.match(header, /on:click=\{\(\) => onswitcher\?\.\(\)\}/);
  assert.match(header, /on:click=\{\(\) => onrename\?\.\(\)\}/);
  assert.match(app, /onswitcher=\{\(\) => void openSwitcher\(\)\}/);
  assert.match(app, /onrename=\{\(\) => void startRename\(\)\}/);
});

// ---------------------------------------------------- 4. パリティ判定との一致

test('B2 の 6 件は PENDING から外れている（件数は parity.mjs が持つ）', async () => {
  const { PENDING } = await importTs('src/lib/coverage.ts');
  for (const id of [
    'nav.prev',
    'nav.next',
    'nav.back',
    'nav.forward',
    'nav.switcher',
    'document.rename',
  ]) {
    assert.ok(!PENDING.includes(id), `${id} がまだ未提供に残っている`);
  }
  // バッチ6 で未提供は 0 件になった（`parity.mjs` が 10 → 0 を突き合わせる）。
  // 判定の 1 か所主義（§21.5.1）に反しないよう、件数は `parity.mjs` が持つ。
  assert.equal(PENDING.length, 0, '未提供が 0 件でない（parity の判定とずれている）');
});
