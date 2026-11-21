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
  // `ProjectPicker` と `picker.ts` は native capability を立て、`App.svelte` も __TAURI__ を見て
  // native と判定する。しかし画面入口 `webui/src/main.ts` は常に `wasmBridge()` を生成する。
  // `webui/src-tauri/README.md` の「いまの状態」も画面は wasmBridge 固定・invoke bridge 未接続と明記。
  // したがって Tauri WebView ではボタンが有効表示されても `project.new` / `project.recent_list`
  // は wasm32 の no_fs 経路へ行く。Rust の Tauri command / `NativeSession` の create テストは
  // あるが、実画面の経路ではない。ブラウザは `src/app/ops/project.rs` の wasm32 cfg で no_fs。
  // `platform.md` §13.3.4 と `webui.md` §21.2.3 のブラウザ制限だけでは native の未接続を正当化できず、
  // 第2画面としては両実行形態の到達が未完了。実際の表示・API経路は tests/project.test.mjs と
  // `webui/src-tauri/README.md` で確認する。
  { section: '9.7', what: 'ブラウザでは project.new / project.recent_list が no_fs。Tauri native の command は存在するが第2画面は wasmBridge 固定で未接続（native判定時にボタンが有効表示されても wasm32 API へ行く）。ネイティブの画面到達は未提供（main.ts / src-tauri/README.md / tests/project.test.mjs）。' },
  // `Alt+↑/↓`, `Alt+←/→`, `Ctrl+Alt+←/→`, `Ctrl+Home/End`, `Ctrl+Tab`, `Ctrl+B`,
  // `Ctrl+Shift+[/]` は SHORTCUTS → `session.command` で到達する。追加した `Ctrl+Shift+B`
  // も `up` に到達し、Write の文脈ビューと Outline のツリーで `Ctrl+K` に続く `Ctrl+0/J` を2秒で受ける。
  // キー写像は `keys.test.mjs`、両画面の折りたたみ実装は `b7-gaps.test.mjs` で検証する。
  // §9.8.2 の表にある構造移動はこのバッチで全て経路ができたため、この gap は除去した。
  // `Ctrl+H` は表の `replace`（検索・置換）を維持し、`nav.back` には割り当てない。
  // （仕様 §9.8.1 と `interfaces.md` の食い違い。仕様側の宿題。）
  // B8 の「機能核にない」という説明は範囲が広すぎた。`src/core/features/search.rs::run`
  // は 1000ms で `timed_out` と部分 hit を返し、`replacement` は複数 FileEdit の WorkspaceEdit を作る。
  // `src/ui/workspaces/regex_search.rs` はプレビュー後に `bench.apply_edit`、
  // `src/app/workbench.rs::apply_edit` は before/after 全体を workspace_undo に 1 件積む。
  // 実行証拠: `tests/stage_two.rs::search_regex_case_deadline_replacement_and_snapshots`。
  // ただしこれは第1画面の経路。第2画面の正規 API カタログでは `search.project` は query/regex を受け
  // count/hits/query のみを返し timeout/partial を返さず、`search.replace_file` は単一 path の置換。
  // `src/app/ops/editing.rs` に既存 WorkspaceEdit を適用する検索操作は無い。また第2画面の
  // `webui/src/main.ts` は wasmBridge 固定で、Tauri native command bridge にも接続されない。
  // よって第2画面の gap は残るが、正しい根拠は「機能核に無い」ではなく「第2画面の正規経路に未接続」。
  { section: '9.8.3', what: '第2画面の残り: 第1画面の search.run は1秒 timeout/partial hits を実装し、WorkspaceEdit preview/apply と1 undoも存在する（core/features/search.rs・ui/workspaces/regex_search.rs・workbench.rs）。しかし第2画面の正規API search.project は timeout/partial を返さず、search.replace_file は単一 path。全体検索置換を同一の複数ファイル WorkspaceEdit に適用する第2画面の口がない（api_catalog.json / app/ops/editing.rs）。' },
  // ブラウザでは外部監視をしない（webui.md §21.2.3）。ネイティブ用 `poll_external` command と
  // `NativeSession::poll_external` は存在し、`webui/src-tauri/bridge/tests/external.rs` は外部更新の
  // clean reload / dirty conflict 等を実行で確認する。しかし実画面 `webui/src/main.ts` は wasmBridge
  // 固定、`Bridge` 型に pollExternal もなく、App.svelte は poll_external を呼ばない。
  // `src-tauri/README.md` も「invoke bridge が入るまではブラウザ用 bridge」と明記している。
  // したがってネイティブ基盤に機能はあるが、第2画面の Tauri UI からは監視できず、§21.2.3 の native MUST
  // を満たさない。衝突解決 dialog があることだけでは、外部変更検知の代替にならない。
  { section: '9.6', what: 'ブラウザ形態は仕様上外部変更を監視しない。Tauri native の poll_external / 衝突処理と bridge tests はあるが、第2画面は wasmBridge 固定で pollExternal を呼ばず、native MUST が画面から未到達（webui.md §21.2.3 / src-tauri/README.md / bridge/tests/external.rs）。' },
  // B8 は参照ペインのクリック Nav (`character/index`, `material_open/id`) を workspace に
  // つないだが、§10.0 固定表の Ctrl+Shift+1..9 の経路は別。`App.svelte` は `keepPayload` から
  // workspace 名だけを `session.workspace_switch` に渡し、当該分岐で `reference.pane` や focus ID を
  // 解決しない。Catalog の `session.workspace_switch` は必須 workspace だけ、`session.command` は
  // 必須 name だけ。`reference.pane` の cast は本文節中に登場した人物を `characters_in` で列挙する
  // (`src/core/features/reference.rs`) もので、mode: フィールド先頭キャラへの固定着地とは別。
  // material については reference.pane が明示資料と候補を返すが shortcut 分岐は最初の row を選ばない。
  // `plot-episode`→write の節は workspace の共有 session scene に追従する範囲がある一方、
  // `plot-outline`→write の「最初の未完節」やそのキャレット位置を shortcut 分岐が計算・反映しない。
  // このため B8 クリック着地をもって表全体を閉じず、残る着地と操作経路を未決として残す。
  { section: '10.0', what: '§10.0 固定表の Ctrl+Shift+1..9 で未到達: 任意→cast は mode: 先頭キャラを選ぶ経路なし（reference.pane の cast は本文登場キャラ）、任意→material は reference.pane の先頭候補を選ぶ経路なし、plot-outline→write は最初の未完節とキャレットの反映なし。Catalog の session.workspace_switch は workspace のみ、session.command は name のみ。B8 の参照Navクリック経路は別経路で、固定表の到達を証明しない。' },
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
