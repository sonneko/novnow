"""依存の更新と週次の監査: 公開経路 / 開発経路の分け方と、設定の網羅（#61 提案 D）。"""
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

import reference

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("dependency_audit", ROOT / "scripts/dependency-audit.py")
audit = importlib.util.module_from_spec(spec)
# dataclass は解決時に sys.modules を見るので、実行前に登録しておく。
sys.modules[spec.name] = audit
spec.loader.exec_module(audit)


def payload(**vulnerabilities):
    """`npm audit --json` の形の最小ペイロード。"""
    return {"vulnerabilities": vulnerabilities}


def advisory(name, severity, *, fix=True, title=None, url=None):
    via = []
    if title:
        via.append({"title": title, "url": url or "", "severity": severity})
    return {"severity": severity, "range": ">= 1.0.0", "fixAvailable": fix, "via": via}


class FakeProc:
    def __init__(self, payload, returncode=1, stderr=""):
        self.stdout = json.dumps(payload)
        self.returncode = returncode
        self.stderr = stderr


class SplittingTests(unittest.TestCase):
    """`--omit=dev` との差集合で「公開経路」と「開発経路」を分ける。"""

    def test_published_and_development_are_separated(self):
        full = {
            "shipped": advisory("shipped", "high"),
            "build-tool": advisory("build-tool", "high"),
            "small": advisory("small", "low"),
        }
        production = {"shipped": advisory("shipped", "high")}
        findings = {item.name: item for item in audit.classify(full, production)}
        self.assertTrue(findings["shipped"].published)
        self.assertFalse(findings["build-tool"].published)
        self.assertFalse(findings["small"].published)
        # 重大度の高い順（同じ重大度は名前順）。
        self.assertEqual([item.name for item in audit.classify(full, production)],
                         ["build-tool", "shipped", "small"])

    def test_advisory_evidence_is_kept(self):
        findings = audit.classify(
            {"bad": advisory("bad", "critical", title="Remote code execution", url="https://github.com/advisories/GHSA-xxxx")},
            {"bad": advisory("bad", "critical")},
        )
        self.assertEqual(findings[0].advisories,
                         ("Remote code execution（https://github.com/advisories/GHSA-xxxx）",))

    def test_unfixable_major_is_said_so(self):
        findings = audit.classify({"old": {"severity": "high", "range": "<2", "fixAvailable": {"name": "old", "isSemVerMajor": True}}}, {})
        self.assertIn("major", findings[0].fix_available)

    def test_blocking_only_counts_published_findings(self):
        report = audit.RootReport(
            root="website",
            findings=tuple(audit.classify(
                {
                    "shipped-high": advisory("shipped-high", "high"),
                    "shipped-moderate": advisory("shipped-moderate", "moderate"),
                    "dev-critical": advisory("dev-critical", "critical"),
                },
                {
                    "shipped-high": advisory("shipped-high", "high"),
                    "shipped-moderate": advisory("shipped-moderate", "moderate"),
                },
            )),
        )
        self.assertEqual([item.name for item in report.blocking("high")], ["shipped-high"])
        self.assertEqual({item.name for item in report.blocking("moderate")},
                         {"shipped-high", "shipped-moderate"})
        self.assertEqual(report.counts(published=False)["critical"], 1)
        self.assertEqual(report.counts(published=True)["high"], 1)


class RunnerTests(unittest.TestCase):
    """npm を 2 回問い合わせて分ける（1 回では dev 依存が混ざる）。"""

    def run_audit(self, full, production, *, returncode=1):
        commands = []
        full = payload(**full)
        production = payload(**production)

        def runner(command, **kwargs):
            commands.append((command, kwargs.get("cwd")))
            return FakeProc(production if "--omit=dev" in command else full, returncode=returncode)

        report = audit.audit_root(ROOT / "website", runner=runner)
        return report, commands

    def test_both_queries_are_sent_to_the_root(self):
        report, commands = self.run_audit({"a": advisory("a", "high")}, {})
        self.assertEqual(len(commands), 2)
        self.assertEqual([command for command, _ in commands],
                         [["npm", "audit", "--json"], ["npm", "audit", "--json", "--omit=dev"]])
        self.assertEqual({cwd for _, cwd in commands}, {ROOT / "website"})
        self.assertFalse(report.findings[0].published)
        self.assertEqual(report.blocking("high"), ())

    def test_published_high_blocks(self):
        report, _ = self.run_audit({"a": advisory("a", "high")}, {"a": advisory("a", "high")})
        self.assertEqual([item.name for item in report.blocking("high")], ["a"])

    def test_npm_failure_is_not_swallowed(self):
        def runner(command, **kwargs):
            return FakeProc({}, returncode=2, stderr="npm ERR! network")

        with self.assertRaises(SystemExit) as caught:
            audit.audit_root(ROOT / "website", runner=runner)
        self.assertIn("npm ERR! network", str(caught.exception))

    def test_non_json_output_is_reported(self):
        class Broken(FakeProc):
            def __init__(self):
                super().__init__({}, returncode=0)
                self.stdout = "not json"

        with self.assertRaises(SystemExit) as caught:
            audit.audit_root(ROOT / "website", runner=lambda command, **kwargs: Broken())
        self.assertIn("JSON", str(caught.exception))


class ReportTests(unittest.TestCase):
    def test_summary_separates_the_two_paths(self):
        report = audit.RootReport(root="vscode", findings=tuple(audit.classify(
            {"shipped": advisory("shipped", "high"), "dev": advisory("dev", "moderate")},
            {"shipped": advisory("shipped", "high")},
        )))
        text = audit.summary_markdown([report], "high")
        self.assertIn("| `vscode` |", text)
        self.assertIn("失敗", text)
        self.assertIn("| 公開 |", text)
        self.assertIn("| 開発 |", text)
        self.assertIn("`shipped`", text)

    def test_summary_reports_a_clean_run(self):
        text = audit.summary_markdown([audit.RootReport(root="website", findings=())], "high")
        self.assertIn("OK", text)
        self.assertIn("指摘は 0 件", text)

    def test_issue_body_carries_the_evidence(self):
        report = audit.RootReport(root="vscode", findings=tuple(audit.classify(
            {"shipped": advisory("shipped", "high", title="Prototype pollution", url="https://example.test/GHSA-1")},
            {"shipped": advisory("shipped", "high")},
        )))
        text = audit.issue_body([report], "high")
        self.assertIn("`shipped`", text)
        self.assertIn("Prototype pollution", text)
        self.assertIn("§4.10", text)

    def test_offline_run_writes_reports_and_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            summary = Path(tmp) / "summary.md"
            report = Path(tmp) / "report.json"
            code = audit.main(["--offline", "--summary", str(summary), "--report", str(report)])
            self.assertEqual(code, 0)
            self.assertIn("依存監査", summary.read_text(encoding="utf-8"))
            data = json.loads(report.read_text(encoding="utf-8"))
            self.assertFalse(data["failed"])
            # `webui`（第 2 の画面実装）は 2026-11 に加わった 3 つ目の npm root。
            self.assertEqual(
                {root["root"] for root in data["roots"]}, {"vscode", "website", "webui"}
            )

    def test_unknown_root_is_rejected(self):
        self.assertEqual(audit.main(["--root", "does-not-exist"]), 2)

    def test_roots_are_found_from_the_lockfiles(self):
        # 一覧は実物の `package-lock.json` から作る（足したときに黙って止まらないため）。
        self.assertEqual(
            {root.name for root in audit.npm_roots()}, {"vscode", "website", "webui"}
        )


class DependencyPolicyTests(unittest.TestCase):
    """検査 14（`check_dependency_policy`）: 網羅・段階分け・自動 merge しない・週次の監査。"""

    def fake_read(self, mapping):
        original = reference.read
        self.addCleanup(setattr, reference, "read", original)

        def reader(path):
            key = path.relative_to(reference.ROOT).as_posix()
            return mapping[key] if key in mapping else original(path)

        reference.read = reader

    def read(self, relative):
        return (reference.ROOT / relative).read_text(encoding="utf-8")

    def test_repository_policy_is_complete(self):
        self.assertEqual(reference.check_dependency_policy(), [])

    def test_roots_come_from_the_lockfiles(self):
        roots = reference.dependency_roots()
        self.assertIn(("npm", "/website"), roots)
        self.assertIn(("npm", "/vscode"), roots)
        self.assertIn(("cargo", "/"), roots)
        self.assertIn(("pip", "/python/novnow-py"), roots)
        self.assertIn(("github-actions", "/"), roots)

    def test_unregistered_root_is_reported(self):
        config = self.read(".github/dependabot.yml").replace('    directory: "/vscode"\n', '    directory: "/website"\n')
        self.fake_read({".github/dependabot.yml": config})
        errors = reference.check_dependency_policy()
        self.assertTrue(any("/vscode" in error and "登録されていない" in error for error in errors), errors)

    def test_stale_root_is_reported(self):
        config = self.read(".github/dependabot.yml") + (
            '\n  - package-ecosystem: npm\n    directory: "/gone"\n    target-branch: develop\n'
            '    schedule:\n      interval: weekly\n    groups:\n      minor-and-patch:\n'
            '        update-types:\n          - minor\n          - patch\n'
        )
        self.fake_read({".github/dependabot.yml": config})
        errors = reference.check_dependency_policy()
        self.assertTrue(any("/gone" in error and "実在しない" in error for error in errors), errors)

    def test_major_updates_are_not_grouped(self):
        config = self.read(".github/dependabot.yml").replace(
            "        update-types:\n          - minor\n          - patch",
            "        update-types:\n          - major\n          - minor\n          - patch",
            1,
        )
        self.fake_read({".github/dependabot.yml": config})
        errors = reference.check_dependency_policy()
        self.assertTrue(any("major を minor/patch とまとめている" in error for error in errors), errors)

    def test_updates_target_develop(self):
        config = self.read(".github/dependabot.yml").replace("target-branch: develop", "target-branch: main", 1)
        self.fake_read({".github/dependabot.yml": config})
        errors = reference.check_dependency_policy()
        self.assertTrue(any("target-branch が develop でない" in error for error in errors), errors)

    def test_missing_config_is_reported(self):
        self.fake_read({".github/dependabot.yml": ""})
        self.assertEqual(reference.check_dependency_policy(),
                         ["[14] .github/dependabot.yml が無い（依存の更新 PR が自動で作られない）"])

    def test_auto_merge_step_is_rejected(self):
        workflow = self.read(".github/workflows/dependency-audit.yml")
        self.fake_read({".github/workflows/dependency-audit.yml": workflow + "\n      - run: gh pr merge 12 --auto --squash\n"})
        errors = reference.check_dependency_policy()
        self.assertTrue(any("自動 merge" in error for error in errors), errors)

    def test_commented_mention_is_not_a_setting(self):
        # dependabot.yml のコメントは「自動 merge はしない」という説明である（コメント行は読まない）。
        self.assertIn("gh pr merge --auto", self.read(".github/dependabot.yml"))
        self.assertEqual(reference.check_dependency_policy(), [])

    def test_audit_workflow_must_stay_weekly_and_report(self):
        self.fake_read({".github/workflows/dependency-audit.yml": (
            "name: Dependency audit\non:\n  pull_request:\npermissions:\n  contents: read\n"
            "concurrency:\n  group: x\n  cancel-in-progress: true\njobs:\n  npm:\n    runs-on: ubuntu-24.04\n"
        )})
        errors = reference.check_dependency_policy()
        joined = "\n".join(errors)
        for expected in ["schedule が無い", "cron が週 1 回", "PR や push で走る",
                         "scripts/dependency-audit.py を実行していない", "check advisories",
                         "Issue にする", "issues: write"]:
            self.assertIn(expected, joined)

    def test_published_path_split_is_required(self):
        script = self.read("scripts/dependency-audit.py").replace('"--omit=dev"', '"--include=dev"')
        self.fake_read({"scripts/dependency-audit.py": script})
        errors = reference.check_dependency_policy()
        self.assertTrue(any("公開経路" in error and "--omit=dev" in error for error in errors), errors)

    def test_deny_config_is_required(self):
        self.fake_read({"deny.toml": "[licenses]\nallow = [\"MIT\"]\n"})
        errors = reference.check_dependency_policy()
        self.assertTrue(any("`[advisories]` が無い" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
