# novnow v{version}

{一行で「何のための版か」を書く。例: 保存の原子性と外部変更の検出を直した版。}

## 変更点

{利用者に見える変化を、影響の大きい順に。実装の詳細（ファイル名・行数）は書かない。
仕様書の節（例: §8.3）を引くときは `REF:docs/specifications/editing.md` の形にする。}

- {変更 1}
- {変更 2}

## 参照（Issue・PR）

{この版に入った Issue と PR。番号だけでよい（例: #61 / PR #62）。
main の履歴が孤立コミットになっている版では、差分の確認方法も書く。}

- Issue #{nnn}: {題名}
- PR #{nnn}: {題名}

## 互換性

{データ形式（`.cnov` / `project.toml`）・API（操作カタログ）・診断コードの互換性を書く。
「変更なし」でも**明示する**。v0 は形式の安定を保証しない（`REF:docs/specifications/external.md`）。}

- {例: `.cnov` と `project.toml` の形式変更は無い。}
- {例: 操作 `section.split` の引数 `keep` の既定値が `true` になった（破壊的変更）。}

## 既知の問題

{この版で**未解消**の問題。掲載中なら `REF:docs/known-issues/ja.md` の ID（KI-nnn）を引く。
無ければ「無い」と書き、それでも既知の問題の文書へのリンクは残す（検査がリンクを要求する）。}

- 掲載中の問題: {KI-nnn / 「無し」}
- 既知の問題の一覧: <https://github.com/sonneko/novnow/blob/main/docs/known-issues/ja.md>（英語は `en.md`）

## 配布物

{形态と数。正本は `REF:docs/distribution.md` §3 と `REF:scripts/release-matrix.py`。
版番号や成果物名を**書き写さない**（公開後にずれる。§6.1）。}

- デスクトップ / Python / VSCode 拡張 / ソース / GitHub Actions の 5 形態。
- 照合表 `SHA256SUMS` を同じリリースに添付する。

## 検証範囲

{何を検証し、何を**保証しないか**。実機でしか確かめられないものは
`REF:docs/developers.md` §4.9 のチェックリストへ送る。}

- {例: Rust テスト・3 OS の VSCode 拡張・3 エンジンのブラウザ試験は成功。}
- {例: 実機の GPU・IME・ファイルダイアログは保証しない。}
