#!/usr/bin/env python3
"""使用已张量化的 UniDic batch 训练结构库或语义库。"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nlp_retraining.contracts import sha256_file, validate_jsonl
from nlp_retraining.models import SemanticModel, StructureModel
from nlp_retraining.trainer import save_checkpoint, train_one_epoch


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--data", type=Path, required=True, help="通过张量化的 .pt batch 列表")
    parser.add_argument("--contract", type=Path, required=True, help="用于记录来源 hash 的 JSONL")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--model", choices=("structure", "semantic"), required=True)
    parser.add_argument("--device", default="cpu")
    args = parser.parse_args()
    try:
        import torch
    except ImportError as error:
        raise SystemExit("训练入口需要 PyTorch；数据校验可使用标准库脚本完成") from error
    config = json.loads(args.config.read_text(encoding="utf-8"))
    count, errors = validate_jsonl(args.contract)
    if errors:
        raise SystemExit("contract validation failed: " + errors[0])
    architecture = config["architecture"]
    cls = StructureModel if args.model == "structure" else SemanticModel
    model = cls(vocab_size=int(architecture["vocab_size"]), hidden_size=int(architecture["hidden_size"]), layers=int(architecture["layers"]), heads=int(architecture["attention_heads"]))
    batches = torch.load(args.data, map_location="cpu", weights_only=True)
    optimizer = torch.optim.AdamW(model.parameters(), lr=float(config["training"]["learning_rate"]), weight_decay=float(config["training"].get("weight_decay", 0.01)))
    loss = train_one_epoch(model, batches, optimizer, loss_weights=config.get("loss_weights"), device=args.device)
    manifest = {"schema": "kotoclip.unidic-checkpoint.v1", "model_id": config["model_id"], "model_kind": args.model, "config_sha256": hashlib.sha256(args.config.read_bytes()).hexdigest(), "contract_sha256": sha256_file(args.contract), "contract_documents": count, "parameters": sum(parameter.numel() for parameter in model.parameters()), "loss": loss}
    save_checkpoint(args.output, model, manifest=manifest)
    print(json.dumps(manifest, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
