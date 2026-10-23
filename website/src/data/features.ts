export interface FeatureItem {
  id: string;
  title: {
    ja: string;
    en: string;
  };
  summary: {
    ja: string;
    en: string;
  };
  nonEngineerSummary: {
    ja: string;
    en: string;
  };
  engineerSummary: {
    ja: string;
    en: string;
  };
  icon: string;
}

// 機能の説明は docs/specifications/ と README.md に合わせる。数を書くときは src/data/site.ts も参照。

export const featuresData: FeatureItem[] = [
  {
    id: "structured-writing",
    title: {
      ja: "1. 構造化された執筆（章・話・場面・節）",
      en: "1. Structured Writing (Chapter/Episode/Scene/Section)",
    },
    summary: {
      ja: "長大な物語を「章・話・場面・節」という明確な単位に細分化し、階層構造のまま執筆・移動できます。",
      en: "Break down long novels into Chapters, Episodes, Scenes, and Sections for seamless navigation.",
    },
    nonEngineerSummary: {
      ja: "「01-04-02-03」という番号で1章4話2場面3節へ瞬時にジャンプ。脳内のワーキングメモリを節（400字程度）単位に集中させ、全体像を見失わずに長編を書き進められます。",
      en: "Jump instantly using SceneId like '01-04-02-03'. Focus working memory on manageable ~400 word section chunks without losing sight of the epic narrative structure.",
    },
    engineerSummary: {
      ja: "SceneId (A-B-C-D) ASTによる階層データモデル。`00`予約番号（未分類ストック章・章あらすじ）や閉区間範囲式 (`@01-04 .. @02-01`) を完全決定論的にパースします。",
      en: "Hierarchical data model based on SceneId (A-B-C-D) AST. Deterministically parses reserved number '00' (stock chapters / blurbs) and interval range expressions.",
    },
    icon: "layers",
  },
  {
    id: "plot-prose-sync",
    title: {
      ja: "2. プロットと本文の完全連動",
      en: "2. Plot & Prose Synchronization",
    },
    summary: {
      ja: "執筆中の節に対応するプロット（あらすじ・狙い・チェック項目）が右側パネルに自動表示されます。",
      en: "Automatically displays matching scene/section outline summaries and checklists right next to your editor.",
    },
    nonEngineerSummary: {
      ja: "「この場面で何を書くべきか」を確認しながら執筆できるため、物語の軸がブレません。プロットを別ウィンドウで探す手間が完全にゼロになります。",
      en: "Keep writing without straying from your outline. Eliminates the friction of toggling back and forth between outline files.",
    },
    engineerSummary: {
      ja: "A形式（全体・章プロット）およびB形式（話・場面プロット）とC形式（本文）がSceneIdで1:1/1:Nにマッピングされ、80msデバウンスで右参照ペインへリアクティブ同期します。",
      en: "Maps A-format/B-format outline models with C-format prose by SceneId, reactivity synced to the Reference Pane via 80ms debounced state pipeline.",
    },
    icon: "sync",
  },
  {
    id: "auto-reference",
    title: {
      ja: "3. 設定資料の自動参照 & キャラモード",
      en: "3. Automatic Character & World Context Popups",
    },
    summary: {
      ja: "カーソル位置のキャラ名・用語を識別し、性格・口調・役割語パターン・世界観設定を右ペインに自動提示します。",
      en: "Detects characters and terms at the cursor position and renders personality, tone, role language, and lore.",
    },
    nonEngineerSummary: {
      ja: "シングルクオテーション `'` を打つだけで用語やキャラ名が予測補完されます。物語の展開に応じて変化する「キャラの性格モード（A:日常 / B:戦闘）」も節ごとに切り替え可能です。",
      en: "Type `'` for instant autocomplete of character names and lore terms. Character modes (e.g. A: Normal, B: Combat) adapt per section.",
    },
    engineerSummary: {
      ja: "Aho-Corasickとトライ木による最短一致辞書エンジン。`characters.md` の役割語パターン小言語をRegexSetへ事前コンパイルし、台詞のみに緑下線をインクリメンタル引画します。",
      en: "Shortest-match trie dictionary engine powered by Aho-Corasick. Role language pattern sub-language pre-compiled into RegexSet for incremental dialog underlines.",
    },
    icon: "book",
  },
  {
    id: "centralized-materials",
    title: {
      ja: "4. 資料の一元管理と引用追跡",
      en: "4. Centralized Material Index & Citation Tracking",
    },
    summary: {
      ja: "取材メモ・参考文献・Web検索結果を `M0137` / `S0042` としてID採番し、本文やプロットから参照・引用追跡できます。",
      en: "Assigns IDs (M0137/S0042) to research notes, citations, and search snapshots, traceable directly from prose.",
    },
    nonEngineerSummary: {
      ja: "行末に `^M0137` と注釈を付けるだけで「どの資料に基づいて書いたか」が一目瞭然。プロジェクト内検索の結果をそのまま1件の資料として保存する機能も備えています。",
      en: "Append `^M0137` to prose lines to track underlying research sources. Freeze and save search results as standalone research snapshot files.",
    },
    engineerSummary: {
      ja: "`materials/index.json` による単調増加ID管理。LSPインレイヒント機能により `'M0137` の直後へタイトルを薄く透過表示。未引用資料の診断検出 (I054) も完備。",
      en: "Monotonically increasing ID management in `materials/index.json`. Uses LSP inlay hints to project material titles without altering raw text files.",
    },
    icon: "folder",
  },
  {
    id: "ai-agent-workflow",
    title: {
      ja: "5. 人間が手綱を握る段階的AIエージェントシステム",
      en: "5. Human-Controlled Gradual AI Agent Workflow",
    },
    summary: {
      ja: "プロット・本文・アイデア・資料検索・文体・論理整合性・統合の7役を独立タスク化。自律度（委譲レベル）を人間が自在にコントロールできます。",
      en: "Modular AI tasks across 7 distinct roles. Humans set the exact delegation level (off / propose / apply / autonomous).",
    },
    nonEngineerSummary: {
      ja: "「アイデア出しだけ任せて本文は自分で書く」「文章のチェックだけ頼む」など自由自在。AIに勝手に文章を書き換えられる心配がなく、いつでも差し戻せます。",
      en: "Flexibly delegate tasks: generate ideas while writing prose yourself, or request style reviews. Your human editorial vision always retains full authority.",
    },
    engineerSummary: {
      ja: "MCP (Model Context Protocol) 準拠。`prompts/` テンプレートに基づく決定論的ContextPacket生成（文字数予算バジェット付き）と `agents/proposals/` 監査ログ管理。",
      en: "Fully MCP (Model Context Protocol) compliant. Generates deterministic ContextPackets with character budgets and stores proposal audits in `agents/proposals/`.",
    },
    icon: "cpu",
  },
  {
    id: "keyboard-first-ui",
    title: {
      ja: "6. キーボードファースト & 3ペインシェル",
      en: "6. Keyboard-First & 3-Pane Ergonomic Shell",
    },
    summary: {
      ja: "マウスを使わず、全操作をキーボードショートカットで完結。不要なラベルを排除したシンプルで美しいダークインターフェース。",
      en: "Complete keyboard navigation without mouse reliance. Minimalist, label-free dark UI designed for uninterrupted focus.",
    },
    nonEngineerSummary: {
      ja: "`Ctrl+G` で節ジャンプ、`Ctrl+B` で本文と話プロットを往復、`Ctrl+1/2/3` でペイン切替。画面から無駄なボタンやメニューラベルが消え、文章だけに没頭できます。",
      en: "Jump via `Ctrl+G`, flip between prose and the episode plot with `Ctrl+B`, switch panes with `Ctrl+1/2/3`. Eliminates UI clutter so you can immerse in prose.",
    },
    engineerSummary: {
      ja: "1フレーム8ms以内の描画性能目標。`Alt+1..9` ワークスペース切替、Command Palette (`Ctrl+P`)、IME変換中の入力状態機械の完全制御 (Composition Events) を搭載。",
      en: "Target 8ms frame budgets. Supports `Alt+1..9` workspace tabs, Command Palette (`Ctrl+P`), and strict IME state machine handling.",
    },
    icon: "keyboard",
  },
  {
    id: "text-based-data",
    title: {
      ja: "7. 完全テキストデータ & プロジェクトの自己完結性",
      en: "7. Plain-Text File Format & Project Self-Containment",
    },
    summary: {
      ja: "設定も本文もプロンプトもすべてMarkdown・Plain Text・JSONで保存。コピーするだけでプロジェクトが丸ごと移動可能です。",
      en: "All manuscripts, lore, prompts, and settings saved in readable Markdown/JSON files. Simple file copy moves the entire project.",
    },
    nonEngineerSummary: {
      ja: "専用の隠しデータベースにデータが閉じ込められることがありません。メモ帳や他のテキストエディタでも原稿を開いて編集でき、`F5` キーで最新状態に同期できます。",
      en: "No proprietary database vendor lock-in. Edit files in any text editor, and hit `F5` in NovNow to sync external changes seamlessly.",
    },
    engineerSummary: {
      ja: "規則10（プロジェクト外アクセス禁止）の徹底。`.gitignore` 適合な `.novnow/` 局所キャッシュ分離、および Git バージョン管理・GitHub Actions 自動デプロイと高い親和性。",
      en: "Strict Project Isolation Rule 10. Separates local caches in `.novnow/` (`.gitignore` friendly) for effortless Git & CI workflow integration.",
    },
    icon: "file-text",
  },
  {
    id: "layered-architecture",
    title: {
      ja: "8. クリーンなレイヤードアーキテクチャ & 言語サーバ",
      en: "8. Layered Architecture & Language Server",
    },
    summary: {
      ja: "小説執筆ロジック (`core`) と描画UI/ストレージ (`ui`, `adapters`) を厳格に分離したRust堅牢設計。",
      en: "Strict architectural decoupling between domain novel logic (`core`) and UI rendering/storage adapters.",
    },
    nonEngineerSummary: {
      ja: "アプリが軽快かつ安定して動作し、万が一のクラッシュ時にも原稿データが壊れない強固な設計で作られています。",
      en: "Ensures lighting-fast performance and crash resiliency, safeguarding your hard-earned manuscript from data corruption.",
    },
    engineerSummary: {
      ja: "`src/core/` は純粋関数群。`tests/layering.rs` が CI で UI/I/O 依存の混入を機械検証。`semantic_tokens`・`hover`・`completion` などの 12 機能は `novnow lsp` の Language Server としてそのまま公開されています。",
      en: "Pure functional `src/core/`. Enforced via `tests/layering.rs` CI linting. Its 12 language features are exposed as a real language server via `novnow lsp`.",
    },
    icon: "shield",
  },
];
