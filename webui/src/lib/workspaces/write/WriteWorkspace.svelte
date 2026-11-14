<script lang="ts">
  // `write` ワークスペース（§10.1）。W2 で作るのはここだけである。
  //
  // 構成は §9.1 のとおり:
  //
  //   ┌───────────────────────┬──────────────┐
  //   │ 文脈ビュー（55%）      │ 参照ペイン    │
  //   ├───────────────────────┤              │
  //   │ 節エディタ（45%）      │              │
  //   └───────────────────────┴──────────────┘
  //
  // 比率は `ui.json` の `pane_ratio.main` / `pane_ratio.context_view` に保存する（§9.1）。
  // ただし W2 ではセッションからの読み出しが `settings.get` の戻り値に載る形になるので、
  // 画面の既定値（0.62 / 0.55）で始め、動かしたら `settings.set` で保存する。
  import SectionEditor from '../../editor/SectionEditor.svelte';
  import ContextView from '../../context/ContextView.svelte';
  import ReferencePane from '../../reference/ReferencePane.svelte';
  import Splitter from '../../shell/Splitter.svelte';
  import type { Screen } from '../../store/session';
  import type { Texts } from '../../i18n';

  /** 文言。 */
  export let t: Texts['t'];
  /** 画面の状態。 */
  export let screen: Screen;
  /** 変換中か（IME）。 */
  export let composing = false;
  /** 前後の節（`section.get`）。 */
  export let before: { scene: string; text: string }[] = [];
  export let after: { scene: string; text: string }[] = [];
  /** 台詞比率（§9.4.2。0〜1）。 */
  export let ratio = 0;
  /** 有効モードの表示（§9.4.2）。 */
  export let modes = '';
  /** 本文が変わった。 */
  export let oninput: ((text: string) => void) | null = null;
  /** キャレットが動いた。 */
  export let oncaret: ((position: number) => void) | null = null;
  /** キー（エディタが扱わなかったもの）。 */
  export let onkey: ((event: KeyboardEvent) => void) | null = null;
  /** 節を選んだ。 */
  export let onnav: ((scene: string) => void) | null = null;
  /** 節を消した。 */
  export let ondelete: ((scene: string) => void) | null = null;
  /** 参照ペインの行を押した。 */
  export let onjump: ((kind: string, value: unknown) => void) | null = null;
  /** 比率を保存した。 */
  export let onratio: ((key: string, value: number) => void) | null = null;
  /** 参照ペインを出すか（`Ctrl+\`）。 */
  export let reference = true;

  /** エディタ（`Ctrl+2` でフォーカスするために持つ）。 */
  let editor: SectionEditor | null = null;

  /** 分割比（`pane_ratio.main` / `pane_ratio.context_view`）。 */
  let main = 0.62;
  let context = 0.55;

  /** メインと参照の比を変えた（5% 刻みで保存）。 */
  function setMain(next: number): void {
    main = next;
    onratio?.('pane_ratio.main', next);
  }

  /** 文脈ビューと節エディタの比を変えた。 */
  function setContext(next: number): void {
    context = next;
    onratio?.('pane_ratio.context_view', next);
  }

  /** エディタへフォーカスする（`Ctrl+2`）。 */
  export function focusEditor(): void {
    editor?.focus();
  }

  /** エディタのキャレット（UTF-16。`nav.jump` に渡す）。 */
  export function caret(): number {
    return editor?.position() ?? 0;
  }

  /** エディタの行（1 始まり）。 */
  export function line(): number {
    return editor?.line() ?? 1;
  }
</script>

<div
  class="workspace"
  style={`grid-template-columns:${reference ? `${Math.round(main * 100)}% auto 1fr` : '1fr'};`}
>
  <div class="main" style={`grid-template-rows:${Math.round(context * 100)}% auto 1fr;`}>
    <ContextView
      {t}
      scene={screen.scene}
      {before}
      {after}
      sectionChars={screen.sectionChars}
      save={screen.save}
      onnav={onnav}
      {ondelete}
    />
    <Splitter axis="vertical" ratio={context} onchange={setContext} onmove={(next) => (context = next)} />
    <SectionEditor
      bind:this={editor}
      t={t}
      section={screen.sections.find((found) => found.id === screen.scene) ?? null}
      text={screen.text}
      {composing}
      save={screen.save}
      {ratio}
      {modes}
      oninput={oninput}
      oncaret={oncaret}
      onkey={onkey}
    />
  </div>
  {#if reference}
    <Splitter axis="horizontal" ratio={main} onchange={setMain} onmove={(next) => (main = next)} />
    <ReferencePane {t} pane={screen.reference} onjump={onjump} />
  {/if}
</div>

<style>
  .workspace {
    display: grid;
    min-height: 0;
    overflow: hidden;
  }
  .main {
    display: grid;
    min-height: 0;
    min-width: 0;
    /* 参照ペインは 320〜560px（§9.1） */
    contain: layout;
  }
  @media (max-width: 900px) {
    .workspace {
      /* 900px 未満は参照ペインを畳む（`Ctrl+3` でオーバーレイ。§9.1） */
      grid-template-columns: 1fr !important;
    }
  }
</style>
