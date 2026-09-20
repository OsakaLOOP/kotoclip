"""来源映射及语义保留的定向检查，直接使用已采集的 KNP 输出。"""
import json
from pathlib import Path
import unittest

from nlp_adapters import KwjaAdapter, normalization_map

ROOT = Path(__file__).resolve().parents[1]


class AdapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from kwja.cli.cli import _normalize_text
        cls.normalize = staticmethod(_normalize_text)
        cls.cases = {c["id"]: c for c in json.loads((ROOT / "data/validation/behavior/kwja.json").read_text(encoding="utf-8"))["segments"]}

    def convert(self, name):
        case = self.cases[name]
        normalized, origins = normalization_map(case["text"], self.normalize)
        adapter = KwjaAdapter.__new__(KwjaAdapter)
        adapter.manifest = {"id": "kwja"}
        return adapter.convert(case["text"], case["raw"], normalized, origins)

    def test_normalization_preserves_expansion_contraction_and_deleted_controls(self):
        normalized, origins = normalization_map("ｶﾞか\u3099㍿\t甲甲", self.normalize)
        self.assertEqual(normalized, "ガが株式会社甲甲")
        self.assertEqual(origins, [[0, 2], [2, 4], [4, 5], [4, 5], [4, 5], [4, 5], [6, 7], [7, 8]])

    def test_expanded_tokens_keep_shared_original_character(self):
        result = self.convert("boundaries")
        stock, company = [n for n in result["nodes"] if n["kind"] == "token" and n["source_surface"] in ("株式", "会社")]
        self.assertEqual(stock["text_ranges"], [[24, 25]])
        self.assertEqual(company["text_ranges"], [[24, 25]])
        self.assertEqual(stock["surface"], "㍿")
        self.assertEqual(result["deleted_ranges"], [[5, 6], [10, 11], [30, 31]])

    def test_cross_sentence_coreference_keeps_node_identity(self):
        result = self.convert("relations")
        nodes = {n["id"]: n for n in result["nodes"]}
        coreferences = [r for r in result["relations"] if r["kind"] == "coreference"]
        self.assertTrue(any(nodes[r["source"]["id"]]["surface"] == "彼は" and nodes[r["target"]["id"]]["surface"] == "太郎は" for r in coreferences))
        self.assertTrue(any(r["kind"] == "predicate_argument" and r["label"] == "ヲ" for r in result["relations"]))

    def test_external_arguments_and_all_references_resolve(self):
        result = self.convert("inflection")
        self.assertTrue(any(r["target"] == {"kind": "exophora", "label": "著者"} for r in result["relations"]))
        for name in self.cases:
            result = self.convert(name)
            ids = {n["id"] for n in result["nodes"]}
            self.assertEqual(len(ids), len(result["nodes"]))
            for node in result["nodes"]:
                self.assertTrue(set(node["members"]).issubset(ids))
                if node["head"]:
                    self.assertIn(node["head"], ids)
            for relation in result["relations"]:
                for endpoint in (relation["source"], relation["target"]):
                    if endpoint["kind"] == "node":
                        self.assertIn(endpoint["id"], ids)


if __name__ == "__main__":
    unittest.main()
