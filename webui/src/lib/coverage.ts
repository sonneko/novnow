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
 * 操作の未提供とは別に、節ごとの見え方の不足を挙げる。1 行 1 項目。
 */
export const SPEC_GAPS: readonly SpecGap[] = [
  // §9.7.1 のコマンドパレット（`Ctrl+P`）の候補一覧と、§9.7.3 の整形・リネーム・シンボル一覧の
  // パレット項目は解消した。`lang.*` の 9 件を `>` パレットから `run('<操作 ID>')` で呼べる。
  // §9.7 の `Ctrl+O`（プロジェクト選択画面へ戻る 2 段構成）は解消した。入口の面は
  // `webui/src/lib/project/ProjectPicker.svelte` が持ち、`Ctrl+O` から開く。
  // ただし**ブラウザでは一覧と新規作成が押せない**（機能核が `no_fs` を返す。下の §9.7 の行）。
  { section: '9.7', what: 'ブラウザでの最近開いた一覧・新規作成（機能核が `no_fs`。ネイティブのみ）' },
  // §9.8.2 のうち、`nav.*` の 4 つ（移動履歴の前後・親へ・子の先頭へ）は解消した。
  // 切替パネル（`Switcher.svelte`）の行から呼べる。**残りは下の行**（節・場面・ファイルの
  // 構造移動そのものは `session.command` の `previous-section` などで、まだ口が無い）。
  // `Ctrl+H` は表の `replace`（検索・置換）が使うので、`nav.back` のキーには**しない**
  // （仕様 §9.8.1 と `interfaces.md` の食い違い。仕様側の宿題）。
  { section: '9.8.2', what: '構造移動の残り（前後の節・場面・ファイルのキー、`Ctrl+Home`/`Ctrl+End`）' },
  { section: '9.8.3', what: '検索・置換の入り口（キーは表にあるが、入力欄が未提供）' },
  // §9.6 の保存状態機のうち、**衝突の解決**（`save.keep_mine` / `save.reload_external`）と
  // 差分の表示（`save.diff_external`）は解消した。ステータスバーの `⇄ <件数>` から開く。
  // ただし**外部変更の監視そのものはブラウザでは行わない**（§13.3）ので、この面が出るのは
  // ネイティブだけである（面は理由を書いて出る。§21.1.2）。
  { section: '9.6', what: 'ブラウザでの外部変更の検知と衝突（§13.3: 監視しない。ネイティブのみ）' },
  // §10.0 の切替パネル（`nav.switcher` の一覧）は解消した（`Ctrl+K` とヘッダの `⇥`）。
  // 9 ワークスペースは `Alt+1..9` / パネルの行から切り替えられ、切替先の中身も 9 つとも
  // 描く（`webui/src/lib/shell/AppShell.svelte`。`write` 以外の 8 つは各ワークスペースの
  // 部品を実際に置く）。着地点の対応表（§10.0 の「対応物」）はまだ届かない。
  { section: '10.0', what: 'ワークスペースを切り替えたときの着地点の対応表（§10.0 の「対応物」）' },
  // §10.3（監査ログ `agent.audit_list`）・§10.4（プロンプト `prompt.*`）・§12.5（ヘルプ本文
  // `help.topic`）はバッチ6 で解消した。パレットの `>` から `run('<操作 ID>')` で呼ぶ口が
  // でき、結果を `OpsDialog` が機能核の戻り値のまま表示する。
  // §11.3（診断の地名解決）は解消した。点検の診断の行が、機能核の `diagnostics.place` の
  // 答え（正本の「どこ」）を `place.*` の文言で下に出す（`ReviewWorkspace.svelte`）。
];

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
