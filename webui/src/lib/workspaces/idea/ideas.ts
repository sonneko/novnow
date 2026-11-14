// `idea` ワークスペースの規則（§10.4・§5.7）。描画に依らない部分だけを置く。
//
// このファイルは**実行時の import を持たない**（テストが単独で読めるように）。キーの写像は
// 画面が `keys.ts` の `chord(strokeOf(event))` で作った文字列を `ideaAction` に渡す。
//
// 第 1 の `REF:src/ui/workspaces/idea.rs` と同じ規則:
//
// - 既定の絞り込みは `dropped` 以外（§5.7「dropped は既定で非表示」）。
// - 捨てたあとは、見えている次のアイデア（無ければ最後）を選ぶ。
// - 昇格先は「プロットの節 / 用語 / キャラのメモ」。資料は正規操作 `idea.promote` が受けない（後述）。

/** アイデアの `status:`（§5.7）。 */
export type IdeaStatus = 'inbox' | 'keep' | 'promoted' | 'dropped';

/** `ideas.md` の 1 件（`idea.list` の要素を解いたもの）。 */
export interface IdeaRow {
  /** 書かれたままの ID（`I0007`）。 */
  id: string;
  /** 並びの番号（`ideas.md` の何番目か。0 始まり）。 */
  index: number;
  /** タイトル。 */
  title: string;
  /** 状態。 */
  status: IdeaStatus;
  /** タグ。 */
  tags: string[];
  /** 見出しの行（0 始まり）から末尾の空行を除いた終わり（終端は含まない）。 */
  lines: [number, number];
}

/** `ideas.md` の正規の文書パス（`REF:src/core/features/ideas.rs` の `PATH`）。 */
export const IDEAS_PATH = 'ideas.md';

/** 一覧の 1 行の高さ（px。仮想化の計算に使う。§10.4.1）。 */
export const ROW_HEIGHT = 28;

const STATUSES: readonly IdeaStatus[] = ['inbox', 'keep', 'promoted', 'dropped'];

/** 状態の読み（大文字小文字は問わない。表に無い値は `inbox`。§5.7 の既定）。 */
export function statusOf(raw: unknown): IdeaStatus {
  const lower = typeof raw === 'string' ? raw.toLowerCase() : '';
  return (STATUSES as readonly string[]).includes(lower) ? (lower as IdeaStatus) : 'inbox';
}

/**
 * `idea.list` の `ideas` を読む。形が壊れた要素は捨てる（1 件の壊れで一覧全体を落とさない）。
 *
 * @param found `idea.list` の `ideas`。
 */
export function parseRows(found: unknown): IdeaRow[] {
  const rows: IdeaRow[] = [];
  if (!Array.isArray(found)) {
    return rows;
  }
  for (const raw of found) {
    if (typeof raw !== 'object' || raw === null) {
      continue;
    }
    const item = raw as Record<string, unknown>;
    const lines = item.lines;
    if (
      typeof item.id !== 'string' ||
      typeof item.index !== 'number' ||
      !Array.isArray(lines) ||
      lines.length !== 2 ||
      typeof lines[0] !== 'number' ||
      typeof lines[1] !== 'number'
    ) {
      continue;
    }
    const tags = Array.isArray(item.tags)
      ? item.tags.filter((tag): tag is string => typeof tag === 'string')
      : [];
    rows.push({
      id: item.id,
      index: item.index,
      title: typeof item.title === 'string' ? item.title : '',
      status: statusOf(item.status),
      tags,
      lines: [lines[0], lines[1]],
    });
  }
  return rows;
}

/** 状態の絞り込み。`active` は「dropped 以外」（既定）。 */
export type IdeaFilter = 'active' | IdeaStatus;

/** 絞り込みの並び（クリックで選ぶ。§10.4）。 */
export const IDEA_FILTERS: readonly IdeaFilter[] = ['active', 'inbox', 'keep', 'promoted', 'dropped'];

/** 絞り込みの表示名のキー（`webui/src/lib/i18n/catalog.ts` の `idea.*`）。 */
export function filterLabelKey(filter: IdeaFilter): string {
  return filter === 'active' ? 'idea.filter_active' : `idea.status_${filter}`;
}

/** 状態の表示名のキー。 */
export function statusLabelKey(status: IdeaStatus): string {
  return `idea.status_${status}`;
}

/** 絞り込みに入るか。 */
export function admits(filter: IdeaFilter, status: IdeaStatus): boolean {
  return filter === 'active' ? status !== 'dropped' : status === filter;
}

/** 見えている一覧（状態の絞り込み → タグの部分一致。並びは `ideas.md` の順のまま）。 */
export function visibleRows(rows: readonly IdeaRow[], filter: IdeaFilter, tag: string): IdeaRow[] {
  const wanted = tag.trim();
  return rows.filter(
    (row) =>
      admits(filter, row.status) &&
      (wanted === '' || row.tags.some((found) => found.includes(wanted))),
  );
}

/**
 * 選んでいるアイデア。見えている中に無ければ、見えている最初のもの（見えるものが無ければ `null`）。
 */
export function currentRow(visible: readonly IdeaRow[], selectedId: string | null): IdeaRow | null {
  return visible.find((row) => row.id === selectedId) ?? visible[0] ?? null;
}

/** 一覧の移動。動けないとき（端・見えていない）は `null`。 */
export function moveSelection(
  visible: readonly IdeaRow[],
  selectedId: string | null,
  step: 'prev' | 'next' | 'first' | 'last',
): string | null {
  if (visible.length === 0) {
    return null;
  }
  const at = visible.findIndex((row) => row.id === selectedId);
  let target: number | null = null;
  switch (step) {
    case 'first':
      target = 0;
      break;
    case 'last':
      target = visible.length - 1;
      break;
    case 'prev':
      target = at > 0 ? at - 1 : null;
      break;
    case 'next':
      if (at === -1) {
        target = 0;
      } else if (at + 1 < visible.length) {
        target = at + 1;
      }
      break;
  }
  if (target === null || target === at) {
    return null;
  }
  return visible[target]?.id ?? null;
}

/**
 * 捨てたあとに選ぶ行（第 1 の `drop_selected` と同じ: 捨てた位置より後ろの最初。無ければ最後）。
 *
 * @param visibleAfter 捨てたあとの見えている一覧（既定では捨てたものは消えている）。
 * @param droppedIndex 捨てたアイデアの `index`。
 */
export function nextAfterDrop(visibleAfter: readonly IdeaRow[], droppedIndex: number): string | null {
  const found =
    visibleAfter.find((row) => row.index > droppedIndex) ?? visibleAfter[visibleAfter.length - 1];
  return found?.id ?? null;
}

/**
 * アイデアの本文（見出しの次の行から `lines` の終わりまで）。見出しは含めない
 * （`idea.write` が見出しを残して本文だけを書き換えるため）。
 *
 * @param text `document.get('ideas.md')` の全文。
 */
export function bodyOf(text: string, row: Pick<IdeaRow, 'lines'>): string {
  const all = text.split('\n');
  const start = Math.min(row.lines[0] + 1, all.length);
  const end = Math.min(row.lines[1], all.length);
  return end <= start ? '' : all.slice(start, end).join('\n');
}

/** 即時入力の 1 行（改行は空白にし、前後の空白を落とす）。空なら `null`（何も積まない）。 */
export function captureTitle(input: string): string | null {
  const title = input.replace(/\s*\n\s*/gu, ' ').trim();
  return title === '' ? null : title;
}

/** 昇格先の 1 行（§10.4）。`available: false` は押せない行（理由を文言で示す。§21.1.2）。 */
export interface PromoteChoice {
  /** 行き先（`idea.promote` の `target`）。 */
  target: 'plot' | 'term' | 'character' | 'material';
  /** 行き先の引数（プロットは節の ID、キャラは索引。それ以外は省く）。 */
  to?: string;
  /** 押せるか。 */
  available: boolean;
  /** 表示名のキー。 */
  labelKey: string;
  /** 表示名の差し込み（`{0}`）。 */
  labelArgs: string[];
}

/**
 * 昇格先の一覧を作る。
 *
 * - プロットの節: 今の節（`scene`）があるときだけ押せる。無ければ「本文で場面を開いてから」。
 * - 用語: いつでも押せる。
 * - キャラのメモ: 登場人物が 1 人以上のとき。人物ごとに並べる（`to` は索引）。
 * - 資料: `idea.promote` の `target` に `material` が無い（Rust 側は usage エラーにする）ため押せない。
 *   第 1 の画面は `materials_ops::propose` で行うが、それはブリッジの正規操作に無い。未提供と示す。
 */
export function promoteChoices(
  scene: string,
  characters: readonly { index: number; name: string }[],
): PromoteChoice[] {
  const choices: PromoteChoice[] = [];
  if (scene === '') {
    choices.push({ target: 'plot', available: false, labelKey: 'idea.promote_plot_none', labelArgs: [] });
  } else {
    choices.push({
      target: 'plot',
      to: scene,
      available: true,
      labelKey: 'idea.promote_plot',
      labelArgs: [scene],
    });
  }
  choices.push({ target: 'term', available: true, labelKey: 'idea.promote_term', labelArgs: [] });
  if (characters.length === 0) {
    choices.push({ target: 'character', available: false, labelKey: 'idea.no_characters', labelArgs: [] });
  } else {
    for (const found of characters) {
      choices.push({
        target: 'character',
        to: String(found.index),
        available: true,
        labelKey: 'idea.promote_character',
        labelArgs: [found.name],
      });
    }
  }
  choices.push({ target: 'material', available: false, labelKey: 'idea.promote_material', labelArgs: [] });
  return choices;
}

/** このワークスペースが横取りする操作（全体の割当より先に見る）。 */
export type IdeaAction = 'promote' | 'drop' | 'prev' | 'next' | 'first' | 'last' | 'focus-editor';

/**
 * 押されたキーの組（`chord` の形。`Ctrl+Enter`）を `idea` の操作へ写す。写せなければ `null`
 * （全体の割当に任せる）。
 *
 * 全体では `Ctrl+Enter` は「節の確定」、`Alt+↑` は「前の節」なので、この画面の中では横取りする。
 * `Ctrl+2`（エディタへ）と `Alt+0` 等は全体の割当のままにする。
 */
export function ideaAction(chord: string): IdeaAction | null {
  switch (chord) {
    case 'Ctrl+Enter':
      return 'promote';
    case 'Ctrl+Delete':
      return 'drop';
    case 'Alt+ArrowUp':
      return 'prev';
    case 'Alt+ArrowDown':
      return 'next';
    case 'Ctrl+Home':
      return 'first';
    case 'Ctrl+End':
      return 'last';
    case 'Ctrl+2':
      return 'focus-editor';
    default:
      return null;
  }
}

/** 仮想化の窓（§10.4.1）。`scrollTop` と表示高さから、描く行の範囲（終端は含まない）を返す。 */
export function windowOf(
  count: number,
  scrollTop: number,
  viewport: number,
  overscan = 6,
): { start: number; end: number } {
  if (count === 0) {
    return { start: 0, end: 0 };
  }
  const first = Math.floor(Math.max(0, scrollTop) / ROW_HEIGHT);
  const visible = Math.ceil(Math.max(0, viewport) / ROW_HEIGHT);
  const start = Math.max(0, first - overscan);
  const end = Math.min(count, first + visible + overscan);
  return { start, end };
}

/**
 * 選んだ行が見えるための最小の移動（§9.7.7「中央寄せせず、見える位置までだけ動かす」）。
 *
 * @returns 新しい `scrollTop`（動かさないなら今の値）。
 */
export function revealOffset(index: number, scrollTop: number, viewport: number): number {
  const top = index * ROW_HEIGHT;
  const bottom = top + ROW_HEIGHT;
  if (top < scrollTop) {
    return top;
  }
  if (bottom > scrollTop + viewport) {
    return Math.max(0, bottom - viewport);
  }
  return scrollTop;
}
