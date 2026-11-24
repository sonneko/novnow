<!-- 点検（§10.9 → §15.7）。9 タブ + 基本タブの内部切替。ジャンプは review.goto / nav.section → 編集画面へ（§10.0）。 -->
<script lang="ts">
  import type { Session } from '../../store/session';
  import type { Texts } from '../../i18n';
  import { chord, strokeOf } from '../../keyboard/keys';
  import {
    BASE_VIEWS,
    BASE_VIEW_LABEL,
    OBSERVATION_GROUPS,
    REVIEW_TABS,
    TAB_LABEL,
    defaultOnGroups,
    filterRows,
    groupRows,
    histogram,
    isBaseView,
    isOutlier,
    lengthSummary,
    memoRowsOf,
    placeOf,
    placeText,
    reviewAction,
    rowsOf,
    sectionChars,
    severityCounts,
    sortRows,
    tabState,
    tScores,
    treeRows,
    visibleTabs,
    moveTab as stepTab,
  } from './review';
  import type { BaseView, DiagRow, MemoRow, PlaceIndex, ReviewTab, TreeRow } from './review';
  import { placeKey, placeRef } from '../../reference/lookups';
  import { commentUsable, diffRows, proposalRowsOf, type DiffRow, type ProposalRow } from '../agent/agent';

  type Speaker = { name: string; chars: number; lines: number };
  type ThreadRow = { id: string; title: string; scope: string; status: string };
  type ThreadDetail = { messages: readonly { body: string; from: string; kind: string }[] };
  type Section = { id: string; lines: readonly number[]; path: string; summary: string };
  type Unmatched = { scene: string; speaker: string; text: string };
  type TreeNode = { id: string; summary: string; path: string };

  export let t: Texts['t'];
  export let session: Session;
  /** いま見ている節（§10.0: 開いたときの選択。空なら診断一覧の先頭）。 */
  export let scene = '';
  export let revision = 0;
  /** スレッドの確認へ移る（§10.9.6 の表）。 */
  export let onopenthread: ((id: string) => void) | null = null;
  /**
   * 診断の「どこ」を機能核へ問い合わせる（`diagnostics.place`。§10.9.2・§14.2.5）。
   *
   * 画面が診断の一覧を組み立てるときの `placeOf` は**画面が持つ索引**（節・ブロック・資料）で
   * 決める近似だが、`diagnostics.place` は機能核が文書を読んで決める**正本**である。
   * 選んでいる行について、機能核の答えを下に出す（近い場合も違う場合も、両方見える）。
   *
   * 渡されていなければ何もしない（未提供を黙って隠さないため、行の下に理由を書く）。
   */
  export let onplace:
    | ((path: string, byte: number | null) => Promise<{ place: unknown; path: string } | null>)
    | null = null;

  let root: HTMLElement | null = null;
  let loading = true;
  let tab: ReviewTab = 'base';
  let view: BaseView = 'diagnostics';
  let tabs: readonly ReviewTab[] = [...REVIEW_TABS];
  let rows: DiagRow[] = [];
  let codeFilter = '';
  let sortMode: 'position' | 'code' | 'severity' = 'position';
  let selected = 0;
  let obsSelected = 0;
  let propSelected = 0;
  let tree: TreeRow[] = [];
  let scores: (number | null)[] = [];
  let bands: { from: number; to: number; count: number }[] = [];
  let summary = { median: 0, mean: 0, max: 0, min: 0 };
  let speakers: Speaker[] = [];
  let dialogueRatio = 0;
  let index: PlaceIndex = { sections: [], blocks: [], materials: [] };
  let obsOn: Set<string> = new Set(defaultOnGroups());
  let unmatched: Unmatched[] = [];
  let memos: MemoRow[] = [];
  let proposals: ProposalRow[] = [];
  let selProposal: ProposalRow | null = null;
  let diff: DiffRow[] = [];
  let drifted = false;
  let threads: ThreadRow[] = [];
  let thread: ThreadDetail | null = null;
  let rejecting = false;
  let rejectComment = '';
  let lastScene = '\u0000';
  /** `diagnostics.place` の答え（選んでいる行のもの。`null` なら未取得）。 */
  let where: { path: string; place: unknown } | null = null;
  /** `diagnostics.place` の問い合わせ中。 */
  let placing = false;

  /** 表示中の診断（絞込・並べ替え後）。 */
  $: visible = sortRows(filterRows(rows, codeFilter), sortMode);

  // いま見ている節へ着地（§10.0）。scene が変わったときだけ。
  $: if (!loading && scene !== lastScene) {
    land(scene);
  }

  /** 重ねて起動したとき（project.toml の変更）。 */
  $: void bump(revision);
  let loadedRevision = -1;

  function land(at: string): void {
    lastScene = at;
    if (at === '') {
      selected = 0;
      return;
    }
    const found = rows.find((row) => placeOf(row.path, row.line, index)?.args.includes(at) === true);
    if (found) {
      view = 'diagnostics';
      selected = rows.indexOf(found);
    } else {
      selected = 0;
    }
  }

  /** 節・本文・診断などを読み直す（初回と revision の変更で）。 */
  async function load(): Promise<void> {
    loading = true;
    // 時系列タブの要否（§10.9.1: when: が 1 件も無ければ出さない）
    const when = await session.run('search.project', { query: 'when:' });
    tabs = visibleTabs(when.ok && ((when.result as { count?: number }).count ?? 0) > 0);
    if (!tabs.includes(tab)) {
      tab = 'base';
    }
    const list = await session.run('review.list');
    rows = list.ok ? rowsOf(list.result) : [];
    // 題（章 / 話 / 場面 / 節の 4 回分）
    const titles = new Map<string, string>();
    for (const level of ['chapter', 'episode', 'scene', 'section'] as const) {
      const got = await session.run('structure.tree', { level });
      for (const node of (got.ok ? ((got.result as { nodes?: TreeNode[] } | null)?.nodes ?? []) : []) as TreeNode[]) {
        titles.set(node.id, node.summary);
      }
    }
    // 節と本文の文字数（表示の数え。§9.6.1）
    const secs = await session.run('section.list', {});
    const sections = (secs.ok ? ((secs.result as { sections?: Section[] } | null)?.sections ?? []) : []).map((sec) => ({
      id: sec.id,
      lines: [sec.lines?.[0] ?? 0, sec.lines?.[1] ?? 0] as [number, number],
      path: sec.path ?? '',
      summary: sec.summary ?? '',
    }));
    const texts = new Map<string, string>();
    for (const sec of sections) {
      if (sec.path === '' || texts.has(sec.path)) {
        continue;
      }
      const doc = await session.run('document.get', { path: sec.path });
      if (doc.ok) {
        texts.set(sec.path, ((doc.result as { text?: unknown }).text ?? '') as string);
      }
    }
    const chars = new Map<string, number>();
    const sceneChars: number[] = [];
    for (const sec of sections) {
      const value = sectionChars((texts.get(sec.path) ?? '') as string, sec.lines);
      chars.set(sec.id, value);
      sceneChars.push(value);
    }
    // A 形式のブロックと状態（`plot.get level: all`。ブロックは「場所」、状態は進捗ツリーの列）
    const blocks: { id: string; path: string; lines: [number, number] }[] = [];
    const states = new Map<string, string>();
    const outlines = await session.run('plot.get', { level: 'all' });
    type OutlineBlock = {
      id?: unknown;
      lines?: number[];
      path?: unknown;
      state?: unknown;
      title?: unknown;
    };
    for (const block of (outlines.ok ? ((outlines.result as { blocks?: OutlineBlock[] } | null)?.blocks ?? []) : []) as OutlineBlock[]) {
      const id = typeof block.id === 'string' ? block.id : '';
      const path = typeof block.path === 'string' ? block.path : '';
      blocks.push({ id, path, lines: [block.lines?.[0] ?? 0, block.lines?.[1] ?? 0] });
      if (typeof block.state === 'string' && block.state !== '') {
        states.set(id, block.state);
      }
      if (typeof block.title === 'string' && block.title !== '') {
        titles.set(id, block.title);
      }
    }
    // 資料（「場所」の材料）
    const mats = await session.run('materials.list', {});
    const materials = (
      (mats.ok ? ((mats.result as unknown as { materials?: unknown[] } | null)?.materials ?? []) : []) as unknown[]
    ).map((row) => {
      const item = row as { id?: unknown; title?: unknown; path?: unknown };
      return {
        id: typeof item.id === 'string' ? item.id : '',
        title: typeof item.title === 'string' ? item.title : '',
        path: typeof item.path === 'string' ? item.path : '',
      };
    });
    index = {
      sections: sections.map((sec) => ({ id: sec.id, path: sec.path, lines: sec.lines })),
      blocks,
      materials,
    };
    tree = treeRows(titles, chars, states);
    scores = tScores(tree.map((row) => row.chars));
    bands = histogram(sceneChars, 100);
    summary = lengthSummary(sceneChars);
    const stats = await session.run('project.stats', {});
    if (stats.ok) {
      speakers = ((stats.result as { speakers?: Speaker[] }).speakers ?? []) as Speaker[];
      dialogueRatio = (stats.result as { dialogue_ratio?: number }).dialogue_ratio ?? 0;
    }
    const scan = await session.run('cast.scan', { role: '' });
    unmatched = (scan.ok ? ((scan.result as { unmatched?: Unmatched[] }).unmatched ?? []) : []) as Unmatched[];
    const found = await session.run('search.project', { query: '^(?:! |illustration:)', regex: true });
    memos = memoRowsOf((found.ok ? ((found.result as { hits?: { path: string; line: number; text: string }[] } | null)?.hits ?? []) : []) as {
      path: string;
      line: number;
      text: string;
    }[], sections);
    await loadProposals();
    await loadThreads();
    loading = false;
    land(scene);
  }

  async function bump(next: number): Promise<void> {
    if (next === loadedRevision) {
      return;
    }
    loadedRevision = next;
    await load();
  }

  async function loadProposals(): Promise<void> {
    const got = await session.run('agent.proposal_list');
    proposals = got.ok ? proposalRowsOf(got.result) : [];
    if (selProposal === null && proposals.length > 0) {
      await selectProposal(proposals[0]?.id ?? '');
    }
    if (propSelected >= proposals.length) {
      propSelected = Math.max(0, proposals.length - 1);
    }
  }

  async function selectProposal(id: string): Promise<void> {
    selProposal = proposals.find((found) => found.id === id) ?? null;
    diff = [];
    drifted = false;
    if (selProposal === null) {
      return;
    }
    const got = await session.run('agent.proposal_get', { id });
    if (got.ok) {
      const detail = (got.result as { proposal?: unknown } | null)?.proposal;
      const found = detail === null || detail === undefined ? selProposal : proposalRowsOf({ proposals: [detail] })[0];
      if (found) {
        selProposal = found;
        diff = diffRows(found.before, found.after);
        drifted = found.drifted;
      }
    }
  }

  async function loadThreads(): Promise<void> {
    const got = await session.run('agent.thread_list');
    threads = (got.ok ? ((got.result as { threads?: ThreadRow[] }).threads ?? []) : []) as ThreadRow[];
    thread = null;
    if (threads.length > 0) {
      await openThread(threads[0]?.id ?? '');
    }
  }

  async function openThread(id: string): Promise<void> {
    const got = await session.run('agent.thread_get', { id });
    thread = got.ok ? ((got.result as ThreadDetail | null) ?? null) : null;
    if (thread !== null && !Array.isArray(thread.messages)) {
      thread = { messages: [] };
    }
  }

  /** 診断の行 → 該当節へ（§10.9.2）。動きは第 1 の画面の review_jump と同じ（編集画面へ）。 */
  async function gotoDiag(row: DiagRow): Promise<void> {
    const ok = await session.run('review.goto', { index: row.index });
    if (ok.ok) {
      await session.run('session.workspace_switch', { workspace: 'write' });
    } else {
      session.notice(ok.message);
    }
  }

  /** 節 ID → 編集画面のその節へ（§10.9.3 のジャンプ）。 */
  async function gotoScene(id: string): Promise<void> {
    const ok = await session.run('nav.section', { scene: id });
    if (ok.ok) {
      await session.run('session.workspace_switch', { workspace: 'write' });
    } else {
      session.notice(ok.message);
    }
  }

  function gotoMemos(row: MemoRow): void {
    if (row.scene !== '') {
      void gotoScene(row.scene);
    }
  }

  /** 文体観察で今出している行（フィルタの ON の指標だけ。役割語未検出は cast.scan）。 */
  function obsRows(): { row: DiagRow }[] {
    const out: { row: DiagRow }[] = [];
    for (const group of groupRows(rows, obsOn)) {
      for (const row of group.rows) {
        out.push({ row });
      }
    }
    return out;
  }

  function obsCount(): number {
    return obsRows().length + (obsOn.has('cast.tab_unmatched') ? unmatched.length : 0);
  }

  function obsAt(at: number): { row: DiagRow | null; unmatched: Unmatched | null } {
    const list = obsRows();
    if (at < list.length) {
      return { row: list[at]?.row ?? null, unmatched: null };
    }
    const un = unmatched[at - list.length];
    return { row: null, unmatched: un ?? null };
  }

  function statusRow(row: DiagRow): string {
    return row.severity;
  }

  /**
   * 選んでいる行の「どこ」を機能核へ聞く（`diagnostics.place`。§10.9.2）。
   *
   * 画面の索引（`placeOf`）は近い答えを出すが、機能核の答えが正本である。行が変わったときだけ
   * 問い合わせる（同じ行で何度も呼ばない）。`onplace` が無ければ何もしない。
   */
  async function askPlace(row: DiagRow | null): Promise<void> {
    if (!row || !onplace) {
      where = null;
      return;
    }
    placing = true;
    try {
      where = await onplace(row.path, row.byte);
    } finally {
      placing = false;
    }
  }

  /** 選んだ行が変わったら「どこ」を取り直す（行の同一性は `index` で見る）。 */
  let placedIndex = -2;
  let chosen: DiagRow | null = null;
  $: {
    const row = view === 'diagnostics' ? (visible[selected] ?? null) : null;
    chosen = row;
    // リアクティブ文の中では `await` できないので、状態の更新だけをここで行い、
    // 問い合わせは別の文から始める（`placing` の切り替えはその中）。
    if ((row?.index ?? -1) !== placedIndex) {
      placedIndex = row?.index ?? -1;
      void askPlace(row);
    }
  }

  function placeRow(row: DiagRow): string {
    return placeText(t, placeOf(row.path, row.line, index), { path: row.path, line: row.line, column: row.column });
  }

  /**
   * `diagnostics.place` の `place` を `place.*` の文言へ写す（知らない種別は null → 生のパス）。
   *
   * 写しは `lookups.ts` の 1 か所だけが持つ（画面ごとに表を作り直さない）。
   */
  function placeKeyOf(value: unknown): { key: string; args: string[] } | null {
    const found = placeRef(value);
    return found === null ? null : placeKey(found);
  }

  function treeTitle(row: TreeRow): string {
    return row.title === '' ? row.id : row.title;
  }

  function treeScore(at: number): string {
    const score = scores[at];
    if (score === null || score === undefined) {
      return '—';
    }
    return `T ${score}` + (isOutlier(score) ? ' ⚠W091' : '');
  }

  /** 画面のキー（§10.9 の表。アプリのショートカットはキーボード層）。 */
  function onkey(event: KeyboardEvent): void {
    if (root === null || !root.contains(event.target as Node | null)) {
      return;
    }
    const target = event.target as HTMLElement | null;
    if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) {
      return;
    }
    const key = chord(strokeOf(event));
    const action = reviewAction(key);
    if (action === null) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    switch (action) {
      case 'tab-prev':
        tab = stepTab(tab, -1, tabs);
        return;
      case 'tab-next':
        tab = stepTab(tab, 1, tabs);
        return;
      case 'rerun':
        void load();
        return;
      case 'open':
        openCurrent();
        return;
      case 'prev':
      case 'next':
      case 'first':
      case 'last':
        moveSelection(action);
        return;
    }
  }

  function openCurrent(): void {
    const diag = tab === 'base' && view === 'diagnostics' ? visible[selected] : undefined;
    const node = tab === 'base' && view === 'progress' ? tree[selected] : undefined;
    const threadRow = tab === 'base' && view === 'threads' ? threads[selected] : undefined;
    const proposal = tab === 'proposals' ? proposals[propSelected] : undefined;
    if (diag) {
      void gotoDiag(diag);
    } else if (node) {
      const id = node.id;
      if (id.split('-').length === 4) {
        void gotoScene(id);
      } else {
        void session.run('session.workspace_switch', { workspace: 'plot-outline' });
      }
    } else if (threadRow) {
      const scope = (threadRow.scope ?? '').replace(/^@/u, '');
      if (scope.split('-').length === 4) {
        void gotoScene(scope);
      }
    } else if (tab === 'observations') {
      const at = obsAt(obsSelected);
      if (at.row) {
        void gotoDiag(at.row);
      } else if (at.unmatched) {
        gotoMemos({ scene: at.unmatched.scene, path: '', line: 0, text: at.unmatched.text });
      }
    } else if (tab === 'illustrations') {
      const row = memos[obsSelected];
      if (row) {
        gotoMemos(row);
      }
    } else if (proposal) {
      void selectProposal(proposal.id);
    }
  }

  function moveSelection(action: 'prev' | 'next' | 'first' | 'last'): void {
    const at =
      tab === 'observations'
        ? obsSelected
        : tab === 'proposals'
          ? propSelected
          : tab === 'illustrations'
            ? obsSelected
            : selected;
    const size =
      tab === 'observations'
        ? obsCount()
        : tab === 'proposals'
          ? proposals.length
          : tab === 'illustrations'
            ? memos.length
            : tab === 'base' && view === 'diagnostics'
              ? visible.length
              : tab === 'base' && view === 'progress'
                ? tree.length
                : threads.length;
    const next =
      action === 'first'
        ? 0
        : action === 'last'
          ? Math.max(0, size - 1)
          : Math.min(Math.max(0, size - 1), at + (action === 'next' ? 1 : -1));
    if (tab === 'observations') {
      obsSelected = next;
    } else if (tab === 'proposals') {
      propSelected = next;
      if (proposals[next]) {
        void selectProposal(proposals[next]?.id ?? '');
      }
    } else if (tab === 'illustrations') {
      obsSelected = next;
    } else {
      selected = next;
      if (view === 'threads' && threads[next]) {
        void openThread(threads[next]?.id ?? '');
      }
    }
  }

  function moveBase(next: BaseView): void {
    view = isBaseView(next) ? next : 'diagnostics';
    selected = 0;
    if (view === 'threads' && threads.length > 0) {
      void openThread(threads[0]?.id ?? '');
    }
  }

  function moveTab(next: ReviewTab): void {
    if (tabs.includes(next)) {
      tab = next;
    }
    obsSelected = 0;
    propSelected = 0;
    if (tab === 'proposals') {
      void loadProposals();
    }
  }

  function toggleObs(labelKey: string): void {
    const next = new Set(obsOn);
    if (next.has(labelKey)) {
      next.delete(labelKey);
    } else {
      next.add(labelKey);
    }
    obsOn = next;
    obsSelected = 0;
  }

  /** 却下（確認付き。`agent.comment_required`）。 */
  async function reject(): Promise<void> {
    if (!commentUsable(rejectComment)) {
      session.notice(t('agent.comment_required'));
      return;
    }
    if (selProposal !== null) {
      const ok = await session.run('agent.proposal_reject', {
        id: selProposal.id,
        role: '',
        comment: rejectComment.trim(),
      });
      if (ok.ok) {
        session.notice(t('agent.rejected', [selProposal.id]));
      } else {
        session.notice(ok.message);
      }
    }
    rejectComment = '';
    rejecting = false;
    await loadProposals();
  }

  /** 採用（確認付き）。 */
  async function accept(): Promise<void> {
    if (selProposal === null) {
      return;
    }
    const ok = await session.run('agent.proposal_apply', { id: selProposal.id, role: '' });
    if (ok.ok) {
      session.notice(t('agent.accepted', [selProposal.id]));
    } else {
      session.notice(ok.message);
    }
    await loadProposals();
  }
</script>

<svelte:window on:keydown={onkey} />

<section class="review" aria-label={t('header.workspace.review')} bind:this={root}>
  {#if !loading}
    <nav class="tabs" aria-label={t('header.workspace.review')}>
      {#each tabs as key (key)}
        <button
          type="button"
          class:current={tab === key}
          aria-current={tab === key ? 'page' : undefined}
          on:click={() => moveTab(key)}
          >{t(TAB_LABEL[key])}</button
        >
      {/each}
    </nav>

    {#if tab === 'base'}
      <nav class="views" aria-label={t('review.tab_diagnostics')}>
        {#each BASE_VIEWS as key (key)}
          <button type="button" class:current={view === key} on:click={() => moveBase(key)}
            >{t(BASE_VIEW_LABEL[key])}</button
          >
        {/each}
      </nav>
      {#if view === 'diagnostics'}
        <div class="bar">
          <label class="inline">
            {t('review.settings_filter_hint')}
            <input bind:value={codeFilter} />
          </label>
          <button type="button" class:current={sortMode === 'position'} on:click={() => (sortMode = 'position')}
            >{t('review.sort_position')}</button
          >
          <button type="button" class:current={sortMode === 'code'} on:click={() => (sortMode = 'code')}
            >{t('review.sort_code')}</button
          >
          <button type="button" class:current={sortMode === 'severity'} on:click={() => (sortMode = 'severity')}
            >{t('review.sort_severity')}</button
          >
          <span class="count">{t('review.errors', [String(severityCounts(rows).error)])}</span>
          <span class="count">{t('review.warnings', [String(severityCounts(rows).warning)])}</span>
          <span class="count">{t('review.infos', [String(severityCounts(rows).info)])}</span>
        </div>
        {#if visible.length === 0}
          <p class="empty">{t('review.no_diagnostics')}</p>
        {:else}
          <div class="list" role="listbox" aria-label={t('review.tab_diagnostics')}>
            <table class="rows">
              <tbody>
                {#each visible as row, at (row.index)}
                  <tr
                    role="option"
                    aria-selected={selected === at}
                    class:selt={selected === at}
                    tabindex={selected === at ? 0 : -1}
                    on:click={() => {
                      selected = at;
                    }}
                    on:dblclick={() => void gotoDiag(row)}
                  >
                    <td class="code">{row.code}</td>
                    <td class="sev">{statusRow(row)}</td>
                    <td class="place">{placeRow(row)}</td>
                    <td class="msg">{row.message}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
          <!-- 機能核が決めた「どこ」（`diagnostics.place`。§10.9.2）。行の `placeOf` は画面の
               索引による近似なので、正本の答えを別の行に出す（黙って置き換えない）。 -->
          {#if chosen}
            <p class="where" role="status">
              <span class="where-label">{t('api.op.diagnostics.place')}</span>
              {#if !onplace}
                <span class="where-none">{t('workspace.unavailable', [t('api.op.diagnostics.place')])}</span>
              {:else if placing}
                <span class="where-none">{t('project.pick.loading')}</span>
              {:else if where === null}
                <span class="where-none">{t('definition.none')}</span>
              {:else}
                <span class="where-text">
                  {placeText(
                    t,
                    placeKeyOf(where.place),
                    { path: where.path, line: chosen.line, column: chosen.column },
                  )}
                </span>
              {/if}
            </p>
          {/if}
          <p class="hint">{t('review.key_select')} {t('review.key_open')} {t('review.rerun_button')}</p>
        {/if}
      {:else if view === 'progress'}
        {#if tree.length === 0}
          <p class="empty">{t('review.no_progress')}</p>
        {:else}
          <div class="list" role="listbox" aria-label={t('review.tab_progress')}>
          {#each tree as row, at (row.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
            <div
              role="option"
              aria-selected={selected === at}
              class="row"
              class:selt={selected === at}
              tabindex={selected === at ? 0 : -1}
              style={`--depth:${row.depth}`}
              on:click={() => {
                selected = at;
              }}
              on:dblclick={() => {
                if (row.id.split('-').length === 4) {
                  void gotoScene(row.id);
                } else {
                  void session.run('session.workspace_switch', { workspace: 'plot-outline' });
                }
              }}
            >
              <span class="id">{row.id}</span>
              <span class="summary">{treeTitle(row)}</span>
              <span class="chars">{t('review.chars', [String(row.chars)])}</span>
              <span class="chars">{row.state ?? '—'}</span>
              <span class="chars">{treeScore(at)}</span>
            </div>
          {/each}
        </div>
        <p class="hint">{t('workspace.unavailable', [t('stage.length_target_ratio')])}</p>
        {/if}
      {:else}
        {#if threads.length === 0}
          <p class="empty">{t('review.no_threads')}</p>
        {:else}
          <div class="split">
            <div class="list" role="listbox" aria-label={t('review.tab_threads')}>
              {#each threads as row, at (row.id)}
                <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                <div
                  role="option"
                  aria-selected={selected === at}
                  class="row"
                  class:selt={selected === at}
                  tabindex={selected === at ? 0 : -1}
                  on:click={() => {
                    selected = at;
                    void openThread(row.id);
                  }}
                >
                  <span class="summary">{row.title}</span>
                  <span class="sev">{row.status}</span>
                </div>
              {/each}
            </div>
            {#if thread !== null}
              <div class="thread">
                <div class="actions">
                  <button
                    type="button"
                    on:click={() => {
                      const picked = threads[selected];
                      if (onopenthread && picked) {
                        onopenthread(picked.id);
                        void session.run('session.workspace_switch', { workspace: 'agent' });
                      }
                    }}
                    >{t('review.open_in_agent')}</button
                  >
                </div>
                {#each thread.messages as message, at (at)}
                  <article>
                    <span class="from">{message.from}</span>
                    <pre>{message.body}</pre>
                  </article>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/if}
    {:else if tab === 'timeline'}
      <!-- 時系列（§10.9.7）。`when:` は並ぶが、並べ替える操作が無いため「未提供」（§21.1.2）。 -->
      <p class="empty">{t('workspace.unavailable', [t('stage.timeline')])}</p>
      <p class="hint">{t('stage.advisory')}</p>
    {:else if tab === 'speakers'}
      {#if speakers.length === 0}
        <p class="empty">{t('workspace.unavailable', [t('stage.speakers')])}</p>
      {:else}
        <div class="list" role="table" aria-label={t('stage.speakers')}>
          <div class="row head">
            <span class="summary">{t('stage.speaker_columns')}</span>
          </div>
          {#each speakers as speaker (speaker.name)}
            <div class="row" role="row">
              <span class="id">{speaker.name}</span>
              <span class="chars">{t('review.chars', [String(speaker.lines)])}</span>
              <span class="chars">{t('review.chars', [String(speaker.chars)])}</span>
              <span class="chars">—</span>
              <span class="chars">—</span>
            </div>
          {/each}
        </div>
        <p class="hint">{t('stage.dialogue_ratio', [String(Math.round(dialogueRatio * 10) / 10)])}</p>
        <p class="hint">{t('workspace.unavailable', [t('stage.speaker_columns')])}</p>
      {/if}
    {:else if tab === 'histogram'}
      {#if tabState('histogram') === 'partial'}
        <p class="hint">{t('workspace.unavailable', [t('stage.length_target_ratio')])}</p>
      {/if}
      {#if bands.length === 0}
        <p class="empty">{t('workspace.unavailable', [t('stage.histogram')])}</p>
      {:else}
        <div class="list" role="list" aria-label={t('stage.histogram')}>
          {#each bands as band (band.from)}
            <div class="row bar-row">
              <span class="id">{band.from}–{band.to}</span>
              <span class="bar" style:width={`${Math.round((band.count * 100) / Math.max(1, ...bands.map((b) => b.count)))}%`}></span>
              <span class="chars">{t('find.count', [String(band.count)])}</span>
            </div>
          {/each}
        </div>
        <p class="hint">
          {t('stage.length_summary', [
            String(summary.median),
            String(summary.mean),
            String(summary.max),
            String(summary.min),
          ])}
        </p>
        {#each tree as row, at (row.id)}
          {#if isOutlier(scores[at] ?? null)}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="row" on:dblclick={() => void gotoScene(row.id)}>
              <span class="id">{row.id}</span>
              <span class="summary">{treeTitle(row)}</span>
              <span class="chars">{treeScore(at)}</span>
            </div>
          {/if}
        {/each}
      {/if}
    {:else if tab === 'observations'}
      <div class="bar" role="group" aria-label={t('stage.obs_filter_label')}>
        {#each OBSERVATION_GROUPS as group (group.labelKey)}
          <label class="inline">
            <input type="checkbox" checked={obsOn.has(group.labelKey)} on:change={() => toggleObs(group.labelKey)} />
            {t(group.labelKey)}
          </label>
        {/each}
      </div>
      {#if obsCount() === 0}
        <p class="empty">{t('workspace.unavailable', [t('stage.observations')])}</p>
      {:else}
        <div class="list" role="listbox" aria-label={t('stage.observations')}>
          {#each obsRows() as entry, at (entry.row.index)}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
            <div
              role="option"
              aria-selected={obsSelected === at}
              class="row"
              class:selt={obsSelected === at}
              tabindex={obsSelected === at ? 0 : -1}
              on:click={() => {
                obsSelected = at;
              }}
              on:dblclick={() => void gotoDiag(entry.row)}
            >
              <span class="code">{entry.row.code}</span>
              <span class="place">{placeRow(entry.row)}</span>
              <span class="msg">{entry.row.message}</span>
            </div>
          {/each}
          {#if obsOn.has('cast.tab_unmatched')}
            {#each unmatched as found, at (at)}
              {@const at2 = obsRows().length + at}
              <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
              <div
                role="option"
                aria-selected={obsSelected === at2}
                class="row"
                class:selt={obsSelected === at2}
                tabindex={obsSelected === at2 ? 0 : -1}
                on:click={() => {
                  obsSelected = at2;
                }}
                on:dblclick={() => gotoMemos({ scene: found.scene, path: '', line: 0, text: found.text })}
              >
                <span class="code">{t('cast.tab_unmatched')}</span>
                <span class="place">{found.scene}</span>
                <span class="msg">{found.speaker}: {found.text}</span>
              </div>
            {/each}
          {/if}
        </div>
        <p class="hint">{t('stage.advisory')}</p>
        {#if obsOn.has('stage.obs_sentence_lengths')}
          <p class="hint">{t('workspace.unavailable', [t('stage.obs_sentence_lengths')])}</p>
        {/if}
      {/if}
    {:else if tab === 'stats'}
      <p class="empty">{t('workspace.unavailable', [t('stage.stats')])}</p>
    {:else if tab === 'senses'}
      <p class="empty">{t('workspace.unavailable', [t('stage.senses')])}</p>
      <p class="hint">{t('stage.advisory')}</p>
    {:else if tab === 'illustrations'}
      {#if memos.length > 0}
        <div class="list" role="listbox" aria-label={t('stage.illustrations')}>
          {#each memos as row, at (row.line + row.text)}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
            <div
              role="option"
              aria-selected={obsSelected === at}
              class="row"
              class:selt={obsSelected === at}
              tabindex={obsSelected === at ? 0 : -1}
              on:click={() => {
                obsSelected = at;
              }}
              on:dblclick={() => gotoMemos(row)}
            >
              <span class="id">{row.scene}</span>
              <span class="msg">{row.text}</span>
            </div>
          {/each}
        </div>
      {/if}
    {:else if tab === 'proposals'}
      {#if proposals.length > 0}
        <div class="split">
          <div class="list" role="listbox" aria-label={t('stage.proposals')}>
            {#each proposals as row, at (row.id)}
              <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
              <div
                role="option"
                aria-selected={propSelected === at}
                class="row"
                class:selt={propSelected === at}
                tabindex={propSelected === at ? 0 : -1}
                on:click={() => {
                  propSelected = at;
                  void selectProposal(row.id);
                }}
              >
                <span class="code">{row.target}</span>
                <span class="msg">{row.note}</span>
                <span class="sev">{row.status}</span>
              </div>
            {/each}
          </div>
          {#if selProposal !== null}
            <div class="thread">
              <div class="actions">
                <button type="button" on:click={() => void accept()}>{t('stage.accept')}</button>
                <button type="button" class="danger" on:click={() => (rejecting = true)}>{t('agent.reject_title')}</button>
              </div>
              {#if drifted}
                <p class="hint">{t('agent.review.drifted')}</p>
              {/if}
              {#each diff as row, at (at)}
                {#if row.kind === 'same'}
                  {#if row.before !== null}<pre class="diff-same">{row.before}</pre>{/if}
                {:else if row.kind === 'before'}
                  <pre class="diff-del">{row.before ?? ''}</pre>
                {:else}
                  <pre class="diff-add">{row.after ?? ''}</pre>
                {/if}
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    {/if}
  {/if}

  {#if rejecting}
    <div class="modal" role="dialog" aria-label={t('agent.reject_title')}>
      <p>{t('agent.reject_title')}</p>
      <textarea bind:value={rejectComment} on:keydown={(event) => event.stopPropagation()}></textarea>
      <p class="hint">{t('agent.comment_required')}</p>
      <div class="actions">
        <button type="button" class="danger" on:click={() => void reject()}>{t('stage.confirm')}</button>
        <button type="button" on:click={() => (rejecting = false)}>{t('modal.cancel')}</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .review {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px;
    height: 100%;
    overflow: hidden;
    background: var(--bg-base);
    color: var(--fg-primary);
    font-size: 13px;
    line-height: 1.6;
  }
  .tabs,
  .views,
  .bar {
    display: flex;
    gap: 4px;
    align-items: center;
    flex-wrap: wrap;
  }
  button {
    cursor: pointer;
    border-radius: 4px;
    padding: 2px 8px;
    background: transparent;
    color: var(--fg-primary);
    border: 1px solid var(--border);
    font-size: 12px;
    line-height: 1.6;
  }
  button.current,
  [aria-current='page'] {
    background: var(--bg-selection);
    border-color: var(--accent);
    color: var(--fg-primary);
  }
  button.danger {
    border-color: var(--state-error);
    color: var(--state-error);
  }
  input,
  textarea {
    background: var(--bg-inset);
    color: var(--fg-primary);
    border: 1px solid var(--border);
    border-radius: 2px;
  }
  .list {
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px;
    flex: 1;
    min-height: 0;
    font-size: 12px;
    line-height: 1.6;
  }
  .row {
    display: flex;
    gap: 6px;
    padding: 2px 6px;
    cursor: pointer;
    border-radius: 2px;
  }
  .row.head {
    cursor: default;
    color: var(--fg-muted);
  }
  .row[aria-selected='true'],
  .row.selt {
    background: var(--bg-selection);
  }
  .row.bar-row .bar {
    flex: 1;
    max-width: 60%;
    background: var(--accent);
    height: 2px;
    border-radius: 2px;
  }
  .split {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    flex: 1;
    min-height: 0;
  }
  .thread {
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
  }
  .thread pre {
    white-space: pre-wrap;
    margin: 0;
    font-family: var(--mono);
    font-size: 12px;
    line-height: 1.6;
  }
  .diff-add {
    color: var(--state-ok);
  }
  .diff-del {
    color: var(--state-error);
    text-decoration: line-through;
    text-decoration-color: var(--state-error);
  }
  .diff-same {
    color: var(--fg-muted);
  }
  .empty,
  .hint,
  .count,
  .from {
    color: var(--fg-muted);
    font-size: 12px;
    line-height: 1.6;
    margin: 0;
  }
  .code,
  .id,
  .chars {
    font-family: var(--mono);
    font-size: 12px;
  }
  .sev {
    color: var(--state-warn);
  }
  .where {
    display: flex;
    gap: 6px;
    margin: 4px 0 0;
    padding: 4px 6px;
    background: var(--bg-inset);
    border-radius: 2px;
    font-size: 11px;
  }
  .where-label {
    color: var(--fg-faint);
  }
  .where-none {
    color: var(--fg-faint);
  }
  .where-text {
    color: var(--fg-primary);
    overflow-wrap: anywhere;
  }
  .place {
    color: var(--fg-muted);
    white-space: nowrap;
  }
  .msg {
    flex: 1;
    line-height: 1.6;
  }
  .summary {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .inline {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    color: var(--fg-muted);
    font-size: 12px;
    line-height: 1.6;
  }
  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 40ch;
    z-index: 10;
  }
  .actions {
    display: flex;
    gap: 4px;
  }
</style>
