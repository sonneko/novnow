<script lang="ts">
  // ヘルプ（`?`、§9.8）とコマンドパレット（`Ctrl+P`、§9.7）の重ね物。
  //
  // どちらも**同じ重ね物**として作る（§9.7.5: 重ね物は 1 つずつ。`Esc` と外側クリックで閉じる）。
  // 中身の文は `src/ui/i18n/*.toml` の `help.*` / `palette.*` を引く（§12.3）。
  //
  // パレットは W4 の作業なので、この版では**候補を出さずに理由を書く**（§21.1.2 MUST:
  // 未提供を黙って隠さない）。`Ctrl+P` が何も起きないより、開いて「まだ使えない」と
  // 書くほうが正直である。

  import { HELP_GROUPS } from '../keyboard/help';
  import type { Texts } from '../i18n';

  /** どちらを出すか（`null` なら閉じている）。 */
  export let mode: 'help' | 'palette' | null = null;
  /** 文言。 */
  export let t: Texts['t'];
  /** 閉じる。 */
  export let onclose: (() => void) | null = null;
  /** キーを 1 つ実行する（`?` の行を押したとき。§9.8.7）。 */
  export let onrun: ((command: string) => void) | null = null;

  /** 背景（外側）を押したら閉じる（§9.7.5）。 */
  function outside(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      onclose?.();
    }
  }
</script>

{#if mode}
  <!-- 最上位のレイヤ（§9.7 の z-index）。背面は押された判定にしない（`pointer-events` で止める）。 -->
  <div class="overlay" role="presentation" on:click={outside}>
    <!-- 重ね物の本体は `role="dialog"` を持つので `div` にする（`section` には付けられない）。 -->
    <div
      class="sheet"
      class:palette={mode === 'palette'}
      role="dialog"
      aria-modal="true"
      aria-label={mode === 'help' ? t('help.title') : t('help.palette')}
      tabindex="-1"
    >
      {#if mode === 'help'}
        <h2>{t('help.title')}</h2>
        <div class="groups">
          {#each HELP_GROUPS as group}
            <div class="group">
              <h3>{t(group.title)}</h3>
              {#each group.rows as row}
                <button
                  type="button"
                  class="row"
                  on:click={() => onrun?.(row.keys)}
                  title={t('help.palette')}
                >
                  <kbd>{row.keys}</kbd>
                  <span>{t(row.label)}</span>
                </button>
              {/each}
            </div>
          {/each}
        </div>
      {:else}
        <h2>{t('help.palette')}</h2>
        <p class="hint">{t('palette.placeholder')}</p>
        <!-- 未提供を明示する（§21.1.2）。空白の一覧を出さない（§9.7 の 0 件表示の規則）。 -->
        <p class="empty" role="status">{t('palette.no_matches')}</p>
      {/if}
      <p class="close-hint">{t('modal.close_hint')}</p>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    background: color-mix(in srgb, var(--bg-base) 66%, transparent);
  }

  .sheet {
    width: min(760px, calc(100vw - 64px));
    max-height: 72vh;
    overflow: auto;
    padding: 16px 20px 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-raised);
    color: var(--fg-primary);
    box-shadow: 0 16px 48px rgb(0 0 0 / 32%);
  }

  /* パレットは上端（VS Code と同じ位置）に出す（§9.7 の表示位置）。 */
  .sheet.palette {
    width: min(560px, calc(100vw - 64px));
  }

  h2 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--fg-muted);
  }

  h3 {
    margin: 12px 0 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-faint);
  }

  .groups {
    columns: 2;
    column-gap: 28px;
  }

  @media (max-width: 700px) {
    .groups {
      columns: 1;
    }
  }

  .group {
    break-inside: avoid;
  }

  .row {
    display: flex;
    gap: 10px;
    align-items: baseline;
    width: 100%;
    padding: 3px 4px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .row:hover,
  .row:focus-visible {
    background: var(--bg-selection);
  }

  kbd {
    flex: 0 0 190px;
    color: var(--accent);
    font-family: inherit;
    font-size: 12px;
  }

  .row span {
    font-size: 13px;
  }

  .hint {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--fg-faint);
  }

  .empty {
    margin: 16px 0;
    font-size: 13px;
    color: var(--fg-muted);
  }

  .close-hint {
    margin: 12px 0 0;
    padding-top: 8px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    color: var(--fg-faint);
  }
</style>
