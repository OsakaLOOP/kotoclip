"""整理 experiments/sources 文集，生成逐文件验证集。

每篇默认保留不超过 1,200 个 Unicode scalar，并在句末截断，避免人工批次落在半句。
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIR = ROOT / "experiments" / "sources"
OUT = ROOT / "data" / "validation" / "sources-validation.json"


def clean(text: str) -> str:
    text = text.replace("\ufeff", "").replace("\r\n", "\n").replace("\r", "\n")
    text = text.replace("\t", " ")
    text = "".join(ch for ch in text if ch == "\n" or ch == " " or ord(ch) >= 0x20)
    lines = [re.sub(r" +$", "", line) for line in text.split("\n")]
    return "\n".join(lines).strip()


def cut_sentence(text: str, limit: int) -> tuple[str, bool]:
    if len(text) <= limit:
        return text, False
    window = text[:limit]
    marks = [m.end() for m in re.finditer(r"[。？！!?]", window)]
    if marks:
        return window[: marks[-1]], True
    return window, True


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--limit", type=int, default=1200)
    parser.add_argument("--output", type=Path, default=OUT)
    args = parser.parse_args()
    files = sorted(SOURCE_DIR.glob("*.txt"), key=lambda path: path.name)
    if not files:
        raise SystemExit(f"未找到文集：{SOURCE_DIR}")
    segments = []
    for path in files:
        text, truncated = cut_sentence(clean(path.read_text(encoding="utf-8")), args.limit)
        if not text:
            raise SystemExit(f"文集为空：{path.name}")
        segments.append({
            "id": f"source-{path.stem}",
            "source_file": str(path.relative_to(ROOT)).replace("\\", "/"),
            "provider": "unidic-cwj-202512",
            "text": text,
            "characters": len(text),
            "truncated": truncated,
            "independent_gold": path.stem.lower() == "collective",
        })
    payload = {
        "schema": "kotoclip.external-source-validation.v1",
        "coordinate_system": "unicode_scalar",
        "source_directory": "experiments/sources",
        "limit_per_source": args.limit,
        "segments": segments,
        "gold_status": "pending_manual_annotation",
        "gold_file": "data/validation/sources-validation.gold.json",
        "gold_method": "interactive_layered_review",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "segments": len(segments), "characters": sum(x["characters"] for x in segments), "independent_gold": [x["id"] for x in segments if x["independent_gold"]]}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
