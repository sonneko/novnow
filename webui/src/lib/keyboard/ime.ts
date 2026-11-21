// IME の変換状態（§9.8.5。**MUST**）。
//
// 規則は 3 つだけ:
//
// 1. `compositionstart` / `compositionupdate` で「変換中」に入り、`compositionend` で出る。
// 2. 「変換中」のあいだは、すべてのショートカット・`Enter` による節確定・`Esc` による閉じを
//    抑止する（変換候補の選択・確定・取消に使うキーを横取りしない）。
// 3. 終了イベントが来ない環境があるので、最後の変換イベントから **2 秒**で自動的に解除する
//    （ウィンドウがフォーカスを失ったときも解除する）。
//
// 画面は `composing` を 1 か所で見るだけにし、`keydown` のハンドラは先頭で早期 return する。

/** 変換中を解除するまでの猶予（§9.8.5 の 3）。 */
export const FAILSAFE_MS = 2000;

/** IME の変換状態を持つもの。 */
export class Ime {
  private composing = false;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private readonly onChange: (composing: boolean) => void;

  constructor(onChange: (composing: boolean) => void = () => {}) {
    this.onChange = onChange;
  }

  /** 変換中か。 */
  get active(): boolean {
    return this.composing;
  }

  /** `compositionstart` / `compositionupdate`。 */
  start(): void {
    this.set(true);
    this.arm();
  }

  /** `compositionend`。 */
  end(): void {
    this.clear();
    this.set(false);
  }

  /** フォーカスを失った（§9.8.5 の 3 の 1 つ目）。 */
  blur(): void {
    this.clear();
    this.set(false);
  }

  /** 後片付け（画面を壊すとき）。 */
  dispose(): void {
    this.clear();
  }

  private set(next: boolean): void {
    if (this.composing === next) {
      return;
    }
    this.composing = next;
    this.onChange(next);
  }

  private arm(): void {
    this.clear();
    this.timer = setTimeout(() => {
      this.timer = null;
      this.set(false);
    }, FAILSAFE_MS);
  }

  private clear(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }
}
