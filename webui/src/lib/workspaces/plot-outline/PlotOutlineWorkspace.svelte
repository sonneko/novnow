<script lang="ts">
  // `plot-outline`（構成。`REF:docs/specifications/ui.md` §10.3）。
  //
  // 左: レベル切替と、章 → 話のツリー（畳み方は `plot.outline_fold` が持つ。§10.3.1）。
  // 中央: A 形式のブロックの一覧（1 段落ずつ。枠線は付けない）と、選んだブロックの編集。
  // 右: タブ（既定は「基本」。基本では右に伏線パネル。他のタブは中央に全幅で出す）。
  //
  // 材料は正規操作だけ（`REF:docs/designs/webui.md` §21）:
  //   - ツリーと一覧 … `plot.get`（level=all / chapter）
  //   - 折り畳み     … `plot.outline_fold`（返された集合をそのまま使う）
  //   - 編集         … `document.get` で文書の全文を読み、行の範囲だけ差し替えて `plot.write`
  //
  // **この版で出せないもの**（§21.1.2）: 伏線パネル（`seed:` / `payoff:` の突き合わせ）、
  // 時系列・俯瞰表・挿絵メモ・五感・空間・章のあらすじ・字数分布、ストックの文字数・配属、
  // ツリーの文字数、ブロックの新設（`Ctrl+Shift+N`）。対応する操作がカタログに無い。
  import { onDestroy } from 'svelte';
  import { Ime } from '../../keyboard/ime';
  import { strokeOf } from '../../keyboard/keys';
  import type { Texts } from '../../i18n';
  import type { Session } from '../../store/session';
  import {
    blocksOf,
    chapterNumber,
    linesOf,
    replaceLines,
    type Block,
  } from './blocks';
  import {
    allFolded,
    blocksAt,
    buildTree,
    chapterLabel,
    defaultChapter,
    moveTab,
    outlineKey,
    TABS,
    toggleFold,
    type Chapter,
    type Level,
    type Tab,
  } from './model';

  /** 文言。 */
  export let t: Texts['t'];
  /** セッションの口（正規操作・通知・状態）。 */
  export let session: Session;
  /** 構成を書いた（保存の予約は画面の仕事。`App` の `schedule` に渡す）。 */
  export let ondirty: (() => void) | null = null;
  /** 画面の状態（`session.state` の写し。`scene` はいまの節＝どの話の位置から入ったか）。 */
  $: screen = session.state;

  /** タブの文言の鍵（§10.3 の表の名称）。 */
  const TAB_LABEL: Record<Tab, string> = {
    base: 'stage.base',
    timeline: 'stage.timeline',
    matrix: 'stage.matrix',
    illustrations: 'stage.illustrations',
    senses: 'stage.senses',
    space: 'stage.space',
    blurb: 'stage.blurb',
    histogram: 'stage.histogram',
    stock: 'stage.outline_stock',
  };

  /** 構成のツリー（章と話）。 */
  let tree: Chapter[] = [];
  /** 畳んでいる章（`plot.outline_fold` が返した集合）。 */
  let folded: number[] = [];
  let level: Level = { kind: 'all' };
  let tab: Tab = 'base';
  /** 選んでいるブロックの ID と、その編集の中身。 */
  let selectedId: string | null = null;
  let editor = '';
  let dirty = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let generation = 0;
  let composing = false;
  const ime = new Ime((found) => {
    composing = found;
  });
  /** 起動直後に全章を畳んだか（セッションごとに 1 回だけ。`WeakSet` で持つ）。 */
  const foldedOnce = new WeakSet<object>();
  let started = false;

  /** 表示の対象のブロック（レベルで決まる）。 */
  $: blocks = blocksAt(tree, level);
  $: selected = blocks.find((block) => block.id === selectedId) ?? null;
  $: stock = tree.find((chapter) => chapter.number === 0)?.episodes ?? [];

  /** 初回だけ読む。以後は画面の位置（いまの節）が変わったときに、その話へ寄せる。 */
  $: if (!started && screen.ready) {
    started = true;
    void reload(true);
  }

  /** 構成を読み直す（全体 → 各章の順。章のファイルが無ければ空のまま）。 */
  async function reload(first = false): Promise<void> {
    const found = ++generation;
    const all = await session.run('plot.get', { level: 'all' });
    if (!all.ok) {
      session.notice(all.message, 'error', 0);
      return;
    }
    const allBlocks = blocksOf(all.result);
    const numbers = [
      ...new Set(
        allBlocks
          .filter((block) => !block.id.includes('-'))
          .map((block) => chapterNumber(block.id))
          .filter((number): number is number => number !== null),
      ),
    ].sort((left, right) => left - right);
    const episodes = new Map<number, Block[]>();
    await Promise.all(
      numbers.map(async (number) => {
        const got = await session.run('plot.get', { level: 'chapter', chapter: number });
        if (got.ok) {
          episodes.set(number, blocksOf(got.result));
        }
      }),
    );
    if (found !== generation) {
      return;
    }
    tree = buildTree(allBlocks, episodes);
    if (!foldedOnce.has(session)) {
      foldedOnce.add(session);
      await foldAll();
    }
    if (first) {
      const at = screen.scene ? Number(screen.scene.slice(0, 2)) : NaN;
      if (Number.isFinite(at) && tree.some((chapter) => chapter.number === at)) {
        level = { kind: 'chapter', chapter: at };
      }
    }
    if (selectedId === null || !blocksAt(tree, level).some((block) => block.id === selectedId)) {
      // 画面の位置（いまの節の話）があればそのブロックを、無ければ先頭を選ぶ
      const here = screen.scene ? screen.scene.split('-').slice(0, 2).join('-') : '';
      const next = blocksAt(tree, level).find((block) => block.id === here) ?? blocksAt(tree, level)[0] ?? null;
      if (next) {
        await select(next);
      } else {
        selectedId = null;
        editor = '';
      }
    }
  }

  /** 全章を畳む（起動直後の既定。§10.3.1）。core の集合をそのまま採る。 */
  async function foldAll(): Promise<void> {
    for (const chapter of allFolded(tree)) {
      const got = await session.run('plot.outline_fold', { chapter, fold: true });
      if (got.ok) {
        folded = got.result.folded.filter((value): value is number => typeof value === 'number');
      }
    }
  }

  /** 章が畳まれているか。 */
  function isFolded(number: number): boolean {
    return folded.includes(number);
  }

  /** 章の畳み・展開（`plot.outline_fold`。集合は core が返したものを使う）。 */
  async function fold(number: number, value: boolean): Promise<void> {
    const next = toggleFold(folded, number, value);
    const got = await session.run('plot.outline_fold', { chapter: number, fold: value });
    if (got.ok) {
      folded = got.result.folded.filter((item): item is number => typeof item === 'number');
    } else {
      folded = next;
      session.notice(got.message, 'error', 0);
    }
  }

  /** レベルを変える（全体 ⇄ 章）。編集中のブロックは先に書く。 */
  async function setLevel(next: Level): Promise<void> {
    await flush();
    level = next;
    const first = blocksAt(tree, next)[0] ?? null;
    if (first) {
      await select(first);
    } else {
      selectedId = null;
      editor = '';
    }
  }

  /** ブロックを選ぶ（文書の全文を読み、そのブロックの行だけを編集に出す）。 */
  async function select(block: Block): Promise<void> {
    await flush();
    selectedId = block.id;
    const got = await session.run('document.get', { path: block.path });
    if (!got.ok) {
      session.notice(got.message, 'error', 0);
      editor = '';
      return;
    }
    editor = linesOf(got.result.text, block.lines);
    dirty = false;
  }

  /** 中央の一覧の行（`↑` / `↓` で動かす。素の矢印は一覧に効く。§9.8.6）。 */
  async function move(step: 1 | -1): Promise<void> {
    if (blocks.length === 0) {
      return;
    }
    const at = blocks.findIndex((block) => block.id === selectedId);
    const next = blocks[Math.min(blocks.length - 1, Math.max(0, at + step))];
    if (next && next.id !== selectedId) {
      await select(next);
    }
  }

  /** 編集（最後の打鍵から 400ms で書く。§8.2 と同じ待ち）。 */
  function oninput(event: Event): void {
    editor = (event.target as HTMLTextAreaElement).value;
    dirty = true;
    if (timer !== null) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = null;
      void flush();
    }, 400);
  }

  /**
   * 未保存の編集を書く。文書の全文を読み直し（他の操作で行がずれていても壊さない）、
   * そのブロックの行の範囲だけを差し替えて `plot.write` に全文を渡す。
   */
  async function flush(): Promise<void> {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    if (!dirty || selectedId === null) {
      return;
    }
    const id = selectedId;
    const at = blocks.find((block) => block.id === id);
    if (!at) {
      return;
    }
    dirty = false;
    const fresh = await session.run('document.get', { path: at.path });
    if (!fresh.ok) {
      dirty = true;
      session.notice(fresh.message, 'error', 0);
      return;
    }
    // 行の範囲は書く直前に取り直す（直前の書き込みで行がずれているかもしれない）
    const again = await session.run('plot.get', levelArgs());
    const lines = again.ok ? (blocksOf(again.result).find((block) => block.id === id)?.lines ?? at.lines) : at.lines;
    const text = replaceLines(fresh.result.text, lines, editor);
    const written = await session.run('plot.write', { ...levelArgs(), role: '', text });
    if (!written.ok) {
      dirty = true;
      session.notice(written.message, 'error', 0);
      return;
    }
    ondirty?.();
    await reload();
  }

  /** `plot.get` / `plot.write` の引数（いまのレベル）。 */
  function levelArgs(): { level: string; chapter?: number } {
    return level.kind === 'all' ? { level: 'all' } : { level: 'chapter', chapter: level.chapter };
  }

  /** 下位へ（`Ctrl+Enter`）: 全体なら章へ、章なら話プロットへ。 */
  async function down(): Promise<void> {
    if (level.kind === 'all') {
      const number = defaultChapter(tree, selected);
      if (number !== null) {
        await setLevel({ kind: 'chapter', chapter: number });
      }
      return;
    }
    if (!selected) {
      return;
    }
    await flush();
    const nav = await session.run('nav.section', { scene: selected.id });
    if (!nav.ok) {
      session.notice(nav.message, 'error', 0);
      return;
    }
    const moved = await session.run('session.workspace_switch', { workspace: 'plot-episode' });
    if (!moved.ok) {
      session.notice(moved.message, 'error', 0);
    }
  }

  /** 上位へ（`Ctrl+Shift+B`）: 章から全体へ。 */
  async function up(): Promise<void> {
    if (level.kind === 'chapter') {
      await setLevel({ kind: 'all' });
    }
  }

  /** この版で出せない操作を、理由つきで知らせる（黙って何もしない、§21.1.2）。 */
  function unavailable(label: string): void {
    session.notice(t('workspace.unavailable', [label]), 'info');
  }

  /**
   * キー（このワークスペースの中だけで受ける。§9.8）。写しは `outlineKey`（テストで確かめる）。
   * 扱ったキーは `stopPropagation` して、シェルの同じ割当が二重に走らないようにする。
   */
  function onkey(event: KeyboardEvent): void {
    if (ime.active || composing) {
      return;
    }
    const inField = event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLInputElement;
    const found = outlineKey({ ...strokeOf(event), code: event.code });
    if (found === null || (inField && (found === 'up' || found === 'down'))) {
      return;
    }
    const handled = (run: () => void): void => {
      event.preventDefault();
      event.stopPropagation();
      run();
    };
    const currentChapter = level.kind === 'chapter' ? level.chapter : defaultChapter(tree, selected);
    switch (found) {
      case 'tab-next':
        handled(() => {
          tab = moveTab(tab, 1);
        });
        return;
      case 'tab-previous':
        handled(() => {
          tab = moveTab(tab, -1);
        });
        return;
      case 'up-level':
        handled(() => void up());
        return;
      case 'down-level':
        handled(() => void down());
        return;
      case 'fold':
        handled(() => {
          if (currentChapter !== null) {
            void fold(currentChapter, true);
          }
        });
        return;
      case 'unfold':
        handled(() => {
          if (currentChapter !== null) {
            void fold(currentChapter, false);
          }
        });
        return;
      case 'new-section':
        handled(() => unavailable(t('outline.new_button')));
        return;
      case 'up':
      case 'down':
        if (tab === 'base') {
          handled(() => void move(found === 'up' ? -1 : 1));
        }
        return;
      default:
        return;
    }
  }

  /** 画面を離れるとき、未保存の編集を書く。 */
  onDestroy(() => {
    void flush();
    ime.dispose();
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="outline" role="application" aria-label={t('header.workspace.plot-outline')} on:keydown={onkey}>
  <!-- 左: レベルと、章 → 話のツリー（§10.3.1） -->
  <nav class="tree" aria-label={t('outline.all')}>
    <button
      type="button"
      class="level"
      aria-pressed={level.kind === 'all'}
      on:click={() => setLevel({ kind: 'all' })}
    >
      {t('outline.all')}
    </button>
    {#each tree as chapter (chapter.number)}
      <div class="chapter">
        <div class="chapter-row">
          <button
            type="button"
            class="fold"
            aria-expanded={!isFolded(chapter.number)}
            aria-label={t('help.fold')}
            on:click={() => fold(chapter.number, !isFolded(chapter.number))}
          >
            {isFolded(chapter.number) ? '▸' : '▾'}
          </button>
          <button
            type="button"
            class="level chapter-name"
            aria-pressed={level.kind === 'chapter' && level.chapter === chapter.number}
            on:click={() => setLevel({ kind: 'chapter', chapter: chapter.number })}
          >
            <span class="id">{chapterLabel(chapter.number)}</span>
            <span class="title">{chapter.block?.title ?? ''}</span>
            <span class="count">{t('stage.outline_episodes_unit', [String(chapter.episodes.length)])}</span>
          </button>
        </div>
        {#if !isFolded(chapter.number)}
          <ul class="episodes">
            {#each chapter.episodes as episode (episode.id)}
              <li>
                <button
                  type="button"
                  class="episode"
                  aria-current={selectedId === episode.id ? 'true' : undefined}
                  on:click={() => {
                    level = { kind: 'chapter', chapter: chapter.number };
                    void select(episode);
                  }}
                >
                  <span class="id">{episode.id}</span>
                  <span class="title">{episode.title}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/each}
  </nav>

  <div class="main">
    <!-- タブ（§10.3）。既定は「基本」。 -->
    <div class="tabs" role="tablist" aria-label={t('header.workspace.plot-outline')}>
      {#each TABS as name (name)}
        <button
          type="button"
          role="tab"
          class="tab"
          aria-selected={tab === name}
          on:click={() => (tab = name)}
        >
          {t(TAB_LABEL[name])}
        </button>
      {/each}
    </div>

    {#if tab === 'base'}
      <div class="base">
        <section class="center" aria-label={t('outline.all')}>
          <!-- A 形式のブロック一覧（1 ブロック = 1 段落。枠線なし。§10.3） -->
          <ul class="blocks">
            {#each blocks as block (block.id)}
              <li>
                <button
                  type="button"
                  class="block"
                  aria-pressed={selectedId === block.id}
                  on:click={() => select(block)}
                >
                  <span class="id">@{block.id}</span>
                  <span class="title">{block.title}</span>
                  {#if block.state}<span class="state">{block.state}</span>{/if}
                  <span class="summary">{block.summary}</span>
                </button>
              </li>
            {:else}
              <li class="empty">
                {level.kind === 'all'
                  ? t('outline.no_blocks', ['plots/all.cnov'])
                  : t('outline.no_blocks', [`plots/${chapterLabel(level.chapter)}.cnov`])}
              </li>
            {/each}
          </ul>
          {#if selected}
            <textarea
              class="editor"
              aria-label={t('outline.all')}
              spellcheck="false"
              value={editor}
              on:input={oninput}
              on:blur={() => flush()}
              on:compositionstart={() => ime.start()}
              on:compositionupdate={() => ime.start()}
              on:compositionend={() => ime.end()}
            ></textarea>
          {:else}
            <p class="empty">{t('outline.empty')}</p>
          {/if}
        </section>

        <!-- 基本タブの右: 伏線パネル。突き合わせる操作が無いので出せない（§21.1.2） -->
        <aside class="foreshadow" aria-label={t('outline.foreshadow')}>
          <h2>{t('outline.foreshadow')}</h2>
          <p class="empty">{t('workspace.unavailable', [t('outline.foreshadow')])}</p>
        </aside>
      </div>
    {:else if tab === 'stock'}
      <!-- ストック章（00）。配属と文字数は出せない（§10.3.3） -->
      <section class="full" aria-label={t('stage.outline_stock')}>
        <ul class="blocks">
          {#each stock as block (block.id)}
            <li>
              <span class="block static">
                <span class="id">@{block.id}</span>
                <span class="title">{block.title}</span>
                <span class="summary">{block.summary}</span>
              </span>
            </li>
          {:else}
            <li class="empty">{t('stage.stock_empty')}</li>
          {/each}
        </ul>
        <p class="empty">{t('workspace.unavailable', [t('stage.stock_assign_button')])}</p>
      </section>
    {:else}
      <!-- 操作の無いタブは「使えません」を出す（空の画面を置かない。§21.1.2） -->
      <section class="full">
        <p class="empty">{t('workspace.unavailable', [t(TAB_LABEL[tab])])}</p>
      </section>
    {/if}
  </div>
</div>

<style>
  .outline {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
    color: var(--fg-primary);
  }
  .tree {
    overflow: auto;
    border-right: 1px solid var(--border);
    padding: 8px 0;
    font-size: 13px;
  }
  .chapter-row {
    display: flex;
    align-items: center;
  }
  .level,
  .fold,
  .episode,
  .block,
  .tab {
    min-height: 24px;
    min-width: 24px;
    background: none;
    border: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
    font-size: 13px;
  }
  .level,
  .chapter-name {
    flex: 1;
    display: flex;
    gap: 6px;
    align-items: baseline;
    padding: 4px 8px;
  }
  .level[aria-pressed='true'] {
    background: var(--bg-selection);
  }
  .level:hover,
  .episode:hover,
  .block:hover,
  .tab:hover,
  .fold:hover {
    background: var(--bg-selection);
    cursor: pointer;
  }
  .episodes {
    list-style: none;
    margin: 0;
    padding: 0 0 0 24px;
  }
  .episode {
    width: 100%;
    display: flex;
    gap: 6px;
    padding: 2px 8px;
  }
  .episode[aria-current='true'] {
    background: var(--bg-selection);
  }
  .id {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--scene-id);
  }
  .count {
    margin-left: auto;
    font-size: 10px;
    color: var(--fg-faint);
    white-space: nowrap;
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
  }
  .tab[aria-selected='true'] {
    text-decoration: underline;
    text-underline-offset: 4px;
  }
  .base {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 280px;
    flex: 1;
    min-height: 0;
  }
  .center {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .blocks {
    list-style: none;
    margin: 0;
    padding: 8px 16px;
    overflow: auto;
    flex: 1;
    min-height: 0;
  }
  .block {
    display: block;
    width: 100%;
    padding: 6px 4px;
    line-height: 1.6;
  }
  .block .title {
    margin: 0 8px;
  }
  .block .state {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg-muted);
    margin-right: 8px;
  }
  .block .summary {
    display: block;
    font-size: 12px;
    color: var(--fg-muted);
  }
  .block.static {
    display: block;
    padding: 6px 4px;
  }
  .editor {
    height: 40%;
    min-height: 120px;
    margin: 0 16px 12px;
    padding: 8px;
    resize: none;
    background: var(--bg-inset);
    color: var(--fg-primary);
    border: 1px solid var(--border);
    font-family: var(--mono);
    font-size: 13px;
    line-height: 1.6;
  }
  .foreshadow {
    border-left: 1px solid var(--border);
    padding: 8px 12px;
    font-size: 13px;
  }
  .foreshadow h2 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  .full {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 8px 16px;
  }
  .empty {
    margin: 8px 0;
    font-size: 13px;
    color: var(--fg-muted);
  }
</style>
