"""按批次人工审阅验证集，使用一行分隔字符串生成分层金标。

输入格式：S=g;T=g;C=g;B=g 选择各层 GiNZA 候选；`k`、`u` 分别选择
KWJA、UniDic 候选，`g:1-3` 选择指定 provider 的锚点，`@起点-终点`
提交人工范围。每批固定 5--10 句，范围均使用 Unicode scalar 坐标。
"""
from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from build_unidic_gold import paragraph_spans, sentence_spans

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data" / "validation" / "unidic-2000.json"
GINZA = ROOT / "experiments" / "ginza-provider-validation.json"
KWJA = ROOT / "experiments" / "kwja-provider-validation.json"
UNIDIC = ROOT / "experiments" / "unidic-provider-validation.json"
OUT = ROOT / "data" / "validation" / "unidic-2000.gold.json"
LAYERS = ("sentences", "tokens", "compounds", "bunsetsu")
SHORT = {"S": "sentences", "T": "tokens", "C": "compounds", "B": "bunsetsu"}


def span_of(item: Any) -> list[int]:
    value = item.get("char_range") if isinstance(item, dict) else item
    if not isinstance(value, (list, tuple)) or len(value) != 2:
        raise ValueError(f"非法范围: {item!r}")
    return [int(value[0]), int(value[1])]


def checked_ranges(items: Any, text: str, *, label: str) -> list[list[int]]:
    if not isinstance(items, list):
        raise ValueError(f"{label} 必须是范围数组")
    result: list[list[int]] = []
    seen: set[tuple[int, int]] = set()
    for item in items:
        start, end = span_of(item)
        if start < 0 or end > len(text) or start >= end:
            raise ValueError(f"{label} 范围越界: {[start, end]}")
        while start < end and text[start].isspace():
            start += 1
        while end > start and text[end - 1].isspace():
            end -= 1
        if start >= end:
            continue
        if (start, end) not in seen:
            result.append([start, end])
            seen.add((start, end))
    return result


def provider_items(items: list[Any], text: str) -> list[Any]:
    result = []
    for item in items:
        try:
            checked_ranges([item], text, label="provider")
        except ValueError:
            continue
        result.append(item)
    return result


def intersect(items: list[Any], start: int, end: int) -> list[list[int]]:
    result = []
    for item in items:
        a, b = span_of(item)
        if a < end and start < b:
            result.append([a, b])
    return result


def render(items: list[Any], text: str, start: int, end: int) -> str:
    spans = intersect(items, start, end)
    return " / ".join(text[a:b] for a, b in spans) or "(无)"


def render_anchored(label: str, spans: list[list[int]], text: str) -> str:
    if not spans:
        return f"  {label}: (无)"
    return "  " + " ".join(f"{label}{i}=[{a},{b}){text[a:b]}" for i, (a, b) in enumerate(spans, 1))


def render_provider(label: str, spans: list[list[int]], text: str) -> str:
    if not spans:
        return f"  {label}: (无)"
    return "  " + " ".join(f"{label}{i}=[{a},{b}){text[a:b]}" for i, (a, b) in enumerate(spans, 1))


def parse_selection(value: str, candidates: dict[str, list[list[int]]], text: str, label: str) -> tuple[str, list[list[int]]]:
    value = value.strip().lower()
    if value in {"-", "r", "reject"}:
        return "reject", []
    provider = "ginza"
    if value in {"g", "g1", "ginza", "k", "kwja", "u", "unidic", "l", "local"}:
        provider = {"g1": "ginza", "g": "ginza", "ginza": "ginza", "k": "kwja", "kwja": "kwja", "u": "unidic", "unidic": "unidic", "l": "local", "local": "local"}[value]
        return "accept", checked_ranges(candidates.get(provider, []), text, label=label)
    if ":" in value and value.split(":", 1)[0] in {"g", "ginza", "k", "kwja", "u", "unidic", "l", "local", "e"}:
        prefix, value = value.split(":", 1)
        provider = {"g": "ginza", "ginza": "ginza", "k": "kwja", "kwja": "kwja", "u": "unidic", "unidic": "unidic", "l": "local", "local": "local", "e": "ginza"}[prefix]
    anchors = candidates.get(provider, [])
    decision = "edit"
    selected: list[list[int]] = []
    for token in value.split(","):
        token = token.strip()
        if not token:
            continue
        if token.startswith("@"):
            raw = token[1:].replace("-", ",", 1)
            parts = [part.strip() for part in raw.split(",")]
            if len(parts) != 2:
                raise ValueError(f"{label} 直接范围应为 @起点-终点")
            selected.append([int(parts[0]), int(parts[1])])
            continue
        if "-" in token:
            left, right = token.split("-", 1)
            indexes = range(int(left), int(right) + 1)
        else:
            indexes = (int(token),)
        for index in indexes:
            if index < 1 or index > len(anchors):
                raise ValueError(f"{label} 锚点 {index} 超出 1..{len(anchors)}")
            selected.append(anchors[index - 1])
    return decision, checked_ranges(selected, text, label=label)


def parse_line(line: str, candidates: dict[str, dict[str, list[list[int]]]], text: str) -> dict[str, tuple[str, list[list[int]]]]:
    values = {key: "-" for key in SHORT}
    for part in line.split(";"):
        if not part.strip():
            continue
        if "=" not in part:
            raise ValueError("每段必须使用层级=值，例如 S=g;T=g;C=-;B=g")
        key, value = part.split("=", 1)
        key = key.strip().upper()
        if key not in SHORT:
            raise ValueError(f"未知层级 {key}，可用 S/T/C/B")
        values[key] = value
    return {SHORT[key]: parse_selection(values[key], candidates[SHORT[key]], text, SHORT[key]) for key in SHORT}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch-size", type=int, choices=(5, 6, 7, 8, 9, 10), default=5)
    parser.add_argument("--operator", default="codex-human-review")
    parser.add_argument("--data", type=Path, default=DATA)
    parser.add_argument("--ginza", type=Path, default=GINZA)
    parser.add_argument("--kwja", type=Path, default=KWJA)
    parser.add_argument("--unidic", type=Path, default=UNIDIC)
    parser.add_argument("--output", type=Path, default=OUT)
    parser.add_argument("--only", action="append", help="只审阅指定 segment，可重复")
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")

    data = json.loads(args.data.read_text(encoding="utf-8"))
    ginza = json.loads(args.ginza.read_text(encoding="utf-8"))
    kwja = json.loads(args.kwja.read_text(encoding="utf-8"))
    unidic = json.loads(args.unidic.read_text(encoding="utf-8"))
    g_by = {x["id"]: x for x in ginza["segments"]}
    k_by = {x["id"]: x for x in kwja["segments"]}
    u_by = {x["id"]: x for x in unidic["segments"]}

    segments: list[dict[str, Any]] = []
    review_log: list[dict[str, Any]] = []
    selected = set(args.only or [])
    for source in data["segments"]:
        if selected and source["id"] not in selected:
            continue
        sid, text = source["id"], source["text"]
        g, k, u = g_by[sid], k_by[sid], u_by[sid]
        g = {**g, **{layer: provider_items(g.get(layer, []), text) for layer in ("tokens", "compounds", "bunsetsu", "sentences")}}
        k = {**k, **{layer: provider_items(k.get(layer, []), text) for layer in ("tokens", "bunsetsu", "sentences")}}
        u = {**u, **{layer: provider_items(u.get(layer, []), text) for layer in ("tokens", "sentences")}}
        paragraphs = checked_ranges(paragraph_spans(text), text, label="paragraphs")
        sentence_candidates = sentence_spans(text)
        batches = [sentence_candidates[i : i + args.batch_size] for i in range(0, len(sentence_candidates), args.batch_size)]
        confirmed: dict[str, list[list[int]]] = {layer: [] for layer in LAYERS}
        for batch_index, batch in enumerate(batches, 1):
            start, end = batch[0][0], batch[-1][1]
            print(f"\n[{sid}] 批次 {batch_index}/{len(batches)}，S 锚点 1..{len(batch)}，范围 [{start},{end})")
            for number, (a, b) in enumerate(batch, 1):
                print(f"  S{number}=[{a},{b}) {text[a:b]}")
            candidates = {
                "sentences": {
                    "local": checked_ranges(batch, text, label="sentences"),
                    "ginza": checked_ranges(intersect(g["sentences"], start, end), text, label="sentences"),
                    "kwja": checked_ranges(intersect(k["sentences"], start, end), text, label="sentences"),
                    "unidic": checked_ranges(intersect(u["sentences"], start, end), text, label="sentences"),
                },
                "tokens": {
                    "ginza": checked_ranges(intersect(g["tokens"], start, end), text, label="tokens"),
                    "kwja": checked_ranges(intersect(k["tokens"], start, end), text, label="tokens"),
                    "unidic": checked_ranges(intersect(u["tokens"], start, end), text, label="tokens"),
                },
                "compounds": {
                    "ginza": checked_ranges(intersect(g["compounds"], start, end), text, label="compounds"),
                },
                "bunsetsu": {
                    "ginza": checked_ranges(intersect(g["bunsetsu"], start, end), text, label="bunsetsu"),
                    "kwja": checked_ranges(intersect(k["bunsetsu"], start, end), text, label="bunsetsu"),
                },
            }
            for provider, prefix in (("ginza", "G"), ("kwja", "K"), ("unidic", "U")):
                if provider in candidates["sentences"]:
                    print(render_provider(f"S({prefix})", candidates["sentences"][provider], text))
            for provider, prefix in (("ginza", "G"), ("kwja", "K"), ("unidic", "U")):
                print(render_provider(f"T({prefix})", candidates["tokens"].get(provider, []), text))
            print(render_provider("C(G)", candidates["compounds"]["ginza"], text))
            for provider, prefix in (("ginza", "G"), ("kwja", "K")):
                print(render_provider(f"B({prefix})", candidates["bunsetsu"].get(provider, []), text))
            print("  一行提交：S=g|k|u，T=g|k|u，C=g，B=g|k；可用 g:1-3 或 @起点-终点，- 表示拒绝")
            while True:
                line = input("  提交：").strip()
                try:
                    decisions = parse_line(line, candidates, text)
                    break
                except (ValueError, IndexError) as exc:
                    print(f"  输入无效：{exc}")
            for layer, (_, ranges) in decisions.items():
                confirmed[layer].extend(ranges)
            review_log.append({
                "segment": sid,
                "batch": batch_index,
                "size": len(batch),
                "span": [start, end],
                "submission": line,
                "layers": {
                    **{"paragraphs": {
                        "decision": "accept",
                        "ranges": checked_ranges(intersect(paragraphs, start, end), text, label="paragraphs"),
                        "surface": [text[a:b] for a, b in checked_ranges(intersect(paragraphs, start, end), text, label="paragraphs")],
                        "provider_ranges": {"source": checked_ranges(intersect(paragraphs, start, end), text, label="paragraphs")},
                    }},
                    **{layer: {
                        "decision": decisions[layer][0],
                        "ranges": decisions[layer][1],
                        "surface": [text[a:b] for a, b in decisions[layer][1]],
                        "provider_ranges": {
                            "ginza": intersect(g.get(layer, []), start, end),
                            "kwja": intersect(k.get(layer, []), start, end),
                            "unidic": intersect(u.get(layer, []), start, end),
                            "local": intersect(candidates[layer].get("local", []), start, end),
                        },
                    } for layer in LAYERS},
                },
            })
        segments.append({"id": sid, "text": text, "paragraphs": paragraphs, **{layer: confirmed[layer] for layer in LAYERS}})

    output = {
        "schema": "kotoclip.validation-gold.v1",
        "coordinate_system": "unicode_scalar",
        "annotation": {
            "status": "complete_manual_interactive",
            "operator": args.operator,
            "reviewed_at": datetime.now(timezone.utc).isoformat(),
            "batch_size": args.batch_size,
            "source_comparison": ["ginza 5.2.1", "kwja 2.1.3", "unidic 2025.12"],
            "method": "每批固定句数，显示编号锚点与三方候选，使用一行 S/T/C/B 分隔字符串提交逐层人工判断；结果校验 Unicode scalar 范围与原文 surface。",
            "review_batches": len(review_log),
            "reviewed_spans": sum(len(s[layer]) for s in segments for layer in ("paragraphs", *LAYERS)),
        },
        "segments": segments,
        "review_log": review_log,
    }
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "status": output["annotation"]["status"], "batches": len(review_log), "sentences": sum(len(s["sentences"]) for s in segments)}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
