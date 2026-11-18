// ナビゲーション（`§9.7`・`§9.8.2`・`§10.0`）の規則。**画面を持たない純粋な関数だけ**を置く。
//
// ここが持つのは「どのキーが、どの正規操作へ、どんな引数で届くか」の判断である。実際に
// `run` を呼ぶのは `App.svelte` で、このファイルは DOM も時計も持たない（`node --test` で
// そのまま検証できる）。
//
// ## 6 つの操作の意味（機能核の実装に合わせる。ここで意味を作らない）
//
// | 操作 | 意味（機能核） | 画面の口 |
// |---|---|---|
// | `nav.prev` / `nav.next` | **移動履歴**を 1 つ戻る / 進む（`Ctrl+[` / `Ctrl+]`） | `Ctrl+[` / `Ctrl+]`・切替パネルの行 |
// | `nav.back` / `nav.forward` | 親ディレクトリへ / 子の先頭へ | 切替パネルの行（キーは表の `Ctrl+H` = 置換と衝突するため画面で取らない） |
// | `nav.switcher` | 切替パネルの一覧（9 ワークスペース + 移動の口。§10.0） | `Ctrl+K`（`ui:switcher`） |
// | `document.rename` | 文書のリネーム（`>` + `rename-document`） | パレットの `>` 行・切替パネルの文書行 |
//
// **`nav.prev` / `nav.next` を構造移動（前後の節）と書かない。** 仕様 §9.8.2 の「前 / 次の節」は
// コマンド名 `previous-section` / `next-section`（`Alt+↑` / `Alt+↓`）が担い、機能核では
// `Command::Nav`（`Workbench::go`）へ届く。カタログの `nav.prev` / `nav.next` は
// `src/app/ops/editing.rs` の `step_history` ＝ **移動履歴の前後**である
// （`docs/specifications/interfaces.md` も「前の位置・次の位置」と書く）。取り違えると
// キーの意味が変わるので、`webui/tests/nav.test.mjs` が機能核のソースを読んで突き合わせる。

/** 画面が使う文脈（`App.svelte` が持つ状態のうち、判断に要るものだけ）。 */
export interface NavContext {
  /** いまの節（`01-04-02-03`。空なら未選択）。 */
  scene: string;
  /** いまのワークスペース名（`write` など）。 */
  workspace: string;
}

/** 切替パネルの 1 行。 */
export interface SwitcherRow {
  /** 行の種類。`workspace` は `Alt+<number>` と同じ切替、`move` は移動の操作。 */
  kind: 'workspace' | 'move' | 'document';
  /** 実行する正規操作の ID（`session.workspace_switch` など）。 */
  op: string;
  /** `op` に渡す引数。 */
  args: Record<string, unknown>;
  /** 画面に出す文言キー（`i18n/catalog.ts` に実在するものだけ）。 */
  label: string;
  /** 数字キーの番号（`workspace` の行だけ。無ければ `null`）。 */
  number: number | null;
  /** いま居る場所か。 */
  current: boolean;
  /** `previous`（`Alt+0` の戻り先）の行か。 */
  previous: boolean;
}

/** 切替パネルに必ず出す移動の行（順序も固定。§9.8.2 のキーと同じ並び）。 */
const MOVES: readonly { op: string; label: string }[] = [
  { op: 'nav.prev', label: 'api.op.nav.prev' },
  { op: 'nav.next', label: 'api.op.nav.next' },
  { op: 'nav.back', label: 'api.op.nav.back' },
  { op: 'nav.forward', label: 'api.op.nav.forward' },
];

/** 移動の行の数（パネルの見出しを分けるときに使う）。 */
export const MOVE_COUNT = MOVES.length;

/**
 * 切替パネルの行を組み立てる（§10.0 の一覧 + §9.7 の移動の口）。
 *
 * 並びは「移動の 4 行 → ワークスペース 9 行」。ワークスペースの番号は
 * `nav.switcher` の `number` をそのまま使う（画面で数え直さない。番号がずれると
 * `Alt+<n>` と食い違う）。`previous` は `nav.switcher` が返した値だけを信じる。
 */
export function switcherRows(
  found: {
    current?: unknown;
    previous?: unknown;
    workspaces?: unknown;
  } | null,
): SwitcherRow[] {
  const rows: SwitcherRow[] = MOVES.map((move) => ({
    kind: 'move',
    op: move.op,
    args: {},
    label: move.label,
    number: null,
    current: false,
    previous: false,
  }));
  const list = found?.workspaces;
  if (!Array.isArray(list)) {
    return rows;
  }
  for (const item of list) {
    const row = item as { name?: unknown; number?: unknown; current?: unknown };
    if (typeof row?.name !== 'string' || row.name === '') {
      continue;
    }
    const number = typeof row.number === 'number' ? row.number : null;
    rows.push({
      kind: 'workspace',
      op: 'session.workspace_switch',
      args: { workspace: row.name },
      label: `header.workspace.${row.name}`,
      number,
      current: row.current === true,
      previous: row.name === found?.previous,
    });
  }
  return rows;
}

/** 切替パネルの見出し（いまのワークスペース名。空なら `null`）。 */
export function switcherCurrent(found: { current?: unknown } | null): string {
  return typeof found?.current === 'string' ? found.current : '';
}

/**
 * キーから移動の操作へ写す（`Ctrl+[` / `Ctrl+]`）。
 *
 * `SHORTCUTS`（`keys.ts`）は**コマンド名**を持つ表で、`nav.*` はカタログに `command` 列が
 * 無い。そのため画面側でキーを操作 ID へ写す（画面側で完結する `ui:` と同じ扱い）。
 * ここに無いキーは `null` を返し、呼び出し側は何もしない。
 *
 * `Ctrl+H` / `Ctrl+Shift+H`（`nav.back` / `nav.forward`）は**写さない**。表の `Ctrl+H` は
 * `replace`（仕様 §9.8.1 の「検索 / 置換」）で、`search.replace_file` の唯一の口だからである。
 * この 2 つは切替パネルの行から呼ぶ。
 */
export function moveOf(key: string): string | null {
  switch (key) {
    case 'Ctrl+BracketLeft':
      return 'nav.prev';
    case 'Ctrl+BracketRight':
      return 'nav.next';
    default:
      return null;
  }
}

/** 文書のリネームの入力（`document.rename` の 1 引数だけを画面が持つ）。 */
export interface RenameInput {
  /** 元のパス（`contents/01-04.cnov` など）。 */
  from: string;
  /** 新しいパス（入力そのまま。検証は `renamePlan` が行う）。 */
  to: string;
}

/** リネームの判定。 */
export type RenamePlan =
  | { ok: true; from: string; to: string }
  | { ok: false; reason: 'empty' | 'same' | 'separator' | 'invalid' };

/**
 * リネームの入力を確かめる（§9.7.5 の「短い入力は閉じたら破棄」の前段）。
 *
 * 画面は**入力の形だけ**を見る。既にあるかどうか・書けるかどうかは機能核が決める
 * （`document.rename` は既存のパスを `not_found`、重複を `not_found`、権限を `policy` で返す）。
 * ここで「ある/ない」を先回りして判定すると、機能核の判定と二重になる（§19.1.4）。
 */
export function renamePlan(from: string, to: string): RenamePlan {
  const source = from.trim();
  const target = to.trim();
  if (target === '') {
    return { ok: false, reason: 'empty' };
  }
  if (target === source) {
    return { ok: false, reason: 'same' };
  }
  // パスの区切りは受け付ける（`contents/` の中を移すのは利用者の領分）が、`..` は断る
  // （`document.rename` は新しいパスをそのまま作るので、外へ出る名前を作らせない）。
  if (target.split('/').some((part) => part === '..')) {
    return { ok: false, reason: 'separator' };
  }
  if (target.startsWith('/') || target.endsWith('/') || target.includes('//')) {
    return { ok: false, reason: 'separator' };
  }
  // 空白だけの名前・制御文字は断る（`rename.invalid` と同じ規則）。
  if (/[\u0000-\u001f\u007f]/.test(target)) {
    return { ok: false, reason: 'invalid' };
  }
  return { ok: true, from: source, to: target };
}

/** リネームの理由 → 画面の文言キー（`catalog.ts` に実在するものだけ）。 */
export const RENAME_MESSAGES: Readonly<Record<string, string>> = {
  empty: 'rename.empty',
  same: 'rename.duplicate',
  separator: 'rename.invalid',
  invalid: 'rename.invalid',
};

/**
 * 履歴の移動の結果を「画面を開き直すべきか」に写す（`nav.prev` / `nav.next` の戻り値）。
 *
 * `moved: false` は端に居るだけなので**何もしない**（通知も出さない。§9.8 の移動は
 * 端で黙って止まる）。`scene` が無い実装差も吸収する。
 */
export function movedScene(found: Record<string, unknown> | null): string {
  if (found?.moved !== true) {
    return '';
  }
  return typeof found.scene === 'string' ? found.scene : '';
}

/**
 * `nav.back`（親へ）の行き先が「無い」ことを、エラーではなく「動かなかった」として扱う。
 *
 * 機能核は親が無ければ `{ moved: false }` を返す（`src/app/ops/editing.rs` の `nav_back`）。
 * 画面はそれを通知にしない（押しただけで警告が出るのは §9.2.1 の静けさに反する）。
 */
export function didMove(found: Record<string, unknown> | null): boolean {
  return found?.moved === true;
}
