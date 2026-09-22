"""通过文档会话采集十段指定语料，供逐句人工复核和 P4 设计使用。"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time
import sys
from validate_p4_language import validate

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "data/validation/p4-sample-review.json"
RAW = ROOT / "experiments/p4-sample-review"
QUERY_SURFACES = {"普選", "白南風", "雜", "霽れ", "すね", "頓着", "教科", "拡大", "扱わ", "サトイモ", "恐れ", "覗か", "てる", "後ろめた", "ゾワゾワゾワ"}


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compact(document):
    """保留原始实体身份、语言字段和模块结果，完整协议另存本机原始文件。"""
    result = {key: document[key] for key in (
        "schema", "id", "text", "routing", "morphemes", "gaps", "bunsetsu", "clause",
        "dictionary_candidates", "structure_diagnostics", "formation", "morphology", "grammar", "stage_timings",
    )}
    result["source_runs"] = document["source"]["runs"]
    result["unidic_fields"] = [{"id": f"m{token['index']}", "lexicon_type": token["lexicon_type"],
        "fields": {field["name"]: field["value"] for field in token["fields"] if field["value"] is not None}}
        for token in document["source"]["tokens"]]
    result["sources"] = []
    for source in document["external_sources"]:
        result["sources"].append({key: source[key] for key in ("provider", "nodes", "relations")})
    graph = document["structure_graph"]
    result["structure_selection"] = graph["candidates"]
    result["incomplete_entities"] = [entity for entity in graph["entities"] if not entity["complete"]]
    result["alignment_counts"] = {alignment["external_provider"]: len(alignment["groups"])
        for alignment in document["provider_token_alignments"]}
    return result


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    source_path = ROOT / "data/validation/refractor_source.txt"
    text = source_path.read_text(encoding="utf-8")
    paragraphs = text.splitlines()
    if len(paragraphs) != 10 or any(not paragraph for paragraph in paragraphs):
        raise ValueError("指定语料应包含十个非空段落")
    RAW.mkdir(parents=True, exist_ok=True)
    report = {"schema": "kotoclip.p4-sample-review.v1", "source": str(source_path.relative_to(ROOT)),
        "source_sha256": digest(source_path), "binary_sha256": digest(ROOT / "target/debug/kotoclip-nlp.exe"),
        "git_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, encoding="utf-8").strip(),
        "date": time.strftime("%Y-%m-%d"), "method": "逐段保留上下文，经 open_document / poll_document 获取真实三来源结果；语言判断另见人工复核文档。",
        "cases": [], "controls": []}
    with tempfile.TemporaryDirectory(prefix="p4-review-", dir=ROOT / "experiments") as data:
        # 词典源包只读复用，派生数据库在独立目录建立。
        dictionary_sources = Path(data) / "dict-sources"
        dictionary_sources.mkdir()
        for source in (ROOT / "data/dict-sources").glob("*.kdict"):
            os.link(source, dictionary_sources / source.name)
        with (RAW / "stderr.log").open("w", encoding="utf-8") as stderr:
            process = subprocess.Popen([str(ROOT / "target/debug/kotoclip-nlp.exe"), "stdio"],
                cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr, encoding="utf-8",
                env={**os.environ, "KOTOCLIP_DATA_DIR": data})
            responses = queue.Queue()

            def read_responses():
                for line in process.stdout:
                    responses.put(line)
                responses.put(None)

            threading.Thread(target=read_responses, daemon=True).start()

            def request(command, **payload):
                process.stdin.write(json.dumps({"command": command, **payload}, ensure_ascii=False) + "\n")
                process.stdin.flush()
                line = responses.get(timeout=240)
                if line is None:
                    raise RuntimeError("服务提前退出，请检查原始日志")
                response = json.loads(line)
                if response["error"]:
                    raise RuntimeError(response["error"])
                return response["result"]

            try:
                report["provider_configuration"] = request("provider_status")
                offset = 0
                for index, paragraph in enumerate(paragraphs, 1):
                    started = time.monotonic()
                    case_id = f"p{index:02}"
                    opened = request("open_document", document_id=f"p4-review-{case_id}", text=paragraph, policy="auto")
                    update = opened["update"]
                    units = {unit["unit_id"]: unit for unit in update["changes"]}
                    deadline = time.monotonic() + 240
                    while update["progress"]["complete"] < update["progress"]["total"]:
                        if time.monotonic() > deadline or any(unit["stage"] == "failed" for unit in units.values()):
                            raise RuntimeError({"id": case_id, "progress": update["progress"], "errors": [u["error"] for u in units.values()]})
                        time.sleep(.1)
                        update = request("poll_document", session_id=update["session_id"], text_version=update["text_version"],
                            generation=update["generation"], after_revision=update["revision"])
                        if update["snapshot"]:
                            units.clear()
                        units.update((unit["unit_id"], unit) for unit in update["changes"])
                    if len(units) != 1:
                        raise ValueError("本次段落复核预期每段一个单元")
                    unit = next(iter(units.values()))
                    document = unit["document"]
                    if document["preparation"]["source_text"] != paragraph or not all(p["status"] == "ready" for p in unit["providers"]):
                        raise ValueError({"id": case_id, "providers": unit["providers"]})
                    raw_path = RAW / f"{case_id}.json"
                    write_json(raw_path, {"plan": opened["plan"], "update": update, "unit": unit})
                    case = {"id": case_id, "paragraph": index, "source_char_range_lf": [offset, offset + len(paragraph)],
                        "elapsed_ms": round((time.monotonic() - started) * 1000), "plan": opened["plan"]["units"],
                        "providers": unit["providers"], "document": compact(document), "queries": []}
                    case["language_validation"] = validate(case_id, document)
                    case["whole_queries"] = []
                    for candidate in document["dictionary_candidates"]["candidates"]:
                        if candidate["kind"] == "token" or candidate["surface"] not in {"方向転換", "教科用図書", "神戸市", "線状降水帯", "扱われます", "這い上がる"}:
                            continue
                        query = request("query_candidate", analysis_id=document["id"], candidate_id=candidate["id"])
                        case["whole_queries"].append({"surface":candidate["surface"], "target":query["target"],
                            "dictionary_status":query["dictionary_status"], "forms":[group["form"] for group in query["groups"]]})
                    for token in document["morphemes"]:
                        if token["surface"] not in QUERY_SURFACES:
                            continue
                        query = request("query_document", session_id=update["session_id"], text_version=update["text_version"],
                            generation=update["generation"], unit_id=unit["unit_id"], artifact_revision=unit["artifact_revision"], token_id=token["id"])
                        query_path = RAW / f"{case_id}-{token['id']}-query.json"
                        write_json(query_path, query)
                        case["queries"].append({"token_id": token["id"], "surface": token["surface"],
                            "reading": query.get("reading"), "target": query["target"], "forms": query["forms"],
                            "groups": [{"form": group["form"], "total": group["total"],
                                "entries": [{key: entry[key] for key in ("headword", "reading", "dictionary_name") if key in entry}
                                    for entry in group["entries"]]} for group in query["groups"]]})
                    report["cases"].append(case)
                    write_json(OUTPUT, report)
                    request("close_document", session_id=update["session_id"])
                    offset += len(paragraph) + 1
                    print(case_id, json.dumps(case["language_validation"], ensure_ascii=False), flush=True)
                # 对照例用于界定既有表达 matcher 的连接条件和词汇／功能歧义。
                for case_id, control in [
                    ("idiom", "彼の話に耳を傾ける。"),
                    ("space", "彼の話に耳 を傾ける。"),
                    ("paragraph_gap", "彼の話に耳\n\nを傾ける。"),
                    ("literal", "子供の手を引く。"),
                    ("functional", "本を読んでいる。そこにいる。読んでくださった。静かな町だ。行くな。"),
                ]:
                    document = request("analyze_routed", text=control, policy="auto")
                    report["controls"].append({"id": case_id, "kind": "人工最小对照", "document": compact(document)})
                report["provider_final_status"] = request("provider_status")
                write_json(OUTPUT, report)
                if any(not case["language_validation"]["passed"] for case in report["cases"]):
                    raise ValueError("十段语言验收存在失败，具体范围已保存到当前报告")
            finally:
                process.stdin.close()
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=10)
    print(OUTPUT, flush=True)


if __name__ == "__main__":
    main()
