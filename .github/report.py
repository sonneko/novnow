#!/usr/bin/env python3
"""CI の出力を**人が読む形**に整える（`.github/workflows/ci.yml` の 4 ジョブ共用）。

## なぜこれがあるか

GitHub Actions のログ取得先ホスト（`objects.githubusercontent.com`）に作業環境から到達できない
ため、このリポジトリでは**失敗の中身を読む経路が PR コメントしかない**
（`PLANS.md` §9.3、`.github/workflows/ci.yml` の冒頭コメント）。ところが `cargo` の生ログは

- コンパイルエラーが数十行の `Compiling …` に埋もれる、
- `cargo test` は fail-fast なので**最初の失敗ターゲットしか出ない**、
- テスト失敗の本体（`---- <name> stdout ----`）はログの**真ん中**にあり、
  `head` / `tail` の切り出しでは落ちる、

という形で、必要な情報がちょうど切り捨てられる位置にある。このスクリプトは生ログから
**「どれが・どこで・何と言って失敗したか」だけ**を抜き出す。

## 出力（3 つ）

1. **ワークフロー注釈**（`::error file=…,line=…,col=…::`）— コンパイルエラーとテスト失敗の
   panic 位置を PR の「Files changed」とチェック画面にインラインで出す。
2. **実行サマリ**（`$GITHUB_STEP_SUMMARY`）— 成功時も含めて Always 書く。テスト件数の内訳と、
   失敗があればその一覧。Actions の run ページを開けばすぐ分かる。
3. **PR コメントの本文**（第 1 引数のパス）— 失敗時のみ投稿される（投稿は workflow 側が
   `gh pr comment` で行う）。GitHub のコメント上限（65,536 字）に収まるよう切り詰める。

## 使い方（workflow 側）

    env:
      CI_LOG: /tmp/ci.log        # cargo の出力を tee したファイル
      CI_JOB: Clippy             # 表示名
      CI_KIND: clippy            # fmt | clippy | test | wasm
      CI_STATUS: ${{ steps.cargo.outcome }}   # success | failure
      CI_SHA: ${{ github.sha }}
    run: python3 .github/report.py /tmp/body.md

解析できなかったときは**生ログの先頭と末尾にフォールバックする**（情報を落とさないことを
優先する。解析器のバグで失敗理由が消えるのが一番悪い）。

標準ライブラリだけを使う（runner の python3 に追加インストールしない）。
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

# ============================================================ 定数

# PR コメントの上限は 65,536 字。**ヘッダとマージンを見て 60,000 字**で切り詰める。
COMMENT_LIMIT = 60_000

# 実行サマリの上限（コメントより厳しいので短くする）。
SUMMARY_LIMIT = 20_000

# コンパイルエラー 1 件につき残す本文の行数（`-->` の行以降）。
ERROR_CONTEXT_LINES = 24

# テスト失敗 1 件につき残す行数（panic メッセージ）。
TEST_CONTEXT_LINES = 30

# 注釈・一覧に載せる失敗の最大件数（それ以上は「他 N 件」とまとめる）。
MAX_ITEMS = 40

# rustc の診断の始まり（`error[E0716]: …` / `warning: …` / `error: …`）。
RE_DIAGNOSTIC = re.compile(r"^(error|warning)(\[[^\]]+\])?:\s*(.*)$")

# 診断の位置（`  --> tests/foo.rs:415:44`）。
RE_LOCATION = re.compile(r"^\s*-->\s*([^:\s][^:]*):(\d+):(\d+)\s*$")

# テスト失敗のブロックの始まり（`---- the_test_name stdout ----`）。
RE_TEST_BLOCK = re.compile(r"^----\s+(\S+)\s+std(out|err)\s+----\s*$")

# panic の位置（`thread '…' panicked at tests/foo.rs:12:5:`）。
RE_PANIC = re.compile(r"panicked at (\S+?):(\d+):(\d+)")

# `test result: FAILED. 12 passed; 1 failed; 0 ignored; 0 measured; 12 filtered out`
RE_RESULT = re.compile(
    r"^test result: (\w+)\.\s+(\d+) passed;\s+(\d+) failed;\s+(\d+) ignored"
)

# rustfmt の差分の始まり（`Diff in /…/src/core/id.rs:527:`）。
RE_FMT_DIFF = re.compile(r"^Diff in (\S+):(\d+):")

# ターゲット（`Running tests/foo.rs (target/debug/deps/foo-…)` / `Running unittests src/lib.rs`）。
RE_RUNNING = re.compile(r"^(?:Running|Doc-tests)\s+(\S+)")

# 失敗テスト名の一覧（末尾の `failures:` セクションの 4 空白インデント）。
RE_FAILURE_NAME = re.compile(r"^    ([a-zA-Z0-9_:]+)$")

# cargo 自身のまとめ行。**診断ではない**ので拾わない（件数の水増しになるだけ）。
# 内容は「失敗したターゲット名」と「再実行コマンド」で、どちらも test result の行と
# 失敗テストの一覧から分かる。
RE_NOISE = re.compile(
    r"^(?:error|warning):\s*(?:could not compile|aborting due to|build failed"
    r"|test failed, to rerun|\d+ targets? failed|Some tests failed"
    r"|not all tests were run|the following warnings|\d+ warnings? emitted)"
)


# ============================================================ 部品


def env(name: str, default: str = "") -> str:
    return os.environ.get(name, default)


def read_log(path: str) -> list[str]:
    """生ログを行で読む。無ければ空（呼び出し側がフォールバックする）。"""
    try:
        return Path(path).read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return []


def escape(text: str) -> str:
    """ワークフローコマンドの値をエスケープする（`%` → `%25`、改行 → `%0A`）。"""
    return text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def annotate(severity: str, message: str, file: str = "", line: str = "", col: str = "") -> None:
    """`::error file=…,line=…,col=…::message` を stdout に出す（PR にインライン表示される）。"""
    where = ""
    if file:
        where = f" file={escape(file)}"
        if line:
            where += f",line={escape(line)}"
        if col:
            where += f",col={escape(col)}"
    print(f"::{severity}{where}::{escape(message)}")


def clip(text: str, limit: int, note: str = "…（以下は切り詰めました）") -> str:
    if len(text) <= limit:
        return text
    return text[: limit - len(note) - 1].rstrip() + "\n" + note


def fence(body: str, language: str = "") -> str:
    """コードフェンス。本文中に ``` があったらフェンスを長くする（壊れないように）。"""
    bar = "```"
    while bar in body:
        bar += "`"
    return f"{bar}{language}\n{body}\n{bar}"


# ============================================================ 解析


class Item:
    """失敗 1 件（コンパイルエラー / テスト失敗のどちらにも使う）。"""

    def __init__(
        self,
        kind: str,
        title: str,
        body: list[str],
        file: str = "",
        line: str = "",
        col: str = "",
    ) -> None:
        self.kind = kind  # "error" | "warning" | "test"
        self.title = title
        self.body = body
        self.file = file
        self.line = line
        self.col = col

    @property
    def where(self) -> str:
        """`file:line:col`（持っている部分だけ）。"""
        if not self.file:
            return ""
        if not self.line:
            return self.file
        if not self.col:
            return f"{self.file}:{self.line}"
        return f"{self.file}:{self.line}:{self.col}"

    def heading(self) -> str:
        return f"{self.title}（{self.where}）" if self.where else self.title


def parse_diagnostics(lines: list[str]) -> list[Item]:
    """rustc / clippy の診断（`error[E0716]: …` ＋ `--> file:line:col` ＋ 本文）を拾う。"""
    found: list[Item] = []
    at = 0
    while at < len(lines):
        matched = RE_DIAGNOSTIC.match(lines[at])
        if not matched or RE_NOISE.match(lines[at]):
            at += 1
            continue
        severity, code, message = matched.groups()
        # `error: could not compile …` のようなまとめ行は位置を持たないので本文だけ拾う
        file = line = col = ""
        body: list[str] = []
        scan = at + 1
        while scan < len(lines) and scan <= at + 4:
            located = RE_LOCATION.match(lines[scan])
            if located:
                file, line, col = located.groups()
                body = lines[scan + 1 : scan + 1 + ERROR_CONTEXT_LINES]
                break
            if RE_DIAGNOSTIC.match(lines[scan]):
                break
            scan += 1
        title = f"{severity}{code or ''}: {message}".strip()
        # 本文は次の診断の手前で切る
        trimmed: list[str] = []
        for text in body:
            if RE_DIAGNOSTIC.match(text):
                break
            trimmed.append(text)
        while trimmed and not trimmed[-1].strip():
            trimmed.pop()
        while trimmed and not trimmed[0].strip():
            trimmed.pop(0)
        found.append(Item(severity, title, trimmed, file, line, col))
        at = scan + 1 if body else at + 1
    return found


def parse_test_failures(lines: list[str]) -> list[Item]:
    """`---- <name> stdout ----` のブロックを拾う（panic 位置とメッセージ）。"""
    found: list[Item] = []
    at = 0
    while at < len(lines):
        matched = RE_TEST_BLOCK.match(lines[at])
        if not matched:
            at += 1
            continue
        name = matched.group(1)
        end = at + 1
        while end < len(lines) and not RE_TEST_BLOCK.match(lines[end]):
            if RE_RESULT.match(lines[end]) or lines[end].strip() == "failures:":
                break
            end += 1
        block = lines[at + 1 : end][:TEST_CONTEXT_LINES]
        file = line = col = ""
        for text in block:
            located = RE_PANIC.search(text)
            if located:
                file, line, col = located.groups()
                break
        while block and not block[-1].strip():
            block.pop()
        while block and not block[0].strip():
            block.pop(0)
        found.append(Item("test", f"テスト失敗: {name}", block, file, line, col))
        at = end
    return found


def parse_fmt_diffs(lines: list[str], workspace: str) -> list[Item]:
    """`cargo fmt --check` の出力（`Diff in <path>:<line>:` ＋ unified diff）を箇所ごとに分ける。

    パスは runner 上の絶対パスなので、`GITHUB_WORKSPACE` の接頭辞を落として**リポジトリ相対**に
    する（注釈を PR の差分にインラインで出すには相対パスでないと結び付かない）。
    """
    found: list[Item] = []
    at = 0
    while at < len(lines):
        matched = RE_FMT_DIFF.match(lines[at])
        if not matched:
            at += 1
            continue
        path, line = matched.groups()
        if workspace and path.startswith(workspace):
            path = path[len(workspace) :].lstrip("/")
        end = at + 1
        while end < len(lines) and not RE_FMT_DIFF.match(lines[end]):
            end += 1
        body = [text.replace(workspace, ".") for text in lines[at:end]] if workspace else lines[at:end]
        found.append(Item("error", f"rustfmt の差分: {path}", body, path, line))
        at = end
    return found


def parse_results(lines: list[str]) -> list[tuple[str, str, int, int, int]]:
    """`test result:` の行をターゲット名と組で拾う（成功時もサマリに出す）。"""
    found: list[tuple[str, str, int, int, int]] = []
    target = ""
    for text in lines:
        running = RE_RUNNING.match(text)
        if running:
            target = Path(running.group(1)).name
            continue
        matched = RE_RESULT.match(text)
        if matched:
            status, passed, failed, ignored = matched.groups()
            found.append((target, status, int(passed), int(failed), int(ignored)))
    return found


def parse_failed_names(lines: list[str]) -> list[str]:
    """末尾の `failures:` セクションから失敗テスト名だけを拾う（一覧用）。"""
    names: list[str] = []
    in_list = False
    for text in lines:
        if text.strip() == "failures:":
            # 2 回目の `failures:`（末尾の一覧）が本命。1 回目はブロックの直前
            in_list = not names or in_list
            continue
        if in_list:
            matched = RE_FAILURE_NAME.match(text)
            if matched:
                names.append(matched.group(1))
            elif not text.strip():
                continue
            else:
                in_list = False
    return names


# ============================================================ 出力


def totals(results: list[tuple[str, str, int, int, int]]) -> tuple[int, int, int]:
    passed = sum(row[2] for row in results)
    failed = sum(row[3] for row in results)
    ignored = sum(row[4] for row in results)
    return passed, failed, ignored


def markdown(
    job: str,
    kind: str,
    status: str,
    sha: str,
    items: list[Item],
    results: list[tuple[str, str, int, int, int]],
    log: list[str],
) -> str:
    """PR コメントと実行サマリに使う Markdown を組み立てる。"""
    failed = status != "success"
    head = f"### {'❌' if failed else '✅'} {job}（`{sha[:7]}`）\n"
    parts: list[str] = [head]

    # --- テスト件数（test ジョブのとき。成功時にも出す＝緑の証跡になる）
    if results:
        passed, bad, ignored = totals(results)
        parts.append(
            f"**{passed} passed / {bad} failed / {ignored} ignored**"
            f"（ターゲット {len(results)} 本）\n"
        )
        if bad:
            names = parse_failed_names(log)
            if names:
                listed = names[:MAX_ITEMS]
                extra = f"\n- …他 {len(names) - len(listed)} 件" if len(names) > len(listed) else ""
                parts.append("**失敗したテスト**\n\n" + "\n".join(f"- `{name}`" for name in listed) + extra + "\n")

    if not items:
        if not failed:
            parts.append("問題なし。\n")
            return "\n".join(parts)
        if not log:
            parts.append(
                "**ログが空**（cargo のステップが実行される前に落ちた可能性）。"
                "ジョブの生ログ（checkout / ツールチェーンのセットアップ）を確認してください。\n"
            )
            return "\n".join(parts)
        # 失敗したのに解析できなかった → 生ログの先頭と末尾（情報を落とさない）
        parts.append(
            "**失敗したが、コンパイルエラーもテスト失敗も見つからなかった**。"
            "生ログの先頭と末尾を載せる。\n"
        )
        head_lines = "\n".join(log[:120])
        tail_lines = "\n".join(log[-60:])
        parts.append(fence(head_lines) + "\n…（中略）…\n" + fence(tail_lines))
        return "\n".join(parts)

    # --- 失敗の一覧
    errors = [item for item in items if item.kind == "error"]
    warnings = [item for item in items if item.kind == "warning"]
    tests = [item for item in items if item.kind == "test"]
    counts = []
    if kind == "fmt":
        files = {item.file for item in errors if item.file}
        counts.append(f"**{len(errors)} 箇所**（{len(files)} ファイル）が `cargo fmt` の形と違います")
    else:
        if errors:
            # clippy の指摘は「コンパイルエラー」と呼ぶと誤解するので言い分ける
            label = "clippy の指摘" if kind == "clippy" else "コンパイルエラー"
            counts.append(f"{label} **{len(errors)} 件**")
        if tests:
            counts.append(f"テスト失敗 **{len(tests)} 件**")
        if warnings:
            counts.append(f"警告 {len(warnings)} 件")
    if counts:
        parts.append(" / ".join(counts) + "\n")

    shown = (errors + tests + warnings)[:MAX_ITEMS]
    for item in shown:
        parts.append(f"#### {item.heading()}\n")
        if item.body:
            parts.append(fence("\n".join(item.body).rstrip()) + "\n")
    hidden = len(errors) + len(tests) + len(warnings) - len(shown)
    if hidden > 0:
        parts.append(f"…他 **{hidden} 件**（全文は Actions のログ、またはローカルで同じコマンドを実行）\n")

    if kind == "fmt":
        parts.append("`cargo fmt --all` で下の差分を適用できます。\n")
    if failed:
        parts.append(
            "ローカルでの再現（CI と同じコマンド。`DEVELOPERS.md` §1）:\n\n"
            + fence(
                "cargo fmt --all --check\n"
                "cargo clippy --locked --all-targets -- -D warnings\n"
                "cargo test --locked --all-targets --no-fail-fast\n"
                "cargo check --locked --target wasm32-unknown-unknown"
            )
            + "\n"
        )
    return "\n".join(parts)


def gist(item: Item) -> str:
    """注釈に付ける「ひとことの理由」。罫線（`|` だけ・行番号だけ）の行は飛ばす。"""
    for text in item.body:
        stripped = text.strip()
        if not stripped:
            continue
        # rustc の罫線（`  |` / `2 |` / `  = note:` の飾り）と backtrace の注記は理由ではない
        if stripped in {"|", "="} or re.fullmatch(r"[\d\s|=+-]+", stripped):
            continue
        if stripped.startswith(("note: run with", "For more information about")):
            continue
        # rustfmt の差分は 1 行目が `Diff in …`（見出しと同じ情報）なので飛ばし、
        # 文脈行（先頭が空白）より**変わった行**（`-` / `+`）を理由にする
        if stripped.startswith("Diff in "):
            continue
        if item.title.startswith("rustfmt") and text.startswith(" "):
            continue
        # テスト失敗は panic 行そのものではなく**その次のメッセージ**が理由
        if item.kind == "test" and "panicked at" in stripped:
            continue
        return re.sub(r"^\s*\d+\s*\|\s*", "", stripped)[:200]
    return ""


def main() -> int:
    body_path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/body.md"
    log_path = env("CI_LOG", "/tmp/ci.log")
    job = env("CI_JOB", "CI")
    kind = env("CI_KIND", "build")
    status = env("CI_STATUS", "failure")
    sha = env("CI_SHA", "")

    log = read_log(log_path)
    items: list[Item] = []
    if kind == "fmt":
        # rustfmt の差分は「診断」ではないので、箇所ごとに分けて注釈を出す
        items.extend(parse_fmt_diffs(log, env("GITHUB_WORKSPACE", "")))
        if not items and log and status != "success":
            items.append(Item("error", "rustfmt の差分", log[:400]))
    else:
        items.extend(parse_diagnostics(log))
        if kind == "test":
            items.extend(parse_test_failures(log))
    results = parse_results(log) if kind == "test" else []

    # 1. ワークフロー注釈（失敗の位置を PR の差分にインラインで出す）
    for item in items:
        if item.kind == "warning":
            continue
        detail = gist(item)
        message = f"{item.heading()} — {detail}" if detail else item.heading()
        annotate("error", message, item.file, item.line, item.col)

    text = markdown(job, kind, status, sha, items, results, log)

    # 2. 実行サマリ（成功時も書く）
    summary = env("GITHUB_STEP_SUMMARY")
    if summary:
        try:
            with open(summary, "a", encoding="utf-8") as out:
                out.write(clip(text, SUMMARY_LIMIT) + "\n")
        except OSError as error:
            print(f"::warning::実行サマリを書けませんでした: {error}")

    # 3. PR コメントの本文（投稿は workflow 側が `gh pr comment` で行う）
    try:
        Path(body_path).write_text(clip(text, COMMENT_LIMIT) + "\n", encoding="utf-8")
    except OSError as error:
        print(f"::error::コメント本文を書けませんでした: {error}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
