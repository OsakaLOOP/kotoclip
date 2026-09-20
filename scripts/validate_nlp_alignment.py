"""使用短文本验证正文映射、多对多对齐及统一结构引用。"""
import json
from pathlib import Path
import subprocess
from collections import Counter

from nlp_adapters import syntax_artifact
from probe_nlp_behavior import ROOT, samples


def main():
    cases = [case for case in samples()[1] if case["id"] in {"news", "boundaries"}]
    cases.append({"id": "repeated-ruby", "text": "警察署《けいさつしょ》へ向かった。\n警察署《けいさつしょ》へ向かった。  \n"})
    process = subprocess.Popen([str(ROOT / "target/debug/kotoclip-nlp.exe"), "stdio"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8")
    report = {"schema": "kotoclip.alignment-check.v1", "cases": []}
    def request(payload):
        process.stdin.write(json.dumps(payload, ensure_ascii=False) + "\n")
        process.stdin.flush()
        return json.loads(process.stdout.readline())
    try:
        for case in cases:
            base = request({"command": "analyze", "text": case["text"], "register": "cwj"})
            assert base["error"] is None, base["error"]
            base = base["result"]
            enriched = request({"command": "enrich", "analysis_id": base["id"]})
            assert enriched["error"] is None, enriched["error"]
            result = enriched["result"]
            assert all(p["status"] == "ready" for p in result["providers"]), result["providers"]
            doc = result["document"]
            assert doc["id"] == base["id"]
            assert doc["preparation"] == base["preparation"]
            assert "".join(case["text"][i] for i in doc["preparation"]["origins"]) == doc["text"]
            graph = doc["structure_graph"]
            entities = {e["id"]: e for e in graph["entities"]}
            assert len(entities) == len(graph["entities"])
            for relation in graph["relations"]:
                for endpoint in (relation["source"], relation["target"]):
                    if endpoint["kind"] == "entity":
                        assert endpoint["id"] in entities
            alignment_groups = [group for source in doc["provider_token_alignments"] for group in source["groups"]]
            group_ids = {g["id"] for g in alignment_groups}
            assert all(set(e["alignment_group_ids"]).issubset(group_ids) for e in entities.values())
            if case["id"] == "news":
                first_kwja = next(e for e in entities.values() if e["provider"] == "kwja" and e["source_id"] == "b0")
                assert not first_kwja["complete"] and "partial_morpheme" in first_kwja["diagnostics"]
            if case["id"] == "boundaries":
                head = next(e for e in entities.values() if e["provider"] == "ginza" and e["source_id"] == "b1")
                assert len(head["head_morpheme_ids"]) == 2
            if case["id"] == "repeated-ruby":
                sentences = [e for e in entities.values() if e["kind"] == "sentence" and e["provider"] == "kwja"]
                assert len(sentences) >= 2 and sentences[0]["id"] != sentences[1]["id"]
                assert len(doc["author_ruby"]) == 2
            artifact = syntax_artifact(doc["external_sources"][0], case["id"])
            valid = request({"command": "analyze_with_artifacts", "text": case["text"], "register": "cwj", "artifacts": [artifact]})
            assert valid["error"] is None, valid["error"]
            artifact["text_sha256"] = "wrong-text"
            wrong = request({"command": "analyze_with_artifacts", "text": case["text"], "register": "cwj", "artifacts": [artifact]})
            assert wrong["error"] == "artifact_text_digest_mismatch", wrong["error"]
            report["cases"].append({"id": case["id"], "input_characters": len(case["text"]), "prepared_characters": doc["characters"],
                "groups": dict(Counter(g["cardinality"] for g in alignment_groups)),
                "alignment_states": dict(Counter(g["status"] for g in alignment_groups)),
                "entities": len(entities), "relations": len(graph["relations"]),
                "selected_candidates": sum(c["selected"] for c in graph["candidates"]),
                "pending_entities": sum(not e["complete"] for e in entities.values()),
                "source_mapping_verified": True, "references_verified": True, "wrong_digest_rejected": True})
            print(case["id"], "通过", flush=True)
    finally:
        process.stdin.close()
        process.wait(timeout=20)
    target = ROOT / "data/validation/behavior/alignment.json"
    target.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(target)


if __name__ == "__main__":
    main()
