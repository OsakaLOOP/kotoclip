"""通过应用适配器采集 GiNZA 完整来源结果及评估跨度。"""
import argparse
import contextlib
import json
from pathlib import Path
import sys

from nlp_adapters import GinzaAdapter, validation_segment

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", type=Path, default=ROOT / "data/validation/unidic-2000.json")
    parser.add_argument("--output", type=Path, default=ROOT / "experiments/ginza-provider-validation.json")
    parser.add_argument("--model", default="ja_ginza")
    args = parser.parse_args()
    data = json.loads(args.data.read_text(encoding="utf-8"))
    with contextlib.redirect_stdout(sys.stderr):
        adapter = GinzaAdapter(args.model)
        segments = [validation_segment(segment, adapter.analyze(segment["text"])) for segment in data["segments"]]
    output = {"schema": "kotoclip.provider-validation.v2", "provider": adapter.manifest, "segments": segments}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "segments": len(segments)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
