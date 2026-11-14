// Navigation and UI translation labels
//
// 公開済みの版は `REF:website/src/data/release.ts` が正本。ここに書き写すと、同じページの中で
// Downloads の見出し（`novnow v<版>を公開しました`）と告知の版が食い違う（#61 追加監査 8:
// v0.1.0 と v0.1.2 が並んで出ていた）。告知は release.ts から導出する。
import { version } from "./release";

export const translations = {
  ja: {
    nav: {
      home: "トップ",
      guide: "機能一覧",
      manual: "ガイド",
      specs: "仕様書",
      api: "API リファレンス",
      troubleshooting: "トラブルシューティング",
      search: "検索",
      github: "GitHub",
    },
    // サイト内検索（仕様 §18.5.1）。{count} と {lang} はブラウザ側で埋める
    // （`data-labels` に JSON で渡すので、ここに関数を置かない）。
    search: {
      title: "サイト内検索",
      description: "公開サイト全体を横断して検索します。索引はサイトを生成したときに作り、入力した語を外部のサービスへ送ることはありません。",
      lead: "ガイド・仕様書・診断一覧・API リファレンスを横断して検索します。索引はこのサイトを生成したときに作った静的な一覧だけで、入力した語がブラウザの外へ出ることはありません。",
      inputLabel: "検索する語",
      placeholder: "例: 保存 / 診断 / project.toml",
      loading: "索引を読み込んでいます…",
      ready: "検索できるページ: {count}",
      results: "{count} 件見つかりました",
      none: "見つかりませんでした。語を短くするか、別の言い方を試してください。",
      error: "索引（{lang}.json）を読み込めませんでした。公開サイトの生成物を確認してください。",
      hint: "矢印キーで結果を移動、Enter で開く、Esc で消します。/ または Ctrl+K でここへ移動します。",
      resultsLabel: "検索結果",
      noscript: "検索には JavaScript が必要です。目次からはガイド・仕様書・診断一覧・API リファレンスへ直接移動できます。",
    },
    hero: {
      badge: "長編小説・シナリオ執筆専用 IDE",
      title: "あなたの世界を、構造的かつ効率的に描く。",
      description: "プロットと本文の完全連動、設定の自動ポップアップ参照、人間が手綱を握る段階的AIエージェントワークフロー。作家とエンジニアのためのモダンな執筆環境。",
      ctaGuide: "ガイドを見る",
      ctaGitHub: "GitHubを見る",
      ctaStatus: "ソースからビルドできます",
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
      status: `v${version} 公開`,
      desc: "ブラウザ版はインストール不要で使えます。デスクトップ版（Windows / macOS / Linux）とブラウザ版（WebAssembly）は同じ画面・同じキー操作で、同じプロジェクトをそのまま開けます。公開バイナリは下のダウンロード一覧から取得できます。",
    }
  },
  en: {
    nav: {
      home: "Home",
      guide: "Features",
      manual: "Guide",
      specs: "Specifications",
      api: "API reference",
      troubleshooting: "Troubleshooting",
      search: "Search",
      github: "GitHub",
    },
    search: {
      title: "Site search",
      description: "Search across the published site. The index is built together with the site, and what you type is never sent to an external service.",
      lead: "Search the guide, the specifications, the diagnostics list and the API reference at once. The index is a static list built with this site; what you type never leaves your browser.",
      inputLabel: "Search terms",
      placeholder: "e.g. save / diagnostics / project.toml",
      loading: "Loading the index…",
      ready: "Pages in this index: {count}",
      results: "{count} results",
      none: "Nothing matched. Try a shorter term or different wording.",
      error: "Could not load the index ({lang}.json). Check the published site build.",
      hint: "Arrow keys move through the results, Enter opens one, Esc clears. Press / or Ctrl+K to jump here.",
      resultsLabel: "Search results",
      noscript: "Search needs JavaScript. Use the navigation to open the guide, specifications, diagnostics and API reference directly.",
    },
    hero: {
      badge: "Dedicated IDE for Long Novels & Scenarios",
      title: "Craft Your World with Structure and Efficiency.",
      description: "Seamless plot-prose synchronization, automatic character reference popups, and human-guided AI agent workflows. A modern writing IDE for authors and engineers.",
      ctaGuide: "Read Guide",
      ctaGitHub: "View GitHub",
      ctaStatus: "Build from source today",
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
      status: `v${version} released`,
      desc: "The browser build needs no installation. The desktop build (Windows / macOS / Linux) and browser build (WebAssembly) share the same layout, the same keys, and open the same project. Published binaries are available in the download section below.",
    }
  }
} as const;

export type Language = "ja" | "en";
