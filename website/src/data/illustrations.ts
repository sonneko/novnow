export interface IllustrationSpec {
  id: string;
  targetPath: string;
  title: {
    ja: string;
    en: string;
  };
  description: {
    ja: string;
    en: string;
  };
  recommendedDimensions: string;
  fileType: string;
}

export const illustrationSpecs: IllustrationSpec[] = [
  {
    id: "hero-ide",
    targetPath: "website/public/images/illustrations/hero-ide.svg",
    title: {
      ja: "トップページ ヒーロー領域 IDE 画面イメージ",
      en: "Top Page Hero Area IDE Screen Artwork",
    },
    description: {
      ja: "3ペイン構成（左上: ファイルツリー＆文脈ビュー / 左下: 1節専用エディタ / 右: 連動コンテキスト表示）を備えたNovNowの洗練されたUIのメインイラスト。",
      en: "Main illustration showing NovNow's polished 3-pane UI layout (File Tree & Context View, Section Editor, and Sync Reference Pane).",
    },
    recommendedDimensions: "1200 x 675 px (16:9)",
    fileType: "SVG or PNG (Dark Theme / Transparent)",
  },
  {
    id: "structured-tree",
    targetPath: "website/public/images/illustrations/structured-tree.svg",
    title: {
      ja: "章・話・場面・節 構造化ツリー概念図",
      en: "Chapter / Episode / Scene / Section Structure Diagram",
    },
    description: {
      ja: "物語が「章(A) - 話(B) - 場面(C) - 節(D)」に分解され、番号ジャンプ（例: 01-04-02-03）で一瞬で巡回できる概念を表すイラスト。",
      en: "Diagram illustrating how novels decompose into SceneId levels with instant numeric jumping.",
    },
    recommendedDimensions: "800 x 500 px",
    fileType: "SVG or PNG",
  },
  {
    id: "character-popups",
    targetPath: "website/public/images/illustrations/character-popups.svg",
    title: {
      ja: "キャラクター設定 & 用語ポップアップ連動図",
      en: "Character & Lore Auto-Popup Diagram",
    },
    description: {
      ja: "本文中のキャラ名や用語にカーソルを合わせると、右側に性格・口調・モード設定がリアルタイム表示される様子を表すイラスト。",
      en: "Artwork demonstrating real-time character lore, mood mode, and role language popups beside prose.",
    },
    recommendedDimensions: "800 x 500 px",
    fileType: "SVG or PNG",
  },
  {
    id: "ai-agents",
    targetPath: "website/public/images/illustrations/ai-agents.svg",
    title: {
      ja: "6エージェント 段階的委譲ワークフロー",
      en: "6-Agent Gradual Delegation Workflow Artwork",
    },
    description: {
      ja: "プロット・本文・アイデア・資料・文体・論理の6役割と人間編集者（Integrator）が協調し、人間が手綱を握る概念イラスト。",
      en: "Concept artwork depicting 6 specialized AI roles under the human author's executive command.",
    },
    recommendedDimensions: "800 x 500 px",
    fileType: "SVG or PNG",
  },
];
