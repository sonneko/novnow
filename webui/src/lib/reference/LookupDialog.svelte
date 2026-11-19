<script lang="ts">
  // 参照の面（`cast.get` / `dictionary.list` / `dictionary.term_full` / `diagnostics.place`）。
  //
  // 4 つの操作は「参照ペインの行から開く」1 つの面にまとめる。開く口は 3 つ:
  //
  // 1. 参照ペインの**配役**の行 → `cast.get`（その人物がいる場面のキャラ情報）
  // 2. 参照ペインの**用語**の行 → `dictionary.term_full`（`Ctrl+Shift+W` と同じ全文表示。§9.5.5）
  // 3. ヘッダの**辞書**のボタン（`Ctrl+D` の辞書）→ `dictionary.list`（辞書の一覧）
  //
  // `diagnostics.place` は、この面の**診断**のタブから呼ぶ（いまの文書の byte 位置の「どこ」）。
  //
  // 意味は機能核が決める（`src/app/ops/knowledge.rs` / `lang.rs`）ので、ここは「いつ呼ぶか」と
  // 「返ってきたものをどう並べるか」だけを持つ。文言は生成物 `i18n/catalog.ts` に実在するキー
  // だけを引く（§12.3）。`Esc` と外側クリックで閉じる（§9.7.5）。

  import type { Texts } from '../i18n';
  import {
    castEntries,
    castFieldLabel,
    dictionaryEntries,
    dictionaryKindLabel,
    filterDictionary,
    termHits,
    type DictionaryEntry,
    type LookupMode,
  } from './lookups';

  /** 文言。 */
  export let t: Texts['t'];
  /** 出す面（`null` なら閉じている）。 */
  export let mode: LookupMode | null = null;
  /** 見出しに添える対象（場面 ID・語・資料の題など）。 */
  export let subject = '';
  /** `cast.get` の結果（`{ cast }`）。 */
  export let cast: Record<string, unknown> | null = null;
  /** `dictionary.list` の結果（`{ count, entries }`）。 */
  export let dictionary: Record<string, unknown> | null = null;
  /** `dictionary.term_full` の結果（`{ words }`）。 */
  export let terms: Record<string, unknown> | null = null;
  /** 失敗の文言（機能核が返した文。空なら出さない）。 */
  export let failure = '';
  /** 読み込み中。 */
  export let busy = false;
  /** 閉じる。 */
  export let onclose: (() => void) | null = null;
  /** 一覧の語で絞り込んだ（打鍵ごと）。 */
  export let onquery: ((query: string) => void) | null = null;
  /** 用語の定義を全文で開く（`words.md` の `document.get`）。 */
  export let onfull: ((term: string) => void) | null = null;

  /** 一覧の絞り込み語。 */
  let query = '';

  /** 表示する見出し（`api.op.*` の文言をそのまま使う）。 */
  $: title =
    mode === 'cast'
      ? t('api.op.cast.get')
      : mode === 'dictionary'
        ? t('api.op.dictionary.list')
        : t('api.op.dictionary.term_full');

  /** 配役（`cast.get` のテキストのブロックを読む）。 */
  $: castRows = castEntries(cast?.['cast']);

  /** 辞書の全件（並べ替えない。絞り込みだけ画面で行う）。 */
  $: allEntries = dictionaryEntries(dictionary?.['entries']);

  /** 用語（`dictionary.term_full` の `words`）。 */
  $: termRows = termHits(terms?.['words']);

  /** 背景（外側）を押したら閉じる（§9.7.5）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      close();
    }
  }

  function close(): void {
    query = '';
    onclose?.();
  }

  /** 見出しの文言（知らない見出しはそのまま出す）。 */
  function fieldLabel(key: string): string {
    const found = castFieldLabel(key);
    return found === '' ? key : t(found);
  }

  /** 種別の文言（知らない種別はそのまま出す）。 */
  function kindLabel(kind: string): string {
    const found = dictionaryKindLabel(kind);
    return found === '' ? kind : t(found);
  }

  /** `dictionary.list` を絞り込んだ結果（`entries` が無ければ空）。 */
  function shown(entries: readonly DictionaryEntry[]): DictionaryEntry[] {
    return filterDictionary(entries, query);
  }

  /** `Esc` で閉じる（`Enter` は欄の意味を変えない）。 */
  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' && !event.isComposing) {
      event.preventDefault();
      close();
    }
  }
</script>

{#if mode}
  <div class="overlay" role="presentation" on:click={outside}>
    <div class="sheet" role="dialog" aria-modal="true" aria-label={title} tabindex="-1" on:keydown={keydown}>
      <h2>{title}</h2>
      {#if subject}
        <p class="subject">{subject}</p>
      {/if}

      {#if busy}
        <p class="hint" role="status">{t('project.pick.loading')}</p>
      {:else if failure}
        <p class="failure" role="alert">{failure}</p>
      {:else if mode === 'cast'}
        {#if castRows.length === 0}
          <p class="empty">{t('cast.none')}</p>
        {:else}
          <div class="groups">
            {#each castRows as row, at (at)}
              <section class="group">
                <h3>{row.name === '' ? t('stage.unknown') : row.name}</h3>
                <dl>
                  {#each row.fields as field}
                    {#if field.key !== 'name'}
                      <dt>{fieldLabel(field.key)}</dt>
                      <dd>{field.value}</dd>
                    {/if}
                  {/each}
                </dl>
              </section>
            {/each}
          </div>
        {/if}
      {:else if mode === 'dictionary'}
        <label class="search">
          <span class="sr">{t('api.op.dictionary.list')}</span>
          <input
            type="search"
            bind:value={query}
            placeholder={t('search.placeholder')}
            on:input={() => onquery?.(query)}
            spellcheck="false"
            autocomplete="off"
          />
        </label>
        {@const rows = shown(allEntries)}
        {#if allEntries.length === 0}
          <p class="empty">{t('palette.no_matches')}</p>
        {:else if rows.length === 0}
          <p class="empty">{t('palette.no_matches')}</p>
        {:else}
          <ul class="entries">
            {#each rows as entry, at (`${entry.kind}:${entry.key}:${at}`)}
              <li>
                <span class="kind">{kindLabel(entry.kind)}</span>
                <span class="key">{entry.key}</span>
              </li>
            {/each}
          </ul>
        {/if}
      {:else}
        {#if termRows.length === 0}
          <p class="empty">{t('definition.none')}</p>
        {:else}
          <div class="groups">
            {#each termRows as row, at (`${row.term}:${at}`)}
              <section class="group">
                <h3>{row.term}</h3>
                {#if row.definition}
                  <p class="definition">{row.definition}</p>
                {:else}
                  <p class="empty">{t('definition.none')}</p>
                {/if}
                <!-- 定義の全文（`words.md` の全文を開く）。語が空なら出さない（押せない行を作らない）。 -->
                {#if onfull && row.term !== ''}
                  <button type="button" class="link" on:click={() => onfull(row.term)}>
                    {t('help.term')}
                  </button>
                {/if}
              </section>
            {/each}
          </div>
        {/if}
      {/if}

      <div class="actions">
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
  }
  .search {
    display: block;
    margin: 4px 0 8px;
  }
  .search input {
    box-sizing: border-box;
    width: 100%;
    padding: 4px 6px;
    background: var(--bg-inset);
    border: 1px solid var(--border);
    border-radius: 2px;
    color: var(--fg-primary);
    font: inherit;
  }
  .search input:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .group {
    padding: 6px 0 8px;
    border-bottom: 1px solid var(--border);
  }
  .group:last-child {
    border-bottom: 0;
  }
  h3 {
    margin: 0 0 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-primary);
  }
  dl {
    display: grid;
    grid-template-columns: 6em 1fr;
    gap: 2px 8px;
    margin: 0;
  }
  dt {
    color: var(--fg-faint);
    font-size: 11px;
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .entries {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .entries li {
    display: flex;
    gap: 8px;
    padding: 2px 0;
  }
  .entries .kind {
    min-width: 5em;
    color: var(--fg-faint);
    font-size: 11px;
  }
  .entries .key {
    overflow-wrap: anywhere;
  }
  .definition {
    margin: 0 0 4px;
    white-space: pre-wrap;
    line-height: 1.6;
  }
  .link {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: 11px;
    text-decoration: underline;
    cursor: pointer;
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
