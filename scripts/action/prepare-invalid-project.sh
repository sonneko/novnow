#!/usr/bin/env bash
# 公開後 smoke（`REF:.github/workflows/action-smoke.yml`）が使う「E 診断が出る作品」を作る。
#
# 本文を拡張子で決め打ちしないこと。サンプルの本文は `*.txt` から `*.cnov` へ変わったことが
# あり、決め打ちの `find -name '*.txt'` は空を返して負例を作れず、後段の「E 診断で exit code 5」
# が一度も走らないまま 3 OS とも準備の段階で失敗した（#61 追加監査 7）。ここでは `contents/`
# 直下の隠れていない通常ファイルを本文とし（`REF:docs/specifications/project.md` §4.2 の C 形式）、
# 1 件も見つからなければ曖昧なリダイレクトではなく明示的なエラーで止まる。
#
# 使い方: prepare-invalid-project.sh <元の作品> <作り先>
# 標準出力: 未知参照を注入した本文のパス
# 終了コード: 0 = 成功 / 1 = 本文が見つからない・注入できない / 2 = 引数が足りない
set -euo pipefail

# 未知参照（`REF:docs/specifications/diagnostics.md` の E001）。辞書に無い語を本文に足す。
UNKNOWN_REFERENCE="'action_unknown_term"

usage() {
  echo "usage: prepare-invalid-project.sh <source project> <target>" >&2
}

source_project="${1:-}"
target="${2:-}"
if [ -z "$source_project" ] || [ -z "$target" ]; then
  usage
  exit 2
fi

fail() {
  # Actions のログで失敗の理由が残る形にする（素の bash としても標準エラーへ出す）。
  echo "::error::$1" >&2
  exit 1
}

[ -d "$source_project" ] || fail "元の作品が無い: $source_project"
[ -d "$source_project/contents" ] || fail "本文のディクトリが無い: $source_project/contents"

rm -rf "$target"
cp -R "$source_project" "$target"

# 拡張子は問わない。ドットで始まる名前（一時ファイル・局所状態）だけを除く。
first="$(find "$target/contents" -type f ! -name '.*' | sort | head -1)"
if [ -z "$first" ]; then
  fail "本文ファイルが無い: $target/contents（負例を作れない。本文の形式が変わったらここを直す）"
fi

printf '\n%s\n' "$UNKNOWN_REFERENCE" >> "$first"
grep -qF "$UNKNOWN_REFERENCE" "$first" || fail "未知参照を注入できなかった: $first"

echo "$first"
