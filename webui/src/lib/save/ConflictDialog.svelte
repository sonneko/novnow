<script lang="ts">
  // 外部変更との衝突の面（§8.5・§9.6・§9.7.5 の差分モーダル）。
  //
  // 出す流れ:
  //
  // 1. `save.state` の `conflicts`（パスの一覧）から、1 件を選ぶ。
  // 2. `save.diff_external` で行ごとの差分を取る（読むためだけ。§8.5）。
  // 3. 利用者がどちらを残すかを選ぶ:
  //    - `save.keep_mine` … 自分の版を残す（外部の版を上書き）
  //    - `save.reload_external` … 外部の版を読み込む（自分の変更を破棄）
  //    画面が勝手に選ばない（MUST。§8.5）。
  // 4. 解決したら一覧を取り直す。
  //
  // **ブラウザではこの面は出ない**: 外部変更の監視はネイティブだけが行う（§13.3。
  // `crates/novnow-wasm` の Host は監視しない）。押せないボタンを並べず、理由を書く
  // （`workspace.unavailable`。§21.1.2）。この判断は `conflictSurface` が持つ。
  //
  // 文言は生成物 `i18n/catalog.ts` に実在するキーだけを使う（§12.3）。

  import type { Texts } from '../i18n';
  import type { Runtime } from '../project/picker';
  import {
    conflictDiffs,
    conflictSurface,
    resolveArgs,
    type ConflictDiff,
  } from '../sections/sectionOps';

  /** 文言。 */
  export let t: Texts['t'];
  /** 動いている環境（`native` = Tauri、`browser` = wasm）。 */
  export let runtime: Runtime = 'browser';
  /** 衝突しているパス（`save.state` の `conflicts`）。 */
  export let conflicts: string[] = [];
  /** 閉じる。 */
  export let onclose: (() => void) | null = null;
  /** 差分を取る（`save.diff_external`）。 */
  export let ondiffs: ((path: string) => Promise<ConflictDiff[]>) | null = null;
  /** 自分の版を残す（`save.keep_mine`）。 */
  export let onkeep: ((path: string) => Promise<string>) | null = null;
  /** 外部の版を読み込む（`save.reload_external`）。 */
  export let onreload: ((path: string) => Promise<string>) | null = null;

  /** いま見ているパス（`null` なら未選択）。 */
  let selected: string | null = null;
  /** その差分。 */
  let diffs: ConflictDiff[] = [];
  /** 差分を取り直し中。 */
  let loading = false;
  /** 実行中。 */
  let busy = false;
  /** 失敗の文言。 */
  let failure = '';

  $: surface = conflictSurface({ native: runtime === 'native', conflicts: conflicts.length });
  // 一覧が変わったら選択を合わせる（消えた 1 件を選んだままにしない）。
  $: if (conflicts.length > 0 && (selected === null || !conflicts.includes(selected))) {
    select(conflicts[0] as string);
  }

  /** 背景（外側）を押したら閉じる（§9.7.5）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      onclose?.();
    }
  }

  /** 1 件選んで差分を取る。 */
  async function select(path: string): Promise<void> {
    selected = path;
    failure = '';
    diffs = [];
    if (!ondiffs) {
      return;
    }
    loading = true;
    try {
      diffs = await ondiffs(path);
    } finally {
      loading = false;
    }
  }

  /** どちらかを選んで実行する（`keep` = 自分の版を残す）。 */
  async function resolve(path: string, keep: boolean): Promise<void> {
    if (busy) {
      return;
    }
    busy = true;
    failure = '';
    try {
      const message = (await (keep ? onkeep?.(path) : onreload?.(path))) ?? '';
      if (message) {
        failure = message;
        return;
      }
      // 解決した 1 件を一覧から外す（次の 1 件を選び直す。$: が合わせる）。
      const left = conflicts.filter((found) => found !== path);
      if (left.length === 0) {
        onclose?.();
      }
    } finally {
      busy = false;
    }
  }

  /** `Esc` で閉じる（重ね物の共通規則）。 */
  function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      onclose?.();
    }
  }
</script>

<svelte:window on:keydown={keydown} />

<div class="overlay" role="presentation" on:click={outside}>
  <div
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-label={t('diff.title')}
    tabindex="-1"
  >
    <h2>{t('diff.title')}</h2>

    {#if surface === 'unsupported'}
      <!-- ブラウザは外部変更を監視しない（§13.3）。押せないボタンを置かず理由を書く。 -->
      <p class="reason" role="status">{t('workspace.unavailable', [t('api.op.save.keep_mine')])}</p>
    {:else if conflicts.length === 0}
      <p class="reason" role="status">{t('api.error.no_conflict')}</p>
    {:else}
      <!-- 衝突しているファイル（1 件ずつ選ぶ） -->
      <ul class="paths">
        {#each conflicts as path (path)}
          <li>
            <button
              type="button"
              class="path"
              class:selected={path === selected}
              aria-current={path === selected ? 'true' : undefined}
              on:click={() => void select(path)}
            >
              {path}
            </button>
          </li>
        {/each}
      </ul>

      {#if selected}
        {#if loading}
          <p class="reason">{t('status.saving')}</p>
        {:else}
          {#each diffs as diff (diff.path)}
            <p class="counts">{t('diff.lines', [String(diff.mine_chars), String(diff.external_chars)])}</p>
            <div class="diff" role="table" aria-label={t('diff.title')}>
              {#each diff.lines as line (`${line.side}-${line.line}`)}
                <div class="line" class:mine={line.side === 'mine'} class:external={line.side === 'external'} role="row">
                  <span class="side" aria-hidden="true">{line.side === 'mine' ? '−' : '+'}</span>
                  <span class="number">{line.line}</span>
                  <span class="text">{line.text}</span>
                </div>
              {/each}
            </div>
          {/each}

          {#if failure}
            <p class="failure" role="alert">{failure}</p>
          {/if}

          <!-- どちらを残すかは利用者が選ぶ（画面が勝手に決めない。§8.5 MUST）。 -->
          <div class="actions">
            <button type="button" class="button" disabled={busy} on:click={() => void resolve(selected as string, true)}>
              {t('diff.keep_mine')}
            </button>
            <button type="button" class="button" disabled={busy} on:click={() => void resolve(selected as string, false)}>
              {t('diff.take_theirs')}
            </button>
          </div>
        {/if}
      {/if}
    {/if}

    <p class="close-hint">{t('modal.close_hint')}</p>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 41;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 64px;
    background: rgb(0 0 0 / 35%);
  }
  .sheet {
    width: min(720px, 92vw);
    max-height: 74vh;
    overflow-y: auto;
    padding: 14px 16px 12px;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--fg-primary);
    font-size: 12px;
  }
  h2 {
    margin: 0 0 10px;
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
  }
  .paths {
    margin: 0 0 10px;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .path {
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: 2px;
    background: transparent;
    color: var(--fg-muted);
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 11px;
    cursor: pointer;
  }
  .path:hover {
    background: var(--bg-selection);
  }
  .path.selected {
    color: var(--accent);
    border-color: var(--accent);
  }
  .counts,
  .reason {
    margin: 0 0 8px;
    color: var(--fg-faint);
    font-size: 11px;
  }
  .diff {
    border: 1px solid var(--border);
    border-radius: 2px;
    background: var(--bg-inset);
    padding: 4px 0;
    max-height: 40vh;
    overflow: auto;
  }
  .line {
    display: flex;
    gap: 8px;
    padding: 0 8px;
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 11px;
    line-height: 1.7;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .line.mine {
    color: var(--state-unsaved);
  }
  .line.external {
    color: var(--accent);
  }
  .side,
  .number {
    flex: 0 0 auto;
    color: var(--fg-faint);
  }
  .text {
    flex: 1 1 auto;
  }
  .failure {
    margin: 8px 0 0;
    color: var(--state-error);
    font-size: 11px;
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
  .close-hint {
    margin: 10px 0 0;
    color: var(--fg-faint);
    font-size: 11px;
  }
</style>
