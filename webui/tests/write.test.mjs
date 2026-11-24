// L1 本文レーンの回帰（Node。画面を起動せずに、数え方と規則だけを確かめる）。
//
// 見るのは 5 つ。どれも §9.3〜§9.4 の**決定論の部分**で、第 1 の画面実装
// （`REF:src/core/features/metrics.rs` ほか）と同じ答えを出すことを確かめる:
//
// 1. 台詞比率（§9.4.2）: 台詞は鉤括弧の**内側**、地の文は全文字 − 台詞 − 注釈。空白は数えない。
//    サンプル作品の節で `export.text` の文字数と一致することも見る（注釈の落とし方が同じ）。
// 2. 有効モードの表示（§9.4.2 の `'リリア B+C`）。
// 3. 句切れ提案（§9.4.3）: 発火条件・優先度・4 つの除外・上位 3 件・文字を入れないこと。
// 4. 文脈ビューの折りたたみ（§9.3.3）: 単位の鍵・現在の節を含む単位は畳めない・畳んだ単位は 1 行。
// 5. 画面の材料（`editor/text.ts` の文字数）。
//
// TypeScript を Node で読むため、`tsc` で 1 度だけ書き出してから読む（`keys.test.mjs` と同じ）。

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import { IDBFactory, IDBKeyRange } from 'fake-indexeddb';

const WEBUI = new URL('..', import.meta.url).pathname;
const REPO = new URL('../../', import.meta.url).pathname;

/** `webui/src/lib/**.ts` を JS へ書き出して読む（1 ファイル 1 回）。 */
const compiled = new Map();

async function importTs(relative) {
  if (compiled.has(relative)) {
    return compiled.get(relative);
  }
  const out = await mkdtemp(join(tmpdir(), 'novnow-write-'));
  process.on('exit', () => {
    rm(out, { recursive: true, force: true }).catch(() => {});
  });
  execFileSync(
    process.execPath,
    [
      join(WEBUI, 'node_modules', 'typescript', 'bin', 'tsc'),
      join(WEBUI, relative),
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

/** 空白を除いた文字数（§9.6.1）。 */
function chars(text) {
  return [...text].filter((found) => !/\s/u.test(found)).length;
}

// ---------------------------------------------------------------- 1. 台詞比率

test('台詞比率: 台詞は鉤括弧の内側、地の文は注釈を除いた残り（§9.4.2）', async () => {
  const { dialogueRatio } = await importTs('src/lib/editor/ratio.ts');
  // 台詞 4 字（あいうえ）。地の文は鉤括弧 2 字 + かきくけこ 5 字（§9.4.2 の 3 は
  // 「全文字数 − 台詞 − 注釈」なので、鉤括弧は地の文に入る）。空白と注釈は数えない。
  const measured = dialogueRatio('「あいうえ」@リリア\nかきくけこ');
  assert.equal(measured.dialogue, 4);
  assert.equal(measured.ground, 7);
  assert.ok(Math.abs(measured.value - 4 / 11) < 1e-9);

  // 鉤括弧そのものは台詞に数えない（§9.4.2 の 2）
  assert.equal(dialogueRatio('「あ」').dialogue, 1);
  // 『』も台詞（§9.4.2 の 2）
  assert.equal(dialogueRatio('『あ』').dialogue, 1);
  // 空白・改行は両方から除く（§9.4.2 の 4）
  const spaced = dialogueRatio('「あ い」\nう え');
  assert.equal(spaced.dialogue, 2);
  // 鉤括弧 2 字 + 「う」「え」の 2 字
  assert.equal(spaced.ground, 4);
});

test('台詞比率: 入れ子は二重に数えない（引き算が合う）', async () => {
  const { dialogueRatio, dialogueSpans, union } = await importTs('src/lib/editor/ratio.ts');
  // 「『あ』」の内側は「外側の 『あ』」と「内側の あ」。重なりをまとめて 3 字。
  // 二重に数えると台詞 4 字になり、地の文（鉤括弧 2 字）との引き算が合わなくなる。
  const measured = dialogueRatio('「『あ』」');
  assert.equal(dialogueSpans('「『あ』」').length, 2);
  assert.deepEqual(union(dialogueSpans('「『あ』」')), [[1, 4]]);
  assert.equal(measured.dialogue, 3);
  assert.equal(measured.ground, 2);
  // 台詞 + 地の文は、注釈を落として空白を除いた文字数に等しい（§9.4.2 の 3）
  for (const body of ['「『あ』」', '「あ」い', '「あいうえ」\nかきくけこ']) {
    const found = dialogueRatio(body);
    assert.equal(found.dialogue + found.ground, chars(body), body);
    assert.ok(found.value >= 0 && found.value <= 1, body);
  }
});

test('台詞比率: 注釈（見出し・区切り・コメント・引用・話者）は地の文に入らない', async () => {
  const { dialogueRatio, withoutAnnotations } = await importTs('src/lib/editor/ratio.ts');
  const body = [
    '@01-04-02 図書室の攻防',
    '「やあ」@リリア',
    ';',
    '// これは行コメント',
    '# これも行頭コメント',
    '! 挿絵メモ',
    "地の文'アルスと'魔法学院 ^M0001",
    '/* 行内 */ 続き',
  ].join('\n');
  const plain = withoutAnnotations(body);
  // 見出し行・区切り行・コメント行・挿絵メモ行は行ごと落ちる
  assert.ok(!plain.includes('図書室の攻防'));
  assert.ok(!plain.includes(';'));
  assert.ok(!plain.includes('//'));
  assert.ok(!plain.includes('#'));
  assert.ok(!plain.includes('!'));
  // 行末の話者注釈・引用注釈・参照シジルは落ちる（表示名は残る）
  assert.ok(!plain.includes('@リリア'));
  assert.ok(!plain.includes('^M0001'));
  assert.ok(!plain.includes("'"));
  assert.ok(plain.includes('地の文アルスと魔法学院'));
  // 台詞と地の文の合計は、注釈を落とした本文の文字数に等しい
  const measured = dialogueRatio(body);
  assert.equal(measured.dialogue + measured.ground, chars(plain));
});

test('台詞比率: サンプル作品の節で export.text と同じ文字数になる', async () => {
  const { dialogueRatio, withoutAnnotations } = await importTs('src/lib/editor/ratio.ts');
  const wasm = await session();
  try {
    const list = await run(wasm, 'section.list');
    assert.ok(list.sections.length > 0);
    for (const section of list.sections) {
      const got = await run(wasm, 'section.get', { scene: section.id });
      const exported = await run(wasm, 'export.text', { range: section.id });
      // 注釈を落とした本文の文字数は、第 1 の書き出しと同じ（落とし方が同じ）
      assert.equal(
        chars(withoutAnnotations(got.text)),
        chars(exported.text),
        `${section.id}: 注釈を落とした文字数が書き出しと違う`,
      );
      const measured = dialogueRatio(got.text);
      assert.equal(measured.dialogue + measured.ground, chars(exported.text), `${section.id}: 台詞 + 地の文`);
      assert.ok(measured.value >= 0 && measured.value <= 1, `${section.id}: 比率が 0〜1 の外`);
    }
  } finally {
    await wasm.close();
  }
});

// ---------------------------------------------------------------- 2. 有効モード

test('有効モード: 本文の `@名前:モード` を本文の順に `\'名前 モード` で出す（§9.4.2）', async () => {
  const { activeModes, modesLabel, speakersOf } = await importTs('src/lib/editor/modes.ts');
  const body = ['「やあ」@リリア:B+C', "'アルスは頷いた。", '「行こう」@アルス'].join('\n');
  assert.deepEqual(
    speakersOf(body).map((row) => [row.name, row.modes]),
    [
      ['リリア', ['B', 'C']],
      ['アルス', []],
    ],
  );
  const cast = [
    { name: 'リリア', modes: ['A 平常'] },
    { name: 'アルス', modes: ['A 平常'] },
    { name: '学院長', modes: ['A 威厳'] },
  ];
  const active = activeModes(body, cast);
  // 本文に `@` が出ていないキャラ（学院長）は出さない。モードの無い `@アルス` は cast の
  // モードで補う（場面の既定モード）。
  assert.deepEqual(
    active.map((row) => [row.name, row.modes]),
    [
      ['リリア', 'B+C'],
      ['アルス', 'A'],
    ],
  );
  assert.equal(modesLabel(active), "'リリア B+C 'アルス A");
  // 話者注釈になるのは**行末側の最後の `@`** だけ（第 1 の `trailing_speaker` と同じ）。
  // 空白を含む尾巴は注釈にならない（`あ@い う` → `@` の後ろに空白があるので W002 のまま）。
  assert.deepEqual(
    speakersOf('あ@い う@え').map((row) => [row.name, row.modes]),
    [['え', []]],
  );
  assert.deepEqual(speakersOf('あ@い う'), []);
  // 修飾子の形（`B+C`）も読む。形が違えばモード無しとして扱う（名前は残す）
  assert.deepEqual(
    speakersOf('「やあ」@リリア:D').map((row) => [row.name, row.modes]),
    [['リリア', ['D']]],
  );
  assert.deepEqual(
    speakersOf('「やあ」@リリア:B+C+D').map((row) => [row.name, row.modes]),
    [['リリア', []]],
  );
});

// ---------------------------------------------------------------- 3. 句切れ提案

test('句切れ提案: 既定は出さず、閾値を超えた読点の無い文だけに出す（§9.4.3）', async () => {
  const { commaCandidates } = await importTs('src/lib/editor/comma.ts');
  const long = '長い文章は長い文章が長い文章を長い文章に長い文章で長い文章と長い文章は長い文章が長い文章を';
  // 既定の閾値（120 字）では出ない
  assert.deepEqual(commaCandidates(long), []);
  // 閾値を下げると、優先度 1（接続助詞の直後）が上位に来る
  const found = commaCandidates(long, 20);
  assert.ok(found.length > 0 && found.length <= 3, '上位 3 件まで');
  for (const row of found) {
    assert.ok(['が', 'と', 'て', 'ば'].includes(long[row.at - 1]), `候補の直前が接続助詞でない: ${row.at}`);
  }
  // 候補は**手がかりの直後**。第 1 の実装（`REF:src/core/features/metrics.rs`）と同じく、
  // 同じ位置で長く一致する手がかりがあればそちらを採る（`けれども` を `けれど` で切らない）。
  const connect = '彼は長いこと考えたけれども答えは出なかったので窓の外の雨をただ眺めていたのだがそれでも気持ちは晴れずに椅子に座ったまま動かなかった';
  const after = commaCandidates(connect, 20).map((row) => connect.slice(Math.max(0, row.at - 6), row.at));
  assert.ok(after.some((row) => row.endsWith('けれども')), `けれども の直後が候補に無い: ${JSON.stringify(after)}`);
  // 長い文が 2 つ以上あっても、文ごとに候補を出す（節全体で 3 件に絞らない）
  const twice = commaCandidates(`${long}。${long}。`, 20);
  assert.ok(twice.length > 3, '文ごとの上限が節全体の上限になっている');
  // 既に読点がある文は対象外
  assert.deepEqual(commaCandidates(long.replace('長い文章が', '長い文章が、'), 20), []);
  // 短い文は対象外
  assert.deepEqual(commaCandidates('短い文です。', 3), []);
});

test('句切れ提案: 台詞の中・行末 4 字以内・ルビと参照の途中は候補にしない（§9.4.3）', async () => {
  const { commaCandidates, insideReference, insideRuby } = await importTs('src/lib/editor/comma.ts');
  const long = '長い文章は長い文章が長い文章を長い文章に長い文章で長い文章と長い文章は長い文章が長い文章を';
  // 台詞の中だけなら候補なし
  assert.deepEqual(commaCandidates(`「${long}」`, 20), []);
  // 地の文の候補は、行末から 4 字以内を除く
  const withTail = `${long}\nああ`;
  for (const row of commaCandidates(withTail, 20)) {
    const end = withTail.indexOf('\n');
    assert.ok(end < 0 || [...withTail.slice(row.at, end)].length > 4, '行末 4 字以内を候補にした');
  }
  // `'参照` の途中は候補にしない
  assert.equal(insideReference("'アル"), true);
  assert.equal(insideReference("'アル "), false);
  assert.equal(insideReference("\\'アル"), false);
  // ルビの読みの途中は候補にしない
  assert.equal(insideRuby('｜親字《よみ'), true);
  assert.equal(insideRuby('｜親字《よみ》'), false);
});

test('句切れ提案: 文字は入れず、`Alt+,` のときだけ `、` を入れる（§9.4.3）', async () => {
  const { commaCandidates, insertComma, nextCandidate, stepCandidate } = await importTs('src/lib/editor/comma.ts');
  const long = '長い文章は長い文章が長い文章を長い文章に長い文章で長い文章と長い文章は長い文章が長い文章を';
  const found = commaCandidates(long, 20);
  // 候補を数えても本文は変わらない（文字を挿入しない MUST）
  assert.equal(long.includes('、'), false);
  // カーソル以降の直近の候補
  const next = nextCandidate(found, 0);
  assert.equal(next?.at, found[0].at);
  // 候補の間を移る（その方向に無ければ null）
  assert.equal(stepCandidate(found, 0, false), null);
  assert.equal(stepCandidate(found, 0, true)?.at, found[0].at);
  assert.equal(stepCandidate(found, long.length, false)?.at, found[found.length - 1].at);
  // `、` を入れる位置とキャレット（`、` の後ろ）
  const applied = insertComma('あいう', 1);
  assert.equal(applied.text, 'あ、いう');
  assert.equal(applied.caret, 2);
  // 挿入したら、その文は読点を持つので次の候補は出ない
  assert.deepEqual(commaCandidates(applied.text, 20), []);
});

// ---------------------------------------------------------------- 4. 折りたたみ

test('折りたたみ: 単位は章 → 話 → 場面、現在の節を含む単位は畳めない（§9.3.3）', async () => {
  const { keyOf, depthOf, containsScene, foldKey, unfoldKey, foldAll } = await importTs('src/lib/context/fold.ts');
  assert.equal(keyOf('01-04-02-03', 1), '@01');
  assert.equal(keyOf('01-04-02-03', 2), '@01-04');
  assert.equal(keyOf('01-04-02-03', 3), '@01-04-02');
  assert.equal(depthOf('@01-04'), 2);
  assert.equal(depthOf('@nope'), 0);
  assert.equal(containsScene('@01-04', '01-04-02-03'), true);
  assert.equal(containsScene('@01-05', '01-04-02-03'), false);
  // 畳む対象はポインタの載っている節（第 1 の実装と同じ）。現在の節を含む単位は畳めない
  // （§9.3.3 の MUST）。現在の節の上では祖先がどれも現在の節を含むので `null`（何もしない）。
  // 黙って現在の節を隠さない（§21.1.2 の精神）。
  assert.equal(foldKey('01-04-02-03', '01-04-02-03', []), null);
  assert.equal(foldKey('01', '01-04-02-03', []), null);
  // 別の話の節に載っていれば、その節の**場面**（3 階層）から順に畳める。既に畳んである段は飛ばす。
  assert.equal(foldKey('01-03-01-01', '01-04-02-03', []), '@01-03-01');
  assert.equal(foldKey('01-03-01-01', '01-04-02-03', ['@01-03-01']), '@01-03');
  // 章（`@01`）は現在の節（`01-04-...`）を含むので畳めない → 何もしない
  assert.equal(foldKey('01-03-01-01', '01-04-02-03', ['@01-03-01', '@01-03']), null);
  // 展開は「対象の節の、一番内側の畳んである単位」
  assert.equal(unfoldKey('01-04-02-03', ['@01', '@01-04']), '@01');
  assert.equal(unfoldKey('01-04-02-03', ['@01-04']), '@01-04');
  // 対象の節を含まない単位は、展開の対象にしない（現在の節の隣の場面などを開かない）
  assert.equal(unfoldKey('01-04-02-04', ['@01-04-02']), '@01-04-02');
  assert.equal(unfoldKey('01-04-02-03', []), null);
  // すべて畳む（章だけが見える）→ 現在の節の章は畳めないので外れる
  const all = foldAll(['01-04-02-03', '02-01-01-01']);
  assert.deepEqual(all, ['@01', '@02']);
});

test('折りたたみ: 畳んだ単位は 1 行（見出し・題・文字数・節数）で、現在の節の本文は出さない（§9.3.3）', async () => {
  const { rowsAround } = await importTs('src/lib/context/fold.ts');
  const before = [{ scene: '01-03-01-01', text: '前の節' }];
  const after = [{ scene: '01-05-01-01', text: '後の節' }];
  const rows = rowsAround(before, { scene: '01-04-02-03', chars: 412 }, after, []);
  assert.equal(rows.length, 3);
  // 編集中の節は本文を持たない（§9.3.2 MUST）
  const here = rows.find((row) => row.id === '01-04-02-03');
  assert.equal(here.text, '');
  // 畳んだ単位は 1 行にまとまり、節数と文字数を持つ
  const folded = rowsAround(before, { scene: '01-04-02-03', chars: 412 }, after, ['@01-03']);
  const covered = folded.find((row) => row.folded);
  assert.equal(covered.id, '01-03');
  assert.equal(covered.sections, 1);
  assert.equal(covered.chars, 3);
  // 現在の節を含む単位は畳んでも 1 行にしない（本文を出す）
  const guarded = rowsAround(before, { scene: '01-04-02-03', chars: 412 }, after, ['@01']);
  assert.equal(guarded.some((row) => row.folded), false);
});

// ---------------------------------------------------------------- 5. 画面の材料

test('文字数は空白を除いて数える（§9.6.1）', async () => {
  const { charsWithoutSpace } = await importTs('src/lib/editor/text.ts');
  assert.equal(charsWithoutSpace('あ い\nう　え'), 4);
});

test('診断の行を節の中の byte へ直す（§11.3 の下線）', async () => {
  const { lineBytes, sectionByteOf } = await importTs('src/lib/editor/text.ts');
  // 見出し行・本文 3 行（うち 1 行は空）・区切り行
  const text = ['@01-04-02 図書室', '「あ」@リリア', '', '地の文です。', ';', '次の節'].join('\n');
  const byLine = lineBytes(text);
  // 1 始まり。UTF-8 の byte で数える（和文は 1 文字 3 byte）
  assert.equal(byLine.get(2), new TextEncoder().encode('@01-04-02 図書室\n').length);
  // 節の先頭（2 行目）を基準にすると、2 行目は 0
  const base = byLine.get(2);
  assert.equal(sectionByteOf(byLine, base, 2), 0);
  // 4 行目（地の文）は、2 行目 + 空行の分だけ後ろ
  const expected = new TextEncoder().encode('「あ」@リリア\n\n').length;
  assert.equal(sectionByteOf(byLine, base, 4), expected);
  // 節より前の行を指す診断は捨てる（位置のずれた下線を引かない）
  assert.equal(sectionByteOf(byLine, base, 1), null);
  assert.equal(sectionByteOf(byLine, base, 99), null);
});

test('この画面の文言のキーは生成物のカタログに実在する（§12.3）', async () => {
  const catalog = await readFile(join(WEBUI, 'src/lib/i18n/catalog.ts'), 'utf8');
  const keys = new Set([...catalog.matchAll(/^\s*'([a-z0-9_.]+)':/gm)].map((found) => found[1]));
  assert.ok(keys.size > 900, `カタログが小さすぎる（${keys.size} 件）`);
  for (const key of [
    'editor.editing',
    'editor.no_sections',
    'context.guide_workspace',
    'context.guide_section',
    'context.guide_palette',
    'status.chars_unit',
    'stage.dialogue_ratio',
    'stage.outline_sections_unit',
    'delete.button',
    'help.delete_section',
    'api.op.settings.set',
    'api.op.lang.folding',
    'workspace.unavailable',
    'stage.length_target_ratio',
  ]) {
    assert.ok(keys.has(key), `文言 ${key} がカタログに無い`);
  }
});

// ---------------------------------------------------------------- wasm の用意

/** wasm を 1 回だけ読む（`session.test.mjs` と同じやり方）。 */
let module_ = null;

async function wasm() {
  if (!module_) {
    const factory = new IDBFactory();
    globalThis.indexedDB = factory;
    globalThis.IDBKeyRange = IDBKeyRange;
    globalThis.window = globalThis;
    globalThis.self = globalThis;
    module_ = await import('../src/wasm/novnow_wasm.js');
    const bytes = await readFile(new URL('../src/wasm/novnow_wasm_bg.wasm', import.meta.url));
    await module_.default({ module_or_path: bytes });
  }
  return module_;
}

/** サンプル作品を開いたセッションを返す（`close` で閉じる）。 */
async function session() {
  const found = await wasm();
  const inner = new found.NovnowSession();
  await inner.open(null);
  return {
    /** 正規操作 1 回（失敗は例外にする）。 */
    async run(op, args) {
      const raw = JSON.parse(inner.run(op, JSON.stringify(args ?? {})));
      if (!raw.ok) {
        throw new Error(`${op}: ${raw.error?.message ?? 'failed'}`);
      }
      return raw.result;
    },
    close() {
      inner.close?.();
      return Promise.resolve();
    },
  };
}

/** セッションで正規操作を 1 回。 */
function run(wasmSession, op, args) {
  return wasmSession.run(op, args);
}
