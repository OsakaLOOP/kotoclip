"""按段落划分小说正文，采集十段 P4 性能数据。"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import platform
import queue
import statistics
import subprocess
import sys
import threading
import time

import psutil


ROOT = Path(__file__).resolve().parents[1]
SOURCE = Path(r"D:\Downloads\epub-exp\source\七日の喰い神 (ガガガ文庫) (カミツキレイニー)\output.md")
OUTPUT = ROOT / ".agents/analysis/p4-novel-10.json"


def partition_novel(path: Path) -> tuple[str, list[tuple[int, int, str]]]:
    original = path.read_text(encoding="utf-8")
    chapter = original[original.index("## 第一話"):original.index("## 第二話")]
    text = "\n".join(line for line in chapter.splitlines()
                     if not line.startswith(("## ", "![](./"))).strip() + "\n"
    boundaries = [index + 2 for index in range(len(text) - 1) if text[index:index + 2] == "\n\n"]
    end = min((index for index in boundaries if 9700 <= index <= 10300),
              key=lambda index: abs(index - 10000))
    text = text[:end]
    points = [0]
    for number in range(1, 10):
        target = len(text) * number / 10
        choices = (index for index in boundaries if points[-1] + 700 <= index <= end - (10 - number) * 700)
        points.append(min(choices, key=lambda index: abs(index - target)))
    points.append(end)
    parts = [(start, finish, text[start:finish]) for start, finish in zip(points, points[1:])]
    assert len(parts) == 10 and "".join(part for _, _, part in parts) == text
    return text, parts


def distribution(values: list[float]) -> dict | None:
    if not values:
        return None
    ordered = sorted(values)
    return {"count": len(values), "total": round(sum(values), 2),
            "min": round(ordered[0], 2), "median": round(statistics.median(ordered), 2),
            "p95": round(ordered[(95 * len(values) + 99) // 100 - 1], 2),
            "max": round(ordered[-1], 2)}


def sample_resources(process: subprocess.Popen, stop: threading.Event, metrics: dict):
    cpu_by_pid = {}
    while not stop.wait(0.25):
        try:
            children = [psutil.Process(process.pid), *psutil.Process(process.pid).children(recursive=True)]
        except psutil.Error:
            break
        total = 0
        for child in children:
            try:
                with child.oneshot():
                    memory = child.memory_info().rss
                    times = child.cpu_times()
                    cpu_by_pid[child.pid] = max(cpu_by_pid.get(child.pid, 0), times.user + times.system)
                    name = child.name()
                total += memory
                entry = metrics["processes"].setdefault(str(child.pid), {"name": name, "peak_rss_bytes": 0})
                entry["peak_rss_bytes"] = max(entry["peak_rss_bytes"], memory)
            except psutil.Error:
                continue
        metrics["peak_total_rss_bytes"] = max(metrics["peak_total_rss_bytes"], total)
        metrics["peak_process_count"] = max(metrics["peak_process_count"], len(children))
    metrics["sampled_cpu_seconds"] = round(sum(cpu_by_pid.values()), 2)


def summarize(report: dict):
    stages = defaultdict(list)
    providers = defaultdict(lambda: {"ms": [], "cache_hits": 0, "failures": 0})
    for part in report["partitions"]:
        for unit in part["units"]:
            if unit["stage"] != "complete":
                continue
            for timing in unit["stage_timings"]:
                stages[timing["stage"]].append(timing["elapsed_ms"])
            for provider in unit["providers"]:
                item = providers[provider["id"]]
                if "elapsed_ms" in provider:
                    item["ms"].append(provider["elapsed_ms"])
                item["cache_hits"] += int(provider.get("cache_hit", False))
                item["failures"] += int(provider["status"] != "ready")
    report["summary"] = {
        "partition_wall_ms": distribution([part["wall_ms"] for part in report["partitions"]]),
        "unit_counts": dict(Counter(unit["stage"] for part in report["partitions"] for unit in part["units"])),
        "analyzed_chars_with_context": sum(unit.get("chars", 0) for part in report["partitions"] for unit in part["units"]),
        "tokens": sum(unit.get("tokens", 0) for part in report["partitions"] for unit in part["units"]),
        "chains": sum(unit.get("chains", 0) for part in report["partitions"] for unit in part["units"]),
        "formations": sum(unit.get("formations", 0) for part in report["partitions"] for unit in part["units"]),
        "stages_ms": {name: distribution(values) for name, values in stages.items()},
        "providers": {name: {"ms": distribution(value["ms"]), "cache_hits": value["cache_hits"],
                             "failures": value["failures"]} for name, value in providers.items()},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/kotoclip-nlp.exe")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    text, partitions = partition_novel(args.source)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "schema": "kotoclip.p4-novel-performance.v1",
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "source_path": str(args.source),
        "source_sha256": hashlib.sha256(args.source.read_bytes()).hexdigest(),
        "text_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
        "text_chars": len(text),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "machine": {"platform": platform.platform(), "cpu": platform.processor(),
                    "logical_cpus": psutil.cpu_count(), "ram_bytes": psutil.virtual_memory().total},
        "method": "第一话前约一万字符，按空段边界划分十份；每份通过真实文档会话完成 P4 基础与来源分析，进程和模型复用。",
        "partitions": [], "resources": {"peak_total_rss_bytes": 0, "peak_process_count": 0, "processes": {}},
        "ipc": {"sent_bytes": 0, "received_bytes": 0, "responses": 0},
    }
    started = time.monotonic()
    responses = queue.Queue(maxsize=2)
    stop = threading.Event()
    process = None
    error = None

    def save():
        summarize(report)
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    try:
        with args.output.with_suffix(".stderr.log").open("w", encoding="utf-8") as stderr:
            process = subprocess.Popen([str(args.binary), "stdio"], cwd=ROOT, stdin=subprocess.PIPE,
                                       stdout=subprocess.PIPE, stderr=stderr, encoding="utf-8", bufsize=1)

            def receive_lines():
                for line in process.stdout:
                    responses.put(line)
                responses.put(None)

            threading.Thread(target=receive_lines, daemon=True).start()
            monitor = threading.Thread(target=sample_resources, args=(process, stop, report["resources"]), daemon=True)
            monitor.start()

            def request(command, **kwargs):
                line = json.dumps({"command": command, **kwargs}, ensure_ascii=False) + "\n"
                report["ipc"]["sent_bytes"] += len(line.encode("utf-8"))
                process.stdin.write(line)
                process.stdin.flush()
                response_line = responses.get(timeout=240)
                if response_line is None:
                    raise RuntimeError(f"服务提前退出：{process.poll()}")
                report["ipc"]["received_bytes"] += len(response_line.encode("utf-8"))
                report["ipc"]["responses"] += 1
                response = json.loads(response_line)
                if response["error"]:
                    raise RuntimeError(f"{command}: {response['error']}")
                return response["result"]

            report["provider_status"] = request("provider_status")
            for number, (start, end, part_text) in enumerate(partitions, 1):
                part_started = time.monotonic()
                opened = request("open_document", document_id=f"p4-novel-{number:02}",
                                 text=part_text, policy="auto")
                plan = opened["plan"]["units"]
                ranges = [unit["anchor"]["char_range"] for unit in plan]
                prepared_chars = len(opened["plan"]["prepared"]["text"])
                if ranges[0][0] != 0 or ranges[-1][1] != prepared_chars or any(
                    previous[1] != current[0] or current[1] - current[0] > 768
                    for previous, current in zip(ranges, ranges[1:])
                ):
                    raise ValueError(f"第 {number} 段分窗范围不连续")
                part = {"index": number, "char_range": [start, end], "source_chars": len(part_text),
                        "prepared_chars": prepared_chars, "text_sha256": hashlib.sha256(part_text.encode("utf-8")).hexdigest(),
                        "plan_ranges": ranges, "units": [], "wall_ms": 0}
                report["partitions"].append(part)
                unit_ids = {unit["id"]: index for index, unit in enumerate(plan)}
                finished = {}

                def collect(update):
                    for item in update["changes"]:
                        document = item["document"]
                        result = {"index": unit_ids[item["unit_id"]], "stage": item["stage"],
                                  "error": item["error"], "providers": item["providers"]}
                        if document:
                            result.update(chars=len(document["text"]), tokens=len(document["morphemes"]),
                                          chains=len(document["morphology"]["chains"]),
                                          formations=len(document["formation"]["nodes"]),
                                          morphology_diagnostics=len(document["morphology"]["diagnostics"]),
                                          stage_timings=document["stage_timings"])
                        finished[item["unit_id"]] = result

                update = opened["update"]
                collect(update)
                deadline = time.monotonic() + 600
                while update["progress"]["complete"] + update["progress"]["failed"] < len(plan):
                    if time.monotonic() > deadline:
                        raise TimeoutError(f"第 {number} 段超过十分钟")
                    if update["progress"]["pending"] == 0:
                        update = request("continue_document", session_id=update["session_id"],
                                         text_version=update["text_version"], generation=update["generation"])
                    else:
                        time.sleep(0.2)
                        update = request("poll_document", session_id=update["session_id"],
                                         text_version=update["text_version"], generation=update["generation"],
                                         after_revision=update["revision"])
                    collect(update)
                part["units"] = sorted(finished.values(), key=lambda unit: unit["index"])
                part["wall_ms"] = round((time.monotonic() - part_started) * 1000, 2)
                part["progress"] = update["progress"]
                save()
                print(f"第 {number}/10 段：{len(part_text)} 字符、{len(plan)} 窗口、{part['wall_ms']:.0f} ms", flush=True)
                if update["progress"]["failed"]:
                    raise RuntimeError(f"第 {number} 段有失败窗口")
                request("close_document", session_id=update["session_id"])
    except Exception as exception:
        error = str(exception)
    finally:
        stop.set()
        if process:
            process.stdin.close()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            monitor.join(timeout=1)
        report["elapsed_seconds"] = round(time.monotonic() - started, 2)
        if error:
            report["error"] = error
        save()
        print(f"测量结果：{args.output}；耗时 {report['elapsed_seconds']} 秒", flush=True)
    if error:
        raise RuntimeError(error)


if __name__ == "__main__":
    main()
