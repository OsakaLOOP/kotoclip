#!/usr/bin/env python3
"""统一语言质量入口的轻量协议测试。"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import language_quality


class LanguageQualityEntryPointTest(unittest.TestCase):
    def test_split_compare_args_keeps_commit_runner_arguments(self) -> None:
        commit, history, lifecycle, no_history = language_quality.split_compare_args(
            [
                "--history-root",
                "experiments/quality",
                "--before",
                "HEAD^",
                "--after",
                "HEAD",
                "--output-dir",
                "experiments/quality/runs/a",
                "--no-history",
            ]
        )
        self.assertEqual(history, Path("experiments/quality"))
        self.assertIsNone(lifecycle)
        self.assertTrue(no_history)
        self.assertEqual(
            commit,
            [
                "--before",
                "HEAD^",
                "--after",
                "HEAD",
                "--output-dir",
                "experiments/quality/runs/a",
            ],
        )

    def test_write_json_is_utf8_and_replaces_atomically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "nested" / "lifecycle.json"
            language_quality.write_json(path, {"状态": "完成", "value": 1})
            self.assertEqual(
                json.loads(path.read_text(encoding="utf-8")),
                {"状态": "完成", "value": 1},
            )
            self.assertEqual(list(path.parent.glob("*.tmp")), [])


if __name__ == "__main__":
    unittest.main()
