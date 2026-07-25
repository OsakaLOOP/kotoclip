#!/usr/bin/env python3
"""捕获可复现的 Kotoclip 大样本语言管线快照。"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import platform
import shutil
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


PRODUCER_VERSION = "3"
MAX_INLINE_STREAM_BYTES = 8 * 1024
ARTIFACT_GZIP_LEVEL = 1


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


def stream_file_descriptor(path: Path) -> dict[str, Any]:
    """描述已落盘的 stdout，不把大文件重新读入内存。"""
    descriptor = file_descriptor(path)
    descriptor.pop("path", None)
    if descriptor["bytes"] <= MAX_INLINE_STREAM_BYTES:
        descriptor["inline"] = path.read_text(encoding="utf-8")
    return descriptor


def stable_resource_descriptor(path: Path, logical_name: str) -> dict[str, Any]:
    descriptor = file_descriptor(path)
    descriptor["path"] = logical_name
    descriptor["logical_name"] = logical_name
    return descriptor


def copy_profile_snapshot(source: Path, destination: Path) -> None:
    source_uri = source.resolve().as_uri() + "?mode=ro"
    source_connection = sqlite3.connect(source_uri, uri=True)
    destination_connection = sqlite3.connect(destination)
    try:
        source_connection.backup(destination_connection)
    finally:
        destination_connection.close()
        source_connection.close()


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


def compressed_json(value: Any) -> bytes:
    """以固定 gzip 时间戳压缩 JSON，避免相同内容因生成时间不同而失去复用。"""
    payload = canonical_json(value).encode("utf-8")
    return gzip.compress(payload, compresslevel=ARTIFACT_GZIP_LEVEL, mtime=0)


def install_compressed_artifact(
    path: Path,
    payload: bytes,
    artifact_store: Path,
) -> None:
    """写入内容寻址 artifact，并在同卷时用硬链接复用物理文件。"""
    digest = hashlib.sha256(payload).hexdigest()
    stored = artifact_store / digest[:2] / f"{digest}.blob"
    stored.parent.mkdir(parents=True, exist_ok=True)
    if not stored.exists():
        temporary = stored.with_suffix(".tmp")
        temporary.write_bytes(payload)
        try:
            temporary.replace(stored)
        except FileExistsError:
            temporary.unlink(missing_ok=True)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.unlink(missing_ok=True)
    try:
        os.link(stored, path)
    except OSError:
        path.write_bytes(payload)


def write_compressed_artifact(
    path: Path,
    value: Any,
    artifact_store: Path,
) -> None:
    install_compressed_artifact(path, compressed_json(value), artifact_store)


def compress_file_artifact(
    source: Path,
    target: Path,
    artifact_store: Path,
) -> None:
    temporary = target.with_suffix(target.suffix + ".tmp")
    target.parent.mkdir(parents=True, exist_ok=True)
    with source.open("rb") as input_stream, temporary.open("wb") as output_stream:
        with gzip.GzipFile(
            fileobj=output_stream,
            mode="wb",
            compresslevel=ARTIFACT_GZIP_LEVEL,
            mtime=0,
        ) as compressed:
            shutil.copyfileobj(input_stream, compressed, length=1024 * 1024)
    source.unlink()
    install_artifact_file(target, temporary, artifact_store)


def install_artifact_file(
    path: Path,
    payload_path: Path,
    artifact_store: Path,
) -> None:
    """安装已压缩文件并复用内容寻址硬链接，全程不聚合 payload。"""
    digest = str(file_descriptor(payload_path)["sha256"])
    stored = artifact_store / digest[:2] / f"{digest}.blob"
    stored.parent.mkdir(parents=True, exist_ok=True)
    try:
        os.link(payload_path, stored)
    except FileExistsError:
        pass
    except OSError:
        if not stored.exists():
            shutil.copyfile(payload_path, stored)
    payload_path.unlink(missing_ok=True)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.unlink(missing_ok=True)
    try:
        os.link(stored, path)
    except OSError:
        shutil.copyfile(stored, path)


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


def run_cli_to_file(
    cli: Path,
    arguments: list[str],
    cwd: Path,
    output_path: Path,
) -> dict[str, Any]:
    """将大规模 CLI stdout 直接写入文件，避免 subprocess 内存聚合。"""
    command = [str(cli), *arguments]
    started = time.perf_counter()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("wb") as output:
        completed = subprocess.run(
            command,
            cwd=cwd,
            env=command_environment(),
            stdout=output,
            stderr=subprocess.PIPE,
            check=False,
            encoding="utf-8",
            errors="strict",
        )
    elapsed_ms = round((time.perf_counter() - started) * 1000, 3)
    execution = {
        "command": command,
        "elapsed_ms": elapsed_ms,
        "exit_code": completed.returncode,
        "stdout": stream_file_descriptor(output_path),
        "stderr": stream_descriptor(completed.stderr or ""),
    }
    if completed.returncode != 0:
        raise RuntimeError(
            f"CLI 命令失败（exit={completed.returncode}）：{' '.join(command)}\n"
            f"{(completed.stderr or '').strip()}"
        )
    return execution


def iter_json_array(path: Path):
    """增量解析 CLI 输出数组，每次只保留一个 token 对象。"""
    decoder = json.JSONDecoder()
    buffer = ""
    position = 0
    end_of_file = False

    with path.open("r", encoding="utf-8") as source:
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
            raise ValueError(f"CLI 输出不是 JSON 数组：{path}")
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


def count_named_object_arrays(path: Path, fields: set[str]) -> dict[str, int]:
    """增量统计顶层对象中的目标数组，不物化审计报告。"""
    decoder = json.JSONDecoder()
    buffer = ""
    position = 0
    end_of_file = False

    with path.open("r", encoding="utf-8") as source:
        def fill() -> None:
            nonlocal buffer, end_of_file
            chunk = source.read(1024 * 1024)
            if chunk:
                buffer += chunk
            else:
                end_of_file = True

        def next_non_whitespace() -> str:
            nonlocal buffer, position
            while True:
                while position < len(buffer) and buffer[position].isspace():
                    position += 1
                if position < len(buffer):
                    return buffer[position]
                if end_of_file:
                    raise ValueError(f"JSON 对象意外结束：{path}")
                buffer = ""
                position = 0
                fill()

        def decode_value() -> Any:
            nonlocal buffer, position
            while True:
                try:
                    value, next_position = decoder.raw_decode(buffer, position)
                except json.JSONDecodeError:
                    if end_of_file:
                        raise ValueError(f"JSON 值不完整：{path}") from None
                    if position:
                        buffer = buffer[position:]
                        position = 0
                    fill()
                    continue
                position = next_position
                return value

        def consume_array() -> int:
            nonlocal position
            if next_non_whitespace() != "[":
                raise ValueError(f"目标字段不是数组：{path}")
            position += 1
            count = 0
            expect_value = True
            while True:
                character = next_non_whitespace()
                if not expect_value:
                    if character == ",":
                        position += 1
                        expect_value = True
                        continue
                    if character == "]":
                        position += 1
                        return count
                    raise ValueError(f"JSON 数组缺少逗号：{path}")
                if character == "]":
                    position += 1
                    return count
                decode_value()
                count += 1
                expect_value = False

        fill()
        if next_non_whitespace() != "{":
            raise ValueError(f"CLI 输出不是 JSON 对象：{path}")
        position += 1
        counts: dict[str, int] = {}
        expect_key = True
        while True:
            character = next_non_whitespace()
            if not expect_key:
                if character == ",":
                    position += 1
                    expect_key = True
                    continue
                if character == "}":
                    return counts
                raise ValueError(f"JSON 对象缺少逗号：{path}")
            if character == "}":
                return counts
            key = decode_value()
            if not isinstance(key, str):
                raise ValueError(f"JSON 对象键必须为字符串：{path}")
            if next_non_whitespace() != ":":
                raise ValueError(f"JSON 对象键缺少冒号：{path}")
            position += 1
            if key in fields:
                counts[key] = consume_array()
            else:
                decode_value()
            expect_key = False


def stage_counts_for_artifact(name: str, path: Path) -> dict[str, int]:
    """计算 compare 可直接复用的稳定阶段计数。"""
    if name == "word_formations":
        counts = count_named_object_arrays(path, {"items", "rejected"})
        return {"word_formation_candidate": sum(counts.values())}
    if name == "lexical_candidates":
        counts = count_named_object_arrays(path, {"items"})
        return {"lexical_candidate": counts.get("items", 0)}
    if name == "bunsetsu":
        return {
            "bunsetsu_boundary": sum(
                len(report.get("boundaries", []))
                for report in iter_json_array(path)
                if isinstance(report, dict)
                and isinstance(report.get("boundaries"), list)
            )
        }
    if name == "expressions":
        accepted = 0
        rejected = 0
        for expression in iter_json_array(path):
            if not isinstance(expression, dict):
                continue
            if expression.get("status", "accepted") == "accepted":
                accepted += 1
            else:
                rejected += 1
        return {"expression": accepted, "expression_candidate": rejected}
    if name == "catalogs":
        return {"resource": sum(1 for _ in iter_json_array(path))}
    return {}


def reconstructed_text_descriptor(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    characters = 0
    for token in iter_json_array(path):
        if not isinstance(token, dict):
            raise ValueError(f"token 必须为对象：{path}")
        bunsetsu = token.get("bunsetsu")
        surface = bunsetsu.get("surface", "") if isinstance(bunsetsu, dict) else ""
        if not isinstance(surface, str):
            raise ValueError(f"bunsetsu.surface 必须为字符串：{path}")
        digest.update(surface.encode("utf-8"))
        characters += len(surface)
    return digest.hexdigest(), characters


def artifact_entry(
    output_dir: Path,
    path: Path,
    adapter: str,
    capture: dict[str, Any],
    stage_counts: dict[str, int] | None = None,
) -> dict[str, Any]:
    descriptor = file_descriptor(path)
    descriptor["path"] = path.relative_to(output_dir).as_posix()
    entry = {
        **descriptor,
        "adapter": adapter,
        "capture": capture,
    }
    if stage_counts:
        entry["stage_counts"] = stage_counts
    return entry


def resource_arguments(args: argparse.Namespace) -> list[str]:
    return [
        "--system-dict",
        str(args.system_dict.resolve()),
        "--dict-source-dir",
        str(args.dict_source_dir.resolve()),
        "--dict-dir",
        str(args.dict_dir.resolve()),
    ]


def supports_quality_snapshot(cli: Path, cwd: Path) -> bool:
    completed = subprocess.run(
        [str(cli), "help"],
        cwd=cwd,
        text=True,
        encoding="utf-8",
        errors="backslashreplace",
        capture_output=True,
    )
    return completed.returncode == 0 and "quality-snapshot" in completed.stdout


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
    artifact_store = (args.artifact_store or output_dir.parent / "artifact-store").resolve()
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

        merged_raw_dir = Path(temporary) / "quality-snapshot"
        merged_snapshot = supports_quality_snapshot(args.cli.resolve(), repo)
        if merged_snapshot:
            merged_raw_dir.mkdir()
            _, merged_execution = run_cli(
                args.cli.resolve(),
                [
                    "quality-snapshot",
                    "--source",
                    str(selected_path),
                    "--profile",
                    str(profile_snapshot),
                    "--output-dir",
                    str(merged_raw_dir),
                    *common_resources,
                    "--quiet",
                ],
                repo,
            )
            executions.append(merged_execution)

        raw_token_path = (
            merged_raw_dir / "tokens.json"
            if merged_snapshot
            else Path(temporary) / "tokens.json"
        )
        if not merged_snapshot:
            execution = run_cli_to_file(
                args.cli.resolve(),
                ["analyze", "--source", str(selected_path), *common_resources],
                repo,
                raw_token_path,
            )
            executions.append(execution)
        # tokens 是同一机器上 compare 与阅读扫描的主输入，保留未压缩内容避免重复解压。
        # artifact store 与 snapshot cache 均通过硬链接复用，不增加同卷物理副本。
        token_path = artifact_dir / "tokens.json"
        analysis_text_sha256, analysis_characters = reconstructed_text_descriptor(
            raw_token_path
        )
        install_artifact_file(token_path, raw_token_path, artifact_store)
        artifacts["tokens"] = artifact_entry(
            output_dir,
            token_path,
            "annotated_tokens",
            {"dictionary": True, "profile": False, "expressions": False},
        )
        if not merged_snapshot:
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
            raw_path = (
                merged_raw_dir / f"{name}.json"
                if merged_snapshot
                else artifact_dir / f"{name}.json"
            )
            path = artifact_dir / f"{name}.json.gz"
            if not merged_snapshot:
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
                        str(raw_path),
                        "--quiet",
                    ]
                )
                _, execution = run_cli(args.cli.resolve(), command_args, repo)
                executions.append(execution)
            stage_counts = stage_counts_for_artifact(name, raw_path)
            compress_file_artifact(raw_path, path, artifact_store)
            artifacts[name] = artifact_entry(
                output_dir, path, adapter, capture, stage_counts
            )

        raw_catalog_path = (
            merged_raw_dir / "catalogs.json"
            if merged_snapshot
            else artifact_dir / "catalogs.json"
        )
        catalog_path = artifact_dir / "catalogs.json.gz"
        if not merged_snapshot:
            _, execution = run_cli(
                args.cli.resolve(),
                ["schema-audit", *common_resources, "--json", str(raw_catalog_path), "--quiet"],
                repo,
            )
            executions.append(execution)
        catalog_stage_counts = stage_counts_for_artifact("catalogs", raw_catalog_path)
        compress_file_artifact(raw_catalog_path, catalog_path, artifact_store)
        artifacts["catalogs"] = artifact_entry(
            output_dir,
            catalog_path,
            "catalog_audit",
            {},
            catalog_stage_counts,
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
            ui_path = artifact_dir / "ui_projection.json.gz"
            write_compressed_artifact(ui_path, ui_projection, artifact_store)
            artifacts["ui_projection"] = artifact_entry(
                output_dir,
                ui_path,
                "ui_projection",
                {"schema": "kotoclip.quality.ui-projection.v1"},
            )
        profile_resource = stable_resource_descriptor(profile_snapshot, "profile.sqlite")

    if profile_resource is None:
        raise RuntimeError("画像快照未生成资源指纹")

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
            "analysis_text_sha256": analysis_text_sha256,
            "analysis_characters": analysis_characters,
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
        "--artifact-store",
        type=Path,
        help="共享内容寻址 artifact 存储；默认使用 output-dir 的同级 artifact-store",
    )
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
