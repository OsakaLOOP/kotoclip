"""训练数据工作流：切分、teacher 对齐、合成和 manifest。"""

from __future__ import annotations

import hashlib
import json
import random
from pathlib import Path
from typing import Any, Iterable, Mapping

from .contracts import ContractError, sha256_file, validate_document, validate_span


def _stable_fraction(value: str) -> float:
    digest = hashlib.sha256(value.encode("utf-8")).digest()
    return int.from_bytes(digest[:8], "big") / 2**64


def split_documents(documents: Iterable[Mapping[str, Any]], *, dev_fraction: float, test_fraction: float) -> dict[str, list[Mapping[str, Any]]]:
    if dev_fraction < 0 or test_fraction < 0 or dev_fraction + test_fraction >= 1:
        raise ValueError("dev_fraction + test_fraction must be between 0 and 1")
    result = {"train": [], "dev": [], "test": []}
    for document in documents:
        validate_document(document)
        fraction = _stable_fraction(str(document["document_id"]))
        target = "test" if fraction < test_fraction else "dev" if fraction < test_fraction + dev_fraction else "train"
        result[target].append(document)
    return result


def _token_map(document: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    return {str(token["token_id"]): token for token in document.get("tokens", [])}


def align_teacher_span(document: Mapping[str, Any], teacher_span: Mapping[str, Any]) -> dict[str, Any]:
    """将外部 char span 映射到连续 UniDic token，并保留失败原因。"""
    text = str(document["text"])
    start, end = teacher_span.get("char_range", [None, None])
    if not isinstance(start, int) or not isinstance(end, int) or start < 0 or end < start or end > len(text):
        return {"status": "unmatched", "reason": "invalid_range", "source": dict(teacher_span)}
    token_items = list(document.get("tokens", []))
    covered = [token for token in token_items if token["char_range"][0] >= start and token["char_range"][1] <= end]
    if not covered or covered[0]["char_range"][0] != start or covered[-1]["char_range"][1] != end:
        return {"status": "partial", "reason": "span_does_not_align_to_token_boundaries", "source": dict(teacher_span)}
    expected = text[start:end]
    if "surface" in teacher_span and teacher_span["surface"] != expected:
        return {"status": "surface_mismatch", "reason": "teacher_surface_differs", "source": dict(teacher_span)}
    token_ids = [str(token["token_id"]) for token in covered]
    aligned = dict(teacher_span)
    aligned.update({"status": "exact" if len(covered) == 1 else "compound", "token_ids": token_ids, "surface": expected})
    return aligned


def align_teacher_document(document: Mapping[str, Any], teacher: Mapping[str, Any], *, layer: str) -> dict[str, Any]:
    validate_document(document)
    spans = teacher.get(layer, [])
    if not isinstance(spans, list):
        raise ContractError(f"teacher.{layer} must be an array")
    aligned = [align_teacher_span(document, span) for span in spans]
    counts: dict[str, int] = {}
    for item in aligned:
        counts[item["status"]] = counts.get(item["status"], 0) + 1
    provenance_keys = (
        "teacher_id",
        "teacher_version",
        "checkpoint_sha256",
        "runner_version",
        "command_hash",
        "raw_artifact_sha256",
        "license_status",
    )
    provenance = {key: teacher[key] for key in provenance_keys if key in teacher}
    return {"document_id": document["document_id"], "layer": layer, "teacher": provenance, "spans": aligned, "counts": counts}


def generate_boundary_negatives(document: Mapping[str, Any], *, count: int, seed: int) -> list[dict[str, Any]]:
    """在相邻 token 边界生成可重放的 hard negative 样本。"""
    validate_document(document)
    tokens = list(document.get("tokens", []))
    if len(tokens) < 2:
        return []
    rng = random.Random(seed)
    candidates = list(range(len(tokens) - 1))
    rng.shuffle(candidates)
    output: list[dict[str, Any]] = []
    for index in candidates[: max(0, count)]:
        left, right = tokens[index], tokens[index + 1]
        start, end = left["char_range"][0], right["char_range"][1]
        output.append(
            {
                "sample_id": f"{document['document_id']}:boundary:{index}",
                "document_id": document["document_id"],
                "char_range": [start, end],
                "surface": document["text"][start:end],
                "token_ids": [str(left["token_id"]), str(right["token_id"])],
                "label": "negative_boundary",
                "status": "synthetic",
                "generator_id": "boundary-negative-v1",
                "random_seed": seed,
            }
        )
    return output


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as stream:
        for row in rows:
            stream.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")


def build_manifest(paths: Iterable[Path], *, schema: str, metadata: Mapping[str, Any] | None = None) -> dict[str, Any]:
    files = [{"path": str(path), "sha256": sha256_file(path), "bytes": path.stat().st_size} for path in paths]
    return {"schema": schema, "files": files, "metadata": dict(metadata or {})}
