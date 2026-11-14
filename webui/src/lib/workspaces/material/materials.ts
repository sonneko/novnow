// `material` ワークスペースの規則（§10.5・§16）。描画に依らない部分だけを置く。
//
// このファイルは**実行時の import を持たない**（テストが単独で読めるように）。
// 規則の正本は `REF:src/core/features/materials_ops.rs`（`draft_of` / `has_no_source` / `search`）と
// `REF:src/core/model/material.rs`（`Source::is_missing` / `shown`）。ここはその写しで、
// 食い違うときは正本に合わせる。

/** 資料の状態（§5.8.2）。 */
export type MaterialStatus = 'proposed' | 'approved' | 'rejected';

/** 一覧の絞り込み。`active` は「却下以外」（既定。§16.3）。 */
export type MaterialFilter = 'active' | MaterialStatus;

/** 絞り込みの並び（クリックで選ぶ）。 */
export const MATERIAL_FILTERS: readonly MaterialFilter[] = ['active', 'proposed', 'approved', 'rejected'];

/** 一覧の 1 行の高さ（px。仮想化に使う。§10.4.1）。 */
export const ROW_HEIGHT = 28;

/** 審査フォームの項目（`materials.get` の `review` と同じ形。値は素の文字列）。 */
export interface Review {
  title: string;
  category: string;
  summary: string;
  tags: string;
  conditions: string;
  source: string;
}

/** 一覧の 1 行。索引に無い資料 MD は `id` が `null`（W064）。 */
export interface MaterialRow {
  /** 選択の鍵（資料 ID。索引に無ければパス）。 */
  key: string;
  /** 資料 ID（索引に無ければ `null`）。 */
  id: string | null;
  /** 一覧に出す名前。 */
  title: string;
  /** 資料 MD のパス。 */
  path: string;
  /** 状態（索引に無い・表に無い値は `null`）。 */
  status: MaterialStatus | null;
  /** 分類（索引に無ければ空）。 */
  category: string;
}

/** `materials.search` の 1 件（必要なものだけ）。 */
interface HitLike {
  id?: unknown;
  title?: unknown;
  path?: unknown;
  status?: unknown;
  category?: unknown;
  saved_at?: unknown;
  date?: unknown;
}

/** 状態の読み（表に無い値は `null`。`REF:…material.rs` の `MaterialStatus::parse`）。 */
export function statusOf(raw: unknown): MaterialStatus | null {
  return raw === 'proposed' || raw === 'approved' || raw === 'rejected' ? raw : null;
}

/** 絞り込みを検索語の 1 語にする（`!状態`）。`active` は語を足さない。 */
export function filterToken(filter: MaterialFilter): string {
  return filter === 'active' ? '' : `!${filter}`;
}

/** 絞り込みの表示名のキー（`webui/src/lib/i18n/catalog.ts` の `material.*`）。 */
export function filterLabelKey(filter: MaterialFilter): string {
  return filter === 'active' ? 'material.filter_active' : `material.status_${filter}`;
}

/** 状態の表示名のキー（表に無ければ `status_unknown`）。 */
export function statusLabelKey(status: MaterialStatus | null): string {
  return status === null ? 'material.status_unknown' : `material.status_${status}`;
}

/** 索引に無い資料 MD の、資料 MD のパス（`materials/` 配下の相対）から一覧の名前を作る。 */
function nameOf(path: string): string {
  return path.replace(/^\.?\/?materials\//u, '');
}

/**
 * `review.list` の診断から、索引に無い資料 MD（W064）を取り出す。
 *
 * @param diagnostics `review.list` の `diagnostics`。
 */
export function unregisteredOf(diagnostics: unknown): MaterialRow[] {
  const out: MaterialRow[] = [];
  const seen = new Set<string>();
  if (!Array.isArray(diagnostics)) {
    return out;
  }
  for (const raw of diagnostics) {
    const item = raw as { code?: unknown; path?: unknown } | null;
    if (!item || item.code !== 'W064' || typeof item.path !== 'string') {
      continue;
    }
    if (seen.has(item.path)) {
      continue;
    }
    seen.add(item.path);
    out.push({
      key: item.path,
      id: null,
      title: nameOf(item.path),
      path: item.path,
      status: null,
      category: '',
    });
  }
  return out;
}

/** `materials.search` の結果を一覧の行にする。 */
export function rowsOf(hits: unknown): MaterialRow[] {
  const out: MaterialRow[] = [];
  if (!Array.isArray(hits)) {
    return out;
  }
  for (const raw of hits) {
    const hit = raw as HitLike | null;
    if (!hit || typeof hit.id !== 'string' || typeof hit.path !== 'string') {
      continue;
    }
    out.push({
      key: hit.id,
      id: hit.id,
      title: typeof hit.title === 'string' && hit.title !== '' ? hit.title : hit.id,
      path: hit.path,
      status: statusOf(hit.status),
      category: typeof hit.category === 'string' ? hit.category : '',
    });
  }
  return out;
}

/**
 * 一覧（§16.3 の並び）: 既定の絞り込みのときだけ、索引に無い資料 MD を先頭に足す。
 * 続けて検索の結果（サーバ側の並び）。
 *
 * @param unregistered 索引に無い資料 MD（`unregisteredOf`）。
 * @param registered 検索の結果（`rowsOf`）。
 * @param filter 絞り込み。
 * @param query 検索語（パスの部分一致で索引外の行を絞る）。
 */
export function listOf(
  unregistered: readonly MaterialRow[],
  registered: readonly MaterialRow[],
  filter: MaterialFilter,
  query: string,
): MaterialRow[] {
  const out: MaterialRow[] = [];
  if (filter === 'active') {
    const wanted = query.trim().toLowerCase();
    for (const row of unregistered) {
      if (wanted === '' || row.path.toLowerCase().includes(wanted)) {
        out.push(row);
      }
    }
  }
  out.push(...registered);
  return out;
}

/** 検索語の組み立て（一覧の検索と絞り込みを 1 本にする。`Ctrl+M` と同じ語）。 */
export function searchText(query: string, filter: MaterialFilter): string {
  return `${query.trim()} ${filterToken(filter)}`.trim();
}

/**
 * 新しい資料の題名と分類（`draft_of` の写し）。素の語を題名に、最初の `#分類` を分類にする。
 * 演算子（`#` `@` `'` `!` `~` と `M0137` の形）は捨てる。
 */
export function draftOf(query: string): { title: string; category: string } {
  const title: string[] = [];
  let category = '';
  for (const token of query.split(/\s+/u).filter((part) => part !== '')) {
    if (token.startsWith('#')) {
      if (category === '') {
        category = token.slice(1);
      }
    } else if (!isOperator(token)) {
      title.push(token);
    }
  }
  return { title: title.join(' '), category };
}

/** 検索の演算子か（`draft_of` の `is_operator` の写し）。 */
function isOperator(token: string): boolean {
  return /^[#@'!~]/u.test(token) || /^[MS]\d{4,}$/u.test(token);
}

/**
 * 出典が無いか（W065 の条件。`Source::is_missing` の写し）。`kind: search` は検索語そのものが出典。
 *
 * @param source `materials.get` の `material.source`。
 */
export function hasNoSource(source: unknown): boolean {
  const found = (source ?? {}) as { kind?: unknown; url?: unknown; text?: unknown };
  if (found.kind === 'search') {
    return false;
  }
  const url = typeof found.url === 'string' ? found.url.trim() : '';
  const text = typeof found.text === 'string' ? found.text.trim() : '';
  return url === '' && text === '';
}

/** 審査フォームの 2 つが同じか（同じなら索引へ書き直さない）。 */
export function sameReview(a: Review, b: Review): boolean {
  return (
    a.title === b.title &&
    a.category === b.category &&
    a.summary === b.summary &&
    a.tags === b.tags &&
    a.conditions === b.conditions &&
    a.source === b.source
  );
}

/** `materials.get` の `review` を読む（形が壊れていれば空の項目）。 */
export function reviewOf(raw: unknown): Review {
  const found = (raw ?? {}) as Record<string, unknown>;
  const text = (key: keyof Review): string => (typeof found[key] === 'string' ? (found[key] as string) : '');
  return {
    title: text('title'),
    category: text('category'),
    summary: text('summary'),
    tags: text('tags'),
    conditions: text('conditions'),
    source: text('source'),
  };
}

/** 次に選ぶ行（却下などで消えたあと）: 同じ位置の行、無ければ最後。 */
export function nextKeyAfterRemoval(rows: readonly MaterialRow[], removedAt: number): string | null {
  const found = rows[removedAt] ?? rows[rows.length - 1];
  return found?.key ?? null;
}

/** 選ぶ行の移動（`prev` / `next` / `first` / `last`）。動けないときは `null`。 */
export function moveKey(
  rows: readonly MaterialRow[],
  selected: string | null,
  step: 'prev' | 'next' | 'first' | 'last',
): string | null {
  if (rows.length === 0) {
    return null;
  }
  const at = rows.findIndex((row) => row.key === selected);
  let target: number | null = null;
  switch (step) {
    case 'first':
      target = 0;
      break;
    case 'last':
      target = rows.length - 1;
      break;
    case 'prev':
      target = at > 0 ? at - 1 : null;
      break;
    case 'next':
      if (at === -1) {
        target = 0;
      } else if (at + 1 < rows.length) {
        target = at + 1;
      }
      break;
  }
  if (target === null || target === at) {
    return null;
  }
  return rows[target]?.key ?? null;
}

/** 押されたキーの組（`chord` の形）を資料の操作へ写す。写せなければ `null`。 */
export type MaterialAction =
  | 'approve'
  | 'reject'
  | 'new'
  | 'prev'
  | 'next'
  | 'first'
  | 'last'
  | 'focus-editor'
  | 'focus-form';

export function materialAction(chord: string): MaterialAction | null {
  switch (chord) {
    case 'Ctrl+Enter':
      return 'approve';
    case 'Ctrl+Delete':
      return 'reject';
    case 'Ctrl+Shift+N':
    case 'Ctrl+Alt+Shift+N': // `Ctrl+Shift+N` を奪う環境の代替（§9.8.6）
      return 'new';
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
    case 'Ctrl+3':
      return 'focus-form';
    default:
      return null;
  }
}

/** 本文の引用注釈を探す語（`^M0137`。行末の資料引用注釈。§5.2.4）。 */
export function citationQuery(id: string): string {
  return `^${id}`;
}

/** 仮想化の窓（§10.4.1）。描く行の範囲（終端は含まない）。 */
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
  return {
    start: Math.max(0, first - overscan),
    end: Math.min(count, first + visible + overscan),
  };
}

/** 選んだ行が見える最小の移動（§9.7.7。中央寄せしない）。 */
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

/**
 * 分類の候補（§16.1）。`material.categories` の文言を、空白で区切らず `,` で割る。
 * 今の値が候補に無ければ先頭に足す（既存の資料の分類を消さない）。
 */
export function categoryOptions(categoryText: string, current: string): string[] {
  const options = categoryText
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part !== '');
  if (current !== '' && !options.includes(current)) {
    options.unshift(current);
  }
  return options;
}
