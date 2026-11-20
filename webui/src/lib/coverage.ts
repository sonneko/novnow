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
  // 素材の添付（`materials.attach`）・配役の情報（`cast.get`）・辞書の一覧（`dictionary.list`）・
  // 用語の全文（`dictionary.term_full`）・診断の「どこ」（`diagnostics.place`）は解消した。
  // 添付は資料の画面のボタン、配役と用語は参照ペインの行のダブルクリック、辞書はヘッダの
  // ボタン、どこは点検の診断の行から呼ぶ。
  // `lang.*`（LSP 相当の 9 操作）はパレットの `>` から `run('<操作 ID>')` で届く口ができた
  // （`lang.tokens` / `lang.hover` / `lang.definition` / `lang.references` / `lang.symbols` /
  // `lang.format` / `lang.rename_preview` / `lang.rename_apply` / `lang.folding`）。
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
  { section: '10.3', what: 'エージェントの監査ログ（`agent.audit_list`）' },
  { section: '10.4', what: 'プロンプトの閲覧・比較・初期化（`prompt.*`）' },
  // §11.3（診断の地名解決）は解消した。点検の診断の行が、機能核の `diagnostics.place` の
  // 答え（正本の「どこ」）を `place.*` の文言で下に出す（`ReviewWorkspace.svelte`）。
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
