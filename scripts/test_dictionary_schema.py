import sqlite3
import unittest

from dictionary_schema import (
    keys_for_entry,
    normalize_form,
    normalize_reading,
    parse_headword,
    rebuild_search_tables,
)


class DictionarySchemaTests(unittest.TestCase):
    def test_parse_daijirin_headword(self):
        forms, readings = parse_headword("なのか【七日】")
        self.assertEqual(forms, [("七日", "kanji")])
        self.assertEqual(readings, ["なのか"])

    def test_normalize_kana_and_variant_kanji(self):
        self.assertEqual(normalize_reading("けいさつ-しょ"), "ケイサツショ")
        self.assertEqual(normalize_form("繋ぐ"), normalize_form("繫ぐ"))

    def test_rebuild_separates_forms_and_readings(self):
        connection = sqlite3.connect(":memory:")
        connection.execute(
            "CREATE TABLE entries (id INTEGER PRIMARY KEY, headword TEXT NOT NULL, definition TEXT NOT NULL, dict_name TEXT NOT NULL)"
        )
        connection.execute(
            "INSERT INTO entries VALUES (1, 'けいさつしょ【警察署】', '<p>释义</p>', '测试')"
        )
        scanned, forms, readings = rebuild_search_tables(connection)
        self.assertEqual((scanned, forms, readings), (1, 1, 1))
        self.assertEqual(
            connection.execute("SELECT form, normalized_form, form_type FROM entry_forms").fetchone(),
            ("警察署", "警察署", "kanji"),
        )
        self.assertEqual(
            connection.execute("SELECT reading, normalized_reading FROM entry_readings").fetchone(),
            ("けいさつしょ", "ケイサツショ"),
        )

    def test_structured_reading_takes_precedence(self):
        forms, readings = keys_for_entry("つなぐ【繫ぐ】", "ツナグ")
        self.assertEqual(forms[0][1], "繫ぐ")
        self.assertEqual(readings, [("ツナグ", "ツナグ")])


if __name__ == "__main__":
    unittest.main()
