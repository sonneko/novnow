// バッチ6 の操作群（監査ログ・プロンプト・メール・ワークスペース・ヘルプ）の画面側の口。
//
// バッチ5 の `langOps.ts` と同じ方針：パレット行の `name` は**操作 ID そのもの**で、
// 画面は `store.run('<操作 ID>')` のリテラルで呼ぶ（`parity.mjs` の到達判定の 1 つ目）。
// `src/app/commands.rs` のパレット表には載らない（`command: null` の操作）ので、
// `session.command` 名ではなく直接 `run` で呼ぶ。`help.topic` のみ `command: "help"` で
// `>help` からも行けるが、パレット行では操作 ID で統一する（`run('help.topic')`）。
//
// 引数が必要な操作（`prompt.get/diff/reset` の `name`、`mail.send` の 4 つ）は
// `OpsDialog` の入力フォームで受け、不要な操作は即実行する。

import type { OpId } from '../bridge/catalog';

/** バッチ6 で画面から呼ぶ操作の ID（`command: null` のものが大半）。 */
export type OpsId =
  | 'agent.audit_list'
  | 'prompt.list'
  | 'prompt.get'
  | 'prompt.diff'
  | 'prompt.reset'
  | 'mail.send'
  | 'session.workspace_get'
  | 'help.topic';

/** パレット行。`label` は i18n の `api.op.*` キー（両言語に存在）。 */
export interface OpsRow {
  name: OpsId;
  label: string;
}

/**
 * パレット行の一覧。バッチ5 の `LANG_PALETTE` と同じ形で、`App.svelte` の
 * `PALETTE_ROWS` に追加する。`name` は操作 ID のまま（`session.command` 名ではない）。
 */
export const OPS_PALETTE: OpsRow[] = [
  { name: 'agent.audit_list', label: 'api.op.agent_audit_list' },
  { name: 'prompt.list', label: 'api.op.prompt_list' },
  { name: 'prompt.get', label: 'api.op.prompt_get' },
  { name: 'prompt.diff', label: 'api.op.prompt_diff' },
  { name: 'prompt.reset', label: 'api.op.prompt_reset' },
  { name: 'mail.send', label: 'api.op.mail_send' },
  { name: 'session.workspace_get', label: 'api.op.session_workspace_get' },
  { name: 'help.topic', label: 'api.op.help_topic' },
];

/** 名前がバッチ6 の操作か（`runPalette` の分岐に使う）。 */
export function isOpsOp(name: string): name is OpsId {
  return (OPS_PALETTE as readonly OpsRow[]).some((row) => row.name === name);
}

/** 引数が不要な操作（即実行）。 */
export function isArglessOp(op: OpsId): boolean {
  return op === 'agent.audit_list' || op === 'prompt.list' || op === 'session.workspace_get';
}

/** `prompt.*` の 3 つに渡す `name` の入力の形。 */
export interface PromptInput {
  name: string;
}

/** `mail.send` に渡す引数（カタログの `OpArgs['mail.send']` に揃える）。 */
export interface MailInput {
  to: string;
  kind: string;
  scope: string;
  body: string;
}

/** `help.topic` に渡す `topic` の入力の形。 */
export interface HelpInput {
  topic: string;
}

/** 画面が操作へ渡す引数（不要な操作は `null`）。 */
export type OpsInput = PromptInput | MailInput | HelpInput | null;

/** `mail.send` の `kind` の選択肢（`agent.md` §15.6 の 5 種。既定は `note`）。 */
export const MAIL_KINDS = ['request', 'deliver', 'question', 'reject', 'note'] as const;

/** `OpId` への收窄（`store.run` の型のため）。 */
export type OpsOpId = OpId;
