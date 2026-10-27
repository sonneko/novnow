# 貢献案内（Contributing）

novnow に関心を持っていただきありがとうございます。この文書は、不具合の報告と提案の出し方を案内します。

> English summary: Code pull requests are not accepted for the time being. Bug reports and proposals via Issues are welcome — please use the templates. Specification changes are approval-based: the specifications under `docs/specifications/` are the single source of truth.

## コードのプルリクエストについて

**当面、外部からのコードのプルリクエストは受け付けていません。** 送っていただいても取り込むことができませんので、作業を始める前にこの点をご承知おきください。

理由は、novnow の開発が「仕様書を正本とし、変更は承認を経てから実装する」進め方を取っているためです。受け入れ体制が整い次第、この案内を更新します。

自分でビルドして試すこと自体は自由です（MIT ライセンス）。開発環境の作り方は [`docs/developers.md`](./docs/developers.md) にあります。

## 不具合の報告

GitHub の Issue から「不具合報告」テンプレートでお寄せください。再現手順と環境（版・OS・ブラウザ）が分かると助かります。診断コード（`E001` など）が表示されている場合は、それも書いてください。

公開済みの版で確認されている未解消の問題は [`docs/known-issues/ja.md`](./docs/known-issues/ja.md) に載っています。報告の前に一度ご確認ください。

## 提案（仕様・機能の変更）

提案は Issue の「提案」テンプレートでお寄せください。novnow では**仕様変更は承認制**です。

1. まず Issue で「困っていること」と「提案」を議論します。
2. 承認された提案は、仕様書（[`docs/specifications/`](./docs/specifications/)）へ反映されてから実装されます。
3. 仕様書に書かれていない振る舞いは、実装だけ先に変わることはありません。

この順序のため、提案が採用されても反映まで時間がかかることがあります。

## やり取りのお願い

お互いに敬意を持ったやり取りをお願いします。行動規範は、貢献受け入れを再開する時点で改めて用意します。
