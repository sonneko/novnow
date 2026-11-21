// 構成（`plot-outline`。`REF:docs/specifications/ui.md` §10.3）の計算。
//
// 材料は `plot.get`（全体プロット＝章の一覧、章プロット＝話の一覧）と `plot.outline_fold`
// （折り畳みの状態）だけ。文字数・作中日時・伏線の突き合わせは、ブリッジの操作に無いので
// ここでは作らない（画面は「この版ではまだ使えません」を出す。§21.1.2）。

import { chapterNumber, type Block } from './blocks';

/** 章 1 つ（ツリーの 1 行。`plot.get` の全体プロットのブロックと、その章の話）。 */
export interface Chapter {
  /** 章番号（`00` は未分類ストック）。 */
  number: number;
  /** 章のブロック（全体プロットの `@01`）。全体プロットに無い章は `null`。 */
  block: Block | null;
  /** 話のブロック（章プロットの `@01-04`）。ID の順。 */
  episodes: Block[];
}

/** 表示の対象（全体、または 1 つの章）。§10.3 の「左のレベル切替」。 */
export type Level = { kind: 'all' } | { kind: 'chapter'; chapter: number };

/** 右のタブ（§10.3 の表の順）。 */
export const TABS = [
  'base',
  'timeline',
  'matrix',
  'illustrations',
  'senses',
  'space',
  'blurb',
  'histogram',
  'stock',
] as const;

export type Tab = (typeof TABS)[number];

/**
 * タブの状態。`ready` は操作だけで出せる。`partial` は一部だけ出せる（残りは未提供）。
 * `unavailable` は操作が無く、中身を出せない。
 */
export function tabState(tab: Tab): 'ready' | 'partial' | 'unavailable' {
  switch (tab) {
    case 'base':
      return 'ready';
    case 'stock':
      return 'partial';
    default:
      return 'unavailable';
  }
}

/** 次のタブ（`Ctrl+Shift+→`）と前のタブ（`Ctrl+Shift+←`）。端では循環する。 */
export function moveTab(tab: Tab, step: 1 | -1): Tab {
  const at = TABS.indexOf(tab);
  const next = (at + step + TABS.length) % TABS.length;
  return TABS[next] ?? 'base';
}

/**
 * 全体プロットのブロックと、章ごとの話のブロックから、ツリーを作る。
 *
 * - 章は番号の昇順。ただしストック章（`00`）は番号順では先頭だが、**ツリーでは常に最後**
 *   （§10.3.1 MUST）。
 * - 話は ID の昇順。
 * - 章のブロックが無い章（章プロットだけがある）も、番号が分かれば一覧に出す。
 */
export function buildTree(allBlocks: readonly Block[], episodes: ReadonlyMap<number, readonly Block[]>): Chapter[] {
  const byNumber = new Map<number, Chapter>();
  for (const block of allBlocks) {
    const number = chapterNumber(block.id);
    if (number === null || block.id.includes('-')) {
      continue;
    }
    byNumber.set(number, { number, block, episodes: [] });
  }
  for (const [number, list] of episodes) {
    const chapter = byNumber.get(number) ?? { number, block: null, episodes: [] };
    chapter.episodes = list
      .filter((block) => block.id.includes('-'))
      .slice()
      .sort((left, right) => (left.id < right.id ? -1 : left.id > right.id ? 1 : 0));
    byNumber.set(number, chapter);
  }
  return [...byNumber.values()].sort((left, right) => rank(left.number) - rank(right.number));
}

/** ツリーの並びの順位（ストック `00` は最後）。 */
function rank(number: number): number {
  return number === 0 ? Number.MAX_SAFE_INTEGER : number;
}

/** 章番号の表示（`01`）。 */
export function chapterLabel(number: number): string {
  return String(number).padStart(2, '0');
}

/** 章のブロックの ID が指す章番号（`01-04` の `01`）。 */
export function chapterOf(block: Block): number | null {
  return chapterNumber(block.id);
}

/** 表示する章の既定（レベルが「全体」のとき、選んでいるブロックの章。無ければ先頭の章）。 */
export function defaultChapter(tree: readonly Chapter[], selected: Block | null): number | null {
  if (selected !== null) {
    const number = chapterOf(selected);
    if (number !== null) {
      return number;
    }
  }
  return tree.find((chapter) => chapter.number !== 0)?.number ?? tree[0]?.number ?? null;
}

/** 起動直後は全章を畳む（§10.3.1 の MUST）。畳んでいる章の集合を返す。 */
export function allFolded(tree: readonly Chapter[]): number[] {
  return tree.map((chapter) => chapter.number);
}

/** 章を畳むか開くかを、集合に反映した新しい集合を返す。 */
export function toggleFold(folded: readonly number[], chapter: number, fold: boolean): number[] {
  const rest = folded.filter((number) => number !== chapter);
  return fold ? [...rest, chapter].sort((left, right) => left - right) : rest;
}

/** 表示の対象のブロック（レベルで決まる）。全体は章のブロック、章は話のブロック。 */
export function blocksAt(tree: readonly Chapter[], level: Level): Block[] {
  if (level.kind === 'all') {
    return tree.flatMap((chapter) => (chapter.block === null ? [] : [chapter.block]));
  }
  const chapter = tree.find((found) => found.number === level.chapter);
  return chapter ? chapter.episodes : [];
}

/** 章のストック（`00`）か。 */
export function isStock(number: number): boolean {
  return number === 0;
}

/**
 * 話・構成のキー（§9.8 のうち、この 2 画面で受けるもの）を、操作の名前へ写す。
 *
 * `keys.ts`（L0 所有）の表に無い組（`Ctrl+Shift+N` など）はここで受ける。
 * 記号は `event.key` が配列で変わるので、`event.code` の物理キーで決める（`[` と `]`）。
 * 扱わないキーは `null` を返し、親（シェル）へ流す。
 */
export type OutlineKey =
  | 'tab-next'
  | 'tab-previous'
  | 'up'
  | 'down'
  | 'fold'
  | 'unfold'
  | 'new-section'
  | 'down-level'
  | 'up-level';

export function outlineKey(stroke: { key: string; code: string; ctrl: boolean; shift: boolean; alt: boolean }): OutlineKey | null {
  if (stroke.alt) {
    return null;
  }
  if (stroke.ctrl && stroke.shift) {
    switch (stroke.code) {
      case 'ArrowRight':
        return 'tab-next';
      case 'ArrowLeft':
        return 'tab-previous';
      case 'BracketLeft':
        return 'fold';
      case 'BracketRight':
        return 'unfold';
      case 'KeyN':
        return 'new-section';
      case 'KeyB':
        return 'up-level';
      default:
        break;
    }
    switch (stroke.key) {
      case 'ArrowRight':
        return 'tab-next';
      case 'ArrowLeft':
        return 'tab-previous';
      default:
        return null;
    }
  }
  if (stroke.ctrl && !stroke.shift && stroke.key === 'Enter') {
    return 'down-level';
  }
  if (!stroke.ctrl && !stroke.shift) {
    if (stroke.key === 'ArrowUp') {
      return 'up';
    }
    if (stroke.key === 'ArrowDown') {
      return 'down';
    }
  }
  return null;
}
