// プロジェクトの入口の面（`project.new` / `recent_list` / `recent_remove` / `import_zip` /
// `export_zip`）の回帰（Node。`webui/src/lib/project/` を読む）。
//
// 見るのは 4 つ:
//
// 1. 規則（`picker.ts`）: 一覧の並び（いま開いている 1 件が先頭・重複を落とす・新しい順）・
//    新規作成のパスと名前の検査・Zip の引数の組み立て（環境で `out` の有無が変わる）・
//    取り込みの不足ファイル・履歴から消せる条件。
// 2. **提供範囲が機能核と一致する**: `capabilities` が「ブラウザでは `project.new` /
//    `recent_list` / `recent_remove` が使えない」と言うのは、`REF:src/app/ops/project.rs` が
//    同じ 3 つを `#[cfg(target_arch = "wasm32")]` で `no_fs` にしているからである。写し間違いを
//    防ぐため、Rust の `cfg` を読んで突き合わせる（画面だけ「使える」と言わない）。
// 3. 配線の約束（`.svelte` / `App.svelte`）: 5 つの操作を `run('<操作 ID>')` で実際に呼ぶ。
//    `Ctrl+O` から入口の面が開く。文言は生成物 `i18n/catalog.ts` に実在する（§12.3）。
// 4. パリティ判定との一致: `coverage.ts` の `PENDING` から 5 件が外れている（`parity.mjs` が見る）。

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
  const out = await mkdtemp(join(tmpdir(), 'novnow-project-'));
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

const picker = () => importTs('src/lib/project/picker.ts');

// ------------------------------------------------------------------ 1. 規則

test('一覧: いま開いている 1 件が先頭、重複は 1 行、新しい順', async () => {
  const { projectRows } = await picker();
  const rows = projectRows('now', [
    { path: 'old', opened_at: '2026-01-01T00:00:00+09:00' },
    { path: 'now', opened_at: '2026-03-01T00:00:00+09:00' },
    { path: 'newest', opened_at: '2026-06-01T00:00:00+09:00' },
    { path: 'middle', opened_at: '2026-02-01T00:00:00+09:00' },
    { path: 'newest', opened_at: '2026-06-02T00:00:00+09:00' },
    { path: '', opened_at: '2026-07-01T00:00:00+09:00' },
  ]);
  assert.deepEqual(
    rows.map((row) => row.path),
    ['now', 'newest', 'middle', 'old'],
  );
  // いま開いている 1 件だけ `current`。履歴に残っていても行は増えない。
  assert.deepEqual(rows.map((row) => row.current), [true, false, false, false]);
  assert.equal(rows.filter((row) => row.path === 'now').length, 1);
});

test('一覧: 開いている 1 件が無ければ履歴だけ（空のパスは行にしない）', async () => {
  const { projectRows } = await picker();
  assert.deepEqual(projectRows('', []), []);
  const rows = projectRows('', [{ path: 'a', opened_at: '2026-01-01T00:00:00+09:00' }]);
  assert.deepEqual(rows.map((row) => [row.path, row.current]), [['a', false]]);
});

test('新規作成: パスと題名の組み立て、環境ごとの可否、名前の検査', async () => {
  const { newProjectPlan, validProjectName, capabilities } = await picker();
  // ネイティブ: 親フォルダ＋名前。末尾の区切りは落とす。`\` も `/` に揃える。
  assert.deepEqual(newProjectPlan('/home/you/projects/', ' 星屑の時計 ', 'native'), {
    path: '/home/you/projects/星屑の時計',
    title: '星屑の時計',
  });
  assert.deepEqual(newProjectPlan('C:\\novels', '新作', 'native'), {
    path: 'C:\\novels/新作',
    title: '新作',
  });
  // ブラウザはファイルに書けない（機能核が `no_fs`）。
  assert.equal(newProjectPlan('/tmp', 'x', 'browser'), 'not-native');
  assert.equal(newProjectPlan('/tmp', '   ', 'native'), 'empty');
  assert.equal(newProjectPlan('  ', 'x', 'native'), 'no-separator');
  for (const bad of ['', '   ', 'a/b', 'a\\b', 'a:b', 'a?b', 'a*b', 'a"b', 'a<b', 'a>b', 'a|b']) {
    assert.equal(validProjectName(bad), false, bad);
  }
  assert.equal(validProjectName('星屑の時計 2'), true);
  assert.equal(capabilities('browser').newProject, false);
  assert.equal(capabilities('native').newProject, true);
});

test('Zip: 書き出しの引数は環境で変わる（ブラウザは out を渡さない）', async () => {
  const { exportArgs, capabilities } = await picker();
  // ネイティブ: 保存ダイアログの行き先を渡す。空なら中身だけ受け取る（黙って別の場所へ書かない）。
  assert.deepEqual(exportArgs('native', '/tmp/out.zip'), { out: '/tmp/out.zip' });
  assert.deepEqual(exportArgs('native', '   '), {});
  // ブラウザ: ブラウザ用 Host の `write_chosen` は UTF-8 の文字列しか受けない
  // （`REF:crates/novnow-wasm/src/host.rs`）。Zip はバイナリなので `out` を渡さず、
  // `bytes` と `name` を受け取って画面がダウンロードする。
  assert.deepEqual(exportArgs('browser', '/tmp/out.zip'), {});
  assert.equal(capabilities('browser').exportToPath, false);
  assert.equal(capabilities('native').exportToPath, true);
  // どちらの環境でも Zip の入出力そのものは提供する。
  for (const runtime of ['browser', 'native']) {
    assert.equal(capabilities(runtime).exportZip, true, runtime);
    assert.equal(capabilities(runtime).importZip, true, runtime);
  }
});

test('Zip: 取り込みの引数（ネイティブはパス、ブラウザは base64）', async () => {
  const { importArgs } = await picker();
  assert.deepEqual(importArgs('native', { path: '/tmp/novel.zip' }), { path: '/tmp/novel.zip' });
  // ネイティブでも選ばれたのが名前だけなら `bytes` を使う（`path` として存在しない）。
  assert.deepEqual(importArgs('native', { bytes: 'UEsDBA==' }), { bytes: 'UEsDBA==' });
  assert.deepEqual(importArgs('browser', { path: 'novel.zip', bytes: 'UEsDBA==' }), { bytes: 'UEsDBA==' });
  assert.deepEqual(importArgs('browser', { path: 'novel.zip' }), {});
});

test('取り込み: 不足ファイルの判定と、履歴から消せる条件', async () => {
  const { missingFiles, canForget, importSummary } = await picker();
  assert.deepEqual(
    missingFiles(['project.json', 'ideas.md', 'words.md', 'characters.md']),
    [],
  );
  assert.deepEqual(missingFiles(['project.json', 'ideas.md']), ['words.md', 'characters.md']);
  assert.equal(importSummary(['a.md', 'b.md'], [1]), '2/3');
  assert.equal(canForget({ path: 'x', openedAt: '', current: false }), true);
  // いま開いている 1 件は履歴から消さない（第 1 の画面と同じ）。
  assert.equal(canForget({ path: 'x', openedAt: '', current: true }), false);
  assert.equal(canForget({ path: '  ', openedAt: '', current: false }), false);
});

// -------------------------------------------------- 2. 提供範囲が機能核と一致する

test('ブラウザで使えない 3 つは、機能核の cfg と一致する（画面だけ「使える」と言わない）', async () => {
  const { capabilities } = await picker();
  const rust = await readFile(join(REPO, 'src/app/ops/project.rs'), 'utf8');
  // `project.rs` は操作ごとに関数を切り、`#[cfg(target_arch = "wasm32")]` の腕で
  // `api.error.no_fs` を返す。その 3 つ（new / recent_list / recent_remove）が
  // 「ブラウザでは押せない」に対応する。
  /** 関数 `name` の本体（`pub fn name(` から、波括弧の対応が閉じるまで）。 */
  function bodyOf(name) {
    const start = rust.indexOf(`fn ${name}(`);
    assert.ok(start >= 0, `${name} が project.rs に無い`);
    const open = rust.indexOf('{', start);
    let depth = 0;
    for (let index = open; index < rust.length; index += 1) {
      if (rust[index] === '{') {
        depth += 1;
      } else if (rust[index] === '}') {
        depth -= 1;
        if (depth === 0) {
          return rust.slice(open, index + 1);
        }
      }
    }
    throw new Error(`${name} の本体が閉じない`);
  }

  // `project.new` は `create_project` へ委ねる（本体に cfg が無い）ので、委ね先も見る。
  const gated = [
    ['new', 'create_project'],
    ['recent_list', null],
    ['recent_remove', null],
  ];
  for (const [name, delegate] of gated) {
    let body = bodyOf(name);
    if (delegate) {
      assert.match(body, new RegExp(delegate), `${name} が ${delegate} へ委ねていない`);
      body = bodyOf(delegate);
    }
    assert.match(body, /cfg\(target_arch = "wasm32"\)/, `${name} に wasm32 の cfg が無い`);
    assert.match(body, /no_fs/, `${name} が wasm32 で no_fs を返していない`);
  }
  assert.deepEqual(
    {
      newProject: capabilities('browser').newProject,
      recentList: capabilities('browser').recentList,
      recentRemove: capabilities('browser').recentRemove,
    },
    { newProject: false, recentList: false, recentRemove: false },
  );
  // Zip の 2 つは cfg で切っていない（両方の環境で提供する）。
  for (const name of ['import_zip', 'export_zip']) {
    assert.doesNotMatch(bodyOf(name), /no_fs/, `${name} は両方の環境で提供する`);
  }
});

// ---------------------------------------------------------------- 3. 配線

test('配線: 5 つの操作を画面が実際に run で呼ぶ（コメントだけにしない）', async () => {
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  for (const op of [
    'project.new',
    'project.recent_list',
    'project.recent_remove',
    'project.import_zip',
    'project.export_zip',
  ]) {
    assert.match(app, new RegExp(`run\\('${op.replace('.', '\\.')}'`), `${op} を呼んでいない`);
  }
});

test('配線: Ctrl+O が入口の面を開き、プロジェクト名のボタンも同じ規則で開く', async () => {
  const { LOCAL, command } = await importTs('src/lib/keyboard/keys.ts');
  assert.equal(LOCAL['Ctrl+O'], 'ui:project');
  const stroke = { key: 'o', code: 'KeyO', ctrl: true, shift: false, alt: false, meta: false };
  assert.equal(command(stroke, { mac: false }), 'ui:project');
  const app = await readFile(join(WEBUI, 'src/App.svelte'), 'utf8');
  // `ui:project` は入口の面を開く（保存してから。§9.7 の Ctrl+O の統一）。
  assert.match(app, /case 'ui:project':/);
  assert.match(app, /openProjectPicker\(\)/);
  assert.match(app, /overlay = 'project'/);
  // 保存に失敗している間は開かない。
  assert.match(app, /screen\.save === 'error' \|\| screen\.save === 'conflict'/);
  // プロジェクト名のボタンも同じ関数へ繋ぐ（黙って画面を移さない）。
  assert.match(app, /onproject=\{\(\) => void openProjectPicker\(\)\}/);
});

test('配線: 入口の面は環境で押せないものを理由付きで示す（§21.1.2）', async () => {
  const svelte = await readFile(join(WEBUI, 'src/lib/project/ProjectPicker.svelte'), 'utf8');
  assert.match(svelte, /caps\.recentList/);
  assert.match(svelte, /caps\.newProject/);
  // 未提供は押せないだけでなく、理由を書く。
  assert.match(svelte, /workspace\.unavailable/);
  // ブラウザの Zip 書き出しは `bytes` をダウンロードさせる（`out` を渡さない）。
  assert.match(svelte, /download\(found\)/);
});

test('配線: 入口の面が引く文言は、生成物のカタログに実在する（§12.3）', async () => {
  const svelte = await readFile(join(WEBUI, 'src/lib/project/ProjectPicker.svelte'), 'utf8');
  const catalog = await readFile(join(WEBUI, 'src/lib/i18n/catalog.ts'), 'utf8');
  const keys = new Set([...svelte.matchAll(/\bt\(\s*'([a-z0-9_.]+)'/g)].map((found) => found[1]));
  assert.ok(keys.size >= 10, `キーを集める（${keys.size} 件）`);
  for (const key of keys) {
    assert.match(catalog, new RegExp(`'${key.replace(/\./g, '\\.')}':`), `${key} がカタログに無い`);
  }
});

// ---------------------------------------------------- 4. パリティ判定との一致

test('B1 の 5 件は PENDING から外れている（件数は parity.mjs が持つ）', async () => {
  const { PENDING } = await importTs('src/lib/coverage.ts');
  for (const id of [
    'project.new',
    'project.recent_list',
    'project.recent_remove',
    'project.import_zip',
    'project.export_zip',
  ]) {
    assert.ok(!PENDING.includes(id), `${id} がまだ未提供に残っている`);
  }
  // バッチ6 で未提供は 0 件になった（`parity.mjs` が 10 → 0 を突き合わせる）。
  // 件そのものは `parity.mjs` が突き合わせる（ここで総数を固定すると、次のバッチで
  // 「前のバッチのテストが落ちる」ことになり、判定の 1 か所主義（§21.5.1）に反する）。
  assert.equal(PENDING.length, 0, '未提供が 0 件でない（parity の判定とずれている）');
});
