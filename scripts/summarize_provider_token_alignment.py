"""统计外部 provider token 映射到 UniDic token 的可行性。"""
from __future__ import annotations

import argparse
import json
from pathlib import Path


def ranges(items: list[dict]) -> list[tuple[int, int]]:
    return [(int(item["char_range"][0]), int(item["char_range"][1])) for item in items]


def classify(external: tuple[int, int], unidic: list[tuple[int, int]]) -> str:
    start, end = external
    contained = [item for item in unidic if start <= item[0] and item[1] <= end]
    overlap = [item for item in unidic if item[0] < end and start < item[1]]
    if any(item == external for item in contained):
        return "exact"
    if contained and contained[0][0] == start and contained[-1][1] == end:
        return "compound"
    if overlap:
        return "partial"
    return "unmatched"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--unidic", type=Path, required=True)
    parser.add_argument("--provider", type=Path, required=True)
    args = parser.parse_args()
    base = json.loads(args.unidic.read_text(encoding="utf-8"))
    provider = json.loads(args.provider.read_text(encoding="utf-8"))
    by_id = {segment["id"]: segment for segment in base["segments"]}
    counts: dict[str, int] = {key: 0 for key in ("exact", "compound", "partial", "unmatched")}
    segments = []
    for segment in provider["segments"]:
        source = by_id[segment["id"]]
        unidic = ranges(source.get("tokens", []))
        local = {key: 0 for key in counts}
        for item in segment.get("tokens", []):
            status = classify(tuple(item["char_range"]), unidic)
            local[status] += 1
            counts[status] += 1
        segments.append({"id": segment["id"], "tokens": sum(local.values()), "counts": local})
    total = sum(counts.values())
    mapped = counts["exact"] + counts["compound"]
    print(json.dumps({"provider": provider.get("provider", {}).get("id"), "total": total, "counts": counts,
                      "fully_mappable_ratio": mapped / total if total else 0.0, "segments": segments},
                     ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
