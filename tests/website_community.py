from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class CommunitySectionTests(unittest.TestCase):
    def test_overview_uses_localized_community_copy_and_links(self):
        overview = (ROOT / "website/src/components/Overview.astro").read_text()
        translations = (ROOT / "website/src/data/i18n.ts").read_text()
        self.assertIn('translations[lang].community', overview)
        self.assertIn('href="https://github.com/sonneko/novnow/issues"', overview)
        self.assertIn('href="https://github.com/sponsors/sonneko"', overview)
        self.assertIn('title: "コミュニティ・貢献・スポンサー"', translations)
        self.assertIn('title: "Community, Contribute & Sponsor"', translations)
        self.assertIn('aria-labelledby="community-title"', overview)


if __name__ == "__main__":
    unittest.main()
