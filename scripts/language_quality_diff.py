#!/usr/bin/env python3
"""对大型语言管线快照生成机器可读差分和原生阅读条目。"""

from __future__ import annotations

import argparse
import bisect
import gc
import gzip
import hashlib
import heapq
import io
import itertools
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import time
import threading
import unicodedata
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from difflib import SequenceMatcher
from pathlib import Path
from typing import Any, Iterable, Sequence


SCHEMA_VERSION = "kotoclip.quality.diff.v4"
SNAPSHOT_SCHEMA_VERSION = "kotoclip.quality.snapshot.v1"
PRODUCER_VERSION = "4"
MAX_INLINE_VALUE_BYTES = 480
EXTERNAL_SORT_CHUNK_ENTITIES = 10_000
OUTPUT_GZIP_LEVEL = 1
CANDIDATE_CACHE_SCHEMA_VERSION = "kotoclip.quality.candidates-cache.v1"
SEVERITY_ORDER = {"critical": 0, "high": 1, "medium": 2, "info": 3}

# 主计数只覆盖会改变用户读解、结构判断或查询目标的最终结果。
# 候选探针、资源指纹、画像和纯身份字段仍写入 diff.jsonl，作为根因与诊断证据。
PRIMARY_CHANGE_DOMAINS = {
    "morpheme": "structure",
    "morphology": "structure",
    "word_formation": "structure",
    "lexical_unit": "lookup",
    "bunsetsu_boundary": "structure",
    "bunsetsu": "structure",
    "grammar_occurrence": "grammar",
    "grammar_projection": "grammar",
    "expression": "expression",
    "ui_projection": "projection",
}

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
    reading_units: dict[str, Any] | None = None
    diff_prepared: bool = False
    reading_prepared: bool = False


def process_memory_snapshot() -> dict[str, int | None]:
    """读取当前进程的实际驻留内存；Windows 使用工作集，其他系统尽量降级。"""
    if sys.platform == "win32":
        import ctypes
        from ctypes import wintypes

        class ProcessMemoryCountersEx(ctypes.Structure):
            _fields_ = [
                ("cb", wintypes.DWORD),
                ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t),
                ("PrivateUsage", ctypes.c_size_t),
            ]

        counters = ProcessMemoryCountersEx()
        counters.cb = ctypes.sizeof(counters)
        get_current_process = ctypes.windll.kernel32.GetCurrentProcess
        get_current_process.restype = wintypes.HANDLE
        get_process_memory_info = ctypes.windll.psapi.GetProcessMemoryInfo
        get_process_memory_info.argtypes = (
            wintypes.HANDLE,
            ctypes.c_void_p,
            wintypes.DWORD,
        )
        get_process_memory_info.restype = wintypes.BOOL
        if get_process_memory_info(get_current_process(), ctypes.byref(counters), counters.cb):
            return {
                "rss_bytes": int(counters.WorkingSetSize),
                "peak_rss_bytes": int(counters.PeakWorkingSetSize),
                "private_bytes": int(counters.PrivateUsage),
            }
    return {"rss_bytes": None, "peak_rss_bytes": None, "private_bytes": None}


class MemoryProbe:
    """低开销阶段采样，记录真实进程工作集而不是 Python 分配器近似值。"""

    def __init__(self, output_path: Path | None) -> None:
        self.output_path = output_path
        self.started = time.perf_counter()
        self.samples: list[dict[str, Any]] = []
        self.phase = "started"
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        if output_path is not None:
            self._thread = threading.Thread(target=self._sample_periodically, daemon=True)
            self._thread.start()

    def _record(self, phase: str, sample_kind: str, **extra: Any) -> None:
        self.samples.append(
            {
                "phase": phase,
                "sample_kind": sample_kind,
                "elapsed_seconds": round(time.perf_counter() - self.started, 6),
                **process_memory_snapshot(),
                **extra,
            }
        )

    def _sample_periodically(self) -> None:
        while not self._stop.wait(0.5):
            self._record(self.phase, "interval")

    def sample(self, phase: str, **extra: Any) -> None:
        self.phase = phase
        self._record(phase, "checkpoint", **extra)

    def write(self) -> None:
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=1)
        if self.output_path is None:
            return
        self.output_path.parent.mkdir(parents=True, exist_ok=True)
        target_peak_bytes = int(1.5 * 1024 * 1024 * 1024)
        observed_peak_bytes = max(
            (int(sample["peak_rss_bytes"] or 0) for sample in self.samples),
            default=0,
        )
        self.output_path.write_text(
            json.dumps(
                {
                    "schema_version": "kotoclip.quality.memory-profile.v1",
                    "target_peak_bytes": target_peak_bytes,
                    "observed_peak_rss_bytes": observed_peak_bytes,
                    "within_target": observed_peak_bytes <= target_peak_bytes,
                    "elapsed_seconds": (
                        self.samples[-1]["elapsed_seconds"] if self.samples else 0.0
                    ),
                    "sample_count": len(self.samples),
                    "samples": self.samples,
                },
                ensure_ascii=False,
                indent=2,
            )
            + "\n",
            encoding="utf-8",
            newline="\n",
        )


class EntitySpool:
    """将规范实体按阶段写入临时 JSONL，避免全管线实体同时常驻内存。"""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        self.counts: Counter[str] = Counter()
        self._streams: dict[str, Any] = {}

    def append(self, entity: dict[str, Any]) -> None:
        stage = str(entity["stage"])
        stream = self._streams.get(stage)
        if stream is None:
            stream = (self.root / f"{stage}.jsonl").open(
                "w",
                encoding="utf-8",
                newline="\n",
            )
            self._streams[stage] = stream
        stream.write(canonical_json(entity))
        stream.write("\n")
        self.counts[stage] += 1

    def close(self) -> None:
        for stream in self._streams.values():
            stream.close()
        self._streams.clear()

    def __enter__(self) -> EntitySpool:
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()


def _iter_jsonl(path: Path) -> Iterable[dict[str, Any]]:
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt", encoding="utf-8") as source:
        for line in source:
            if line.strip():
                yield json.loads(line)


def _entity_key(entity: dict[str, Any]) -> tuple[str, str]:
    return str(entity["key"]), canonical_json(entity["value"])


def _entity_anchor_key(entity: dict[str, Any]) -> tuple[str, str, str]:
    key, value = _entity_key(entity)
    return str(entity["anchor"]), key, value


def _iter_external_sorted_jsonl(
    source_path: Path,
    temporary_parent: Path,
    key: Any,
) -> Iterable[dict[str, Any]]:
    """分块排序 JSONL，并在临时文件之间归并，峰值受单块大小限制。"""
    if not source_path.is_file():
        return
    with tempfile.TemporaryDirectory(prefix=".quality-sort-", dir=temporary_parent) as temporary:
        temporary_root = Path(temporary)
        chunk_paths: list[Path] = []
        chunk: list[dict[str, Any]] = []

        def flush() -> None:
            if not chunk:
                return
            chunk.sort(key=key)
            chunk_path = temporary_root / f"chunk-{len(chunk_paths):06d}.jsonl"
            with chunk_path.open("w", encoding="utf-8", newline="\n") as output:
                for entity in chunk:
                    output.write(canonical_json(entity))
                    output.write("\n")
            chunk_paths.append(chunk_path)
            chunk.clear()

        for entity in _iter_jsonl(source_path):
            chunk.append(entity)
            if len(chunk) >= EXTERNAL_SORT_CHUNK_ENTITIES:
                flush()
        flush()
        if not chunk_paths:
            return
        streams = [_iter_jsonl(path) for path in chunk_paths]
        yield from heapq.merge(*streams, key=key)


def _iter_entity_groups(
    entities: Iterable[dict[str, Any]], key: Any
) -> Iterable[tuple[Any, list[dict[str, Any]]]]:
    for group_key, group in itertools.groupby(entities, key=key):
        yield group_key, list(group)


def _iter_stage_changes_from_spools(
    stage: str,
    before_root: Path,
    after_root: Path,
    temporary_parent: Path,
) -> Iterable[dict[str, Any]]:
    """以实体 key 与 anchor 两次外部归并取代全量索引和未匹配列表。"""
    before_path = before_root / f"{stage}.jsonl"
    after_path = after_root / f"{stage}.jsonl"
    with tempfile.TemporaryDirectory(prefix=f".quality-stage-{stage}-", dir=temporary_parent) as temporary:
        temporary_root = Path(temporary)
        before_unmatched_path = temporary_root / "before-unmatched.jsonl"
        after_unmatched_path = temporary_root / "after-unmatched.jsonl"
        before_groups = iter(
            _iter_entity_groups(
                _iter_external_sorted_jsonl(before_path, temporary_root, _entity_key),
                lambda entity: _entity_key(entity)[0],
            )
        )
        after_groups = iter(
            _iter_entity_groups(
                _iter_external_sorted_jsonl(after_path, temporary_root, _entity_key),
                lambda entity: _entity_key(entity)[0],
            )
        )
        before_group = next(before_groups, None)
        after_group = next(after_groups, None)
        with before_unmatched_path.open("w", encoding="utf-8", newline="\n") as before_unmatched, after_unmatched_path.open("w", encoding="utf-8", newline="\n") as after_unmatched:
            while before_group is not None or after_group is not None:
                if after_group is None or (
                    before_group is not None and before_group[0] < after_group[0]
                ):
                    for entity in before_group[1]:
                        before_unmatched.write(canonical_json(entity) + "\n")
                    before_group = next(before_groups, None)
                    continue
                if before_group is None or after_group[0] < before_group[0]:
                    for entity in after_group[1]:
                        after_unmatched.write(canonical_json(entity) + "\n")
                    after_group = next(after_groups, None)
                    continue
                for before_entity, after_entity in zip(before_group[1], after_group[1]):
                    change = entity_change(stage, "modified", before_entity, after_entity)
                    if change is not None:
                        yield change
                for entity in before_group[1][len(after_group[1]):]:
                    before_unmatched.write(canonical_json(entity) + "\n")
                for entity in after_group[1][len(before_group[1]):]:
                    after_unmatched.write(canonical_json(entity) + "\n")
                before_group = next(before_groups, None)
                after_group = next(after_groups, None)

        before_anchor_groups = iter(
            _iter_entity_groups(
                _iter_external_sorted_jsonl(before_unmatched_path, temporary_root, _entity_anchor_key),
                lambda entity: _entity_anchor_key(entity)[0],
            )
        )
        after_anchor_groups = iter(
            _iter_entity_groups(
                _iter_external_sorted_jsonl(after_unmatched_path, temporary_root, _entity_anchor_key),
                lambda entity: _entity_anchor_key(entity)[0],
            )
        )
        before_group = next(before_anchor_groups, None)
        after_group = next(after_anchor_groups, None)
        while before_group is not None or after_group is not None:
            if after_group is None or (
                before_group is not None and before_group[0] < after_group[0]
            ):
                for entity in before_group[1]:
                    change = entity_change(stage, "removed", entity, None)
                    if change is not None:
                        yield change
                before_group = next(before_anchor_groups, None)
                continue
            if before_group is None or after_group[0] < before_group[0]:
                for entity in after_group[1]:
                    change = entity_change(stage, "added", None, entity)
                    if change is not None:
                        yield change
                after_group = next(after_anchor_groups, None)
                continue
            pairs, remaining_before, remaining_after = pair_unmatched_by_anchor(
                before_group[1], after_group[1]
            )
            for before_entity, after_entity in pairs:
                change = entity_change(stage, "modified", before_entity, after_entity)
                if change is not None:
                    yield change
            for entity in remaining_before:
                change = entity_change(stage, "removed", entity, None)
                if change is not None:
                    yield change
            for entity in remaining_after:
                change = entity_change(stage, "added", None, entity)
                if change is not None:
                    yield change
            before_group = next(before_anchor_groups, None)
            after_group = next(after_anchor_groups, None)


def compact_change(change: dict[str, Any]) -> dict[str, Any]:
    """保留统计、归因和阅读投影字段；完整字段变化继续写入外部 spool。"""
    fields = (
        "change_id",
        "stage",
        "channel",
        "type",
        "operation",
        "scope",
        "severity",
        "entity_kind",
        "anchor",
        "ranges",
        "context",
        "source_artifacts",
        "status_before",
        "status_after",
        "affects_stages",
    )
    return {field: change[field] for field in fields if field in change}


def iter_gzip_jsonl(path: Path) -> Iterable[dict[str, Any]]:
    if not path.is_file():
        return
    with gzip.open(path, "rt", encoding="utf-8") as source:
        for line in source:
            if line.strip():
                yield json.loads(line)


def write_annotated_diff(
    raw_root: Path,
    changes: Sequence[dict[str, Any]],
    output_path: Path,
) -> None:
    """将紧凑索引中的归因标记合并回完整变化事实。"""
    annotations = {str(change["change_id"]): change for change in changes}
    annotation_fields = (
        "causal_status",
        "cause_change_ids",
        "causal_basis",
        "causal_confidence",
        "counted_in_primary",
        "primary_domain",
        "primary_exclusion",
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    temporary_path = output_path.with_suffix(output_path.suffix + ".tmp")
    with temporary_path.open("wb") as output_stream:
        with gzip.GzipFile(
            fileobj=output_stream,
            mode="wb",
            compresslevel=OUTPUT_GZIP_LEVEL,
            mtime=0,
        ) as compressed:
            for stage in STAGE_ORDER:
                records = _iter_external_sorted_jsonl(
                    raw_root / f"{stage}.jsonl",
                    raw_root,
                    lambda change: (
                        SEVERITY_ORDER.get(change["severity"], 99),
                        span_of_ranges(change_ranges(change)) or (-1, -1),
                        change["type"],
                        change["change_id"],
                    ),
                )
                for record in records:
                    annotation = annotations.get(str(record["change_id"]), {})
                    for field in annotation_fields:
                        if field in annotation:
                            record[field] = annotation[field]
                    compressed.write(canonical_json(record).encode("utf-8"))
                    compressed.write(b"\n")
                gc.collect()
    temporary_path.replace(output_path)


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
    if path.suffix == ".gz":
        with gzip.open(path, "rt", encoding="utf-8") as source:
            return json.load(source)
    with path.open("r", encoding="utf-8") as source:
        return json.load(source)


def iter_json_array(path: Path) -> Iterable[Any]:
    """增量解析顶层 JSON 数组，支持普通文件与 gzip artifact。"""
    decoder = json.JSONDecoder()
    buffer = ""
    position = 0
    end_of_file = False
    source_context = (
        gzip.open(path, "rt", encoding="utf-8")
        if path.suffix == ".gz"
        else path.open("r", encoding="utf-8")
    )
    with source_context as source:
        def fill() -> None:
            nonlocal buffer, end_of_file
            chunk = source.read(1024 * 1024)
            if chunk:
                buffer += chunk
            else:
                end_of_file = True

        fill()
        while True:
            while position >= len(buffer):
                if end_of_file:
                    raise ValueError(f"JSON 数组意外结束：{path}")
                buffer = ""
                position = 0
                fill()
            while position < len(buffer) and buffer[position].isspace():
                position += 1
            if position < len(buffer):
                break
        if buffer[position] != "[":
            raise ValueError(f"artifact 不是顶层 JSON 数组：{path}")
        position += 1
        expect_value = True
        while True:
            while True:
                while position < len(buffer) and buffer[position].isspace():
                    position += 1
                if position < len(buffer):
                    break
                if end_of_file:
                    raise ValueError(f"JSON 数组缺少结束符：{path}")
                buffer = ""
                position = 0
                fill()
            if not expect_value:
                if buffer[position] == ",":
                    position += 1
                    expect_value = True
                    continue
                if buffer[position] == "]":
                    return
                raise ValueError(f"JSON 数组缺少逗号：{path}")
            if buffer[position] == "]":
                return
            while True:
                try:
                    value, next_position = decoder.raw_decode(buffer, position)
                except json.JSONDecodeError:
                    if end_of_file:
                        raise ValueError(f"JSON 数组元素不完整：{path}")
                    if position:
                        buffer = buffer[position:]
                        position = 0
                    fill()
                    continue
                yield value
                position = next_position
                expect_value = False
                break


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


def field_change_count(before: Any, after: Any) -> int:
    """计算与 field_changes 等价的条目数，不构造候选评分阶段不需要的描述对象。"""
    if before == after:
        return 0
    if isinstance(before, dict) and isinstance(after, dict):
        count = 0
        for key in set(before) | set(after):
            if key not in before or key not in after:
                count += 1
            else:
                count += field_change_count(before[key], after[key])
        return count
    if isinstance(before, list) and isinstance(after, list):
        before_keyed = keyed_list(before)
        after_keyed = keyed_list(after)
        if before_keyed is not None and after_keyed is not None:
            count = 0
            for key in set(before_keyed) | set(after_keyed):
                if key not in before_keyed or key not in after_keyed:
                    count += 1
                else:
                    count += field_change_count(before_keyed[key], after_keyed[key])
            return count
    return 1


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
    target: dict[str, list[dict[str, Any]]] | EntitySpool,
    seen: set[bytes],
    entity: dict[str, Any],
) -> None:
    digest = hashlib.sha256()
    for value in (str(entity["stage"]), str(entity["key"]), canonical_json(entity["value"])):
        digest.update(value.encode("utf-8"))
        digest.update(b"\0")
    signature = digest.digest()[:16]
    if signature in seen:
        return
    seen.add(signature)
    if isinstance(target, EntitySpool):
        target.append(entity)
    else:
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
    entities: dict[str, list[dict[str, Any]]] | EntitySpool,
    seen: set[bytes],
    bunsetsu: dict[str, Any],
    artifact: str,
    authoritative_grammar_keys: set[tuple[str, str]] | None = None,
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
        append_grammar_occurrence(
            entities,
            seen,
            occurrence,
            artifact,
            record_authoritative_keys=authoritative_grammar_keys,
        )
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
    entities: dict[str, list[dict[str, Any]]] | EntitySpool,
    seen: set[bytes],
    occurrence: dict[str, Any],
    artifact: str,
    authoritative_keys: set[tuple[str, str]] | None = None,
    record_authoritative_keys: set[tuple[str, str]] | None = None,
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
    if record_authoritative_keys is not None:
        record_authoritative_keys.add((stage, entity["key"]))


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


def snapshot_artifact_path(
    manifest_path: Path, name: str, descriptor: dict[str, Any]
) -> Path:
    path = (manifest_path.parent / str(descriptor["path"])).resolve()
    if not path.is_file():
        raise FileNotFoundError(f"快照产物不存在：{name} -> {path}")
    expected_hash = descriptor.get("sha256")
    actual_hash = file_hash(path)
    if expected_hash and expected_hash != actual_hash:
        raise ValueError(f"快照产物哈希不一致：{name} -> {path}")
    return path


def load_snapshot_artifact(
    manifest_path: Path, name: str, descriptor: dict[str, Any]
) -> Any:
    path = snapshot_artifact_path(manifest_path, name, descriptor)
    return read_json(path)


def iter_snapshot_artifact(
    manifest_path: Path, name: str, descriptor: dict[str, Any]
) -> Iterable[Any]:
    return iter_json_array(snapshot_artifact_path(manifest_path, name, descriptor))


def normalize_snapshot(
    manifest_path: Path,
    spool: EntitySpool,
) -> tuple[dict[str, Any], dict[str, int], set[str]]:
    manifest = read_json(manifest_path)
    if manifest.get("schema_version") != SNAPSHOT_SCHEMA_VERSION:
        raise ValueError(
            f"快照 schema 不兼容：{manifest.get('schema_version')}，"
            f"要求 {SNAPSHOT_SCHEMA_VERSION}"
        )
    entities = spool
    seen: set[bytes] = set()
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

    def load_named(name: str) -> Any:
        descriptor = artifact_descriptors.get(name)
        if not isinstance(descriptor, dict):
            return None
        return load_snapshot_artifact(manifest_path, name, descriptor)

    def iter_named(name: str) -> Iterable[Any]:
        descriptor = artifact_descriptors.get(name)
        if not isinstance(descriptor, dict):
            return ()
        return iter_snapshot_artifact(manifest_path, name, descriptor)

    for name in artifact_descriptors:
        covered.update(ARTIFACT_STAGE_COVERAGE.get(name, set()))

    has_tokens = isinstance(artifact_descriptors.get("tokens"), dict)
    authoritative_grammar_keys: set[tuple[str, str]] = set()
    if has_tokens:
        for token in iter_named("tokens"):
            bunsetsu = token.get("bunsetsu", {})
            is_content = token.get("display_class", "content") == "content"
            if is_content and isinstance(bunsetsu, dict):
                add_bunsetsu_entities(
                    entities,
                    seen,
                    bunsetsu,
                    "tokens",
                    authoritative_grammar_keys,
                )
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
    if isinstance(artifact_descriptors.get("bunsetsu"), dict):
        for report_index, report in enumerate(iter_named("bunsetsu")):
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

    word_report = load_named("word_formations")
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
        del word_report

    lexical_report = load_named("lexical_candidates")
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
        del lexical_report

    if isinstance(artifact_descriptors.get("grammar_occurrences"), dict):
        for occurrence in iter_named("grammar_occurrences"):
            append_grammar_occurrence(
                entities,
                seen,
                occurrence,
                "grammar_occurrences",
                authoritative_grammar_keys,
            )
    residual_report = load_named("grammar_residuals")
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
        del residual_report
    if isinstance(artifact_descriptors.get("expressions"), dict):
        for expression in iter_named("expressions"):
            append_expression_entity(entities, seen, expression, "expressions")
    if isinstance(artifact_descriptors.get("catalogs"), dict):
        for ordinal, catalog in enumerate(iter_named("catalogs")):
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
    ui_projection = load_named("ui_projection")
    if ui_projection is not None:
        append_ui_projection_entities(entities, seen, ui_projection)
        del ui_projection

    counts = dict(spool.counts)
    del seen
    gc.collect()
    return manifest, counts, covered


def append_expression_entity(
    entities: dict[str, list[dict[str, Any]]] | EntitySpool,
    seen: set[bytes],
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
    entities: dict[str, list[dict[str, Any]]] | EntitySpool,
    seen: set[bytes],
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
        candidate: dict[str, Any] | None = None
        candidate_score: int | None = None
        for item in after_groups.get(before_entity["anchor"], []):
            if id(item) in used_after:
                continue
            score = field_change_count(before_entity["value"], item["value"])
            if candidate_score is None or score < candidate_score:
                candidate = item
                candidate_score = score
        if candidate is None:
            remaining_before.append(before_entity)
            continue
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


def primary_change_domain(change: dict[str, Any]) -> str | None:
    """返回主计数领域；None 表示仅作为诊断证据。"""
    stage = str(change.get("stage", ""))
    domain = PRIMARY_CHANGE_DOMAINS.get(stage)
    if domain is None:
        return None
    if change.get("scope") in {"identity", "evidence"}:
        return None
    return domain


def annotate_primary_count(changes: list[dict[str, Any]]) -> list[dict[str, Any]]:
    primary: list[dict[str, Any]] = []
    for change in changes:
        domain = primary_change_domain(change)
        if domain is None:
            change["counted_in_primary"] = False
            change["primary_exclusion"] = (
                "non_result_stage_or_diagnostic_scope"
            )
            continue
        change["counted_in_primary"] = True
        change["primary_domain"] = domain
        primary.append(change)
    return primary


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


def iter_entity_stage_changes(
    stage: str,
    before_values: Sequence[dict[str, Any]],
    after_values: Sequence[dict[str, Any]],
) -> Iterable[dict[str, Any]]:
    before_index = indexed_entities(before_values)
    after_index = indexed_entities(after_values)
    matched_keys = set(before_index) & set(after_index)
    for key in sorted(matched_keys):
        change = entity_change(stage, "modified", before_index[key], after_index[key])
        if change:
            yield change
    before_unmatched = [before_index[key] for key in sorted(set(before_index) - matched_keys)]
    after_unmatched = [after_index[key] for key in sorted(set(after_index) - matched_keys)]
    pairs, before_unmatched, after_unmatched = pair_unmatched_by_anchor(
        before_unmatched, after_unmatched
    )
    for before, after in pairs:
        change = entity_change(stage, "modified", before, after)
        if change:
            yield change
    for entity in before_unmatched:
        change = entity_change(stage, "removed", entity, None)
        if change is not None:
            yield change
    for entity in after_unmatched:
        change = entity_change(stage, "added", None, entity)
        if change is not None:
            yield change


def compare_entity_stage(
    stage: str,
    before_values: Sequence[dict[str, Any]],
    after_values: Sequence[dict[str, Any]],
) -> list[dict[str, Any]]:
    return list(iter_entity_stage_changes(stage, before_values, after_values))


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
    # 桶仅保留列表下标；完整紧凑 change 仍由调用方持有，避免多份 dict 引用视图。
    by_stage: dict[str, list[int]] = defaultdict(list)
    interval_buckets: dict[str, dict[int, list[int]]] = defaultdict(
        lambda: defaultdict(list)
    )
    ranges_by_index: list[list[tuple[int, int]]] = []
    resource_affects_by_index: list[set[str]] = []
    bucket_size = 64
    for index, change in enumerate(changes):
        stage = str(change["stage"])
        ranges = change_ranges(change)
        ranges_by_index.append(ranges)
        resource_affects_by_index.append(set(change.get("affects_stages", [])))
        by_stage[stage].append(index)
        for start, end in ranges:
            first_bucket = start // bucket_size
            last_bucket = max(start, end - 1) // bucket_size
            for bucket in range(first_bucket, last_bucket + 1):
                interval_buckets[stage][bucket].append(index)

    for stage in STAGE_ORDER:
        dependencies = sorted(
            transitive_dependencies(stage),
            key=lambda item: STAGE_INDEX.get(item, -1),
            reverse=True,
        )
        for index in by_stage.get(stage, []):
            change = changes[index]
            current_ranges = ranges_by_index[index]
            closest: list[int] = []
            for dependency in dependencies:
                if dependency == "resource":
                    closest = [
                        upstream_index
                        for upstream_index in by_stage.get("resource", [])
                        if any(
                            stage_reaches(affected, stage)
                            for affected in resource_affects_by_index[upstream_index]
                        )
                    ]
                elif current_ranges:
                    candidate_indices: set[int] = set()
                    for start, end in current_ranges:
                        first_bucket = start // bucket_size
                        last_bucket = max(start, end - 1) // bucket_size
                        for bucket in range(first_bucket, last_bucket + 1):
                            candidate_indices.update(
                                interval_buckets[dependency].get(bucket, [])
                            )
                    closest = [
                        upstream_index
                        for upstream_index in candidate_indices
                        if ranges_intersect(
                            current_ranges,
                            ranges_by_index[upstream_index],
                        )
                    ]
                if closest:
                    break
            if not closest:
                change["causal_status"] = "root"
                change["cause_change_ids"] = []
                change["causal_basis"] = []
                continue
            closest.sort(key=lambda item: str(changes[item]["change_id"]))
            change["causal_status"] = "propagated_candidate"
            change["cause_change_ids"] = [
                changes[item]["change_id"] for item in closest[:20]
            ]
            change["causal_basis"] = ["declared_dependency", "range_overlap"]
            change["causal_confidence"] = "candidate"


def root_impact_summary(changes: Sequence[dict[str, Any]]) -> list[dict[str, Any]]:
    by_id = {change["change_id"]: change for change in changes}
    affected_counts: Counter[str] = Counter()
    affected_stages: dict[str, set[str]] = defaultdict(set)
    root_cache: dict[str, frozenset[str]] = {}

    def roots(change: dict[str, Any], visiting: set[str]) -> frozenset[str]:
        change_id = change["change_id"]
        if change_id in root_cache:
            return root_cache[change_id]
        if change_id in visiting:
            return frozenset()
        causes = change.get("cause_change_ids", [])
        if not causes:
            result = frozenset((change_id,))
            root_cache[change_id] = result
            return result
        result: set[str] = set()
        for cause_id in causes:
            cause = by_id.get(cause_id)
            if cause is not None:
                result.update(roots(cause, visiting | {change_id}))
        resolved = frozenset(result or {change_id})
        root_cache[change_id] = resolved
        return resolved

    for change in changes:
        for root in roots(change, set()):
            if root != change["change_id"]:
                affected_counts[root] += 1
                affected_stages[root].add(str(change["stage"]))
    result = []
    for change in changes:
        if change.get("causal_status") != "root":
            continue
        change_id = str(change["change_id"])
        result.append(
            {
                "change_id": change_id,
                "stage": change["stage"],
                "type": change["type"],
                "context": change.get("context", ""),
                "affected_changes": affected_counts[change_id],
                "affected_stages": sorted(
                    affected_stages[change_id],
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
    before_entity_counts: dict[str, int],
    after_entity_counts: dict[str, int],
    before_covered: set[str],
    after_covered: set[str],
    changes: list[dict[str, Any]],
    contract_mismatches: list[dict[str, Any]],
    blocked_stages: set[str],
) -> dict[str, Any]:
    primary_changes = [change for change in changes if change.get("counted_in_primary")]
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
        evidence_stage_changes = [change for change in changes if change["stage"] == stage]
        stage_changes = [change for change in primary_changes if change["stage"] == stage]
        before_count = before_entity_counts.get(stage, 0)
        after_count = after_entity_counts.get(stage, 0)
        statistics = stage_change_statistics(before_count, after_count, stage_changes)
        evidence_statistics = stage_change_statistics(
            before_count, after_count, evidence_stage_changes
        )
        stage_rows.append(
            {
                "stage": stage,
                "coverage": coverage_status,
                "before_entities": before_count,
                "after_entities": after_count,
                "changes": len(stage_changes),
                "evidence_changes": len(evidence_stage_changes),
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
                "evidence_scopes": dict(
                    sorted(Counter(change["scope"] for change in evidence_stage_changes).items())
                ),
                "evidence_churn": evidence_statistics["churn"],
                **statistics,
            }
        )
    type_counts = Counter(change["type"] for change in primary_changes)
    evidence_type_counts = Counter(change["type"] for change in changes)
    severity_counts = Counter(change["severity"] for change in primary_changes)
    causal_counts = Counter(change.get("causal_status", "unknown") for change in changes)
    primary_causal_counts = Counter(
        change.get("causal_status", "unknown") for change in primary_changes
    )
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
    evidence_comparison_units = sum(
        row["evidence_churn"]["comparison_units"] for row in stage_rows
    )
    evidence_changed_units = sum(
        row["evidence_churn"]["changed_units"] for row in stage_rows
    )
    scope_counts = Counter(change["scope"] for change in primary_changes)
    evidence_scope_counts = Counter(change["scope"] for change in changes)
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
        "changes": len(primary_changes),
        "evidence_changes": len(changes),
        "root_changes": causal_counts.get("root", 0),
        "propagated_candidates": causal_counts.get("propagated_candidate", 0),
        "meaningful_root_changes": primary_causal_counts.get("root", 0),
        "meaningful_propagated_candidates": primary_causal_counts.get("propagated_candidate", 0),
        "change_types": dict(sorted(type_counts.items())),
        "evidence_change_types": dict(sorted(evidence_type_counts.items())),
        "severities": {
            key: severity_counts.get(key, 0)
            for key in sorted(SEVERITY_ORDER, key=SEVERITY_ORDER.get)
        },
        "causal_statuses": dict(sorted(causal_counts.items())),
        "churn": {
            "changed_units": changed_units,
            "comparison_units": comparison_units,
            "rate": round(changed_units / comparison_units, 8) if comparison_units else 0.0,
            "ci95": wilson_interval(changed_units, comparison_units),
            "interval_method": "wilson_entity_units",
        },
        "evidence_churn": {
            "changed_units": evidence_changed_units,
            "comparison_units": evidence_comparison_units,
            "rate": round(evidence_changed_units / evidence_comparison_units, 8) if evidence_comparison_units else 0.0,
            "ci95": wilson_interval(evidence_changed_units, evidence_comparison_units),
            "interval_method": "wilson_entity_units",
        },
        "scope_counts": dict(sorted(scope_counts.items())),
        "scope_rates": {
            scope: round(count / len(primary_changes), 8) if primary_changes else 0.0
            for scope, count in sorted(scope_counts.items())
        },
        "evidence_scope_counts": dict(sorted(evidence_scope_counts.items())),
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


def _link_or_copy(source: Path, target: Path) -> None:
    """优先复用不可变缓存文件，跨卷时退回普通复制。"""
    try:
        os.link(source, target)
    except OSError:
        shutil.copy2(source, target)


def _candidate_cache_directory(
    spool_parent: Path | None,
    accelerator: Path,
    before_path: Path,
    after_path: Path,
) -> Path | None:
    if spool_parent is None:
        return None
    key = content_hash(
        {
            "schema_version": CANDIDATE_CACHE_SCHEMA_VERSION,
            "accelerator_sha256": file_hash(accelerator),
            "python_implementation_sha256": file_hash(Path(__file__)),
            "before_manifest_sha256": file_hash(before_path),
            "after_manifest_sha256": file_hash(after_path),
        }
    )
    return spool_parent.parent / ".cache" / "candidates-v1" / key


def _restore_candidate_cache(
    cache_directory: Path | None,
    candidates_path: Path,
    metadata_path: Path,
) -> bool:
    if cache_directory is None:
        return False
    cached_candidates = cache_directory / "candidates.jsonl"
    cached_metadata = cache_directory / "metadata.json"
    if not cached_candidates.is_file() or not cached_metadata.is_file():
        return False
    _link_or_copy(cached_candidates, candidates_path)
    _link_or_copy(cached_metadata, metadata_path)
    return True


def _store_candidate_cache(
    cache_directory: Path | None,
    candidates_path: Path,
    metadata_path: Path,
) -> None:
    if cache_directory is None:
        return
    if (cache_directory / "candidates.jsonl").is_file() and (
        cache_directory / "metadata.json"
    ).is_file():
        return
    cache_root = cache_directory.parent
    cache_root.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=".candidate-cache-", dir=cache_root))
    try:
        shutil.copy2(candidates_path, staging / "candidates.jsonl")
        shutil.copy2(metadata_path, staging / "metadata.json")
        try:
            os.replace(staging, cache_directory)
        except FileExistsError:
            shutil.rmtree(staging)
    except BaseException:
        if staging.exists():
            shutil.rmtree(staging)
        raise


def _reading_cache_paths(cache_directory: Path, side: str) -> tuple[Path, Path]:
    return (
        cache_directory / f"reading-{side}.jsonl",
        cache_directory / f"reading-{side}-metadata.json",
    )


def _restore_reading_cache(
    cache_directory: Path | None,
    side: str,
    output_path: Path,
    metadata_path: Path,
) -> bool:
    if cache_directory is None:
        return False
    cached_output, cached_metadata = _reading_cache_paths(cache_directory, side)
    if not cached_output.is_file() or not cached_metadata.is_file():
        return False
    _link_or_copy(cached_output, output_path)
    _link_or_copy(cached_metadata, metadata_path)
    return True


def _store_reading_cache(
    cache_directory: Path | None,
    side: str,
    output_path: Path,
    metadata_path: Path,
) -> None:
    if cache_directory is None:
        return
    cached_output, cached_metadata = _reading_cache_paths(cache_directory, side)
    if cached_output.is_file() and cached_metadata.is_file():
        return
    cache_directory.mkdir(parents=True, exist_ok=True)
    for source, target in (
        (output_path, cached_output),
        (metadata_path, cached_metadata),
    ):
        if target.exists():
            continue
        staging = target.with_name(f".{target.name}.{os.getpid()}.tmp")
        shutil.copy2(source, staging)
        try:
            os.replace(staging, target)
        except FileExistsError:
            staging.unlink(missing_ok=True)


def _snapshot_tokens(manifest_path: Path) -> Iterable[dict[str, Any]]:
    manifest = read_json(manifest_path)
    descriptor = manifest.get("artifacts", {}).get("tokens")
    if not isinstance(descriptor, dict):
        return ()
    return iter_snapshot_artifact(manifest_path, "tokens", descriptor)


def _sentence_spans(text: str) -> list[tuple[int, int]]:
    """按日文句末标点建立阅读句坐标，保留连续标点和闭合引号。"""
    spans: list[tuple[int, int]] = []
    start = 0
    closers = set("」』）】》〉〕］］”’")
    terminal_marks = set("。！？!?")
    for index, char in enumerate(text):
        if index < start:
            continue
        if char == "\n":
            end = index + 1
        elif char in terminal_marks:
            end = index + 1
            while end < len(text) and text[end] in terminal_marks:
                end += 1
        else:
            continue
        while end < len(text) and text[end] in closers:
            end += 1
        spans.append((start, end))
        start = end
    if start < len(text):
        spans.append((start, len(text)))
    return spans


def _token_record(token: dict[str, Any]) -> dict[str, Any] | None:
    bunsetsu = token.get("bunsetsu")
    if not isinstance(bunsetsu, dict):
        return None
    char_range = normalized_range(bunsetsu.get("char_range"))
    if char_range is None:
        return None
    # 只保留阅读和查询所需字段；画像分数不属于内容 diff。
    lexical = (bunsetsu.get("lexical_units") or [None])[0]
    formations = bunsetsu.get("word_formations") or []
    morphemes = bunsetsu.get("morphemes") or []
    if isinstance(lexical, dict):
        query = {
            "word": lexical.get("base_form", ""),
            "observed_form": lexical.get("base_form", ""),
            "reading": lexical.get("reading"),
            "pos": lexical.get("output_pos"),
        }
    elif formations and isinstance(formations[0], dict) and isinstance(formations[0].get("head_morpheme"), int) and formations[0]["head_morpheme"] < len(morphemes):
        morpheme = morphemes[formations[0]["head_morpheme"]]
        pos = morpheme.get("pos") or {}
        word = morpheme.get("surface") if pos.get("major") in {"助詞", "助動詞"} or (pos.get("major") == "動詞" and pos.get("sub1") == "接尾") else (morpheme.get("base_form") if morpheme.get("base_form") not in (None, "*") else morpheme.get("surface"))
        query = {"word": word or "", "observed_form": word or "", "reading": morpheme.get("reading"), "pos": pos}
    else:
        head = bunsetsu.get("head_word") or {}
        pos = head.get("pos") or {}
        word = head.get("surface") if pos.get("major") in {"助詞", "助動詞"} or (pos.get("major") == "動詞" and pos.get("sub1") == "接尾") else (head.get("base_form") if head.get("base_form") not in (None, "*") else head.get("surface"))
        query = {"word": word or "", "observed_form": word or "", "reading": head.get("reading"), "pos": pos}
    request_id = content_hash(query)[:20]
    return {
        "surface": bunsetsu.get("surface", ""),
        "char_range": list(char_range),
        "head_word": bunsetsu.get("head_word"),
        "morphemes": bunsetsu.get("morphemes", []),
        "morphology": bunsetsu.get("morphology", {"chains": []}),
        "word_formations": bunsetsu.get("word_formations", []),
        "lexical_units": bunsetsu.get("lexical_units", []),
        "grammar_occurrences": bunsetsu.get("grammar_occurrences", []),
        "grammar_tags": bunsetsu.get("grammar_tags", []),
        "functional_residuals": bunsetsu.get("functional_residuals", []),
        "function": bunsetsu.get("function"),
        "expressions": token.get("expressions", []),
        "display_class": token.get("display_class", "content"),
        "lookup_request_id": request_id,
        "lookup_request": query,
    }


def _project_raw_reading_sentence(sentence: dict[str, Any]) -> dict[str, Any]:
    """兼容旧 raw spool 与 Rust 已裁剪的阅读记录。"""
    records: list[dict[str, Any]] = []
    for token in sentence.get("tokens", []):
        if not isinstance(token, dict):
            continue
        if isinstance(token.get("lookup_request"), dict) and "bunsetsu" not in token:
            record = dict(token)
            record.setdefault("lookup_request_id", content_hash(record["lookup_request"])[:20])
        else:
            record = _token_record(token)
        if record is not None:
            records.append(record)
    return {
        **sentence,
        "tokens": records,
        "text": "".join(str(token.get("surface", "")) for token in records),
    }


def _reading_sentence_headers(manifest_path: Path) -> list[dict[str, Any]]:
    """流式切句，仅保留坐标和摘要，正文在受影响句二次扫描时再物化。"""
    headers: list[dict[str, Any]] = []
    terminal_marks = set("。！？!?")
    closers = set("」』）】》〉〕］］”’")
    current: list[str] = []
    start: int | None = None
    end = 0
    terminal_pending = False

    def emit() -> None:
        nonlocal current, start, terminal_pending
        if start is None or not current:
            return
        text = "".join(current)
        headers.append(
            {
                "index": len(headers),
                "char_range": [start, end],
                "text_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
            }
        )
        current = []
        start = None
        terminal_pending = False

    for token in _snapshot_tokens(manifest_path):
        bunsetsu = token.get("bunsetsu")
        if not isinstance(bunsetsu, dict):
            continue
        token_range = normalized_range(bunsetsu.get("char_range"))
        if token_range is None:
            continue
        surface = str(bunsetsu.get("surface", ""))
        for offset, char in enumerate(surface, start=token_range[0]):
            if terminal_pending and char not in terminal_marks and char not in closers:
                emit()
            if start is None:
                start = offset
            current.append(char)
            end = offset + 1
            if char == "\n" or char in terminal_marks:
                terminal_pending = True
    emit()
    return headers


def _sentence_for_range(
    headers: Sequence[dict[str, Any]],
    raw: tuple[int, int],
    starts: Sequence[int] | None = None,
) -> dict[str, Any] | None:
    if not headers:
        return None
    if starts is None:
        starts = [int(item["char_range"][0]) for item in headers]
    position = max(0, bisect.bisect_right(starts, raw[0]) - 1)
    candidates = [
        headers[index]
        for index in range(max(0, position - 1), min(len(headers), position + 3))
        if ranges_intersect([raw], [tuple(headers[index]["char_range"])])
    ]
    if not candidates:
        return None
    return min(
        candidates,
        key=lambda item: abs(int(item["char_range"][0]) - raw[0]),
    )


def _reading_sentences(
    manifest_path: Path,
    headers: Sequence[dict[str, Any]],
    indices: set[int],
) -> list[dict[str, Any]]:
    """二次扫描只物化受影响句子的原生分析 token。"""
    selected = [
        {
            **header,
            "tokens": [],
        }
        for header in headers
        if int(header["index"]) in indices
    ]
    if not selected:
        return []
    cursor = 0
    for token in _snapshot_tokens(manifest_path):
        bunsetsu = token.get("bunsetsu")
        if not isinstance(bunsetsu, dict):
            continue
        token_range = normalized_range(bunsetsu.get("char_range"))
        if token_range is None:
            continue
        while (
            cursor < len(selected)
            and int(selected[cursor]["char_range"][1]) <= token_range[0]
        ):
            cursor += 1
        if cursor >= len(selected):
            break
        index = cursor
        record: dict[str, Any] | None = None
        while (
            index < len(selected)
            and int(selected[index]["char_range"][0]) < token_range[1]
        ):
            sentence_range = tuple(selected[index]["char_range"])
            if ranges_intersect([token_range], [sentence_range]):
                if record is None:
                    record = _token_record(token)
                if record is not None:
                    selected[index]["tokens"].append(record)
            index += 1
    for sentence in selected:
        sentence["text"] = "".join(
            str(token.get("surface", "")) for token in sentence["tokens"]
        )
    return selected


def _merge_ranges(ranges: Sequence[tuple[int, int]]) -> list[tuple[int, int]]:
    merged: list[list[int]] = []
    for start, end in sorted(ranges):
        if not merged or start > merged[-1][1]:
            merged.append([start, end])
        else:
            merged[-1][1] = max(merged[-1][1], end)
    return [tuple(item) for item in merged]


def _is_reading_separator(text: str) -> bool:
    return all(
        char.isspace() or unicodedata.category(char).startswith(("P", "Z"))
        for char in text
    )


def _merge_reading_ranges(
    ranges: Sequence[tuple[int, int]],
    sentence: dict[str, Any],
) -> list[tuple[tuple[int, int], list[tuple[int, int]]]]:
    """跨纯标点或空白合并连续阅读变化，同时保留精确着色范围。"""
    exact_ranges = _merge_ranges(ranges)
    if not exact_ranges:
        return []
    sentence_start = int(sentence["char_range"][0])
    text = str(sentence.get("text", ""))
    groups: list[list[tuple[int, int]]] = [[exact_ranges[0]]]
    for current in exact_ranges[1:]:
        previous = groups[-1][-1]
        gap_start = max(0, previous[1] - sentence_start)
        gap_end = max(gap_start, current[0] - sentence_start)
        gap = text[gap_start:gap_end]
        if gap and _is_reading_separator(gap):
            groups[-1].append(current)
        else:
            groups.append([current])
    return [
        ((group[0][0], group[-1][1]), group)
        for group in groups
    ]


def _reading_unit_payload(
    before_manifest_path: Path,
    after_manifest_path: Path,
    changes: Sequence[dict[str, Any]],
    before_headers: list[dict[str, Any]] | None = None,
    after_headers: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    if before_headers is None or after_headers is None:
        with ThreadPoolExecutor(max_workers=2) as executor:
            before_future = executor.submit(
                _reading_sentence_headers, before_manifest_path
            )
            after_future = executor.submit(
                _reading_sentence_headers, after_manifest_path
            )
            before_headers = before_future.result()
            after_headers = after_future.result()
    if not before_headers and not after_headers:
        return {"schema_version": "kotoclip.quality.reading-diff.v1", "units": []}

    primary = [change for change in changes if change.get("counted_in_primary")]
    before_starts = [int(item["char_range"][0]) for item in before_headers]
    after_starts = [int(item["char_range"][0]) for item in after_headers]
    evidence_by_sentence: dict[int, list[dict[str, Any]]] = defaultdict(list)
    primary_ranges_by_sentence: dict[
        int, list[tuple[tuple[int, int], dict[str, Any]]]
    ] = defaultdict(list)
    for change in primary:
        for raw in change_ranges(change):
            sentence = _sentence_for_range(after_headers, raw, after_starts)
            if sentence is None:
                sentence = _sentence_for_range(before_headers, raw, before_starts)
            if sentence is None:
                continue
            sentence_index = int(sentence["index"])
            primary_ranges_by_sentence[sentence_index].append((raw, change))

    affected_indices = set(primary_ranges_by_sentence)
    after_header_by_index = {
        int(item["index"]): item
        for item in after_headers
    }
    before_by_range = {
        tuple(item["char_range"]): item
        for item in before_headers
    }
    before_index_for_after: dict[int, int] = {}
    for sentence_index in affected_indices:
        after_header = after_header_by_index.get(sentence_index)
        before_header = (
            before_by_range.get(tuple(after_header["char_range"]))
            if after_header is not None
            else None
        )
        if before_header is None and sentence_index < len(before_headers):
            before_header = before_headers[sentence_index]
        if before_header is not None:
            before_index_for_after[sentence_index] = int(before_header["index"])
    with ThreadPoolExecutor(max_workers=2) as executor:
        after_future = executor.submit(
            _reading_sentences,
            after_manifest_path,
            after_headers,
            affected_indices,
        )
        before_future = executor.submit(
            _reading_sentences,
            before_manifest_path,
            before_headers,
            set(before_index_for_after.values()),
        )
        after_sentences = after_future.result()
        before_sentences = before_future.result()
    after_by_index = {int(item["index"]): item for item in after_sentences}
    raw_before_by_index = {int(item["index"]): item for item in before_sentences}
    before_by_after_index = {
        after_index: raw_before_by_index[before_index]
        for after_index, before_index in before_index_for_after.items()
        if before_index in raw_before_by_index
    }

    # 证据只需在同一句内下钻，避免每个条目重新扫描完整变化集合。
    for change in changes:
        sentence_indices = {
            int(sentence["index"])
            for raw in change_ranges(change)
            if (
                sentence := _sentence_for_range(after_headers, raw, after_starts)
                or _sentence_for_range(before_headers, raw, before_starts)
            )
            is not None
        }
        for sentence_index in sentence_indices & affected_indices:
            evidence_by_sentence[sentence_index].append(change)

    spans_by_sentence: dict[int, list[tuple[int, int]]] = defaultdict(list)
    for sentence_index, items in primary_ranges_by_sentence.items():
        sentence = after_by_index.get(sentence_index) or before_by_after_index.get(
            sentence_index
        )
        if sentence is None:
            continue
        for raw, _ in items:
            token_ranges = [
                tuple(token["char_range"])
                for token in sentence.get("tokens", [])
                if ranges_intersect([raw], [tuple(token["char_range"])])
            ]
            spans_by_sentence[sentence_index].append(
                (
                    min((item[0] for item in token_ranges), default=raw[0]),
                    max((item[1] for item in token_ranges), default=raw[1]),
                )
            )

    units: list[dict[str, Any]] = []
    for sentence_index, changed_ranges in sorted(spans_by_sentence.items()):
        sentence_after = after_by_index.get(sentence_index)
        sentence_before = before_by_after_index.get(sentence_index)
        if sentence_after is None:
            continue
        groups = _merge_reading_ranges(changed_ranges, sentence_after)
        sentence_changes = [
            change
            for _, change in primary_ranges_by_sentence[sentence_index]
        ]
        for group_index, (group, exact_ranges) in enumerate(groups):
            group_changes = list(
                {
                    change["change_id"]: change
                    for change in sentence_changes
                    if ranges_intersect(change_ranges(change), [group])
                }.values()
            )
            evidence_changes = list(
                {
                    change["change_id"]: change
                    for change in evidence_by_sentence[sentence_index]
                    if ranges_intersect(change_ranges(change), [group])
                }.values()
            )
            domain_counts = Counter(
                str(change["primary_domain"])
                for change in group_changes
                if change.get("primary_domain")
            )
            unit_id = f"sentence-{sentence_index}-span-{group[0]}-{group[1]}-{group_index}"
            units.append(
                {
                    "unit_id": unit_id,
                    "sentence_index": sentence_index,
                    "changed_range": list(group),
                    "changed_ranges": [list(item) for item in exact_ranges],
                    "primary_change_count": len(group_changes),
                    "evidence_change_count": len(evidence_changes),
                    "domains": dict(sorted(domain_counts.items())),
                    "stages": sorted(
                        {str(change["stage"]) for change in group_changes},
                        key=lambda stage: STAGE_INDEX.get(stage, 999),
                    ),
                    "change_ids": [change["change_id"] for change in group_changes],
                    "evidence_change_ids": [
                        change["change_id"] for change in evidence_changes
                    ],
                    "before": {
                        "char_range": (sentence_before or sentence_after)["char_range"],
                        "text": (sentence_before or sentence_after).get("text", ""),
                        "tokens": (sentence_before or {"tokens": []})["tokens"],
                    },
                    "after": {
                        "char_range": sentence_after["char_range"],
                        "text": sentence_after.get("text", ""),
                        "tokens": sentence_after["tokens"],
                    },
                }
            )
    return {
        "schema_version": "kotoclip.quality.reading-diff.v1",
        "unit_count": len(units),
        "units": units,
    }


def _write_reading_unit_payload(
    before_manifest_path: Path,
    after_manifest_path: Path,
    changes: Sequence[dict[str, Any]],
    output_path: Path,
    accelerator: Path | None = None,
    reading_cache_directory: Path | None = None,
    before_scan: tuple[
        list[dict[str, Any]],
        dict[int, tuple[int, int]],
        dict[int, list[tuple[int, int]]],
        Path,
    ]
    | None = None,
    after_scan: tuple[
        list[dict[str, Any]],
        dict[int, tuple[int, int]],
        dict[int, list[tuple[int, int]]],
        Path,
    ]
    | None = None,
) -> dict[str, Any]:
    """单侧单次扫描并将阅读条目直接写入 gzip，避免完整 token 列表常驻内存。"""
    primary = [change for change in changes if change.get("counted_in_primary")]
    primary_ranges = [
        raw
        for change in primary
        for raw in change_ranges(change)
    ]
    with tempfile.TemporaryDirectory(prefix=".quality-reading-") as temporary:
        temporary_root = Path(temporary)
        before_token_ranges: dict[int, list[tuple[int, int]]] = {}
        after_token_ranges: dict[int, list[tuple[int, int]]] = {}
        before_source_path = temporary_root / "before.jsonl"
        after_source_path = temporary_root / "after.jsonl"
        raw_token_spool = before_scan is not None and after_scan is not None
        if before_scan is not None and after_scan is not None:
            (
                before_headers,
                before_index,
                before_token_ranges,
                before_source_path,
            ) = before_scan
            (
                after_headers,
                after_index,
                after_token_ranges,
                after_source_path,
            ) = after_scan
        elif accelerator is not None:
            ranges_path = temporary_root / "ranges.json"
            ranges_path.write_text(
                json.dumps(primary_ranges, ensure_ascii=False, separators=(",", ":")),
                encoding="utf-8",
            )
            before_metadata_path = temporary_root / "before-metadata.json"
            after_metadata_path = temporary_root / "after-metadata.json"
            before_cached = _restore_reading_cache(
                reading_cache_directory,
                "before",
                before_source_path,
                before_metadata_path,
            )
            after_cached = _restore_reading_cache(
                reading_cache_directory,
                "after",
                after_source_path,
                after_metadata_path,
            )
            if before_cached and after_cached:
                raw_token_spool = bool(
                    read_json(before_metadata_path).get("raw_tokens", False)
                ) and bool(read_json(after_metadata_path).get("raw_tokens", False))
            with ThreadPoolExecutor(max_workers=2) as executor:
                before_future = executor.submit(
                    _scan_reading_sentences_accelerated,
                    accelerator,
                    before_manifest_path,
                    ranges_path,
                    before_source_path,
                    before_metadata_path,
                ) if not before_cached else None
                after_future = executor.submit(
                    _scan_reading_sentences_accelerated,
                    accelerator,
                    after_manifest_path,
                    ranges_path,
                    after_source_path,
                    after_metadata_path,
                ) if not after_cached else None
                if before_future is None:
                    before_headers, before_index = _scan_reading_sentences_metadata(
                        before_metadata_path
                    )
                else:
                    before_headers, before_index = before_future.result()
                    _store_reading_cache(
                        reading_cache_directory,
                        "before",
                        before_source_path,
                        before_metadata_path,
                    )
                if after_future is None:
                    after_headers, after_index = _scan_reading_sentences_metadata(
                        after_metadata_path
                    )
                else:
                    after_headers, after_index = after_future.result()
                    _store_reading_cache(
                        reading_cache_directory,
                        "after",
                        after_source_path,
                        after_metadata_path,
                    )
        else:
            before_headers, before_index = _scan_reading_sentences(
                before_manifest_path,
                primary_ranges,
                before_source_path,
            )
            after_headers, after_index = _scan_reading_sentences(
                after_manifest_path,
                primary_ranges,
                after_source_path,
            )
        if not before_headers and not after_headers:
            write_gzip_json(
                output_path,
                {"schema_version": "kotoclip.quality.reading-diff.v1", "units": []},
            )
            return {
                "schema_version": "kotoclip.quality.reading-diff.v1",
                "unit_count": 0,
                "affected_sentences": 0,
                "domain_counts": {},
            }
        before_starts = [int(item["char_range"][0]) for item in before_headers]
        after_starts = [int(item["char_range"][0]) for item in after_headers]
        primary_ranges_by_sentence: dict[
            int, list[tuple[tuple[int, int], dict[str, Any]]]
        ] = defaultdict(list)
        for change in primary:
            for raw in change_ranges(change):
                sentence = _sentence_for_range(after_headers, raw, after_starts)
                if sentence is None:
                    sentence = _sentence_for_range(before_headers, raw, before_starts)
                if sentence is not None:
                    primary_ranges_by_sentence[int(sentence["index"])].append((raw, change))
        affected_indices = set(primary_ranges_by_sentence)
        after_header_by_index = {int(item["index"]): item for item in after_headers}
        before_by_range = {tuple(item["char_range"]): item for item in before_headers}
        before_index_for_after: dict[int, int] = {}
        for sentence_index in affected_indices:
            after_header = after_header_by_index.get(sentence_index)
            before_header = (
                before_by_range.get(tuple(after_header["char_range"]))
                if after_header is not None
                else None
            )
            if before_header is None and sentence_index < len(before_headers):
                before_header = before_headers[sentence_index]
            if before_header is not None:
                before_index_for_after[sentence_index] = int(before_header["index"])
        evidence_by_sentence: dict[int, list[dict[str, Any]]] = defaultdict(list)
        for change in changes:
            sentence_indices = {
                int(sentence["index"])
                for raw in change_ranges(change)
                if (
                    sentence := _sentence_for_range(after_headers, raw, after_starts)
                    or _sentence_for_range(before_headers, raw, before_starts)
                )
                is not None
            }
            for sentence_index in sentence_indices & affected_indices:
                evidence_by_sentence[sentence_index].append(change)

        unit_count = 0
        affected_sentences = 0
        domain_counts: Counter[str] = Counter()
        index_units: list[dict[str, Any]] = []
        output_path.parent.mkdir(parents=True, exist_ok=True)
        with output_path.open("wb") as output_stream, (
            output_path.parent / "reading-units.bin"
        ).open("wb") as unit_output:
            with gzip.GzipFile(
                fileobj=output_stream,
                mode="wb",
                compresslevel=OUTPUT_GZIP_LEVEL,
                mtime=0,
            ) as compressed:
                with io.TextIOWrapper(compressed, encoding="utf-8", newline="\n") as target:
                    target.write('{"schema_version":"kotoclip.quality.reading-diff.v1","units":[')
                    first_unit = True
                    before_source = before_source_path.open("rb")
                    after_source = after_source_path.open("rb")
                    try:
                        for sentence_index in sorted(affected_indices):
                            sentence_after = _read_scanned_sentence(
                                after_source, after_index, sentence_index
                            )
                            before_sentence_index = before_index_for_after.get(
                                sentence_index
                            )
                            sentence_before = (
                                _read_scanned_sentence(
                                    before_source,
                                    before_index,
                                    before_sentence_index,
                                )
                                if before_sentence_index is not None
                                else None
                            )
                            if sentence_after is None:
                                continue
                            if raw_token_spool:
                                sentence_after = _project_raw_reading_sentence(sentence_after)
                                if sentence_before is not None:
                                    sentence_before = _project_raw_reading_sentence(
                                        sentence_before
                                    )
                            spans: list[tuple[int, int]] = []
                            for raw, _ in primary_ranges_by_sentence[sentence_index]:
                                token_ranges = [
                                    tuple(token["char_range"])
                                    for token in sentence_after.get("tokens", [])
                                    if ranges_intersect(
                                        [raw], [tuple(token["char_range"])]
                                    )
                                ]
                                spans.append(
                                    (
                                        min((item[0] for item in token_ranges), default=raw[0]),
                                        max((item[1] for item in token_ranges), default=raw[1]),
                                    )
                                )
                            groups = _merge_reading_ranges(spans, sentence_after)
                            sentence_changes = [
                                change
                                for _, change in primary_ranges_by_sentence[sentence_index]
                            ]
                            wrote_sentence = False
                            for group_index, (group, exact_ranges) in enumerate(groups):
                                group_changes = list(
                                    {
                                        change["change_id"]: change
                                        for change in sentence_changes
                                        if ranges_intersect(change_ranges(change), [group])
                                    }.values()
                                )
                                evidence_changes = list(
                                    {
                                        change["change_id"]: change
                                        for change in evidence_by_sentence[sentence_index]
                                        if ranges_intersect(change_ranges(change), [group])
                                    }.values()
                                )
                                domains = dict(
                                    sorted(
                                        Counter(
                                            str(change["primary_domain"])
                                            for change in group_changes
                                            if change.get("primary_domain")
                                        ).items()
                                    )
                                )
                                unit = {
                                    "unit_id": f"sentence-{sentence_index}-span-{group[0]}-{group[1]}-{group_index}",
                                    "sentence_index": sentence_index,
                                    "changed_range": list(group),
                                    "changed_ranges": [list(item) for item in exact_ranges],
                                    "primary_change_count": len(group_changes),
                                    "evidence_change_count": len(evidence_changes),
                                    "domains": domains,
                                    "stages": sorted(
                                        {str(change["stage"]) for change in group_changes},
                                        key=lambda stage: STAGE_INDEX.get(stage, 999),
                                    ),
                                    "change_ids": [change["change_id"] for change in group_changes],
                                    "evidence_change_ids": [
                                        change["change_id"] for change in evidence_changes
                                    ],
                                    "before": {
                                        "char_range": (sentence_before or sentence_after)[
                                            "char_range"
                                        ],
                                        "text": (sentence_before or sentence_after).get(
                                            "text", ""
                                        ),
                                        "tokens": (sentence_before or {"tokens": []})[
                                            "tokens"
                                        ],
                                    },
                                    "after": {
                                        "char_range": sentence_after["char_range"],
                                        "text": sentence_after.get("text", ""),
                                        "tokens": sentence_after["tokens"],
                                    },
                                }
                                if not first_unit:
                                    target.write(",")
                                json.dump(
                                    unit,
                                    target,
                                    ensure_ascii=False,
                                    sort_keys=True,
                                    separators=(",", ":"),
                                )
                                encoded = canonical_json(unit).encode("utf-8")
                                member = gzip.compress(
                                    encoded,
                                    compresslevel=OUTPUT_GZIP_LEVEL,
                                    mtime=0,
                                )
                                offset = unit_output.tell()
                                unit_output.write(member)
                                index_units.append(
                                    {
                                        "unit_id": unit["unit_id"],
                                        "sentence_index": unit["sentence_index"],
                                        "changed_range": unit["changed_range"],
                                        "changed_ranges": unit["changed_ranges"],
                                        "primary_change_count": unit[
                                            "primary_change_count"
                                        ],
                                        "evidence_change_count": unit[
                                            "evidence_change_count"
                                        ],
                                        "domains": unit["domains"],
                                        "stages": unit["stages"],
                                        "before": {
                                            "char_range": unit["before"]["char_range"],
                                            "text": unit["before"]["text"],
                                        },
                                        "after": {
                                            "char_range": unit["after"]["char_range"],
                                            "text": unit["after"]["text"],
                                        },
                                        "offset": offset,
                                        "bytes": len(member),
                                    }
                                )
                                first_unit = False
                                wrote_sentence = True
                                unit_count += 1
                                domain_counts.update(domains)
                            if wrote_sentence:
                                affected_sentences += 1
                    finally:
                        before_source.close()
                        after_source.close()
                    target.write(
                        f'],"unit_count":{unit_count}}}\n'
                    )
        write_gzip_json(
            output_path.parent / "reading-index.json.gz",
            {
                "schema_version": "kotoclip.quality.reading-index.v1",
                "reading_schema_version": "kotoclip.quality.reading-diff.v1",
                "unit_count": unit_count,
                "units": index_units,
            },
        )
    return {
        "schema_version": "kotoclip.quality.reading-diff.v1",
        "unit_count": unit_count,
        "affected_sentences": affected_sentences,
        "domain_counts": dict(sorted(domain_counts.items())),
    }


def _scan_reading_sentences_accelerated(
    accelerator: Path,
    manifest_path: Path,
    ranges_path: Path,
    output_path: Path,
    metadata_path: Path,
) -> tuple[list[dict[str, Any]], dict[int, tuple[int, int]]]:
    completed = subprocess.run(
        [
            str(accelerator),
            "reading-sentences",
            "--manifest",
            str(manifest_path),
            "--ranges",
            str(ranges_path),
            "--output",
            str(output_path),
            "--metadata",
            str(metadata_path),
        ],
        text=True,
        encoding="utf-8",
        errors="backslashreplace",
        capture_output=True,
    )
    if completed.returncode:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(f"Rust 阅读句扫描失败：{detail}")
    return _scan_reading_sentences_metadata(metadata_path)


def _scan_reading_sentences_metadata(
    metadata_path: Path,
) -> tuple[list[dict[str, Any]], dict[int, tuple[int, int]]]:
    metadata = read_json(metadata_path)
    headers = metadata.get("headers", [])
    if not isinstance(headers, list):
        raise ValueError("Rust 阅读句 metadata.headers 必须是数组")
    positions = {
        int(item["index"]): (int(item["offset"]), int(item["bytes"]))
        for item in metadata.get("positions", [])
    }
    return headers, positions


def _scan_reading_sentences(
    manifest_path: Path,
    relevant_ranges: Sequence[tuple[int, int]],
    output_path: Path,
) -> tuple[list[dict[str, Any]], dict[int, tuple[int, int]]]:
    """构建全部句头，只把命中变化范围的句子按 JSONL 写入临时文件。"""
    headers: list[dict[str, Any]] = []
    positions: dict[int, tuple[int, int]] = {}
    terminal_marks = set("。！？!?")
    closers = set("」』）】》〉〕］］”’")
    current: list[str] = []
    pending_tokens: list[tuple[tuple[int, int], dict[str, Any]]] = []
    start: int | None = None
    end = 0
    terminal_pending = False
    sorted_relevant = sorted(relevant_ranges)
    relevant_cursor = 0
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("wb") as output:
        def emit() -> None:
            nonlocal current, pending_tokens, start, terminal_pending, relevant_cursor
            if start is None or not current:
                return
            sentence_range = (start, end)
            header = {
                "index": len(headers),
                "char_range": [start, end],
                "text_sha256": hashlib.sha256(
                    "".join(current).encode("utf-8")
                ).hexdigest(),
            }
            headers.append(header)
            while (
                relevant_cursor < len(sorted_relevant)
                and sorted_relevant[relevant_cursor][1] < sentence_range[0]
            ):
                relevant_cursor += 1
            selected = False
            probe = relevant_cursor
            while (
                probe < len(sorted_relevant)
                and sorted_relevant[probe][0] <= sentence_range[1]
            ):
                if ranges_intersect([sentence_range], [sorted_relevant[probe]]):
                    selected = True
                    break
                probe += 1
            if selected:
                tokens = [
                    record
                    for token_range, record in pending_tokens
                    if ranges_intersect([token_range], [sentence_range])
                ]
                sentence = {
                    **header,
                    "tokens": tokens,
                    "text": "".join(
                        str(token.get("surface", "")) for token in tokens
                    ),
                }
                line = canonical_json(sentence).encode("utf-8") + b"\n"
                positions[int(header["index"])] = (output.tell(), len(line))
                output.write(line)
            pending_tokens = [
                item for item in pending_tokens if item[0][1] > end
            ]
            current = []
            start = None
            terminal_pending = False

        for token in _snapshot_tokens(manifest_path):
            bunsetsu = token.get("bunsetsu")
            if not isinstance(bunsetsu, dict):
                continue
            token_range = normalized_range(bunsetsu.get("char_range"))
            if token_range is None:
                continue
            record = _token_record(token)
            if record is not None:
                pending_tokens.append((token_range, record))
            surface = str(bunsetsu.get("surface", ""))
            for offset, char in enumerate(surface, start=token_range[0]):
                if terminal_pending and char not in terminal_marks and char not in closers:
                    emit()
                if start is None:
                    start = offset
                current.append(char)
                end = offset + 1
                if char == "\n" or char in terminal_marks:
                    terminal_pending = True
        emit()
    return headers, positions


def _read_scanned_sentence(
    source: Any,
    positions: dict[int, tuple[int, int]],
    index: int | None,
) -> dict[str, Any] | None:
    if index is None or index not in positions:
        return None
    offset, size = positions[index]
    source.seek(offset)
    return json.loads(source.read(size))


def compare_snapshot_manifests(
    before_path: Path,
    after_path: Path,
    spool_parent: Path | None = None,
    diff_output_path: Path | None = None,
    memory_profile_path: Path | None = None,
    reading_output_path: Path | None = None,
) -> ComparisonBundle:
    accelerator = os.environ.get("KOTOCLIP_QUALITY_DIFF_ACCELERATOR")
    if accelerator:
        accelerated = _compare_snapshot_manifests_accelerated(
            Path(accelerator),
            before_path,
            after_path,
            spool_parent,
            diff_output_path,
            memory_profile_path,
            reading_output_path,
        )
        if accelerated is not None:
            return accelerated
    memory = MemoryProbe(memory_profile_path)
    memory.sample("started")
    if spool_parent is not None:
        spool_parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix=".quality-diff-spool-",
        dir=spool_parent,
    ) as temporary:
        spool_root = Path(temporary)
        before_root = spool_root / "before"
        after_root = spool_root / "after"
        with EntitySpool(before_root) as before_spool:
            before_manifest, before_entity_counts, before_covered = normalize_snapshot(
                before_path,
                before_spool,
            )
        gc.collect()
        memory.sample("before_normalized", entities=sum(before_entity_counts.values()))
        with EntitySpool(after_root) as after_spool:
            after_manifest, after_entity_counts, after_covered = normalize_snapshot(
                after_path,
                after_spool,
            )
        gc.collect()
        memory.sample("after_normalized", entities=sum(after_entity_counts.values()))

        mismatches = artifact_contract_mismatches(before_manifest, after_manifest)
        blocked_stages = {
            stage
            for mismatch in mismatches
            for stage in ARTIFACT_STAGE_COVERAGE.get(mismatch["artifact"], set())
        }
        changes: list[dict[str, Any]] = []
        raw_change_root = spool_root / "changes"
        raw_change_root.mkdir()
        for stage in STAGE_ORDER:
            if (
                stage in blocked_stages
                or stage not in before_covered
                or stage not in after_covered
            ):
                continue
            if diff_output_path is None:
                changes.extend(
                    _iter_stage_changes_from_spools(
                        stage, before_root, after_root, spool_root
                    )
                )
            else:
                with (raw_change_root / f"{stage}.jsonl").open(
                    "w",
                    encoding="utf-8",
                    newline="\n",
                ) as raw_stream:
                    for change in _iter_stage_changes_from_spools(
                        stage,
                        before_root,
                        after_root,
                        spool_root,
                    ):
                        raw_stream.write(canonical_json(change))
                        raw_stream.write("\n")
                        changes.append(compact_change(change))
            gc.collect()
            memory.sample("stage_compared", stage=stage, compact_changes=len(changes))
        annotate_causality(changes)
        annotate_primary_count(changes)
        memory.sample("causality_annotated", compact_changes=len(changes))
        changes.sort(
            key=lambda change: (
                STAGE_INDEX.get(change["stage"], 999),
                SEVERITY_ORDER.get(change["severity"], 99),
                span_of_ranges(change_ranges(change)) or (-1, -1),
                change["type"],
                change["change_id"],
            )
        )
        if diff_output_path is not None:
            write_annotated_diff(raw_change_root, changes, diff_output_path)
            memory.sample("diff_written", compact_changes=len(changes))
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
        "implementation": {
            "before": before_manifest.get("implementation", {}),
            "after": after_manifest.get("implementation", {}),
        },
        "stage_graph": [
            {"stage": stage, "depends_on": list(STAGE_DEPENDENCIES.get(stage, ()))}
            for stage in STAGE_ORDER
        ],
    }
    reading_units = (
        _write_reading_unit_payload(
            before_path,
            after_path,
            changes,
            reading_output_path,
        )
        if reading_output_path is not None
        else _reading_unit_payload(before_path, after_path, changes)
    )
    memory.sample("reading_projection_built", reading_units=reading_units.get("unit_count", 0))
    summary = pipeline_summary(
        before_entity_counts,
        after_entity_counts,
        before_covered,
        after_covered,
        changes,
        mismatches,
        blocked_stages,
    )
    summary["reading_units"] = reading_units.get("unit_count", 0)
    summary["affected_sentences"] = len(
        {item["sentence_index"] for item in reading_units.get("units", [])}
    ) if "affected_sentences" not in reading_units else reading_units["affected_sentences"]
    summary["reading_unit_domains"] = (
        dict(sorted(reading_units["domain_counts"].items()))
        if "domain_counts" in reading_units
        else dict(
            sorted(
                Counter(
                    domain
                    for item in reading_units.get("units", [])
                    for domain in item.get("domains", {})
                ).items()
            )
        )
    )
    memory.sample("summary_built", compact_changes=len(changes))
    memory.write()
    return ComparisonBundle(
        manifest=manifest,
        summary=summary,
        changes=changes,
        reading_units=reading_units,
        diff_prepared=diff_output_path is not None,
        reading_prepared=reading_output_path is not None,
    )


def _compare_snapshot_manifests_accelerated(
    accelerator: Path,
    before_path: Path,
    after_path: Path,
    spool_parent: Path | None,
    diff_output_path: Path | None,
    memory_profile_path: Path | None,
    reading_output_path: Path | None,
) -> ComparisonBundle | None:
    memory = MemoryProbe(memory_profile_path)
    memory.sample("started")
    before_manifest = read_json(before_path)
    after_manifest = read_json(after_path)
    if spool_parent is not None:
        spool_parent.mkdir(parents=True, exist_ok=True)
    prepared_reading: dict[str, Any] | None = None
    with tempfile.TemporaryDirectory(
        prefix=".quality-diff-rust-",
        dir=spool_parent,
    ) as temporary:
        temporary_root = Path(temporary)
        candidates_path = temporary_root / "candidates.jsonl"
        metadata_path = temporary_root / "metadata.json"
        before_reading_path = temporary_root / "reading-before.jsonl"
        after_reading_path = temporary_root / "reading-after.jsonl"
        cache_directory = _candidate_cache_directory(
            spool_parent, accelerator, before_path, after_path
        )
        candidate_command = [
            str(accelerator),
            "candidates",
            "--before-manifest",
            str(before_path),
            "--after-manifest",
            str(after_path),
            "--output",
            str(candidates_path),
            "--metadata",
            str(metadata_path),
            "--before-reading-output",
            str(before_reading_path),
            "--after-reading-output",
            str(after_reading_path),
        ]
        if spool_parent is not None:
            candidate_command.extend(
                [
                    "--count-cache",
                    str(spool_parent.parent / ".cache" / "artifact-counts-v1"),
                ]
            )
        candidate_cache_hit = _restore_candidate_cache(
            cache_directory, candidates_path, metadata_path
        )
        if not candidate_cache_hit:
            completed = subprocess.run(
                candidate_command,
                text=True,
                encoding="utf-8",
                errors="backslashreplace",
                capture_output=True,
            )
            if completed.returncode:
                print(
                    "Rust diff 候选提取失败，回落 Python："
                    + (completed.stderr.strip() or completed.stdout.strip()),
                    file=sys.stderr,
                )
                return None
        metadata = read_json(metadata_path)
        if not metadata.get("supported"):
            print(
                "Rust diff 尚未覆盖这些变化产物，回落 Python："
                + ", ".join(metadata.get("changed_artifacts", [])),
                file=sys.stderr,
            )
            return None
        if not candidate_cache_hit:
            _store_candidate_cache(cache_directory, candidates_path, metadata_path)
        before_entity_counts = {
            str(key): int(value)
            for key, value in metadata["partial_counts_before"].items()
        }
        after_entity_counts = {
            str(key): int(value)
            for key, value in metadata["partial_counts_after"].items()
        }
        reading_headers_before = metadata.get("reading_headers_before")
        reading_headers_after = metadata.get("reading_headers_after")
        before_covered = {"source", "preprocessing"}
        after_covered = {"source", "preprocessing"}
        for name in before_manifest.get("artifacts", {}):
            before_covered.update(ARTIFACT_STAGE_COVERAGE.get(name, set()))
        for name in after_manifest.get("artifacts", {}):
            after_covered.update(ARTIFACT_STAGE_COVERAGE.get(name, set()))
        if before_entity_counts.get("resource") or after_entity_counts.get("resource"):
            before_covered.add("resource")
            after_covered.add("resource")
        memory.sample(
            "rust_candidates_extracted",
            candidates=int(metadata.get("event_count", 0)),
            before_entities=sum(before_entity_counts.values()),
            after_entities=sum(after_entity_counts.values()),
            cache="hit" if candidate_cache_hit else "miss",
        )

        direct_pairs: dict[str, list[tuple[dict[str, Any], dict[str, Any]]]] = defaultdict(list)
        unmatched_before: dict[str, list[dict[str, Any]]] = defaultdict(list)
        unmatched_after: dict[str, list[dict[str, Any]]] = defaultdict(list)
        with candidates_path.open("r", encoding="utf-8") as source:
            for line in source:
                if not line.strip():
                    continue
                candidate = json.loads(line)
                kind = candidate["kind"]
                if kind == "pair":
                    before_entity = candidate["before"]
                    after_entity = candidate["after"]
                    direct_pairs[str(before_entity["stage"])].append(
                        (before_entity, after_entity)
                    )
                else:
                    entity = candidate["entity"]
                    target = unmatched_before if kind == "before" else unmatched_after
                    target[str(entity["stage"])].append(entity)

        mismatches = artifact_contract_mismatches(before_manifest, after_manifest)
        blocked_stages = {
            stage
            for mismatch in mismatches
            for stage in ARTIFACT_STAGE_COVERAGE.get(mismatch["artifact"], set())
        }
        changes: list[dict[str, Any]] = []
        raw_change_root = temporary_root / "changes"
        raw_change_root.mkdir()
        for stage in STAGE_ORDER:
            if (
                stage in blocked_stages
                or stage not in before_covered
                or stage not in after_covered
            ):
                continue
            stage_changes: list[dict[str, Any]] = []
            for before_entity, after_entity in direct_pairs.get(stage, []):
                change = entity_change(stage, "modified", before_entity, after_entity)
                if change is not None:
                    stage_changes.append(change)
            pairs, remaining_before, remaining_after = pair_unmatched_by_anchor(
                unmatched_before.get(stage, []),
                unmatched_after.get(stage, []),
            )
            for before_entity, after_entity in pairs:
                change = entity_change(stage, "modified", before_entity, after_entity)
                if change is not None:
                    stage_changes.append(change)
            for entity in remaining_before:
                change = entity_change(stage, "removed", entity, None)
                if change is not None:
                    stage_changes.append(change)
            for entity in remaining_after:
                change = entity_change(stage, "added", None, entity)
                if change is not None:
                    stage_changes.append(change)
            if diff_output_path is None:
                changes.extend(stage_changes)
            else:
                with (raw_change_root / f"{stage}.jsonl").open(
                    "w",
                    encoding="utf-8",
                    newline="\n",
                ) as raw_stream:
                    for change in stage_changes:
                        raw_stream.write(canonical_json(change))
                        raw_stream.write("\n")
                        changes.append(compact_change(change))
            memory.sample("stage_compared", stage=stage, compact_changes=len(changes))

        annotate_causality(changes)
        annotate_primary_count(changes)
        memory.sample("causality_annotated", compact_changes=len(changes))
        changes.sort(
            key=lambda change: (
                STAGE_INDEX.get(change["stage"], 999),
                SEVERITY_ORDER.get(change["severity"], 99),
                span_of_ranges(change_ranges(change)) or (-1, -1),
                change["type"],
                change["change_id"],
            )
        )
        if diff_output_path is not None:
            write_annotated_diff(raw_change_root, changes, diff_output_path)
            memory.sample("diff_written", compact_changes=len(changes))
        if reading_output_path is not None and not candidate_cache_hit:
            def candidate_reading_scan(
                side: str, source_path: Path
            ) -> tuple[
                list[dict[str, Any]],
                dict[int, tuple[int, int]],
                dict[int, list[tuple[int, int]]],
                Path,
            ]:
                raw_positions = metadata.get(f"reading_positions_{side}", [])
                positions = {
                    int(item["index"]): (int(item["offset"]), int(item["bytes"]))
                    for item in raw_positions
                }
                cache_metadata_path = temporary_root / f"reading-{side}-metadata.json"
                cache_metadata_path.write_text(
                    canonical_json(
                        {
                            "headers": metadata.get(f"reading_headers_{side}", []),
                            "positions": raw_positions,
                            "raw_tokens": True,
                        }
                    ),
                    encoding="utf-8",
                    newline="\n",
                )
                _store_reading_cache(
                    cache_directory,
                    side,
                    source_path,
                    cache_metadata_path,
                )
                return (
                    metadata.get(f"reading_headers_{side}", []),
                    positions,
                    {},
                    source_path,
                )

            prepared_reading = _write_reading_unit_payload(
                before_path,
                after_path,
                changes,
                reading_output_path,
                before_scan=candidate_reading_scan("before", before_reading_path),
                after_scan=candidate_reading_scan("after", after_reading_path),
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
        "implementation": {
            "before": before_manifest.get("implementation", {}),
            "after": after_manifest.get("implementation", {}),
        },
        "stage_graph": [
            {"stage": stage, "depends_on": list(STAGE_DEPENDENCIES.get(stage, ()))}
            for stage in STAGE_ORDER
        ],
    }
    reading_units = prepared_reading or (
        _write_reading_unit_payload(
            before_path,
            after_path,
            changes,
            reading_output_path,
            accelerator,
            cache_directory,
        )
        if reading_output_path is not None
        else _reading_unit_payload(
            before_path,
            after_path,
            changes,
            reading_headers_before,
            reading_headers_after,
        )
    )
    memory.sample("reading_projection_built", reading_units=reading_units.get("unit_count", 0))
    summary = pipeline_summary(
        before_entity_counts,
        after_entity_counts,
        before_covered,
        after_covered,
        changes,
        mismatches,
        blocked_stages,
    )
    summary["reading_units"] = reading_units.get("unit_count", 0)
    summary["affected_sentences"] = len(
        {item["sentence_index"] for item in reading_units.get("units", [])}
    ) if "affected_sentences" not in reading_units else reading_units["affected_sentences"]
    summary["reading_unit_domains"] = (
        dict(sorted(reading_units["domain_counts"].items()))
        if "domain_counts" in reading_units
        else dict(
            sorted(
                Counter(
                    domain
                    for item in reading_units.get("units", [])
                    for domain in item.get("domains", {})
                ).items()
            )
        )
    )
    memory.sample("summary_built", compact_changes=len(changes))
    memory.write()
    return ComparisonBundle(
        manifest=manifest,
        summary=summary,
        changes=changes,
        reading_units=reading_units,
        diff_prepared=diff_output_path is not None,
        reading_prepared=reading_output_path is not None,
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


def write_bundle(bundle: ComparisonBundle, output_dir: Path) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "manifest.json").write_text(
        json.dumps(bundle.manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    (output_dir / "summary.json").write_text(
        json.dumps(bundle.summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    if not bundle.diff_prepared:
        with (output_dir / "diff.jsonl.gz").open("wb") as output_stream:
            with gzip.GzipFile(fileobj=output_stream, mode="wb", compresslevel=OUTPUT_GZIP_LEVEL, mtime=0) as compressed:
                for change in bundle.changes:
                    compressed.write(canonical_json(change).encode("utf-8"))
                    compressed.write(b"\n")
    if "stages" in bundle.summary:
        write_gzip_json(output_dir / "stage-summary.json.gz", bundle.summary["stages"])
    write_gzip_json(output_dir / "root-causes.json.gz", root_impact_summary(bundle.changes))
    if not bundle.reading_prepared:
        write_reading_artifacts(
            output_dir,
            bundle.reading_units
            or {"schema_version": "kotoclip.quality.reading-diff.v1", "units": []},
        )


def write_reading_artifacts(output_dir: Path, payload: dict[str, Any]) -> None:
    """同时写权威 reading-diff 和原生 UI 的随机读取 bundle。"""
    units = payload.get("units", [])
    if not isinstance(units, list):
        raise ValueError("reading-diff.units 必须是数组")
    schema_version = str(
        payload.get("schema_version", "kotoclip.quality.reading-diff.v1")
    )
    reading_path = output_dir / "reading-diff.json.gz"
    bundle_path = output_dir / "reading-units.bin"
    index_units: list[dict[str, Any]] = []
    with reading_path.open("wb") as reading_output, bundle_path.open("wb") as unit_output:
        with gzip.GzipFile(
            fileobj=reading_output,
            mode="wb",
            compresslevel=OUTPUT_GZIP_LEVEL,
            mtime=0,
        ) as compressed:
            compressed.write(
                (
                    '{"schema_version":'
                    + canonical_json(schema_version)
                    + ',"unit_count":'
                    + str(len(units))
                    + ',"units":['
                ).encode("utf-8")
            )
            for index, unit in enumerate(units):
                if not isinstance(unit, dict):
                    raise ValueError("reading-diff unit 必须是对象")
                encoded = canonical_json(unit).encode("utf-8")
                if index:
                    compressed.write(b",")
                compressed.write(encoded)
                member = gzip.compress(
                    encoded,
                    compresslevel=OUTPUT_GZIP_LEVEL,
                    mtime=0,
                )
                offset = unit_output.tell()
                unit_output.write(member)
                before = unit.get("before") if isinstance(unit.get("before"), dict) else {}
                after = unit.get("after") if isinstance(unit.get("after"), dict) else {}
                index_units.append(
                    {
                        "unit_id": unit.get("unit_id"),
                        "sentence_index": unit.get("sentence_index"),
                        "changed_range": unit.get("changed_range"),
                        "changed_ranges": unit.get(
                            "changed_ranges", [unit.get("changed_range")]
                        ),
                        "primary_change_count": unit.get("primary_change_count", 0),
                        "evidence_change_count": unit.get("evidence_change_count", 0),
                        "domains": unit.get("domains", {}),
                        "stages": unit.get("stages", []),
                        "before": {
                            "char_range": before.get("char_range"),
                            "text": before.get("text", ""),
                        },
                        "after": {
                            "char_range": after.get("char_range"),
                            "text": after.get("text", ""),
                        },
                        "offset": offset,
                        "bytes": len(member),
                    }
                )
            compressed.write(b"]}\n")
    write_gzip_json(
        output_dir / "reading-index.json.gz",
        {
            "schema_version": "kotoclip.quality.reading-index.v1",
            "reading_schema_version": schema_version,
            "unit_count": len(index_units),
            "units": index_units,
        },
    )


def write_gzip_json(path: Path, value: Any) -> None:
    """直接向确定性 gzip 写 JSON，避免同时保留未压缩与压缩 payload。"""
    with path.open("wb") as output_stream:
        with gzip.GzipFile(fileobj=output_stream, mode="wb", compresslevel=OUTPUT_GZIP_LEVEL, mtime=0) as compressed:
            with io.TextIOWrapper(compressed, encoding="utf-8", newline="\n") as text_stream:
                json.dump(
                    value,
                    text_stream,
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                )
                text_stream.write("\n")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="比较大型语言管线 JSON 快照，并输出完整机器差分与原生阅读条目。"
    )
    parser.add_argument("--before", type=Path, help="基准 JSON")
    parser.add_argument("--after", type=Path, help="候选 JSON")
    parser.add_argument("--before-run", type=Path, help="基准快照 manifest.json")
    parser.add_argument("--after-run", type=Path, help="候选快照 manifest.json")
    parser.add_argument(
        "--adapter", choices=("bunsetsu", "expression"), help="单产物输入适配器"
    )
    parser.add_argument("--output-dir", required=True, type=Path, help="机器产物目录")
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
        bundle = compare_snapshot_manifests(
            args.before_run,
            args.after_run,
            args.output_dir.parent,
            args.output_dir / "diff.jsonl.gz",
            args.output_dir / "memory-profile.json",
            args.output_dir / "reading-diff.json.gz",
        )
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
