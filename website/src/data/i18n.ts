// Navigation and UI translation labels

export const translations = {
  ja: {
    nav: {
      home: "トップ",
      guide: "機能ガイド",
      api: "API リファレンス",
      troubleshooting: "トラブルシューティング",
      github: "GitHub",
    },
    hero: {
      badge: "長編小説・シナリオ執筆専用 IDE",
      title: "あなたの世界を、構造的かつ効率的に描く。",
      description: "プロットと本文の完全連動、設定の自動ポップアップ参照、人間が手綱を握る段階的AIエージェントワークフロー。作家とエンジニアのためのモダンな執筆環境。",
      ctaGuide: "ガイドを見る",
      ctaGitHub: "GitHubを見る",
      ctaStatus: "準備中（近日公開予定）",
    },
    community: {
      title: "コミュニティ・貢献・スポンサー",
      description: "NovNowはオープンソースプロジェクトとして開発されています。バグ報告、機能提案、プルリクエスト、スポンサーシップを心より歓迎します。",
      contributeTitle: "コントリビュート",
      contributeDesc: "Rust (egui / core / ports / adapters) または Astro / TypeScript によるWeb開発に興味がある方は、GitHubのリポジトリへお越しください。",
      sponsorTitle: "スポンサーシップ",
      sponsorDesc: "作家のための最高の創作環境を維持・発展させるため、開発活動へのスポンサー支援を募集しています。",
      githubBtn: "GitHubで貢献する",
    },
    credits: {
      text: "NovNow - Created by",
      author: "sonneko",
      rights: "All rights reserved.",
    },
    installNotice: {
      title: "インストール・導入方法について",
      status: "準備中（ベータ版公開をお待ちください）",
      desc: "デスクトップ版（Windows / macOS / Linux）および Web版の正式リリース準備を行っています。インストール手順およびバイナリ配布情報は順次公開されます。",
    }
  },
  en: {
    nav: {
      home: "Home",
      guide: "Feature Guide",
      api: "API reference",
      troubleshooting: "Troubleshooting",
      github: "GitHub",
    },
    hero: {
      badge: "Dedicated IDE for Long Novels & Scenarios",
      title: "Craft Your World with Structure and Efficiency.",
      description: "Seamless plot-prose synchronization, automatic character reference popups, and human-guided AI agent workflows. A modern writing IDE for authors and engineers.",
      ctaGuide: "Read Guide",
      ctaGitHub: "View GitHub",
      ctaStatus: "Under Construction (Coming Soon)",
    },
    community: {
      title: "Community, Contribute & Sponsor",
      description: "NovNow is developed as an open-source project. Bug reports, feature suggestions, pull requests, and sponsorships are warmly welcome.",
      contributeTitle: "Contribute",
      contributeDesc: "Interested in Rust (egui / core / ports / adapters) or Astro / TypeScript web development? Join us on our GitHub repository.",
      sponsorTitle: "Sponsorship",
      sponsorDesc: "We welcome sponsorships to sustain and accelerate the development of the ultimate writing IDE for authors.",
      githubBtn: "Contribute on GitHub",
    },
    credits: {
      text: "NovNow - Created by",
      author: "sonneko",
      rights: "All rights reserved.",
    },
    installNotice: {
      title: "Installation & Setup Guide",
      status: "Under Construction (Awaiting Beta Release)",
      desc: "Desktop builds (Windows / macOS / Linux) and the official Web version are currently in preparation. Installation commands and binaries will be published upon release.",
    }
  }
} as const;

export type Language = "ja" | "en";
