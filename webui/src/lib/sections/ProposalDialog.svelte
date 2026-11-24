<script lang="ts">
  // 節の提案（`section.propose`。§8.7.1）。
  //
  // 提案は**本文を変えない**（`agents/proposals/` に 1 件置くだけ）。したがって確認は要らず、
  // 押したら置いて、置いた提案の ID を通知に出す（§21.1.2: 何が起きたか分かるようにする）。
  //
  // 対象の節は開いている節（`scene`）。提案する本文は、いまの節の本文を初期値にして
  // 書き換えてもらう（空の欄から書き直させない）。`note` は任意。
  //
  // 閉じたら入力を破棄する（§9.7.5 の「短い 1 項目の入力は破棄」に準じる。項目は 2 つだが
  // 本文を抱えたままにしない）。`Esc` と外側クリックで閉じる。
  // 文言は生成物 `i18n/catalog.ts` に実在するキーだけを使う（§12.3）。

  import type { Texts } from '../i18n';
  import { proposalArgs } from '../sections/sectionOps';

  /** 文言。 */
  export let t: Texts['t'];
  /** 対象の節（`section.propose` の `scene`）。 */
  export let scene = '';
  /** 提案の初期値（いまの節の本文）。 */
  export let text = '';
  /** 閉じる。 */
  export let onclose: (() => void) | null = null;
  /** 提案を置く（`section.propose`）。失敗したら文言を返す。 */
  export let onpropose: ((args: Record<string, unknown>) => Promise<string>) | null = null;

  /** 提案する本文。 */
  let body = text;
  /** 覚え書き（任意）。 */
  let note = '';
  /** 実行中。 */
  let busy = false;
  /** 出している失敗の文言。 */
  let failure = '';
  /** 本文の欄（開いたら focus する）。 */
  let field: HTMLTextAreaElement | null = null;

  /** 開いたら本文の欄へ focus する（キーボードだけで済む。§21.2.1）。 */
  export function focus(): void {
    field?.focus();
  }

  /** 背景（外側）を押したら閉じる（§9.7.5。入力は破棄する）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      close();
    }
  }

  function close(): void {
    body = '';
    note = '';
    failure = '';
    onclose?.();
  }

  /** 提案を置く。 */
  async function submit(): Promise<void> {
    if (busy) {
      return;
    }
    const plan = proposalArgs({ scene, text: body, note });
    if (!plan.ok) {
      failure =
        plan.reason === 'no-scene' ? t('editor.no_sections') : t('rename.empty');
      return;
    }
    busy = true;
    try {
      const message = (await onpropose?.(plan.args)) ?? '';
      if (message) {
        failure = message;
        return;
      }
      close();
    } finally {
      busy = false;
    }
  }

  /** `Enter` は改行（本文を書く欄）。`Esc` で閉じる。実行はボタンで行う。 */
  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      close();
    }
  }
</script>

<div class="overlay" role="presentation" on:click={outside}>
  <div
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-label={t('api.op.section.propose')}
    tabindex="-1"
  >
    <h2>{t('api.op.section.propose')}</h2>
    <p class="scene">{scene}</p>
    <textarea
      bind:this={field}
      bind:value={body}
      class="body"
      aria-label={t('api.op.section.propose')}
      spellcheck="false"
      on:keydown={keydown}
    ></textarea>
    <input
      bind:value={note}
      type="text"
      class="note"
      aria-label={t('rename.placeholder')}
      placeholder={t('rename.placeholder')}
      spellcheck="false"
      autocomplete="off"
      on:keydown={keydown}
    />
    {#if failure}
      <p class="failure" role="alert">{failure}</p>
    {/if}
    <div class="actions">
      <button type="button" class="button" disabled={busy} on:click={() => void submit()}>
        {busy ? t('status.saving') : t('project.pick.ok')}
      </button>
      <button type="button" class="button" on:click={close}>{t('modal.cancel')}</button>
    </div>
    <p class="close-hint">{t('modal.close_hint')}</p>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 41;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 64px;
    background: rgb(0 0 0 / 35%);
  }
  .sheet {
    width: min(620px, 92vw);
    padding: 14px 16px 12px;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--fg-primary);
    font-size: 12px;
  }
  h2 {
    margin: 0 0 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
  }
  .scene {
    margin: 0 0 8px;
    color: var(--fg-faint);
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 11px;
  }
  .body {
    width: 100%;
    box-sizing: border-box;
    min-height: 180px;
    padding: 8px;
    background: var(--bg-inset);
    border: 1px solid var(--border);
    border-radius: 2px;
    color: var(--fg-primary);
    font: inherit;
    line-height: 1.7;
    resize: vertical;
  }
  .note {
    width: 100%;
    box-sizing: border-box;
    margin-top: 8px;
    padding: 6px 8px;
    background: var(--bg-inset);
    border: 1px solid var(--border);
    border-radius: 2px;
    color: var(--fg-primary);
    font: inherit;
  }
  .body:focus,
  .note:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .failure {
    margin: 8px 0 0;
    color: var(--state-error);
    font-size: 11px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
  .button {
    padding: 5px 12px;
    border: 1px solid var(--border);
    border-radius: 2px;
    background: transparent;
    color: var(--fg-primary);
    font: inherit;
    cursor: pointer;
  }
  .button:hover {
    background: var(--bg-selection);
  }
  .button:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .close-hint {
    margin: 10px 0 0;
    color: var(--fg-faint);
    font-size: 11px;
  }
</style>
