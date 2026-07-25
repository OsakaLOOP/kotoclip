#!/usr/bin/env python3
"""生成语言质量对比轮次 JSON 索引。"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any
from urllib.parse import quote


SCHEMA_VERSION = "kotoclip.quality.comparison-history.v1"


def _read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"JSON 根节点必须为对象：{path}")
    return value


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _url(path: Path, root: Path) -> str:
    return quote(path.relative_to(root).as_posix(), safe="/@:.-_~")


def _artifact(path: Path, root: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    return {
        "url": _url(path, root),
        "bytes": path.stat().st_size,
        "sha256": _sha256(path),
    }


def _resource_summary(resources: object) -> dict[str, Any] | None:
    if not isinstance(resources, dict):
        return None
    result: dict[str, Any] = {}
    for kind, value in sorted(resources.items()):
        if isinstance(value, dict):
            result[kind] = {
                key: value.get(key)
                for key in ("path", "bytes", "sha256", "logical_name")
                if key in value
            }
        elif isinstance(value, list):
            result[kind] = [
                {
                    key: item.get(key)
                    for key in ("path", "bytes", "sha256", "logical_name")
                    if key in item
                }
                for item in value
                if isinstance(item, dict)
            ]
        else:
            result[kind] = None
    return result


def _snapshot_side(
    descriptor: object,
    snapshots: dict[str, list[tuple[Path, dict[str, Any], str]]],
    root: Path,
) -> dict[str, Any]:
    source = descriptor if isinstance(descriptor, dict) else {}
    run_id = source.get("run_id")
    candidates = snapshots.get(str(run_id), []) if run_id is not None else []
    expected_sha256 = source.get("sha256")
    exact = [candidate for candidate in candidates if candidate[2] == expected_sha256]
    selected = exact[0] if exact else (candidates[0] if len(candidates) == 1 else None)
    if selected is None:
        return {
            "run_id": run_id,
            "label": source.get("label"),
            "descriptor_sha256": source.get("sha256"),
            "snapshot": None,
            "implementation": {
                "git_commit": None,
                "git_dirty": None,
                "git_status_sha256": None,
                "cli_sha256": None,
            },
            "corpus": None,
            "resources": None,
        }
    path, snapshot, snapshot_sha256 = selected
    implementation = snapshot.get("implementation")
    implementation = implementation if isinstance(implementation, dict) else {}
    corpus = snapshot.get("corpus")
    corpus = corpus if isinstance(corpus, dict) else {}
    return {
        "run_id": snapshot.get("run_id"),
        "label": snapshot.get("label"),
        "created_at": snapshot.get("created_at"),
        "descriptor_sha256": expected_sha256,
        "snapshot": {
            "manifest_url": _url(path, root),
            "bytes": path.stat().st_size,
            "manifest_sha256": snapshot_sha256,
        },
        "implementation": {
            "git_commit": implementation.get("git_commit"),
            "git_dirty": implementation.get("git_dirty"),
            "git_status_sha256": implementation.get("git_status_sha256"),
            "cli_sha256": implementation.get("cli_sha256"),
            "platform": implementation.get("platform"),
            "python": implementation.get("python"),
            "legacy_import": implementation.get("legacy_import", False),
        },
        "corpus": {
            "id": corpus.get("id"),
            "source_path": corpus.get("source_path"),
            "source_sha256": corpus.get("source_sha256"),
            "selection": corpus.get("selection"),
            "selected_sha256": corpus.get("selected_sha256"),
            "selected_bytes": corpus.get("selected_bytes"),
            "selected_characters": corpus.get("selected_characters"),
            "analysis_text_sha256": corpus.get("analysis_text_sha256"),
            "analysis_characters": corpus.get("analysis_characters"),
        },
        "resources": _resource_summary(snapshot.get("resources")),
    }


def _run_record(
    directory: Path,
    root: Path,
    snapshots: dict[str, list[tuple[Path, dict[str, Any], str]]],
) -> dict[str, Any] | None:
    manifest_path = directory / "manifest.json"
    summary_path = directory / "summary.json"
    diff_path = directory / "diff.jsonl.gz"
    if not diff_path.is_file():
        diff_path = directory / "diff.jsonl"
    if not manifest_path.is_file() or not summary_path.is_file() or not diff_path.is_file():
        return None
    try:
        manifest = _read_json(manifest_path)
        summary = _read_json(summary_path)
    except (OSError, ValueError, json.JSONDecodeError):
        return None
    stage_churn: dict[str, float] = {}
    for stage in summary.get("stages", []):
        if isinstance(stage, dict) and isinstance(stage.get("churn"), dict):
            rate = stage["churn"].get("rate")
            if isinstance(rate, (int, float)):
                stage_churn[str(stage.get("stage", ""))] = rate
    gate_path = directory / "gate.json"
    stage_summary_path = directory / "stage-summary.json.gz"
    root_causes_path = directory / "root-causes.json.gz"
    memory_profile_path = directory / "memory-profile.json"
    build_root = directory.parent if directory.name == "diff" else directory
    lifecycle_path = build_root / "lifecycle.json"
    build_before_path = build_root / "build-before.log"
    build_after_path = build_root / "build-after.log"
    gate_status = None
    memory_summary: dict[str, Any] = {}
    if memory_profile_path.is_file():
        try:
            memory_profile = _read_json(memory_profile_path)
            memory_summary = {
                key: memory_profile.get(key)
                for key in (
                    "target_peak_bytes",
                    "observed_peak_rss_bytes",
                    "within_target",
                    "elapsed_seconds",
                    "sample_count",
                )
            }
        except (OSError, ValueError, json.JSONDecodeError):
            memory_summary = {"invalid": True}
    if gate_path.is_file():
        try:
            gate = _read_json(gate_path)
            gate_status = gate.get("status")
        except (OSError, ValueError, json.JSONDecodeError):
            gate_status = "invalid"
    relative_directory = directory.relative_to(root).as_posix()
    before_side = _snapshot_side(manifest.get("before"), snapshots, root)
    after_side = _snapshot_side(manifest.get("after"), snapshots, root)

    # 快照保存完整 SHA；历史索引在本地仓库可用时补充可读的 Git 提交信息。
    repository = root.parents[1] if len(root.parents) > 1 else None
    for side in (before_side, after_side):
        commit = side.get("implementation", {}).get("git_commit")
        if not commit or repository is None or not (repository / ".git").exists():
            continue
        result = subprocess.run(
            ["git", "show", "-s", "--format=%s%x1f%an%x1f%aI%x1f%cI", commit],
            cwd=repository,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=True,
        )
        if result.returncode == 0:
            values = result.stdout.strip().split("\x1f")
            if len(values) == 4:
                side["implementation"].update({
                    "git_subject": values[0],
                    "git_author": values[1],
                    "git_author_time": values[2],
                    "git_committer_time": values[3],
                })
    record: dict[str, Any] = {
        "comparison_id": relative_directory,
        "created_at": after_side.get("created_at") or before_side.get("created_at"),
        "adapter": manifest.get("adapter", "unknown"),
        "manifest": _artifact(manifest_path, root),
        "summary_artifact": _artifact(summary_path, root),
        "diff": _artifact(diff_path, root),
        "reading_diff": _artifact(directory / "reading-diff.json.gz", root),
        "reading_index": _artifact(directory / "reading-index.json.gz", root),
        "reading_units": _artifact(directory / "reading-units.bin", root),
        "stage_summary": _artifact(stage_summary_path, root),
        "root_causes": _artifact(root_causes_path, root),
        "memory_profile": _artifact(memory_profile_path, root),
        "gate": _artifact(gate_path, root),
        "lifecycle": _artifact(lifecycle_path, root),
        "build_before_log": _artifact(build_before_path, root),
        "build_after_log": _artifact(build_after_path, root),
        "before": before_side,
        "after": after_side,
        "summary": {
            "status": summary.get("status"),
            "comparable": summary.get("comparable"),
            "complete_stage_coverage": summary.get("complete_stage_coverage"),
            "quality_conclusion": summary.get("quality_conclusion"),
            "changes": summary.get("changes", 0),
            "evidence_changes": summary.get("evidence_changes", 0),
            "root_changes": summary.get("root_changes", 0),
            "propagated_candidates": summary.get("propagated_candidates", 0),
            "churn_rate": (summary.get("churn") or {}).get("rate"),
            "change_types": summary.get("change_types", {}),
            "severities": summary.get("severities", {}),
            "scope_counts": summary.get("scope_counts", {}),
            "scope_rates": summary.get("scope_rates", {}),
            "status_transitions": summary.get("status_transitions", []),
            "missing_stages": summary.get("missing_stages", []),
            "stage_churn": stage_churn,
            "reading_units": summary.get("reading_units", 0),
            "affected_sentences": summary.get("affected_sentences", 0),
            "memory": memory_summary,
        },
        "gate_status": gate_status,
    }
    return record


def build_history(root: Path) -> dict[str, Any]:
    """扫描 root 下完整对比目录，生成确定性索引。"""
    root = root.resolve()

    def is_internal(path: Path) -> bool:
        return any(part.startswith(".") for part in path.relative_to(root).parts)

    snapshots: dict[str, list[tuple[Path, dict[str, Any], str]]] = {}
    for manifest_path in sorted(root.rglob("manifest.json")):
        if is_internal(manifest_path):
            continue
        try:
            manifest = _read_json(manifest_path)
        except (OSError, ValueError, json.JSONDecodeError):
            continue
        if manifest.get("schema_version") != "kotoclip.quality.snapshot.v1":
            continue
        run_id = manifest.get("run_id")
        if run_id is not None:
            snapshots.setdefault(str(run_id), []).append(
                (manifest_path, manifest, _sha256(manifest_path))
            )
    records = []
    for summary_path in sorted(root.rglob("summary.json")):
        if is_internal(summary_path):
            continue
        record = _run_record(summary_path.parent, root, snapshots)
        if record is not None:
            records.append(record)
    records.sort(
        key=lambda item: (item.get("created_at") or "", item["comparison_id"]),
        reverse=True,
    )
    return {"schema_version": SCHEMA_VERSION, "root": ".", "comparisons": records}


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="生成语言质量对比轮次 JSON 索引")
    parser.add_argument("--root", type=Path, required=True, help="包含多个对比目录的实验根目录")
    parser.add_argument("--output", type=Path, help="history.json 输出路径；默认 root/history.json")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="backslashreplace")
    args = parse_args(argv)
    root = args.root.resolve()
    if not root.is_dir():
        raise SystemExit(f"历史根目录不存在：{root}")
    output = (args.output or root / "history.json").resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    history = build_history(root)
    output.write_text(
        json.dumps(history, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    print(f"历史索引：{output}（{len(history['comparisons'])} 轮）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
