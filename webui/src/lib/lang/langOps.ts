// `lang.*`（LSP 相当の操作群）のパレット行と、結果の表示用の純粋関数。
//
// 機能核（`src/app/ops/lang.rs`）は 9 件すべて実装済み。ここは「パレットに並べる行」と
// 「返ってきた結果を画面へ並べる形」だけを持つ（§19.1.4: 意味は機能核が決める）。
//
// パレット行の `name` は**操作 ID**（`lang.tokens` など）。`runPalette` が `lang.` で始まる
// 名前を検出して `runLang` へ渡し、直接 `run('<操作 ID>')` で呼ぶ（`session.command` には
// `tokens` / `hover` / `symbols` / `rename_apply` / `folding` の名前が無いため、直接呼びが
// 唯一の口）。`label` は生成物 `i18n/catalog.ts` の `api.op.lang.*`（既存）。

import type { OpResult } from '../bridge/types';

/** 結果の面の 1 行（どの操作も同じ形にそろえる）。 */
export interface LangRow {
  /** 行の本文（種別名・パス・範囲など）。 */
  text: string;
  /** 薄い補足（範囲・階層・詳細）。 */
  detail: string;
  /** 行の種別（`token` / `symbol` / `reference` / `definition` / `hover` など）。 */
  kind: string;
  /** ジャンプへ渡す値（無ければ `null`）。 */
  value: unknown;
}

/** `>` パレットに出す `lang.*` の行（§9.7.1）。 */
export const LANG_PALETTE: readonly { name: string; label: string }[] = [
  { name: 'lang.tokens', label: 'api.op.lang.tokens' },
  { name: 'lang.hover', label: 'api.op.lang.hover' },
  { name: 'lang.definition', label: 'api.op.lang.definition' },
  { name: 'lang.references', label: 'api.op.lang.references' },
  { name: 'lang.symbols', label: 'api.op.lang.symbols' },
  { name: 'lang.format', label: 'api.op.lang.format' },
  { name: 'lang.rename_preview', label: 'api.op.lang.rename_preview' },
  { name: 'lang.rename_apply', label: 'api.op.lang.rename_apply' },
  { name: 'lang.folding', label: 'api.op.lang.folding' },
];

/** `lang.*` の操作 ID か（パレットから `runLang` へ分けるため）。 */
export function isLangOp(name: string): boolean {
  return name.startsWith('lang.');
}

/** 配列を取り出す（壊れていても落ちない）。 */
function asArray(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value)
    ? value.filter((item): item is Record<string, unknown> => item !== null && typeof item === 'object')
    : [];
}

/** 文字列を取り出す。 */
function str(row: Record<string, unknown>, key: string): string {
  const found = row[key];
  return typeof found === 'string' ? found : '';
}

/** `range: [start, end]` の表示（数値で無ければ空）。 */
function rangeText(row: Record<string, unknown>, key = 'range'): string {
  const found = row[key];
  if (Array.isArray(found) && found.length === 2) {
    const start = found[0];
    const end = found[1];
    if (typeof start === 'number' && typeof end === 'number') {
      return `${start}-${end}`;
    }
  }
  return '';
}

/** 値を 1 行の文字列へ（配列は连接、オブジェクトは JSON）。 */
function renderValue(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }
  if (Array.isArray(value)) {
    return value.map(renderValue).join(', ');
  }
  if (value === null || value === undefined) {
    return '';
  }
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}

/** `lang.tokens`: 字句トークンの一覧。 */
export function tokenRows(result: OpResult['lang.tokens']): LangRow[] {
  return asArray(result.tokens).map((row): LangRow => ({
    text: str(row, 'kind'),
    detail: rangeText(row),
    kind: 'token',
    value: row,
  }));
}

/** `lang.symbols`: シンボルの木（子も展開して平らに並べる）。 */
export function symbolRows(result: OpResult['lang.symbols']): LangRow[] {
  const out: LangRow[] = [];
  const walk = (nodes: unknown[]): void => {
    for (const node of nodes) {
      if (node === null || typeof node !== 'object') {
        continue;
      }
      const row = node as Record<string, unknown>;
      out.push({
        text: str(row, 'name'),
        detail: `${str(row, 'kind')} ${str(row, 'detail')}`.trim(),
        kind: 'symbol',
        value: row,
      });
      const children = row['children'];
      if (Array.isArray(children)) {
        walk(children);
      }
    }
  };
  walk(asArray(result.symbols));
  return out;
}

/** `lang.folding`: 折り畳み範囲。 */
export function foldRows(result: OpResult['lang.folding']): LangRow[] {
  return asArray(result.ranges).map((row): LangRow => ({
    text: rangeText(row),
    detail: str(row, 'depth'),
    kind: 'fold',
    value: row,
  }));
}

/** `lang.references`: 参照の一覧（ジャンプへ渡す値を残す）。 */
export function referenceRows(result: OpResult['lang.references']): LangRow[] {
  return asArray(result.references).map((row): LangRow => ({
    text: str(row, 'path'),
    detail: rangeText(row),
    kind: 'reference',
    value: row,
  }));
}

/** `lang.hover`: ホバーの中身（種別ごとのフィールドを key: value で並べる）。 */
export function hoverRows(result: OpResult['lang.hover']): LangRow[] {
  const hover = result.hover;
  if (hover === null || hover === undefined || typeof hover !== 'object') {
    return [];
  }
  const rows: LangRow[] = [];
  const range = rangeText(hover, 'range');
  if (range !== '') {
    rows.push({ text: range, detail: '', kind: 'hover-range', value: null });
  }
  const contents = hover['contents'];
  if (contents !== null && contents !== undefined && typeof contents === 'object') {
    for (const [variant, body] of Object.entries(contents as Record<string, unknown>)) {
      rows.push({ text: variant, detail: '', kind: 'hover', value: body });
      if (body !== null && body !== undefined && typeof body === 'object') {
        for (const [field, value] of Object.entries(body as Record<string, unknown>)) {
          rows.push({ text: field, detail: renderValue(value), kind: 'hover-field', value: null });
        }
      }
    }
  }
  return rows;
}

/** `lang.definition`: 定義の位置（ジャンプへ渡す値を残す）。 */
export function definitionRow(result: OpResult['lang.definition']): LangRow[] {
  const def = result.definition;
  if (def === null || def === undefined || typeof def !== 'object') {
    return [];
  }
  return [
    {
      text: str(def, 'path'),
      detail: rangeText(def),
      kind: 'definition',
      value: def,
    },
  ];
}

/** `lang.rename_preview`: リネームの計画（現在の名前と範囲）。 */
export function renamePlanRow(result: OpResult['lang.rename_preview']): LangRow[] {
  const prepared = result.prepared;
  if (prepared === null || prepared === undefined || typeof prepared !== 'object') {
    return [];
  }
  return [
    {
      text: str(prepared, 'placeholder'),
      detail: rangeText(prepared),
      kind: 'rename-plan',
      value: prepared,
    },
  ];
}
