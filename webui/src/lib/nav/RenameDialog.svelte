<script lang="ts">
  // 文書の名前の変更（`document.rename`。パレットの `>` + `rename-document`、切替パネルの
  // 文書行・ヘッダのボタンから開く）。
  //
  // 流れ:
  //
  // 1. 新しい名前（パス）を入力する。初期値は今のパス（`contents/01-04.cnov`）。
  // 2. 形だけを画面で確かめる（`renamePlan`）。既にあるか・書けるかは**機能核が決める**ので
  //    先回りしない（§19.1.4）。
  // 3. `document.rename` を `run` する。失敗はそのままの文言を出す（`rename.*` を使う）。
  // 4. 成功したら、新しいパスの節を開き直す（いま開いていたパスが変わったため）。
  //
  // 閉じたら入力を破棄する（§9.7.5 の「短い 1 項目の入力は破棄」）。`Esc` と外側クリックで閉じる。
  // 文言は生成物 `i18n/catalog.ts` に実在するキーだけを引く（§12.3）。

  import type { Texts } from '../i18n';
  import { RENAME_MESSAGES, renamePlan } from '../nav/moves';

  /** 文言。 */
  export let t: Texts['t'];
  /** いまの文書のパス。 */
  export let from = '';
  /** 閉じる。 */
  export let onclose: (() => void) | null = null;
  /** 実行する（`document.rename` の `from` / `to`）。失敗したら文言を返す。 */
  export let onrename: ((from: string, to: string) => Promise<string>) | null = null;

  /** 入力（初期値は今のパス）。 */
  let value = from;
  /** 実行中。 */
  let busy = false;
  /** 出している失敗の文言。 */
  let failure = '';
  /** 入力欄（開いたら focus する）。 */
  let field: HTMLInputElement | null = null;

  $: if (from && value === '') {
    value = from;
  }

  /** 開いたら入力欄へ focus する（キーボードだけで済む。§21.2.1）。 */
  export function focus(): void {
    field?.focus();
    field?.select();
  }

  /** 背景（外側）を押したら閉じる（§9.7.5。入力は破棄する）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      close();
    }
  }

  function close(): void {
    value = '';
    failure = '';
    onclose?.();
  }

  /** 実行する。 */
  async function submit(): Promise<void> {
    if (busy) {
      return;
    }
    const plan = renamePlan(from, value);
    if (!plan.ok) {
      failure = t(RENAME_MESSAGES[plan.reason] ?? 'rename.invalid');
      return;
    }
    busy = true;
    try {
      const message = (await onrename?.(plan.from, plan.to)) ?? '';
      if (message) {
        failure = message;
        return;
      }
      close();
    } finally {
      busy = false;
    }
  }

  /** `Enter` で実行、`Esc` で閉じる（重ね物の共通規則）。 */
  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      void submit();
      return;
    }
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
    aria-label={t('api.op.document.rename')}
    tabindex="-1"
  >
    <h2>{t('api.op.document.rename')}</h2>
    <!-- 入力欄は 1 つだけ（§7.9 の表記に合わせ、ラベルは placeholder で示す） -->
    <input
      bind:this={field}
      bind:value
      type="text"
      class="field"
      aria-label={t('rename.placeholder')}
      placeholder={t('rename.placeholder')}
      spellcheck="false"
      autocomplete="off"
      on:keydown={keydown}
    />
    <p class="path">{from}</p>
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
    padding-top: 96px;
    background: rgb(0 0 0 / 35%);
  }
  .sheet {
    width: min(480px, 92vw);
    padding: 14px 16px 12px;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--fg-primary);
    font-size: 12px;
  }
  h2 {
    margin: 0 0 10px;
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
  }
  .field {
    width: 100%;
    box-sizing: border-box;
    padding: 6px 8px;
    background: var(--bg-inset);
    border: 1px solid var(--border);
    border-radius: 2px;
    color: var(--fg-primary);
    font: inherit;
    font-family: var(--mono, ui-monospace, monospace);
  }
  .field:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .path {
    margin: 6px 0 0;
    color: var(--fg-faint);
    font-size: 11px;
    overflow-wrap: anywhere;
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
