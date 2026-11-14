// ブラウザの橋渡し（wasm-bindgen。`REF:crates/novnow-wasm/src/lib.rs`）。
//
// ここだけが `wasm-bindgen` の生成物を触る。画面の他の場所は `Bridge`（`types.ts`）だけを
// 知る（`REF:docs/designs/webui.md` §21 の「差分は Bridge の実装だけに閉じる」）。
//
// 生成物は `npm run wasm`（`scripts/webui-build.sh` と同じ手順。wasm-pack が無い環境では
// `cargo build --target wasm32-unknown-unknown` ＋ `wasm-bindgen --target web`）が
// `src/wasm/` に置く。**`src/wasm/` は git に入れない**（`.gitignore`）。

import init, {
  NovnowSession,
  version,
} from '../../wasm/novnow_wasm.js';
import type {
  AppEvent,
  Bridge,
  OpError,
  RunOutcome,
} from './types';

/** `run` / `flush` の戻り値（文字列の JSON。§8.10 の 32）。 */
interface RawRun {
  ok: boolean;
  result?: unknown;
  error?: OpError & { message?: string };
  pending?: string[];
}

/** `wasm-bindgen` の初期化（1 回だけ）。 */
let started: Promise<void> | null = null;

function start(): Promise<void> {
  if (!started) {
    started = init().then(() => undefined);
  }
  return started;
}

/** `run` の文字列を解く（壊れていたら失敗として返す。例外は投げない）。 */
function decode(text: string): RunOutcome {
  try {
    const raw = JSON.parse(text) as RawRun;
    if (raw.ok) {
      return { ok: true, result: raw.result, pending: raw.pending, error: raw.error && (raw.error as OpError).message };
    }
    return {
      ok: false,
      error: {
        code: raw.error?.code ?? 'unknown',
        kind: raw.error?.kind ?? '',
        message: raw.error?.message ?? text,
        path: raw.error?.path ?? null,
      },
    };
  } catch {
    return {
      ok: false,
      error: { code: 'bridge', kind: '', message: text, path: null },
    };
  }
}

/** wasm の橋渡し。 */
export function wasmBridge(): Bridge {
  let session: NovnowSession | null = null;

  const ready = async (): Promise<NovnowSession> => {
    await start();
    if (!session) {
      session = new NovnowSession();
    }
    return session;
  };

  return {
    async open(root) {
      const found = await ready();
      await found.open(root ?? null);
    },
    openFallback() {
      // `open` が失敗したときだけ呼ばれる（画面が通知を出す）
      session = new NovnowSession();
      session.open_fallback();
    },
    async run(op, args) {
      const found = await ready();
      // 引数は文字列の JSON で渡す（キーの順は Rust 側で固定される。§8.10 の 32）
      return decode(found.run(op, args ? JSON.stringify(args) : ''));
    },
    async pollEvents() {
      const found = await ready();
      try {
        return JSON.parse(found.poll_events()) as AppEvent[];
      } catch {
        return [];
      }
    },
    async flush() {
      const found = await ready();
      return decode(await found.flush());
    },
    text(key, args) {
      if (!session) {
        return '';
      }
      return session.text(key, args && args.length ? JSON.stringify(args) : undefined);
    },
    locale() {
      return session ? session.locale() : 'ja';
    },
    async setLocale(locale) {
      const found = await ready();
      found.set_locale(locale);
    },
    projectId() {
      return session ? session.project_id() : '';
    },
    pendingWrites() {
      return session ? session.pending_writes() : 0;
    },
    version() {
      return version();
    },
  };
}
