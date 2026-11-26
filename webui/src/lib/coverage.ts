// 第 2 の画面実装のパリティ判定（`REF:docs/specifications/webui.md` §21.5.1。
// 計画は `REF:docs/plans.md#webui-w7`、設計は `REF:docs/designs/webui.md` §21.5）。
//
// ここが持つのは **判定の入力** だけである。数え方と突き合わせは `webui/tests/parity.mjs` が行い、
// このファイルの定数は「画面のどこから到達できるか」の宣言として使う。
//
// ## 判定の単位
//
// 正本は `REF:tests/golden/api_catalog.json` の 121 操作（§19.7 の機能パリティカタログ）。
// 画面から到達できるかは、次の 3 つの口のどれかで判定する（`parity.mjs` が実物から集める）。
//
// 1. `run('<操作 ID>')` … 画面が正規操作を直接呼ぶ。
// 2. `session.command` の名前 … `REF:src/app/commands.rs` の名前を送ると、機能核が
//    カタログの `command` 列から操作へ写す（例: `save` → `save.now`）。
// 3. ショートカット表（`webui/src/lib/keyboard/keys.ts` の `SHORTCUTS`）の値 … キーから
//    2 の名前へ届く（例: `Ctrl+S` → `save`）。
//
// 1〜3 のどれでも届かないものを `pending`（未提供）とする。**推測で `done` にしない**。
//
// ## 公開（§21.5.1 MUST）
//
// 判定の結果は未提供を含めて公開する。`pending` が 0 件でないかぎり「同等」と書かない。
// いまの件数は `webui/tests/parity.mjs` が出力する（PR 本文・リリースノートへ同じ値を写す）。

/** 到達できるか。`done` は画面から届く、`pending` は未提供。 */
export type OpState = 'done' | 'pending';

/**
 * 仕様上「画面からは呼ばない」と定められた操作（§21.5.1 の判定対象外）。
 *
 * 到達判定（`parity.mjs`）の対象外とし、`PENDING`（未提供）にも入れない。**根拠なしに
 * 除外してはならない**（`parity.mjs` が「除外対象は実物からも到達していない」ことを
 * 検証する）。根拠は仕様の節をここに引用し、PR 本文にも同じ引用を書く。
 *
 * - `session.poll_events` … イベントキューは**橋渡しの層**で消費される
 *   （`interfaces.md` §19.2.4 / §147 / §202）。画面は `App.svelte` の `drain()` が
 *   `bridge.pollEvents()` を呼ぶだけで、正規操作として画面から呼ぶ口を持たない。
 * - `api.list` … 正規操作カタログの**機械可読出力**で、開発者／MCP／LSP／Python ツール向け
 *   （`interfaces.md` §19.8.2）。§19.7 の対応表の行にも画面の経路が無い。
 */
export const EXCLUDED: readonly { id: string; section: string; why: string }[] = [
  {
    id: 'session.poll_events',
    section: 'interfaces.md §19.2.4 / §147 / §202',
    why: 'イベントキューは橋渡しの pollEvents() が消費する。画面からは正規操作として呼ばない。',
  },
  {
    id: 'api.list',
    section: 'interfaces.md §19.8.2',
    why: '正規操作カタログの機械可読出力（開発者・MCP・LSP・Python 向け）。画面の操作ではない。',
  },
];

/** 画面仕様（§9〜§12）の項目 1 つ分の未提供一覧の行。 */
export interface SpecGap {
  /** §9〜§12 の節番号（`9.8.2` など）。 */
  section: string;
  /** 何が使えないか（1 行）。 */
  what: string;
}

/**
 * 画面から到達できない操作（未提供）。`parity.mjs` が実物から集めた結果と**一致**することを
 * テストが見る（片方だけ直して「到達できる」と書く事故を防ぐ）。
 *
 * 並びはカタログの順（`tests/golden/api_catalog.json` の `operations`）に合わせる。
 *
 * バッチ6 で残っていた 10 件はすべて閉じた。8 件はパレットの `>` から
 * `run('<操作 ID>')` で直接呼ぶ口ができ、2 件（`session.poll_events` / `api.list`）は
 * 仕様上「画面からは呼ばない」と定められた操作として `EXCLUDED` へ移した
 * （根拠は上と PR 本文。`parity.mjs` が実物からも到達していないことを検証する）。
 */
export const PENDING: readonly string[] = [];

/**
 * §9〜§12 の画面仕様のうち、まだ提供していない項目（§21.5.1 の 2 つ目の MUST）。
 *
 * 操作の未提供とは別に、節ごとの見え方の不足を挙げる。1 行 1 項目。各行は未解消の機能または
 * 仕様に根拠のある形態制限を表し、到達していない機能をコメントだけで解消扱いにしない。
 */
export const SPEC_GAPS: readonly SpecGap[] = [];

/** 未提供の操作 ID（`PENDING` と同じ内容を集合で見るためのもの）。 */
export const PENDING_SET: ReadonlySet<string> = new Set(PENDING);

/** 操作 ID が未提供か。 */
export function isPending(id: string): boolean {
  return PENDING_SET.has(id);
}

/** 到達できる操作の件数（カタログの全件数から未提供を引いたもの）。 */
export function reachableCount(total: number): number {
  return total - PENDING.length;
}

/**
 * 判定の見出し。**未提供が 0 件のときだけ**「同等」と書ける（§21.5.1 MUST）。
 * 表示（サイト・リリースノート）はこの 1 か所から引く。
 */
export function verdict(total: number): 'equivalent' | 'partial' {
  return PENDING.length === 0 && total > 0 ? 'equivalent' : 'partial';
}
