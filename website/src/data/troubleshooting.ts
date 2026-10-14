export interface TroubleshootingItem {
  id: string;
  category: {
    ja: string;
    en: string;
  };
  title: {
    ja: string;
    en: string;
  };
  problem: {
    ja: string;
    en: string;
  };
  cause: {
    ja: string;
    en: string;
  };
  solution: {
    ja: string;
    en: string;
  };
  code?: string;
}

// TODO: インストール方法の準備中 - リリース後のビルド手順、インストーラに関するトラブルシューティングは順次追加する

export const troubleshootingData: TroubleshootingItem[] = [
  {
    id: "startup-missing-files",
    category: {
      ja: "起動・プロジェクト構造",
      en: "Startup & Project Structure",
    },
    title: {
      ja: "起動時に「このディレクトリは novnow プロジェクトではありません」とエラーが出る",
      en: "Error: 'This directory is not a novnow project' on startup",
    },
    problem: {
      ja: "コマンドラインまたはフォルダ選択でプロジェクトを開こうとすると、画面が起動せずに終了メッセージが表示される。",
      en: "When attempting to open a directory, the GUI does not launch and prints missing file errors to terminal.",
    },
    cause: {
      ja: "NovNowの必須ファイル（`project.json`, `characters.md`, `words.md`, `materials/index.json`, `plots/all.txt` など）のいずれかが不足しています。",
      en: "One or more required project files (e.g., `project.json`, `characters.md`, `words.md`, `materials/index.json`, `plots/all.txt`) are missing.",
    },
    solution: {
      ja: "対象フォルダ内で `novnow new .` を実行して必須ファイルを自動生成するか、新規プロジェクト作成メニューからフォルダを開き直してください。",
      en: "Run `novnow new .` inside the directory to generate boilerplate project files, or create a project using the starter modal.",
    },
    code: `novnow: このディレクトリは novnow プロジェクトではありません
  不足しているもの:
  - project.json
  - characters.md
  - plots/all.txt`,
  },
  {
    id: "legacy-v0-project",
    category: {
      ja: "バージョン移行",
      en: "Migration",
    },
    title: {
      ja: "v0形式のプロジェクト（characters.json）が開けない",
      en: "Cannot open legacy v0 projects (characters.json detected)",
    },
    problem: {
      ja: "旧バージョン (v0) のプロジェクトフォルダを開こうとするとエラーになり起動しない。",
      en: "Opening a v0 directory results in a legacy format error.",
    },
    cause: {
      ja: "NovNow v1 は `characters.json` ではなく `characters.md` を使用する新しいファイル形式を採用しています。",
      en: "NovNow v1 uses `characters.md` instead of legacy `characters.json`.",
    },
    solution: {
      ja: "ターミナルで `novnow migrate <プロジェクトパス>` を実行してください。既存データを無損失で自動変換・バックアップ作成のうえ移行します。",
      en: "Run `novnow migrate <project-path>` in your terminal. Automatically converts project files with full backup.",
    },
    code: `novnow migrate ./my-novel`,
  },
  {
    id: "character-mode-error",
    category: {
      ja: "構文・診断エラー",
      en: "Diagnostics & Syntax",
    },
    title: {
      ja: "キャラのモード設定で赤波線（E003 / E004）が表示される",
      en: "Red wavy line under character mode (E003 / E004)",
    },
    problem: {
      ja: "話プロットの `mode:` フィールドや本文の注釈 `@リリア:C` にエラー波線が引かれる。",
      en: "Error wavy lines appear under `mode:` fields or speaker annotation `@Character:Mode`.",
    },
    cause: {
      ja: "`E003`: 指定したモードIDが `characters.md` に未定義、`E004`: モードの有効範囲 (`range: @01 .. @02`) 外の節で割り当てられています。",
      en: "`E003`: The mode ID is not defined in `characters.md`. `E004`: Assigned outside the mode's valid SceneId `range`.",
    },
    solution: {
      ja: "クイックフィックス (`Ctrl+.` または電球アイコン) を押し、`characters.md` の `range:` を拡張するか、既存のモード名へ置換してください。",
      en: "Open quick fix (`Ctrl+.`) to expand the `range:` in `characters.md` or replace with an existing defined mode.",
    },
  },
  {
    id: "mcp-server-connection",
    category: {
      ja: "AIエージェント・MCP",
      en: "AI Agent & MCP",
    },
    title: {
      ja: "AIエージェント実行時に E030 / E063 エラーが発生する",
      en: "E030 / E063 error when executing AI tasks",
    },
    problem: {
      ja: "プロット生成や本文執筆タスクを実行すると「環境変数が設定されていません」「MCPに接続できません」と表示される。",
      en: "AI task execution fails with missing environment variables or unreachable MCP server messages.",
    },
    cause: {
      ja: "`agents/mcp.json` 内で指定された環境変数 (`NOVNOW_LLM_KEY` 等) が未設定であるか、ローカルLLMサーバが起動していません。",
      en: "Required environment variables in `agents/mcp.json` are missing, or the local LLM endpoint is offline.",
    },
    solution: {
      ja: "環境変数を設定してアプリを再起動するか、`agent` ワークスペースのローカルLLM設定で `http://localhost:11434` (Ollama等) の接続確認を行ってください。",
      en: "Set the required env variables and restart, or check the Local LLM settings tab in the `agent` workspace (`http://localhost:11434`).",
    },
  },
  {
    id: "web-storage-indexeddb",
    category: {
      ja: "Web版・ストレージ",
      en: "Web Version & Storage",
    },
    title: {
      ja: "Web版で「この環境ではデータを保存できません」と表示される",
      en: "Web Version: 'Storage is unavailable in this environment'",
    },
    problem: {
      ja: "ブラウザでWeb版NovNowを開いた際、全画面メッセージが表示されて起動できない。",
      en: "Opening the web version displays a full-screen storage error preventing editor launch.",
    },
    cause: {
      ja: "プライベートブラウジングモードやブラウザ設定によって IndexedDB へのアクセスが遮断されています。",
      en: "IndexedDB access is disabled due to Private/Incognito browsing or browser security settings.",
    },
    solution: {
      ja: "通常ウィンドウで開き直すか、サードパーティCookie/ストレージ制限を解除してください。またはデスクトップ版をご利用ください。",
      en: "Open NovNow in a standard browsing window, allow storage access, or use the Desktop version.",
    },
  },
  {
    id: "file-encoding-utf8",
    category: {
      ja: "ファイル文字コード",
      en: "Encoding & UTF-8",
    },
    title: {
      ja: "外部で編集したファイルの文字が化ける / 読み込めない",
      en: "Garbled characters or encoding errors after external editing",
    },
    problem: {
      ja: "`ProjectError::Encoding` が発生し、プロジェクトの一部ファイルが開けない。",
      en: "`ProjectError::Encoding` error prevents opening modified project files.",
    },
    cause: {
      ja: "外部のテキストエディタで保存した際、Shift_JIS や UTF-16 などのUTF-8以外の文字コードで保存されています。",
      en: "An external text editor saved files in Shift_JIS or UTF-16 instead of UTF-8.",
    },
    solution: {
      ja: "VS Codeやテキストエディタで該当ファイルを開き、文字コードを「UTF-8 (BOMなし) / 改行LF」で保存し直して `F5` キーを押してください。",
      en: "Re-open the file in VS Code or any editor, save as UTF-8 (no BOM) with LF line endings, and press `F5` in NovNow.",
    },
  },
];
