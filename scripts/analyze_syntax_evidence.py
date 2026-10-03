"""汇总三方 provider 的跨度一致性、标签覆盖和关键分歧。"""
from __future__ import annotations
import json
import sys
from collections import Counter
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data" / "validation" / "unidic-2000.json"
FILES = {"unidic": ROOT / "experiments" / "unidic-provider-validation.json", "ginza": ROOT / "experiments" / "ginza-provider-validation.json", "kwja": ROOT / "experiments" / "kwja-provider-validation.json"}
OUT = ROOT / "experiments" / "syntax-evidence-report.json"

def spans(data, segment, layer):
    return {tuple(x["char_range"]) if isinstance(x, dict) else tuple(x) for x in segment.get(layer, [])}

def pair(a, b, layer):
    total_a = total_b = matched = 0
    for sa, sb in zip(a["segments"], b["segments"]):
        pa, pb = spans(a, sa, layer), spans(b, sb, layer)
        total_a += len(pa); total_b += len(pb); matched += len(pa & pb)
    p = matched / total_a if total_a else 0.0; r = matched / total_b if total_b else 0.0
    return {"left": total_a, "right": total_b, "matched": matched, "precision": p, "recall": r, "f1": 2*p*r/(p+r) if p+r else 0.0}

def main():
    sys.stdout.reconfigure(encoding="utf-8")
    raw = {name: json.loads(path.read_text(encoding="utf-8")) for name, path in FILES.items()}
    layers = {}
    for layer in ("tokens", "sentences"):
        layers[layer] = {f"{left}_vs_{right}": pair(raw[left], raw[right], layer) for left, right in (("unidic", "ginza"), ("unidic", "kwja"), ("ginza", "kwja"))}
    tag_counts = {}
    tag_counts["ginza_dependency"] = dict(Counter(t["dep"] for s in raw["ginza"]["segments"] for t in s["tokens"]))
    tag_counts["ginza_pos"] = dict(Counter(t["pos"] for s in raw["ginza"]["segments"] for t in s["tokens"]))
    tag_counts["kwja_features"] = dict(Counter(f for s in raw["kwja"]["segments"] for f in s["features"]))
    tag_counts["unidic_pos"] = dict(Counter((t.get("pos") or [None])[0] for s in raw["unidic"]["segments"] for t in s["tokens"]))
    disagreements = []
    for segment in raw["unidic"]["segments"]:
        gid = segment["id"]
        gs = next(x for x in raw["ginza"]["segments"] if x["id"] == gid)
        ks = next(x for x in raw["kwja"]["segments"] if x["id"] == gid)
        for layer in ("tokens", "sentences"):
            u, g, k = spans(raw["unidic"], segment, layer), spans(raw["ginza"], gs, layer), spans(raw["kwja"], ks, layer)
            disagreements.append({"segment": gid, "layer": layer, "unidic_only": sorted(u-g-k), "ginza_only": sorted(g-u), "kwja_only": sorted(k-u)})
    output = {"schema": "kotoclip.syntax-evidence-report.v1", "dataset": {"characters": sum(s["characters"] for s in json.loads(DATA.read_text(encoding="utf-8"))["segments"]), "gold_status": "pending_manual_annotation"}, "layers": layers, "label_richness": {key: {"distinct": len(value), "counts": value} for key, value in tag_counts.items()}, "disagreements": disagreements}
    OUT.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(OUT), "layers": layers, "distinct_labels": {k: v["distinct"] for k, v in output["label_richness"].items()}}, ensure_ascii=False, indent=2))
if __name__ == "__main__": main()
