#!/usr/bin/env python3
"""
docs/script/duplicate.py — ドキュメント管理規則・重複・REF 整合性の機械検証スクリプト。

検査項目:
1. ルートの廃止ファイル（DEVELOPERS.md, PLANS.md, SPECIFICATIONS.md）が存在しないこと。
2. docs/ 配下の必須構成が揃っていること。
3. docs/developers.md にユーザー指示のドキュメント管理規則が原文のまま含まれていること。
4. docs/specifications/**.md にコード情報・他ファイル参照・過去版注記が一切含まれないこと。
5. docs/designs/**.md のファイル集合および ## / ### 見出し構成が docs/specifications/**.md と完全一致し、
   すべての ### 節に実装状態（実装待ち / 実装中 / 実装完了 + REF:src/...）が記録されていること。
6. docs/（specifications 以外）内のすべての REF:<path>[#anchor] が実在するファイルを指していること。
7. docs/history.md の全 ### エントリが grep 検索用の固定ヘッダと [HIST-TAG: ...] を持つこと。
8. 複数ファイル間の重複検査:
   - <!-- SYNC:START <id> --> ... <!-- SYNC:END <id> --> ブロックが全出現箇所で一字一句一致し、
     かつ docs/designs/*.md から集計した実装状況の件数と一致すること。
   - SYNC ブロック外で複数ドキュメント間に重複した段落が存在しないこと。
"""

from __future__ import annotations

import os
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs"

RETIRED_ROOT_FILES = ["DEVELOPERS.md", "PLANS.md", "SPECIFICATIONS.md"]

REQUIRED_DOCS = [
    DOCS / "developers.md",
    DOCS / "handoff.md",
    DOCS / "plans.md",
    DOCS / "history.md",
    DOCS / "script" / "duplicate.py",
]

VERBATIM_RULES_BLOCK = """1. ドキュメント整理
ドキュメントが肥大化してきたため、整理を行う。「最初はXだったが、Yに変更した」のような歴史的情報は読む際のノイズになるため、現行の仕様や設計からは分離し、`docs/history.md` に移動する。この際、`docs/history.md` は先頭から全文を読まなくても `grep` 等で機械的に該当箇所を特定できる構造にすること。

docs/
├ script/ : ドキュメントのルールが守られているか確認するスクリプト群（`duplicate.py` を含む）
├ handoff.md : 開発を始めるにあたって必要な最小限の流動的情報（現在の進捗など）。毎セッション更新する
├ add-dependencies.md : 依存関係を追加する際に見るべき内容。基本変更しない。
├ specifications/
│ └ **.md : 複数のマークダウンファイルに分類された純粋な仕様書（DSLのEBNF、プロジェクトのディレクトリ構造、novnowの応答など、ユーザーから見える仕様のみを記述する。将来的に公開リファレンスとして利用する可能性がある）
├ designs/
│ └ **.md : 複数のマークダウンファイルに分類されたコードの設計書（ファイル構成や章立ては `specifications` と完全に一致させる）
├ plans.md : 一時的な計画や方針、途中の履歴などを記述するファイル（マイルストーン達成時に `history.md` へ移動する）
├ developers.md : 開発環境の構築手順、このプロンプトで指示しているドキュメント管理ルール、ソースコードのディレクトリ構造、スクリプトの使い方、ユーザーの開発の進め方、ArenaAgentの仕様、その他仕様外の注意点など、開発開始時に必要な固定情報
└ history.md : 変更履歴や過去の経緯などを記録するファイル（追記専用）

ドキュメント管理のルールは以下の通りとする。
・複数のドキュメントに重複した情報を記述しない。ただし、実装状況などどうしても重複が避けられない情報については、`docs/script/duplicate.py` によって機械的（決定論的）に内容の同一性が検証されている場合に限り許容する。
・矛盾が生じた場合の優先順位は「`specifications/**.md` > `designs/**.md` > ソースコード」とする。例えば、仕様書にない機能が実装されている場合は、仕様書の記述漏れか不要な実装である可能性を疑う。逆に仕様書にある機能が実装されていない場合は、単に未実装であるか、仕様変更の反映漏れの可能性がある。なお、仕様書や設計書を修正する際は、単なる記述ミスの修正を除き、人間の承認を得ること。
・`specifications/**.md` にはコードに関する情報を一切含めず、純粋な仕様のみを記述する。また、他のファイルへの参照も一切含めない。
・`designs/**.md` の構造は `specifications/**.md` と一致させる。例えば、`specifications/A.md` のB章に対応する実装は、`designs/A.md` のB章に記述する。また、各章には実装状況（実装待ち / 実装中 / 実装完了：（関連するソースコードへの参照必須））を記録する。
・他ファイルへの参照は `specifications/**.md` 以外では積極的に行ってよい。ただし、機械的に管理できるよう、すべての参照には `REF` プレフィックスを付与するなどの工夫を行うこと。
・ここで指示したドキュメント管理ルール自体も `docs/developers.md` にそのまま記載し、明示的な削除指示がない限り削除しないこと。"""

FORBIDDEN_SPEC_PATTERNS = [
    r"\bsrc/",
    r"\btests/",
    r"\bbenches/",
    r"\bCargo\.toml\b",
    r"\bcrate::",
    r"\bcore::",
    r"\bports::",
    r"\badapters::",
    r"\bui::",
    r"\begui\b",
    r"\beframe\b",
    r"\bropey\b",
    r"\baho-corasick\b",
    r"\bserde\b",
    r"\btokio\b",
    r"\bwasm-bindgen\b",
    r"\brfd\b",
    r"\bfontdb\b",
    r"\bcriterion\b",
    r"\.rs\b",
    r"\bPLANS\.md\b",
    r"\bDEVELOPERS\.md\b",
    r"\bSPECIFICATIONS\.md\b",
    r"\bdocs/(?:specifications|designs|script|developers\.md|plans\.md|handoff\.md|history\.md)",
    r"\bREF:",
    r"第\s*[12]\.\d+\s*版",
    r"\]\(",
]


def parse_sections_outside_fences(text: str) -> list[tuple[str, list[str]]]:
    """Return list of (heading_line, body_lines) for ## and ### headings outside code fences."""
    sections: list[tuple[str, list[str]]] = []
    in_fence = False
    current_head: str | None = None
    current_body: list[str] = []
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("```"):
            in_fence = not in_fence
            if current_head is not None:
                current_body.append(line)
            continue
        if not in_fence and (s.startswith("## ") or s.startswith("### ")):
            if current_head is not None:
                sections.append((current_head, current_body))
            current_head = s
            current_body = []
        else:
            if current_head is not None:
                current_body.append(line)
    if current_head is not None:
        sections.append((current_head, current_body))
    return sections


def extract_sync_blocks(text: str) -> dict[str, str]:
    blocks: dict[str, str] = {}
    pattern = re.compile(
        r"<!--\s*SYNC:START\s+([A-Za-z0-9_-]+)\s*-->\n(.*?)\n<!--\s*SYNC:END\s+\1\s*-->",
        re.DOTALL,
    )
    for m in pattern.finditer(text):
        blocks[m.group(1)] = m.group(2).strip()
    return blocks


def strip_sync_and_fences(text: str) -> list[tuple[int, str]]:
    """Return (lineno, stripped_line) outside SYNC blocks and code fences."""
    out: list[tuple[int, str]] = []
    in_fence = False
    in_sync = False
    for idx, line in enumerate(text.splitlines(), 1):
        s = line.strip()
        if s.startswith("<!-- SYNC:START "):
            in_sync = True
            continue
        if s.startswith("<!-- SYNC:END "):
            in_sync = False
            continue
        if s.startswith("```"):
            in_fence = not in_fence
            continue
        if not in_fence and not in_sync:
            out.append((idx, s))
    return out


def main() -> int:
    errors: list[str] = []

    # 1. ルートの旧ファイルが廃止されていること
    for name in RETIRED_ROOT_FILES:
        if (ROOT / name).exists():
            errors.append(f"[rule-1] 廃止されたルートファイルが残っています: {name}")

    # 2. 必須ファイルの存在確認
    for path in REQUIRED_DOCS:
        if not path.is_file():
            errors.append(f"[rule-2] 必須ファイルが存在しません: {path.relative_to(ROOT)}")

    spec_dir = DOCS / "specifications"
    design_dir = DOCS / "designs"
    if not spec_dir.is_dir() or not design_dir.is_dir():
        errors.append("[rule-2] docs/specifications または docs/designs ディレクトリが存在しません")
        for err in errors:
            print(err, file=sys.stderr)
        return 1

    # 3. developers.md に原文ブロックが含まれていること
    dev_path = DOCS / "developers.md"
    if dev_path.is_file():
        dev_text = dev_path.read_text(encoding="utf-8")
        if VERBATIM_RULES_BLOCK not in dev_text:
            errors.append("[rule-3] docs/developers.md にドキュメント管理規則の原文がそのまま記載されていません")

    # 4. specifications/*.md の純粋性検査
    spec_files = sorted(p.name for p in spec_dir.glob("*.md"))
    design_files = sorted(p.name for p in design_dir.glob("*.md"))
    if not spec_files:
        errors.append("[rule-4] docs/specifications/*.md が空です")
    if spec_files != design_files:
        errors.append(
            f"[rule-5] specifications と designs のファイル一覧が一致しません: {spec_files} != {design_files}"
        )

    combined_forbidden = re.compile("|".join(f"(?:{p})" for p in FORBIDDEN_SPEC_PATTERNS))
    for name in spec_files:
        sp = spec_dir / name
        text = sp.read_text(encoding="utf-8")
        for lineno, line in enumerate(text.splitlines(), 1):
            m = combined_forbidden.search(line)
            if m:
                errors.append(
                    f"[rule-4] docs/specifications/{name}:{lineno} に禁止語 '{m.group(0)}' が含まれています: {line.strip()}"
                )

    # 5. specifications と designs の見出し一致 & 実装状態の検査
    status_counts = {
        "実装完了": 0,
        "実装中": 0,
        "未実装・実装計画済み": 0,
        "未実装・未計画": 0,
        "実装待ち": 0,
    }
    status_re = re.compile(
        r"^状態:\s*(実装完了|実装中|未実装・実装計画済み|未実装・未計画|実装待ち)（(.+)）$"
    )

    for name in spec_files:
        sp = spec_dir / name
        dp = design_dir / name
        if not dp.is_file():
            continue
        s_secs = parse_sections_outside_fences(sp.read_text(encoding="utf-8"))
        d_secs = parse_sections_outside_fences(dp.read_text(encoding="utf-8"))
        s_heads = [h for h, _ in s_secs]
        d_heads = [h for h, _ in d_secs]
        if s_heads != d_heads:
            errors.append(
                f"[rule-5] docs/specifications/{name} と docs/designs/{name} の見出し構成が一致しません"
            )
            for idx, (sh, dh) in enumerate(zip(s_heads, d_heads)):
                if sh != dh:
                    errors.append(f"  index {idx}: spec='{sh}' vs design='{dh}'")
                    break
            if len(s_heads) != len(d_heads):
                errors.append(f"  見出し数: spec={len(s_heads)}, design={len(d_heads)}")

        for head, body in d_secs:
            if not head.startswith("### "):
                continue
            non_empty = [l.strip() for l in body if l.strip()]
            if not non_empty:
                errors.append(f"[rule-5] docs/designs/{name} の '{head}' が空です")
                continue
            m = status_re.match(non_empty[0])
            if not m:
                errors.append(
                    f"[rule-5] docs/designs/{name} の '{head}' の冒頭に有効な '状態: ...（REF:...）' 行がありません: {non_empty[0]}"
                )
                continue
            kind, detail = m.group(1), m.group(2)
            status_counts[kind] += 1
            if kind == "実装完了" and "REF:src/" not in detail and "REF:tests/" not in detail:
                errors.append(
                    f"[rule-5] docs/designs/{name} の '{head}' は実装完了ですが REF:src/... への参照がありません: {detail}"
                )
            elif "REF:" not in detail:
                errors.append(
                    f"[rule-5] docs/designs/{name} の '{head}' の状態行に REF: 参照がありません: {detail}"
                )

    # 6. REF: 参照の存在確認（specifications 以外の全 Markdown）
    ref_re = re.compile(r"\bREF:([A-Za-z0-9_./-]+)(?:#([A-Za-z0-9_-]+))?")
    md_link_re = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
    non_spec_mds = [
        DOCS / "developers.md",
        DOCS / "handoff.md",
        DOCS / "plans.md",
        DOCS / "history.md",
        *sorted(design_dir.glob("*.md")),
    ]
    for md_path in non_spec_mds:
        if not md_path.is_file():
            continue
        rel = md_path.relative_to(ROOT)
        text = md_path.read_text(encoding="utf-8")
        for m in ref_re.finditer(text):
            target_rel, anchor = m.group(1), m.group(2)
            # プレースホルダ（REF:src/... 等）はスキップ
            if "..." in target_rel:
                continue
            target_path = ROOT / target_rel
            if not target_path.exists():
                errors.append(f"[rule-6] {rel} の REF:{target_rel} が存在しないパスを指しています")
            elif anchor and target_path.is_file():
                target_text = target_path.read_text(encoding="utf-8")
                if f'id="{anchor}"' not in target_text and f"#{anchor}" not in target_text:
                    errors.append(
                        f"[rule-6] {rel} の REF:{target_rel}#{anchor} のアンカーが対象ファイル内に見つかりません"
                    )
        for lineno, line in strip_sync_and_fences(text):
            for lm in md_link_re.finditer(line):
                link = lm.group(1)
                if not link.startswith(("http://", "https://")):
                    errors.append(
                        f"[rule-6] {rel}:{lineno} に REF: プレフィックスを使わないローカルリンクがあります: {link}"
                    )

    # 7. docs/history.md の grep 検索構造検査
    hist_path = DOCS / "history.md"
    if hist_path.is_file():
        hist_text = hist_path.read_text(encoding="utf-8")
        hist_secs = parse_sections_outside_fences(hist_text)
        head_re = re.compile(r"^### \[HIST:([A-Z-]+):([A-Za-z0-9._-]+)\] \d{4}-\d{2}-\d{2} .+$")
        tag_re = re.compile(r"^\[HIST-TAG: category=([A-Z-]+) id=([A-Za-z0-9._-]+) topics=[^\]]+\]$")
        seen_ids: set[tuple[str, str]] = set()
        h3_count = 0
        for head, body in hist_secs:
            if not head.startswith("### "):
                continue
            h3_count += 1
            hm = head_re.match(head)
            if not hm:
                errors.append(f"[rule-7] docs/history.md の見出しが規約外です: {head}")
                continue
            non_empty = [l.strip() for l in body if l.strip()]
            if not non_empty:
                errors.append(f"[rule-7] docs/history.md の '{head}' の本文が空です")
                continue
            tm = tag_re.match(non_empty[0])
            if not tm:
                errors.append(
                    f"[rule-7] docs/history.md の '{head}' 直下に [HIST-TAG: ...] 行がありません: {non_empty[0]}"
                )
                continue
            if (hm.group(1), hm.group(2)) != (tm.group(1), tm.group(2)):
                errors.append(
                    f"[rule-7] docs/history.md の見出しと HIST-TAG の category/id が不一致です: {head}"
                )
            key = (hm.group(1), hm.group(2))
            if key in seen_ids:
                errors.append(f"[rule-7] docs/history.md に重複した HIST ID があります: {key}")
            seen_ids.add(key)
        if h3_count == 0:
            errors.append("[rule-7] docs/history.md に履歴エントリが存在しません")

    # 8. 重複検査（SYNC ブロックの一致 + designs 集計値との一致 + SYNC 外の段落重複禁止）
    sync_occurrences: dict[str, list[tuple[Path, str]]] = defaultdict(list)
    for md_path in non_spec_mds:
        if not md_path.is_file():
            continue
        for sync_id, content in extract_sync_blocks(md_path.read_text(encoding="utf-8")).items():
            sync_occurrences[sync_id].append((md_path, content))

    if "stage1-status" not in sync_occurrences or len(sync_occurrences["stage1-status"]) < 2:
        errors.append(
            "[rule-8] stage1-status SYNC ブロックが docs/plans.md と docs/handoff.md の双方に存在しません"
        )

    for sync_id, items in sync_occurrences.items():
        base_path, base_content = items[0]
        for other_path, other_content in items[1:]:
            if base_content != other_content:
                errors.append(
                    f"[rule-8] SYNC ブロック '{sync_id}' の内容が {base_path.relative_to(ROOT)} と {other_path.relative_to(ROOT)} で一致しません"
                )
        if sync_id == "stage1-status":
            total = sum(status_counts.values())
            expected_fragment = (
                f"実装完了 {status_counts['実装完了']} 節 / "
                f"実装中 {status_counts['実装中']} 節 / "
                f"未実装・実装計画済み {status_counts['未実装・実装計画済み']} 節 / "
                f"未実装・未計画 {status_counts['未実装・未計画']} 節 / "
                f"実装待ち {status_counts['実装待ち']} 節"
            )
            expected_total = f"合計 {total} 節"
            if expected_fragment not in base_content or expected_total not in base_content:
                errors.append(
                    f"[rule-8] stage1-status SYNC ブロックの集計値が docs/designs/*.md の実測値（{expected_fragment}、{expected_total}）と一致しません"
                )

    # SYNC 外の散文段落（40 文字以上）が複数のファイルに重複していないか検査
    prose_locations: dict[str, list[str]] = defaultdict(list)
    for md_path in non_spec_mds:
        if not md_path.is_file():
            continue
        rel = str(md_path.relative_to(ROOT))
        seen_in_file: set[str] = set()
        for lineno, line in strip_sync_and_fences(md_path.read_text(encoding="utf-8")):
            if len(line) < 40:
                continue
            if line.startswith(("状態:", "|---", "[HIST-TAG:", "#")):
                continue
            if line in seen_in_file:
                continue
            seen_in_file.add(line)
            prose_locations[line].append(f"{rel}:{lineno}")

    for line, locs in prose_locations.items():
        if len(locs) > 1:
            errors.append(
                f"[rule-8] SYNC ブロック外で複数ファイルに重複した記述があります ({', '.join(locs)}): {line[:60]}..."
            )

    if errors:
        for err in errors:
            print(err, file=sys.stderr)
        return 1

    print(
        f"OK: docs rules verified ({len(spec_files)} specifications, {len(design_files)} designs, "
        f"{status_counts['実装完了']} completed / {status_counts['実装中']} in-progress / "
        f"{status_counts['未実装・実装計画済み']} planned / {status_counts['未実装・未計画']} unplanned / "
        f"{status_counts['実装待ち']} stage2-waiting sections, SYNC blocks & REF links valid)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
