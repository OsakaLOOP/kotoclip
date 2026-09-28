"""验证阅读器完整单元、重启缓存和耗时诊断，并保存测量结果。"""

import argparse
import json
import subprocess
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_BINARY = ROOT / "target" / "debug" / "kotoclip-nlp.exe"
DEFAULT_REPORT = ROOT / ".agents" / "analysis" / "reader-pipeline-test.json"
TEXT = "七日は警察署へ向かった。彼女は窓の外を見て、ゆっくり歩いている。\n" * 48


class Service:
    def __init__(self, binary):
        self.process = subprocess.Popen(
            [str(binary), "stdio"], cwd=ROOT, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8",
        )

    def request(self, command, **fields):
        self.process.stdin.write(json.dumps({"command": command, **fields}, ensure_ascii=False) + "\n")
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f"分析进程提前退出：{self.process.stderr.read()}")
        response = json.loads(line)
        if response.get("error"):
            raise RuntimeError(f"{command}: {response['error']}")
        return response["result"]

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=10)
        self.process.stdout.close()
        self.process.stderr.close()


def collect(binary, text, initial_offset=0, cached=False):
    service = Service(binary)
    try:
        started = time.perf_counter()
        opened = service.request("open_document", document_id="reader-pipeline-test", text=text,
                                 policy="auto", initial_offset=initial_offset)
        update = opened["update"]
        plan = opened["plan"]
        initial_progress = update["progress"]
        total = len(plan["units"])
        analysis_total, cache_total = (0, total) if cached else (total, 0)
        assert initial_progress["analysis"] == {"complete": 0, "total": analysis_total}, initial_progress
        assert initial_progress["cache"] == {"complete": 0, "total": cache_total}, initial_progress
        expected = next(unit["id"] for unit in plan["units"]
                        if unit["anchor"]["char_range"][0] <= initial_offset < unit["anchor"]["char_range"][1])
        session = update["session_id"]
        version = update["text_version"]
        generation = update["generation"]
        known = {unit["unit_id"]: unit for unit in update["changes"]}
        update = service.request("continue_document", session_id=session, text_version=version, generation=generation)
        generation = update["generation"]
        known.update((unit["unit_id"], unit) for unit in update["changes"])
        first = None
        deadline = time.monotonic() + 240
        while time.monotonic() < deadline:
            previous_revision = update["revision"]
            update = service.request("poll_document", session_id=session, text_version=version,
                                     generation=generation, after_revision=previous_revision)
            assert update["progress"]["analysis"]["total"] == analysis_total, update["progress"]
            assert update["progress"]["cache"]["total"] == cache_total, update["progress"]
            if update["snapshot"]:
                known = {unit["unit_id"]: unit for unit in update["changes"]}
            else:
                known.update((unit["unit_id"], unit) for unit in update["changes"])
            changes = [unit for unit in update["changes"] if unit["stage"] == "complete"]
            if changes and first is None:
                first = changes[0]["unit_id"]
            if update["progress"]["failed"]:
                raise AssertionError(str([(unit["unit_id"], unit["error"]) for unit in known.values() if unit["stage"] == "failed"]))
            if update["progress"]["complete"] == update["progress"]["total"]:
                break
            time.sleep(0.15)
        else:
            raise TimeoutError("单元完整分析超过 240 秒")
        completed = [unit for unit in known.values() if unit["stage"] == "complete"]
        assert update["progress"]["analysis"]["complete"] == analysis_total, update["progress"]
        assert update["progress"]["cache"]["complete"] == cache_total, update["progress"]
        assert first == expected, f"首个完成单元 {first} 与优先单元 {expected} 不同"
        assert len(completed) == len(plan["units"])
        assert all(unit["document"] and unit["document"]["external_sources"] for unit in completed)
        assert all(unit["lookup"] and unit["lookup"]["outer_targets"] for unit in completed)
        assert all(unit["timing"] is None or unit["timing"]["elapsed_ms"] > 0 for unit in completed)
        target = next(target for target in completed[0]["lookup"]["outer_targets"] if target["matrix_request"])
        query_started = time.perf_counter()
        service.request("query_lookup_document", session_id=session, text_version=version,
                        generation=generation, unit_id=completed[0]["unit_id"],
                        artifact_revision=completed[0]["artifact_revision"], target_id=target["id"])
        query_ms = round((time.perf_counter() - query_started) * 1000, 2)
        return {
            "wall_ms": round((time.perf_counter() - started) * 1000, 2),
            "query_ms": query_ms,
            "first_unit": first,
            "total": len(completed),
            "initial_progress": initial_progress,
            "final_progress": update["progress"],
            "units": [{"unit_id": unit["unit_id"], "stage": unit["stage"],
                       "cache_hit": unit["cache_hit"],
                       "lookup_targets": len(unit["lookup"]["outer_targets"]),
                       "timing": unit["timing"], "providers": unit["providers"]} for unit in completed],
        }
    finally:
        service.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    args = parser.parse_args()
    nonce = time.time_ns()
    text = "".join(f"検証番号{nonce}第{index}。" + TEXT[index:index + 500]
                   for index in range(0, len(TEXT), 500))
    cold = collect(args.binary, text, 850)
    assert not any(unit["cache_hit"] for unit in cold["units"]), "新正文应从完整分析开始"
    warm = collect(args.binary, text, 850, cached=True)
    assert all(unit["cache_hit"] for unit in warm["units"]), "重启后完整产物未全部命中缓存"
    service = Service(args.binary)
    try:
        service.request("set_analysis_timing", enabled=False)
    finally:
        service.close()
    try:
        disabled = collect(args.binary, text, 850, cached=True)
        assert all(unit["timing"] is None and unit["cache_hit"] for unit in disabled["units"])
    finally:
        service = Service(args.binary)
        try:
            service.request("set_analysis_timing", enabled=True)
        finally:
            service.close()
    report = {"cold": cold, "warm": warm, "timing_disabled": disabled}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"通过：{cold['total']} 个完整单元；冷启动 {cold['wall_ms']} ms；缓存重启 {warm['wall_ms']} ms；报告 {args.report}")


if __name__ == "__main__":
    main()
