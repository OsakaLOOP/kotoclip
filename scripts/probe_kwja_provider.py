"""运行 KWJA 外部 provider，并保存可复核的结构跨度与原始输出。"""
from __future__ import annotations

import json
import os
import sys
import argparse
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data" / "validation" / "unidic-2000.json"
OUT = ROOT / "experiments" / "kwja-provider-validation.json"


def parse_document(text: str, raw: str) -> dict[str, Any]:
    tokens: list[dict[str, Any]] = []
    bunsetsu: list[dict[str, Any]] = []
    sentences: list[list[int]] = []
    features: list[str] = []
    cursor = 0
    sentence_start = 0
    bun_start: int | None = None
    bun_id = 0
    for line in raw.splitlines():
        if line.startswith("# S-ID:"):
            if sentence_start < cursor:
                sentences.append([sentence_start, cursor])
            sentence_start = cursor
        elif line.startswith("*"):
            if bun_start is not None and bun_start < cursor:
                bunsetsu.append({"id": f"b{bun_id}", "kind": "bunsetsu", "char_range": [bun_start, cursor]})
                bun_id += 1
            bun_start = cursor
        elif line == "EOS":
            if bun_start is not None and bun_start < cursor:
                bunsetsu.append({"id": f"b{bun_id}", "kind": "bunsetsu", "char_range": [bun_start, cursor]})
                bun_id += 1
                bun_start = None
            if sentence_start < cursor:
                sentences.append([sentence_start, cursor])
                sentence_start = cursor
        elif line and not line.startswith(("+", "#")):
            fields = line.split()
            if len(fields) < 2:
                continue
            surface = fields[0]
            start = text.find(surface, cursor)
            if start < 0:
                continue
            end = start + len(surface)
            cursor = end
            tokens.append({"id": f"t{len(tokens)}", "kind": "token", "char_range": [start, end], "surface": surface, "fields": fields[1:]})
            for field in fields:
                if field.startswith("<") and field.endswith(">"):
                    features.append(field)
    if sentence_start < cursor:
        sentences.append([sentence_start, cursor])
    return {"tokens": tokens, "bunsetsu": bunsetsu, "sentences": sentences, "features": sorted(set(features)), "raw": raw}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", type=Path, default=ROOT / "data" / "validation" / "unidic-2000.json")
    parser.add_argument("--output", type=Path, default=OUT)
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    os.environ.setdefault("KWJA_CLI_MODE", "1")
    os.environ.setdefault("HF_HUB_OFFLINE", "1")
    os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
    # KWJA 的资源文件统一采用 UTF-8；Path.read_text() 在 Windows 日文区域
    # 会默认使用 cp932，导致 grammar.json 等资源在模型初始化阶段失败。
    from pathlib import Path as _Path
    _read_text = _Path.read_text
    def read_text_utf8(self, *args, **kwargs):
        if "encoding" not in kwargs and len(args) < 1:
            kwargs["encoding"] = "utf-8"
        return _read_text(self, *args, **kwargs)
    _Path.read_text = read_text_utf8
    # KWJA 的词表文件是 UTF-8，Windows 日文区域的默认编码为 cp932。
    # 在加载模块前替换读取函数，保证实验结果与系统区域无关。
    import kwja.utils.reading_prediction as reading_prediction
    def read_utf8(path):
        values = {reading_prediction.UNK: reading_prediction.UNK_ID, reading_prediction.ID: reading_prediction.ID_ID}
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line and line not in values:
                values[line] = len(values)
        return values
    reading_prediction.get_reading2reading_id = read_utf8
    from pytorch_lightning.callbacks.progress.rich_progress import RichProgressBar
    from kwja.cli.cli import CLIProcessor
    from kwja.cli.config import CLIConfig, Device, ModelSize

    payload = json.loads(args.data.read_text(encoding="utf-8"))
    texts = [segment["text"] for segment in payload["segments"]]
    config = CLIConfig(model_size=ModelSize.tiny, device=Device.cpu)
    segments = []
    for index, segment in enumerate(payload["segments"]):
        processor = CLIProcessor(config, ["senter", "char", "word"])
        processor.load_all_modules()
        for item in processor.processors:
            if item.trainer is not None:
                item.trainer.callbacks = [cb for cb in item.trainer.callbacks if not isinstance(cb, RichProgressBar)]
        raw = processor.run([segment["text"]], interactive=True)
        result = parse_document(segment["text"], raw)
        segments.append({"id": segment["id"], **{k: v for k, v in result.items() if k != "raw"}, "raw": result["raw"]})
    output = {"schema": "kotoclip.kwja-provider-validation.v1", "provider": {"id": "kwja", "version": "2.1.3", "model_size": "tiny", "device": "cpu"}, "segments": segments}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "segments": len(segments), "tokens": sum(len(x["tokens"]) for x in segments), "bunsetsu": sum(len(x["bunsetsu"]) for x in segments), "sentences": sum(len(x["sentences"]) for x in segments)}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
