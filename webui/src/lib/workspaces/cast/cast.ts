// `cast` ワークスペースの補助（§10.6）。
//
// 画面は正規操作の戻り値だけを材料にし、`characters.md` の構造解析を**自前でしない**
// （§11.11.5 の「独自のドーン規則を持たない」）。守る 2 つの規則:
//
// 1. **行の並び順は `reading` 順**（§10.6。`cast.list` の返り値の `reading`）。
// 2. **対角線（自分自身）は常に `—`**（§10.6.1 MUST。一人称は別概念）。

/** 対角線（自分自身）の表示。MUST: 常に `—`。 */
export const DIAGONAL = '—';

/** 空セル（`address:` 未定義）の表示。`—` は対角線と区別が付かないため別の形。 */
export const EMPTY_CELL = '';

/**
 * 行を `reading` 順に並べる（§10.6）。
 *
 * `reading` が空の行は末尾へ回し、同点は `name` の昇順で安定させる
 * （決定論。同じ入力は常に同じ並び）。
 */
export function sortByReading(
  rows: { name: string; reading: string }[],
): { name: string; reading: string }[] {
  return rows
    .map((row) => ({ ...row, reading: row.reading ?? '' }))
    .sort((left, right) =>
      left.reading.localeCompare(right.reading, 'ja') ||
      left.name.localeCompare(right.name),
    );
}

/**
 * 行 × 列の表の形（§10.6.1）。
 *
 * セルの内容は代表呼称 1 つ。複数定義は件数を添える（`めぐみ +1`）。
 * データは `cast.timeline` の戻り値（`modes_list`）から集める。
 */
export interface AddressCell {
  /** 代表呼称（空なら未定義）。 */
  value: string;
  /** モード別定義の件数（0 ならモード別定義無し）。 */
  modeCount: number;
}

/** 集めたセルを行ごとにまとめる（`reading` 順に並べた `names` の順で）。 */
export function buildMatrix(
  names: { name: string; reading: string }[],
  cells: Map<string, Map<string, AddressCell>>,
): AddressCell[][] {
  const rows = sortByReading(names);
  return rows.map((row) =>
    rows.map((column) => {
      if (row.name === column.name) {
        // 対角線は常に `—`（MUST）
        return { value: DIAGONAL, modeCount: 0 };
      }
      return cells.get(row.name)?.get(column.name) ?? { value: EMPTY_CELL, modeCount: 0 };
    }),
  );
}

/** セルの表示（件数があれば `+N` を添える）。 */
export function cellText(cell: AddressCell): string {
  if (cell.modeCount > 0) {
    return cell.value ? `${cell.value} +${cell.modeCount}` : `+${cell.modeCount}`;
  }
  return cell.value;
}
