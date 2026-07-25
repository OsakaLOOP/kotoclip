#!/usr/bin/env python3
"""语言质量评估统一入口。

底层脚本仍可独立调用；本入口负责把提交比较、历史刷新和生命周期状态
收束为一个可观察的调用，并把其它阶段暴露在同一个命令空间中。
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
DELEGATES = {
    "snapshot": "language_quality_snapshot.py",
    "diff": "language_quality_diff.py",
    "gate": "language_quality_gate.py",
    "history": "language_quality_history.py",
    "serve": "language_quality_dashboard_server.py",
}
QUALITY_ROOT_NAME = "experiments/quality-audit-series"


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def json_hash(value: object) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            newline="\n",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as stream:
            temporary = Path(stream.name)
            json.dump(value, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
        os.replace(temporary, path)
    finally:
        if temporary is not None and temporary.exists():
            temporary.unlink()


def git_state(repo: Path) -> dict[str, Any]:
    def capture(*args: str) -> str | None:
        result = subprocess.run(
            ["git", *args],
            cwd=repo,
            text=True,
            encoding="utf-8",
            errors="backslashreplace",
            capture_output=True,
        )
        return result.stdout.strip() if result.returncode == 0 else None

    status = capture("status", "--porcelain")
    return {
        "repo": str(repo),
        "head": capture("rev-parse", "HEAD"),
        "branch": capture("branch", "--show-current"),
        "dirty": bool(status),
        "status_sha256": hashlib.sha256((status or "").encode("utf-8")).hexdigest(),
    }


def phase(name: str, status: str, started_at: str | None = None, **extra: Any) -> dict[str, Any]:
    value: dict[str, Any] = {"name": name, "status": status}
    if started_at is not None:
        value["started_at"] = started_at
    value.update(extra)
    return value


def resolve_commit(repo: Path, reference: str) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"{reference}^{{commit}}"],
        cwd=repo,
        text=True,
        encoding="utf-8",
        errors="backslashreplace",
        capture_output=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"无法解析 Git commit：{reference}\n{result.stderr.strip()}")
    return result.stdout.strip()


def default_library_corpus(library: Path, cache_root: Path) -> tuple[Path, str]:
    """按 library.sqlite 顺序合并用户书库正文，结果按内容哈希缓存。"""
    database = library / "library.sqlite"
    if not database.is_file():
        raise SystemExit(f"用户书库数据库不存在：{database}")
    database_uri = database.resolve().as_uri() + "?mode=ro"
    connection = sqlite3.connect(database_uri, uri=True)
    try:
        rows = connection.execute(
            "SELECT id, title, author, language FROM books ORDER BY id"
        ).fetchall()
    finally:
        connection.close()
    if not rows:
        raise SystemExit(f"用户书库没有可比较的书籍：{library}")
    digest = hashlib.sha256()
    for book_id, title, author, language in rows:
        content_path = library / "books" / str(book_id) / "content.md"
        if not content_path.is_file():
            raise SystemExit(f"书库数据库记录缺少正文：{content_path}")
        digest.update(
            "\x1f".join(
                (str(book_id), str(title or ""), str(author or ""), str(language or ""))
            ).encode("utf-8")
        )
        digest.update(b"\x1e")
        content_digest = hashlib.sha256()
        with content_path.open("rb") as content:
            for chunk in iter(lambda: content.read(1024 * 1024), b""):
                content_digest.update(chunk)
        digest.update(content_digest.digest())
    corpus_hash = digest.hexdigest()
    source_path = cache_root / "corpora" / f"library-{corpus_hash[:20]}.md"
    if not source_path.is_file():
        source_path.parent.mkdir(parents=True, exist_ok=True)
        temporary: Path | None = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                newline="\n",
                dir=source_path.parent,
                prefix=f".{source_path.name}.",
                suffix=".tmp",
                delete=False,
            ) as output:
                temporary = Path(output.name)
                output.write("# Kotoclip 语言质量全书库语料\n")
                for book_id, title, author, language in rows:
                    heading = " / ".join(
                        item.replace("\n", " ").strip()
                        for item in (str(title or ""), str(author or ""), str(language or ""))
                        if item and item.strip()
                    )
                    output.write(f"\n## {heading or book_id}\n\n")
                    content_path = library / "books" / str(book_id) / "content.md"
                    with content_path.open("r", encoding="utf-8", newline="") as content:
                        for chunk in iter(lambda: content.read(1024 * 1024), ""):
                            output.write(chunk)
                output.write("\n")
                output.flush()
                os.fsync(output.fileno())
            os.replace(temporary, source_path)
        finally:
            if temporary is not None and temporary.exists():
                temporary.unlink()
    return source_path, f"library-{corpus_hash[:20]}"


def default_commit_arguments(repo: Path, before: str, after: str) -> tuple[list[str], Path, Path]:
    quality_root = (repo / QUALITY_ROOT_NAME).resolve()
    cache_root = quality_root / ".cache"
    library = Path.home() / "Documents" / "Kotoclip Library"
    source, corpus_id = default_library_corpus(library, cache_root)
    before_commit = resolve_commit(repo, before)
    after_commit = resolve_commit(repo, after)
    comparison = quality_root / f"{before_commit[:12]}-to-{after_commit[:12]}"
    common = [
        "--before", before_commit,
        "--after", after_commit,
        "--source", str(source),
        "--profile", str(repo / "data" / "research-profile.sqlite"),
        "--corpus-id", corpus_id,
        "--system-dict", str(repo / "ipadic" / "system.dic"),
        "--dict-source-dir", str(repo / "data" / "dict-sources"),
        "--dict-dir", str(repo / "data" / "dicts"),
        "--output-dir", str(comparison),
        "--repo", str(repo),
        "--build-profile", "release",
        "--artifact-store", str(quality_root / "artifact-store"),
        "--gate-config", str(repo / "scripts" / "language_quality_gate.example.json"),
    ]
    return common, comparison, quality_root


def run_delegate(command: str, arguments: Sequence[str], cwd: Path) -> int:
    script = SCRIPT_DIR / DELEGATES[command]
    result = subprocess.run(
        [sys.executable, str(script), *arguments],
        cwd=cwd,
        text=True,
        encoding="utf-8",
        errors="backslashreplace",
    )
    return result.returncode


def gate_outcome(output: Path, returncode: int) -> tuple[bool, str | None]:
    """区分比较执行失败与预期的门禁非零退出。"""
    if returncode == 0:
        return True, None
    gate_path = output / "diff" / "gate.json"
    if not gate_path.is_file():
        return False, None
    try:
        gate = json.loads(gate_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False, None
    status = gate.get("status")
    if (returncode, status) in {(1, "review_required"), (2, "blocked")}:
        return True, str(status)
    return False, None


def compare(values: Sequence[str]) -> int:
    if len(values) != 2 or any(value.startswith("-") for value in values):
        raise SystemExit("统一 compare 入口只接受两个 Git commit：python scripts/language_quality.py compare BEFORE AFTER")
    repo = Path.cwd().resolve()
    commit_args, output, history_root = default_commit_arguments(repo, values[0], values[1])
    lifecycle_path = (output / "lifecycle.json").resolve()
    no_history = False
    started_at = now()
    lifecycle: dict[str, Any] = {
        "schema_version": "kotoclip.quality.lifecycle.v1",
        "run_id": f"quality-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')}",
        "command": "compare",
        "status": "running",
        "started_at": started_at,
        "updated_at": started_at,
        "git": git_state(repo),
        "request": {
            "before": values[0],
            "after": values[1],
            "output_dir": str(output),
            "history_root": str(history_root),
            "no_history": no_history,
            "commit_args_sha256": json_hash(commit_args),
        },
        "phases": [],
    }

    def save() -> None:
        lifecycle["updated_at"] = now()
        write_json(lifecycle_path, lifecycle)

    save()
    compare_started = now()
    lifecycle["phases"].append(phase("compare_commits", "running", compare_started))
    save()
    command = [sys.executable, str(SCRIPT_DIR / "language_quality_commit_diff.py"), *commit_args]
    result = subprocess.run(
        command,
        cwd=repo,
        text=True,
        encoding="utf-8",
        errors="backslashreplace",
    )
    compare_phase = lifecycle["phases"][-1]
    compare_phase["status"] = "completed" if result.returncode == 0 else "failed"
    compare_phase["completed_at"] = now()
    compare_phase["exit_code"] = result.returncode
    comparison_completed, gate_status = gate_outcome(output, result.returncode)
    if gate_status is not None:
        compare_phase["gate_status"] = gate_status
    save()
    if not comparison_completed:
        lifecycle["status"] = "failed"
        lifecycle["failed_phase"] = "compare_commits"
        lifecycle["completed_at"] = now()
        save()
        return result.returncode

    if no_history:
        lifecycle["phases"].append(phase("refresh_history", "skipped", reason="no_history"))
    else:
        history_started = now()
        lifecycle["phases"].append(phase("refresh_history", "running", history_started))
        save()
        history_result = subprocess.run(
            [
                sys.executable,
                str(SCRIPT_DIR / "language_quality_history.py"),
                "--root",
                str(history_root),
            ],
            cwd=repo,
            text=True,
            encoding="utf-8",
            errors="backslashreplace",
        )
        history_phase = lifecycle["phases"][-1]
        history_phase["status"] = "completed" if history_result.returncode == 0 else "failed"
        history_phase["completed_at"] = now()
        history_phase["exit_code"] = history_result.returncode
        if history_result.returncode != 0:
            lifecycle["status"] = "failed"
            lifecycle["failed_phase"] = "refresh_history"
            lifecycle["completed_at"] = now()
            save()
            return history_result.returncode

    lifecycle["status"] = gate_status or "completed"
    lifecycle["completed_at"] = now()
    lifecycle["artifacts"] = {
        "comparison_dir": str(output),
        "summary": str(output / "diff" / "summary.json"),
        "reading_diff": str(output / "diff" / "reading-diff.json.gz"),
        "reading_index": str(output / "diff" / "reading-index.json.gz"),
        "reading_units": str(output / "diff" / "reading-units.bin"),
        "lifecycle": str(lifecycle_path),
        "history": None if no_history else str(history_root / "history.json"),
    }
    save()
    print(f"统一语言质量评估完成：{output}")
    print(f"生命周期：{lifecycle_path}")
    if not no_history:
        print(f"历史索引：{history_root / 'history.json'}")
    return result.returncode


def status(values: Sequence[str]) -> int:
    parser = argparse.ArgumentParser(description="读取统一语言质量入口的生命周期状态")
    parser.add_argument("--lifecycle", type=Path, required=True)
    args = parser.parse_args(list(values))
    path = args.lifecycle.resolve()
    if not path.is_file():
        raise SystemExit(f"生命周期文件不存在：{path}")
    print(path.read_text(encoding="utf-8"), end="")
    return 0


def print_help() -> None:
    print(
        """Kotoclip 语言质量统一入口

用法：
  language_quality.py compare BEFORE_COMMIT AFTER_COMMIT
  language_quality.py snapshot <language_quality_snapshot.py 选项>
  language_quality.py diff <language_quality_diff.py 选项>
  language_quality.py gate <language_quality_gate.py 选项>
  language_quality.py history <language_quality_history.py 选项>
  language_quality.py serve <language_quality_dashboard_server.py 选项>
  language_quality.py status --lifecycle PATH

示例：
  python scripts/language_quality.py compare HEAD^ HEAD
  python scripts/language_quality.py serve --root experiments/quality-audit-series --port 8765

compare 固定读取用户 Documents/Kotoclip Library 全部书籍，并使用仓库内 IPADIC、
词典源／缓存、data/research-profile.sqlite、release 构建和统一门禁配置。
输出、内容寻址 artifact、生命周期与历史索引均位于 experiments/quality-audit-series。
"""
    )


def main(argv: Sequence[str] | None = None) -> int:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="backslashreplace")
    values = list(sys.argv[1:] if argv is None else argv)
    if not values or values[0] in {"help", "--help", "-h"}:
        print_help()
        return 0
    command, remainder = values[0], values[1:]
    if command == "compare":
        return compare(remainder)
    if command == "status":
        return status(remainder)
    if command in DELEGATES:
        return run_delegate(command, remainder, Path.cwd())
    raise SystemExit(f"未知阶段：{command}；运行 help 查看用法")


if __name__ == "__main__":
    raise SystemExit(main())
