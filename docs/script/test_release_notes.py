"""リリースノートの定型（変更点 / 参照 / 互換性 / 既知の問題）と配線の検査（#61 提案 E）。"""
import importlib.util
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("release_notes", ROOT / "scripts/release-notes.py")
notes = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notes)

GOOD = """# novnow v0.2.0

保存の原子性を直した版。

## 変更点

- 保存を一時ファイルと rename にした（§8.3）。

## 参照（Issue・PR）

- Issue #61: 監査
- PR #62: 修正

## 互換性

- `.cnov` と `project.toml` の形式変更は無い。

## 既知の問題

- 掲載中の問題: 無し。
- 一覧: <https://github.com/sonneko/novnow/blob/main/docs/known-issues/ja.md>

## 配布物

- 5 形態。
"""


def without(text, needle):
    """`needle` の行を落とした本文（節を消す・リンクを消す試験に使う）。"""
    return "\n".join(line for line in text.splitlines() if needle not in line) + "\n"


class TemplateTests(unittest.TestCase):
    def test_template_has_the_required_sections(self):
        template = notes.read(notes.TEMPLATE)
        for title in [*notes.REQUIRED_SECTIONS, *notes.RECOMMENDED_SECTIONS]:
            self.assertIn(f"## {title}", template)
        self.assertIn(notes.KNOWN_ISSUES_TARGET, template)
        self.assertIn("{version}", template)

    def test_new_note_is_created_from_the_template(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp) / "release-notes"
            directory.mkdir()
            template = directory / "TEMPLATE.md"
            template.write_text(notes.read(notes.TEMPLATE), encoding="utf-8")
            original_dir, original_template = notes.NOTES_DIR, notes.TEMPLATE
            notes.NOTES_DIR, notes.TEMPLATE = directory, template
            self.addCleanup(setattr, notes, "NOTES_DIR", original_dir)
            self.addCleanup(setattr, notes, "TEMPLATE", original_template)

            path = notes.new_notes("0.2.0")
            self.assertEqual(path.name, "v0.2.0.md")
            self.assertIn("# novnow v0.2.0", path.read_text(encoding="utf-8"))
            # 書きかけはそのままでは検査を通らない（公開前に埋めさせる）。
            self.assertTrue(notes.check_notes("0.2.0", path.read_text(encoding="utf-8")))
            with self.assertRaises(SystemExit):
                notes.new_notes("0.2.0")


class NoteTests(unittest.TestCase):
    def test_a_complete_note_passes(self):
        self.assertEqual(notes.check_notes("0.2.0", GOOD), [])

    def test_a_missing_note_is_reported_with_the_command(self):
        errors = notes.check_notes("9.9.9", "")
        self.assertEqual(len(errors), 1)
        self.assertIn("--new --version 9.9.9", errors[0])

    def test_the_heading_must_carry_the_same_version(self):
        errors = notes.check_notes("0.3.0", GOOD)
        self.assertTrue(any("H1" in error for error in errors), errors)

    def test_each_required_section_is_reported(self):
        for title in notes.REQUIRED_SECTIONS:
            errors = notes.check_notes("0.2.0", without(GOOD, f"## {title}"))
            self.assertTrue(any(f"`## {title}`" in error for error in errors), (title, errors))

    def test_the_order_of_the_required_sections_matters(self):
        swapped = GOOD.replace("## 変更点", "## 変更点X").replace("## 互換性", "## 変更点")
        swapped = swapped.replace("## 変更点X", "## 互換性")
        errors = notes.check_notes("0.2.0", swapped)
        self.assertTrue(any("順が違う" in error for error in errors), errors)

    def test_template_placeholders_are_reported(self):
        errors = notes.check_notes("0.2.0", GOOD.replace("- 5 形態。", "- {形態と数}。{version}"))
        self.assertTrue(any("書きかけ" in error for error in errors), errors)

    def test_todo_is_reported(self):
        errors = notes.check_notes("0.2.0", GOOD.replace("- 5 形態。", "- TODO: 書く"))
        self.assertTrue(any("TODO" in error for error in errors), errors)

    def test_changes_need_at_least_one_item(self):
        errors = notes.check_notes("0.2.0", GOOD.replace("- 保存を一時ファイルと rename にした（§8.3）。", ""))
        self.assertTrue(any("箇条書き" in error for error in errors), errors)

    def test_references_need_an_issue_or_a_pull_request(self):
        body = GOOD.replace("- Issue #61: 監査", "- 監査の記録").replace("- PR #62: 修正", "- 修正")
        errors = notes.check_notes("0.2.0", body)
        self.assertTrue(any("Issue / PR の参照" in error for error in errors), errors)

    def test_a_release_url_counts_as_a_reference(self):
        body = GOOD.replace("- Issue #61: 監査", "- <https://github.com/sonneko/novnow/releases/tag/v0.1.2>")
        self.assertEqual(notes.check_notes("0.2.0", body), [])

    def test_compatibility_must_say_something(self):
        errors = notes.check_notes("0.2.0", without(GOOD, "`.cnov` と `project.toml` の形式変更は無い"))
        self.assertTrue(any("`## 互換性` が空" in error for error in errors), errors)

    def test_known_issues_must_link_the_list(self):
        errors = notes.check_notes("0.2.0", without(GOOD, notes.KNOWN_ISSUES_TARGET))
        self.assertTrue(any(f"{notes.KNOWN_ISSUES_TARGET} へのリンクが無い" in error for error in errors), errors)

    def test_known_issues_must_not_be_empty(self):
        body = notes.section_body(GOOD, "既知の問題")
        errors = notes.check_notes("0.2.0", GOOD.replace(body, "\n"))
        self.assertTrue(any("`## 既知の問題` が空" in error for error in errors), errors)

    def test_a_ref_link_to_the_known_issues_also_counts(self):
        body = GOOD.replace(
            "- 一覧: <https://github.com/sonneko/novnow/blob/main/docs/known-issues/ja.md>",
            "- 一覧: `REF:docs/known-issues/ja.md`",
        )
        self.assertEqual(notes.check_notes("0.2.0", body), [])

    def test_headings_inside_a_code_block_are_not_sections(self):
        body = GOOD.replace("- 5 形態。", "- 5 形態。\n\n```\n## 変更点\n```")
        self.assertEqual(notes.headings(body).count((2, "変更点")), 1)


class LegacyTests(unittest.TestCase):
    def test_the_published_v0_1_0_body_is_kept_as_it_was(self):
        text = notes.read(notes.notes_path("0.1.0"))
        self.assertTrue(text.lstrip().startswith(notes.LEGACY_MARKER))
        self.assertEqual(notes.check_notes("0.1.0", text), [])

    def test_legacy_still_checks_the_heading(self):
        text = notes.read(notes.notes_path("0.1.0")).replace("# novnow v0.1.0", "# novnow v9.9.9")
        self.assertTrue(any("H1" in error for error in notes.check_notes("0.1.0", text)))

    def test_printing_drops_the_marker(self):
        printed = notes.print_notes("0.1.0")
        self.assertTrue(printed.startswith("# novnow v0.1.0"))
        self.assertNotIn("<!--", printed)


class RepositoryTests(unittest.TestCase):
    def fake_read(self, mapping):
        original = notes.read
        self.addCleanup(setattr, notes, "read", original)

        def reader(path):
            key = path.relative_to(notes.ROOT).as_posix()
            return mapping[key] if key in mapping else original(path)

        notes.read = reader

    def test_repository_notes_and_wiring_are_complete(self):
        self.assertEqual(notes.wiring_errors(), [])
        for version, path in notes.existing_notes():
            self.assertEqual(notes.check_notes(version, notes.read(path)), [], path.name)

    def test_every_published_release_has_a_note(self):
        versions = {version for version, _ in notes.existing_notes()}
        self.assertLessEqual({"0.1.0", "0.1.1", "0.1.2"}, versions)

    def test_a_one_line_release_body_is_rejected(self):
        workflow = notes.read(notes.RELEASE_WORKFLOW).replace(
            '            python3 scripts/release-notes.py --print --version "$VERSION" > release-notes.md\n', ""
        ).replace(
            '--notes-file release-notes.md', '--notes "novnow $VERSION distribution artifacts"'
        )
        self.fake_read({".github/workflows/release.yml": workflow})
        errors = notes.wiring_errors()
        self.assertTrue(any("--notes-file" in error for error in errors), errors)

    def test_missing_gate_in_prepare_is_rejected(self):
        workflow = without(notes.read(notes.RELEASE_WORKFLOW), "release-notes.py --check")
        self.fake_read({".github/workflows/release.yml": workflow})
        errors = notes.wiring_errors()
        self.assertTrue(any("prepare" in error for error in errors), errors)

    def test_the_guide_must_link_the_notes(self):
        self.fake_read({"docs/distribution.md": without(notes.read(notes.GUIDE), "docs/release-notes/")})
        errors = notes.wiring_errors()
        self.assertTrue(any("docs/distribution.md" in error for error in errors), errors)

    def test_the_known_issues_must_link_the_notes(self):
        self.fake_read({"docs/known-issues/ja.md": without(notes.read(notes.KNOWN_ISSUES), "docs/release-notes/")})
        errors = notes.wiring_errors()
        self.assertTrue(any("known-issues/ja.md" in error for error in errors), errors)

    def test_ci_and_verify_must_run_check_all(self):
        self.fake_read({
            ".github/workflows/ci.yml": without(notes.read(notes.CI_WORKFLOW), "release-notes.py --check-all"),
            "scripts/verify.sh": without(notes.read(notes.VERIFY), "release-notes.py"),
        })
        errors = notes.wiring_errors()
        self.assertTrue(any("ci.yml" in error for error in errors), errors)
        self.assertTrue(any("verify.sh" in error for error in errors), errors)

    def test_release_check_sh_is_a_local_gate(self):
        self.fake_read({"scripts/release-check.sh": without(notes.read(notes.RELEASE_CHECK), "release-notes.py")})
        errors = notes.wiring_errors()
        self.assertTrue(any("release-check.sh" in error for error in errors), errors)

    def test_check_all_reports_a_broken_note(self):
        broken = notes.read(notes.notes_path("0.1.2")).replace("## 互換性", "## 兼容性")
        self.fake_read({"docs/release-notes/v0.1.2.md": broken})
        errors = []
        for version, path in notes.existing_notes():
            errors.extend(notes.check_notes(version, notes.read(path)))
        self.assertTrue(any("v0.1.2" in error for error in errors), errors)

    def test_main_check_all_passes(self):
        self.assertEqual(notes.main(["--check-all"]), 0)

    def test_main_check_fails_for_an_unknown_version(self):
        self.assertEqual(notes.main(["--check", "--version", "9.9.9"]), 1)


if __name__ == "__main__":
    unittest.main()
