#!/usr/bin/env python3
"""npm の依存監査を「公開経路」と「開発経路」に分けて報告する（`docs/developers.md` §4.10）。

`npm audit` は dev 依存（ビルドツール・テストランナー）の指摘も混ぜて返すため、そのまま
CI を赤くすると「公開される成果物に入っていない指摘」で止まる。このスクリプトは
同じ lockfile に対して 2 回問い合わせ、差集合で分ける。

- **公開経路**（`npm audit --omit=dev`）: 配布物・公開サイトに入る依存。
  `--fail-on`（既定 `high`）以上の指摘があれば exit 1。
- **開発経路**（上の差集合）: 報告はするが失敗させない。実害の判断は人が行う
  （ビルドマシンで悪用できるか、公開成果物に到達しないか）。

Rust 側は `cargo deny check advisories`（`REF:.github/workflows/dependency-audit.yml` の
`rust` ジョブ、設定は `REF:deny.toml`）が別に見る。更新 PR は `REF:.github/dependabot.yml`。
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKIP_PARTS = {"node_modules", "dist", "target", "build", ".git", "vendor"}
SEVERITIES = ("info", "low", "moderate", "high", "critical")
DEFAULT_THRESHOLD = "high"


def rank(severity: str) -> int:
    """重大度の順序（未知の値は low 扱いで 1）。"""
    try:
        return SEVERITIES.index(str(severity).lower())
    except ValueError:
        return 1


def npm_roots(root: Path = ROOT) -> list[Path]:
    """`package-lock.json` を持つディレクトリ（生成物・vendor は除く）。

    新しいフロントエンドの root を足したときに Dependabot と週次監査が取りこぼさないよう、
    一覧は書かずに実物から作る。`REF:docs/script/reference.py` の検査 14 が
    `REF:.github/dependabot.yml` の網羅と突き合わせる。
    """
    found: list[Path] = []
    for lock in sorted(root.rglob("package-lock.json")):
        parts = set(lock.relative_to(root).parts[:-1])
        if parts & SKIP_PARTS:
            continue
        found.append(lock.parent)
    return found


@dataclass(frozen=True)
class Finding:
    """1 件の指摘。`published` が真なら公開経路に入る。"""

    name: str
    severity: str
    range: str
    fix_available: str
    published: bool
    advisories: tuple[str, ...]

    @property
    def rank(self) -> int:
        return rank(self.severity)


def advisories_of(via: object) -> tuple[str, ...]:
    """`via` の中の advisory（dict 形）だけを取り出す。文字列は依存の名前なので無視する。"""
    items: list[str] = []
    if isinstance(via, list):
        for entry in via:
            if isinstance(entry, dict):
                title = str(entry.get("title") or "").strip()
                url = str(entry.get("url") or "").strip()
                if title:
                    items.append(f"{title}（{url}）" if url else title)
    return tuple(items)


def classify(full: dict[str, dict], production: dict[str, dict]) -> list[Finding]:
    """全体の指摘と `--omit=dev` の指摘から、公開経路と開発経路を分ける。"""
    findings: list[Finding] = []
    for name, info in full.items():
        if not isinstance(info, dict):
            continue
        fix = info.get("fixAvailable")
        fix_available = "あり" if fix is True else ("なし" if fix in (False, None) else str(fix.get("name", "")) + " へ major")
        findings.append(
            Finding(
                name=str(name),
                severity=str(info.get("severity") or "info"),
                range=str(info.get("range") or ""),
                fix_available=fix_available,
                published=name in production,
                advisories=advisories_of(info.get("via")),
            )
        )
    # 公開経路だけに出た名前（full に無い＝あり得ないが、来たら公開側を正とする）。
    for name, info in production.items():
        if isinstance(info, dict) and name not in full:
            findings.append(
                Finding(
                    name=str(name),
                    severity=str(info.get("severity") or "info"),
                    range=str(info.get("range") or ""),
                    fix_available="あり" if info.get("fixAvailable") is True else "なし",
                    published=True,
                    advisories=advisories_of(info.get("via")),
                )
            )
    findings.sort(key=lambda item: (-item.rank, item.name))
    return findings


@dataclass(frozen=True)
class RootReport:
    """1 つの npm root の監査結果。"""

    root: str
    findings: tuple[Finding, ...]
    scanned: bool = True

    @property
    def published(self) -> tuple[Finding, ...]:
        return tuple(item for item in self.findings if item.published)

    @property
    def development(self) -> tuple[Finding, ...]:
        return tuple(item for item in self.findings if not item.published)

    def blocking(self, threshold: str) -> tuple[Finding, ...]:
        """公開経路のうち、失敗させる重大度以上のもの。"""
        limit = rank(threshold)
        return tuple(item for item in self.published if item.rank >= limit)

    def counts(self, published: bool) -> dict[str, int]:
        source = self.published if published else self.development
        counts = {severity: 0 for severity in SEVERITIES}
        for item in source:
            key = item.severity.lower()
            counts[key if key in counts else "info"] += 1
        return counts


def audit_root(
    root: Path,
    npm: str = "npm",
    *,
    runner: object = subprocess.run,
) -> RootReport:
    """`npm audit --json` を 2 回（全体・`--omit=dev`）実行して分ける。

    npm は指摘があると exit 1 を返すので、0 と 1 は正常として扱う。
    """
    label = root.name if root != ROOT else "."
    full = _vulnerabilities(root, [npm, "audit", "--json"], label, runner=runner)
    production = _vulnerabilities(
        root, [npm, "audit", "--json", "--omit=dev"], label, runner=runner
    )
    return RootReport(root=label, findings=tuple(classify(full, production)))


def _vulnerabilities(
    root: Path, command: list[str], label: str, *, runner: object
) -> dict[str, dict]:
    proc = runner(command, cwd=root, capture_output=True, text=True)  # type: ignore[operator]
    code = getattr(proc, "returncode", 0)
    if code not in (0, 1):
        stderr = str(getattr(proc, "stderr", "")).strip()[:400]
        raise SystemExit(f"{label}: `{' '.join(command)}` が失敗した（exit {code}）: {stderr}")
    stdout = str(getattr(proc, "stdout", "") or "")
    try:
        payload = json.loads(stdout or "{}")
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{label}: `{' '.join(command)}` の出力が JSON ではなかった（{exc}）")
    vulns = payload.get("vulnerabilities") if isinstance(payload, dict) else None
    return {str(k): v for k, v in (vulns or {}).items() if isinstance(v, dict)}


def summary_markdown(reports: list[RootReport], threshold: str) -> str:
    """`$GITHUB_STEP_SUMMARY` に出す Markdown（公開経路と開発経路を分けた表）。"""
    lines = [
        "## 依存監査（週次）",
        "",
        f"公開経路（`--omit=dev`）に **{threshold} 以上**があれば失敗する。開発経路は報告のみ。",
        "",
        "| npm root | 公開経路 | 開発経路 | 判定 |",
        "|---|---|---|---|",
    ]
    for report in reports:
        pub = report.counts(published=True)
        dev = report.counts(published=False)
        blocking = report.blocking(threshold)
        verdict = "失敗" if blocking else "OK"
        lines.append(
            f"| `{report.root}` | {pub['critical']}C / {pub['high']}H / {pub['moderate']}M / {pub['low']}L "
            f"| {dev['critical']}C / {dev['high']}H / {dev['moderate']}M / {dev['low']}L | {verdict} |"
        )
    details = [item for report in reports for item in report.findings]
    if details:
        lines += ["", "### 指摘の一覧", "", "| root | 経路 | パッケージ | 重大度 | 範囲 | 修正 |", "|---|---|---|---|---|---|"]
        for report in reports:
            for item in report.findings:
                path = "公開" if item.published else "開発"
                lines.append(
                    f"| `{report.root}` | {path} | `{item.name}` | {item.severity} | {item.range} | {item.fix_available} |"
                )
        lines.append("")
    else:
        lines += ["", "指摘は 0 件だった。", ""]
    return "\n".join(lines)


def issue_body(reports: list[RootReport], threshold: str) -> str:
    """Issue に貼る本文（公開経路の失敗があるときだけ作る）。"""
    blocking = [(report, item) for report in reports for item in report.blocking(threshold)]
    lines = [
        f"週次の依存監査（`REF:.github/workflows/dependency-audit.yml`）が公開経路に {threshold} 以上の指摘を見つけた。",
        "",
        f"- 実行日: {date.today().isoformat()}",
        f"- run: {_run_url()}",
        "",
        "| npm root | パッケージ | 重大度 | 範囲 | 修正 | 根拠 |",
        "|---|---|---|---|---|---|",
    ]
    for report, item in blocking:
        evidence = "<br>".join(item.advisories) or "-"
        lines.append(
            f"| `{report.root}` | `{item.name}` | {item.severity} | {item.range} | {item.fix_available} | {evidence} |"
        )
    lines += [
        "",
        "対応: 更新 PR（Dependabot）を待つだけでなく、`npm audit fix` の対象か、major の上げが必要か、",
        "あるいは公開経路から外せるかを `REF:docs/developers.md` §4.10 の手順で確かめる。",
        "開発経路のみの指摘は失敗させていない（上の表に無い）。",
    ]
    return "\n".join(lines) + "\n"


def _run_url() -> str:
    server = os.environ.get("GITHUB_SERVER_URL", "https://github.com")
    repo = os.environ.get("GITHUB_REPOSITORY", "")
    run_id = os.environ.get("GITHUB_RUN_ID", "")
    if repo and run_id:
        return f"{server}/{repo}/actions/runs/{run_id}"
    return f"{server}/{repo}" if repo else server


def report_json(reports: list[RootReport], threshold: str) -> dict:
    """成果物として残す JSON（run 間の比較に使う）。"""
    return {
        "generated_at": date.today().isoformat(),
        "threshold": threshold,
        "failed": any(report.blocking(threshold) for report in reports),
        "roots": [
            {
                "root": report.root,
                "published": [vars(item) for item in report.published],
                "development": [vars(item) for item in report.development],
                "blocking": [item.name for item in report.blocking(threshold)],
            }
            for report in reports
        ],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", action="append", default=[], help="監査する npm root（省略時は全部探す）")
    parser.add_argument("--npm", default="npm", help="npm 実行ファイル（既定 npm）")
    parser.add_argument(
        "--fail-on",
        default=DEFAULT_THRESHOLD,
        choices=[*SEVERITIES, "none"],
        help="公開経路で失敗させる重大度（既定 high。none で失敗させない）",
    )
    parser.add_argument("--summary", help="Markdown を追記するファイル（$GITHUB_STEP_SUMMARY）")
    parser.add_argument("--report", help="JSON を書くファイル")
    parser.add_argument("--issue-body", help="失敗したとき Issue 本文を書くファイル")
    parser.add_argument("--offline", action="store_true", help="npm を実行せず 0 件として報告する（設定検査用）")
    args = parser.parse_args(argv)

    if args.root:
        roots = [Path(item) if Path(item).is_absolute() else ROOT / item for item in args.root]
        missing = [str(item) for item in roots if not item.is_dir()]
        if missing:
            print(f"NG: npm root が実在しない: {', '.join(missing)}", file=sys.stderr)
            return 2
    else:
        roots = npm_roots()
        if not roots:
            print("NG: package-lock.json が見つからない（監査の対象が無い）", file=sys.stderr)
            return 2

    threshold = "critical" if args.fail_on == "none" else args.fail_on
    reports: list[RootReport] = []
    for root in roots:
        if args.offline:
            reports.append(RootReport(root=root.name if root != ROOT else ".", findings=()))
            continue
        reports.append(audit_root(root, args.npm))

    text = summary_markdown(reports, threshold)
    print(text)
    if args.summary:
        Path(args.summary).write_text(text, encoding="utf-8")
    if args.report:
        Path(args.report).write_text(
            json.dumps(report_json(reports, threshold), ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )

    blocking = [item for report in reports for item in report.blocking(threshold)]
    if blocking and args.fail_on != "none":
        if args.issue_body:
            Path(args.issue_body).write_text(issue_body(reports, threshold), encoding="utf-8")
        names = ", ".join(f"{item.name}（{item.severity}）" for item in blocking)
        print(f"NG: 公開経路に {threshold} 以上の指摘が {len(blocking)} 件ある: {names}", file=sys.stderr)
        return 1
    development = [item for report in reports for item in report.development]
    print(
        f"OK: 公開経路に {threshold} 以上の指摘は無い"
        f"（開発経路 {len(development)} 件は報告のみ。npm root {len(reports)} 件）"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
