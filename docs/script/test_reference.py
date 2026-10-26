"""Regression tests for docs/script/reference.py.

Run: python3 -m unittest discover -s docs/script -p 'test_*.py'

検査器そのものが壊れると「不整合が無い」という結論だけが残るので、
判定の関数を直接叩いて、検出できること・誤検出しないことを固定する。
"""
import unittest
from pathlib import Path

import reference


class HeadingTests(unittest.TestCase):
    def test_numbers_are_read_from_headings(self):
        text = "## 7. 診断カタログ\n### 7.0 コア\n#### 7.0.1 詳細\n### P1. 設計原則\n"
        path = Path("/tmp/novnow-headings.md")
        path.write_text(text, encoding="utf-8")
        self.assertEqual(reference.headings(path), {"7", "7.0", "7.0.1"})

    def test_text_mentions_are_not_headings(self):
        path = Path("/tmp/novnow-headings2.md")
        path.write_text("本文の §7.9 は見出しではない。\n- §8.1 も見出しではない\n", encoding="utf-8")
        self.assertEqual(reference.headings(path), set())


class ChapterMapTests(unittest.TestCase):
    def test_every_specification_is_reachable(self):
        expected = {
            "overview",
            "project",
            "syntax",
            "semantics",
            "diagnostics",
            "editing",
            "ui",
            "platform",
            "agent",
            "workflows",
            "interfaces",
            "external",
        }
        self.assertEqual(set(reference.CHAPTERS.values()), expected)


class SourceTreeTests(unittest.TestCase):
    def test_tree_parser_reads_directories_and_files(self):
        block = (
            "├── src/\n"
            "│   ├── main.rs                # 入口\n"
            "│   ├── core/                  # 純粋\n"
            "│   └── ui/                    # GUI\n"
            "├── tests/\n"
        )
        listed = set()
        for line in block.splitlines():
            item = reference.re.match(r"^│   [├└]── ([a-z_]+(?:/|\.rs))", line)
            if item:
                listed.add(item.group(1).rstrip("/"))
        self.assertEqual(listed, {"main.rs", "core", "ui"})


class RetiredNameTests(unittest.TestCase):
    def test_explanation_lines_are_allowed(self):
        allowed = "旧 `SPECIFICATIONS.md` を廃止した"
        self.assertTrue(any(marker in allowed for marker in reference.RETIRED_LINE_MARKERS))

    def test_plain_reference_is_reported(self):
        plain = "//! 仕様は `SPECIFICATIONS.md` §5.1 にある。"
        self.assertFalse(any(marker in plain for marker in reference.RETIRED_LINE_MARKERS))


class CountPatternTests(unittest.TestCase):
    """「N ワークスペース」の数を、見出し番号や節番号から誤って拾わないこと。"""

    def setUp(self):
        self.pattern = reference.re.compile(r"(?<![0-9.\-§])([0-9]+) ワークスペース")

    def test_plain_sentence(self):
        found = self.pattern.findall("キーボード完結の 9 ワークスペースと統一パレット")
        self.assertEqual(found, ["9"])

    def test_section_number_is_not_a_count(self):
        self.assertEqual(self.pattern.findall("§10 ワークスペース・§11 テーマ"), [])

    def test_acceptance_id_is_not_a_count(self):
        self.assertEqual(self.pattern.findall("**19.14-1 ワークスペース瞬時切替**"), [])


class RetiredScopeTests(unittest.TestCase):
    """検査 1 が「コード側」をどこまで見るか（漏らすと緑のまま残る）。"""

    def test_root_manifests_are_scanned(self):
        for name in ("Cargo.toml", "rust-toolchain.toml", ".github/pull_request_template.md"):
            self.assertIn(name, reference.CODE_ROOT_FILES)
            self.assertTrue((reference.ROOT / name).is_file(), name)

    def test_root_files_are_in_the_target_list(self):
        targets = {reference.rel(p) for p in reference.walk(reference.CODE_DIRS, reference.CODE_SUFFIXES)}
        self.assertIn(".github/workflows/ci.yml", targets)
        self.assertNotIn("Cargo.toml", targets)  # ルート直下は CODE_ROOT_FILES 側で足す


class ActionsBudgetTests(unittest.TestCase):
    """検査 10（Actions を無駄に使わない設定）。"""

    def test_every_workflow_has_concurrency(self):
        for path in sorted((reference.ROOT / ".github/workflows").glob("*.yml")):
            text = reference.read(path)
            self.assertIn("concurrency:", text, path.name)
            self.assertIn("cancel-in-progress", text, path.name)

    def test_draft_guard_is_uniform(self):
        for path in sorted((reference.ROOT / ".github/workflows").glob("*.yml")):
            text = reference.read(path)
            if not reference.workflow_triggers(text) & {"pull_request", "pull_request_target"}:
                continue
            self.assertIn("ready_for_review", text, path.name)
            for job, expr in reference.workflow_job_ifs(text).items():
                self.assertIsNotNone(expr, f"{path.name}: {job}")
                self.assertIn("pull_request.draft", expr, f"{path.name}: {job}")

    def test_workflow_run_jobs_require_success(self):
        for path in sorted((reference.ROOT / ".github/workflows").glob("*.yml")):
            text = reference.read(path)
            if "workflow_run" not in reference.workflow_triggers(text):
                continue
            for job, expr in reference.workflow_job_ifs(text).items():
                self.assertIsNotNone(expr, f"{path.name}: {job}")
                self.assertIn("workflow_run.conclusion == 'success'", expr, f"{path.name}: {job}")

    def test_no_retired_runner(self):
        for path in sorted((reference.ROOT / ".github/workflows").glob("*.yml")):
            self.assertNotRegex(reference.read(path), r"^\s*(runs-on|os):\s*['\"]?macos-13", path.name)

    def test_hook_and_runner_scripts_exist(self):
        hook = reference.ROOT / ".githooks/commit-msg"
        self.assertTrue(hook.is_file())
        self.assertIn("[ci skip]", reference.read(hook))
        self.assertTrue(hook.stat().st_mode & 0o111)
        self.assertTrue((reference.ROOT / "scripts/install-git-hooks.sh").is_file())
        self.assertTrue((reference.ROOT / "scripts/ci-run.sh").is_file())


class WorkflowParseTests(unittest.TestCase):
    """検査 10 のワークフロー走査（ジョブ単位の draft 条件を見る土台）。"""

    SAMPLE = (
        "name: sample\n"
        "on:\n"
        "  push:\n"
        "    branches: [main]\n"
        "  pull_request:\n"
        "    types: [opened, synchronize, reopened, ready_for_review]\n"
        "jobs:\n"
        "  guarded:\n"
        "    if: github.event_name != 'pull_request' || !github.event.pull_request.draft\n"
        "    runs-on: ubuntu-latest\n"
        "    steps:\n"
        "      - run: echo hi\n"
        "        if: always()\n"
        "  via_needs:\n"
        "    needs: guarded\n"
        "    runs-on: ubuntu-latest\n"
        "    steps:\n"
        "      - run: echo hi\n"
    )

    def test_triggers_are_read(self):
        self.assertEqual(reference.workflow_triggers(self.SAMPLE), {"push", "pull_request"})

    def test_shorthand_trigger(self):
        self.assertEqual(
            reference.workflow_triggers("on: [push, pull_request]\n"),
            {"push", "pull_request"},
        )

    def test_job_ifs_are_read_per_job(self):
        ifs = reference.workflow_job_ifs(self.SAMPLE)
        self.assertIn("pull_request.draft", ifs["guarded"])
        # needs 先の skip 頼みは「条件無し」として検出する（draft で実際に走った形）。
        self.assertIsNone(ifs["via_needs"])

    def test_step_if_is_not_a_job_if(self):
        text = (
            "on:\n  pull_request:\njobs:\n  only_step:\n"
            "    runs-on: ubuntu-latest\n    steps:\n"
            "      - run: echo hi\n        if: always()\n"
        )
        self.assertIsNone(reference.workflow_job_ifs(text)["only_step"])


class SlugTests(unittest.TestCase):
    def test_japanese_heading(self):
        self.assertEqual(reference.slug("エディタの仕様の要約"), "エディタの仕様の要約")

    def test_symbols_are_dropped(self):
        # `<a id=...>` を書けば記号を含む見出しでも確実に飛べる（`GUIDE.md` の節はこの形）
        self.assertEqual(reference.slug("4.5 段階（追加仕様）"), "45-段階追加仕様")
        self.assertEqual(reference.slug("`Alt+7` と `F1`"), "alt7-と-f1")

    def test_anchor_from_a_tag(self):
        path = Path("/tmp/novnow-anchors.md")
        path.write_text('<a id="stage-2"></a>\n## 4. 段階 ②\n', encoding="utf-8")
        self.assertIn("stage-2", reference.anchors(path))
        self.assertIn(reference.slug("4. 段階 ②"), reference.anchors(path))


class IncludePathTests(unittest.TestCase):
    def test_missing_include_is_found(self):
        # 実在しないパスを 1 つだけ持つ仮のファイルを作らずに、正規表現だけを確かめる。
        found = reference.re.findall(
            r'include_(?:str|bytes)!\("([^"]+)"\)',
            'const A: &str = include_str!("../docs/specifications/ui.md");\n'
            'const B: &[u8] = include_bytes!("../tests/golden/api_catalog.json");\n',
        )
        self.assertEqual(
            found, ["../docs/specifications/ui.md", "../tests/golden/api_catalog.json"]
        )


class ReleaseDocumentationTests(unittest.TestCase):
    def test_retired_credentials_and_settings_are_detected(self):
        errors = reference.stale_release_claims(
            "PYPI_API_TOKEN novnow.downloadVersion cargo publish` は行わない", 17)
        self.assertEqual(len(errors), 3)

    def test_source_archive_is_included_in_final_count(self):
        self.assertEqual(len(reference.stale_release_claims("VSIX 1 の 16 成果物", 17)), 1)
        self.assertEqual(reference.stale_release_claims("全ソース 1 の 17 成果物", 17), [])

    def test_form_choices_are_not_artifact_counts(self):
        self.assertEqual(reference.stale_release_claims("5 形態・18 通り", 17), [])


if __name__ == "__main__":
    unittest.main()
