// 台詞比率（§9.4.2 の算法。**決定論・MUST**）。
//
// 数え方は仕様の 5 段をそのまま実装する:
//
// 1. 対象は**現在の節**だけ（本文全体ではない）。
// 2. 台詞文字数 = `「…」` と `『…』` の**内側**の文字数の合計（鉤括弧そのものは数えない）。
// 3. 地の文字数 = 節の全文字数 − 台詞文字数 − アノテーション（§5.0.7 の 9 種）の文字数。
// 4. 空白・改行は両方から除外する。
// 5. 比率 = 台詞 ÷（台詞 + 地の文）。分母 0 のときはバーを描かない（`value` は `0`）。
//
// 実装では 3 を「アノテーションを落とした本文の文字数 − 台詞」として求める（注釈を落としてから
// 数えるので、生の文字数から注釈を引くと二重に引くことになる）。この形にすると
// **台詞 + 地の文 = アノテーションを落とした本文の文字数** が常に成り立ち、比率が 1 を超えない。
//
// アノテーションの落とし方は第 1 の画面実装（`REF:src/core/export.rs` の `plain()`）と同じ:
// 見出し行・節区切り行・行頭 `# ` コメント行・挿絵メモ行は行ごと落とし、行の中の
// 行内コメント `/* */`・行コメント `//`・引用注釈 `^M0137`・話者注釈 `@名前`・参照シジル `'`
// を落とす（参照の**表示名は残す**。§5.0.7 の 3）。エスケープ（`\'` など）は 1 文字に戻す。
//
// 入れ子の鉤括弧（`「『…』」`）は内側を**二重に数えない**（§9.4.2 の 3 が
// 「全文字数 − 台詞 − 注釈」という引き算で書かれているため、二重に数えると引き算が合わなくなる）。
//
// このファイルは**画面に依存しない**（`webui/tests/write.test.mjs` がそのまま読む）。

/** 台詞比率の材料（数えた結果）。 */
export interface Ratio {
  /** 台詞の文字数（空白・注釈を除く）。 */
  dialogue: number;
  /** 地の文の文字数（空白・注釈を除く）。 */
  ground: number;
  /** 台詞 ÷（台詞 + 地の文）。分母 0 のときは `0`。 */
  value: number;
}

/** 空白（半角・全角の空白と改行。§9.4.2 の 4）。`\s` は全角空白 `U+3000` も含む。 */
const SPACE = /\s/u;

/** エスケープの対象（§5.0.3。`REF:src/core/text/lexer.rs` の `ESCAPE_TARGETS` と同じ）。 */
const ESCAPABLE = '@\';/"「」\\#!^';

/**
 * 節の本文から台詞比率を数える（§9.4.2）。
 *
 * `body` は節の中身（`section.get` の `text`。見出し行は含まれない）。
 */
export function dialogueRatio(body: string): Ratio {
  const plain = withoutAnnotations(body);
  let total = 0;
  for (const found of plain) {
    if (!SPACE.test(found)) {
      total += 1;
    }
  }
  let dialogue = 0;
  for (const [start, end] of union(dialogueSpans(plain))) {
    for (const found of plain.slice(start, end)) {
      if (!SPACE.test(found)) {
        dialogue += 1;
      }
    }
  }
  const ground = Math.max(0, total - dialogue);
  const denominator = dialogue + ground;
  return {
    dialogue,
    ground,
    value: denominator === 0 ? 0 : dialogue / denominator,
  };
}

/**
 * 台詞の内側の範囲（`[始まり, 終わり)`。**UTF-16 の位置**。§9.4.2 の 2）。
 *
 * 入れ子（`「『…』」`）は内側と外側の両方を返す（重なりは [`union`] がまとめる）。
 * 閉じ忘れは**行末**までを台詞とみなす（行をまたいで地の文を台詞にしない。
 * `REF:src/core/resolve/role_language.rs` の `dialogue_spans` と同じ規則）。
 */
export function dialogueSpans(text: string): [number, number][] {
  const quotes: [string, string][] = [
    ['「', '」'],
    ['『', '』'],
  ];
  const stack: { close: string; start: number }[] = [];
  const out: [number, number][] = [];
  for (let at = 0; at < text.length; at += 1) {
    const found = text[at] ?? '';
    const opened = quotes.find(([open]) => open === found);
    if (opened) {
      stack.push({ close: opened[1], start: at + 1 });
      continue;
    }
    const top = stack[stack.length - 1];
    if (top && top.close === found) {
      stack.pop();
      out.push([top.start, at]);
    }
  }
  for (const left of stack) {
    const end = text.indexOf('\n', left.start);
    out.push([left.start, end < 0 ? text.length : end]);
  }
  out.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  return out;
}

/**
 * 範囲を昇順に並べ、重なり・隣接を 1 本にまとめる（`REF:src/core/resolve/role_language.rs`
 * の `union` と同じ規則）。
 */
export function union(ranges: [number, number][]): [number, number][] {
  const sorted = ranges.slice().sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const out: [number, number][] = [];
  for (const range of sorted) {
    const last = out[out.length - 1];
    if (last && range[0] <= last[1]) {
      last[1] = Math.max(last[1], range[1]);
    } else {
      out.push([range[0], range[1]]);
    }
  }
  return out;
}

/**
 * アノテーション（§5.0.7 の 9 種）を落とした本文。
 *
 * 落とすのは 1（構造見出し行）・2（行末話者注釈）・3（参照シジルと修飾子。**表示名は残す**）・
 * 4（節区切り `;`）・5（行コメント・行内コメント）・6（行頭 `# `）・7（行末の資料引用注釈
 * `^M0137`）・8（挿絵メモ行 `! `）。9（フィールド行）は本文に現れないので扱わない。
 *
 * ルビ（`｜親文字《読み》`）は §5.0.7 の 9 種に**入っていない**ので残す。
 */
export function withoutAnnotations(body: string): string {
  const kept: string[] = [];
  for (const line of body.split('\n')) {
    // 1・4・6・8: 行ごと落とす行
    if (isHeading(line) || isBreak(line) || isCommentLine(line) || isIllustration(line)) {
      continue;
    }
    kept.push(withoutLineAnnotations(line));
  }
  return kept.join('\n');
}

/** 1: 構造見出し行（`@A-B-C[-D] 題`。行頭の `@` の直後が構造 ID の形）。 */
function isHeading(line: string): boolean {
  return /^\s*@\d+(?:-\d+){1,3}(?:\s|$)/u.test(line);
}

/** 4: 節区切り（`;` だけの行）。 */
function isBreak(line: string): boolean {
  return /^\s*;\s*$/u.test(line);
}

/** 6: 行頭コメント行（`# `。`#` の直後が空白か行末。§5.0.8）。 */
function isCommentLine(line: string): boolean {
  return /^#(?:\s|$)/u.test(line);
}

/** 8: 挿絵メモ行（`! `）。 */
function isIllustration(line: string): boolean {
  return /^\s*!(?:\s|$)/u.test(line);
}

/** 5・7・2・3: 行の中の注釈を落とす（参照の表示名は残す）。 */
function withoutLineAnnotations(line: string): string {
  let found = line;
  // 5: 行内コメント `/* … */`
  found = found.replace(/\/\*[\s\S]*?\*\//gu, '');
  // 5: 行コメント `//`（エスケープされていないもの）
  const comment = unescapedIndex(found, '//');
  if (comment >= 0) {
    found = found.slice(0, comment);
  }
  // 7: 行末の資料引用注釈 `^M0137` / `^S0042`（連なっていれば全部）
  for (;;) {
    const next = found.replace(/\s*\^[MS]\d{4,}\s*$/u, '');
    if (next === found) {
      break;
    }
    found = next;
  }
  // 2: 行末の話者注釈 `@名前[:モード]`。落とすのは**直前の空白ごと行末まで**（§17.1-5）で、
  //    行の残り（本文）は消さない。本文が空になった行は地の文に数えない（見出し行と同じ扱い）。
  found = withoutSpeaker(found);
  // 3: 参照シジルとエスケープの解除
  return withoutSigils(found);
}

/**
 * 行末の話者注釈を落とす（§5.2.1 `speaker_annot`）。
 *
 * 規則は `REF:src/core/syntax/contents.rs` の `trailing_speaker` と同じ: **最後の
 * エスケープされていない `@`** で、そこから行末までが空白を含まないときだけ注釈とみなす
 * （直前の空白も注釈に含めて落とす）。形に合わなければそのまま残す（W002 の `@` と同じ扱い）。
 */
function withoutSpeaker(line: string): string {
  let last = -1;
  for (let at = 0; at < line.length; at += 1) {
    if (line[at] !== '@') {
      continue;
    }
    let slashes = 0;
    for (let back = at - 1; back >= 0 && line[back] === '\\'; back -= 1) {
      slashes += 1;
    }
    if (slashes % 2 === 0) {
      last = at;
    }
  }
  if (last < 0) {
    return line;
  }
  const tail = line.slice(last + 1).trimEnd();
  if (tail.length === 0 || /\s/u.test(tail)) {
    return line;
  }
  // 注釈は**行末まで**（`@` より後ろだけを落とす）。本文は消さない。
  return line.slice(0, last);
}

/**
 * 参照シジルを落とし、エスケープ（§5.0.3）を 1 文字に戻す（§5.0.7 の 3）。
 *
 * 1 度だけ走査する。エスケープは「消費」してから 1 文字にするので、`\'` の `'` を
 * シジルとして落としてしまわない（`REF:src/core/export.rs` はエスケープを伏せてから参照を探す）。
 */
function withoutSigils(line: string): string {
  let out = '';
  for (let at = 0; at < line.length; at += 1) {
    const found = line[at] ?? '';
    if (found === '\\') {
      const target = line[at + 1];
      if (target !== undefined && ESCAPABLE.includes(target)) {
        out += target;
        at += 1;
        continue;
      }
      out += found;
      continue;
    }
    if (found === "'") {
      // `'{…}` は中身を残す（中身は表示名）
      if (line[at + 1] === '{') {
        const close = line.indexOf('}', at + 2);
        if (close >= 0) {
          out += line.slice(at + 2, close);
          at = close;
          continue;
        }
      }
      // 裸の `'` は落とす（直後の表示名はそのまま残る）
      continue;
    }
    out += found;
  }
  return out;
}

/** エスケープされていない `needle` の位置（無ければ `-1`）。 */
function unescapedIndex(text: string, needle: string): number {
  let from = 0;
  for (;;) {
    const at = text.indexOf(needle, from);
    if (at < 0) {
      return -1;
    }
    let slashes = 0;
    for (let back = at - 1; back >= 0 && text[back] === '\\'; back -= 1) {
      slashes += 1;
    }
    if (slashes % 2 === 0) {
      return at;
    }
    from = at + needle.length;
  }
}

/**
 * 待ってから 1 度だけ実行する仕組み（§9.3.2・§9.4.2 の 200ms デバウンス）。
 *
 * 打鍵のたびに節全体を数え直すと、長い節で 1 文字ごとに走査することになる。最後の打鍵から
 * `wait` ミリ秒待ってから 1 度だけ数える。途中の予約は捨てる（最後の値だけを反映する）。
 */
export class Debounced<T> {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private pending: T;
  private readonly notify: (value: T) => void;
  private readonly wait: number;

  constructor(notify: (value: T) => void, initial: T, wait = 200) {
    this.notify = notify;
    this.pending = initial;
    this.wait = wait;
  }

  /** いまの値（予約中の値を含む）。 */
  get value(): T {
    return this.pending;
  }

  /** 値を予約する（前の予約は取り消す）。 */
  set(next: T, wait = this.wait): void {
    this.pending = next;
    this.cancel();
    this.timer = setTimeout(() => {
      this.timer = null;
      this.notify(this.pending);
    }, wait);
  }

  /** 予約を取り消す（部品が消えるとき）。 */
  cancel(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }
}
