#!/usr/bin/env python3
"""在两个 Git 提交之间创建隔离 worktree 并运行完整语言质量差分。"""

from __future__ import annotations

import argparse
import concurrent.futures
import gzip
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Iterable, Sequence


ARTIFACT_GZIP_LEVEL = 1


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


def resolve_quality_diff(repo: Path) -> Path:
    """构建当前工作树中的审计协调器；被比较提交只负责生成各自快照。"""
    run(
        [
            "cargo",
            "build",
            "--locked",
            "-p",
            "kotoclip-quality-diff",
            "--release",
        ],
        cwd=repo,
    )
    suffix = ".exe" if os.name == "nt" else ""
    executable = repo / "target" / "release" / f"kotoclip-quality-diff{suffix}"
    if not executable.is_file():
        raise FileNotFoundError(f"构建成功但找不到 Rust diff：{executable}")
    return executable


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
        "--artifact-store",
        str(args.snapshot_artifact_store),
    ]
    if args.chapter:
        command.extend(["--chapter", args.chapter])
    if args.ui_projection:
        command.extend(["--ui-projection", str(args.ui_projection)])
    run(command, cwd=tool_root)


def update_file_digest(digest: Any, path: Path) -> None:
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)


def update_directory_fingerprint(digest: Any, directory: Path) -> None:
    for path in sorted(item for item in directory.rglob("*") if item.is_file()):
        stat = path.stat()
        digest.update(path.relative_to(directory).as_posix().encode("utf-8"))
        digest.update(str(stat.st_size).encode("ascii"))
        digest.update(str(stat.st_mtime_ns).encode("ascii"))


def snapshot_cache_key(
    commit: str,
    cli: Path,
    args: argparse.Namespace,
    system_dict: Path,
    dict_source_dir: Path,
    dict_dir: Path,
) -> str:
    digest = hashlib.sha256()
    digest.update(commit.encode("ascii"))
    digest.update((args.chapter or "").encode("utf-8"))
    digest.update(args.corpus_id.encode("utf-8"))
    for path in (args.source, args.profile, cli, system_dict):
        update_file_digest(digest, path.resolve())
    update_directory_fingerprint(digest, dict_source_dir.resolve())
    update_directory_fingerprint(digest, dict_dir.resolve())
    return digest.hexdigest()


def copy_snapshot_tree(source: Path, target: Path) -> None:
    def link_or_copy(source_file: str, target_file: str) -> str:
        try:
            os.link(source_file, target_file)
            return target_file
        except OSError:
            return shutil.copy2(source_file, target_file)

    shutil.copytree(source, target, copy_function=link_or_copy)


def store_snapshot_cache(source: Path, target: Path) -> None:
    """完整复制后再发布缓存条目，半成品永远不会以正式 key 出现。"""
    target.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=".snapshot-cache-", dir=target.parent))
    try:
        shutil.rmtree(staging)
        copy_snapshot_tree(source, staging)
        try:
            os.replace(staging, target)
        except OSError:
            if not target.exists():
                raise
    finally:
        if staging.exists():
            shutil.rmtree(staging)


def iter_json_object_array(path: Path, field: str) -> Iterable[object]:
    """增量读取顶层 JSON 对象中的数组字段，避免加载 reading-diff 全文。"""
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

        def skip_space() -> None:
            nonlocal buffer, position
            while True:
                while position < len(buffer) and buffer[position].isspace():
                    position += 1
                if position < len(buffer) or end_of_file:
                    return
                buffer = ""
                position = 0
                fill()

        def decode_value() -> object:
            nonlocal buffer, position
            while True:
                try:
                    value, next_position = decoder.raw_decode(buffer, position)
                except json.JSONDecodeError:
                    if end_of_file:
                        raise ValueError(f"JSON 值不完整：{path}")
                    if position:
                        buffer = buffer[position:]
                        position = 0
                    fill()
                    continue
                position = next_position
                return value

        fill()
        skip_space()
        if position >= len(buffer) or buffer[position] != "{":
            raise ValueError(f"reading-diff 不是顶层 JSON 对象：{path}")
        position += 1
        while True:
            skip_space()
            if position >= len(buffer):
                raise ValueError(f"reading-diff 意外结束：{path}")
            if buffer[position] == "}":
                return
            key = decode_value()
            if not isinstance(key, str):
                raise ValueError(f"reading-diff 键不是字符串：{path}")
            skip_space()
            if position >= len(buffer) or buffer[position] != ":":
                raise ValueError(f"reading-diff 键缺少冒号：{path}")
            position += 1
            skip_space()
            if key != field:
                decode_value()
            else:
                if position >= len(buffer) or buffer[position] != "[":
                    raise ValueError(f"reading-diff.{field} 不是数组：{path}")
                position += 1
                while True:
                    skip_space()
                    if position >= len(buffer):
                        raise ValueError(f"reading-diff.{field} 意外结束：{path}")
                    if buffer[position] == "]":
                        position += 1
                        break
                    yield decode_value()
                    skip_space()
                    if position < len(buffer) and buffer[position] == ",":
                        position += 1
                        continue
                    if position < len(buffer) and buffer[position] == "]":
                        position += 1
                        break
                    raise ValueError(f"reading-diff.{field} 数组缺少分隔符：{path}")
            skip_space()
            if position < len(buffer) and buffer[position] == ",":
                position += 1
                continue
            if position < len(buffer) and buffer[position] == "}":
                return
            if end_of_file:
                raise ValueError(f"reading-diff 对象缺少结束符：{path}")


def capture_dictionary_lookups(
    cli: Path,
    reading_diff: Path,
    side: str,
    input_path: Path,
    output_path: Path,
    repo: Path,
    system_dict: Path,
    dict_source_dir: Path,
    dict_dir: Path,
) -> int:
    seen_request_ids: set[str] = set()
    request_count = 0
    with input_path.open("w", encoding="utf-8", newline="\n") as input_stream:
        input_stream.write("[")
        for unit in iter_json_object_array(reading_diff, "units"):
            if not isinstance(unit, dict):
                continue
            for token in unit.get(side, {}).get("tokens", []):
                request = token.get("lookup_request")
                request_id = token.get("lookup_request_id")
                if not isinstance(request, dict) or not request_id or not request.get("word"):
                    continue
                request_id = str(request_id)
                if request_id in seen_request_ids:
                    continue
                seen_request_ids.add(request_id)
                if request_count:
                    input_stream.write(",")
                json.dump(
                    {
                        "request_id": request_id,
                        "word": request.get("word", ""),
                        "observed_form": request.get("observed_form"),
                        "reading": request.get("reading"),
                        "pos": request.get("pos"),
                        "selected_form": request.get("selected_form"),
                    },
                    input_stream,
                    ensure_ascii=False,
                    separators=(",", ":"),
                )
                request_count += 1
        input_stream.write("]\n")
    run(
        [
            str(cli),
            "dictionary-lookup-batch",
            "--input",
            str(input_path),
            "--json",
            str(output_path),
            "--system-dict",
            str(system_dict),
            "--dict-source-dir",
            str(dict_source_dir),
            "--dict-dir",
            str(dict_dir),
        ],
        cwd=repo,
    )
    return request_count


def supports_dictionary_lookup_batch(cli: Path, cwd: Path) -> bool:
    """旧提交可能尚未包含批量查询命令；此时保留明确的未捕获状态。"""
    result = subprocess.run(
        [str(cli), "help"],
        cwd=cwd,
        text=True,
        encoding="utf-8",
        errors="backslashreplace",
        capture_output=True,
    )
    return result.returncode == 0 and "dictionary-lookup-batch" in result.stdout


def compress_file(source: Path, target: Path) -> None:
    with source.open("rb") as input_stream, target.open("wb") as output_stream:
        with gzip.GzipFile(
            fileobj=output_stream,
            mode="wb",
            compresslevel=ARTIFACT_GZIP_LEVEL,
            mtime=0,
        ) as compressed:
            shutil.copyfileobj(input_stream, compressed, length=1024 * 1024)
    source.unlink()


def embed_snapshot_metadata(diff_manifest_path: Path, before: Path, after: Path) -> None:
    """将历史索引所需元数据内嵌到 diff manifest，随后可删除完整快照。"""
    manifest = json.loads(diff_manifest_path.read_text(encoding="utf-8"))
    for side, path in (("before", before), ("after", after)):
        snapshot_manifest = json.loads(path.read_text(encoding="utf-8"))
        descriptor = manifest.get(side)
        if not isinstance(descriptor, dict):
            raise ValueError(f"diff manifest 缺少 {side} 描述")
        descriptor["snapshot_metadata"] = {
            key: snapshot_manifest.get(key)
            for key in (
                "created_at",
                "implementation",
                "corpus",
                "resources",
            )
        }
        descriptor.pop("path", None)
    diff_manifest_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )


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
    parser.add_argument(
        "--snapshot-cache-root",
        type=Path,
        help="独立、可重建且由统一入口限额的完成快照缓存",
    )
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
    if diff_output.exists():
        raise FileExistsError(f"提交比较已有 diff，拒绝覆盖：{diff_output}")
    for side_output in (before_output, after_output):
        if side_output.exists() and not side_output.is_dir():
            raise FileExistsError(f"提交比较快照路径不是目录：{side_output}")
    rust_diff = resolve_quality_diff(repo)
    diff_command = [
        str(rust_diff),
        "compare",
        "--before-run",
        str(before_output / "manifest.json"),
        "--after-run",
        str(after_output / "manifest.json"),
        "--output-dir",
        str(diff_output),
        "--python-script",
        str(tool_root / "language_quality_diff.py"),
        "--python",
        sys.executable,
        "--keep-reading-spool",
    ]

    args.snapshot_artifact_store = output / ".snapshot-artifact-store"
    with tempfile.TemporaryDirectory(prefix="kotoclip-quality-commits-") as temporary:
        temporary_root = Path(temporary)
        try:
            stable_clis: dict[str, Path] = {}
            worktrees: dict[str, Path] = {}
            shared_target = (repo / "target").resolve()
            for name, commit in (("before", args.before), ("after", args.after)):
                worktree = temporary_root / f"worktree-{name}"
                run(["git", "worktree", "add", "--detach", "--force", str(worktree), commit], cwd=repo)
                worktrees[name] = worktree
                cli = build_cli(
                    worktree,
                    shared_target,
                    output / f"build-{name}.log",
                    args.build_profile,
                )
                stable_cli = temporary_root / "bin" / name / cli.name
                stable_cli.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(cli, stable_cli)
                stable_clis[name] = stable_cli

            def snapshot_side(name: str, commit: str) -> None:
                side_output = before_output if name == "before" else after_output
                side_system = before_system_dict if name == "before" else after_system_dict
                side_source = before_dict_source_dir if name == "before" else after_dict_source_dir
                side_cache = before_dict_dir if name == "before" else after_dict_dir
                cache_path: Path | None = None
                if args.snapshot_cache_root is not None:
                    cache_path = args.snapshot_cache_root.resolve() / snapshot_cache_key(
                        commit,
                        stable_clis[name],
                        args,
                        side_system,
                        side_source,
                        side_cache,
                    )
                    if (cache_path / "manifest.json").is_file():
                        copy_snapshot_tree(cache_path, side_output)
                        os.utime(cache_path, None)
                        print(f"{name} 全库端点缓存命中：{cache_path.name}")
                        return
                snapshot(
                    tool_root,
                    worktrees[name],
                    stable_clis[name],
                    args,
                    side_output,
                    f"{name}-{commit}",
                    side_system,
                    side_source,
                    side_cache,
                )
                if cache_path is not None:
                    if not cache_path.exists():
                        store_snapshot_cache(side_output, cache_path)

            try:
                with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
                    futures = [
                        executor.submit(snapshot_side, name, commit)
                        for name, commit in (("before", args.before), ("after", args.after))
                    ]
                    for future in futures:
                        future.result()
            finally:
                for worktree in worktrees.values():
                    subprocess.run(
                        ["git", "worktree", "remove", "--force", str(worktree)],
                        cwd=repo,
                        text=True,
                        encoding="utf-8",
                        errors="backslashreplace",
                        capture_output=True,
                    )
            before_cli = stable_clis["before"]
            after_cli = stable_clis["after"]
            run(diff_command, cwd=repo)
            lookup_capture: dict[str, object] = {
                "schema_version": "kotoclip.quality.dictionary-lookup-capture.v1",
                "before": {"status": "unsupported_cli"},
                "after": {"status": "unsupported_cli"},
            }
            lookup_sides = (
                ("before", before_cli, before_system_dict, before_dict_source_dir, before_dict_dir),
                ("after", after_cli, after_system_dict, after_dict_source_dir, after_dict_dir),
            )

            def capture_side(
                side: str,
                cli: Path,
                system_dict: Path,
                source_dir: Path,
                dict_dir: Path,
            ) -> tuple[str, dict[str, object]]:
                if not supports_dictionary_lookup_batch(cli, repo):
                    print(f"{side} 提交未提供 dictionary-lookup-batch，保留未捕获状态")
                    return side, {"status": "unsupported_cli"}
                raw_lookup_path = temporary_root / f"dictionary-lookups-{side}.json"
                request_count = capture_dictionary_lookups(
                    cli,
                    diff_output / ".reading-diff-spool.json.gz",
                    side,
                    temporary_root / f"dictionary-requests-{side}.json",
                    raw_lookup_path,
                    repo,
                    system_dict,
                    source_dir,
                    dict_dir,
                )
                compress_file(raw_lookup_path, diff_output / f"dictionary-lookups-{side}.json.gz")
                return side, {"status": "captured", "request_count": request_count}

            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
                futures = [executor.submit(capture_side, *values) for values in lookup_sides]
                for future in futures:
                    side, result = future.result()
                    lookup_capture[side] = result
            (diff_output / "dictionary-lookup-capture.json").write_text(
                json.dumps(lookup_capture, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
                newline="\n",
            )
            embed_snapshot_metadata(
                diff_output / "manifest.json",
                before_output / "manifest.json",
                after_output / "manifest.json",
            )
            (diff_output / ".reading-diff-spool.json.gz").unlink(missing_ok=True)
        finally:
            git(repo, "worktree", "prune")

    gate_returncode = 0
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
        gate_returncode = gate.returncode
    for transient in (before_output, after_output, args.snapshot_artifact_store):
        if transient.exists():
            shutil.rmtree(transient)
    print(f"提交比较完成：{diff_output}")
    return gate_returncode


if __name__ == "__main__":
    raise SystemExit(main())
