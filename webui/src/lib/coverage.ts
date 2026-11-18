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
 */
export const PENDING: readonly string[] = [
  'section.split', // 節の分割
  'section.propose', // 節の提案
  'plot.set_state', // プロットの確認状態
  'save.keep_mine', // 外部変更との衝突の解決（ブラウザは監視しないが、操作としては未提供）
  'save.reload_external',
  'materials.attach',
  'cast.get',
  'dictionary.list',
  'dictionary.term_full',
  'lang.tokens', // LSP 相当の 12 操作（パレットの `>` から届く口がまだ無い）
  'lang.hover',
  'lang.definition',
  'lang.references',
  'lang.symbols',
  'lang.format',
  'lang.rename_preview',
  'lang.rename_apply',
  'lang.folding',
  'diagnostics.place',
  'agent.audit_list',
  'prompt.list', // プロンプトの閲覧・比較・初期化
  'prompt.get',
  'prompt.diff',
  'prompt.reset',
  'mail.send',
  'session.workspace_get',
  'session.poll_events', // 橋渡しの `pollEvents()` が担う（正規操作として画面からは呼ばない）
  'help.topic', // ヘルプ本文（`?` の重ね物は画面側の表で描く）
  'api.list', // 正規操作カタログの出力（開発者向け。画面の操作ではない）
];

/**
 * §9〜§12 の画面仕様のうち、まだ提供していない項目（§21.5.1 の 2 つ目の MUST）。
 *
 * 操作の未提供とは別に、節ごとの見え方の不足を挙げる。1 行 1 項目。
 */
export const SPEC_GAPS: readonly SpecGap[] = [
  { section: '9.7.1', what: 'コマンドパレット（`Ctrl+P`）の候補一覧（開いて理由を書くところまで）' },
  { section: '9.7.3', what: '整形・リネーム・シンボル一覧のパレット項目' },
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
  // §10.0 の切替パネル（`nav.switcher` の一覧）は解消した（`Ctrl+K` とヘッダの `⇥`）。
  // 9 ワークスペースは `Alt+1..9` / パネルの行から切り替えられる。ただし切替先の**中身**は
  // `write` 以外が未実装なので、着地点の対応表（§10.0 の「対応物」）はまだ効かない。
  { section: '10.0', what: '`write` 以外のワークスペースの中身（着地点の対応表が未実装）' },
  { section: '10.3', what: 'エージェントの監査ログ（`agent.audit_list`）' },
  { section: '10.4', what: 'プロンプトの閲覧・比較・初期化（`prompt.*`）' },
  { section: '11.3', what: '診断の地名解決（`diagnostics.place`）の表示' },
  { section: '12.5', what: 'ヘルプ本文（`help.topic`）の全文表示' },
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
