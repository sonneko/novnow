<script lang="ts">
  // シェル（§9.1）。ヘッダ・本文・ステータスバーの 3 段だけを持つ。
  //
  // 本文の中身はワークスペースが決める（W2 は `write` だけ。他は「この版では使えません」
  // を出す。§21.1.2 MUST: 黙って別の画面に置き換えない）。
  import Header from './Header.svelte';
  import StatusBar from './StatusBar.svelte';
  import WriteWorkspace from '../workspaces/write/WriteWorkspace.svelte';
  import type { Screen } from '../store/session';
  import type { Texts } from '../i18n';

  /** 文言。 */
  export let t: Texts['t'];
  /** 画面の状態。 */
  export let screen: Screen;
  /** 前後の節。 */
  export let before: { scene: string; text: string }[] = [];
  export let after: { scene: string; text: string }[] = [];
  /** 台詞比率・有効モード（§9.4.2）。 */
  export let ratio = 0;
  export let modes = '';
  /** 変換中。 */
  export let composing = false;
  /** 受け渡し。 */
  export let oninput: ((text: string) => void) | null = null;
  export let oncaret: ((position: number) => void) | null = null;
  export let onkey: ((event: KeyboardEvent) => void) | null = null;
  export let onnav: ((scene: string) => void) | null = null;
  export let ondelete: ((scene: string) => void) | null = null;
  export let onjump: ((kind: string, value: unknown) => void) | null = null;
  export let onratio: ((key: string, value: number) => void) | null = null;
  export let onworkspace: ((name: string) => void) | null = null;
  export let onlanguage: (() => void) | null = null;
  export let ontheme: (() => void) | null = null;
  export let onsave: (() => void) | null = null;
  export let onreview: (() => void) | null = null;

  /** テーマ（`ui.json` の `theme`。W2 では OS の設定に従う）。 */
  let theme: 'dark' | 'light' = 'dark';

  /** OS の設定を見る（§11.4: 分からなければ dark）。 */
  $: if (typeof matchMedia === 'function') {
    const query = matchMedia('(prefers-color-scheme: light)');
    theme = query.matches ? 'light' : 'dark';
  }

  /** `theme` を `<html>` の `data-theme` に反映する。 */
  $: if (typeof document !== 'undefined') {
    document.documentElement.dataset.theme = theme;
  }

  /** キーを受ける（画面で 1 か所。`REF:docs/designs/webui.md` §21.2）。 */
  function keydown(event: KeyboardEvent): void {
    if (composing) {
      return;
    }
    onkey?.(event);
  }
</script>

<svelte:window on:keydown={keydown} />

<div class="shell">
  <Header
    {t}
    title={screen.title}
    breadcrumb={screen.breadcrumb}
    workspace={screen.workspace}
    workspaces={screen.workspaces}
    locale={screen.locale}
    {theme}
    onworkspace={onworkspace}
    onlanguage={onlanguage}
    ontheme={ontheme}
    onproject={() => onworkspace?.('write')}
    oncrumb={() => onworkspace?.('write')}
  />
  <main class="body">
    {#if !screen.ready}
      <p class="loading">{t('status.saving')}</p>
    {:else if screen.failure}
      <p class="failure" role="alert">{screen.failure}</p>
    {:else if screen.workspace === 'write'}
      <WriteWorkspace
        {t}
        {screen}
        {composing}
        reference={screen.referenceVisible}
        {before}
        {after}
        {ratio}
        {modes}
        oninput={oninput}
        oncaret={oncaret}
        onkey={(event) => onkey?.(event)}
        onnav={onnav}
        ondelete={ondelete}
        onjump={onjump}
        onratio={onratio}
      />
    {:else}
      <!-- 未提供の画面は「この版では使えません」を出す（§21.1.2）。空の画面を置かない。 -->
      <p class="unavailable">{t(`header.workspace.${screen.workspace}`)}{t('workspace.unavailable', [''])}</p>
    {/if}
  </main>
  <StatusBar
    {t}
    breadcrumb={screen.breadcrumb}
    sectionChars={screen.sectionChars}
    documentChars={screen.documentChars}
    projectChars={screen.projectChars}
    pages={screen.projectChars / 400}
    diagnostics={screen.diagnostics}
    save={screen.save}
    saveMessage={screen.saveMessage}
    onsave={onsave}
    onreview={onreview}
  />
</div>

<style>
  .shell {
    display: grid;
    grid-template-rows: auto 1fr auto;
    height: 100vh;
    overflow: hidden;
    background: var(--bg-base);
  }
  .body {
    min-height: 0;
    overflow: hidden;
  }
  .loading,
  .failure,
  .unavailable {
    margin: 24px;
    color: var(--fg-muted);
    font-size: 13px;
  }
  .failure {
    color: var(--state-error);
  }
</style>
