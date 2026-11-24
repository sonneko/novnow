<script lang="ts">
  // `plot-episode`（話プロット。`REF:docs/specifications/ui.md` §10.2）。
  //
  // 配置は §10.2 のとおり: 上段 = 章プロットの該当話（読み取り専用）、中段 = 節の一覧、
  // 下段 = 選んだ節の話プロット（場面の本文を操作で読み書きする）、右 = 参照ペイン。
  //
  // 材料はすべて正規操作から取る（`REF:docs/designs/webui.md` §21・`REF:docs/specifications/webui.md`
  // §21.1.3）:
  //   - 上段 … `plot.get`（level=chapter）の A 形式ブロック
  //   - 中段 … `section.list`（話の中の節）と `section.get`（台詞比率）
  //   - 下段 … `plot.get`（level=episode）と `plot.write`（場面の話プロットを丸ごと）
  //   - 右   … `reference.pane`（選んだ節）
  //
  // **この版で出せないもの**（§21.1.2: 黙って別の結果に置き換えず「使えません」を出す）:
  //   節ごとの `state:`（3px の色線）と `when:`、`Alt+Shift+↑/↓` の入れ替え、`Ctrl+D` の mode 行、
  //   `Ctrl+M` の資料挿入。いずれも対応する正規操作がカタログに無い（`REF:docs/plans.md#webui-w4`）。
  //
  // キーは §9.8 の表にあるものだけ、このワークスペースの中で受ける。IME の変換中は何も横取りしない。
  import { onDestroy } from 'svelte';
  import ReferencePane from '../../reference/ReferencePane.svelte';
  import { Ime } from '../../keyboard/ime';
  import { strokeOf } from '../../keyboard/keys';
  import type { Texts } from '../../i18n';
  import type { Session } from '../../store/session';
  import { blocksOf, linesOf, type Block } from '../plot-outline/blocks';
  import {
    dialogueRatio,
    episodeKey,
    episodeOf,
    keepSelection,
    percent,
    sceneOf,
    sectionsInEpisode,
    sectionsOf,
    type SectionItem,
  } from './model';

  /** 節ごとに読めない字句のキー（§12.6: 字句は翻訳しない）。 */
  const FIELD_STATE = 'state:';
  const FIELD_WHEN = 'when:';

  /** 文言。 */
  export let t: Texts['t'];
  /** セッションの口（正規操作・通知・状態）。 */
  export let session: Session;
  /** 画面の状態は `session.state` から読む（`scene` は「いまの節」）。 */
  /** 話プロットを書いた（保存の予約は画面の仕事。`App` の `schedule` に渡す）。 */
  export let ondirty: (() => void) | null = null;
  /** 画面の状態（`session.state` の写し）。 */
  $: screen = session.state;

  /** 中段の行（この話の節）。 */
  let rows: SectionItem[] = [];
  /** 節の本文（台詞比率に使う）。 */
  let prose = new Map<string, string>();
  /** 上段（章プロットの該当話）。 */
  let upper: Block | null = null;
  let upperMissing = '';
  let chapterPath = '';
  /** いまの話（`01-04`）。 */
  let episode: string | null = null;
  /** 選んでいる節の ID。 */
  let selected: string | null = null;
  /** 下段の本文（場面の話プロット）と、その場面の ID。 */
  let plot = '';
  let plotScene: string | null = null;
  let plotMissing = '';
  /** 削除の確認中の節（空でない節は確認を挟む。§8.1.2）。 */
  let armed: string | null = null;
  /** 参照ペイン（`reference.pane` の戻り値）。 */
  let reference: Record<string, unknown> | null = null;

  let dirty = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  /** 読み直しの世代（古い応答で新しい状態を上書きしない）。 */
  let generation = 0;
  let composing = false;
  const ime = new Ime((found) => {
    composing = found;
  });
  /** 一覧の要素（素の `↑` / `↓` で行を移すとき、フォーカスを追わせる）。 */
  let listEl: HTMLElement | null = null;

  /** 行ごとの表示値（台詞比率・文字数）。 */
  $: view = rows.map((row) => {
    const text = prose.get(row.id) ?? '';
    const found = dialogueRatio(text);
    return { row, empty: text.trim() === '', ratio: found.ratio, chars: countOf(text) };
  });

  /** 話全体の台詞比率（ヘッダのバー。§9.4.2 の最後の段落）。 */
  $: episodeRatio = dialogueRatio([...prose.values()].join('\n')).ratio;

  /** 選んでいる節の場面の題。 */
  $: sceneTitle = rows.find((row) => row.id === selected)?.summary ?? '';

  /** 話が変わったら読み直す（起動時と、別の話へ移ったとき）。 */
  $: nextEpisode = screen.scene ? episodeOf(screen.scene) : null;
  $: if (nextEpisode !== episode) {
    episode = nextEpisode;
    void refresh(screen.scene);
  }

  /** `plot.get`（level=episode）の本文。形が違えば空。 */
  function episodeText(result: unknown): string {
    if (typeof result !== 'object' || result === null) {
      return '';
    }
    const text = (result as { text?: unknown }).text;
    return typeof text === 'string' ? text : '';
  }

  /** 文字数（空白を除く）。 */
  function countOf(text: string): number {
    let found = 0;
    for (const ch of text) {
      if (!/\s/u.test(ch)) {
        found += 1;
      }
    }
    return found;
  }

  /** 話を読み直す（上段・節の一覧・本文・参照ペイン）。 */
  async function refresh(from: string): Promise<void> {
    const found = ++generation;
    await flush();
    const scene = from || screen.scene;
    const current = episodeOf(scene);
    if (!current) {
      rows = [];
      prose = new Map();
      reference = null;
      upper = null;
      selected = null;
      plot = '';
      plotScene = null;
      return;
    }
    const chapter = Number(current.slice(0, 2));
    const [parent, list] = await Promise.all([
      session.run('plot.get', { level: 'chapter', chapter }),
      session.run('section.list', {}),
    ]);
    if (found !== generation) {
      return;
    }
    if (parent.ok) {
      upper = blocksOf(parent.result).find((block) => block.id === current) ?? null;
      upperMissing = upper ? '' : current;
    } else {
      upper = null;
      upperMissing = current;
    }
    chapterPath = `plots/${String(chapter).padStart(2, '0')}.cnov`;
    const all = list.ok ? sectionsOf(list.result) : [];
    const inEpisode = sectionsInEpisode(all, current);
    const texts = await Promise.all(
      inEpisode.map(async (item) => {
        const got = await session.run('section.get', { scene: item.id });
        return [item.id, got.ok ? got.result.text : ''] as const;
      }),
    );
    if (found !== generation) {
      return;
    }
    rows = inEpisode;
    prose = new Map(texts);
    const next = keepSelection(inEpisode, selected ?? scene);
    await show(next, found);
  }

  /** 選んだ節を開く（下段の場面と参照ペイン）。前の場面の未保存は先に書く。 */
  async function show(id: string | null, token = generation): Promise<void> {
    selected = id;
    const scene = id ? sceneOf(id) : null;
    if (scene !== plotScene) {
      await flush();
      plotScene = scene;
      plot = '';
      plotMissing = '';
      if (scene) {
        const got = await session.run('plot.get', { level: 'episode', scene });
        if (token !== generation) {
          return;
        }
        if (got.ok) {
          // `level=episode` は MCP の戻り（`{ path, level, text }`）をそのまま返す。形を確かめて読む
          plot = episodeText(got.result);
        } else {
          plotMissing = `plots/${episodeOf(scene) ?? scene}.cnov`;
        }
      }
    }
    if (id) {
      const pane = await session.run('reference.pane', { scene: id });
      if (token === generation) {
        reference = pane.ok ? pane.result : null;
      }
    } else {
      reference = null;
    }
  }

  /** 節を選ぶ（クリックとキー）。 */
  async function select(id: string): Promise<void> {
    armed = null;
    await show(id);
  }

  /** 行を上下に動かす（素の `↑` / `↓`。一覧に効く。§9.8.6）。 */
  async function move(step: 1 | -1): Promise<void> {
    if (rows.length === 0) {
      return;
    }
    const at = rows.findIndex((row) => row.id === selected);
    const next = rows[Math.min(rows.length - 1, Math.max(0, at + step))];
    if (next && next.id !== selected) {
      await select(next.id);
      listEl?.querySelector<HTMLButtonElement>('[aria-current="true"]')?.focus();
    }
  }

  /** 下段の本文を入力した。 */
  function oninput(event: Event): void {
    plot = (event.target as HTMLTextAreaElement).value;
    dirty = true;
    schedule();
  }

  /** 自動保存（最後の打鍵から 400ms。§8.2 と同じ待ち）。 */
  function schedule(): void {
    if (timer !== null) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = null;
      void flush();
    }, 400);
  }

  /** 未保存の下段を `plot.write` で書く（場面の話プロットを丸ごと。核が節に割り直す）。 */
  async function flush(): Promise<void> {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    if (!dirty || plotScene === null || plotMissing !== '') {
      return;
    }
    const scene = plotScene;
    const text = plot;
    dirty = false;
    const written = await session.run('plot.write', { level: 'episode', role: '', scene, text });
    if (!written.ok) {
      dirty = true;
      session.notice(written.message, 'error', 0);
      return;
    }
    ondirty?.();
    // 書いた後は一覧（台詞比率・文字数）を読み直す。場面の本文は変えていないので下段はそのまま
    await reloadRows();
  }

  /** 節の一覧と本文だけを読み直す（下段と参照ペインは触らない）。 */
  async function reloadRows(): Promise<void> {
    const current = episode;
    if (!current) {
      return;
    }
    const list = await session.run('section.list', {});
    if (!list.ok || current !== episode) {
      return;
    }
    const inEpisode = sectionsInEpisode(sectionsOf(list.result), current);
    const texts = await Promise.all(
      inEpisode.map(async (item) => {
        const got = await session.run('section.get', { scene: item.id });
        return [item.id, got.ok ? got.result.text : ''] as const;
      }),
    );
    if (current !== episode) {
      return;
    }
    rows = inEpisode;
    prose = new Map(texts);
  }

  /** 削除（§8.1.2）。空の節は確認なし、空でない節は一度目で確認を出す。 */
  async function remove(id: string): Promise<void> {
    const empty = (prose.get(id) ?? '').trim() === '';
    if (!empty && armed !== id) {
      armed = id;
      return;
    }
    armed = null;
    const deleted = await session.run('section.delete', { role: '', scene: id });
    if (!deleted.ok) {
      session.notice(deleted.message, 'error', 0);
      return;
    }
    ondirty?.();
    await refresh(screen.scene);
  }

  /** 節を挿入（`Ctrl+Shift+N`。場面の最後の節の後ろ）。 */
  async function insert(): Promise<void> {
    await flush();
    const made = await session.run('plot.insert_section', { role: '', scene: selected ?? undefined });
    if (!made.ok) {
      session.notice(made.message, 'error', 0);
      return;
    }
    ondirty?.();
    plotScene = null;
    await show(selected);
  }

  /** 本文へ移る（`Ctrl+Enter`。§10.2。同じ節に着地する）。 */
  async function toWrite(): Promise<void> {
    await flush();
    if (selected) {
      await session.run('nav.section', { scene: selected });
    }
    const moved = await session.run('session.workspace_switch', { workspace: 'write' });
    if (!moved.ok) {
      session.notice(moved.message, 'error', 0);
    }
  }

  /** この版で出せない操作を、理由つきで知らせる（黙って何もしない、§21.1.2）。 */
  function unavailable(label: string): void {
    session.notice(t('workspace.unavailable', [label]), 'info');
  }

  /**
   * キー（このワークスペースの中だけで受ける。§9.8）。写しは `episodeKey`（テストで確かめる）。
   * 扱ったキーは `stopPropagation` して、シェルの同じ割当が二重に走らないようにする。
   */
  function onkey(event: KeyboardEvent): void {
    if (ime.active || composing) {
      return;
    }
    const inField = event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLInputElement;
    const found = episodeKey({ ...strokeOf(event), code: event.code });
    if (found === null || (inField && (found === 'up' || found === 'down'))) {
      return;
    }
    const handled = (run: () => void): void => {
      event.preventDefault();
      event.stopPropagation();
      run();
    };
    switch (found) {
      case 'new-section':
        handled(() => void insert());
        return;
      case 'to-write':
        handled(() => void toWrite());
        return;
      case 'swap-up':
      case 'swap-down':
        // 節の入れ替えは、対応する正規操作が無いので出せない（§21.1.2）
        handled(() => unavailable(t('stage.swap_plot_only')));
        return;
      case 'mode':
        handled(() => unavailable(t('stage.insert_mode_title')));
        return;
      case 'material':
        handled(() => unavailable(t('help.material')));
        return;
      case 'up':
      case 'down':
        handled(() => void move(found === 'up' ? -1 : 1));
        return;
      default:
        return;
    }
  }

  /** 画面を離れるとき、未保存の下段を書く。 */
  onDestroy(() => {
    void flush();
    ime.dispose();
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- キーは領域ごと受ける（ワークスペースの中だけ。§9.8）。領域は役割を持たないので `role="region"` -->
<div
  class="episode"
  role="application"
  aria-label={t('header.workspace.plot-episode')}
  on:keydown={onkey}
>
  <!-- 上段: 章プロットの該当話（読み取り専用。§10.2） -->
  <section class="upper" aria-label={t('pane.upper')}>
    {#if upper}
      <p class="upper-title">
        <span class="id">@{upper.id}</span>
        <span class="title">{upper.title}</span>
        {#if upper.state}<span class="state-name">{upper.state}</span>{/if}
      </p>
      {#if upper.summary}<p class="upper-summary">{upper.summary}</p>{/if}
    {:else if episode}
      <p class="empty">{t('episode.no_block', [chapterPath, upperMissing])}</p>
    {:else}
      <p class="empty">{t('episode.empty')}</p>
    {/if}
  </section>

  <!-- 中段: 節の一覧（1 行 1 節。右端に台詞比率バーと文字数） -->
  <section class="list-pane" aria-label={t('pane.plot')}>
    <header class="list-head">
      <span class="id">{episode ?? ''}</span>
      {#if episodeRatio !== null}
        <span class="bar" role="img" aria-label={`${t('stage.dialogue_ratio')} ${percent(episodeRatio)}`}>
          <span class="fill" style={`width:${Math.round(episodeRatio * 100)}%`}></span>
        </span>
        <span class="percent">{percent(episodeRatio)}</span>
      {/if}
    </header>
    <!-- 節ごとの `state:` と `when:` は読む操作が無いので出せない（§21.1.2）。字句のキーは翻訳しない（§12.6） -->
    <p class="note">{t('workspace.unavailable', [FIELD_STATE])} {t('workspace.unavailable', [FIELD_WHEN])}</p>
    <ul class="rows" bind:this={listEl} role="list">
      {#each view as item (item.row.id)}
        <li class="row-item">
          <button
            type="button"
            class="row"
            aria-current={selected === item.row.id ? 'true' : undefined}
            on:click={() => select(item.row.id)}
          >
            <span class="state-line" aria-hidden="true"></span>
            <span class="id">{item.row.id.split('-').slice(2).join('-')}</span>
            <span class="summary">{item.row.summary}</span>
            {#if !item.empty && item.ratio !== null}
              <span class="bar small" aria-hidden="true">
                <span class="fill" style={`width:${Math.round(item.ratio * 100)}%`}></span>
              </span>
            {/if}
            <span class="chars">
              {item.empty ? t('episode.no_prose') : t('episode.prose_chars', [String(item.chars)])}
            </span>
          </button>
          {#if armed === item.row.id}
            <span class="confirm">
              <span>{t('delete.confirm')}</span>
              <button type="button" class="danger" on:click={() => remove(item.row.id)}>{t('delete.yes')}</button>
            </span>
          {:else}
            <button type="button" class="del" aria-label={`${t('delete.button')} ${item.row.id}`} on:click={() => remove(item.row.id)}>
              {t('delete.button')}
            </button>
          {/if}
        </li>
      {:else}
        <li class="empty">{t('episode.empty')}</li>
      {/each}
    </ul>
  </section>

  <!-- 下段: 選んだ節の話プロット（場面の本文を操作で読み書きする） -->
  <section class="lower" aria-label={t('pane.plot')}>
    <header class="lower-head">
      <span class="id">{plotScene ?? ''}</span>
      <span class="title">{sceneTitle}</span>
    </header>
    {#if plotMissing}
      <p class="empty">{t('episode.no_plot', [plotMissing])}</p>
    {:else if plotScene}
      <textarea
        class="plot-text"
        aria-label={t('pane.plot')}
        spellcheck="false"
        value={plot}
        on:input={oninput}
        on:blur={() => flush()}
        on:compositionstart={() => ime.start()}
        on:compositionupdate={() => ime.start()}
        on:compositionend={() => ime.end()}
      ></textarea>
    {:else}
      <p class="empty">{t('episode.empty')}</p>
    {/if}
  </section>

  <aside class="pane">
    <ReferencePane {t} pane={reference} />
  </aside>
</div>

<style>
  .episode {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    grid-template-rows: auto minmax(0, 1fr) minmax(0, 1.2fr);
    grid-template-areas:
      'upper pane'
      'list pane'
      'lower pane';
    height: 100%;
    min-height: 0;
    color: var(--fg-primary);
  }
  .upper {
    grid-area: upper;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    font-size: 13px;
    color: var(--fg-muted);
  }
  .upper-title,
  .upper-summary {
    margin: 0;
  }
  .upper-title .id {
    font-family: var(--mono);
    margin-right: 8px;
  }
  .upper-title .title {
    color: var(--fg-primary);
    margin-right: 8px;
  }
  .state-name {
    font-family: var(--mono);
    font-size: 11px;
  }
  .list-pane {
    grid-area: list;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-bottom: 1px solid var(--border);
  }
  .list-head,
  .lower-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px;
    font-size: 13px;
  }
  .note {
    margin: 0;
    padding: 0 16px 4px;
    font-size: 11px;
    color: var(--fg-faint);
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
    min-height: 0;
    flex: 1;
  }
  .row-item {
    display: flex;
    align-items: center;
    gap: 4px;
    padding-right: 8px;
  }
  .row {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: 3px 64px minmax(0, 1fr) 48px auto;
    align-items: center;
    gap: 8px;
    padding: 4px 8px 4px 0;
    min-height: 24px;
    background: none;
    border: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
    text-decoration: underline;
    text-decoration-color: var(--border);
    text-underline-offset: 3px;
  }
  .row:hover {
    background: var(--bg-selection);
  }
  .row[aria-current='true'] {
    background: var(--bg-selection);
  }
  .state-line {
    align-self: stretch;
    width: 3px;
    background: var(--border);
  }
  .summary {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .chars {
    font-size: 10px;
    color: var(--fg-faint);
    white-space: nowrap;
  }
  .id {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--scene-id);
  }
  .bar {
    display: inline-block;
    width: 64px;
    height: 4px;
    background: var(--bg-inset);
    border-radius: 2px;
    overflow: hidden;
  }
  .bar.small {
    width: 48px;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--fg-faint);
  }
  .percent {
    font-size: 10px;
    color: var(--fg-faint);
  }
  .del,
  .danger {
    min-height: 24px;
    min-width: 24px;
    padding: 0 8px;
    font-size: 11px;
    background: none;
    border: 1px solid var(--border);
    color: var(--fg-muted);
    cursor: pointer;
  }
  .danger {
    color: var(--state-error);
  }
  .confirm {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: var(--fg-muted);
  }
  .lower {
    grid-area: lower;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .lower-head .title {
    color: var(--scene-title);
  }
  .plot-text {
    flex: 1;
    min-height: 0;
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
  .empty {
    margin: 8px 16px;
    font-size: 13px;
    color: var(--fg-muted);
  }
  .pane {
    grid-area: pane;
    min-height: 0;
    overflow: auto;
    border-left: 1px solid var(--border);
  }
</style>
