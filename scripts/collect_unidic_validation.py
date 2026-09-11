"""采集当前 UniDic 分词和本地结构候选，供外部 provider 对照。"""
from __future__ import annotations
import json
import subprocess
import sys
import argparse
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data" / "validation" / "unidic-2000.json"
BIN = ROOT / "target" / "debug" / "kotoclip-nlp.exe"
OUT = ROOT / "experiments" / "unidic-provider-validation.json"

def collect(segment):
    register = "csj" if segment["id"].startswith("csj") else "cwj"
    proc = subprocess.run([str(BIN), "inspect", register, segment["text"]], capture_output=True, text=True, encoding="utf-8", check=True)
    value = json.loads(proc.stdout)
    tokens = [{"id": t["id"], "kind": "token", "char_range": t["char_range"], "surface": t["surface"], "pos": t.get("pos"), "lemma": t.get("lemma")} for t in value["morphemes"]]
    return {"id": segment["id"], "tokens": tokens, "paragraphs": value["structure"]["paragraphs"], "sentences": value["structure"]["sentences"], "clauses": value["structure"]["clauses"], "characters": value["characters"], "routing": value["routing"]}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", type=Path, default=ROOT / "data" / "validation" / "unidic-2000.json")
    parser.add_argument("--output", type=Path, default=OUT)
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    data = json.loads(args.data.read_text(encoding="utf-8"))
    output = {"schema": "kotoclip.unidic-provider-validation.v1", "provider": {"id": "unidic", "version": "2025.12", "status": "candidate_structure"}, "segments": [collect(s) for s in data["segments"]]}
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "tokens": sum(len(s["tokens"]) for s in output["segments"]), "sentences": sum(len(s["sentences"]) for s in output["segments"]), "clauses": sum(len(s["clauses"]) for s in output["segments"])}, ensure_ascii=False, indent=2))
if __name__ == "__main__": main()
