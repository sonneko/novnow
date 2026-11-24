// 位置の計算（§3.4 の位置表現。**UTF-16 のコード単位**で数える）。
//
// 画面の位置（キャレット・選択範囲）は UTF-16 のコード単位で数える。Rust 側は**バイト**で
// 数えるので、`lang.*` / `nav.jump` へ渡すときだけ [`byteOffset`] で直す。逆に、診断の
// 範囲（バイト）を画面へ出すときは [`charIndex`] で戻す。
//
// JS の `String.prototype.slice` も `selectionStart` も UTF-16 で数えるので、画面の側は
// そのまま扱える。**サロゲートペアを 2 と数える**ことは §3.4 の決まりどおり。

/** 1 行（行番号と中身）。 */
export interface Line {
  /** 1 始まりの行番号（節内のローカル行番号。§9.4.1）。 */
  number: number;
  /** 行の中身（改行を含まない）。 */
  text: string;
  /** 本文の先頭からの位置（UTF-16。行の先頭）。 */
  start: number;
}

/** 本文を行に分ける（`\n` だけが区切り。§5.0.4 の内部表現は LF）。 */
export function linesOf(text: string): Line[] {
  const lines: Line[] = [];
  let start = 0;
  let number = 1;
  for (;;) {
    const found = text.indexOf('\n', start);
    if (found < 0) {
      lines.push({ number, text: text.slice(start), start });
      return lines;
    }
    lines.push({ number, text: text.slice(start, found), start });
    start = found + 1;
    number += 1;
  }
}

/** 位置が何行目か（0 始まり。範囲外は末尾）。 */
export function lineIndexAt(lines: readonly Line[], offset: number): number {
  let low = 0;
  let high = lines.length - 1;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    const found = lines[middle];
    if (found && found.start <= offset) {
      low = middle;
    } else {
      high = middle - 1;
    }
  }
  return low;
}

/** 位置の行と列（列は行の先頭からの UTF-16 の数）。 */
export function lineColAt(
  lines: readonly Line[],
  offset: number,
): { line: number; column: number } {
  const index = lineIndexAt(lines, offset);
  const found = lines[index];
  return { line: index, column: Math.max(0, offset - (found?.start ?? 0)) };
}

/**
 * UTF-16 の位置を UTF-8 のバイト数へ直す（Rust 側へ渡すとき）。
 *
 * サロゲートペアは 4 バイト。孤立したサロゲートは 3 バイト（`TextEncoder` と同じ数え方）。
 */
export function byteOffset(text: string, offset: number): number {
  return new TextEncoder().encode(text.slice(0, offset)).length;
}

/** UTF-8 のバイト数を UTF-16 の位置へ戻す（診断の範囲を画面へ出すとき）。 */
export function charIndex(text: string, bytes: number): number {
  let seen = 0;
  let index = 0;
  for (const found of text) {
    if (seen >= bytes) {
      return index;
    }
    seen += new TextEncoder().encode(found).length;
    index += found.length;
  }
  return text.length;
}

/** 文字数を数える（空白を除く。§9.6.1 の「文字数」）。 */
export function charsWithoutSpace(text: string): number {
  // 空白は `\s`（半角・全角の空白と改行）を除く。数えるのは「コードポイント」ではなく
  // 「利用者が見る 1 文字」なので、サロゲートペアは 1 と数える（`for…of` はコードポイント単位）。
  let count = 0;
  for (const found of text) {
    if (!/\s/u.test(found)) {
      count += 1;
    }
  }
  return count;
}

/** 3 桁区切り（`128400` → `128,400`。§9.6.1 の表示）。 */
export function grouped(count: number): string {
  return count.toLocaleString('en-US');
}

/**
 * 文書の本文を行ごとの byte 位置に分ける（`diagnostics.run` の `line` を節の中の位置へ直すため。
 * §11.3）。
 *
 * `line` は 1 始まりなので、返す `Map` の鍵も 1 始まり。`text` が空でも 1 行目（0 byte）を返す。
 */
export function lineBytes(text: string): Map<number, number> {
  const encoder = new TextEncoder();
  const out = new Map<number, number>();
  let byte = 0;
  const rows = text.split('\n');
  for (let index = 0; index < rows.length; index += 1) {
    out.set(index + 1, byte);
    byte += encoder.encode(rows[index] ?? '').length + 1;
  }
  return out;
}

/**
 * 診断の行（1 始まり・文書の中）を、節の中の byte 位置へ直す。
 *
 * `base` は節の先頭（文書の中の byte）、`byLine` は [`lineBytes`] の戻り値。
 * 節の外（前の行）を指す診断は捨てる（`null`）。位置のずれた下線を引かない。
 */
export function sectionByteOf(byLine: Map<number, number>, base: number, line: number): number | null {
  const at = byLine.get(line);
  if (at === undefined) {
    return null;
  }
  const local = at - base;
  return local < 0 ? null : local;
}
