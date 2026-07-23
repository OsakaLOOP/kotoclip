#!/usr/bin/env python3
"""在两个 Git 提交之间创建隔离 worktree 并运行完整语言质量差分。"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Sequence


def run(command: Sequence[str], *, cwd: Path, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        list(command),
        cwd=cwd,
        env=env,
        text=True,
        encoding="utf-8",
        errors="backslashreplace",
        capture_output=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            "命令失败（{}）：\n{}\n{}".format(
                result.returncode,
                " ".join(command),
                (result.stdout + "\n" + result.stderr).strip(),
            )
        )
    return result


def git(repo: Path, *args: str) -> str:
    return run(["git", *args], cwd=repo).stdout.strip()


def resolve_executable(target: Path) -> Path:
    candidates = [target / "kotoclip-cli.exe", target / "kotoclip-cli"]
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise FileNotFoundError(f"构建成功但找不到 kotoclip-cli：{target}")


def build_cli(worktree: Path, target_dir: Path, log_path: Path, profile: str) -> Path:
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    result = subprocess.run(
        [
            "cargo",
            "build",
            "--locked",
            "--manifest-path",
            str(worktree / "Cargo.toml"),
            "-p",
            "kotoclip-core",
            "--bin",
            "kotoclip-cli",
            "--profile",
            profile,
        ],
        cwd=worktree,
        env=env,
        text=True,
        encoding="utf-8",
        errors="backslashreplace",
        capture_output=True,
    )
    log_path.write_text(result.stdout + "\n" + result.stderr, encoding="utf-8", newline="\n")
    if result.returncode != 0:
        raise RuntimeError(f"cargo 构建失败，完整日志见：{log_path}")
    artifact_profile = "debug" if profile == "dev" else profile
    return resolve_executable(target_dir / artifact_profile)


def snapshot(
    tool_root: Path,
    repo: Path,
    cli: Path,
    args: argparse.Namespace,
    output: Path,
    label: str,
    system_dict: Path,
    dict_source_dir: Path,
    dict_dir: Path,
) -> None:
    command = [
        sys.executable,
        str(tool_root / "language_quality_snapshot.py"),
        "--source",
        str(args.source),
        "--profile",
        str(args.profile),
        "--output-dir",
        str(output),
        "--corpus-id",
        args.corpus_id,
        "--label",
        label,
        "--repo",
        str(repo),
        "--cli",
        str(cli),
        "--system-dict",
        str(system_dict),
        "--dict-source-dir",
        str(dict_source_dir),
        "--dict-dir",
        str(dict_dir),
    ]
    if args.chapter:
        command.extend(["--chapter", args.chapter])
    if args.ui_projection:
        command.extend(["--ui-projection", str(args.ui_projection)])
    run(command, cwd=tool_root)


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="比较两个提交的完整语言管线；提交两端使用独立 detached worktree。"
    )
    parser.add_argument("--before", required=True, help="基准提交，例如 HEAD^ 或 commit SHA")
    parser.add_argument("--after", default="HEAD", help="候选提交，默认当前 HEAD")
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--chapter")
    parser.add_argument("--profile", required=True, type=Path)
    parser.add_argument("--corpus-id", required=True)
    parser.add_argument("--system-dict", required=True, type=Path)
    parser.add_argument("--dict-source-dir", required=True, type=Path)
    parser.add_argument("--dict-dir", required=True, type=Path)
    parser.add_argument(
        "--before-system-dict",
        type=Path,
        help="before 使用的系统词典；默认使用 --system-dict",
    )
    parser.add_argument(
        "--before-dict-source-dir",
        type=Path,
        help="before 使用的词典源目录；默认使用 --dict-source-dir",
    )
    parser.add_argument(
        "--before-dict-dir",
        type=Path,
        help="before 使用的词典缓存目录；默认使用 --dict-dir",
    )
    parser.add_argument(
        "--after-system-dict",
        type=Path,
        help="after 使用的系统词典；默认使用 --system-dict",
    )
    parser.add_argument(
        "--after-dict-source-dir",
        type=Path,
        help="after 使用的词典源目录；默认使用 --dict-source-dir",
    )
    parser.add_argument(
        "--after-dict-dir",
        type=Path,
        help="after 使用的词典缓存目录；默认使用 --dict-dir",
    )
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--build-profile", default="release", choices=("dev", "release"))
    parser.add_argument("--ui-projection", type=Path)
    parser.add_argument("--gate-config", type=Path)
    return parser.parse_args(argv)


def side_resources(args: argparse.Namespace, side: str) -> tuple[Path, Path, Path]:
    """解析一侧资源；未覆盖的项目回落到公共资源参数。"""
    if side not in {"before", "after"}:
        raise ValueError(f"未知提交侧：{side}")
    system_dict = getattr(args, f"{side}_system_dict") or args.system_dict
    dict_source_dir = (
        getattr(args, f"{side}_dict_source_dir") or args.dict_source_dir
    )
    dict_dir = getattr(args, f"{side}_dict_dir") or args.dict_dir
    return system_dict.resolve(), dict_source_dir.resolve(), dict_dir.resolve()


def main(argv: Sequence[str] | None = None) -> int:
    # 在 argparse 输出帮助或错误前固定 UTF-8，避免 Windows 日文区域的 cp932 失败。
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="backslashreplace")
    args = parse_args(argv)
    repo = args.repo.resolve()
    tool_root = Path(__file__).resolve().parent
    source = args.source.resolve()
    profile = args.profile.resolve()
    output = args.output_dir.resolve()
    for path in (source, profile):
        if not path.exists():
            raise FileNotFoundError(f"缺少提交比较输入：{path}")
    before_system_dict, before_dict_source_dir, before_dict_dir = side_resources(
        args, "before"
    )
    after_system_dict, after_dict_source_dir, after_dict_dir = side_resources(
        args, "after"
    )
    for side, paths in (
        (
            "before",
            (before_system_dict, before_dict_source_dir, before_dict_dir),
        ),
        ("after", (after_system_dict, after_dict_source_dir, after_dict_dir)),
    ):
        if not paths[0].is_file() or not paths[1].is_dir() or not paths[2].is_dir():
            raise FileNotFoundError(
                f"缺少 {side} 提交比较资源：系统词典必须为文件，词典源／缓存必须为目录；"
                + "、".join(str(path) for path in paths)
            )
    # snapshot 在 scripts 目录中启动，固定为调用方解析后的绝对输入路径。
    args.source = source
    args.profile = profile
    if args.ui_projection is not None:
        args.ui_projection = args.ui_projection.resolve()
    run(["git", "rev-parse", "--verify", f"{args.before}^{{commit}}"], cwd=repo)
    run(["git", "rev-parse", "--verify", f"{args.after}^{{commit}}"], cwd=repo)
    output.mkdir(parents=True, exist_ok=True)
    before_output = output / "before"
    after_output = output / "after"
    diff_output = output / "diff"
    if any(path.exists() for path in (before_output, after_output, diff_output)):
        raise FileExistsError(f"提交比较输出目录已有内容，拒绝覆盖：{output}")

    with tempfile.TemporaryDirectory(prefix="kotoclip-quality-commits-") as temporary:
        temporary_root = Path(temporary)
        worktrees: list[Path] = []
        try:
            for name, commit in (("before", args.before), ("after", args.after)):
                worktree = temporary_root / f"worktree-{name}"
                run(
                    ["git", "worktree", "add", "--detach", "--force", str(worktree), commit],
                    cwd=repo,
                )
                worktrees.append(worktree)

            before_cli = build_cli(
                worktrees[0],
                temporary_root / "target-before",
                output / "build-before.log",
                args.build_profile,
            )
            after_cli = build_cli(
                worktrees[1],
                temporary_root / "target-after",
                output / "build-after.log",
                args.build_profile,
            )
            snapshot(
                tool_root,
                worktrees[0],
                before_cli,
                args,
                before_output,
                f"before-{args.before}",
                before_system_dict,
                before_dict_source_dir,
                before_dict_dir,
            )
            snapshot(
                tool_root,
                worktrees[1],
                after_cli,
                args,
                after_output,
                f"after-{args.after}",
                after_system_dict,
                after_dict_source_dir,
                after_dict_dir,
            )
        finally:
            for worktree in reversed(worktrees):
                subprocess.run(
                    ["git", "worktree", "remove", "--force", str(worktree)],
                    cwd=repo,
                    text=True,
                    encoding="utf-8",
                    errors="backslashreplace",
                    capture_output=True,
                )
            git(repo, "worktree", "prune")

    diff_command = [
        sys.executable,
        str(tool_root / "language_quality_diff.py"),
        "--before-run",
        str(before_output / "manifest.json"),
        "--after-run",
        str(after_output / "manifest.json"),
        "--output-dir",
        str(diff_output),
    ]
    run(diff_command, cwd=repo)
    if args.gate_config:
        gate_output = diff_output / "gate.json"
        gate = subprocess.run(
            [
                sys.executable,
                str(tool_root / "language_quality_gate.py"),
                "--summary",
                str(diff_output / "summary.json"),
                "--config",
                str(args.gate_config.resolve()),
                "--output",
                str(gate_output),
            ],
            cwd=repo,
            text=True,
            encoding="utf-8",
            errors="backslashreplace",
        )
        print(f"提交比较门禁退出码：{gate.returncode}")
        if gate.returncode:
            return gate.returncode
    print(f"提交比较完成：{diff_output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
