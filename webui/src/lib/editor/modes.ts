// 節エディタのミニヘッダーに出す**有効モード**の組み立て（§9.4.2）。
//
// 仕様の形は `'リリア B+C`。出すのは「その節の節本文に `@名前:モード` で書かれた有効モード」
// だけで、モードの無いキャラ（`@リリア`）は出さない（出すと `'リリア` と空のモードが並び、
// 何を伝えているのか分からなくなる）。
//
// 状態は機能核ではなく**参照ペイン**（`reference.pane` の `cast`。有効モードの情報だけを持つ。
// `REF:src/core/features/reference.rs` の `Cast::modes`）から取る。画面が独自に
// キャラクター定義を解釈しない（§21.1.3）。節の本文に `@名前` が出ていないキャラは
// 「この節のモード」ではないので出さない（`cast` は場面のキャラ一覧なので、それだけでは多い）。
//
// `webui/tests/write.test.mjs` がそのまま読めるよう、画面に依存させない。

/** ミニヘッダーに出す 1 人分（`name` と `B+C`）。 */
export interface ActiveMode {
  /** キャラ名。 */
  name: string;
  /** モード ID を `+` でつないだもの（`B+C`）。 */
  modes: string;
}

/**
 * 節本文に現れる `@名前[:モード式]` を、キャラ名 → モード ID の並びに集める。
 *
 * 行末の話者注釈だけを見る（行の途中の `@` は W002 の印なので拾わない）。モード式が
 * `mode_id[\+mode_id]` の形でなければ「モード無し」として扱う（`REF:src/core/syntax/contents.rs`
 * の `split_speaker` と同じ判定）。
 */
export function speakersOf(body: string): { name: string; modes: string[] }[] {
  const out: { name: string; modes: string[] }[] = [];
  for (const line of body.split('\n')) {
    const found = trailingSpeaker(line);
    if (!found) {
      continue;
    }
    const known = out.find((row) => row.name === found.name);
    if (known) {
      for (const mode of found.modes) {
        if (!known.modes.includes(mode)) {
          known.modes.push(mode);
        }
      }
    } else {
      out.push({ name: found.name, modes: [...found.modes] });
    }
  }
  return out;
}

/**
 * 本文と参照ペインの `cast` から、ミニヘッダーに出す有効モードを組み立てる（§9.4.2）。
 *
 * 出す順は**本文に現れた順**（`@` の順）。同じキャラが複数のモードで出たら `+` でつなぐ。
 */
export function activeModes(
  body: string,
  cast: { name: string; modes: string[] }[],
): ActiveMode[] {
  const fromCast = new Map<string, string[]>();
  for (const row of cast) {
    fromCast.set(row.name, row.modes);
  }
  const out: ActiveMode[] = [];
  for (const speaker of speakersOf(body)) {
    const modes = speaker.modes.length > 0 ? speaker.modes : (fromCast.get(speaker.name) ?? []);
    if (modes.length === 0) {
      continue;
    }
    // 表示名は `### A 平常` の ID だけ（`A`）。`REF:src/ui/status.rs` の `mini_header` と同じ。
    const ids = modes.map((found) => found.split(' ')[0] ?? '').filter(Boolean);
    if (ids.length === 0) {
      continue;
    }
    out.push({ name: speaker.name, modes: ids.join('+') });
  }
  return out;
}

/** ミニヘッダーの 1 行（`'リリア B+C 'アルス A`。§9.4.2）。 */
export function modesLabel(found: readonly ActiveMode[]): string {
  return found.map((row) => `'${row.name} ${row.modes}`).join(' ');
}

/**
 * 行末の話者注釈（`@名前[:モード式]`）を読む（§5.2.1）。
 *
 * **最後のエスケープされていない `@`** だけを見る（`REF:src/core/syntax/contents.rs` の
 * `trailing_speaker`）。空白を含む名前は注釈にならない。
 */
export function trailingSpeaker(line: string): { name: string; modes: string[] } | null {
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
    return null;
  }
  const tail = line.slice(last + 1).trimEnd();
  if (tail.length === 0 || /\s/u.test(tail)) {
    return null;
  }
  const cut = tail.indexOf(':');
  const name = cut < 0 ? tail : tail.slice(0, cut);
  if (name.length === 0) {
    return null;
  }
  if (cut < 0) {
    return { name, modes: [] };
  }
  const modes = splitModes(tail.slice(cut + 1));
  return modes === null ? { name, modes: [] } : { name, modes };
}

/**
 * `mode_expr = mode_id , [ "+" , mode_id ]`（§5.1）。形に合わなければ `null`。
 * 3 つ以上は話者注釈にしない（`REF:src/core/syntax/contents.rs` の `split_modes`）。
 */
function splitModes(expr: string): string[] | null {
  const modes = expr.split('+');
  if (modes.length === 0 || modes.length > 2) {
    return null;
  }
  return modes.every((found) => /^[A-Za-z][A-Za-z0-9_]{0,15}$/u.test(found)) ? modes : null;
}
