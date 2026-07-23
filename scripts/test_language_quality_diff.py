#!/usr/bin/env python3
"""语言质量快照差分器的快速合成数据测试。"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))

from language_quality_diff import (  # noqa: E402
    SNAPSHOT_SCHEMA_VERSION,
    compare_files,
    compare_snapshot_manifests,
    file_descriptor,
    write_bundle,
)
from language_quality_gate import (  # noqa: E402
    CONFIG_SCHEMA_VERSION,
    evaluate_gate,
)
from language_quality_snapshot import (  # noqa: E402
    MAX_INLINE_STREAM_BYTES,
    extract_chapter,
    stream_descriptor,
)


class LanguageQualityDiffTest(unittest.TestCase):
    def write_json(self, root: Path, name: str, value: object) -> Path:
        path = root / name
        path.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
        return path

    def test_bunsetsu_detects_segmentation_boundary_and_nested_field_changes(self) -> None:
        before = [
            {
                "bunsetsus": [
                    {
                        "surface": "本を",
                        "char_range": [0, 2],
                        "grammar_occurrences": [
                            {"occurrence_id": "grammar.wo@1-2", "confidence": 90}
                        ],
                    },
                    {"surface": "読む。", "char_range": [2, 5]},
                ],
                "boundaries": [
                    {"morpheme_index": 1, "decision": "split", "score": 80}
                ],
                "unresolved_boundaries": 0,
                "reconstruction_ok": True,
                "range_integrity_ok": True,
            }
        ]
        after = [
            {
                "bunsetsus": [
                    {
                        "surface": "本を",
                        "char_range": [0, 2],
                        "grammar_occurrences": [
                            {"occurrence_id": "grammar.wo@1-2", "confidence": 95}
                        ],
                    },
                    {"surface": "読む", "char_range": [2, 4]},
                    {"surface": "。", "char_range": [4, 5]},
                ],
                "boundaries": [
                    {"morpheme_index": 1, "decision": "join", "score": 80}
                ],
                "unresolved_boundaries": 0,
                "reconstruction_ok": True,
                "range_integrity_ok": True,
            }
        ]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle = compare_files(
                self.write_json(root, "before.json", before),
                self.write_json(root, "after.json", after),
                "bunsetsu",
            )
        types = Counter(change["type"] for change in bundle.changes)
        self.assertEqual(types["bunsetsu_segmentation_changed"], 1)
        self.assertEqual(types["boundary_changed"], 1)
        self.assertEqual(types["bunsetsu_fields_changed"], 1)
        self.assertTrue(bundle.summary["comparable"])
        self.assertEqual(bundle.summary["metrics"]["bunsetsus"]["delta"], 1)
        paths = {
            item["path"]
            for change in bundle.changes
            for item in change.get("field_changes", [])
        }
        self.assertIn(
            "/grammar_occurrences[occurrence_id=grammar.wo@1-2]/confidence", paths
        )

    def test_expression_writes_machine_and_human_outputs(self) -> None:
        before = [
            {
                "match_id": "old:1",
                "status": "pending",
                "rule_id": "idiom.test",
                "origin": "builtin",
                "surface": "手を引く",
                "char_range": [3, 7],
                "context": "事業から手を引く。",
            }
        ]
        after = [
            {
                "match_id": "new:9",
                "status": "accepted",
                "rule_id": "idiom.test",
                "origin": "builtin",
                "surface": "手を引く",
                "char_range": [3, 7],
                "context": "事業から手を引く。",
            },
            {
                "match_id": "new:10",
                "status": "accepted",
                "rule_id": "idiom.extra",
                "origin": "builtin",
                "surface": "耳を傾ける",
                "char_range": [12, 18],
                "context": "話に耳を傾ける。",
            },
        ]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle = compare_files(
                self.write_json(root, "before.json", before),
                self.write_json(root, "after.json", after),
                "expression",
            )
            output = root / "report"
            write_bundle(bundle, output)
            self.assertTrue((output / "manifest.json").is_file())
            self.assertTrue((output / "summary.json").is_file())
            self.assertEqual(len((output / "diff.jsonl").read_text(encoding="utf-8").splitlines()), 2)
            report = (output / "report.html").read_text(encoding="utf-8")
            self.assertIn("Kotoclip 语言质量差分", report)
            self.assertIn("diff.jsonl", report)
            self.assertNotIn("expression_fields_changed", report)
            self.assertIn("expression_fields_changed", (output / "diff.jsonl").read_text(encoding="utf-8"))
        self.assertEqual(bundle.summary["change_types"]["expression_added"], 1)
        self.assertEqual(bundle.summary["change_types"]["expression_fields_changed"], 1)

    def write_snapshot(
        self, root: Path, name: str, token: dict[str, object]
    ) -> Path:
        run = root / name
        artifact = run / "artifacts" / "tokens.json"
        artifact.parent.mkdir(parents=True)
        artifact.write_text(
            json.dumps([token], ensure_ascii=False), encoding="utf-8"
        )
        descriptor = file_descriptor(artifact)
        descriptor["path"] = "artifacts/tokens.json"
        manifest = {
            "schema_version": SNAPSHOT_SCHEMA_VERSION,
            "run_id": name,
            "label": name,
            "corpus": {
                "id": "synthetic",
                "selected_sha256": "same-source",
                "selected_bytes": 12,
                "selected_characters": 4,
                "analysis_text_sha256": "same-analysis",
                "analysis_characters": 4,
            },
            "resources": {},
            "artifacts": {
                "tokens": {
                    **descriptor,
                    "adapter": "annotated_tokens",
                    "capture": {
                        "dictionary": True,
                        "profile": False,
                        "expressions": False,
                    },
                }
            },
        }
        manifest_path = run / "manifest.json"
        manifest_path.write_text(
            json.dumps(manifest, ensure_ascii=False), encoding="utf-8"
        )
        return manifest_path

    def write_artifact_snapshot(
        self,
        root: Path,
        name: str,
        artifact_values: dict[str, tuple[object, str, dict[str, object]]],
    ) -> Path:
        run = root / name
        artifacts: dict[str, object] = {}
        for artifact_name, (value, adapter, capture) in artifact_values.items():
            path = run / "artifacts" / f"{artifact_name}.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
            descriptor = file_descriptor(path)
            descriptor["path"] = f"artifacts/{artifact_name}.json"
            artifacts[artifact_name] = {
                **descriptor,
                "adapter": adapter,
                "capture": capture,
            }
        manifest = {
            "schema_version": SNAPSHOT_SCHEMA_VERSION,
            "run_id": name,
            "label": name,
            "corpus": {
                "id": "synthetic",
                "selected_sha256": "same-source",
                "selected_bytes": 12,
                "selected_characters": 4,
                "analysis_text_sha256": "same-analysis",
                "analysis_characters": 4,
            },
            "resources": {},
            "artifacts": artifacts,
        }
        manifest_path = run / "manifest.json"
        manifest_path.write_text(
            json.dumps(manifest, ensure_ascii=False), encoding="utf-8"
        )
        return manifest_path

    def test_pipeline_diff_separates_root_and_propagated_changes(self) -> None:
        def token(base_form: str, head_form: str, confidence: int) -> dict[str, object]:
            return {
                "bunsetsu": {
                    "surface": "本を読む",
                    "char_range": [0, 4],
                    "head_word": {
                        "surface": "読む",
                        "base_form": head_form,
                        "reading": "ヨム",
                        "pos": {"major": "動詞", "sub1": "自立", "sub2": "*", "sub3": "*"},
                    },
                    "function": {"function": "predicate", "confidence": 90},
                    "morphemes": [
                        {
                            "surface": "本",
                            "base_form": "本",
                            "reading": "ホン",
                            "pos": {"major": "名詞", "sub1": "一般", "sub2": "*", "sub3": "*"},
                            "conjugation_type": "*",
                            "conjugation_form": "*",
                            "char_range": [0, 1],
                        },
                        {
                            "surface": "を",
                            "base_form": "を",
                            "reading": "ヲ",
                            "pos": {"major": "助詞", "sub1": "格助詞", "sub2": "*", "sub3": "*"},
                            "conjugation_type": "*",
                            "conjugation_form": "*",
                            "char_range": [1, 2],
                        },
                        {
                            "surface": "読む",
                            "base_form": base_form,
                            "reading": "ヨム",
                            "pos": {"major": "動詞", "sub1": "自立", "sub2": "*", "sub3": "*"},
                            "conjugation_type": "五段・マ行",
                            "conjugation_form": "基本形",
                            "char_range": [2, 4],
                        },
                    ],
                    "word_formations": [],
                    "lexical_units": [],
                    "morphology": {"chains": [], "unclassified": []},
                    "grammar_occurrences": [
                        {
                            "occurrence_id": "grammar.object@1-2",
                            "concept_id": "grammar.case.object",
                            "rule_id": "particle.wo",
                            "matched_ranges": [[1, 2]],
                            "anchor_range": [1, 2],
                            "status": "accepted",
                            "confidence": confidence,
                        }
                    ],
                    "grammar_tags": [],
                    "functional_residuals": [],
                },
                "novelty_score": 1.0,
                "is_selected": False,
                "is_known": False,
                "inference_reason": None,
                "expressions": [],
                "display_class": "content",
            }

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            before = self.write_snapshot(root, "before", token("読", "読", 80))
            after = self.write_snapshot(root, "after", token("読む", "読む", 95))
            bundle = compare_snapshot_manifests(before, after)
            output = root / "report"
            write_bundle(bundle, output)

            self.assertTrue((output / "stage-summary.json").is_file())
            self.assertTrue((output / "root-causes.json").is_file())
            self.assertIn("管线层级", (output / "report.html").read_text(encoding="utf-8"))

        morpheme = next(change for change in bundle.changes if change["stage"] == "morpheme")
        bunsetsu = next(change for change in bundle.changes if change["stage"] == "bunsetsu")
        grammar = next(
            change for change in bundle.changes if change["stage"] == "grammar_occurrence"
        )
        self.assertEqual(morpheme["causal_status"], "root")
        self.assertEqual(bunsetsu["causal_status"], "propagated_candidate")
        self.assertIn(morpheme["change_id"], bunsetsu["cause_change_ids"])
        self.assertEqual(grammar["causal_status"], "propagated_candidate")
        self.assertIn(bunsetsu["change_id"], grammar["cause_change_ids"])
        self.assertEqual(bundle.summary["root_changes"], 1)

    def test_extract_chapter_matches_markdown_second_level_heading(self) -> None:
        source = "# 书名\r\n## 第一话\r\n正文一\r\n## 第二话\r\n正文二\r\n"
        self.assertEqual(extract_chapter(source, "## 第一话"), "正文一\r")

    def test_candidate_ui_statistics_and_gate_are_layered(self) -> None:
        grammar_before = [
            {
                "occurrence_id": "grammar.test@0-2",
                "concept_id": "grammar.test",
                "rule_id": "test.rule",
                "status": "pending",
                "matched_ranges": [[0, 2]],
                "confidence": 0.6,
            }
        ]
        grammar_after = [{**grammar_before[0], "status": "rejected", "confidence": 0.2}]
        ui_before = {
            "schema_version": "kotoclip.quality.ui-projection.v1",
            "items": [
                {
                    "projection_id": "grammar.test@0-2",
                    "kind": "grammar_badge",
                    "char_range": [0, 2],
                    "visible": True,
                }
            ],
        }
        ui_after = {
            **ui_before,
            "items": [{**ui_before["items"][0], "visible": False}],
        }
        capture = {
            "include_pending": True,
            "include_rejected": True,
            "dictionary": False,
        }
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            before = self.write_artifact_snapshot(
                root,
                "before",
                {
                    "grammar_occurrences": (
                        grammar_before,
                        "grammar_occurrences",
                        capture,
                    ),
                    "ui_projection": (
                        ui_before,
                        "ui_projection",
                        {"schema": "kotoclip.quality.ui-projection.v1"},
                    ),
                },
            )
            after = self.write_artifact_snapshot(
                root,
                "after",
                {
                    "grammar_occurrences": (
                        grammar_after,
                        "grammar_occurrences",
                        capture,
                    ),
                    "ui_projection": (
                        ui_after,
                        "ui_projection",
                        {"schema": "kotoclip.quality.ui-projection.v1"},
                    ),
                },
            )
            bundle = compare_snapshot_manifests(before, after)

        grammar_change = next(
            change
            for change in bundle.changes
            if change["stage"] == "grammar_candidate"
        )
        self.assertEqual(grammar_change["status_before"], "pending")
        self.assertEqual(grammar_change["status_after"], "rejected")
        self.assertTrue(
            any(change["stage"] == "ui_projection" for change in bundle.changes)
        )
        grammar_row = next(
            row for row in bundle.summary["stages"] if row["stage"] == "grammar_candidate"
        )
        self.assertEqual(grammar_row["churn"]["rate"], 1.0)
        self.assertEqual(
            bundle.summary["status_transitions"],
            [{"before": "pending", "after": "rejected", "count": 1}],
        )

        config = {
            "schema_version": CONFIG_SCHEMA_VERSION,
            "require_comparable": True,
            "allowed_quality_conclusions": ["partial"],
            "allowed_missing_stages": bundle.summary["missing_stages"],
            "max_root_changes": 0,
        }
        gate = evaluate_gate(bundle.summary, config)
        self.assertEqual(gate["status"], "review_required")
        self.assertEqual(gate["violations"][0]["rule"], "max_root_changes")

    def test_large_command_stream_is_hashed_instead_of_inlined(self) -> None:
        small = stream_descriptor("完成")
        large = stream_descriptor("x" * (MAX_INLINE_STREAM_BYTES + 1))
        self.assertEqual(small["inline"], "完成")
        self.assertNotIn("inline", large)
        self.assertEqual(large["bytes"], MAX_INLINE_STREAM_BYTES + 1)

    def test_html_uses_external_full_data_files(self) -> None:
        before = [{"match_id": "old", "rule_id": "test", "origin": "builtin", "surface": "手を引く", "char_range": [0, 3]}]
        after = [{"match_id": "new", "rule_id": "test", "origin": "builtin", "surface": "手を引く", "char_range": [0, 3]}]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle = compare_files(
                self.write_json(root, "before.json", before),
                self.write_json(root, "after.json", after),
                "expression",
            )
            output = root / "report"
            write_bundle(bundle, output)
            report = (output / "report.html").read_text(encoding="utf-8")
            self.assertIn('"detail_strategy":"full_external_jsonl"', report)
            self.assertIn('"diff":"diff.jsonl"', report)
            self.assertNotIn("手を引く", report)
            self.assertEqual(len((output / "diff.jsonl").read_text(encoding="utf-8").splitlines()), 1)
            self.assertTrue((output / "root-causes.json").is_file())

    def test_html_detail_limit_is_removed(self) -> None:
        with self.assertRaises(SystemExit):
            from language_quality_diff import parse_args

            parse_args(["--output-dir", "report", "--html-detail-limit", "10"])


if __name__ == "__main__":
    unittest.main()
