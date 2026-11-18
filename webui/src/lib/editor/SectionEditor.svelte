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
  // 5. ミニヘッダーに置いてよいのは**パンくず・台詞比率バー・有効モードの 3 つだけ**（§9.4.2）。
  //    保存状態の点はステータスバーと文脈ビューのプレースホルダが持つ（ここには置かない）。
  //    台詞比率バーの色は**常に `fg.faint` 単色**（達成・未達で赤くしない。§9.4.2 の色の MUST）。
  // 6. 句切れ提案レイヤー（§9.4.3）は**縦線だけ**を描き、文字は挿入しない。`Alt+,` のときだけ
  //    `、` を入れる。
  // 7. 行内の診断は下線で示す（§11.3 の表。エラーは波線 1px `state.error`、警告は `state.warn`）。
  //
  // 第 1 の画面実装（`REF:src/ui/widgets/section_editor.rs`）と同じ規則を、同じ順序で適用している。

  import { tick } from 'svelte';
  import type { SectionRef } from '../store/session';
  import type { Texts } from '../i18n';
  import { lineColAt, linesOf, type Line } from './text';
  import { commaCandidates, insertComma, type CommaCandidate } from './comma';

  /** 節（`section.list` の要素）。 */
  export let section: SectionRef | null = null;
  /** 本文（節の中身）。 */
  export let text = '';
  /** 文言。 */
  export let t: Texts['t'];
  /** 変換中か（画面が持つ `Ime` の状態）。 */
  export let composing = false;
  /** 台詞比率（§9.4.2。0〜1）。**200ms デバウンス済みの値**を渡す。 */
  export let ratio = 0;
  /** 有効モードの表示（§9.4.2 の `'リリア B+C`）。 */
  export let modes = '';
  /** 句切れ提案を出すか（`ui.json` の `suggest_comma`。§9.4.3）。 */
  export let suggestComma = false;
  /** 句切れ提案の閾値（`writing.long_sentence_chars`。既定 120）。 */
  export let longSentenceChars = 120;
  /** この節の診断（`reference.pane` の `diagnostics`。`byte` は**節の中**の位置）。 */
  export let diagnostics: { byte: number | null; code: string; message: string; severity: string }[] = [];
  /** 本文が変わった（入力のたび）。 */
  export let oninput: ((text: string) => void) | null = null;
  /** キャレットが動いた（UTF-16）。 */
  export let oncaret: ((position: number) => void) | null = null;
  /** キーが押された（エディタが扱わなかったもの）。 */
  export let onkey: ((event: KeyboardEvent) => void) | null = null;
  /** フォーカスが入った。 */
  export let onfocus: (() => void) | null = null;
  /** 提案の候補の間を移った（`Alt+,` で挿入した）。 */
  export let oncomma: ((position: number) => void) | null = null;
  /**
   * 節を分ける（`Ctrl+Enter`。§8.1.1・§9.8.3）。
   *
   * 引数は**キャレットの位置**（UTF-16。バイトへの換算は `App.svelte` が `byteOffset` で行う）。
   * キーを知らなくても届くように、ミニバーのボタンからも呼ぶ（§21.2.1）。
   *
   * エディタが直接呼ぶ（`onkey` へ流さない）。キーを押した瞬間のキャレットは、画面の
   * キャレットを経由すると 1 打鍵ぶんずれることがあるため、ここで持っている値を渡す。
   */
  export let onsplit: ((caret: number) => void) | null = null;
  /** この節の提案を置く（`section.propose`。§8.7.1。本文は変えない）。 */
  export let onpropose: (() => void) | null = null;

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
  /**
   * 本文の行（診断の下線を**この時点で**切り分けておく）。
   *
   * 下線の断片を template の中で関数越しに取ると、Svelte はその依存を見ない（関数の外で
   * 変化しても描き直さない）。ここで 1 つの値にまとめておくと、行の中身の変化が確実に
   * 画面へ届く（`REF:docs/designs/webui.md` §21.2 の「1 つの値にまとめる」）。
   */
  $: parts = (() => {
    // `piecesOf` は `lineDiagnostics` を読むが、関数の中の依存は Svelte から見えない。
    // ここで 1 度読んで、診断が変わったら行の断片も作り直されるようにする。
    const byLine = lineDiagnostics;
    const rows = lines.map((line, index) => ({
      ...line,
      here: caret >= line.start && caret <= line.start + line.text.length,
      pieces: piecesOf(byLine, index, line.text),
    }));
    const index = rows.findIndex((row) => row.here);
    return { rows, index: index < 0 ? rows.length - 1 : index };
  })();

  /** 行ごとの診断（`byte` は**節の中**の位置。行の範囲へ切り詰める）。 */
  let lineDiagnostics: Map<
    number,
    { start: number; end: number; severity: string; code: string; message: string }[]
  > = new Map();
  $: lineDiagnostics = diagnosticsByLine(diagnostics, lines);

  /** 台詞比率バーの塗り（0〜1。分母 0 のときは 0 で、バーは描かない）。 */
  $: ratioValue = Math.max(0, Math.min(1, ratio));
  /** 台詞比率の分母があるか（空の節では描かない。§9.4.2 の 5）。 */
  $: hasRatio = text.trim().length > 0;

  /** 句切れの候補（§9.4.3）。オフのときは数えない。 */
  $: commaCandidates_ = suggestComma ? commaCandidates(text, longSentenceChars) : [];

  /** 句切れ候補の縦線の置き場所（描画後の実測。行の折り返しも含めて合わせる）。 */
  let commaMarks: { at: number; cue: number; left: number; top: number }[] = [];
  /** キャレットの置き場所（`lines` からの相対。`.caret` の `style` にそのまま入る）。 */
  let caretRect: { left: number; top: number; height: number } | null = null;

  // 候補・本文・キャレットが変わったら、次の描画の後で測り直す
  $: if (surface) {
    void tick().then(() => {
      placeCommas(commaCandidates_, text);
      updateAnchor();
      revealCaret();
    });
  }

  /**
   * 句切れ候補の縦線の置き場所を測る（§9.4.3）。
   *
   * 文字の間の位置は、行の `<span class="text">` の中に `Range` を作って測る。折り返した行でも
   * 正しい位置に出る（等幅の近似だと折り返しでずれる）。`Range` を作れない環境では何もしない。
   */
  function placeCommas(candidates: readonly CommaCandidate[], body: string): void {
    if (!surface) {
      return;
    }
    const rows = surface.querySelectorAll<HTMLElement>('.row');
    const out: { at: number; cue: number; left: number; top: number }[] = [];
    for (const candidate of candidates) {
      const found = lineColAt(linesOf(body), candidate.at);
      const row = rows[found.line];
      const cell = row?.querySelector<HTMLElement>('.text');
      if (!row || !cell) {
        continue;
      }
      const offset = measureInCell(cell, found.column);
      if (offset === null) {
        continue;
      }
      out.push({ at: candidate.at, cue: candidate.cue, left: offset.left, top: offset.top });
    }
    commaMarks = out;
  }

  /**
   * 行の中の文字位置（UTF-16）の、`lines` の中での位置。
   *
   * 文字の境目に `Range` を作り、その矩形の左上を返す。範囲を作れなければ `null`。
   * **折り返した行では行の `<span class="text">` の左上を使わない**（2 行目以降は折り返しの
   * 前の位置を指すため、縦線やキャレットが上へずれる）。`Range` の矩形から引く。
   */
  function measureInCell(cell: HTMLElement, column: number): { left: number; top: number } | null {
    const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT);
    let seen = 0;
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const content = node.textContent ?? '';
      const offset = Math.min(content.length, column - seen);
      if (seen + content.length < column) {
        seen += content.length;
        continue;
      }
      const range = document.createRange();
      range.setStart(node, offset);
      range.setEnd(node, offset);
      const rect = range.getBoundingClientRect();
      if (rect.height <= 0 && rect.width <= 0 && rect.top === 0 && rect.left === 0) {
        return null;
      }
      const origin = contentOrigin();
      if (!origin) {
        return null;
      }
      return { left: rect.left - origin.left, top: rect.top - origin.top };
    }
    return null;
  }

  /**
   * 本文の座標の原点（`lines` の中身の左上）と、可視範囲。
   *
   * 節エディタは `position: relative` のスクロール領域なので、中に絶対配置した要素の座標は
   * 「枠（padding box）の左上 + スクロール量」で測る（スクロールすると本文と一緒に動く）。
   */
  function contentOrigin(): { left: number; top: number; box: DOMRect } | null {
    if (!surface) {
      return null;
    }
    const box = surface.getBoundingClientRect();
    return {
      left: box.left + surface.clientLeft - surface.scrollLeft,
      top: box.top + surface.clientTop - surface.scrollTop,
      box,
    };
  }

  /** 行ごとの診断（`byte` は**節の中**の位置。行の範囲へ切り詰める）。 */


  /** 診断の `byte` を、その行の中の UTF-16 の位置へ直す（行内の範囲だけ）。 */
  function diagnosticsByLine(
    found: readonly { byte: number | null; severity: string; code: string; message: string }[],
    rows: readonly Line[],
  ): Map<number, { start: number; end: number; severity: string; code: string; message: string }[]> {
    const out = new Map<number, { start: number; end: number; severity: string; code: string; message: string }[]>();
    for (const row of found) {
      if (row.byte === null) {
        continue;
      }
      const index = lineIndexOfByte(rows, row.byte);
      if (index < 0) {
        continue;
      }
      const line = rows[index];
      if (!line) {
        continue;
      }
      const at = charIndexInLine(line.text, row.byte - byteStart(rows, index));
      if (at < 0) {
        continue;
      }
      const list = out.get(index) ?? [];
      list.push({ start: at, end: Math.min(line.text.length, at + 1), severity: row.severity, code: row.code, message: row.message });
      out.set(index, list);
    }
    return out;
  }

  /**
   * 節の中の byte 位置が何行目か（見つからなければ `-1`）。
   *
   * **行の終端は次の行の先頭と重なる**（`a\nb` の `b` は 2 の前後どちらでもない）。終端は
   * その行に含め、次の行の先頭と重ならないよう「次の行の先頭より小さい」で止める
   * （含めてしまうと、行頭ちょうどの診断が前の行の末尾に付く）。
   */
  function lineIndexOfByte(rows: readonly Line[], byte: number): number {
    for (let index = 0; index < rows.length; index += 1) {
      const start = byteStart(rows, index);
      const line = rows[index];
      if (!line) {
        continue;
      }
      const end = start + utf8Length(line.text);
      const last = index + 1 >= rows.length;
      if (byte >= start && (byte < end || (last && byte <= end))) {
        return index;
      }
      // 空行は `start === end`。ちょうどの位置はその空行のもの
      if (start === end && byte === start) {
        return index;
      }
    }
    return -1;
  }

  /** 行の先頭の byte 位置（節の中）。 */
  function byteStart(rows: readonly Line[], index: number): number {
    let at = 0;
    for (let step = 0; step < index; step += 1) {
      const line = rows[step];
      at += utf8Length(line?.text ?? '') + 1;
    }
    return at;
  }

  /** 文字列の UTF-8 の長さ。 */
  function utf8Length(found: string): number {
    return new TextEncoder().encode(found).length;
  }

  /** 行の中の byte 位置を UTF-16 の位置へ（見つからなければ `-1`）。 */
  function charIndexInLine(line: string, byte: number): number {
    if (byte < 0) {
      return -1;
    }
    let seen = 0;
    let index = 0;
    for (const found of line) {
      if (seen >= byte) {
        return index;
      }
      seen += utf8Length(found);
      index += found.length;
    }
    return seen >= byte ? index : -1;
  }

  /** 行の中身を、診断の下線つきの断片に分ける（§11.3）。 */
  function piecesOf(
    byLine: Map<number, { start: number; end: number; severity: string; code: string; message: string }[]>,
    index: number,
    line: string,
  ): { text: string; severity: string; message: string }[] {
    const found = byLine.get(index);
    if (!found || found.length === 0) {
      return [{ text: line, severity: '', message: '' }];
    }
    const points = new Set<number>([0, line.length]);
    for (const row of found) {
      points.add(Math.max(0, Math.min(line.length, row.start)));
      points.add(Math.max(0, Math.min(line.length, row.end)));
    }
    const sorted = [...points].sort((a, b) => a - b);
    const out: { text: string; severity: string; message: string }[] = [];
    for (let at = 0; at + 1 < sorted.length; at += 1) {
      const start = sorted[at] ?? 0;
      const end = sorted[at + 1] ?? 0;
      if (start === end) {
        continue;
      }
      const hit = found.find((row) => row.start <= start && end <= row.end);
      out.push({
        text: line.slice(start, end),
        severity: hit?.severity ?? '',
        message: hit?.message ?? '',
      });
    }
    return out;
  }

  /**
   * キャレットの矩形を測り、隠した入力欄をそこへ置く（§9.8.5 の 4）。
   *
   * キャレットは本文と同じ座標系に置く（スクロールすると一緒に動く）。`Range` で測れない
   * 環境では等幅の近似に落ちる（見た目は崩れるが、入力は続けられる）。
   */
  function updateAnchor(): void {
    if (!surface) {
      return;
    }
    const found = lineColAt(linesOf(text), caret);
    const row = surface.querySelector<HTMLElement>(`[data-line="${found.line}"]`);
    const cell = row?.querySelector<HTMLElement>('.text');
    if (!row || !cell) {
      anchor = { left: 0, top: 0, line: found.line };
      caretRect = null;
      return;
    }
    const style = getComputedStyle(row);
    const lineHeight = Number.parseFloat(style.lineHeight) || 28;
    const size = Number.parseFloat(style.fontSize) || 15;
    // 文字の境目を `Range` で測る。測れない環境（古いブラウザ・テスト）では近似へ落とす。
    const measured = measureInCell(cell, found.column);
    let left: number;
    let top: number;
    if (measured) {
      left = measured.left;
      top = measured.top;
    } else {
      const origin = contentOrigin();
      const cellBox = cell.getBoundingClientRect();
      const rowBox = row.getBoundingClientRect();
      left = cellBox.left - (origin?.left ?? cellBox.left) + estimateWidth((row.dataset.text ?? '').slice(0, found.column), size);
      top = rowBox.top - (origin?.top ?? rowBox.top);
    }
    // キャレットは文字の大きさに合わせる（行の高さ 1.9 倍いっぱいには伸ばさない。§9.4.5）。
    const height = size * 1.14;
    caretRect = { left, top: top + Math.max(0, (lineHeight - height) / 2), height };
    // 変換候補はキャレットのすぐ下に出す（§9.8.5 の 4）。
    anchor = { left, top: top + lineHeight - 2, line: found.line };
  }

  /**
   * キャレットを可視範囲へ入れる（§9.4.5）。
   *
   * 画面の外へ出たときだけ最小限にスクロールする（常に中央へ寄せない）。`scrollIntoView` は
   * 祖先すべてを動かしてしまうので使わない。
   */
  function revealCaret(): void {
    if (!surface || !caretRect) {
      return;
    }
    const top = caretRect.top;
    const bottom = top + caretRect.height;
    const viewTop = surface.scrollTop;
    const viewBottom = viewTop + surface.clientHeight;
    if (top < viewTop) {
      surface.scrollTop = Math.max(0, top - 8);
    } else if (bottom > viewBottom) {
      surface.scrollTop = bottom - surface.clientHeight + 8;
    }
  }

  /** おおよその文字幅（和文 1em・欧文 0.5em。`space` は 0.5em）。 */
  function estimateWidth(found: string, size: number): number {
    let width = 0;
    for (const ch of found) {
      width += /[\u3000-\u9fff\uff00-\uffef]/u.test(ch) ? size : size * 0.5;
    }
    return width;
  }

  /**
   * キャレットを移す。
   *
   * 隠した入力欄は**常に空**にする。空でないと 1 打鍵ごとに `input` が全値を返し、
   * 同じ文字が何度も本文へ入る（打鍵した文字だけが入るのは、欄が空のときだけ）。
   * IME の候補位置は欄そのものではなく、欄を置く `anchor`（キャレットの直下）で決める。
   */
  function moveCaret(next: number): void {
    caret = Math.min(Math.max(next, 0), text.length);
    if (hidden && hidden.value !== '') {
      hidden.value = '';
    }
    oncaret?.(caret);
    updateAnchor();
    revealCaret();
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

  /** `Alt+←` / `Alt+→` で候補の間を移る（§9.4.3）。移れなければ `false`。 */
  function stepComma(forward: boolean): boolean {
    const list = commaCandidates_;
    if (list.length === 0) {
      return false;
    }
    const chosen = forward
      ? list.find((row) => row.at > caret)
      : [...list].reverse().find((row) => row.at < caret);
    if (!chosen) {
      return false;
    }
    moveCaret(chosen.at);
    return true;
  }

  /** `Alt+,`（カーソル以降の直近の候補に `、` を入れる。§9.4.3）。 */
  function applyComma(): boolean {
    const list: CommaCandidate[] = commaCandidates_;
    const chosen = list.find((row) => row.at >= caret);
    if (!chosen) {
      return false;
    }
    const applied = insertComma(text, chosen.at);
    replace(applied.text, applied.caret);
    oncomma?.(applied.caret);
    return true;
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
    // 句切れ提案のキー（§9.4.3）。オンのときだけ先に扱う。
    if (suggestComma && event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
      if (event.key === 'ArrowLeft') {
        if (stepComma(false)) {
          event.preventDefault();
          return;
        }
      } else if (event.key === 'ArrowRight') {
        if (stepComma(true)) {
          event.preventDefault();
          return;
        }
      } else if (event.key === ',') {
        if (applyComma()) {
          event.preventDefault();
          return;
        }
      }
    }
    switch (event.key) {
      case 'Enter':
        if (event.ctrlKey || event.metaKey) {
          // `Ctrl+Enter` はカーソルのバイト位置で節を分ける（§8.1.1・§9.8.3）。
          // 分割の分岐（末尾なら次の節へ、先頭なら何もしない）は機能核が持つので、
          // ここは位置を渡すだけにする（§19.1.4）。
          if (onsplit) {
            event.preventDefault();
            onsplit(caret);
            return;
          }
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
<div class="mini">
  <span class="crumb" aria-hidden="true">{section?.id.replaceAll('-', '›') ?? ''}</span>
  {#if section}
    <!-- 節の操作（§8.1.1・§8.7.1）。キーを知らなくても押せるように置く（§21.2.1）。 -->
    <button
      type="button"
      class="action"
      title={`${t('api.op.section.split')} (Ctrl+Enter)`}
      aria-label={t('api.op.section.split')}
      on:click={() => onsplit?.(caret)}
    >
      <span aria-hidden="true">⑛</span>
    </button>
    <button
      type="button"
      class="action"
      title={t('api.op.section.propose')}
      aria-label={t('api.op.section.propose')}
      on:click={() => onpropose?.()}
    >
      <span aria-hidden="true">✎</span>
    </button>
  {/if}
  {#if hasRatio}
    <span class="ratio" title={t('stage.dialogue_ratio')}>
      <span class="bar" role="img" aria-label={`${t('stage.dialogue_ratio')} ${Math.round(ratioValue * 100)}%`}>
        <span class="fill" style={`width:${Math.round(ratioValue * 100)}%`}></span>
      </span>
      <span class="percent">{Math.round(ratioValue * 100)}%</span>
    </span>
  {/if}
  <span class="modes">{modes}</span>
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
            {#each row.pieces as piece}
              {#if piece.severity}
                <span
                  class="diag"
                  class:error={piece.severity === 'error'}
                  class:warning={piece.severity === 'warning'}
                  title={piece.message}
                >{piece.text}</span>
              {:else}
                {piece.text}
              {/if}
            {/each}
            {#if row.number - 1 === parts.index && pending}
              <span class="pending">{pending}</span>
            {/if}
          </span>
        </div>
      {/each}
    </div>
    <!-- キャレット（§9.4.5）。文字の大きさに合わせた縦線 1px を、ベースライン基準で置く。 -->
    {#if focused && caretRect}
      <span
        class="caret"
        aria-hidden="true"
        style={`left:${caretRect.left}px;top:${caretRect.top}px;height:${caretRect.height}px`}
      ></span>
    {/if}
    <!-- 句切れ提案の縦線（§9.4.3。1px × 高さ 10px・`fg.faint` の 40%）。文字は描かない。 -->
    {#if suggestComma}
      {#each commaMarks as mark (mark.at)}
        <span
          class="comma"
          aria-hidden="true"
          style={`left:${mark.left}px;top:${mark.top}px`}
          data-cue={mark.cue}
        ></span>
      {/each}
    {/if}
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
  .action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 16px;
    padding: 0;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 11px;
    line-height: 1;
    cursor: pointer;
  }
  .action:hover {
    background: var(--bg-selection);
    color: var(--fg-primary);
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
    /* 台詞比率バーの色は**常に `fg.faint` 単色**（§9.4.2 の色の MUST。赤くしない） */
    background: var(--border);
  }
  .fill {
    display: block;
    height: 4px;
    background: var(--fg-faint);
  }
  .percent {
    font-size: 10px;
  }
  .modes {
    margin-left: auto;
    font-size: 10px;
    color: var(--fg-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lines {
    flex: 1;
    min-height: 0;
    padding: 4px 0 24px;
    cursor: text;
    position: relative;
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
    line-height: 2.85;
    user-select: none;
  }
  .text {
    flex: 1;
    min-width: 0;
    padding-left: 2px;
    position: relative;
  }
  /* §11.3: エラーは波線 1px `state.error`、警告は波線 1px `state.warn`（どちらもベースライン + 2px） */
  .diag.error {
    text-decoration: underline wavy var(--state-error) 1px;
    text-underline-offset: 2px;
  }
  .diag.warning {
    text-decoration: underline wavy var(--state-warn) 1px;
    text-underline-offset: 2px;
  }
  .pending {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .caret {
    position: absolute;
    width: 1px;
    /* キャレットの色（§9.4.5。`fg.primary` の対で見える色） */
    background: var(--fg-primary);
    pointer-events: none;
    animation: blink 1s step-end infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  .comma {
    position: absolute;
    width: 1px;
    height: 10px;
    /* `fg.faint` の 40%（§9.4.3）。`color-mix` で透明度を出す（トークン以外の色を足さない）。 */
    background: color-mix(in srgb, var(--fg-faint) 40%, transparent);
    pointer-events: none;
    margin-top: 0.7em;
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
