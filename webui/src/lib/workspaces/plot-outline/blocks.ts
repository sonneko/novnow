// A 形式（全体・章プロット。`REF:docs/specifications/syntax.md` §5.3）のブロックを、
// 正規操作 `plot.get` の戻り値から読む材料。話プロット（`plot-episode`）の上段も同じ型を使う
// ので、このレーン（L2）の中で共有する（他のレーンのディレクトリは import しない）。
//
// 画面は**ファイル形式を解釈しない**（`REF:docs/designs/webui.md` §21 と §21.1.3）。
// ここで扱うのは操作が返した構造（ID・題・要約・状態・行の範囲）だけで、行の範囲に従って
// 文書の一部を置き換える処理は `plot.write` に全文を渡す直前の 1 回だけ使う。

/** A 形式のブロック 1 つ（`plot.get` の `blocks` の要素。`REF:src/app/ops/editing.rs` の `plot_get`）。 */
export interface Block {
  /** 構造 ID（全体・章のブロックは `01`、話のブロックは `01-04`）。 */
  id: string;
  /** 文書のパス（`plots/all.cnov` / `plots/01.cnov`）。 */
  path: string;
  /** 文書内の行の範囲（0 始まり。終わりは含まない。見出しから `;` まで）。 */
  lines: [number, number];
  /** ブロックの閉じ位置（次のブロックの前まで。0 始まり）。 */
  end: number;
  /** 見出しの題（`@01-04 図書室の密談` の「図書室の密談」）。 */
  title: string;
  /** `summary:` の 1 行目。無ければ空。 */
  summary: string;
  /** `state:` の値（`draft` / `review` / `fixed`）。書かれていなければ `null`。 */
  state: string | null;
}

/** 値が A 形式のブロックの形をしているか（壊れた値は読まない）。 */
export function isBlock(value: unknown): value is Block {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const found = value as Record<string, unknown>;
  const lines = found.lines;
  return (
    typeof found.id === 'string' &&
    typeof found.path === 'string' &&
    Array.isArray(lines) &&
    lines.length === 2 &&
    typeof lines[0] === 'number' &&
    typeof lines[1] === 'number' &&
    typeof found.end === 'number' &&
    typeof found.title === 'string' &&
    typeof found.summary === 'string' &&
    (found.state === null || typeof found.state === 'string')
  );
}

/** `plot.get` の戻り値（`{ blocks, level }`）からブロックを取り出す。形が違えば空。 */
export function blocksOf(result: unknown): Block[] {
  if (typeof result !== 'object' || result === null) {
    return [];
  }
  const list = (result as { blocks?: unknown }).blocks;
  return Array.isArray(list) ? list.filter(isBlock) : [];
}

/** 章番号（`01` → 1、`00` → 0）。数字で始まらない ID は `null`。 */
export function chapterNumber(id: string): number | null {
  const found = /^(\d{1,3})(?:-\d{1,3})?$/u.exec(id);
  return found?.[1] === undefined ? null : Number(found[1]);
}

/** 話の ID（`01-04`）の章番号。ブロックの ID が章の形（`01`）でも使える。 */
export function chapterOfId(id: string): number | null {
  return chapterNumber(id);
}

/** 話のブロックの ID か（`01-04`。章だけの `01` は false）。 */
export function isEpisodeId(id: string): boolean {
  return /^\d{1,3}-\d{1,3}$/u.test(id);
}

/**
 * 文書の行の範囲だけを `replacement` に置き換えた全文を返す（`lines` は 0 始まり・終わり除く）。
 *
 * 範囲が文書の外なら、範囲を文書の中へ丸める（壊れた範囲で他の行を消さないため）。
 * 改行は `\n` で扱う（核が読んだ文書と同じ区切り。`REF:src/core/features/project.rs` の行分割）。
 */
export function replaceLines(
  text: string,
  lines: readonly [number, number],
  replacement: string,
): string {
  const rows = text.split('\n');
  const start = Math.max(0, Math.min(lines[0], rows.length));
  const end = Math.max(start, Math.min(lines[1], rows.length));
  return [...rows.slice(0, start), ...replacement.split('\n'), ...rows.slice(end)].join('\n');
}

/** 行の範囲の本文（`replaceLines` と同じ丸め方）。 */
export function linesOf(text: string, lines: readonly [number, number]): string {
  const rows = text.split('\n');
  const start = Math.max(0, Math.min(lines[0], rows.length));
  const end = Math.max(start, Math.min(lines[1], rows.length));
  return rows.slice(start, end).join('\n');
}
