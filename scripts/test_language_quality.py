#!/usr/bin/env python3
"""统一语言质量入口的轻量协议测试。"""

from __future__ import annotations

import json
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import language_quality
import language_quality_snapshot


class LanguageQualityEntryPointTest(unittest.TestCase):
    def test_snapshot_stage_counts_stream_reports(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            reports = {
                "word_formations": {"items": [{"id": 1}], "rejected": [{"id": 2}]},
                "lexical_candidates": {"items": [{"id": 1}, {"id": 2}]},
                "bunsetsu": [{"boundaries": [{}, {}]}, {"boundaries": [{}]}],
                "expressions": [{"status": "accepted"}, {"status": "rejected"}],
                "catalogs": [{"layer": "grammar"}, {"layer": "dictionary"}],
            }
            observed: dict[str, dict[str, int]] = {}
            for name, report in reports.items():
                path = root / f"{name}.json"
                path.write_text(json.dumps(report), encoding="utf-8")
                observed[name] = language_quality_snapshot.stage_counts_for_artifact(
                    name, path
                )

        self.assertEqual(observed["word_formations"], {"word_formation_candidate": 2})
        self.assertEqual(observed["lexical_candidates"], {"lexical_candidate": 2})
        self.assertEqual(observed["bunsetsu"], {"bunsetsu_boundary": 3})
        self.assertEqual(
            observed["expressions"], {"expression": 1, "expression_candidate": 1}
        )
        self.assertEqual(observed["catalogs"], {"resource": 2})

    def test_default_library_corpus_uses_all_books_deterministically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            library = root / "library"
            library.mkdir()
            connection = sqlite3.connect(library / "library.sqlite")
            try:
                connection.execute(
                    "CREATE TABLE books (id TEXT PRIMARY KEY, title TEXT, author TEXT, language TEXT)"
                )
                connection.executemany(
                    "INSERT INTO books VALUES (?, ?, ?, ?)",
                    [("book-b", "乙", "作者乙", "ja"), ("book-a", "甲", "作者甲", "ja")],
                )
                connection.commit()
            finally:
                connection.close()
            for book_id, content in (("book-a", "甲の本文。"), ("book-b", "乙の本文。")):
                book = library / "books" / book_id
                book.mkdir(parents=True)
                (book / "content.md").write_text(content, encoding="utf-8")
            first, first_id = language_quality.default_library_corpus(library, root / "cache")
            second, second_id = language_quality.default_library_corpus(library, root / "cache")
            text = first.read_text(encoding="utf-8")

        self.assertEqual(first, second)
        self.assertEqual(first_id, second_id)
        self.assertLess(text.index("甲の本文"), text.index("乙の本文"))

    def test_write_json_is_utf8_and_replaces_atomically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "nested" / "lifecycle.json"
            language_quality.write_json(path, {"状态": "完成", "value": 1})
            self.assertEqual(
                json.loads(path.read_text(encoding="utf-8")),
                {"状态": "完成", "value": 1},
            )
            self.assertEqual(list(path.parent.glob("*.tmp")), [])

    def test_gate_outcome_keeps_reviewable_comparison_in_lifecycle(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            gate = output / "diff" / "gate.json"
            gate.parent.mkdir(parents=True)
            gate.write_text(
                json.dumps({"status": "review_required"}),
                encoding="utf-8",
            )
            self.assertEqual(
                language_quality.gate_outcome(output, 1),
                (True, "review_required"),
            )
            self.assertEqual(language_quality.gate_outcome(output, 2), (False, None))
            gate.write_text(json.dumps({"status": "blocked"}), encoding="utf-8")
            self.assertEqual(
                language_quality.gate_outcome(output, 2),
                (True, "blocked"),
            )

    def test_gate_outcome_rejects_unexplained_nonzero_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            self.assertEqual(
                language_quality.gate_outcome(Path(temporary), 1),
                (False, None),
            )

    def test_publish_directory_replaces_existing_round(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "round"
            staging = root / ".round.run"
            target.mkdir()
            staging.mkdir()
            (target / "value.txt").write_text("old", encoding="utf-8")
            (staging / "value.txt").write_text("new", encoding="utf-8")
            backup = language_quality.publish_directory(staging, target)
            self.assertEqual((target / "value.txt").read_text(encoding="utf-8"), "new")
            self.assertIsNotNone(backup)
            self.assertEqual((backup / "value.txt").read_text(encoding="utf-8"), "old")


if __name__ == "__main__":
    unittest.main()
