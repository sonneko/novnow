// サイトで使う「実装の事実」。数を直書きせず、ここ 1 か所を直せば全ページに反映されるようにする。
// 値は README.md / docs/specifications/ と一致させること（`docs/script/reference.py` の検査 7 が
// README 側の数の主張を見張っている）。

export interface Bilingual {
  ja: string;
  en: string;
}

export interface WorkspaceRow {
  key: string;
  id: string;
  name: Bilingual;
  purpose: Bilingual;
}

/** `Alt+1..9` で切り替える 9 ワークスペース（docs/specifications/ui.md §10）。 */
export const workspaces: WorkspaceRow[] = [
  {
    key: "Alt+1",
    id: "write",
    name: { ja: "本文 / Write", en: "Manuscript / Write" },
    purpose: {
      ja: "節エディタ・文脈ビュー・参照ペイン。執筆の中心",
      en: "Section editor, context view, and reference pane — the heart of writing",
    },
  },
  {
    key: "Alt+2",
    id: "plot-episode",
    name: { ja: "話プロット / Episode", en: "Episode plot" },
    purpose: {
      ja: "話ごとのプロット（B 形式）。本文と同じ節に着地して往復できる",
      en: "Per-episode plot (format B), pinned to the same section as the prose",
    },
  },
  {
    key: "Alt+3",
    id: "plot-outline",
    name: { ja: "構成 / Outline", en: "Outline" },
    purpose: {
      ja: "全体・章プロット（A 形式）と章→話の構成ツリー",
      en: "Whole-work and chapter plots (format A) with a chapter → episode tree",
    },
  },
  {
    key: "Alt+4",
    id: "idea",
    name: { ja: "アイデア / Ideas", en: "Ideas" },
    purpose: {
      ja: "思いつきの保管と、プロット・用語・キャラへの昇格",
      en: "Capture ideas and promote them to plots, terms, or characters",
    },
  },
  {
    key: "Alt+5",
    id: "material",
    name: { ja: "資料 / Materials", en: "Materials" },
    purpose: {
      ja: "資料の収集・審査・参照。`'M0137` の ID で本文から引く",
      en: "Collect, vet, and cite materials by ID such as `'M0137`",
    },
  },
  {
    key: "Alt+6",
    id: "cast",
    name: { ja: "配役 / Cast", en: "Cast" },
    purpose: {
      ja: "キャラクター・モード・役割語の管理",
      en: "Characters, their modes, and speech styles",
    },
  },
  {
    key: "Alt+7",
    id: "reader",
    name: { ja: "読者 / Reader", en: "Reader" },
    purpose: {
      ja: "縦書き・注釈なしの読み返し。執筆用の記号は表示しない",
      en: "Vertical, annotation-free reading view of the finished text",
    },
  },
  {
    key: "Alt+8",
    id: "agent",
    name: { ja: "エージェント / Agents", en: "Agents" },
    purpose: {
      ja: "AI の管制。役割ごとの委譲レベルと提案の差分レビュー",
      en: "AI control room: per-role delegation levels and proposal diffs",
    },
  },
  {
    key: "Alt+9",
    id: "review",
    name: { ja: "点検 / Review", en: "Review" },
    purpose: {
      ja: "診断の一覧・進捗・伏線の確認",
      en: "Full diagnostics, progress, and foreshadowing checks",
    },
  },
];

export interface InterfaceRow {
  name: string;
  how: string;
  what: Bilingual;
}

/** ヘッドレスに開かれている入口（docs/specifications/interfaces.md）。 */
export const interfaces: InterfaceRow[] = [
  {
    name: "CLI",
    how: "novnow new | check | export | import | migrate | api | mcp | lsp",
    what: {
      ja: "作成・診断・書き出し・取り込み・移行・API 一覧。`novnow <domain> <verb> <dir>` の統一書式",
      en: "Create, check, export, import, migrate, and list the API. Uniform `novnow <domain> <verb> <dir>` form",
    },
  },
  {
    name: "LSP",
    how: "novnow lsp <dir>",
    what: {
      ja: "補完・ホバー・定義移動・参照検索・診断・整形・折りたたみ（LSP 3.17）",
      en: "Completion, hover, definitions, references, diagnostics, formatting, folding (LSP 3.17)",
    },
  },
  {
    name: "MCP",
    how: "novnow mcp <dir>",
    what: {
      ja: "AI クライアント向けのツール群。提案は差分レビューを経て本文へ",
      en: "Tools for AI clients; proposals reach the manuscript only through diff review",
    },
  },
  {
    name: "Python",
    how: "import novnow",
    what: {
      ja: "GUI と同じ正規操作を Python から（PyO3・abi3-py39 ホイール）",
      en: "The same canonical operations from Python (PyO3, abi3-py39 wheel)",
    },
  },
  {
    name: "Rust",
    how: "novnow クレート",
    what: {
      ja: "`core` / `app` の API を直接利用。GUI と同じ 1 本の実装を共有",
      en: "Use the `core` / `app` APIs directly — the same single implementation as the GUI",
    },
  },
];

export interface PlatformRow {
  name: string;
  setup: Bilingual;
  bestFor: Bilingual;
  highlight?: boolean;
}

/**
 * 「どこで動くか」。ブラウザで動くことと、入口が複数あることが伝わっていなかったための表。
 * 文言は README.md の同じ表と揃える（README 側は `reference.py` の検査 7 が見張る）。
 */
export const platforms: PlatformRow[] = [
  {
    name: "Browser",
    setup: { ja: "`https://sonneko.github.io/novnow/app/` を開くだけ", en: "Just open `https://sonneko.github.io/novnow/app/`" },
    bestFor: { ja: "まず試したい・環境を変えたくない", en: "Trying it first, keeping your machine clean" },
    highlight: true,
  },
  {
    name: "Desktop",
    setup: { ja: "GitHub Releases", en: "GitHub Releases" },
    bestFor: { ja: "手元のフォルダで原稿を管理したい", en: "Managing manuscripts in a local folder" },
  },
  {
    name: "CLI",
    setup: { ja: "同上", en: "Same as above" },
    bestFor: { ja: "原稿をまとめて検査・書き出ししたい", en: "Checking and exporting a whole manuscript at once" },
  },
  {
    name: "LSP",
    setup: { ja: "同上", en: "Same as above" },
    bestFor: { ja: "VS Code など手持ちのエディタで書きたい", en: "Writing in VS Code or another editor" },
  },
  {
    name: "MCP",
    setup: { ja: "同上", en: "Same as above" },
    bestFor: { ja: "AI に作品を読ませたい", en: "Letting AI read and propose changes" },
  },
  {
    name: "Python",
    setup: { ja: "PyPI の `pip install novnow`", en: "`pip install novnow` from PyPI" },
    bestFor: { ja: "作業を自動化したい", en: "Automating your own workflow" },
  },
  {
    name: "VS Code",
    setup: { ja: "GitHub Releases の VSIX", en: "VSIX on GitHub Releases" },
    bestFor: { ja: "ふだん VS Code で書いている", en: "Writing in VS Code every day" },
  },
  {
    name: "GitHub Actions",
    setup: { ja: "`uses: sonneko/novnow@v0`", en: "`uses: sonneko/novnow@v0`" },
    bestFor: { ja: "点検や書き出しを CI で行いたい", en: "Checking or exporting in CI" },
  },
];

/** ユーザー向けガイドの入口（docs/users から生成したサイト内のページ）。 */
export const guideLinks = {
  start: "/ja/manual/start",
  index: "/ja/manual/",
  glossary: "/ja/manual/glossary",
  cheatsheet: "/ja/manual/cheatsheet",
  faq: "/ja/manual/faq",
  install: "/ja/manual/install",
};

/** 覚えておくと迷わないキー（docs/specifications/ui.md §9.9）。 */
export const keyBindings: { key: string; what: Bilingual }[] = [
  { key: "Alt+1..9", what: { ja: "ワークスペース切替（現在の節を維持）", en: "Switch workspace (keeps the current section)" } },
  { key: "Ctrl+P", what: { ja: "統一パレット（`@` 構造 / `'` 辞書 / `#` 資料 / `>` コマンド）", en: "Unified palette (`@` structure, `'` dictionary, `#` materials, `>` commands)" } },
  { key: "Ctrl+G", what: { ja: "番号でジャンプ（`010402` でも可）", en: "Jump by scene number (`010402` works too)" } },
  { key: "Ctrl+B", what: { ja: "対になるファイルへ（本文 ⇄ 話プロット）", en: "Toggle to the paired file (prose ⇄ episode plot)" } },
  { key: "Ctrl+Enter", what: { ja: "確定して次の節へ（無ければ作る）", en: "Commit and move to the next section (create one if needed)" } },
  { key: "Ctrl+.", what: { ja: "その場の診断にクイックフィックス", en: "Quick fix for the diagnostic at the cursor" } },
  { key: "Ctrl+Shift+E", what: { ja: "点検ワークスペースで全体を診断", en: "Run full-project diagnostics in the review workspace" } },
  { key: "F5", what: { ja: "プロジェクト再読込（外部変更の取り込み）", en: "Reload the project (pick up external edits)" } },
];
