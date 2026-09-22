"""验证 Rust 服务到两项本机模型的短文本调用、复用及恢复。"""
from __future__ import annotations

import json
import argparse
import os
from pathlib import Path
import subprocess
import tempfile
import time

from probe_nlp_behavior import ROOT, samples


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cases", nargs="*")
    parser.add_argument("--output", type=Path, default=ROOT / "data/validation/behavior/integration.json")
    args = parser.parse_args()
    report = {"schema": "kotoclip.provider-integration-check.v1", "cases": []}
    with tempfile.TemporaryDirectory(prefix="kotoclip-provider-") as data:
        env = {**os.environ, "KOTOCLIP_DATA_DIR": data}
        process = subprocess.Popen([str(ROOT / "target/debug/kotoclip-nlp.exe"), "stdio"],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, encoding="utf-8", env=env)
        def request(value):
            process.stdin.write(json.dumps(value, ensure_ascii=False) + "\n")
            process.stdin.flush()
            response = json.loads(process.stdout.readline())
            if response["error"]:
                raise RuntimeError(response["error"])
            return response["result"]
        try:
            cases = [case for case in samples()[1] if not args.cases or case["id"] in args.cases]
            for case in cases:
                started = time.perf_counter()
                base = request({"command": "analyze", "text": case["text"], "register": "cwj"})
                result = request({"command": "enrich", "analysis_id": base["id"]})
                assert all(p["status"] == "ready" for p in result["providers"]), result["providers"]
                sources = result["document"]["external_sources"]
                assert {s["provider"]["id"] for s in sources} == {"ginza", "kwja"}
                report["cases"].append({"id": case["id"], "characters": len(case["text"]),
                    "elapsed_ms": (time.perf_counter() - started) * 1000, "providers": result["providers"],
                    "sources": [{"provider": s["provider"], "nodes": len(s["nodes"]), "relations": len(s["relations"]), "deleted_ranges": s["deleted_ranges"]} for s in sources]})
                if case["id"] == "boundaries":
                    (ROOT / "data/validation/behavior/integration-boundaries.json").write_text(json.dumps(sources, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
                status = request({"command": "provider_status"})
                pids = {p["id"]: p["pid"] for p in status["providers"]}
                if "pids" in report:
                    assert report["pids"] == pids, "连续请求应复用模型进程"
                report["pids"] = pids
                print(case["id"], "通过", flush=True)
            settings = status["settings"]
            original = settings["ginza"]["python"]
            settings["ginza"]["python"] = str(Path(data) / "missing-python.exe")
            settings["kwja"]["enabled"] = False
            request({"command": "configure_providers", "settings": settings})
            failed = request({"command": "enrich", "analysis_id": base["id"]})
            assert failed["providers"][0]["status"] == "failed"
            assert failed["document"]["morphemes"] == base["morphemes"]
            settings["ginza"]["python"] = original
            request({"command": "configure_providers", "settings": settings})
            recovered = request({"command": "enrich", "analysis_id": base["id"]})
            assert recovered["providers"][0]["status"] == "ready"
            report["missing_interpreter_recovery"] = True
            # 排队后立即取消，覆盖任务启动前收到取消的情况。
            messages = [{"command": "enrich", "analysis_id": base["id"]}, {"command": "cancel_external"}]
            process.stdin.write("".join(json.dumps(message) + "\n" for message in messages))
            process.stdin.flush()
            cancelled = json.loads(process.stdout.readline())
            acknowledged = json.loads(process.stdout.readline())
            assert cancelled["result"]["providers"][0]["status"] == "cancelled", cancelled
            assert acknowledged["result"]["cancelled"]
            after_cancel = request({"command": "enrich", "analysis_id": base["id"]})
            assert after_cancel["providers"][0]["status"] == "ready"
            report["cancellation_recovery"] = True
            settings["ginza"]["timeout_seconds"] = 1
            request({"command": "configure_providers", "settings": settings})
            timed_out = request({"command": "enrich", "analysis_id": base["id"]})
            assert timed_out["providers"][0]["status"] == "failed"
            assert "超过" in timed_out["providers"][0]["error"]
            settings["ginza"]["timeout_seconds"] = 120
            request({"command": "configure_providers", "settings": settings})
            assert request({"command": "enrich", "analysis_id": base["id"]})["providers"][0]["status"] == "ready"
            report["timeout_recovery"] = True
            status = request({"command": "provider_status"})
            pid = next(p["pid"] for p in status["providers"] if p["id"] == "ginza")
            subprocess.run(["taskkill", "/PID", str(pid), "/F"], capture_output=True, check=True)
            # 新正文触发实际推理，确保异常退出检查经过进程通信。
            uncached = request({"command": "analyze", "text": "新しい本を読む。", "register": "cwj"})
            exited = request({"command": "enrich", "analysis_id": uncached["id"]})
            assert exited["providers"][0]["status"] == "failed"
            assert request({"command": "enrich", "analysis_id": uncached["id"]})["providers"][0]["status"] == "ready"
            report["unexpected_exit_recovery"] = True
        finally:
            process.stdin.close()
            process.wait(timeout=20)
            if process.returncode:
                raise RuntimeError(process.stderr.read())
    target = args.output
    target.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(target)


if __name__ == "__main__":
    main()
