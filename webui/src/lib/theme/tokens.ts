// このファイルは **生成物**。手で編集しない（`scripts/theme-ts.py` が書く）。
//
// 正本は `REF:docs/specifications/ui.md` §11.1・§11.2 の表。第 1 の画面実装は
// `REF:src/ui/theme.rs` の `palette()` が同じ表から作っている。第 2 の画面実装はこの
// 生成物だけを使い、コンポーネントに生の色を書かない（§11.1 MUST）。
//
// 変更するときは `python3 scripts/theme-ts.py` を実行する（`scripts/verify.sh` の
// `webui-theme` が `--check` で一致を強制する）。

/** 配色（§11.4: OS の設定が分からなければ dark）。 */
export type Mode = 'dark' | 'light';

/** §11.1 のセマンティックカラートークン。 */
export const COLORS: Record<Mode, Record<string, string>> = {
  dark: {
    'bg.base': '#15171B',
    'bg.raised': '#1D2026',
    'bg.inset': '#111317',
    'bg.selection': '#2D3A4D',
    'fg.primary': '#E6E6E3',
    'fg.muted': '#A6A8AD',
    'fg.faint': '#82878E',
    'border': '#2A2E35',
    'accent': '#6FA8FF',
    'accent.faint': '#354A6B',
    'state.unsaved': '#E8A33D',
    'state.error': '#E5484D',
    'state.warn': '#D8A200',
    'state.ok': '#49B86B',
    'scene.id': '#8FB8FF',
    'scene.title': '#A6A8AD',
    'ref.character': '#E7C96B',
    'ref.word': '#7FD1A7',
    'ref.material': '#9BB4D6',
    'ref.unresolved': '#E5484D',
    'modifier.mode': '#C792EA',
    'modifier.title': '#7E838A',
    'dialogue.text': '#E6E6E3',
    'speaker': '#E7C96B',
    'field.key': '#8C9CB8',
    'comment': '#60666E',
    'ruby.text': '#A6A8AD',
  },
  light: {
    'bg.base': '#FBFBF9',
    'bg.raised': '#FFFFFF',
    'bg.inset': '#F1F1EE',
    'bg.selection': '#D6E4F7',
    'fg.primary': '#1B1C1E',
    'fg.muted': '#55585E',
    'fg.faint': '#727479',
    'border': '#DEDED9',
    'accent': '#1D5FD0',
    'accent.faint': '#ADC4EB',
    'state.unsaved': '#B9720A',
    'state.error': '#C62A2F',
    'state.warn': '#8F6B00',
    'state.ok': '#1E7F43',
    'scene.id': '#1D5FD0',
    'scene.title': '#55585E',
    'ref.character': '#8A6A00',
    'ref.word': '#157A52',
    'ref.material': '#37567F',
    'ref.unresolved': '#C62A2F',
    'modifier.mode': '#7A3BA8',
    'modifier.title': '#6B6E73',
    'dialogue.text': '#1B1C1E',
    'speaker': '#8A6A00',
    'field.key': '#425777',
    'comment': '#8B8E93',
    'ruby.text': '#55585E',
  },
};

/** `:root` に置く CSS 変数（コンポーネントはこの名前だけを参照する）。 */
export const CSS_VARIABLES: Record<string, string> = {
  'bg.base': '--bg-base',
  'bg.raised': '--bg-raised',
  'bg.inset': '--bg-inset',
  'bg.selection': '--bg-selection',
  'fg.primary': '--fg-primary',
  'fg.muted': '--fg-muted',
  'fg.faint': '--fg-faint',
  'border': '--border',
  'accent': '--accent',
  'accent.faint': '--accent-faint',
  'state.unsaved': '--state-unsaved',
  'state.error': '--state-error',
  'state.warn': '--state-warn',
  'state.ok': '--state-ok',
  'scene.id': '--scene-id',
  'scene.title': '--scene-title',
  'ref.character': '--ref-character',
  'ref.word': '--ref-word',
  'ref.material': '--ref-material',
  'ref.unresolved': '--ref-unresolved',
  'modifier.mode': '--modifier-mode',
  'modifier.title': '--modifier-title',
  'dialogue.text': '--dialogue-text',
  'speaker': '--speaker',
  'field.key': '--field-key',
  'comment': '--comment',
  'ruby.text': '--ruby-text',
};

/** トークン名を CSS 変数の参照（`var(--bg-base)`）にする。 */
export function css(token: string): string {
  return `var(${CSS_VARIABLES[token] ?? token})`;
}

/** いまの配色を `<html>` の `data-theme` に反映する（§11.4）。 */
export function applyTheme(mode: Mode): void {
  const root = document.documentElement;
  root.dataset.theme = mode;
  for (const [token, name] of Object.entries(CSS_VARIABLES)) {
    const value = COLORS[mode][token];
    if (value) {
      root.style.setProperty(name, value);
    }
  }
  root.style.colorScheme = mode;
}

/** OS の設定から配色を決める（`system` のとき。§11.4）。 */
export function systemMode(): Mode {
  return typeof matchMedia === 'function' &&
    matchMedia('(prefers-color-scheme: light)').matches
    ? 'light'
    : 'dark';
}
