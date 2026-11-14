<script lang="ts">
  // `material` ワークスペース（§10.5・§16）。3 カラム:
  //
  //   左 = 状態の絞り込み・検索・一覧（仮想化。索引に無い資料 MD は先頭）
  //   中央 = 資料 MD の編集（ファイル全体を素のまま。§16.6）
  //   右 = 審査フォーム（タブ「審査」）と、本文からの引用（タブ「引用元」。§16.5）
  //
  // - 規則（絞り込み・検索語・審査の差分・出典の有無）は `materials.ts` にある。ここは配線だけ。
  // - 読むのは `materials.search` / `review.list` / `materials.get` / `document.get`。書くのは
  //   `document.write`（本文）、`materials.update`（審査）、`materials.approve` / `reject` / `propose`。
  // - 書き込みは `role` を空で送る（webui は MCP ではないので関門を通らない。`App.svelte` と同じ）。
  // - 索引に無い資料 MD（W064）の登録は、カタログに専用の正規操作が無いので**未提供**（画面は押せない
  //   形にし、理由を `diag.W064` の文言で示す。§21.1.2 MUST）。
  import { onDestroy, tick } from 'svelte';
  import { chord, strokeOf } from '../../keyboard/keys';
  import type { Session } from '../../store/session';
  import type { Texts } from '../../i18n';
  import {
    MATERIAL_FILTERS,
    ROW_HEIGHT,
    categoryOptions,
    citationQuery,
    draftOf,
    filterLabelKey,
    hasNoSource,
    listOf,
    materialAction,
    moveKey,
    nextKeyAfterRemoval,
    reviewOf,
    rowsOf,
    sameReview,
    searchText,
    statusLabelKey,
    unregisteredOf,
    revealOffset,
    windowOf,
    type MaterialFilter,
    type MaterialRow,
    type Review,
  } from './materials';

  /** 文言。 */
  export let t: Texts['t'];
  /** セッション（正規操作の口）。 */
  export let session: Session;
  /** 読み直しの合図（値が変わると一覧を読み直す）。 */
  export let revision = 0;

  const EMPTY: Review = { title: '', category: '', summary: '', tags: '', conditions: '', source: '' };

  let rows: MaterialRow[] = [];
  let filter: MaterialFilter = 'active';
  let query = '';
  let selected: string | null = null;
  let material: Record<string, unknown> | null = null;
  let body = '';
  let bodyDirty = false;
  let review: Review = { ...EMPTY };
  let applied: Review = { ...EMPTY };
  let reviewDirty = false;
  let confirmApprove = false;
  let creating = false;
  let newTitle = '';
  let tab: 'form' | 'cite' = 'form';
  let citations: { path: string; line: number; text: string }[] = [];
  let root: HTMLElement | null = null;
  let scroller: HTMLDivElement | null = null;
  let scrollTop = 0;
  let viewport = 0;
  let composing = false;
  let bodyTimer: ReturnType<typeof setTimeout> | null = null;
  let reviewTimer: ReturnType<typeof setTimeout> | null = null;
  let queryTimer: ReturnType<typeof setTimeout> | null = null;
  let loadedRevision = -1;

  $: span = windowOf(rows.length, scrollTop, viewport);
  $: current = rows.find((row) => row.key === selected) ?? null;
  $: unregistered = current !== null && current.id === null;
  $: categories = categoryOptions(t('material.categories'), review.category);

  /** 一覧を読み直す。選んでいた行が消えたら、同じ位置の行を選ぶ。 */
  export async function reload(): Promise<void> {
    const hits = await session.run('materials.search', { query: searchText(query, filter) });
    const diagnostics = await session.run('review.list');
    const registered = hits.ok ? rowsOf(hits.result.hits) : [];
    const loose = diagnostics.ok ? unregisteredOf(diagnostics.result.diagnostics) : [];
    rows = listOf(loose, registered, filter, query);
    if (!rows.some((row) => row.key === selected)) {
      select(rows[0]?.key ?? null);
    }
  }

  /** 選ぶ。未送信の本文・審査は先に送る。 */
  function select(key: string | null): void {
    if (key === selected) {
      return;
    }
    void flushAll();
    selected = key;
    confirmApprove = false;
    citations = [];
    void loadSelected();
    void tick().then(reveal);
  }

  /** 選んでいる資料を読む（本文と審査。索引に無ければ MD だけ）。 */
  async function loadSelected(): Promise<void> {
    const row = rows.find((found) => found.key === selected);
    if (!row) {
      material = null;
      body = '';
      review = { ...EMPTY };
      applied = { ...EMPTY };
      return;
    }
    if (row.id !== null) {
      const got = await session.run('materials.get', { id: row.id });
      if (got.ok) {
        body = got.result.body;
        material = got.result.material;
        applied = reviewOf(got.result.review);
        review = { ...applied };
      } else {
        session.notice(got.message, 'error');
      }
    } else {
      const doc = await session.run('document.get', { path: row.path });
      body = doc.ok ? doc.result.text : '';
      if (!doc.ok) {
        session.notice(t('material.missing_file', [row.path]), 'error');
      }
      material = null;
      applied = { ...EMPTY };
      review = { ...EMPTY };
    }
    bodyDirty = false;
    reviewDirty = false;
    if (tab === 'cite') {
      void loadCitations();
    }
  }

  /** 一覧の選ばれた行が見える位置までだけ動かす（§9.7.7）。 */
  function reveal(): void {
    const index = rows.findIndex((row) => row.key === selected);
    if (index === -1 || !scroller) {
      return;
    }
    const next = revealOffset(index, scroller.scrollTop, scroller.clientHeight);
    if (next !== scroller.scrollTop) {
      scroller.scrollTop = next;
      scrollTop = next;
    }
  }

  /** 本文の打鍵（400ms 待って `document.write`。変換中は置かない）。 */
  function editBody(next: string): void {
    body = next;
    bodyDirty = true;
    if (bodyTimer !== null) {
      clearTimeout(bodyTimer);
    }
    bodyTimer = setTimeout(() => void flushBody(), 400);
  }

  async function flushBody(): Promise<void> {
    if (bodyTimer !== null) {
      clearTimeout(bodyTimer);
      bodyTimer = null;
    }
    if (!bodyDirty || composing || current === null) {
      return;
    }
    const sent = body;
    bodyDirty = false;
    const outcome = await session.run('document.write', { path: current.path, text: sent, role: '' });
    if (!outcome.ok) {
      bodyDirty = true;
      session.notice(outcome.message, 'error');
    }
  }

  /** 審査の打鍵（300ms 待って `materials.update`。索引の値が変わったら一覧を読み直す）。 */
  function editReview(next: Partial<Review>): void {
    review = { ...review, ...next };
    reviewDirty = true;
    if (reviewTimer !== null) {
      clearTimeout(reviewTimer);
    }
    reviewTimer = setTimeout(() => void flushReview(), 300);
  }

  async function flushReview(): Promise<void> {
    if (reviewTimer !== null) {
      clearTimeout(reviewTimer);
      reviewTimer = null;
    }
    if (!reviewDirty || composing || current === null || current.id === null) {
      return;
    }
    if (sameReview(review, applied)) {
      reviewDirty = false;
      return;
    }
    const id = current.id;
    const sent = { ...review };
    reviewDirty = false;
    const outcome = await session.run('materials.update', { id, review: sent, role: '' });
    if (!outcome.ok) {
      reviewDirty = true;
      session.notice(outcome.message, 'error');
      return;
    }
    applied = sent;
    await reload();
  }

  /**
   * 本文と審査の未送信分を、まとめて送る。対象（資料・本文・審査）は**呼んだ時点で**取る。
   * 送っている間に別の資料を選んでも、送信先は取り違えない（順に待つと後の送信が新しい資料に当たる）。
   */
  function flushAll(): Promise<void> {
    return Promise.all([flushBody(), flushReview()]).then(() => undefined);
  }

  /** 採用（`inbox/` → `docs/`、`approved`）。出典が無ければ一度確認を挟む（§16.7・W065）。 */
  async function approve(): Promise<void> {
    const target = current;
    if (target === null || target.id === null) {
      return;
    }
    if (!confirmApprove && hasNoSource(material?.source)) {
      confirmApprove = true;
      return;
    }
    const id = target.id;
    await flushAll();
    const outcome = await session.run('materials.approve', { id, role: '' });
    confirmApprove = false;
    if (!outcome.ok) {
      session.notice(outcome.message, 'error');
      return;
    }
    session.notice(t('material.approved', [id]));
    await reload();
    await loadSelected();
  }

  /** 却下（`rejected`。ファイルは inbox に残す。ID は回収しない）。 */
  async function reject(): Promise<void> {
    const target = current;
    if (target === null || target.id === null) {
      return;
    }
    const id = target.id;
    await flushAll();
    const at = rows.findIndex((row) => row.key === id);
    const outcome = await session.run('materials.reject', { id, role: '' });
    if (!outcome.ok) {
      session.notice(outcome.message, 'error');
      return;
    }
    session.notice(t('material.rejected', [id]));
    await reload();
    if (!rows.some((row) => row.key === id)) {
      select(nextKeyAfterRemoval(rows, at));
    } else {
      await loadSelected();
    }
  }

  /** 新しい資料を提案として作る（`Ctrl+Shift+N`）。題名の `#分類` は分類になる。 */
  async function createMaterial(): Promise<void> {
    const parsed = draftOf(newTitle);
    const title = parsed.title === '' ? t('material.untitled') : parsed.title;
    const outcome = await session.run('materials.propose', {
      title,
      category: parsed.category,
      markdown: '',
      source: '',
    });
    if (!outcome.ok) {
      session.notice(outcome.message, 'error');
      return;
    }
    session.notice(t('material.created', [outcome.result.path]));
    creating = false;
    newTitle = '';
    query = '';
    filter = 'active';
    await reload();
    select(outcome.result.id);
  }

  /** 本文の引用（`^M0137`）を探す（右の「引用元」タブ）。 */
  async function loadCitations(): Promise<void> {
    if (current === null || current.id === null) {
      citations = [];
      return;
    }
    const found = await session.run('search.project', { query: citationQuery(current.id) });
    if (!found.ok) {
      citations = [];
      return;
    }
    const hits: unknown[] = Array.isArray(found.result.hits) ? found.result.hits : [];
    citations = hits.flatMap((raw) => {
      const hit = raw as { path?: unknown; line?: unknown; text?: unknown } | null;
      return hit && typeof hit.path === 'string' && typeof hit.line === 'number'
        ? [{ path: hit.path, line: hit.line, text: typeof hit.text === 'string' ? hit.text : '' }]
        : [];
    });
  }

  /** 引用の行へ移る（該当の節を開き、行へ動く）。 */
  async function jumpToCitation(hit: { path: string; line: number }): Promise<void> {
    const file = await session.run('nav.file', { path: hit.path });
    if (!file.ok) {
      session.notice(file.message, 'error');
      return;
    }
    await session.run('nav.jump', { line: hit.line });
  }

  /** 絞り込み・検索語の変更で、一覧を読み直す（検索語は打鍵を待つ）。 */
  function onquery(next: string): void {
    query = next;
    if (queryTimer !== null) {
      clearTimeout(queryTimer);
    }
    queryTimer = setTimeout(() => void reload(), 200);
  }

  function setFilter(next: MaterialFilter): void {
    filter = next;
    void reload();
  }

  /** キー（この画面の中にフォーカスがあるときだけ。IME の変換中は抑える。§9.8.5）。 */
  function onkey(event: KeyboardEvent): void {
    if (event.isComposing || event.keyCode === 229) {
      return;
    }
    if (!root || !(event.target instanceof Node) || !root.contains(event.target)) {
      return;
    }
    const action = materialAction(chord(strokeOf(event)));
    if (action === null) {
      return;
    }
    // 文字の移動（本文の `Ctrl+Home/End`）は残す。
    if ((action === 'first' || action === 'last') && event.target instanceof HTMLTextAreaElement) {
      return;
    }
    event.preventDefault();
    switch (action) {
      case 'approve':
        void approve();
        break;
      case 'reject':
        void reject();
        break;
      case 'new':
        creating = true;
        void tick().then(() => document.getElementById('material-new')?.focus());
        break;
      case 'prev':
        select(moveKey(rows, selected, 'prev'));
        break;
      case 'next':
        select(moveKey(rows, selected, 'next'));
        break;
      case 'first':
        select(moveKey(rows, selected, 'first'));
        break;
      case 'last':
        select(moveKey(rows, selected, 'last'));
        break;
      case 'focus-editor':
        document.getElementById('material-body')?.focus();
        break;
      case 'focus-form':
        tab = 'form';
        document.getElementById('material-title')?.focus();
        break;
    }
  }

  /** 状態の表示（色だけに頼らず文字で出す。§9.9）。索引に無い資料は診断の文言を出す。 */
  function rowStatus(row: MaterialRow): string {
    return row.id === null ? t('diag.W064') : t(statusLabelKey(row.status));
  }

  // 開いたとき（初回を含む）と、読み直しの合図が変わったとき。
  $: if (revision !== loadedRevision) {
    loadedRevision = revision;
    void reload();
  }

  $: if (tab === 'cite' && current) {
    void loadCitations();
  }

  onDestroy(() => {
    void flushAll();
  });
</script>

<svelte:window on:keydown={onkey} />

<section class="material" aria-label={t('header.workspace.material')} bind:this={root}>
  <div class="list-pane">
    <div class="filters" role="group" aria-label={t('material.filter_active')}>
      {#each MATERIAL_FILTERS as option}
        <button
          type="button"
          class="chip"
          class:on={filter === option}
          aria-pressed={filter === option}
          on:click={() => setFilter(option)}>{t(filterLabelKey(option))}</button
        >
      {/each}
    </div>
    <label class="query">
      <span class="sr">{t('material.query_hint')}</span>
      <input
        type="search"
        value={query}
        placeholder={t('material.search_hint')}
        on:input={(event) => onquery((event.currentTarget as HTMLInputElement).value)}
      />
    </label>
    <div class="new">
      {#if creating}
        <input
          id="material-new"
          type="text"
          bind:value={newTitle}
          placeholder={t('material.new_hint')}
          aria-label={t('material.new_title')}
          on:keydown={(event) => {
            if (event.isComposing) {
              return;
            }
            if (event.key === 'Enter') {
              event.preventDefault();
              event.stopPropagation();
              void createMaterial();
            } else if (event.key === 'Escape') {
              event.preventDefault();
              event.stopPropagation();
              creating = false;
            }
          }}
        />
      {:else}
        <button type="button" on:click={() => (creating = true)}>{t('material.new_button')}</button>
      {/if}
    </div>
    {#if rows.length === 0}
      <p class="empty">{t('material.none')}</p>
    {:else}
      <div
        class="scroll"
        bind:this={scroller}
        bind:clientHeight={viewport}
        on:scroll={(event) => (scrollTop = (event.currentTarget as HTMLDivElement).scrollTop)}
      >
        <ul role="listbox" aria-label={t('material.filter_active')} style:height="{rows.length * ROW_HEIGHT}px">
          {#each rows.slice(span.start, span.end) as row, offset (row.key)}
            <li
              role="presentation"
              class="item"
              style:top="{(span.start + offset) * ROW_HEIGHT}px"
              style:height="{ROW_HEIGHT}px"
            >
              <button
                type="button"
                role="option"
                aria-selected={selected === row.key}
                class="row"
                class:selected={selected === row.key}
                class:unregistered={row.id === null}
                on:click={() => select(row.key)}
              >
                <span class="status">{rowStatus(row)}</span>
                <span class="title">{row.title}</span>
                {#if row.category}<span class="cat">{row.category}</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>

  <div class="edit-pane">
    {#if current === null}
      <p class="empty">{t('material.empty')}</p>
    {:else}
      <header>
        <h2>{current.title}</h2>
        <p class="meta">
          <span>{current.id ?? current.path}</span>
          <span>{rowStatus(current)}</span>
          <span class="keys">{t('material.key_approve')} · {t('material.key_reject')}</span>
        </p>
        <div class="actions">
          <button type="button" disabled={unregistered} on:click={() => void approve()}>{t('material.approve')}</button>
          <button type="button" class="danger" disabled={unregistered} on:click={() => void reject()}
            >{t('material.reject')}</button
          >
        </div>
        {#if unregistered}
          <p class="note">{t('diag.W064')}</p>
        {/if}
        {#if confirmApprove}
          <p class="confirm" role="alert">
            <span>{t('material.confirm_approve')}</span>
            <button type="button" on:click={() => void approve()}>{t('material.confirm_yes')}</button>
            <button type="button" on:click={() => (confirmApprove = false)}>{t('modal.cancel')}</button>
          </p>
        {/if}
      </header>
      <label class="body-label" for="material-body">{current.path}</label>
      <textarea
        id="material-body"
        class="body"
        value={body}
        on:input={(event) => editBody((event.currentTarget as HTMLTextAreaElement).value)}
        on:compositionstart={() => (composing = true)}
        on:compositionend={() => {
          composing = false;
          editBody(body);
        }}
      ></textarea>
    {/if}
  </div>

  <aside class="review-pane" aria-label={t('material.key_form')}>
    <div class="tabs" role="tablist">
      <button
        type="button"
        role="tab"
        id="tab-form"
        aria-selected={tab === 'form'}
        aria-controls="panel-form"
        on:click={() => (tab = 'form')}>{t('material.key_form')}</button
      >
      <button
        type="button"
        role="tab"
        id="tab-cite"
        aria-selected={tab === 'cite'}
        aria-controls="panel-cite"
        on:click={() => (tab = 'cite')}>{t('stage.citations')}</button
      >
    </div>
    {#if tab === 'form'}
      <div role="tabpanel" id="panel-form" aria-labelledby="tab-form">
        {#if current === null}
          <p class="empty">{t('material.form_empty')}</p>
        {:else if unregistered}
          <p class="empty">{t('diag.W064')}</p>
        {:else}
          <form class="form" on:submit|preventDefault>
            <label>
              <span>{t('material.field_title')}</span>
              <input
                id="material-title"
                type="text"
                value={review.title}
                on:input={(event) => editReview({ title: (event.currentTarget as HTMLInputElement).value })}
              />
            </label>
            <label>
              <span>{t('material.field_category')}</span>
              <select
                value={review.category}
                on:change={(event) => editReview({ category: (event.currentTarget as HTMLSelectElement).value })}
              >
                {#each categories as option}
                  <option value={option}>{option}</option>
                {/each}
              </select>
            </label>
            <label>
              <span>{t('material.field_summary')}</span>
              <input
                type="text"
                value={review.summary}
                on:input={(event) => editReview({ summary: (event.currentTarget as HTMLInputElement).value })}
              />
            </label>
            <label>
              <span>{t('material.field_tags')}</span>
              <input
                type="text"
                value={review.tags}
                placeholder={t('material.tags_hint')}
                on:input={(event) => editReview({ tags: (event.currentTarget as HTMLInputElement).value })}
              />
            </label>
            <label>
              <span>{t('material.field_conditions')}</span>
              <textarea
                rows="3"
                value={review.conditions}
                placeholder={t('material.conditions_hint')}
                on:input={(event) => editReview({ conditions: (event.currentTarget as HTMLTextAreaElement).value })}
              ></textarea>
            </label>
            <label>
              <span>{t('material.field_source')}</span>
              <input
                type="text"
                value={review.source}
                placeholder={t('material.source_hint')}
                on:input={(event) => editReview({ source: (event.currentTarget as HTMLInputElement).value })}
              />
            </label>
          </form>
        {/if}
      </div>
    {:else}
      <div role="tabpanel" id="panel-cite" aria-labelledby="tab-cite">
        {#if citations.length === 0}
          <p class="empty">{t('material.none')}</p>
        {:else}
          <ul class="cites">
            {#each citations as hit}
              <li>
                <button type="button" on:click={() => void jumpToCitation(hit)}>
                  <span class="cite-path">{hit.path}:{hit.line}</span>
                  <span class="cite-text">{hit.text}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </aside>
</section>

<style>
  .material {
    display: grid;
    grid-template-columns: minmax(200px, 28%) 1fr minmax(240px, 30%);
    height: 100%;
    min-height: 0;
    background: var(--bg-base);
    color: var(--fg-primary);
  }
  .list-pane,
  .edit-pane,
  .review-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .list-pane {
    border-right: 1px solid var(--border);
  }
  .review-pane {
    border-left: 1px solid var(--border);
    padding: 8px;
    overflow-y: auto;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 8px;
  }
  .chip,
  .actions button,
  .new button,
  .cites button,
  .tabs button {
    min-height: 24px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-raised);
    color: var(--fg-primary);
    cursor: pointer;
    font: inherit;
  }
  .chip.on,
  .tabs button[aria-selected='true'] {
    border-color: var(--accent);
    background: var(--bg-selection);
  }
  .actions button:disabled {
    color: var(--fg-faint);
    cursor: not-allowed;
  }
  .query,
  .new {
    display: block;
    padding: 0 8px 8px;
  }
  input,
  select,
  textarea {
    box-sizing: border-box;
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
  ul[role='listbox'],
  .cites {
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
  .row.unregistered .status {
    color: var(--state-error);
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
  .cat {
    margin-left: auto;
    font-size: 12px;
    color: var(--fg-faint);
    white-space: nowrap;
  }
  .edit-pane {
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
  .note,
  .confirm {
    font-size: 13px;
    color: var(--state-warn);
  }
  .confirm {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .body-label {
    font-size: 12px;
    color: var(--fg-muted);
  }
  .body {
    flex: 1;
    min-height: 0;
    margin-top: 4px;
    font-family: var(--mono);
    resize: none;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .form label span {
    display: block;
    font-size: 12px;
    color: var(--fg-muted);
    margin-bottom: 2px;
  }
  .tabs {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
  .cites li {
    margin-bottom: 4px;
  }
  .cites button {
    display: block;
    width: 100%;
    text-align: left;
  }
  .cite-path {
    display: block;
    font-size: 12px;
    color: var(--fg-muted);
  }
  .cite-text {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
