// `reader` ワークスペースの縦書き組版（§17.3.3）。
//
// 正本は `REF:src/core/reader.rs` の純粋な組版。字送りは**文字 1 つ = 1 マス**の簡易版
// （`REF:src/core/reader.rs` の `layout_cells` と同じ）。実際のフォントの字送りで組む
// ときは `Metrics` を渡すが、画面は測定できないため固定仮定で決定論的に算出する
// （§17.3.3: グリフ幅を固定仮定せず、各文字の座標を決定論的に算出する）。
//
// 収集するルール（すべて決定論・同じ入力から同じ結果）:
//
// 1. **行頭禁則** `、。，．）］｝〉》」』】！？ゝゞ々ー` と小書き仮名
// 2. **行末禁則** `（［｛〈《「『【`
// 3. **ぶら下げはしない**（禁則処理で追い出す）
// 4. **縦中横**: 半角数字 2 桁は 1 単位。3 桁以上は 1 字ずつ縦置き
// 5. **回転**: ラテン文字の連続（3 文字以上）は 90° 回転
// 6. **句読点**: 全角の右上寄せ（仮想ボディの右上 1/4）
// 7. **長音・ダッシュ** `ー` `―` `…` は縦向きグリフ（簡易版では回転）
// 8. **ルビ**: 親文字に添えて残す（§17.3.5 の「読者の目に入るもの」に含む）

/** 行頭禁則（§17.3.3）。 */
export const NO_START = '、。，．）］｝〉》」』】！？ゝゞ々ーぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮ';

/** 行末禁則（§17.3.3）。 */
export const NO_END = '（［｛〈《「『【';

/** 句読点（右上寄せ）。 */
export const PUNCTUATION = '、。';

/** 縦向きグリフ（簡易版では回転）。 */
export const VERTICAL_ROTATE = 'ー―…';

/** 1 文字の単位（組版の最小要素）。 */
export interface Cell {
  /** 元の文字（1 文字。回転する文字列は `text`）。 */
  ch: string;
  /** 描画する文字列（縦中横の 2 桁など）。 */
  text: string;
  /** ルビ（親文字に添える。空ならなし）。 */
  ruby: string;
  /** 90° 回転して描く。 */
  rotate: boolean;
  /** 縦中横（2 桁の数字。1 字幅）。 */
  compact: boolean;
  /** 句読点（右上寄せ）。 */
  punctuation: boolean;
}

/** 1 行の幅（列数）・1 ページの行数。 */
export interface PageOptions {
  /** 1 行の字数（§17.3.2 の「列数」）。 */
  columns: number;
  /** 1 ページの行数（§17.3.2 の「行数」）。 */
  rows: number;
}

/** 1 ページ。行は右から左へ、行内は上から下へ進む（§17.3.2）。 */
export interface Page {
  /** 行ごとの文字の並び（行 0 = 最右）。 */
  lines: Cell[][];
}

/**
 * テキストを縦書きの単位に分ける（§17.3.3）。
 *
 * ルビ（`《読み》親文字`）は `《…》` を描かず、読みを親文字に添えて残す
 * （`REF:src/core/reader.rs` の `units` と同じ。親文字 1 つに読みを均等に割り振る）。
 * 半角数字は 2 桁を 1 単位の縦中横に、3 桁以上は 1 字ずつ縦置きする（§17.3.3）。
 */
export function units(text: string): Cell[] {
  const out: Cell[] = [];
  // ルビの範囲（`《…》` + 直後の親文字）を先に拾う。
  const rubies: { start: number; end: number; reading: string; baseStart: number }[] = [];
  const re = /《([^》]*)》/gu;
  let found = re.exec(text);
  while (found) {
    const close = found.index + found[0].length;
    const base = [...text.slice(close)][0] ?? '';
    rubies.push({
      start: found.index,
      end: close + base.length,
      reading: found[1] ?? '',
      baseStart: close,
    });
    found = re.exec(text);
  }
  let cursor = 0;
  for (const ruby of rubies) {
    // ルビより前の素の文字
    out.push(...plainCells(text.slice(cursor, ruby.start)));
    // 親文字（読みを添える）
    const base = [...text.slice(ruby.baseStart, ruby.end)];
    const reading = [...ruby.reading];
    base.forEach((ch, at) => {
      const lo = Math.floor((at * reading.length) / Math.max(1, base.length));
      const hi = Math.floor(((at + 1) * reading.length) / Math.max(1, base.length));
      out.push({ ...cellOf(ch), ruby: reading.slice(lo, hi).join('') });
    });
    cursor = ruby.end;
  }
  out.push(...plainCells(text.slice(cursor)));
  return out;
}

/** 素の文字列を単位に分ける（半角数字 2 桁は縦中横の 1 単位）。 */
function plainCells(text: string): Cell[] {
  const out: Cell[] = [];
  let index = 0;
  const chars = [...text];
  while (index < chars.length) {
    const ch = chars[index] ?? '';
    if (/[0-9]/.test(ch)) {
      // 連続する数字を拾う
      let run = '';
      while (index < chars.length && /[0-9]/.test(chars[index] ?? '')) {
        run += chars[index] ?? '';
        index += 1;
      }
      if (run.length === 2) {
        // 2 桁は縦中横（1 単位）
        out.push({ ...cellOf(run[0] ?? ''), text: run, compact: true, rotate: false });
      } else {
        // 1 桁・3 桁以上は 1 字ずつ縦置き
        for (const digit of run) {
          out.push({ ...cellOf(digit), compact: true });
        }
      }
      continue;
    }
    out.push(cellOf(ch));
    index += 1;
  }
  return out;
}

/** 1 文字の単位を作る（回転・句読点の判定）。 */
function cellOf(ch: string): Cell {
  const latin = /[A-Za-z]/.test(ch);
  // ラテン文字 3 文字以上は回転、長音・ダッシュも回転（§17.3.3）
  return {
    ch,
    text: ch,
    ruby: '',
    rotate: latin || VERTICAL_ROTATE.includes(ch),
    compact: false,
    punctuation: PUNCTUATION.includes(ch),
  };
}

/**
 * テキストを縦書きのページへ組む（§17.3.3）。
 *
 * 禁則処理: 行頭禁則の文字は次の行へ追い出し、行末禁則の文字は前の行へ戻す
 * （ぶら下げはしない）。決定論的で同じ入力から同じページになる。
 */
export function layout(text: string, options: PageOptions): Page[] {
  const columns = Math.max(1, Math.min(100, options.columns));
  const rows = Math.max(1, Math.min(100, options.rows));
  const unitsList = units(text);
  // 行のリスト。`here` は現在の行への参照（`noUncheckedIndexedAccess` に考慮）。
  const lines: Cell[][] = [[]];
  let here: Cell[] = lines[0] ?? [];
  for (const unit of unitsList) {
    if (unit.ch === '\r') {
      continue;
    }
    if (unit.ch === '\n') {
      lines.push([]);
      here = lines[lines.length - 1] ?? [];
      continue;
    }
    if (here.length >= columns) {
      const moved: Cell[] = [];
      if (NO_START.includes(unit.ch) && columns > 1) {
        const last = here.pop();
        if (last) {
          moved.push(last);
        }
      }
      while (here.length > 0 && NO_END.includes(here[here.length - 1]?.ch ?? '')) {
        const popped = here.pop();
        if (popped) {
          moved.push(popped);
        }
      }
      while (
        moved.length > 0 &&
        NO_START.includes(moved[moved.length - 1]?.ch ?? '') &&
        here.length > 0
      ) {
        const popped = here.pop();
        if (popped) {
          moved.push(popped);
        }
      }
      moved.reverse();
      lines.push(moved);
      here = moved;
    }
    here.push(unit);
  }
  // 行をページへ分ける（行数 `rows` ごと）。
  const pages: Page[] = [];
  let page: Page = { lines: [] as Cell[][] };
  let count = 0;
  for (const line of lines) {
    if (count > 0 && count % rows === 0) {
      pages.push(page);
      page = { lines: [] as Cell[][] };
    }
    page.lines.push(line);
    count += 1;
  }
  pages.push(page);
  // 余白の空ページは落とす
  while (pages.length > 1) {
    const last = pages[pages.length - 1];
    if (last && last.lines.length === 1 && (last.lines[0]?.length ?? 0) === 0) {
      pages.pop();
    } else {
      break;
    }
  }
  return pages;
}
