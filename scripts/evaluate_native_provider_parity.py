"""比较 Rust provider artifact 与固定 Python provider 基准。"""
from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def artifacts(payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if "artifacts" in payload:
        return {str(item["segment_id"]): item for item in payload["artifacts"]}
    return {
        str(item["id"]): {
            "segment_id": item["id"],
            "spans": [
                {"kind": "token", **value} for value in item.get("tokens", [])
            ] + [
                {"kind": "compound", **value} for value in item.get("compounds", [])
            ] + [
                {"kind": "bunsetsu", **value} for value in item.get("bunsetsu", [])
            ] + [
                {"kind": "sentence", "char_range": value}
                for value in item.get("sentences", [])
            ],
        }
        for item in payload["segments"]
    }


def provider(payload: dict[str, Any]) -> dict[str, Any] | None:
    """读取 bundle 或单段采集结果的 provider 元数据。"""
    if isinstance(payload.get("provider"), dict):
        return payload["provider"]
    values = payload.get("artifacts")
    if isinstance(values, list) and values:
        value = values[0].get("provider")
        return value if isinstance(value, dict) else None
    return None


def span_set(artifact: dict[str, Any], kind: str) -> set[tuple[int, int]]:
    return {
        (int(span["char_range"][0]), int(span["char_range"][1]))
        for span in artifact.get("spans", [])
        if span.get("kind") == kind
    }


def score(reference: set[tuple[int, int]], candidate: set[tuple[int, int]]) -> dict[str, Any]:
    matched = len(reference & candidate)
    precision = matched / len(candidate) if candidate else 0.0
    recall = matched / len(reference) if reference else 0.0
    return {
        "reference": len(reference),
        "candidate": len(candidate),
        "matched": matched,
        "precision": precision,
        "recall": recall,
        "f1": 2 * precision * recall / (precision + recall) if precision + recall else 0.0,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    reference_payload = read_json(args.reference)
    candidate_payload = read_json(args.candidate)
    reference = artifacts(reference_payload)
    candidate = artifacts(candidate_payload)
    results: dict[str, Any] = {
        "schema": "kotoclip.native-provider-parity.v1",
        "reference": {"path": str(args.reference), "provider": provider(reference_payload)},
        "candidate": {"path": str(args.candidate), "provider": provider(candidate_payload)},
        "layers": {},
        "segments": [],
    }
    totals: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    for segment_id, expected in reference.items():
        actual = candidate.get(segment_id)
        if actual is None:
            layers = {}
            for kind in ("token", "compound", "bunsetsu", "sentence"):
                value = score(span_set(expected, kind), set())
                layers[kind] = value
                for key in ("reference", "candidate", "matched"):
                    totals[kind][key] += value[key]
            results["segments"].append({"id": segment_id, "status": "missing_candidate", "layers": layers})
            continue
        layers = {}
        for kind in ("token", "compound", "bunsetsu", "sentence"):
            value = score(span_set(expected, kind), span_set(actual, kind))
            layers[kind] = value
            for key in ("reference", "candidate", "matched"):
                totals[kind][key] += value[key]
        results["segments"].append({"id": segment_id, "status": "compared", "layers": layers})
    for segment_id in sorted(candidate.keys() - reference.keys()):
        results["segments"].append({"id": segment_id, "status": "unexpected_candidate"})
    for kind, total in totals.items():
        candidate_count = total["candidate"]
        matched = total["matched"]
        precision = matched / candidate_count if candidate_count else 0.0
        recall = matched / total["reference"] if total["reference"] else 0.0
        results["layers"][kind] = {
            **total,
            "precision": precision,
            "recall": recall,
            "f1": 2 * precision * recall / (precision + recall) if precision + recall else 0.0,
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps({"output": str(args.output), "layers": results["layers"], "segments": len(results["segments"])}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
