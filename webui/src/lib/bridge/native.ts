// Tauri のネイティブ橋渡し。invoke は既存4 commandsだけを呼ぶ。
import type { AppEvent, Bridge, OpError, RunOutcome } from './types';
import { texts, type Locale } from '../i18n';

interface TauriGlobal {
  core?: { invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> };
  dialog?: { open(options?: Record<string, unknown>): Promise<string | string[] | null> };
}
interface RawRun {
  ok: boolean;
  result?: unknown;
  error?: Partial<OpError>;
}

function decode(raw: string): RunOutcome {
  try {
    const value = JSON.parse(raw) as RawRun;
    if (value.ok) return { ok: true, result: value.result };
    return { ok: false, error: {
      code: value.error?.code ?? 'unknown', kind: value.error?.kind ?? '',
      message: value.error?.message ?? raw, path: value.error?.path ?? null,
    } };
  } catch {
    return { ok: false, error: { code: 'bridge', kind: '', message: raw, path: null } };
  }
}

function tauri(): TauriGlobal {
  const value = (window as Window & { __TAURI__?: TauriGlobal }).__TAURI__;
  if (!value?.core?.invoke) throw new Error('Tauri invoke bridge is unavailable');
  return value;
}

export function nativeBridge(): Bridge {
  let root = '';
  let version = '';
  let locale = 'ja';
  const invoke = <T>(command: string, args?: Record<string, unknown>) => tauri().core!.invoke<T>(command, args);
  const run = async (op: string, args?: Record<string, unknown>): Promise<RunOutcome> => {
    const result = decode(await invoke<string>('run', { op, args: JSON.stringify(args ?? {}) }));
    if (result.ok && op === 'session.command' && args?.name === 'language') {
      locale = locale === 'ja' ? 'en' : 'ja';
    }
    return result;
  };
  return {
    async open(path) {
      let selected = path ?? '';
      if (!selected) {
        const dialog = tauri().dialog;
        if (!dialog) throw new Error('プロジェクトフォルダーを選択できません');
        const choice = await dialog.open({ directory: true, multiple: false, title: 'Open project' });
        if (typeof choice !== 'string') throw new Error('プロジェクトが選択されませんでした');
        selected = choice;
      }
      const result = decode(await invoke<string>('open', { path: selected, create_options: null }));
      if (!result.ok) throw new Error(result.error.message);
      const data = result.result as { root?: string; version?: string; locale?: string };
      root = data.root ?? selected;
      version = data.version ?? '';
      locale = data.locale ?? 'ja';
    },
    async run(op, args) { return run(op, args); },
    async projectSearch(args) { return run('ui.search.project', args); },
    async pollEvents() {
      try { return JSON.parse(await invoke<string>('poll_events')) as AppEvent[]; } catch { return []; }
    },
    async pollExternal() { return decode(await invoke<string>('poll_external')); },
    async flush() { return run('save.now'); },
    text(key, args) { return texts(locale as Locale).t(key, args); },
    locale() { return locale; },
    async setLocale(next) {
      const found = await run('settings.set', { key: 'language', value: next });
      if (!found.ok) throw new Error(found.error.message);
      locale = next;
    },
    projectId() { return root; },
    pendingWrites() { return 0; },
    version() { return version; },
  };
}
