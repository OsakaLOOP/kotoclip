"""将 KWJA/KNP 采集结果转换为 SyntaxArtifact 兼容 JSON。"""
from __future__ import annotations

import argparse
import json
from pathlib import Path


def token_labels(item: dict) -> list[str]:
    return [field for field in item.get("fields", []) if field.startswith("<") and field.endswith(">")]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, default=Path("experiments/kwja-provider-validation.json"))
    parser.add_argument("--output", type=Path, default=Path("experiments/kwja-syntax-artifacts.json"))
    args = parser.parse_args()
    source = json.loads(args.input.read_text(encoding="utf-8"))
    artifacts = []
    for segment in source["segments"]:
        tokens = segment.get("tokens", [])
        spans = []
        for item in tokens:
            spans.append({
                "id": item["id"], "kind": "token", "char_range": item["char_range"],
                "head_char_range": None, "source_id": segment["id"],
                "surface": item.get("surface"), "labels": token_labels(item),
            })
        for item in segment.get("bunsetsu", []):
            start, end = item["char_range"]
            members = [token for token in tokens if token["char_range"][0] >= start and token["char_range"][1] <= end]
            head = next((token for token in members if any("基本句-主辞" in label for label in token_labels(token))), None)
            labels = sorted({label for token in members for label in token_labels(token)})
            spans.append({
                "id": item["id"], "kind": "bunsetsu", "char_range": item["char_range"],
                "head_char_range": head["char_range"] if head else None,
                "source_id": segment["id"], "surface": item.get("surface"), "labels": labels,
            })
        for index, char_range in enumerate(segment.get("sentences", [])):
            spans.append({
                "id": f"s{index}", "kind": "sentence", "char_range": char_range,
                "head_char_range": None, "source_id": segment["id"], "surface": None, "labels": [],
            })
        artifacts.append({
            "segment_id": segment["id"], "schema": "kotoclip.syntax-artifact.v1",
            "provider": {"id": "kwja", "version": source.get("provider", {}).get("version", "2.1.3"), "capabilities": ["token", "bunsetsu", "sentence", "bunsetsu_identity"], "license": None},
            "text_characters": max((item["char_range"][1] for item in spans), default=0), "spans": spans,
        })
    output = {"schema": "kotoclip.kwja-syntax-artifacts.v1", "artifacts": artifacts}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "artifacts": len(artifacts), "spans": sum(len(item["spans"]) for item in artifacts)}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
