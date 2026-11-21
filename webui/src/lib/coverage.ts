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
export const SPEC_GAPS: readonly SpecGap[] = [
  // §9.7.1 のパレット候補、§9.7.3 の言語操作、`Ctrl+O` の入口は提供済み。
  // ここに残すのは第2の画面全体の不足ではなく、ブラウザ形態の制限である。
  // 機能核はブラウザで `project.recent_list` / `project.new` に `no_fs` を返す。
  // 仕様根拠: `platform.md` §13.3.4「Web には『プロジェクトディレクトリパス』が存在しないため、
  // その等価物としてプロジェクト ID の履歴のみを許可する」および `webui.md` §21.2.3
  // 「ブラウザでは OS の保存ダイアログを使えない」。ネイティブの一覧・新規作成とは区別して記録する。
  { section: '9.7', what: 'ブラウザ形態では最近開いた一覧・新規作成を提供しない（`no_fs`。ネイティブのみ。platform.md §13.3.4 / webui.md §21.2.3）' },
  // `Alt+↑/↓`, `Alt+←/→`, `Ctrl+Alt+←/→`, `Ctrl+Home/End`, `Ctrl+Tab`, `Ctrl+B`,
  // `Ctrl+Shift+[/]` は SHORTCUTS → `session.command` で到達する。追加した `Ctrl+Shift+B`
  // も `up` に到達し、Write の文脈ビューと Outline のツリーで `Ctrl+K` に続く `Ctrl+0/J` を2秒で受ける。
  // キー写像は `keys.test.mjs`、両画面の折りたたみ実装は `b7-gaps.test.mjs` で検証する。
  // §9.8.2 の表にある構造移動はこのバッチで全て経路ができたため、この gap は除去した。
  // `Ctrl+H` は表の `replace`（検索・置換）を維持し、`nav.back` には割り当てない。
  // （仕様 §9.8.1 と `interfaces.md` の食い違い。仕様側の宿題。）
  // カタログ `tests/golden/api_catalog.json`: `search.project` は query/regex のみを受け
  // count/hits/query を返し、timeout・partial の引数/戻り値がない。`search.replace_file` は
  // path/query/replace/regex/role を受ける単一 path の操作。機能核 `src/app/ops/editing.rs`
  // も search_project は全 documents を走査して完了後に返し、replace_file は editing_document
  // 1 件を commit する。`src/core/features/search.rs` の1秒停止と WorkspaceEdit 生成は別機能核処理で、
  // 上記の正規操作へ結線されておらず、画面から呼べる適用口ではない。
  { section: '9.8.3', what: '検索・置換の残り: プロジェクト全体の置換をプレビュー後に原子的に適用する操作、および search.project の1秒中断/部分結果の正規API口がない（catalog: search.project / search.replace_file、src/app/ops/editing.rs）。入力・大小文字トグル・全体検索・単一文書のプレビュー置換は提供済み。' },
  // §9.6 の保存状態機の**衝突解決**（`save.keep_mine` / `save.reload_external`）と差分表示
  // (`save.diff_external`) は提供済み。外部変更の監視自体はブラウザ形態では意図的に行わない制限。
  // 仕様根拠: `webui.md` §21.2.3 は「外部変更の検知（§8.5）は、ブラウザでは行わない（§13.3）」
  // と明記。従ってこの gap は第2の画面全体の不足ではなく、ネイティブのみの機能として残す。
  { section: '9.6', what: 'ブラウザ形態での外部変更検知・衝突（仕様で無効。webui.md §21.2.3 / platform.md §13.3）' },
  // §10.0 の reference.pane Nav から cast/material の選択着地は実装済み（App.svelte →
  // AppShell → CastWorkspace / MaterialWorkspace。material_open は materials.get で解決）。
  // ただし固定表全体は未実証: `tests/golden/api_catalog.json` の `session.workspace_switch`
  // は必須 `workspace` のみ、`session.command` は必須 `name` のみで、着地点SceneIdや
  // キャラ/資料IDを受け取らない。`reference.pane` は現在節の出現キャラ (`Nav::Character(index)`)
  // と明示資料 (`Nav::OpenMaterial(id)`) を返すが、§10.0 の任意→cast は `mode:` の先頭キャラ、
  // 任意→material は条件一致候補も必要。さらに plot-episode/outline と write 間のカーソル/節対応は
  // workspace switch の返り値に位置を含まず、`src/app/ops/session_ops.rs` に位置選択処理がない。
  // 従って partial な実装をもって固定表を閉じず、未到達の表全体は gap として維持する。
  { section: '10.0', what: '§10.0固定表の未実証部分: 任意→cast の mode: 先頭キャラと任意→material の条件一致候補、plot-episode/outline↔write のSceneId・カーソル位置選択。session.workspace_switch はworkspace名のみ、session.command はcommand名のみ（catalog）、reference.pane Navのcharacter/material_openだけでは固定表全体を満たさない。参照Navから既知キャラ/明示資料への切替着地は提供済み。' },
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
