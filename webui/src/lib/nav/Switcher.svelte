<script lang="ts">
  // 切替パネル（§10.0 の一覧 + §9.7 の移動の口）。`Ctrl+K` で開く。
  //
  // 出すもの:
  //
  // 1. **移動の行**（`nav.prev` / `nav.next` / `nav.back` / `nav.forward`）
  // 2. **ワークスペース 9 つ**（`nav.switcher` の `workspaces` をそのまま並べる。番号も写す）
  // 3. **プロットの確認状態**（`plot.set_state`。§10.3.3）と
  //    **文書の行**（`document.rename` の口）
  //
  // 一覧は `nav.switcher` の戻り値だけで作る（画面で並べ直さない。§19.1.4）。押せないものは
  // 置かない。文言は生成物 `i18n/catalog.ts` に実在するキーだけを引く（§12.3）。
  //
  // 数字キー（`1`〜`9`）でもワークスペースを選べるようにする（`Alt+<n>` と同じ番号）。
  // これは入力欄ではないので、キーは画面の 1 か所（`App.svelte`）が受けて `onpick` に来る。

  import type { Texts } from '../i18n';
  import { switcherCurrent, switcherRows, type SwitcherRow } from '../nav/moves';

  /** 文言。 */
  export let t: Texts['t'];
  /** `nav.switcher` の戻り値（そのまま）。 */
  export let found: Record<string, unknown> | null = null;
  /** いまの文書のパス（`document.rename` の `from`）。空なら文書の行を出さない。 */
  export let document = '';
  /** 閉じる。 */
  export let onclose: (() => void) | null = null;
  /** 行を選んだ。 */
  export let onpick: ((row: SwitcherRow) => void) | null = null;
  /** 文書のリネームを始める（`document.rename`）。 */
  export let onrename: (() => void) | null = null;
  /** いまの節（`plot.set_state` の対象）。空なら出さない。 */
  export let scene = '';
  /** いまの節のプロットの確認状態（`fixed` / `draft`）。 */
  export let plotState: 'fixed' | 'draft' = 'draft';
  /** 確認状態を切り替える（`plot.set_state`。§10.3.3）。 */
  export let ontogglestate: (() => void) | null = null;

  let rows: SwitcherRow[] = switcherRows(found);
  $: rows = switcherRows(found);
  $: current = switcherCurrent(found);
  $: workspaces = rows.filter((row) => row.kind === 'workspace');
  $: moves = rows.filter((row) => row.kind === 'move');

  /** 背景（外側）を押したら閉じる（§9.7.5）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      onclose?.();
    }
  }
</script>

<div class="overlay" role="presentation" on:click={outside}>
  <div
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-label={t('api.op.nav.switcher')}
    tabindex="-1"
  >
    <h2>{t('api.op.nav.switcher')}</h2>

    <!-- 移動の口（§9.8.2 のキーと同じ並び） -->
    <div class="section">
      {#each moves as row (row.op)}
        <button type="button" class="row" on:click={() => onpick?.(row)}>
          <span class="label">{t(row.label)}</span>
          <kbd>{row.op.replace('nav.', '')}</kbd>
        </button>
      {/each}
    </div>

    <!-- ワークスペース 9 つ（`Alt+<number>` と同じ番号） -->
    <div class="section">
      {#each workspaces as row (row.label)}
        <button
          type="button"
          class="row"
          class:current={row.current}
          class:previous={row.previous}
          aria-current={row.current ? 'page' : undefined}
          on:click={() => onpick?.(row)}
        >
          <span class="number" aria-hidden="true">{row.number ?? ''}</span>
          <span class="label">{t(row.label)}</span>
          {#if row.previous && !row.current}
            <kbd>Alt+0</kbd>
          {/if}
        </button>
      {/each}
      {#if workspaces.length === 0}
        <p class="empty" role="status">{t('workspace.unavailable')}</p>
      {/if}
    </div>

    <!-- プロットの確認状態（`plot.set_state`。§10.3.3） -->
    {#if scene}
      <div class="section">
        <button type="button" class="row" on:click={() => ontogglestate?.()}>
          <span class="label">{t('api.op.plot.set_state')}</span>
          <kbd>{plotState}</kbd>
        </button>
        <p class="path">{scene}</p>
      </div>
    {/if}

    <!-- 文書のリネーム（`document.rename`） -->
    {#if document}
      <div class="section">
        <button type="button" class="row" on:click={() => onrename?.()}>
          <span class="label">{t('project.pick.key_rename')}</span>
          <kbd>rename-document</kbd>
        </button>
        <p class="path">{document}</p>
      </div>
    {/if}

    <p class="current">{current}</p>
    <p class="close-hint">{t('modal.close_hint')}</p>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 48px;
    background: rgb(0 0 0 / 35%);
  }
  .sheet {
    width: min(560px, 92vw);
    max-height: 70vh;
    overflow-y: auto;
    padding: 12px 14px 10px;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--fg-primary);
    font-size: 12px;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
  }
  .section + .section {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 6px;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--fg-primary);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover,
  .row:focus-visible {
    background: var(--bg-selection);
  }
  .row.current {
    color: var(--accent);
  }
  .row.previous .label::after {
    content: '';
  }
  .number {
    min-width: 1.2em;
    color: var(--fg-faint);
    font-family: var(--mono, ui-monospace, monospace);
  }
  .label {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  kbd {
    color: var(--fg-faint);
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 11px;
  }
  .path,
  .current,
  .empty {
    margin: 4px 6px 0;
    color: var(--fg-faint);
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .current {
    margin-top: 8px;
  }
  .close-hint {
    margin: 8px 0 0;
    color: var(--fg-faint);
    font-size: 11px;
  }
</style>
