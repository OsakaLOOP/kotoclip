"""合并各文集的交互式人工金标，并保留每批原始提交记录。"""
from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path

from build_unidic_gold import sentence_spans


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("data/validation/sources-validation.gold.json"))
    parser.add_argument("--collective", type=Path, default=Path("data/validation/collective.gold.json"))
    parser.add_argument("--data", type=Path, default=Path("data/validation/sources-validation.json"))
    parser.add_argument("--source", action="append", required=True, help="交互式人工金标文件，可重复")
    args = parser.parse_args()

    documents = [json.loads(Path(path).read_text(encoding="utf-8")) for path in [*args.source, str(args.collective)]]
    segments = [segment for document in documents for segment in document.get("segments", [])]
    logs = [entry for document in documents for entry in document.get("review_log", [])]
    segments.sort(key=lambda item: item["id"])
    logs.sort(key=lambda item: (item["segment"], item["batch"]))
    text_by_id = {segment["id"]: segment["text"] for segment in segments}
    for log in logs:
        sentence_layer = log.get("layers", {}).get("sentences")
        if sentence_layer is not None:
            start, end = log["span"]
            sentence_layer.setdefault("provider_ranges", {})["local"] = [
                span for span in sentence_spans(text_by_id[log["segment"]]) if span[0] < end and start < span[1]
            ]
    segment_status = {segment["id"]: "complete_manual_interactive" for segment in segments}
    output = {
        "schema": "kotoclip.external-source-gold.v1",
        "coordinate_system": "unicode_scalar",
        "annotation": {
            "status": "complete_manual_interactive",
            "operator": "codex-human-review",
            "reviewed_at": datetime.now(timezone.utc).isoformat(),
            "batch_size": "5-10",
            "primary_external": ["ginza 5.2.1", "kwja 2.1.3", "unidic 2025.12"],
            "independent_manual_segments": ["source-collective"],
            "method": "每批固定 5--10 句，逐层阅读三方候选；可选择 GiNZA、KWJA、UniDic、local 或直接 Unicode scalar 范围，保存原始一行提交、最终范围、surface 和 provider 对照。",
            "review_batches": len(logs),
            "reviewed_spans": sum(len(segment.get(layer, [])) for segment in segments for layer in ("paragraphs", "sentences", "tokens", "compounds", "bunsetsu")),
            "segment_status": segment_status,
        },
        "segments": segments,
        "review_log": logs,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    data = json.loads(args.data.read_text(encoding="utf-8"))
    data["gold_status"] = "complete_manual_interactive"
    data["gold_method"] = "每批固定 5--10 句，逐层阅读三方候选；collective 作为非同源独立对照，其余文集同样执行人工 provider 选择和范围复核。"
    args.data.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "segments": len(segments), "batches": len(logs), "sentences": sum(len(s.get("sentences", [])) for s in segments)}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
