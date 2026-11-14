<script lang="ts">
  // ステータスバー（§9.6）。高さ 22px。
  //
  // 並びは左から「パンくず · 文字数（節 / 話 / 全体）· 原稿用紙 · 診断 · 保存」。
  // 保存インジケータは右端に**幅を予約して**常に残す（§9.6.4。`REF:src/ui/status.rs` と同じ:
  // 失敗 170px / 閉じるのを止める 46px / それ以外 23px）。足りないときは
  // セッション情報 → 原稿用紙 → 文字数の順に落とす。
  import type { SaveState } from '../bridge/types';
  import type { Texts } from '../i18n';
  import { grouped } from '../editor/text';

  /** 文言。 */
  export let t: Texts['t'];
  /** パンくず（`01›04›02›03`）。 */
  export let breadcrumb = '';
  /** 節の文字数（空白除外）。 */
  export let sectionChars = 0;
  /** 話の文字数。 */
  export let documentChars = 0;
  /** プロジェクト全体の文字数。 */
  export let projectChars = 0;
  /** 原稿用紙換算の枚数（`project.toml` の `manuscript_page_chars` 基準。無ければ 0）。 */
  export let pages = 0;
  /** 診断の件数（0 なら出さない）。 */
  export let diagnostics = 0;
  /** 保存状態（§9.6 の 6 状態）。 */
  export let save: SaveState = 'idle';
  /** 保存の失敗の理由。 */
  export let saveMessage = '';
  /** 保存インジケータを押した。 */
  export let onsave: (() => void) | null = null;
  /** 診断を押した（点検へ）。 */
  export let onreview: (() => void) | null = null;

  /** 状態 → 右端の予約幅（`REF:src/ui/status.rs` と同じ値）。 */
  $: reserved = save === 'error' ? 170 : save === 'saving' ? 46 : 23;

  /** 状態 → 出る文字（`Indicator` の 4 種と同じ）。 */
  $: mark = save === 'dirty' ? '●' : save === 'saving' ? '◐' : save === 'error' || save === 'conflict' ? '⚠' : '';

  /** ツールチップ。 */
  $: detail =
    save === 'error' || save === 'conflict'
      ? `${t('status.save_failed')}${saveMessage ? `: ${saveMessage}` : ''}`
      : save === 'saving'
        ? t('status.saving')
        : save === 'dirty'
          ? t('status.unsaved')
          : '';
</script>

<footer class="status" style={`--reserved:${reserved}px`}>
  <div class="left">
    {#if breadcrumb}
      <span class="crumb">{breadcrumb}</span>
    {/if}
    <span class="counts">
      <span class="section">{grouped(sectionChars)}{t('status.chars_unit')}</span>
      <span class="sep">/</span>
      <span class="document">{grouped(documentChars)}{t('status.chars_unit')}</span>
      <span class="sep">/</span>
      <span class="project">{grouped(projectChars)}{t('status.chars_unit')}</span>
    </span>
    {#if pages > 0}
      <span class="pages">{pages.toFixed(1)}{t('status.pages_unit')}</span>
    {/if}
    {#if diagnostics > 0}
      <button type="button" class="flag" on:click={() => onreview?.()} title={t('status.open_review')}>
        ⚑{diagnostics}
      </button>
    {/if}
  </div>
  <button type="button" class="save" data-state={save} title={detail} aria-label={detail || t('status.saving')} on:click={() => onsave?.()}>
    {#if mark}
      <span aria-hidden="true">{mark}</span>
    {/if}
    {#if save === 'saving'}
      <span class="spinner" aria-hidden="true"></span>
    {/if}
    {#if save === 'error' || save === 'conflict'}
      <span class="message">{t('status.save_failed')}</span>
    {/if}
  </button>
</footer>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 22px;
    padding: 0 8px;
    background: var(--bg-raised);
    border-top: 1px solid var(--border);
    font-size: 11px;
    color: var(--fg-muted);
    overflow: hidden;
  }
  .left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex: 1 1 auto;
    white-space: nowrap;
    overflow: hidden;
  }
  .crumb {
    font-family: var(--mono, ui-monospace, monospace);
    color: var(--fg-faint);
  }
  .counts {
    display: inline-flex;
    gap: 3px;
  }
  .sep {
    color: var(--fg-faint);
  }
  .pages {
    color: var(--fg-faint);
  }
  .flag {
    border: 0;
    background: transparent;
    color: var(--state-warn);
    font: inherit;
    cursor: pointer;
    padding: 0 2px;
  }
  .flag:hover {
    text-decoration: underline;
  }
  .save {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: 0 0 auto;
    width: var(--reserved);
    justify-content: flex-end;
    border: 0;
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    cursor: pointer;
    overflow: hidden;
  }
  .save[data-state='dirty'] {
    color: var(--state-unsaved);
  }
  .save[data-state='error'],
  .save[data-state='conflict'] {
    color: var(--state-error);
  }
  .save[data-state='saved'] {
    color: var(--state-ok);
  }
  .message {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 10px;
  }
  .spinner {
    width: 8px;
    height: 8px;
    border: 1px solid var(--fg-muted);
    border-top-color: transparent;
    border-radius: 50%;
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (max-width: 760px) {
    .pages {
      display: none;
    }
  }
  @media (max-width: 620px) {
    .counts .project,
    .counts .sep:last-of-type {
      display: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }
</style>
