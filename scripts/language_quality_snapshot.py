#!/usr/bin/env python3
"""捕获可复现的 Kotoclip 大样本语言管线快照。"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import sqlite3
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence


sys.path.insert(0, str(Path(__file__).resolve().parent))

from language_quality_diff import (  # noqa: E402
    SNAPSHOT_SCHEMA_VERSION,
    STAGE_DEPENDENCIES,
    STAGE_ORDER,
    canonical_json,
    file_descriptor,
)


PRODUCER_VERSION = "2"
MAX_INLINE_STREAM_BYTES = 8 * 1024


def configure_utf8_stdio() -> None:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="backslashreplace")


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def stream_descriptor(value: str) -> dict[str, Any]:
    encoded = value.encode("utf-8")
    descriptor: dict[str, Any] = {
        "bytes": len(encoded),
        "sha256": hashlib.sha256(encoded).hexdigest(),
    }
    if len(encoded) <= MAX_INLINE_STREAM_BYTES:
        descriptor["inline"] = value
    return descriptor


def stable_resource_descriptor(path: Path, logical_name: str) -> dict[str, Any]:
    descriptor = file_descriptor(path)
    descriptor["path"] = logical_name
    descriptor["logical_name"] = logical_name
    return descriptor


def copy_profile_snapshot(source: Path, destination: Path) -> None:
    source_uri = source.resolve().as_uri() + "?mode=ro"
    with sqlite3.connect(source_uri, uri=True) as source_connection:
        with sqlite3.connect(destination) as destination_connection:
            source_connection.backup(destination_connection)


def extract_chapter(source: str, chapter: str | None) -> str:
    """与 kotoclip-cli 的 Markdown 二级标题选择规则保持一致。"""
    if chapter is None:
        return source
    requested = chapter.strip().lstrip("#").strip()
    lines = source.splitlines(keepends=True)
    body_start: int | None = None
    offset = 0
    for line in lines:
        title = line.rstrip("\r\n").strip()
        if title.startswith("## ") and title[3:].strip() == requested:
            body_start = offset + len(line)
            break
        offset += len(line)
    if body_start is None:
        raise ValueError(f"找不到章节标题：{chapter}")
    body = source[body_start:]
    end = body.find("\n## ")
    return body if end < 0 else body[:end]


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    temporary.replace(path)


def git_value(repo: Path, *arguments: str) -> str | None:
    completed = subprocess.run(
        ["git", "-C", str(repo), *arguments],
        capture_output=True,
        check=False,
        encoding="utf-8",
        errors="replace",
    )
    return completed.stdout.strip() if completed.returncode == 0 else None


def matching_files(directory: Path, patterns: Sequence[str]) -> list[dict[str, Any]]:
    if not directory.is_dir():
        return []
    paths: set[Path] = set()
    for pattern in patterns:
        paths.update(path for path in directory.glob(pattern) if path.is_file())
    result = []
    for path in sorted(paths):
        descriptor = file_descriptor(path)
        descriptor["path"] = path.relative_to(directory).as_posix()
        result.append(descriptor)
    return result


def command_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment["PYTHONUTF8"] = "1"
    environment["RUST_BACKTRACE"] = environment.get("RUST_BACKTRACE", "1")
    return environment


def run_cli(
    cli: Path,
    arguments: list[str],
    cwd: Path,
    *,
    expect_json_stdout: bool = False,
) -> tuple[Any | None, dict[str, Any]]:
    command = [str(cli), *arguments]
    started = time.perf_counter()
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=command_environment(),
        capture_output=True,
        check=False,
        encoding="utf-8",
        errors="strict",
    )
    elapsed_ms = round((time.perf_counter() - started) * 1000, 3)
    execution = {
        "command": command,
        "elapsed_ms": elapsed_ms,
        "exit_code": completed.returncode,
        "stdout": stream_descriptor(completed.stdout),
        "stderr": stream_descriptor(completed.stderr),
    }
    if completed.returncode != 0:
        raise RuntimeError(
            f"CLI 命令失败（exit={completed.returncode}）：{' '.join(command)}\n"
            f"{completed.stderr.strip()}"
        )
    if not expect_json_stdout:
        return None, execution
    try:
        return json.loads(completed.stdout), execution
    except json.JSONDecodeError as error:
        raise RuntimeError(
            f"CLI stdout 不是完整 JSON：{' '.join(command)}\n"
            f"stdout={completed.stdout[:1000]!r}"
        ) from error


def artifact_entry(
    output_dir: Path,
    path: Path,
    adapter: str,
    capture: dict[str, Any],
) -> dict[str, Any]:
    descriptor = file_descriptor(path)
    descriptor["path"] = path.relative_to(output_dir).as_posix()
    return {
        **descriptor,
        "adapter": adapter,
        "capture": capture,
    }


def resource_arguments(args: argparse.Namespace) -> list[str]:
    return [
        "--system-dict",
        str(args.system_dict.resolve()),
        "--dict-source-dir",
        str(args.dict_source_dir.resolve()),
        "--dict-dir",
        str(args.dict_dir.resolve()),
    ]


def capture_snapshot(args: argparse.Namespace) -> Path:
    repo = args.repo.resolve()
    source_path = args.source.resolve()
    output_dir = args.output_dir.resolve()
    manifest_path = output_dir / "manifest.json"
    if manifest_path.exists():
        raise FileExistsError(f"快照目录已有 manifest，拒绝覆盖：{manifest_path}")
    required_files = [args.cli, args.system_dict, source_path, args.profile]
    if args.ui_projection is not None:
        required_files.append(args.ui_projection)
    missing = [str(path) for path in required_files if not path.is_file()]
    if missing:
        raise FileNotFoundError("缺少快照输入：" + "、".join(missing))
    if not args.dict_source_dir.is_dir() or not args.dict_dir.is_dir():
        raise FileNotFoundError("词典源目录或本机缓存目录不存在")

    output_dir.mkdir(parents=True, exist_ok=True)
    artifact_dir = output_dir / "artifacts"
    artifact_dir.mkdir(parents=True, exist_ok=True)
    source = source_path.read_text(encoding="utf-8")
    selected = extract_chapter(source, args.chapter)
    executions: list[dict[str, Any]] = []
    artifacts: dict[str, dict[str, Any]] = {}
    common_resources = resource_arguments(args)
    profile_resource: dict[str, Any] | None = None

    with tempfile.TemporaryDirectory(prefix="kotoclip-quality-") as temporary:
        selected_path = Path(temporary) / "selected.md"
        selected_path.write_text(selected, encoding="utf-8", newline="\n")
        profile_snapshot = Path(temporary) / "profile.sqlite"
        copy_profile_snapshot(args.profile.resolve(), profile_snapshot)

        tokens, execution = run_cli(
            args.cli.resolve(),
            ["analyze", "--source", str(selected_path), *common_resources],
            repo,
            expect_json_stdout=True,
        )
        executions.append(execution)
        token_path = artifact_dir / "tokens.json"
        write_json(token_path, tokens)
        artifacts["tokens"] = artifact_entry(
            output_dir,
            token_path,
            "annotated_tokens",
            {"dictionary": True, "profile": False, "expressions": False},
        )
        execution["stdout"]["decoded_artifact"] = artifacts["tokens"]["path"]

        scans = (
            (
                "word_formations",
                "word-formation-scan",
                "word_formation_audit",
                ["--include-rejected"],
                {"include_rejected": True},
            ),
            (
                "lexical_candidates",
                "lexical-unit-scan",
                "lexical_candidate_audit",
                ["--include-pending", "--include-rejected"],
                {"include_pending": True, "include_rejected": True},
            ),
            (
                "bunsetsu",
                "bunsetsu-scan",
                "bunsetsu_audit",
                ["--include-alternatives"],
                {"include_alternatives": True, "dictionary": True},
            ),
            (
                "grammar_occurrences",
                "grammar-scan",
                "grammar_occurrences",
                ["--include-pending", "--include-rejected"],
                {
                    "include_pending": True,
                    "include_rejected": True,
                    "dictionary": False,
                },
            ),
            (
                "grammar_residuals",
                "grammar-residual",
                "grammar_residuals",
                [],
                {"dictionary": False},
            ),
            (
                "expressions",
                "expression-scan",
                "expression_candidates",
                ["--include-pending", "--include-rejected"],
                {
                    "include_pending": True,
                    "include_rejected": True,
                    "profile": True,
                    "dictionary": True,
                },
            ),
        )
        for name, command, adapter, flags, capture in scans:
            path = artifact_dir / f"{name}.json"
            command_args = [command]
            if command in {
                "word-formation-scan",
                "lexical-unit-scan",
                "bunsetsu-scan",
                "expression-scan",
            }:
                command_args.extend(["--profile", str(profile_snapshot)])
            command_args.extend(
                [
                    *common_resources,
                    "--source",
                    str(selected_path),
                    *flags,
                    "--json",
                    str(path),
                    "--quiet",
                ]
            )
            _, execution = run_cli(args.cli.resolve(), command_args, repo)
            executions.append(execution)
            artifacts[name] = artifact_entry(output_dir, path, adapter, capture)

        catalog_path = artifact_dir / "catalogs.json"
        _, execution = run_cli(
            args.cli.resolve(),
            ["schema-audit", *common_resources, "--json", str(catalog_path), "--quiet"],
            repo,
        )
        executions.append(execution)
        artifacts["catalogs"] = artifact_entry(
            output_dir,
            catalog_path,
            "catalog_audit",
            {},
        )

        if args.ui_projection is not None:
            ui_projection = json.loads(
                args.ui_projection.resolve().read_text(encoding="utf-8")
            )
            if (
                not isinstance(ui_projection, dict)
                or ui_projection.get("schema_version")
                != "kotoclip.quality.ui-projection.v1"
                or not isinstance(ui_projection.get("items"), list)
            ):
                raise ValueError(
                    "--ui-projection 必须是 kotoclip.quality.ui-projection.v1 对象"
                )
            ui_path = artifact_dir / "ui_projection.json"
            write_json(ui_path, ui_projection)
            artifacts["ui_projection"] = artifact_entry(
                output_dir,
                ui_path,
                "ui_projection",
                {"schema": "kotoclip.quality.ui-projection.v1"},
            )
        profile_resource = stable_resource_descriptor(profile_snapshot, "profile.sqlite")

    if profile_resource is None:
        raise RuntimeError("画像快照未生成资源指纹")

    reconstructed = "".join(
        str(token.get("bunsetsu", {}).get("surface", "")) for token in tokens
    )
    git_status = git_value(repo, "status", "--porcelain=v1")
    resources = {
        "cli": stable_resource_descriptor(args.cli.resolve(), "kotoclip-cli"),
        "system_dictionary": stable_resource_descriptor(
            args.system_dict.resolve(), "ipadic/system.dic"
        ),
        "profile": profile_resource,
        "dictionary_sources": matching_files(args.dict_source_dir.resolve(), ("*.kdict",)),
        "dictionary_caches": matching_files(
            args.dict_dir.resolve(), ("*.db", "*.sqlite")
        ),
        "catalogs": matching_files(
            repo / "crates" / "kotoclip-core" / "resources",
            ("**/*.json", "**/*.yaml", "**/*.md"),
        ),
    }
    fingerprint = sha256_text(
        canonical_json(
            {
                "selected_sha256": sha256_text(selected),
                "git_commit": git_value(repo, "rev-parse", "HEAD"),
                "resources": resources,
                "artifacts": artifacts,
            }
        )
    )
    manifest = {
        "schema_version": SNAPSHOT_SCHEMA_VERSION,
        "producer": "scripts/language_quality_snapshot.py",
        "producer_version": PRODUCER_VERSION,
        "run_id": fingerprint[:20],
        "label": args.label or output_dir.name,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "corpus": {
            "id": args.corpus_id,
            "source_path": str(source_path),
            "source_sha256": sha256_text(source),
            "selection": {"chapter": args.chapter},
            "selected_sha256": sha256_text(selected),
            "selected_bytes": len(selected.encode("utf-8")),
            "selected_characters": len(selected),
            "analysis_text_sha256": sha256_text(reconstructed),
            "analysis_characters": len(reconstructed),
        },
        "implementation": {
            "git_commit": git_value(repo, "rev-parse", "HEAD"),
            "git_dirty": bool(git_status),
            "git_status_sha256": sha256_text(git_status or ""),
            "cli_sha256": resources["cli"]["sha256"],
            "platform": platform.platform(),
            "python": platform.python_version(),
        },
        "resources": resources,
        "artifacts": artifacts,
        "stage_graph": [
            {"stage": stage, "depends_on": list(STAGE_DEPENDENCIES.get(stage, ()))}
            for stage in STAGE_ORDER
        ],
        "executions": executions,
    }
    write_json(manifest_path, manifest)
    return manifest_path


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="调用现有 CLI 捕获同一语料、资源和画像下的完整离线质量快照。"
    )
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--chapter")
    parser.add_argument("--profile", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--corpus-id", required=True)
    parser.add_argument("--label")
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--system-dict", required=True, type=Path)
    parser.add_argument("--dict-source-dir", required=True, type=Path)
    parser.add_argument("--dict-dir", required=True, type=Path)
    parser.add_argument(
        "--ui-projection",
        type=Path,
        help="可选的 kotoclip.quality.ui-projection.v1 前端投影 JSON",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    configure_utf8_stdio()
    args = parse_args(argv)
    manifest = capture_snapshot(args)
    print(f"语言质量快照完成：{manifest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
