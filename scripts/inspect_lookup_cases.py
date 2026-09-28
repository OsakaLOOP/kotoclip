"""实测助词、作者注音和表记／读音查询，保存词典索引与服务输出。"""
import argparse
import json
from pathlib import Path
import sqlite3
import subprocess
from time import perf_counter

ROOT = Path(__file__).resolve().parents[1]
CASES = [
    "いつの間にか、けしきもない。",
    "一中節《いっちゅうぶし》。宇治紫暁《うじしぎょう》。本卦《ほんけ》返《がえ》り。",
    "橋場。橋場《はしば》。軒《けん》。戸外《そと》。内外《うち》。",
    "ありそうなもの。読みませんでした。",
]


def compact_entry(entry):
    return {key: entry.get(key) for key in (
        "dict_name", "occurrence_id", "headword", "reading", "entry_kind", "match_evidence"
    )}


def compact_lookup(value):
    result = {key: value.get(key) for key in ("query", "reading", "selected_form_id", "forms")}
    result["entries"] = [compact_entry(entry) for entry in value.get("entries", [])]
    return result


def inspect(output, binary):
    process = subprocess.Popen(
        [str(binary), "stdio"], cwd=ROOT,
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding="utf-8",
    )

    def request(command, **payload):
        process.stdin.write(json.dumps({"command": command, **payload}, ensure_ascii=False) + "\n")
        process.stdin.flush()
        response = json.loads(process.stdout.readline())
        if response.get("error"):
            raise RuntimeError(response["error"])
        return response["result"]

    report = {"cases": [], "index": {}}
    try:
        for source in CASES:
            document = request("analyze", text=source, register="cwj")
            started = perf_counter()
            group = request("lookup_targets", analysis_id=document["id"], range=[0, len(document["text"])])
            targets_ms = (perf_counter() - started) * 1000
            case = {"source": source, "text": document["text"], "ruby": document["ruby_validations"],
                    "tokens": [{key: token.get(key) for key in ("surface", "pos", "reading", "query_forms", "char_range")}
                               for token in document["morphemes"]], "targets_ms": targets_ms, "targets": []}
            for target in group["outer_targets"]:
                started = perf_counter()
                lookup = request("query_target", analysis_id=document["id"], target_id=target["id"])
                item = {"surface": target["surface"], "range": target["char_range"],
                        "reason": target["reason"], "forms": target["lookup_forms"], "lookup": compact_lookup(lookup),
                        "query_with_targets_ms": (perf_counter() - started) * 1000}
                item["alternatives"] = [compact_lookup(request(
                    "query_target", analysis_id=document["id"], target_id=target["id"], selected_form=form["form_id"]
                )) for form in lookup["forms"] if form["display_form"] in {"羽柴", "外", "内", "本卦帰り", "本卦還り"}
                    and form["form_id"] != lookup["selected_form_id"]]
                case["targets"].append(item)
            report["cases"].append(case)
            print("已采集：", source, flush=True)
        for path in sorted((ROOT / "data/dicts").glob("*.db")):
            connection = sqlite3.connect(path.as_uri() + "?mode=ro", uri=True)
            try:
                if not connection.execute("SELECT name FROM sqlite_master WHERE name='entry_keys'").fetchone():
                    continue
                queries = {}
                for key in ["橋場", "羽柴", "はしば", "軒", "けん", "戸外", "そと", "内外", "うち"]:
                    rows = connection.execute(
                        "SELECT k.kind, k.normalized_value, e.headword, k.display_value FROM entry_keys k JOIN entries e ON e.id=k.entry_id WHERE k.normalized_value IN (?, ?)",
                        (key, ''.join(chr(ord(c) + 0x60) if 'ぁ' <= c <= 'ゖ' else c for c in key)),
                    ).fetchall()
                    queries[key] = rows
                report["index"][path.name] = queries
            finally:
                connection.close()
    finally:
        process.stdin.close()
        process.wait(timeout=30)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    for case in report["cases"]:
        print(case["source"])
        for target in case["targets"]:
            lookup = target["lookup"]
            print(" ", target["surface"], lookup["selected_form_id"],
                  [(form["display_form"], form["readings"]) for form in lookup["forms"]][:12])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=ROOT / "target-lookup-review/debug/kotoclip-nlp.exe")
    args = parser.parse_args()
    inspect(args.output, args.binary)
