// エージェント管制（`agent`。`REF:docs/specifications/ui.md` §10.8・`REF:docs/specifications/agent.md` §15.7）の規則。
//
// 材料は正規操作の戻り値だけ（`agent.*` / `context.build` / `agent.mcp_config_*`。§11.11.5）。
// `agents/` のファイル形式はここでは解釈しない（§21.1.3）。行うのは:
//
// 1. 4 列（① タスク / ② コンテキスト / ③ 結果レビュー / ④ 委譲レベル）のキー写像（§15.7 の表）。
// 2. タスク行・委譲行の形の確認と表示の分解（壊れた値は捨てる）。
// 3. ContextPacket のブロック分解（`## 1 project` … `## 9 history` の固定見出し、§15.4.1）と
//    ブロック別予算（§15.4.2 の既定表を `cap` に比例配分）。
// 4. 結果レビューの行単位の差分（§15.7.3）。
//
// 未提供（この版で操作が無いもの。§21.1.2 MUST で画面は「未提供」と出す）:
// - 一部採用 `P`（`agent.proposal_apply` に `text` 引数がカタログに無い）
// - ブロックの除外トグル（`exclude:` を書く操作が無い）
// - 未承認メッセージの承認 `M`（承認を書く操作が無い）
// - `applied` / `accepted` の差し戻し（`apply::revert` に相当する操作が無い）
// - ローカル LLM の設定（`project.toml` の `agents.local_llm` を書く操作が無い）

/** 4 列（§15.7 のレイアウト図の ①〜④）。 */
export const COLUMNS = ['tasks', 'context', 'review', 'delegation'] as const;

export type Column = (typeof COLUMNS)[number];

/** 列の見出しの文言キー（§15.7 のレイアウト図）。 */
export const COLUMN_LABEL: Record<Column, string> = {
  tasks: 'agent.column.tasks',
  context: 'agent.column.context',
  review: 'agent.column.review',
  delegation: 'agent.column.delegation',
};

/** この画面が横取りするキーの写像（§15.7 の操作表）。無いキーは `null`（全体の割当のまま）。 */
export type AgentAction =
  | 'new'
  | 'run'
  | 'stop'
  | 'dry-run'
  | 'select'
  | 'select-all'
  | 'run-batch'
  | 'detail'
  | 'toggle-exclude'
  | 'accept'
  | 'partial'
  | 'reject'
  | 'approve-messages'
  | 'stop-all'
  | 'level-off'
  | 'level-propose'
  | 'level-apply'
  | 'level-autonomous'
  | 'prev'
  | 'next'
  | 'first'
  | 'last'
  | 'left'
  | 'right';

/**
 * キー（`chord()` の形、例 `Ctrl+Shift+R`）を操作へ写す。
 *
 * 修飾キーなしの文字キーはフォーカスのある列のものだけが効く（§9.8.4）。`←` / `→` は列、
 * `↑` / `↓` は列の中。`Ctrl+Shift+X` は総停止（§15.7.4）で列を問わない。
 */
export function agentAction(key: string, column: Column): AgentAction | null {
  // 列を問わないもの
  switch (key) {
    case 'Ctrl+Shift+X':
      return 'stop-all';
    case 'ArrowLeft':
      return 'left';
    case 'ArrowRight':
      return 'right';
    case 'ArrowUp':
      return 'prev';
    case 'ArrowDown':
      return 'next';
    case 'Home':
      return 'first';
    case 'End':
      return 'last';
    default:
      break;
  }
  if (column === 'tasks') {
    switch (key) {
      case 'N':
        return 'new';
      case 'R':
        return 'run';
      case 'X':
        return 'stop';
      case 'D':
        return 'dry-run';
      case 'Space':
        return 'select';
      case 'Ctrl+A':
        return 'select-all';
      case 'Shift+R':
        return 'run-batch';
      case 'Enter':
        return 'detail';
      default:
        return null;
    }
  }
  if (column === 'context') {
    switch (key) {
      case 'Space':
        return 'toggle-exclude';
      case 'D':
        return 'dry-run';
      default:
        return null;
    }
  }
  if (column === 'review') {
    switch (key) {
      case 'A':
        return 'accept';
      case 'P':
        return 'partial';
      case 'X':
        return 'reject';
      case 'M':
        return 'approve-messages';
      default:
        return null;
    }
  }
  // delegation（§15.7.6。修飾キーなしの数字キー）
  switch (key) {
    case '1':
      return 'level-off';
    case '2':
      return 'level-propose';
    case '3':
      return 'level-apply';
    case '4':
      return 'level-autonomous';
    default:
      return null;
  }
}

/** 数字キーから委譲レベル（§15.1.1）。 */
export function levelOfKey(key: string): 'off' | 'propose' | 'apply' | 'autonomous' | null {
  switch (key) {
    case '1':
      return 'off';
    case '2':
      return 'propose';
    case '3':
      return 'apply';
    case '4':
      return 'autonomous';
    default:
      return null;
  }
}

/** 列の移動（`←` / `→`。端で循環）。 */
export function moveColumn(column: Column, step: 1 | -1): Column {
  const at = COLUMNS.indexOf(column);
  const next = (at + step + COLUMNS.length) % COLUMNS.length;
  return COLUMNS[next] ?? 'tasks';
}

/** タスク 1 行（`agent.task_list` / `agent.task_get` の `task`）。壊れた値は捨てる。 */
export interface TaskRow {
  /** `T0042`。 */
  id: string;
  /** 役割（`writer` など）。 */
  role: string;
  /** 対象（`@01-04-02-03` など）。 */
  scope: string;
  /** 状態（`draft` / `ready` / `running` / `review` / `applied` …§5.10.1）。表示は文字列のまま。 */
  status: string;
  /** 目的。 */
  intent: string;
  /** 起動者（`human` / `agent`。§15.7.1 MUST で `agent` と出る）。 */
  initiator: string;
}

/** 値がタスクの形をしているか。 */
export function isTask(value: unknown): value is TaskRow {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const found = value as Record<string, unknown>;
  return (
    typeof found.id === 'string' &&
    typeof found.role === 'string' &&
    typeof found.scope === 'string' &&
    typeof found.status === 'string'
  );
}

/** `agent.task_list` / `agent.task_get` のタスクを読む（形が違う行は捨てない、そのまま補う）。 */
export function taskRowsOf(result: unknown): TaskRow[] {
  const list = (result as { tasks?: unknown } | null)?.tasks;
  if (!Array.isArray(list)) {
    return [];
  }
  return list.filter(isTask).map((task) => ({
    id: task.id,
    role: task.role,
    scope: task.scope,
    status: task.status,
    intent: typeof task.intent === 'string' ? task.intent : '',
    initiator: typeof task.initiator === 'string' ? task.initiator : 'human',
  }));
}

/** 状態ごとの見た目の区分（色はトークンだけで、意味は文字列にもある。§9.9）。 */
export function statusTone(status: string): 'run' | 'wait' | 'done' | 'idle' {
  switch (status) {
    case 'running':
      return 'run';
    case 'ready':
    case 'draft':
      return 'wait';
    case 'review':
    case 'applied':
    case 'accepted':
      return 'done';
    default:
      return 'idle';
  }
}

/** `agent.initiator_agent` の文言を付けるべきか（`autonomous` で自動起動されたタスク、§15.7.1）。 */
export function startedByAgent(task: TaskRow): boolean {
  return task.initiator !== 'human' && task.initiator !== '';
}

/** 表示中の `ready` のタスク（`Ctrl+A` の対象。§15.7.5(1)）。 */
export function readyOf(rows: readonly TaskRow[]): TaskRow[] {
  return rows.filter((task) => task.status === 'ready');
}

/** `Space` の選択（選択は id の並び順を保つ）。 */
export function toggleSelect(selected: readonly string[], id: string): string[] {
  return selected.includes(id) ? selected.filter((found) => found !== id) : [...selected, id];
}

/** 一覧の中の移動（`↑` / `↓`。端で止まる。選択が無ければ先頭）。 */
export function moveInColumn(rows: readonly { id: string }[], current: string | null, step: 1 | -1 | 0 | 'first' | 'last'): string | null {
  if (rows.length === 0) {
    return null;
  }
  if (current === null || !rows.some((row) => row.id === current)) {
    return rows[0]?.id ?? null;
  }
  const at = rows.findIndex((row) => row.id === current);
  if (step === 'first') {
    return rows[0]?.id ?? null;
  }
  if (step === 'last') {
    return rows[rows.length - 1]?.id ?? null;
  }
  const next = Math.min(rows.length - 1, Math.max(0, at + step));
  return rows[next]?.id ?? null;
}

// ---------------------------------------------------------------- ② コンテキスト検査（§15.7.2）

/** ContextPacket のブロック 1 つ（`## N name` の見出し単位。§15.4.1）。 */
export interface PacketBlock {
  /** 見出しの番号（1〜9）。 */
  order: number;
  /** 名前（`project` … `history`）。 */
  name: string;
  /** 本文（見出しの行を除く）。 */
  body: string;
  /** 本文の文字数（Unicode のスカラー値。§15.4.2）。 */
  chars: number;
  /** 切り詰め印から読んだ省略文字数の合計（`…（N 文字省略）`。§15.7.2「何が落ちたか」）。 */
  omitted: number;
}

/** 切り詰め印（`REF:src/core/aicontext/budget.rs` の `omitted` と同じ形）。 */
const OMITTED = /…（(\d+) 文字省略）/gu;

/** パケット本文（`context.build` の `packet`）を 9 ブロックに分ける。 */
export function packetBlocks(text: string): PacketBlock[] {
  const out: PacketBlock[] = [];
  let current: PacketBlock | null = null;
  for (const line of text.split('\n')) {
    const found = /^## (\d) ([a-z]+)$/u.exec(line);
    if (found) {
      current = {
        order: Number(found[1]),
        name: found[2] ?? '',
        body: '',
        chars: 0,
        omitted: 0,
      };
      out.push(current);
      continue;
    }
    if (current) {
      current.body += (current.body ? '\n' : '') + line;
    }
  }
  for (const block of out) {
    block.body = block.body.replace(/^\n+|\n+$/gu, '');
    block.chars = [...block.body].length;
    block.omitted = 0;
    for (const match of block.body.matchAll(OMITTED)) {
      block.omitted += Number(match[1] ?? 0);
    }
  }
  return out;
}

/**
 * ブロック別の文字数予算（§15.4.2 の既定表）。
 * `max-chars` で合計が上書きされた分は `cap` に比例して伸び縮みする（同節）。
 * 表に無い役割（カスタムロール）は `writer` の比率を使う。
 */
export const BUDGETS: Record<string, readonly number[]> = {
  //             1 project 2 outline 3 plot 4 target 5 cast 6 lexicon 7 materials 8 constraints 9 history
  writer: [400, 300, 1200, 2400, 1200, 600, 800, 600, 400],
  plotter: [400, 1200, 2000, 600, 800, 400, 600, 600, 400],
  stylist: [800, 0, 400, 2400, 1600, 400, 0, 600, 400],
  logician: [300, 1600, 2000, 2400, 800, 800, 600, 600, 400],
  researcher: [200, 300, 600, 300, 200, 200, 3000, 600, 200],
  ideator: [300, 800, 800, 400, 600, 400, 1000, 600, 300],
  integrator: [300, 1200, 600, 300, 300, 200, 200, 600, 2300],
};

/** ブロック 1 つの予算（`cap` が 0 より大きければ既定表の比率で配分）。 */
export function blockBudget(role: string, order: number, cap: number): number {
  const base = BUDGETS[role] ?? BUDGETS.writer ?? [];
  const at = Math.min(Math.max(1, order), 9) - 1;
  const share = base[at] ?? 0;
  const total = base.reduce((sum, value) => sum + value, 0);
  if (cap > 0 && total > 0) {
    return Math.round((share * cap) / total);
  }
  return share;
}

/** 1 ブロックの予算バーの比率（0〜1。予算 0 は満タンで描かない）。 */
export function budgetRatio(chars: number, budget: number): number {
  if (budget <= 0) {
    return 0;
  }
  return Math.min(1, chars / budget);
}

/** 外せるブロックか（`1 project` と `8 constraints` は必ず送る。`agent.block.locked`）。 */
export function canExclude(name: string): boolean {
  return name !== 'project' && name !== 'constraints';
}

// ---------------------------------------------------------------- ③ 結果レビュー（§15.7.3）

/** 差分の 1 行（行単位。§15.7.3）。 */
export interface DiffRow {
  /** `same` = 両方にある / `before` = 前だけ / `after` = 後だけ。 */
  kind: 'same' | 'before' | 'after';
  /** 前の行（無ければ null）。 */
  before: string | null;
  /** 後の行（無ければ null）。 */
  after: string | null;
}

/**
 * `before` / `after` の行単位の差分（LCS。決定論）。
 *
 * 提案は節全体のスナップショット（§8.7.2）なので、行が入れ替わっても「前だけ・後だけ」で
 * 並べる。長い入力でも動くよう、行数の積が大きいときは先頭から一致を抜く簡易版に落とす。
 */
export function diffRows(before: string, after: string): DiffRow[] {
  const left = before === '' ? [] : before.split('\n');
  const right = after === '' ? [] : after.split('\n');
  if (left.length * right.length > 1_000_000) {
    return roughDiff(left, right);
  }
  // LCS の表
  const width = right.length + 1;
  const table = new Uint32Array((left.length + 1) * width);
  for (let i = left.length - 1; i >= 0; i -= 1) {
    for (let j = right.length - 1; j >= 0; j -= 1) {
      const same = left[i] === right[j] ? 1 : 0;
      const down = table[(i + 1) * width + j] ?? 0;
      const side = table[i * width + j + 1] ?? 0;
      const cross = same + (table[(i + 1) * width + j + 1] ?? 0);
      table[i * width + j] = Math.max(down, side, cross);
    }
  }
  const out: DiffRow[] = [];
  let i = 0;
  let j = 0;
  while (i < left.length && j < right.length) {
    if (left[i] === right[j]) {
      out.push({ kind: 'same', before: left[i] ?? null, after: right[j] ?? null });
      i += 1;
      j += 1;
    } else if ((table[(i + 1) * width + j] ?? 0) >= (table[i * width + j + 1] ?? 0)) {
      out.push({ kind: 'before', before: left[i] ?? null, after: null });
      i += 1;
    } else {
      out.push({ kind: 'after', before: null, after: right[j] ?? null });
      j += 1;
    }
  }
  while (i < left.length) {
    out.push({ kind: 'before', before: left[i] ?? null, after: null });
    i += 1;
  }
  while (j < right.length) {
    out.push({ kind: 'after', before: null, after: right[j] ?? null });
    j += 1;
  }
  return out;
}

/** 大きな差分の簡易版（先頭・末尾の一致を抜いて、残りを前→後で並べる）。 */
function roughDiff(left: readonly string[], right: readonly string[]): DiffRow[] {
  let head = 0;
  while (head < left.length && head < right.length && left[head] === right[head]) {
    head += 1;
  }
  let tail = 0;
  while (
    tail < left.length - head &&
    tail < right.length - head &&
    left[left.length - 1 - tail] === right[right.length - 1 - tail]
  ) {
    tail += 1;
  }
  const out: DiffRow[] = [];
  for (let at = 0; at < head; at += 1) {
    out.push({ kind: 'same', before: left[at] ?? null, after: right[at] ?? null });
  }
  for (let at = head; at < left.length - tail; at += 1) {
    out.push({ kind: 'before', before: left[at] ?? null, after: null });
  }
  for (let at = head; at < right.length - tail; at += 1) {
    out.push({ kind: 'after', before: null, after: right[at] ?? null });
  }
  for (let at = 0; at < tail; at += 1) {
    out.push({
      kind: 'same',
      before: left[left.length - tail + at] ?? null,
      after: right[right.length - tail + at] ?? null,
    });
  }
  return out;
}

/** 提案 1 件（`agent.proposal_list` の要素。`agent.proposal_get` は `before` / `after` が付く）。 */
export interface ProposalRow {
  id: string;
  /** `open` / `accepted` / `rejected` / `superseded` / `applied`（§8.7.2）。 */
  status: string;
  /** 対象（`@01-04-02-03` など）。 */
  target: string;
  /** 提案時の一言。 */
  note: string;
  /** 却下・採用の理由。 */
  comment: string;
  /** `human`（提案モード）/ `mcp` / `mcp:<役割>`（§8.7.2）。 */
  origin: string;
  /** 対象が提案後に書き換わっている（`agent.proposal_get` の `drifted`）。 */
  drifted: boolean;
  /** 節の一部だけの提案（`partial`。§8.7.2）。 */
  partial: boolean;
  before: string;
  after: string;
}

/** 値が提案の形をしているか。 */
export function isProposal(value: unknown): value is ProposalRow {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const found = value as Record<string, unknown>;
  return typeof found.id === 'string' && typeof found.status === 'string' && typeof found.target === 'string';
}

/** `agent.proposal_list` / `agent.proposal_get` の提案を読む。 */
export function proposalRowsOf(result: unknown): ProposalRow[] {
  const list = (result as { proposals?: unknown } | null)?.proposals;
  if (!Array.isArray(list)) {
    return [];
  }
  return list.filter(isProposal).map(proposalOf);
}

/** 提案 1 件（`agent.proposal_get` の戻り値も同じ形）。 */
export function proposalOf(value: unknown): ProposalRow {
  const found = value as Record<string, unknown>;
  return {
    id: String(found.id ?? ''),
    status: String(found.status ?? ''),
    target: String(found.target ?? ''),
    note: typeof found.note === 'string' ? found.note : '',
    comment: typeof found.comment === 'string' ? found.comment : '',
    origin: typeof found.origin === 'string' ? found.origin : '',
    drifted: found.drifted === true,
    partial: found.partial === true,
    before: typeof found.before === 'string' ? found.before : '',
    after: typeof found.after === 'string' ? found.after : '',
  };
}

/** 却下・差し戻しはコメント必須（§15.7.3。どちらもコメント必須）。 */
export function commentUsable(comment: string): boolean {
  return comment.trim().length > 0;
}

/** 決着済みか（`applied` / `accepted` は差し戻しの対象。§15.7.3）。 */
export function settled(status: string): boolean {
  return status === 'applied' || status === 'accepted';
}

// ---------------------------------------------------------------- ④ 委譲レベル（§15.7.6）

/** 委譲レベルの 1 行（`agent.delegation_get` の `levels`）。 */
export interface DelegationRow {
  role: string;
  /** `off` / `propose` / `apply` / `autonomous`。 */
  level: string;
}

/** `agent.delegation_get` の行を読む。 */
export function delegationRowsOf(result: unknown): DelegationRow[] {
  const list = (result as { levels?: unknown } | null)?.levels;
  if (!Array.isArray(list)) {
    return [];
  }
  return list
    .filter((value): value is Record<string, unknown> => typeof value === 'object' && value !== null)
    .filter((value) => typeof value.role === 'string' && typeof value.level === 'string')
    .map((value) => ({ role: String(value.role), level: String(value.level) }));
}

/** 委譲レベルの表示キー（§15.1.1。文言は `agent.status.*` ではなく数字で選ぶ）。 */
export function levelTone(level: string): 'off' | 'propose' | 'apply' | 'autonomous' {
  switch (level) {
    case 'propose':
      return 'propose';
    case 'apply':
      return 'apply';
    case 'autonomous':
      return 'autonomous';
    default:
      return 'off';
  }
}

// ---------------------------------------------------------------- まとめて実行の確認（§15.7.5(1)）

/** 確認ダイアログの中身（n 件・合計入力 m 文字・使用サーバ一覧）。 */
export interface RunConfirm {
  /** 件数。 */
  count: number;
  /** 合計入力文字数（1 件でも作れなければ `null`＝出さない。`agent.no_packet` を添える）。 */
  chars: number | null;
  /** コンテキストを作れなかったタスクの id。 */
  withoutPacket: string[];
}

/** 選択したタスクの確認材料を集める（`charsById` は `context.build` 済みの文字数）。 */
export function runConfirm(ids: readonly string[], charsById: ReadonlyMap<string, number>): RunConfirm {
  let chars = 0;
  const withoutPacket: string[] = [];
  for (const id of ids) {
    const found = charsById.get(id);
    if (found === undefined) {
      withoutPacket.push(id);
    } else {
      chars += found;
    }
  }
  // 1 件でも作れなければ合計を出さない（部分合計を「合計入力」として出さない。§21.1.2）
  return {
    count: ids.length,
    chars: withoutPacket.length > 0 ? null : chars,
    withoutPacket,
  };
}

// ---------------------------------------------------------------- スレッド（§15.6）

/** メッセージ 1 件（`agent.thread_get` の `messages`）。 */
export interface MessageRow {
  from: string;
  to: string;
  /** `request` / `deliver` / `question` / `reject` / `note`（§15.6）。 */
  kind: string;
  body: string;
  created_at: string;
}

/** スレッド 1 件（`agent.thread_list` / `agent.thread_get`）。 */
export interface ThreadRow {
  id: string;
  title: string;
  scope: string;
  status: string;
  messages: MessageRow[];
}

/** `agent.thread_list` / `agent.thread_get` のスレッドを読む。 */
export function threadRowsOf(result: unknown): ThreadRow[] {
  const list = (result as { threads?: unknown } | null)?.threads;
  if (!Array.isArray(list)) {
    return [];
  }
  return list
    .filter((value): value is Record<string, unknown> => typeof value === 'object' && value !== null)
    .filter((value) => typeof value.id === 'string')
    .map((value) => ({
      id: String(value.id),
      title: typeof value.title === 'string' ? value.title : '',
      scope: typeof value.scope === 'string' ? value.scope : '',
      status: typeof value.status === 'string' ? value.status : '',
      messages: Array.isArray(value.messages)
        ? value.messages
            .filter((message): message is Record<string, unknown> => typeof message === 'object' && message !== null)
            .map((message) => ({
              from: typeof message.from === 'string' ? message.from : '',
              to: typeof message.to === 'string' ? message.to : '',
              kind: typeof message.kind === 'string' ? message.kind : '',
              body: typeof message.body === 'string' ? message.body : '',
              created_at: typeof message.created_at === 'string' ? message.created_at : '',
            }))
        : [],
    }));
}

/** 稼働の表示（`agent.progress`。§15.7.5(4)）。実行中 / 待機 / 完了の件数。 */
export function progressCounts(rows: readonly TaskRow[]): { running: number; waiting: number; done: number } {
  let running = 0;
  let waiting = 0;
  let done = 0;
  for (const task of rows) {
    if (task.status === 'running') {
      running += 1;
    } else if (task.status === 'ready' || task.status === 'draft') {
      waiting += 1;
    } else {
      done += 1;
    }
  }
  return { running, waiting, done };
}

/** 対象（scope）から節 ID を抜く（`@01-04-02-03` → `01-04-02-03`）。無ければ null。 */
export function sceneOfScope(scope: string): string | null {
  const found = /^@?(\d{1,3}-\d{1,3}-\d{1,3}-\d{1,3})$/u.exec(scope.trim());
  return found?.[1] ?? null;
}
