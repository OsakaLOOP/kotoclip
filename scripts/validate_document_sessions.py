"""用指定语料短段验证会话调度、前台查询、取消、缓存及长文档定位。"""
import json
import os
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
import time

from probe_nlp_behavior import ROOT, samples


def main():
    report = {"schema": "kotoclip.document-session-check.v1"}
    with TemporaryDirectory(prefix="kotoclip-session-", dir=ROOT / "experiments") as data:
        process = subprocess.Popen([str(ROOT / "target/debug/kotoclip-nlp.exe"), "stdio"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, encoding="utf-8",
            env={**os.environ, "KOTOCLIP_DATA_DIR": data})

        def raw(value):
            process.stdin.write(json.dumps(value, ensure_ascii=False) + "\n"); process.stdin.flush()
            return json.loads(process.stdout.readline())

        def request(value):
            response = raw(value)
            assert response["error"] is None, response
            return response["result"]

        def bound(update, command, **payload):
            return {"command": command, "session_id": update["session_id"], "text_version": update["text_version"], "generation": update["generation"], **payload}

        def wait_for(update, predicate, timeout=90):
            units = {u["unit_id"]: u for u in update["changes"]}
            deadline = time.monotonic() + timeout
            while not predicate(update, units):
                assert time.monotonic() < deadline, {"timeout": update["progress"], "units": [(u['stage'], u['error']) for u in units.values()]}
                time.sleep(.1)
                patch = request(bound(update, "poll_document", after_revision=update["revision"]))
                assert patch["snapshot"] or patch["base_revision"] == update["revision"]
                if patch["snapshot"]: units.clear()
                units.update((u["unit_id"], u) for u in patch["changes"])
                update = patch
            return update, units

        try:
            settings = request({"command": "provider_status"})["settings"]
            # 查询使用仓库已准备的三词典，用户配置仍写入本次临时目录。
            dictionary_sources = ROOT / "data/dict-sources"
            dictionary_cache = Path(data) / "dict-sources"
            dictionary_cache.mkdir()
            for source in dictionary_sources.glob('*.kdict'):
                os.link(source, dictionary_cache / source.name)
            request({"command": "search", "word": "警察"})
            cases = {case["id"]: case["text"] for case in samples()[1]}
            text = cases["news"] + "\n" + cases["contraction"]
            started = time.monotonic()
            opened = request({"command": "open_document", "document_id": "session-fixture", "text": text, "policy": "auto"})
            report["open_ms"] = (time.monotonic() - started) * 1000
            update, units = wait_for(opened["update"], lambda _, units: any(u["stage"] == "enriching" for u in units.values()))
            unit = next(u for u in units.values() if u["document"])
            token = next(t for t in unit["document"]["morphemes"] if len(t["surface"]) > 1)
            query = bound(update, "query_document", unit_id=unit["unit_id"], artifact_revision=unit["artifact_revision"], token_id=token["id"])
            started = time.monotonic(); result = request(query)
            report["query_during_model_ms"] = (time.monotonic() - started) * 1000
            assert report["query_during_model_ms"] < 3000
            assert result["target"]["artifact_revision"] == unit["artifact_revision"]
            old_generation = update["generation"]
            update = request(bound(update, "cancel_document"))
            assert update["paused"] and update["generation"] > old_generation
            assert raw(query)["error"] is not None
            assert raw(bound(update, "poll_document", after_revision=update["revision"], text_version="wrong"))["error"] is not None
            target = opened["plan"]["units"][-1]
            update = request(bound(update, "request_range", range=target["anchor"]["char_range"]))
            update, units = wait_for(update, lambda update, _: update["progress"]["complete"] == update["progress"]["total"])
            assert all(u["stage"] == "complete" for u in units.values())
            canonical = {key: value["document"] for key, value in units.items()}
            assert any(len(d["source"]["runs"]) > 1 for d in canonical.values())
            print("真实来源、查询并发及取消恢复通过", flush=True)
            request({"command": "close_document", "session_id": update["session_id"]})
            assert raw(query)["error"] == "文档会话已关闭"
            warm = request({"command": "open_document", "document_id": "session-fixture", "text": text, "policy": "auto"})
            update, units = wait_for(warm["update"], lambda update, _: update["progress"]["complete"] == update["progress"]["total"])
            assert canonical == {key: value["document"] for key, value in units.items()}
            assert all(p["cache_hit"] for u in units.values() for p in u["providers"])
            report.update(real_units=len(units), cold_warm_equal=True, stale_query_rejected=True, wrong_text_rejected=True, cancellation_recovery=True)
            settings["ginza"]["enabled"] = False; settings["kwja"]["enabled"] = False
            request({"command": "configure_providers", "settings": settings})
            long_text = "甲。\n" * 7000
            large = request({"command": "open_document", "document_id": "synthetic-plan", "text": long_text, "policy": "cwj"})
            update = request(bound(large["update"], "cancel_document"))
            assert len(large["plan"]["units"]) == 7000
            assert raw(bound(update, "request_range", range=[0, len(long_text) + 1]))["error"]
            snapshot = request({"command": "sync_document", "session_id": update["session_id"]})
            assert snapshot["generation"] == update["generation"]
            tail = large["plan"]["units"][-1]
            update = request(bound(update, "request_range", range=tail["anchor"]["char_range"]))
            update, units = wait_for(update, lambda _, units: units[tail["id"]]["stage"] == "complete", timeout=15)
            assert units[tail["id"]]["document"]["text"].endswith("甲。\n")
            update = request(bound(update, "continue_document"))
            update = request(bound(update, "cancel_document"))
            assert update["paused"]
            report.update(planned_characters=len(long_text), planned_units=7000, tail_range_verified=True, invalid_range_atomic=True, continue_cancel=True)
            print("缓存一致性、长文档规划及尾部定位通过", flush=True)
        finally:
            process.stdin.close(); process.wait(timeout=30)
    (ROOT / "data/validation/behavior/sessions.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
