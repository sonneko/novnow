// 句切れ提案レイヤー（§9.4.3 の算法。決定論・形態素解析なし）。
//
// 仕様（§9.4.3）は次の 5 つを決めている:
//
// 1. 既定は**オフ**（`ui.json` の `suggest_comma: false`）。`Ctrl+Alt+,` でトグル。
// 2. 発火条件は「1 文（`。！？` までの区切り）に読点が 0 個、かつ
//    `writing.long_sentence_chars`（既定 120 字）超」。
// 3. 候補は優先度つきの手がかりで決め、**上位 3 件**だけを出す。
// 4. 除外は 4 つ（台詞の中・`、` の前後 6 字以内・行末から 4 字以内・`'参照` と `｜ルビ《…》` の途中）。
// 5. 表示は 1px × 高さ 10px の縦線（`fg.faint` の 40%）。**文字は挿入しない**。
//    `Alt+,` を押したときだけ `、` を入れる。
//
// 第 1 の画面実装（`REF:src/core/features/metrics.rs` の `comma_candidates`）と同じ規則・
// 同じ定数を使う。違うのは、あちらが 4 つの優先度のどれかを 1 つだけ採るのに対し、こちらは
// **優先度の低い手がかりも候補として残す**点（上位 3 件を選ぶための材料を減らさない）。
//
// `webui/tests/write.test.mjs` がそのまま読めるよう、画面に依存させない。

/** 候補の手がかりの優先度（1 が最も強い。§9.4.3 の表）。 */
export type Cue = 1 | 2 | 3 | 4;

/** 1 件の候補。 */
export interface CommaCandidate {
  /** 挿入位置（**UTF-16**。この位置の**前**に `、` を入れる）。 */
  at: number;
  /** 手がかりの優先度。 */
  cue: Cue;
}

/** 候補の上限（§9.4.3「上位 3 件だけ」）。 */
export const COMMA_TOP = 3;
/** 既に `、` がある位置の前後、この字数以内は候補にしない（§9.4.3）。 */
export const COMMA_GAP = 6;
/** 行末からこの字数以内は候補にしない（§9.4.3）。 */
export const LINE_END_GAP = 4;
/** 主題の助詞を見る範囲（文頭から何字まで。§9.4.3 の優先度 3）。 */
export const TOPIC_WINDOW = 10;
/** 列挙と見なす回数（§9.4.3 の優先度 4「3 回以上」）。 */
export const ENUM_COUNT = 3;
/** 既定の `writing.long_sentence_chars`（§9.4.3）。 */
export const LONG_SENTENCE_CHARS = 120;

/**
 * 優先度 1: 接続助詞（§9.4.3 の表）。**長いものから**照合する（`けれども` を `けれど` で切らない）。
 * i18n: format-vocabulary
 */
export const CONNECTIVES = [
  'けれども',
  'けれど',
  'ので',
  'のに',
  'から',
  'ても',
  'たら',
  'なら',
  'ば',
  'が',
  'と',
  'て',
] as const;

/**
 * 優先度 2: 読点の直前に来やすい副詞句（§9.4.3 の表）。
 * i18n: format-vocabulary
 */
export const ADVERBIALS = ['ときに', 'ために', 'ように', 'ながら', 'つつ'] as const;

/** 優先度 3: 文頭から [`TOPIC_WINDOW`] 字以内の主題の助詞（§9.4.3 の表）。 */
export const TOPIC_PARTICLES = ['は', 'も'] as const;

/** 文末記号（§7.2.3 と同じ。`…` は含めない）。 */
const STOPS = /[。！？]/u;

/**
 * 節の本文から候補を返す（§9.4.3）。**文字は挿入しない**。
 *
 * 返すのは `at`（UTF-16 の位置）の昇順。同じ位置に複数の手がかりが当たったら、
 * 優先度の高いほうを採る。
 */
export function commaCandidates(body: string, longSentenceChars = LONG_SENTENCE_CHARS): CommaCandidate[] {
  const out: CommaCandidate[] = [];
  for (const sentence of sentences(body)) {
    if (sentence.text.includes('、')) {
      continue;
    }
    const chars = [...sentence.text].filter((found) => !/\s/u.test(found)).length;
    if (chars <= longSentenceChars) {
      continue;
    }
    // **文ごとに**上位 3 件まで（第 1 の実装 `REF:src/core/features/metrics.rs` と同じ。
    // 画面全体で 3 件にすると、長い文が 2 つ以上ある節で後の文の候補が消える）。
    out.push(...topOf(candidatesOf(body, sentence)));
  }
  // 同じ位置は優先度の高いほうだけを残し、位置の昇順で返す。
  const best = new Map<number, Cue>();
  for (const found of out) {
    const known = best.get(found.at);
    if (known === undefined || found.cue < known) {
      best.set(found.at, found.cue);
    }
  }
  return [...best.entries()]
    .map(([at, cue]) => ({ at, cue }))
    .sort((a, b) => a.at - b.at);
}

/** 手がかりの優先度 → 位置の順に上位 [`COMMA_TOP`] 件（第 1 の実装と同じ並べ方）。 */
function topOf(found: readonly CommaCandidate[]): CommaCandidate[] {
  return [...found].sort((a, b) => a.cue - b.cue || a.at - b.at).slice(0, COMMA_TOP);
}

/** 文（`。！？` を含む。`……` では切らない）。 */
export function sentences(body: string): { start: number; text: string }[] {
  const out: { start: number; text: string }[] = [];
  let start = 0;
  for (let at = 0; at < body.length; at += 1) {
    const found = body[at] ?? '';
    if (STOPS.test(found)) {
      out.push({ start, text: body.slice(start, at + 1) });
      start = at + 1;
    }
  }
  if (start < body.length) {
    out.push({ start, text: body.slice(start) });
  }
  return out.filter((found) => found.text.length > 0);
}

/** 1 文の中の候補（§9.4.3 の 4 つの手がかりと 4 つの除外）。 */
function candidatesOf(
  body: string,
  sentence: { start: number; text: string },
): CommaCandidate[] {
  const out: CommaCandidate[] = [];
  const dialogue = insideDialogue(body, sentence);
  const commas = commaPositions(body);
  const to = [...sentence.text].filter((found) => found === 'と').length;
  const ya = [...sentence.text].filter((found) => found === 'や').length;
  for (let at = 0; at < sentence.text.length; at += 1) {
    const found = sentence.text[at] ?? '';
    if (found === '\n' || '「」『』'.includes(found)) {
      continue;
    }
    const absolute = sentence.start + at;
    if (dialogue(absolute)) {
      continue;
    }
    const hint = cueAfter(sentence.text, at, found, to, ya);
    if (hint === null) {
      continue;
    }
    // 挿入する位置は**手がかりの直後**（`が` の後ろ、`けれども` の後ろ。§9.4.3 の表）
    const after = absolute + hint.length;
    // 行末から 4 字以内は候補にしない
    const lineEnd = body.indexOf('\n', after);
    const tail = body.slice(after, lineEnd < 0 ? body.length : lineEnd);
    if ([...tail].length <= LINE_END_GAP) {
      continue;
    }
    // 既存の `、` の前後 6 字以内は候補にしない
    const charsBefore = [...body.slice(0, after)].length;
    if (commas.some((comma) => Math.abs(comma - charsBefore) <= COMMA_GAP)) {
      continue;
    }
    // `'参照` と `｜ルビ《…》` の途中は候補にしない
    const prefix = body.slice(body.lastIndexOf('\n', after - 1) + 1, after);
    if (insideReference(prefix) || insideRuby(prefix)) {
      continue;
    }
    out.push({ at: after, cue: hint.cue });
  }
  return out;
}

/**
 * その位置（UTF-16）から始まる手がかり（§9.4.3 の表）。無ければ `null`。
 *
 * 返す `length` は手がかりの長さで、`、` はその**直後**に入れる（`けれども` を `けれど` で
 * 切らないよう、長いものから照合する）。
 *
 * 選び方は第 1 の実装（`REF:src/core/features/metrics.rs`）と同じ:
 * **優先度 1 を先に見て、優先度 2 は「より長く一致するとき」だけ採る。**
 * 例えば `ときに` は接続助詞 `と`（優先度 1）にも副詞句 `ときに`（優先度 2）にも当たるが、
 * 第 1 の実装は長い `ときに` を採る（`と` だけを切り取らない）。主題・列挙の助詞は
 * どちらも当たらなかったときだけ採る。
 */
function cueAfter(
  sentence: string,
  at: number,
  found: string,
  to: number,
  ya: number,
): { cue: Cue; length: number } | null {
  const rest = sentence.slice(at);
  let best: { cue: Cue; length: number } | null = null;
  for (const pattern of CONNECTIVES) {
    if (rest.startsWith(pattern) && (best === null || pattern.length > best.length)) {
      best = { cue: 1, length: pattern.length };
    }
  }
  // 優先度 2 は、優先度 1 より長く一致するときだけ採る（第 1 の実装と同じ）
  for (const pattern of ADVERBIALS) {
    if (rest.startsWith(pattern) && (best === null || pattern.length > best.length)) {
      best = { cue: 2, length: pattern.length };
    }
  }
  if (best !== null) {
    return best;
  }
  // 優先度 3: 文頭から 10 字以内の主題の助詞
  const before = sentence.slice(0, at);
  if (TOPIC_PARTICLES.includes(found as (typeof TOPIC_PARTICLES)[number])) {
    if ([...before].length < TOPIC_WINDOW) {
      return { cue: 3, length: found.length };
    }
  }
  // 優先度 4: 列挙の助詞（`と` / `や` が 3 回以上）
  if (found === 'と' && to >= ENUM_COUNT) {
    return { cue: 4, length: found.length };
  }
  if (found === 'や' && ya >= ENUM_COUNT) {
    return { cue: 4, length: found.length };
  }
  return null;
}

/** 文の各位置がまだ閉じていない `「『` の内側か（[`insideDialogue`] の判定関数を返す）。 */
function insideDialogue(body: string, sentence: { start: number; text: string }): (at: number) => boolean {
  // 文より前から開いている括弧を数える（文をまたいで開いた台詞も内側とみなす）
  const stack: string[] = [];
  for (let at = 0; at < sentence.start; at += 1) {
    const found = body[at] ?? '';
    if (found === '「') {
      stack.push('」');
    } else if (found === '『') {
      stack.push('』');
    } else if (found === stack[stack.length - 1]) {
      stack.pop();
    }
  }
  const open = new Set<number>();
  let depth = stack.length;
  for (let at = 0; at < sentence.text.length; at += 1) {
    const found = sentence.text[at] ?? '';
    if (depth > 0) {
      open.add(sentence.start + at);
    }
    if (found === '「') {
      stack.push('」');
      depth += 1;
    } else if (found === '『') {
      stack.push('』');
      depth += 1;
    } else if (found === stack[stack.length - 1]) {
      stack.pop();
      depth -= 1;
      // 閉じ括弧そのものは内側ではない（§9.4.3 は「台詞の中」を除外する）
      open.delete(sentence.start + at);
    }
  }
  return (at: number) => open.has(at);
}

/** `、` の位置（本文の先頭からの**文字数**。§9.4.3 の 6 字判定に使う）。 */
function commaPositions(body: string): number[] {
  const out: number[] = [];
  let seen = 0;
  for (const found of body) {
    if (found === '、') {
      out.push(seen);
    }
    seen += 1;
  }
  return out;
}

/** `prefix` の末尾が `'参照`（または `'{…}`）の途中か（§9.4.3 の除外）。 */
export function insideReference(prefix: string): boolean {
  const at = prefix.lastIndexOf("'");
  if (at < 0) {
    return false;
  }
  let slashes = 0;
  for (let back = at - 1; back >= 0 && prefix[back] === '\\'; back -= 1) {
    slashes += 1;
  }
  if (slashes % 2 === 1) {
    return false;
  }
  const rest = prefix.slice(at + 1);
  if (rest.startsWith('{')) {
    return !rest.includes('}');
  }
  return rest.length > 0 && !/[\s、。「」『』]/u.test(rest);
}

/** `prefix` の末尾がルビの読み（`《…》` の内側）か（§9.4.3 の除外）。 */
export function insideRuby(prefix: string): boolean {
  const open = prefix.lastIndexOf('《');
  const close = prefix.lastIndexOf('》');
  return open > close;
}

/**
 * カーソル以降の直近の候補（`Alt+,` で `、` を入れる位置。§9.4.3）。
 *
 * 候補が無ければ `null`（呼び出し側は何もしない。**文字を勝手に入れない**）。
 */
export function nextCandidate(
  candidates: readonly CommaCandidate[],
  caret: number,
): CommaCandidate | null {
  return candidates.find((found) => found.at >= caret) ?? null;
}

/**
 * 候補の間を動く（`Alt+→` / `Alt+←`。§9.4.3）。
 *
 * その方向に候補が無ければ `null` を返す（呼び出し側は §9.8.2 の場面移動へ回す）。
 */
export function stepCandidate(
  candidates: readonly CommaCandidate[],
  caret: number,
  forward: boolean,
): CommaCandidate | null {
  if (forward) {
    return candidates.find((found) => found.at > caret) ?? null;
  }
  const back = candidates.filter((found) => found.at < caret);
  return back[back.length - 1] ?? null;
}

/**
 * `、` を入れる（`at` の**前**に 1 文字）。返すのは挿入後の本文とキャレット（`、` の後ろ）。
 */
export function insertComma(
  body: string,
  at: number,
): { text: string; caret: number } {
  const before = body.slice(0, at);
  const after = body.slice(at);
  return { text: `${before}、${after}`, caret: at + 1 };
}
