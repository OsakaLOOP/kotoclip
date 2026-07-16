#!/usr/bin/env python3
"""校验并编译 Kotoclip 语法目录与讲解库。"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import tempfile
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = ROOT / "crates" / "kotoclip-core" / "resources" / "grammar" / "source"
DEFAULT_OUTPUT = ROOT / "crates" / "kotoclip-core" / "resources" / "grammar" / "compiled"
STABLE_ID = re.compile(r"^[a-z][a-z0-9]*(?:[._][a-z0-9]+)+$")
PLACEHOLDER = re.compile(r"\{\{([a-z][a-z0-9_]*)\}\}")
ATOM_FIELDS = {
    "surfaces", "base_forms", "pos_major", "pos_sub1", "conjugation_types",
    "conjugation_forms", "morphology_features", "provider_components",
    "capture", "optional",
}
ATOM_MATCH_FIELDS = ATOM_FIELDS - {"capture", "optional"}

BUNDLE_FIELDS = {
    "bundle_id", "audit_status", "source_refs", "review", "provenance",
    "review_status", "items",
}
BUNDLE_REVIEW_FIELDS = {"item_count", "baseline", "method"}
BUNDLE_PROVENANCE_FIELDS = {"origin", "author", "date", "version"}
BUNDLE_REVIEW_STATUSES = {"unverified", "ai_checked", "trusted"}
BUNDLE_ORIGINS = {"ai", "human", "builtin"}
BUNDLE_ITEM_FIELDS = {
    "concept_id", "kind", "canonical_label", "aliases", "semantic_domains",
    "function_tags", "jlpt_level", "register", "related_concept_ids",
    "contrast_concept_ids", "source_refs", "audit_status", "concept_version",
    "enabled", "extend_existing", "provenance", "review_status", "explanation",
    "senses", "realizations",
}
BUNDLE_EXPLANATION_FIELDS = {
    "explanation_id", "sense_id", "language", "title", "compact_summary",
    "function_summary", "connection", "formation", "usage_notes",
    "semantic_constraints", "pragmatic_notes", "examples", "counter_examples",
    "source_refs", "authoring_status", "content_version", "provenance",
    "review_status", "body_blocks",
}
BUNDLE_SENSE_FIELDS = {
    "sense_id", "label", "function_summary", "semantic_features",
    "context_requirements", "exclusion_conditions", "related_sense_ids",
    "contrast_sense_ids", "explanation_id", "sense_version", "audit_status",
}
BUNDLE_REALIZATION_FIELDS = {
    "realization_id", "rule_id", "possible_sense_ids", "connection_signature",
    "morphology_requirements", "functional_requirements", "context_requirements",
    "examples", "counter_examples", "realization_version", "audit_status",
    "source_refs", "rule",
}
BUNDLE_RULE_FIELDS = {
    "kind", "priority", "enabled", "audit_status", "atoms", "display_from",
    "display_to", "captures", "refines_rule_ids", "conflict_group", "examples",
    "counter_examples", "source_refs", "rule_version", "show_badge",
}
BODY_BLOCK_FIELDS = {"kind", "label", "text"}
BODY_BLOCK_KINDS = {
    "paragraph", "definition_list", "example_pair", "comparison_table",
    "warning", "occurrence_binding",
}

ALLOWED_FIELDS = {
    "concepts": {
        "concept_id", "kind", "canonical_label", "aliases", "semantic_domains",
        "function_tags", "jlpt_level", "register", "related_concept_ids",
        "contrast_concept_ids", "default_explanation_id", "source_refs",
        "audit_status", "concept_version", "enabled",
    },
    "senses": {
        "sense_id", "concept_id", "label", "function_summary", "semantic_features",
        "context_requirements", "exclusion_conditions", "related_sense_ids",
        "contrast_sense_ids", "explanation_id", "sense_version", "audit_status",
    },
    "realizations": {
        "realization_id", "concept_id", "possible_sense_ids", "rule_id",
        "connection_signature", "morphology_requirements", "functional_requirements",
        "context_requirements", "examples", "counter_examples", "realization_version",
        "audit_status", "source_refs",
    },
    "rules": {
        "rule_id", "realization_id", "concept_id", "kind", "priority", "enabled",
        "audit_status", "atoms", "display_from", "display_to", "captures",
        "refines_rule_ids", "conflict_group", "examples", "counter_examples",
        "source_refs", "rule_version", "show_badge",
    },
    "explanations": {
        "explanation_id", "concept_id", "sense_id", "language", "title",
        "compact_summary", "function_summary", "connection", "formation",
        "usage_notes", "semantic_constraints", "pragmatic_notes", "examples",
        "counter_examples", "source_refs", "authoring_status", "content_version",
        "provenance", "review_status", "body_blocks",
    },
    "redirects": {
        "from_concept_id", "to_concept_id", "reason", "redirect_version",
    },
}


def load_catalog_metadata(source: Path) -> dict[str, Any]:
    path = source / "catalog_metadata.json"
    metadata = json.loads(path.read_text(encoding="utf-8"))
    allowed = {"default_provenance", "default_review_status"}
    unknown = set(metadata) - allowed
    if unknown:
        raise ValueError(f"{path}: catalog metadata 含未知字段 {sorted(unknown)}")
    provenance = metadata.get("default_provenance")
    if not isinstance(provenance, dict):
        raise ValueError(f"{path}: default_provenance 必须是对象")
    validate_provenance(path, provenance)
    review_status = metadata.get("default_review_status")
    if review_status not in BUNDLE_REVIEW_STATUSES:
        raise ValueError(f"{path}: 非法 default_review_status {review_status!r}")
    return metadata


def load_items(directory: Path) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    if not directory.exists():
        return items
    for path in sorted(directory.rglob("*.json")):
        value = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(value, list):
            raise ValueError(f"{path}: 顶层必须是数组")
        for index, item in enumerate(value):
            if not isinstance(item, dict):
                raise ValueError(f"{path}[{index}]: 必须是对象")
            item["__source_file"] = path.relative_to(ROOT).as_posix()
            items.append(item)
    return items


def expand_bundles(source: Path) -> dict[str, list[dict[str, Any]]]:
    """把批次化作者输入展开为运行时使用的五层标准目录。"""
    expanded = {kind: [] for kind in ALLOWED_FIELDS}
    metadata = load_catalog_metadata(source)
    directory = source / "bundles"
    if not directory.exists():
        return expanded
    for path in sorted(directory.rglob("*.json")):
        bundle = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(bundle, dict) or not isinstance(bundle.get("items"), list):
            raise ValueError(f"{path}: bundle 必须是含 items 数组的对象")
        reject_unknown_fields(path, "bundle", bundle, BUNDLE_FIELDS)
        review = bundle.get("review")
        if review is not None:
            if not isinstance(review, dict):
                raise ValueError(f"{path}: bundle review 必须是对象")
            reject_unknown_fields(path, "bundle review", review, BUNDLE_REVIEW_FIELDS)
        bundle_provenance = bundle.get("provenance", metadata["default_provenance"])
        validate_provenance(path, bundle_provenance)
        bundle_review_status = bundle.get(
            "review_status", metadata["default_review_status"]
        )
        validate_review_status(path, bundle_review_status)
        bundle_refs = list(bundle.get("source_refs", []))
        bundle_status = bundle.get("audit_status", "reviewed")
        source_file = path.relative_to(ROOT).as_posix()
        for item_index, item in enumerate(bundle["items"]):
            if not isinstance(item, dict):
                raise ValueError(f"{path}[{item_index}]: bundle item 必须是对象")
            item_source = f"{path}[{item_index}]"
            reject_unknown_fields(item_source, "bundle item", item, BUNDLE_ITEM_FIELDS)
            concept_id = item.get("concept_id")
            if not isinstance(concept_id, str) or not STABLE_ID.fullmatch(concept_id):
                raise ValueError(f"{path}[{item_index}]: 非法 concept_id {concept_id!r}")
            item_refs = [*bundle_refs, *item.get("source_refs", [])]
            item_status = item.get("audit_status", bundle_status)
            item_provenance = item.get("provenance", bundle_provenance)
            validate_provenance(item_source, item_provenance)
            item_review_status = item.get("review_status", bundle_review_status)
            validate_review_status(item_source, item_review_status)
            if not item.get("extend_existing", False):
                explanation = item.get("explanation")
                if not isinstance(explanation, dict):
                    raise ValueError(f"{path}[{item_index}]: 新 concept 必须含 explanation")
                reject_unknown_fields(
                    item_source,
                    "bundle explanation",
                    explanation,
                    BUNDLE_EXPLANATION_FIELDS,
                )
                explanation_id = explanation.get(
                    "explanation_id", f"explanation.{concept_id}"
                )
                expanded["concepts"].append({
                    "concept_id": concept_id,
                    "kind": item["kind"],
                    "canonical_label": item["canonical_label"],
                    "aliases": item.get("aliases", []),
                    "semantic_domains": item.get("semantic_domains", []),
                    "function_tags": item.get("function_tags", []),
                    "jlpt_level": item.get("jlpt_level"),
                    "register": item.get("register", ["modern_standard"]),
                    "related_concept_ids": item.get("related_concept_ids", []),
                    "contrast_concept_ids": item.get("contrast_concept_ids", []),
                    "default_explanation_id": explanation_id,
                    "source_refs": item_refs,
                    "audit_status": item_status,
                    "concept_version": item.get("concept_version", 1),
                    "enabled": item.get("enabled", True),
                    "__source_file": source_file,
                })
                expanded["explanations"].append({
                    "explanation_id": explanation_id,
                    "concept_id": concept_id,
                    "sense_id": explanation.get("sense_id"),
                    "language": explanation.get("language", "zh-CN"),
                    "title": explanation.get("title", item["canonical_label"]),
                    "compact_summary": explanation["compact_summary"],
                    "function_summary": explanation.get(
                        "function_summary", explanation["compact_summary"]
                    ),
                    "connection": explanation["connection"],
                    "formation": explanation.get("formation", ""),
                    "usage_notes": explanation.get("usage_notes", []),
                    "semantic_constraints": explanation.get("semantic_constraints", []),
                    "pragmatic_notes": explanation.get("pragmatic_notes", []),
                    "examples": explanation.get("examples", []),
                    "counter_examples": explanation.get("counter_examples", []),
                    "source_refs": [*item_refs, *explanation.get("source_refs", [])],
                    "authoring_status": explanation.get("authoring_status", item_status),
                    "content_version": explanation.get("content_version", 1),
                    "provenance": explanation.get("provenance", item_provenance),
                    "review_status": explanation.get(
                        "review_status", item_review_status
                    ),
                    "body_blocks": explanation["body_blocks"],
                    "__source_file": source_file,
                })

            for sense in item.get("senses", []):
                if not isinstance(sense, dict):
                    raise ValueError(f"{item_source}: sense 必须是对象")
                reject_unknown_fields(item_source, "bundle sense", sense, BUNDLE_SENSE_FIELDS)
                expanded["senses"].append({
                    "sense_id": sense["sense_id"],
                    "concept_id": concept_id,
                    "label": sense["label"],
                    "function_summary": sense["function_summary"],
                    "semantic_features": sense.get("semantic_features", {}),
                    "context_requirements": sense.get("context_requirements", []),
                    "exclusion_conditions": sense.get("exclusion_conditions", []),
                    "related_sense_ids": sense.get("related_sense_ids", []),
                    "contrast_sense_ids": sense.get("contrast_sense_ids", []),
                    "explanation_id": sense.get(
                        "explanation_id", f"explanation.{concept_id}"
                    ),
                    "sense_version": sense.get("sense_version", 1),
                    "audit_status": sense.get("audit_status", item_status),
                    "__source_file": source_file,
                })

            for realization in item.get("realizations", []):
                if not isinstance(realization, dict):
                    raise ValueError(f"{item_source}: realization 必须是对象")
                reject_unknown_fields(
                    item_source,
                    "bundle realization",
                    realization,
                    BUNDLE_REALIZATION_FIELDS,
                )
                rule = realization.get("rule")
                if not isinstance(rule, dict):
                    raise ValueError(
                        f"{path}[{item_index}]: realization {realization.get('realization_id')} 缺少 rule"
                    )
                reject_unknown_fields(item_source, "bundle rule", rule, BUNDLE_RULE_FIELDS)
                realization_id = realization["realization_id"]
                rule_id = realization["rule_id"]
                examples = realization.get("examples", [])
                counter_examples = realization.get("counter_examples", [])
                realization_refs = [*item_refs, *realization.get("source_refs", [])]
                realization_status = realization.get("audit_status", item_status)
                expanded["realizations"].append({
                    "realization_id": realization_id,
                    "concept_id": concept_id,
                    "possible_sense_ids": realization.get("possible_sense_ids", []),
                    "rule_id": rule_id,
                    "connection_signature": realization.get("connection_signature", ""),
                    "morphology_requirements": realization.get("morphology_requirements", []),
                    "functional_requirements": realization.get("functional_requirements", []),
                    "context_requirements": realization.get("context_requirements", []),
                    "examples": examples,
                    "counter_examples": counter_examples,
                    "realization_version": realization.get("realization_version", 1),
                    "audit_status": realization_status,
                    "source_refs": realization_refs,
                    "__source_file": source_file,
                })
                expanded["rules"].append({
                    "rule_id": rule_id,
                    "realization_id": realization_id,
                    "concept_id": concept_id,
                    "kind": rule.get("kind", "functional_morpheme"),
                    "priority": rule.get("priority", 40),
                    "enabled": rule.get("enabled", True),
                    "audit_status": rule.get("audit_status", realization_status),
                    "atoms": rule["atoms"],
                    "display_from": rule.get("display_from", 0),
                    "display_to": rule.get("display_to"),
                    "captures": rule.get("captures", []),
                    "refines_rule_ids": rule.get("refines_rule_ids", []),
                    "conflict_group": rule.get("conflict_group"),
                    "examples": rule.get("examples", examples),
                    "counter_examples": rule.get("counter_examples", counter_examples),
                    "source_refs": [*realization_refs, *rule.get("source_refs", [])],
                    "rule_version": rule.get("rule_version", 1),
                    "show_badge": rule.get("show_badge", False),
                    "__source_file": source_file,
                })
    return expanded


def reject_unknown_fields(
    source: Path | str,
    label: str,
    item: dict[str, Any],
    allowed: set[str],
) -> None:
    unknown = set(item) - allowed
    if unknown:
        raise ValueError(f"{source}: {label} 含未知字段 {sorted(unknown)}")


def validate_provenance(source: Path | str, provenance: Any) -> None:
    if not isinstance(provenance, dict):
        raise ValueError(f"{source}: provenance 必须是对象")
    reject_unknown_fields(source, "provenance", provenance, BUNDLE_PROVENANCE_FIELDS)
    if provenance.get("origin") not in BUNDLE_ORIGINS:
        raise ValueError(f"{source}: 非法 provenance origin {provenance.get('origin')!r}")
    for field in ("author", "date", "version"):
        if not isinstance(provenance.get(field), str) or not provenance[field].strip():
            raise ValueError(f"{source}: provenance {field} 不能为空")


def validate_review_status(source: Path | str, review_status: Any) -> None:
    if review_status not in BUNDLE_REVIEW_STATUSES:
        raise ValueError(f"{source}: 非法 review_status {review_status!r}")


def require_fields(kind: str, item: dict[str, Any], fields: Iterable[str]) -> None:
    source = item.get("__source_file", kind)
    unknown = set(item) - ALLOWED_FIELDS[kind] - {"__source_file"}
    if unknown:
        raise ValueError(f"{source}: {kind} 含未知字段 {sorted(unknown)}")
    missing = [field for field in fields if field not in item]
    if missing:
        raise ValueError(f"{source}: {kind} 缺少字段 {missing}")


def unique_map(items: list[dict[str, Any]], key: str) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for item in items:
        value = item[key]
        if not isinstance(value, str) or not STABLE_ID.fullmatch(value):
            raise ValueError(f"{item.get('__source_file')}: 非法稳定 ID {value!r}")
        if value in result:
            raise ValueError(f"重复 ID: {value}")
        result[value] = item
    return result


def validate(source: Path) -> dict[str, list[dict[str, Any]]]:
    data = {kind: load_items(source / kind) for kind in ALLOWED_FIELDS}
    metadata = load_catalog_metadata(source)
    bundle_data = expand_bundles(source)
    for kind, items in bundle_data.items():
        data[kind].extend(items)
    for explanation in data["explanations"]:
        explanation.setdefault("provenance", metadata["default_provenance"].copy())
        explanation.setdefault("review_status", metadata["default_review_status"])
    required = {
        "concepts": ["concept_id", "kind", "canonical_label", "default_explanation_id", "source_refs", "audit_status", "concept_version"],
        "senses": ["sense_id", "concept_id", "label", "function_summary", "explanation_id", "sense_version", "audit_status"],
        "realizations": ["realization_id", "concept_id", "possible_sense_ids", "rule_id", "examples", "counter_examples", "realization_version", "audit_status"],
        "rules": ["rule_id", "realization_id", "concept_id", "kind", "priority", "enabled", "audit_status", "atoms", "examples", "counter_examples", "rule_version"],
        "explanations": ["explanation_id", "concept_id", "language", "title", "compact_summary", "function_summary", "connection", "examples", "counter_examples", "source_refs", "authoring_status", "content_version", "provenance", "review_status", "body_blocks"],
        "redirects": ["from_concept_id", "to_concept_id", "reason", "redirect_version"],
    }
    for kind, items in data.items():
        for item in items:
            require_fields(kind, item, required[kind])

    concepts = unique_map(data["concepts"], "concept_id")
    senses = unique_map(data["senses"], "sense_id")
    realizations = unique_map(data["realizations"], "realization_id")
    rules = unique_map(data["rules"], "rule_id")
    explanations = unique_map(data["explanations"], "explanation_id")
    redirects = unique_map(data["redirects"], "from_concept_id")

    for concept in concepts.values():
        explanation_id = concept["default_explanation_id"]
        if explanation_id not in explanations:
            raise ValueError(f"{concept['concept_id']}: 缺少讲解 {explanation_id}")
        if concept["audit_status"] == "verified" and not concept.get("source_refs"):
            raise ValueError(f"{concept['concept_id']}: verified concept 缺少来源")

    for sense in senses.values():
        if sense["concept_id"] not in concepts:
            raise ValueError(f"{sense['sense_id']}: 悬空 concept 引用")
        if sense["explanation_id"] not in explanations:
            raise ValueError(f"{sense['sense_id']}: 悬空 explanation 引用")

    for realization in realizations.values():
        if realization["concept_id"] not in concepts:
            raise ValueError(f"{realization['realization_id']}: 悬空 concept 引用")
        if realization["rule_id"] not in rules:
            raise ValueError(f"{realization['realization_id']}: 悬空 rule 引用")
        for sense_id in realization["possible_sense_ids"]:
            if sense_id not in senses or senses[sense_id]["concept_id"] != realization["concept_id"]:
                raise ValueError(f"{realization['realization_id']}: 非法 sense 引用 {sense_id}")
        if realization["audit_status"] == "verified" and not realization.get("source_refs"):
            raise ValueError(f"{realization['realization_id']}: verified realization 缺少来源")

    for rule in rules.values():
        if rule["concept_id"] not in concepts or rule["realization_id"] not in realizations:
            raise ValueError(f"{rule['rule_id']}: 悬空目录引用")
        realization = realizations[rule["realization_id"]]
        if realization["rule_id"] != rule["rule_id"] or realization["concept_id"] != rule["concept_id"]:
            raise ValueError(f"{rule['rule_id']}: realization 反向引用或 concept 身份不一致")
        if rule["audit_status"] == "verified" and (not rule["examples"] or not rule["counter_examples"]):
            raise ValueError(f"{rule['rule_id']}: verified rule 必须含正反例")
        if rule["audit_status"] == "verified" and not rule.get("source_refs"):
            raise ValueError(f"{rule['rule_id']}: verified rule 缺少来源")
        captures = set(rule.get("captures", []))
        for atom in rule["atoms"]:
            if not isinstance(atom, dict) or not atom:
                raise ValueError(f"{rule['rule_id']}: rule atom 不能为空")
            unknown_atom_fields = set(atom) - ATOM_FIELDS
            if unknown_atom_fields:
                raise ValueError(f"{rule['rule_id']}: atom 含未知字段 {sorted(unknown_atom_fields)}")
            if not any(atom.get(field) for field in ATOM_MATCH_FIELDS):
                raise ValueError(f"{rule['rule_id']}: atom 缺少表面、词性、活用或形态特征约束")
            provider_components = atom.get("provider_components", [])
            if provider_components:
                if not isinstance(provider_components, list):
                    raise ValueError(f"{rule['rule_id']}: provider_components 必须是数组")
                for component in provider_components:
                    if not isinstance(component, dict) or set(component) != {"role", "surface", "base_form"}:
                        raise ValueError(f"{rule['rule_id']}: provider component 必须含 role、surface、base_form")
                    if not all(isinstance(component[field], str) and component[field] for field in component):
                        raise ValueError(f"{rule['rule_id']}: provider component 字段不能为空")
            capture = atom.get("capture")
            if capture:
                captures.add(capture)
        for refined in rule.get("refines_rule_ids", []):
            if refined not in rules:
                raise ValueError(f"{rule['rule_id']}: refines 悬空 {refined}")
        explanation = explanations[concepts[rule["concept_id"]]["default_explanation_id"]]
        text = json.dumps(explanation, ensure_ascii=False)
        unknown_placeholders = set(PLACEHOLDER.findall(text)) - captures - {"actual_form"}
        if unknown_placeholders:
            raise ValueError(f"{rule['rule_id']}: 未绑定模板变量 {sorted(unknown_placeholders)}")

    for explanation in explanations.values():
        if explanation["concept_id"] not in concepts:
            raise ValueError(f"{explanation['explanation_id']}: 悬空 concept 引用")
        if explanation.get("sense_id"):
            sense_id = explanation["sense_id"]
            if sense_id not in senses or senses[sense_id]["concept_id"] != explanation["concept_id"]:
                raise ValueError(f"{explanation['explanation_id']}: 悬空或跨 concept 的 sense 引用")
        if explanation["authoring_status"] == "verified":
            if not explanation["compact_summary"] or not explanation["function_summary"] or not explanation["body_blocks"]:
                raise ValueError(f"{explanation['explanation_id']}: verified 讲解不完整")
            if not explanation.get("source_refs"):
                raise ValueError(f"{explanation['explanation_id']}: verified 讲解缺少来源")
        validate_provenance(explanation["explanation_id"], explanation["provenance"])
        validate_review_status(
            explanation["explanation_id"], explanation["review_status"]
        )
        body_blocks = explanation.get("body_blocks", [])
        if not isinstance(body_blocks, list):
            raise ValueError(f"{explanation['explanation_id']}: body_blocks 必须是数组")
        for block_index, block in enumerate(body_blocks):
            if not isinstance(block, dict):
                raise ValueError(
                    f"{explanation['explanation_id']}: body_blocks[{block_index}] 必须是对象"
                )
            reject_unknown_fields(
                explanation["explanation_id"],
                f"body_blocks[{block_index}]",
                block,
                BODY_BLOCK_FIELDS,
            )
            if block.get("kind") not in BODY_BLOCK_KINDS:
                raise ValueError(
                    f"{explanation['explanation_id']}: 不支持的内容块 {block.get('kind')!r}"
                )
            if not isinstance(block.get("text"), str) or not block["text"].strip():
                raise ValueError(
                    f"{explanation['explanation_id']}: body_blocks[{block_index}] text 不能为空"
                )
            if block.get("label") is not None and not isinstance(block["label"], str):
                raise ValueError(
                    f"{explanation['explanation_id']}: body_blocks[{block_index}] label 必须是字符串或 null"
                )

    for redirect in redirects.values():
        source_id = redirect["from_concept_id"]
        target_id = redirect["to_concept_id"]
        if source_id not in concepts or target_id not in concepts:
            raise ValueError(f"{source_id}: 重定向引用不存在的 concept {target_id}")
        visited = {source_id}
        cursor = target_id
        while cursor in redirects:
            if cursor in visited:
                raise ValueError(f"{source_id}: concept 重定向形成循环")
            visited.add(cursor)
            cursor = redirects[cursor]["to_concept_id"]

    for items in data.values():
        for item in items:
            item.pop("__source_file", None)
    return data


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def compile_catalog(source: Path, output: Path) -> dict[str, Any]:
    data = validate(source)
    redirect_map = {item["from_concept_id"]: item["to_concept_id"] for item in data["redirects"]}
    for concept in data["concepts"]:
        if concept["concept_id"] in redirect_map:
            concept["audit_status"] = "deprecated"
            concept["enabled"] = False
    catalog = {
        "schema_version": 1,
        "catalog_version": "grammar-2026.07.16",
        "concepts": data["concepts"],
        "senses": data["senses"],
        "realizations": data["realizations"],
        "rules": sorted(data["rules"], key=lambda item: (-item["priority"], item["rule_id"])),
        "redirects": data["redirects"],
    }
    explanations = {
        "schema_version": 1,
        "content_version": "grammar-content-2026.07.16",
        "explanations": data["explanations"],
    }
    search_entries = []
    realizations_by_concept: dict[str, list[dict[str, Any]]] = {}
    for realization in data["realizations"]:
        realizations_by_concept.setdefault(realization["concept_id"], []).append(realization)
    senses_by_concept: dict[str, list[dict[str, Any]]] = {}
    for sense in data["senses"]:
        senses_by_concept.setdefault(sense["concept_id"], []).append(sense)
    redirected_aliases: dict[str, list[str]] = {}
    for concept in data["concepts"]:
        if concept["concept_id"] in redirect_map:
            redirected_aliases.setdefault(redirect_map[concept["concept_id"]], []).extend(
                [concept["concept_id"], concept["canonical_label"], *concept.get("aliases", [])]
            )
    for concept in data["concepts"]:
        if concept["concept_id"] in redirect_map:
            continue
        search_entries.append({
            "concept_id": concept["concept_id"],
            "label": concept["canonical_label"],
            "aliases": [*concept.get("aliases", []), *redirected_aliases.get(concept["concept_id"], [])],
            "semantic_domains": concept.get("semantic_domains", []),
            "function_tags": concept.get("function_tags", []),
            "jlpt_level": concept.get("jlpt_level"),
            "surface_hints": [item.get("connection_signature", "") for item in realizations_by_concept.get(concept["concept_id"], [])],
            "register": concept.get("register", []),
            "related_concept_ids": concept.get("related_concept_ids", []),
            "contrast_concept_ids": concept.get("contrast_concept_ids", []),
            "sense_hints": [
                hint
                for sense in senses_by_concept.get(concept["concept_id"], [])
                for hint in (sense.get("label", ""), sense.get("function_summary", ""))
                if hint
            ],
        })
    canonical = json.dumps({"catalog": catalog, "explanations": explanations}, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    fingerprint = hashlib.sha256(canonical).hexdigest()
    manifest = {
        "schema_version": 1,
        "catalog_version": catalog["catalog_version"],
        "content_version": explanations["content_version"],
        "fingerprint": fingerprint,
        "counts": {kind: len(items) for kind, items in data.items()},
    }
    write_json(output / "grammar_catalog.json", catalog)
    write_json(output / "grammar_explanations.json", explanations)
    write_json(output / "grammar_search_index.json", {"entries": search_entries})
    write_json(output / "manifest.json", manifest)
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true", help="校验已提交编译产物是否最新")
    args = parser.parse_args()
    if args.check:
        with tempfile.TemporaryDirectory(prefix="kotoclip-grammar-") as temporary:
            temporary_output = Path(temporary)
            manifest = compile_catalog(args.source, temporary_output)
            names = ["grammar_catalog.json", "grammar_explanations.json", "grammar_search_index.json", "manifest.json"]
            stale = [name for name in names if not (args.output / name).exists() or (args.output / name).read_bytes() != (temporary_output / name).read_bytes()]
            if stale:
                raise SystemExit(f"语法编译产物不是最新版本：{', '.join(stale)}")
    else:
        manifest = compile_catalog(args.source, args.output)
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
