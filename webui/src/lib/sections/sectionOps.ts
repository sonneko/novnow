// 節の操作と衝突の解決（§8.1.1・§8.5・§9.6・§10.3.3）の規則。**画面を持たない純粋な関数だけ**。
//
// ここが持つのは「キーや押した位置を、どの正規操作へ、どんな引数で渡すか」の判断である。
// 実際に `run` を呼ぶのは `App.svelte`（`node --test` でそのまま検証できる）。
//
// ## 5 つの操作の意味（機能核の実装に合わせる。ここで意味を作らない）
//
// | 操作 | 意味（機能核） | 画面の口 |
// |---|---|---|
// | `section.split` | カーソルの**バイト位置**で節を 2 つに分ける（`src/app/workbench.rs` の `split_section`） | `Ctrl+Enter`（§9.8.3） |
// | `section.propose` | 本文を変えず `agents/proposals/` に提案を 1 件置く（§8.7.1） | 節エディタの提案ボタン |
// | `plot.set_state` | 話プロットの節の `state:` を切り替える（§10.3.3） | 切替パネルのプロット行 |
// | `save.keep_mine` | 衝突したファイルで自分の版を残す（§8.5） | 衝突の面（`save === 'conflict'`） |
// | `save.reload_external` | 衝突したファイルで外部の版を読み込む（§8.5） | 衝突の面 |
//
// ### 位置の単位を取り違えない
//
// `section.split` の `at` は**バイト位置**で、節エディタのキャレット（UTF-16 の添字）とは
// 単位が違う（日本語 1 文字 = UTF-8 で 3 バイト）。ここを混ぜると節が別の場所で切れるので、
// `byteOffset` で明示的に変換する。`at` は**節の本文の中の**位置である（文書の先頭ではない）。

/** 節 1 つ（`section.list` の要素。`store/session.ts` の `SectionRef` と同じ形）。 */
export interface SectionRef {
  /** `01-04-02-03` の 4 階層 ID。 */
  id: string;
  /** 文書のパス。 */
  path: string;
  /** 文書内の行範囲（1 始まり）。 */
  lines: [number, number];
  /** 見出し行。 */
  header: string;
  /** 場面の題名。 */
  scene_title: string;
}

/**
 * 文字列の添字（UTF-16）を UTF-8 のバイト位置へ直す。
 *
 * `section.split` の `at` はバイト位置なので、キャレットをそのまま渡すと和文で 3 倍ずれる。
 * 範囲外は端へ丸める（機能核も `at.min(buffer.len())` で丸めるが、画面でも同じ規則にする）。
 */
export function byteOffset(text: string, index: number): number {
  const at = Math.min(Math.max(index, 0), text.length);
  return new TextEncoder().encode(text.slice(0, at)).length;
}

/**
 * `Ctrl+Enter` の引数を組み立てる（§8.1.1・§9.8.3）。
 *
 * 仕様: 「`Ctrl+Enter` はカーソルのバイト位置で節を 2 つに分割する。節の末尾では『確定して
 * 次の節へ』と同じ結果になり、次が無ければ新規節を作る」。その分岐は**機能核が持つ**
 * （`split_section` が `at >= buffer.len()` で `confirm_next` へ回す）ので、画面は
 * 「いまの節・いまの位置」をそのまま渡すだけでよい（ここで分岐を写さない。§19.1.4）。
 */
export function splitArgs(scene: string, text: string, caret: number): Record<string, unknown> {
  return { at: byteOffset(text, caret), role: '', scene };
}

/** 提案の入力（`section.propose` は `scene` / `text` / `note` を取る）。 */
export interface ProposalInput {
  /** 対象の節（空なら断る）。 */
  scene: string;
  /** 提案する本文（空なら断る）。 */
  text: string;
  /** 添える覚え書き（任意）。 */
  note: string;
}

/** 提案の判定。 */
export type ProposalPlan =
  | { ok: true; args: Record<string, unknown> }
  | { ok: false; reason: 'no-scene' | 'empty' };

/**
 * 提案の入力を確かめて引数にする（§8.7.1。**本文は変えない**）。
 *
 * 空の提案は機能核も断る（`text` が無ければ `Argument`）ので、画面で先に同じことを言う
 * （押しても何も起きない送信ボタンを置かない。§21.1.2）。`scene` は必須。
 */
export function proposalArgs(input: ProposalInput): ProposalPlan {
  const scene = input.scene.trim();
  if (scene === '') {
    return { ok: false, reason: 'no-scene' };
  }
  if (input.text.trim() === '') {
    return { ok: false, reason: 'empty' };
  }
  const args: Record<string, unknown> = { scene, text: input.text };
  const note = input.note.trim();
  if (note !== '') {
    args.note = note;
  }
  return { ok: true, args };
}

/** プロットの `state:` の値（機能核は `fixed` / `draft` の 2 つ）。 */
export type PlotState = 'fixed' | 'draft';

/**
 * `plot.set_state` の引数（§10.3.3）。
 *
 * 機能核は `bool` の `state` を取り、`true` → `fixed`（確認済み）、`false` → `draft` にする。
 * 画面は「いまの状態を反転する」ので、**いまの値から次の真偽**を決める（同じ値を送って
 * 何も起きないボタンにしない）。
 */
export function setStateArgs(scene: string, next: PlotState): Record<string, unknown> {
  return { role: '', scene, state: next === 'fixed' };
}

/** `state:` を反転する。 */
export function toggleState(current: PlotState | string): PlotState {
  return current === 'fixed' ? 'draft' : 'fixed';
}

/**
 * 節の見出し行から `state:` を読む（話プロットの節ブロック。§10.3.3）。
 *
 * 見出しの形は `@01-04-02 題名` のあとに `state: fixed` などの行が続く。見つからなければ
 * `draft`（機能核の既定と同じ）。
 */
export function stateOf(header: string): PlotState {
  const found = /(?:^|\n)\s*state:\s*([a-z]+)/.exec(header);
  return found?.[1] === 'fixed' ? 'fixed' : 'draft';
}

/**
 * 衝突の一覧を作る（`save.state` の `conflicts`）。
 *
 * `conflicts` の形は**パスの配列**（`REF:src/app/ops/editing.rs` の `conflict_paths` が
 * `save.state` の値をそのまま読む）。形が違えば空にする（画面で推測しない）。
 */
export function conflictPaths(found: Record<string, unknown> | null): string[] {
  const list = found?.conflicts;
  if (!Array.isArray(list)) {
    return [];
  }
  return list.filter((item): item is string => typeof item === 'string' && item !== '');
}

/** 衝突の 1 件（`save.diff_external` の結果。§8.5 の差分モーダル）。 */
export interface ConflictDiff {
  /** 対象のパス。 */
  path: string;
  /** 自分の版の文字数。 */
  mine_chars: number;
  /** 外部の版の文字数。 */
  external_chars: number;
  /** 行ごとの差分（`{ line, side, text }`）。 */
  lines: { line: number; side: 'mine' | 'external'; text: string }[];
}

/**
 * `save.diff_external` の戻り値を画面の形へ直す（形が違う行は落とす）。
 *
 * 差分は**読むためだけ**に使う。どちらを残すかは利用者が `save.keep_mine` /
 * `save.reload_external` で選ぶ（画面が勝手に決めない。§8.5）。
 */
export function conflictDiffs(found: Record<string, unknown> | null): ConflictDiff[] {
  const list = found?.diffs;
  if (!Array.isArray(list)) {
    return [];
  }
  const out: ConflictDiff[] = [];
  for (const item of list) {
    const row = item as Partial<ConflictDiff>;
    if (typeof row?.path !== 'string' || row.path === '') {
      continue;
    }
    out.push({
      path: row.path,
      mine_chars: typeof row.mine_chars === 'number' ? row.mine_chars : 0,
      external_chars: typeof row.external_chars === 'number' ? row.external_chars : 0,
      lines: Array.isArray(row.lines)
        ? row.lines.filter(
            (line) =>
              typeof line === 'object' &&
              line !== null &&
              typeof (line as { line?: unknown }).line === 'number' &&
              ((line as { side?: unknown }).side === 'mine' ||
                (line as { side?: unknown }).side === 'external') &&
              typeof (line as { text?: unknown }).text === 'string',
          )
        : [],
    });
  }
  return out;
}

/**
 * 衝突の解決を頼めるか（`save.keep_mine` / `save.reload_external` の `path`）。
 *
 * 機能核は `path` を省くと**すべての衝突**を解決し、指定するとその 1 件だけを解決する
 * （`conflict_paths`）。画面は 1 件ずつ選ばせる（まとめて消すと、どれを残したか分からなくなる）。
 */
export function resolveArgs(path: string): Record<string, unknown> {
  return path === '' ? {} : { path };
}

/**
 * 「`.novnow/` を読み直す」で衝突が消えるか（§8.5・§9.6）。
 *
 * 外部変更の監視はネイティブだけが行う（§13.3）。ブラウザでは衝突イベントが来ないので、
 * この面は**ネイティブでだけ**出る。押せないボタンを置かず、理由を書く（§21.1.2）。
 */
export function conflictSurface(locale: { native: boolean; conflicts: number }): 'hidden' | 'shown' | 'unsupported' {
  if (locale.conflicts > 0) {
    return 'shown';
  }
  return locale.native ? 'hidden' : 'unsupported';
}
