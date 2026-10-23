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
| `/ja` `/en` | `src/pages/<lang>/index.astro` | `src/data/features.ts`・`src/data/i18n.ts` |
| `/<lang>/guide` | `src/pages/<lang>/guide.astro` | `src/data/i18n.ts` |
| `/<lang>/diagnostics` | `src/pages/<lang>/diagnostics.astro` | **`src/data/diagnostics.ts`（自動生成）** |
| `/<lang>/diagnostics/<code>` | `src/pages/[lang]/diagnostics/[code].astro` | 同上 + `diagnostics-examples.json` |
| `/<lang>/api` | `src/pages/<lang>/api.astro` | **`src/data/api.ts`（自動生成）** |
| `/<lang>/troubleshooting` | `src/pages/<lang>/troubleshooting.astro` | `src/data/troubleshooting.ts` |

**生成物は手で編集しない。** 次のどちらかで作り直し、CI の `--check` と一致させる。

```bash
python3 scripts/diagnostics-site.py          # → website/src/data/diagnostics.ts
python3 scripts/api-site.py                  # → website/src/data/api.ts
python3 scripts/diagnostics-site.py --check   # CI と同じ検査
python3 scripts/api-site.py --check
```

正本は `docs/specifications/diagnostics.md` §7 と `docs/specifications/interfaces.md` §19.7（説明文は `src/ui/i18n/{ja,en}.toml`）。
どちらの `--check` も `bash scripts/verify.sh` に含まれる。
