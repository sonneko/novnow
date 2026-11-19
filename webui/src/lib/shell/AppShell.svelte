<script lang="ts">
  // シェル（§9.1）。ヘッダ・本文・ステータスバーの 3 段だけを持つ。
  //
  // 本文の中身はワークスペースが決める（§10 の 9 つ）。中身の無いワークスペースだけ
  // 「この版では使えません」を出す（§21.1.2 MUST: 黙って別の画面に置き換えない）。
  import Header from './Header.svelte';
  import StatusBar from './StatusBar.svelte';
  import WriteWorkspace from '../workspaces/write/WriteWorkspace.svelte';
  import MaterialWorkspace from '../workspaces/material/MaterialWorkspace.svelte';
  import CastWorkspace from '../workspaces/cast/CastWorkspace.svelte';
  import IdeaWorkspace from '../workspaces/idea/IdeaWorkspace.svelte';
  import PlotEpisodeWorkspace from '../workspaces/plot-episode/PlotEpisodeWorkspace.svelte';
  import PlotOutlineWorkspace from '../workspaces/plot-outline/PlotOutlineWorkspace.svelte';
  import ReaderWorkspace from '../workspaces/reader/ReaderWorkspace.svelte';
  import AgentWorkspace from '../workspaces/agent/AgentWorkspace.svelte';
  import ReviewWorkspace from '../workspaces/review/ReviewWorkspace.svelte';
  import type { Outcome, Screen, Session } from '../store/session';
  import type { OpArgs, OpId } from '../bridge/types';
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
  /**
   * 参照ペインの行をダブルクリックした（`cast.get` / `dictionary.term_full`。§9.5.5）。
   *
   * 単クリックの `onjump`（移動）とは意味が違う。ダブルクリックは「開く」。
   */
  export let onopen: ((kind: string, value: unknown) => void) | null = null;
  export let onratio: ((key: string, value: number) => void) | null = null;
  export let onworkspace: ((name: string) => void) | null = null;
  export let onlanguage: (() => void) | null = null;
  export let ontheme: (() => void) | null = null;
  export let onsave: (() => void) | null = null;
  export let onreview: (() => void) | null = null;
  /**
   * プロジェクト名を押した（`Ctrl+O` と同じ規則。§9.1・§9.7）。
   *
   * 押すと**プロジェクトの入口の面**を開く（変更を保存してから）。渡されていなければ
   * 何もしない（黙って別の画面へ移らない）。
   */
  export let onproject: (() => void) | null = null;
  /** 切替パネルを開く（`Ctrl+K`。§10.0）。 */
  export let onswitcher: (() => void) | null = null;
  /** 文書の名前を変える（`document.rename`。パレットの `>rename-document` と同じ）。 */
  export let onrename: (() => void) | null = null;
  /** 節を分ける（`section.split`。`Ctrl+Enter` と同じ。§8.1.1）。 */
  export let onsplit: ((caret: number) => void) | null = null;
  /** この節の提案を置く（`section.propose`。§8.7.1）。 */
  export let onpropose: (() => void) | null = null;
  /** 外部変更との衝突を解決する面を開く（§8.5）。 */
  export let onconflict: (() => void) | null = null;
  /** 衝突しているパス（`save.state` の `conflicts`）。 */
  export let conflicts: string[] = [];

  /**
   * セッション（正規操作の口。§21.1.3）。ワークスペースはこれを直接受け取る
   * （`CastWorkspace` / `IdeaWorkspace` / … は `session` を要求する）。
   *
   * `write` 以外の 8 つのワークスペースを**実際に描く**ための口である。渡されていなければ
   * それらは「この版では使えません」のままにする（黙って空の画面を出さない。§21.1.2）。
   */
  export let session: Session | null = null;
  /** 読み直しの合図（ワークスペースを切り替えるたびに 1 つ進む）。 */
  export let revision = 0;
  /** 参照ペインの行をダブルクリックした（`cast.get` / `dictionary.term_full`）。 */
  export let onplace: ((path: string, byte: number | null) => Promise<{ path: string; place: unknown } | null>) | null =
    null;
  /** 資料を添付する（`materials.attach`）。 */
  export let onattach: ((id: string, path: string) => Promise<string>) | null = null;
  /** 診断から節へ移る（点検の `Enter`）。 */
  export let onreviewgoto: ((scene: string) => void) | null = null;
  /** 通知（ワークスペースからの失敗の文言。画面の上部に出す）。 */
  export let onnotice: ((text: string, kind?: 'info' | 'error') => void) | null = null;
  /** 辞書の一覧を開く（`dictionary.list`。ヘッダの常設のボタン）。 */
  export let ondictionary: (() => void) | null = null;

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

  /**
   * `session.run` をそのまま渡すための関数（`run` を受け取るワークスペース向け）。
   *
   * セッションが繋がっていないときは `usage` の失敗を返す（例外を投げない）。
   * `session === null` の分岐は本文の描き分けで先に落ちるので、実際には届かない。
   */
  function run<T extends OpId>(op: T, args?: OpArgs[T]): Promise<Outcome<T>> {
    if (session === null) {
      return Promise.resolve({ ok: false, code: 'usage', message: '' } as Outcome<T>);
    }
    return session.run(op, args);
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
    onproject={onproject}
    onswitcher={onswitcher}
    onrename={onrename}
    {ondictionary}
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
        {onopen}
        onratio={onratio}
        onsplit={onsplit}
        onpropose={onpropose}
      />
    {:else if session === null}
      <!-- セッションが繋がっていないワークスペースは「この版では使えません」を出す
           （§21.1.2: 空の画面・動かないボタンを置かない）。 -->
      <p class="unavailable">{t(`header.workspace.${screen.workspace}`)}{t('workspace.unavailable', [''])}</p>
    {:else if screen.workspace === 'material'}
      <MaterialWorkspace {t} {session} {revision} {onattach} />
    {:else if screen.workspace === 'cast'}
      <CastWorkspace {t} run={run} onnav={onnav} onnotice={onnotice} {composing} />
    {:else if screen.workspace === 'idea'}
      <IdeaWorkspace {t} {session} scene={screen.scene} {revision} />
    {:else if screen.workspace === 'plot-episode'}
      <PlotEpisodeWorkspace {t} {session} />
    {:else if screen.workspace === 'plot-outline'}
      <PlotOutlineWorkspace {t} {session} />
    {:else if screen.workspace === 'reader'}
      <ReaderWorkspace {t} {screen} run={run} onworkspace={onworkspace} onnotice={onnotice} {composing} />
    {:else if screen.workspace === 'agent'}
      <AgentWorkspace {t} {session} {revision} />
    {:else if screen.workspace === 'review'}
      <ReviewWorkspace
        {t}
        {session}
        scene={screen.scene}
        {revision}
        {onplace}
        onopenthread={onreviewgoto}
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
    {conflicts}
    onconflict={onconflict}
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
