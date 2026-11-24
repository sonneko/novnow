<script lang="ts">
  // 分割線（§9.1）。つまみを動かすと比率が変わり、`settings.set` で `ui.json` に保存する
  // （保存は正規操作を通す。画面が勝手にファイルを書かない。§21.1.3）。
  //
  // 比の制約は §9.1 のまま: メイン 0.3–0.85（参照ペイン 320〜560px）、文脈ビュー 0.2–0.8。
  // 制約の計算は CSS（`grid-template`）と `minmax` に任せ、ここでは「掴んだ量」だけを渡す。

  /** 向き（`horizontal` は左右に分ける縦線）。 */
  export let axis: 'horizontal' | 'vertical' = 'horizontal';
  /** いまの比率（0〜1）。 */
  export let ratio = 0.62;
  /** 動かし終わった（`settings.set` に渡す）。 */
  export let onchange: ((ratio: number) => void) | null = null;
  /** 動かしている（プレビュー用。保存はしない）。 */
  export let onmove: ((ratio: number) => void) | null = null;
  /** キーで動かす量（5% 刻み。§9.1）。 */
  export const step = 0.05;

  let dragging = false;
  let start = 0;
  let from = 0;
  let parent = 0;

  /** 掴んだ。 */
  function down(event: PointerEvent): void {
    dragging = true;
    start = axis === 'horizontal' ? event.clientX : event.clientY;
    from = ratio;
    const target = (event.currentTarget as HTMLElement | null)?.parentElement;
    parent = target ? (axis === 'horizontal' ? target.clientWidth : target.clientHeight) : 0;
    (event.currentTarget as HTMLElement | null)?.setPointerCapture?.(event.pointerId);
  }

  /** 動かした。 */
  function move(event: PointerEvent): void {
    if (!dragging || parent === 0) {
      return;
    }
    const now = axis === 'horizontal' ? event.clientX : event.clientY;
    const next = clamp(from + (now - start) / parent);
    onmove?.(next);
  }

  /** 離した。 */
  function up(event: PointerEvent): void {
    if (!dragging) {
      return;
    }
    dragging = false;
    const now = axis === 'horizontal' ? event.clientX : event.clientY;
    const next = clamp(from + (now - start) / (parent || 1));
    onchange?.(next);
  }

  /** キー（`ArrowLeft/Right` は 5% 刻み）。 */
  function keydown(event: KeyboardEvent): void {
    const back = axis === 'horizontal' ? 'ArrowLeft' : 'ArrowUp';
    const forward = axis === 'horizontal' ? 'ArrowRight' : 'ArrowDown';
    if (event.key === back) {
      event.preventDefault();
      onchange?.(clamp(ratio - step));
    } else if (event.key === forward) {
      event.preventDefault();
      onchange?.(clamp(ratio + step));
    }
  }

  /** §9.1 の範囲（メイン 0.3–0.85。文脈ビュー 0.2–0.8）。 */
  function clamp(value: number): number {
    const low = axis === 'horizontal' ? 0.3 : 0.2;
    const high = axis === 'horizontal' ? 0.85 : 0.8;
    return Math.min(Math.max(Math.round(value * 100) / 100, low), high);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  class:vertical={axis === 'vertical'}
  role="separator"
  aria-orientation={axis === 'horizontal' ? 'vertical' : 'horizontal'}
  aria-valuenow={Math.round(ratio * 100)}
  aria-valuemin={axis === 'horizontal' ? 30 : 20}
  aria-valuemax={axis === 'horizontal' ? 85 : 80}
  tabindex="0"
  aria-label={axis === 'horizontal' ? 'pane ratio' : 'context ratio'}
  on:pointerdown={down}
  on:pointermove={move}
  on:pointerup={up}
  on:pointercancel={up}
  on:keydown={keydown}
></div>

<style>
  .splitter {
    /* 見た目は 1px、当たり判定は 5px（§9.9.2 の 3） */
    width: 5px;
    margin: 0 -2px;
    cursor: col-resize;
    background: transparent;
    position: relative;
    z-index: 2;
  }
  .splitter.vertical {
    width: auto;
    height: 5px;
    margin: -2px 0;
    cursor: row-resize;
  }
  .splitter::after {
    content: '';
    position: absolute;
    inset: 0 2px;
    background: var(--border);
  }
  .splitter.vertical::after {
    inset: 2px 0;
  }
  .splitter:hover::after,
  .splitter:focus-visible::after {
    background: var(--accent);
  }
  .splitter:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
</style>
