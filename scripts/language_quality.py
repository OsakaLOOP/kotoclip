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


def split_compare_args(values: Sequence[str]) -> tuple[list[str], Path | None, Path | None, bool]:
    """取出统一入口自己的选项，其余参数原样交给 commit_diff。"""
    commit_args: list[str] = []
    history_root: Path | None = None
    lifecycle_path: Path | None = None
    no_history = False
    index = 0
    while index < len(values):
        value = values[index]
        if value == "--history-root":
            if index + 1 >= len(values):
                raise SystemExit("--history-root 需要路径")
            history_root = Path(values[index + 1])
            index += 2
            continue
        if value == "--lifecycle":
            if index + 1 >= len(values):
                raise SystemExit("--lifecycle 需要路径")
            lifecycle_path = Path(values[index + 1])
            index += 2
            continue
        if value == "--no-history":
            no_history = True
            index += 1
            continue
        commit_args.append(value)
        index += 1
    return commit_args, history_root, lifecycle_path, no_history


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
    commit_args, history_root, lifecycle_path, no_history = split_compare_args(values)
    from language_quality_commit_diff import parse_args as parse_commit_args

    parsed = parse_commit_args(commit_args)
    repo = parsed.repo.resolve()
    output = parsed.output_dir.resolve()
    history_root = (history_root or output.parent).resolve()
    lifecycle_path = (lifecycle_path or output / "lifecycle.json").resolve()
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
            "before": parsed.before,
            "after": parsed.after,
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
        "report": str(output / "diff" / "report.html"),
        "summary": str(output / "diff" / "summary.json"),
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
  language_quality.py compare [统一选项] <language_quality_commit_diff.py 选项>
  language_quality.py snapshot <language_quality_snapshot.py 选项>
  language_quality.py diff <language_quality_diff.py 选项>
  language_quality.py gate <language_quality_gate.py 选项>
  language_quality.py history <language_quality_history.py 选项>
  language_quality.py serve <language_quality_dashboard_server.py 选项>
  language_quality.py status --lifecycle PATH

compare 统一选项：
  --history-root PATH  比较完成后刷新该目录的 history.json/history.html，默认 output-dir 的父目录
  --lifecycle PATH     生命周期 JSON 路径，默认 output-dir/lifecycle.json
  --no-history         只运行比较，不刷新历史索引

示例：
  python scripts/language_quality.py compare `
    --before HEAD^ --after HEAD `
    --output-dir experiments/quality/runs/current `
    --history-root experiments/quality
  python scripts/language_quality.py serve --root experiments/quality --port 8765
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
