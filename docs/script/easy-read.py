#!/usr/bin/env python3
"""docs/script/easy-read.py — ユーザー向け文書（`docs/users/`）を読みやすさで機械的に見る検査。

`docs/developers.md` §4.8 の「書き方」を、人の目ではなく機械で守るための道具。標準ライブラリだけで動く
（CI の runner に何も入れない）。

見るもの（`docs/plans.md` §6 Phase 3）:

1. 1 文 60 字以内（句点で区切る。コードと URL は数えない）
2. 1 段落 3 文以内
3. 漢字の比率が 40% 以下（ひらがな・カタカナ・漢字だけで数える。60 字以上の段落だけ）
4. 禁止語（`AST`・`enum`・`デバウンス`・`シリアライズ`）
5. です・ます調と、だ・である調を 1 段落の中で混ぜない

使い方:

    python3 docs/script/easy-read.py            # 検査（違反があれば終了コード 1）
    python3 docs/script/easy-read.py --list     # 規則の一覧だけ出す

対象は `docs/users/*.md`。仕様書（`docs/specifications/`）は正確さが優先なので対象外。
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
USERS = ROOT / "docs" / "users"

# 規則の値（変えるときは `docs/plans.md` §6 Phase 3 も直す）。
MAX_SENTENCE = 60
MAX_SENTENCES = 3
MAX_KANJI = 0.40
FORBIDDEN = ("AST", "enum", "デバウンス", "シリアライズ")

RULE_LABELS = {
    "sentence": f"1 文 {MAX_SENTENCE} 字以内",
    "paragraph": f"1 段落 {MAX_SENTENCES} 文以内",
    "kanji": f"漢字の比率 {int(MAX_KANJI * 100)}% 以下",
    "forbidden": "禁止語を使わない（" + "・".join(FORBIDDEN) + "）",
    "style": "です・ます調と だ・である調を混ぜない",
}

FENCE = re.compile(r"^```")
INLINE_CODE = re.compile(r"`[^`]*`")
URL = re.compile(r"https?://\S+")
LINK = re.compile(r"\[([^\]]*)\]\([^)]*\)")
SENTENCE_END = re.compile(r"[。！？]")
KANJI = re.compile(r"[\u4e00-\u9fff]")
KANA = re.compile(r"[\u3040-\u30ff]")
POLITE = re.compile(r"(です|ます|ません|でした|ましょう)$")
PLAIN = re.compile(r"(だ|である|だった|だろう|になる|とする)$")


def plain(text: str) -> str:
    """数える前の下ごしらえ: リンクは見出し語だけ、コードと URL は落とす。"""
    text = LINK.sub(r"\1", text)
    text = INLINE_CODE.sub("", text)
    text = URL.sub("", text)
    return text


def paragraphs(markdown: str) -> list[tuple[int, list[str]]]:
    """(開始行, 段落の行の並び)。コード・表・空行で段落を切る。箇条書きは 1 行 1 段落。"""
    found: list[tuple[int, list[str]]] = []
    current: list[str] = []
    start = 0
    in_fence = False
    for number, line in enumerate(markdown.splitlines(), start=1):
        stripped = line.strip()
        if FENCE.match(stripped):
            in_fence = not in_fence
            if current:
                found.append((start, current))
                current = []
            continue
        if in_fence:
            continue
        if not stripped or stripped.startswith("#") or stripped.startswith("|"):
            if current:
                found.append((start, current))
                current = []
            continue
        if re.match(r"^([-*]|\d+\.)\s", stripped) or stripped.startswith(">"):
            if current:
                found.append((start, current))
                current = []
            found.append((number, [stripped]))
            continue
        if not current:
            start = number
        current.append(stripped)
    if current:
        found.append((start, current))
    return found


def sentences(text: str) -> list[str]:
    """句点で文に分ける（句点は残す）。"""
    out = []
    for piece in SENTENCE_END.split(text):
        if piece.strip():
            out.append(piece)
    return out


def kanji_ratio(text: str) -> tuple[float, int]:
    kanji = len(KANJI.findall(text))
    kana = len(KANA.findall(text))
    total = kanji + kana
    if total == 0:
        return 0.0, 0
    return kanji / total, total


def check_file(path: Path) -> list[str]:
    errors: list[str] = []
    rel = path.relative_to(ROOT).as_posix()
    text = path.read_text(encoding="utf-8")

    for word in FORBIDDEN:
        if word.lower() in text.lower():
            errors.append(f"{rel}: 禁止語「{word}」がある（言い換える。{RULE_LABELS['forbidden']}）")

    for start, lines in paragraphs(text):
        joined = plain(" ".join(lines))
        if not joined.strip():
            continue
        found = sentences(joined)
        # 箇条書きは 1 行 1 文のことが多いので、段落の文の数は「3 文を超えたら」だけ見る
        if not lines[0].startswith(("-", "*")) and not re.match(r"^\d+\.", lines[0]) and not lines[0].startswith(">"):
            if len(found) > MAX_SENTENCES:
                errors.append(
                    f"{rel}:{start}: 1 段落に {len(found)} 文ある（{MAX_SENTENCES} 文以内。"
                    f"{RULE_LABELS['paragraph']}）"
                )
        for sentence in found:
            if len(sentence) > MAX_SENTENCE:
                errors.append(
                    f"{rel}:{start}: 1 文が {len(sentence)} 字ある"
                    f"（{RULE_LABELS['sentence']}）: {sentence[:30]}…"
                )
        ratio, counted = kanji_ratio(joined)
        # 短い定義（用語集の 1 行）は漢字が多くて当たり前なので、ある程度の長さの文章だけ見る
        if counted >= 60 and ratio > MAX_KANJI:
            errors.append(
                f"{rel}:{start}: 漢字が {ratio:.0%} ある（{RULE_LABELS['kanji']}）: "
                f"{joined[:24]}…"
            )
        if len(found) >= 2:
            polite = any(POLITE.search(sentence.strip()) for sentence in found)
            plain_style = any(PLAIN.search(sentence.strip()) for sentence in found)
            if polite and plain_style:
                errors.append(
                    f"{rel}:{start}: です・ます調と だ・である調が混ざっている"
                    f"（{RULE_LABELS['style']}）: {joined[:24]}…"
                )

    return errors


def main() -> int:
    if "--list" in sys.argv:
        for label in RULE_LABELS.values():
            print(f"- {label}")
        return 0

    files = sorted(USERS.glob("*.md"))
    if not files:
        print(f"NG: {USERS.relative_to(ROOT)} に .md が無い", file=sys.stderr)
        return 1

    errors: list[str] = []
    for path in files:
        errors.extend(check_file(path))

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        print(f"NG: {len(errors)} 件（docs/script/easy-read.py）", file=sys.stderr)
        return 1

    print(f"OK: ユーザー向け文書 {len(files)} 本が読みやすさの規則を満たしている（docs/script/easy-read.py）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
