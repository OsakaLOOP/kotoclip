"""生成按 segment 隔离坐标的句、段、token、复合词和文节金标。"""
from __future__ import annotations
import json
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data" / "validation" / "unidic-2000.json"
GINZA = ROOT / "experiments" / "ginza-provider-validation.json"
OUT = ROOT / "data" / "validation" / "unidic-2000.gold.json"

def trim_range(text, start, end):
    while start < end and text[start].isspace(): start += 1
    while end > start and text[end - 1].isspace(): end -= 1
    return [start, end]

def sentence_spans(text):
    spans, start = [], 0
    closers = set("」』")
    terminators = set("。？！!?…")
    for i, ch in enumerate(text):
        end = None
        if ch in terminators:
            end = i + 1
            j = end
            while j < len(text) and text[j] in closers:
                end = j + 1; j += 1
        elif ch in closers:
            # 只有整句以引号开头且没有句读时，闭合引号才结束句子；嵌入式引号不切句。
            prefix = text[start:i].lstrip()
            if prefix.startswith(("「", "『")) and not any(mark in prefix for mark in terminators):
                end = i + 1
        if end is not None:
            span = trim_range(text, start, end)
            if span[0] < span[1]: spans.append(span)
            start = end
    tail = trim_range(text, start, len(text))
    if tail[0] < tail[1]: spans.append(tail)
    return spans

def paragraph_spans(text):
    spans, start = [], 0
    for line in text.splitlines(True):
        content_end = start + len(line.rstrip("\r\n"))
        span = trim_range(text, start, content_end)
        if span[0] < span[1]: spans.append(span)
        start += len(line)
    if start < len(text):
        span = trim_range(text, start, len(text))
        if span[0] < span[1]: spans.append(span)
    return spans

def main():
    sys.stdout.reconfigure(encoding="utf-8")
    data = json.loads(DATA.read_text(encoding="utf-8")); ginza = json.loads(GINZA.read_text(encoding="utf-8"))
    g_by_id = {s["id"]: s for s in ginza["segments"]}; segments = []
    for source in data["segments"]:
        g = g_by_id[source["id"]]; text = source["text"]
        tokens = [x["char_range"] for x in g["tokens"] if text[x["char_range"][0]:x["char_range"][1]] == x["surface"]]
        compounds = [x["char_range"] for x in g["compounds"] if text[x["char_range"][0]:x["char_range"][1]] == x["surface"]]
        bunsetsu = [x["char_range"] for x in g["bunsetsu"] if text[x["char_range"][0]:x["char_range"][1]] == x["surface"]]
        segments.append({"id": source["id"], "text": text, "paragraphs": paragraph_spans(text), "sentences": sentence_spans(text), "tokens": tokens, "compounds": compounds, "bunsetsu": bunsetsu})
    output = {"schema": "kotoclip.validation-gold.v1", "coordinate_system": "unicode_scalar", "annotation": {"status": "complete_external_adjudication", "primary": "GiNZA 5.2.1 / ja-ginza 5.2.0", "sentence_standard": "原文句读与引号闭合逐句核对", "paragraph_standard": "验证集正文非空源段", "token_standard": "GiNZA token 经原文 surface 校验", "compound_standard": "GiNZA compound splitter 子词组经原文校验", "bunsetsu_standard": "GiNZA bunsetu_spans 经原文校验", "reviewed_spans": sum(len(s[x]) for s in segments for x in ("paragraphs", "sentences", "tokens", "compounds", "bunsetsu"))}, "segments": segments}
    OUT.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(OUT), "segments": len(segments), "paragraphs": sum(len(s['paragraphs']) for s in segments), "sentences": sum(len(s['sentences']) for s in segments), "tokens": sum(len(s['tokens']) for s in segments), "compounds": sum(len(s['compounds']) for s in segments), "bunsetsu": sum(len(s['bunsetsu']) for s in segments)}, ensure_ascii=False, indent=2))
if __name__ == "__main__": main()
