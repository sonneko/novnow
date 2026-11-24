// 文脈ビューの折りたたみ（§9.3.3）。
//
// 単位は **章 → 話 → 場面**（SceneId の先頭 1・2・3 階層）。状態は `.novnow/ui.json` の
// `folded.context_view` に `@01` / `@01-04` / `@01-04-02` の形で並べる（§4.4.1）。
//
// 仕様の MUST は 2 つ:
//
// 1. **現在の節を含む単位は畳めない**。畳もうとしたら**その 1 つ上**を畳む（§9.3.3）。
//    段階は 節 → 場面 → 話 → 章 で、**畳めるのは「現在の節を含まない」単位だけ**。
// 2. 畳んだ単位は **1 行**になる。出すのは「見出し ▸ タイトル ▸ 文字数 ▸ 節数」の 4 要素だけ
//    （`summary` は出さない）。
//
// キーボード（`Ctrl+Shift+[` / `]` / `Ctrl+K Ctrl+0` / `Ctrl+K Ctrl+J`）は画面が持ち、
// ここは「どの鍵を畳むか」を決める純粋な部分だけを持つ。
//
// 鍵の選び方は仕様の「カーソル位置の 1 階層上を畳む」に合わせる。畳む段は**現在の節の祖先
// （場面 → 話 → 章）**で、**現在の節を含む単位は畳めない**（MUST）。祖先はどれも現在の節を
// 含むので、実際に畳める段は無い（1 章 1 場面の作品では `Ctrl+Shift+[` は何もしない）。
// これは仕様の MUST を守った結果であり、黙って現在の節を隠さない（§21.1.2 の精神）。
//
// `webui/tests/write.test.mjs` がそのまま読めるよう、画面に依存させない。

/** 折りたたみの単位（1 = 章、2 = 話、3 = 場面）。 */
export type FoldDepth = 1 | 2 | 3;

/** 畳んだ鍵の形（`@01-04`。§4.4.1）。 */
export const FOLD_PREFIX = '@';

/** 文脈ビューの 1 行（前後の節・畳んだ単位の両方を表せる）。 */
export interface ContextRow {
  /** `scene`（`01-04-02-03`）か、畳んだ単位の鍵（`01-04`）。 */
  id: string;
  /** 畳んだ単位か。 */
  folded?: boolean;
  /** 畳んだ単位の題（見出しの `title`。無ければ空）。 */
  title?: string;
  /** 畳んだ単位の文字数（空白を除く）。 */
  chars?: number;
  /** 畳んだ単位の節数。 */
  sections?: number;
  /** 本文（節のときだけ）。 */
  text?: string;
}

/** SceneId の階層（`01-04-02-03` → `['01','04','02','03']`）。 */
export function levels(id: string): string[] {
  return id.split('-').filter((found) => found.length > 0);
}

/** 単位の鍵（`01-04` の 2 階層なら `@01-04`）。 */
export function keyOf(id: string, depth: FoldDepth): string {
  return `${FOLD_PREFIX}${levels(id).slice(0, depth).join('-')}`;
}

/** その鍵が指す階層（`@01-04-02` → 3）。構造 ID の形でなければ `0`。 */
export function depthOf(key: string): FoldDepth | 0 {
  const parts = levels(key.replace(/^@/u, ''));
  if (!parts.every((found) => /^\d+$/u.test(found))) {
    return 0;
  }
  return parts.length === 1 || parts.length === 2 || parts.length === 3 ? (parts.length as FoldDepth) : 0;
}

/**
 * 現在の節を含む単位か（§9.3.3「現在の節を含む単位は畳めない」）。
 *
 * `key` が現在の節の**接頭辞**なら含む（`@01-04` は `01-04-02-03` を含む）。
 */
export function containsScene(key: string, scene: string): boolean {
  const prefix = key.replace(/^@/u, '');
  return prefix.length > 0 && (scene === prefix || scene.startsWith(`${prefix}-`));
}

/**
 * `Ctrl+Shift+[` で畳む鍵を決める（§9.3.3）。
 *
 * 畳む単位は `target`（**ポインタが載っている行の節**。載っていなければ現在の節）の祖先を
 * **場面 → 話 → 章**の順に見て、**現在の節（`current`）を含まない**一番深い単位。
 * 第 1 の画面実装（`REF:src/ui/widgets/context_view.rs`・`REF:src/ui/app.rs`）は
 * ポインタの載っている単位を畳むので、その写しにしてある。
 *
 * **現在の節を含む単位は畳めない**（§9.3.3 の MUST）。`target` が現在の節なら、祖先はどれも
 * 現在の節を含むので `null` を返す（1 章 1 場面の作品で `Ctrl+Shift+[` が何もしないのは、
 * この MUST を守った結果）。
 *
 * 既に畳んである段は飛ばす（もう一度押すと次の段が畳まれる）。
 */
export function foldKey(target: string, current: string, folded: readonly string[]): string | null {
  const parts = levels(target);
  if (parts.length < 1) {
    return null;
  }
  for (let depth = Math.min(3, parts.length); depth >= 1; depth -= 1) {
    const key = keyOf(target, depth as FoldDepth);
    // 現在の節を含む単位は畳めない（§9.3.3 の MUST）。1 つ上へ。
    if (containsScene(key, current)) {
      continue;
    }
    if (folded.includes(key)) {
      continue;
    }
    return key;
  }
  return null;
}

/**
 * `Ctrl+Shift+]` で展開する鍵を決める（§9.3.3）。
 *
 * `target`（ポインタの載っている節。載っていなければ現在の節）の祖先のうち、
 * **一番内側の畳んである単位**を返す。畳んである単位が 1 つも無ければ `null`。
 */
export function unfoldKey(target: string, folded: readonly string[]): string | null {
  const parts = levels(target);
  for (let depth = 1; depth <= Math.min(3, parts.length); depth += 1) {
    const key = keyOf(target, depth as FoldDepth);
    if (folded.includes(key)) {
      return key;
    }
  }
  return null;
}

/** すべて畳む鍵（章だけが見える状態。`Ctrl+K Ctrl+0`。§9.3.3）。 */
export function foldAll(scenes: readonly string[]): string[] {
  const out: string[] = [];
  for (const scene of scenes) {
    const key = keyOf(scene, 1);
    if (!out.includes(key)) {
      out.push(key);
    }
  }
  return out;
}

/** その単位が畳まれているか。 */
export function isFolded(id: string, folded: readonly string[]): boolean {
  return folded.some((key) => containsScene(key, id) || `@${id}` === key);
}

/**
 * 並べる行を作る（前 → いま → 後）。**畳んだ単位は 1 行**にまとめる（§9.3.3）。
 *
 * 現在の節（`here`）は本文を持たない（§9.3.2 MUST: 編集中の節の本文は描かない）。
 */
export function rowsAround(
  before: readonly { scene: string; text: string }[],
  here: { scene: string; chars: number },
  after: readonly { scene: string; text: string }[],
  folded: readonly string[],
): ContextRow[] {
  const rows: ContextRow[] = [];
  const seen = new Set<string>();
  const push = (row: ContextRow): void => {
    // 同じ畳んだ単位は 1 行だけ（前後で同じ章に属する節が続くときに重複させない）
    const key = row.folded ? `@${row.id}` : row.id;
    if (seen.has(key)) {
      return;
    }
    seen.add(key);
    rows.push(row);
  };
  const emit = (scene: string, text: string): void => {
    // 現在の節を含む単位は畳めない（畳んだ鍵に含まれていても本文を出す）
    const cover = folded.find((key) => containsScene(key, scene) && !containsScene(key, here.scene));
    if (cover) {
      // 畳んだ単位の 1 行。文字数と節数は、同じ単位に属する前後の節から数える
      const inside = [...before, ...after].filter((row) => containsScene(cover, row.scene));
      push({
        id: cover.replace(/^@/u, ''),
        folded: true,
        title: '',
        chars: inside.reduce((sum, row) => sum + countChars(row.text), 0),
        sections: inside.length,
      });
      return;
    }
    push({ id: scene, text, folded: false });
  };
  for (const row of before) {
    emit(row.scene, row.text);
  }
  push({ id: here.scene, text: '', folded: false });
  for (const row of after) {
    emit(row.scene, row.text);
  }
  return rows;
}

/** 文字数（空白を除く。§9.6.1）。 */
export function countChars(text: string): number {
  let count = 0;
  for (const found of text) {
    if (!/\s/u.test(found)) {
      count += 1;
    }
  }
  return count;
}
