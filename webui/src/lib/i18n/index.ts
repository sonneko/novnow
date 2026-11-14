// 文言（§12）。正本は `REF:src/ui/i18n/ja.toml` / `en.toml` で、`catalog.ts` はその生成物。
//
// 画面は `t('header.theme')` の形でだけ文言を引く。文字列リテラルを直接コンポーネントへ
// 書かない（§12.3 MUST: 第 2 のための文言集を持たない）。

import { CATALOGS, type Catalog, type Locale } from './catalog';

export type { Catalog, Locale } from './catalog';
export { CATALOGS, KEYS, LOCALES } from './catalog';

/** `{0}` を差し込む（`REF:src/ui/i18n/mod.rs` の `fill` と同じ規則）。 */
export function fill(template: string, args: readonly string[] = []): string {
  return template.replace(/\{(\d+)\}/g, (whole, index: string) => {
    const found = args[Number(index)];
    return found === undefined ? whole : found;
  });
}

/** 1 つの言語のカタログ（`t` を束ねたもの）。 */
export interface Texts {
  /** 言語。 */
  readonly locale: Locale;
  /** キー → 文（`{0}` は `args` で差し込む）。無いキーは空文字。 */
  t(key: string, args?: readonly string[]): string;
  /** カタログを丸ごと入れ替えたもの（言語切替に使う）。 */
  with(locale: Locale): Texts;
}

/** カタログから `t` を作る。 */
export function texts(locale: Locale): Texts {
  const catalog: Catalog = CATALOGS[locale];
  return {
    locale,
    t(key, args) {
      const found = catalog[key] ?? '';
      return args && args.length ? fill(found, args) : found;
    },
    with(next) {
      return texts(next);
    },
  };
}
