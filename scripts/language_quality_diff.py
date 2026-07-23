#!/usr/bin/env python3
"""对大型语言管线快照生成机器可读差分和本地可视化报告。"""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import math
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from difflib import SequenceMatcher
from pathlib import Path
from typing import Any, Iterable, Sequence


SCHEMA_VERSION = "kotoclip.quality.diff.v3"
SNAPSHOT_SCHEMA_VERSION = "kotoclip.quality.snapshot.v1"
PRODUCER_VERSION = "3"
MAX_INLINE_VALUE_BYTES = 480
SEVERITY_ORDER = {"critical": 0, "high": 1, "medium": 2, "info": 3}

STAGE_ORDER = (
    "resource",
    "source",
    "preprocessing",
    "morpheme",
    "morphology",
    "word_formation_candidate",
    "word_formation",
    "lexical_candidate",
    "lexical_unit",
    "bunsetsu_boundary",
    "bunsetsu",
    "grammar_candidate",
    "grammar_occurrence",
    "grammar_projection",
    "grammar_residual",
    "personalization",
    "expression_candidate",
    "expression",
    "ui_projection",
)
STAGE_INDEX = {stage: index for index, stage in enumerate(STAGE_ORDER)}
STAGE_DEPENDENCIES = {
    "preprocessing": ("source",),
    "morpheme": ("preprocessing", "resource"),
    # MorphologyArtifact 直接消费原始语素，并参与文节原子连接；它不是文节边界的投影。
    "morphology": ("morpheme", "resource"),
    "word_formation_candidate": ("morpheme", "resource"),
    "word_formation": ("word_formation_candidate",),
    "lexical_candidate": ("morpheme", "word_formation", "resource"),
    "lexical_unit": ("lexical_candidate", "word_formation"),
    "bunsetsu_boundary": (
        "morpheme",
        "morphology",
        "word_formation",
        "lexical_unit",
        "resource",
    ),
    "bunsetsu": ("bunsetsu_boundary",),
    "grammar_candidate": ("morphology", "bunsetsu", "resource"),
    "grammar_occurrence": ("grammar_candidate",),
    "grammar_projection": ("grammar_occurrence",),
    "grammar_residual": ("morphology", "bunsetsu", "resource"),
    "personalization": ("bunsetsu",),
    "expression_candidate": (
        "morpheme",
        "bunsetsu",
        "grammar_occurrence",
        "lexical_unit",
        "resource",
    ),
    "expression": ("expression_candidate", "personalization"),
    "ui_projection": (
        "lexical_unit",
        "grammar_projection",
        "expression",
        "personalization",
    ),
}


@dataclass(frozen=True)
class ComparisonBundle:
    manifest: dict[str, Any]
    summary: dict[str, Any]
    changes: list[dict[str, Any]]


def canonical_json(value: Any) -> str:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    )


def content_hash(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def file_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_descriptor(path: Path) -> dict[str, Any]:
    return {
        "path": path.as_posix(),
        "bytes": path.stat().st_size,
        "sha256": file_hash(path),
    }


def read_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as source:
        return json.load(source)


def pointer_escape(value: str) -> str:
    return value.replace("~", "~0").replace("/", "~1")


def value_descriptor(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    encoded = canonical_json(value).encode("utf-8")
    if len(encoded) <= MAX_INLINE_VALUE_BYTES:
        return value
    return {
        "kind": "array" if isinstance(value, list) else "object",
        "items": len(value),
        "bytes": len(encoded),
        "sha256": hashlib.sha256(encoded).hexdigest(),
    }


def semantic_list_key(value: Any) -> str | None:
    if not isinstance(value, dict):
        return None
    for field in ("occurrence_id", "match_id", "chain_id", "review_id"):
        if value.get(field) not in (None, ""):
            return f"{field}={value[field]}"
    parts: list[str] = []
    for field in ("rule_id", "concept_id", "surface", "base_form", "name"):
        if value.get(field) not in (None, ""):
            parts.append(f"{field}={value[field]}")
    if "char_range" in value:
        parts.append(f"char_range={canonical_json(value['char_range'])}")
    return "|".join(parts) if parts else None


def keyed_list(values: list[Any]) -> dict[str, Any] | None:
    result: dict[str, Any] = {}
    for value in values:
        key = semantic_list_key(value)
        if key is None or key in result:
            return None
        result[key] = value
    return result


def field_changes(before: Any, after: Any, path: str = "") -> list[dict[str, Any]]:
    if before == after:
        return []
    if isinstance(before, dict) and isinstance(after, dict):
        changes: list[dict[str, Any]] = []
        for key in sorted(set(before) | set(after)):
            child_path = f"{path}/{pointer_escape(str(key))}"
            if key not in before:
                changes.append(
                    {"path": child_path, "before": None, "after": value_descriptor(after[key])}
                )
            elif key not in after:
                changes.append(
                    {"path": child_path, "before": value_descriptor(before[key]), "after": None}
                )
            else:
                changes.extend(field_changes(before[key], after[key], child_path))
        return changes
    if isinstance(before, list) and isinstance(after, list):
        before_keyed = keyed_list(before)
        after_keyed = keyed_list(after)
        if before_keyed is not None and after_keyed is not None:
            changes = []
            for key in sorted(set(before_keyed) | set(after_keyed)):
                child_path = f"{path}[{pointer_escape(key)}]"
                if key not in before_keyed:
                    changes.append(
                        {
                            "path": child_path,
                            "before": None,
                            "after": value_descriptor(after_keyed[key]),
                        }
                    )
                elif key not in after_keyed:
                    changes.append(
                        {
                            "path": child_path,
                            "before": value_descriptor(before_keyed[key]),
                            "after": None,
                        }
                    )
                else:
                    changes.extend(
                        field_changes(before_keyed[key], after_keyed[key], child_path)
                    )
            return changes
    return [
        {
            "path": path or "/",
            "before": value_descriptor(before),
            "after": value_descriptor(after),
        }
    ]


def with_change_id(change: dict[str, Any]) -> dict[str, Any]:
    payload = dict(change)
    payload["change_id"] = content_hash(change)[:20]
    return payload


def text_of_report(report: dict[str, Any]) -> str:
    return "".join(str(item.get("surface", "")) for item in report.get("bunsetsus", []))


def range_key(value: Any) -> str:
    return canonical_json(value if value is not None else [])


def indexed_unique(
    values: Iterable[dict[str, Any]], key_getter: Any
) -> dict[str, dict[str, Any]]:
    groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for value in values:
        groups[str(key_getter(value))].append(value)
    result: dict[str, dict[str, Any]] = {}
    for key, group in groups.items():
        for ordinal, value in enumerate(sorted(group, key=canonical_json)):
            result[f"{key}#{ordinal}"] = value
    return result


def bunsetsu_metrics(reports: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "lines": len(reports),
        "bunsetsus": sum(len(report.get("bunsetsus", [])) for report in reports),
        "boundaries": sum(len(report.get("boundaries", [])) for report in reports),
        "unresolved_boundaries": sum(
            int(report.get("unresolved_boundaries", 0)) for report in reports
        ),
        "reconstruction_failures": sum(
            report.get("reconstruction_ok") is False for report in reports
        ),
        "range_integrity_failures": sum(
            report.get("range_integrity_ok") is False for report in reports
        ),
    }


def segmentation_opcodes(before: Sequence[str], after: Sequence[str]) -> list[dict[str, Any]]:
    matcher = SequenceMatcher(a=list(before), b=list(after), autojunk=False)
    return [
        {
            "operation": operation,
            "before_range": [before_start, before_end],
            "after_range": [after_start, after_end],
            "before": list(before[before_start:before_end]),
            "after": list(after[after_start:after_end]),
        }
        for operation, before_start, before_end, after_start, after_end in matcher.get_opcodes()
        if operation != "equal"
    ]


def compare_bunsetsu(
    before: Any, after: Any
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    if not isinstance(before, list) or not isinstance(after, list):
        raise ValueError("bunsetsu 适配器要求输入为逐行报告 JSON 数组")
    if not all(isinstance(item, dict) for item in before + after):
        raise ValueError("bunsetsu 报告数组的每一项必须是对象")

    changes: list[dict[str, Any]] = []
    changed_lines: set[tuple[int | None, int | None]] = set()
    before_texts = [text_of_report(report) for report in before]
    after_texts = [text_of_report(report) for report in after]
    matcher = SequenceMatcher(a=before_texts, b=after_texts, autojunk=False)

    def add(change: dict[str, Any], before_line: int | None, after_line: int | None) -> None:
        changed_lines.add((before_line, after_line))
        changes.append(
            with_change_id(
                {
                    "channel": "bunsetsu",
                    "before_line": before_line,
                    "after_line": after_line,
                    **change,
                }
            )
        )

    for operation, before_start, before_end, after_start, after_end in matcher.get_opcodes():
        if operation == "delete":
            for before_line in range(before_start, before_end):
                add(
                    {
                        "type": "input_line_removed",
                        "severity": "critical",
                        "context": before_texts[before_line],
                    },
                    before_line,
                    None,
                )
            continue
        if operation == "insert":
            for after_line in range(after_start, after_end):
                add(
                    {
                        "type": "input_line_added",
                        "severity": "critical",
                        "context": after_texts[after_line],
                    },
                    None,
                    after_line,
                )
            continue
        if operation == "replace":
            pair_count = min(before_end - before_start, after_end - after_start)
            for offset in range(pair_count):
                before_line = before_start + offset
                after_line = after_start + offset
                add(
                    {
                        "type": "input_text_changed",
                        "severity": "critical",
                        "before": before_texts[before_line],
                        "after": after_texts[after_line],
                        "context": after_texts[after_line],
                    },
                    before_line,
                    after_line,
                )
            for before_line in range(before_start + pair_count, before_end):
                add(
                    {
                        "type": "input_line_removed",
                        "severity": "critical",
                        "context": before_texts[before_line],
                    },
                    before_line,
                    None,
                )
            for after_line in range(after_start + pair_count, after_end):
                add(
                    {
                        "type": "input_line_added",
                        "severity": "critical",
                        "context": after_texts[after_line],
                    },
                    None,
                    after_line,
                )
            continue

        for before_line, after_line in zip(
            range(before_start, before_end), range(after_start, after_end), strict=True
        ):
            before_report = before[before_line]
            after_report = after[after_line]
            context = after_texts[after_line]
            before_bunsetsus = before_report.get("bunsetsus", [])
            after_bunsetsus = after_report.get("bunsetsus", [])
            before_surfaces = [str(item.get("surface", "")) for item in before_bunsetsus]
            after_surfaces = [str(item.get("surface", "")) for item in after_bunsetsus]
            if before_surfaces != after_surfaces:
                add(
                    {
                        "type": "bunsetsu_segmentation_changed",
                        "severity": "high",
                        "context": context,
                        "before": before_surfaces,
                        "after": after_surfaces,
                        "operations": segmentation_opcodes(before_surfaces, after_surfaces),
                    },
                    before_line,
                    after_line,
                )

            before_anchored = indexed_unique(
                before_bunsetsus, lambda item: range_key(item.get("char_range"))
            )
            after_anchored = indexed_unique(
                after_bunsetsus, lambda item: range_key(item.get("char_range"))
            )
            for anchor in sorted(set(before_anchored) & set(after_anchored)):
                differences = field_changes(before_anchored[anchor], after_anchored[anchor])
                if differences:
                    add(
                        {
                            "type": "bunsetsu_fields_changed",
                            "severity": "medium",
                            "context": context,
                            "anchor": anchor.rsplit("#", 1)[0],
                            "field_changes": differences,
                        },
                        before_line,
                        after_line,
                    )

            before_boundaries = indexed_unique(
                before_report.get("boundaries", []), lambda item: item.get("morpheme_index")
            )
            after_boundaries = indexed_unique(
                after_report.get("boundaries", []), lambda item: item.get("morpheme_index")
            )
            for anchor in sorted(set(before_boundaries) | set(after_boundaries)):
                before_boundary = before_boundaries.get(anchor)
                after_boundary = after_boundaries.get(anchor)
                if before_boundary is None:
                    add(
                        {
                            "type": "boundary_added",
                            "severity": "high",
                            "context": context,
                            "anchor": anchor.rsplit("#", 1)[0],
                            "after": value_descriptor(after_boundary),
                        },
                        before_line,
                        after_line,
                    )
                elif after_boundary is None:
                    add(
                        {
                            "type": "boundary_removed",
                            "severity": "high",
                            "context": context,
                            "anchor": anchor.rsplit("#", 1)[0],
                            "before": value_descriptor(before_boundary),
                        },
                        before_line,
                        after_line,
                    )
                else:
                    differences = field_changes(before_boundary, after_boundary)
                    if differences:
                        severity = (
                            "high"
                            if before_boundary.get("decision") != after_boundary.get("decision")
                            else "medium"
                        )
                        add(
                            {
                                "type": "boundary_changed",
                                "severity": severity,
                                "context": context,
                                "anchor": anchor.rsplit("#", 1)[0],
                                "field_changes": differences,
                            },
                            before_line,
                            after_line,
                        )

            before_flags = {
                key: before_report.get(key)
                for key in ("unresolved_boundaries", "reconstruction_ok", "range_integrity_ok")
            }
            after_flags = {
                key: after_report.get(key)
                for key in ("unresolved_boundaries", "reconstruction_ok", "range_integrity_ok")
            }
            differences = field_changes(before_flags, after_flags)
            if differences:
                add(
                    {
                        "type": "line_integrity_changed",
                        "severity": "critical",
                        "context": context,
                        "field_changes": differences,
                    },
                    before_line,
                    after_line,
                )

    metrics_before = bunsetsu_metrics(before)
    metrics_after = bunsetsu_metrics(after)
    metrics = {
        key: {
            "before": metrics_before[key],
            "after": metrics_after[key],
            "delta": metrics_after[key] - metrics_before[key],
        }
        for key in metrics_before
    }
    metrics["changed_lines"] = {
        "before": 0,
        "after": len(changed_lines),
        "delta": len(changed_lines),
    }
    return metrics, changes


def expression_key(item: dict[str, Any]) -> str:
    return "|".join(
        [
            str(item.get("origin", "")),
            str(item.get("rule_id") or item.get("label") or ""),
            range_key(item.get("char_range")),
            str(item.get("surface", "")),
        ]
    )


def expression_metrics(items: list[dict[str, Any]]) -> dict[str, Any]:
    statuses = Counter(str(item.get("status", "unknown")) for item in items)
    origins = Counter(str(item.get("origin", "unknown")) for item in items)
    return {
        "occurrences": len(items),
        "statuses": dict(sorted(statuses.items())),
        "origins": dict(sorted(origins.items())),
        "rules": len({str(item.get("rule_id") or item.get("label") or "") for item in items}),
    }


def expression_preview(item: dict[str, Any] | None) -> Any:
    if item is None:
        return None
    return {
        key: item.get(key)
        for key in (
            "status",
            "rule_id",
            "label",
            "origin",
            "surface",
            "char_range",
            "matched_ranges",
            "rejection_reason",
        )
        if key in item
    }


def compare_expression(
    before: Any, after: Any
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    if not isinstance(before, list) or not isinstance(after, list):
        raise ValueError("expression 适配器要求输入为 occurrence JSON 数组")
    if not all(isinstance(item, dict) for item in before + after):
        raise ValueError("expression occurrence 必须是对象")
    before_index = indexed_unique(before, expression_key)
    after_index = indexed_unique(after, expression_key)
    changes: list[dict[str, Any]] = []
    for anchor in sorted(set(before_index) | set(after_index)):
        before_item = before_index.get(anchor)
        after_item = after_index.get(anchor)
        context = str((after_item or before_item or {}).get("context", ""))
        if before_item is None:
            change = {
                "channel": "expression",
                "type": "expression_added",
                "severity": "medium",
                "anchor": anchor.rsplit("#", 1)[0],
                "context": context,
                "after": expression_preview(after_item),
            }
        elif after_item is None:
            change = {
                "channel": "expression",
                "type": "expression_removed",
                "severity": "high",
                "anchor": anchor.rsplit("#", 1)[0],
                "context": context,
                "before": expression_preview(before_item),
            }
        else:
            differences = field_changes(before_item, after_item)
            if not differences:
                continue
            change = {
                "channel": "expression",
                "type": "expression_fields_changed",
                "severity": "high"
                if before_item.get("status") != after_item.get("status")
                else "medium",
                "anchor": anchor.rsplit("#", 1)[0],
                "context": context,
                "before": expression_preview(before_item),
                "after": expression_preview(after_item),
                "field_changes": differences,
            }
        changes.append(with_change_id(change))

    before_metrics = expression_metrics(before)
    after_metrics = expression_metrics(after)
    metrics: dict[str, Any] = {
        "occurrences": {
            "before": before_metrics["occurrences"],
            "after": after_metrics["occurrences"],
            "delta": after_metrics["occurrences"] - before_metrics["occurrences"],
        },
        "rules": {
            "before": before_metrics["rules"],
            "after": after_metrics["rules"],
            "delta": after_metrics["rules"] - before_metrics["rules"],
        },
        "statuses": {"before": before_metrics["statuses"], "after": after_metrics["statuses"]},
        "origins": {"before": before_metrics["origins"], "after": after_metrics["origins"]},
    }
    return metrics, changes


ARTIFACT_STAGE_COVERAGE = {
    "tokens": {
        "morpheme",
        "morphology",
        "word_formation",
        "lexical_unit",
        "bunsetsu",
        "grammar_occurrence",
        "grammar_projection",
        "grammar_residual",
        "personalization",
    },
    "word_formations": {"word_formation_candidate"},
    "lexical_candidates": {"lexical_candidate"},
    "bunsetsu": {
        "morpheme",
        "word_formation",
        "lexical_unit",
        "bunsetsu_boundary",
        "bunsetsu",
    },
    "grammar_occurrences": {"grammar_candidate", "grammar_occurrence"},
    "grammar_residuals": {"grammar_residual"},
    "expressions": {"expression_candidate", "expression"},
    "catalogs": {"resource"},
    "ui_projection": {"ui_projection"},
}


def normalized_range(value: Any) -> tuple[int, int] | None:
    if (
        isinstance(value, (list, tuple))
        and len(value) == 2
        and all(isinstance(item, int) for item in value)
    ):
        return int(value[0]), int(value[1])
    return None


def normalized_ranges(value: Any) -> tuple[tuple[int, int], ...]:
    if isinstance(value, dict):
        for field in (
            "matched_ranges",
            "display_ranges",
            "source_ranges",
            "matchedRanges",
            "displayRanges",
            "sourceRanges",
        ):
            ranges = tuple(
                item
                for item in (normalized_range(raw) for raw in value.get(field, []))
                if item is not None
            )
            if ranges:
                return ranges
        for field in (
            "char_range",
            "anchor_range",
            "_quality_range",
            "charRange",
            "anchorRange",
            "range",
        ):
            item = normalized_range(value.get(field))
            if item is not None:
                return (item,)
    item = normalized_range(value)
    return (item,) if item is not None else ()


def span_of_ranges(ranges: Sequence[tuple[int, int]]) -> tuple[int, int] | None:
    if not ranges:
        return None
    return min(item[0] for item in ranges), max(item[1] for item in ranges)


def ranges_intersect(
    left: Sequence[tuple[int, int]], right: Sequence[tuple[int, int]]
) -> bool:
    if not left or not right:
        return False
    for left_start, left_end in left:
        for right_start, right_end in right:
            if left_start == left_end:
                if right_start <= left_start <= right_end:
                    return True
            elif right_start == right_end:
                if left_start <= right_start <= left_end:
                    return True
            elif left_start < right_end and right_start < left_end:
                return True
    return False


def entity_key(kind: str, ranges: Sequence[tuple[int, int]], *parts: Any) -> str:
    clean_parts = [str(part) for part in parts if part not in (None, "")]
    return "|".join([kind, canonical_json(list(ranges)), *clean_parts])


def context_for(value: dict[str, Any]) -> str:
    for field in ("context", "surface", "label", "base_form", "query"):
        if value.get(field) not in (None, ""):
            return str(value[field])
    return ""


def snapshot_entity(
    stage: str,
    kind: str,
    value: dict[str, Any],
    artifact: str,
    *,
    key_parts: Sequence[Any] = (),
    anchor_parts: Sequence[Any] = (),
    ranges: Sequence[tuple[int, int]] | None = None,
) -> dict[str, Any]:
    entity_ranges = tuple(ranges) if ranges is not None else normalized_ranges(value)
    return {
        "stage": stage,
        "kind": kind,
        "key": entity_key(kind, entity_ranges, *key_parts),
        "anchor": entity_key(kind, entity_ranges, *(anchor_parts or key_parts)),
        "ranges": [list(item) for item in entity_ranges],
        "context": context_for(value),
        "artifact": artifact,
        "value": value,
    }


def append_entity(
    target: dict[str, list[dict[str, Any]]],
    seen: set[tuple[str, str, str]],
    entity: dict[str, Any],
) -> None:
    signature = (entity["stage"], entity["key"], content_hash(entity["value"]))
    if signature in seen:
        return
    seen.add(signature)
    target[entity["stage"]].append(entity)


def stripped_bunsetsu(value: dict[str, Any]) -> dict[str, Any]:
    excluded = {
        "morphemes",
        "word_formations",
        "lexical_units",
        "morphology",
        "grammar_occurrences",
        "grammar_tags",
        "functional_residuals",
    }
    return {key: item for key, item in value.items() if key not in excluded}


def add_bunsetsu_entities(
    entities: dict[str, list[dict[str, Any]]],
    seen: set[tuple[str, str, str]],
    bunsetsu: dict[str, Any],
    artifact: str,
) -> None:
    for morpheme in bunsetsu.get("morphemes", []):
        ranges = normalized_ranges(morpheme)
        append_entity(
            entities,
            seen,
            snapshot_entity(
                "morpheme",
                "morpheme",
                morpheme,
                artifact,
                key_parts=(range_key(morpheme.get("char_range")),),
                anchor_parts=(range_key(morpheme.get("char_range")),),
                ranges=ranges,
            ),
        )
    for formation in bunsetsu.get("word_formations", []):
        append_entity(
            entities,
            seen,
            snapshot_entity(
                "word_formation",
                "accepted_word_formation",
                formation,
                artifact,
                key_parts=(formation.get("rule_id"),),
                anchor_parts=(formation.get("rule_id"), formation.get("surface")),
            ),
        )
    for lexical_unit in bunsetsu.get("lexical_units", []):
        append_entity(
            entities,
            seen,
            snapshot_entity(
                "lexical_unit",
                "accepted_lexical_unit",
                lexical_unit,
                artifact,
                key_parts=(lexical_unit.get("surface"), lexical_unit.get("base_form")),
                anchor_parts=(lexical_unit.get("surface"),),
            ),
        )
    append_entity(
        entities,
        seen,
        snapshot_entity(
            "bunsetsu",
            "bunsetsu",
            stripped_bunsetsu(bunsetsu),
            artifact,
            key_parts=(range_key(bunsetsu.get("char_range")),),
            anchor_parts=(range_key(bunsetsu.get("char_range")),),
        ),
    )
    morphology = bunsetsu.get("morphology", {})
    for chain in morphology.get("chains", []):
        append_entity(
            entities,
            seen,
            snapshot_entity(
                "morphology",
                "morphology_chain",
                chain,
                artifact,
                key_parts=(chain.get("role"), chain.get("base_lexeme")),
                anchor_parts=(chain.get("role"), chain.get("base_lexeme")),
            ),
        )
    for unclassified in morphology.get("unclassified", []):
        ranges = normalized_ranges(unclassified)
        value = {"char_range": list(ranges[0])} if ranges else {"char_range": unclassified}
        append_entity(
            entities,
            seen,
            snapshot_entity(
                "morphology",
                "unclassified_morphology",
                value,
                artifact,
                key_parts=(range_key(unclassified),),
                ranges=ranges,
            ),
        )
    for occurrence in bunsetsu.get("grammar_occurrences", []):
        append_grammar_occurrence(entities, seen, occurrence, artifact)
    for tag in bunsetsu.get("grammar_tags", []):
        append_entity(
            entities,
            seen,
            snapshot_entity(
                "grammar_projection",
                "grammar_tag",
                tag,
                artifact,
                key_parts=(tag.get("concept_id"), tag.get("pattern_id")),
                anchor_parts=(tag.get("concept_id"),),
            ),
        )
    for residual in bunsetsu.get("functional_residuals", []):
        append_entity(
            entities,
            seen,
            snapshot_entity(
                "grammar_residual",
                "functional_residual",
                residual,
                artifact,
                key_parts=(residual.get("surface"), residual.get("base_form")),
                anchor_parts=(residual.get("surface"),),
            ),
        )


def append_grammar_occurrence(
    entities: dict[str, list[dict[str, Any]]],
    seen: set[tuple[str, str, str]],
    occurrence: dict[str, Any],
    artifact: str,
    authoritative_keys: set[tuple[str, str]] | None = None,
) -> None:
    status = str(occurrence.get("status", "accepted"))
    stage = "grammar_occurrence" if status == "accepted" else "grammar_candidate"
    entity = snapshot_entity(
        stage,
        "grammar_occurrence",
        occurrence,
        artifact,
        key_parts=(occurrence.get("concept_id"), occurrence.get("rule_id")),
        anchor_parts=(occurrence.get("concept_id"),),
    )
    if authoritative_keys is not None and (stage, entity["key"]) in authoritative_keys:
        return
    append_entity(entities, seen, entity)


def resource_affects(kind: str, name: str) -> list[str]:
    lowered = name.lower()
    if kind == "cli":
        return [stage for stage in STAGE_ORDER if stage not in {"resource", "source"}]
    if kind == "profile":
        return ["personalization", "expression_candidate", "expression"]
    if kind == "system_dictionary":
        return ["morpheme"]
    if kind.startswith("dictionary_"):
        return ["lexical_candidate", "lexical_unit", "expression_candidate"]
    if "word" in lowered and "formation" in lowered:
        return ["word_formation_candidate", "word_formation"]
    if "lexical" in lowered:
        return ["lexical_candidate", "lexical_unit"]
    if "bunsetsu" in lowered:
        return ["bunsetsu_boundary", "bunsetsu"]
    if "grammar" in lowered or "morph" in lowered:
        return [
            "morphology",
            "grammar_candidate",
            "grammar_occurrence",
            "grammar_projection",
            "grammar_residual",
        ]
    if "expression" in lowered:
        return ["expression_candidate", "expression"]
    return [
        "word_formation",
        "word_formation_candidate",
        "lexical_candidate",
        "bunsetsu_boundary",
        "grammar_candidate",
        "grammar_occurrence",
        "expression_candidate",
    ]


def normalized_resource_descriptor(kind: str, descriptor: dict[str, Any]) -> dict[str, Any]:
    name = str(
        descriptor.get("logical_name", descriptor.get("path", "unknown"))
    ).replace("\\", "/")
    return {
        "resource_kind": kind,
        "name": name,
        "bytes": descriptor.get("bytes"),
        "sha256": descriptor.get("sha256"),
        "affects_stages": resource_affects(kind, name),
    }


def load_snapshot_artifact(
    manifest_path: Path, name: str, descriptor: dict[str, Any]
) -> Any:
    path = (manifest_path.parent / str(descriptor["path"])).resolve()
    if not path.is_file():
        raise FileNotFoundError(f"快照产物不存在：{name} -> {path}")
    expected_hash = descriptor.get("sha256")
    actual_hash = file_hash(path)
    if expected_hash and expected_hash != actual_hash:
        raise ValueError(f"快照产物哈希不一致：{name} -> {path}")
    return read_json(path)


def normalize_snapshot(
    manifest_path: Path,
) -> tuple[dict[str, Any], dict[str, list[dict[str, Any]]], set[str]]:
    manifest = read_json(manifest_path)
    if manifest.get("schema_version") != SNAPSHOT_SCHEMA_VERSION:
        raise ValueError(
            f"快照 schema 不兼容：{manifest.get('schema_version')}，"
            f"要求 {SNAPSHOT_SCHEMA_VERSION}"
        )
    entities: dict[str, list[dict[str, Any]]] = defaultdict(list)
    seen: set[tuple[str, str, str]] = set()
    covered = {"source", "preprocessing"}
    corpus = manifest.get("corpus", {})
    source_value = {
        "corpus_id": corpus.get("id"),
        "selected_sha256": corpus.get("selected_sha256"),
        "selected_bytes": corpus.get("selected_bytes"),
        "selected_characters": corpus.get("selected_characters"),
    }
    append_entity(
        entities,
        seen,
        snapshot_entity(
            "source",
            "selected_source",
            source_value,
            "manifest",
            key_parts=(corpus.get("id", "corpus"),),
            ranges=((0, int(corpus.get("selected_characters") or 0)),),
        ),
    )
    preprocessing_value = {
        "analysis_text_sha256": corpus.get("analysis_text_sha256"),
        "analysis_characters": corpus.get("analysis_characters"),
    }
    append_entity(
        entities,
        seen,
        snapshot_entity(
            "preprocessing",
            "prepared_text",
            preprocessing_value,
            "manifest",
            key_parts=(corpus.get("id", "corpus"),),
            ranges=((0, int(corpus.get("analysis_characters") or 0)),),
        ),
    )

    resources = manifest.get("resources", {})
    for kind in ("cli", "system_dictionary", "profile"):
        raw = resources.get(kind)
        if isinstance(raw, dict):
            value = normalized_resource_descriptor(kind, raw)
            append_entity(
                entities,
                seen,
                snapshot_entity(
                    "resource",
                    "pipeline_resource",
                    value,
                    "manifest",
                    key_parts=(kind, value["name"]),
                    ranges=(),
                ),
            )
            covered.add("resource")
    for kind in ("dictionary_sources", "dictionary_caches", "catalogs"):
        singular = kind.removesuffix("s")
        for raw in resources.get(kind, []):
            value = normalized_resource_descriptor(singular, raw)
            append_entity(
                entities,
                seen,
                snapshot_entity(
                    "resource",
                    "pipeline_resource",
                    value,
                    "manifest",
                    key_parts=(singular, value["name"]),
                    ranges=(),
                ),
            )
            covered.add("resource")

    artifact_descriptors = manifest.get("artifacts", {})
    loaded = {
        name: load_snapshot_artifact(manifest_path, name, descriptor)
        for name, descriptor in artifact_descriptors.items()
    }
    for name in artifact_descriptors:
        covered.update(ARTIFACT_STAGE_COVERAGE.get(name, set()))

    tokens = loaded.get("tokens")
    has_tokens = isinstance(tokens, list)
    if has_tokens:
        for token in tokens:
            bunsetsu = token.get("bunsetsu", {})
            is_content = token.get("display_class", "content") == "content"
            if is_content and isinstance(bunsetsu, dict):
                add_bunsetsu_entities(entities, seen, bunsetsu, "tokens")
            for expression in token.get("expressions", []):
                append_expression_entity(entities, seen, expression, "tokens")
            if not is_content:
                continue
            personalization = {
                key: token.get(key)
                for key in (
                    "novelty_score",
                    "is_selected",
                    "is_known",
                    "inference_reason",
                    "display_class",
                )
                if key in token
            }
            token_range = normalized_ranges(bunsetsu)
            append_entity(
                entities,
                seen,
                snapshot_entity(
                    "personalization",
                    "token_personalization",
                    {**personalization, "char_range": bunsetsu.get("char_range")},
                    "tokens",
                    key_parts=(range_key(bunsetsu.get("char_range")),),
                    ranges=token_range,
                ),
            )
    authoritative_grammar_keys = {
        (stage, entity["key"])
        for stage in ("grammar_candidate", "grammar_occurrence")
        for entity in entities.get(stage, [])
    }

    bunsetsu_reports = loaded.get("bunsetsu")
    if isinstance(bunsetsu_reports, list):
        for report_index, report in enumerate(bunsetsu_reports):
            bunsetsus = report.get("bunsetsus", [])
            flat_morphemes = [
                morpheme
                for bunsetsu in bunsetsus
                for morpheme in bunsetsu.get("morphemes", [])
            ]
            report_ranges = [
                item
                for item in (normalized_range(item.get("char_range")) for item in bunsetsus)
                if item is not None
            ]
            report_span = span_of_ranges(report_ranges)
            if not has_tokens:
                for bunsetsu in bunsetsus:
                    add_bunsetsu_entities(entities, seen, bunsetsu, "bunsetsu")
            for boundary in report.get("boundaries", []):
                morpheme_index = int(boundary.get("morpheme_index", 0))
                point = None
                if 0 < morpheme_index <= len(flat_morphemes):
                    previous = normalized_range(
                        flat_morphemes[morpheme_index - 1].get("char_range")
                    )
                    point = previous[1] if previous else None
                value = dict(boundary)
                if point is not None:
                    value["_quality_range"] = [point, point]
                ranges = ((point, point),) if point is not None else ()
                append_entity(
                    entities,
                    seen,
                    snapshot_entity(
                        "bunsetsu_boundary",
                        "bunsetsu_boundary",
                        value,
                        "bunsetsu",
                        key_parts=(report_index, point, morpheme_index),
                        anchor_parts=(point,),
                        ranges=ranges,
                    ),
                )
            integrity = {
                "report_index": report_index,
                "char_range": list(report_span) if report_span else None,
                "unresolved_boundaries": report.get("unresolved_boundaries"),
                "reconstruction_ok": report.get("reconstruction_ok"),
                "range_integrity_ok": report.get("range_integrity_ok"),
            }
            append_entity(
                entities,
                seen,
                snapshot_entity(
                    "bunsetsu_boundary",
                    "segment_integrity",
                    integrity,
                    "bunsetsu",
                    key_parts=(report_index,),
                    ranges=(report_span,) if report_span else (),
                ),
            )

    word_report = loaded.get("word_formations")
    if isinstance(word_report, dict):
        for item in word_report.get("items", []):
            formation = dict(item.get("formation", {}))
            value = {
                **formation,
                "status": "accepted",
                "morpheme_signature": item.get("morpheme_signature", []),
                "output_pos": item.get("output_pos"),
            }
            append_entity(
                entities,
                seen,
                snapshot_entity(
                    "word_formation_candidate",
                    "word_formation_candidate",
                    value,
                    "word_formations",
                    key_parts=(formation.get("rule_id"),),
                    anchor_parts=(formation.get("rule_id"), formation.get("surface")),
                ),
            )
        for item in word_report.get("rejected", []):
            value = {**item, "status": "rejected"}
            append_entity(
                entities,
                seen,
                snapshot_entity(
                    "word_formation_candidate",
                    "word_formation_candidate",
                    value,
                    "word_formations",
                    key_parts=(item.get("rule_id"),),
                    anchor_parts=(item.get("rule_id"),),
                ),
            )

    lexical_report = loaded.get("lexical_candidates")
    if isinstance(lexical_report, dict):
        for item in lexical_report.get("items", []):
            candidate = dict(item.get("candidate", {}))
            value = {
                **candidate,
                "morpheme_signature": item.get("morpheme_signature", []),
            }
            append_entity(
                entities,
                seen,
                snapshot_entity(
                    "lexical_candidate",
                    "lexical_candidate",
                    value,
                    "lexical_candidates",
                    key_parts=(candidate.get("query"), candidate.get("lexical_shape")),
                    anchor_parts=(candidate.get("surface"), candidate.get("query")),
                ),
            )

    grammar_occurrences = loaded.get("grammar_occurrences")
    if isinstance(grammar_occurrences, list):
        for occurrence in grammar_occurrences:
            append_grammar_occurrence(
                entities,
                seen,
                occurrence,
                "grammar_occurrences",
                authoritative_grammar_keys,
            )
    residual_report = loaded.get("grammar_residuals")
    if isinstance(residual_report, dict):
        for residual in residual_report.get("items", []):
            append_entity(
                entities,
                seen,
                snapshot_entity(
                    "grammar_residual",
                    "functional_residual",
                    residual,
                    "grammar_residuals",
                    key_parts=(residual.get("surface"), residual.get("base_form")),
                    anchor_parts=(residual.get("surface"),),
                ),
            )
    expressions = loaded.get("expressions")
    if isinstance(expressions, dict):
        expressions = expressions.get("items", [])
    if isinstance(expressions, list):
        for expression in expressions:
            append_expression_entity(entities, seen, expression, "expressions")
    catalogs = loaded.get("catalogs")
    if isinstance(catalogs, list):
        for ordinal, catalog in enumerate(catalogs):
            if not isinstance(catalog, dict):
                continue
            name = str(catalog.get("layer", catalog.get("name", ordinal)))
            value = {
                "resource_kind": "catalog_audit",
                "name": name,
                "catalog": catalog,
                "affects_stages": [
                    "morphology",
                    "grammar_candidate",
                    "grammar_occurrence",
                    "grammar_projection",
                    "grammar_residual",
                ],
            }
            append_entity(
                entities,
                seen,
                snapshot_entity(
                    "resource",
                    "catalog_audit",
                    value,
                    "catalogs",
                    key_parts=(name,),
                    ranges=(),
                ),
            )
    ui_projection = loaded.get("ui_projection")
    if ui_projection is not None:
        append_ui_projection_entities(entities, seen, ui_projection)

    for stage in entities:
        entities[stage].sort(
            key=lambda entity: (
                span_of_ranges(
                    tuple(tuple(item) for item in entity.get("ranges", []))
                )
                or (-1, -1),
                entity["kind"],
                entity["key"],
                canonical_json(entity["value"]),
            )
        )
    return manifest, entities, covered


def append_expression_entity(
    entities: dict[str, list[dict[str, Any]]],
    seen: set[tuple[str, str, str]],
    expression: dict[str, Any],
    artifact: str,
) -> None:
    value = dict(expression)
    value.setdefault("status", "accepted")
    stage = "expression" if value["status"] == "accepted" else "expression_candidate"
    label = value.get("label") or value.get("rule_id")
    append_entity(
        entities,
        seen,
        snapshot_entity(
            stage,
            "expression",
            value,
            artifact,
            key_parts=(value.get("origin"), value.get("rule_id"), label),
            anchor_parts=(label, value.get("surface")),
        ),
    )


def append_ui_projection_entities(
    entities: dict[str, list[dict[str, Any]]],
    seen: set[tuple[str, str, str]],
    report: Any,
) -> None:
    if isinstance(report, dict):
        items = report.get("items")
        if not isinstance(items, list):
            raise ValueError("ui_projection 产物必须包含 items 数组")
    elif isinstance(report, list):
        items = report
    else:
        raise ValueError("ui_projection 产物必须是数组或包含 items 的对象")
    for ordinal, item in enumerate(items):
        if not isinstance(item, dict):
            raise ValueError(f"ui_projection.items[{ordinal}] 必须是对象")
        projection_id = next(
            (
                item.get(field)
                for field in (
                    "projection_id",
                    "projectionId",
                    "target_id",
                    "targetId",
                    "occurrence_id",
                    "occurrenceId",
                    "match_id",
                    "matchId",
                    "token_id",
                    "tokenId",
                    "id",
                )
                if item.get(field) not in (None, "")
            ),
            ordinal,
        )
        kind = str(item.get("kind", item.get("type", "projection")))
        append_entity(
            entities,
            seen,
            snapshot_entity(
                "ui_projection",
                kind,
                item,
                "ui_projection",
                key_parts=(projection_id,),
                anchor_parts=(projection_id, kind),
            ),
        )


def indexed_entities(values: Sequence[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for value in values:
        groups[value["key"]].append(value)
    result: dict[str, dict[str, Any]] = {}
    for key, group in groups.items():
        for ordinal, value in enumerate(sorted(group, key=lambda item: canonical_json(item["value"]))):
            result[f"{key}#{ordinal}"] = value
    return result


def pair_unmatched_by_anchor(
    before: list[dict[str, Any]], after: list[dict[str, Any]]
) -> tuple[
    list[tuple[dict[str, Any], dict[str, Any]]],
    list[dict[str, Any]],
    list[dict[str, Any]],
]:
    after_groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for entity in after:
        after_groups[entity["anchor"]].append(entity)
    pairs: list[tuple[dict[str, Any], dict[str, Any]]] = []
    remaining_before: list[dict[str, Any]] = []
    used_after: set[int] = set()
    for before_entity in before:
        candidates = [
            candidate
            for candidate in after_groups.get(before_entity["anchor"], [])
            if id(candidate) not in used_after
        ]
        if not candidates:
            remaining_before.append(before_entity)
            continue
        candidate = min(
            candidates,
            key=lambda item: len(field_changes(before_entity["value"], item["value"])),
        )
        used_after.add(id(candidate))
        pairs.append((before_entity, candidate))
    remaining_after = [entity for entity in after if id(entity) not in used_after]
    return pairs, remaining_before, remaining_after


EVIDENCE_PATH_PARTS = {
    "evidence",
    "counter_evidence",
    "confidence",
    "score",
    "alternative_score",
    "alternatives",
    "hard_constraint",
    "analyzer_version",
    "catalog_version",
    "dictionary_refs",
    "reading_candidates",
}
IDENTITY_PATH_PARTS = {
    "match_id",
    "candidate_id",
    "occurrence_id",
    "chain_id",
    "operator_id",
    "rule_id",
    "token_range",
    "covered_token_range",
    "morpheme_range",
    "source_morpheme_range",
    "analyzer_version",
    "catalog_version",
    "knowledge_item_id",
}
DECISION_PATH_PARTS = {
    "status",
    "decision",
    "rejection_reason",
    "surface",
    "base_form",
    "reading",
    "char_range",
    "matched_ranges",
    "display_ranges",
    "selected_sense_id",
    "concept_id",
    "head_word",
}


def field_path_parts(path: str) -> set[str]:
    normalized = path.replace("[", "/").replace("]", "").replace("~1", "/")
    return {part for part in normalized.split("/") if part}


def change_scope(differences: Sequence[dict[str, Any]]) -> str:
    parts = set().union(*(field_path_parts(item["path"]) for item in differences))
    if parts and parts <= EVIDENCE_PATH_PARTS:
        return "evidence"
    if parts and parts <= EVIDENCE_PATH_PARTS | IDENTITY_PATH_PARTS:
        return "identity"
    if parts & {"char_range", "matched_ranges", "display_ranges", "anchor_range"}:
        return "range"
    if parts & DECISION_PATH_PARTS:
        return "decision"
    return "content"


def stage_change_severity(stage: str, operation: str, scope: str) -> str:
    if stage in {"source", "preprocessing"}:
        return "critical"
    if scope == "evidence":
        return "info"
    if scope == "identity":
        return "medium"
    if stage in {
        "morpheme",
        "lexical_unit",
        "bunsetsu_boundary",
        "bunsetsu",
        "grammar_occurrence",
        "expression",
    } and (operation == "removed" or scope in {"decision", "range"}):
        return "high"
    return "medium"


def combined_ranges(
    before: dict[str, Any] | None, after: dict[str, Any] | None
) -> list[list[int]]:
    values = {
        tuple(item)
        for entity in (before, after)
        if entity is not None
        for item in entity.get("ranges", [])
    }
    return [list(item) for item in sorted(values)]


def entity_change(
    stage: str,
    operation: str,
    before: dict[str, Any] | None,
    after: dict[str, Any] | None,
) -> dict[str, Any] | None:
    before_value = before["value"] if before else None
    after_value = after["value"] if after else None
    differences = (
        field_changes(before_value, after_value)
        if before is not None and after is not None
        else []
    )
    if before is not None and after is not None and not differences:
        return None
    scope = change_scope(differences) if differences else "decision"
    kind = (after or before or {})["kind"]
    change = {
        "stage": stage,
        "channel": stage,
        "type": f"{kind}.{operation}",
        "operation": operation,
        "scope": scope,
        "severity": stage_change_severity(stage, operation, scope),
        "entity_kind": kind,
        "entity_key_before": before.get("key") if before else None,
        "entity_key_after": after.get("key") if after else None,
        "anchor": (after or before or {}).get("anchor", ""),
        "ranges": combined_ranges(before, after),
        "context": (after or before or {}).get("context", ""),
        "source_artifacts": sorted(
            {
                entity["artifact"]
                for entity in (before, after)
                if entity is not None
            }
        ),
        "before": value_descriptor(before_value),
        "after": value_descriptor(after_value),
    }
    if isinstance(before_value, dict) and "status" in before_value:
        change["status_before"] = str(before_value["status"])
    if isinstance(after_value, dict) and "status" in after_value:
        change["status_after"] = str(after_value["status"])
    if differences:
        change["field_changes"] = differences
    if stage == "resource":
        affects = set()
        for value in (before_value, after_value):
            if isinstance(value, dict):
                affects.update(value.get("affects_stages", []))
        change["affects_stages"] = sorted(affects, key=lambda item: STAGE_INDEX.get(item, 999))
    return with_change_id(change)


def compare_entity_stage(
    stage: str,
    before_values: Sequence[dict[str, Any]],
    after_values: Sequence[dict[str, Any]],
) -> list[dict[str, Any]]:
    before_index = indexed_entities(before_values)
    after_index = indexed_entities(after_values)
    changes: list[dict[str, Any]] = []
    matched_keys = set(before_index) & set(after_index)
    for key in sorted(matched_keys):
        change = entity_change(stage, "modified", before_index[key], after_index[key])
        if change:
            changes.append(change)
    before_unmatched = [before_index[key] for key in sorted(set(before_index) - matched_keys)]
    after_unmatched = [after_index[key] for key in sorted(set(after_index) - matched_keys)]
    pairs, before_unmatched, after_unmatched = pair_unmatched_by_anchor(
        before_unmatched, after_unmatched
    )
    for before, after in pairs:
        change = entity_change(stage, "modified", before, after)
        if change:
            changes.append(change)
    changes.extend(
        change
        for change in (
            entity_change(stage, "removed", entity, None) for entity in before_unmatched
        )
        if change is not None
    )
    changes.extend(
        change
        for change in (
            entity_change(stage, "added", None, entity) for entity in after_unmatched
        )
        if change is not None
    )
    return changes


def transitive_dependencies(stage: str) -> set[str]:
    result: set[str] = set()
    pending = list(STAGE_DEPENDENCIES.get(stage, ()))
    while pending:
        dependency = pending.pop()
        if dependency in result:
            continue
        result.add(dependency)
        pending.extend(STAGE_DEPENDENCIES.get(dependency, ()))
    return result


def stage_reaches(source: str, target: str) -> bool:
    return source == target or source in transitive_dependencies(target)


def change_ranges(change: dict[str, Any]) -> tuple[tuple[int, int], ...]:
    return tuple(
        item
        for item in (normalized_range(raw) for raw in change.get("ranges", []))
        if item is not None
    )


def resource_can_affect(resource_change: dict[str, Any], stage: str) -> bool:
    return any(
        stage_reaches(affected, stage)
        for affected in resource_change.get("affects_stages", [])
    )


def annotate_causality(changes: list[dict[str, Any]]) -> None:
    by_stage: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for change in changes:
        by_stage[change["stage"]].append(change)
    ordered = sorted(
        changes,
        key=lambda change: (STAGE_INDEX.get(change["stage"], 999), change["change_id"]),
    )
    for change in ordered:
        stage = change["stage"]
        dependencies = transitive_dependencies(stage)
        current_ranges = change_ranges(change)
        candidates: list[dict[str, Any]] = []
        for dependency in dependencies:
            for upstream in by_stage.get(dependency, []):
                if dependency == "resource" and not resource_can_affect(upstream, stage):
                    continue
                upstream_ranges = change_ranges(upstream)
                if dependency == "resource" or (
                    current_ranges
                    and upstream_ranges
                    and ranges_intersect(current_ranges, upstream_ranges)
                ):
                    candidates.append(upstream)
        if not candidates:
            change["causal_status"] = "root"
            change["cause_change_ids"] = []
            change["causal_basis"] = []
            continue
        closest_index = max(STAGE_INDEX.get(item["stage"], -1) for item in candidates)
        closest = [
            item
            for item in candidates
            if STAGE_INDEX.get(item["stage"], -1) == closest_index
        ]
        change["causal_status"] = "propagated_candidate"
        change["cause_change_ids"] = [item["change_id"] for item in closest[:20]]
        change["causal_basis"] = ["declared_dependency", "range_overlap"]
        change["causal_confidence"] = "candidate"


def root_impact_summary(changes: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    by_id = {change["change_id"]: change for change in changes}
    descendants: dict[str, set[str]] = defaultdict(set)

    def roots(change: dict[str, Any], visiting: set[str]) -> set[str]:
        change_id = change["change_id"]
        if change_id in visiting:
            return set()
        causes = change.get("cause_change_ids", [])
        if not causes:
            return {change_id}
        result: set[str] = set()
        for cause_id in causes:
            cause = by_id.get(cause_id)
            if cause is not None:
                result.update(roots(cause, visiting | {change_id}))
        return result or {change_id}

    for change in changes:
        for root in roots(change, set()):
            if root != change["change_id"]:
                descendants[root].add(change["change_id"])
    result = []
    for change in changes:
        if change.get("causal_status") != "root":
            continue
        impacted = descendants.get(change["change_id"], set())
        result.append(
            {
                "change_id": change["change_id"],
                "stage": change["stage"],
                "type": change["type"],
                "context": change.get("context", ""),
                "affected_changes": len(impacted),
                "affected_stages": sorted(
                    {by_id[item]["stage"] for item in impacted},
                    key=lambda stage: STAGE_INDEX.get(stage, 999),
                ),
            }
        )
    return sorted(
        result,
        key=lambda item: (-item["affected_changes"], STAGE_INDEX.get(item["stage"], 999), item["change_id"]),
    )


def artifact_contract_mismatches(
    before_manifest: dict[str, Any], after_manifest: dict[str, Any]
) -> list[dict[str, Any]]:
    before_artifacts = before_manifest.get("artifacts", {})
    after_artifacts = after_manifest.get("artifacts", {})
    result = []
    for name in sorted(set(before_artifacts) & set(after_artifacts)):
        before_contract = {
            "adapter": before_artifacts[name].get("adapter"),
            "capture": before_artifacts[name].get("capture", {}),
        }
        after_contract = {
            "adapter": after_artifacts[name].get("adapter"),
            "capture": after_artifacts[name].get("capture", {}),
        }
        if before_contract != after_contract:
            result.append(
                {
                    "artifact": name,
                    "before": before_contract,
                    "after": after_contract,
                }
            )
    return result


def wilson_interval(successes: int, total: int) -> list[float]:
    if total <= 0:
        return [0.0, 0.0]
    z = 1.959963984540054
    proportion = successes / total
    denominator = 1 + z * z / total
    center = (proportion + z * z / (2 * total)) / denominator
    margin = (
        z
        * math.sqrt(
            proportion * (1 - proportion) / total + z * z / (4 * total * total)
        )
        / denominator
    )
    return [round(max(0.0, center - margin), 8), round(min(1.0, center + margin), 8)]


def stage_change_statistics(
    before_count: int,
    after_count: int,
    changes: Sequence[dict[str, Any]],
) -> dict[str, Any]:
    operations = Counter(change["operation"] for change in changes)
    modified = operations.get("modified", 0)
    added = operations.get("added", 0)
    removed = operations.get("removed", 0)
    stable_before = max(0, before_count - modified - removed)
    stable_after = max(0, after_count - modified - added)
    stable = min(stable_before, stable_after)
    changed = modified + added + removed
    units = stable + changed
    transitions = Counter(
        (
            str(change.get("status_before", "absent")),
            str(change.get("status_after", "absent")),
        )
        for change in changes
        if "status_before" in change or "status_after" in change
    )
    scopes = Counter(change["scope"] for change in changes)
    return {
        "operations": {
            "stable": stable,
            "modified": modified,
            "added": added,
            "removed": removed,
        },
        "alignment_balance": {
            "stable_before": stable_before,
            "stable_after": stable_after,
            "balanced": stable_before == stable_after,
        },
        "churn": {
            "changed_units": changed,
            "comparison_units": units,
            "rate": round(changed / units, 8) if units else 0.0,
            "ci95": wilson_interval(changed, units),
            "interval_method": "wilson_entity_units",
        },
        "scope_rates": {
            scope: round(count / changed, 8) if changed else 0.0
            for scope, count in sorted(scopes.items())
        },
        "status_transitions": [
            {"before": before, "after": after, "count": count}
            for (before, after), count in sorted(transitions.items())
        ],
    }


def pipeline_summary(
    before_entities: dict[str, list[dict[str, Any]]],
    after_entities: dict[str, list[dict[str, Any]]],
    before_covered: set[str],
    after_covered: set[str],
    changes: list[dict[str, Any]],
    contract_mismatches: list[dict[str, Any]],
    blocked_stages: set[str],
) -> dict[str, Any]:
    stage_rows = []
    for stage in STAGE_ORDER:
        before_present = stage in before_covered
        after_present = stage in after_covered
        if stage in blocked_stages:
            coverage_status = "contract_mismatch"
        elif before_present and after_present:
            coverage_status = "comparable"
        elif before_present:
            coverage_status = "before_only"
        elif after_present:
            coverage_status = "after_only"
        else:
            coverage_status = "missing"
        stage_changes = [change for change in changes if change["stage"] == stage]
        before_count = len(before_entities.get(stage, []))
        after_count = len(after_entities.get(stage, []))
        statistics = stage_change_statistics(before_count, after_count, stage_changes)
        stage_rows.append(
            {
                "stage": stage,
                "coverage": coverage_status,
                "before_entities": before_count,
                "after_entities": after_count,
                "changes": len(stage_changes),
                "root_changes": sum(
                    change.get("causal_status") == "root" for change in stage_changes
                ),
                "propagated_candidates": sum(
                    change.get("causal_status") == "propagated_candidate"
                    for change in stage_changes
                ),
                "scopes": dict(
                    sorted(Counter(change["scope"] for change in stage_changes).items())
                ),
                **statistics,
            }
        )
    type_counts = Counter(change["type"] for change in changes)
    severity_counts = Counter(change["severity"] for change in changes)
    causal_counts = Counter(change.get("causal_status", "unknown") for change in changes)
    source_changed = any(change["stage"] == "source" for change in changes)
    comparable_stages = sum(row["coverage"] == "comparable" for row in stage_rows)
    missing_stages = [row["stage"] for row in stage_rows if row["coverage"] == "missing"]
    noncomparable_stages = [
        row["stage"]
        for row in stage_rows
        if row["coverage"] in {"before_only", "after_only", "contract_mismatch"}
    ]
    comparison_units = sum(row["churn"]["comparison_units"] for row in stage_rows)
    changed_units = sum(row["churn"]["changed_units"] for row in stage_rows)
    scope_counts = Counter(change["scope"] for change in changes)
    transitions = Counter(
        (
            str(change.get("status_before", "absent")),
            str(change.get("status_after", "absent")),
        )
        for change in changes
        if "status_before" in change or "status_after" in change
    )
    root_impacts = root_impact_summary(changes)
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "unchanged" if not changes else "changed",
        "comparable": not source_changed and comparable_stages > 0,
        "complete_stage_coverage": not missing_stages and not noncomparable_stages,
        "quality_conclusion": (
            "paused_input_changed"
            if source_changed
            else "partial"
            if missing_stages or noncomparable_stages or contract_mismatches
            else "eligible"
        ),
        "changes": len(changes),
        "root_changes": causal_counts.get("root", 0),
        "propagated_candidates": causal_counts.get("propagated_candidate", 0),
        "change_types": dict(sorted(type_counts.items())),
        "severities": {
            key: severity_counts.get(key, 0)
            for key in sorted(SEVERITY_ORDER, key=SEVERITY_ORDER.get)
        },
        "causal_statuses": dict(sorted(causal_counts.items())),
        "churn": {
            "changed_units": changed_units,
            "comparison_units": comparison_units,
            "rate": round(changed_units / comparison_units, 8)
            if comparison_units
            else 0.0,
            "ci95": wilson_interval(changed_units, comparison_units),
            "interval_method": "wilson_entity_units",
        },
        "scope_counts": dict(sorted(scope_counts.items())),
        "scope_rates": {
            scope: round(count / len(changes), 8) if changes else 0.0
            for scope, count in sorted(scope_counts.items())
        },
        "status_transitions": [
            {"before": before, "after": after, "count": count}
            for (before, after), count in sorted(transitions.items())
        ],
        "stages": stage_rows,
        "missing_stages": missing_stages,
        "noncomparable_stages": noncomparable_stages,
        "contract_mismatches": contract_mismatches,
        "root_impacts_total": len(root_impacts),
        "root_impacts": root_impacts[:100],
    }


def compare_snapshot_manifests(
    before_path: Path, after_path: Path
) -> ComparisonBundle:
    before_manifest, before_entities, before_covered = normalize_snapshot(before_path)
    after_manifest, after_entities, after_covered = normalize_snapshot(after_path)
    mismatches = artifact_contract_mismatches(before_manifest, after_manifest)
    blocked_stages = {
        stage
        for mismatch in mismatches
        for stage in ARTIFACT_STAGE_COVERAGE.get(mismatch["artifact"], set())
    }
    changes: list[dict[str, Any]] = []
    for stage in STAGE_ORDER:
        if (
            stage in blocked_stages
            or stage not in before_covered
            or stage not in after_covered
        ):
            continue
        changes.extend(
            compare_entity_stage(
                stage,
                before_entities.get(stage, []),
                after_entities.get(stage, []),
            )
        )
    annotate_causality(changes)
    changes.sort(
        key=lambda change: (
            STAGE_INDEX.get(change["stage"], 999),
            SEVERITY_ORDER.get(change["severity"], 99),
            span_of_ranges(change_ranges(change)) or (-1, -1),
            change["type"],
            change["change_id"],
        )
    )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "producer": "scripts/language_quality_diff.py",
        "producer_version": PRODUCER_VERSION,
        "adapter": "pipeline",
        "before": {
            **file_descriptor(before_path),
            "run_id": before_manifest.get("run_id"),
            "label": before_manifest.get("label"),
        },
        "after": {
            **file_descriptor(after_path),
            "run_id": after_manifest.get("run_id"),
            "label": after_manifest.get("label"),
        },
        "stage_graph": [
            {"stage": stage, "depends_on": list(STAGE_DEPENDENCIES.get(stage, ()))}
            for stage in STAGE_ORDER
        ],
    }
    return ComparisonBundle(
        manifest=manifest,
        summary=pipeline_summary(
            before_entities,
            after_entities,
            before_covered,
            after_covered,
            changes,
            mismatches,
            blocked_stages,
        ),
        changes=changes,
    )


def summarize_changes(metrics: dict[str, Any], changes: list[dict[str, Any]]) -> dict[str, Any]:
    type_counts = Counter(change["type"] for change in changes)
    severity_counts = Counter(change["severity"] for change in changes)
    paths = Counter(
        field["path"]
        for change in changes
        for field in change.get("field_changes", [])
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "unchanged" if not changes else "changed",
        "comparable": not any(change["type"].startswith("input_") for change in changes),
        "changes": len(changes),
        "change_types": dict(sorted(type_counts.items())),
        "severities": {
            key: severity_counts.get(key, 0)
            for key in sorted(SEVERITY_ORDER, key=SEVERITY_ORDER.get)
        },
        "field_paths": [
            {"path": path, "count": count}
            for path, count in sorted(paths.items(), key=lambda item: (-item[1], item[0]))
        ],
        "metrics": metrics,
    }


def compare_files(before_path: Path, after_path: Path, adapter: str) -> ComparisonBundle:
    before = read_json(before_path)
    after = read_json(after_path)
    if adapter == "bunsetsu":
        metrics, changes = compare_bunsetsu(before, after)
    elif adapter == "expression":
        metrics, changes = compare_expression(before, after)
    else:
        raise ValueError(f"未知适配器：{adapter}")
    changes.sort(
        key=lambda change: (
            SEVERITY_ORDER.get(change["severity"], 99),
            change["type"],
            change.get("before_line", -1) if change.get("before_line") is not None else -1,
            change.get("after_line", -1) if change.get("after_line") is not None else -1,
            change.get("anchor", ""),
            change["change_id"],
        )
    )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "producer": "scripts/language_quality_diff.py",
        "producer_version": PRODUCER_VERSION,
        "adapter": adapter,
        "before": file_descriptor(before_path),
        "after": file_descriptor(after_path),
    }
    return ComparisonBundle(
        manifest=manifest,
        summary=summarize_changes(metrics, changes),
        changes=changes,
    )


def html_report(bundle: ComparisonBundle) -> str:
    payload = {
        "schema_version": SCHEMA_VERSION,
        "adapter": bundle.manifest["adapter"],
        "sources": {
            "manifest": "manifest.json",
            "summary": "summary.json",
            "diff": "diff.jsonl",
            "root_impacts": "root-causes.json",
        },
        "detail_strategy": "full_external_jsonl",
        "total_changes": len(bundle.changes),
    }
    encoded = canonical_json(payload).replace("</", "<\\/")
    title = html.escape(f"Kotoclip 语言质量差分 - {bundle.manifest['adapter']}")
    return f"""<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <style>
    :root {{ color-scheme: light; font-family: Inter, "Segoe UI", "Microsoft YaHei", sans-serif; color: #202426; background: #f4f6f7; }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; min-width: 320px; }}
    header {{ padding: 24px clamp(18px, 4vw, 48px); color: #f8fafb; background: #202a2f; border-bottom: 4px solid #27a376; }}
    h1 {{ margin: 0; font-size: 1.45rem; letter-spacing: 0; }}
    header p {{ margin: 8px 0 0; color: #cbd5d9; overflow-wrap: anywhere; }}
    main {{ width: min(1500px, 100%); margin: 0 auto; padding: 22px clamp(14px, 3vw, 36px) 48px; }}
    section {{ margin: 0 0 22px; }}
    h2 {{ margin: 0 0 12px; font-size: 1rem; letter-spacing: 0; }}
    .status-line {{ display: flex; flex-wrap: wrap; gap: 10px 18px; align-items: center; margin-bottom: 18px; }}
    .status {{ padding: 4px 8px; border-radius: 4px; font-weight: 700; background: #fff0d8; color: #70420a; }}
    .status.unchanged {{ background: #dff5eb; color: #155d44; }}
    .muted {{ color: #657178; }}
    .metrics {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 1px; border: 1px solid #d6dcdf; background: #d6dcdf; }}
    .metric {{ min-width: 0; padding: 13px 14px; background: #fff; }}
    .metric span {{ display: block; color: #667279; font-size: .78rem; overflow-wrap: anywhere; }}
    .metric strong {{ display: block; margin-top: 5px; font-size: 1.2rem; font-variant-numeric: tabular-nums; }}
    .split {{ display: grid; grid-template-columns: minmax(260px, .8fr) minmax(0, 2fr); gap: 22px; }}
    .panel {{ border: 1px solid #d6dcdf; border-radius: 6px; background: #fff; }}
    .bars {{ padding: 14px; }}
    .bar-row {{ display: grid; grid-template-columns: minmax(110px, 1fr) minmax(80px, 3fr) 50px; gap: 10px; align-items: center; margin: 8px 0; font-size: .8rem; }}
    .bar-row label {{ overflow-wrap: anywhere; }}
    .track {{ height: 10px; background: #e8ecee; }}
    .fill {{ height: 100%; background: #2887a8; }}
    .bar-row output {{ text-align: right; font-variant-numeric: tabular-nums; }}
    .controls {{ display: grid; grid-template-columns: minmax(180px, 1fr) repeat(3, minmax(120px, 190px)) minmax(120px, 150px) minmax(90px, 120px) auto auto auto; gap: 10px; padding: 12px; border-bottom: 1px solid #dfe4e6; }}
    .impact-controls {{ display: flex; flex-wrap: wrap; gap: 10px; align-items: center; padding: 12px; border-bottom: 1px solid #dfe4e6; }}
    .impact-controls input {{ flex: 1 1 240px; }}
    .pager-info {{ min-width: 130px; color: #657178; font-size: .8rem; text-align: center; font-variant-numeric: tabular-nums; }}
    input, select, button {{ min-width: 0; min-height: 36px; border: 1px solid #b9c3c8; border-radius: 4px; background: #fff; color: #202426; font: inherit; }}
    input, select {{ padding: 7px 9px; }}
    button {{ padding: 7px 12px; cursor: pointer; background: #26373f; color: #fff; border-color: #26373f; }}
    .table-wrap {{ overflow: auto; max-height: 68vh; }}
    table {{ width: 100%; border-collapse: collapse; table-layout: fixed; font-size: .8rem; }}
    th, td {{ padding: 9px 10px; border-bottom: 1px solid #e2e6e8; text-align: left; vertical-align: top; overflow-wrap: anywhere; }}
    th {{ position: sticky; top: 0; z-index: 1; background: #eef2f3; color: #445158; }}
    .detail-table th:nth-child(1), .detail-table td:nth-child(1) {{ width: 120px; }}
    .detail-table th:nth-child(2), .detail-table td:nth-child(2) {{ width: 82px; }}
    .detail-table th:nth-child(3), .detail-table td:nth-child(3) {{ width: 210px; }}
    .detail-table th:nth-child(4), .detail-table td:nth-child(4) {{ width: 120px; }}
    .detail-table th:nth-child(5), .detail-table td:nth-child(5) {{ width: 250px; }}
    .detail-table td:nth-child(6) {{ white-space: pre-wrap; }}
    .sev-critical {{ color: #9e2020; font-weight: 700; }}
    .sev-high {{ color: #a64c13; font-weight: 700; }}
    .sev-medium {{ color: #17617a; font-weight: 700; }}
    .coverage-comparable {{ color: #155d44; }}
    .coverage-missing, .coverage-before_only, .coverage-after_only, .coverage-contract_mismatch {{ color: #9e2020; }}
    .causal-root {{ color: #7d3311; font-weight: 700; }}
    .causal-propagated_candidate {{ color: #17617a; }}
    code {{ font-family: "Cascadia Mono", Consolas, monospace; font-size: .76rem; }}
    .empty {{ padding: 30px; text-align: center; color: #667279; }}
    .footnote {{ margin: 10px 0 0; font-size: .78rem; color: #667279; }}
    @media (max-width: 760px) {{ .split {{ grid-template-columns: 1fr; }} .controls {{ grid-template-columns: 1fr; }} .pager-info {{ text-align: left; }} table {{ min-width: 900px; }} }}
  </style>
</head>
<body>
  <header>
    <h1>Kotoclip 语言质量差分</h1>
    <p id="run-meta"></p>
  </header>
  <main>
    <section>
      <div class="status-line"><span id="status" class="status"></span><span id="comparable" class="muted"></span></div>
      <div id="metrics" class="metrics"></div>
    </section>
    <section id="stage-section" hidden>
      <h2>管线层级</h2>
      <div class="panel table-wrap"><table><thead><tr><th>阶段</th><th>覆盖</th><th>基准实体</th><th>候选实体</th><th>变化</th><th>变化率（95% CI）</th><th>根变化</th><th>传播候选</th></tr></thead><tbody id="stage-rows"></tbody></table></div>
    </section>
    <section class="split">
      <div><h2>变化类型</h2><div id="type-bars" class="panel bars"></div></div>
      <div><h2>高频字段路径</h2><div id="field-bars" class="panel bars"></div></div>
    </section>
    <section id="transition-section" hidden>
      <h2>候选状态转移</h2>
      <div id="transition-bars" class="panel bars"></div>
    </section>
    <section id="impact-section" hidden>
      <h2>根变化影响</h2>
      <div class="panel table-wrap"><table><thead><tr><th>阶段</th><th>变化</th><th>根变化 ID</th><th>下游变化</th><th>受影响阶段</th><th>上下文</th></tr></thead><tbody id="impact-rows"></tbody></table></div>
      <div class="panel impact-controls">
        <input id="impact-search" type="search" placeholder="筛选根变化、阶段、类型或上下文">
        <button id="impact-prev" type="button" title="上一页" aria-label="根变化影响上一页">←</button>
        <output id="impact-page-info" class="pager-info"></output>
        <button id="impact-next" type="button" title="下一页" aria-label="根变化影响下一页">→</button>
      </div>
    </section>
    <section>
      <h2>变化明细</h2>
      <div class="panel">
        <div class="controls">
          <input id="search" type="search" placeholder="筛选上下文、类型、字段或锚点">
          <select id="stage-filter"><option value="">全部阶段</option></select>
          <select id="type-filter"><option value="">全部变化类型</option></select>
          <select id="causal-filter"><option value="">全部归因</option><option value="root">根变化</option><option value="propagated_candidate">传播候选</option></select>
          <input id="coordinate-filter" type="number" min="0" step="1" inputmode="numeric" placeholder="字符坐标" title="只显示覆盖该字符坐标的变化">
          <select id="page-size" title="每页渲染的变化数"><option value="100">每页 100</option><option value="250">每页 250</option><option value="500">每页 500</option><option value="0">全部</option></select>
          <button id="prev-page" type="button" title="上一页" aria-label="变化明细上一页">←</button>
          <output id="page-info" class="pager-info"></output>
          <button id="next-page" type="button" title="下一页" aria-label="变化明细下一页">→</button>
          <button id="reset" type="button">重置</button>
        </div>
        <div class="table-wrap"><table class="detail-table"><thead><tr><th>阶段</th><th>级别</th><th>类型</th><th>归因</th><th>位置 / 锚点</th><th>变化</th><th>上下文</th></tr></thead><tbody id="rows"></tbody></table><div id="empty" class="empty" hidden>当前筛选没有结果</div></div>
      </div>
      <p id="detail-note" class="footnote"></p>
    </section>
  </main>
  <script id="report-config" type="application/json">{encoded}</script>
  <script>
    const config = JSON.parse(document.getElementById('report-config').textContent);
    const status = document.getElementById('status');
    const comparableNode = document.getElementById('comparable');
    status.textContent = '正在读取报告数据…';
    async function loadReport() {{
      try {{
        const loadJson = async path => {{
          const response = await fetch(path, {{ cache: 'no-store' }});
          if (!response.ok) throw new Error(`${{path}} HTTP ${{response.status}}`);
          return response.json();
        }};
        const loadText = async path => {{
          const response = await fetch(path, {{ cache: 'no-store' }});
          if (!response.ok) throw new Error(`${{path}} HTTP ${{response.status}}`);
          return response.text();
        }};
        const [manifest, summary, diffText, rootImpacts] = await Promise.all([
          loadJson(config.sources.manifest),
          loadJson(config.sources.summary),
          loadText(config.sources.diff),
          loadJson(config.sources.root_impacts),
        ]);
        const changes = diffText.split(/\\r?\\n/).filter(Boolean).map((line, index) => {{
          try {{ return JSON.parse(line); }}
          catch (error) {{ throw new Error(`diff.jsonl 第 ${{index + 1}} 行无效: ${{error.message}}`); }}
        }});
        const report = {{ manifest, summary, changes, root_impacts: rootImpacts, ...config }};
        const metricRoot = document.getElementById('metrics');
        status.textContent = summary.status === 'unchanged' ? '无变化' : `发现 ${{summary.changes}} 项变化`;
        status.classList.toggle('unchanged', summary.status === 'unchanged');
        const conclusion = summary.quality_conclusion || (summary.comparable ? 'eligible' : 'paused_input_changed');
        const conclusionLabels = {{ eligible: '输入与阶段契约可比较', partial: '阶段覆盖不完整，仅可作局部结论', paused_input_changed: '输入文本变化，质量结论已暂停' }};
        comparableNode.textContent = conclusionLabels[conclusion] || conclusion;
        document.getElementById('run-meta').textContent = `${{report.manifest.adapter}} · ${{report.manifest.before.sha256.slice(0, 12)}} → ${{report.manifest.after.sha256.slice(0, 12)}} · ${{changes.length}} 条完整变化`;

    const formatPercent = value => `${{(Number(value || 0) * 100).toFixed(3)}}%`;
    const metrics = [
      ['总变化', summary.changes],
      ['根变化', summary.root_changes ?? summary.changes],
      ['传播候选', summary.propagated_candidates ?? 0],
      ['全层实体变化率', summary.churn ? formatPercent(summary.churn.rate) : '单产物'],
      ['严重', summary.severities.critical],
      ['高', summary.severities.high],
      ['中等', summary.severities.medium],
      ['阶段覆盖', summary.stages ? `${{summary.stages.filter(item => item.coverage === 'comparable').length}} / ${{summary.stages.length}}` : '单产物'],
    ];
    for (const [label, value] of metrics) {{
      const node = document.createElement('div');
      node.className = 'metric';
      const caption = document.createElement('span'); caption.textContent = label;
      const strong = document.createElement('strong'); strong.textContent = value;
      node.append(caption, strong); metricRoot.append(node);
    }}

    function renderBars(rootId, entries) {{
      const root = document.getElementById(rootId);
      const max = Math.max(1, ...entries.map(([, count]) => count));
      if (!entries.length) {{ root.textContent = '无'; return; }}
      for (const [label, count] of entries.slice(0, 12)) {{
        const row = document.createElement('div'); row.className = 'bar-row';
        const name = document.createElement('label'); name.textContent = label;
        const track = document.createElement('div'); track.className = 'track';
        const fill = document.createElement('div'); fill.className = 'fill'; fill.style.width = `${{count / max * 100}}%`; track.append(fill);
        const value = document.createElement('output'); value.textContent = count;
        row.append(name, track, value); root.append(row);
      }}
    }}
    renderBars('type-bars', Object.entries(summary.change_types).sort((a, b) => b[1] - a[1]));
    const fieldCounts = new Map();
    for (const change of changes) for (const field of change.field_changes || []) fieldCounts.set(field.path, (fieldCounts.get(field.path) || 0) + 1);
    const fieldEntries = summary.field_paths
      ? summary.field_paths.map(item => [item.path, item.count])
      : [...fieldCounts.entries()].sort((a, b) => b[1] - a[1]);
    renderBars('field-bars', fieldEntries);
    if (summary.status_transitions?.length) {{
      document.getElementById('transition-section').hidden = false;
      renderBars('transition-bars', summary.status_transitions.map(item => [`${{item.before}} → ${{item.after}}`, item.count]).sort((a, b) => b[1] - a[1]));
    }}

    if (summary.stages) {{
      document.getElementById('stage-section').hidden = false;
      const stageRows = document.getElementById('stage-rows');
      for (const item of summary.stages) {{
        const tr = document.createElement('tr');
        const churn = item.churn ? `${{formatPercent(item.churn.rate)}} (${{formatPercent(item.churn.ci95[0])}}–${{formatPercent(item.churn.ci95[1])}})` : '';
        for (const [value, className] of [[item.stage, ''], [item.coverage, `coverage-${{item.coverage}}`], [item.before_entities, ''], [item.after_entities, ''], [item.changes, ''], [churn, ''], [item.root_changes, ''], [item.propagated_candidates, '']]) {{
          const td = document.createElement('td'); td.textContent = value; td.className = className; tr.append(td);
        }}
        stageRows.append(tr);
      }}
    }}
    if (summary.root_impacts?.length) {{
      document.getElementById('impact-section').hidden = false;
      document.getElementById('impact-section').dataset.available = String(report.root_impacts?.length || 0);
    }}

    const stageFilter = document.getElementById('stage-filter');
    for (const stage of [...new Set(changes.map(change => change.stage || change.channel).filter(Boolean))]) {{
      const option = document.createElement('option'); option.value = stage; option.textContent = stage; stageFilter.append(option);
    }}
    const typeFilter = document.getElementById('type-filter');
    for (const type of [...new Set(changes.map(change => change.type))].sort()) {{
      const option = document.createElement('option'); option.value = type; option.textContent = type; typeFilter.append(option);
    }}
    const search = document.getElementById('search');
    const coordinateFilter = document.getElementById('coordinate-filter');
    const causalFilter = document.getElementById('causal-filter');
    const pageSize = document.getElementById('page-size');
    const pageInfo = document.getElementById('page-info');
    const prevPage = document.getElementById('prev-page');
    const nextPage = document.getElementById('next-page');
    const rows = document.getElementById('rows');
    const empty = document.getElementById('empty');
    let pageIndex = 0;
    let impactPageIndex = 0;
    function compact(value) {{
      if (value === undefined) return '';
      const text = typeof value === 'string' ? value : JSON.stringify(value);
      return text.length > 420 ? `${{text.slice(0, 417)}}...` : text;
    }}
    function changeText(change) {{
      if (change.field_changes) return change.field_changes.map(field => `${{field.path}}: ${{compact(field.before)}} → ${{compact(field.after)}}`).join('\\n');
      if ('before' in change || 'after' in change) return `${{compact(change.before)}} → ${{compact(change.after)}}`;
      return '';
    }}
    function locationText(change) {{
      const lines = [change.before_line, change.after_line].filter(value => value !== undefined && value !== null);
      const ranges = Array.isArray(change.ranges)
        ? change.ranges.map(range => Array.isArray(range) && range.length === 2 ? `[${{range[0]}},${{range[1]}})` : '').filter(Boolean).join(', ')
        : '';
      return [ranges ? `char ${{ranges}}` : '', lines.length ? `line ${{[...new Set(lines)].join(' / ')}}` : '', change.anchor || ''].filter(Boolean).join(' · ');
    }}
    function coordinateMatches(change, rawValue) {{
      if (!rawValue.trim()) return true;
      const position = Number(rawValue);
      if (!Number.isInteger(position) || position < 0) return false;
      return (change.ranges || []).some(range => {{
        if (!Array.isArray(range) || range.length !== 2) return false;
        const [start, end] = range;
        return start === end ? position === start : start <= position && position < end;
      }});
    }}
    function renderRows() {{
      const needle = search.value.trim().toLocaleLowerCase();
      const selectedStage = stageFilter.value;
      const selectedType = typeFilter.value;
      const selectedCausal = causalFilter.value;
      const rawCoordinate = coordinateFilter.value.trim();
      const filtered = changes.filter(change => {{
        if (selectedStage && (change.stage || change.channel) !== selectedStage) return false;
        if (selectedType && change.type !== selectedType) return false;
        if (selectedCausal && change.causal_status !== selectedCausal) return false;
        if (!coordinateMatches(change, rawCoordinate)) return false;
        if (!needle) return true;
        return JSON.stringify(change).toLocaleLowerCase().includes(needle);
      }});
      const size = Number(pageSize.value);
      const pageCount = size > 0 ? Math.max(1, Math.ceil(filtered.length / size)) : 1;
      pageIndex = Math.min(pageIndex, pageCount - 1);
      const start = size > 0 ? pageIndex * size : 0;
      const visible = size > 0 ? filtered.slice(start, start + size) : filtered;
      rows.replaceChildren();
      for (const change of visible) {{
        const tr = document.createElement('tr');
        const stage = document.createElement('td'); stage.textContent = change.stage || change.channel || '';
        const severity = document.createElement('td'); severity.className = `sev-${{change.severity}}`; severity.textContent = change.severity;
        const type = document.createElement('td'); const typeCode = document.createElement('code'); typeCode.textContent = change.type; type.append(typeCode);
        const causal = document.createElement('td'); causal.className = `causal-${{change.causal_status || ''}}`; causal.textContent = change.causal_status || '';
        const location = document.createElement('td'); const locationCode = document.createElement('code'); locationCode.textContent = locationText(change); location.append(locationCode);
        const detail = document.createElement('td'); detail.textContent = changeText(change);
        const context = document.createElement('td'); context.textContent = change.context || '';
        tr.append(stage, severity, type, causal, location, detail, context); rows.append(tr);
      }}
      empty.hidden = filtered.length !== 0;
      pageInfo.textContent = filtered.length ? `${{start + 1}}–${{Math.min(start + visible.length, filtered.length)}} / ${{filtered.length}}` : '0 / 0';
      prevPage.disabled = pageIndex <= 0;
      nextPage.disabled = pageIndex >= pageCount - 1;
    }}
    search.addEventListener('input', renderRows);
    stageFilter.addEventListener('change', renderRows);
    typeFilter.addEventListener('change', renderRows);
    causalFilter.addEventListener('change', renderRows);
    coordinateFilter.addEventListener('input', renderRows);
    pageSize.addEventListener('change', () => {{ pageIndex = 0; renderRows(); }});
    prevPage.addEventListener('click', () => {{ pageIndex = Math.max(0, pageIndex - 1); renderRows(); }});
    nextPage.addEventListener('click', () => {{ pageIndex += 1; renderRows(); }});
    document.getElementById('reset').addEventListener('click', () => {{ search.value = ''; coordinateFilter.value = ''; stageFilter.value = ''; typeFilter.value = ''; causalFilter.value = ''; pageIndex = 0; renderRows(); }});
    document.getElementById('detail-note').textContent =
      `已将全部 ${{report.total_changes}} 项变化载入面板；页面按坐标和筛选条件切片渲染，完整机器明细见 diff.jsonl。`;

    const impactSearch = document.getElementById('impact-search');
    const impactPageInfo = document.getElementById('impact-page-info');
    const impactRows = document.getElementById('impact-rows');
    const impactPrev = document.getElementById('impact-prev');
    const impactNext = document.getElementById('impact-next');
    function renderImpacts() {{
      const allImpacts = report.root_impacts || [];
      const needle = impactSearch.value.trim().toLocaleLowerCase();
      const filtered = allImpacts.filter(item => JSON.stringify(item).toLocaleLowerCase().includes(needle));
      const size = 100;
      const pageCount = Math.max(1, Math.ceil(filtered.length / size));
      impactPageIndex = Math.min(impactPageIndex, pageCount - 1);
      const start = impactPageIndex * size;
      impactRows.replaceChildren();
      for (const item of filtered.slice(start, start + size)) {{
        const tr = document.createElement('tr');
        for (const value of [item.stage, item.type, item.change_id, item.affected_changes, item.affected_stages.join(', '), item.context]) {{ const td = document.createElement('td'); td.textContent = value; tr.append(td); }}
        impactRows.append(tr);
      }}
      impactPageInfo.textContent = filtered.length ? `${{start + 1}}–${{Math.min(start + size, filtered.length)}} / ${{filtered.length}}` : '0 / 0';
      impactPrev.disabled = impactPageIndex <= 0;
      impactNext.disabled = impactPageIndex >= pageCount - 1;
    }}
    impactSearch.addEventListener('input', () => {{ impactPageIndex = 0; renderImpacts(); }});
    impactPrev.addEventListener('click', () => {{ impactPageIndex = Math.max(0, impactPageIndex - 1); renderImpacts(); }});
    impactNext.addEventListener('click', () => {{ impactPageIndex += 1; renderImpacts(); }});
    renderRows();
    renderImpacts();
      }} catch (error) {{
        status.textContent = '报告加载失败';
        status.classList.remove('unchanged');
        comparableNode.textContent = `${{error.message}}；请通过本地 HTTP 服务器打开 report.html`;
        document.getElementById('detail-note').textContent = '报告数据未嵌入 HTML，需从同目录读取 manifest.json、summary.json、diff.jsonl 和 root-causes.json。';
      }}
    }}
    loadReport();
  </script>
</body>
</html>
"""


def write_bundle(bundle: ComparisonBundle, output_dir: Path) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "manifest.json").write_text(
        json.dumps(bundle.manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    (output_dir / "summary.json").write_text(
        json.dumps(bundle.summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    with (output_dir / "diff.jsonl").open("w", encoding="utf-8", newline="\n") as output:
        for change in bundle.changes:
            output.write(canonical_json(change) + "\n")
    (output_dir / "report.html").write_text(
        html_report(bundle), encoding="utf-8", newline="\n"
    )
    if "stages" in bundle.summary:
        (output_dir / "stage-summary.json").write_text(
            json.dumps(bundle.summary["stages"], ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
    (output_dir / "root-causes.json").write_text(
        json.dumps(root_impact_summary(bundle.changes), ensure_ascii=False, indent=2)
        + "\n",
        encoding="utf-8",
    )


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="比较大型语言管线 JSON 快照，并输出 JSONL 与本地数据面板。"
    )
    parser.add_argument("--before", type=Path, help="基准 JSON")
    parser.add_argument("--after", type=Path, help="候选 JSON")
    parser.add_argument("--before-run", type=Path, help="基准快照 manifest.json")
    parser.add_argument("--after-run", type=Path, help="候选快照 manifest.json")
    parser.add_argument(
        "--adapter", choices=("bunsetsu", "expression"), help="单产物输入适配器"
    )
    parser.add_argument("--output-dir", required=True, type=Path, help="报告目录")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="backslashreplace")
    args = parse_args(argv)
    run_mode = args.before_run is not None or args.after_run is not None
    file_mode = args.before is not None or args.after is not None or args.adapter is not None
    if run_mode and file_mode:
        raise SystemExit("快照模式与单产物模式不能混用")
    if run_mode:
        if args.before_run is None or args.after_run is None:
            raise SystemExit("快照模式必须同时提供 --before-run 与 --after-run")
        bundle = compare_snapshot_manifests(args.before_run, args.after_run)
    else:
        if args.before is None or args.after is None or args.adapter is None:
            raise SystemExit("单产物模式必须提供 --before、--after 与 --adapter")
        bundle = compare_files(args.before, args.after, args.adapter)
    write_bundle(bundle, args.output_dir)
    print(
        f"语言质量差分完成：adapter={bundle.manifest['adapter']} "
        f"status={bundle.summary['status']} "
        f"changes={bundle.summary['changes']} output={args.output_dir}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
