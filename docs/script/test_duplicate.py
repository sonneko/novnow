"""Regression tests for status tables after Accept both changes merges.

Run: python3 -m unittest discover -s docs/script -p 'test_*.py'
"""
import unittest

from duplicate import bare_module_files, validate_bare_module_files, validate_status_table


class StatusTableTests(unittest.TestCase):
    def setUp(self):
        self.counts = {
            "実装完了": 126,
            "実装中": 0,
            "未実装・実装計画済み": 8,
            "未実装・未計画": 0,
            "実装待ち": 0,
        }
        self.row = (
            "| 設計書セクション実装状況 | "
            + " / ".join(f"{name} {count} 節" for name, count in self.counts.items())
            + " | 全 11 設計書の `### ` 節（合計 134 節） | `REF:docs/designs/` |"
        )

    def validate(self, text):
        return validate_status_table(text, self.counts, 11)

    def test_current_table(self):
        self.assertEqual(self.validate(self.row + "\n| 段階 ② | 完了 | 対象 | 参照 |"), [])

    def test_both_changes_cannot_keep_two_aggregates(self):
        stale = self.row.replace("126 節", "123 節")
        self.assertTrue(self.validate(self.row + "\n" + stale))

    def test_duplicate_feature_even_with_correct_aggregate(self):
        self.assertTrue(self.validate(self.row + "\n| L-01 | 完了 | 対象 | 参照 |\n| L-01 | 未実装 | 対象 | 参照 |"))

    def test_stale_count(self):
        self.assertTrue(self.validate(self.row.replace("126 節", "123 節")))

    def test_stale_file_count(self):
        self.assertTrue(self.validate(self.row.replace("全 11", "全 10")))

    def test_stale_total(self):
        self.assertTrue(self.validate(self.row.replace("合計 134", "合計 126")))

    def test_missing_aggregate(self):
        self.assertTrue(self.validate("| L-01 | 完了 | 対象 | 参照 |"))

    def test_counts_cannot_be_hidden_in_another_row(self):
        self.assertTrue(self.validate(self.row.replace("126 節", "123 節") + "\n| 備考 | " + self.row[1:]))


if __name__ == "__main__":
    unittest.main()


class BareModuleFileTests(unittest.TestCase):
    """rule-9: `REF:<dir>/mod.rs` の行に並ぶ素の `<file>.rs` の実在検査。

    `REF:` を付けないファイル名は rule-6 では検査できないため、ここで見る。
    実在しないファイル（かつての `review.rs` / `proposals.rs` / `search.rs`）を
    挙げ続けたまま腐るのを防ぐ。
    """

    FILES = {"src/core/features": {"project.rs", "sections.rs", "cast.rs"}}

    def exists(self, directory, name):
        return name in self.FILES.get(directory, set())

    def validate(self, text):
        return validate_bare_module_files(text, self.exists, label="docs/designs/overview.md")

    def test_existing_files_pass(self):
        line = "- `REF:src/core/features/mod.rs`: 補助（`project.rs`, `sections.rs`, `cast.rs`）。\n"
        self.assertEqual(self.validate(line), [])

    def test_a_file_that_does_not_exist_is_reported(self):
        line = "- `REF:src/core/features/mod.rs`: 補助（`project.rs`, `review.rs`）。\n"
        errors = self.validate(line)
        self.assertEqual(len(errors), 1)
        self.assertIn("review.rs", errors[0])
        self.assertIn("docs/designs/overview.md", errors[0])
        self.assertIn("src/core/features/", errors[0])

    def test_lines_without_a_mod_ref_are_ignored(self):
        # `REF:` の付いた完全パスは rule-6 が検証する。ここでは素の言及だけを見る
        line = "- `REF:src/cli/mod.rs`, `REF:src/cli/open.rs`: CLI サブコマンド群。\n"
        self.assertEqual(self.validate(line), [])
        self.assertEqual(bare_module_files(line), [])

    def test_any_of_the_directories_on_the_line_is_enough(self):
        # 1 行に 2 つの `mod.rs` 参照があれば、どちらかに実在すればよい
        files = {"src/app": {"session.rs"}, "src/core": {"diagnostics.rs"}}
        line = "- `REF:src/app/mod.rs` と `REF:src/core/mod.rs`（`session.rs`, `diagnostics.rs`）。\n"
        errors = validate_bare_module_files(
            line, lambda directory, name: name in files.get(directory, set())
        )
        self.assertEqual(errors, [])

    def test_fenced_code_blocks_are_not_checked(self):
        text = "```\n`REF:src/core/features/mod.rs`（`nope.rs`）\n```\n"
        self.assertEqual(bare_module_files(text), [])
