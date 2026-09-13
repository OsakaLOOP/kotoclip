#!/usr/bin/env python3
"""测量本地 GiNZA/KWJA 安装的组件、配置和参数规模。"""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import sys


def _module_path(name: str) -> str | None:
    spec = importlib.util.find_spec(name)
    return str(spec.origin) if spec and spec.origin else None


def _count_torch(path: Path) -> dict[str, int] | None:
    try:
        import torch
    except ImportError:
        return None
    try:
        state = torch.load(path, map_location="cpu", weights_only=True)
    except Exception:
        # checkpoint 来自本地已审计目录；KWJA 旧格式包含 OmegaConf 配置，
        # 仅在安全加载失败后读取其 Python pickle 容器，不执行训练代码。
        state = torch.load(path, map_location="cpu", weights_only=False)
    values = state.get("state_dict", state) if isinstance(state, dict) else {}
    tensors = [value for value in values.values() if hasattr(value, "numel")]
    return {"parameters": int(sum(value.numel() for value in tensors)), "tensors": len(tensors), "bytes_fp32": int(sum(value.numel() * value.element_size() for value in tensors))}


def measure_ginza(model_dir: Path | None) -> dict[str, object]:
    result: dict[str, object] = {"python_module": _module_path("spacy"), "model_dir": str(model_dir) if model_dir else None}
    if model_dir:
        for name in ("meta.json", "config.cfg"):
            path = model_dir / name
            if path.is_file():
                result[name] = path.stat().st_size
        meta = model_dir / "meta.json"
        if meta.is_file():
            result["meta"] = json.loads(meta.read_text(encoding="utf-8"))
        result["directory_bytes"] = sum(path.stat().st_size for path in model_dir.rglob("*") if path.is_file())
    return result


def measure_kwja(checkpoint_dir: Path | None) -> dict[str, object]:
    result: dict[str, object] = {"python_module": _module_path("kwja"), "checkpoint_dir": str(checkpoint_dir) if checkpoint_dir else None, "checkpoints": []}
    if checkpoint_dir:
        for path in sorted(checkpoint_dir.rglob("*.ckpt")):
            item: dict[str, object] = {"path": str(path), "bytes": path.stat().st_size}
            try:
                item["torch"] = _count_torch(path)
            except Exception as error:  # checkpoint formats differ across versions
                item["load_error"] = f"{type(error).__name__}: {error}"
            result["checkpoints"].append(item)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ginza-model", type=Path)
    parser.add_argument("--kwja-checkpoints", type=Path)
    parser.add_argument("--output", type=Path, default=Path("experiments/unidic-nlp-architecture.json"))
    args = parser.parse_args()
    result = {"schema": "kotoclip.unidic-nlp-architecture.v1", "python": sys.version, "ginza": measure_ginza(args.ginza_model), "kwja": measure_kwja(args.kwja_checkpoints)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
