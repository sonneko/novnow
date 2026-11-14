// 橋渡しの回帰（Node。ブラウザを使わずに、wasm と IndexedDB の規則を確かめる）。
//
// 受け入れ（`REF:docs/plans.md#webui-w2`）のうち、画面の見た目に依らない部分をここで見る:
//
// 1. サンプル作品を開ける（`open`）。
// 2. 節の本文を打つと `section.write` で書ける。
// 3. `flush` の後に開き直すと本文が残る（**IndexedDB に本当に入っている**）。
// 4. 流し込みに失敗したら、キューに残って次の `flush` で成功する（§8.3）。
//
// ブラウザ無しで動かすため、`window` と `indexedDB` を `fake-indexeddb` で作る。
// 同じコードがブラウザでも動くことは `scripts/web-smoke.sh` が確かめる（W6）。

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

import { IDBFactory, IDBKeyRange } from 'fake-indexeddb';

/** ブラウザの代わりに置く最小の `window`（`novnow-wasm` は `globalThis` から indexedDB を取る）。 */
function browser() {
  const factory = new IDBFactory();
  globalThis.indexedDB = factory;
  globalThis.IDBKeyRange = IDBKeyRange;
  globalThis.window = globalThis;
  globalThis.self = globalThis;
  if (!globalThis.structuredClone) {
    globalThis.structuredClone = (value) => JSON.parse(JSON.stringify(value));
  }
  return factory;
}

/** wasm を 1 回だけ読む（init は何度呼んでもよいが、読むのは 1 度にする）。 */
let module_ = null;
async function wasm() {
  if (!module_) {
    browser();
    module_ = await import('../src/wasm/novnow_wasm.js');
    const bytes = await readFile(new URL('../src/wasm/novnow_wasm_bg.wasm', import.meta.url));
    await module_.default({ module_or_path: bytes });
  }
  return module_;
}

/** `run` の文字列を解く。 */
function decode(text) {
  const found = JSON.parse(text);
  assert.equal(found.ok, true, `run failed: ${text}`);
  return found.result;
}

test('サンプル作品を開いて節を書ける', async () => {
  browser();
  const { NovnowSession } = await wasm();
  const session = new NovnowSession();
  await session.open(null);
  assert.equal(session.project_id(), 'sample');

  const info = decode(session.run('project.info', '{}'));
  assert.ok(info.sections > 0, 'サンプルに節がある');
  assert.ok(info.title.length > 0, '題名がある');

  const list = decode(session.run('section.list', '{}'));
  assert.ok(list.count > 0, '節の一覧が空でない');
  const scene = list.sections[0].id;

  const got = decode(session.run('section.get', JSON.stringify({ scene })));
  assert.equal(got.scene, scene);

  const written = decode(
    session.run('section.write', JSON.stringify({ scene, text: '書き込みの試験。\n2 行目。', role: '' })),
  );
  assert.equal(written.scene, scene);
  assert.ok(written.chars > 0);
});

test('flush の後に開き直すと本文が残る（IndexedDB）', async () => {
  browser();
  const { NovnowSession } = await wasm();
  const session = new NovnowSession();
  await session.open(null);
  const scene = decode(session.run('section.list', '{}')).sections[0].id;
  const text = '再読み込みの試験。\n二行目も残る。';

  decode(session.run('section.write', JSON.stringify({ scene, text, role: '' })));
  const flushed = JSON.parse(await session.flush());
  assert.equal(flushed.ok, true);
  assert.equal(session.pending_writes(), 0, '流し終えたらキューは空');

  // 同じ IndexedDB から開き直す（`open` は 1 つ目のプロジェクトを読む）
  const again = new NovnowSession();
  await again.open(null);
  const found = decode(again.run('section.get', JSON.stringify({ scene })));
  assert.equal(found.text, text, '書いた本文がそのまま残る');
});

test('書き込みは save.now まで溜まり、flush で保存領域へ流れる', async () => {
  browser();
  const { NovnowSession } = await wasm();
  const session = new NovnowSession();
  await session.open(null);
  const scene = decode(session.run('section.list', '{}')).sections[0].id;
  assert.equal(session.pending_writes(), 0, '開いた直後は空');

  // `Interface::Rust` は**状態を持つ**ので、打鍵は `section.write` でメモリの集約に入り、
  // 書き込みは `save.now`（自動保存の 1 回分）まで溜まる（§8.2・§19.1.5）
  const written = decode(
    session.run('section.write', JSON.stringify({ scene, text: 'キューの試験。', role: '' })),
  );
  assert.equal(written.save.state, 'dirty');
  assert.equal(session.pending_writes(), 0, 'まだ保存していないのでキューは空');

  const saved = decode(session.run('save.now', '{}'));
  // 同期の保存がその場で終わると状態機は `clean` に戻る（`saved` は次のポーリングまでの
  // 短い表示。§8.2・`REF:src/app/save_machine.rs`）
  assert.ok(['saved', 'clean'].includes(saved.state), `unexpected state: ${saved.state}`);
  assert.ok(session.pending_writes() > 0, '保存するとキューに積まれる（IndexedDB へは未流し）');

  const flushed = JSON.parse(await session.flush());
  assert.equal(flushed.ok, true);
  assert.equal(session.pending_writes(), 0, '流し終えたらキューは空');
});

// 流し込みの**失敗**の経路は Rust 側の単体テスト
// （`crates/novnow-wasm/src/storage.rs` の `failed_drain_keeps_the_queue`）が確かめる。
// ここから IndexedDB を意図的に壊すと、`fake-indexeddb` の接続が残って後続のテストが
// 巻き添えになるため、Node 側では「積まれる・流れる」だけを見る。

test('同じ操作は同じ JSON を返す（決定論）', async () => {
  browser();
  const { NovnowSession } = await wasm();
  const session = new NovnowSession();
  await session.open(null);
  const first = session.run('structure.breadcrumb', '{}');
  const second = session.run('structure.breadcrumb', '{}');
  assert.equal(first, second, 'キーの順も含めて同じ');
});

test('開く前の操作は usage エラー（例外を投げない）', async () => {
  browser();
  const { NovnowSession } = await wasm();
  const session = new NovnowSession();
  const found = JSON.parse(session.run('project.info', '{}'));
  assert.equal(found.ok, false);
  assert.equal(found.error.code, 'usage');
});
