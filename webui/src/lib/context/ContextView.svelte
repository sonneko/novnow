<script lang="ts">
  // 文脈ビュー（§9.3）。前後の節を読み取り専用で出す。
  //
  // 仕様の MUST:
  //
  // - 1 行の最大幅は全角 44 字（§9.3.1）。15px / 行間 1.9 / 段落間 8px。
  // - 節の境目に `border` の 1px 横線。節番号は**4 階層の完全形**を `fg.faint` の 10px で右寄せ。
  // - **編集中の節の本文は絶対に描画しない**（§9.3.2）。代わりに高さ 56px のプレースホルダ
  //   （✎・SceneId・文字数・保存状態の点）だけを置く。これがないと、同じ本文が画面の上下に
  //   二重に出て「どちらを直しているのか」が分からなくなる。
  // - スクロールは「現在の節が画面の上から 38%」に自動で追従する（§9.3.1）。
  // - 各節に削除ボタンを置く（§9.3.5）。削除の意味と確認は §8.1.2（機能核の仕事）。
  // - 畳んだ単位は 1 行（見出し ▸ 題 ▸ 文字数 ▸ 節数。§9.3.3）。現在の節を含む単位は畳めない。
  //
  // 文字数は**打鍵から 200ms 待って**更新する（§9.3.2 の 6）。節の一覧・前後の節は画面の
  // 状態から受け取り、ここでは解釈しない（§21.1.3）。
  import { tick } from 'svelte';
  import type { Texts } from '../i18n';
  import { charsWithoutSpace, grouped } from '../editor/text';
  import { rowsAround, type ContextRow } from './fold';

  /** 文言。 */
  export let t: Texts['t'];
  /** いまの節（`01-04-02-03`）。 */
  export let scene = '';
  /** 前後の節の中身（`section.get` の `before` / `after`）。 */
  export let before: { scene: string; text: string }[] = [];
  export let after: { scene: string; text: string }[] = [];
  /** いまの節の文字数（プレースホルダに出す。200ms デバウンス済みの値を渡す）。 */
  export let sectionChars = 0;
  /** 目標字数（`length:` があるときだけ。§9.3.2 の `412 / 500`）。 */
  export let target: number | null = null;
  /** 保存状態（プレースホルダの点）。 */
  export let save: string = 'idle';
  /** 畳んでいる単位の鍵（`@01-04`。§9.3.3）。 */
  export let folded: readonly string[] = [];
  /** 節を選んだ（`nav.section`）。 */
  export let onnav: ((scene: string) => void) | null = null;
  /** 節を消した（`section.delete`。確認は §8.1.2）。 */
  export let ondelete: ((scene: string) => void) | null = null;
  /** 畳んだ単位を開いた（鍵を渡す）。 */
  export let onfold: ((key: string) => void) | null = null;
  /** 折りたたみを使えるか（保存先 `ui.json` へ書ける配線があるか。§9.3.3）。 */
  export let foldable = false;
  /**
   * ポインタが載った（節を渡す。`Ctrl+Shift+[` / `]` はこの節の単位を畳む。§9.3.3）。
   *
   * 第 1 の画面実装（`REF:src/ui/widgets/context_view.rs`）は、ポインタの載っている節の
   * 祖先を畳む。同じ操作にするため、画面へ「いま載っている節」を伝える。
   */
  export let onhover: ((scene: string) => void) | null = null;

  /** スクロール領域（38% の追従に使う）。 */
  let root: HTMLElement | null = null;
  /** 編集中プレースホルダ（38% の基準）。 */
  let placeholder: HTMLElement | null = null;
  /** 最後に寄せた節（同じ節で毎回スクロールし直さない）。 */
  let scrolledTo = '';

  /**
   * 並び（前 → いま → 後。畳んだ単位は 1 行にまとめる。§9.3.3）。
   *
   * 編集中の節そのものは**常にプレースホルダ**として出す（§9.3.2 MUST）。畳んだ単位に
   * 現在の節が含まれていても、プレースホルダは消さない（本文は出さない）。
   */
  $: rows = rowsAround(
    before.map((row) => ({ scene: row.scene, text: row.text })),
    { scene, chars: sectionChars },
    after.map((row) => ({ scene: row.scene, text: row.text })),
    // 現在の節を含む単位は畳まない（§9.3.3 の MUST。画面が鍵を持っていてもここで守る）
    folded.filter((key) => !contains(key, scene)),
  ) as ContextRow[];

  /** `key`（`@01-04`）が節 `scene` を含むか。 */
  function contains(key: string, scene: string): boolean {
    const prefix = key.replace(/^@/u, '');
    return prefix.length > 0 && (scene === prefix || scene.startsWith(`${prefix}-`));
  }

  /** 節番号の表示（`01-04-02-03` をそのまま。§9.3.1 の完全形）。 */
  function label(id: string): string {
    return id;
  }

  /** 編集中の節が変わったら、その位置を画面の上から 38% に寄せる（§9.3.1）。 */
  $: if (scene && scene !== scrolledTo) {
    scrolledTo = scene;
    void tick().then(scrollToPlaceholder);
  }

  /** プレースホルダを 38% の位置へ。 */
  function scrollToPlaceholder(): void {
    if (!root || !placeholder) {
      return;
    }
    const top = placeholder.offsetTop - root.clientHeight * 0.38;
    root.scrollTop = Math.max(0, top);
  }

  /** 畳んだ単位を開く。 */
  function open(key: string): void {
    onfold?.(key);
  }
</script>

<section class="context" aria-label={t('editor.editing')} bind:this={root}>
  {#if scene}
    {#each rows as row (row.folded ? `@${row.id}` : row.id)}
      {#if row.folded}
        <!-- 畳んだ単位（§9.3.3）。出すのは 見出し ▸ 題 ▸ 文字数 ▸ 節数 の 4 つだけ。 -->
        <button
          type="button"
          class="folded"
          title={foldable ? t('help.fold') : undefined}
          on:click={() => open(`@${row.id}`)}
          on:mouseenter={() => onhover?.(`${row.id}-00-00-00`)}
        >
          <span class="mark">▸</span>
          <span class="folded-id">{label(row.id)}</span>
          {#if row.title}<span class="folded-title">{row.title}</span>{/if}
          <span class="dots" aria-hidden="true">·····</span>
          <span class="folded-count">{grouped(row.chars ?? 0)}{t('status.chars_unit')}</span>
          <span class="folded-sections">/ {t('stage.outline_sections_unit', [String(row.sections ?? 0)])}</span>
        </button>
      {:else if row.id === scene}
        <!-- 編集中プレースホルダ（§9.3.2）。本文は絶対に描画しない。 -->
        <div
          class="placeholder"
          data-save={save}
          bind:this={placeholder}
          role="presentation"
          on:mouseenter={() => onhover?.(row.id)}
        >
          <span class="pencil" aria-hidden="true">✎</span>
          <span class="id">{label(row.id)}</span>
          <span class="chars">
            {grouped(sectionChars)}{#if target !== null}<span class="target"> / {grouped(target)}</span>{/if}{t('status.chars_unit')}
          </span>
          <span class="dot" data-state={save} aria-hidden="true"></span>
          <button
            type="button"
            class="remove"
            aria-label={`${t('help.delete_section')}: ${row.id}`}
            title={t('help.delete_section')}
            on:click={() => ondelete?.(row.id)}
          >
            {t('delete.button')}
          </button>
        </div>
      {:else}
        <article class="section" on:mouseenter={() => onhover?.(row.id)}>
          <div class="head">
            <button
              type="button"
              class="remove"
              aria-label={`${t('help.delete_section')}: ${row.id}`}
              title={t('help.delete_section')}
              on:click={() => ondelete?.(row.id)}
            >
              {t('delete.button')}
            </button>
            <button
              type="button"
              class="head-id"
              title={t('context.guide_section')}
              on:click={() => onnav?.(row.id)}
            >
              {label(row.id)}
            </button>
          </div>
          <p class="prose">{row.text ?? ''}</p>
        </article>
      {/if}
    {/each}
    {#if !foldable}
      <!-- 折りたたみの保存先（`ui.json` の `folded.context_view`）へ書く配線がまだ無いとき、
           黙って効かないボタンを置かずに**未提供と書く**（§21.1.2 MUST）。 -->
      <p class="unavailable">{t('workspace.unavailable', [t('api.op.lang.folding')])}</p>
    {/if}
  {:else}
    <div class="guide">
      <p>{t('context.guide_workspace')}</p>
      <p>{t('context.guide_section')}</p>
      <p>{t('context.guide_palette')}</p>
    </div>
  {/if}
</section>

<style>
  .context {
    overflow: auto;
    padding: 8px 12px 16px;
    background: var(--bg-base);
    color: var(--fg-primary);
    font-size: 15px;
    line-height: 1.9;
  }
  .section {
    position: relative;
    border-top: 1px solid var(--border);
    padding-top: 8px;
    margin-bottom: 8px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    min-height: 14px;
  }
  .head-id {
    border: 0;
    background: transparent;
    color: var(--fg-faint);
    font: inherit;
    font-size: 10px;
    font-family: var(--mono, ui-monospace, monospace);
    cursor: pointer;
    padding: 0;
  }
  .head-id:hover {
    color: var(--accent);
    text-decoration: underline;
  }
  .remove {
    border: 0;
    background: transparent;
    color: var(--fg-faint);
    font: inherit;
    font-size: 10px;
    cursor: pointer;
    padding: 0 2px;
    /* 破壊的操作のラベルなので、常に置く（§9.3.5）。押しやすいよう当たり判定だけ広げる。 */
    min-width: 24px;
    min-height: 14px;
    opacity: 0.55;
  }
  .remove:hover,
  .remove:focus-visible {
    opacity: 1;
    color: var(--state-error);
  }
  .prose {
    margin: 4px 0 0;
    /* 1 行の最大幅は全角 44 字（§9.3.1）。全角 1 字 = 1em。 */
    max-width: 44em;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .placeholder {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 56px;
    margin: 8px 0;
    padding: 0 10px;
    background: var(--bg-inset);
    /* 編集中の節の左端の 3px 線（§9.3.4。節エディタのペインフォーカス線と対で見せる） */
    border-left: 3px solid var(--accent);
    color: var(--fg-muted);
    font-size: 11px;
  }
  .pencil {
    color: var(--fg-faint);
  }
  .id {
    font-family: var(--mono, ui-monospace, monospace);
  }
  .chars {
    margin-left: auto;
  }
  .target {
    color: var(--fg-faint);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: transparent;
  }
  .dot[data-state='dirty'] {
    background: var(--state-unsaved);
  }
  .dot[data-state='saving'] {
    background: var(--fg-muted);
  }
  .dot[data-state='error'],
  .dot[data-state='conflict'] {
    background: var(--state-error);
  }
  .folded {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    border: 0;
    border-top: 1px solid var(--border);
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 8px 0;
    cursor: pointer;
  }
  .folded:hover {
    color: var(--fg-primary);
  }
  .mark {
    color: var(--fg-faint);
  }
  .folded-id {
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 11px;
    color: var(--fg-faint);
  }
  .dots {
    color: var(--fg-faint);
    overflow: hidden;
    white-space: nowrap;
    flex: 1;
  }
  .folded-count,
  .folded-sections {
    color: var(--fg-faint);
    font-size: 11px;
  }
  .unavailable {
    margin: 12px 0 0;
    color: var(--fg-faint);
    font-size: 11px;
  }
  .guide {
    color: var(--fg-faint);
    font-size: 13px;
  }
  .guide p {
    margin: 2px 0;
  }
</style>
