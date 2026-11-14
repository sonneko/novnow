<script lang="ts">
  // `idea` ワークスペース（§10.4）。2 カラム: 左 = 絞り込みと一覧（仮想化）、右 = アイデアの本文。
  //
  // - 規則（絞り込み・次の行・本文の切り出し・昇格先）は `ideas.ts` にある。ここは描画と操作の配線だけ。
  // - 読むのは `idea.list`（行と範囲）と `document.get`（`ideas.md` の全文）。書くのは `idea.write` /
  //   `idea.promote` / `idea.drop`。`ideas.md` を自分で解釈しない（§21.1.3）。
  // - 書き込みは `role` を空で送る（webui は MCP ではないので関門を通らない。`App.svelte` と同じ）。
  // - 本文の打鍵は 300ms 待ってから `idea.write` に送る（IME の変換中は送らない。§9.8.5）。
  import { onDestroy, tick } from 'svelte';
  import { chord, strokeOf } from '../../keyboard/keys';
  import type { Session } from '../../store/session';
  import type { Texts } from '../../i18n';
  import {
    IDEA_FILTERS,
    IDEAS_PATH,
    ROW_HEIGHT,
    bodyOf,
    currentRow,
    filterLabelKey,
    ideaAction,
    moveSelection,
    nextAfterDrop,
    parseRows,
    promoteChoices,
    revealOffset,
    statusLabelKey,
    visibleRows,
    windowOf,
    type IdeaFilter,
    type IdeaRow,
    type PromoteChoice,
  } from './ideas';

  /** 文言。 */
  export let t: Texts['t'];
  /** セッション（正規操作の口）。 */
  export let session: Session;
  /** 今の節（`Ctrl+1..` の文脈。空なら本文で場面を開いていない）。 */
  export let scene = '';
  /** 一覧の再読込を求められた（即時入力などから）。値が変わると読み直す。 */
  export let revision = 0;

  let rows: IdeaRow[] = [];
  let text = '';
  let characters: { index: number; name: string }[] = [];
  let filter: IdeaFilter = 'active';
  let tag = '';
  let selectedId: string | null = null;
  let body = '';
  let promoteOpen = false;
  let composing = false;
  let root: HTMLElement | null = null;
  let scroller: HTMLDivElement | null = null;
  let scrollTop = 0;
  let viewport = 0;
  let writeTimer: ReturnType<typeof setTimeout> | null = null;
  let dirty = false;
  let loadedRevision = -1;

  $: visible = visibleRows(rows, filter, tag);
  $: current = currentRow(visible, selectedId);
  $: span = windowOf(visible.length, scrollTop, viewport);
  $: choices = promoteChoices(scene, characters);

  /** 一覧と本文を読み直す（`ideas.md` が変わったあと）。選んでいた行が消えたら先頭を選ぶ。 */
  export async function reload(): Promise<void> {
    const list = await session.run('idea.list');
    const doc = await session.run('document.get', { path: IDEAS_PATH });
    if (list.ok) {
      rows = parseRows(list.result.ideas);
    } else {
      session.notice(list.message, 'error');
    }
    if (doc.ok) {
      text = doc.result.text;
    }
    const shown = visibleRows(rows, filter, tag);
    if (!shown.some((row) => row.id === selectedId)) {
      select(shown[0]?.id ?? null);
    }
  }

  /** 人物の一覧（昇格先に使う）。読めなければ空のまま（昇格先に出ないだけ）。 */
  async function loadCharacters(): Promise<void> {
    const found = await session.run('cast.list');
    if (found.ok) {
      characters = parseCharacters(found.result.characters);
    }
  }

  /** `cast.list` の人物を読む（形が壊れた要素は捨てる）。 */
  function parseCharacters(found: unknown): { index: number; name: string }[] {
    const out: { index: number; name: string }[] = [];
    if (!Array.isArray(found)) {
      return out;
    }
    for (const raw of found) {
      const item = raw as { index?: unknown; name?: unknown } | null;
      if (item && typeof item.index === 'number' && typeof item.name === 'string') {
        out.push({ index: item.index, name: item.name });
      }
    }
    return out;
  }

  /** 選ぶ。本文の未送信分は先に送る。 */
  function select(id: string | null): void {
    if (id === selectedId) {
      return;
    }
    void flush();
    selectedId = id;
    promoteOpen = false;
    const row = rows.find((found) => found.id === id);
    body = row ? bodyOf(text, row) : '';
    dirty = false;
    void tick().then(reveal);
  }

  /** 選んだ行が見える位置までだけ動かす（§9.7.7。中央寄せはしない）。 */
  function reveal(): void {
    const index = visible.findIndex((row) => row.id === selectedId);
    if (index === -1 || !scroller) {
      return;
    }
    const next = revealOffset(index, scroller.scrollTop, scroller.clientHeight);
    if (next !== scroller.scrollTop) {
      scroller.scrollTop = next;
      scrollTop = next;
    }
  }

  /** 本文の打鍵を書き込む（待ちを置く。変換中は置かない）。 */
  function edit(next: string): void {
    body = next;
    dirty = true;
    if (writeTimer !== null) {
      clearTimeout(writeTimer);
    }
    writeTimer = setTimeout(() => void flush(), 300);
  }

  /** 未送信の本文を `idea.write` へ送り、一覧（行の範囲）を読み直す。 */
  async function flush(): Promise<void> {
    if (writeTimer !== null) {
      clearTimeout(writeTimer);
      writeTimer = null;
    }
    if (!dirty || composing || selectedId === null) {
      return;
    }
    const id = selectedId;
    const sent = body;
    dirty = false;
    const outcome = await session.run('idea.write', { id, text: sent, role: '' });
    if (!outcome.ok) {
      dirty = true;
      session.notice(outcome.message, 'error');
      return;
    }
    await reload();
  }

  /** 昇格する（本文を先に送ってから）。 */
  async function promote(choice: PromoteChoice): Promise<void> {
    const target = current;
    if (target === null || !choice.available) {
      return;
    }
    const id = target.id;
    await flush();
    // `to` が空なら行き先の既定（今の節・先頭の人物）になる。`idea.promote` の引数は空文字を省略と同じに読む。
    const outcome = await session.run('idea.promote', {
      id,
      target: choice.target,
      to: choice.to ?? '',
      role: '',
    });
    if (!outcome.ok) {
      session.notice(t('idea.promote_failed'), 'error');
      return;
    }
    session.notice(t('idea.promoted', [id, String(outcome.result.target)]));
    promoteOpen = false;
    await reload();
  }

  /** 捨てる（行は消さず `status: dropped`。次の行を選ぶ）。 */
  async function drop(): Promise<void> {
    const dropped = current;
    if (!dropped) {
      return;
    }
    await flush();
    const outcome = await session.run('idea.drop', { id: dropped.id, role: '' });
    if (!outcome.ok) {
      session.notice(outcome.message, 'error');
      return;
    }
    session.notice(t('idea.dropped', [dropped.id]));
    await reload();
    const after = visibleRows(rows, filter, tag);
    select(nextAfterDrop(after, dropped.index));
  }

  /** キー（この画面が横取りするものだけ。残りは全体の割当に任せる）。 */
  function onkey(event: KeyboardEvent): void {
    // IME の変換中は一切のショートカットを抑える（§9.8.5 MUST）。
    if (event.isComposing || event.keyCode === 229) {
      return;
    }
    // フォーカスがこの画面の中に無いときは触らない（別の重ね物や画面のキーを奪わない）。
    if (!root || !(event.target instanceof Node) || !root.contains(event.target)) {
      return;
    }
    const action = ideaAction(chord(strokeOf(event)));
    if (action === null) {
      return;
    }
    // 本文の編集中は、`Ctrl+Home/End` などの文字の移動を残す。横取りするのは画面の操作だけ。
    if (action === 'first' || action === 'last') {
      if (event.target instanceof HTMLTextAreaElement) {
        return;
      }
    }
    event.preventDefault();
    switch (action) {
      case 'promote':
        promoteOpen = !promoteOpen;
        break;
      case 'drop':
        void drop();
        break;
      case 'prev':
        select(moveSelection(visible, selectedId, 'prev'));
        break;
      case 'next':
        select(moveSelection(visible, selectedId, 'next'));
        break;
      case 'first':
        select(moveSelection(visible, selectedId, 'first'));
        break;
      case 'last':
        select(moveSelection(visible, selectedId, 'last'));
        break;
      case 'focus-editor':
        document.getElementById('idea-body')?.focus();
        break;
    }
  }

  /** 一覧の状態の表示（色だけに頼らず、文字で出す。§9.9）。 */
  function statusText(row: IdeaRow): string {
    return t(statusLabelKey(row.status));
  }

  // 開いたとき（初回を含む）と、読み直しの合図が変わったとき。
  $: if (revision !== loadedRevision) {
    loadedRevision = revision;
    void reload();
  }
  void loadCharacters();

  onDestroy(() => {
    void flush();
  });
</script>

<svelte:window on:keydown={onkey} />

<section class="idea" aria-label={t('header.workspace.idea')} bind:this={root}>
  <div class="list-pane">
    <div class="filters" role="group" aria-label={t('idea.filter_active')}>
      {#each IDEA_FILTERS as option}
        <button
          type="button"
          class="chip"
          class:on={filter === option}
          aria-pressed={filter === option}
          on:click={() => (filter = option)}>{t(filterLabelKey(option))}</button
        >
      {/each}
    </div>
    <label class="tag">
      <span class="sr">{t('idea.tag_filter')}</span>
      <input type="search" bind:value={tag} placeholder={t('idea.tag_filter')} />
    </label>
    {#if visible.length === 0}
      <p class="empty">{t('idea.none')}</p>
    {:else}
      <div
        class="scroll"
        bind:this={scroller}
        bind:clientHeight={viewport}
        on:scroll={(event) => (scrollTop = (event.currentTarget as HTMLDivElement).scrollTop)}
      >
        <ul role="listbox" aria-label={t('idea.filter_active')} style:height="{visible.length * ROW_HEIGHT}px">
          {#each visible.slice(span.start, span.end) as row, offset (row.id)}
            <li
              role="presentation"
              class="item"
              style:top="{(span.start + offset) * ROW_HEIGHT}px"
              style:height="{ROW_HEIGHT}px"
            >
              <button
                type="button"
                role="option"
                aria-selected={current?.id === row.id}
                class="row"
                class:selected={current?.id === row.id}
                class:dropped={row.status === 'dropped'}
                on:click={() => select(row.id)}
              >
                <span class="status">{statusText(row)}</span>
                <span class="title">{row.title || row.id}</span>
                {#if row.tags.length > 0}
                  <span class="tags">{row.tags.join(', ')}</span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>

  <div class="edit-pane">
    {#if current === null}
      <p class="empty">{t('idea.empty')}</p>
    {:else}
      <header>
        <h2>{current.title || current.id}</h2>
        <p class="meta">
          <span>{current.id}</span>
          <span>{statusText(current)}</span>
          <span class="keys">{t('idea.key_promote')} · {t('idea.key_drop')}</span>
        </p>
        <div class="actions">
          <button type="button" aria-expanded={promoteOpen} on:click={() => (promoteOpen = !promoteOpen)}
            >{t('idea.promote_title')}</button
          >
          <button type="button" class="danger" on:click={() => void drop()}>{t('idea.key_drop')}</button>
        </div>
        {#if promoteOpen}
          <ul class="promote" aria-label={t('idea.promote_title')}>
            {#each choices as choice}
              <li>
                <button
                  type="button"
                  disabled={!choice.available}
                  on:click={() => void promote(choice)}
                  >{t(choice.labelKey, choice.labelArgs)}</button
                >
              </li>
            {/each}
          </ul>
        {/if}
      </header>
      <label class="body-label" for="idea-body">{current.title || current.id}</label>
      <textarea
        id="idea-body"
        class="body"
        value={body}
        on:input={(event) => edit((event.currentTarget as HTMLTextAreaElement).value)}
        on:compositionstart={() => (composing = true)}
        on:compositionend={() => {
          composing = false;
          edit(body);
        }}
      ></textarea>
    {/if}
  </div>
</section>

<style>
  .idea {
    display: grid;
    grid-template-columns: minmax(220px, 38%) 1fr;
    height: 100%;
    min-height: 0;
    background: var(--bg-base);
    color: var(--fg-primary);
  }
  .list-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 8px;
  }
  .chip,
  .actions button,
  .promote button {
    min-height: 24px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-raised);
    color: var(--fg-primary);
    cursor: pointer;
  }
  .chip.on {
    border-color: var(--accent);
    background: var(--bg-selection);
  }
  .tag {
    display: block;
    padding: 0 8px 8px;
  }
  .tag input {
    width: 100%;
    font: inherit;
    padding: 4px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-inset);
    color: var(--fg-primary);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    position: relative;
  }
  ul[role='listbox'] {
    position: relative;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    position: absolute;
    left: 0;
    right: 0;
    margin: 0;
    padding: 0;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    width: 100%;
    height: 100%;
    padding: 0 8px;
    border: none;
    background: transparent;
    color: var(--fg-primary);
    text-align: left;
    cursor: pointer;
    font: inherit;
  }
  .row:hover {
    background: var(--bg-raised);
  }
  .row.selected {
    background: var(--bg-selection);
  }
  .row.dropped {
    color: var(--fg-faint);
  }
  .status {
    flex: none;
    font-size: 12px;
    color: var(--fg-muted);
    text-decoration: underline dotted;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tags {
    margin-left: auto;
    font-size: 12px;
    color: var(--fg-faint);
    white-space: nowrap;
  }
  .edit-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 12px;
  }
  header h2 {
    margin: 0 0 4px;
    font-size: 17px;
  }
  .meta {
    display: flex;
    gap: 12px;
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--fg-muted);
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .danger {
    color: var(--state-error);
  }
  .promote {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 8px 0;
    padding: 0;
    list-style: none;
  }
  .promote button:disabled {
    color: var(--fg-faint);
    cursor: not-allowed;
  }
  .body-label {
    font-size: 12px;
    color: var(--fg-muted);
  }
  .body {
    flex: 1;
    min-height: 0;
    width: 100%;
    margin-top: 4px;
    padding: 8px;
    font: inherit;
    font-family: var(--mono);
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-inset);
    color: var(--fg-primary);
    resize: none;
  }
  .empty {
    padding: 16px;
    color: var(--fg-muted);
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
