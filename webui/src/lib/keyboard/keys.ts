// キー入力の写像（§9.8）。**`keydown` を受けるのは画面でここ 1 か所だけ**にする
// （`REF:docs/designs/webui.md` §21.2）。
//
// 規則は第 1 の画面実装と同じ:
//
// 1. IME の変換中はすべてのショートカットを抑止する（§9.8.5。`IME` クラスが判定する）。
// 2. 修飾キーの順序は `Ctrl+Shift+…` → `Ctrl+…` の順に見る（`REF:docs/developers.md` §8.7 の 20）。
// 3. 出力は `REF:src/app/command.rs` の `Command` に**すでに付いている名前**だけにする。
//    名前は `REF:src/app/commands.rs` の表（`PALETTE_COMMANDS` と `SHORTCUT_PALETTE_COMMANDS`）が
//    正本で、`session.command` はその表に無い名前を `usage` エラーにする。**綴りを写し間違えると
//    キーが黙って効かなくなる**ので、`webui/tests/keys.test.ts` が全名前を実物と突き合わせる。
// 4. macOS では `Alt` 系を `Ctrl+Alt` に読み替える（§9.8.4）。

/** キー写像の前提。 */
export interface KeyOptions {
  /** macOS か（`Alt` を `Ctrl+Alt` に読み替える）。 */
  mac: boolean;
}

/** 押されたキーの素性（`KeyboardEvent` から作る。テストで直接作れるようにする）。 */
export interface KeyStroke {
  /** `event.code`（`KeyA` / `Digit1` / `Semicolon` / `BracketLeft` …）。数字の判定に使う。 */
  code: string;
  /** `event.key`（`a` / `ArrowUp` / `Enter` / `;` など）。 */
  key: string;
  /** `Ctrl`（macOS では `Meta` も真にする）。 */
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  meta: boolean;
}

/** `KeyboardEvent` から素性を作る（`ctrlKey || metaKey` を `Ctrl` とみなす）。 */
export function strokeOf(event: KeyboardEvent): KeyStroke {
  return {
    code: event.code,
    key: event.key,
    ctrl: event.ctrlKey || event.metaKey,
    shift: event.shiftKey,
    alt: event.altKey,
    meta: event.metaKey,
  };
}

/** `event.key` を表の名前へ寄せる（` ` → `Space`、1 文字は大文字）。 */
export function name(key: string): string {
  switch (key) {
    case ' ':
      return 'Space';
    case 'ArrowUp':
    case 'ArrowDown':
    case 'ArrowLeft':
    case 'ArrowRight':
    case 'Enter':
    case 'Escape':
    case 'Tab':
    case 'Backspace':
    case 'Delete':
    case 'Home':
    case 'End':
    case 'PageUp':
    case 'PageDown':
      return key;
    default:
      return key.length === 1 ? key.toUpperCase() : key;
  }
}

/** キーの組を `Ctrl+Shift+ArrowUp` の形にする（第 1 の表記と同じ並び）。 */
export function chord(stroke: KeyStroke): string {
  const parts: string[] = [];
  if (stroke.ctrl) {
    parts.push('Ctrl');
  }
  if (stroke.shift) {
    parts.push('Shift');
  }
  if (stroke.alt) {
    parts.push('Alt');
  }
  parts.push(name(stroke.key));
  return parts.join('+');
}

/**
 * ショートカット表（§9.8.1〜§9.8.3）。
 *
 * 値は `REF:src/app/commands.rs` の**コマンド名**。W2 で使うものだけを置き、残りは
 * ワークスペースを作るときに足す（`webui/src/lib/coverage.ts` の `pending` に載っている）。
 */
export const SHORTCUTS: Readonly<Record<string, string>> = {
  // §9.8.1 グローバル
  'Ctrl+P': 'palette',
  'Ctrl+S': 'save',
  'Ctrl+O': 'open',
  'Ctrl+F': 'find',
  'Ctrl+H': 'replace',
  'Ctrl+Shift+F': 'search',
  'F3': 'find-next',
  'Shift+F3': 'find-previous',
  'F5': 'reload',
  'Ctrl+Shift+E': 'review',
  'Ctrl+Shift+I': 'capture',
  'Ctrl+Alt+Shift+I': 'capture', // ブラウザが Ctrl+Shift+I を奪う環境の代替（§9.8.6）
  'Ctrl+D': 'insert-mode',
  'Ctrl+1': 'focus-context',
  'Ctrl+2': 'focus-editor',
  'Ctrl+3': 'focus-reference',
  'Alt+0': 'previous-workspace',
  'Ctrl+\\': 'ui:toggle-reference',
  'Ctrl+Shift+\\': 'swap-reference',
  'Ctrl+Alt+L': 'language',
  'Ctrl+Alt+T': 'theme',
  'Alt+H': 'inlay-hints',
  'Ctrl+Alt+S': 'ui:status-stats',
  'Alt+Shift+ArrowUp': 'swap-section-up',
  'Alt+Shift+ArrowDown': 'swap-section-down',
  'Ctrl+Shift+S': 'save-search',
  'Ctrl+Shift+Enter': 'return-to-stock',
  'Ctrl+Shift+A': 'extract-addresses',
  // §9.8.2 構造移動
  'Alt+ArrowUp': 'previous-section',
  'Alt+ArrowDown': 'next-section',
  'Alt+ArrowLeft': 'previous-scene',
  'Alt+ArrowRight': 'next-scene',
  'Ctrl+Alt+ArrowLeft': 'previous-file',
  'Ctrl+Alt+ArrowRight': 'next-file',
  'Ctrl+Home': 'file-start',
  'Ctrl+End': 'file-end',
  'Ctrl+Tab': 'recent-next',
  'Ctrl+Shift+Tab': 'recent-previous',
  'Ctrl+B': 'up',
  'Ctrl+Shift+[': 'fold',
  'Ctrl+Shift+]': 'unfold',
  // §9.8.3 編集
  'Ctrl+Enter': 'confirm-next',
  'Ctrl+;': 'new-section',
  'Ctrl+Shift+;': 'delete-section',
  'Ctrl+Space': 'complete',
  'Ctrl+T': 'register-term',
  'Ctrl+Shift+T': 'register-character',
  'Ctrl+Alt+Shift+T': 'register-character', // `Ctrl+Shift+T` を奪う環境の代替（§9.8.6）
  'Ctrl+M': 'material-search',
  'Ctrl+.': 'quick-fix',
  'Ctrl+Shift+M': 'modes',
  'Ctrl+Shift+W': 'term',
  'Ctrl+Alt+Shift+W': 'term', // `Ctrl+Shift+W` を奪う環境の代替（§9.8.6）
  'Ctrl+Shift+Space': 'space-memo',
  'Ctrl+Shift+X': 'stop-all',
  'Ctrl+Shift+D': 'diff',
  'Ctrl+Alt+Shift+Left': 'ratio-main-left',
  'Ctrl+Alt+Shift+Right': 'ratio-main-right',
  'Ctrl+Alt+ArrowUp': 'ratio-context-up',
  'Ctrl+Alt+ArrowDown': 'ratio-context-down',
  // パレットからも引けるコマンド（`>` 経由。§9.7.1）
  'Ctrl+G': 'jump',
  'Alt+1': 'workspace-write',
  'Alt+2': 'workspace-episode',
  'Alt+3': 'workspace-outline',
  'Alt+4': 'workspace-idea',
  'Alt+5': 'workspace-material',
  'Alt+6': 'workspace-cast',
  'Alt+7': 'workspace-reader',
  'Alt+8': 'workspace-agent',
  'Alt+9': 'workspace-review',
};

/** `?` を押したとき（入力欄の外）。 */
export const HELP = '?';

/**
 * 画面側だけで完結する操作（`session.command` へ渡さない）。
 *
 * `REF:src/app/commands.rs` の表に**無い**ものは、`session.command` が `usage` エラーに
 * するので `ui:` を付けて区別する（`webui/tests/keys.test.mjs` が、`ui:` 以外の名前が
 * すべて表に載っていることを確かめる）。
 *
 * - `ui:help` … `?`（§9.8.1 の「入力欄外」）。表に `help` はあるが、パレット用の
 *   `Command::Help` とは別物（オーバーレイを出すのは画面の仕事）。
 * - `ui:palette` … `Ctrl+P`。表の `palette` は機能核のパレットで、画面の重ね物は画面が開く。
 * - `ui:close` … `Esc`（重ね物を閉じる → 無ければ節エディタへ。§9.7.5）。
 */
export const LOCAL: Readonly<Record<string, string>> = {
  '?': 'ui:help',
  'Ctrl+P': 'ui:palette',
  Escape: 'ui:close',
};

/** コマンド名からキーの表示を得る（`?` の一覧とボタンのツールチップに使う）。 */
export function shortcutFor(command: string): string {
  for (const [key, found] of Object.entries(SHORTCUTS)) {
    if (found === command) {
      return key;
    }
  }
  return '';
}

/** 押されたキーをコマンド名へ写す（写せなければ `null`）。 */
export function command(stroke: KeyStroke, options: KeyOptions): string | null {
  // 画面だけで完結する 3 つ（`?` / `Ctrl+P` / `Esc`）は、修飾キーの読み替えより先に見る。
  // `?` は配列によって `Shift+/` として届くので、シフトは無視する（`?` 以外の Shift 付きは
  // 表にそのまま載っている）。`Ctrl+Shift+P` は提案モードのトグルで、`Ctrl+P` とは別。
  if (!stroke.alt && !stroke.ctrl) {
    if (stroke.key === '?' || stroke.key === 'Escape') {
      return LOCAL[stroke.key] ?? null;
    }
  }
  if (!stroke.alt) {
    const local = LOCAL[chord(stroke)];
    if (local) {
      return local;
    }
  }
  let found = stroke;
  // macOS では `Option` が文字入力に使われるので `Alt` を `Ctrl+Alt` に読み替える（§9.8.4）
  if (options.mac && stroke.alt && !stroke.ctrl) {
    found = { ...stroke, ctrl: true };
  }
  if (found.alt && !found.ctrl && found.shift) {
    // `Alt+Shift+…` は表にそのまま載っている（節の入れ替え）ので、読み替えを使わない
    return SHORTCUTS[chord(stroke)] ?? null;
  }
  return SHORTCUTS[chord(found)] ?? null;
}

/**
 * 画面側だけで完結する操作（`session.command` に渡さない）。
 *
 * `REF:src/app/commands.rs` の表に**無い**ものは、`session.command` が `usage` エラーに
 * するので `ui:` を付けて区別する（`webui/tests/keys.test.mjs` が、`ui:` 以外の名前が
 * すべて表に載っていることを確かめる）。
 */
export function isLocal(command: string): boolean {
  return command.startsWith('ui:');
}

/**
 * 数字のワークスペース切替（`Alt+1..9`）の引数（§10.0）。
 *
 * `Alt+1` は `workspace-write`（`REF:src/app/commands.rs` の表）なので、**引数を足さない**
 * （`session.command` は表のコマンドをそのまま実行する）。`Ctrl+Shift+1..9`（節を持ち越す
 * 切替）は表に無いので、こちらで `session.workspace_switch` へ回す。
 */
export function keepPayload(name: string): Record<string, unknown> | null {
  const found = /^workspace-keep-([1-9])$/.exec(name);
  return found ? { workspace: Number(found[1]) } : null;
}
