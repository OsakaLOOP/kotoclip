#!/usr/bin/env python3
"""把旧版分散审计 JSON 登记为覆盖范围明确的只读质量快照。"""

from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence


sys.path.insert(0, str(Path(__file__).resolve().parent))

from language_quality_diff import (  # noqa: E402
    SNAPSHOT_SCHEMA_VERSION,
    STAGE_DEPENDENCIES,
    STAGE_ORDER,
    canonical_json,
    content_hash,
    file_descriptor,
)
from language_quality_snapshot import (  # noqa: E402
    extract_chapter,
    sha256_text,
    write_json,
)


ARTIFACT_SPECS = {
    "bunsetsu": (
        "bunsetsu_audit",
        {"include_alternatives": False, "dictionary": True},
    ),
    "word_formations": (
        "word_formation_audit",
        {"include_rejected": False},
    ),
    "lexical_candidates": (
        "lexical_candidate_audit",
        {"include_pending": True, "include_rejected": True},
    ),
    "grammar_occurrences": (
        "grammar_occurrences",
        {"include_pending": True, "include_rejected": True, "dictionary": False},
    ),
    "grammar_residuals": ("grammar_residuals", {"dictionary": False}),
    "expressions": (
        "expression_candidates",
        {
            "include_pending": True,
            "include_rejected": False,
            "profile": True,
            "dictionary": True,
        },
    ),
}


def relative_descriptor(path: Path, manifest_dir: Path) -> dict[str, Any]:
    descriptor = file_descriptor(path.resolve())
    descriptor["path"] = os.path.relpath(path.resolve(), manifest_dir.resolve()).replace(
        "\\", "/"
    )
    return descriptor


def import_legacy(args: argparse.Namespace) -> Path:
    output = args.output.resolve()
    if output.exists():
        raise FileExistsError(f"目标 manifest 已存在，拒绝覆盖：{output}")
    source_path = args.source.resolve()
    if not source_path.is_file():
        raise FileNotFoundError(f"语料源不存在：{source_path}")
    selected = extract_chapter(source_path.read_text(encoding="utf-8"), args.chapter)
    selected_sha256 = sha256_text(selected)
    reference = json.loads(args.reference_run.read_text(encoding="utf-8"))
    reference_corpus = reference.get("corpus", {})
    if reference_corpus.get("selected_sha256") != selected_sha256:
        raise ValueError("参考运行与旧基线的语料选择哈希不一致")

    artifacts: dict[str, dict[str, Any]] = {}
    for name, (adapter, capture) in ARTIFACT_SPECS.items():
        path = getattr(args, name)
        if path is None:
            continue
        if not path.is_file():
            raise FileNotFoundError(f"旧基线产物不存在：{name} -> {path}")
        artifacts[name] = {
            **relative_descriptor(path, output.parent),
            "adapter": adapter,
            "capture": capture,
        }
    if not artifacts:
        raise ValueError("至少需要登记一个旧基线产物")

    identity = {
        "selected_sha256": selected_sha256,
        "implementation_commit": args.implementation_commit,
        "artifacts": artifacts,
    }
    manifest = {
        "schema_version": SNAPSHOT_SCHEMA_VERSION,
        "producer": "scripts/language_quality_import_legacy.py",
        "producer_version": "1",
        "run_id": content_hash(identity)[:20],
        "label": args.label,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "corpus": {
            "id": args.corpus_id,
            "source_path": str(source_path),
            "source_sha256": sha256_text(source_path.read_text(encoding="utf-8")),
            "selection": {"chapter": args.chapter},
            "selected_sha256": selected_sha256,
            "selected_bytes": len(selected.encode("utf-8")),
            "selected_characters": len(selected),
            "analysis_text_sha256": reference_corpus.get("analysis_text_sha256"),
            "analysis_characters": reference_corpus.get("analysis_characters"),
        },
        "implementation": {
            "git_commit": args.implementation_commit,
            "legacy_import": True,
        },
        "resources": {},
        "artifacts": artifacts,
        "stage_graph": [
            {"stage": stage, "depends_on": list(STAGE_DEPENDENCIES.get(stage, ()))}
            for stage in STAGE_ORDER
        ],
        "legacy_assumptions": {
            "analysis_text_identity_from_reference_run": str(args.reference_run.resolve()),
            "resource_fingerprints_available": False,
            "missing_stages_are_not_inferred": True,
            "identity_fingerprint": content_hash(canonical_json(identity)),
        },
    }
    write_json(output, manifest)
    return output


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="登记历史审计 JSON；不会补造当时不存在的阶段或资源信息。"
    )
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--chapter")
    parser.add_argument("--corpus-id", required=True)
    parser.add_argument("--reference-run", required=True, type=Path)
    parser.add_argument("--implementation-commit")
    parser.add_argument("--label", required=True)
    parser.add_argument("--output", required=True, type=Path)
    for name in ARTIFACT_SPECS:
        parser.add_argument("--" + name.replace("_", "-"), type=Path)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="backslashreplace")
    output = import_legacy(parse_args(argv))
    print(f"旧质量基线登记完成：{output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
