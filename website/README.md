# novnow 公式サイト（Astro）

`novnow` の公式サイト。日本語（`/ja`）と英語（`/en`）の 2 言語で、トップ・利用ガイド・診断カタログ・API リファレンス・トラブルシューティングを配信する。

```bash
cd website
npm install
npm run dev      # http://localhost:4321（ルートは /en へリダイレクト）
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
