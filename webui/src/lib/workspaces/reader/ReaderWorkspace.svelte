<script lang="ts">
  // `reader` ワークスペース（§10.7・§17.3）。縦書きの読者モード。
  //
  // 原稿に対して**読み取り専用**（§17.3.1 MUST）。いかなる操作でも原稿・プロット・資料・
  // 共有設定を変更しない。保存するのは読書位置・表示設定・しおりの**ローカル状態**だけ
  // （ブラウザの保存領域。第 1 の画面の保存領域とは別。§21.3.2）。
  //
  // 材料は正規操作の戻り値だけ（§11.11.5）:
  // - `structure.tree`（`level: chapter`）→ 章の一覧（ID と summary）
  // - `document.get`（`plots/all.cnov`）→ 章題（`@NN 題名` の行）
  // - `export.text`（`range` = 章）→ その章の本文（アノテーション変換済み。§17.1）
  //
  // 組版は純粋な `reader.ts` の `layout`（§17.3.3 の禁則処理）。描画は CSS の
  // `writing-mode: vertical-rl` に任せる（行は右から左へ、行内は上から下へ）。
  import type { OpArgs, OpId } from '../../bridge/types';
  import type { Outcome, Screen } from '../../store/session';
  import type { Texts } from '../../i18n';
  import { onMount } from 'svelte';
  import { layout, type Cell, type Page } from './reader';

  /** 文言。 */
  export let t: Texts['t'];
  /** 画面の状態。 */
  export let screen: Screen;
  /** 正規操作 1 回（`Session.run` と同じ手順）。 */
  export let run: <T extends OpId>(op: T, args?: OpArgs[T]) => Promise<Outcome<T>>;
  /** ワークスペースを切り替える（`Esc` / `Alt+1` で編集へ戻る）。 */
  export let onworkspace: ((name: string) => void) | null = null;
  /** 通知（§21.1.2）。 */
  export let onnotice: ((text: string, kind?: 'info' | 'error') => void) | null = null;
  /** 変換中か（IME）。 */
  export let composing = false;

  /** 既定の版面（§17.3.2: 既定 18 列 × 42 字）。 */
  const DEFAULT_ROWS = 42;
  const DEFAULT_COLUMNS = 18;

  /** ローカル保存のキー（ブラウザの保存領域。`.novnow/reader.json` の代わり）。 */
  const STORE_KEY = 'novnow-webui:reader';

  /** 読者モードのローカル状態（§17.3.1 の例外）。 */
  interface ReaderState {
    page: number;
    fontScale: number;
    showRuby: boolean;
    columns: number;
    bookmarks: number[];
  }

  /** 章 1 つの組版（表紙ページ + 本文ページ）。 */
  interface Chapter {
    id: string;
    title: string;
    blurb: string;
    pages: Page[];
  }

  /** 組み上がった作品（章ごとのページ列）。 */
  let chapters: Chapter[] = [];
  /** 全ページ（章をつなげたもの）と、各ページの章題（柱）。 */
  let pages: Page[] = [];
  let pageChapterTitles: string[] = [];
  /** 現在のページ（0 始まり）。 */
  let pageIndex = 0;
  /** 文字サイズ（`+` / `-` / `0`。§17.3.4）。 */
  let fontScale = 1.0;
  /** ルビ表示（`r`。§17.3.4）。 */
  let showRuby = true;
  /** 1 行の字数（列数。幅に応じて自動調整）。 */
  let columns = DEFAULT_COLUMNS;
  /** しおり（ページ番号。§17.3.4 の `b`）。 */
  let bookmarks: number[] = [];
  /** 版面の測定元（ResizeObserver）。 */
  let pageBox: HTMLElement | null = null;

  /** ローカル状態を読む（無ければ既定）。 */
  function loadState(): ReaderState {
    try {
      const raw = localStorage.getItem(STORE_KEY);
      if (raw) {
        const found = JSON.parse(raw) as Partial<ReaderState>;
        return {
          page: found.page ?? 0,
          fontScale: found.fontScale ?? 1.0,
          showRuby: found.showRuby ?? true,
          columns: found.columns ?? DEFAULT_COLUMNS,
          bookmarks: found.bookmarks ?? [],
        };
      }
    } catch {
      // 壊れた保存は既定に戻る（黙って失敗しない。§21.1.2）
    }
    return { page: 0, fontScale: 1.0, showRuby: true, columns: DEFAULT_COLUMNS, bookmarks: [] };
  }

  /** ローカル状態を書く（読書位置・表示設定・しおりだけ。§17.3.1）。 */
  function saveState(): void {
    try {
      localStorage.setItem(
        STORE_KEY,
        JSON.stringify({ page: pageIndex, fontScale, showRuby, columns, bookmarks }),
      );
    } catch {
      // 保存領域が使えない環境は黙って捨てる（読者モードの本体操作ではない）
    }
  }

  /** `plots/all.cnov` から章題（`@NN 題名`）を集める。 */
  function parseChapterTitles(text: string): Map<string, string> {
    const out = new Map<string, string>();
    for (const line of text.split('\n')) {
      const found = /^@(\d+)\s+(.+)$/.exec(line.trim());
      if (found) {
        const id = found[1];
        const title = found[2];
        if (id && title) {
          out.set(id, title.trim());
        }
      }
    }
    return out;
  }

  /** 章の本文を組版して章を組み上げる。 */
  async function buildChapter(
    id: string,
    title: string,
    blurb: string,
    text: string,
  ): Promise<Chapter> {
    const cover = layout(blurb ? `${title}\n\n${blurb}` : title, { columns, rows: DEFAULT_ROWS });
    const content = layout(text, { columns, rows: DEFAULT_ROWS });
    // §17.3.2: 章の先頭に表紙ページを 1 枚挟む（blurb が無ければ章題のみ）。
    const coverPages = cover.length > 0 ? cover.slice(0, 1) : [];
    return { id, title, blurb, pages: [...coverPages, ...content] };
  }

  /** 作品全体を組み上げる（章ごと）。 */
  async function buildBook(): Promise<void> {
    const tree = await run('structure.tree', { level: 'chapter' });
    if (!tree.ok) {
      onnotice?.(tree.message, 'error');
      return;
    }
    const nodes = (tree.result as unknown as { nodes: { id: string; summary: string }[] }).nodes ?? [];
    const titles = new Map<string, string>();
    const all = await run('document.get', { path: 'plots/all.cnov' });
    if (all.ok) {
      for (const [id, title] of parseChapterTitles((all.result as unknown as { text: string }).text)) {
        titles.set(id, title);
      }
    }
    const built: Chapter[] = [];
    for (const node of nodes) {
      const range = await run('export.text', { range: node.id, blurb: false });
      const text = range.ok ? ((range.result as unknown as { text?: string }).text ?? '') : '';
      const title = titles.get(node.id) ?? node.id;
      const chapter = await buildChapter(node.id, title, node.summary, text);
      if (chapter.pages.length > 0) {
        built.push(chapter);
      }
    }
    chapters = built;
    // 全ページへつなぎ、各ページの章題（柱）を覚える
    pages = [];
    pageChapterTitles = [];
    for (const chapter of chapters) {
      for (const page of chapter.pages) {
        pages.push(page);
        pageChapterTitles.push(chapter.title);
      }
    }
    // §10.0: 任意 → reader は現在の節の先頭（その章の表紙）へ着地する。
    const chapterOf = screen.scene.slice(0, 2);
    const start = chapters.findIndex((chapter) => chapter.id === chapterOf);
    if (start >= 0) {
      pageIndex = chapterOffset(start);
    } else if (pageIndex >= pages.length) {
      pageIndex = Math.max(0, pages.length - 1);
    }
  }

  /** 章 `index` の先頭ページ（表紙）の全局ページ番号。 */
  function chapterOffset(index: number): number {
    let at = 0;
    let i = 0;
    for (const chapter of chapters) {
      if (i >= index) {
        break;
      }
      at += chapter.pages.length;
      i += 1;
    }
    return at;
  }

  /** 版面の幅から列数を自動調整して再組版（§17.3.2 の `auto`）。 */
  function relayout(): void {
    if (pageBox) {
      const width = pageBox.clientWidth;
      // 1 字の幅は文字サイズに比例（簡易版: 1 マス = fontScale の正方形）。
      const next = Math.round(width / (16 * fontScale));
      if (next >= 6 && next !== columns) {
        columns = Math.min(24, Math.max(6, next));
        saveState();
        void buildBook();
      }
    }
  }

  /** 幅の変化を観測して列数を自動調整（`on:resize` は div で発火しないため）。 */
  let observer: ResizeObserver | null = null;
  onMount(() => {
    if (pageBox && typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(() => relayout());
      observer.observe(pageBox);
    }
    return () => observer?.disconnect();
  });

  /** 次のページ（左へ進む。§17.3.4）。 */
  function nextPage(): void {
    if (pageIndex < pages.length - 1) {
      pageIndex += 1;
      saveState();
    }
  }

  /** 前のページ。 */
  function prevPage(): void {
    if (pageIndex > 0) {
      pageIndex -= 1;
      saveState();
    }
  }

  /** 先頭 / 末尾。 */
  function firstPage(): void {
    pageIndex = 0;
    saveState();
  }
  function lastPage(): void {
    pageIndex = Math.max(0, pages.length - 1);
    saveState();
  }

  /** 文字サイズ（`+` / `-` / `0`。§17.3.4）。 */
  function setFont(delta: number): void {
    fontScale = Math.min(1.6, Math.max(0.8, Math.round((fontScale + delta) * 10) / 10));
    saveState();
  }
  function resetFont(): void {
    fontScale = 1.0;
    saveState();
  }

  /** ルビ表示の切替（`r`）。 */
  function toggleRuby(): void {
    showRuby = !showRuby;
    saveState();
  }

  /** テーマ切替（`t`。§17.3.4）。`session.command` の `theme` と同じ口）。 */
  async function toggleTheme(): Promise<void> {
    const found = await run('session.command', { name: 'theme' });
    if (!found.ok) {
      onnotice?.(found.message, 'error');
    }
  }

  /** しおり（`b`。現在のページを保存）。§17.3.4。 */
  function bookmark(): void {
    if (!bookmarks.includes(pageIndex)) {
      bookmarks.push(pageIndex);
      bookmarks.sort((left, right) => left - right);
      saveState();
    }
  }

  /** しおりからジャンプ。 */
  function gotoBookmark(page: number): void {
    pageIndex = page;
    saveState();
  }

  /** 章題の目次（しおり・章移動）。 */
  function chapterStart(index: number): void {
    pageIndex = chapterOffset(index);
    saveState();
  }

  /** 現在のページ。 */
  $: page = pages[pageIndex] ?? { lines: [] as Cell[][] };
  $: chapterTitle = pageChapterTitles[pageIndex] ?? '';

  /** 初期読み込み（ローカル状態を復元してから組み上げる）。 */
  let state = loadState();
  pageIndex = state.page;
  fontScale = state.fontScale;
  showRuby = state.showRuby;
  columns = state.columns;
  bookmarks = state.bookmarks;
  $: void buildBook();

  /** キー（`reader` 固有。§17.3.4）。 */
  function onkey(event: KeyboardEvent): void {
    if (composing) {
      return;
    }
    const key = event.key;
    if (key === 'ArrowLeft' || key === ' ' || key === 'PageDown') {
      event.preventDefault();
      nextPage();
      return;
    }
    if (key === 'ArrowRight' || key === 'PageUp') {
      event.preventDefault();
      prevPage();
      return;
    }
    if (key === 'Home') {
      event.preventDefault();
      firstPage();
      return;
    }
    if (key === 'End') {
      event.preventDefault();
      lastPage();
      return;
    }
    if (key === '+' || key === '=') {
      event.preventDefault();
      setFont(0.1);
      return;
    }
    if (key === '-' || key === '_') {
      event.preventDefault();
      setFont(-0.1);
      return;
    }
    if (key === '0') {
      event.preventDefault();
      resetFont();
      return;
    }
    if (key === 'r' || key === 'R') {
      event.preventDefault();
      toggleRuby();
      return;
    }
    if (key === 't' || key === 'T') {
      event.preventDefault();
      void toggleTheme();
      return;
    }
    if (key === 'b' || key === 'B') {
      event.preventDefault();
      bookmark();
      return;
    }
    if (key === 'Escape' || (event.altKey && key === '1')) {
      event.preventDefault();
      onworkspace?.('write');
    }
  }
</script>

<svelte:window on:keydown={onkey} />

<div class="reader">
  <!-- 目次（章題としおり。縦書きの導線） -->
  <aside class="toc" aria-label={t('stage.toc')}>
    <h2>{t('stage.toc')}</h2>
    {#each chapters as chapter, index (chapter.id)}
      <button type="button" class="toc-row" on:click={() => chapterStart(index)}>
        <span class="title">{chapter.title}</span>
        <span class="pages">{chapter.pages.length}</span>
      </button>
    {/each}
    {#if bookmarks.length > 0}
      <h2>{t('stage.bookmark')}</h2>
      {#each bookmarks as mark (mark)}
        <button type="button" class="toc-row" on:click={() => gotoBookmark(mark)}>
          <span class="title">{mark + 1}</span>
        </button>
      {/each}
    {/if}
  </aside>

  <!-- 本文（縦書き。§17.3.2） -->
  <main
    class="page-wrap"
    bind:this={pageBox}
    role="region"
    aria-label={chapterTitle || t('stage.reader')}
  >
    <div
      class="page"
      style={`writing-mode:vertical-rl; font-size:${Math.round(16 * fontScale)}px;`}
    >
      {#each page.lines as line (line)}
        <div class="line">
          {#each line as cell (cell.ch + cell.ruby)}
            <span class="cell" class:punctuation={cell.punctuation}>
              {cell.text}
              {#if showRuby && cell.ruby}
                <span class="ruby">{cell.ruby}</span>
              {/if}
            </span>
          {/each}
        </div>
      {/each}
    </div>
    <!-- 柱（章題）とノンブル（ページ番号）。§17.3.2。 -->
    {#if chapterTitle}
      <span class="head">{chapterTitle}</span>
    {/if}
    <span class="number">{pageIndex + 1}</span>
  </main>
</div>

<style>
  .reader {
    display: grid;
    grid-template-columns: 180px 1fr;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
    background: var(--bg-base);
  }
  @media (max-width: 900px) {
    .reader {
      grid-template-columns: 1fr;
      grid-template-rows: auto 1fr;
    }
  }
  /* 目次 */
  .toc {
    overflow: auto;
    padding: 8px;
    border-right: 1px solid var(--border);
    background: var(--bg-raised);
    color: var(--fg-muted);
  }
  .toc h2 {
    margin: 8px 0 4px;
    font-size: 11px;
    font-weight: 400;
    color: var(--fg-faint);
  }
  .toc-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    width: 100%;
    border: 0;
    background: transparent;
    color: var(--fg-primary);
    text-align: left;
    padding: 3px 6px;
    cursor: pointer;
    border-radius: 2px;
    text-decoration: underline;
    text-decoration-color: var(--border);
    font-size: 12px;
  }
  .toc-row:hover {
    background: var(--bg-selection);
  }
  .toc-row .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .toc-row .pages {
    color: var(--fg-faint);
    font-size: 10px;
    flex: 0 0 auto;
  }
  /* 本文 */
  .page-wrap {
    position: relative;
    min-height: 0;
    overflow: auto;
    display: flex;
    justify-content: center;
    padding: 16px 24px;
    background: var(--bg-base);
  }
  .page {
    writing-mode: vertical-rl;
    display: block;
    height: calc(100vh - 160px);
    overflow: hidden;
    color: var(--fg-primary);
    line-height: 2;
    /* 行（列）は右から左へ（`writing-mode` が自動で並べる） */
  }
  .line {
    display: block;
    white-space: nowrap;
  }
  .cell {
    display: inline;
    white-space: pre;
  }
  .cell.punctuation {
    /* 句読点は全角の右上寄せ（§17.3.3。CSS の縦組みに任せる） */
    margin-top: 0.25em;
  }
  .ruby {
    font-size: 0.5em;
    color: var(--ruby-text);
    margin-left: 0.1em;
  }
  /* 柱（章題）とノンブル */
  .head {
    position: absolute;
    top: 4px;
    right: 8px;
    color: var(--fg-faint);
    font-size: 9px;
  }
  .number {
    position: absolute;
    bottom: 4px;
    left: 8px;
    color: var(--fg-faint);
    font-size: 9px;
  }
</style>
