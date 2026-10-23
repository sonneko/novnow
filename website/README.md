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

トップページの「ひと目で分かる novnow」「9 つのワークスペース」「キー一覧」「外部インターフェース」の各ブロックは
`src/components/Overview.astro` が描き、データは `src/data/site.ts` に置く。**ワークスペース名・キー・入口の一覧は
`docs/specifications/ui.md` §9.9・§10 と `docs/specifications/interfaces.md` を正本とし、`site.ts` だけを直す**
（ページに直書きしない）。数を変えたら `README.md` 側も合わせる（`docs/script/reference.py` の検査 7 が見張る）。

**生成物は手で編集しない。** 次のどちらかで作り直し、CI の `--check` と一致させる。

```bash
python3 scripts/diagnostics-site.py          # → website/src/data/diagnostics.ts
python3 scripts/api-site.py                  # → website/src/data/api.ts
python3 scripts/diagnostics-site.py --check   # CI と同じ検査
python3 scripts/api-site.py --check
```

正本は `docs/specifications/diagnostics.md` §7 と `docs/specifications/interfaces.md` §19.7（説明文は `src/ui/i18n/{ja,en}.toml`）。
どちらの `--check` も `bash scripts/verify.sh` に含まれる。
