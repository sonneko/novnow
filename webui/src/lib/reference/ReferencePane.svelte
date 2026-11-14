<script lang="ts">
  // 参照ペイン（§9.5）。表示順は固定（§9.5.2 の 1〜9）。
  //
  // 5（五感）・6（空間）・8（挿絵）は内容が無ければ**見出しごと出さない**（§9.5.2 MUST）。
  // 空の見出しが並ぶと情報密度が落ちるため。
  //
  // 材料は `reference.pane` の戻り値（`REF:src/app/ops/knowledge.rs` の `reference_pane`）。
  import type { Texts } from '../i18n';

  /** 文言。 */
  export let t: Texts['t'];
  /** `reference.pane` の戻り値（形は `REF:src/app/ops/knowledge.rs` の `reference_pane`）。 */
  export let pane: Record<string, unknown> | null = null;
  /** 行を押した（§9.5.5 の飛び先）。 */
  export let onjump: ((kind: string, value: unknown) => void) | null = null;

  /** 1 行（どのまとまりも同じ形にそろえる。`detail` は薄い補足）。 */
  interface Entry {
    text: string;
    detail: string;
    kind: string;
    value: unknown;
  }

  /** 配列を取り出す（`pane` が壊れていても落ちない）。 */
  function rows(key: string): Record<string, unknown>[] {
    const found = pane?.[key];
    return Array.isArray(found) ? (found as Record<string, unknown>[]) : [];
  }

  /** 文字列を取り出す。 */
  function str(row: Record<string, unknown>, key: string): string {
    const found = row[key];
    return typeof found === 'string' ? found : '';
  }

  /** 文字列の配列を取り出す（`modes`）。 */
  function list(row: Record<string, unknown>, key: string): string[] {
    const found = row[key];
    return Array.isArray(found) ? found.filter((item): item is string => typeof item === 'string') : [];
  }

  /** 診断は E → W の順に並べる（§9.5.2 の 9）。 */
  $: diagnostics = rows('diagnostics')
    .slice()
    .sort((left, right) => str(left, 'severity').localeCompare(str(right, 'severity')))
    .map((row): Entry => ({
      text: `${str(row, 'severity').toUpperCase()} ${str(row, 'code')} ${str(row, 'message')}`,
      detail: str(row, 'path'),
      kind: 'diagnostic',
      value: row,
    }));
  $: plots = rows('plot').map((row): Entry => ({
    text: str(row, 'text'),
    detail: '',
    kind: 'plot',
    value: row,
  }));
  $: upper = rows('upper').map((row): Entry => ({
    text: str(row, 'summary'),
    detail: '',
    kind: 'upper',
    value: row,
  }));
  $: cast = rows('cast').map((row): Entry => ({
    text: `${str(row, 'name')}  ${list(row, 'modes').join('+')}`,
    detail: [str(row, 'personality'), str(row, 'emotion'), str(row, 'role')].filter(Boolean).join(' / '),
    kind: 'cast',
    value: row,
  }));
  $: words = rows('words').map((row): Entry => ({
    text: str(row, 'term'),
    detail: str(row, 'definition').split('\n')[0] ?? '',
    kind: 'word',
    value: row,
  }));
  $: materials = rows('materials').map((row): Entry => ({
    text: `${str(row, 'id')} ${str(row, 'title')}`,
    detail: str(row, 'summary'),
    kind: 'material',
    value: row,
  }));
  /** `senses` は文字列の配列（宣言そのもの）。 */
  $: senses = Array.isArray(pane?.['senses'])
    ? (pane?.['senses'] as unknown[]).filter((row): row is string => typeof row === 'string')
    : [];
  /** `space` は 1 行 1 要素（`space-mode: ascii` なら等幅・折り返しなし。§9.5.3）。 */
  $: space = Array.isArray(pane?.['space'])
    ? (pane?.['space'] as unknown[]).filter((row): row is string => typeof row === 'string')
    : [];
  /** `illustrations` は文字列の配列。 */
  $: illustrations = Array.isArray(pane?.['illustrations'])
    ? (pane?.['illustrations'] as unknown[]).filter((row): row is string => typeof row === 'string')
    : [];

  /** 表示順は固定（§9.5.2 の 1〜4・7・9）。 */
  $: groups = [
    { title: 'pane.plot', entries: plots },
    { title: 'pane.upper', entries: upper },
    { title: 'pane.cast', entries: cast },
    { title: 'pane.words', entries: words },
    { title: 'pane.materials', entries: materials },
    { title: 'pane.diagnostics', entries: diagnostics },
  ] as { title: string; entries: Entry[] }[];
</script>

<aside class="pane" aria-label={t('pane.plot')}>
  {#if pane}
    {#each groups as group (group.title)}
      {#if group.entries.length > 0}
        <section class="group">
          <h2>{t(group.title)}</h2>
          {#each group.entries as row, index (index)}
            <button type="button" class="row" on:click={() => onjump?.(row.kind, row.value)}>
              <span class="line">{row.text}</span>
              {#if row.detail}
                <span class="detail">{row.detail}</span>
              {/if}
            </button>
          {/each}
        </section>
      {/if}
    {/each}

    {#if senses.length > 0}
      <section class="group">
        <h2>{t('stage.senses')}</h2>
        {#each senses as found (found)}
          <p class="line">{found}</p>
        {/each}
      </section>
    {/if}

    {#if space.length > 0}
      <section class="group">
        <h2>{t('stage.space')}</h2>
        <pre class="space">{space.join('\n')}</pre>
      </section>
    {/if}

    {#if illustrations.length > 0}
      <section class="group">
        <h2>{t('stage.illustrations')}</h2>
        {#each illustrations as found (found)}
          <p class="line">{found}</p>
        {/each}
      </section>
    {/if}
  {:else}
    <p class="none">{t('pane.more')}</p>
  {/if}
</aside>

<style>
  .pane {
    overflow: auto;
    padding: 10px 12px 24px;
    background: var(--bg-base);
    color: var(--fg-primary);
    font-size: 13px;
    line-height: 1.6;
  }
  .group {
    padding-bottom: 10px;
    margin-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }
  .group:last-child {
    border-bottom: 0;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 11px;
    font-weight: 400;
    color: var(--fg-faint);
    letter-spacing: 0.02em;
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    padding: 2px 4px;
    cursor: pointer;
    border-radius: 2px;
  }
  .row:hover {
    background: var(--bg-selection);
  }
  .line {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .detail {
    color: var(--fg-muted);
    font-size: 12px;
  }
  .space {
    margin: 0;
    white-space: pre;
    overflow-x: auto;
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 12px;
  }
  .none {
    color: var(--fg-faint);
    font-size: 12px;
  }
</style>
