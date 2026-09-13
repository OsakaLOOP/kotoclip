"""UniDic 模型的最小可复用训练循环和 checkpoint 写入器。"""

from __future__ import annotations

from typing import Any, Iterable, Mapping

try:
    import torch
    from torch import Tensor, nn
except ImportError:  # pragma: no cover
    torch = None  # type: ignore[assignment]
    Tensor = object  # type: ignore[misc,assignment]
    nn = object  # type: ignore[assignment]


def _require_torch() -> None:
    if torch is None:
        raise RuntimeError("training requires PyTorch")


def multitask_loss(outputs: Mapping[str, Tensor], targets: Mapping[str, Tensor], weights: Mapping[str, float] | None = None) -> Tensor:
    """对 token 分类和二维 relation logits 计算带权交叉熵。"""
    _require_torch()
    losses = []
    for name, target in targets.items():
        if name not in outputs:
            continue
        logits = outputs[name]
        if logits.ndim == target.ndim + 1:
            value = nn.functional.cross_entropy(logits.reshape(-1, logits.size(-1)), target.reshape(-1), ignore_index=-100)
        elif logits.ndim == target.ndim:
            value = nn.functional.binary_cross_entropy_with_logits(logits, target.float())
        else:
            raise ValueError(f"task {name} logits/target dimensions are incompatible: {tuple(logits.shape)} / {tuple(target.shape)}")
        weight_name = name.removesuffix("_logits")
        weight_map = weights or {}
        weight = weight_map.get(weight_name, weight_map.get(name, 1.0))
        losses.append(value * float(weight))
    if not losses:
        raise ValueError("batch contains no matching task targets")
    return sum(losses)


def train_one_epoch(model: nn.Module, batches: Iterable[Mapping[str, Any]], optimizer: Any, *, loss_weights: Mapping[str, float] | None = None, device: str = "cpu") -> float:
    _require_torch()
    model.to(device).train()
    total = 0.0
    count = 0
    for batch in batches:
        features = {key: value.to(device) for key, value in batch["features"].items()}
        targets = {key: value.to(device) for key, value in batch["targets"].items()}
        optimizer.zero_grad(set_to_none=True)
        loss = multitask_loss(model(features), targets, loss_weights)
        loss.backward()
        optimizer.step()
        total += float(loss.detach().cpu())
        count += 1
    return total / count if count else 0.0


def save_checkpoint(path: Any, model: nn.Module, *, manifest: Mapping[str, Any]) -> None:
    _require_torch()
    path.parent.mkdir(parents=True, exist_ok=True)
    torch.save({"manifest": dict(manifest), "state_dict": model.state_dict()}, path)
