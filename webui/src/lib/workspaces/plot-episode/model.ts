// 話プロット（`plot-episode`。`REF:docs/specifications/ui.md` §10.2）の計算。
//
// 画面は**操作の戻り値だけ**から節の一覧と台詞比率を作る（`REF:docs/plans.md#webui-parallel`
// §11.11.5 と `REF:docs/designs/webui.md` §21）。ここに置くのは「どの節を並べるか」「比率を
// どう数えるか」の 2 つだけで、ファイル形式（B 形式の場面見出し・`;` 区切り）は解釈しない。

import { isEpisodeId } from '../plot-outline/blocks';

/** 節 1 つ（`section.list` の要素。`REF:src/app/ops/editing.rs` の `section_list`）。 */
export interface SectionItem {
  /** 4 階層 ID（`01-04-02-03`）。 */
  id: string;
  /** 本文の文書のパス。 */
  path: string;
  /** 本文内の行の範囲（0 始まり）。 */
  lines: [number, number];
  /** 場面の題（`section.list` の `summary`）。 */
  summary: string;
}

/** 値が節の形をしているか。 */
export function isSection(value: unknown): value is SectionItem {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const found = value as Record<string, unknown>;
  const lines = found.lines;
  return (
    typeof found.id === 'string' &&
    typeof found.path === 'string' &&
    typeof found.summary === 'string' &&
    Array.isArray(lines) &&
    lines.length === 2 &&
    typeof lines[0] === 'number' &&
    typeof lines[1] === 'number'
  );
}

/** `section.list` の戻り値（`{ count, sections }`）から節を取り出す。 */
export function sectionsOf(result: unknown): SectionItem[] {
  if (typeof result !== 'object' || result === null) {
    return [];
  }
  const list = (result as { sections?: unknown }).sections;
  return Array.isArray(list) ? list.filter(isSection) : [];
}

/** 節の ID の話の部分（`01-04-02-03` → `01-04`）。4 階層でなければ `null`。 */
export function episodeOf(sectionId: string): string | null {
  const parts = sectionId.split('-');
  if (parts.length < 2) {
    return null;
  }
  const episode = `${parts[0]}-${parts[1]}`;
  return isEpisodeId(episode) ? episode : null;
}

/** 節の ID の場面の部分（`01-04-02-03` → `01-04-02`）。 */
export function sceneOf(sectionId: string): string | null {
  const parts = sectionId.split('-');
  if (parts.length < 3) {
    return null;
  }
  return parts.slice(0, 3).join('-');
}

/** 話に属する節を ID 順に並べる（一覧は話の中だけを見る。§10.2）。 */
export function sectionsInEpisode(all: readonly SectionItem[], episode: string): SectionItem[] {
  return all
    .filter((item) => episodeOf(item.id) === episode)
    .slice()
    .sort((left, right) => (left.id < right.id ? -1 : left.id > right.id ? 1 : 0));
}

/** 選んでいた節が一覧から消えたとき、同じ場面の先頭か、先頭の節を返す。 */
export function keepSelection(rows: readonly SectionItem[], selected: string | null): string | null {
  if (rows.length === 0) {
    return null;
  }
  if (selected !== null && rows.some((row) => row.id === selected)) {
    return selected;
  }
  if (selected !== null) {
    const scene = sceneOf(selected);
    const sameScene = rows.find((row) => sceneOf(row.id) === scene);
    if (sameScene) {
      return sameScene.id;
    }
  }
  return rows[0]?.id ?? null;
}

/**
 * 台詞比率（§9.4.2 の算法）。
 *
 * - 台詞文字数 = `「…」` と `『…』` の**内側**の文字数の合計（鉤括弧は数えない）。
 * - 地の文字数 = 全文字数 − 台詞文字数 − アノテーション（空白・改行は両方から除く）。
 * - 比率 = 台詞 ÷（台詞 + 地の文）。分母が 0 のときは `null`（バーを描かない）。
 *
 * **既知の差**: アノテーションの除外（§5.0.7 の 9 種）は字句規則の全体を写していない。
 * ここで除くのは `//`・`/* … *\/`・行頭の `#`・行頭の `!`・行末の `^M…` だけである。
 * 正確な値は核の算法で出す操作が要る（`REF:docs/plans.md#webui-w4` の残件）。
 */
export function dialogueRatio(text: string): { dialogue: number; narration: number; ratio: number | null } {
  const bare = stripAnnotations(text);
  let dialogue = 0;
  for (const match of bare.matchAll(/「([^」]*)」|『([^』]*)』/gu)) {
    dialogue += countChars(match[1] ?? match[2] ?? '');
  }
  // 地の文 = 全文字 − 台詞の中身 − 鉤括弧そのもの（§9.4.2 の 2: 鉤括弧は数えない）
  const brackets = (bare.match(/[「」『』]/gu) ?? []).length;
  const total = countChars(bare);
  const narration = Math.max(0, total - dialogue - brackets);
  const sum = dialogue + narration;
  return { dialogue, narration, ratio: sum === 0 ? null : dialogue / sum };
}

/** 空白を除いた文字数（コードポイント単位）。 */
export function countChars(text: string): number {
  let found = 0;
  for (const ch of text) {
    if (!/\s/u.test(ch)) {
      found += 1;
    }
  }
  return found;
}

/** 算出の対象から外すアノテーションの近似（§5.0.7 の部分集合。上の注を見る）。 */
export function stripAnnotations(text: string): string {
  return text
    .replace(/\/\*[\s\S]*?\*\//gu, '')
    .split('\n')
    .map((line) => line.replace(/\/\/.*$/u, '').replace(/\^M\d+/gu, ''))
    .filter((line) => !/^\s*[#!]/u.test(line))
    .join('\n');
}

/** 台詞比率の表示（百分率の整数。`null` は表示しない）。 */
export function percent(ratio: number | null): string {
  return ratio === null ? '' : `${Math.round(ratio * 100)}%`;
}

/** 話プロットのキー（§9.8 のうち、この画面で受けるもの）。写しは構成と同じ方式（`outlineKey`）。 */
export type EpisodeKey = 'new-section' | 'to-write' | 'up' | 'down' | 'mode' | 'material' | 'swap-up' | 'swap-down';

export function episodeKey(stroke: { key: string; code: string; ctrl: boolean; shift: boolean; alt: boolean }): EpisodeKey | null {
  if (stroke.ctrl && stroke.shift && !stroke.alt) {
    if (stroke.code === 'KeyN') {
      return 'new-section';
    }
    return null;
  }
  if (stroke.alt && stroke.shift && !stroke.ctrl) {
    if (stroke.key === 'ArrowUp') {
      return 'swap-up';
    }
    if (stroke.key === 'ArrowDown') {
      return 'swap-down';
    }
    return null;
  }
  if (stroke.ctrl && !stroke.shift && !stroke.alt) {
    if (stroke.key === 'Enter') {
      return 'to-write';
    }
    if (stroke.code === 'KeyD') {
      return 'mode';
    }
    if (stroke.code === 'KeyM') {
      return 'material';
    }
    return null;
  }
  if (!stroke.ctrl && !stroke.shift && !stroke.alt) {
    if (stroke.key === 'ArrowUp') {
      return 'up';
    }
    if (stroke.key === 'ArrowDown') {
      return 'down';
    }
  }
  return null;
}
