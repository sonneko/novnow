#!/usr/bin/env python3
"""
docs/script/reference.py — ドキュメントとコードの**実物の突き合わせ**を行う検査。

`duplicate.py` は「文書の規約」（構成・重複・`REF:` の実在・見出し一致・`SYNC` の一致）を見る。
それに対してこのスクリプトは、**規約では守れない「中身のずれ」**を見る。対象は次の 12 個。

1. 廃止済みの文書名（`SPECIFICATIONS.md` / `PLANS.md` / `DEVELOPERS.md`）を
   コード・スクリプト・CI から参照していないこと。
2. `docs/specifications/<name>.md` の参照先の `§N.M.K` が、その仕様書に**実在する見出しか**。
3. `docs/history.md` と一緒に書かれた `B-nnn` / `A-nn` が history に**実在するか**。
4. `docs/developers.md` §3 の `src/` ツリーが、`src/` の**実際の中身と一致するか**。
5. `scripts/verify.sh` が CI（`.github/workflows/ci.yml`）の**静的検査をすべて含むか**。
6. `README.md` / `README.en.md` / `GUIDE.md` の間に**丸写しの重複**が無いか。
7. 文書に書いた**数の主張**（ワークスペース数・MCP ツール数・正規操作数）が実装と一致するか。
8. `include_str!` で読み込むパス（仕様書・カタログ・ゴールデン）が**実在するか**。
9. ルート文書の相対リンク・画像と、`REF:` のアンカーが**実在するか**。
10. Actions を無駄に使わない設定（全ワークフローの `concurrency`・変更フィルタ・`[ci skip]` のフック）が揃っているか。
11. 公開する手引き（仕様 §14.2.1 の 3 URL・§18.5）が実在し、`docs/users` の参照が仕様書だけを指し、
    ガイドモードの段階と `docs/users/start.md` の段階が一致するか。
12. 配布形態（仕様 §20）の選択肢の数が 1・10・5・1・1 で、廃止した軸（同時起動の制限・同梱物の外置き）が
    仕様・設計・手順・利用者向けの文書へ戻っていないか。

使い方:
    python3 docs/script/reference.py           # 検査（違反があれば終了コード 1）
    python3 docs/script/reference.py --list    # 検査項目の一覧だけ出す

`docs/script/duplicate.py` と同じく、標準ライブラリだけで動く（CI の runner に何も入れない）。
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# 参照を探す範囲（コードとそれに準ずるもの）。
CODE_DIRS = ["src", "tests", "benches", "examples", "python", "scripts", ".github"]
CODE_SUFFIXES = {".rs", ".toml", ".py", ".sh", ".mjs", ".yml", ".yaml"}

# ディレクトリの外にあるが「コード側」として見るもの（マニフェストと PR テンプレート）。
# ここを漏らすと、`Cargo.toml` のコメントや `.github/pull_request_template.md` に
# 廃止名が残ったまま検査が緑になる（実際に残っていた）。
CODE_ROOT_FILES = ["Cargo.toml", "rust-toolchain.toml", ".github/pull_request_template.md"]

# 章番号 → 仕様書（`docs/developers.md` §1.1 の対応表と同じ）。
CHAPTERS = {
    "3": "overview",
    "4": "project",
    "5": "syntax",
    "6": "semantics",
    "7": "diagnostics",
    "8": "editing",
    "9": "ui",
    "10": "ui",
    "11": "ui",
    "12": "ui",
    "13": "platform",
    "14": "platform",
    "15": "agent",
    "16": "workflows",
    "17": "workflows",
    "18": "workflows",
    "19": "interfaces",
    "20": "external",
}

RETIRED = ("SPECIFICATIONS.md", "PLANS.md", "DEVELOPERS.md")

# 廃止名を含んでよいファイル（歴史の記録・検査器そのもの・名前の説明）。
RETIRED_ALLOWED = {
    "docs/history.md",
    "docs/script/duplicate.py",
    "docs/script/reference.py",
    "docs/script/test_reference.py",
}
# 廃止名を含んでよい行（「廃止した」と説明している行）。
RETIRED_LINE_MARKERS = ("廃止", "RETIRED", "旧", "ルートの")


def rel(path: Path) -> str:
    """リポジトリからの相対パス（`/` 区切り）。"""
    return path.relative_to(ROOT).as_posix()


def walk(dirs: list[str], suffixes: set[str]) -> list[Path]:
    """対象のファイルを集める（`.git` と `target` は見ない）。"""
    found: list[Path] = []
    for name in dirs:
        base = ROOT / name
        if not base.exists():
            continue
        for path in sorted(base.rglob("*")):
            if path.is_file() and path.suffix in suffixes:
                if "target/" in rel(path) or "/vendor/" in rel(path):
                    continue
                found.append(path)
    return found


def read(path: Path) -> str:
    """UTF-8 のテキスト。読めないファイルは空文字（バイナリ扱い）。"""
    try:
        return path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return ""


# ============================================================ 1. 廃止名


def check_retired() -> list[str]:
    """廃止済みの文書名がコード側に残っていないこと。"""
    errors: list[str] = []
    targets = walk(CODE_DIRS, CODE_SUFFIXES)
    targets += [ROOT / name for name in CODE_ROOT_FILES if (ROOT / name).exists()]
    for path in targets:
        name = rel(path)
        if name in RETIRED_ALLOWED:
            continue
        for number, line in enumerate(read(path).splitlines(), 1):
            if not any(old in line for old in RETIRED):
                continue
            if any(marker in line for marker in RETIRED_LINE_MARKERS):
                continue
            errors.append(
                f"[1] {name}:{number}: 廃止済みの文書名を参照している（{line.strip()[:80]}）"
            )
    return errors


# ============================================================ 2. 仕様書の見出し


def headings(path: Path) -> set[str]:
    """`## 7.` / `### 7.1` / `#### 7.1.2` の番号を集める。"""
    numbers: set[str] = set()
    for line in read(path).splitlines():
        if not line.startswith("#"):
            continue
        match = re.match(r"^#+\s*([0-9]+(?:\.[0-9]+)*)", line)
        if match:
            numbers.add(match.group(1))
    return numbers


def check_spec_sections() -> list[str]:
    """`docs/specifications/x.md` §N.M の節が実在すること。

    参照の形は 3 つを許す（コードと文書の両方で使われている書き方）。

        `docs/specifications/diagnostics.md` §7.1
        §7.1（`docs/specifications/diagnostics.md`）
        docs/developers.md の §3 の表が定める `docs/specifications/ui.md`
    """
    errors: list[str] = []
    cache: dict[str, set[str]] = {}
    pattern = re.compile(r"docs/specifications/([a-z_]+)\.md`?\s*(?:\*\*)?\s*§([0-9]+(?:\.[0-9]+)*)")
    for path in walk(CODE_DIRS + ["docs"], {".rs", ".toml", ".py", ".sh", ".mjs", ".yml", ".md"}):
        name = rel(path)
        if name.startswith("docs/history.md"):
            continue
        for number, line in enumerate(read(path).splitlines(), 1):
            for match in pattern.finditer(line):
                book, section = match.group(1), match.group(2)
                target = ROOT / "docs" / "specifications" / f"{book}.md"
                if not target.exists():
                    errors.append(f"[2] {name}:{number}: 仕様書 {book}.md が無い")
                    continue
                if book not in cache:
                    cache[book] = headings(target)
                if section not in cache[book]:
                    errors.append(
                        f"[2] {name}:{number}: {book}.md に §{section} の見出しが無い"
                    )
    return errors


# ============================================================ 3. history の ID


def check_history_ids() -> list[str]:
    """`docs/history.md` と一緒に書かれた `B-nnn` / `A-nn` が history に実在すること。"""
    errors: list[str] = []
    history = read(ROOT / "docs" / "history.md")
    pattern = re.compile(r"docs/history\.md`?[^\n]{0,40}?\b([AB])-([0-9]+)\b")
    for path in walk(CODE_DIRS, CODE_SUFFIXES) + walk(["docs"], {".md"}):
        name = rel(path)
        if name == "docs/history.md":
            continue
        for number, line in enumerate(read(path).splitlines(), 1):
            for match in pattern.finditer(line):
                ident = f"{match.group(1)}-{match.group(2)}"
                if ident not in history:
                    errors.append(
                        f"[3] {name}:{number}: {ident} が docs/history.md に無い"
                    )
    return errors


# ============================================================ 4. developers.md のツリー


def check_source_tree() -> list[str]:
    """`docs/developers.md` §3 の `src/` ツリーが実物と一致すること。"""
    text = read(ROOT / "docs" / "developers.md")
    match = re.search(r"^├── src/\n(?P<body>(?:.*\n)*?)^├── ", text, re.MULTILINE)
    if not match:
        return ["[4] docs/developers.md に src/ のツリーが無い"]
    body = match.group("body")
    listed: set[str] = set()
    for line in body.splitlines():
        item = re.match(r"^│   [├└]── ([a-z_]+(?:/|\.rs))", line)
        if item:
            listed.add(item.group(1).rstrip("/"))
    actual = {p.name for p in (ROOT / "src").iterdir() if p.is_dir()}
    actual |= {p.name for p in (ROOT / "src").iterdir() if p.suffix == ".rs"}
    errors: list[str] = []
    for missing in sorted(actual - listed):
        errors.append(f"[4] src/{missing} が docs/developers.md §3 のツリーに無い")
    for extra in sorted(listed - actual):
        errors.append(f"[4] docs/developers.md §3 のツリーにある src/{extra} が存在しない")
    return errors


# ============================================================ 5. verify.sh と CI


def ci_static_steps() -> set[str]:
    """CI が走らせる静的検査（`scripts/…` と `docs/script/…`）の集合。"""
    text = read(ROOT / ".github" / "workflows" / "ci.yml")
    found = set(re.findall(r"(?:docs/script/[a-z_]+\.py|scripts/[a-z-]+\.py|scripts/[a-z-]+\.sh)", text))
    return {step for step in found if step != "scripts/ci-report.sh"}


def check_verify_coverage() -> list[str]:
    """`scripts/verify.sh` が CI の静的検査をすべて含むこと（手元で CI と同じ判定をするため）。"""
    verify = read(ROOT / "scripts" / "verify.sh")
    errors = []
    for step in sorted(ci_static_steps()):
        name = Path(step).name
        if name not in verify:
            errors.append(f"[5] scripts/verify.sh が CI の検査 {step} を実行していない")
    return errors


# ============================================================ 6. ルート文書の重複


def check_root_documents() -> list[str]:
    """`README.md` / `README.en.md` / `GUIDE.md` の間に丸写しの重複ブロックが無いこと。"""
    files = [ROOT / "README.md", ROOT / "README.en.md", ROOT / "GUIDE.md"]
    blocks: dict[str, list[tuple[str, int]]] = {}
    for path in files:
        lines = [line.strip() for line in read(path).splitlines()]
        for index in range(len(lines) - 2):
            window = lines[index : index + 3]
            if any(len(line) < 30 for line in window):
                continue
            key = "\n".join(window)
            blocks.setdefault(key, []).append((path.name, index + 1))
    errors = []
    for key, places in blocks.items():
        if len({place[0] for place in places}) < 2:
            continue
        where = ", ".join(f"{name}:{line}" for name, line in places)
        first = key.splitlines()[0][:50]
        errors.append(f"[6] ルート文書で重複した段落がある（{where}）: {first}")
    return errors


# ============================================================ 7. 数の主張


def count_items(path: Path, pattern: str) -> int | None:
    """正規表現に一致する行数・出現数を数える（実装側の数）。"""
    text = read(path)
    if not text:
        return None
    return len(re.findall(pattern, text, re.MULTILINE))


def check_claims() -> list[str]:
    """文書に書いた数が実装と一致すること。

    - 「N ワークスペース」⇔ `src/app/workspace.rs` の `Workspace` の variant 数
    - 「MCP N ツール」⇔ `src/core/agents/server.rs` の `TOOLS` の件数
    - 「N の正規操作」⇔ `tests/golden/api_catalog.json` の操作数
    """
    errors: list[str] = []

    workspaces = count_items(ROOT / "src" / "app" / "workspace.rs", r"^\s{4}[A-Z][A-Za-z]+,\s*$")
    tools = count_items(ROOT / "src" / "core" / "agents" / "server.rs", r'^\s{8}"novnow\.[a-z_.]+",$')
    # 正規操作の数は JSON の `operations` 配列の長さで数える（`"id":` を数えると、
    # 引数や例外の中の `id` まで拾って 135 になる）。
    operations = len(json.loads(read(ROOT / "tests" / "golden" / "api_catalog.json")).get("operations", []))

    claims = [
        (workspaces, r"(?<![0-9.\-§])([0-9]+) ワークスペース", "ワークスペース数"),
        (workspaces, r"(?<![0-9.\-§])([0-9]+) Workspaces", "ワークスペース数（英語）"),
        (tools, r"MCP ([0-9]+) ツール", "MCP ツール数"),
        (operations, r"(?<![0-9.\-§])([0-9]+) の正規操作", "正規操作数"),
    ]
    documents = [ROOT / "README.md", ROOT / "README.en.md", ROOT / "GUIDE.md"]
    documents += sorted((ROOT / "docs").glob("*.md"))
    for path in documents:
        if path.name == "history.md":
            continue
        for number, line in enumerate(read(path).splitlines(), 1):
            for actual, pattern, label in claims:
                if actual is None:
                    continue
                for match in re.finditer(pattern, line):
                    if int(match.group(1)) != actual:
                        errors.append(
                            f"[7] {rel(path)}:{number}: {label}が {match.group(1)} と書かれている"
                            f"（実装は {actual}）"
                        )
    return errors


# ============================================================ 8. include_str!


def check_include_paths() -> list[str]:
    """`include_str!` / `include_bytes!` のパスが実在すること（テストの埋め込み）。"""
    errors = []
    pattern = re.compile(r'include_(?:str|bytes)!\("([^"]+)"\)')
    for path in walk(["src", "tests", "benches", "examples"], {".rs"}):
        for number, line in enumerate(read(path).splitlines(), 1):
            for match in pattern.finditer(line):
                target = (path.parent / match.group(1)).resolve()
                if not target.exists():
                    errors.append(f"[8] {rel(path)}:{number}: {match.group(1)} が無い")
    return errors



# ============================================================ 9. リンクとアンカー


def slug(heading: str) -> str:
    """GitHub の見出しアンカー（空白 → `-`、記号を落とす。CJK はそのまま）。"""
    text = heading.strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def anchors(path: Path) -> set[str]:
    """`<a id="...">` と見出しのスラグの集合（そのファイルで飛べる先）。"""
    found: set[str] = set(re.findall(r'<a id="([^"]+)"', read(path)))
    for line in read(path).splitlines():
        if line.startswith("#"):
            found.add(slug(line.lstrip("#").strip()))
    return found


def check_links() -> list[str]:
    """ルート文書の相対リンク・画像と、`REF:` のアンカーが実在すること。

    `duplicate.py` は `docs/` の `REF:` のパスだけを見る。ここでは
    (1) `README.md` / `README.en.md` / `GUIDE.md` の相対リンクと画像
    (2) アンカーが実際の見出し（または `<a id=...>`）に解決するか
    を確かめる。
    """
    errors: list[str] = []
    link = re.compile(r"\]\((?P<target>[^)\s]+)\)")

    for path in [ROOT / "README.md", ROOT / "README.en.md", ROOT / "GUIDE.md"]:
        text = read(path)
        for number, line in enumerate(text.splitlines(), 1):
            for match in link.finditer(line):
                target = match.group("target")
                if target.startswith(("http://", "https://", "mailto:")):
                    continue
                if target.startswith("#"):
                    if target[1:] not in anchors(path):
                        errors.append(f"[9] {rel(path)}:{number}: アンカー {target} が無い")
                    continue
                target_path = target.split("#")[0]
                if not (path.parent / target_path).exists():
                    errors.append(f"[9] {rel(path)}:{number}: リンク先 {target} が無い")

    ref = re.compile(r"REF:([A-Za-z0-9_./-]+)#([A-Za-z0-9_-]+)")
    for path in walk(CODE_DIRS + ["docs"], {".rs", ".toml", ".py", ".sh", ".mjs", ".yml", ".md"}):
        for number, line in enumerate(read(path).splitlines(), 1):
            for match in ref.finditer(line):
                target = ROOT / match.group(1)
                if not target.is_file():
                    continue  # パスの検査は duplicate.py の担当
                if match.group(2) not in anchors(target):
                    errors.append(
                        f"[9] {rel(path)}:{number}: {match.group(1)} に #{match.group(2)} が無い"
                    )
    return errors



# ============================================================ 10. Actions の節約


def check_actions_budget() -> list[str]:
    """Actions を無駄に走らせない設定が揃っていること。

    課金は「走った分」に付くので、**走らせない仕組み**そのものを検査する。

    - 全ワークフローに `concurrency`（同じブランチの古い実行を取り消す）
    - PR で走るワークフローは draft では起動しない（`pull_request.draft`）
    - 廃止したランナー（`macos-13`）を使っていない
    - `ci.yml` に変更フィルタ（`rust_changed`）がある
    - コミット既定で `[ci skip]` を付けるフックと、その有効化スクリプトがある
    """
    errors: list[str] = []

    for path in sorted((ROOT / ".github/workflows").glob("*.yml")):
        text = read(path)
        name = rel(path)
        if not re.search(r"^concurrency:", text, re.MULTILINE):
            errors.append(f"[10] {name}: concurrency が無い（古い実行が取り消されない）")
        elif "cancel-in-progress" not in text:
            errors.append(f"[10] {name}: cancel-in-progress が無い（自動では取り消されない）")
        # PR で走るワークフローは、draft の間は走らせない（全ワークフローで同じ条件に統一）。
        if re.search(r"^  pull_request:", text, re.MULTILINE) and "pull_request.draft" not in text:
            errors.append(f"[10] {name}: draft の PR でも走ってしまう（pull_request.draft の条件が無い）")
        if re.search(r"^\s*(?:runs-on|os):\s*['\"]?macos-13", text, re.MULTILINE):
            errors.append(
                f"[10] {name}: macos-13 は廃止した（待ち行列が長く、確認以上のコストになる）"
            )

    ci = read(ROOT / ".github/workflows/ci.yml")
    if "rust_changed" not in ci:
        errors.append("[10] .github/workflows/ci.yml: 変更フィルタ（rust_changed）が無い")

    hook = ROOT / ".githooks/commit-msg"
    if not hook.is_file():
        errors.append("[10] .githooks/commit-msg: 無い（[ci skip] が付かない）")
    elif "[ci skip]" not in read(hook):
        errors.append("[10] .githooks/commit-msg: [ci skip] を付けていない")
    elif not (hook.stat().st_mode & 0o111):
        errors.append("[10] .githooks/commit-msg: 実行ビットが無い")

    if not (ROOT / "scripts/install-git-hooks.sh").is_file():
        errors.append("[10] scripts/install-git-hooks.sh: 無い（フックを有効にできない）")
    if not (ROOT / "scripts/ci-run.sh").is_file():
        errors.append("[10] scripts/ci-run.sh: 無い（必要なときに CI を回せない）")

    return errors


# ---------------------------------------------------------------- 11. 公開する手引き（§14.2.1・§18.5）

# 仕様 §14.2.1 の 3 URL。`src/core/model/project.rs` の定数と同じ並び。
GUIDE_URLS = (
    "guide/project_structure.html",
    "guide/git.html",
    "guide/publish.html",
)


def check_published_guide() -> list[str]:
    errors: list[str] = []

    # (1) 3 URL の実体がリポジトリにあり、trunk が dist へ写す指定になっている
    for url in GUIDE_URLS:
        if not (ROOT / url).is_file():
            errors.append(
                f"[11] {url}: 無い（仕様 §14.2.1 の URL が公開物に実在しない。"
                " `python3 scripts/docs-site.py` を実行すること）"
            )
    manifest = (ROOT / "index.html").read_text(encoding="utf-8")
    if 'rel="copy-dir" href="guide"' not in manifest:
        errors.append(
            "[11] index.html: `guide` の copy-dir が無い（trunk の dist に手引きが載らない）"
        )

    # (2) 仕様 §14.2.1 とコードの URL が、同じ 3 本を指している
    spec = (ROOT / "docs/specifications/platform.md").read_text(encoding="utf-8")
    code = (ROOT / "src/core/model/project.rs").read_text(encoding="utf-8")
    for url in GUIDE_URLS:
        spec_url = f"https://sonneko.github.io/novnow/{url}"
        if spec_url not in spec:
            errors.append(f"[11] 仕様 §14.2.1 に {spec_url} が無い")
        if spec_url not in code:
            errors.append(f"[11] src/core/model/project.rs に {spec_url} が無い")

    # (3) 手引きのリンクが、同じフォルダの HTML か外部 URL か仕様書を指している
    users = ROOT / "docs/users"
    for path in sorted(users.glob("*.md")):
        text = path.read_text(encoding="utf-8")
        for target in re.findall(r"\]\(([^)\s]+)\)", text):
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            rel = target.split("#", 1)[0]
            if rel.startswith("./"):
                if not (users / rel[2:]).is_file():
                    errors.append(
                        f"[11] {path.relative_to(ROOT).as_posix()}: {target} の飛び先が無い"
                    )
            elif rel.startswith("../"):
                resolved = (users / rel).resolve()
                if not resolved.exists():
                    errors.append(
                        f"[11] {path.relative_to(ROOT).as_posix()}: {target} の飛び先が無い"
                    )
                elif not str(resolved).startswith(str(ROOT)):
                    errors.append(
                        f"[11] {path.relative_to(ROOT).as_posix()}: {target} がリポジトリの外を指す"
                    )
                elif resolved.is_dir() and resolved.name == "specifications":
                    pass
                elif "docs/specifications/" not in resolved.as_posix():
                    errors.append(
                        f"[11] {path.relative_to(ROOT).as_posix()}: {target} は仕様書以外を指す"
                        "（docs/users から参照してよいのは docs/specifications と docs/users だけ）"
                    )
            else:
                errors.append(
                    f"[11] {path.relative_to(ROOT).as_posix()}: {target} は相対リンクでない"
                )

    # (3b) 生成した HTML どうしのリンクも実在する
    guide_dir = ROOT / "guide"
    for page in sorted(guide_dir.glob("*.html")):
        html = page.read_text(encoding="utf-8")
        for href in re.findall(r'href="([^"]+)"', html):
            if href.startswith(("http://", "https://", "#", "../", "mailto:")):
                continue
            target = href.split("#", 1)[0]
            if target and not (guide_dir / target).is_file():
                errors.append(f"[11] {page.relative_to(ROOT).as_posix()}: {href} の飛び先が無い")

    # (4) ガイドモードの段階（src/ui/guide.rs）と start.md の見出しが同じ数・同じ順
    guide_rs = (ROOT / "src/ui/guide.rs").read_text(encoding="utf-8")
    steps = re.findall(r"^\s*number: (\d+),$", guide_rs, flags=re.MULTILINE)
    if [int(number) for number in steps] != list(range(1, 10)):
        errors.append(
            f"[11] src/ui/guide.rs の段階が 1〜9 でない（{', '.join(steps) or 'なし'}）"
        )
    start = (ROOT / "docs/users/start.md").read_text(encoding="utf-8")
    headings = re.findall(r"^## (\d+)\. ", start, flags=re.MULTILINE)
    if [int(number) for number in headings] != list(range(1, 10)):
        errors.append(
            "[11] docs/users/start.md の段階の見出しが 1〜9 でない"
            f"（{', '.join(headings) or 'なし'}）。ガイドモード（§9.10）と同じ順序にすること"
        )

    return errors


# ---------------------------------------------------------------- 12. 配布形態（§20）

# 仕様 §20.1.1 の表の行（形態名 → 選択肢の数）。デスクトップ版は「GUI あり / なし × 対象環境 5」の 10 通り。
DISTRIBUTION_FORMS = {
    "ブラウザ版": 1,
    "デスクトップ版": 10,
    "Python ライブラリ版": 5,
    "VSCode 拡張版": 1,
    "GitHub Actions 版": 1,
}

# 2026-10-23 に廃止した軸。仕様・設計・手順・利用者向けには戻さない（経緯は docs/history.md にだけ残す）。
RETIRED_AXES = ("シングルトン", "マルチプル", "スタンドアロン", "standalone", "single-instance", "-single", "-multi")


def stale_release_claims(text: str, expected_artifacts: int) -> list[str]:
    """Completed release docs must not reintroduce retired settings or counts."""
    errors = []
    for obsolete in ("PYPI_API_TOKEN", "novnow.downloadVersion", "cargo publish` は行わない"):
        if obsolete in text:
            errors.append(f"廃止済みの公開設計: {obsolete}")
    for count in re.findall(r"の\s*([0-9]+)\s*成果物", text):
        if int(count) != expected_artifacts:
            errors.append(f"最終成果物が {count} 個（全ソース込み {expected_artifacts} 個）")
    return errors


def check_distribution_forms() -> list[str]:
    """配布形態の数と、廃止した軸が文書へ戻っていないこと。

    数を 1 か所（仕様 §20.1.1 の表）に保ち、ほかの文書はその値を使う。
    軸を足し直すと、名前（`novnow-<版>-<対象環境>-<gui|nogui>`）の検査でも落ちる。
    """
    errors: list[str] = []
    spec = read(ROOT / "docs/specifications/external.md")
    if not spec:
        return ["[12] docs/specifications/external.md が無い（仕様 §20 配布形態）"]

    found: dict[str, int] = {}
    row = re.compile(r"^\|\s*(ブラウザ版|デスクトップ版|Python ライブラリ版|VSCode 拡張版|GitHub Actions 版)\s*\|[^|]*\|\s*([0-9]+)\s*\|")
    for line in spec.splitlines():
        match = row.match(line)
        if match:
            found[match.group(1)] = int(match.group(2))
    for name, expected in DISTRIBUTION_FORMS.items():
        actual = found.get(name)
        if actual is None:
            errors.append(f"[12] 仕様 §20.1.1 の表に「{name}」の行が無い")
        elif actual != expected:
            errors.append(
                f"[12] 仕様 §20.1.1 の「{name}」が {actual} 通り（{expected} 通りであること）"
            )

    total = sum(DISTRIBUTION_FORMS.values())
    if "novnow-<版>-<対象環境>-<gui|nogui><拡張子>" not in spec:
        errors.append("[12] 仕様 §20.2.2 の名前の形（novnow-<版>-<対象環境>-<gui|nogui>）が無い")
    for path, label in (
        (ROOT / "docs/designs/external.md", f"{total} 通り"),
        (ROOT / "docs/plans.md", f"5 形態・{total} 通り"),
    ):
        if label not in read(path):
            errors.append(f"[12] {rel(path)} に「{label}」が無い（仕様 §20.1.1 の合計と合わせる）")

    desktop = re.search(r"^## 2\. デスクトップ版（([0-9]+) 通り）$", read(ROOT / "docs/distribution.md"), re.MULTILINE)
    if not desktop or int(desktop.group(1)) != DISTRIBUTION_FORMS["デスクトップ版"]:
        errors.append(
            f"[12] docs/distribution.md の「## 2. デスクトップ版（N 通り）」が "
            f"{DISTRIBUTION_FORMS['デスクトップ版']} 通りになっていない"
        )

    # 数を書き写している文書（README・手引き）。書き写す以上、直し忘れを機械で見る。
    claims = (
        (ROOT / "README.md", r"デスクトップ\s*([0-9]+)\s*通り"),
        (ROOT / "README.en.md", r"([0-9]+) desktop builds"),
        (ROOT / "docs/users/install.md", r"デスクトップ版は\s*([0-9]+)\s*通り"),
    )
    for path, pattern in claims:
        text = read(path)
        if not text:
            errors.append(f"[12] {rel(path)} が無い")
            continue
        found_numbers = {int(number) for number in re.findall(pattern, text)}
        if not found_numbers:
            errors.append(f"[12] {rel(path)} にデスクトップ版の数（N 通り / N desktop builds）が無い")
        for number in sorted(found_numbers):
            if number != DISTRIBUTION_FORMS["デスクトップ版"]:
                errors.append(
                    f"[12] {rel(path)} のデスクトップ版が {number} 通り"
                    f"（{DISTRIBUTION_FORMS['デスクトップ版']} 通りであること）"
                )

    targets = [
        ROOT / "docs/specifications/external.md",
        ROOT / "docs/designs/external.md",
        ROOT / "docs/distribution.md",
        *sorted((ROOT / "docs/users").glob("*.md")),
    ]
    for path in targets:
        for number, line in enumerate(read(path).splitlines(), 1):
            for axis in RETIRED_AXES:
                if axis in line:
                    errors.append(
                        f"[12] {rel(path)}:{number} に廃止した軸「{axis}」がある"
                        "（デスクトップ版は GUI あり / なし × 対象環境 5 の 10 通りだけ）"
                    )
    # Binary forms plus the complete source attachment; not the 18 installation choices.
    artifact_count = sum(DISTRIBUTION_FORMS[name] for name in (
        "デスクトップ版", "Python ライブラリ版", "VSCode 拡張版")) + 1
    for path in (ROOT / "docs/designs/external.md", ROOT / "docs/plans.md"):
        errors.extend(f"[12] {rel(path)}: {message}"
                      for message in stale_release_claims(read(path), artifact_count))
    return errors


CHECKS = {
    "1": ("廃止済みの文書名を参照しない", check_retired),
    "2": ("仕様書の節番号が実在する", check_spec_sections),
    "3": ("history の ID が実在する", check_history_ids),
    "4": ("developers.md の src/ ツリーが実物と一致する", check_source_tree),
    "5": ("verify.sh が CI の静的検査を含む", check_verify_coverage),
    "6": ("ルート文書に丸写しの重複が無い", check_root_documents),
    "7": ("文書の数の主張が実装と一致する", check_claims),
    "8": ("include_str! のパスが実在する", check_include_paths),
    "9": ("リンクとアンカーが実在する", check_links),
    "10": ("Actions を無駄に使わない設定が揃っている", check_actions_budget),
    "11": ("公開する手引きが実在し、参照と段階が揃っている", check_published_guide),
    "12": ("配布形態の数と、廃止した軸が戻っていない", check_distribution_forms),
}


def main() -> int:
    if "--list" in sys.argv:
        for key, (label, _) in CHECKS.items():
            print(f"{key}. {label}")
        return 0

    errors: list[str] = []
    for _, (_, check) in CHECKS.items():
        errors.extend(check())

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        print(f"NG: {len(errors)} 件の不整合（docs/script/reference.py）", file=sys.stderr)
        return 1

    total = len(CHECKS)
    print(
        f"OK: ドキュメントと実装の突き合わせ {total} 項目（廃止名 / 節番号 / history ID / "
        f"src ツリー / verify と CI / ルート文書 / 数の主張 / include_str / リンク / Actions の節約 / 手引き / 配布形態）"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
