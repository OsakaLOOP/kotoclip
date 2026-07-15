import json
import struct
import tempfile
import unittest
from pathlib import Path

from dictionary_bundle import MAGIC, build_bundle_from_entries
from dictionary_schema import keys_for_entry, normalize_form, normalize_reading, parse_headword


class DictionarySchemaTests(unittest.TestCase):
    def test_parse_daijirin_headword(self):
        forms, readings = parse_headword("なのか【七日】")
        self.assertEqual(forms, ["七日"])
        self.assertEqual(readings, ["なのか"])

    def test_normalize_kana_and_variant_kanji(self):
        self.assertEqual(normalize_reading("けいさつ-しょ"), "ケイサツショ")
        self.assertEqual(normalize_form("繋ぐ"), normalize_form("繫ぐ"))

    def test_structured_reading_takes_precedence(self):
        forms, readings = keys_for_entry("つなぐ【繫ぐ】", "ツナグ")
        self.assertEqual(forms[0], ("繫ぐ", "繫ぐ"))
        self.assertEqual(readings, [("ツナグ", "ツナグ")])

    def test_bundle_separates_aliases_and_compresses_definitions(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "test.kdict"
            report = build_bundle_from_entries(
                [
                    ("けいさつしょ", None, "@@@LINK=けいさつしょ【警察署】"),
                    ("けいさつしょ【警察署】", None, "<p>释义</p>"),
                ],
                output,
                "测试词典",
            )
            self.assertEqual(report["canonical_count"], 1)
            self.assertEqual(report["alias_count"], 1)
            with output.open("rb") as source:
                self.assertEqual(source.read(len(MAGIC)), MAGIC)
                header_size = struct.unpack("<I", source.read(4))[0]
                header = json.loads(source.read(header_size).decode("utf-8"))
            self.assertEqual(header["schema_version"], 4)
            self.assertEqual(header["source_name"], "测试词典")


if __name__ == "__main__":
    unittest.main()
