"""将 GiNZA 采集结果转换为 Rust SyntaxArtifact 兼容 JSON。"""
from __future__ import annotations
import json
import sys
import argparse
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "experiments" / "ginza-syntax-artifacts.json"

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, default=ROOT / "experiments" / "ginza-provider-validation.json")
    parser.add_argument("--output", type=Path, default=OUT)
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    source = json.loads(args.input.read_text(encoding="utf-8"))
    artifacts = []
    for segment in source["segments"]:
        spans = []
        token_ranges = {item["id"]: item["char_range"] for item in segment.get("tokens", [])}
        for layer in ("tokens", "compounds", "bunsetsu"):
            for item in segment[layer]:
                head_range = token_ranges.get(f"t{item.get('head')}") if layer == "tokens" else None
                spans.append({"id": item["id"], "kind": layer[:-1] if layer != "bunsetsu" else layer, "char_range": item["char_range"], "head_char_range": head_range, "source_id": segment["id"], "surface": item.get("surface"), "labels": []})
        for i, char_range in enumerate(segment["sentences"]):
            spans.append({"id": f"s{i}", "kind": "sentence", "char_range": char_range, "head_char_range": None, "source_id": segment["id"], "surface": None, "labels": []})
        artifacts.append({"segment_id": segment["id"], "schema": "kotoclip.syntax-artifact.v1", "provider": {"id": "ginza", "version": "5.2.1", "capabilities": ["token", "compound", "bunsetsu", "sentence", "dependency"], "license": "MIT"}, "text_characters": max((item["char_range"][1] for item in spans), default=0), "spans": spans})
    output = {"schema": "kotoclip.ginza-syntax-artifacts.v1", "artifacts": artifacts}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "artifacts": len(artifacts), "spans": sum(len(x["spans"]) for x in artifacts)}, ensure_ascii=False, indent=2))

if __name__ == "__main__": main()
