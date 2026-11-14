import gettext
import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOMAIN = "habit-trackers-by-madhab"


def parse_po(path):
    entries = {}
    current_id = None
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("msgid "):
            current_id = json.loads(line[6:])
        elif line.startswith("msgstr ") and current_id is not None:
            entries[current_id] = json.loads(line[7:])
            current_id = None
    return entries


class LocalizationCatalogTests(unittest.TestCase):
    def test_english_and_bengali_catalogs_have_identical_complete_keys(self):
        english = parse_po(ROOT / "locale/en/LC_MESSAGES" / (DOMAIN + ".po"))
        bengali = parse_po(ROOT / "locale/bn/LC_MESSAGES" / (DOMAIN + ".po"))
        self.assertEqual(set(english), set(bengali))
        self.assertTrue(all(value for value in bengali.values()))

    def test_compiled_catalogs_load_with_utf8_and_expected_translations(self):
        for language, expected in (("en", "Today"), ("bn", "আজ")):
            path = ROOT / "locale" / language / "LC_MESSAGES" / (DOMAIN + ".mo")
            with path.open("rb") as catalog_file:
                catalog = gettext.GNUTranslations(catalog_file)
            self.assertEqual(catalog.gettext("Today"), expected)
            self.assertEqual(catalog.gettext("not-yet-translated"), "not-yet-translated")

    def test_every_fixed_translation_call_has_catalog_entries(self):
        keys = set()
        for relative in ("src/main.rs", "src/goals.rs"):
            source = (ROOT / relative).read_text(encoding="utf-8")
            keys.update(re.findall(r'i18n::tr\("([^"]+)"\)', source))
            keys.update(re.findall(r'i18n::format\("([^"]+)"', source))
        english = parse_po(ROOT / "locale/en/LC_MESSAGES" / (DOMAIN + ".po"))
        self.assertTrue(keys.issubset(set(english)), sorted(keys - set(english)))


if __name__ == "__main__":
    unittest.main()
