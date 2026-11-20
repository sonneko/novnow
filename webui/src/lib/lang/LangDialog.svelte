<script lang="ts">
  // `lang.*`（LSP 相当の操作群）の結果の面（§9.7.1 の `>` パレットから開く）。
  //
  // 8 つの操作（`lang.format` は実行して通知に出すのでここへ来ない）の結果を、機能核が
  // 返した形のまま並べる。意味は機能核が決める（`src/app/ops/lang.rs`）ので、ここは
  // 「どう並べるか」と「ジャンプ・適用の口」だけを持つ。
  //
  // - `lang.definition` / `lang.references` … 行を押してジャンプ（`onjump`）。
  // - `lang.rename_apply` … 新しい名前の入力欄と「適用」（`onapply`）。プレビュー
  //   （`lang.rename_preview`）と対になる（§9.7）。
  //
  // `Esc` と外側クリックで閉じる（§9.7.5）。文言は生成物 `i18n/catalog.ts` に実在する
  // キーだけを引く（§12.3）。

  import type { Texts } from '../i18n';
  import type { LangRow } from './langOps';

  /** 文言。 */
  export let t: Texts['t'];
  /** 出している操作（操作 ID。`null` なら閉じている）。 */
  export let mode: string | null = null;
  /** 見出しに添える対象（文書のパス）。 */
  export let subject = '';
  /** 表示する行。 */
  export let rows: LangRow[] = [];
  /** 件数（ある操作だけ）。 */
  export let count: number | null = null;
  /** 失敗の文言（機能核が返した文。空なら出さない）。 */
  export let failure = '';
  /** 読み込み中。 */
  export let busy = false;
  /** 閉じる。 */
  export let onclose: (() => void) | null = null;
  /** 行を押した（定義・参照のジャンプ）。 */
  export let onjump: ((value: unknown) => void) | null = null;
  /** リネームを適用する（`lang.rename_apply` の `to`）。失敗したら文言を返す。 */
  export let onapply: ((to: string) => Promise<string>) | null = null;

  /** `lang.rename_apply` の入力（新しい名前）。 */
  let value = '';
  /** 入力欄（開いたら focus する）。 */
  let field: HTMLInputElement | null = null;

  /** 見出し（`api.op.lang.*` の文言をそのまま使う）。 */
  $: title = mode ? t(`api.op.${mode}`) : '';
  /** リネームの適用面か。 */
  $: isApply = mode === 'lang.rename_apply';
  /** 行をジャンプへ使える操作か。 */
  $: jumpable = mode === 'lang.definition' || mode === 'lang.references';

  /** 空のときの文言（操作ごとに分ける）。 */
  $: emptyText = (() => {
    if (mode === 'lang.hover') return t('lang.none_hover');
    if (mode === 'lang.references') return t('lang.none_references');
    if (mode === 'lang.symbols') return t('lang.none_symbols');
    if (mode === 'lang.folding') return t('lang.none_folding');
    if (mode === 'lang.rename_preview') return t('lang.none_rename');
    return t('lang.empty');
  })();

  /** 背景（外側）を押したら閉じる（§9.7.5）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      close();
    }
  }

  function close(): void {
    value = '';
    onclose?.();
  }

  /** リネームを適用する。 */
  async function apply(): Promise<void> {
    if (busy) {
      return;
    }
    busy = true;
    try {
      const message = (await onapply?.(value)) ?? '';
      if (message) {
        failure = message;
        return;
      }
      close();
    } finally {
      busy = false;
    }
  }

  /** `Enter` で適用（入力欄のとき）、`Esc` で閉じる（重ね物の共通規則）。 */
  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' && !event.isComposing) {
      event.preventDefault();
      close();
      return;
    }
    if (isApply && event.key === 'Enter' && !event.isComposing) {
      event.preventDefault();
      void apply();
    }
  }
</script>

{#if mode}
  <div class="overlay" role="presentation" on:click={outside}>
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      tabindex="-1"
      on:keydown={keydown}
    >
      <h2>{title}</h2>
      {#if subject}
        <p class="subject">{subject}</p>
      {/if}

      {#if busy}
        <p class="hint" role="status">{t('project.pick.loading')}</p>
      {:else if failure}
        <p class="failure" role="alert">{failure}</p>
      {:else if isApply}
        <!-- リネームの適用（`lang.rename_apply`）。新しい名前を入力して適用する。 -->
        <label class="field-label">
          <span class="sr">{t('lang.to')}</span>
          <input
            bind:this={field}
            bind:value
            type="text"
            class="field"
            placeholder={t('lang.to')}
            spellcheck="false"
            autocomplete="off"
            on:keydown={keydown}
          />
        </label>
      {:else if rows.length === 0}
        <p class="empty">{emptyText}</p>
      {:else}
        {#if count !== null}
          <p class="count">{t('find.count', [String(count)])}</p>
        {/if}
        <ul class="rows">
          {#each rows as row, at (at)}
            <li>
              {#if jumpable && onjump}
                <button
                  type="button"
                  class="row jump"
                  title={t('lang.jump')}
                  on:click={() => onjump?.(row.value)}
                >
                  <span class="line">{row.text}</span>
                  {#if row.detail}
                    <span class="detail">{row.detail}</span>
                  {/if}
                </button>
              {:else}
                <div class="row">
                  <span class="line">{row.text}</span>
                  {#if row.detail}
                    <span class="detail">{row.detail}</span>
                  {/if}
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}

      <div class="actions">
        {#if isApply}
          <button type="button" class="button primary" disabled={busy} on:click={() => void apply()}>
            {busy ? t('status.saving') : t('lang.apply')}
          </button>
        {/if}
        <button type="button" class="button" on:click={close}>{t('modal.close')}</button>
      </div>
      <p class="close-hint">{t('modal.close_hint')}</p>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 42;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 64px;
    background: rgb(0 0 0 / 35%);
  }
  .sheet {
    width: min(620px, 92vw);
    max-height: 72vh;
    overflow: auto;
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
  .subject {
    margin: 0 0 8px;
    color: var(--fg-faint);
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .count {
    margin: 0 0 6px;
    color: var(--fg-faint);
    font-size: 11px;
  }
  .field-label {
    display: block;
    margin: 4px 0 8px;
  }
  .field {
    box-sizing: border-box;
    width: 100%;
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
  .rows {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .rows li {
    padding: 0;
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
    padding: 3px 4px;
    cursor: default;
    border-radius: 2px;
  }
  .row.jump {
    cursor: pointer;
  }
  .row.jump:hover {
    background: var(--bg-selection);
  }
  .line {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .detail {
    color: var(--fg-faint);
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 11px;
  }
  .empty,
  .hint {
    margin: 8px 0;
    color: var(--fg-faint);
    font-size: 12px;
  }
  .failure {
    margin: 8px 0;
    color: var(--state-error);
    font-size: 12px;
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
  .button.primary {
    border-color: var(--accent);
    color: var(--accent);
  }
  .close-hint {
    margin: 10px 0 0;
    color: var(--fg-faint);
    font-size: 11px;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
