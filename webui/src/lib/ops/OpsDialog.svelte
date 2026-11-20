<script lang="ts">
  // バッチ6 の操作群（監査ログ・プロンプト・メール・ワークスペース・ヘルプ）の結果の面。
  // バッチ5 の `LangDialog.svelte` と同じ方針：パレット（`>`）から操作 ID を選んで開き、
  // 結果を機能核が返した形のまま並べる（意味は機能核が決める）。
  //
  // - 引数不要な操作（`agent.audit_list` / `prompt.list` / `session.workspace_get`）は
  //   開いたら即実行する。
  // - 引数のある操作は入力フォームを出して「実行 / 送る」で呼ぶ：
  //   `prompt.get/diff/reset` の `name`、`mail.send` の `to/kind/scope/body`、
  //   `help.topic` の `topic`。
  //
  // `prompt.reset` は安全面の例外（`{M: safety}`）なので、結果の件数を画面に出す。
  // `mail.send` は承認待ち（`approved_by: pending`）でキューに入ることを文言で示す。
  //
  // `Esc` と外側クリックで閉じる（§9.7.5）。文言は生成物 `i18n/catalog.ts` に実在する
  // キーだけを引く（§12.3）。

  import type { Texts } from '../i18n';
  import type { OpsId, OpsInput } from './opsOps';

  /** 文言。 */
  export let t: Texts['t'];
  /** 出している操作（操作 ID。`null` なら閉じている）。 */
  export let mode: OpsId | null = null;
  /** 見出しに添える対象（文書のパス・プロンプト名など）。 */
  export let subject = '';
  /** 表示する行（一覧系の操作）。 */
  export let rows: { text: string; detail: string }[] = [];
  /** 表示する本文（`prompt.get` / `help.topic` など本文系の操作）。 */
  export let text = '';
  /** 件数（ある操作だけ）。 */
  export let count: number | null = null;
  /** 結果の文言（`prompt.reset` / `mail.send` の成功文）。 */
  export let result = '';
  /** 失敗の文言（機能核が返した文。空なら出さない）。 */
  export let failure = '';
  /** 読み込み中。 */
  export let busy = false;
  /** 閉じる。 */
  export let onclose: (() => void) | null = null;
  /** 実行する（引数を渡す。失敗したら文言を返す）。 */
  export let onrun: ((input: OpsInput) => Promise<string>) | null = null;

  /** `prompt.get/diff/reset` の `name` 入力。 */
  let name = '';
  /** `mail.send` の入力。 */
  let mailTo = '';
  let mailKind = 'note';
  let mailScope = '';
  let mailBody = '';
  /** `help.topic` の `topic` 入力。 */
  let topic = 'shortcuts';
  /** 入力欄（開いたら focus する）。 */
  let field: HTMLInputElement | null = null;

  /** 見出し（`api.op.*` の文言をそのまま使う）。 */
  $: title = mode ? t(`api.op.${mode}`) : '';
  /** `prompt.*` の 3 つ（`name` 入力）。 */
  $: isPrompt = mode === 'prompt.get' || mode === 'prompt.diff' || mode === 'prompt.reset';
  /** `mail.send`（4 入力）。 */
  $: isMail = mode === 'mail.send';
  /** `help.topic`（`topic` 入力）。 */
  $: isHelp = mode === 'help.topic';
  /** 入力フォームを出す操作か（即実行ではない）。 */
  $: hasForm = isPrompt || isMail || isHelp;

  /** 空のときの文言（操作ごとに分ける）。 */
  $: emptyText = (() => {
    if (mode === 'agent.audit_list') return t('ops.empty_audit');
    if (mode === 'prompt.list') return t('ops.empty_prompt');
    if (mode === 'session.workspace_get') return t('ops.empty_workspace');
    return t('ops.empty');
  })();

  /** 背景（外側）を押したら閉じる（§9.7.5）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      close();
    }
  }

  function close(): void {
    name = '';
    mailTo = '';
    mailKind = 'note';
    mailScope = '';
    mailBody = '';
    topic = 'shortcuts';
    onclose?.();
  }

  /** 入力を集めて実行する（引数不要な操作は `null`）。 */
  async function run(): Promise<void> {
    if (busy) {
      return;
    }
    let input: OpsInput = null;
    if (isPrompt) {
      input = { name };
    } else if (isMail) {
      input = { to: mailTo, kind: mailKind, scope: mailScope, body: mailBody };
    } else if (isHelp) {
      input = { topic };
    }
    busy = true;
    try {
      const message = (await onrun?.(input)) ?? '';
      if (message) {
        failure = message;
      }
    } finally {
      busy = false;
    }
  }

  /** `Enter` で実行（入力欄のとき）、`Esc` で閉じる（重ね物の共通規則）。 */
  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' && !event.isComposing) {
      event.preventDefault();
      close();
      return;
    }
    if (hasForm && event.key === 'Enter' && !event.isComposing) {
      event.preventDefault();
      void run();
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
      {:else if hasForm}
        <!-- 入力フォーム（`prompt.*` / `mail.send` / `help.topic`）。 -->
        {#if isPrompt}
          <label class="field-label">
            <span class="sr">{t('ops.name')}</span>
            <input
              bind:this={field}
              bind:value={name}
              type="text"
              class="field"
              placeholder={t('ops.name_placeholder')}
              spellcheck="false"
              autocomplete="off"
              on:keydown={keydown}
            />
          </label>
        {:else if isHelp}
          <label class="field-label">
            <span class="sr">{t('ops.topic')}</span>
            <input
              bind:this={field}
              bind:value={topic}
              type="text"
              class="field"
              placeholder={t('ops.topics')}
              spellcheck="false"
              autocomplete="off"
              on:keydown={keydown}
            />
          </label>
        {:else if isMail}
          <p class="hint">{t('ops.mail_kinds')}</p>
          <label class="field-label">
            <span class="sr">{t('ops.mail_to')}</span>
            <input
              bind:this={field}
              bind:value={mailTo}
              type="text"
              class="field"
              placeholder={t('ops.mail_to')}
              spellcheck="false"
              autocomplete="off"
              on:keydown={keydown}
            />
          </label>
          <label class="field-label">
            <span class="sr">{t('ops.mail_kind')}</span>
            <input
              bind:value={mailKind}
              type="text"
              class="field"
              placeholder={t('ops.mail_kind')}
              spellcheck="false"
              autocomplete="off"
              on:keydown={keydown}
            />
          </label>
          <label class="field-label">
            <span class="sr">{t('ops.mail_scope')}</span>
            <input
              bind:value={mailScope}
              type="text"
              class="field"
              placeholder={t('ops.mail_scope')}
              spellcheck="false"
              autocomplete="off"
              on:keydown={keydown}
            />
          </label>
          <label class="field-label">
            <span class="sr">{t('ops.mail_body')}</span>
            <input
              bind:value={mailBody}
              type="text"
              class="field"
              placeholder={t('ops.mail_body')}
              spellcheck="false"
              autocomplete="off"
              on:keydown={keydown}
            />
          </label>
        {/if}
      {:else if result}
        <!-- 成功の結果文（`prompt.reset` / `mail.send`）。 -->
        <p class="result" role="status">{result}</p>
      {:else if text}
        <!-- 本文系（`prompt.get` / `help.topic`）。機能核が返した本文を改行維持で。 -->
        <pre class="body">{text}</pre>
      {:else if rows.length === 0}
        <p class="empty">{emptyText}</p>
      {:else}
        {#if count !== null}
          <p class="count">{t('find.count', [String(count)])}</p>
        {/if}
        <ul class="rows">
          {#each rows as row, at (at)}
            <li>
              <div class="row">
                <span class="line">{row.text}</span>
                {#if row.detail}
                  <span class="detail">{row.detail}</span>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="actions">
        {#if hasForm}
          <button
            type="button"
            class="button primary"
            disabled={busy}
            on:click={() => void run()}
          >
            {busy ? t('status.saving') : isMail ? t('ops.send') : t('ops.run')}
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
    padding: 3px 4px;
    border-radius: 2px;
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
  .body {
    margin: 0;
    padding: 8px;
    max-height: 48vh;
    overflow: auto;
    background: var(--bg-inset);
    border: 1px solid var(--border);
    border-radius: 2px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 11px;
  }
  .empty,
  .hint {
    margin: 8px 0;
    color: var(--fg-faint);
    font-size: 12px;
  }
  .result {
    margin: 8px 0;
    color: var(--state-success, var(--fg-primary));
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
