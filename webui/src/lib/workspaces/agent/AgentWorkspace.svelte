<!-- 管制（§10.8 → §15.7）。4 列: ①tasks ②context ③review ④delegation。 -->
<script lang="ts">
  import { tick } from 'svelte';
  import type { Session } from '../../store/session';
  import type { Texts } from '../../i18n';
  import { chord, strokeOf } from '../../keyboard/keys';
  import {
    COLUMN_LABEL,
    agentAction,
    blockBudget,
    budgetRatio,
    canExclude,
    commentUsable,
    delegationRowsOf,
    diffRows,
    levelTone,
    moveColumn,
    moveInColumn,
    packetBlocks,
    progressCounts,
    proposalRowsOf,
    readyOf,
    runConfirm,
    sceneOfScope,
    settled,
    startedByAgent,
    statusTone,
    taskRowsOf,
    threadRowsOf,
    toggleSelect,
  } from './agent';
  import type {
    Column,
    DelegationRow,
    DiffRow,
    PacketBlock,
    ProposalRow,
    TaskRow,
    ThreadRow,
  } from './agent';

  type ServerRow = { name: string; missing: string[] };
  type RunConfirmView = ReturnType<typeof runConfirm>;

  export let t: Texts['t'];
  export let session: Session;
  /** 点検から開いたスレッド（§10.9.6 の表）。 */
  export let focusThread = '';
  export let revision = 0;

  let root: HTMLElement | null = null;
  let column: Column = 'tasks';
  let tasks: TaskRow[] = [];
  let current: string | null = null;
  let checked: string[] = [];
  let packet: PacketBlock[] = [];
  let packetChars = 0;
  let cap = 12000;
  let excluded = new Set<string>();
  let blockSel = 0;
  let dryRun: { chars: number; path: string } | null = null;
  let proposal: ProposalRow | null = null;
  let resultText = '';
  let diff: DiffRow[] = [];
  let threads: ThreadRow[] = [];
  let threadId = '';
  let delegation: { enabled: boolean; levels: DelegationRow[]; maxParallel: number } = {
    enabled: false,
    levels: [],
    maxParallel: 2,
  };
  let levelSel = 0;
  let servers: ServerRow[] = [];
  let creating = false;
  let confirming = false;
  let confirmView: RunConfirmView | null = null;
  let rejecting = false;
  let rejectComment = '';
  let mcpOpen = false;
  let mcpText = '';
  let newRole = 'writer';
  let newScope = '';
  let newIntent = '';
  let newComment = '';

  $: visible = tasks;
  $: progress = progressCounts(tasks);
  $: thread = threads.find((found) => found.id === threadId) ?? null;

  /** 重ねて起動したとき（project.toml の変更）。 */
  $: void bump(revision);
  let loadedRevision = -1;

  async function bump(next: number): Promise<void> {
    if (next === loadedRevision) {
      return;
    }
    loadedRevision = next;
    await reload();
  }

  async function reload(): Promise<void> {
    const got = await session.run('agent.task_list');
    tasks = got.ok ? taskRowsOf(got.result) : [];
    if (current === null || !tasks.some((task) => task.id === current)) {
      current = tasks[0]?.id ?? null;
    }
    const set = await session.run('settings.get');
    const agents = (set.ok ? ((set.result as { agents?: Record<string, unknown> } | null)?.agents ?? {}) : {}) as Record<
      string,
      unknown
    >;
    cap = typeof agents.max_chars_per_request === 'number' ? agents.max_chars_per_request : 12000;
    await loadPacket();
    await loadReview();
    await loadThreads();
    await loadDelegation();
    await loadServers();
    if (focusThread !== '') {
      threadId = focusThread;
    }
  }

  function task(): TaskRow | null {
    return tasks.find((found) => found.id === current) ?? null;
  }

  /** ② コンテキスト（`context.build`）。作れなければ「まだ」（§15.8.2）。 */
  async function loadPacket(): Promise<void> {
    packet = [];
    packetChars = 0;
    dryRun = null;
    const at = task();
    const scene = at ? sceneOfScope(at.scope) : null;
    if (at === null || scene === null) {
      return;
    }
    const got = await session.run('context.build', { role: at.role, scene });
    if (got.ok) {
      const built = got.result as { chars?: number; packet?: string };
      packetChars = built.chars ?? 0;
      packet = packetBlocks(built.packet ?? '');
    }
  }

  /** ③ 結果レビュー（`agent.task_get`）。 */
  async function loadReview(): Promise<void> {
    proposal = null;
    resultText = '';
    diff = [];
    const at = task();
    if (at === null) {
      return;
    }
    const got = await session.run('agent.task_get', { id: at.id });
    if (!got.ok) {
      return;
    }
    const detail = (got.result as { proposal?: unknown; result?: unknown } | null) ?? {};
    if (detail.proposal !== null && detail.proposal !== undefined) {
      proposal = proposalRowsOf({ proposals: [detail.proposal] })[0] ?? null;
      if (proposal !== null) {
        diff = diffRows(proposal.before, proposal.after);
      }
    }
    resultText = typeof detail.result === 'string' ? detail.result : '';
  }

  async function loadThreads(): Promise<void> {
    const got = await session.run('agent.thread_list');
    threads = got.ok ? threadRowsOf(got.result) : [];
    if (threadId === '' && threads.length > 0) {
      threadId = threads[0]?.id ?? '';
    }
  }

  async function loadDelegation(): Promise<void> {
    const got = await session.run('agent.delegation_get');
    if (got.ok) {
      const found = got.result as { enabled?: boolean; levels?: unknown; max_parallel?: number } | null;
      delegation = {
        enabled: found?.enabled === true,
        levels: delegationRowsOf(found),
        maxParallel: typeof found?.max_parallel === 'number' ? found.max_parallel : 2,
      };
    }
  }

  async function loadServers(): Promise<void> {
    const got = await session.run('agent.mcp_config_get');
    servers = got.ok
      ? (((got.result as unknown as { servers?: unknown[] } | null)?.servers ?? []) as unknown[]).map((row) => {
          const pair = row as [string, string[]];
          return { name: pair[0] ?? '', missing: pair[1] ?? [] };
        })
      : [];
  }

  /** 書き込みは `role` 付きで関門を通す（Web UI では止まる。§15.8.3）。 */
  async function run(
    op: 'agent.task_create' | 'agent.task_run' | 'agent.task_dry_run' | 'agent.task_stop' | 'agent.stop_all' | 'agent.proposal_apply' | 'agent.proposal_reject' | 'agent.delegation_set',
    args: Record<string, unknown>,
  ): Promise<{ ok: boolean; result: unknown; message: string }> {
    const got = await session.run(op, { ...args, role: '' } as never);
    if (!got.ok) {
      session.notice(got.message);
      return { ok: false, result: null, message: got.message };
    }
    return { ok: true, result: got.result, message: '' };
  }

  /** ①の操作（§15.7 の表）。 */
  async function act(action: ReturnType<typeof agentAction>): Promise<void> {
    switch (action) {
      case 'left':
        column = moveColumn(column, -1);
        return;
      case 'right':
        column = moveColumn(column, 1);
        return;
      case 'prev':
        moveSel(-1);
        return;
      case 'next':
        moveSel(1);
        return;
      case 'first':
        moveSel('first');
        return;
      case 'last':
        moveSel('last');
        return;
      case 'new':
        creating = true;
        await tick();
        document.getElementById('agent-new-role')?.focus();
        return;
      case 'run': {
        const at = task();
        if (at === null) {
          return;
        }
        const got = await run('agent.task_run', { id: at.id });
        if (got.ok) {
          const queued = (got.result as { notices?: unknown[] }).notices ?? [];
          for (const notice of queued) {
            session.notice(String(notice));
          }
        }
        await reload();
        return;
      }
      case 'stop': {
        const at = task();
        if (at === null) {
          return;
        }
        await run('agent.task_stop', { id: at.id });
        await reload();
        return;
      }
      case 'dry-run': {
        const at = task();
        if (at === null) {
          return;
        }
        const got = await run('agent.task_dry_run', { id: at.id });
        if (got.ok) {
          const found = got.result as { chars?: number; path?: string };
          dryRun = { chars: found.chars ?? 0, path: found.path ?? '' };
        }
        return;
      }
      case 'select': {
        const at = task();
        if (at !== null) {
          checked = toggleSelect(checked, at.id);
        }
        return;
      }
      case 'select-all':
        checked = readyOf(tasks).map((found) => found.id);
        return;
      case 'run-batch': {
        if (checked.length === 0) {
          session.notice(t('agent.no_selection'));
          return;
        }
        // 合計入力は各タスクの `context.build` から（1 件でも作れなければ出さない。§15.7.5(1)）
        const charsById = new Map<string, number>();
        for (const at of tasks) {
          if (!checked.includes(at.id)) {
            continue;
          }
          const scene = sceneOfScope(at.scope);
          if (scene === null) {
            continue;
          }
          const built = await session.run('context.build', { role: at.role, scene });
          if (built.ok) {
            charsById.set(at.id, (built.result as { chars?: number }).chars ?? 0);
          }
        }
        confirmView = runConfirm(checked, charsById);
        await loadServers();
        confirming = true;
        return;
      }
      case 'detail': {
        await loadReview();
        return;
      }
      case 'toggle-exclude': {
        const block = packet[blockSel];
        if (block === undefined) {
          return;
        }
        if (!canExclude(block.name)) {
          session.notice(t('agent.block.locked'));
          return;
        }
        const next = new Set(excluded);
        if (next.has(block.name)) {
          next.delete(block.name);
        } else {
          next.add(block.name);
        }
        excluded = next;
        return;
      }
      case 'accept': {
        if (proposal === null) {
          return;
        }
        const got = await run('agent.proposal_apply', { id: proposal.id });
        if (got.ok) {
          session.notice(t('agent.accepted', [proposal.id]));
          await reload();
        }
        return;
      }
      case 'partial':
        // 「P」はキー名（§12.6: キー・ID は翻訳しない）。カタログに「一部採用」の文言が無い
        unavailable('P');
        return;
      case 'reject': {
        if (proposal === null) {
          return;
        }
        if (settled(proposal.status)) {
          unavailable(t('agent.keys.revert'));
          return;
        }
        rejecting = true;
        await tick();
        document.getElementById('agent-comment')?.focus();
        return;
      }
      case 'approve-messages':
        unavailable(t('agent.approve_thread'));
        return;
      case 'stop-all': {
        const got = await run('agent.stop_all', {});
        if (got.ok) {
          session.notice(String((got.result as { message?: unknown }).message ?? ''));
          await reload();
        }
        return;
      }
      case 'level-off':
      case 'level-propose':
      case 'level-apply':
      case 'level-autonomous': {
        const level = levelTone(action.replace('level-', ''));
        const row = delegation.levels[levelSel];
        if (row === undefined) {
          return;
        }
        await run('agent.delegation_set', { role: row.role, level });
        session.notice(t('agent.level_changed', [row.role, level]));
        await loadDelegation();
        return;
      }
      default:
        return;
    }
  }

  function moveSel(step: 1 | -1 | 'first' | 'last'): void {
    if (column === 'tasks') {
      current = moveInColumn(visible, current, step);
      void loadPacket();
      void loadReview();
    } else if (column === 'context') {
      blockSel = Math.min(
        Math.max(0, packet.length - 1),
        step === 'first' ? 0 : step === 'last' ? packet.length - 1 : blockSel + step,
      );
    } else if (column === 'delegation') {
      const rows = delegation.levels;
      levelSel = Math.min(
        Math.max(0, rows.length - 1),
        step === 'first' ? 0 : step === 'last' ? rows.length - 1 : levelSel + step,
      );
    } else if (column === 'review' && threads.length > 0) {
      const at = threads.findIndex((found) => found.id === threadId);
      const next = step === 'first' ? 0 : step === 'last' ? threads.length - 1 : Math.min(Math.max(0, threads.length - 1), (at < 0 ? 0 : at) + step);
      threadId = threads[next]?.id ?? '';
    }
  }

  function unavailable(name: string): void {
    session.notice(t('workspace.unavailable', [name]), 'info');
  }

  /** 画面のキー（§15.7 の表。アプリのショートカットはキーボード層）。 */
  function onkey(event: KeyboardEvent): void {
    if (root === null || !root.contains(event.target as Node | null)) {
      return;
    }
    const target = event.target as HTMLElement | null;
    if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) {
      return;
    }
    const key = chord(strokeOf(event));
    if (key === 'Escape' && (creating || confirming || rejecting || mcpOpen)) {
      creating = false;
      confirming = false;
      rejecting = false;
      mcpOpen = false;
      event.preventDefault();
      return;
    }
    const action = agentAction(key, column);
    if (action === null) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    void act(action);
  }

  /** 新しいタスク（`agent.task_create`）。 */
  async function createTask(): Promise<void> {
    const got = await run('agent.task_create', {
      role: newRole,
      scope: newScope,
      intent: newIntent,
      comment: newComment,
    });
    if (got.ok) {
      const created = (got.result as { task?: { id?: unknown } }).task;
      current = typeof created?.id === 'string' ? created.id : current;
      session.notice(t('agent.created', [String(created?.id ?? '')]));
    }
    creating = false;
    newIntent = '';
    newComment = '';
    await reload();
  }

  /** まとめて実行（§15.7.5(1)）。 */
  async function runBatch(): Promise<void> {
    for (const id of checked) {
      await run('agent.task_run', { id });
    }
    confirming = false;
    await reload();
  }

  /** 却下（`agent.comment_required`）。 */
  async function reject(): Promise<void> {
    if (!commentUsable(rejectComment)) {
      session.notice(t('agent.comment_required'));
      return;
    }
    if (proposal !== null) {
      const got = await run('agent.proposal_reject', { id: proposal.id, comment: rejectComment.trim() });
      if (got.ok) {
        session.notice(t('agent.rejected', [proposal.id]));
      }
    }
    rejectComment = '';
    rejecting = false;
    await reload();
  }

  /** MCP 設定（§15.7.7）。 */
  async function openMcp(): Promise<void> {
    const got = await session.run('agent.mcp_config_get');
    if (got.ok) {
      const text = (got.result as { text?: unknown }).text;
      mcpText = typeof text === 'string' ? text : '';
    }
    mcpOpen = true;
  }

  async function saveMcp(): Promise<void> {
    const checkedText = await session.run('agent.mcp_config_validate', { text: mcpText });
    if (checkedText.ok) {
      const saved = await session.run('agent.mcp_config_set', { text: mcpText });
      if (saved.ok) {
        session.notice(t('help.save_mcp'));
      } else {
        session.notice(saved.message);
      }
    } else {
      session.notice(checkedText.message);
    }
    mcpOpen = false;
    await loadServers();
  }

  function blockBudgetOf(block: PacketBlock, role: string): number {
    return blockBudget(role, block.order, cap);
  }
</script>

<svelte:window on:keydown={onkey} />

<section class="agent" aria-label={t('header.workspace.agent')} bind:this={root}>
  <div class="columns">
    <!-- ① タスク -->
    <div class="col" class:current={column === 'tasks'} aria-label={t(COLUMN_LABEL.tasks)}>
      <header>
        <h2>{t(COLUMN_LABEL.tasks)}</h2>
        <span class="count">{t('agent.progress', [String(progress.running), String(progress.waiting), String(progress.done)])}</span>
        <div class="actions">
          <button type="button" on:click={() => void act('new')}>{t('agent.new_button')}</button>
          <button type="button" disabled={current === null} on:click={() => void act('run')}>{t('agent.run_button')}</button>
          <button type="button" class="danger" disabled={current === null} on:click={() => void act('stop')}
            >{t('agent.cancel_button')}</button
          >
          <button type="button" disabled={current === null} on:click={() => void act('dry-run')}
            >{t('agent.dry_run_button')}</button
          >
          <button type="button" on:click={() => void act('run-batch')}>{t('agent.confirm_button')}</button>
        </div>
      </header>
      {#if visible.length === 0}
        <p class="empty">{t('agent.no_tasks')}</p>
      {:else}
        <div class="list" role="listbox" aria-label={t(COLUMN_LABEL.tasks)}>
          {#each visible as row (row.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
            <div
              role="option"
              aria-selected={current === row.id}
              class="row"
              class:selt={current === row.id}
              class:wait={statusTone(row.status) === 'wait'}
              class:run={statusTone(row.status) === 'run'}
              class:done={statusTone(row.status) === 'done'}
              tabindex={current === row.id ? 0 : -1}
              on:click={() => {
                current = row.id;
                void loadPacket();
                void loadReview();
              }}
            >
              <input
                type="checkbox"
                checked={checked.includes(row.id)}
                on:change={() => (checked = toggleSelect(checked, row.id))}
              />
              <span class="code">{row.id}</span>
              <span class="code">{row.role}</span>
              <span class="place">{row.scope}</span>
              <span class="sev">{row.status}</span>
              <span class="msg">{row.intent}</span>
              {#if startedByAgent(row)}
                <span class="hint">{t('agent.initiator_agent')}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
      <p class="hint">{t('agent.keys.tasks')}</p>
    </div>

    <!-- ② コンテキスト検査 -->
    <div class="col" class:current={column === 'context'} aria-label={t(COLUMN_LABEL.context)}>
      <header>
        <h2>{t(COLUMN_LABEL.context)}</h2>
      </header>
      {#if dryRun !== null}
        <p class="hint">{t('agent.dry_run_saved', [dryRun.path])} <span class="chars">{dryRun.chars}</span></p>
      {/if}
      {#if packet.length === 0}
        <p class="empty">{t('agent.no_packet')}</p>
      {:else}
        <div class="list" role="listbox" aria-label={t(COLUMN_LABEL.context)}>
          {#each packet as block, at (block.name)}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
            <div
              role="option"
              aria-selected={blockSel === at}
              class="row"
              class:selt={blockSel === at}
              class:locked={!canExclude(block.name)}
              class:dropped={excluded.has(block.name)}
              tabindex={blockSel === at ? 0 : -1}
              on:click={() => (blockSel = at)}
            >
              <span class="code">## {block.order} {block.name}</span>
              <span class="chars"
                >{t('agent.packet_size', [
                  String(block.chars),
                  String(blockBudgetOf(block, task()?.role ?? 'writer')),
                ])}</span
              >
              {#if excluded.has(block.name)}
                <span class="hint">{t('agent.block.excluded')}</span>
              {:else if !canExclude(block.name)}
                <span class="hint">{t('agent.block.required')}</span>
              {:else if block.omitted > 0}
                <span class="hint">{t('agent.block.trimmed')}</span>
              {/if}
              <span class="bar" style:width={`${Math.round(budgetRatio(block.chars, blockBudgetOf(block, task()?.role ?? 'writer')) * 100)}%`}></span>
              {#if blockSel === at}
                <pre class="body">{block.body}</pre>
              {/if}
            </div>
          {/each}
        </div>
        <p class="hint">{t('agent.packet_size', [String(packetChars), String(cap)])}</p>
        {#if packetChars > cap}
          <p class="hint">{t('agent.packet_over')}</p>
        {/if}
      {/if}
      <p class="hint">{t('agent.keys.context')}</p>
    </div>

    <!-- ③ 結果レビュー -->
    <div class="col" class:current={column === 'review'} aria-label={t(COLUMN_LABEL.review)}>
      <header>
        <h2>{t(COLUMN_LABEL.review)}</h2>
        <div class="actions">
          <button type="button" disabled={proposal === null} on:click={() => void act('accept')}>{t('agent.accept_yes')}</button>
          <button type="button" class="unavailable" on:click={() => void act('partial')}>P</button>
          <button type="button" class="danger" disabled={proposal === null} on:click={() => void act('reject')}
            >{t('agent.reject_title')}</button
          >
          <button type="button" class="unavailable" on:click={() => void act('approve-messages')}
            >{t('agent.approve_thread')}</button
          >
        </div>
      </header>
      {#if proposal === null}
        <p class="empty">{t('agent.review.none')}</p>
        {#if resultText !== ''}
          <pre class="body">{resultText}</pre>
        {/if}
      {:else}
        {#if proposal.drifted}
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
        <p class="hint">{t('agent.keys.review')}</p>
      {/if}
      <div class="list threads" role="listbox" aria-label={t('agent.threads_heading')}>
        {#each threads as row (row.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
          <div
            role="option"
            aria-selected={threadId === row.id}
            class="row"
            class:selt={threadId === row.id}
            tabindex={threadId === row.id ? 0 : -1}
            on:click={() => (threadId = row.id)}
          >
            <span class="msg">{row.title}</span>
            <span class="sev">{row.status}</span>
            {#if row.messages.filter((message) => message.kind === 'deliver').length > 0}
              <span class="hint"
                >{t('agent.review.pending', [
                  String(row.messages.filter((message) => message.kind === 'deliver').length),
                ])}</span
              >
            {/if}
          </div>
        {/each}
      </div>
      {#if thread !== null}
        <div class="thread">
          {#each thread.messages as message, at (at)}
            <article>
              <span class="from">{message.from} → {message.to}</span>
              <pre>{message.body}</pre>
            </article>
          {/each}
        </div>
      {/if}
    </div>

    <!-- ④ 委譲レベル -->
    <div class="col" class:current={column === 'delegation'} aria-label={t(COLUMN_LABEL.delegation)}>
      <header>
        <h2>{t(COLUMN_LABEL.delegation)}</h2>
        <button type="button" class="danger" on:click={() => void act('stop-all')}>{t('agent.stop_all_button')}</button>
      </header>
      <div class="list" role="listbox" aria-label={t(COLUMN_LABEL.delegation)}>
        {#each delegation.levels as row, at (row.role)}
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
          <div
            role="option"
            aria-selected={levelSel === at}
            class="row"
            class:selt={levelSel === at}
            tabindex={levelSel === at ? 0 : -1}
            on:click={() => (levelSel = at)}
          >
            <span class="code">{row.role}</span>
            <span class="sev">{row.level}</span>
          </div>
        {/each}
      </div>
      <p class="hint">{t('agent.servers')}</p>
      {#if servers.length === 0}
        <p class="hint">{t('agent.no_servers')}</p>
      {:else}
        {#each servers as server (server.name)}
          <p class="hint">{server.name}{server.missing.length > 0 ? ` (${server.missing.join(', ')})` : ''}</p>
        {/each}
      {/if}
      <div class="actions">
        <button type="button" on:click={() => void openMcp()}>{t('help.edit_mcp')}</button>
      </div>
    </div>
  </div>

  {#if creating}
    <div class="modal" role="dialog" aria-label={t('agent.new_title')}>
      <p>{t('agent.new_hint')}</p>
      <label class="inline">
        {t('agent.new_role')}
        <input id="agent-new-role" bind:value={newRole} on:keydown={(event) => event.stopPropagation()} />
      </label>
      <label class="inline">
        {t('agent.new_scope')}
        <input bind:value={newScope} on:keydown={(event) => event.stopPropagation()} />
      </label>
      <label class="inline">
        {t('agent.new_intent')}
        <input bind:value={newIntent} on:keydown={(event) => event.stopPropagation()} />
      </label>
      <label class="inline">
        {t('agent.new_comment')}
        <input bind:value={newComment} on:keydown={(event) => event.stopPropagation()} />
      </label>
      <div class="actions">
        <button type="button" on:click={() => void createTask()}>{t('agent.new_button')}</button>
        <button type="button" on:click={() => (creating = false)}>{t('modal.cancel')}</button>
      </div>
    </div>
  {/if}

  {#if confirming && confirmView !== null}
    <div class="modal" role="dialog" aria-label={t('agent.confirm_title', [String(confirmView.count)])}>
      <p>{t('agent.confirm_title', [String(confirmView.count)])}</p>
      {#if confirmView.chars !== null}
        <p>{t('agent.confirm_chars', [String(confirmView.chars)])}</p>
      {:else}
        <p>{t('agent.no_packet')}</p>
      {/if}
      {#each servers as server (server.name)}
        <p>{t('agent.confirm_servers', [server.name])}</p>
      {/each}
      <div class="actions">
        <button type="button" on:click={() => void runBatch()}>{t('agent.confirm_yes')}</button>
        <button type="button" on:click={() => (confirming = false)}>{t('modal.cancel')}</button>
      </div>
    </div>
  {/if}

  {#if rejecting}
    <div class="modal" role="dialog" aria-label={t('agent.reject_title')}>
      <p>{t('agent.reject_title')}</p>
      <textarea id="agent-comment" bind:value={rejectComment} on:keydown={(event) => event.stopPropagation()}></textarea>
      <p class="hint">{t('agent.comment_required')}</p>
      <div class="actions">
        <button type="button" class="danger" on:click={() => void reject()}>{t('stage.confirm')}</button>
        <button type="button" on:click={() => (rejecting = false)}>{t('modal.cancel')}</button>
      </div>
    </div>
  {/if}

  {#if mcpOpen}
    <div class="modal" role="dialog" aria-label={t('help.edit_mcp')}>
      <textarea bind:value={mcpText} on:keydown={(event) => event.stopPropagation()}></textarea>
      <div class="actions">
        <button type="button" on:click={() => void saveMcp()}>{t('help.save_mcp')}</button>
        <button type="button" on:click={() => (mcpOpen = false)}>{t('modal.cancel')}</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .agent {
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
  .columns {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    flex: 1;
    min-height: 0;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 8px;
    min-height: 0;
  }
  .col.current {
    border-color: var(--accent);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }
  h2 {
    font-size: 13px;
    font-weight: 700;
    margin: 0;
  }
  .actions {
    display: flex;
    gap: 4px;
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
  }
  .row {
    display: flex;
    gap: 6px;
    padding: 2px 6px;
    cursor: pointer;
    border-radius: 2px;
    align-items: baseline;
    flex-wrap: wrap;
    line-height: 1.6;
  }
  .row[aria-selected='true'],
  .row.selt {
    background: var(--bg-selection);
  }
  .row.locked {
    cursor: default;
    color: var(--fg-muted);
  }
  .row.dropped {
    text-decoration: line-through;
    text-decoration-color: var(--fg-muted);
    color: var(--fg-muted);
  }
  .row .bar {
    flex-basis: 100%;
    background: var(--accent);
    height: 2px;
    border-radius: 2px;
  }
  .run {
    color: var(--accent);
  }
  .wait {
    color: var(--state-warn);
  }
  .done {
    color: var(--state-ok);
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
  .chars,
  .place {
    font-family: var(--mono);
    font-size: 12px;
  }
  .sev {
    color: var(--state-warn);
  }
  .msg {
    flex: 1;
  }
  .body,
  .thread pre {
    white-space: pre-wrap;
    margin: 0;
    font-family: var(--mono);
    font-size: 12px;
    line-height: 1.6;
  }
  .thread {
    overflow: auto;
    max-height: 30%;
  }
  .threads {
    max-height: 30%;
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
</style>
