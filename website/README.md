# novnow 公式サイト（Astro）

`novnow` の公式サイト。日本語（`/ja`）と英語（`/en`）の 2 言語で、トップ・利用ガイド・診断カタログ・API リファレンス・トラブルシューティングを配信する。

```bash
cd website
npm ci
npm run dev      # http://localhost:4321/novnow/（/novnow/en/ へリダイレクト）
npm run build    # ./dist へ静的出力
npm run preview  # ビルド結果をローカルで確認
```

## ページと生成物

| ページ | 実体 | 内容の正本 |
|---|---|---|
| `/ja` `/en` | `src/pages/<lang>/index.astro` | `src/data/features.ts`・`src/data/i18n.ts`・`src/data/site.ts` |
| `/<lang>/guide` | `src/pages/<lang>/guide.astro` | `src/data/features.ts` |
| `/<lang>/diagnostics` | `src/pages/<lang>/diagnostics.astro` | **`src/data/diagnostics.ts`（自動生成）** |
| `/<lang>/diagnostics/<code>` | `src/pages/[lang]/diagnostics/[code].astro` | 同上 + `diagnostics-examples.json` |
| `/<lang>/api` | `src/pages/<lang>/api.astro` | **`src/data/api.ts`（自動生成）** |
| `/<lang>/troubleshooting` | `src/pages/<lang>/troubleshooting.astro` | `src/data/troubleshooting.ts` |
| `/ja/manual` `/ja/manual/<slug>` | `src/pages/ja/manual/{index,[slug]}.astro` | **`docs/users/*.md`（自動生成）** |
| `/ja/specifications` `/ja/specifications/<name>` | `src/pages/ja/specifications/{index,[slug]}.astro` | **`docs/specifications/*.md`（自動生成）** |
| `/en/manual` `/en/specifications` | `src/pages/en/manual/[...slug].astro` ほか | 本文は日本語のみ。日本語版への案内を出す |

トップページの「ひと目で分かる novnow」「9 つのワークスペース」「キー一覧」「外部インターフェース」の各ブロックは
`src/components/Overview.astro` が描き、データは `src/data/site.ts` に置く。**ワークスペース名・キー・入口の一覧は
`docs/specifications/ui.md` §9.9・§10 と `docs/specifications/interfaces.md` を正本とし、`site.ts` だけを直す**
（ページに直書きしない）。数を変えたら `README.md` 側も合わせる（`docs/script/reference.py` の検査 7 が見張る）。

**生成物は手で編集しない。** 次のどちらかで作り直し、CI の `--check` と一致させる。

```bash
python3 scripts/diagnostics-site.py          # → website/src/data/diagnostics.ts
python3 scripts/api-site.py                  # → website/src/data/api.ts
python3 scripts/docs-site.py                 # → website/src/content/docs/** と src/data/docs-nav.ts
python3 scripts/docs-site.py --check          # CI と同じ検査
python3 scripts/diagnostics-site.py --check
python3 scripts/api-site.py --check
```

正本は `docs/specifications/diagnostics.md` §7 と `docs/specifications/interfaces.md` §19.7（説明文は `src/ui/i18n/{ja,en}.toml`）。
3 つの `--check` はいずれも `bash scripts/verify.sh` の `site-check` に含まれる。

**ガイドと仕様書の本文はこのリポジトリのどこにも複製しない。** `docs/users/*.md` と
`docs/specifications/*.md` が唯一の正本で、`scripts/docs-site.py` が
`src/content/docs/{users,specifications}/` と `src/data/docs-nav.ts` に写す。本文を直すときは
`docs/` を直してから生成器を回す（`scripts/verify.sh` が食い違いを落とす）。ページの枠は
`src/components/DocShell.astro`（左に目次）と `src/styles/doc.scss` が担う。

## 公開とサブパス

本番 URL は `https://sonneko.github.io/novnow/`。上表のパスには `/novnow` を付ける。
`REF:website/astro.config.mjs` で `site` と `base` を設定し、
`REF:website/integrations/base-links.mjs` が静的 HTML の手書き・Markdown リンクも補正する。
言語切替とナビゲーションは base を除いたパスで言語・現在位置を判定する。
`trailingSlash: ignore` により末尾 `/` の有無を両方受け付ける。
canonical / hreflang は末尾 `/` 付きに統一するが、アクセス時には強制しない。

`REF:.github/workflows/deploy.yml` がサイトと Web アプリを一括デプロイする。
Astro の出力に Trunk の出力を `app/` として追加し、Pages へ渡す artifact は 1 つだけにする。
別ワークフローから Pages をデプロイしない。認証と公開ゲートは `REF:docs/distribution.md` §8〜§10。

```bash
node --test website/integrations/base-links.test.mjs  # リポジトリ直下から
```

## サイト全体の検証

リポジトリ直下で次を実行する。`check:site` はロック済み依存で静的ビルドし、
リンク補正テストと `REF:scripts/website-check.py` の全出力検査を行う。

```bash
npm ci --prefix website
npm run check:site --prefix website
python3 -B -m unittest discover -s docs/script -p 'test_*.py'
```

検査対象は内部リンク・アンカー・画像/CSS の参照先・重複 ID・画像 alt・記事位置を保つ言語切替・
製品版表示・互換性告知。製品版は `REF:website/src/data/release.ts` が本体マニフェストから読む。
型検査・ブラウザ実行・外部公開先の可用性は、この静的検査とは別に確認する。

サイト単独では Web アプリがまだ無いので `--website-only` でアプリ入口だけを検査対象外にする。
`REF:.github/workflows/deploy.yml` では app 結合後に例外なしの検査を再実行する。
PR の `REF:.github/workflows/website.yml` は関連ファイル変更時だけビルド検査し、公開は行わない。
開発モードでは `REF:website/src/middleware.ts` が同じ base 補正を行い、Vite の開発用エンドポイントは補正しない。

`REF:website/src/components/ReleaseNotice.astro` は全言語ページでデータ互換性の注意を表示する。
診断・手引き・仕様書の詳細ページでも、言語切替は同じ記事へ移動する。
