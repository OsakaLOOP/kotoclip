"""将 provider 结构 artifact 转为 UniDic token 监督样本。

只有完整覆盖 UniDic token 的 span 才进入 labels；partial/unmatched span
记录在 conflicts，供人工复核，不参与自动训练。
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def covered_indices(span: dict[str, Any], tokens: list[dict[str, Any]]) -> list[int] | None:
    start, end = span["char_range"]
    indices = [i for i, token in enumerate(tokens) if token["char_range"][0] >= start and token["char_range"][1] <= end]
    if not indices or tokens[indices[0]]["char_range"][0] != start or tokens[indices[-1]]["char_range"][1] != end:
        return None
    if any(tokens[i]["char_range"][1] != tokens[i + 1]["char_range"][0] for i in indices[:-1]):
        return None
    return indices


def build(artifact_path: Path, unidic_path: Path) -> dict[str, Any]:
    artifact_bundle = json.loads(artifact_path.read_text(encoding="utf-8"))
    unidic_bundle = json.loads(unidic_path.read_text(encoding="utf-8"))
    unidic = {segment["id"]: segment for segment in unidic_bundle["segments"]}
    samples: list[dict[str, Any]] = []
    conflicts: list[dict[str, Any]] = []
    totals: dict[str, int] = {kind: 0 for kind in ("sentence", "compound", "bunsetsu")}
    accepted: dict[str, int] = {kind: 0 for kind in totals}
    for artifact in artifact_bundle["artifacts"]:
        segment_id = artifact["segment_id"]
        segment = unidic.get(segment_id)
        if segment is None:
            conflicts.append({"segment_id": segment_id, "reason": "missing_unidic_segment"})
            continue
        tokens = segment.get("tokens", [])
        labels = [{"token_id": token["id"], "char_range": token["char_range"], "surface": token.get("surface"), "sentence_start": False, "sentence_end": False, "compound_start": False, "compound_end": False, "bunsetsu_start": False, "bunsetsu_end": False} for token in tokens]
        for span in artifact.get("spans", []):
            kind = span.get("kind")
            if kind not in {"sentence", "compound", "bunsetsu"}:
                continue
            totals[kind] += 1
            indices = covered_indices(span, tokens)
            if indices is None:
                conflicts.append({"segment_id": segment_id, "provider": artifact["provider"]["id"], "span_id": span["id"], "kind": kind, "char_range": span["char_range"], "reason": "incomplete_unidic_coverage"})
                continue
            accepted[kind] += 1
            prefix = f"{kind}_"
            labels[indices[0]][prefix + "start"] = True
            labels[indices[-1]][prefix + "end"] = True
        samples.append({"segment_id": segment_id, "provider": artifact["provider"], "tokens": labels})
    rates = {kind: (accepted[kind] / totals[kind] if totals[kind] else None) for kind in totals}
    return {"schema": "kotoclip.unidic-structure-supervision.v1", "input": {"artifact": str(artifact_path), "unidic": str(unidic_path)}, "statistics": {"total_spans": totals, "accepted_spans": accepted, "coverage_rate": rates}, "samples": samples, "conflicts": conflicts}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--unidic", type=Path, default=Path("experiments/unidic-provider-validation.json"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = build(args.artifact, args.unidic)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "samples": len(result["samples"]), "conflicts": len(result["conflicts"]), "statistics": result["statistics"]}, ensure_ascii=False))


if __name__ == "__main__":
    main()
