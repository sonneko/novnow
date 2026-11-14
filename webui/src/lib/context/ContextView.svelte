<script lang="ts">
  // 文脈ビュー（§9.3）。前後の節を読み取り専用で出す。
  //
  // 編集中の節の位置には**本文を一切描画しない**（§9.3.2 MUST）。代わりに高さ 56px の
  // プレースホルダ（✎・SceneId・文字数・保存状態の点）を置く。これがないと、同じ本文が
  // 画面の上下に二重に出て「どちらを直しているのか」が分からなくなる。
  import type { Texts } from '../i18n';
  import { charsWithoutSpace, grouped } from '../editor/text';

  /** 文言。 */
  export let t: Texts['t'];
  /** いまの節（`01-04-02-03`）。 */
  export let scene = '';
  /** 前後の節の中身（`section.get` の `before` / `after`）。 */
  export let before: { scene: string; text: string }[] = [];
  export let after: { scene: string; text: string }[] = [];
  /** いまの節の文字数（プレースホルダに出す）。 */
  export let sectionChars = 0;
  /** 保存状態（プレースホルダの点）。 */
  export let save: string = 'idle';
  /** 節を選んだ（`nav.section`）。 */
  export let onnav: ((scene: string) => void) | null = null;
  /** 節を消した（`section.delete`。確認は §8.1.2）。 */
  export let ondelete: ((scene: string) => void) | null = null;

  /** 節番号の表示（`01-04-02-03` をそのまま。§9.3.4 の完全形）。 */
  function label(id: string): string {
    return id;
  }

  /** 並び（前 → いま → 後）。 */
  $: rows = [
    ...before.map((row) => ({ ...row, here: false })),
    { scene, text: '', here: true },
    ...after.map((row) => ({ ...row, here: false })),
  ].filter((row) => row.here || row.text !== '');
</script>

<section class="context" aria-label={t('editor.editing')}>
  {#if scene}
    {#each rows as row (row.scene)}
      {#if row.here}
        <!-- 編集中プレースホルダ（§9.3.2）。本文は絶対に描画しない。 -->
        <div class="placeholder" data-save={save}>
          <span class="pencil" aria-hidden="true">✎</span>
          <span class="id">{label(row.scene)}</span>
          <span class="chars">{grouped(sectionChars)}{t('status.chars_unit')}</span>
          <span class="dot" aria-hidden="true"></span>
        </div>
      {:else}
        <article class="section">
          <button
            type="button"
            class="head"
            title={t('context.guide_section')}
            on:click={() => onnav?.(row.scene)}
          >
            {label(row.scene)}
          </button>
          <button
            type="button"
            class="remove"
            aria-label={`${t('command.delete_section')}: ${row.scene}`}
            title={t('command.delete_section')}
            on:click={() => ondelete?.(row.scene)}
          >
            ×
          </button>
          <p class="prose">{row.text}</p>
        </article>
      {/if}
    {/each}
  {:else}
    <div class="guide">
      <p>{t('context.guide_workspace')}</p>
      <p>{t('context.guide_section')}</p>
      <p>{t('context.guide_palette')}</p>
    </div>
  {/if}
</section>

<style>
  .context {
    overflow: auto;
    padding: 8px 12px 16px;
    background: var(--bg-base);
    color: var(--fg-primary);
    font-size: 15px;
    line-height: 1.9;
  }
  .section {
    position: relative;
    border-bottom: 1px solid var(--border);
    padding-bottom: 8px;
    margin-bottom: 8px;
  }
  .head {
    border: 0;
    background: transparent;
    color: var(--fg-faint);
    font: inherit;
    font-size: 10px;
    font-family: var(--mono, ui-monospace, monospace);
    cursor: pointer;
    padding: 0;
    float: right;
  }
  .head:hover {
    color: var(--accent);
    text-decoration: underline;
  }
  .remove {
    position: absolute;
    right: 0;
    bottom: 4px;
    width: 20px;
    height: 20px;
    border: 0;
    background: transparent;
    color: var(--fg-faint);
    cursor: pointer;
    visibility: hidden;
  }
  .section:hover .remove,
  .remove:focus-visible {
    visibility: visible;
  }
  .remove:hover {
    color: var(--state-error);
  }
  .prose {
    margin: 4px 0 0;
    /* 1 行の最大幅は全角 44 字（§9.3.1） */
    max-width: 44em;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .placeholder {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 56px;
    margin: 8px 0;
    padding: 0 10px;
    background: var(--bg-inset);
    border-left: 3px solid var(--accent);
    color: var(--fg-muted);
    font-size: 11px;
  }
  .id {
    font-family: var(--mono, ui-monospace, monospace);
  }
  .chars {
    margin-left: auto;
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: transparent;
  }
  .placeholder[data-save='dirty'] .dot {
    background: var(--state-unsaved);
  }
  .placeholder[data-save='error'] .dot,
  .placeholder[data-save='conflict'] .dot {
    background: var(--state-error);
  }
  .guide {
    color: var(--fg-faint);
    font-size: 13px;
  }
  .guide p {
    margin: 2px 0;
  }
</style>
