<script lang="ts">
  // ヘッダ（§9.1・§9.4.2）。高さ 28px、ラベル無し。
  //
  // 並びは左から「画面切替アイコン列 → プロジェクト名 → パンくず → 右端の常設ボタン」
  // （§9.1.5）。幅が足りないときは画面名 → プロジェクト名 → パンくずの順に落とす（CSS で行う）。
  import type { Texts } from '../i18n';

  /** 文言。 */
  export let t: Texts['t'];
  /** プロジェクトの題名。 */
  export let title = '';
  /** パンくず（`01›04›02›03`）。 */
  export let breadcrumb = '';
  /** いまのワークスペース。 */
  export let workspace = 'write';
  /** 使えるワークスペース（§10 の 9 つ）。 */
  export let workspaces: string[] = [];
  /** 言語。 */
  export let locale = 'ja';
  /** テーマ。 */
  export let theme: 'dark' | 'light' = 'dark';
  /** ワークスペースを選んだ（`Alt+数字` と同じ規則）。 */
  export let onworkspace: ((name: string) => void) | null = null;
  /** 右端の常設ボタン（§9.1.4）。 */
  export let onpalette: (() => void) | null = null;
  export let onhelp: (() => void) | null = null;
  export let ontheme: (() => void) | null = null;
  export let onlanguage: (() => void) | null = null;
  /** プロジェクト名を押した（`Ctrl+O` と同じ規則）。 */
  export let onproject: (() => void) | null = null;
  /** パンくずを押した（`@` だけでパレットを開く）。 */
  export let oncrumb: (() => void) | null = null;

  /** ワークスペースのアイコン（文字ではなく図形で示す。§9.2）。 */
  const ICONS: Record<string, string> = {
    write: '¶',
    'plot-episode': '▤',
    'plot-outline': '≡',
    idea: '✧',
    material: '❐',
    cast: '☺',
    reader: '☰',
    agent: '⚙',
    review: '⚑',
  };

  /** 押せる要素の共通の見た目（下線・ホバー背景・ポインタ。§9.1.4）。 */
</script>

<header class="header">
  <!-- ① ワークスペースの切替（§10 の 9 つ。W2 は write だけ中身がある） -->
  <nav class="workspaces" aria-label={t('header.workspace.write')}>
    {#each workspaces as name (name)}
      <button
        type="button"
        class="icon"
        class:active={name === workspace}
        aria-current={name === workspace ? 'page' : undefined}
        aria-label={t(`header.workspace.${name}`)}
        title={`${t(`header.workspace.${name}`)} (${workspaces.indexOf(name) + 1})`}
        on:click={() => onworkspace?.(name)}
      >
        <span aria-hidden="true">{ICONS[name] ?? '·'}</span>
      </button>
    {/each}
  </nav>

  <button type="button" class="name" title={title} on:click={() => onproject?.()}>
    {title}
  </button>

  {#if breadcrumb}
    <button type="button" class="crumb" on:click={() => oncrumb?.()}>{breadcrumb}</button>
  {/if}

  <div class="right">
    <button
      type="button"
      class="icon"
      aria-label={t('header.language')}
      title={t('header.language')}
      on:click={() => onlanguage?.()}
    >
      <span aria-hidden="true">{t('header.language_symbol')}</span>
    </button>
    <button
      type="button"
      class="icon"
      aria-label={t('header.theme')}
      title={t('header.theme')}
      on:click={() => ontheme?.()}
    >
      <span aria-hidden="true">{theme === 'dark' ? '◐' : '◑'}</span>
    </button>
    <button type="button" class="icon" aria-label="Ctrl+P" title="Ctrl+P" on:click={() => onpalette?.()}>
      <span aria-hidden="true">⌘</span>
    </button>
    <button type="button" class="icon" aria-label="?" title="?" on:click={() => onhelp?.()}>
      <span aria-hidden="true">?</span>
    </button>
  </div>
  <span class="visually-hidden" aria-live="polite">{locale}</span>
</header>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 6px;
    background: var(--bg-raised);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    color: var(--fg-muted);
    /* 幅が足りないときに落とす順序（§9.1.5）: 表示名 → プロジェクト名 → パンくず */
    overflow: hidden;
  }
  .workspaces,
  .right {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    flex: 0 0 auto;
  }
  .right {
    margin-left: auto;
  }
  .icon {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 12px;
    cursor: pointer;
    /* 当たり判定は 24×24px 以上（§9.9.2 の 3） */
  }
  .icon::before {
    content: '';
    position: absolute;
    inset: -2px;
  }
  .icon:hover {
    background: var(--bg-selection);
    color: var(--fg-primary);
  }
  .icon.active {
    color: var(--accent);
    background: var(--bg-selection);
  }
  .name,
  .crumb {
    border: 0;
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    padding: 2px 4px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-decoration: underline;
    text-decoration-color: var(--border);
    text-underline-offset: 2px;
  }
  .name {
    max-width: 24ch;
  }
  .crumb {
    font-family: var(--mono, ui-monospace, monospace);
    color: var(--fg-faint);
  }
  .name:hover,
  .crumb:hover {
    background: var(--bg-selection);
    color: var(--fg-primary);
  }
  @media (max-width: 640px) {
    .crumb {
      display: none;
    }
  }
  @media (max-width: 480px) {
    .name {
      display: none;
    }
  }
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    clip-path: inset(50%);
    overflow: hidden;
  }
</style>
