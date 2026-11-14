// キー写像の回帰（Node。`webui/src/lib/keyboard/keys.ts` をそのまま読む）。
//
// 見るのは 4 つ:
//
// 1. **表に載っている名前が実在する**（`REF:src/app/commands.rs` の `PALETTE_COMMANDS` と
//    `SHORTCUT_PALETTE_COMMANDS` から作る）。綴りを写し間違えると、キーが黙って効かなくなる。
// 2. `?` / `Ctrl+P` / `Esc` は画面側だけで完結する（`ui:` の接頭辞。`session.command` へ渡さない）。
// 3. `Ctrl+Shift+1..9` は節を持ち越す切替として引数を返す（§10.0）。
// 4. ヘルプ（`?`）の一覧が引く文言のキーが、生成物 `i18n/catalog.ts` に実在する（§12.3）。
//
// TypeScript を Node で読むため、`tsc` で 1 度だけ書き出してから読む（型検査は `npm run check`）。

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

const ROOT = new URL('..', import.meta.url);
const REPO = new URL('../../', import.meta.url);

/** `webui/src/lib/keyboard/*.ts` を JS へ書き出して読む（1 ファイル 1 回）。 */
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
      // `tsconfig.json` があると「コマンドラインの指定と併用できない」と断られるので外す。
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

/** `keys.ts`（キー写像）。 */
function keys() {
  return importTs('src/lib/keyboard/keys.ts');
}

/** `event.key` と修飾キーから素性を作る（`KeyboardEvent` の代わり）。 */
function stroke(key, { ctrl = false, shift = false, alt = false, code = '' } = {}) {
  return { key, code: code || `Key${key.toUpperCase()}`, ctrl, shift, alt, meta: false };
}

/**
 * `src/app/commands.rs` の表からコマンド名を集める。
 *
 * 表は 1 行の組と、`(` で始まる複数行の組が混ざる（`swap-section-up` など）ので、
 * `"名前",` と `"Alt+…"` の並びを 1 組として拾う。
 */
async function commandNames() {
  const text = await readFile(new URL('src/app/commands.rs', REPO), 'utf8');
  const names = new Set();
  // 2 列目はキー（`Ctrl+S`）のこともあれば文言のキー（`command.save`）のこともある。
  // どちらも「名前, 文字列」の並びなので、引用符の中身は問わない（改行はまたぐ）。
  for (const found of text.matchAll(/"([a-z0-9-]+)",\s*\n?\s*"[^"\n]*"/g)) {
    names.add(found[1]);
  }
  return names;
}

test('ショートカットの値は session.command が知っている名前だけ', async () => {
  const { SHORTCUTS } = await keys();
  const known = await commandNames();
  assert.ok(known.size > 50, `コマンド表が小さすぎる（${known.size} 件）`);
  for (const [key, name] of Object.entries(SHORTCUTS)) {
    if (name.startsWith('ui:')) {
      continue;
    }
    assert.ok(known.has(name), `${key} → ${name} が src/app/commands.rs の表に無い`);
  }
});

test('? はヘルプ、Ctrl+P はパレット、Esc は閉じる（画面側だけで完結する）', async () => {
  const { command } = await keys();
  const options = { mac: false };
  const help = command(stroke('?', { shift: true, code: 'Slash' }), options);
  assert.equal(help, 'ui:help');
  assert.ok(help.startsWith('ui:'));
  assert.equal(command(stroke('p', { ctrl: true }), options), 'ui:palette');
  assert.equal(command(stroke('Escape'), options), 'ui:close');
  // IME の変換中は画面が先に捨てるので、ここでは何も見ない（§9.8.5）。
});

test('Ctrl+Shift+P は提案モードで、パレットではない（§8.7）', async () => {
  const { command } = await keys();
  const found = command(stroke('P', { ctrl: true, shift: true }), { mac: false });
  assert.notEqual(found, 'ui:palette');
});

test('Ctrl+Shift+1..9 は節を持ち越す切替の引数を返す（§10.0）', async () => {
  const { keepPayload } = await keys();
  assert.deepEqual(keepPayload('workspace-keep-3'), { workspace: 3 });
  assert.deepEqual(keepPayload('workspace-keep-9'), { workspace: 9 });
  assert.equal(keepPayload('workspace-keep-0'), null);
  assert.equal(keepPayload('workspace-write'), null);
});

test('ヘルプの一覧は、文言カタログにあるキーだけを使う', async () => {
  const { HELP_GROUPS } = await import('../src/lib/keyboard/help.ts').catch(async () => {
    // Node は `.ts` を読めないので、`tsc` で書き出す（`keys()` と同じやり方）。
    const out = await mkdtemp(join(tmpdir(), 'novnow-help-'));
    execFileSync(
      process.execPath,
      [
        join(new URL('.', ROOT).pathname, 'node_modules', 'typescript', 'bin', 'tsc'),
        join(new URL('.', ROOT).pathname, 'src', 'lib', 'keyboard', 'help.ts'),
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
    return import(join(out, 'help.js'));
  });
  const catalog = await readFile(new URL('src/lib/i18n/catalog.ts', ROOT), 'utf8');
  const keys = new Set([...catalog.matchAll(/^\s*'([a-z0-9_.]+)':/gm)].map((found) => found[1]));
  assert.ok(keys.size > 900, `カタログが小さすぎる（${keys.size} 件）`);
  for (const group of HELP_GROUPS) {
    assert.ok(keys.has(group.title), `見出し ${group.title} がカタログに無い`);
    for (const row of group.rows) {
      assert.ok(keys.has(row.label), `説明 ${row.label} がカタログに無い`);
      assert.ok(row.keys.length > 0, `${row.label} のキー表示が空`);
    }
  }
});
