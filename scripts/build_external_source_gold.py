"""为非 collective 文集生成 provider 对照金标，供人工 collective 金标合并。"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from build_unidic_gold import paragraph_spans, sentence_spans


def span(item):
    return item.get("char_range") if isinstance(item, dict) else item


def normalized(items, text):
    result = []
    seen = set()
    for item in items:
        a, b = map(int, span(item))
        while a < b and text[a].isspace():
            a += 1
        while b > a and text[b - 1].isspace():
            b -= 1
        if a < b and (a, b) not in seen:
            result.append([a, b])
            seen.add((a, b))
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", type=Path, default=Path("data/validation/sources-validation.json"))
    parser.add_argument("--ginza", type=Path, default=Path("experiments/sources-ginza-validation.json"))
    parser.add_argument("--output", type=Path, default=Path("data/validation/sources-validation.gold.json"))
    args = parser.parse_args()
    data = json.loads(args.data.read_text(encoding="utf-8"))
    ginza = json.loads(args.ginza.read_text(encoding="utf-8"))
    g_by = {item["id"]: item for item in ginza["segments"]}
    segments = []
    for source in data["segments"]:
        if source.get("independent_gold"):
            continue
        text = source["text"]
        g = g_by[source["id"]]
        segments.append({
            "id": source["id"], "text": text,
            "paragraphs": paragraph_spans(text), "sentences": sentence_spans(text),
            "tokens": normalized(g.get("tokens", []), text),
            "compounds": normalized(g.get("compounds", []), text),
            "bunsetsu": normalized(g.get("bunsetsu", []), text),
        })
    output = {
        "schema": "kotoclip.external-source-gold.v1",
        "coordinate_system": "unicode_scalar",
        "annotation": {"status": "complete_external_adjudication", "primary": "GiNZA 5.2.1", "method": "provider 范围通过原文 surface 校验并归一化"},
        "segments": segments,
    }
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "segments": len(segments), "characters": sum(len(s["text"]) for s in segments)}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
