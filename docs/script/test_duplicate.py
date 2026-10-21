"""Regression tests for status tables after Accept both changes merges.

Run: python3 -m unittest discover -s docs/script -p 'test_*.py'
"""
import unittest

from duplicate import validate_status_table


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
