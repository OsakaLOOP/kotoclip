"""通过应用适配器复用 KWJA 模型，采集完整来源结果及评估跨度。"""
import argparse
import contextlib
import json
import os
from pathlib import Path
import sys

from nlp_adapters import KwjaAdapter, validation_segment

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", type=Path, default=ROOT / "data/validation/unidic-2000.json")
    parser.add_argument("--output", type=Path, default=ROOT / "experiments/kwja-provider-validation.json")
    parser.add_argument("--model", default="tiny", choices=["tiny", "base", "large"])
    parser.add_argument("--kwja-cache", type=Path, default=ROOT / "experiments/kwja-cache")
    parser.add_argument("--hf-cache", type=Path, default=ROOT / "experiments/hf-cache")
    args = parser.parse_args()
    os.environ.update(KWJA_CLI_MODE="1", HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1", KWJA_CACHE_DIR=str(args.kwja_cache), HF_HOME=str(args.hf_cache))
    data = json.loads(args.data.read_text(encoding="utf-8"))
    with contextlib.redirect_stdout(sys.stderr):
        adapter = KwjaAdapter(args.model)
        try:
            segments = [validation_segment(segment, adapter.analyze(segment["text"])) for segment in data["segments"]]
        finally:
            adapter.close()
    output = {"schema": "kotoclip.provider-validation.v2", "provider": adapter.manifest, "segments": segments}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "segments": len(segments)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
