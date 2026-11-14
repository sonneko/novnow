// 画面と橋渡しのあいだの型（**W2 で凍結**。`REF:docs/plans.md#webui-parallel` の §11.11.5）。
//
// ここに書いた型は、以降のレーンが**読むだけ**で、変更権を持たない。変える必要が出たら
// 進行役（L0 の所有者）へ提案し、`run` / `open` / `poll_events` / `flush` の 4 つの口と
// イベントの形を変えない範囲で追加する。
//
// 正本は 2 つに分かれる:
//
// | 何 | 正本 | このファイルでの扱い |
// |---|---|---|
// | 操作 ID と引数・戻り値 | `REF:tests/golden/api_catalog.json` | `catalog.ts`（生成物）を再輸出 |
// | 橋渡しの口とイベント | `REF:crates/novnow-wasm/src/lib.rs` と `REF:src/app/event.rs` | ここで手書き（形は凍結） |
//
// Rust 側の `NovnowSession` は JSON を**文字列**で返す（`REF:docs/developers.md` §8.10 の 32）。
// 文字列と型の変換はこのファイルの 1 か所だけが行う。

export type {
  OpArgs,
  OpId,
  OpResult,
  OpId as OperationId,
} from './catalog';
export { CATALOG_VERSION, OP_IDS, PRODUCT_VERSION, isOp } from './catalog';

// ---------------------------------------------------------------------------
// 橋渡しの口（§21.3。wasm と Tauri で同じ形）
// ---------------------------------------------------------------------------

/** 正規操作 1 回の結果（`run` の戻り値を解いたもの）。 */
export type RunOutcome<R = unknown> =
  | { ok: true; result: R; pending?: string[]; error?: string }
  | { ok: false; error: OpError };

/** 失敗（`REF:src/app/error.rs` の `Error::to_json`）。 */
export interface OpError {
  /** 種別（`usage` / `not_found` / `policy` / `conflict` / `io` / `project` …）。 */
  code: string;
  /** 診断コード（`E060` など。無ければ空文字）。 */
  kind: string;
  /** 表示用の文言（利用者の言語で返る）。 */
  message: string;
  /** 対象のパス（無ければ `null`）。 */
  path: string | null;
}

/** 保存状態機の 6 状態（§9.6。`src/ui/status.rs` と同じ名前）。 */
export type SaveState =
  | 'idle'
  | 'dirty'
  | 'saving'
  | 'saved'
  | 'error'
  | 'conflict';

/** セッションが外へ知らせる出来事（`REF:src/app/event.rs` の `AppEvent`）。 */
export type AppEvent =
  | { kind: 'diagnostics_changed'; scope: string; count: number; path: string }
  | { kind: 'save_state_changed'; state: SaveState; message: string }
  | { kind: 'agent_progress'; task: string; stage: string; message: string }
  | { kind: 'materials_changed'; id: string; change: string }
  | { kind: 'document_changed'; path: string; change: string }
  | { kind: 'write_failed'; path: string; message: string }
  | { kind: 'external_unreadable'; path: string; code: string; message: string }
  | { kind: 'settings_changed'; key: string }
  | { kind: 'cancelled'; op: string; handle: string };

/** 橋渡し（ブラウザは wasm、デスクトップは Tauri の `invoke`）。 */
export interface Bridge {
  /** プロジェクトを開く。`root` を省くと保存領域の 1 つ目（無ければサンプル）を開く。 */
  open(root?: string | null): Promise<void>;
  /**
   * 保存領域を使えない環境で、メモリだけでサンプルを開く（`open` が失敗したときだけ）。
   *
   * 画面は「保存されない」ことを通知に出す（§21.1.2）。ブラウザの wasm だけが持ち、
   * Tauri の橋渡しは持たないので省略可能にしてある。
   */
  openFallback?(): void;
  /** 正規操作 1 回。`args` は省ける（空のオブジェクトと同じ）。 */
  run(op: string, args?: Record<string, unknown>): Promise<RunOutcome>;
  /** 溜まったイベントを取る（無ければ空の配列）。 */
  pollEvents(): Promise<AppEvent[]>;
  /** 未保存の変更を書き、保存領域へ流し終えるまで待つ。 */
  flush(): Promise<RunOutcome>;
  /** 文言（`REF:src/ui/i18n/*.toml` の 1 行。`{0}` は `args` で差し込む）。 */
  text(key: string, args?: readonly string[]): string;
  /** いまの言語（`ja` / `en`）。 */
  locale(): string;
  /** 言語を切り替える（`ui.json` にも書く）。 */
  setLocale(locale: string): Promise<void>;
  /** 開いているプロジェクトの ID（保存領域のキー）。 */
  projectId(): string;
  /** まだ保存領域へ流していない書き込みの件数（`flush` の後始末の確認用）。 */
  pendingWrites(): number;
  /** 版（`Cargo.toml` の `version`）。 */
  version(): string;
}
