<script lang="ts">
  // 節エディタ（§9.4）。**`<textarea>` を使わない**（§9.4 の設計指定）。
  //
  // なぜ `<textarea>` を使わないか: 行ごとの下線・診断の波線・インレイヒントを重ねるには、
  // 行を要素として持つ必要がある。`<textarea>` の中は装飾できない（`::first-line` 程度）。
  //
  // 作りの要点:
  //
  // 1. 見た目は行単位の `<div>`。1 行 = 1 要素で、行番号もその中に置く（§9.4.1 のローカル行番号）。
  // 2. 入力は **隠した `<input>`（1 行）** で受ける。`input` は 1 行なので、改行は
  //    `keydown` の `Enter` で自前に入れる（`compositionend` の直後の `Enter` は IME の
  //    確定なので Ime が見て止める。§9.8.5）。
  // 3. キャレットは `position`（UTF-16）で持ち、`keydown` / `input` / クリックのたびに更新する。
  //    隠した `<input>` は **常にキャレットの直下**へ置く（§9.8.5 の 4。変換候補の位置）。
  // 4. IME の変換中は `keydown` を先頭で捨てる（`composing` を 1 か所で見る）。
  //
  // 第 1 の画面実装（`REF:src/ui/editor.rs`）と同じ規則を、同じ順序で適用している。

  import type { SectionRef } from '../store/session';
  import type { Texts } from '../i18n';
  import { lineColAt, linesOf, type Line } from './text';

  /** 節（`section.list` の要素）。 */
  export let section: SectionRef | null = null;
  /** 本文（節の中身）。 */
  export let text = '';
  /** 文言。 */
  export let t: Texts['t'];
  /** 変換中か（画面が持つ `Ime` の状態）。 */
  export let composing = false;
  /** 保存状態（ミニヘッダーのドット）。 */
  export let save: 'idle' | 'dirty' | 'saving' | 'saved' | 'error' | 'conflict' = 'idle';
  /** 台詞比率（§9.4.2。0〜1）。 */
  export let ratio = 0;
  /** 有効モードの表示（§9.4.2 の `'リリア B+C`）。 */
  export let modes = '';
  /** 本文が変わった（入力のたび）。 */
  export let oninput: ((text: string) => void) | null = null;
  /** キャレットが動いた（UTF-16）。 */
  export let oncaret: ((position: number) => void) | null = null;
  /** キーが押された（エディタが扱わなかったもの）。 */
  export let onkey: ((event: KeyboardEvent) => void) | null = null;
  /** フォーカスが入った。 */
  export let onfocus: (() => void) | null = null;

  let hidden: HTMLInputElement | null = null;
  let surface: HTMLElement | null = null;
  let caret = 0;
  let focused = false;
  /** 変換中の文字（下線を引く。`compositionupdate` の `data`）。 */
  let pending = '';
  /** 変換候補を出す位置（キャレットの矩形。`position` で置く）。 */
  let anchor = { left: 0, top: 0, line: 0 };

  // 本文が外から変わったら、キャレットを末尾に寄せ直す（節を移ったとき）
  let lastText = text;
  $: if (text !== lastText) {
    lastText = text;
    caret = Math.min(caret, text.length);
    updateAnchor();
  }

  $: lines = linesOf(text) as Line[];
  $: parts = (() => {
    const rows = lines.map((line) => ({
      ...line,
      here: caret >= line.start && caret <= line.start + line.text.length,
    }));
    const index = rows.findIndex((row) => row.here);
    return { rows, index: index < 0 ? rows.length - 1 : index };
  })();

  /** キャレットの矩形を測り、隠した入力欄をそこへ置く（§9.8.5 の 4）。 */
  function updateAnchor(): void {
    if (!surface) {
      return;
    }
    const found = lineColAt(linesOf(text), caret);
    const row = surface.querySelector<HTMLElement>(`[data-line="${found.line}"]`);
    if (!row) {
      anchor = { left: 0, top: 0, line: found.line };
      return;
    }
    const style = getComputedStyle(row);
    const lineHeight = Number.parseFloat(style.lineHeight) || 28;
    const paddingLeft = Number.parseFloat(style.paddingLeft) || 0;
    // 文字幅は等幅の近似（和文は 1em、欧文は 0.5em）。厳密な測定より、ずれても候補が
    // 近くに出ることを優先する（`Range` を作ると行ごとの再計測が要る）。
    const text_ = row.dataset.text ?? '';
    const upTo = text_.slice(0, found.column);
    const width = estimateWidth(upTo, Number.parseFloat(style.fontSize) || 15);
    anchor = {
      left: row.offsetLeft + paddingLeft + width,
      top: row.offsetTop + lineHeight - 2,
      line: found.line,
    };
  }

  /** おおよその文字幅（和文 1em・欧文 0.5em。`space` は 0.5em）。 */
  function estimateWidth(found: string, size: number): number {
    let width = 0;
    for (const ch of found) {
      width += /[\u3000-\u9fff\uff00-\uffef]/u.test(ch) ? size : size * 0.5;
    }
    return width;
  }

  /** キャレットを移す（隠した入力欄の `selectionStart` も合わせる）。 */
  function moveCaret(next: number): void {
    caret = Math.min(Math.max(next, 0), text.length);
    if (hidden && hidden.value !== '') {
      hidden.value = '';
    }
    oncaret?.(caret);
    updateAnchor();
  }

  /** 本文を差し替える（`input` から呼ぶ。改行を保つ）。 */
  function replace(next: string, after: number, emit = true): void {
    text = next;
    lastText = next;
    moveCaret(after);
    if (emit) {
      oninput?.(next);
    }
  }

  /** 入力欄が受けた文字を本文のキャレットへ入れる。 */
  function insert(found: string): void {
    const before = text.slice(0, caret);
    const after = text.slice(caret);
    replace(before + found + after, caret + found.length);
  }

  /** `input` イベント（合成中の文字も含む。確定は `compositionend`）。 */
  function onInput(): void {
    if (!hidden) {
      return;
    }
    const found = hidden.value;
    hidden.value = '';
    if (!found) {
      return;
    }
    insert(found);
  }

  /** `compositionupdate`（変換中の文字を下線つきで見せる）。 */
  function onCompositionUpdate(event: CompositionEvent): void {
    pending = event.data ?? '';
  }

  /** `compositionend`（確定した文字を本文へ入れる）。 */
  function onCompositionEnd(event: CompositionEvent): void {
    const found = event.data ?? '';
    pending = '';
    if (found) {
      insert(found);
    }
  }

  /** `keydown`（エディタが使うキーだけ扱い、残りは画面の写像へ渡す）。 */
  function onKeydown(event: KeyboardEvent): void {
    if (composing) {
      // 変換中のキーは 1 つも横取りしない（§9.8.5 の 2）
      return;
    }
    if (event.isComposing) {
      return;
    }
    switch (event.key) {
      case 'Enter':
        if (event.ctrlKey || event.metaKey) {
          onkey?.(event);
          return;
        }
        event.preventDefault();
        insert('\n');
        return;
      case 'Backspace': {
        if (caret === 0) {
          return;
        }
        event.preventDefault();
        const before = text.slice(0, caret);
        const removed = Array.from(before).pop() ?? '';
        replace(before.slice(0, before.length - removed.length) + text.slice(caret), caret - removed.length);
        return;
      }
      case 'Delete': {
        if (caret >= text.length) {
          return;
        }
        event.preventDefault();
        const removed = Array.from(text.slice(caret)).shift() ?? '';
        replace(text.slice(0, caret) + text.slice(caret + removed.length), caret);
        return;
      }
      case 'ArrowLeft':
        if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
          onkey?.(event);
          return;
        }
        event.preventDefault();
        moveCaret(caret - 1);
        return;
      case 'ArrowRight':
        if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
          onkey?.(event);
          return;
        }
        event.preventDefault();
        moveCaret(caret + 1);
        return;
      case 'ArrowUp':
      case 'ArrowDown':
      case 'Home':
      case 'End':
      case 'PageUp':
      case 'PageDown':
      case 'Tab':
      case 'Escape': {
        const found = lineColAt(linesOf(text), caret);
        if (event.key === 'Tab' || event.key === 'Escape' || event.altKey) {
          onkey?.(event);
          return;
        }
        event.preventDefault();
        const rows = linesOf(text);
        if (event.key === 'ArrowUp') {
          const row = rows[Math.max(0, found.line - 1)];
          moveCaret((row?.start ?? 0) + Math.min(found.column, row?.text.length ?? 0));
        } else if (event.key === 'ArrowDown') {
          const row = rows[Math.min(rows.length - 1, found.line + 1)];
          moveCaret((row?.start ?? 0) + Math.min(found.column, row?.text.length ?? 0));
        } else if (event.key === 'Home') {
          moveCaret(rows[found.line]?.start ?? 0);
        } else {
          const row = rows[found.line];
          moveCaret((row?.start ?? 0) + (row?.text.length ?? 0));
        }
        return;
      }
      default:
        onkey?.(event);
    }
  }

  /** 行のクリックでキャレットを置く（行の文字位置は近似）。 */
  function onLineClick(event: MouseEvent, line: Line): void {
    if (!focused) {
      focus();
    }
    const target = event.currentTarget as HTMLElement | null;
    if (!target) {
      return;
    }
    const rect = target.getBoundingClientRect();
    const style = getComputedStyle(target);
    const paddingLeft = Number.parseFloat(style.paddingLeft) || 0;
    const size = Number.parseFloat(style.fontSize) || 15;
    let width = 0;
    let column = 0;
    for (const ch of line.text) {
      const step = /[\u3000-\u9fff\uff00-\uffef]/u.test(ch) ? size : size * 0.5;
      if (event.clientX - rect.left - paddingLeft < width + step / 2) {
        break;
      }
      width += step;
      column += ch.length;
    }
    moveCaret(line.start + column);
  }

  /** エディタへフォーカスする（`Ctrl+2` からも呼べる）。 */
  export function focus(): void {
    hidden?.focus({ preventScroll: true });
  }

  /** キャレットの位置（UTF-16。画面が `nav.jump` に渡す）。 */
  export function position(): number {
    return caret;
  }

  /** 行番号（1 始まり。`nav.jump` の `line` に渡す）。 */
  export function line(): number {
    return lineColAt(linesOf(text), caret).line + 1;
  }

  /** キャレットを外から置く（節を開いた直後など）。 */
  export function setCaret(next: number): void {
    moveCaret(next);
    focus();
  }
</script>

<!-- ミニヘッダー（§9.4.2）。置いてよいのはパンくず・台詞比率バー・有効モードの 3 つだけ。 -->
<div class="mini" aria-hidden="true">
  <span class="crumb">{section?.id.replaceAll('-', '›') ?? ''}</span>
  <span class="ratio" title={t('pane.prose')}>
    <span class="bar"><span class="fill" style={`width:${Math.round(ratio * 100)}%`}></span></span>
    <span class="percent">{Math.round(ratio * 100)}%</span>
  </span>
  <span class="modes">{modes}</span>
  <span class="save" data-state={save} title={t('status.saving')}></span>
</div>

<div class="editor" bind:this={surface} class:focused>
  {#if section}
    <div class="lines" role="textbox" aria-multiline="true" tabindex="-1">
      {#each parts.rows as row (row.number)}
        <div
          class="row"
          class:selected={row.number - 1 === parts.index}
          data-line={row.number - 1}
          data-text={row.text}
          role="presentation"
          on:click={(event) => onLineClick(event, row)}
        >
          <span class="number">{row.number}</span>
          <span class="text">
            {row.text}
            {#if row.number - 1 === parts.index && pending}
              <span class="pending">{pending}</span>
            {/if}
          </span>
        </div>
      {/each}
    </div>
    <!-- 隠した入力欄。キャレットの直下に置くことで、変換候補がキャレットの近くに出る。 -->
    <div class="anchor" style={`left:${anchor.left}px;top:${anchor.top}px`}>
      <input
        bind:this={hidden}
        class="hidden"
        type="text"
        aria-label={t('editor.editing')}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        value=""
        on:input={onInput}
        on:keydown={onKeydown}
        on:compositionstart={() => (pending = '')}
        on:compositionupdate={onCompositionUpdate}
        on:compositionend={onCompositionEnd}
        on:focus={() => {
          focused = true;
          onfocus?.();
        }}
        on:blur={() => {
          focused = false;
        }}
      />
    </div>
  {:else}
    <p class="empty">{t('editor.no_sections')}</p>
  {/if}
</div>

<style>
  /* 色は `tokens.ts` の CSS 変数だけを使う（§11.1 MUST。生の色を書かない）。 */
  .editor {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
    border-left: 3px solid var(--accent-faint);
    background: var(--bg-base);
    color: var(--fg-primary);
  }
  .editor.focused {
    /* フォーカス中は濃い青（§9.2.4 の二段表示） */
    border-left-color: var(--accent);
  }
  .mini {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 18px;
    padding: 0 8px;
    font-size: 10px;
    color: var(--fg-muted);
  }
  .crumb {
    font-family: var(--mono, ui-monospace, monospace);
  }
  .ratio {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .bar {
    display: inline-block;
    width: 64px;
    height: 4px;
    background: var(--border);
  }
  .fill {
    display: block;
    height: 4px;
    background: var(--accent);
  }
  .percent {
    font-size: 10px;
  }
  .modes {
    margin-left: auto;
    font-size: 10px;
    color: var(--fg-faint);
  }
  .save {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: transparent;
  }
  .save[data-state='dirty'] {
    background: var(--state-unsaved);
  }
  .save[data-state='saving'] {
    background: var(--fg-muted);
  }
  .save[data-state='saved'] {
    background: var(--state-ok);
  }
  .save[data-state='error'],
  .save[data-state='conflict'] {
    background: var(--state-error);
  }
  .lines {
    flex: 1;
    min-height: 0;
    padding: 4px 0 24px;
    cursor: text;
  }
  .row {
    display: flex;
    gap: 8px;
    min-height: 1.9em;
    line-height: 1.9;
    font-size: 15px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .row.selected {
    background: color-mix(in srgb, var(--bg-selection) 40%, transparent);
  }
  .number {
    flex: 0 0 3em;
    text-align: right;
    color: var(--fg-faint);
    font-size: 10px;
    user-select: none;
  }
  .text {
    flex: 1;
    min-width: 0;
    padding-left: 2px;
  }
  .pending {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .anchor {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
  }
  .hidden {
    /* 見えないが、フォーカスは受ける（`opacity: 0` は IME の候補位置を壊す環境があるので
       `clip` で隠す）。 */
    position: absolute;
    width: 1px;
    height: 1em;
    padding: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: transparent;
    caret-color: transparent;
    clip-path: inset(50%);
  }
  .empty {
    padding: 16px;
    color: var(--fg-faint);
    font-size: 13px;
  }
</style>
