"""在固定 2,000 字验证集上采集 GiNZA 结构和依存结果。"""
from __future__ import annotations
import json
import sys
import argparse
from pathlib import Path
import ginza
import spacy

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "experiments" / "ginza-provider-validation.json"

def collect(nlp, segment):
    text = segment["text"]
    doc = nlp(text)
    tokens = [{"id": f"t{i}", "kind": "token", "char_range": [t.idx, t.idx + len(t.text)], "surface": t.text, "dep": t.dep_, "head": t.head.i, "pos": t.pos_} for i, t in enumerate(doc)]
    compounds = []
    for i, t in enumerate(doc):
        entry = doc.user_data.get("sub_tokens", [])[i] if i < len(doc.user_data.get("sub_tokens", [])) else []
        sub = (entry or [])[0] if entry else []
        if sub:
            compounds.append({"id": f"c{i}", "kind": "compound", "char_range": [t.idx, t.idx + len(t.text)], "surface": t.text, "parts": [x.surface for x in sub]})
    bunsetsu = [{"id": f"b{i}", "kind": "bunsetsu", "char_range": [s.start_char, s.end_char], "surface": s.text} for i, s in enumerate(ginza.bunsetu_spans(doc))]
    return {"id": segment["id"], "tokens": tokens, "compounds": compounds, "bunsetsu": bunsetsu, "sentences": [[s.start_char, s.end_char] for s in doc.sents], "features": sorted(set(t.dep_ for t in doc))}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", type=Path, default=ROOT / "data" / "validation" / "unidic-2000.json")
    parser.add_argument("--output", type=Path, default=OUT)
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    data = json.loads(args.data.read_text(encoding="utf-8"))
    nlp = spacy.load("ja_ginza")
    output = {"schema": "kotoclip.ginza-provider-validation.v1", "provider": {"id": "ginza", "version": "5.2.1", "model": "ja_ginza 5.2.0", "license": "MIT"}, "segments": [collect(nlp, s) for s in data["segments"]]}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "tokens": sum(len(s["tokens"]) for s in output["segments"]), "compounds": sum(len(s["compounds"]) for s in output["segments"]), "bunsetsu": sum(len(s["bunsetsu"]) for s in output["segments"]), "sentences": sum(len(s["sentences"]) for s in output["segments"])}, ensure_ascii=False, indent=2))

if __name__ == "__main__":
    main()
