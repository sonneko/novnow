// ヘルプ（`?`）とパレット（`Ctrl+P`）の中身（§9.7・§9.8）。
//
// 説明の文は `src/ui/i18n/ja.toml` の `help.*` をそのまま引く（§12.3。第 2 のための
// 文言集を持たない）。ここが持つのは「どのキーを、どの順で、どの説明に対応させるか」だけ。
//
// **この一覧は、画面がいま扱えるキーだけを載せる**（§21.1.2: 未提供を黙って隠さない）。
// 表に無いキー（例: `Ctrl+F`）は `SHORTCUTS` にはあるが、まだ動かないのでここには出さない。
// ワークスペースを作るときに 1 行ずつ足していく。

/** ヘルプの 1 行（キーの表示と、`help.*` の説明のキー）。 */
export interface HelpRow {
  /** キーの表示（`Alt+↑ / Alt+↓` のように複数でもよい）。 */
  keys: string;
  /** 説明（`t()` に渡すキー）。 */
  label: string;
}

/** ヘルプの見出し 1 つ。 */
export interface HelpGroup {
  /** 見出し（`t()` に渡すキー）。 */
  title: string;
  rows: HelpRow[];
}

/**
 * `?` の一覧（§9.8 の節の並びに合わせる）。
 *
 * `help.*` に無い見出しは使わない（`section` / `scene` / `file` は 1 行にまとめている）。
 */
export const HELP_GROUPS: readonly HelpGroup[] = [
  {
    title: 'help.workspace',
    rows: [
      { keys: 'Alt+1 … Alt+9', label: 'help.workspace' },
      { keys: 'Alt+0', label: 'help.workspace' },
      { keys: 'Ctrl+Shift+1 … Ctrl+Shift+9', label: 'help.workspace' },
      { keys: 'Ctrl+\\', label: 'help.reference' },
      { keys: 'Ctrl+Shift+\\', label: 'help.swap' },
      { keys: 'Ctrl+Alt+L', label: 'help.language' },
      { keys: 'Ctrl+Alt+T', label: 'help.theme' },
      { keys: 'Alt+H', label: 'help.inlay_hints' },
      { keys: 'F5', label: 'help.reload' },
      { keys: 'Ctrl+S', label: 'help.save' },
      { keys: 'Ctrl+P', label: 'help.palette' },
      { keys: 'Esc', label: 'help.escape' },
    ],
  },
  {
    title: 'help.section',
    rows: [
      { keys: 'Alt+↑ / Alt+↓', label: 'help.section' },
      { keys: 'Alt+← / Alt+→', label: 'help.scene' },
      { keys: 'Ctrl+Alt+← / Ctrl+Alt+→', label: 'help.file' },
      { keys: 'Ctrl+Home / Ctrl+End', label: 'help.section' },
      { keys: 'Ctrl+Enter', label: 'help.confirm_next' },
      { keys: 'Ctrl+;', label: 'help.new_section' },
      { keys: 'Ctrl+Shift+;', label: 'help.delete_section' },
      { keys: 'Ctrl+Tab / Ctrl+Shift+Tab', label: 'help.recent' },
      { keys: 'Ctrl+B', label: 'help.up' },
    ],
  },
  {
    title: 'help.focus',
    rows: [
      { keys: 'Ctrl+1 / Ctrl+2 / Ctrl+3', label: 'help.focus' },
      { keys: 'Ctrl+Alt+↑ / Ctrl+Alt+↓', label: 'help.context_ratio' },
      { keys: 'Ctrl+Alt+Shift+← / →', label: 'help.main_ratio' },
    ],
  },
];

/** 一覧に載っている行の数（テストが「動くキーが全部載っているか」を見るのに使う）。 */
export const HELP_KEYS: readonly string[] = HELP_GROUPS.flatMap((group) =>
  group.rows.map((row) => row.label),
);
