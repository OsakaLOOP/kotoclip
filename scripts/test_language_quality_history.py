#!/usr/bin/env python3
"""语言质量历史索引的合成数据测试。"""

from __future__ import annotations

import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))

from language_quality_history import build_history, history_page  # noqa: E402


class LanguageQualityHistoryTest(unittest.TestCase):
    def write_json(self, path: Path, value: object) -> Path:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
        return path

    def sha256(self, path: Path) -> str:
        return hashlib.sha256(path.read_bytes()).hexdigest()

    def write_snapshot(self, root: Path, name: str, run_id: str) -> Path:
        return self.write_json(
            root / name / "manifest.json",
            {
                "schema_version": "kotoclip.quality.snapshot.v1",
                "run_id": run_id,
                "label": name,
                "created_at": "2026-07-23T02:00:00+00:00",
                "implementation": {
                    "git_commit": f"commit-{name}",
                    "git_dirty": False,
                    "git_status_sha256": f"status-{name}",
                    "cli_sha256": f"cli-{name}",
                    "platform": "Windows-test",
                    "python": "3.test",
                },
                "corpus": {
                    "id": "novel-v1",
                    "source_path": "D:/corpus/novel.md",
                    "source_sha256": "source",
                    "selection": {"chapter": "第一章"},
                    "selected_sha256": "selected",
                    "selected_bytes": 120,
                    "selected_characters": 80,
                    "analysis_text_sha256": "analysis",
                    "analysis_characters": 72,
                },
                "resources": {
                    "system_dictionary": {
                        "path": "ipadic/system.dic",
                        "bytes": 1234,
                        "sha256": "system-dict",
                        "logical_name": "ipadic/system.dic",
                    },
                    "catalogs": [
                        {"path": "grammar.json", "sha256": "grammar-catalog"}
                    ],
                },
            },
        )

    def write_comparison(
        self,
        root: Path,
        name: str,
        before: Path,
        after: Path,
    ) -> None:
        output = root / name
        output.mkdir(parents=True)
        before_value = json.loads(before.read_text(encoding="utf-8"))
        after_value = json.loads(after.read_text(encoding="utf-8"))
        self.write_json(
            output / "manifest.json",
            {
                "schema_version": "kotoclip.quality.diff.v3",
                "adapter": "pipeline",
                "before": {
                    "run_id": before_value["run_id"],
                    "label": before_value["label"],
                    "sha256": self.sha256(before),
                },
                "after": {
                    "run_id": after_value["run_id"],
                    "label": after_value["label"],
                    "sha256": self.sha256(after),
                },
            },
        )
        self.write_json(
            output / "summary.json",
            {
                "status": "changed",
                "quality_conclusion": "partial",
                "changes": 8,
                "root_changes": 3,
                "propagated_candidates": 5,
                "churn": {"rate": 0.125},
                "missing_stages": ["ui_projection"],
                "stages": [
                    {"stage": "morpheme", "churn": {"rate": 0.25}}
                ],
            },
        )
        (output / "diff.jsonl").write_text(
            '{"context":"不得进入历史 HTML"}\n', encoding="utf-8"
        )
        (output / "report.html").write_text(
            "<!doctype html><title>单轮报告</title>", encoding="utf-8"
        )
        self.write_json(output / "gate.json", {"status": "review_required"})

    def test_build_history_links_snapshots_and_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            before = self.write_snapshot(root, "snapshots/before", "before-id")
            after = self.write_snapshot(root, "snapshots/after", "after-id")
            self.write_comparison(root, "comparisons/round-1", before, after)
            history = build_history(root)

        self.assertEqual(history["schema_version"], "kotoclip.quality.comparison-history.v1")
        self.assertEqual(len(history["comparisons"]), 1)
        record = history["comparisons"][0]
        self.assertEqual(record["comparison_id"], "comparisons/round-1")
        self.assertEqual(record["report"]["url"], "comparisons/round-1/report.html")
        self.assertEqual(record["summary"]["stage_churn"], {"morpheme": 0.25})
        self.assertEqual(record["gate_status"], "review_required")
        self.assertEqual(record["created_at"], "2026-07-23T02:00:00+00:00")
        self.assertEqual(record["before"]["implementation"]["git_commit"], "commit-snapshots/before")
        self.assertFalse(record["before"]["implementation"]["git_dirty"])
        self.assertEqual(record["before"]["implementation"]["platform"], "Windows-test")
        self.assertEqual(record["after"]["corpus"]["selection"], {"chapter": "第一章"})
        self.assertEqual(record["after"]["corpus"]["analysis_characters"], 72)
        self.assertEqual(
            record["before"]["resources"]["system_dictionary"]["sha256"],
            "system-dict",
        )
        self.assertEqual(
            record["before"]["resources"]["system_dictionary"]["bytes"], 1234
        )
        self.assertTrue(record["manifest"]["sha256"])
        self.assertTrue(record["summary_artifact"]["sha256"])
        self.assertTrue(record["gate"]["sha256"])

    def test_missing_snapshot_is_explicit_and_incomplete_report_is_ignored(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            before = self.write_snapshot(root, "before", "before-id")
            after = self.write_snapshot(root, "after", "after-id")
            self.write_comparison(root, "round", before, after)
            before.unlink()
            incomplete = root / "incomplete"
            incomplete.mkdir()
            (incomplete / "report.html").write_text("x", encoding="utf-8")
            record = build_history(root)["comparisons"][0]

        self.assertIsNone(record["before"]["snapshot"])
        self.assertIsNone(record["before"]["implementation"]["git_commit"])
        self.assertIsNone(record["before"]["corpus"])
        self.assertIsNone(record["before"]["resources"])

    def test_history_page_loads_external_registry_only(self) -> None:
        page = history_page("history.json")
        self.assertIn('"source":"history.json"', page)
        self.assertIn("fetch(config.source", page)
        self.assertIn("item.report.url", page)
        self.assertIn("item.manifest.url", page)
        self.assertNotIn("不得进入历史 HTML", page)


if __name__ == "__main__":
    unittest.main()
