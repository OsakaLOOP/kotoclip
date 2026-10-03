"""比较 GiNZA 与 KWJA 的外部跨度结果，输出一致性和分歧样本。"""
from __future__ import annotations
import json
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
GINZA = ROOT / "experiments" / "ginza-provider-validation.json"
KWJA = ROOT / "experiments" / "kwja-provider-validation.json"
OUT = ROOT / "experiments" / "syntax-provider-comparison.json"

def main():
    sys.stdout.reconfigure(encoding="utf-8")
    g = json.loads(GINZA.read_text(encoding="utf-8")); k = json.loads(KWJA.read_text(encoding="utf-8"))
    by_id = {x["id"]: x for x in k["segments"]}; layers = {}
    disagreements = []
    for layer in ("tokens", "bunsetsu", "sentences"):
        total_g = total_k = matched = 0
        for gs in g["segments"]:
            ks = by_id[gs["id"]]
            gp = {tuple(x["char_range"]) if isinstance(x, dict) else tuple(x) for x in gs[layer]}
            kp = {tuple(x["char_range"]) if isinstance(x, dict) else tuple(x) for x in ks[layer]}
            total_g += len(gp); total_k += len(kp); matched += len(gp & kp)
            if gp != kp:
                disagreements.append({"segment": gs["id"], "layer": layer, "ginza_only": sorted(gp-kp), "kwja_only": sorted(kp-gp)})
        p = matched / total_k if total_k else 0.0; r = matched / total_g if total_g else 0.0
        layers[layer] = {"ginza": total_g, "kwja": total_k, "matched": matched, "precision_vs_ginza": p, "recall_vs_ginza": r, "f1_vs_ginza": 2*p*r/(p+r) if p+r else 0.0}
    output = {"schema": "kotoclip.syntax-provider-comparison.v1", "providers": [g["provider"], k["provider"]], "layers": layers, "disagreements": disagreements}
    OUT.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(OUT), "layers": layers, "disagreement_count": len(disagreements)}, ensure_ascii=False, indent=2))

if __name__ == "__main__": main()
