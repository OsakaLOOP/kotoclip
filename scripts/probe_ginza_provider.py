"""采集 GiNZA 独立 provider 结果，供结构对齐和人工审阅使用。"""
from __future__ import annotations
import json
import sys
from pathlib import Path
import ginza
import spacy

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "experiments" / "ginza-provider-sample.json"
TEXT = "太郎は本を買った。彼は家でそれを読んだ。情報処理技術者試験を受験する。"

def main() -> None:
    sys.stdout.reconfigure(encoding="utf-8")
    nlp = spacy.load("ja_ginza")
    doc = nlp(TEXT)
    tokens = [{"id": f"t{i}", "kind": "token", "char_range": [t.idx, t.idx + len(t.text)], "surface": t.text, "dep": t.dep_, "head": t.head.i} for i, t in enumerate(doc)]
    compounds = []
    for i, t in enumerate(doc):
        sub = doc.user_data.get("sub_tokens", [])[i][0] if i < len(doc.user_data.get("sub_tokens", [])) else []
        if sub:
            compounds.append({"id": f"c{i}", "kind": "compound", "char_range": [t.idx, t.idx + len(t.text)], "surface": t.text, "parts": [x.surface for x in sub]})
    bunsetsu = [{"id": f"b{i}", "kind": "bunsetsu", "char_range": [s.start_char, s.end_char], "surface": s.text} for i, s in enumerate(ginza.bunsetu_spans(doc))]
    payload = {"schema": "kotoclip.ginza-provider-sample.v1", "provider": {"id": "ginza", "version": "5.2.1", "model": "ja_ginza 5.2.0", "license": "MIT"}, "text": TEXT, "text_characters": len(TEXT), "spans": tokens + compounds + bunsetsu, "sentences": [[s.start_char, s.end_char] for s in doc.sents]}
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(OUT), "tokens": len(tokens), "compounds": len(compounds), "bunsetsu": len(bunsetsu), "sentences": len(payload["sentences"])}, ensure_ascii=False, indent=2))

if __name__ == "__main__":
    main()
