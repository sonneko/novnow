<script lang="ts" context="module">
  // アイデアの即時入力（`Ctrl+Shift+I`、§10.4・§9.7.5）。
  //
  // 画面遷移はしない重ね物。1 行を書いて Enter で `ideas.md` に積み、閉じても下書きは残す
  // （§9.7.5 の下書き規則。閉じたあとで開き直せば続きから書ける）。下書きは画面を開き直しても
  // 残るように、モジュールに持つ。
  import { writable } from 'svelte/store';

  /** 下書き（送るまで残す）。 */
  export const draft = writable('');
</script>

<script lang="ts">
  import { tick } from 'svelte';
  import type { Session } from '../../store/session';
  import type { Texts } from '../../i18n';
  import { captureTitle } from './ideas';

  /** 開いているか。 */
  export let open = false;
  /** 文言。 */
  export let t: Texts['t'];
  /** セッション。 */
  export let session: Session;
  /** 閉じる（Esc・外側クリック）。下書きは残す。 */
  export let onclose: (() => void) | null = null;
  /** 積んだ（一覧を読み直す合図）。 */
  export let oncaptured: (() => void) | null = null;

  let field: HTMLInputElement | null = null;
  let composing = false;

  $: if (open) {
    void tick().then(() => field?.focus());
  }

  /** 1 行を積む。変換中の Enter は確定だけに使う（§9.8.5）。 */
  async function submit(event: KeyboardEvent): Promise<void> {
    if (event.isComposing || composing || event.keyCode === 229) {
      return;
    }
    event.preventDefault();
    const title = captureTitle($draft);
    if (title === null) {
      return;
    }
    const outcome = await session.run('idea.capture', { title, role: '' });
    if (!outcome.ok) {
      session.notice(outcome.message, 'error');
      return;
    }
    session.notice(t('idea.captured', [title]));
    draft.set('');
    oncaptured?.();
    onclose?.();
  }

  /** `Esc` で閉じる（下書きは残す）。 */
  function onkey(event: KeyboardEvent): void {
    if (event.key === 'Escape' && !event.isComposing) {
      event.preventDefault();
      onclose?.();
    }
  }

  /** 外側（背景）を押したら閉じる（§9.7.5）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      onclose?.();
    }
  }

</script>

{#if open}
  <div class="scrim" role="presentation" on:click={outside} on:keydown={onkey}>
    <div class="sheet" role="dialog" aria-modal="true" aria-label={t('idea.capture_title')} tabindex="-1">
      <h2>{t('idea.capture_title')}</h2>
      <label>
        <span class="hint">{t('idea.capture_hint')}</span>
        <input
          bind:this={field}
          type="text"
          value={$draft}
          on:input={(event) => draft.set((event.currentTarget as HTMLInputElement).value)}
          on:keydown={submit}
          on:compositionstart={() => (composing = true)}
          on:compositionend={() => (composing = false)}
        />
      </label>
      <p class="close-hint">{t('modal.close_hint')}</p>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 15vh;
    background: color-mix(in srgb, var(--bg-base) 66%, transparent);
  }
  .sheet {
    width: min(560px, 90vw);
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-raised);
    color: var(--fg-primary);
    box-shadow: 0 16px 48px rgb(0 0 0 / 32%);
  }
  h2 {
    margin: 0 0 8px;
    font-size: 16px;
  }
  .hint,
  .close-hint {
    display: block;
    font-size: 12px;
    color: var(--fg-muted);
  }
  input {
    box-sizing: border-box;
    width: 100%;
    min-height: 32px;
    margin-top: 6px;
    padding: 4px 8px;
    font: inherit;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-inset);
    color: var(--fg-primary);
  }
  .close-hint {
    margin-top: 8px;
  }
</style>
