"""`easy-read.py`（ユーザー向け文書の読みやすさ）の回帰テスト。

Run: python3 -B -m unittest discover -s docs/script -p 'test_*.py'
"""
import unittest

import importlib.util
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "easy-read.py"
spec = importlib.util.spec_from_file_location("easy_read", SCRIPT)
easy_read = importlib.util.module_from_spec(spec)
sys.modules["easy_read"] = easy_read
spec.loader.exec_module(easy_read)


class EasyReadTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(easy_read.ROOT)
        self.tmp = self.root / "docs" / "users" / "_tmp_check.md"
        self.addCleanup(lambda: self.tmp.unlink(missing_ok=True))

    def errors_for(self, text):
        self.tmp.write_text(text, encoding="utf-8")
        return easy_read.check_file(self.tmp)

    def test_short_sentences_pass(self):
        self.assertEqual(self.errors_for("短い文です。もう一つ短い文です。\n"), [])

    def test_long_sentence_is_reported(self):
        text = "あ" * 70 + "。\n"
        errors = self.errors_for(text)
        self.assertTrue(any("1 文が" in error for error in errors), errors)

    def test_four_sentences_in_a_paragraph_are_reported(self):
        text = "一。二。三。四。\n"
        errors = self.errors_for(text)
        self.assertTrue(any("1 段落に" in error for error in errors), errors)

    def test_forbidden_word_is_reported(self):
        errors = self.errors_for("これは enum です。\n")
        self.assertTrue(any("禁止語" in error for error in errors), errors)

    def test_mixed_style_is_reported(self):
        text = "これは良いです。それは良くないのだ。\n"
        errors = self.errors_for(text)
        self.assertTrue(any("混ざっている" in error for error in errors), errors)

    def test_code_and_urls_are_not_counted(self):
        text = "`" + "あ" * 80 + "` と https://example.com/" + "a" * 80 + " を見る。\n"
        self.assertEqual(self.errors_for(text), [])


if __name__ == "__main__":
    unittest.main()
