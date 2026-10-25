# novnow Web サイト設計

`REF:docs/specifications/website.md` の実装設計。Astro サイト (`REF:website/`) を対象とする。

## 21. Web サイト

### 21.1 トップページとコミュニティ導線

状態: 実装完了（REF:website/src/components/Overview.astro, REF:website/src/data/i18n.ts, REF:tests/website_community.py）

- トップページ共通コンポーネント `Overview.astro` にコミュニティ案内を置き、日英の文言は `data/i18n.ts` の `community` 翻訳を用いる。
- GitHub Issues をコントリビュートの入口に、GitHub Sponsors の作者ページをスポンサーの入口にする。
- `aria-labelledby` で案内の見出しと領域を関連づける。既存のピル型リンクスタイルを再利用し、追加の JavaScript や依存を導入しない。
- 両言語ページが同じ `Overview` を使用するため、一方だけの表示漏れを避ける。

### 21.2 検証

状態: 実装完了（REF:tests/website_community.py, REF:scripts/website-check.py）

`npm run build` で Astro の両言語ページ生成を確認し、生成済みページに案内文と外部リンクが含まれることを確認する。既存の website-check / リンク検査も実行する。

### 21.3 受け入れ基準

状態: 実装完了（REF:tests/website_community.py, REF:website/src/pages/ja/index.astro, REF:website/src/pages/en/index.astro）

- 両言語で同じコミュニティ案内を表示する。
- 見出しは支援技術から識別でき、GitHub の課題報告とスポンサー窓口へ遷移できる。
